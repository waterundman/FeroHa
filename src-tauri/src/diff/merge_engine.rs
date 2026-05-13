// Merge Engine — Apply accepted diff operations to original Markdown content
// Stage 5 implementation placeholder

use super::ast_diff::DiffOp;

#[derive(Debug, thiserror::Error)]
pub enum MergeError {
    #[error("Block {0} not found in original")]
    BlockNotFound(String),
    #[error("Merge conflict at block {0}")]
    Conflict(String),
}

/// Apply accepted diff operations to produce merged content
///
/// The merge engine receives:
/// - original_md: the current Markdown file content
/// - accepted_ops: user-approved diff operations
///
/// It produces:
/// - merged_md: the new Markdown content with accepted changes
pub fn apply_merge(
    original_md: &str,
    accepted_ops: &[DiffOp],
) -> Result<String, MergeError> {
    let mut result = original_md.to_string();

    for op in accepted_ops {
        match op {
            DiffOp::ModifyBlock { block_id, old_text, new_text } => {
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
            DiffOp::DeleteBlock { block_id, .. } => {
                // Remove block by ID marker
                let marker = format!("<!--^{}-->", block_id);
                if let Some(_pos) = result.find(&marker) {
                    // Find the paragraph containing this marker and remove it
                    // Full implementation in Stage 5
                }
            }
            _ => {
                // InsertLink, DeleteLink — handled in Stage 5
            }
        }
    }

    Ok(result)
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
}
