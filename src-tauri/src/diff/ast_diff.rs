// AST Diff Engine — Compare two Markdown ASTs and produce diff operations
// Stage 5 implementation placeholder

use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", content = "data")]
pub enum DiffOp {
    InsertBlock {
        position: usize,
        block_id: String,
        text: String,
    },
    DeleteBlock {
        block_id: String,
    },
    ModifyBlock {
        block_id: String,
        old_text: String,
        new_text: String,
    },
    InsertLink {
        block_id: String,
        target: String,
    },
    DeleteLink {
        block_id: String,
        target: String,
    },
}

/// Compare two lists of content blocks and produce diff operations
///
/// Stage 1 implementation: block_id-based alignment + text-level diff
/// Future stages can upgrade to semantic diff
pub fn diff_blocks(
    original_blocks: &[crate::parser::ast::ContentBlock],
    suggested_blocks: &[crate::parser::ast::ContentBlock],
) -> Vec<DiffOp> {
    let mut ops = Vec::new();
    let mut orig_iter = original_blocks.iter().peekable();
    let mut sugg_iter = suggested_blocks.iter().peekable();

    // Simple block_id-based alignment for now
    // Full implementation in Stage 5
    while let (Some(orig), Some(sugg)) = (orig_iter.peek(), sugg_iter.peek()) {
        if orig.id == sugg.id {
            if orig.text != sugg.text {
                ops.push(DiffOp::ModifyBlock {
                    block_id: orig.id.clone(),
                    old_text: orig.text.clone(),
                    new_text: sugg.text.clone(),
                });
            }
            orig_iter.next();
            sugg_iter.next();
        } else if !suggested_blocks.iter().any(|b| b.id == orig.id) {
            ops.push(DiffOp::DeleteBlock {
                block_id: orig.id.clone(),
            });
            orig_iter.next();
        } else {
            ops.push(DiffOp::InsertBlock {
                position: sugg.position,
                block_id: sugg.id.clone(),
                text: sugg.text.clone(),
            });
            sugg_iter.next();
        }
    }

    // Remaining inserts
    for block in sugg_iter {
        ops.push(DiffOp::InsertBlock {
            position: block.position,
            block_id: block.id.clone(),
            text: block.text.clone(),
        });
    }

    // Remaining deletes
    for block in orig_iter {
        ops.push(DiffOp::DeleteBlock {
            block_id: block.id.clone(),
        });
    }

    ops
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ast::{ContentBlock, BlockType, generate_block_id};

    fn make_block(text: &str) -> ContentBlock {
        ContentBlock {
            id: generate_block_id(text),
            block_type: BlockType::Paragraph,
            text: text.to_string(),
            parent_heading: None,
            position: 0,
            links: Vec::new(),
        }
    }

    #[test]
    fn test_same_content_no_diff() {
        let a = vec![make_block("Hello")];
        let b = vec![make_block("Hello")];
        let ops = diff_blocks(&a, &b);
        assert!(ops.is_empty());
    }

    #[test]
    fn test_modified_block() {
        let a = vec![make_block("Hello")];
        let b = vec![make_block("Hello World")];
        let ops = diff_blocks(&a, &b);
        assert!(!ops.is_empty());
        // When block_id changes (content-based hash), it's a delete+insert, not modify
        assert!(ops.iter().any(|op| matches!(op, DiffOp::DeleteBlock { .. })) || 
                ops.iter().any(|op| matches!(op, DiffOp::InsertBlock { .. })));
    }

    #[test]
    fn test_insert_block() {
        let a = vec![make_block("A")];
        let b = vec![make_block("A"), make_block("B")];
        let ops = diff_blocks(&a, &b);
        assert!(ops.iter().any(|op| matches!(op, DiffOp::InsertBlock { .. })));
    }
}
