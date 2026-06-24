// Vault Manager — Local Markdown file system operations

use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

#[allow(dead_code)]
pub type VaultHandle = VaultManager;

#[derive(Debug, thiserror::Error)]
pub enum VaultError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Not a valid vault directory: {0}")]
    InvalidVault(String),
    #[error("Invalid relative path: {0}")]
    InvalidPath(String),
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
    pub tags: Vec<String>,
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
            return Err(VaultError::InvalidVault(format!(
                "Directory does not exist: {}",
                root_path.display()
            )));
        }
        if !root_path.is_dir() {
            return Err(VaultError::InvalidVault(format!(
                "Not a directory: {}",
                root_path.display()
            )));
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
        let safe_path = normalize_relative_path(relative_path.as_ref())?;
        let full_path = self.root_path.join(&safe_path);
        if !full_path.exists() {
            return Err(VaultError::FileNotFound(safe_path));
        }
        Ok(fs::read_to_string(full_path)?)
    }

    /// Write content to a note (create or overwrite) — atomic: temp → fsync → rename
    pub fn write_note<P: AsRef<Path>>(
        &self,
        relative_path: P,
        content: &str,
    ) -> Result<(), VaultError> {
        let safe_path = normalize_relative_path(relative_path.as_ref())?;
        let full_path = self.root_path.join(&safe_path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let temp_path = full_path.with_extension("tmp");
        let mut file = fs::File::create(&temp_path)?;
        file.write_all(content.as_bytes())?;
        file.sync_all()?;
        replace_file(&temp_path, &full_path)?;

        // Update cache
        let relative = safe_path.to_string_lossy().replace('\\', "/");
        let file_name = safe_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let metadata = fs::metadata(&full_path)?;
        let tags = crate::parser::frontmatter::parse_frontmatter(content)
            .map(|(fm, body_offset)| {
                let mut all_tags = fm.tags;
                let inline_tags =
                    crate::parser::frontmatter::extract_inline_tags(content, body_offset);
                for tag in inline_tags {
                    if !all_tags.contains(&tag) {
                        all_tags.push(tag);
                    }
                }
                all_tags
            })
            .unwrap_or_default();
        let note = NoteMeta {
            path: relative.clone(),
            title: file_name.trim_end_matches(".md").to_string(),
            size: metadata.len(),
            modified: metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs().to_string())
                .unwrap_or_default(),
            created: metadata
                .created()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs().to_string())
                .unwrap_or_default(),
            links: Vec::new(),
            tags,
        };
        self.notes_cache.borrow_mut().insert(relative, note);

        Ok(())
    }

    /// Save binary asset (e.g. pasted image) to vault
    pub fn save_asset(&self, relative_path: &str, content: &[u8]) -> Result<(), VaultError> {
        let safe_path = normalize_relative_path(Path::new(relative_path))?;
        let full_path = self.root_path.join(safe_path);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let temp_path = full_path.with_extension("tmp");
        let mut file = fs::File::create(&temp_path)?;
        file.write_all(content)?;
        file.sync_all()?;
        replace_file(&temp_path, &full_path)?;
        Ok(())
    }

    /// Delete a note
    pub fn delete_note<P: AsRef<Path>>(&self, relative_path: P) -> Result<(), VaultError> {
        let safe_path = normalize_relative_path(relative_path.as_ref())?;
        let full_path = self.root_path.join(&safe_path);
        if !full_path.exists() {
            return Err(VaultError::FileNotFound(safe_path));
        }
        fs::remove_file(full_path)?;
        let key = safe_path.to_string_lossy().replace('\\', "/");
        self.notes_cache.borrow_mut().remove(&key);
        Ok(())
    }

    /// Create a folder inside the vault.
    pub fn create_folder<P: AsRef<Path>>(&self, relative_path: P) -> Result<(), VaultError> {
        let safe_path = normalize_relative_path(relative_path.as_ref())?;
        fs::create_dir_all(self.root_path.join(safe_path))?;
        Ok(())
    }

    /// Rename a note (move/rename file)
    pub fn rename_note(&self, old_path: &str, new_path: &str) -> Result<(), String> {
        let old_safe = normalize_relative_path(Path::new(old_path)).map_err(|e| e.to_string())?;
        let new_safe = normalize_relative_path(Path::new(new_path)).map_err(|e| e.to_string())?;
        let old_full = self.root_path.join(&old_safe);
        let new_full = self.root_path.join(&new_safe);

        if !old_full.exists() {
            return Err(format!("File not found: {}", old_path));
        }
        if new_full.exists() {
            return Err(format!("Target already exists: {}", new_path));
        }

        if let Some(parent) = new_full.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        std::fs::rename(&old_full, &new_full).map_err(|e| e.to_string())?;
        self.refresh_cache().map_err(|e| e.to_string())?;
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
                let file_content = fs::read_to_string(&path).ok();
                let tags = file_content
                    .as_ref()
                    .and_then(|c| {
                        crate::parser::frontmatter::parse_frontmatter(c).map(|(fm, body_offset)| {
                            let mut all_tags = fm.tags;
                            let inline_tags =
                                crate::parser::frontmatter::extract_inline_tags(c, body_offset);
                            for tag in inline_tags {
                                if !all_tags.contains(&tag) {
                                    all_tags.push(tag);
                                }
                            }
                            all_tags
                        })
                    })
                    .unwrap_or_default();
                let note = NoteMeta {
                    path: relative.clone(),
                    title: file_name.trim_end_matches(".md").to_string(),
                    size: metadata.len(),
                    modified: metadata
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs().to_string())
                        .unwrap_or_default(),
                    created: metadata
                        .created()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs().to_string())
                        .unwrap_or_default(),
                    links: Vec::new(), // populated after AST parsing
                    tags,
                };
                self.notes_cache.borrow_mut().insert(relative, note);
            }
        }
        Ok(())
    }
}

fn normalize_relative_path(path: &Path) -> Result<PathBuf, VaultError> {
    if path.as_os_str().is_empty() || path.is_absolute() {
        return Err(VaultError::InvalidPath(path.display().to_string()));
    }

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::Normal(part) => normalized.push(part),
            std::path::Component::CurDir => {}
            _ => return Err(VaultError::InvalidPath(path.display().to_string())),
        }
    }

    if normalized.as_os_str().is_empty() {
        return Err(VaultError::InvalidPath(path.display().to_string()));
    }

    Ok(normalized)
}

fn replace_file(temp_path: &Path, target_path: &Path) -> Result<(), VaultError> {
    match fs::rename(temp_path, target_path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists && target_path.exists() => {
            fs::remove_file(target_path)?;
            fs::rename(temp_path, target_path)?;
            Ok(())
        }
        Err(error) => Err(VaultError::Io(error)),
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

    #[test]
    fn test_write_note_overwrites_existing_file() {
        let dir = TempDir::new().unwrap();
        let vault = VaultManager::open(dir.path()).unwrap();

        vault.write_note("test.md", "first").unwrap();
        vault.write_note("test.md", "second").unwrap();

        assert_eq!(vault.read_note("test.md").unwrap(), "second");
    }

    #[test]
    fn test_save_asset_overwrites_existing_file() {
        let dir = TempDir::new().unwrap();
        let vault = VaultManager::open(dir.path()).unwrap();

        vault.save_asset("asset.bin", b"first").unwrap();
        vault.save_asset("asset.bin", b"second").unwrap();

        assert_eq!(fs::read(dir.path().join("asset.bin")).unwrap(), b"second");
    }

    #[test]
    fn test_rejects_path_traversal() {
        let dir = TempDir::new().unwrap();
        let vault = VaultManager::open(dir.path()).unwrap();

        assert!(vault.write_note("../outside.md", "nope").is_err());
        assert!(vault.read_note("../outside.md").is_err());
        assert!(vault.delete_note("../outside.md").is_err());
    }

    #[test]
    fn test_create_folder_inside_vault() {
        let dir = TempDir::new().unwrap();
        let vault = VaultManager::open(dir.path()).unwrap();

        vault.create_folder("nested/folder").unwrap();
        assert!(dir.path().join("nested").join("folder").is_dir());
        assert!(vault.create_folder("../outside").is_err());
    }

    #[test]
    fn test_save_asset_rejects_path_traversal() {
        let dir = TempDir::new().unwrap();
        let vault_dir = dir.path().join("vault");
        fs::create_dir_all(&vault_dir).unwrap();
        let vault = VaultManager::open(&vault_dir).unwrap();

        assert!(vault.save_asset("../outside.bin", b"nope").is_err());
        assert!(!dir.path().join("outside.bin").exists());
    }

    #[test]
    fn test_rename_note_rejects_path_traversal() {
        let dir = TempDir::new().unwrap();
        let vault_dir = dir.path().join("vault");
        fs::create_dir_all(&vault_dir).unwrap();
        let vault = VaultManager::open(&vault_dir).unwrap();

        vault.write_note("inside.md", "# Inside").unwrap();

        assert!(vault.rename_note("inside.md", "../outside.md").is_err());
        assert!(vault.read_note("inside.md").is_ok());
        assert!(!dir.path().join("outside.md").exists());
    }
}
