use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ContextLayer {
    System,
    Note,
    Session,
    Project,
    Transient,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ContextSource {
    User,
    Note,
    System,
    RAG,
    Agent,
    Pipeline,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextFragment {
    pub id: String,
    pub key: String,
    pub value: serde_json::Value,
    pub source: ContextSource,
    pub layer: ContextLayer,
    pub created_at: u64,
    pub ttl: Option<u64>,
    pub hash: String,
}

impl ContextFragment {
    pub fn compute_hash(key: &str, value: &serde_json::Value) -> String {
        let mut hasher = Sha256::new();
        hasher.update(key.as_bytes());
        hasher.update(serde_json::to_string(value).unwrap_or_default().as_bytes());
        format!("{:x}", hasher.finalize())
    }

    pub fn layer_name(&self) -> &str {
        match self.layer {
            ContextLayer::System => "System",
            ContextLayer::Note => "Note",
            ContextLayer::Session => "Session",
            ContextLayer::Project => "Project",
            ContextLayer::Transient => "Transient",
        }
    }

    pub fn value_summary(&self) -> String {
        let s = serde_json::to_string(&self.value).unwrap_or_default();
        if s.len() > 100 {
            format!("{}...", &s[..97])
        } else {
            s
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ContextRef {
    pub key: String,
    pub layer: ContextLayer,
    pub hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ChangeOp {
    Added,
    Modified,
    Removed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextFragmentChange {
    pub key: String,
    pub op: ChangeOp,
    pub old_value: Option<serde_json::Value>,
    pub new_value: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextDiff {
    pub prev: Vec<ContextFragment>,
    pub curr: Vec<ContextFragment>,
    pub changed: Vec<ContextFragmentChange>,
    pub drift_score: f32,
}

impl ContextDiff {
    pub fn compute(prev: Vec<ContextFragment>, curr: Vec<ContextFragment>) -> Self {
        use std::collections::HashMap;
        let prev_map: HashMap<&str, &ContextFragment> =
            prev.iter().map(|f| (f.key.as_str(), f)).collect();
        let curr_map: HashMap<&str, &ContextFragment> =
            curr.iter().map(|f| (f.key.as_str(), f)).collect();

        let all_keys: Vec<&str> = {
            let mut keys: Vec<&str> = prev_map
                .keys()
                .copied()
                .chain(curr_map.keys().copied())
                .collect();
            keys.sort();
            keys.dedup();
            keys
        };

        let mut changed = Vec::new();
        for key in &all_keys {
            match (prev_map.get(key), curr_map.get(key)) {
                (None, Some(curr_frag)) => {
                    changed.push(ContextFragmentChange {
                        key: key.to_string(),
                        op: ChangeOp::Added,
                        old_value: None,
                        new_value: Some(curr_frag.value.clone()),
                    });
                }
                (Some(prev_frag), None) => {
                    changed.push(ContextFragmentChange {
                        key: key.to_string(),
                        op: ChangeOp::Removed,
                        old_value: Some(prev_frag.value.clone()),
                        new_value: None,
                    });
                }
                (Some(prev_frag), Some(curr_frag)) => {
                    if prev_frag.hash != curr_frag.hash {
                        changed.push(ContextFragmentChange {
                            key: key.to_string(),
                            op: ChangeOp::Modified,
                            old_value: Some(prev_frag.value.clone()),
                            new_value: Some(curr_frag.value.clone()),
                        });
                    }
                }
                (None, None) => {}
            }
        }

        let total_unique = all_keys.len() as f32;
        let drift_score = if total_unique > 0.0 {
            changed.len() as f32 / total_unique
        } else {
            0.0
        };

        ContextDiff {
            prev,
            curr,
            changed,
            drift_score,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_context_fragment_serde_roundtrip() {
        let frag = ContextFragment {
            id: "test-1".to_string(),
            key: "note.content".to_string(),
            value: json!("hello world"),
            source: ContextSource::Note,
            layer: ContextLayer::Note,
            created_at: 1234567890,
            ttl: None,
            hash: ContextFragment::compute_hash("note.content", &json!("hello world")),
        };
        let json_str = serde_json::to_string(&frag).unwrap();
        let parsed: ContextFragment = serde_json::from_str(&json_str).unwrap();
        assert_eq!(frag.id, parsed.id);
        assert_eq!(frag.key, parsed.key);
        assert_eq!(frag.value, parsed.value);
        assert_eq!(frag.hash, parsed.hash);
        assert_eq!(frag.source, parsed.source);
        assert_eq!(frag.layer, parsed.layer);
    }

    #[test]
    fn test_context_fragment_hash_consistent() {
        let h1 = ContextFragment::compute_hash("key1", &json!("value1"));
        let h2 = ContextFragment::compute_hash("key1", &json!("value1"));
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_context_diff_no_changes() {
        let f1 = ContextFragment {
            id: "1".to_string(),
            key: "a".to_string(),
            value: json!("v"),
            source: ContextSource::Note,
            layer: ContextLayer::Note,
            created_at: 1,
            ttl: None,
            hash: ContextFragment::compute_hash("a", &json!("v")),
        };
        let f2 = f1.clone();
        let diff = ContextDiff::compute(vec![f1], vec![f2]);
        assert_eq!(diff.changed.len(), 0);
        assert_eq!(diff.drift_score, 0.0);
    }

    #[test]
    fn test_context_diff_with_changes() {
        let prev = vec![ContextFragment {
            id: "1".to_string(),
            key: "a".to_string(),
            value: json!("old"),
            source: ContextSource::Note,
            layer: ContextLayer::Note,
            created_at: 1,
            ttl: None,
            hash: ContextFragment::compute_hash("a", &json!("old")),
        }];
        let curr = vec![ContextFragment {
            id: "2".to_string(),
            key: "a".to_string(),
            value: json!("new"),
            source: ContextSource::Note,
            layer: ContextLayer::Note,
            created_at: 2,
            ttl: None,
            hash: ContextFragment::compute_hash("a", &json!("new")),
        }];
        let diff = ContextDiff::compute(prev, curr);
        assert_eq!(diff.changed.len(), 1);
        assert_eq!(diff.changed[0].op, ChangeOp::Modified);
        assert_eq!(diff.drift_score, 1.0);
    }

    #[test]
    fn test_context_diff_added_removed() {
        let prev = vec![ContextFragment {
            id: "1".to_string(),
            key: "a".to_string(),
            value: json!("v"),
            source: ContextSource::Note,
            layer: ContextLayer::Note,
            created_at: 1,
            ttl: None,
            hash: ContextFragment::compute_hash("a", &json!("v")),
        }];
        let curr = vec![ContextFragment {
            id: "2".to_string(),
            key: "b".to_string(),
            value: json!("v"),
            source: ContextSource::Note,
            layer: ContextLayer::Note,
            created_at: 2,
            ttl: None,
            hash: ContextFragment::compute_hash("b", &json!("v")),
        }];
        let diff = ContextDiff::compute(prev, curr);
        assert_eq!(diff.changed.len(), 2);
        assert_eq!(diff.drift_score, 1.0);
    }

    #[test]
    fn test_context_diff_partial_drift() {
        let prev = vec![
            ContextFragment {
                id: "1".into(),
                key: "a".into(),
                value: json!("v1"),
                source: ContextSource::Note,
                layer: ContextLayer::Note,
                created_at: 1,
                ttl: None,
                hash: ContextFragment::compute_hash("a", &json!("v1")),
            },
            ContextFragment {
                id: "2".into(),
                key: "b".into(),
                value: json!("v2"),
                source: ContextSource::Note,
                layer: ContextLayer::Note,
                created_at: 1,
                ttl: None,
                hash: ContextFragment::compute_hash("b", &json!("v2")),
            },
        ];
        let curr = vec![
            ContextFragment {
                id: "3".into(),
                key: "a".into(),
                value: json!("v1"),
                source: ContextSource::Note,
                layer: ContextLayer::Note,
                created_at: 2,
                ttl: None,
                hash: ContextFragment::compute_hash("a", &json!("v1")),
            },
            ContextFragment {
                id: "4".into(),
                key: "b".into(),
                value: json!("changed"),
                source: ContextSource::Note,
                layer: ContextLayer::Note,
                created_at: 2,
                ttl: None,
                hash: ContextFragment::compute_hash("b", &json!("changed")),
            },
        ];
        let diff = ContextDiff::compute(prev, curr);
        assert_eq!(diff.changed.len(), 1);
        assert_eq!(diff.drift_score, 0.5);
    }

    #[test]
    fn test_context_ref_serialization() {
        let r#ref = ContextRef {
            key: "note.content".to_string(),
            layer: ContextLayer::Note,
            hash: "abc123".to_string(),
        };
        let json_str = serde_json::to_string(&r#ref).unwrap();
        let parsed: ContextRef = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed.key, "note.content");
        assert_eq!(parsed.layer, ContextLayer::Note);
    }
}
