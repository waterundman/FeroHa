// Dream Engine — Three-phase memory consolidation for AI surface
// Inspired by mazemaker's NREM/REM/Insight architecture

use serde::{Serialize, Deserialize};
use std::collections::{HashMap, HashSet, VecDeque};
use rand::seq::SliceRandom;
use super::vectordb::{VectorStore, ChunkRecord};

/// Dream session statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DreamStats {
    pub nrem_connections_strengthened: usize,
    pub nrem_connections_pruned: usize,
    pub rem_bridges_created: usize,
    pub insight_communities_found: usize,
    pub insight_summaries_generated: usize,
    pub total_memories_processed: usize,
    pub duration_ms: u64,
}

/// Dream insight — discovered pattern or community
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DreamInsight {
    pub id: String,
    pub insight_type: InsightType,
    pub title: String,
    pub summary: String,
    pub related_chunks: Vec<String>,
    pub confidence: f32,
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InsightType {
    Community,
    Pattern,
    Connection,
    Summary,
}

/// Connection between two chunks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    pub from_id: String,
    pub to_id: String,
    pub weight: f32,
    pub connection_type: ConnectionType,
    pub created_at: u64,
    pub last_activated: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConnectionType {
    Semantic,
    Temporal,
    Reference,
    Bridge,
}

/// Dream Engine — consolidates memories during idle periods
pub struct DreamEngine {
    connections: HashMap<String, Vec<Connection>>,
    insights: Vec<DreamInsight>,
    activation_scores: HashMap<String, f32>,
    config: DreamConfig,
}

#[derive(Debug, Clone)]
pub struct DreamConfig {
    pub nrem_sample_size: usize,
    pub rem_sample_size: usize,
    pub strengthen_rate: f32,
    pub prune_threshold: f32,
    pub bridge_similarity_threshold: f32,
    pub max_depth: usize,
}

impl Default for DreamConfig {
    fn default() -> Self {
        DreamConfig {
            nrem_sample_size: 100,
            rem_sample_size: 50,
            strengthen_rate: 0.05,
            prune_threshold: 0.05,
            bridge_similarity_threshold: 0.3,
            max_depth: 3,
        }
    }
}

impl DreamEngine {
    pub fn new() -> Self {
        DreamEngine {
            connections: HashMap::new(),
            insights: Vec::new(),
            activation_scores: HashMap::new(),
            config: DreamConfig::default(),
        }
    }

    pub fn with_config(config: DreamConfig) -> Self {
        DreamEngine {
            connections: HashMap::new(),
            insights: Vec::new(),
            activation_scores: HashMap::new(),
            config,
        }
    }

    /// Run a complete dream cycle (NREM + REM + Insight)
    pub fn run_cycle(&mut self, store: &VectorStore) -> DreamStats {
        let start = std::time::Instant::now();
        let mut stats = DreamStats {
            nrem_connections_strengthened: 0,
            nrem_connections_pruned: 0,
            rem_bridges_created: 0,
            insight_communities_found: 0,
            insight_summaries_generated: 0,
            total_memories_processed: 0,
            duration_ms: 0,
        };

        // Phase 1: NREM — strengthen important connections, prune weak ones
        let nrem_stats = self.run_nrem(store);
        stats.nrem_connections_strengthened = nrem_stats.0;
        stats.nrem_connections_pruned = nrem_stats.1;
        stats.total_memories_processed += nrem_stats.2;

        // Phase 2: REM — bridge isolated memories
        let rem_stats = self.run_rem(store);
        stats.rem_bridges_created = rem_stats.0;
        stats.total_memories_processed += rem_stats.1;

        // Phase 3: Insight — discover communities and patterns
        let insight_stats = self.run_insight(store);
        stats.insight_communities_found = insight_stats.0;
        stats.insight_summaries_generated = insight_stats.1;

        stats.duration_ms = start.elapsed().as_millis() as u64;
        stats
    }

    /// Phase 1: NREM — Non-Rapid Eye Movement sleep
    /// Strengthen active connections, prune weak ones
    fn run_nrem(&mut self, store: &VectorStore) -> (usize, usize, usize) {
        let mut strengthened = 0;
        let mut pruned = 0;

        // Sample memories: 50% recent, 30% random, 20% low-salience
        let all_chunks = self.sample_memories(store, self.config.nrem_sample_size);
        let processed = all_chunks.len();

        for chunk in &all_chunks {
            // Get or initialize activation score
            let _activation = self.activation_scores
                .entry(chunk.id.clone())
                .or_insert(0.5);

            // Spreading activation from this chunk
            let activated_neighbors = self.spread_activation(&chunk.id, 2);

            // Strengthen connections to activated neighbors
            for neighbor_id in &activated_neighbors {
                if let Some(connections) = self.connections.get_mut(&chunk.id) {
                    for conn in connections.iter_mut() {
                        if conn.to_id == *neighbor_id {
                            conn.weight += self.config.strengthen_rate;
                            conn.last_activated = current_timestamp();
                            strengthened += 1;
                        }
                    }
                }
            }

            // Prune weak connections
            if let Some(connections) = self.connections.get_mut(&chunk.id) {
                let before = connections.len();
                connections.retain(|c| c.weight >= self.config.prune_threshold);
                pruned += before - connections.len();
            }
        }

        (strengthened, pruned, processed)
    }

    /// Phase 2: REM — Rapid Eye Movement sleep
    /// Bridge isolated memories by finding similar unconnected pairs
    fn run_rem(&mut self, store: &VectorStore) -> (usize, usize) {
        let mut bridges_created = 0;

        // Find isolated memories (few or no connections)
        let isolated = self.find_isolated_memories(store, self.config.rem_sample_size);
        let processed = isolated.len();

        for chunk in &isolated {
            // Search for similar memories
            let similar = store.search_by_embedding(&chunk.embedding, 5);

            for result in &similar {
                if result.chunk_id == chunk.id {
                    continue;
                }

                // Check if connection already exists
                if !self.has_connection(&chunk.id, &result.chunk_id) {
                    // Create bridge connection
                    let bridge = Connection {
                        from_id: chunk.id.clone(),
                        to_id: result.chunk_id.clone(),
                        weight: result.similarity * 0.3, // Bridge weight is lower
                        connection_type: ConnectionType::Bridge,
                        created_at: current_timestamp(),
                        last_activated: current_timestamp(),
                    };

                    self.connections
                        .entry(chunk.id.clone())
                        .or_insert_with(Vec::new)
                        .push(bridge);

                    bridges_created += 1;
                }
            }
        }

        (bridges_created, processed)
    }

    /// Phase 3: Insight — Discover communities and generate summaries
    fn run_insight(&mut self, store: &VectorStore) -> (usize, usize) {
        let mut communities_found = 0;
        let mut summaries_generated = 0;

        // Find connected components using BFS
        let communities = self.find_communities();

        for (i, community) in communities.iter().enumerate() {
            if community.len() < 3 {
                continue; // Skip small communities
            }

            communities_found += 1;

            // Generate summary for this community
            let summary = self.generate_community_summary(store, community);
            if let Some(summary) = summary {
                let insight = DreamInsight {
                    id: format!("insight_{}", i),
                    insight_type: InsightType::Community,
                    title: format!("Community {}", i + 1),
                    summary: summary.text,
                    related_chunks: community.clone(),
                    confidence: summary.confidence,
                    created_at: current_timestamp(),
                };

                self.insights.push(insight);
                summaries_generated += 1;
            }
        }

        (communities_found, summaries_generated)
    }

    /// Spread activation from a starting chunk
    fn spread_activation(&self, start_id: &str, max_depth: usize) -> HashSet<String> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back((start_id.to_string(), 0));
        visited.insert(start_id.to_string());

        while let Some((current_id, depth)) = queue.pop_front() {
            if depth >= max_depth {
                continue;
            }

            if let Some(connections) = self.connections.get(&current_id) {
                for conn in connections {
                    if !visited.contains(&conn.to_id) {
                        visited.insert(conn.to_id.clone());
                        queue.push_back((conn.to_id.clone(), depth + 1));
                    }
                }
            }
        }

        visited
    }

    /// Find isolated memories (few connections)
    fn find_isolated_memories(&self, store: &VectorStore, limit: usize) -> Vec<ChunkRecord> {
        let all = store.records();
        let mut isolated: Vec<ChunkRecord> = all.iter()
            .filter(|c| {
                let count = self.connections.get(&c.id).map(|v| v.len()).unwrap_or(0);
                count < 2
            })
            .take(limit)
            .cloned()
            .collect();

        // Sort by connection count ascending (most isolated first)
        isolated.sort_by_key(|c| {
            self.connections.get(&c.id).map(|v| v.len()).unwrap_or(0)
        });
        isolated
    }

    /// Check if connection exists between two chunks
    fn has_connection(&self, from_id: &str, to_id: &str) -> bool {
        self.connections
            .get(from_id)
            .map(|conns| conns.iter().any(|c| c.to_id == to_id))
            .unwrap_or(false)
    }

    /// Find communities using BFS on the connection graph
    fn find_communities(&self) -> Vec<Vec<String>> {
        let mut communities = Vec::new();
        let mut visited = HashSet::new();

        for start_id in self.connections.keys() {
            if visited.contains(start_id) {
                continue;
            }

            let mut community = Vec::new();
            let mut queue = VecDeque::new();
            queue.push_back(start_id.clone());
            visited.insert(start_id.clone());

            while let Some(current_id) = queue.pop_front() {
                community.push(current_id.clone());

                if let Some(connections) = self.connections.get(&current_id) {
                    for conn in connections {
                        if !visited.contains(&conn.to_id) {
                            visited.insert(conn.to_id.clone());
                            queue.push_back(conn.to_id.clone());
                        }
                    }
                }
            }

            if !community.is_empty() {
                communities.push(community);
            }
        }

        communities
    }

    /// Generate summary for a community of chunks
    fn generate_community_summary(&self, store: &VectorStore, community: &[String]) -> Option<CommunitySummary> {
        if community.is_empty() {
            return None;
        }

        // Collect texts from community members by fetching from store
        let texts: Vec<String> = community.iter()
            .filter_map(|id| store.get_record(id).map(|r| r.chunk_text.clone()))
            .collect();

        if texts.is_empty() {
            return None;
        }

        // Simple summary: join first few texts
        let summary_text = texts.iter()
            .take(3)
            .cloned()
            .collect::<Vec<_>>()
            .join(". ");

        Some(CommunitySummary {
            text: summary_text,
            confidence: 0.7,
        })
    }

    /// Sample memories for processing
    /// Three-slice sampling: 50% most recent, 30% random, 20% lowest salience
    fn sample_memories(&self, store: &VectorStore, limit: usize) -> Vec<ChunkRecord> {
        let all = store.records();
        if all.is_empty() {
            return Vec::new();
        }

        let n = limit.min(all.len());
        let n_recent = (n as f64 * 0.5).ceil() as usize;
        let n_random = (n as f64 * 0.3).ceil() as usize;
        let n_low_salience = n.saturating_sub(n_recent).min(n.saturating_sub(n_random));

        let mut rng = rand::thread_rng();

        // 50% most recent by updated_at descending
        let mut by_time: Vec<&ChunkRecord> = all.iter().collect();
        by_time.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        let recent: Vec<ChunkRecord> = by_time.iter().take(n_recent).map(|&c| c.clone()).collect();

        // 30% random from remaining
        let recent_ids: HashSet<&str> = recent.iter().map(|c| c.id.as_str()).collect();
        let remaining: Vec<&ChunkRecord> = all.iter()
            .filter(|c| !recent_ids.contains(c.id.as_str()))
            .collect();
        let random: Vec<ChunkRecord> = remaining
            .choose_multiple(&mut rng, n_random)
            .map(|&c| c.clone())
            .collect();

        // 20% lowest salience (fewest connections)
        let sampled_ids: HashSet<&str> = recent.iter().chain(random.iter())
            .map(|c| c.id.as_str())
            .collect();
        let unsampled: Vec<&ChunkRecord> = all.iter()
            .filter(|c| !sampled_ids.contains(c.id.as_str()))
            .collect();
        let mut with_conn_count: Vec<(usize, &ChunkRecord)> = unsampled.iter()
            .map(|c| {
                let count = self.connections.get(&c.id).map(|v| v.len()).unwrap_or(0);
                (count, *c)
            })
            .collect();
        with_conn_count.sort_by_key(|&(count, _)| count);
        let low_salience: Vec<ChunkRecord> = with_conn_count.iter()
            .take(n_low_salience)
            .map(|(_, c)| (*c).clone())
            .collect();

        let mut result = recent;
        result.extend(random);
        result.extend(low_salience);
        result
    }

    /// Get all insights
    pub fn get_insights(&self) -> &[DreamInsight] {
        &self.insights
    }

    /// Get connection count
    pub fn connection_count(&self) -> usize {
        self.connections.values().map(|c| c.len()).sum()
    }
}

struct CommunitySummary {
    text: String,
    confidence: f32,
}

fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dream_engine_creation() {
        let engine = DreamEngine::new();
        assert_eq!(engine.connection_count(), 0);
        assert_eq!(engine.get_insights().len(), 0);
    }

    #[test]
    fn test_spread_activation() {
        let mut engine = DreamEngine::new();

        // Add some connections
        engine.connections.insert("a".to_string(), vec![
            Connection {
                from_id: "a".to_string(),
                to_id: "b".to_string(),
                weight: 0.8,
                connection_type: ConnectionType::Semantic,
                created_at: 0,
                last_activated: 0,
            },
        ]);
        engine.connections.insert("b".to_string(), vec![
            Connection {
                from_id: "b".to_string(),
                to_id: "c".to_string(),
                weight: 0.6,
                connection_type: ConnectionType::Semantic,
                created_at: 0,
                last_activated: 0,
            },
        ]);

        let activated = engine.spread_activation("a", 2);
        assert!(activated.contains("a"));
        assert!(activated.contains("b"));
        assert!(activated.contains("c"));
    }

    #[test]
    fn test_find_communities() {
        let mut engine = DreamEngine::new();

        // Create two separate communities
        engine.connections.insert("a".to_string(), vec![
            Connection {
                from_id: "a".to_string(),
                to_id: "b".to_string(),
                weight: 0.8,
                connection_type: ConnectionType::Semantic,
                created_at: 0,
                last_activated: 0,
            },
        ]);
        engine.connections.insert("b".to_string(), vec![
            Connection {
                from_id: "b".to_string(),
                to_id: "a".to_string(),
                weight: 0.8,
                connection_type: ConnectionType::Semantic,
                created_at: 0,
                last_activated: 0,
            },
        ]);
        engine.connections.insert("x".to_string(), vec![
            Connection {
                from_id: "x".to_string(),
                to_id: "y".to_string(),
                weight: 0.7,
                connection_type: ConnectionType::Semantic,
                created_at: 0,
                last_activated: 0,
            },
        ]);

        let communities = engine.find_communities();
        assert_eq!(communities.len(), 2);
    }
}
