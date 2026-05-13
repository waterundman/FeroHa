// Shared HTTP API client for LLM and Embedding calls
// Uses reqwest with JSON support (already in Cargo.toml)

use reqwest::Client;
use serde::{Serialize, Deserialize};

static USER_AGENT: &str = "DualTrackNote/0.1";

pub fn http_client() -> Result<Client, reqwest::Error> {
    Client::builder()
        .user_agent(USER_AGENT)
        .timeout(std::time::Duration::from_secs(60))
        .build()
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
    let client = http_client().map_err(|e| format!("HTTP client error: {}", e))?;
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
    let client = http_client().map_err(|e| format!("HTTP client error: {}", e))?;

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
    let client = http_client().map_err(|e| format!("HTTP client error: {}", e))?;

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

// ─── Gemini Embedding API ───────────────────────────

#[derive(Serialize)]
struct GeminiEmbedRequest {
    model: String,
    content: GeminiEmbedContent,
}

#[derive(Serialize)]
struct GeminiEmbedContent {
    parts: Vec<GeminiPart>,
}

#[derive(Deserialize, Debug)]
struct GeminiEmbedResponse {
    embedding: Option<GeminiEmbedValues>,
}

#[derive(Deserialize, Debug)]
struct GeminiEmbedValues {
    values: Option<Vec<f32>>,
}

pub async fn gemini_embed(
    api_key: &str,
    texts: &[String],
) -> Result<Vec<Vec<f32>>, String> {
    let client = http_client().map_err(|e| format!("HTTP client error: {}", e))?;
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/text-embedding-004:embedContent?key={}",
        api_key
    );

    let mut results = Vec::new();

    for text in texts {
        let req = GeminiEmbedRequest {
            model: "text-embedding-004".to_string(),
            content: GeminiEmbedContent {
                parts: vec![GeminiPart {
                    text: text.clone(),
                }],
            },
        };

        let resp = client
            .post(&url)
            .json(&req)
            .send()
            .await
            .map_err(|e| format!("Gemini embed API request failed: {}", e))?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Gemini embed API error: {}", body));
        }

        let data: GeminiEmbedResponse = resp
            .json()
            .await
            .map_err(|e| format!("Gemini embed response parse error: {}", e))?;

        let vec = data
            .embedding
            .and_then(|e| e.values)
            .unwrap_or_else(|| vec![0.0_f32; 768]);

        results.push(vec);
    }

    Ok(results)
}
