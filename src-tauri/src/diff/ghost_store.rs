// Ghost Store — AI-generated suggestion storage for Diff/Merge workflow
// Ghost notes are NOT written to user files until explicitly accepted via merge

use crate::harness::context::{ContextDiff, ContextFragment};
use crate::harness::lean_kernel::VerificationResult;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhostNote {
    pub id: String,
    pub task_id: Option<String>,
    pub source_note: String,
    pub task_description: String,
    pub suggested_blocks: Vec<GhostBlock>,
    pub created_at: i64,
    pub status: GhostStatus,
    // 新增字段
    /// 优先级 (0-100, 越高越优先)
    pub priority: u8,
    /// 过期时间
    pub expires_at: Option<i64>,
    /// 关联的其他Ghost IDs (用于协调)
    pub related_ghosts: Vec<String>,
    /// 置信度
    pub confidence: f32,
    /// 用户反馈历史
    pub feedback_history: Vec<FeedbackEntry>,
    /// 部分接受的块IDs
    pub accepted_blocks: Vec<String>,
    /// 部分拒绝的块IDs
    pub rejected_blocks: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhostBlock {
    pub block_id: String,
    pub content: String,
    pub operation: GhostOp,
    pub after_block_id: Option<String>,
    pub heading_context: String,
    #[serde(default)]
    pub context: Vec<ContextFragment>,
    #[serde(default)]
    pub verified: Option<bool>,
    #[serde(default)]
    pub verification_result: Option<VerificationResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackEntry {
    pub timestamp: i64,
    pub action: String,
    pub block_ids: Vec<String>,
    pub reason: Option<String>,
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
    // 新增状态
    Expired,         // 超时未处理
    Superseded,      // 被新建议取代
    AwaitingContext, // 等待更多上下文
    InReview,        // 用户正在审阅中
    Blocked { reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConflictSeverity {
    Info,
    Warning,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConflictType {
    SameTarget,
    OverlappingBlocks,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhostConflict {
    pub ghost_ids: Vec<String>,
    pub conflict_type: ConflictType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictReport {
    pub source_note: String,
    pub conflicting_ghosts: Vec<GhostConflict>,
    pub severity: ConflictSeverity,
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
        task_id: Option<String>,
    ) -> Result<GhostNote, String> {
        let id = format!(
            "ghost_{}",
            uuid::Uuid::new_v4().to_string().replace('-', "")
        );
        let ghost = GhostNote {
            id: id.clone(),
            task_id,
            source_note: source_note.to_string(),
            task_description: task_description.to_string(),
            suggested_blocks,
            created_at: chrono::Utc::now().timestamp_millis(),
            status: GhostStatus::Pending,
            // 新字段默认值
            priority: 50, // 中等优先级
            expires_at: None,
            related_ghosts: Vec::new(),
            confidence: 0.7, // 默认置信度
            feedback_history: Vec::new(),
            accepted_blocks: Vec::new(),
            rejected_blocks: Vec::new(),
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
                            && matches!(
                                ghost.status,
                                GhostStatus::Pending | GhostStatus::PartiallyAccepted
                            )
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
                        if matches!(
                            ghost.status,
                            GhostStatus::Pending | GhostStatus::PartiallyAccepted
                        ) {
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

    /// Save a ghost note back to its file
    pub fn save(&self, ghost: &GhostNote) -> Result<(), String> {
        let file_path = self.ghost_file_path(&ghost.id);
        let json = serde_json::to_string_pretty(ghost).map_err(|e| e.to_string())?;
        fs::write(&file_path, json).map_err(|e| e.to_string())
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

    pub fn detect_context_overlap(
        ghost1_ctx: &[ContextFragment],
        ghost2_ctx: &[ContextFragment],
    ) -> f32 {
        if ghost1_ctx.is_empty() || ghost2_ctx.is_empty() {
            return 0.0;
        }
        ContextDiff::compute(ghost1_ctx.to_vec(), ghost2_ctx.to_vec()).drift_score
    }

    pub fn verify_ghost(
        &self,
        ghost_id: &str,
        graph: &crate::harness::lean_kernel::PropositionGraph,
    ) -> Result<VerificationResult, String> {
        let result = crate::harness::lean_kernel::HybridLeanKernel::verify(graph);
        if let Some(mut ghost) = self.get(ghost_id) {
            let passed = result.passed;
            for block in &mut ghost.suggested_blocks {
                block.verified = Some(passed);
                block.verification_result = Some(result.clone());
            }
            if !passed {
                ghost.status = GhostStatus::Blocked {
                    reason: format!(
                        "Verification failed: {} violations",
                        result.violations.len()
                    ),
                };
            }
            self.save(&ghost)?;
        }
        Ok(result)
    }

    fn collect_ghost_context(ghost: &GhostNote) -> Vec<ContextFragment> {
        ghost
            .suggested_blocks
            .iter()
            .flat_map(|b| b.context.clone())
            .collect()
    }

    pub fn detect_conflicts(&self, source_note: Option<&str>) -> Vec<ConflictReport> {
        let all = self.list_all();
        let mut by_source: std::collections::HashMap<String, Vec<&GhostNote>> =
            std::collections::HashMap::new();
        for ghost in &all {
            if let Some(filter) = source_note {
                if ghost.source_note != filter {
                    continue;
                }
            }
            by_source
                .entry(ghost.source_note.clone())
                .or_default()
                .push(ghost);
        }

        let mut reports = Vec::new();
        for (source, ghosts) in &by_source {
            if ghosts.len() < 2 {
                continue;
            }
            let severity = if ghosts.len() >= 4 {
                ConflictSeverity::Critical
            } else if ghosts.len() >= 3 {
                ConflictSeverity::Warning
            } else {
                ConflictSeverity::Info
            };

            let ids: Vec<String> = ghosts.iter().map(|g| g.id.clone()).collect();
            reports.push(ConflictReport {
                source_note: source.clone(),
                conflicting_ghosts: vec![GhostConflict {
                    ghost_ids: ids,
                    conflict_type: ConflictType::SameTarget,
                }],
                severity,
            });

            for i in 0..ghosts.len() {
                for j in (i + 1)..ghosts.len() {
                    let ctx_i = Self::collect_ghost_context(ghosts[i]);
                    let ctx_j = Self::collect_ghost_context(ghosts[j]);
                    let overlap = Self::detect_context_overlap(&ctx_i, &ctx_j);
                    if overlap > 0.0 {
                        reports.push(ConflictReport {
                            source_note: source.clone(),
                            conflicting_ghosts: vec![GhostConflict {
                                ghost_ids: vec![ghosts[i].id.clone(), ghosts[j].id.clone()],
                                conflict_type: ConflictType::OverlappingBlocks,
                            }],
                            severity: if overlap > 0.5 {
                                ConflictSeverity::Critical
                            } else if overlap > 0.3 {
                                ConflictSeverity::Warning
                            } else {
                                ConflictSeverity::Info
                            },
                        });
                    }
                }
            }
        }
        reports
    }

    fn ghost_file_path(&self, id: &str) -> PathBuf {
        self.store_dir.join(format!("{}.json", id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ghost_note_serialization() {
        let ghost = GhostNote {
            id: "test_ghost".to_string(),
            task_id: None,
            source_note: "test.md".to_string(),
            task_description: "Test task".to_string(),
            suggested_blocks: vec![],
            created_at: 1234567890,
            status: GhostStatus::Pending,
            priority: 50,
            expires_at: None,
            related_ghosts: vec![],
            confidence: 0.7,
            feedback_history: vec![],
            accepted_blocks: vec![],
            rejected_blocks: vec![],
        };

        let json = serde_json::to_string(&ghost).unwrap();
        let deserialized: GhostNote = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, "test_ghost");
        assert_eq!(deserialized.priority, 50);
        assert_eq!(deserialized.confidence, 0.7);
        assert!(deserialized.expires_at.is_none());
        assert!(deserialized.feedback_history.is_empty());
    }

    #[test]
    fn test_ghost_status_variants() {
        let statuses = vec![
            GhostStatus::Pending,
            GhostStatus::PartiallyAccepted,
            GhostStatus::Accepted,
            GhostStatus::Rejected,
            GhostStatus::Expired,
            GhostStatus::Superseded,
            GhostStatus::AwaitingContext,
            GhostStatus::InReview,
            GhostStatus::Blocked {
                reason: "test block".to_string(),
            },
        ];

        for status in statuses {
            let json = serde_json::to_string(&status).unwrap();
            let deserialized: GhostStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(format!("{:?}", status), format!("{:?}", deserialized));
        }
    }

    #[test]
    fn test_feedback_entry_serialization() {
        let entry = FeedbackEntry {
            timestamp: 1234567890,
            action: "accept".to_string(),
            block_ids: vec!["block1".to_string(), "block2".to_string()],
            reason: Some("Looks good".to_string()),
        };

        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: FeedbackEntry = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.timestamp, 1234567890);
        assert_eq!(deserialized.action, "accept");
        assert_eq!(deserialized.block_ids.len(), 2);
        assert_eq!(deserialized.reason, Some("Looks good".to_string()));
    }
}
