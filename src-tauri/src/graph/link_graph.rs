// In-memory bidirectional link graph

use std::collections::{HashMap, HashSet};
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,        // relative file path
    pub title: String,
    pub outgoing: usize,   // links from this note
    pub incoming: usize,   // backlinks to this note
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Backlink {
    pub from_file: String,
    pub from_title: String,
    pub context: String,
    pub position: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphData {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

/// Bidirectional link graph for a vault
#[derive(Debug, Default, Clone)]
pub struct LinkGraph {
    /// outgoing[source] = set of targets
    outgoing: HashMap<String, HashSet<String>>,
    /// incoming[target] = set of sources
    incoming: HashMap<String, HashSet<String>>,
    /// note_id -> title mapping
    titles: HashMap<String, String>,
}

impl LinkGraph {
    pub fn new() -> Self {
        LinkGraph::default()
    }

    /// Add a link from one note to another
    pub fn add_link(&mut self, from: &str, to: &str) {
        self.outgoing
            .entry(from.to_string())
            .or_default()
            .insert(to.to_string());
        self.incoming
            .entry(to.to_string())
            .or_default()
            .insert(from.to_string());
    }

    /// Remove a link
    pub fn remove_link(&mut self, from: &str, to: &str) {
        if let Some(targets) = self.outgoing.get_mut(from) {
            targets.remove(to);
        }
        if let Some(sources) = self.incoming.get_mut(to) {
            sources.remove(from);
        }
    }

    /// Remove all links from a note (when note is deleted)
    pub fn remove_note(&mut self, id: &str) {
        // Remove outgoing
        if let Some(targets) = self.outgoing.remove(id) {
            for target in &targets {
                if let Some(sources) = self.incoming.get_mut(target) {
                    sources.remove(id);
                }
            }
        }
        // Remove incoming
        if let Some(sources) = self.incoming.remove(id) {
            for source in &sources {
                if let Some(targets) = self.outgoing.get_mut(source) {
                    targets.remove(id);
                }
            }
        }
        self.titles.remove(id);
    }

    /// Get backlinks for a note
    pub fn get_backlinks(&self, note_id: &str) -> Vec<String> {
        self.incoming
            .get(note_id)
            .map(|sources| sources.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Update a note's title
    pub fn set_title(&mut self, id: &str, title: &str) {
        self.titles.insert(id.to_string(), title.to_string());
    }

    /// Rename a note (update all references)
    pub fn rename_note(&mut self, old_id: &str, new_id: &str) {
        // Move outgoing edges
        if let Some(targets) = self.outgoing.remove(old_id) {
            for target in &targets {
                self.incoming
                    .entry(target.clone())
                    .or_default()
                    .insert(new_id.to_string());
            }
            self.outgoing.insert(new_id.to_string(), targets);
        }
        // Move incoming edges
        if let Some(sources) = self.incoming.remove(old_id) {
            for source in &sources {
                self.outgoing
                    .entry(source.clone())
                    .or_default()
                    .insert(new_id.to_string());
            }
            self.incoming.insert(new_id.to_string(), sources);
        }
        if let Some(title) = self.titles.remove(old_id) {
            self.titles.insert(new_id.to_string(), title);
        }
    }

    /// Export graph data for frontend visualization
    pub fn to_frontend_json(&self) -> GraphData {
        let mut all_ids: HashSet<&String> = HashSet::new();
        for (k, v) in &self.outgoing {
            all_ids.insert(k);
            all_ids.extend(v.iter());
        }
        for k in self.incoming.keys() {
            all_ids.insert(k);
        }

        let nodes: Vec<GraphNode> = all_ids
            .iter()
            .map(|id| GraphNode {
                id: id.to_string(),
                title: self.titles.get(*id).cloned().unwrap_or_else(|| {
                    id.rsplit('/').next().unwrap_or(id).trim_end_matches(".md").to_string()
                }),
                outgoing: self.outgoing.get(*id).map(|s| s.len()).unwrap_or(0),
                incoming: self.incoming.get(*id).map(|s| s.len()).unwrap_or(0),
            })
            .collect();

        let mut edges: Vec<GraphEdge> = Vec::new();
        for (from, targets) in &self.outgoing {
            for to in targets {
                edges.push(GraphEdge {
                    from: from.clone(),
                    to: to.clone(),
                });
            }
        }

        GraphData { nodes, edges }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_and_get_backlinks() {
        let mut graph = LinkGraph::new();
        graph.add_link("a.md", "b.md");
        graph.add_link("c.md", "b.md");

        let backlinks = graph.get_backlinks("b.md");
        assert_eq!(backlinks.len(), 2);
        assert!(backlinks.contains(&"a.md".to_string()));
        assert!(backlinks.contains(&"c.md".to_string()));
    }

    #[test]
    fn test_rename_note() {
        let mut graph = LinkGraph::new();
        graph.add_link("old.md", "target.md");
        graph.add_link("other.md", "old.md");

        graph.rename_note("old.md", "new.md");

        assert!(graph.get_backlinks("target.md").contains(&"new.md".to_string()));
        assert!(graph.get_backlinks("new.md").contains(&"other.md".to_string()));
    }

    #[test]
    fn test_remove_note() {
        let mut graph = LinkGraph::new();
        graph.add_link("a.md", "b.md");
        graph.remove_note("a.md");
        assert!(graph.get_backlinks("b.md").is_empty());
    }
}
