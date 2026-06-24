use crate::ai::agent_scheduler::AgentScheduler;
use crate::ai::dream_engine::DreamEngine;
use crate::ai::embedding::{EmbeddingBackend, EmbeddingPipeline};
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
    pub snapshot_listeners_started: bool,
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
    pub task_worker_started: bool,
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

impl AppConfig {
    pub fn from_env() -> Self {
        let mut config = AppConfig::default();
        let mut pairs = read_dotenv_pairs();
        pairs.extend(std::env::vars());
        config.apply_env_pairs(pairs);
        config
    }

    pub fn from_env_pairs<I, K, V>(pairs: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<str>,
        V: AsRef<str>,
    {
        let mut config = AppConfig::default();
        config.apply_env_pairs(pairs);
        config
    }

    pub fn to_router_config(&self) -> crate::ai::llm_router::RouterConfig {
        crate::ai::llm_router::RouterConfig {
            llm_provider: self.llm_provider.clone(),
            llm_api_key: self.llm_api_key.clone(),
            llm_model: self.llm_model.clone(),
            embedding_provider: self.embedding_provider.clone(),
            embedding_api_key: self.embedding_api_key.clone(),
            ollama_base_url: self.ollama_base_url.clone(),
            ..Default::default()
        }
    }

    pub fn to_embedding_backend(&self) -> EmbeddingBackend {
        match self.embedding_provider.as_str() {
            "openai" => EmbeddingBackend::OpenAi {
                api_key: self.embedding_api_key.clone(),
                model: "text-embedding-3-small".to_string(),
            },
            "gemini" => EmbeddingBackend::Gemini {
                api_key: self.embedding_api_key.clone(),
            },
            _ => EmbeddingBackend::None,
        }
    }

    fn apply_env_pairs<I, K, V>(&mut self, pairs: I)
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<str>,
        V: AsRef<str>,
    {
        let mut env = std::collections::HashMap::<String, String>::new();
        for (key, value) in pairs {
            let key = key.as_ref().trim();
            let value = value.as_ref().trim();
            if !key.is_empty() && !value.is_empty() {
                env.insert(key.to_string(), value.to_string());
            }
        }

        if let Some(provider) = env
            .get("FEROHA_LLM_PROVIDER")
            .and_then(|raw| normalize_llm_provider(raw, env.get("FEROHA_LLM_MODEL").map(String::as_str)))
        {
            self.llm_provider = provider;
        }
        if let Some(value) = env.get("FEROHA_LLM_API_KEY") {
            self.llm_api_key = strip_env_quotes(value);
        }
        if let Some(value) = env.get("FEROHA_LLM_MODEL") {
            self.llm_model = strip_env_quotes(value);
        }
        if let Some(value) = env
            .get("FEROHA_EMBEDDING_PROVIDER")
            .and_then(|raw| normalize_embedding_provider(raw))
        {
            self.embedding_provider = value;
        }
        if let Some(value) = env.get("FEROHA_EMBEDDING_API_KEY") {
            self.embedding_api_key = strip_env_quotes(value);
        }
        if let Some(value) = env.get("FEROHA_OLLAMA_BASE_URL") {
            self.ollama_base_url = strip_env_quotes(value);
        }
    }
}

fn read_dotenv_pairs() -> Vec<(String, String)> {
    let Some(path) = find_dotenv_path() else {
        return Vec::new();
    };
    let Ok(content) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    parse_dotenv_pairs(&content)
}

fn find_dotenv_path() -> Option<PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    loop {
        let candidate = dir.join(".env");
        if candidate.is_file() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

fn parse_dotenv_pairs(content: &str) -> Vec<(String, String)> {
    content
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let line = line.strip_prefix("export ").unwrap_or(line);
            let (key, value) = line.split_once('=')?;
            Some((key.trim().to_string(), strip_env_quotes(value.trim())))
        })
        .collect()
}

fn strip_env_quotes(value: &str) -> String {
    let value = value.trim();
    if value.len() >= 2 {
        let first = value.as_bytes()[0];
        let last = value.as_bytes()[value.len() - 1];
        if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
            return value[1..value.len() - 1].to_string();
        }
    }
    value.to_string()
}

fn normalize_llm_provider(raw: &str, model: Option<&str>) -> Option<String> {
    let raw = strip_env_quotes(raw).to_lowercase();
    let model = model.unwrap_or_default().to_lowercase();
    match raw.as_str() {
        "gemini" | "openai" | "deepseek" | "anthropic" | "ollama" => Some(raw),
        value if value.contains("deepseek") || model.contains("deepseek") => {
            Some("deepseek".to_string())
        }
        value if value.contains("openai") || model.starts_with("gpt-") => {
            Some("openai".to_string())
        }
        value if value.contains("anthropic") || value.contains("claude") || model.contains("claude") => {
            Some("anthropic".to_string())
        }
        value if value.contains("ollama") => Some("ollama".to_string()),
        value if value.starts_with("http://") || value.starts_with("https://") => None,
        value if !value.is_empty() => Some(value.to_string()),
        _ => None,
    }
}

fn normalize_embedding_provider(raw: &str) -> Option<String> {
    match strip_env_quotes(raw).to_lowercase().as_str() {
        "gemini" => Some("gemini".to_string()),
        "openai" => Some("openai".to_string()),
        "none" | "off" | "disabled" => Some("none".to_string()),
        _ => None,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_config_loads_feroha_env_values_and_normalizes_provider_url() {
        let config = AppConfig::from_env_pairs([
            ("FEROHA_LLM_PROVIDER", "https://api.deepseek.com"),
            ("FEROHA_LLM_API_KEY", "test-api-key"),
            ("FEROHA_LLM_MODEL", "deepseek-v4-flash"),
            ("FEROHA_EMBEDDING_PROVIDER", "openai"),
            ("FEROHA_EMBEDDING_API_KEY", "test-embedding-key"),
            ("FEROHA_OLLAMA_BASE_URL", "http://localhost:11435"),
        ]);

        assert_eq!(config.llm_provider, "deepseek");
        assert_eq!(config.llm_api_key, "test-api-key");
        assert_eq!(config.llm_model, "deepseek-v4-flash");
        assert_eq!(config.embedding_provider, "openai");
        assert_eq!(config.embedding_api_key, "test-embedding-key");
        assert_eq!(config.ollama_base_url, "http://localhost:11435");
    }

    #[test]
    fn app_config_ignores_empty_feroha_env_values() {
        let config = AppConfig::from_env_pairs([
            ("FEROHA_LLM_PROVIDER", ""),
            ("FEROHA_LLM_API_KEY", ""),
            ("FEROHA_LLM_MODEL", ""),
        ]);

        assert_eq!(config.llm_provider, "gemini");
        assert_eq!(config.llm_api_key, "");
        assert_eq!(config.llm_model, "gemini-2.0-flash");
    }
}
