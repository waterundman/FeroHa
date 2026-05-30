use crate::ai::agent_scheduler::AgentScheduler;
use crate::ai::dream_engine::DreamEngine;
use crate::ai::embedding::EmbeddingPipeline;
use crate::ai::llm_router::LlmRouter;
use crate::ai::search_engine::SearchEngine;
use crate::ai::skill_manager::SkillManager;
use crate::ai::subagent::Subagent;
use crate::ai::sync_engine::SyncEngine;
use crate::ai::task_scheduler::TaskScheduler;
use crate::diff::ghost_store::GhostStore;
use crate::fs::vault::VaultManager;
use crate::fs::watcher::FileWatcher;
use crate::graph::link_graph::LinkGraph;
use crate::ipc::protocol::TwoSurfaceProtocol;
use crate::snapshot::engine::SnapshotEngine;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Notify;

pub struct AppState {
    pub vault: Option<VaultManager>,
    pub file_watcher: Option<FileWatcher>,
    pub link_graph: LinkGraph,
    pub vault_path: String,
    pub sync_engine: Option<SyncEngine>,
    pub vector_store_path: PathBuf,
    pub dualtrack_dir: PathBuf,
    pub snapshot_engine: Option<SnapshotEngine>,
    pub dream_engine: Option<DreamEngine>,
    pub protocol: Option<TwoSurfaceProtocol>,
    pub search_engine: Option<Arc<SearchEngine>>,
    pub output_manager: Option<std::sync::Arc<crate::harness::output_hook::OutputManager>>,
    pub bridge_store: Option<crate::bridge::store::BridgeProposalStore>,
}

pub struct AiState {
    pub agent_scheduler: AgentScheduler,
    pub llm_router: LlmRouter,
    pub embedding_pipeline: EmbeddingPipeline,
    pub ghost_store: GhostStore,
    pub subagent: Option<Subagent>,
    pub skill_manager: Option<SkillManager>,
    pub scheduler: Option<Arc<TaskScheduler>>,
    pub task_notifier: Arc<Notify>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct AppConfig {
    pub llm_provider: String,
    pub llm_api_key: String,
    pub llm_model: String,
    pub embedding_provider: String,
    pub embedding_api_key: String,
    pub theme: String,
    #[serde(default = "default_ollama_base_url")]
    pub ollama_base_url: String,
    #[serde(default = "default_true")]
    pub orchestrator_auto_terminate: bool,
    #[serde(default = "default_three")]
    pub orchestrator_max_consecutive: usize,
    #[serde(default = "default_sixty_thousand")]
    pub orchestrator_cooldown_ms: u64,
}

fn default_ollama_base_url() -> String {
    "http://localhost:11434".to_string()
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            llm_provider: "gemini".to_string(),
            llm_api_key: String::new(),
            llm_model: "gemini-2.0-flash".to_string(),
            embedding_provider: "none".to_string(),
            embedding_api_key: String::new(),
            theme: "feroha".to_string(),
            ollama_base_url: "http://localhost:11434".to_string(),
            orchestrator_auto_terminate: true,
            orchestrator_max_consecutive: 3,
            orchestrator_cooldown_ms: 60000,
        }
    }
}

fn default_true() -> bool {
    true
}
fn default_three() -> usize {
    3
}
fn default_sixty_thousand() -> u64 {
    60000
}
