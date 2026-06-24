// In-memory bidirectional link graph

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String, // relative file path
    pub title: String,
    pub outgoing: usize, // links from this note
    pub incoming: usize, // backlinks to this note
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activation: Option<f32>, // semantic gravity activation 0.0-1.0
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum GraphEdgeType {
    Parent,
    Reference,
    Related,
    Source,
    Sequence,
    Semantic,
    Temporal,
    Bridge,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub from: String,
    pub to: String,
    pub edge_type: GraphEdgeType,
    pub origin: String,
    pub confidence: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weight: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_region: Option<String>,
}

#[derive(Debug, Clone)]
struct GraphEdgeMeta {
    edge_type: GraphEdgeType,
    origin: String,
    confidence: f32,
    weight: Option<f32>,
    memory_region: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
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
    /// edge metadata keyed by (source, target, edge type)
    edge_meta: HashMap<(String, String, GraphEdgeType), GraphEdgeMeta>,
    /// note_id -> title mapping
    titles: HashMap<String, String>,
}

impl LinkGraph {
    pub fn new() -> Self {
        LinkGraph::default()
    }

    /// Add a link from one note to another
    pub fn add_link(&mut self, from: &str, to: &str) {
        self.add_typed_link(from, to, GraphEdgeType::Reference, "wikilink", 1.0);
    }

    pub fn add_typed_link(
        &mut self,
        from: &str,
        to: &str,
        edge_type: GraphEdgeType,
        origin: &str,
        confidence: f32,
    ) {
        self.outgoing
            .entry(from.to_string())
            .or_default()
            .insert(to.to_string());
        self.incoming
            .entry(to.to_string())
            .or_default()
            .insert(from.to_string());
        self.edge_meta.insert(
            (from.to_string(), to.to_string(), edge_type.clone()),
            GraphEdgeMeta {
                edge_type,
                origin: origin.to_string(),
                confidence,
                weight: None,
                memory_region: None,
            },
        );
    }

    pub fn add_graph_edge(&mut self, edge: GraphEdge) {
        self.outgoing
            .entry(edge.from.clone())
            .or_default()
            .insert(edge.to.clone());
        self.incoming
            .entry(edge.to.clone())
            .or_default()
            .insert(edge.from.clone());
        self.edge_meta.insert(
            (edge.from, edge.to, edge.edge_type.clone()),
            GraphEdgeMeta {
                edge_type: edge.edge_type,
                origin: edge.origin,
                confidence: edge.confidence,
                weight: edge.weight,
                memory_region: edge.memory_region,
            },
        );
    }

    /// Remove a link
    pub fn remove_link(&mut self, from: &str, to: &str) {
        if let Some(targets) = self.outgoing.get_mut(from) {
            targets.remove(to);
        }
        if let Some(sources) = self.incoming.get_mut(to) {
            sources.remove(from);
        }
        self.edge_meta
            .retain(|(source, target, _), _| source != from || target != to);
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
        self.edge_meta
            .retain(|(from, to, _), _| from != id && to != id);
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
        let mut renamed_meta = HashMap::new();
        for ((from, to, edge_type), meta) in self.edge_meta.drain() {
            let renamed_from = if from == old_id {
                new_id.to_string()
            } else {
                from
            };
            let renamed_to = if to == old_id { new_id.to_string() } else { to };
            renamed_meta.insert((renamed_from, renamed_to, edge_type), meta);
        }
        self.edge_meta = renamed_meta;
    }

    /// BFS: get outgoing neighbor IDs for a node
    pub fn outgoing_neighbors(&self, id: &str) -> Vec<String> {
        self.outgoing
            .get(id)
            .map(|s| s.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Outgoing edge count
    pub fn outgoing_count(&self, id: &str) -> usize {
        self.outgoing.get(id).map(|s| s.len()).unwrap_or(0)
    }

    /// Incoming edge count (backlinks)
    pub fn incoming_count(&self, id: &str) -> usize {
        self.incoming.get(id).map(|s| s.len()).unwrap_or(0)
    }

    /// Total degree = outgoing + incoming
    pub fn degree(&self, id: &str) -> usize {
        self.outgoing_count(id) + self.incoming_count(id)
    }

    /// Get stored title for a node
    pub fn title_of(&self, id: &str) -> Option<String> {
        self.titles.get(id).cloned()
    }

    /// All node IDs present in the graph
    pub fn all_ids(&self) -> Vec<String> {
        let mut ids: std::collections::HashSet<String> = self.outgoing.keys().cloned().collect();
        ids.extend(self.incoming.keys().cloned());
        ids.into_iter().collect()
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
        for k in self.titles.keys() {
            all_ids.insert(k);
        }

        let nodes: Vec<GraphNode> = all_ids
            .iter()
            .map(|id| GraphNode {
                id: id.to_string(),
                title: self.titles.get(*id).cloned().unwrap_or_else(|| {
                    id.rsplit('/')
                        .next()
                        .unwrap_or(id)
                        .trim_end_matches(".md")
                        .to_string()
                }),
                outgoing: self.outgoing.get(*id).map(|s| s.len()).unwrap_or(0),
                incoming: self.incoming.get(*id).map(|s| s.len()).unwrap_or(0),
                activation: None,
            })
            .collect();

        let edges = self.edges_from_meta();

        GraphData { nodes, edges }
    }

    /// Returns GraphData with activation scores based on BFS distance from focus_path.
    /// Nodes reachable at distance d get activation = 1.0 / (1.0 + d);
    /// unreachable nodes get activation = 0.0.
    pub fn get_graph_with_focus(&self, focus_path: &str) -> GraphData {
        let mut all_ids: HashSet<&String> = HashSet::new();
        for (k, v) in &self.outgoing {
            all_ids.insert(k);
            all_ids.extend(v.iter());
        }
        for k in self.incoming.keys() {
            all_ids.insert(k);
        }
        for k in self.titles.keys() {
            all_ids.insert(k);
        }

        // BFS from focus_path through outgoing + incoming edges to compute distances
        let mut distances: HashMap<String, usize> = HashMap::new();
        let mut queue: VecDeque<String> = VecDeque::new();

        if all_ids.contains(&focus_path.to_string()) {
            distances.insert(focus_path.to_string(), 0);
            queue.push_back(focus_path.to_string());
        }

        while let Some(current) = queue.pop_front() {
            let dist = distances[&current];
            // Enqueue outgoing neighbors
            if let Some(targets) = self.outgoing.get(&current) {
                for neighbor in targets {
                    if !distances.contains_key(neighbor) {
                        distances.insert(neighbor.clone(), dist + 1);
                        queue.push_back(neighbor.clone());
                    }
                }
            }
            // Enqueue incoming neighbors (backlinks are also semantically related)
            if let Some(sources) = self.incoming.get(&current) {
                for neighbor in sources {
                    if !distances.contains_key(neighbor) {
                        distances.insert(neighbor.clone(), dist + 1);
                        queue.push_back(neighbor.clone());
                    }
                }
            }
        }

        let nodes: Vec<GraphNode> = all_ids
            .iter()
            .map(|id| {
                let activation = distances
                    .get(*id)
                    .map(|d| 1.0f32 / (1.0 + *d as f32))
                    .unwrap_or(0.0);
                GraphNode {
                    id: id.to_string(),
                    title: self.titles.get(*id).cloned().unwrap_or_else(|| {
                        id.rsplit('/')
                            .next()
                            .unwrap_or(id)
                            .trim_end_matches(".md")
                            .to_string()
                    }),
                    outgoing: self.outgoing.get(*id).map(|s| s.len()).unwrap_or(0),
                    incoming: self.incoming.get(*id).map(|s| s.len()).unwrap_or(0),
                    activation: Some(activation),
                }
            })
            .collect();

        let edges = self.edges_from_meta();

        GraphData { nodes, edges }
    }

    fn edges_from_meta(&self) -> Vec<GraphEdge> {
        self.edge_meta
            .iter()
            .map(|((from, to, _), meta)| GraphEdge {
                from: from.clone(),
                to: to.clone(),
                edge_type: meta.edge_type.clone(),
                origin: meta.origin.clone(),
                confidence: meta.confidence,
                weight: meta.weight,
                memory_region: meta.memory_region.clone(),
            })
            .collect()
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

        assert!(graph
            .get_backlinks("target.md")
            .contains(&"new.md".to_string()));
        assert!(graph
            .get_backlinks("new.md")
            .contains(&"other.md".to_string()));
    }

    #[test]
    fn test_remove_note() {
        let mut graph = LinkGraph::new();
        graph.add_link("a.md", "b.md");
        graph.remove_note("a.md");
        assert!(graph.get_backlinks("b.md").is_empty());
    }

    #[test]
    fn test_typed_edges_are_exported() {
        let mut graph = LinkGraph::new();
        graph.add_typed_link("a.md", "b.md", GraphEdgeType::Reference, "wikilink", 1.0);
        let data = graph.to_frontend_json();
        assert_eq!(data.edges[0].edge_type, GraphEdgeType::Reference);
        assert_eq!(data.edges[0].origin, "wikilink");
        assert_eq!(data.edges[0].confidence, 1.0);
    }

    #[test]
    fn test_multiple_edge_types_between_same_nodes_are_preserved() {
        let mut graph = LinkGraph::new();
        graph.add_typed_link("a.md", "b.md", GraphEdgeType::Reference, "wikilink", 1.0);
        graph.add_typed_link("a.md", "b.md", GraphEdgeType::Bridge, "dream", 0.7);

        let data = graph.to_frontend_json();
        let types: HashSet<GraphEdgeType> = data
            .edges
            .iter()
            .map(|edge| edge.edge_type.clone())
            .collect();

        assert_eq!(data.edges.len(), 2);
        assert!(types.contains(&GraphEdgeType::Reference));
        assert!(types.contains(&GraphEdgeType::Bridge));
    }
}
