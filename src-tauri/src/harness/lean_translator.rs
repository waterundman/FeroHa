use crate::ai::llm_router::LlmRouter;
use crate::harness::lean_kernel::{
    Proposition, PropositionGraph, PropositionId, PropositionRelation, RelationType,
};
use crate::harness::scientist::CleanKnowledge;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

pub const DEFAULT_LEAN_PROMPT: &str = r#"
You are a formalization translator. Your role is to convert natural language research data into a structured proposition graph.

DO NOT verify the truth of any claim. DO NOT judge correctness.
ONLY translate what is presented into the following JSON format:

{
  "propositions": [
    {
      "pid": { "id": "P1", "content_hash": "hash_of_content", "human_readable": "Short label" },
      "content": "The full claim text",
      "source_agent_id": "agent_name",
      "confidence": 0.85
    }
  ],
  "relations": [
    {
      "from": { "id": "P1", "content_hash": "hash", "human_readable": "label" },
      "to": { "id": "P2", "content_hash": "hash", "human_readable": "label" },
      "relation_type": "Implies",
      "strength": 0.9
    }
  ],
  "source_agent_id": "agent_name",
  "timestamp": 0
}

Rules:
1. Each distinct claim becomes one Proposition.
2. Relation types: "Implies" (A logically implies B), "Contradicts" (A contradicts B), "Supports" (A provides evidence for B), "DependsOn" (B depends on A being true).
3. Assign unique IDs like P1, P2, P3...
4. content_hash should be a short hash of the content string (use sha256 first 8 chars).
5. human_readable should be a short label (max 8 words).
6. confidence reflects the source's stated confidence, not your judgment.
7. Only output valid JSON — no explanations before or after.
"#;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranslationResult {
    pub success: bool,
    pub graph: Option<PropositionGraph>,
    pub error: Option<String>,
    pub raw_response: Option<String>,
}

pub struct LeanShapedTranslator {
    llm_router: Arc<Mutex<LlmRouter>>,
    prompt_template: String,
}

impl LeanShapedTranslator {
    pub fn new(llm_router: Arc<Mutex<LlmRouter>>, prompt_template: Option<String>) -> Self {
        Self {
            llm_router,
            prompt_template: prompt_template.unwrap_or_else(|| DEFAULT_LEAN_PROMPT.to_string()),
        }
    }

    pub async fn translate(&self, knowledge: &CleanKnowledge, agent_id: &str) -> TranslationResult {
        let user_prompt = Self::build_user_prompt(knowledge);

        let mut router = self.llm_router.lock().await;
        match router
            .complete(&self.prompt_template, &user_prompt, None)
            .await
        {
            Ok(response) => Self::parse_response(&response.text, agent_id),
            Err(e) => TranslationResult {
                success: false,
                graph: None,
                error: Some(format!("LLM error: {}", e)),
                raw_response: None,
            },
        }
    }

    fn build_user_prompt(knowledge: &CleanKnowledge) -> String {
        let mut prompt = String::new();

        prompt.push_str("## Claims\n");
        for (i, claim) in knowledge.claims.iter().enumerate() {
            prompt.push_str(&format!("{}. {}\n", i + 1, claim));
            if let Some(conf) = knowledge.confidence_map.get(claim) {
                prompt.push_str(&format!("   Confidence: {:.2}\n", conf));
            }
        }

        prompt.push_str("\n## Sources\n");
        for source in &knowledge.sources {
            prompt.push_str(&format!(
                "- [{}] {} = {}\n",
                source.layer_name(),
                source.key,
                source.value_summary()
            ));
        }

        prompt
    }

    fn parse_response(response: &str, agent_id: &str) -> TranslationResult {
        let json_str = if let Some(start) = response.find("```json") {
            let content = &response[start + 7..];
            if let Some(end) = content.find("```") {
                &content[..end]
            } else {
                content
            }
        } else if let Some(start) = response.find('{') {
            &response[start..]
        } else {
            return TranslationResult {
                success: false,
                graph: None,
                error: Some("No JSON found in LLM response".to_string()),
                raw_response: Some(response.to_string()),
            };
        };

        match serde_json::from_str::<serde_json::Value>(json_str.trim()) {
            Ok(value) => {
                let mut graph = PropositionGraph {
                    propositions: Vec::new(),
                    relations: Vec::new(),
                    source_agent_id: agent_id.to_string(),
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64,
                };

                if let Some(props) = value.get("propositions").and_then(|v| v.as_array()) {
                    for prop in props {
                        if let (Some(pid), Some(content)) = (
                            Self::parse_proposition_id(prop.get("pid")),
                            prop.get("content").and_then(|v| v.as_str()),
                        ) {
                            let confidence = prop
                                .get("confidence")
                                .and_then(|v| v.as_f64())
                                .unwrap_or(0.5) as f32;

                            graph.propositions.push(Proposition {
                                pid,
                                content: content.to_string(),
                                source_agent_id: agent_id.to_string(),
                                confidence: confidence.clamp(0.0, 1.0),
                            });
                        }
                    }
                }

                if let Some(rels) = value.get("relations").and_then(|v| v.as_array()) {
                    for rel in rels {
                        if let (Some(from), Some(to)) = (
                            Self::parse_proposition_id(rel.get("from")),
                            Self::parse_proposition_id(rel.get("to")),
                        ) {
                            let rel_type = rel
                                .get("relation_type")
                                .and_then(|v| v.as_str())
                                .map(|s| match s {
                                    "Implies" => RelationType::Implies,
                                    "Contradicts" => RelationType::Contradicts,
                                    "Supports" => RelationType::Supports,
                                    "DependsOn" => RelationType::DependsOn,
                                    _ => RelationType::Supports,
                                })
                                .unwrap_or(RelationType::Supports);

                            let strength =
                                rel.get("strength").and_then(|v| v.as_f64()).unwrap_or(0.5) as f32;

                            graph.relations.push(PropositionRelation {
                                from,
                                to,
                                relation_type: rel_type,
                                strength: strength.clamp(0.0, 1.0),
                            });
                        }
                    }
                }

                if graph.propositions.is_empty() {
                    TranslationResult {
                        success: false,
                        graph: None,
                        error: Some("No propositions extracted from response".to_string()),
                        raw_response: Some(response.to_string()),
                    }
                } else {
                    TranslationResult {
                        success: true,
                        graph: Some(graph),
                        error: None,
                        raw_response: Some(response.to_string()),
                    }
                }
            }
            Err(e) => TranslationResult {
                success: false,
                graph: None,
                error: Some(format!("JSON parse error: {}", e)),
                raw_response: Some(response.to_string()),
            },
        }
    }

    fn parse_proposition_id(value: Option<&serde_json::Value>) -> Option<PropositionId> {
        value.and_then(|v| {
            let id = v.get("id")?.as_str()?.to_string();
            let content_hash = v
                .get("content_hash")
                .and_then(|h| h.as_str())
                .unwrap_or("")
                .to_string();
            let human_readable = v
                .get("human_readable")
                .and_then(|h| h.as_str())
                .unwrap_or("")
                .to_string();
            Some(PropositionId {
                id,
                content_hash,
                human_readable,
            })
        })
    }

    #[allow(dead_code)]
    pub fn prompt_template(&self) -> &str {
        &self.prompt_template
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_json() {
        let response = r#"{
            "propositions": [
                {
                    "pid": { "id": "P1", "content_hash": "abc12345", "human_readable": "Gravity exists" },
                    "content": "Gravity is a fundamental force that attracts objects with mass.",
                    "source_agent_id": "scientist",
                    "confidence": 0.95
                },
                {
                    "pid": { "id": "P2", "content_hash": "def67890", "human_readable": "Mass bends spacetime" },
                    "content": "Massive objects bend the fabric of spacetime.",
                    "source_agent_id": "scientist",
                    "confidence": 0.9
                }
            ],
            "relations": [
                {
                    "from": { "id": "P1", "content_hash": "abc12345", "human_readable": "Gravity exists" },
                    "to": { "id": "P2", "content_hash": "def67890", "human_readable": "Mass bends spacetime" },
                    "relation_type": "Implies",
                    "strength": 0.85
                }
            ]
        }"#;

        let result = LeanShapedTranslator::parse_response(response, "test_agent");
        assert!(
            result.success,
            "Expected successful parse, got error: {:?}",
            result.error
        );
        assert!(result.raw_response.is_some());

        let graph = result.graph.unwrap();
        assert_eq!(graph.propositions.len(), 2);
        assert_eq!(graph.propositions[0].pid.id, "P1");
        assert_eq!(
            graph.propositions[0].content,
            "Gravity is a fundamental force that attracts objects with mass."
        );
        assert!((graph.propositions[0].confidence - 0.95).abs() < 0.001);
        assert_eq!(graph.propositions[1].pid.id, "P2");
        assert_eq!(graph.relations.len(), 1);
        assert_eq!(graph.relations[0].relation_type, RelationType::Implies);
        assert!((graph.relations[0].strength - 0.85).abs() < 0.001);
        assert_eq!(graph.source_agent_id, "test_agent");
    }

    #[test]
    fn test_parse_invalid_json() {
        let response = "this is not json at all {{{";

        let result = LeanShapedTranslator::parse_response(response, "test_agent");
        assert!(!result.success);
        assert!(result.graph.is_none());
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("JSON parse error"));
    }

    #[test]
    fn test_parse_empty_response() {
        let response = r#"{
            "propositions": [],
            "relations": []
        }"#;

        let result = LeanShapedTranslator::parse_response(response, "test_agent");
        assert!(!result.success);
        assert!(result.graph.is_none());
        assert_eq!(
            result.error.unwrap(),
            "No propositions extracted from response"
        );
    }

    #[test]
    fn test_parse_markdown_wrapped_json() {
        let response = r#"Here is my analysis:

```json
{
    "propositions": [
        {
            "pid": { "id": "P1", "content_hash": "hash0001", "human_readable": "Water boils at 100C" },
            "content": "Water boils at 100 degrees Celsius at sea level.",
            "source_agent_id": "verifier",
            "confidence": 0.99
        }
    ],
    "relations": []
}
```

That's my translation."#;

        let result = LeanShapedTranslator::parse_response(response, "test_agent");
        assert!(
            result.success,
            "Expected success with markdown-wrapped JSON, got: {:?}",
            result.error
        );
        let graph = result.graph.unwrap();
        assert_eq!(graph.propositions.len(), 1);
        assert_eq!(graph.propositions[0].pid.id, "P1");
        assert_eq!(
            graph.propositions[0].content,
            "Water boils at 100 degrees Celsius at sea level."
        );
    }

    #[test]
    fn test_parse_no_json_in_response() {
        let response = "I'm sorry, I cannot process this request.";

        let result = LeanShapedTranslator::parse_response(response, "test_agent");
        assert!(!result.success);
        assert!(result.error.unwrap().contains("No JSON found"));
    }

    #[test]
    fn test_parse_relations_only_response() {
        let response = r#"{
            "propositions": [],
            "relations": [
                {
                    "from": { "id": "P1", "content_hash": "h1", "human_readable": "A" },
                    "to": { "id": "P2", "content_hash": "h2", "human_readable": "B" },
                    "relation_type": "Contradicts",
                    "strength": 0.7
                }
            ]
        }"#;

        let result = LeanShapedTranslator::parse_response(response, "test_agent");
        assert!(!result.success);
        assert!(result.error.unwrap().contains("No propositions extracted"));
    }

    #[test]
    fn test_parse_unknown_relation_defaults_to_supports() {
        let response = r#"{
            "propositions": [
                {
                    "pid": { "id": "P1", "content_hash": "h1", "human_readable": "Test" },
                    "content": "Test content",
                    "source_agent_id": "agent",
                    "confidence": 0.5
                },
                {
                    "pid": { "id": "P2", "content_hash": "h2", "human_readable": "Test2" },
                    "content": "Test content 2",
                    "source_agent_id": "agent",
                    "confidence": 0.5
                }
            ],
            "relations": [
                {
                    "from": { "id": "P1", "content_hash": "h1", "human_readable": "Test" },
                    "to": { "id": "P2", "content_hash": "h2", "human_readable": "Test2" },
                    "relation_type": "UnknownRelation",
                    "strength": 0.5
                }
            ]
        }"#;

        let result = LeanShapedTranslator::parse_response(response, "test_agent");
        assert!(result.success);
        let graph = result.graph.unwrap();
        assert_eq!(graph.relations.len(), 1);
        assert_eq!(graph.relations[0].relation_type, RelationType::Supports);
    }

    #[test]
    fn test_parse_missing_confidence_defaults() {
        let response = r#"{
            "propositions": [
                {
                    "pid": { "id": "P1", "content_hash": "h1", "human_readable": "Test" },
                    "content": "Test content",
                    "source_agent_id": "agent"
                }
            ],
            "relations": []
        }"#;

        let result = LeanShapedTranslator::parse_response(response, "test_agent");
        assert!(result.success);
        let graph = result.graph.unwrap();
        assert!((graph.propositions[0].confidence - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_prompt_template_default() {
        let config = crate::ai::llm_router::RouterConfig::default();
        let router = Arc::new(Mutex::new(LlmRouter::new(config)));
        let translator = LeanShapedTranslator::new(router, None);
        assert_eq!(translator.prompt_template(), DEFAULT_LEAN_PROMPT);
    }

    #[test]
    fn test_prompt_template_custom() {
        let config = crate::ai::llm_router::RouterConfig::default();
        let router = Arc::new(Mutex::new(LlmRouter::new(config)));
        let custom = "Custom prompt".to_string();
        let translator = LeanShapedTranslator::new(router, Some(custom.clone()));
        assert_eq!(translator.prompt_template(), custom);
    }
}
