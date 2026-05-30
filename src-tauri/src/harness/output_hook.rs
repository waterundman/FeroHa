use crate::harness::scientist::ScientistResult;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HookTrigger {
    OnRefineComplete,
    OnDreamCycle,
    OnInsight,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum HookTarget {
    HttpWebhook { url: String },
    StdioCommand { program: String, args: Vec<String> },
    FileSink { path: PathBuf },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RAGFilter {
    #[serde(default = "default_min_confidence")]
    pub min_confidence: f32,
    #[serde(default = "default_max_chunks")]
    pub max_chunks: usize,
    #[serde(default)]
    pub topic_keywords: Vec<String>,
}

fn default_min_confidence() -> f32 {
    0.7
}
fn default_max_chunks() -> usize {
    10
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputHook {
    pub id: String,
    pub name: String,
    pub trigger: HookTrigger,
    pub target: HookTarget,
    #[serde(default)]
    pub filter: RAGFilter,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputPayload {
    pub source: String,
    pub timestamp: u64,
    pub trigger: String,
    pub topic: String,
    pub claims: Vec<OutputClaim>,
    pub related_notes: Vec<String>,
    pub suggested_actions: Vec<String>,
    pub overall_confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputClaim {
    pub content: String,
    pub confidence: f32,
    pub source_note: String,
}

pub struct OutputManager {
    hooks: Vec<OutputHook>,
}

impl OutputManager {
    pub fn new() -> Self {
        Self { hooks: vec![] }
    }

    pub fn load_defaults(dualtrack_dir: &PathBuf) -> Self {
        let output_dir = dualtrack_dir.join("output");
        Self {
            hooks: vec![OutputHook {
                id: "default-file-sink".to_string(),
                name: "Default File Output".to_string(),
                trigger: HookTrigger::OnRefineComplete,
                target: HookTarget::FileSink { path: output_dir },
                filter: RAGFilter {
                    min_confidence: 0.7,
                    max_chunks: 10,
                    topic_keywords: vec![],
                },
                enabled: true,
            }],
        }
    }

    pub fn add_hook(&mut self, hook: OutputHook) {
        self.hooks.push(hook);
    }

    pub fn remove_hook(&mut self, id: &str) -> bool {
        if let Some(pos) = self.hooks.iter().position(|h| h.id == id) {
            self.hooks.remove(pos);
            true
        } else {
            false
        }
    }

    pub fn list_hooks(&self) -> Vec<OutputHook> {
        self.hooks.clone()
    }

    pub async fn trigger(
        &self,
        event: &HookTrigger,
        scientist_result: &ScientistResult,
        topic: &str,
        dualtrack_dir: &PathBuf,
    ) {
        for hook in &self.hooks {
            if !hook.enabled {
                continue;
            }
            if std::mem::discriminant(&hook.trigger) != std::mem::discriminant(event) {
                continue;
            }

            let payload = self.build_payload(scientist_result, topic, event);

            self.dispatch(&hook.target, &payload, dualtrack_dir).await;
        }
    }

    fn build_payload(
        &self,
        result: &ScientistResult,
        topic: &str,
        trigger: &HookTrigger,
    ) -> OutputPayload {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let claims: Vec<OutputClaim> = result
            .clean_knowledge
            .claims
            .iter()
            .enumerate()
            .filter_map(|(i, claim)| {
                let confidence = result
                    .clean_knowledge
                    .confidence_map
                    .values()
                    .nth(i)
                    .copied()
                    .unwrap_or(0.5);
                if confidence < 0.5 {
                    return None;
                }
                let source_note = result
                    .clean_knowledge
                    .sources
                    .get(i)
                    .map(|s| s.key.clone())
                    .unwrap_or_else(|| "unknown".to_string());
                Some(OutputClaim {
                    content: claim.clone(),
                    confidence,
                    source_note,
                })
            })
            .take(20)
            .collect();

        let related_notes: Vec<String> = result
            .clean_knowledge
            .sources
            .iter()
            .map(|s| s.key.clone())
            .collect();

        let suggested_actions = if let Some(ref v) = result.verification {
            v.violations
                .iter()
                .map(|v| format!("Investigate: {:?}", v))
                .collect()
        } else {
            vec![]
        };

        OutputPayload {
            source: format!("feroha-scientist-v{}", env!("CARGO_PKG_VERSION")),
            timestamp: now,
            trigger: format!("{:?}", trigger),
            topic: topic.to_string(),
            claims,
            related_notes,
            suggested_actions,
            overall_confidence: result.overall_confidence,
        }
    }

    async fn dispatch(
        &self,
        target: &HookTarget,
        payload: &OutputPayload,
        dualtrack_dir: &PathBuf,
    ) {
        let json = serde_json::to_string_pretty(payload).unwrap_or_default();

        match target {
            HookTarget::FileSink { path } => {
                let dir = if path.is_relative() {
                    dualtrack_dir.join(path)
                } else {
                    path.clone()
                };
                std::fs::create_dir_all(&dir).ok();
                let filename = format!(
                    "output_{}.json",
                    chrono::Local::now().format("%Y%m%d_%H%M%S")
                );
                let filepath = dir.join(filename);
                std::fs::write(&filepath, &json).ok();
                tracing::info!("OutputHook: wrote to {}", filepath.display());
            }
            HookTarget::HttpWebhook { url } => {
                let client = reqwest::Client::new();
                let _ = client
                    .post(url)
                    .header("Content-Type", "application/json")
                    .body(json)
                    .send()
                    .await;
            }
            HookTarget::StdioCommand { program, args } => {
                let child = std::process::Command::new(program)
                    .args(args)
                    .stdin(std::process::Stdio::piped())
                    .spawn();
                if let Ok(mut child) = child {
                    use std::io::Write;
                    if let Some(stdin) = child.stdin.as_mut() {
                        let _ = stdin.write_all(json.as_bytes());
                    }
                    let _ = child.wait();
                }
            }
        }
    }
}
