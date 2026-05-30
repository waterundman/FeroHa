// VectorStore — SQLite-backed vector store with cosine similarity search
// Uses rusqlite for persistence, in-memory for queries
// Migration: auto-detects old vectors.json and migrates on first open

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunityRecord {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub member_count: usize,
    pub members: Vec<String>,
    pub sample_texts: Vec<String>,
    pub confidence: f32,
    pub created_at: i64,
    pub cycle_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct VectorDbStats {
    pub total_chunks: usize,
    pub total_files: usize,
    pub db_size_bytes: u64,
    pub dimension: usize,
    pub index_status: String,
}

/// VectorStore backed by SQLite persistence + in-memory cosine similarity search
pub struct SqliteVectorStore {
    conn: Connection,
    db_path: PathBuf,
    dimension: usize,
}

/// Backward-compatible type alias
pub type VectorStore = SqliteVectorStore;

fn embedding_to_blob(embedding: &[f32]) -> Vec<u8> {
    embedding.iter().flat_map(|f| f.to_ne_bytes()).collect()
}

fn blob_to_embedding(data: &[u8]) -> Vec<f32> {
    data.chunks_exact(4)
        .map(|chunk| f32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

fn create_tables(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS chunks (
            id TEXT PRIMARY KEY,
            content_hash INTEGER NOT NULL,
            chunk_text TEXT NOT NULL,
            embedding BLOB NOT NULL,
            source_file TEXT NOT NULL,
            heading_context TEXT NOT NULL DEFAULT '',
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_chunks_source_file ON chunks(source_file);
        CREATE INDEX IF NOT EXISTS idx_chunks_content_hash ON chunks(content_hash);
        CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(chunk_id, title, content, tokenize='porter');
        CREATE TABLE IF NOT EXISTS vectordb_meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS communities (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL DEFAULT '',
            summary TEXT NOT NULL DEFAULT '',
            member_count INTEGER NOT NULL DEFAULT 0,
            members TEXT NOT NULL DEFAULT '[]',
            sample_texts TEXT NOT NULL DEFAULT '[]',
            confidence REAL NOT NULL DEFAULT 0.0,
            created_at INTEGER NOT NULL,
            cycle_id TEXT NOT NULL DEFAULT ''
        );
        CREATE INDEX IF NOT EXISTS idx_communities_cycle ON communities(cycle_id);",
    )
}

fn sanitize_fts_query(query: &str) -> String {
    query
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c.is_whitespace() {
                c
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

impl SqliteVectorStore {
    pub fn new(db_path: &str) -> Self {
        if db_path == ":memory:" {
            let conn =
                Connection::open_in_memory().expect("Failed to open in-memory SQLite database");
            create_tables(&conn).expect("Failed to create tables");
            return SqliteVectorStore {
                conn,
                db_path: PathBuf::from(db_path),
                dimension: 384,
            };
        }

        let path = PathBuf::from(db_path);
        fs::create_dir_all(&path).ok();
        let db_file = path.join("vectors.db");
        let conn = Connection::open(&db_file).expect("Failed to open SQLite database");
        create_tables(&conn).expect("Failed to create tables");
        SqliteVectorStore {
            conn,
            db_path: path,
            dimension: 384,
        }
    }

    /// Open or create the vector database, loading persisted data
    pub fn open(db_path: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        if db_path == ":memory:" {
            let conn = Connection::open_in_memory()?;
            create_tables(&conn)?;
            return Ok(SqliteVectorStore {
                conn,
                db_path: PathBuf::from(db_path),
                dimension: 384,
            });
        }

        let path = PathBuf::from(db_path);
        fs::create_dir_all(&path)?;
        let db_file = path.join("vectors.db");
        let conn = Connection::open(&db_file)?;
        create_tables(&conn)?;

        // Backfill FTS5 index for existing data (one-time migration)
        let fts_count: i64 = conn.query_row("SELECT COUNT(*) FROM chunks_fts", [], |r| r.get(0))?;
        if fts_count == 0 {
            let chunk_count: i64 =
                conn.query_row("SELECT COUNT(*) FROM chunks", [], |r| r.get(0))?;
            if chunk_count > 0 {
                conn.execute(
                    "INSERT INTO chunks_fts(chunk_id, title, content) SELECT id, heading_context, chunk_text FROM chunks",
                    [],
                )?;
            }
        }

        let mut store = SqliteVectorStore {
            conn,
            db_path: path,
            dimension: 384,
        };

        // Check for JSON migration
        store.migrate_from_json()?;

        Ok(store)
    }

    pub fn set_dimension(&mut self, dim: usize) {
        self.dimension = dim;
    }

    fn store_file(&self) -> PathBuf {
        self.db_path.join("vectors.db")
    }

    fn json_file(&self) -> PathBuf {
        self.db_path.join("vectors.json")
    }

    /// Migrate from JSON file if it exists. Returns number of records migrated.
    pub fn migrate_from_json(&mut self) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        let json_path = self.json_file();
        if !json_path.exists() {
            return Ok(0);
        }

        let data = fs::read_to_string(&json_path)?;
        if data.trim().is_empty() {
            fs::remove_file(&json_path)?;
            return Ok(0);
        }

        let records: Vec<ChunkRecord> = serde_json::from_str(&data)?;
        if records.is_empty() {
            fs::remove_file(&json_path)?;
            return Ok(0);
        }

        let count = records.len();
        let tx = self.conn.transaction()?;
        {
            let mut chunk_stmt = tx.prepare(
                "INSERT OR REPLACE INTO chunks (id, content_hash, chunk_text, embedding, source_file, heading_context, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"
            )?;
            let mut fts_stmt = tx.prepare(
                "INSERT OR REPLACE INTO chunks_fts(chunk_id, title, content) VALUES (?1, ?2, ?3)",
            )?;
            for rec in &records {
                let blob = embedding_to_blob(&rec.embedding);
                chunk_stmt.execute(params![
                    rec.id,
                    rec.content_hash as i64,
                    rec.chunk_text,
                    blob,
                    rec.source_file,
                    rec.heading_context,
                    rec.created_at,
                    rec.updated_at
                ])?;
                fts_stmt.execute(params![rec.id, rec.heading_context, rec.chunk_text])?;
            }
        }
        tx.commit()?;

        // Delete JSON file only after successful commit
        fs::remove_file(&json_path)?;
        tracing::info!("Migrated {} records from vectors.json to SQLite", count);

        Ok(count)
    }

    /// Insert or update a chunk record, then persist
    pub fn upsert(&mut self, record: ChunkRecord) -> Result<(), std::io::Error> {
        let blob = embedding_to_blob(&record.embedding);
        let tx = self
            .conn
            .transaction()
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        tx.execute(
            "INSERT OR REPLACE INTO chunks (id, content_hash, chunk_text, embedding, source_file, heading_context, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                record.id, record.content_hash as i64, record.chunk_text, blob,
                record.source_file, record.heading_context, record.created_at, record.updated_at
            ],
        ).map_err(|e| std::io::Error::other(e.to_string()))?;
        tx.execute(
            "INSERT OR REPLACE INTO chunks_fts(chunk_id, title, content) VALUES (?1, ?2, ?3)",
            params![record.id, record.heading_context, record.chunk_text],
        )
        .map_err(|e| std::io::Error::other(e.to_string()))?;
        tx.commit()
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        Ok(())
    }

    /// Batch upsert — much faster for bulk operations
    pub fn upsert_batch(&mut self, new_records: Vec<ChunkRecord>) -> Result<(), std::io::Error> {
        let tx = self
            .conn
            .transaction()
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        {
            let mut chunk_stmt = tx.prepare(
                "INSERT OR REPLACE INTO chunks (id, content_hash, chunk_text, embedding, source_file, heading_context, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"
            ).map_err(|e| std::io::Error::other(e.to_string()))?;
            let mut fts_stmt = tx.prepare(
                "INSERT OR REPLACE INTO chunks_fts(chunk_id, title, content) VALUES (?1, ?2, ?3)"
            ).map_err(|e| std::io::Error::other(e.to_string()))?;
            for rec in &new_records {
                let blob = embedding_to_blob(&rec.embedding);
                chunk_stmt
                    .execute(params![
                        rec.id,
                        rec.content_hash as i64,
                        rec.chunk_text,
                        blob,
                        rec.source_file,
                        rec.heading_context,
                        rec.created_at,
                        rec.updated_at
                    ])
                    .map_err(|e| std::io::Error::other(e.to_string()))?;
                fts_stmt
                    .execute(params![rec.id, rec.heading_context, rec.chunk_text])
                    .map_err(|e| std::io::Error::other(e.to_string()))?;
            }
        }
        tx.commit()
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        Ok(())
    }

    fn load_all_records(&self) -> Vec<ChunkRecord> {
        let mut records = Vec::new();
        if let Ok(mut stmt) = self.conn.prepare(
            "SELECT id, content_hash, chunk_text, embedding, source_file, heading_context, created_at, updated_at FROM chunks"
        ) {
            let rows = stmt.query_map([], |row| {
                let id: String = row.get(0)?;
                let content_hash: i64 = row.get(1)?;
                let chunk_text: String = row.get(2)?;
                let embedding_blob: Vec<u8> = row.get(3)?;
                let source_file: String = row.get(4)?;
                let heading_context: String = row.get(5)?;
                let created_at: i64 = row.get(6)?;
                let updated_at: i64 = row.get(7)?;
                Ok(ChunkRecord {
                    id, content_hash: content_hash as u64, chunk_text,
                    embedding: blob_to_embedding(&embedding_blob),
                    source_file, heading_context, created_at, updated_at,
                })
            });
            if let Ok(rows) = rows {
                records = rows.filter_map(|r| r.ok()).collect();
            }
        }
        records
    }

    /// Search by embedding vector using real cosine similarity
    pub fn search_by_embedding(&self, query_embedding: &[f32], top_k: usize) -> Vec<SearchResult> {
        let records = self.load_all_records();
        if records.is_empty() {
            return vec![];
        }

        let mut scored: Vec<(f32, &ChunkRecord)> = records
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

    /// Full-text search via FTS5 with BM25 relevance ranking
    pub fn search_text(&self, query: &str, top_k: usize) -> Vec<SearchResult> {
        let sanitized = sanitize_fts_query(query);
        if sanitized.is_empty() {
            return vec![];
        }

        let mut results = Vec::new();
        let sql = "SELECT c.id, c.chunk_text, c.source_file, c.heading_context, bm25(chunks_fts) as relevance_score
                   FROM chunks_fts fts
                   JOIN chunks c ON c.id = fts.chunk_id
                   WHERE chunks_fts MATCH ?
                   ORDER BY relevance_score DESC
                   LIMIT ?";

        if let Ok(mut stmt) = self.conn.prepare(sql) {
            if let Ok(rows) = stmt.query_map(params![sanitized, top_k as i64], |row| {
                let score: f64 = row.get(4)?;
                let s = score.max(0.0) as f32;
                let normalized = s / (s + 1.0);
                Ok(SearchResult {
                    chunk_id: row.get(0)?,
                    chunk_text: row.get(1)?,
                    source_file: row.get(2)?,
                    heading_context: row.get(3)?,
                    score: normalized,
                    similarity: normalized,
                })
            }) {
                results = rows.filter_map(|r| r.ok()).collect();
            }
        }

        results
    }

    /// Delete all records for a given source file
    pub fn delete_by_file(&mut self, source_file: &str) -> Result<(), std::io::Error> {
        let tx = self
            .conn
            .transaction()
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        // Collect chunk IDs to clean FTS5
        let mut id_stmt = tx
            .prepare("SELECT id FROM chunks WHERE source_file = ?1")
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        let ids: Vec<String> = id_stmt
            .query_map(params![source_file], |row| row.get(0))
            .map_err(|e| std::io::Error::other(e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();
        drop(id_stmt);
        for id in &ids {
            tx.execute("DELETE FROM chunks_fts WHERE chunk_id = ?1", params![id])
                .map_err(|e| std::io::Error::other(e.to_string()))?;
        }
        tx.execute(
            "DELETE FROM chunks WHERE source_file = ?1",
            params![source_file],
        )
        .map_err(|e| std::io::Error::other(e.to_string()))?;
        tx.commit()
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        Ok(())
    }

    /// Get chunks for a specific source file (for GraphManifest description priority)
    pub fn get_chunks_for_file(&self, source_file: &str) -> Vec<SearchResult> {
        let mut results = Vec::new();
        if let Ok(mut stmt) = self.conn.prepare(
            "SELECT id, chunk_text, source_file, heading_context FROM chunks WHERE source_file = ?1 LIMIT 3"
        ) {
            if let Ok(rows) = stmt.query_map(params![source_file], |row| {
                Ok(SearchResult {
                    chunk_id: row.get(0)?,
                    chunk_text: row.get(1)?,
                    source_file: row.get(2)?,
                    heading_context: row.get(3)?,
                    score: 1.0,
                    similarity: 1.0,
                })
            }) {
                results = rows.filter_map(|r| r.ok()).collect();
            }
        }
        results
    }

    /// Delete records by content hash
    pub fn delete_by_hash(&mut self, content_hash: u64) -> Result<(), std::io::Error> {
        let tx = self
            .conn
            .transaction()
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        let mut id_stmt = tx
            .prepare("SELECT id FROM chunks WHERE content_hash = ?1")
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        let ids: Vec<String> = id_stmt
            .query_map(params![content_hash as i64], |row| row.get(0))
            .map_err(|e| std::io::Error::other(e.to_string()))?
            .filter_map(|r| r.ok())
            .collect();
        drop(id_stmt);
        for id in &ids {
            tx.execute("DELETE FROM chunks_fts WHERE chunk_id = ?1", params![id])
                .map_err(|e| std::io::Error::other(e.to_string()))?;
        }
        tx.execute(
            "DELETE FROM chunks WHERE content_hash = ?1",
            params![content_hash as i64],
        )
        .map_err(|e| std::io::Error::other(e.to_string()))?;
        tx.commit()
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        Ok(())
    }

    /// Get total chunk count
    pub fn count(&self) -> usize {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM chunks", [], |row| row.get(0))
            .unwrap_or(0);
        count as usize
    }

    /// Get all records (read-only vec)
    pub fn records(&self) -> Vec<ChunkRecord> {
        self.load_all_records()
    }

    /// Get a single record by id
    pub fn get_record(&self, id: &str) -> Option<ChunkRecord> {
        let mut stmt = self.conn.prepare(
            "SELECT id, content_hash, chunk_text, embedding, source_file, heading_context, created_at, updated_at FROM chunks WHERE id = ?1"
        ).ok()?;
        stmt.query_row(params![id], |row| {
            let id: String = row.get(0)?;
            let content_hash: i64 = row.get(1)?;
            let chunk_text: String = row.get(2)?;
            let embedding_blob: Vec<u8> = row.get(3)?;
            let source_file: String = row.get(4)?;
            let heading_context: String = row.get(5)?;
            let created_at: i64 = row.get(6)?;
            let updated_at: i64 = row.get(7)?;
            Ok(ChunkRecord {
                id,
                content_hash: content_hash as u64,
                chunk_text,
                embedding: blob_to_embedding(&embedding_blob),
                source_file,
                heading_context,
                created_at,
                updated_at,
            })
        })
        .optional()
        .ok()
        .flatten()
    }

    /// Get all chunk texts for a source file (for rebuilding)
    pub fn chunks_for_file(&self, source_file: &str) -> Vec<ChunkRecord> {
        let mut records = Vec::new();
        if let Ok(mut stmt) = self.conn.prepare(
            "SELECT id, content_hash, chunk_text, embedding, source_file, heading_context, created_at, updated_at FROM chunks WHERE source_file = ?1"
        ) {
            let rows = stmt.query_map(params![source_file], |row| {
                let id: String = row.get(0)?;
                let content_hash: i64 = row.get(1)?;
                let chunk_text: String = row.get(2)?;
                let embedding_blob: Vec<u8> = row.get(3)?;
                let source_file: String = row.get(4)?;
                let heading_context: String = row.get(5)?;
                let created_at: i64 = row.get(6)?;
                let updated_at: i64 = row.get(7)?;
                Ok(ChunkRecord {
                    id, content_hash: content_hash as u64, chunk_text,
                    embedding: blob_to_embedding(&embedding_blob),
                    source_file, heading_context, created_at, updated_at,
                })
            });
            if let Ok(rows) = rows {
                records = rows.filter_map(|r| r.ok()).collect();
            }
        }
        records
    }

    pub fn save_communities(&self, communities: &[CommunityRecord]) -> Result<(), std::io::Error> {
        if communities.is_empty() {
            return Ok(());
        }
        let mut stmt = self.conn.prepare(
            "INSERT OR REPLACE INTO communities (id, title, summary, member_count, members, sample_texts, confidence, created_at, cycle_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)"
        ).map_err(|e| std::io::Error::other(e.to_string()))?;
        for c in communities {
            let members_json = serde_json::to_string(&c.members).unwrap_or_default();
            let texts_json = serde_json::to_string(&c.sample_texts).unwrap_or_default();
            stmt.execute(params![
                c.id,
                c.title,
                c.summary,
                c.member_count as i64,
                members_json,
                texts_json,
                c.confidence,
                c.created_at,
                c.cycle_id
            ])
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        }
        Ok(())
    }

    pub fn load_communities(&self) -> Vec<CommunityRecord> {
        let mut records = Vec::new();
        if let Ok(mut stmt) = self.conn.prepare(
            "SELECT id, title, summary, member_count, members, sample_texts, confidence, created_at, cycle_id
             FROM communities ORDER BY member_count DESC"
        ) {
            let rows = stmt.query_map([], |row| {
                let id: String = row.get(0)?;
                let title: String = row.get(1)?;
                let summary: String = row.get(2)?;
                let member_count: i64 = row.get(3)?;
                let members_json: String = row.get(4)?;
                let texts_json: String = row.get(5)?;
                let confidence: f64 = row.get(6)?;
                let created_at: i64 = row.get(7)?;
                let cycle_id: String = row.get(8)?;
                let members: Vec<String> = serde_json::from_str(&members_json).unwrap_or_default();
                let sample_texts: Vec<String> = serde_json::from_str(&texts_json).unwrap_or_default();
                Ok(CommunityRecord {
                    id, title, summary, member_count: member_count as usize,
                    members, sample_texts, confidence: confidence as f32,
                    created_at, cycle_id,
                })
            });
            if let Ok(rows) = rows {
                records = rows.filter_map(|r| r.ok()).collect();
            }
        }
        records
    }

    pub fn clear_communities(&self) -> Result<(), std::io::Error> {
        self.conn
            .execute("DELETE FROM communities", [])
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        Ok(())
    }

    pub fn community_count(&self) -> usize {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM communities", [], |row| row.get(0))
            .unwrap_or(0);
        count as usize
    }

    /// Get database statistics
    pub fn get_stats(&self) -> Result<VectorDbStats, Box<dyn std::error::Error + Send + Sync>> {
        let total_chunks: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM chunks", [], |row| row.get(0))?;
        let total_files: i64 = self.conn.query_row(
            "SELECT COUNT(DISTINCT source_file) FROM chunks",
            [],
            |row| row.get(0),
        )?;
        let db_size_bytes = fs::metadata(self.store_file())
            .map(|m| m.len())
            .unwrap_or(0);

        Ok(VectorDbStats {
            total_chunks: total_chunks as usize,
            total_files: total_files as usize,
            db_size_bytes,
            dimension: self.dimension,
            index_status: "exact".to_string(),
        })
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
    fn test_sqlite_upsert_search() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().to_str().unwrap();
        let mut store = SqliteVectorStore::new(db_path);

        store
            .upsert(ChunkRecord {
                id: "c1".to_string(),
                content_hash: 1,
                chunk_text: "Rust is a systems programming language".to_string(),
                embedding: vec![0.0; 384],
                source_file: "rust.md".to_string(),
                heading_context: "Intro".to_string(),
                created_at: 0,
                updated_at: 0,
            })
            .unwrap();

        let results = store.search_text("Rust programming", 5);
        assert!(!results.is_empty());
        assert_eq!(results[0].source_file, "rust.md");
    }

    #[test]
    fn test_in_memory_store() {
        let mut store = SqliteVectorStore::new(":memory:");
        store
            .upsert(ChunkRecord {
                id: "c1".to_string(),
                content_hash: 1,
                chunk_text: "ephemeral chunk".to_string(),
                embedding: vec![0.0; 10],
                source_file: "memory.md".to_string(),
                heading_context: "".to_string(),
                created_at: 0,
                updated_at: 0,
            })
            .unwrap();

        assert_eq!(store.count(), 1);
        assert!(store.get_record("c1").is_some());
    }

    #[test]
    fn test_sqlite_cosine_search() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().to_str().unwrap();
        let mut store = SqliteVectorStore::new(db_path);

        store
            .upsert(ChunkRecord {
                id: "c1".to_string(),
                content_hash: 1,
                chunk_text: "rust lang".to_string(),
                embedding: vec![1.0, 0.0, 0.0],
                source_file: "a.md".to_string(),
                heading_context: "".to_string(),
                created_at: 0,
                updated_at: 0,
            })
            .unwrap();
        store
            .upsert(ChunkRecord {
                id: "c2".to_string(),
                content_hash: 2,
                chunk_text: "python lang".to_string(),
                embedding: vec![0.0, 1.0, 0.0],
                source_file: "b.md".to_string(),
                heading_context: "".to_string(),
                created_at: 0,
                updated_at: 0,
            })
            .unwrap();

        let results = store.search_by_embedding(&[1.0, 0.0, 0.0], 2);
        assert!(
            results[0].similarity > results[1].similarity,
            "c1 should be more similar to query than c2"
        );
    }

    #[test]
    fn test_sqlite_persistence() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().to_str().unwrap();

        {
            let mut store = SqliteVectorStore::new(db_path);
            store
                .upsert(ChunkRecord {
                    id: "c1".to_string(),
                    content_hash: 1,
                    chunk_text: "test".to_string(),
                    embedding: vec![0.5; 10],
                    source_file: "test.md".to_string(),
                    heading_context: "".to_string(),
                    created_at: 0,
                    updated_at: 0,
                })
                .unwrap();
        }

        // Re-open and verify persistence
        let store = SqliteVectorStore::open(db_path).unwrap();
        assert_eq!(store.count(), 1);
    }

    #[test]
    fn test_sqlite_delete_by_file() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().to_str().unwrap();
        let mut store = SqliteVectorStore::new(db_path);

        store
            .upsert(ChunkRecord {
                id: "c1".to_string(),
                content_hash: 1,
                chunk_text: "file a content".to_string(),
                embedding: vec![0.0; 10],
                source_file: "a.md".to_string(),
                heading_context: "".to_string(),
                created_at: 0,
                updated_at: 0,
            })
            .unwrap();
        store
            .upsert(ChunkRecord {
                id: "c2".to_string(),
                content_hash: 2,
                chunk_text: "file b content".to_string(),
                embedding: vec![0.0; 10],
                source_file: "b.md".to_string(),
                heading_context: "".to_string(),
                created_at: 0,
                updated_at: 0,
            })
            .unwrap();

        store.delete_by_file("a.md").unwrap();
        assert_eq!(store.count(), 1);
        assert!(store.get_record("c1").is_none());
        assert!(store.get_record("c2").is_some());
    }

    #[test]
    fn test_sqlite_batch_upsert() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().to_str().unwrap();
        let mut store = SqliteVectorStore::new(db_path);

        let records: Vec<ChunkRecord> = (0..100)
            .map(|i| ChunkRecord {
                id: format!("c{}", i),
                content_hash: i as u64,
                chunk_text: format!("chunk {}", i),
                embedding: vec![0.0; 10],
                source_file: "test.md".to_string(),
                heading_context: "".to_string(),
                created_at: 0,
                updated_at: 0,
            })
            .collect();

        store.upsert_batch(records).unwrap();
        assert_eq!(store.count(), 100);
    }

    #[test]
    fn test_json_migration() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().to_str().unwrap();
        let path = PathBuf::from(db_path);

        // Create vectors.json
        let json_path = path.join("vectors.json");
        let records = vec![ChunkRecord {
            id: "c1".to_string(),
            content_hash: 1,
            chunk_text: "migrated chunk".to_string(),
            embedding: vec![0.5; 10],
            source_file: "migrated.md".to_string(),
            heading_context: "".to_string(),
            created_at: 0,
            updated_at: 0,
        }];
        fs::create_dir_all(&path).unwrap();
        fs::write(&json_path, serde_json::to_string_pretty(&records).unwrap()).unwrap();
        assert!(json_path.exists());

        // Open store — should trigger migration
        let store = SqliteVectorStore::open(db_path).unwrap();
        assert_eq!(store.count(), 1);
        assert!(
            !json_path.exists(),
            "vectors.json should be deleted after migration"
        );

        let record = store.get_record("c1").unwrap();
        assert_eq!(record.chunk_text, "migrated chunk");
    }

    #[test]
    fn test_get_stats() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().to_str().unwrap();
        let mut store = SqliteVectorStore::new(db_path);

        store
            .upsert(ChunkRecord {
                id: "c1".to_string(),
                content_hash: 1,
                chunk_text: "test".to_string(),
                embedding: vec![0.0; 10],
                source_file: "a.md".to_string(),
                heading_context: "".to_string(),
                created_at: 0,
                updated_at: 0,
            })
            .unwrap();
        store
            .upsert(ChunkRecord {
                id: "c2".to_string(),
                content_hash: 2,
                chunk_text: "test2".to_string(),
                embedding: vec![0.0; 10],
                source_file: "b.md".to_string(),
                heading_context: "".to_string(),
                created_at: 0,
                updated_at: 0,
            })
            .unwrap();

        let stats = store.get_stats().unwrap();
        assert_eq!(stats.total_chunks, 2);
        assert_eq!(stats.total_files, 2);
        assert!(stats.db_size_bytes > 0);
        assert_eq!(stats.dimension, 384);
        assert_eq!(stats.index_status, "exact");
    }
}
