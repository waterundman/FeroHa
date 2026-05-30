// SearchEngine — Tantivy-based full-text search with Chinese tokenization (jieba)
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use serde::Serialize;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::*;
use tantivy::{Index, IndexReader, IndexWriter, ReloadPolicy, TantivyDocument};
use tantivy_jieba::JiebaTokenizer;
use tokio::sync::Mutex;

const COMMIT_BATCH_SIZE: usize = 10;

#[derive(Debug, Serialize, Clone)]
pub struct SearchResult {
    pub path: String,
    pub title: String,
    pub snippet: String,
    pub score: f32,
}

pub struct SearchEngine {
    index: Index,
    reader: IndexReader,
    writer: Arc<Mutex<IndexWriter>>,
    schema: Schema,
    vault_path: PathBuf,
    pending_count: AtomicUsize,
}

impl SearchEngine {
    pub fn new(vault_path: &Path) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let index_path = vault_path.join(".dualtrack").join("fts");
        std::fs::create_dir_all(&index_path)?;

        let mut schema_builder = Schema::builder();
        schema_builder.add_text_field("path", STRING | STORED);
        schema_builder.add_text_field("title", TEXT | STORED);
        schema_builder.add_text_field("content", TEXT);
        schema_builder.add_i64_field("modified", INDEXED | STORED);
        let schema = schema_builder.build();

        let index = if index_path.join("meta.json").exists() {
            Index::open_in_dir(&index_path)?
        } else {
            let index = Index::create_in_dir(&index_path, schema.clone())?;
            let tokenizer = JiebaTokenizer {};
            index.tokenizers().register("jieba", tokenizer);
            index
        };

        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()?;

        let writer = index.writer(50_000_000)?;

        Ok(SearchEngine {
            index,
            reader,
            writer: Arc::new(Mutex::new(writer)),
            schema,
            vault_path: vault_path.to_path_buf(),
            pending_count: AtomicUsize::new(0),
        })
    }

    pub fn index_all_md_files(&self) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        let mut count = 0;
        self.walk_and_index(&self.vault_path, &mut count)?;
        if count > 0 {
            self.commit()?;
        }
        Ok(count)
    }

    fn walk_and_index(
        &self,
        dir: &Path,
        count: &mut usize,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if name.starts_with('.') && name != ".dualtrack" {
                    continue;
                }
                if path.is_dir() {
                    if name != ".dualtrack" {
                        self.walk_and_index(&path, count)?;
                    }
                } else if path.extension().map(|e| e == "md").unwrap_or(false) {
                    let rel = path
                        .strip_prefix(&self.vault_path)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .to_string();
                    let content = std::fs::read_to_string(&path).unwrap_or_default();
                    let title = extract_title(&content, &rel);
                    let modified = path
                        .metadata()
                        .ok()
                        .and_then(|m| m.modified().ok())
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs() as i64)
                        .unwrap_or(0);
                    self.add_document_internal(&rel, &title, &content, modified)?;
                    *count += 1;
                }
            }
        }
        Ok(())
    }

    pub fn add_document(
        &self,
        path: &str,
        title: &str,
        content: &str,
        modified: i64,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.add_document_internal(path, title, content, modified)
    }

    fn add_document_internal(
        &self,
        path: &str,
        title: &str,
        content: &str,
        modified: i64,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let path_field = self.schema.get_field("path").unwrap();
        let title_field = self.schema.get_field("title").unwrap();
        let content_field = self.schema.get_field("content").unwrap();
        let modified_field = self.schema.get_field("modified").unwrap();

        let mut doc = TantivyDocument::default();
        doc.add_text(path_field, path);
        doc.add_text(title_field, title);
        doc.add_text(content_field, content);
        doc.add_i64(modified_field, modified);

        let path_term = tantivy::Term::from_field_text(path_field, path);
        let writer = self.writer.blocking_lock();
        writer.delete_term(path_term);
        writer.add_document(doc)?;

        let count = self.pending_count.fetch_add(1, Ordering::Relaxed) + 1;
        if count >= COMMIT_BATCH_SIZE {
            drop(writer);
            self.commit()?;
        }

        Ok(())
    }

    pub fn delete_document(
        &self,
        path: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let path_field = self.schema.get_field("path").unwrap();
        let path_term = tantivy::Term::from_field_text(path_field, path);
        let writer = self.writer.blocking_lock();
        writer.delete_term(path_term);

        let count = self.pending_count.fetch_add(1, Ordering::Relaxed) + 1;
        if count >= COMMIT_BATCH_SIZE {
            drop(writer);
            self.commit()?;
        }

        Ok(())
    }

    pub fn commit(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut writer = self.writer.blocking_lock();
        writer.commit()?;
        self.pending_count.store(0, Ordering::Relaxed);
        Ok(())
    }

    pub fn search(
        &self,
        query_str: &str,
        limit: usize,
    ) -> Result<Vec<SearchResult>, Box<dyn std::error::Error + Send + Sync>> {
        if self.pending_count.load(Ordering::Relaxed) > 0 {
            self.commit()?;
        }

        let title_field = self.schema.get_field("title").unwrap();
        let content_field = self.schema.get_field("content").unwrap();
        let path_field = self.schema.get_field("path").unwrap();

        let reader = self.reader.searcher();
        let query_parser = QueryParser::for_index(&self.index, vec![title_field, content_field]);
        let query = query_parser.parse_query(query_str)?;

        let top_docs = reader.search(&query, &TopDocs::with_limit(limit))?;

        let mut results = Vec::new();
        for (score, doc_address) in top_docs {
            let doc: TantivyDocument = reader.doc(doc_address)?;
            let path = doc
                .get_first(path_field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let title = doc
                .get_first(title_field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let snippet = doc
                .get_first(content_field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .chars()
                .take(200)
                .collect();

            results.push(SearchResult {
                path,
                title,
                snippet,
                score,
            });
        }

        Ok(results)
    }
}

fn extract_title(content: &str, path: &str) -> String {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("# ") {
            return trimmed[2..].to_string();
        }
    }
    std::path::Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Untitled")
        .to_string()
}
