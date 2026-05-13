// Ghost Store — AI-generated suggestion storage for Diff/Merge workflow
// Ghost notes are NOT written to user files until explicitly accepted via merge

use serde::{Serialize, Deserialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhostNote {
    pub id: String,
    pub source_note: String,
    pub task_description: String,
    pub suggested_blocks: Vec<GhostBlock>,
    pub created_at: i64,
    pub status: GhostStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhostBlock {
    pub block_id: String,
    pub content: String,
    pub operation: GhostOp,
    pub after_block_id: Option<String>,
    pub heading_context: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GhostOp {
    Insert,
    Modify,
    Delete,
    Suggestion,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GhostStatus {
    Pending,
    PartiallyAccepted,
    Accepted,
    Rejected,
}

pub struct GhostStore {
    store_dir: PathBuf,
}

impl GhostStore {
    pub fn new(dualtrack_dir: &Path) -> Self {
        let store_dir = dualtrack_dir.join("ghosts");
        GhostStore { store_dir }
    }

    pub fn init(&self) -> Result<(), std::io::Error> {
        fs::create_dir_all(&self.store_dir)
    }

    /// Create a new ghost note for a source file
    pub fn create(
        &self,
        source_note: &str,
        task_description: &str,
        suggested_blocks: Vec<GhostBlock>,
    ) -> Result<GhostNote, String> {
        let id = format!("ghost_{}", uuid::Uuid::new_v4().to_string().replace('-', ""));
        let ghost = GhostNote {
            id: id.clone(),
            source_note: source_note.to_string(),
            task_description: task_description.to_string(),
            suggested_blocks,
            created_at: chrono::Utc::now().timestamp_millis(),
            status: GhostStatus::Pending,
        };

        let file_path = self.ghost_file_path(&id);
        let dir = file_path.parent().unwrap();
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;

        let json = serde_json::to_string_pretty(&ghost).map_err(|e| e.to_string())?;
        fs::write(&file_path, json).map_err(|e| e.to_string())?;

        Ok(ghost)
    }

    /// List all ghost notes for a source file
    pub fn list_for_file(&self, source_note: &str) -> Vec<GhostNote> {
        let mut ghosts = Vec::new();
        if let Ok(entries) = fs::read_dir(&self.store_dir) {
            for entry in entries.flatten() {
                if let Ok(content) = fs::read_to_string(entry.path()) {
                    if let Ok(mut ghost) = serde_json::from_str::<GhostNote>(&content) {
                        if ghost.source_note == source_note
                            && matches!(ghost.status, GhostStatus::Pending | GhostStatus::PartiallyAccepted)
                        {
                            // Use filename as fallback ID
                            if ghost.id.is_empty() {
                                ghost.id = entry
                                    .file_name()
                                    .to_string_lossy()
                                    .trim_end_matches(".json")
                                    .to_string();
                            }
                            ghosts.push(ghost);
                        }
                    }
                }
            }
        }
        ghosts.sort_by_key(|g| -g.created_at);
        ghosts
    }

    /// List all pending ghost notes
    pub fn list_all(&self) -> Vec<GhostNote> {
        let mut ghosts = Vec::new();
        if let Ok(entries) = fs::read_dir(&self.store_dir) {
            for entry in entries.flatten() {
                if let Ok(content) = fs::read_to_string(entry.path()) {
                    if let Ok(mut ghost) = serde_json::from_str::<GhostNote>(&content) {
                        if matches!(ghost.status, GhostStatus::Pending | GhostStatus::PartiallyAccepted) {
                            if ghost.id.is_empty() {
                                ghost.id = entry
                                    .file_name()
                                    .to_string_lossy()
                                    .trim_end_matches(".json")
                                    .to_string();
                            }
                            ghosts.push(ghost);
                        }
                    }
                }
            }
        }
        ghosts
    }

    /// Get a specific ghost note by ID
    pub fn get(&self, id: &str) -> Option<GhostNote> {
        let file_path = self.ghost_file_path(id);
        fs::read_to_string(&file_path)
            .ok()
            .and_then(|content| serde_json::from_str::<GhostNote>(&content).ok())
    }

    /// Update ghost status
    pub fn update_status(&self, id: &str, status: GhostStatus) -> Result<(), String> {
        let file_path = self.ghost_file_path(id);
        let content = fs::read_to_string(&file_path).map_err(|e| e.to_string())?;
        let mut ghost: GhostNote = serde_json::from_str(&content).map_err(|e| e.to_string())?;
        ghost.status = status;
        let json = serde_json::to_string_pretty(&ghost).map_err(|e| e.to_string())?;
        fs::write(&file_path, json).map_err(|e| e.to_string())
    }

    fn ghost_file_path(&self, id: &str) -> PathBuf {
        self.store_dir.join(format!("{}.json", id))
    }
}
