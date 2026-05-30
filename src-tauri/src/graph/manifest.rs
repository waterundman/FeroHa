// GraphManifest v2 — token-budgeted ranked graph context for AI synthesis
// v2: Communities layer (SQLite-backed or cold-start folder clustering)
use super::link_graph::LinkGraph;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

pub const HUB_DEGREE_THRESHOLD: usize = 15;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphManifest {
    pub builder_version: u32,
    pub total_nodes: usize,
    pub token_budget: usize,
    pub total_tokens: usize,
    pub truncated: bool,
    pub omitted_neighbors: usize,
    pub omitted_communities: usize,
    pub nodes: Vec<ManifestNode>,
    pub communities: Vec<ManifestCommunity>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestNode {
    pub path: String,
    pub title: String,
    pub description: String,
    pub relevance: f32,
    pub hop: u32,
    pub is_hub: bool,
    pub degree: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestCommunity {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub member_count: usize,
    pub overlap_with_targets: usize,
    pub confidence: f32,
}

impl GraphManifest {
    pub fn to_toml(&self) -> String {
        let mut out = String::new();
        out.push_str("[manifest]\n");
        out.push_str(&format!("builder_version = {}\n", self.builder_version));
        out.push_str(&format!("total_nodes = {}\n", self.total_nodes));
        out.push_str(&format!("token_budget = {}\n", self.token_budget));
        out.push_str(&format!("total_tokens = {}\n", self.total_tokens));
        out.push_str(&format!("truncated = {}\n", self.truncated));
        if self.truncated {
            out.push_str(&format!("omitted_neighbors = {}\n", self.omitted_neighbors));
            out.push_str(&format!(
                "omitted_communities = {}\n",
                self.omitted_communities
            ));
        }

        for node in &self.nodes {
            if node.path == "[truncated]" {
                out.push('\n');
                out.push_str(&format!("# [truncated] {}\n", node.description));
            } else {
                out.push('\n');
                out.push_str("[[nodes]]\n");
                out.push_str(&format!("path = \"{}\"\n", escape_toml_str(&node.path)));
                out.push_str(&format!("title = \"{}\"\n", escape_toml_str(&node.title)));
                out.push_str(&format!(
                    "description = \"{}\"\n",
                    escape_toml_str(&node.description)
                ));
                out.push_str(&format!("relevance = {}\n", fmt_f32(node.relevance)));
                out.push_str(&format!("hop = {}\n", node.hop));
                out.push_str(&format!("is_hub = {}\n", node.is_hub));
                out.push_str(&format!("degree = {}\n", node.degree));
            }
        }

        for community in &self.communities {
            out.push('\n');
            out.push_str("[[communities]]\n");
            out.push_str(&format!("id = \"{}\"\n", escape_toml_str(&community.id)));
            out.push_str(&format!(
                "title = \"{}\"\n",
                escape_toml_str(&community.title)
            ));
            out.push_str(&format!(
                "summary = \"{}\"\n",
                escape_toml_str(&community.summary)
            ));
            out.push_str(&format!("member_count = {}\n", community.member_count));
            out.push_str(&format!(
                "overlap_with_targets = {}\n",
                community.overlap_with_targets
            ));
            out.push_str(&format!("confidence = {}\n", fmt_f32(community.confidence)));
        }

        out
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }
}

pub struct GraphManifestBuilder<'a> {
    link_graph: &'a LinkGraph,
    vector_store: Option<&'a crate::ai::vectordb::VectorStore>,
    vault: Option<&'a crate::fs::vault::VaultManager>,
}

struct ScoredCandidate {
    node: ManifestNode,
    final_score: f32,
}

#[derive(Clone)]
struct BfsInfo {
    hop: u32,
    relevance: f32,
}

impl<'a> GraphManifestBuilder<'a> {
    pub fn new(link_graph: &'a LinkGraph) -> Self {
        GraphManifestBuilder {
            link_graph,
            vector_store: None,
            vault: None,
        }
    }

    pub fn with_vector_store(mut self, vs: &'a crate::ai::vectordb::VectorStore) -> Self {
        self.vector_store = Some(vs);
        self
    }

    pub fn with_vault(mut self, vault: &'a crate::fs::vault::VaultManager) -> Self {
        self.vault = Some(vault);
        self
    }

    pub fn build(
        &self,
        target_paths: &[String],
        max_hops: u32,
        token_budget: usize,
    ) -> GraphManifest {
        let vault_ref = self.vault;
        let vs_ref = self.vector_store;
        let mut visited: HashMap<String, BfsInfo> = HashMap::new();
        let mut queue: VecDeque<(String, u32, f32)> = VecDeque::new();

        for target in target_paths {
            visited.insert(
                target.clone(),
                BfsInfo {
                    hop: 0,
                    relevance: 1.0,
                },
            );
            queue.push_back((target.clone(), 0, 1.0));
        }

        while let Some((current, current_hop, current_relevance)) = queue.pop_front() {
            if current_hop >= max_hops {
                continue;
            }
            let degree = self.link_graph.degree(&current);
            if degree > HUB_DEGREE_THRESHOLD {
                continue;
            }
            let neighbors = self.link_graph.outgoing_neighbors(&current);
            for neighbor in neighbors {
                let new_hop = current_hop + 1;
                let new_relevance = current_relevance * 0.6_f32;
                if let Some(info) = visited.get_mut(&neighbor) {
                    if new_relevance > info.relevance {
                        info.relevance = new_relevance;
                    }
                } else {
                    visited.insert(
                        neighbor.clone(),
                        BfsInfo {
                            hop: new_hop,
                            relevance: new_relevance,
                        },
                    );
                    if new_hop < max_hops {
                        queue.push_back((neighbor, new_hop, new_relevance));
                    }
                }
            }
        }

        let mut candidates: Vec<ScoredCandidate> = Vec::new();
        for (path, info) in &visited {
            let degree = self.link_graph.degree(path);
            let is_hub = degree > HUB_DEGREE_THRESHOLD;
            let hop_decay = 0.6_f32.powi(info.hop as i32);
            let specificity = 1.0 / (1.0 + (degree as f32 + 1.0).log2());
            let final_score = info.relevance * hop_decay * specificity;
            let title = self.link_graph.title_of(path).unwrap_or_else(|| {
                path.rsplit('/')
                    .next()
                    .unwrap_or(path)
                    .trim_end_matches(".md")
                    .to_string()
            });
            let description = self.resolve_description(path, vault_ref, vs_ref);
            candidates.push(ScoredCandidate {
                node: ManifestNode {
                    path: path.clone(),
                    title,
                    description,
                    relevance: info.relevance,
                    hop: info.hop,
                    is_hub,
                    degree,
                },
                final_score,
            });
        }

        candidates.sort_by(|a, b| {
            b.final_score
                .partial_cmp(&a.final_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let total_candidates = candidates.len();
        let mut manifest_nodes: Vec<ManifestNode> = Vec::new();
        let mut total_tokens: usize = 0;
        let mut truncated = false;

        let header_tokens = estimate_tokens(
            "[manifest]\nbuilder_version = 2\ntotal_nodes = 0\ntoken_budget = 0\ntotal_tokens = 0\ntruncated = false\n",
        );
        total_tokens += header_tokens;

        for (i, candidate) in candidates.into_iter().enumerate() {
            let node = candidate.node;
            let node_tokens = estimate_node_tokens(&node.path, &node.title, &node.description);
            if total_tokens + node_tokens > token_budget {
                truncated = true;
                let omitted_nodes = total_candidates - i;
                if omitted_nodes > 0 {
                    manifest_nodes.push(ManifestNode {
                        path: "[truncated]".to_string(),
                        title: "[truncated]".to_string(),
                        description: format!(
                            "{} neighbors + communities omitted due to token budget",
                            omitted_nodes
                        ),
                        relevance: 0.0,
                        hop: 0,
                        is_hub: false,
                        degree: 0,
                    });
                }
                break;
            }
            total_tokens += node_tokens;
            manifest_nodes.push(node);
        }

        let all_candidate_paths: Vec<String> = visited.keys().cloned().collect();
        let communities = self.build_communities(target_paths, &all_candidate_paths);
        let omitted_communities = if truncated && !communities.is_empty() {
            communities.len()
        } else {
            0
        };

        GraphManifest {
            builder_version: 2,
            total_nodes: manifest_nodes.len(),
            token_budget,
            total_tokens,
            truncated,
            omitted_neighbors: if truncated {
                total_candidates.saturating_sub(manifest_nodes.len().saturating_sub(1))
            } else {
                0
            },
            omitted_communities,
            nodes: manifest_nodes,
            communities: if truncated { Vec::new() } else { communities },
        }
    }

    fn build_communities(
        &self,
        target_paths: &[String],
        _candidate_paths: &[String],
    ) -> Vec<ManifestCommunity> {
        if let Some(vs) = self.vector_store {
            let db_communities = vs.load_communities();
            if !db_communities.is_empty() {
                let target_set: HashSet<&str> = target_paths.iter().map(|s| s.as_str()).collect();
                let mut scored: Vec<(usize, &crate::ai::vectordb::CommunityRecord)> =
                    db_communities
                        .iter()
                        .map(|c| {
                            let overlap = c
                                .members
                                .iter()
                                .filter(|m| target_set.contains(m.as_str()))
                                .count();
                            (overlap, c)
                        })
                        .collect();
                scored.sort_by_key(|b| std::cmp::Reverse(b.0));
                return scored
                    .iter()
                    .take(3)
                    .map(|(overlap, c)| ManifestCommunity {
                        id: c.id.clone(),
                        title: c.title.clone(),
                        summary: c.summary.clone(),
                        member_count: c.member_count,
                        overlap_with_targets: *overlap,
                        confidence: c.confidence,
                    })
                    .collect();
            }
        }

        if let Some(vault) = self.vault {
            if let Ok(notes) = vault.list_notes() {
                let mut prefix_groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
                for note in &notes {
                    let prefix = note
                        .path
                        .rsplit_once('/')
                        .map(|(dir, _)| dir.to_string())
                        .unwrap_or_else(|| "root".to_string());
                    prefix_groups
                        .entry(prefix)
                        .or_default()
                        .push(note.path.clone());
                }
                let target_set: HashSet<&str> = target_paths.iter().map(|s| s.as_str()).collect();
                let mut cold_communities: Vec<(usize, ManifestCommunity)> = prefix_groups
                    .iter()
                    .filter(|(_, members)| members.len() >= 2)
                    .map(|(prefix, members)| {
                        let overlap = members
                            .iter()
                            .filter(|m| target_set.contains(m.as_str()))
                            .count();
                        (
                            overlap,
                            ManifestCommunity {
                                id: format!("cold_{}", prefix.replace('/', "_")),
                                title: format!("Folder: {}", prefix),
                                summary: format!("{} notes in folder '{}'", members.len(), prefix),
                                member_count: members.len(),
                                overlap_with_targets: overlap,
                                confidence: 0.3,
                            },
                        )
                    })
                    .collect();
                cold_communities.sort_by_key(|b| std::cmp::Reverse(b.0));
                return cold_communities
                    .into_iter()
                    .take(3)
                    .map(|(_, c)| c)
                    .collect();
            }
        }

        Vec::new()
    }

    fn resolve_description(
        &self,
        path: &str,
        vault: Option<&crate::fs::vault::VaultManager>,
        vector_store: Option<&crate::ai::vectordb::VectorStore>,
    ) -> String {
        if let Some(vs) = vector_store {
            let chunks = vs.get_chunks_for_file(path);
            if let Some(chunk) = chunks.first() {
                let text = if !chunk.heading_context.is_empty() {
                    &chunk.heading_context
                } else {
                    &chunk.chunk_text
                };
                let desc: String = text.chars().take(200).collect();
                if !desc.is_empty() {
                    return desc;
                }
            }
        }
        if let Some(vault) = vault {
            if let Ok(content) = vault.read_note(path) {
                let para = extract_first_paragraph(&content);
                if !para.is_empty() {
                    return para;
                }
            }
        }
        String::new()
    }
}

fn fmt_f32(v: f32) -> String {
    let s = format!("{:.6}", v);
    let s = s.trim_end_matches('0');
    let s = s.trim_end_matches('.');
    if s.is_empty() {
        "0".to_string()
    } else {
        s.to_string()
    }
}

fn escape_toml_str(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

pub fn estimate_tokens(text: &str) -> usize {
    let ascii_count = text.chars().filter(|c| c.is_ascii()).count();
    let non_ascii_count = text.chars().filter(|c| !c.is_ascii()).count();
    (ascii_count / 4) + non_ascii_count
}

fn estimate_node_tokens(path: &str, title: &str, description: &str) -> usize {
    estimate_tokens(path) + estimate_tokens(title) + estimate_tokens(description) + 10
}

pub fn extract_first_paragraph(content: &str) -> String {
    let mut in_frontmatter = false;
    let mut frontmatter_done = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "---" {
            if !frontmatter_done {
                in_frontmatter = !in_frontmatter;
                if !in_frontmatter {
                    frontmatter_done = true;
                }
            }
            continue;
        }
        if in_frontmatter {
            continue;
        }
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let truncated: String = trimmed.chars().take(200).collect();
        return truncated;
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_graph() -> LinkGraph {
        let mut g = LinkGraph::new();
        g.add_link("target.md", "a.md");
        g.add_link("target.md", "b.md");
        g.add_link("a.md", "c.md");
        g.add_link("b.md", "d.md");
        g.set_title("target.md", "Target Note");
        g.set_title("a.md", "Note A");
        g.set_title("b.md", "Note B");
        g.set_title("c.md", "Note C");
        g.set_title("d.md", "Note D");
        g
    }

    #[test]
    fn test_build_empty_targets() {
        let graph = LinkGraph::new();
        let builder = GraphManifestBuilder::new(&graph);
        let manifest = builder.build(&[], 2, 500);
        assert_eq!(manifest.nodes.len(), 0);
        assert_eq!(manifest.total_nodes, 0);
        assert!(!manifest.truncated);
    }

    #[test]
    fn test_single_target_basic() {
        let graph = make_test_graph();
        let builder = GraphManifestBuilder::new(&graph);
        let targets = vec!["target.md".to_string()];
        let manifest = builder.build(&targets, 2, 5000);
        assert!(!manifest.nodes.is_empty());
        let target_node = manifest
            .nodes
            .iter()
            .find(|n| n.path == "target.md")
            .unwrap();
        assert_eq!(target_node.hop, 0);
        assert!((target_node.relevance - 1.0).abs() < 0.001);
        assert_eq!(target_node.title, "Target Note");
        let a_node = manifest.nodes.iter().find(|n| n.path == "a.md").unwrap();
        assert_eq!(a_node.hop, 1);
        assert!((a_node.relevance - 0.6).abs() < 0.01);
    }

    #[test]
    fn test_hub_skip() {
        let mut graph = LinkGraph::new();
        graph.add_link("hub.md", "leaf1.md");
        graph.add_link("hub.md", "leaf2.md");
        graph.add_link("hub.md", "leaf3.md");
        graph.add_link("leaf1.md", "hub.md");
        graph.add_link("leaf2.md", "hub.md");
        graph.add_link("leaf3.md", "hub.md");
        for i in 0..20 {
            graph.add_link("hub.md", &format!("extra_{}.md", i));
        }
        let builder = GraphManifestBuilder::new(&graph);
        let targets = vec!["hub.md".to_string()];
        let manifest = builder.build(&targets, 2, 5000);
        let hub_node = manifest.nodes.iter().find(|n| n.path == "hub.md").unwrap();
        assert!(hub_node.is_hub);
        assert!(hub_node.degree > HUB_DEGREE_THRESHOLD);
        for leaf in &["leaf1.md", "leaf2.md", "leaf3.md"] {
            assert!(manifest.nodes.iter().find(|n| n.path == *leaf).is_none());
        }
    }

    #[test]
    fn test_token_budget_truncation() {
        let mut graph = LinkGraph::new();
        graph.add_link("root.md", "child1.md");
        graph.add_link("root.md", "child2.md");
        graph.add_link("root.md", "child3.md");
        graph.set_title("root.md", "Root");
        graph.set_title("child1.md", "Child 1 with a longish title text here");
        graph.set_title("child2.md", "Child 2");
        graph.set_title("child3.md", "Child 3");
        let builder = GraphManifestBuilder::new(&graph);
        let targets = vec!["root.md".to_string()];
        let manifest = builder.build(&targets, 2, 30);
        assert!(manifest.truncated);
        assert!(manifest.nodes.len() >= 1);
        assert!(manifest.nodes.iter().any(|n| n.path == "[truncated]"));
    }

    #[test]
    fn test_estimate_tokens() {
        assert_eq!(estimate_tokens("hello"), 1);
        assert_eq!(estimate_tokens("hello world"), 2);
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("你好"), 2);
        assert_eq!(estimate_tokens("你好世界"), 4);
        assert_eq!(estimate_tokens("hello你好"), 3);
    }

    #[test]
    fn test_to_toml_format() {
        let manifest = GraphManifest {
            builder_version: 2,
            total_nodes: 2,
            token_budget: 500,
            total_tokens: 100,
            truncated: false,
            omitted_neighbors: 0,
            omitted_communities: 0,
            nodes: vec![
                ManifestNode {
                    path: "notes/test.md".to_string(),
                    title: "Test Note".to_string(),
                    description: "A test note about testing".to_string(),
                    relevance: 1.0,
                    hop: 0,
                    is_hub: false,
                    degree: 3,
                },
                ManifestNode {
                    path: "notes/other.md".to_string(),
                    title: "Other".to_string(),
                    description: "".to_string(),
                    relevance: 0.6,
                    hop: 1,
                    is_hub: false,
                    degree: 1,
                },
            ],
            communities: vec![ManifestCommunity {
                id: "comm_1".to_string(),
                title: "Test Community".to_string(),
                summary: "A community from dream".to_string(),
                member_count: 5,
                overlap_with_targets: 2,
                confidence: 0.8,
            }],
        };
        let toml_str = manifest.to_toml();
        assert!(toml_str.contains("[manifest]"));
        assert!(toml_str.contains("builder_version = 2"));
        assert!(toml_str.contains("[[nodes]]"));
        assert!(toml_str.contains("path = \"notes/test.md\""));
        assert!(toml_str.contains("[[communities]]"));
        assert!(toml_str.contains("id = \"comm_1\""));
        assert!(toml_str.contains("overlap_with_targets = 2"));
    }

    #[test]
    fn test_to_toml_truncated_comment() {
        let manifest = GraphManifest {
            builder_version: 2,
            total_nodes: 2,
            token_budget: 100,
            total_tokens: 80,
            truncated: true,
            omitted_neighbors: 5,
            omitted_communities: 2,
            nodes: vec![
                ManifestNode {
                    path: "a.md".to_string(),
                    title: "A".to_string(),
                    description: "".to_string(),
                    relevance: 1.0,
                    hop: 0,
                    is_hub: false,
                    degree: 0,
                },
                ManifestNode {
                    path: "[truncated]".to_string(),
                    title: "[truncated]".to_string(),
                    description: "5 neighbors + communities omitted due to token budget"
                        .to_string(),
                    relevance: 0.0,
                    hop: 0,
                    is_hub: false,
                    degree: 0,
                },
            ],
            communities: vec![],
        };
        let toml_str = manifest.to_toml();
        assert!(toml_str
            .contains("# [truncated] 5 neighbors + communities omitted due to token budget"));
        assert!(toml_str.contains("omitted_neighbors = 5"));
        assert!(toml_str.contains("omitted_communities = 2"));
        assert!(toml_str.contains("truncated = true"));
    }

    #[test]
    fn test_extract_first_paragraph() {
        assert_eq!(extract_first_paragraph("---\ntitle: Test\n---\n\n# Heading\n\nThis is the first paragraph.\n\nSecond paragraph."), "This is the first paragraph.");
        assert_eq!(
            extract_first_paragraph("# Heading\n\nFirst para here."),
            "First para here."
        );
        assert_eq!(extract_first_paragraph(""), "");
    }
}
