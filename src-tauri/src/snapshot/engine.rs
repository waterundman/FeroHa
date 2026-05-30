#![allow(dead_code)]

use crate::ai::embedding::EmbeddingPipeline;
use crate::harness::context::{ContextDiff, ContextFragment};
use crate::snapshot::store::SnapshotStore;
use crate::snapshot::{
    chunk_for_snapshot, BlockDiffEntry, DriftResult, Snapshot, SnapshotBlock, SnapshotMetadata,
    SnapshotType,
};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};

const DRIFT_THRESHOLD: f32 = 0.25;

pub struct SnapshotEngine {
    store: SnapshotStore,
    embedding_pipeline: Mutex<EmbeddingPipeline>,
}

impl SnapshotEngine {
    pub fn new(store: SnapshotStore, embedding_pipeline: EmbeddingPipeline) -> Self {
        SnapshotEngine {
            store,
            embedding_pipeline: Mutex::new(embedding_pipeline),
        }
    }

    pub fn init(&self) -> Result<(), std::io::Error> {
        self.store.init()
    }

    pub fn cleanup_expired(&self) -> usize {
        self.store.cleanup_expired(72 * 3600)
    }

    pub fn create_global_snapshot(
        &self,
        note_id: &str,
        content: &str,
        backlinks: &[String],
        app_handle: &AppHandle,
    ) -> Result<Snapshot, String> {
        let timestamp = chrono::Utc::now().timestamp_millis();
        let note_title = note_id
            .rsplit('/')
            .next()
            .unwrap_or(note_id)
            .trim_end_matches(".md");

        let prev = self.store.get_latest(note_id, &SnapshotType::Global);
        let prev_ts = prev.as_ref().map(|s| s.timestamp);

        let mut blocks = chunk_for_snapshot(content, note_id);

        let texts: Vec<String> = blocks.iter().map(|b| b.content.clone()).collect();
        let embeddings = {
            let pipe = self.embedding_pipeline.lock().map_err(|e| e.to_string())?;
            pipe.embed_sync(&texts)
        };

        for (i, emb) in embeddings.iter().enumerate() {
            if i < blocks.len() {
                blocks[i].embedding = emb.clone();
            }
        }

        let snapshot = Snapshot {
            note_id: note_id.to_string(),
            snapshot_type: SnapshotType::Global,
            timestamp,
            blocks,
            metadata: SnapshotMetadata {
                note_title: note_title.to_string(),
                total_blocks: texts.len(),
                total_chars: content.chars().count(),
                backlinks: backlinks.to_vec(),
                selection_range: None,
                previous_snapshot_ts: prev_ts,
            },
        };

        self.store.save(&snapshot)?;

        if let Some(prev_snap) = prev {
            let drift = self.detect_drift(&snapshot, &prev_snap);
            if drift.exceeded_threshold {
                let _ = app_handle.emit("snapshot-drift", &drift);
                tracing::info!(
                    "Global drift detected for {}: avg_cosine_distance={:.4}",
                    note_id,
                    drift.avg_cosine_distance
                );
            }
        }

        Ok(snapshot)
    }

    pub fn create_local_snapshot(
        &self,
        note_id: &str,
        content: &str,
        selection_range: (usize, usize),
        app_handle: &AppHandle,
    ) -> Result<Snapshot, String> {
        let timestamp = chrono::Utc::now().timestamp_millis();
        let note_title = note_id
            .rsplit('/')
            .next()
            .unwrap_or(note_id)
            .trim_end_matches(".md");

        let prev = self.store.get_latest(note_id, &SnapshotType::Local);
        let prev_ts = prev.as_ref().map(|s| s.timestamp);

        let mut blocks = chunk_for_snapshot(content, note_id);

        let texts: Vec<String> = blocks.iter().map(|b| b.content.clone()).collect();
        let embeddings = {
            let pipe = self.embedding_pipeline.lock().map_err(|e| e.to_string())?;
            pipe.embed_sync(&texts)
        };

        for (i, emb) in embeddings.iter().enumerate() {
            if i < blocks.len() {
                blocks[i].embedding = emb.clone();
            }
        }

        let snapshot = Snapshot {
            note_id: note_id.to_string(),
            snapshot_type: SnapshotType::Local,
            timestamp,
            blocks,
            metadata: SnapshotMetadata {
                note_title: note_title.to_string(),
                total_blocks: texts.len(),
                total_chars: content.chars().count(),
                backlinks: Vec::new(),
                selection_range: Some(selection_range),
                previous_snapshot_ts: prev_ts,
            },
        };

        self.store.save(&snapshot)?;

        if let Some(prev_snap) = prev {
            let drift = self.detect_drift(&snapshot, &prev_snap);
            if drift.exceeded_threshold {
                let _ = app_handle.emit("snapshot-drift", &drift);
                tracing::info!(
                    "Local drift detected for {}: avg_cosine_distance={:.4}",
                    note_id,
                    drift.avg_cosine_distance
                );
            }
        }

        Ok(snapshot)
    }

    pub fn get_current(&self, note_id: &str, snapshot_type: &SnapshotType) -> Option<Snapshot> {
        self.store.get_latest(note_id, snapshot_type)
    }

    pub fn get_diff(
        &self,
        note_id: &str,
        snapshot_type: &SnapshotType,
        ts1: Option<i64>,
        ts2: Option<i64>,
    ) -> Result<DriftResult, String> {
        let snap1 = if let Some(t) = ts1 {
            self.store.get_by_timestamp(note_id, snapshot_type, t)
        } else {
            self.store.get_latest(note_id, snapshot_type)
        };

        let snap2 = if let Some(t) = ts2 {
            self.store.get_by_timestamp(note_id, snapshot_type, t)
        } else {
            if snap1.is_some() {
                // Get second-latest by getting all and picking index 1
                let all = self.store.list_for_note(note_id, snapshot_type);
                if all.len() > 1 {
                    Some(all[1].clone())
                } else {
                    None
                }
            } else {
                None
            }
        };

        match (snap1, snap2) {
            (Some(s1), Some(s2)) => Ok(self.detect_drift(&s1, &s2)),
            _ => Err("Not enough snapshots for diff comparison".to_string()),
        }
    }

    fn detect_drift(&self, current: &Snapshot, previous: &Snapshot) -> DriftResult {
        let mut block_diffs: Vec<BlockDiffEntry> = Vec::new();
        let mut total_drift: f32 = 0.0;
        let mut compared: usize = 0;

        let prev_map: std::collections::HashMap<&str, &SnapshotBlock> = previous
            .blocks
            .iter()
            .map(|b| (b.block_id.as_str(), b))
            .collect();

        let curr_map: std::collections::HashMap<&str, &SnapshotBlock> = current
            .blocks
            .iter()
            .map(|b| (b.block_id.as_str(), b))
            .collect();

        // Compare matching blocks
        for curr_block in &current.blocks {
            if let Some(prev_block) = prev_map.get(curr_block.block_id.as_str()) {
                if !curr_block.embedding.is_empty() && !prev_block.embedding.is_empty() {
                    let drift =
                        1.0 - cosine_similarity(&curr_block.embedding, &prev_block.embedding);
                    total_drift += drift;
                    compared += 1;

                    let op = if drift > DRIFT_THRESHOLD {
                        "modified"
                    } else {
                        "unchanged"
                    };

                    block_diffs.push(BlockDiffEntry {
                        block_id: curr_block.block_id.clone(),
                        drift,
                        old_content: if op == "modified" {
                            Some(prev_block.content.clone())
                        } else {
                            None
                        },
                        new_content: if op == "modified" {
                            Some(curr_block.content.clone())
                        } else {
                            None
                        },
                        op: op.to_string(),
                    });
                } else if curr_block.content != prev_block.content {
                    total_drift += DRIFT_THRESHOLD + 0.01;
                    compared += 1;
                    block_diffs.push(BlockDiffEntry {
                        block_id: curr_block.block_id.clone(),
                        drift: DRIFT_THRESHOLD + 0.01,
                        old_content: Some(prev_block.content.clone()),
                        new_content: Some(curr_block.content.clone()),
                        op: "modified".to_string(),
                    });
                }
            }
        }

        // Detect added blocks
        for curr_block in &current.blocks {
            if !prev_map.contains_key(curr_block.block_id.as_str()) {
                block_diffs.push(BlockDiffEntry {
                    block_id: curr_block.block_id.clone(),
                    drift: 1.0,
                    old_content: None,
                    new_content: Some(curr_block.content.clone()),
                    op: "added".to_string(),
                });
                total_drift += 1.0;
                compared += 1;
            }
        }

        // Detect removed blocks
        for prev_block in &previous.blocks {
            if !curr_map.contains_key(prev_block.block_id.as_str()) {
                block_diffs.push(BlockDiffEntry {
                    block_id: prev_block.block_id.clone(),
                    drift: 1.0,
                    old_content: Some(prev_block.content.clone()),
                    new_content: None,
                    op: "removed".to_string(),
                });
                total_drift += 1.0;
                compared += 1;
            }
        }

        let avg_drift = if compared > 0 {
            total_drift / compared as f32
        } else {
            0.0
        };

        let prev_ctx: Vec<ContextFragment> = previous
            .blocks
            .iter()
            .flat_map(|b| b.context.clone())
            .collect();
        let curr_ctx: Vec<ContextFragment> = current
            .blocks
            .iter()
            .flat_map(|b| b.context.clone())
            .collect();

        let has_context = !prev_ctx.is_empty() || !curr_ctx.is_empty();
        let context_drift = if has_context {
            ContextDiff::compute(prev_ctx, curr_ctx).drift_score
        } else {
            0.0
        };

        let combined_drift = 0.7 * avg_drift + 0.3 * context_drift;

        let type_str = match current.snapshot_type {
            SnapshotType::Global => "global",
            SnapshotType::Local => "local",
        };

        DriftResult {
            avg_cosine_distance: combined_drift,
            exceeded_threshold: combined_drift > DRIFT_THRESHOLD,
            block_diffs,
            note_id: current.note_id.clone(),
            snapshot_type: type_str.to_string(),
            prev_timestamp: previous.timestamp,
            curr_timestamp: current.timestamp,
            context_drift_score: if has_context {
                Some(context_drift)
            } else {
                None
            },
        }
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        (dot / (na * nb)).clamp(-1.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::embedding::EmbeddingBackend;

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0];
        let b = vec![1.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.001);

        let c = vec![0.0, 1.0];
        assert!((cosine_similarity(&a, &c) - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_create_and_detect_drift() {
        let tmp = tempfile::tempdir().unwrap();
        let store = SnapshotStore::new(tmp.path());
        let pipe = EmbeddingPipeline::new(EmbeddingBackend::None);
        let engine = SnapshotEngine::new(store, pipe);
        engine.init().unwrap();

        // Can't easily test AppHandle emission in unit test,
        // but we can verify snapshot creation works
        let meta = SnapshotMetadata {
            note_title: "T".to_string(),
            total_blocks: 0,
            total_chars: 0,
            backlinks: vec![],
            selection_range: None,
            previous_snapshot_ts: None,
        };

        let s1 = Snapshot {
            note_id: "a.md".to_string(),
            snapshot_type: SnapshotType::Global,
            timestamp: 1000,
            blocks: vec![SnapshotBlock {
                block_id: "b1".into(),
                content: "hello".into(),
                embedding: vec![1.0, 0.0, 0.0],
                heading_context: String::new(),
                context: vec![],
            }],
            metadata: meta.clone(),
        };

        let s2 = Snapshot {
            note_id: "a.md".to_string(),
            snapshot_type: SnapshotType::Global,
            timestamp: 2000,
            blocks: vec![SnapshotBlock {
                block_id: "b1".into(),
                content: "world".into(),
                embedding: vec![1.0, 0.0, 0.0],
                heading_context: String::new(),
                context: vec![],
            }],
            metadata: meta,
        };

        engine.store.save(&s1).unwrap();
        let drift = engine.detect_drift(&s2, &s1);
        assert_eq!(drift.exceeded_threshold, false);
    }

    #[test]
    fn test_detect_added_blocks() {
        let tmp = tempfile::tempdir().unwrap();
        let store = SnapshotStore::new(tmp.path());
        let pipe = EmbeddingPipeline::new(EmbeddingBackend::None);
        let engine = SnapshotEngine::new(store, pipe);
        engine.init().unwrap();

        let meta = SnapshotMetadata {
            note_title: "T".to_string(),
            total_blocks: 0,
            total_chars: 0,
            backlinks: vec![],
            selection_range: None,
            previous_snapshot_ts: None,
        };

        let s1 = Snapshot {
            note_id: "a.md".into(),
            snapshot_type: SnapshotType::Global,
            timestamp: 1000,
            blocks: vec![],
            metadata: meta.clone(),
        };

        let s2 = Snapshot {
            note_id: "a.md".into(),
            snapshot_type: SnapshotType::Global,
            timestamp: 2000,
            blocks: vec![SnapshotBlock {
                block_id: "new".into(),
                content: "x".into(),
                embedding: vec![1.0],
                heading_context: String::new(),
                context: vec![],
            }],
            metadata: meta,
        };

        let drift = engine.detect_drift(&s2, &s1);
        assert!(drift.avg_cosine_distance > DRIFT_THRESHOLD);
        assert!(drift.block_diffs.iter().any(|d| d.op == "added"));
    }
}
