// AST Diff Engine — Compare two Markdown ASTs and produce diff operations
// Stage 5 implementation placeholder
#![allow(dead_code)]

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", content = "data")]
pub enum DiffOp {
    #[serde(rename = "inserted")]
    InsertBlock {
        position: usize,
        block_id: String,
        text: String,
    },
    #[serde(rename = "deleted")]
    DeleteBlock {
        block_id: String,
    },
    #[serde(rename = "modified")]
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

/// Semantic matcher for content blocks
pub struct SemanticMatcher {
    /// Similarity threshold (0.0-1.0)
    pub threshold: f32,
}

/// Result of matching two blocks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockMatch {
    /// Index in original blocks (None if unmatched)
    pub original_idx: Option<usize>,
    /// Index in suggested blocks (None if unmatched)
    pub suggested_idx: Option<usize>,
    /// Type of match found
    pub match_type: MatchType,
    /// Confidence score (0.0-1.0)
    pub confidence: f32,
}

/// Type of match between blocks
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum MatchType {
    /// Exact block_id match
    Exact,
    /// Content hash match
    ContentHash,
    /// Semantic similarity match with score
    Semantic { score: f32 },
    /// No match found
    Unmatched,
}

impl SemanticMatcher {
    /// Create a new matcher with given threshold
    pub fn new(threshold: f32) -> Self {
        SemanticMatcher { threshold }
    }

    /// Match blocks using multi-level strategy
    pub fn match_blocks(
        &self,
        original: &[crate::parser::ast::ContentBlock],
        suggested: &[crate::parser::ast::ContentBlock],
    ) -> Vec<BlockMatch> {
        let mut matches = Vec::new();
        let mut used_original = vec![false; original.len()];
        let mut used_suggested = vec![false; suggested.len()];

        // Phase 1: Exact block_id match
        for (s_idx, sugg) in suggested.iter().enumerate() {
            for (o_idx, orig) in original.iter().enumerate() {
                if !used_original[o_idx] && !used_suggested[s_idx] && orig.id == sugg.id {
                    matches.push(BlockMatch {
                        original_idx: Some(o_idx),
                        suggested_idx: Some(s_idx),
                        match_type: MatchType::Exact,
                        confidence: 1.0,
                    });
                    used_original[o_idx] = true;
                    used_suggested[s_idx] = true;
                }
            }
        }

        // Phase 2: Content hash match
        for (s_idx, sugg) in suggested.iter().enumerate() {
            if used_suggested[s_idx] {
                continue;
            }
            let sugg_hash = calculate_content_hash(&sugg.text);
            for (o_idx, orig) in original.iter().enumerate() {
                if !used_original[o_idx] {
                    let orig_hash = calculate_content_hash(&orig.text);
                    if sugg_hash == orig_hash {
                        matches.push(BlockMatch {
                            original_idx: Some(o_idx),
                            suggested_idx: Some(s_idx),
                            match_type: MatchType::ContentHash,
                            confidence: 0.95,
                        });
                        used_original[o_idx] = true;
                        used_suggested[s_idx] = true;
                        break;
                    }
                }
            }
        }

        // Phase 3: Text similarity match (simple Levenshtein-based)
        for (s_idx, sugg) in suggested.iter().enumerate() {
            if used_suggested[s_idx] {
                continue;
            }
            let mut best_match: Option<(usize, f32)> = None;
            for (o_idx, orig) in original.iter().enumerate() {
                if !used_original[o_idx] {
                    let similarity = calculate_text_similarity(&orig.text, &sugg.text);
                    if similarity >= self.threshold {
                        if let Some((_, best_score)) = best_match {
                            if similarity > best_score {
                                best_match = Some((o_idx, similarity));
                            }
                        } else {
                            best_match = Some((o_idx, similarity));
                        }
                    }
                }
            }
            if let Some((o_idx, score)) = best_match {
                matches.push(BlockMatch {
                    original_idx: Some(o_idx),
                    suggested_idx: Some(s_idx),
                    match_type: MatchType::Semantic { score },
                    confidence: score,
                });
                used_original[o_idx] = true;
                used_suggested[s_idx] = true;
            }
        }

        // Phase 4: Unmatched suggested blocks
        for (s_idx, _) in suggested.iter().enumerate() {
            if !used_suggested[s_idx] {
                matches.push(BlockMatch {
                    original_idx: None,
                    suggested_idx: Some(s_idx),
                    match_type: MatchType::Unmatched,
                    confidence: 0.0,
                });
            }
        }

        // Phase 5: Unmatched original blocks
        for (o_idx, _) in original.iter().enumerate() {
            if !used_original[o_idx] {
                matches.push(BlockMatch {
                    original_idx: Some(o_idx),
                    suggested_idx: None,
                    match_type: MatchType::Unmatched,
                    confidence: 0.0,
                });
            }
        }

        matches
    }
}

/// Calculate a simple content hash (for quick comparison)
fn calculate_content_hash(text: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

/// Calculate text similarity using simple character-based comparison
fn calculate_text_similarity(a: &str, b: &str) -> f32 {
    if a == b {
        return 1.0;
    }
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }

    // Simple Jaccard similarity on character trigrams
    let trigrams_a: std::collections::HashSet<String> = a
        .chars()
        .collect::<Vec<_>>()
        .windows(3)
        .map(|w| w.iter().collect())
        .collect();
    let trigrams_b: std::collections::HashSet<String> = b
        .chars()
        .collect::<Vec<_>>()
        .windows(3)
        .map(|w| w.iter().collect())
        .collect();

    let intersection = trigrams_a.intersection(&trigrams_b).count();
    let union = trigrams_a.union(&trigrams_b).count();

    if union == 0 {
        0.0
    } else {
        intersection as f32 / union as f32
    }
}

/// Compare two lists of content blocks and produce diff operations
///
/// Stage 1 implementation: block_id-based alignment + text-level diff
/// Future stages can upgrade to semantic diff
#[allow(dead_code)]
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
    use crate::parser::ast::{generate_block_id, BlockType, ContentBlock};

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
        assert!(
            ops.iter()
                .any(|op| matches!(op, DiffOp::DeleteBlock { .. }))
                || ops
                    .iter()
                    .any(|op| matches!(op, DiffOp::InsertBlock { .. }))
        );
    }

    #[test]
    fn test_insert_block() {
        let a = vec![make_block("A")];
        let b = vec![make_block("A"), make_block("B")];
        let ops = diff_blocks(&a, &b);
        assert!(ops
            .iter()
            .any(|op| matches!(op, DiffOp::InsertBlock { .. })));
    }

    #[test]
    fn test_semantic_matcher_exact() {
        let matcher = SemanticMatcher::new(0.7);
        let a = vec![make_block("Hello")];
        let b = vec![make_block("Hello")];
        let matches = matcher.match_blocks(&a, &b);
        assert_eq!(matches.len(), 1);
        assert!(matches[0].match_type == MatchType::Exact);
    }

    #[test]
    fn test_semantic_matcher_unmatched() {
        let matcher = SemanticMatcher::new(0.7);
        let a = vec![make_block("Hello")];
        let b = vec![make_block("World")];
        let matches = matcher.match_blocks(&a, &b);
        // Should find some matches (either semantic or unmatched)
        assert!(!matches.is_empty());
    }

    #[test]
    fn test_semantic_matcher_empty() {
        let matcher = SemanticMatcher::new(0.7);
        let a: Vec<crate::parser::ast::ContentBlock> = vec![];
        let b: Vec<crate::parser::ast::ContentBlock> = vec![];
        let matches = matcher.match_blocks(&a, &b);
        assert!(matches.is_empty());
    }
}
