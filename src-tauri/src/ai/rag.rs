// RAG (Retrieval-Augmented Generation) Pipeline
// Stage 4: Hybrid retrieval combining vector search + graph traversal + keyword search

use super::vectordb::{SearchResult, VectorStore};
use crate::graph::link_graph::LinkGraph;
use crate::harness::context::{ContextFragment, ContextLayer, ContextSource};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalContext {
    pub chunk: SearchResult,
    pub source: RetrievalSource,
    pub relevance_score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RetrievalSource {
    VectorMatch,    // Direct semantic match
    GraphNeighbor,  // Retrieved via link graph traversal
    KeywordMatch,   // Full-text keyword match
    HeadingContext, // Retrieved because of shared heading
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagQuery {
    pub query_text: String,
    pub anchor_links: Vec<String>, // [[wikilinks]] that triggered the query
    pub max_results: usize,
    pub include_neighbors: bool, // Expand via graph neighbors
    pub neighbor_depth: usize,   // How many hops in graph
    pub vector_weight: f32,      // 0.0 - 1.0 contribution of vector search
    pub graph_weight: f32,       // 0.0 - 1.0 contribution of graph traversal
    pub keyword_weight: f32,     // 0.0 - 1.0 contribution of keyword search
}

impl Default for RagQuery {
    fn default() -> Self {
        RagQuery {
            query_text: String::new(),
            anchor_links: Vec::new(),
            max_results: 10,
            include_neighbors: true,
            neighbor_depth: 2,
            vector_weight: 0.5,
            graph_weight: 0.3,
            keyword_weight: 0.2,
        }
    }
}

impl RetrievalContext {
    #[allow(dead_code)]
    pub fn to_fragment(&self) -> ContextFragment {
        let key = format!("rag.{}", self.chunk.chunk_id);
        let value = serde_json::json!({
            "chunk_id": self.chunk.chunk_id,
            "chunk_text": self.chunk.chunk_text,
            "source_file": self.chunk.source_file,
            "heading_context": self.chunk.heading_context,
            "relevance_score": self.relevance_score,
        });
        let hash = ContextFragment::compute_hash(&key, &value);
        ContextFragment {
            id: format!("rag-{}", self.chunk.chunk_id),
            key,
            value,
            source: ContextSource::RAG,
            layer: ContextLayer::Note,
            created_at: chrono::Utc::now().timestamp_millis() as u64,
            ttl: Some(3600 * 1000),
            hash,
        }
    }
}

/// Hybrid RAG Pipeline
///
/// Retrieval strategy:
///   1. Vector search: semantic similarity in LanceDB
///   2. Graph traversal: follow [[wikilinks]] from anchors
///   3. Keyword search: SQLite FTS5 fallback
///
/// Results are merged, de-duplicated, and re-ranked by weighted score.
pub struct RagPipeline<'a> {
    vector_store: &'a VectorStore,
    link_graph: Option<&'a LinkGraph>,
}

impl<'a> RagPipeline<'a> {
    pub fn new(vector_store: &'a VectorStore) -> Self {
        RagPipeline {
            vector_store,
            link_graph: None,
        }
    }

    #[allow(dead_code)]
    pub fn with_graph(mut self, graph: &'a LinkGraph) -> Self {
        self.link_graph = Some(graph);
        self
    }

    /// Execute hybrid retrieval
    pub fn retrieve(&self, query: &RagQuery) -> Vec<RetrievalContext> {
        let mut results: Vec<RetrievalContext> = Vec::new();

        // 1. Vector search
        let vector_results = self.vector_search(query);
        results.extend(vector_results);

        // 2. Graph expansion from anchor links
        if query.include_neighbors {
            if let Some(ref graph) = self.link_graph {
                let graph_results = self.graph_expand(graph, query);
                results.extend(graph_results);
            }
        }

        // 3. Keyword search fallback
        let keyword_results = self.keyword_search(query);
        results.extend(keyword_results);

        // 4. Deduplicate by chunk_id
        let mut seen = std::collections::HashSet::new();
        results.retain(|r| seen.insert(r.chunk.chunk_id.clone()));

        // 5. Sort by relevance score (descending)
        results.sort_by(|a, b| {
            b.relevance_score
                .partial_cmp(&a.relevance_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // 6. Truncate to max results
        results.truncate(query.max_results);

        results
    }

    /// Vector search (LanceDB semantic similarity)
    fn vector_search(&self, query: &RagQuery) -> Vec<RetrievalContext> {
        // Stage 3: use actual embedding + vector search
        // Stage 0-4: text-based search as development fallback
        let raw_results = self
            .vector_store
            .search_text(&query.query_text, query.max_results);

        raw_results
            .into_iter()
            .map(|sr| RetrievalContext {
                relevance_score: sr.score * query.vector_weight,
                source: RetrievalSource::VectorMatch,
                chunk: sr,
            })
            .collect()
    }

    /// Graph expansion — follow [[wikilinks]] from anchor notes
    fn graph_expand(&self, graph: &LinkGraph, query: &RagQuery) -> Vec<RetrievalContext> {
        let mut results = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut frontier: Vec<String> = query.anchor_links.clone();

        for depth in 0..query.neighbor_depth {
            let mut next_frontier: Vec<String> = Vec::new();

            for note_id in &frontier {
                if !visited.insert(note_id.clone()) {
                    continue;
                }

                // Get backlinks (notes pointing TO this note)
                let backlinks = graph.get_backlinks(note_id);
                for bl in &backlinks {
                    if visited.contains(bl) {
                        continue;
                    }
                    next_frontier.push(bl.clone());

                    // Create a synthetic search result from graph neighbor
                    results.push(RetrievalContext {
                        chunk: SearchResult {
                            chunk_id: format!("graph_{}", bl),
                            chunk_text: format!("[Graph neighbor at depth {}]", depth + 1),
                            source_file: bl.clone(),
                            heading_context: format!("Linked from: {}", note_id),
                            score: 0.0,
                            similarity: 0.0,
                        },
                        source: RetrievalSource::GraphNeighbor,
                        relevance_score: (0.8_f32).powi(depth as i32 + 1) * query.graph_weight,
                    });
                }
            }

            frontier = next_frontier;
        }

        results
    }

    /// Keyword search fallback (SQLite FTS5 or simple text matching)
    fn keyword_search(&self, query: &RagQuery) -> Vec<RetrievalContext> {
        let raw_results = self
            .vector_store
            .search_text(&query.query_text, query.max_results / 2);

        raw_results
            .into_iter()
            .map(|sr| RetrievalContext {
                relevance_score: sr.score * 0.6 * query.keyword_weight, // Keyword slightly less weight
                source: RetrievalSource::KeywordMatch,
                chunk: sr,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::vectordb::ChunkRecord;

    fn make_store() -> VectorStore {
        let mut store = VectorStore::new(":memory:");
        store
            .upsert(ChunkRecord {
                id: "c1".to_string(),
                content_hash: 1,
                chunk_text: "Rust is a systems programming language with zero-cost abstractions."
                    .to_string(),
                embedding: vec![],
                source_file: "rust.md".to_string(),
                heading_context: "Introduction".to_string(),
                created_at: 0,
                updated_at: 0,
            })
            .unwrap();
        store
            .upsert(ChunkRecord {
                id: "c2".to_string(),
                content_hash: 2,
                chunk_text: "Tauri enables building desktop apps with web technologies."
                    .to_string(),
                embedding: vec![],
                source_file: "tauri.md".to_string(),
                heading_context: "Overview".to_string(),
                created_at: 0,
                updated_at: 0,
            })
            .unwrap();
        store
    }

    #[test]
    fn test_retrieve_vector_only() {
        let store = make_store();
        let pipeline = RagPipeline::new(&store);

        let query = RagQuery {
            query_text: "Rust programming".to_string(),
            max_results: 5,
            include_neighbors: false,
            ..Default::default()
        };

        let results = pipeline.retrieve(&query);
        assert!(!results.is_empty());
        assert!(results.iter().any(|r| r.chunk.source_file == "rust.md"));
    }

    #[test]
    fn test_retrieve_hybrid() {
        let store = make_store();
        let mut graph = LinkGraph::new();
        // tauri.md links to rust.md (tauri -> rust)
        graph.add_link("tauri.md", "rust.md");

        let pipeline = RagPipeline::new(&store).with_graph(&graph);

        let query = RagQuery {
            query_text: "desktop".to_string(),
            anchor_links: vec!["rust.md".to_string()],
            max_results: 10,
            include_neighbors: true,
            neighbor_depth: 2,
            ..Default::default()
        };

        let results = pipeline.retrieve(&query);
        // Should include graph neighbors (backlinks to rust.md)
        assert!(results
            .iter()
            .any(|r| matches!(r.source, RetrievalSource::GraphNeighbor)));
    }

    #[test]
    fn test_deduplication() {
        let store = make_store();
        let pipeline = RagPipeline::new(&store);

        let query = RagQuery {
            query_text: "rust".to_string(),
            max_results: 5,
            ..Default::default()
        };

        let results = pipeline.retrieve(&query);
        // No duplicate chunk_ids
        let ids: Vec<_> = results.iter().map(|r| &r.chunk.chunk_id).collect();
        let unique: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(ids.len(), unique.len());
    }
}
