// Kernel facade — Pure Rust API wrapping existing modules without Tauri State/Window dependencies.
// This module provides a clean API boundary so that both the Tauri GUI and the standalone CLI
// can access the same functionality through identical Rust types.
//
// v2.14.0 — Stage 1: Agent CLI + Kernel Facade

use crate::ai::agent_scheduler::AgentScheduler;
use crate::ai::embedding::EmbeddingBackend;
use crate::diff::ghost_store::GhostStore;
use crate::fs::vault::{NoteMeta, VaultManager};
use crate::graph::link_graph::{GraphData, LinkGraph};
use std::path::{Path, PathBuf};

/// Errors returned by kernel operations
pub type KernelResult<T> = Result<T, String>;

/// The Kernel is the central facade for all FeroHa operations.
///
/// It wraps the modules that were previously only accessible through Tauri IPC commands,
/// exposing them as plain Rust functions. Both the GUI (via main.rs) and the CLI
/// (via cli/main.rs) use this facade.
pub struct Kernel {
    /// Vault root directory path
    vault_path: PathBuf,
    /// Vault file manager
    vault_manager: VaultManager,
    /// Knowledge link graph
    link_graph: LinkGraph,
    /// Ghost diff store
    ghost_store: GhostStore,
}

impl Kernel {
    /// Open a vault and initialize the kernel.
    ///
    /// This is the primary entry point. It validates the vault path,
    /// loads the VaultManager, builds the LinkGraph, and initializes the GhostStore.
    pub fn open<P: AsRef<Path>>(vault_path: P) -> KernelResult<Self> {
        let vault_path = vault_path.as_ref().to_path_buf();
        let dualtrack_dir = vault_path.join(".dualtrack");

        // Ensure .dualtrack directory exists
        std::fs::create_dir_all(&dualtrack_dir)
            .map_err(|e| format!("Failed to create .dualtrack directory: {}", e))?;

        // Open the vault manager
        let vault_manager =
            VaultManager::open(&vault_path).map_err(|e| format!("Failed to open vault: {}", e))?;

        // Build the link graph from all notes
        let mut link_graph = LinkGraph::new();
        Kernel::rebuild_link_graph_inner(&vault_manager, &mut link_graph)?;

        // Initialize ghost store
        let ghost_store = GhostStore::new(&dualtrack_dir);
        ghost_store
            .init()
            .map_err(|e| format!("Failed to init ghost store: {}", e))?;

        Ok(Kernel {
            vault_path,
            vault_manager,
            link_graph,
            ghost_store,
        })
    }

    /// Return the vault path
    pub fn vault_path(&self) -> &Path {
        &self.vault_path
    }

    // -----------------------------------------------------------------------
    // Note Operations
    // -----------------------------------------------------------------------

    /// Create a new note with the given title. If the title does not end with
    /// `.md`, the extension is appended automatically.
    pub fn create_note(&self, title: &str) -> KernelResult<NoteMeta> {
        let path_str = if title.ends_with(".md") {
            title.to_string()
        } else {
            format!("{}.md", title)
        };

        let safe_path = Path::new(&path_str);
        let file_stem = safe_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(title);

        let template = format!("# {}\n\n", file_stem);
        self.vault_manager
            .write_note(&path_str, &template)
            .map_err(|e| format!("Failed to create note: {}", e))?;

        // Re-list to find the created note
        let notes = self
            .vault_manager
            .list_notes()
            .map_err(|e| format!("Failed to list notes: {}", e))?;

        notes
            .into_iter()
            .find(|n| n.path == path_str)
            .ok_or_else(|| "Note was created but could not be found in listing".to_string())
    }

    /// List all notes in the vault.
    pub fn list_notes(&self) -> KernelResult<Vec<NoteMeta>> {
        self.vault_manager
            .list_notes()
            .map_err(|e| format!("Failed to list notes: {}", e))
    }

    /// Search notes by keyword (case-insensitive substring match over titles and paths).
    pub fn search_notes(&self, keyword: &str) -> KernelResult<Vec<NoteMeta>> {
        let notes = self.list_notes()?;
        let lower = keyword.to_lowercase();

        Ok(notes
            .into_iter()
            .filter(|note| {
                note.title.to_lowercase().contains(&lower)
                    || note.path.to_lowercase().contains(&lower)
                    || note.tags.iter().any(|t| t.to_lowercase().contains(&lower))
            })
            .collect())
    }

    /// Read the full content of a note by its relative path.
    pub fn read_note(&self, relative_path: &str) -> KernelResult<String> {
        self.vault_manager
            .read_note(relative_path)
            .map_err(|e| format!("Failed to read note '{}': {}", relative_path, e))
    }

    /// Delete a note by its relative path.
    pub fn delete_note(&self, relative_path: &str) -> KernelResult<()> {
        self.vault_manager
            .delete_note(relative_path)
            .map_err(|e| format!("Failed to delete note '{}': {}", relative_path, e))
    }

    // -----------------------------------------------------------------------
    // Graph Operations
    // -----------------------------------------------------------------------

    /// Get the current knowledge graph data (nodes + edges) suitable for
    /// JSON serialization and frontend rendering.
    pub fn get_graph(&self) -> KernelResult<GraphData> {
        Ok(self.link_graph.to_frontend_json())
    }

    // -----------------------------------------------------------------------
    // Agent / Scheduler Operations (stubs — wired after full kernel init)
    // -----------------------------------------------------------------------

    /// Create a fresh (uninitialized) AgentScheduler. Callers that need
    /// the full agent pipeline should use open_and_init() instead.
    pub fn create_scheduler(max_concurrent: usize) -> AgentScheduler {
        AgentScheduler::new(max_concurrent)
    }

    /// Build a default embedding backend (no external API key needed).
    pub fn default_embedding_backend() -> EmbeddingBackend {
        EmbeddingBackend::None
    }

    /// Return a reference to the underlying VaultManager (for use by Tauri commands).
    pub fn vault_manager(&self) -> &VaultManager {
        &self.vault_manager
    }

    /// Return a reference to the LinkGraph (for use by Tauri commands).
    pub fn link_graph_ref(&self) -> &LinkGraph {
        &self.link_graph
    }

    /// Return a reference to the GhostStore (for use by Tauri commands).
    pub fn ghost_store_ref(&self) -> &GhostStore {
        &self.ghost_store
    }

    /// Return the dualtrack directory path.
    pub fn dualtrack_dir(&self) -> PathBuf {
        self.vault_path.join(".dualtrack")
    }

    /// Return the vector store path.
    pub fn vector_store_path(&self) -> PathBuf {
        self.dualtrack_dir().join("vectors")
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Helper: rebuild in-memory link graph by scanning all notes for wikilinks.
    fn rebuild_link_graph_inner(vault: &VaultManager, graph: &mut LinkGraph) -> KernelResult<()> {
        let notes = vault
            .list_notes()
            .map_err(|e| format!("Failed to list notes: {}", e))?;

        let known_paths: std::collections::HashSet<String> =
            notes.iter().map(|note| note.path.clone()).collect();

        for note in &notes {
            graph.set_title(&note.path, &note.title);
        }

        for note in &notes {
            let content = vault.read_note(&note.path).unwrap_or_default();

            for link in crate::parser::ast::extract_wikilinks(&content, &note.path) {
                let target = resolve_wikilink_target(&link.target, &known_paths);
                if !target.is_empty() {
                    graph.add_link(&note.path, &target);
                }
            }
        }

        Ok(())
    }
}

/// Resolve a wikilink target against known note paths.
///
/// Tries case-insensitive suffix matching if an exact path is not found.
pub fn resolve_wikilink_target(
    target: &str,
    known_paths: &std::collections::HashSet<String>,
) -> String {
    let normalized = target.trim().replace('\\', "/");
    if normalized.is_empty() {
        return normalized;
    }

    let with_ext = if normalized.ends_with(".md") {
        normalized.clone()
    } else {
        format!("{}.md", normalized)
    };

    if known_paths.contains(&with_ext) {
        return with_ext;
    }

    // Case-insensitive suffix match
    let lower_stem = with_ext
        .trim_end_matches(".md")
        .rsplit('/')
        .next()
        .unwrap_or(&with_ext)
        .to_lowercase();

    known_paths
        .iter()
        .find(|path| {
            path.trim_end_matches(".md")
                .rsplit('/')
                .next()
                .unwrap_or(path)
                .eq_ignore_ascii_case(&lower_stem)
        })
        .cloned()
        .unwrap_or(with_ext)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_kernel_open() {
        let dir = TempDir::new().unwrap();
        let kernel = Kernel::open(dir.path()).unwrap();
        assert!(kernel.list_notes().unwrap().is_empty());
    }

    #[test]
    fn test_create_and_read_note() {
        let dir = TempDir::new().unwrap();
        let kernel = Kernel::open(dir.path()).unwrap();

        let meta = kernel.create_note("hello").unwrap();
        assert_eq!(meta.title, "hello");

        let content = kernel.read_note("hello.md").unwrap();
        assert!(content.contains("# hello"));

        let notes = kernel.list_notes().unwrap();
        assert_eq!(notes.len(), 1);
    }

    #[test]
    fn test_delete_note() {
        let dir = TempDir::new().unwrap();
        let kernel = Kernel::open(dir.path()).unwrap();

        kernel.create_note("temp").unwrap();
        kernel.delete_note("temp.md").unwrap();

        let notes = kernel.list_notes().unwrap();
        assert_eq!(notes.len(), 0);
    }

    #[test]
    fn test_search_notes() {
        let dir = TempDir::new().unwrap();
        let kernel = Kernel::open(dir.path()).unwrap();

        kernel.create_note("rust-guide").unwrap();
        kernel.create_note("python-notes").unwrap();
        kernel.create_note("rust-advanced").unwrap();

        let results = kernel.search_notes("rust").unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_get_graph_empty() {
        let dir = TempDir::new().unwrap();
        let kernel = Kernel::open(dir.path()).unwrap();

        let graph = kernel.get_graph().unwrap();
        assert!(graph.nodes.is_empty());
        assert!(graph.edges.is_empty());
    }

    #[test]
    fn test_read_nonexistent() {
        let dir = TempDir::new().unwrap();
        let kernel = Kernel::open(dir.path()).unwrap();

        let result = kernel.read_note("nonexistent.md");
        assert!(result.is_err());
    }
}
