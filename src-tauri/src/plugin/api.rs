// Plugin API trait — Interface contracts for plugin development
// Stage 6: API surface definition
#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// Lifecycle trait implemented by every plugin
pub trait Plugin: Send + Sync {
    fn init(&mut self, api: PluginApi) -> Result<(), PluginError>;
    fn on_enable(&mut self) -> Result<(), PluginError> {
        Ok(())
    }
    fn on_disable(&mut self) -> Result<(), PluginError> {
        Ok(())
    }
    fn on_tick(&mut self) -> Result<(), PluginError> {
        Ok(())
    }
    fn on_uninstall(&mut self) -> Result<(), PluginError> {
        Ok(())
    }
    fn id(&self) -> &str;
    fn version(&self) -> &str;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginApi {
    pub note: NoteApiClient,
    pub search: SearchApiClient,
    pub ai: AiApiClient,
    pub graph: GraphApiClient,
    pub ui: UiApiClient,
    pub storage: StorageApiClient,
}

#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    #[error("IO error: {0}")]
    Io(String),
    #[error("Network error: {0}")]
    Network(String),
    #[error("Plugin error: {0}")]
    Other(String),
}

// ── Note API ──────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteApiClient;

impl NoteApiClient {
    pub fn read(&self, _path: &str) -> Result<String, PluginError> {
        Err(PluginError::Other(
            "NoteApi::read not connected".to_string(),
        ))
    }
    pub fn list(&self) -> Result<Vec<String>, PluginError> {
        Err(PluginError::Other(
            "NoteApi::list not connected".to_string(),
        ))
    }
    pub fn write(&self, _path: &str, _content: &str) -> Result<(), PluginError> {
        Err(PluginError::Other(
            "NoteApi::write not connected".to_string(),
        ))
    }
}

// ── Search API ────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchApiClient;

impl SearchApiClient {
    pub fn search(&self, _query: &str, _top_k: usize) -> Result<Vec<String>, PluginError> {
        Err(PluginError::Other(
            "SearchApi::search not connected".to_string(),
        ))
    }
}

// ── AI API ────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiApiClient;

impl AiApiClient {
    pub fn submit_task(&self, _task: String) -> Result<String, PluginError> {
        Err(PluginError::Other(
            "AiApi::submit_task not connected".to_string(),
        ))
    }
}

// ── Graph API ─────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphApiClient;

impl GraphApiClient {
    pub fn get_neighbors(&self, _node: &str) -> Result<Vec<String>, PluginError> {
        Err(PluginError::Other(
            "GraphApi::get_neighbors not connected".to_string(),
        ))
    }
}

// ── UI API ────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiApiClient;

impl UiApiClient {
    pub fn notify(&self, _title: &str, _message: &str) {}
    pub fn register_view(&self, _config: ViewConfig) -> Result<String, PluginError> {
        Ok(String::new())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViewConfig {
    pub id: String,
    pub title: String,
    pub icon: String,
    pub position: ViewPosition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ViewPosition {
    Sidebar,
    Panel,
    Modal,
}

// ── Storage API ───────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageApiClient;

impl StorageApiClient {
    pub fn get(&self, _key: &str) -> Result<Option<String>, PluginError> {
        Ok(None)
    }
    pub fn set(&self, _key: &str, _value: &str) -> Result<(), PluginError> {
        Ok(())
    }
    pub fn delete(&self, _key: &str) -> Result<(), PluginError> {
        Ok(())
    }
}
