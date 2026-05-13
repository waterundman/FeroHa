// Backlinks operations

use super::link_graph::LinkGraph;
use serde::{Serialize, Deserialize};

/// Detailed backlink with surrounding context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacklinkContext {
    pub from: String,
    pub from_title: String,
    pub text: String,    // paragraph containing the link
    pub position: usize,
}

/// Get contextualized backlinks for a note
pub fn get_contextual_backlinks(
    graph: &LinkGraph,
    note_id: &str,
    _vault_content_provider: &dyn Fn(&str) -> Option<String>, // async read from vault
) -> Vec<BacklinkContext> {
    let sources = graph.get_backlinks(note_id);
    let mut results = Vec::new();

    for source in sources {
        // In production, read the source file content and extract context
        // For now, return basic backlink info
        if let Some(_content) = _vault_content_provider(&source) {
            // Parse content to find the [[link]] and surrounding paragraph
            results.push(BacklinkContext {
                from: source.clone(),
                from_title: source
                    .rsplit('/')
                    .next()
                    .unwrap_or(&source)
                    .trim_end_matches(".md")
                    .to_string(),
                text: format!("[[{}]]", note_id),
                position: 0,
            });
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_graph() {
        let graph = LinkGraph::new();
        let provider = |_: &str| None;
        let backlinks = get_contextual_backlinks(&graph, "nonexistent.md", &provider);
        assert!(backlinks.is_empty());
    }
}
