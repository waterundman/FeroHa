// Sync Engine — Coordinate FileWatcher events with VectorStore + Embedding updates

use crate::ai::chunker;
use crate::ai::embedding::EmbeddingPipeline;
use crate::ai::vectordb::{ChunkRecord, VectorStore};
use crate::fs::watcher::FileEvent;

pub struct SyncEngine {
    store: VectorStore,
    embedder: EmbeddingPipeline,
}

impl SyncEngine {
    pub fn new(store: VectorStore, embedder: EmbeddingPipeline) -> Self {
        SyncEngine { store, embedder }
    }

    /// Process a file system event: chunk + embed + update vector index
    pub async fn process_event(
        &mut self,
        event: &FileEvent,
        vault_root: &str,
    ) -> Result<usize, String> {
        self.process_event_with_embeddings(event, vault_root, true)
            .await
    }

    pub fn process_event_sync(
        &mut self,
        event: &FileEvent,
        vault_root: &str,
    ) -> Result<usize, String> {
        let full_path = std::path::Path::new(vault_root).join(&event.path);
        let source_file = event.path.clone();

        match event.kind {
            crate::fs::watcher::FileEventKind::Created
            | crate::fs::watcher::FileEventKind::Modified => {
                let content = std::fs::read_to_string(&full_path)
                    .map_err(|e| format!("Failed to read {}: {}", source_file, e))?;

                let _ = self.store.delete_by_file(&source_file);

                let chunks = chunker::chunk_markdown(&content, &source_file);
                if chunks.is_empty() {
                    return Ok(0);
                }

                let texts: Vec<String> = chunks.iter().map(|c| c.text.clone()).collect();
                let embeddings = self.embedder.embed_sync(&texts);
                self.store_chunks(&source_file, &chunks, &embeddings)
            }
            crate::fs::watcher::FileEventKind::Deleted => {
                let count = self.store.chunks_for_file(&source_file).len();
                self.store
                    .delete_by_file(&source_file)
                    .map_err(|e| format!("Failed to delete chunks: {}", e))?;

                tracing::info!("Deleted chunks for: {} ({} chunks)", source_file, count);
                Ok(count)
            }
            crate::fs::watcher::FileEventKind::Renamed { ref from } => {
                self.store
                    .delete_by_file(from)
                    .map_err(|e| format!("Failed to delete old chunks: {}", e))?;
                Ok(0)
            }
        }
    }

    async fn process_event_with_embeddings(
        &mut self,
        event: &FileEvent,
        vault_root: &str,
        allow_remote_embeddings: bool,
    ) -> Result<usize, String> {
        let full_path = std::path::Path::new(vault_root).join(&event.path);
        let source_file = event.path.clone();

        match event.kind {
            crate::fs::watcher::FileEventKind::Created
            | crate::fs::watcher::FileEventKind::Modified => {
                let content = std::fs::read_to_string(&full_path)
                    .map_err(|e| format!("Failed to read {}: {}", source_file, e))?;

                // Remove old chunks for this file
                let _ = self.store.delete_by_file(&source_file);

                // Chunk the new content
                let chunks = chunker::chunk_markdown(&content, &source_file);
                if chunks.is_empty() {
                    return Ok(0);
                }

                let texts: Vec<String> = chunks.iter().map(|c| c.text.clone()).collect();
                let embeddings = if allow_remote_embeddings {
                    self.embedder.embed(&texts).await
                } else {
                    self.embedder.embed_sync(&texts)
                };
                self.store_chunks(&source_file, &chunks, &embeddings)
            }
            crate::fs::watcher::FileEventKind::Deleted => {
                let count = self.store.chunks_for_file(&source_file).len();
                self.store
                    .delete_by_file(&source_file)
                    .map_err(|e| format!("Failed to delete chunks: {}", e))?;

                tracing::info!("Deleted chunks for: {} ({} chunks)", source_file, count);
                Ok(count)
            }
            crate::fs::watcher::FileEventKind::Renamed { ref from } => {
                // Remove old, will be re-created by the new path event
                self.store
                    .delete_by_file(from)
                    .map_err(|e| format!("Failed to delete old chunks: {}", e))?;
                Ok(0)
            }
        }
    }

    /// Get a reference to the store (for search)
    pub fn store(&self) -> &VectorStore {
        &self.store
    }

    pub fn store_mut(&mut self) -> &mut VectorStore {
        &mut self.store
    }

    pub fn set_embedder(&mut self, embedder: EmbeddingPipeline) {
        self.store.set_dimension(embedder.dimension());
        self.embedder = embedder;
    }

    fn store_chunks(
        &mut self,
        source_file: &str,
        chunks: &[chunker::Chunk],
        embeddings: &[Vec<f32>],
    ) -> Result<usize, String> {
        let now = chrono::Utc::now().timestamp_millis();
        let records: Vec<ChunkRecord> = chunks
            .iter()
            .zip(embeddings.iter())
            .map(|(chunk, emb)| ChunkRecord {
                id: chunk.id.clone(),
                content_hash: hash_u64(&chunk.content_hash),
                chunk_text: chunk.text.clone(),
                embedding: emb.clone(),
                source_file: source_file.to_string(),
                heading_context: chunk.heading_context.clone(),
                created_at: now,
                updated_at: now,
            })
            .collect();

        let count = records.len();
        self.store
            .upsert_batch(records)
            .map_err(|e| format!("Failed to store chunks: {}", e))?;

        tracing::info!("Synced {}: {} chunks", source_file, count);
        Ok(count)
    }
}

fn hash_u64(s: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}
