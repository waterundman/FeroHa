pub mod commands;
pub mod engine;
pub mod store;

use crate::ai::chunker::chunk_markdown;
use crate::harness::context::ContextFragment;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SnapshotType {
    Global,
    Local,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotBlock {
    pub block_id: String,
    pub content: String,
    pub embedding: Vec<f32>,
    pub heading_context: String,
    #[serde(default)]
    pub context: Vec<ContextFragment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotMetadata {
    pub note_title: String,
    pub total_blocks: usize,
    pub total_chars: usize,
    pub backlinks: Vec<String>,
    pub selection_range: Option<(usize, usize)>,
    pub previous_snapshot_ts: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub note_id: String,
    pub snapshot_type: SnapshotType,
    pub timestamp: i64,
    pub blocks: Vec<SnapshotBlock>,
    pub metadata: SnapshotMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockDiffEntry {
    pub block_id: String,
    pub drift: f32,
    pub old_content: Option<String>,
    pub new_content: Option<String>,
    #[serde(rename = "op")]
    pub op: String, // "added" | "removed" | "modified" | "unchanged"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftResult {
    pub avg_cosine_distance: f32,
    pub exceeded_threshold: bool,
    pub block_diffs: Vec<BlockDiffEntry>,
    pub note_id: String,
    pub snapshot_type: String,
    pub prev_timestamp: i64,
    pub curr_timestamp: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_drift_score: Option<f32>,
}

impl Snapshot {
    pub fn file_name(&self) -> String {
        let type_prefix = match self.snapshot_type {
            SnapshotType::Global => "global",
            SnapshotType::Local => "local",
        };
        format!("{}_{}.json", type_prefix, self.timestamp)
    }
}

pub fn chunk_for_snapshot(content: &str, source_file: &str) -> Vec<SnapshotBlock> {
    let chunks = chunk_markdown(content, source_file);
    chunks
        .into_iter()
        .map(|c| SnapshotBlock {
            block_id: c.id,
            content: c.text,
            embedding: Vec::new(),
            heading_context: c.heading_context,
            context: vec![],
        })
        .collect()
}
