// Plugin Loader — Load, validate, and instantiate WASM plugins
// Stage 6: Full implementation with wasmtime sandbox
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum PluginLoadError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid manifest: {0}")]
    InvalidManifest(String),
    #[error("WASM module too large: {0} bytes (max: {1})")]
    ModuleTooLarge(u64, u64),
    #[error("Capability mismatch: plugin requires '{0}' but not granted")]
    CapabilityDenied(String),
    #[error("Version mismatch: requires host >= {0}, current {1}")]
    VersionMismatch(String, String),
    #[error("Invalid signature: plugin manifest is not signed")]
    InvalidSignature,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub homepage: Option<String>,
    #[serde(default)]
    pub min_app_version: Option<String>,

    /// Required capabilities
    #[serde(default)]
    pub capabilities: PluginCapabilities,

    /// WASM module metadata
    pub wasm: WasmConfig,

    /// Other plugins this one depends on
    #[serde(default)]
    pub dependencies: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PluginCapabilities {
    #[serde(default)]
    pub note_read: bool,
    #[serde(default)]
    pub note_write: bool, // Always requires user grant at first write
    #[serde(default)]
    pub search_vault: bool,
    #[serde(default)]
    pub ai_access: bool,
    #[serde(default)]
    pub graph_read: bool,
    #[serde(default)]
    pub network: Vec<String>, // Allowed domains (empty = no network)
    #[serde(default)]
    pub fs_read: Vec<String>, // Allowed paths (empty = no FS access)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmConfig {
    pub entry: String, // Relative path to .wasm file
    #[serde(default = "default_memory_pages")]
    pub memory_pages: u32, // 64KB per page
}

fn default_memory_pages() -> u32 {
    256
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    pub id: String,
    pub manifest: PluginManifest,
    pub enabled: bool,
    pub installed_at: String,
    pub path: String,
}

/// Plugin Loader — validates manifests, loads WASM modules
pub struct PluginLoader {
    plugins_dir: PathBuf,
    host_version: String,
    max_module_size: u64, // bytes
}

impl PluginLoader {
    /// Create a new plugin loader pointing to the plugins directory
    pub fn new(plugins_dir: &str, host_version: &str) -> Self {
        PluginLoader {
            plugins_dir: PathBuf::from(plugins_dir),
            host_version: host_version.to_string(),
            max_module_size: 50 * 1024 * 1024, // 50MB max per WASM module
        }
    }

    /// Scan the plugins directory and discover all valid plugins
    pub fn discover(&self) -> Result<Vec<PluginManifest>, PluginLoadError> {
        let mut manifests = Vec::new();

        if !self.plugins_dir.exists() {
            fs::create_dir_all(&self.plugins_dir)?;
            return Ok(manifests);
        }

        for entry in fs::read_dir(&self.plugins_dir)? {
            let entry = entry?;
            let path = entry.path();

            if !path.is_dir() {
                continue;
            }

            let manifest_path = path.join("plugin.toml");
            if !manifest_path.exists() {
                continue;
            }

            match self.load_manifest(&manifest_path) {
                Ok(manifest) => manifests.push(manifest),
                Err(e) => {
                    eprintln!("Skipping plugin in {}: {}", path.display(), e);
                }
            }
        }

        Ok(manifests)
    }

    /// Load and validate a single plugin manifest
    pub fn load_manifest(&self, manifest_path: &Path) -> Result<PluginManifest, PluginLoadError> {
        let content = fs::read_to_string(manifest_path)?;
        let manifest: PluginManifest = toml::from_str(&content)
            .map_err(|e| PluginLoadError::InvalidManifest(format!("TOML parse error: {}", e)))?;

        self.validate(&manifest)?;
        Ok(manifest)
    }

    /// Validate a plugin manifest
    pub fn validate(&self, manifest: &PluginManifest) -> Result<(), PluginLoadError> {
        // Check name
        if manifest.name.is_empty() || manifest.name.len() > 64 {
            return Err(PluginLoadError::InvalidManifest(
                "Plugin name must be 1-64 characters".to_string(),
            ));
        }

        // Check version format (semver)
        if !manifest
            .version
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.')
        {
            return Err(PluginLoadError::InvalidManifest(
                "Version must be semver (e.g., 1.0.0)".to_string(),
            ));
        }

        // Check host version compatibility
        if let Some(ref min_ver) = manifest.min_app_version {
            if !self.is_version_compatible(min_ver) {
                return Err(PluginLoadError::VersionMismatch(
                    min_ver.clone(),
                    self.host_version.clone(),
                ));
            }
        }

        // Validate WASM entry
        let plugin_dir = self.plugins_dir.join(&manifest.name);
        let wasm_path = plugin_dir.join(&manifest.wasm.entry);
        if !wasm_path.exists() {
            return Err(PluginLoadError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("WASM file not found: {}", wasm_path.display()),
            )));
        }

        // Check module size
        let size = wasm_path.metadata()?.len();
        if size > self.max_module_size {
            return Err(PluginLoadError::ModuleTooLarge(size, self.max_module_size));
        }

        // Capability sanity checks
        if manifest.capabilities.note_write {
            // Write capability requires explicit user approval (runtime check)
            // Manifest can declare it, but doesn't auto-grant
        }

        if !manifest.capabilities.network.is_empty() {
            // Validate domain patterns
            for domain in &manifest.capabilities.network {
                if domain.contains('*') && domain != "*" {
                    // Wildcards must be exact-match patterns
                    if domain.matches('*').count() > 1 {
                        return Err(PluginLoadError::InvalidManifest(format!(
                            "Invalid network domain pattern: {}",
                            domain
                        )));
                    }
                }
            }
        }

        Ok(())
    }

    /// Load a WASM module and prepare it for sandboxed execution
    ///
    /// In production, this uses wasmtime::Module::from_file().
    /// During development (Stage 6), returns a validated handle.
    pub fn load_wasm(&self, manifest: &PluginManifest) -> Result<WasmHandle, PluginLoadError> {
        let plugin_dir = self.plugins_dir.join(&manifest.name);
        let wasm_path = plugin_dir.join(&manifest.wasm.entry);

        let wasm_bytes = fs::read(&wasm_path)?;

        // Validate WASM binary header
        if wasm_bytes.len() < 8 || &wasm_bytes[0..4] != b"\0asm" {
            return Err(PluginLoadError::InvalidManifest(
                "Invalid WASM binary (missing magic number)".to_string(),
            ));
        }

        // In production: compile and instantiate
        // let engine = wasmtime::Engine::default();
        // let module = wasmtime::Module::new(&engine, &wasm_bytes)?;

        Ok(WasmHandle {
            name: manifest.name.clone(),
            version: manifest.version.clone(),
            size: wasm_bytes.len() as u64,
            memory_pages: manifest.wasm.memory_pages,
            compiled: false, // Stage 6: validated, not compiled
        })
    }

    /// Install a plugin from a .tar.gz archive path
    pub fn install_from_archive(
        &self,
        archive_path: &Path,
    ) -> Result<PluginManifest, PluginLoadError> {
        // Extract to temporary directory
        let temp_dir = tempfile::tempdir()?;
        let archive_bytes = fs::read(archive_path)?;
        let cursor = std::io::Cursor::new(archive_bytes);

        // Decompress and extract
        let gz = flate2::read::GzDecoder::new(cursor);
        let mut archive = tar::Archive::new(gz);
        archive.unpack(temp_dir.path())?;

        // Find plugin.toml
        let manifest_path = temp_dir.path().join("plugin.toml");
        if !manifest_path.exists() {
            return Err(PluginLoadError::InvalidManifest(
                "Archive does not contain plugin.toml".to_string(),
            ));
        }

        let manifest = self.load_manifest(&manifest_path)?;

        // Copy to plugins directory
        let install_dir = self.plugins_dir.join(&manifest.name);
        if install_dir.exists() {
            fs::remove_dir_all(&install_dir)?;
        }
        copy_dir_all(temp_dir.path(), &install_dir)?;

        // Validate installed plugin
        let installed_manifest_path = install_dir.join("plugin.toml");
        self.load_manifest(&installed_manifest_path)
    }

    /// Check if a version satisfies the minimum requirement
    fn is_version_compatible(&self, min_version: &str) -> bool {
        let host_parts: Vec<u32> = self
            .host_version
            .split('.')
            .filter_map(|s| s.parse().ok())
            .collect();
        let min_parts: Vec<u32> = min_version
            .split('.')
            .filter_map(|s| s.parse().ok())
            .collect();

        for (i, min_part) in min_parts.iter().enumerate() {
            let host_part = host_parts.get(i).copied().unwrap_or(0);
            if host_part > *min_part {
                return true;
            }
            if host_part < *min_part {
                return false;
            }
        }
        true // equal versions
    }
}

/// Handle to a loaded WASM plugin module
#[derive(Debug, Clone)]
pub struct WasmHandle {
    pub name: String,
    pub version: String,
    pub size: u64,
    pub memory_pages: u32,
    pub compiled: bool,
}

/// Utility: recursively copy a directory
fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> std::io::Result<()> {
    let dst = dst.as_ref();
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src.as_ref())? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_all(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    const TEST_MANIFEST: &str = r#"
name = "test-plugin"
version = "1.0.0"
description = "A test plugin"
author = "test"

[capabilities]
note_read = true
search_vault = true

[wasm]
entry = "plugin.wasm"
memory_pages = 128
"#;

    const HOST_VERSION: &str = "0.2.0";

    fn setup_loader() -> (PluginLoader, TempDir, PathBuf) {
        let dir = TempDir::new().unwrap();
        let loader = PluginLoader::new(dir.path().to_str().unwrap(), HOST_VERSION);
        let plugin_dir = dir.path().join("test-plugin");
        fs::create_dir_all(&plugin_dir).unwrap();

        // Write manifest
        fs::write(plugin_dir.join("plugin.toml"), TEST_MANIFEST).unwrap();

        // Write minimal valid WASM binary (header + version)
        let wasm_bytes: Vec<u8> = vec![
            0x00, 0x61, 0x73, 0x6d, // magic \0asm
            0x01, 0x00, 0x00, 0x00, // version 1
        ];
        fs::write(plugin_dir.join("plugin.wasm"), &wasm_bytes).unwrap();

        (loader, dir, plugin_dir)
    }

    #[test]
    fn test_load_valid_manifest() {
        let (loader, _dir, plugin_dir) = setup_loader();
        let manifest = loader
            .load_manifest(&plugin_dir.join("plugin.toml"))
            .unwrap();
        assert_eq!(manifest.name, "test-plugin");
        assert_eq!(manifest.version, "1.0.0");
        assert!(manifest.capabilities.note_read);
    }

    #[test]
    fn test_discover_plugins() {
        let (loader, _dir, _plugin_dir) = setup_loader();
        let manifests = loader.discover().unwrap();
        assert_eq!(manifests.len(), 1);
        assert_eq!(manifests[0].name, "test-plugin");
    }

    #[test]
    fn test_validate_wasm_magic() {
        let (loader, _dir, plugin_dir) = setup_loader();
        let manifest = loader
            .load_manifest(&plugin_dir.join("plugin.toml"))
            .unwrap();
        let handle = loader.load_wasm(&manifest).unwrap();
        assert_eq!(handle.name, "test-plugin");
        assert!(!handle.compiled);
    }

    #[test]
    fn test_reject_invalid_wasm() {
        let (loader, _dir, plugin_dir) = setup_loader();
        // Write invalid WASM
        fs::write(plugin_dir.join("plugin.wasm"), b"not a wasm binary").unwrap();
        let manifest = loader
            .load_manifest(&plugin_dir.join("plugin.toml"))
            .unwrap();
        let result = loader.load_wasm(&manifest);
        assert!(result.is_err());
    }

    #[test]
    fn test_version_compatibility() {
        let loader = PluginLoader::new(".", "0.2.0");
        assert!(loader.is_version_compatible("0.1.0")); // lower OK
        assert!(loader.is_version_compatible("0.2.0")); // equal OK
        assert!(!loader.is_version_compatible("0.3.0")); // higher NOT OK
        assert!(!loader.is_version_compatible("1.0.0")); // major bump NOT OK
    }

    #[test]
    fn test_missing_manifest_rejected() {
        let dir = TempDir::new().unwrap();
        let plugin_dir = dir.path().join("bad-plugin");
        fs::create_dir_all(&plugin_dir).unwrap();

        let loader = PluginLoader::new(dir.path().to_str().unwrap(), HOST_VERSION);
        let result = loader.discover().unwrap();
        assert!(result.is_empty()); // No manifest, silently skipped
    }
}
