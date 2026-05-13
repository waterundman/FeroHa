// LLM Router — Route prompts to local or cloud models, with real HTTP completion

use std::collections::HashMap;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouterConfig {
    pub llm_provider: String,          // "gemini", "openai", "anthropic"
    pub llm_api_key: String,
    pub llm_model: String,
    pub embedding_provider: String,    // "gemini", "openai", "none"
    pub embedding_api_key: String,
    pub embedding_model: String,
    pub monthly_budget_usd: f64,
    pub temperature: f32,
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
}

/// Cost tracking
#[derive(Debug, Default)]
struct CostTracker {
    total_spent_usd: f64,
    calls: HashMap<String, u64>,
}

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

    /// Real HTTP completion — calls the configured LLM provider
    pub async fn complete(
        &mut self,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<LlmResponse, String> {
        let full_prompt = if system_prompt.is_empty() {
            user_prompt.to_string()
        } else {
            format!("{}\n\n---\n\n{}", system_prompt, user_prompt)
        };

        let start = std::time::Instant::now();

        let result = match self.config.llm_provider.as_str() {
            "gemini" => {
                super::api_client::gemini_complete(
                    &self.config.llm_api_key,
                    &self.config.llm_model,
                    &full_prompt,
                    self.config.temperature,
                )
                .await
            }
            "openai" => {
                super::api_client::openai_complete(
                    &self.config.llm_api_key,
                    &self.config.llm_model,
                    &full_prompt,
                    self.config.temperature,
                )
                .await
            }
            _ => Err(format!("Unknown LLM provider: {}", self.config.llm_provider)),
        };

        let latency_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok((text, tokens_in, tokens_out)) => {
                let cost = self.estimate_cost(tokens_in, tokens_out);
                self.cost_tracker.total_spent_usd += cost;
                *self.cost_tracker.calls
                    .entry(self.config.llm_model.clone())
                    .or_default() += 1;

                Ok(LlmResponse {
                    text,
                    model_used: self.config.llm_model.clone(),
                    tokens_in,
                    tokens_out,
                    cost_usd: cost,
                    latency_ms,
                })
            }
            Err(e) => {
                // Try fallback to a simple response when API fails
                tracing::warn!("LLM API call failed: {}", e);
                Err(format!("LLM API error: {}. Please check your API key in Settings.", e))
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

        self.complete(system_prompt, &user_prompt).await
    }

    fn estimate_cost(&self, tokens_in: u64, tokens_out: u64) -> f64 {
        let rates: HashMap<&str, (f64, f64)> = HashMap::from([
            ("gemini-2.0-flash",       (0.00010, 0.00040)),
            ("gemini-1.5-flash",       (0.000075, 0.0003)),
            ("gemini-1.5-pro",         (0.00125,  0.005)),
            ("gpt-4o-mini",            (0.00015,  0.0006)),
            ("gpt-4o",                 (0.0025,   0.01)),
            ("claude-3-haiku",         (0.00025,  0.00125)),
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
}
