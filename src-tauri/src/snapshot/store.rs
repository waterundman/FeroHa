#![allow(dead_code)]

use super::{Snapshot, SnapshotType};
use std::fs;
use std::path::{Path, PathBuf};

pub struct SnapshotStore {
    snapshots_dir: PathBuf,
}

impl SnapshotStore {
    pub fn new(dualtrack_dir: &Path) -> Self {
        let snapshots_dir = dualtrack_dir.join("snapshots");
        SnapshotStore { snapshots_dir }
    }

    pub fn init(&self) -> Result<(), std::io::Error> {
        fs::create_dir_all(&self.snapshots_dir)
    }

    fn note_dir(&self, note_id: &str) -> PathBuf {
        let safe_name = note_id.replace(['\\', '/', '.', ':', '*', '?', '"', '<', '>', '|'], "_");
        self.snapshots_dir.join(&safe_name)
    }

    pub fn save(&self, snapshot: &Snapshot) -> Result<(), String> {
        let dir = self.note_dir(&snapshot.note_id);
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

        let file_path = dir.join(snapshot.file_name());
        let json = serde_json::to_string_pretty(snapshot).map_err(|e| e.to_string())?;
        fs::write(&file_path, json).map_err(|e| e.to_string())
    }

    pub fn get_latest(&self, note_id: &str, snapshot_type: &SnapshotType) -> Option<Snapshot> {
        let dir = self.note_dir(note_id);
        if !dir.exists() {
            return None;
        }

        let type_prefix = match snapshot_type {
            SnapshotType::Global => "global_",
            SnapshotType::Local => "local_",
        };

        let mut snapshots: Vec<(i64, PathBuf)> = Vec::new();
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with(type_prefix) && name.ends_with(".json") {
                    let ts_str = name
                        .trim_start_matches(type_prefix)
                        .trim_end_matches(".json");
                    if let Ok(ts) = ts_str.parse::<i64>() {
                        snapshots.push((ts, entry.path()));
                    }
                }
            }
        }

        snapshots.sort_by_key(|(ts, _)| -ts);

        snapshots
            .first()
            .and_then(|(_, path)| self.load_from_path(path))
    }

    pub fn get_by_timestamp(
        &self,
        note_id: &str,
        snapshot_type: &SnapshotType,
        timestamp: i64,
    ) -> Option<Snapshot> {
        let dir = self.note_dir(note_id);
        let type_prefix = match snapshot_type {
            SnapshotType::Global => "global_",
            SnapshotType::Local => "local_",
        };
        let file_path = dir.join(format!("{}_{}.json", type_prefix, timestamp));
        self.load_from_path(&file_path)
    }

    fn load_from_path(&self, path: &Path) -> Option<Snapshot> {
        fs::read_to_string(path)
            .ok()
            .and_then(|content| serde_json::from_str::<Snapshot>(&content).ok())
    }

    pub fn list_for_note(&self, note_id: &str, snapshot_type: &SnapshotType) -> Vec<Snapshot> {
        let dir = self.note_dir(note_id);
        if !dir.exists() {
            return Vec::new();
        }

        let type_prefix = match snapshot_type {
            SnapshotType::Global => "global_",
            SnapshotType::Local => "local_",
        };

        let mut snapshots: Vec<Snapshot> = Vec::new();
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with(type_prefix) && name.ends_with(".json") {
                    if let Some(s) = self.load_from_path(&entry.path()) {
                        snapshots.push(s);
                    }
                }
            }
        }

        snapshots.sort_by_key(|s| -s.timestamp);
        snapshots
    }

    pub fn cleanup_expired(&self, ttl_secs: i64) -> usize {
        let now = chrono::Utc::now().timestamp_millis();
        let cutoff = now - (ttl_secs * 1000);
        let mut deleted: usize = 0;

        if !self.snapshots_dir.exists() {
            return 0;
        }

        if let Ok(note_dirs) = fs::read_dir(&self.snapshots_dir) {
            for note_dir in note_dirs.flatten() {
                if let Ok(snapshot_files) = fs::read_dir(note_dir.path()) {
                    let mut dir_empty = true;
                    for snap_file in snapshot_files.flatten() {
                        let name = snap_file.file_name().to_string_lossy().to_string();
                        if name.ends_with(".json") {
                            let ts_str = name
                                .split('_')
                                .nth(1)
                                .and_then(|s| s.trim_end_matches(".json").parse::<i64>().ok());

                            if let Some(ts) = ts_str {
                                if ts < cutoff {
                                    let _ = fs::remove_file(snap_file.path());
                                    deleted += 1;
                                } else {
                                    dir_empty = false;
                                }
                            }
                        }
                    }
                    if dir_empty {
                        let _ = fs::remove_dir(note_dir.path());
                    }
                }
            }
        }

        if deleted > 0 {
            tracing::info!(
                "Snapshot TTL cleanup: removed {} expired snapshots",
                deleted
            );
        }

        deleted
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::{SnapshotBlock, SnapshotMetadata};

    #[test]
    fn test_save_and_load() {
        let tmp = tempfile::tempdir().unwrap();
        let store = SnapshotStore::new(tmp.path());
        store.init().unwrap();

        let snapshot = Snapshot {
            note_id: "test.md".to_string(),
            snapshot_type: SnapshotType::Global,
            timestamp: 1000,
            blocks: vec![SnapshotBlock {
                block_id: "b1".to_string(),
                content: "hello".to_string(),
                embedding: vec![],
                heading_context: String::new(),
                context: vec![],
            }],
            metadata: SnapshotMetadata {
                note_title: "Test".to_string(),
                total_blocks: 1,
                total_chars: 5,
                backlinks: vec![],
                selection_range: None,
                previous_snapshot_ts: None,
            },
        };

        store.save(&snapshot).unwrap();
        let loaded = store.get_latest("test.md", &SnapshotType::Global);
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().timestamp, 1000);
    }

    #[test]
    fn test_get_latest_returns_most_recent() {
        let tmp = tempfile::tempdir().unwrap();
        let store = SnapshotStore::new(tmp.path());
        store.init().unwrap();

        for ts in &[1000, 2000, 3000] {
            let s = Snapshot {
                note_id: "t.md".to_string(),
                snapshot_type: SnapshotType::Global,
                timestamp: *ts,
                blocks: vec![],
                metadata: SnapshotMetadata {
                    note_title: "T".to_string(),
                    total_blocks: 0,
                    total_chars: 0,
                    backlinks: vec![],
                    selection_range: None,
                    previous_snapshot_ts: None,
                },
            };
            store.save(&s).unwrap();
        }

        let latest = store.get_latest("t.md", &SnapshotType::Global);
        assert_eq!(latest.unwrap().timestamp, 3000);
    }

    #[test]
    fn test_local_and_global_independent() {
        let tmp = tempfile::tempdir().unwrap();
        let store = SnapshotStore::new(tmp.path());
        store.init().unwrap();

        let meta = SnapshotMetadata {
            note_title: "T".to_string(),
            total_blocks: 0,
            total_chars: 0,
            backlinks: vec![],
            selection_range: None,
            previous_snapshot_ts: None,
        };

        store
            .save(&Snapshot {
                note_id: "x.md".into(),
                snapshot_type: SnapshotType::Global,
                timestamp: 1000,
                blocks: vec![],
                metadata: meta.clone(),
            })
            .unwrap();
        store
            .save(&Snapshot {
                note_id: "x.md".into(),
                snapshot_type: SnapshotType::Local,
                timestamp: 2000,
                blocks: vec![],
                metadata: meta.clone(),
            })
            .unwrap();

        let global = store.get_latest("x.md", &SnapshotType::Global).unwrap();
        let local = store.get_latest("x.md", &SnapshotType::Local).unwrap();
        assert_eq!(global.timestamp, 1000);
        assert_eq!(local.timestamp, 2000);
    }

    #[test]
    fn test_cleanup_expired() {
        let tmp = tempfile::tempdir().unwrap();
        let store = SnapshotStore::new(tmp.path());
        store.init().unwrap();

        let meta = SnapshotMetadata {
            note_title: "T".to_string(),
            total_blocks: 0,
            total_chars: 0,
            backlinks: vec![],
            selection_range: None,
            previous_snapshot_ts: None,
        };

        let old_ts = chrono::Utc::now().timestamp_millis() - 300_000_000;
        store
            .save(&Snapshot {
                note_id: "x.md".into(),
                snapshot_type: SnapshotType::Global,
                timestamp: old_ts,
                blocks: vec![],
                metadata: meta,
            })
            .unwrap();

        let deleted = store.cleanup_expired(72 * 3600);
        assert!(deleted >= 1);
    }
}
