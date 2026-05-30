// Shared HTTP API client for LLM and Embedding calls
// Uses reqwest with JSON support (already in Cargo.toml)

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::sync::LazyLock;

static USER_AGENT: &str = "DualTrackNote/0.1";

static HTTP_CLIENT: LazyLock<Client> = LazyLock::new(|| {
    Client::builder()
        .user_agent(USER_AGENT)
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .expect("Failed to build HTTP client")
});

pub fn http_client() -> &'static Client {
    &HTTP_CLIENT
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Provider {
    #[serde(rename = "gemini")]
    Gemini,
    #[serde(rename = "openai")]
    OpenAI,
    #[serde(rename = "anthropic")]
    Anthropic,
    #[serde(rename = "deepseek")]
    DeepSeek,
    #[serde(rename = "ollama")]
    Ollama,
}

impl FromStr for Provider {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "gemini" => Ok(Provider::Gemini),
            "openai" => Ok(Provider::OpenAI),
            "anthropic" => Ok(Provider::Anthropic),
            "deepseek" => Ok(Provider::DeepSeek),
            "ollama" => Ok(Provider::Ollama),
            _ => Err(format!("Unknown provider: {}", s)),
        }
    }
}

// ─── Gemini API ─────────────────────────────────────

#[derive(Serialize)]
struct GeminiRequest {
    contents: Vec<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "generationConfig")]
    generation_config: Option<GeminiConfig>,
}

#[derive(Serialize)]
struct GeminiContent {
    role: String,
    parts: Vec<GeminiPart>,
}

#[derive(Serialize)]
struct GeminiPart {
    text: String,
}

#[derive(Serialize)]
struct GeminiConfig {
    temperature: f32,
    #[serde(rename = "maxOutputTokens")]
    max_output_tokens: u32,
}

#[derive(Deserialize, Debug)]
struct GeminiResponse {
    candidates: Vec<GeminiCandidate>,
    #[serde(rename = "usageMetadata")]
    usage_metadata: Option<GeminiUsage>,
}

#[derive(Deserialize, Debug)]
struct GeminiCandidate {
    content: GeminiContentResp,
}

#[derive(Deserialize, Debug)]
struct GeminiContentResp {
    parts: Vec<GeminiPartResp>,
}

#[derive(Deserialize, Debug)]
struct GeminiPartResp {
    text: String,
}

#[derive(Deserialize, Debug)]
struct GeminiUsage {
    #[serde(rename = "promptTokenCount")]
    prompt_token_count: u64,
    #[serde(rename = "candidatesTokenCount")]
    candidates_token_count: u64,
    #[allow(dead_code)]
    #[serde(rename = "totalTokenCount")]
    total_token_count: u64,
}

pub async fn gemini_complete(
    api_key: &str,
    model: &str,
    prompt: &str,
    temperature: f32,
) -> Result<(String, u64, u64), String> {
    let client = http_client();
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
        model, api_key
    );

    let req = GeminiRequest {
        contents: vec![GeminiContent {
            role: "user".to_string(),
            parts: vec![GeminiPart {
                text: prompt.to_string(),
            }],
        }],
        generation_config: Some(GeminiConfig {
            temperature,
            max_output_tokens: 4096,
        }),
    };

    let resp = client
        .post(&url)
        .json(&req)
        .send()
        .await
        .map_err(|e| format!("Gemini API request failed: {}", e))?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Gemini API error: {}", body));
    }

    let data: GeminiResponse = resp
        .json()
        .await
        .map_err(|e| format!("Gemini response parse error: {}", e))?;

    let text = data
        .candidates
        .first()
        .and_then(|c| c.content.parts.first())
        .map(|p| p.text.clone())
        .unwrap_or_default();

    let (tokens_in, tokens_out) = data
        .usage_metadata
        .map(|u| (u.prompt_token_count, u.candidates_token_count))
        .unwrap_or((0, 0));

    Ok((text, tokens_in, tokens_out))
}

// ─── OpenAI API ─────────────────────────────────────

#[derive(Serialize)]
struct OpenAiRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
    temperature: f32,
    #[serde(rename = "max_tokens")]
    max_tokens: u32,
}

#[derive(Serialize)]
struct OpenAiMessage {
    role: String,
    content: String,
}

#[derive(Deserialize, Debug)]
struct OpenAiResponse {
    choices: Vec<OpenAiChoice>,
    usage: Option<OpenAiUsage>,
}

#[derive(Deserialize, Debug)]
struct OpenAiChoice {
    message: OpenAiMessageResp,
}

#[derive(Deserialize, Debug)]
struct OpenAiMessageResp {
    content: String,
}

#[derive(Deserialize, Debug)]
struct OpenAiUsage {
    prompt_tokens: u64,
    completion_tokens: u64,
    #[allow(dead_code)]
    total_tokens: u64,
}

pub async fn openai_complete(
    api_key: &str,
    model: &str,
    prompt: &str,
    temperature: f32,
) -> Result<(String, u64, u64), String> {
    let client = http_client();

    let req = OpenAiRequest {
        model: model.to_string(),
        messages: vec![OpenAiMessage {
            role: "user".to_string(),
            content: prompt.to_string(),
        }],
        temperature,
        max_tokens: 4096,
    };

    let resp = client
        .post("https://api.openai.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&req)
        .send()
        .await
        .map_err(|e| format!("OpenAI API request failed: {}", e))?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("OpenAI API error: {}", body));
    }

    let data: OpenAiResponse = resp
        .json()
        .await
        .map_err(|e| format!("OpenAI response parse error: {}", e))?;

    let text = data
        .choices
        .first()
        .map(|c| c.message.content.clone())
        .unwrap_or_default();

    let (tokens_in, tokens_out) = data
        .usage
        .map(|u| (u.prompt_tokens, u.completion_tokens))
        .unwrap_or((0, 0));

    Ok((text, tokens_in, tokens_out))
}

// ─── DeepSeek API (OpenAI-compatible) ───────────────

pub async fn deepseek_complete(
    api_key: &str,
    model: &str,
    prompt: &str,
    temperature: f32,
) -> Result<(String, u64, u64), String> {
    let client = http_client();

    let req = OpenAiRequest {
        model: model.to_string(),
        messages: vec![OpenAiMessage {
            role: "user".to_string(),
            content: prompt.to_string(),
        }],
        temperature,
        max_tokens: 4096,
    };

    let resp = client
        .post("https://api.deepseek.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&req)
        .send()
        .await
        .map_err(|e| format!("DeepSeek API request failed: {}", e))?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("DeepSeek API error: {}", body));
    }

    let data: OpenAiResponse = resp
        .json()
        .await
        .map_err(|e| format!("DeepSeek response parse error: {}", e))?;

    let text = data
        .choices
        .first()
        .map(|c| c.message.content.clone())
        .unwrap_or_default();

    let (tokens_in, tokens_out) = data
        .usage
        .map(|u| (u.prompt_tokens, u.completion_tokens))
        .unwrap_or((0, 0));

    Ok((text, tokens_in, tokens_out))
}

// ─── Anthropic Messages API ─────────────────────────

#[derive(Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<AnthropicMessage>,
    temperature: f32,
}

#[derive(Serialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

#[derive(Deserialize, Debug)]
struct AnthropicResponse {
    content: Vec<AnthropicContent>,
    usage: AnthropicUsage,
}

#[derive(Deserialize, Debug)]
struct AnthropicContent {
    text: String,
}

#[derive(Deserialize, Debug)]
struct AnthropicUsage {
    input_tokens: u64,
    output_tokens: u64,
}

pub async fn anthropic_complete(
    api_key: &str,
    model: &str,
    prompt: &str,
    temperature: f32,
) -> Result<(String, u64, u64), String> {
    let client = http_client();

    let req = AnthropicRequest {
        model: model.to_string(),
        max_tokens: 4096,
        messages: vec![AnthropicMessage {
            role: "user".to_string(),
            content: prompt.to_string(),
        }],
        temperature,
    };

    let resp = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&req)
        .send()
        .await
        .map_err(|e| format!("Anthropic API request failed: {}", e))?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Anthropic API error: {}", body));
    }

    let data: AnthropicResponse = resp
        .json()
        .await
        .map_err(|e| format!("Anthropic response parse error: {}", e))?;

    let text = data
        .content
        .first()
        .map(|c| c.text.clone())
        .unwrap_or_default();

    Ok((text, data.usage.input_tokens, data.usage.output_tokens))
}

// ─── Ollama Local API ───────────────────────────────

#[derive(Serialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    stream: bool,
}

#[derive(Deserialize, Debug)]
struct OllamaResponse {
    response: String,
}

pub async fn ollama_complete(
    base_url: &str,
    model: &str,
    prompt: &str,
) -> Result<(String, u64, u64), String> {
    let client = http_client();

    let req = OllamaRequest {
        model: model.to_string(),
        prompt: prompt.to_string(),
        stream: false,
    };

    let url = format!("{}/api/generate", base_url.trim_end_matches('/'));

    let resp = client
        .post(&url)
        .json(&req)
        .send()
        .await
        .map_err(|e| format!("Ollama API request failed: {}", e))?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Ollama API error: {}", body));
    }

    let data: OllamaResponse = resp
        .json()
        .await
        .map_err(|e| format!("Ollama response parse error: {}", e))?;

    Ok((data.response, 0, 0))
}

// ─── Embedding API (OpenAI compatible) ──────────────

#[derive(Serialize)]
struct EmbeddingRequest {
    model: String,
    input: Vec<String>,
}

#[derive(Deserialize, Debug)]
struct EmbeddingResponse {
    data: Vec<EmbeddingData>,
}

#[derive(Deserialize, Debug)]
struct EmbeddingData {
    embedding: Vec<f32>,
}

pub async fn openai_embed(
    api_key: &str,
    model: &str,
    texts: &[String],
) -> Result<Vec<Vec<f32>>, String> {
    let client = http_client();

    let req = EmbeddingRequest {
        model: model.to_string(),
        input: texts.to_vec(),
    };

    let resp = client
        .post("https://api.openai.com/v1/embeddings")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&req)
        .send()
        .await
        .map_err(|e| format!("Embedding API request failed: {}", e))?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Embedding API error: {}", body));
    }

    let data: EmbeddingResponse = resp
        .json()
        .await
        .map_err(|e| format!("Embedding response parse error: {}", e))?;

    Ok(data.data.into_iter().map(|d| d.embedding).collect())
}

// ─── Gemini Embedding API (batch) ───────────────────

#[derive(Serialize)]
struct GeminiBatchEmbedRequest {
    requests: Vec<GeminiSingleEmbedReq>,
}

#[derive(Serialize)]
struct GeminiSingleEmbedReq {
    model: String,
    content: GeminiEmbedContent,
}

#[derive(Serialize)]
struct GeminiEmbedContent {
    parts: Vec<GeminiPart>,
}

#[derive(Deserialize, Debug)]
struct GeminiBatchEmbedResponse {
    embeddings: Vec<GeminiEmbedValues>,
}

#[derive(Deserialize, Debug)]
struct GeminiEmbedValues {
    values: Option<Vec<f32>>,
}

pub async fn gemini_embed(api_key: &str, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
    if texts.is_empty() {
        return Ok(vec![]);
    }

    let client = http_client();
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/text-embedding-004:batchEmbedContents?key={}",
        api_key
    );

    let requests: Vec<GeminiSingleEmbedReq> = texts
        .iter()
        .map(|text| GeminiSingleEmbedReq {
            model: "models/text-embedding-004".to_string(),
            content: GeminiEmbedContent {
                parts: vec![GeminiPart { text: text.clone() }],
            },
        })
        .collect();

    let resp = client
        .post(&url)
        .json(&GeminiBatchEmbedRequest { requests })
        .send()
        .await
        .map_err(|e| format!("Gemini batch embed API request failed: {}", e))?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Gemini batch embed API error: {}", body));
    }

    let data: GeminiBatchEmbedResponse = resp
        .json()
        .await
        .map_err(|e| format!("Gemini batch embed response parse error: {}", e))?;

    Ok(data
        .embeddings
        .into_iter()
        .map(|e| e.values.unwrap_or_else(|| vec![0.0_f32; 768]))
        .collect())
}

pub async fn complete(
    provider: &Provider,
    api_key: &str,
    model: &str,
    prompt: &str,
    temperature: f32,
    ollama_base_url: &str,
) -> Result<(String, u64, u64), String> {
    match provider {
        Provider::Gemini => gemini_complete(api_key, model, prompt, temperature).await,
        Provider::OpenAI => openai_complete(api_key, model, prompt, temperature).await,
        Provider::Anthropic => anthropic_complete(api_key, model, prompt, temperature).await,
        Provider::DeepSeek => deepseek_complete(api_key, model, prompt, temperature).await,
        Provider::Ollama => ollama_complete(ollama_base_url, model, prompt).await,
    }
}

pub async fn embed_texts(
    provider: &Provider,
    api_key: &str,
    model: &str,
    texts: &[String],
) -> Result<Vec<Vec<f32>>, String> {
    match provider {
        Provider::OpenAI => openai_embed(api_key, model, texts).await,
        Provider::Gemini => gemini_embed(api_key, texts).await,
        Provider::Anthropic | Provider::DeepSeek | Provider::Ollama => Err(
            "Embedding is not supported for this provider. Please use Gemini or OpenAI."
                .to_string(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gemini_batch_embed_request_serializes_correctly() {
        let texts = vec!["hello world".to_string(), "foo bar".to_string()];
        let requests: Vec<GeminiSingleEmbedReq> = texts
            .iter()
            .map(|text| GeminiSingleEmbedReq {
                model: "models/text-embedding-004".to_string(),
                content: GeminiEmbedContent {
                    parts: vec![GeminiPart { text: text.clone() }],
                },
            })
            .collect();
        let req = GeminiBatchEmbedRequest { requests };
        let json = serde_json::to_value(&req).unwrap();

        assert_eq!(json["requests"][0]["model"], "models/text-embedding-004");
        assert_eq!(
            json["requests"][0]["content"]["parts"][0]["text"],
            "hello world"
        );
        assert_eq!(
            json["requests"][1]["content"]["parts"][0]["text"],
            "foo bar"
        );
    }

    #[test]
    fn gemini_batch_embed_response_deserializes_correctly() {
        let json = r#"{
            "embeddings": [
                {"values": [0.1, 0.2, 0.3]},
                {"values": [0.4, 0.5, 0.6]}
            ]
        }"#;
        let data: GeminiBatchEmbedResponse = serde_json::from_str(json).unwrap();
        assert_eq!(data.embeddings.len(), 2);
        assert_eq!(
            data.embeddings[0].values.as_ref().unwrap(),
            &vec![0.1, 0.2, 0.3]
        );
        assert_eq!(
            data.embeddings[1].values.as_ref().unwrap(),
            &vec![0.4, 0.5, 0.6]
        );
    }

    #[tokio::test]
    async fn gemini_embed_empty_returns_empty() {
        let result = gemini_embed("fake-key", &[]).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }
}
