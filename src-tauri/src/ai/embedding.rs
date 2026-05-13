// Embedding Pipeline — Real implementation via API
// Falls back to hash-based pseudo-embedding when no API key configured

use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EmbeddingBackend {
    None,
    /// OpenAI text-embedding-3-small (1536 dims) or similar
    OpenAi { api_key: String, model: String },
    /// Google text-embedding-004 (768 dims)
    Gemini { api_key: String },
}

impl Default for EmbeddingBackend {
    fn default() -> Self {
        EmbeddingBackend::None
    }
}

impl EmbeddingBackend {
    pub fn is_available(&self) -> bool {
        match self {
            EmbeddingBackend::None => false,
            EmbeddingBackend::OpenAi { api_key, .. } => !api_key.is_empty(),
            EmbeddingBackend::Gemini { api_key } => !api_key.is_empty(),
        }
    }

    pub fn dimension(&self) -> usize {
        match self {
            EmbeddingBackend::None => 384,
            EmbeddingBackend::OpenAi { .. } => 1536,
            EmbeddingBackend::Gemini { .. } => 768,
        }
    }
}

pub struct EmbeddingPipeline {
    backend: EmbeddingBackend,
}

impl EmbeddingPipeline {
    pub fn new(backend: EmbeddingBackend) -> Self {
        EmbeddingPipeline { backend }
    }

    pub fn backend_config(&self) -> EmbeddingBackend {
        self.backend.clone()
    }

    pub fn dimension(&self) -> usize {
        self.backend.dimension()
    }

    pub fn is_real(&self) -> bool {
        self.backend.is_available()
    }

    pub async fn embed(&self, texts: &[String]) -> Vec<Vec<f32>> {
        if texts.is_empty() {
            return vec![];
        }

        match &self.backend {
            EmbeddingBackend::OpenAi { api_key, model } => {
                match super::api_client::openai_embed(api_key, model, texts).await {
                    Ok(vectors) => vectors,
                    Err(e) => {
                        tracing::warn!("OpenAI embed failed: {}, falling back to hash", e);
                        hash_embed(texts, 1536)
                    }
                }
            }
            EmbeddingBackend::Gemini { api_key } => {
                match super::api_client::gemini_embed(api_key, texts).await {
                    Ok(vectors) => vectors,
                    Err(e) => {
                        tracing::warn!("Gemini embed failed: {}, falling back to hash", e);
                        hash_embed(texts, 768)
                    }
                }
            }
            EmbeddingBackend::None => {
                hash_embed(texts, 384)
            }
        }
    }

    pub fn embed_sync(&self, texts: &[String]) -> Vec<Vec<f32>> {
        if texts.is_empty() {
            return vec![];
        }
        hash_embed(texts, self.dimension())
    }
}

/// Deterministic hash-based embedding (fallback when no API)
/// Uses a simple TF-like approach with character n-grams for semantic signal
fn hash_embed(texts: &[String], dim: usize) -> Vec<Vec<f32>> {
    texts.iter().map(|text| text_to_vector(text, dim)).collect()
}

fn text_to_vector(text: &str, dim: usize) -> Vec<f32> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let text_lower = text.to_lowercase();
    let mut vector = vec![0.0_f32; dim];

    // Character trigram hashing to distribute semantic signal across dimensions
    let chars: Vec<char> = text_lower.chars().collect();
    for i in 0..chars.len().saturating_sub(2) {
        let trigram = format!("{}{}{}", chars[i], chars[i + 1], chars[i + 2]);
        let mut hasher = DefaultHasher::new();
        trigram.hash(&mut hasher);
        let hash_val = hasher.finish();
        let idx = (hash_val as usize) % dim;
        vector[idx] += 1.0;
    }

    // L2 normalize
    let norm: f32 = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for v in &mut vector {
            *v /= norm;
        }
    }

    vector
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_embed_deterministic() {
        let v1 = hash_embed(&["hello world".to_string()], 384);
        let v2 = hash_embed(&["hello world".to_string()], 384);
        assert_eq!(v1[0], v2[0]);
    }

    #[test]
    fn test_similar_texts_closer() {
        let v1 = hash_embed(&["rust programming language".to_string()], 384);
        let v2 = hash_embed(&["rust programming guide".to_string()], 384);
        let v3 = hash_embed(&["banana smoothie recipe".to_string()], 384);

        let sim12 = cosine(&v1[0], &v2[0]);
        let sim13 = cosine(&v1[0], &v3[0]);
        assert!(sim12 > sim13, "Similar texts should be closer");
    }

    #[test]
    fn test_dimension() {
        let pipe = EmbeddingPipeline::new(EmbeddingBackend::None);
        let v = pipe.embed_sync(&["test".to_string()]);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].len(), 384);
    }

    fn cosine(a: &[f32], b: &[f32]) -> f32 {
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        if na == 0.0 || nb == 0.0 { 0.0 } else { dot / (na * nb) }
    }
}
