// Backlinks operations

use super::link_graph::LinkGraph;
use serde::{Deserialize, Serialize};

/// Detailed backlink with surrounding context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacklinkContext {
    pub from: String,
    pub from_title: String,
    pub text: String, // paragraph containing the link
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
        let from_title = source
            .rsplit('/')
            .next()
            .unwrap_or(&source)
            .trim_end_matches(".md")
            .to_string();

        if let Some(content) = _vault_content_provider(&source) {
            // Find the wikilink and extract surrounding paragraph
            let stem = note_id
                .rsplit('/')
                .next()
                .unwrap_or(note_id)
                .trim_end_matches(".md");
            let patterns = [
                format!("[[{}]]", note_id),
                format!("[[{}|", note_id),
                format!("[[{}#", note_id),
                format!("[[{}]]", stem),
                format!("[[{}|", stem),
                format!("[[{}#", stem),
            ];

            if let Some(pos) = patterns.iter().find_map(|pattern| content.find(pattern)) {
                // Find paragraph boundaries (double newline or start/end of content)
                let para_start = content[..pos].rfind("\n\n").map(|p| p + 2).unwrap_or(0);
                let para_end = content[pos..]
                    .find("\n\n")
                    .map(|p| pos + p)
                    .unwrap_or(content.len());
                let paragraph = content[para_start..para_end].trim().to_string();

                results.push(BacklinkContext {
                    from: source.clone(),
                    from_title,
                    text: paragraph,
                    position: pos,
                });
            } else {
                results.push(BacklinkContext {
                    from: source.clone(),
                    from_title,
                    text: format!("Links to [[{}]]", note_id),
                    position: 0,
                });
            }
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
