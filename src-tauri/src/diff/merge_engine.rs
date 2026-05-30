// Merge Engine — Apply accepted diff operations to original Markdown content
#![allow(dead_code)]

use super::ast_diff::DiffOp;
use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum MergeError {
    #[error("Block {0} not found in original")]
    BlockNotFound(String),
    #[error("Merge conflict at block {0}")]
    Conflict(String),
}

/// Represents a conflict between ghost notes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeConflict {
    /// Block ID where conflict occurs
    pub block_id: String,
    /// Type of conflict
    pub conflict_type: ConflictType,
    /// First ghost note ID
    pub ghost_a: String,
    /// Second ghost note ID
    pub ghost_b: String,
    /// Resolution strategy (if resolved)
    pub resolution: Option<Resolution>,
}

/// Type of conflict between ghost notes
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ConflictType {
    /// Two ghosts modify the same block
    OverlappingEdit { text_a: String, text_b: String },
    /// Contradictory suggestions
    ContradictorySuggestion { reason: String },
    /// Dependency violation (order matters)
    DependencyViolation {
        required_first: String,
        attempted_first: String,
    },
}

/// Resolution strategy for conflicts
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Resolution {
    /// Accept ghost A's changes
    AcceptA,
    /// Accept ghost B's changes
    AcceptB,
    /// Merge both changes
    MergeBoth { merged_text: String },
    /// Manual resolution required
    Manual,
}

/// Conflict resolver with configurable strategy
pub struct ConflictResolver {
    strategy: ResolutionStrategy,
}

/// Strategy for resolving conflicts
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResolutionStrategy {
    /// Last writer wins
    LastWriterWins,
    /// Highest priority wins
    HighestPriority,
    /// Wait for user manual resolution
    UserManual,
    /// Try automatic merge
    AutoMerge,
}

/// Find paragraph boundaries containing `pos`
/// Returns (start, end) byte offsets for the paragraph
fn find_paragraph_bounds(content: &str, pos: usize) -> (usize, usize) {
    let bytes = content.as_bytes();

    // Find start: scan backwards for empty line or beginning
    let start = {
        let mut i = pos;
        while i > 0 {
            // Check for empty line (two consecutive newlines)
            if i >= 2 && bytes[i - 1] == b'\n' && bytes[i - 2] == b'\n' {
                break;
            }
            // Check for start of file preceded by newline
            if i == 1 && bytes[0] == b'\n' {
                break;
            }
            i -= 1;
        }
        i
    };

    // Find end: scan forwards for empty line or end
    let end = {
        let mut i = pos;
        let len = bytes.len();
        while i < len {
            if i + 1 < len && bytes[i] == b'\n' && bytes[i + 1] == b'\n' {
                break;
            }
            i += 1;
        }
        i
    };

    (start, end)
}

/// Remove a paragraph from content given byte offsets
fn remove_paragraph(content: &str, start: usize, end: usize) -> String {
    let mut result = String::with_capacity(content.len());
    result.push_str(&content[..start]);
    // Skip trailing newlines
    let remaining = &content[end..];
    let trimmed = remaining.trim_start_matches('\n');
    result.push_str(trimmed);
    result
}

/// Apply accepted diff operations to produce merged content
///
/// The merge engine receives:
/// - original_md: the current Markdown file content
/// - accepted_ops: user-approved diff operations
///
/// It produces:
/// - merged_md: the new Markdown content with accepted changes
pub fn apply_merge(original_md: &str, accepted_ops: &[DiffOp]) -> Result<String, MergeError> {
    let mut result = original_md.to_string();

    for op in accepted_ops {
        match op {
            DiffOp::ModifyBlock {
                block_id,
                old_text,
                new_text,
            } => {
                if !result.contains(old_text) {
                    // If exact match fails, try the block_id
                    if let Some(_pos) = result.find(&format!("<!--^{}-->", block_id)) {
                        // Block found by ID — replace nearby text
                        // Full implementation requires more context in Stage 5
                    }
                    // Fallback: try to replace old_text
                    if result.contains(old_text) {
                        result = result.replacen(old_text, new_text, 1);
                    }
                } else {
                    result = result.replacen(old_text, new_text, 1);
                }
            }
            DiffOp::InsertBlock { text, .. } => {
                result.push_str("\n\n");
                result.push_str(text);
            }
            DiffOp::DeleteBlock { block_id } => {
                let marker = format!("<!--^{}-->", block_id);
                if let Some(pos) = result.find(&marker) {
                    let (start, end) = find_paragraph_bounds(&result, pos);
                    result = remove_paragraph(&result, start, end);
                }
            }
            DiffOp::InsertLink { block_id, target } => {
                let marker = format!("<!--^{}-->", block_id);
                if let Some(pos) = result.find(&marker) {
                    // Find end of paragraph containing this marker
                    let (_, end) = find_paragraph_bounds(&result, pos);
                    let link = format!("\n[[{}]]", target);
                    result.insert_str(end, &link);
                }
            }
            DiffOp::DeleteLink {
                block_id: _,
                target,
            } => {
                // Try [[target|display]] first, then [[target]]
                let re_wiki_alias =
                    Regex::new(&format!(r"\[\[{}\|([^\]]+)\]\]", regex::escape(target))).unwrap();
                let re_wiki = Regex::new(&format!(r"\[\[{}\]\]", regex::escape(target))).unwrap();
                let re_md_link =
                    Regex::new(&format!(r"\[([^\]]*)\]\({}\)", regex::escape(target))).unwrap();

                if re_wiki_alias.is_match(&result) {
                    result = re_wiki_alias.replace_all(&result, "$1").to_string();
                } else if re_wiki.is_match(&result) {
                    result = re_wiki.replace_all(&result, target).to_string();
                } else if re_md_link.is_match(&result) {
                    result = re_md_link.replace_all(&result, "$1").to_string();
                }
            }
        }
    }

    Ok(result)
}

impl ConflictResolver {
    /// Create a new resolver with given strategy
    pub fn new(strategy: ResolutionStrategy) -> Self {
        ConflictResolver { strategy }
    }

    /// Detect conflicts between multiple ghost notes
    pub fn detect_conflicts(&self, ghosts: &[super::ghost_store::GhostNote]) -> Vec<MergeConflict> {
        let mut conflicts = Vec::new();

        // Compare each pair of ghosts
        for i in 0..ghosts.len() {
            for j in (i + 1)..ghosts.len() {
                let ghost_a = &ghosts[i];
                let ghost_b = &ghosts[j];

                // Check for overlapping edits
                let overlapping = self.find_overlapping_edits(ghost_a, ghost_b);
                conflicts.extend(overlapping);

                // Check for contradictions
                let contradictions = self.find_contradictions(ghost_a, ghost_b);
                conflicts.extend(contradictions);
            }
        }

        conflicts
    }

    /// Find overlapping edits between two ghosts
    fn find_overlapping_edits(
        &self,
        ghost_a: &super::ghost_store::GhostNote,
        ghost_b: &super::ghost_store::GhostNote,
    ) -> Vec<MergeConflict> {
        let mut conflicts = Vec::new();

        // Get block IDs from both ghosts
        let blocks_a: std::collections::HashSet<String> = ghost_a
            .suggested_blocks
            .iter()
            .map(|b| b.block_id.clone())
            .collect();

        for block_b in &ghost_b.suggested_blocks {
            if blocks_a.contains(&block_b.block_id) {
                // Same block modified by both ghosts
                let block_a = ghost_a
                    .suggested_blocks
                    .iter()
                    .find(|b| b.block_id == block_b.block_id)
                    .unwrap();

                // Check if content is different
                if block_a.content != block_b.content {
                    conflicts.push(MergeConflict {
                        block_id: block_b.block_id.clone(),
                        conflict_type: ConflictType::OverlappingEdit {
                            text_a: block_a.content.clone(),
                            text_b: block_b.content.clone(),
                        },
                        ghost_a: ghost_a.id.clone(),
                        ghost_b: ghost_b.id.clone(),
                        resolution: None,
                    });
                }
            }
        }

        conflicts
    }

    /// Find contradictions between two ghosts
    fn find_contradictions(
        &self,
        ghost_a: &super::ghost_store::GhostNote,
        ghost_b: &super::ghost_store::GhostNote,
    ) -> Vec<MergeConflict> {
        let mut conflicts = Vec::new();

        // Simple contradiction detection: if one ghost deletes a block
        // that another ghost modifies
        let delete_ops_a: std::collections::HashSet<String> = ghost_a
            .suggested_blocks
            .iter()
            .filter(|b| matches!(b.operation, super::ghost_store::GhostOp::Delete))
            .map(|b| b.block_id.clone())
            .collect();

        for block_b in &ghost_b.suggested_blocks {
            if delete_ops_a.contains(&block_b.block_id)
                && !matches!(block_b.operation, super::ghost_store::GhostOp::Delete)
            {
                conflicts.push(MergeConflict {
                    block_id: block_b.block_id.clone(),
                    conflict_type: ConflictType::ContradictorySuggestion {
                        reason: format!(
                            "Ghost {} deletes block that Ghost {} modifies",
                            ghost_a.id, ghost_b.id
                        ),
                    },
                    ghost_a: ghost_a.id.clone(),
                    ghost_b: ghost_b.id.clone(),
                    resolution: None,
                });
            }
        }

        conflicts
    }

    /// Resolve a conflict using the configured strategy
    pub fn resolve(&self, conflict: &MergeConflict) -> Resolution {
        match &self.strategy {
            ResolutionStrategy::LastWriterWins => {
                // For simplicity, accept the second ghost's changes
                Resolution::AcceptB
            }
            ResolutionStrategy::HighestPriority => {
                // Would need priority comparison - default to AcceptA
                Resolution::AcceptA
            }
            ResolutionStrategy::UserManual => Resolution::Manual,
            ResolutionStrategy::AutoMerge => {
                // Try to merge based on conflict type
                match &conflict.conflict_type {
                    ConflictType::OverlappingEdit { text_a, text_b } => Resolution::MergeBoth {
                        merged_text: format!(
                            "<<<<<<< Ghost A\n{}\n=======\n{}\n>>>>>>> Ghost B",
                            text_a, text_b
                        ),
                    },
                    _ => Resolution::Manual,
                }
            }
        }
    }

    /// Auto-resolve conflicts where possible
    pub fn auto_resolve(&self, conflicts: &mut [MergeConflict]) {
        for conflict in conflicts.iter_mut() {
            if conflict.resolution.is_none() {
                conflict.resolution = Some(self.resolve(conflict));
            }
        }
    }
}

impl Default for ConflictResolver {
    fn default() -> Self {
        Self::new(ResolutionStrategy::UserManual)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_text_replace() {
        let original = "# Hello\n\nThis is old text.";
        let ops = vec![DiffOp::ModifyBlock {
            block_id: "test".to_string(),
            old_text: "old text".to_string(),
            new_text: "new text".to_string(),
        }];
        let result = apply_merge(original, &ops).unwrap();
        assert!(result.contains("new text"));
        assert!(!result.contains("old text"));
    }

    #[test]
    fn test_delete_block() {
        let original = "# Hello\n\n<!--^abc123-->\nSome content here\n\nOther paragraph";
        let ops = vec![DiffOp::DeleteBlock {
            block_id: "abc123".to_string(),
        }];
        let result = apply_merge(original, &ops).unwrap();
        assert!(!result.contains("abc123"));
        assert!(!result.contains("Some content here"));
        assert!(result.contains("Other paragraph"));
    }

    #[test]
    fn test_delete_block_at_start() {
        let original = "<!--^start-->\nFirst paragraph\n\nSecond paragraph";
        let ops = vec![DiffOp::DeleteBlock {
            block_id: "start".to_string(),
        }];
        let result = apply_merge(original, &ops).unwrap();
        assert!(!result.contains("start"));
        assert!(!result.contains("First paragraph"));
        assert!(result.contains("Second paragraph"));
    }

    #[test]
    fn test_insert_link() {
        let original = "# Hello\n\n<!--^block1-->\nSome content\n\nOther paragraph";
        let ops = vec![DiffOp::InsertLink {
            block_id: "block1".to_string(),
            target: "target_page".to_string(),
        }];
        let result = apply_merge(original, &ops).unwrap();
        assert!(result.contains("[[target_page]]"));
        assert!(result.contains("Other paragraph"));
    }

    #[test]
    fn test_delete_link_wiki() {
        let original = "Check [[my_link]] for details";
        let ops = vec![DiffOp::DeleteLink {
            block_id: "any".to_string(),
            target: "my_link".to_string(),
        }];
        let result = apply_merge(original, &ops).unwrap();
        assert!(!result.contains("[[my_link]]"));
        assert!(result.contains("my_link"));
    }

    #[test]
    fn test_delete_link_wiki_alias() {
        let original = "Check [[my_link|custom text]] for details";
        let ops = vec![DiffOp::DeleteLink {
            block_id: "any".to_string(),
            target: "my_link".to_string(),
        }];
        let result = apply_merge(original, &ops).unwrap();
        assert!(!result.contains("[[my_link"));
        assert!(result.contains("custom text"));
    }

    #[test]
    fn test_delete_link_markdown() {
        let original = "Check [click here](http://example.com) for details";
        let ops = vec![DiffOp::DeleteLink {
            block_id: "any".to_string(),
            target: "http://example.com".to_string(),
        }];
        let result = apply_merge(original, &ops).unwrap();
        assert!(!result.contains("](http://example.com)"));
        assert!(result.contains("click here"));
    }

    #[test]
    fn test_conflict_detection_no_conflict() {
        use crate::diff::ghost_store::{GhostBlock, GhostNote, GhostOp, GhostStatus};

        let resolver = ConflictResolver::new(ResolutionStrategy::UserManual);

        let ghost_a = GhostNote {
            id: "ghost_a".to_string(),
            task_id: None,
            source_note: "test.md".to_string(),
            task_description: "Test".to_string(),
            suggested_blocks: vec![GhostBlock {
                block_id: "block1".to_string(),
                content: "Content A".to_string(),
                operation: GhostOp::Modify,
                after_block_id: None,
                heading_context: String::new(),
                context: vec![],
                verified: None,
                verification_result: None,
            }],
            created_at: 0,
            status: GhostStatus::Pending,
            priority: 50,
            expires_at: None,
            related_ghosts: vec![],
            confidence: 0.7,
            feedback_history: vec![],
            accepted_blocks: vec![],
            rejected_blocks: vec![],
        };

        let ghost_b = GhostNote {
            id: "ghost_b".to_string(),
            task_id: None,
            source_note: "test.md".to_string(),
            task_description: "Test".to_string(),
            suggested_blocks: vec![GhostBlock {
                block_id: "block2".to_string(),
                content: "Content B".to_string(),
                operation: GhostOp::Modify,
                after_block_id: None,
                heading_context: String::new(),
                context: vec![],
                verified: None,
                verification_result: None,
            }],
            created_at: 0,
            status: GhostStatus::Pending,
            priority: 50,
            expires_at: None,
            related_ghosts: Vec::new(),
            confidence: 0.7,
            feedback_history: Vec::new(),
            accepted_blocks: Vec::new(),
            rejected_blocks: Vec::new(),
        };

        let conflicts = resolver.detect_conflicts(&[ghost_a, ghost_b]);
        assert!(conflicts.is_empty());
    }

    #[test]
    fn test_conflict_detection_overlapping() {
        use crate::diff::ghost_store::{GhostBlock, GhostNote, GhostOp, GhostStatus};

        let resolver = ConflictResolver::new(ResolutionStrategy::UserManual);

        let ghost_a = GhostNote {
            id: "ghost_a".to_string(),
            task_id: None,
            source_note: "test.md".to_string(),
            task_description: "Test".to_string(),
            suggested_blocks: vec![GhostBlock {
                block_id: "block1".to_string(),
                content: "Content A".to_string(),
                operation: GhostOp::Modify,
                after_block_id: None,
                heading_context: String::new(),
                context: vec![],
                verified: None,
                verification_result: None,
            }],
            created_at: 0,
            status: GhostStatus::Pending,
            priority: 50,
            expires_at: None,
            related_ghosts: Vec::new(),
            confidence: 0.7,
            feedback_history: Vec::new(),
            accepted_blocks: Vec::new(),
            rejected_blocks: Vec::new(),
        };

        let ghost_b = GhostNote {
            id: "ghost_b".to_string(),
            task_id: None,
            source_note: "test.md".to_string(),
            task_description: "Test".to_string(),
            suggested_blocks: vec![GhostBlock {
                block_id: "block1".to_string(),
                content: "Content B".to_string(),
                operation: GhostOp::Modify,
                after_block_id: None,
                heading_context: String::new(),
                context: vec![],
                verified: None,
                verification_result: None,
            }],
            created_at: 0,
            status: GhostStatus::Pending,
            priority: 50,
            expires_at: None,
            related_ghosts: Vec::new(),
            confidence: 0.7,
            feedback_history: Vec::new(),
            accepted_blocks: Vec::new(),
            rejected_blocks: Vec::new(),
        };

        let conflicts = resolver.detect_conflicts(&[ghost_a, ghost_b]);
        assert_eq!(conflicts.len(), 1);
        assert!(matches!(
            conflicts[0].conflict_type,
            ConflictType::OverlappingEdit { .. }
        ));
    }

    #[test]
    fn test_conflict_resolver_default() {
        let resolver = ConflictResolver::default();
        assert!(matches!(resolver.strategy, ResolutionStrategy::UserManual));
    }
}
