// VectorStore — Persisted vector store with real cosine similarity search
// Uses JSON file for persistence, in-memory for queries

use serde::{Serialize, Deserialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub chunk_id: String,
    pub chunk_text: String,
    pub source_file: String,
    pub heading_context: String,
    pub score: f32,
    pub similarity: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkRecord {
    pub id: String,
    pub content_hash: u64,
    pub chunk_text: String,
    pub embedding: Vec<f32>,
    pub source_file: String,
    pub heading_context: String,
    pub created_at: i64,
    pub updated_at: i64,
}

/// VectorStore backed by JSON file persistence + in-memory cosine similarity search
pub struct VectorStore {
    records: Vec<ChunkRecord>,
    db_path: PathBuf,
    dimension: usize,
}

impl VectorStore {
    pub fn new(db_path: &str) -> Self {
        VectorStore {
            records: Vec::new(),
            db_path: PathBuf::from(db_path),
            dimension: 384,
        }
    }

    /// Open or create the vector database, loading persisted data
    pub fn open(db_path: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let path = PathBuf::from(db_path);
        let mut store = VectorStore {
            records: Vec::new(),
            db_path: path,
            dimension: 384,
        };
        store.load()?;
        Ok(store)
    }

    pub fn set_dimension(&mut self, dim: usize) {
        self.dimension = dim;
    }

    fn store_file(&self) -> PathBuf {
        self.db_path.join("vectors.json")
    }

    fn load(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let file = self.store_file();
        if file.exists() {
            let data = fs::read_to_string(&file)?;
            if !data.trim().is_empty() {
                self.records = serde_json::from_str(&data)?;
            }
        }
        Ok(())
    }

    fn save(&self) -> Result<(), std::io::Error> {
        let file = self.store_file();
        if let Some(parent) = file.parent() {
            fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_string_pretty(&self.records)?;
        fs::write(&file, data)
    }

    /// Insert or update a chunk record, then persist
    pub fn upsert(&mut self, record: ChunkRecord) -> Result<(), std::io::Error> {
        if let Some(existing) = self.records.iter_mut().find(|r| r.id == record.id) {
            *existing = record;
        } else {
            self.records.push(record);
        }
        self.save()
    }

    /// Batch upsert — much faster for bulk operations
    pub fn upsert_batch(&mut self, new_records: Vec<ChunkRecord>) -> Result<(), std::io::Error> {
        for rec in new_records {
            if let Some(existing) = self.records.iter_mut().find(|r| r.id == rec.id) {
                *existing = rec;
            } else {
                self.records.push(rec);
            }
        }
        self.save()
    }

    /// Search by embedding vector using real cosine similarity
    pub fn search_by_embedding(&self, query_embedding: &[f32], top_k: usize) -> Vec<SearchResult> {
        if self.records.is_empty() {
            return vec![];
        }

        let mut scored: Vec<(f32, &ChunkRecord)> = self
            .records
            .iter()
            .map(|r| {
                let sim = cosine_similarity(query_embedding, &r.embedding);
                (sim, r)
            })
            .collect();

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        scored
            .into_iter()
            .take(top_k)
            .map(|(sim, r)| SearchResult {
                chunk_id: r.id.clone(),
                chunk_text: r.chunk_text.clone(),
                source_file: r.source_file.clone(),
                heading_context: r.heading_context.clone(),
                score: sim,
                similarity: sim,
            })
            .collect()
    }

    /// Full-text search (keyword fallback)
    pub fn search_text(&self, query: &str, top_k: usize) -> Vec<SearchResult> {
        let query_lower = query.to_lowercase();
        let words: Vec<&str> = query_lower.split_whitespace().collect();

        let mut scored: Vec<(f32, &ChunkRecord)> = self
            .records
            .iter()
            .map(|r| {
                let text_lower = r.chunk_text.to_lowercase();
                let score = if words.iter().all(|w| text_lower.contains(w)) {
                    let matches: usize = words.iter().filter(|w| text_lower.contains(*w)).count();
                    matches as f32 / words.len() as f32
                } else {
                    words.iter()
                        .filter(|w| text_lower.contains(*w))
                        .count() as f32
                        / words.len().max(1) as f32
                        * 0.5
                };
                (score, r)
            })
            .filter(|(s, _)| *s > 0.0)
            .collect();

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        scored
            .into_iter()
            .take(top_k)
            .map(|(score, r)| SearchResult {
                chunk_id: r.id.clone(),
                chunk_text: r.chunk_text.clone(),
                source_file: r.source_file.clone(),
                heading_context: r.heading_context.clone(),
                score,
                similarity: score,
            })
            .collect()
    }

    /// Delete all records for a given source file
    pub fn delete_by_file(&mut self, source_file: &str) -> Result<(), std::io::Error> {
        self.records.retain(|r| r.source_file != source_file);
        self.save()
    }

    /// Delete records by content hash
    pub fn delete_by_hash(&mut self, content_hash: u64) -> Result<(), std::io::Error> {
        self.records.retain(|r| r.content_hash != content_hash);
        self.save()
    }

    /// Get total chunk count
    pub fn count(&self) -> usize {
        self.records.len()
    }

    /// Get all records (read-only slice)
    pub fn records(&self) -> &[ChunkRecord] {
        &self.records
    }

    /// Get a single record by id
    pub fn get_record(&self, id: &str) -> Option<&ChunkRecord> {
        self.records.iter().find(|r| r.id == id)
    }

    /// Get all chunk texts for a source file (for rebuilding)
    pub fn chunks_for_file(&self, source_file: &str) -> Vec<&ChunkRecord> {
        self.records.iter().filter(|r| r.source_file == source_file).collect()
    }
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len().min(b.len());
    if n == 0 {
        return 0.0;
    }
    let dot: f32 = a[..n].iter().zip(&b[..n]).map(|(x, y)| x * y).sum();
    let na: f32 = a[..n].iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b[..n].iter().map(|x| x * x).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        dot / (na * nb)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_upsert_search_text() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().to_str().unwrap();
        let mut store = VectorStore::new(db_path);

        store.upsert(ChunkRecord {
            id: "c1".to_string(),
            content_hash: 1,
            chunk_text: "Rust is a systems programming language".to_string(),
            embedding: vec![0.0; 384],
            source_file: "rust.md".to_string(),
            heading_context: "Intro".to_string(),
            created_at: 0,
            updated_at: 0,
        }).unwrap();

        let results = store.search_text("Rust programming", 5);
        assert!(!results.is_empty());
        assert_eq!(results[0].source_file, "rust.md");
    }

    #[test]
    fn test_cosine_search() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().to_str().unwrap();
        let mut store = VectorStore::new(db_path);

        // Two similar embeddings, one different
        store.upsert(ChunkRecord {
            id: "c1".to_string(),
            content_hash: 1,
            chunk_text: "rust lang".to_string(),
            embedding: vec![1.0, 0.0, 0.0],
            source_file: "a.md".to_string(),
            heading_context: "".to_string(),
            created_at: 0,
            updated_at: 0,
        }).unwrap();
        store.upsert(ChunkRecord {
            id: "c2".to_string(),
            content_hash: 2,
            chunk_text: "python lang".to_string(),
            embedding: vec![0.0, 1.0, 0.0],
            source_file: "b.md".to_string(),
            heading_context: "".to_string(),
            created_at: 0,
            updated_at: 0,
        }).unwrap();

        let results = store.search_by_embedding(&[1.0, 0.0, 0.0], 2);
        assert!(results[0].similarity > results[1].similarity,
            "c1 should be more similar to query than c2");
    }

    #[test]
    fn test_persistence() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().to_str().unwrap();

        {
            let mut store = VectorStore::new(db_path);
            store.upsert(ChunkRecord {
                id: "c1".to_string(),
                content_hash: 1,
                chunk_text: "test".to_string(),
                embedding: vec![0.5; 10],
                source_file: "test.md".to_string(),
                heading_context: "".to_string(),
                created_at: 0,
                updated_at: 0,
            }).unwrap();
        }

        // Re-open and verify persistence
        let store = VectorStore::open(db_path).unwrap();
        assert_eq!(store.count(), 1);
    }
}
