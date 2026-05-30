// Dream Engine — Three-phase memory consolidation for AI surface
// Inspired by mazemaker's NREM/REM/Insight architecture
// v2.8: Community persistence to SQLite + dream run logs

use super::vectordb::{ChunkRecord, CommunityRecord, VectorStore};
use crate::harness::regression::DreamAuditSnapshot;
use chrono::Local;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

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
#[derive(Clone)]
pub struct DreamEngine {
    connections: HashMap<String, Vec<Connection>>,
    insights: Vec<DreamInsight>,
    activation_scores: HashMap<String, f32>,
    config: DreamConfig,
    cycle_id: String,
    dualtrack_dir: Option<PathBuf>,
    pub last_stats: Option<DreamStats>,
    pub last_insights: Vec<DreamInsight>,
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

impl Default for DreamEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl DreamEngine {
    pub fn new() -> Self {
        DreamEngine {
            connections: HashMap::new(),
            insights: Vec::new(),
            activation_scores: HashMap::new(),
            config: DreamConfig::default(),
            cycle_id: String::new(),
            dualtrack_dir: None,
            last_stats: None,
            last_insights: Vec::new(),
        }
    }

    pub fn with_config(config: DreamConfig) -> Self {
        DreamEngine {
            connections: HashMap::new(),
            insights: Vec::new(),
            activation_scores: HashMap::new(),
            config,
            cycle_id: String::new(),
            dualtrack_dir: None,
            last_stats: None,
            last_insights: Vec::new(),
        }
    }

    pub fn set_dualtrack_dir(&mut self, dir: &Path) {
        self.dualtrack_dir = Some(dir.to_path_buf());
    }

    /// Run a complete dream cycle (NREM + REM + Insight)
    pub fn run_cycle(&mut self, store: &VectorStore) -> DreamStats {
        let start = std::time::Instant::now();
        let now = current_timestamp();
        self.cycle_id = format!("dream_{}", now);

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

        self.last_stats = Some(stats.clone());
        self.last_insights = self.insights.clone();

        // Persist communities to SQLite
        self.persist_communities(store);

        // Write dream run log
        self.write_dream_log(&stats);

        stats
    }

    fn persist_communities(&self, store: &VectorStore) {
        let records: Vec<CommunityRecord> = self
            .insights
            .iter()
            .map(|insight| CommunityRecord {
                id: insight.id.clone(),
                title: insight.title.clone(),
                summary: insight.summary.clone(),
                member_count: insight.related_chunks.len(),
                members: insight.related_chunks.clone(),
                sample_texts: vec![insight.summary.clone()],
                confidence: insight.confidence,
                created_at: insight.created_at as i64,
                cycle_id: self.cycle_id.clone(),
            })
            .collect();

        if let Err(e) = store.save_communities(&records) {
            tracing::warn!("Failed to persist communities: {}", e);
        } else {
            tracing::info!(
                "Persisted {} communities to SQLite (cycle: {})",
                records.len(),
                self.cycle_id
            );
        }
    }

    fn write_dream_log(&self, stats: &DreamStats) {
        let dir = match &self.dualtrack_dir {
            Some(d) => d.clone(),
            None => return,
        };
        let dream_dir = dir.join("dream");
        if let Err(e) = std::fs::create_dir_all(&dream_dir) {
            tracing::warn!("Failed to create dream log dir: {}", e);
            return;
        }

        let today = Local::now().format("%Y%m%d").to_string();
        let log_path = dream_dir.join(format!("dream_{}.log", today));

        let log_entry = format!(
            "[{}] cycle={} nrem_strengthened={} nrem_pruned={} rem_bridges={} communities={} summaries={} memories={} duration_ms={} insights={}\n",
            Local::now().format("%Y-%m-%dT%H:%M:%S%.3f"),
            self.cycle_id,
            stats.nrem_connections_strengthened,
            stats.nrem_connections_pruned,
            stats.rem_bridges_created,
            stats.insight_communities_found,
            stats.insight_summaries_generated,
            stats.total_memories_processed,
            stats.duration_ms,
            self.insights.len(),
        );

        use std::io::Write;
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
        {
            let _ = file.write_all(log_entry.as_bytes());
        }
    }

    /// Load previous communities from SQLite and restore engine state
    pub fn load_from_db(&mut self, store: &VectorStore) {
        let communities = store.load_communities();
        if communities.is_empty() {
            tracing::info!("No previous communities found in database");
            return;
        }

        self.insights = communities
            .iter()
            .map(|c| DreamInsight {
                id: c.id.clone(),
                insight_type: InsightType::Community,
                title: c.title.clone(),
                summary: c.summary.clone(),
                related_chunks: c.members.clone(),
                confidence: c.confidence,
                created_at: c.created_at as u64,
            })
            .collect();

        tracing::info!(
            "Loaded {} communities from previous dream cycles",
            self.insights.len()
        );
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
            let _activation = self
                .activation_scores
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
                        .or_default()
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
        let mut isolated: Vec<ChunkRecord> = all
            .iter()
            .filter(|c| {
                let count = self.connections.get(&c.id).map(|v| v.len()).unwrap_or(0);
                count < 2
            })
            .take(limit)
            .cloned()
            .collect();

        // Sort by connection count ascending (most isolated first)
        isolated.sort_by_key(|c| self.connections.get(&c.id).map(|v| v.len()).unwrap_or(0));
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
    fn generate_community_summary(
        &self,
        store: &VectorStore,
        community: &[String],
    ) -> Option<CommunitySummary> {
        if community.is_empty() {
            return None;
        }

        let texts: Vec<String> = community
            .iter()
            .filter_map(|id| store.get_record(id).map(|r| r.chunk_text.clone()))
            .collect();

        if texts.is_empty() {
            return None;
        }

        let summary_text = texts.iter().take(3).cloned().collect::<Vec<_>>().join(". ");

        Some(CommunitySummary {
            text: summary_text,
            confidence: 0.7,
        })
    }

    /// Enhance existing insights with LLM-generated summaries.
    /// Called externally after a dream cycle completes (requires async context).
    pub async fn enhance_insights_with_llm(
        &mut self,
        store: &VectorStore,
        llm_router: &mut crate::ai::llm_router::LlmRouter,
    ) {
        for insight in self.insights.iter_mut() {
            let texts: Vec<String> = insight
                .related_chunks
                .iter()
                .filter_map(|id| store.get_record(id).map(|r| r.chunk_text.clone()))
                .collect();

            if texts.is_empty() {
                continue;
            }

            let combined_text = texts
                .iter()
                .take(5)
                .cloned()
                .collect::<Vec<_>>()
                .join("\n\n---\n\n");
            let prompt = format!(
                "Summarize the key themes and insights from the following collection of notes. \
                 Provide: 1) A concise title (max 10 words) 2) A 2-3 sentence summary 3) Key connections between ideas.\n\n\
                 Notes:\n{}\n\nRespond with JSON: {{\"title\": \"...\", \"summary\": \"...\", \"connections\": \"...\", \"confidence\": 0.X}}",
                combined_text.chars().take(3000).collect::<String>()
            );

            let mut router_clone = llm_router.clone();
            if let Ok(response) = router_clone
                .complete(
                    "You are a knowledge synthesizer. Return only valid JSON.",
                    &prompt,
                    None,
                )
                .await
            {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&response.text) {
                    let title = json["title"].as_str().unwrap_or("Community Insight");
                    let summary = json["summary"].as_str().unwrap_or("");
                    let connections = json["connections"].as_str().unwrap_or("");
                    insight.title = title.to_string();
                    insight.summary = format!("**{}**: {}\n\n{}", title, summary, connections);
                    insight.confidence = json["confidence"].as_f64().unwrap_or(0.7) as f32;
                }
            }
        }
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
        let n_low_salience = n.saturating_sub(n_recent + n_random);

        let mut rng = rand::thread_rng();

        // 50% most recent by updated_at descending
        let mut by_time: Vec<&ChunkRecord> = all.iter().collect();
        by_time.sort_by_key(|b| std::cmp::Reverse(b.updated_at));
        let recent: Vec<ChunkRecord> = by_time.iter().take(n_recent).map(|&c| c.clone()).collect();

        // 30% random from remaining
        let recent_ids: HashSet<&str> = recent.iter().map(|c| c.id.as_str()).collect();
        let remaining: Vec<&ChunkRecord> = all
            .iter()
            .filter(|c| !recent_ids.contains(c.id.as_str()))
            .collect();
        let random: Vec<ChunkRecord> = remaining
            .choose_multiple(&mut rng, n_random)
            .map(|&c| c.clone())
            .collect();

        // 20% lowest salience (fewest connections)
        let sampled_ids: HashSet<&str> = recent
            .iter()
            .chain(random.iter())
            .map(|c| c.id.as_str())
            .collect();
        let unsampled: Vec<&ChunkRecord> = all
            .iter()
            .filter(|c| !sampled_ids.contains(c.id.as_str()))
            .collect();
        let mut with_conn_count: Vec<(usize, &ChunkRecord)> = unsampled
            .iter()
            .map(|c| {
                let count = self.connections.get(&c.id).map(|v| v.len()).unwrap_or(0);
                (count, *c)
            })
            .collect();
        with_conn_count.sort_by_key(|&(count, _)| count);
        let low_salience: Vec<ChunkRecord> = with_conn_count
            .iter()
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

    /// Get last dream stats
    pub fn last_stats(&self) -> Option<&DreamStats> {
        self.last_stats.as_ref()
    }

    /// Get last dream insights
    pub fn last_insights(&self) -> &[DreamInsight] {
        &self.last_insights
    }

    pub fn audit_snapshot(&self) -> DreamAuditSnapshot {
        let insight_iter = self.insights.iter().chain(self.last_insights.iter());
        let mut community_chunks = HashSet::new();
        let mut contradiction_risk = 0.0_f32;

        for insight in insight_iter {
            if matches!(insight.insight_type, InsightType::Community) {
                community_chunks.extend(insight.related_chunks.iter().cloned());
            }

            let contradiction_text =
                format!("{} {}", insight.title, insight.summary).to_ascii_lowercase();
            if contradiction_text.contains("contradiction")
                || contradiction_text.contains("contradicts")
                || contradiction_text.contains("conflict")
                || contradiction_text.contains("inconsistent")
            {
                contradiction_risk = contradiction_risk.max(insight.confidence);
            }
        }

        let coverage_denominator = if self.activation_scores.is_empty() {
            community_chunks.len()
        } else {
            self.activation_scores.len()
        };
        let community_coverage = if coverage_denominator == 0 {
            0.0
        } else {
            (community_chunks.len() as f32 / coverage_denominator as f32).min(1.0)
        };

        let salience_shift = if self.activation_scores.len() < 2 {
            0.0
        } else {
            let min = self
                .activation_scores
                .values()
                .copied()
                .fold(f32::INFINITY, f32::min);
            let max = self
                .activation_scores
                .values()
                .copied()
                .fold(f32::NEG_INFINITY, f32::max);
            (max - min).max(0.0)
        };

        DreamAuditSnapshot {
            community_coverage,
            salience_shift,
            contradiction_risk,
        }
    }

    /// Get connection count
    pub fn connection_count(&self) -> usize {
        self.connections.values().map(|c| c.len()).sum()
    }
}

pub struct CommunitySummary {
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
        engine.connections.insert(
            "a".to_string(),
            vec![Connection {
                from_id: "a".to_string(),
                to_id: "b".to_string(),
                weight: 0.8,
                connection_type: ConnectionType::Semantic,
                created_at: 0,
                last_activated: 0,
            }],
        );
        engine.connections.insert(
            "b".to_string(),
            vec![Connection {
                from_id: "b".to_string(),
                to_id: "c".to_string(),
                weight: 0.6,
                connection_type: ConnectionType::Semantic,
                created_at: 0,
                last_activated: 0,
            }],
        );

        let activated = engine.spread_activation("a", 2);
        assert!(activated.contains("a"));
        assert!(activated.contains("b"));
        assert!(activated.contains("c"));
    }

    #[test]
    fn test_find_communities() {
        let mut engine = DreamEngine::new();

        // Create two separate communities
        engine.connections.insert(
            "a".to_string(),
            vec![Connection {
                from_id: "a".to_string(),
                to_id: "b".to_string(),
                weight: 0.8,
                connection_type: ConnectionType::Semantic,
                created_at: 0,
                last_activated: 0,
            }],
        );
        engine.connections.insert(
            "b".to_string(),
            vec![Connection {
                from_id: "b".to_string(),
                to_id: "a".to_string(),
                weight: 0.8,
                connection_type: ConnectionType::Semantic,
                created_at: 0,
                last_activated: 0,
            }],
        );
        engine.connections.insert(
            "x".to_string(),
            vec![Connection {
                from_id: "x".to_string(),
                to_id: "y".to_string(),
                weight: 0.7,
                connection_type: ConnectionType::Semantic,
                created_at: 0,
                last_activated: 0,
            }],
        );

        let communities = engine.find_communities();
        assert_eq!(communities.len(), 2);
    }

    fn make_store_with_records(n: usize) -> VectorStore {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().to_str().unwrap();
        let mut store = VectorStore::new(db_path);
        for i in 0..n {
            store
                .upsert(ChunkRecord {
                    id: format!("chunk_{}", i),
                    content_hash: i as u64,
                    chunk_text: format!("Memory content number {}", i),
                    embedding: vec![0.0; 384],
                    source_file: format!("file_{}.md", i % 3),
                    heading_context: format!("Heading {}", i),
                    created_at: i as i64,
                    updated_at: i as i64,
                })
                .unwrap();
        }
        store
    }

    #[test]
    fn test_sample_memories_returns_real_records() {
        let store = make_store_with_records(10);
        let engine = DreamEngine::new();
        let sampled = engine.sample_memories(&store, 6);
        assert_eq!(sampled.len(), 6);
        for chunk in &sampled {
            assert!(chunk.id.starts_with("chunk_"));
            assert!(chunk.chunk_text.starts_with("Memory content"));
        }
    }

    #[test]
    fn test_sample_memories_three_slice_ratio() {
        let store = make_store_with_records(20);
        let engine = DreamEngine::new();
        let sampled = engine.sample_memories(&store, 10);
        assert_eq!(sampled.len(), 10);
        // 50% recent: first 5 should be highest updated_at (chunk_15..chunk_19)
        let recent_ids: Vec<&str> = sampled.iter().take(5).map(|c| c.id.as_str()).collect();
        for id in &recent_ids {
            let num: usize = id.strip_prefix("chunk_").unwrap().parse().unwrap();
            assert!(
                num >= 15,
                "recent slice should have high indices, got {}",
                num
            );
        }
        // 20% low-salience: last 2 should be lowest connection count (all 0)
        let low_ids: Vec<&str> = sampled.iter().skip(8).map(|c| c.id.as_str()).collect();
        assert_eq!(low_ids.len(), 2);
    }

    #[test]
    fn test_sample_memories_empty_store() {
        let store = make_store_with_records(0);
        let engine = DreamEngine::new();
        let sampled = engine.sample_memories(&store, 10);
        assert!(sampled.is_empty());
    }

    #[test]
    fn test_find_isolated_memories_filters_by_connection_count() {
        let store = make_store_with_records(5);
        let mut engine = DreamEngine::new();
        // Give chunk_0 and chunk_1 many connections
        engine.connections.insert(
            "chunk_0".to_string(),
            vec![
                Connection {
                    from_id: "chunk_0".to_string(),
                    to_id: "chunk_1".to_string(),
                    weight: 0.8,
                    connection_type: ConnectionType::Semantic,
                    created_at: 0,
                    last_activated: 0,
                },
                Connection {
                    from_id: "chunk_0".to_string(),
                    to_id: "chunk_2".to_string(),
                    weight: 0.7,
                    connection_type: ConnectionType::Semantic,
                    created_at: 0,
                    last_activated: 0,
                },
            ],
        );
        engine.connections.insert(
            "chunk_1".to_string(),
            vec![
                Connection {
                    from_id: "chunk_1".to_string(),
                    to_id: "chunk_0".to_string(),
                    weight: 0.8,
                    connection_type: ConnectionType::Semantic,
                    created_at: 0,
                    last_activated: 0,
                },
                Connection {
                    from_id: "chunk_1".to_string(),
                    to_id: "chunk_2".to_string(),
                    weight: 0.6,
                    connection_type: ConnectionType::Semantic,
                    created_at: 0,
                    last_activated: 0,
                },
            ],
        );
        let isolated = engine.find_isolated_memories(&store, 10);
        // chunk_0 (2 connections), chunk_1 (2 connections) should NOT be in isolated
        // chunk_2, chunk_3, chunk_4 (0 connections each) should be isolated
        assert_eq!(isolated.len(), 3);
        for chunk in &isolated {
            assert!(chunk.id == "chunk_2" || chunk.id == "chunk_3" || chunk.id == "chunk_4");
        }
    }

    #[test]
    fn test_find_isolated_memories_returns_real_records() {
        let store = make_store_with_records(3);
        let engine = DreamEngine::new();
        let isolated = engine.find_isolated_memories(&store, 10);
        assert_eq!(isolated.len(), 3);
        for chunk in &isolated {
            assert!(chunk.id.starts_with("chunk_"));
            assert!(chunk.chunk_text.starts_with("Memory content"));
        }
    }

    #[test]
    fn test_generate_community_summary_uses_real_text() {
        let store = make_store_with_records(5);
        let engine = DreamEngine::new();
        let community = vec![
            "chunk_0".to_string(),
            "chunk_1".to_string(),
            "chunk_2".to_string(),
        ];
        let summary = engine.generate_community_summary(&store, &community);
        assert!(summary.is_some());
        let summary = summary.unwrap();
        assert!(summary.text.contains("Memory content number 0"));
        assert!(summary.text.contains("Memory content number 1"));
        assert!(summary.text.contains("Memory content number 2"));
        assert!(!summary.text.contains("Memory about"));
    }

    #[test]
    fn test_generate_community_summary_limits_to_three() {
        let store = make_store_with_records(10);
        let engine = DreamEngine::new();
        let community: Vec<String> = (0..8).map(|i| format!("chunk_{}", i)).collect();
        let summary = engine.generate_community_summary(&store, &community);
        assert!(summary.is_some());
        let summary = summary.unwrap();
        assert!(summary.text.contains("Memory content number 0"));
        assert!(summary.text.contains("Memory content number 2"));
        assert!(!summary.text.contains("Memory content number 3"));
    }

    #[test]
    fn test_generate_community_summary_empty_community() {
        let store = make_store_with_records(5);
        let engine = DreamEngine::new();
        let summary = engine.generate_community_summary(&store, &[]);
        assert!(summary.is_none());
    }

    #[test]
    fn test_audit_snapshot_reports_coverage_salience_and_contradiction() {
        let mut engine = DreamEngine::new();
        engine.activation_scores.insert("chunk_a".to_string(), 0.1);
        engine.activation_scores.insert("chunk_b".to_string(), 0.9);
        engine.activation_scores.insert("chunk_c".to_string(), 0.4);
        engine.insights.push(DreamInsight {
            id: "insight_1".to_string(),
            insight_type: InsightType::Community,
            title: "Contradiction cluster".to_string(),
            summary: "This community contains an internal contradiction.".to_string(),
            related_chunks: vec!["chunk_a".to_string(), "chunk_b".to_string()],
            confidence: 0.92,
            created_at: 1,
        });

        let snapshot = engine.audit_snapshot();

        assert!(snapshot.community_coverage > 0.66);
        assert!(snapshot.salience_shift > 0.7);
        assert!(snapshot.contradiction_risk >= 0.92);
    }
}
