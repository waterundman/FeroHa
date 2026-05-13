// Vault Manager — Local Markdown file system operations

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs;
use std::cell::RefCell;

pub type VaultHandle = VaultManager;

#[derive(Debug, thiserror::Error)]
pub enum VaultError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Not a valid vault directory: {0}")]
    InvalidVault(String),
    #[error("File not found: {0}")]
    FileNotFound(PathBuf),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NoteMeta {
    pub path: String,
    pub title: String,
    pub size: u64,
    pub modified: String,
    pub created: String,
    pub links: Vec<String>,
}

/// Core Vault Manager — operates on a local directory of .md files
#[derive(Debug)]
pub struct VaultManager {
    pub root_path: PathBuf,
    /// Cache of note metadata
    notes_cache: RefCell<HashMap<String, NoteMeta>>,
}

impl VaultManager {
    /// Open a vault from a local directory path
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, VaultError> {
        let root_path = path.as_ref().to_path_buf();
        if !root_path.exists() {
            return Err(VaultError::InvalidVault(
                format!("Directory does not exist: {}", root_path.display())
            ));
        }
        if !root_path.is_dir() {
            return Err(VaultError::InvalidVault(
                format!("Not a directory: {}", root_path.display())
            ));
        }

        let vault = VaultManager {
            root_path,
            notes_cache: RefCell::new(HashMap::new()),
        };
        vault.refresh_cache()?;
        Ok(vault)
    }

    /// List all .md files recursively
    pub fn list_notes(&self) -> Result<Vec<NoteMeta>, VaultError> {
        Ok(self.notes_cache.borrow().values().cloned().collect())
    }

    /// Read a note's content
    pub fn read_note<P: AsRef<Path>>(&self, relative_path: P) -> Result<String, VaultError> {
        let full_path = self.root_path.join(relative_path.as_ref());
        if !full_path.exists() {
            return Err(VaultError::FileNotFound(relative_path.as_ref().to_path_buf()));
        }
        Ok(fs::read_to_string(full_path)?)
    }

    /// Write content to a note (create or overwrite)
    pub fn write_note<P: AsRef<Path>>(&self, relative_path: P, content: &str) -> Result<(), VaultError> {
        let full_path = self.root_path.join(relative_path.as_ref());
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&full_path, content)?;
        
        // Update cache
        let relative = relative_path.as_ref().to_string_lossy().to_string();
        let file_name = relative_path.as_ref()
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let metadata = fs::metadata(&full_path)?;
        let note = NoteMeta {
            path: relative.clone(),
            title: file_name.trim_end_matches(".md").to_string(),
            size: metadata.len(),
            modified: metadata.modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs().to_string())
                .unwrap_or_default(),
            created: metadata.created()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs().to_string())
                .unwrap_or_default(),
            links: Vec::new(),
        };
        self.notes_cache.borrow_mut().insert(relative, note);
        
        Ok(())
    }

    /// Delete a note
    pub fn delete_note<P: AsRef<Path>>(&self, relative_path: P) -> Result<(), VaultError> {
        let full_path = self.root_path.join(relative_path.as_ref());
        if !full_path.exists() {
            return Err(VaultError::FileNotFound(relative_path.as_ref().to_path_buf()));
        }
        fs::remove_file(full_path)?;
        let key = relative_path.as_ref().to_string_lossy().to_string();
        self.notes_cache.borrow_mut().remove(&key);
        Ok(())
    }

    /// Rescan vault directory and refresh metadata cache
    pub fn refresh_cache(&self) -> Result<(), VaultError> {
        self.notes_cache.borrow_mut().clear();
        self.scan_directory(&self.root_path.clone(), "")?;
        Ok(())
    }

    fn scan_directory(&self, dir: &Path, prefix: &str) -> Result<(), VaultError> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let file_name = entry.file_name().to_string_lossy().to_string();

            // Skip hidden directories and files
            if file_name.starts_with('.') || file_name.starts_with('_') {
                continue;
            }

            let relative = if prefix.is_empty() {
                file_name.clone()
            } else {
                format!("{}/{}", prefix, file_name)
            };

            if path.is_dir() {
                self.scan_directory(&path, &relative)?;
            } else if path.extension().map(|e| e == "md").unwrap_or(false) {
                let metadata = entry.metadata()?;
                let note = NoteMeta {
                    path: relative.clone(),
                    title: file_name.trim_end_matches(".md").to_string(),
                    size: metadata.len(),
                    modified: metadata.modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs().to_string())
                        .unwrap_or_default(),
                    created: metadata.created()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs().to_string())
                        .unwrap_or_default(),
                    links: Vec::new(), // populated after AST parsing
                };
                self.notes_cache.borrow_mut().insert(relative, note);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_open_valid_vault() {
        let dir = TempDir::new().unwrap();
        let vault = VaultManager::open(dir.path()).unwrap();
        assert_eq!(vault.list_notes().unwrap().len(), 0);
    }

    #[test]
    fn test_open_invalid_path() {
        let result = VaultManager::open("/nonexistent/path");
        assert!(result.is_err());
    }

    #[test]
    fn test_read_write_delete() {
        let dir = TempDir::new().unwrap();
        let vault = VaultManager::open(dir.path()).unwrap();

        vault.write_note("test.md", "# Hello").unwrap();
        let content = vault.read_note("test.md").unwrap();
        assert_eq!(content, "# Hello");

        let notes = vault.list_notes().unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].title, "test");

        vault.delete_note("test.md").unwrap();
        assert!(vault.read_note("test.md").is_err());
    }
}
