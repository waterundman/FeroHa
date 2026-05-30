// LLM Router — Route prompts to local or cloud models, with fallback cascade

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio_util::sync::CancellationToken;

use super::api_client::{self, Provider};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouterConfig {
    pub llm_provider: String, // "gemini", "openai", "anthropic", "deepseek", "ollama"
    pub llm_api_key: String,
    pub llm_model: String,
    pub embedding_provider: String, // "gemini", "openai", "none"
    pub embedding_api_key: String,
    pub embedding_model: String,
    pub monthly_budget_usd: f64,
    pub temperature: f32,
    #[serde(default)]
    pub fallback_providers: Vec<Provider>,
    #[serde(default)]
    pub fallback_embedding_providers: Vec<Provider>,
    #[serde(default = "default_ollama_base_url")]
    pub ollama_base_url: String,
}

fn default_ollama_base_url() -> String {
    "http://localhost:11434".to_string()
}

impl Default for RouterConfig {
    fn default() -> Self {
        RouterConfig {
            llm_provider: "gemini".to_string(),
            llm_api_key: String::new(),
            llm_model: "gemini-2.0-flash".to_string(),
            embedding_provider: "none".to_string(),
            embedding_api_key: String::new(),
            embedding_model: "text-embedding-3-small".to_string(),
            monthly_budget_usd: 5.0,
            temperature: 0.7,
            fallback_providers: Vec::new(),
            fallback_embedding_providers: Vec::new(),
            ollama_base_url: "http://localhost:11434".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    pub text: String,
    pub model_used: String,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub cost_usd: f64,
    pub latency_ms: u64,
    #[serde(default)]
    pub used_fallback: bool,
}

/// Cost tracking
#[derive(Debug, Default, Clone)]
struct CostTracker {
    total_spent_usd: f64,
    calls: HashMap<String, u64>,
}

#[derive(Clone)]
pub struct LlmRouter {
    config: RouterConfig,
    cost_tracker: CostTracker,
}

impl LlmRouter {
    pub fn new(config: RouterConfig) -> Self {
        LlmRouter {
            config,
            cost_tracker: CostTracker::default(),
        }
    }

    pub fn is_available(&self) -> bool {
        !self.config.llm_api_key.is_empty()
    }

    pub fn update_config(&mut self, config: RouterConfig) {
        self.config = config;
    }

    pub fn config(&self) -> RouterConfig {
        self.config.clone()
    }

    /// Real HTTP completion — calls the configured LLM provider with fallback cascade
    pub async fn complete(
        &mut self,
        system_prompt: &str,
        user_prompt: &str,
        cancel_token: Option<CancellationToken>,
    ) -> Result<LlmResponse, String> {
        let full_prompt = if system_prompt.is_empty() {
            user_prompt.to_string()
        } else {
            format!("{}\n\n---\n\n{}", system_prompt, user_prompt)
        };

        let start = std::time::Instant::now();

        let primary_provider: Provider = self
            .config
            .llm_provider
            .parse()
            .map_err(|e| format!("Unknown LLM provider: {}", e))?;

        let make_call = api_client::complete(
            &primary_provider,
            &self.config.llm_api_key,
            &self.config.llm_model,
            &full_prompt,
            self.config.temperature,
            &self.config.ollama_base_url,
        );

        let result = if let Some(token) = cancel_token.as_ref() {
            tokio::select! {
                r = make_call => r,
                _ = token.cancelled() => {
                    return Err("Task cancelled by user".to_string());
                }
            }
        } else {
            make_call.await
        };

        match result {
            Ok((text, tokens_in, tokens_out)) => {
                let latency_ms = start.elapsed().as_millis() as u64;
                let cost = self.estimate_cost(tokens_in, tokens_out);
                self.cost_tracker.total_spent_usd += cost;
                *self
                    .cost_tracker
                    .calls
                    .entry(self.config.llm_model.clone())
                    .or_default() += 1;

                Ok(LlmResponse {
                    text,
                    model_used: self.config.llm_model.clone(),
                    tokens_in,
                    tokens_out,
                    cost_usd: cost,
                    latency_ms,
                    used_fallback: false,
                })
            }
            Err(e) => {
                if !is_retryable_error(&e) {
                    tracing::warn!("LLM API call failed (non-retryable): {}", e);
                    return Err(format!(
                        "LLM API error: {}. Please check your API key in Settings.",
                        e
                    ));
                }

                tracing::warn!("LLM API call failed (retryable): {}", e);

                for fallback_provider in &self.config.fallback_providers {
                    if let Some(token) = cancel_token.as_ref() {
                        if token.is_cancelled() {
                            return Err("Task cancelled by user".to_string());
                        }
                    }

                    tracing::warn!("Attempting fallback: primary -> {:?}", fallback_provider);

                    let fb_call = api_client::complete(
                        fallback_provider,
                        &self.config.llm_api_key,
                        &self.config.llm_model,
                        &full_prompt,
                        self.config.temperature,
                        &self.config.ollama_base_url,
                    );

                    let fb_result = if let Some(token) = cancel_token.as_ref() {
                        tokio::select! {
                            r = fb_call => r,
                            _ = token.cancelled() => {
                                return Err("Task cancelled by user".to_string());
                            }
                        }
                    } else {
                        fb_call.await
                    };

                    match fb_result {
                        Ok((text, tokens_in, tokens_out)) => {
                            let latency_ms = start.elapsed().as_millis() as u64;
                            let cost = self.estimate_cost(tokens_in, tokens_out);
                            self.cost_tracker.total_spent_usd += cost;
                            *self
                                .cost_tracker
                                .calls
                                .entry(self.config.llm_model.clone())
                                .or_default() += 1;

                            tracing::warn!("Fallback succeeded: {:?}", fallback_provider);

                            return Ok(LlmResponse {
                                text,
                                model_used: self.config.llm_model.clone(),
                                tokens_in,
                                tokens_out,
                                cost_usd: cost,
                                latency_ms,
                                used_fallback: true,
                            });
                        }
                        Err(fb_err) => {
                            tracing::warn!(
                                "Fallback {:?} also failed: {}",
                                fallback_provider,
                                fb_err
                            );
                        }
                    }
                }

                let mut tried = vec![self.config.llm_provider.clone()];
                tried.extend(
                    self.config
                        .fallback_providers
                        .iter()
                        .map(|p| format!("{:?}", p).to_lowercase()),
                );
                Err(format!(
                    "LLM API error: {}. All providers exhausted (tried: {})",
                    e,
                    tried.join(", ")
                ))
            }
        }
    }

    /// Synthesize RAG context + query into final answer
    pub async fn synthesize(
        &mut self,
        query: &str,
        contexts: &[String],
    ) -> Result<LlmResponse, String> {
        let system_prompt = "You are a research assistant for a note-taking app. \
            Synthesize the provided context into a concise, informative response. \
            Use Markdown formatting. Cite sources where possible.";

        let context_text = contexts
            .iter()
            .enumerate()
            .map(|(i, c)| format!("[Source {}]\n{}", i + 1, c))
            .collect::<Vec<_>>()
            .join("\n\n");

        let user_prompt = format!(
            "Context:\n{}\n\nQuestion: {}\n\nProvide a clear, well-organized answer.",
            context_text, query
        );

        self.complete(system_prompt, &user_prompt, None).await
    }

    pub async fn embed_texts(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
        let primary_provider: Provider = self
            .config
            .embedding_provider
            .parse()
            .map_err(|e| format!("Unknown embedding provider: {}", e))?;

        let result = api_client::embed_texts(
            &primary_provider,
            &self.config.embedding_api_key,
            &self.config.embedding_model,
            texts,
        )
        .await;

        match result {
            Ok(embeddings) => Ok(embeddings),
            Err(e) => {
                if !is_retryable_error(&e) {
                    return Err(e);
                }

                tracing::warn!("Embedding API call failed: {}", e);

                for fallback_provider in &self.config.fallback_embedding_providers {
                    let fb_result = api_client::embed_texts(
                        fallback_provider,
                        &self.config.embedding_api_key,
                        &self.config.embedding_model,
                        texts,
                    )
                    .await;

                    match fb_result {
                        Ok(embeddings) => {
                            tracing::warn!("Embedding fallback succeeded: {:?}", fallback_provider);
                            return Ok(embeddings);
                        }
                        Err(fb_err) => {
                            tracing::warn!(
                                "Embedding fallback {:?} failed: {}",
                                fallback_provider,
                                fb_err
                            );
                        }
                    }
                }

                Err(format!(
                    "Embedding API error: {}. All providers exhausted.",
                    e
                ))
            }
        }
    }

    fn estimate_cost(&self, tokens_in: u64, tokens_out: u64) -> f64 {
        let rates: HashMap<&str, (f64, f64)> = HashMap::from([
            ("gemini-2.0-flash", (0.00010, 0.00040)),
            ("gemini-1.5-flash", (0.000075, 0.0003)),
            ("gemini-1.5-pro", (0.00125, 0.005)),
            ("gpt-4o-mini", (0.00015, 0.0006)),
            ("gpt-4o", (0.0025, 0.01)),
            ("claude-3-haiku", (0.00025, 0.00125)),
            ("claude-3-5-sonnet-20241022", (0.003, 0.015)),
            ("claude-3-opus-20240229", (0.015, 0.075)),
            ("deepseek-chat", (0.00014, 0.00028)),
            ("deepseek-reasoner", (0.00055, 0.00219)),
        ]);

        let (input_rate, output_rate) = rates
            .get(self.config.llm_model.as_str())
            .copied()
            .unwrap_or((0.00015, 0.0006));

        (tokens_in as f64 / 1000.0 * input_rate) + (tokens_out as f64 / 1000.0 * output_rate)
    }

    pub fn budget_remaining(&self) -> f64 {
        self.config.monthly_budget_usd - self.cost_tracker.total_spent_usd
    }
}

fn is_retryable_error(err: &str) -> bool {
    let err_lower = err.to_lowercase();
    err_lower.contains("429")
        || err_lower.contains("500")
        || err_lower.contains("502")
        || err_lower.contains("503")
        || err_lower.contains("504")
        || err_lower.contains("timeout")
        || err_lower.contains("timed out")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_api_key() {
        let config = RouterConfig::default();
        let router = LlmRouter::new(config);
        assert!(!router.is_available());
    }

    #[test]
    fn test_with_api_key() {
        let config = RouterConfig {
            llm_api_key: "test-key".to_string(),
            ..Default::default()
        };
        let router = LlmRouter::new(config);
        assert!(router.is_available());
    }

    #[test]
    fn test_cost_deduction() {
        let config = RouterConfig::default();
        let router = LlmRouter::new(config);
        let initial = router.budget_remaining();
        assert_eq!(initial, 5.0);
    }

    #[test]
    fn test_is_retryable_error() {
        assert!(is_retryable_error("HTTP 429 Too Many Requests"));
        assert!(is_retryable_error("Internal Server Error 500"));
        assert!(is_retryable_error("Bad Gateway 502"));
        assert!(is_retryable_error("Service Unavailable 503"));
        assert!(is_retryable_error("Gateway Timeout 504"));
        assert!(is_retryable_error("request timeout"));
        assert!(is_retryable_error("connection timed out"));
        assert!(!is_retryable_error("Bad Request 400"));
        assert!(!is_retryable_error("Unauthorized 401"));
        assert!(!is_retryable_error("Not Found 404"));
    }
}
