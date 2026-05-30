// Plugin Manager — Install, enable/disable, auto-update plugins
// Stage 6: Full implementation
#![allow(dead_code)]

use super::loader::{PluginInfo, PluginLoadError, PluginLoader};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct PluginManagerConfig {
    pub plugins_dir: String,
    pub auto_update: bool,
    pub marketplace_url: String,
}

impl Default for PluginManagerConfig {
    fn default() -> Self {
        PluginManagerConfig {
            plugins_dir: "./plugins".to_string(),
            auto_update: false,
            marketplace_url: "https://plugins.dualtrack.dev/api/v1".to_string(),
        }
    }
}

/// Plugin Manager — manages the lifecycle of installed plugins
pub struct PluginManager {
    loader: PluginLoader,
    config: PluginManagerConfig,
    /// Currently installed plugins (id → info)
    installed: HashMap<String, PluginInfo>,
    /// Currently enabled plugin IDs
    enabled: Vec<String>,
}

impl PluginManager {
    /// Initialize the plugin manager
    pub fn new(config: PluginManagerConfig, host_version: &str) -> Self {
        let loader = PluginLoader::new(&config.plugins_dir, host_version);
        PluginManager {
            loader,
            config,
            installed: HashMap::new(),
            enabled: Vec::new(),
        }
    }

    /// Scan and load all installed plugins
    pub fn initialize(&mut self) -> Result<usize, PluginLoadError> {
        let manifests = self.loader.discover()?;
        let now = chrono::Local::now().to_rfc3339();

        for manifest in manifests {
            let manifest_name = manifest.name.clone();
            let info = PluginInfo {
                id: manifest_name.clone(),
                manifest,
                enabled: self.enabled.contains(&manifest_name),
                installed_at: now.clone(),
                path: format!("{}/{}", self.config.plugins_dir, manifest_name),
            };
            self.installed.insert(info.id.clone(), info);
        }

        Ok(self.installed.len())
    }

    /// Install a plugin from a local archive file
    pub fn install(&mut self, archive_path: &str) -> Result<PluginInfo, PluginLoadError> {
        let path = PathBuf::from(archive_path);
        let manifest = self.loader.install_from_archive(&path)?;
        let manifest_name = manifest.name.clone();

        let now = chrono::Local::now().to_rfc3339();
        let info = PluginInfo {
            id: manifest_name.clone(),
            manifest,
            enabled: false, // Not enabled by default after fresh install
            installed_at: now,
            path: format!("{}/{}", self.config.plugins_dir, manifest_name),
        };

        self.installed.insert(info.id.clone(), info.clone());
        Ok(info)
    }

    /// Uninstall a plugin
    pub fn uninstall(&mut self, plugin_id: &str) -> Result<(), String> {
        if !self.installed.contains_key(plugin_id) {
            return Err(format!("Plugin '{}' is not installed", plugin_id));
        }

        // Disable first if enabled
        self.disable(plugin_id).ok();

        // Remove from disk
        let plugin_dir = PathBuf::from(&self.config.plugins_dir).join(plugin_id);
        if plugin_dir.exists() {
            fs::remove_dir_all(&plugin_dir)
                .map_err(|e| format!("Failed to remove plugin directory: {}", e))?;
        }

        self.installed.remove(plugin_id);
        Ok(())
    }

    /// Enable a plugin
    pub fn enable(&mut self, plugin_id: &str) -> Result<(), String> {
        if !self.installed.contains_key(plugin_id) {
            return Err(format!("Plugin '{}' is not installed", plugin_id));
        }
        if !self.enabled.contains(&plugin_id.to_string()) {
            self.enabled.push(plugin_id.to_string());
            if let Some(info) = self.installed.get_mut(plugin_id) {
                info.enabled = true;
            }
        }
        Ok(())
    }

    /// Disable a plugin
    pub fn disable(&mut self, plugin_id: &str) -> Result<(), String> {
        self.enabled.retain(|id| id != plugin_id);
        if let Some(info) = self.installed.get_mut(plugin_id) {
            info.enabled = false;
        }
        Ok(())
    }

    /// List all installed plugins
    pub fn list_all(&self) -> Vec<&PluginInfo> {
        self.installed.values().collect()
    }

    /// List enabled plugins
    pub fn list_enabled(&self) -> Vec<&PluginInfo> {
        self.enabled
            .iter()
            .filter_map(|id| self.installed.get(id))
            .collect()
    }

    /// Get a specific plugin's info
    pub fn get(&self, plugin_id: &str) -> Option<&PluginInfo> {
        self.installed.get(plugin_id)
    }

    /// Check for updates from the marketplace
    pub fn check_updates(&self) -> Vec<PluginUpdate> {
        // Stage 6: mock marketplace response
        // In production: HTTP GET to config.marketplace_url/updates
        Vec::new()
    }

    /// Get the number of installed plugins
    pub fn count(&self) -> usize {
        self.installed.len()
    }

    /// Get the number of enabled plugins
    pub fn enabled_count(&self) -> usize {
        self.enabled.len()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginUpdate {
    pub plugin_id: String,
    pub current_version: String,
    pub latest_version: String,
    pub changelog: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::loader::PluginManifest;
    use tempfile::TempDir;

    fn setup_manager() -> (PluginManager, TempDir) {
        let dir = TempDir::new().unwrap();
        let config = PluginManagerConfig {
            plugins_dir: dir.path().to_str().unwrap().to_string(),
            ..Default::default()
        };
        let manager = PluginManager::new(config, "0.2.0");
        (manager, dir)
    }

    #[test]
    fn test_init_empty() {
        let (mut manager, _dir) = setup_manager();
        let count = manager.initialize().unwrap();
        assert_eq!(count, 0);
        assert_eq!(manager.count(), 0);
    }

    #[test]
    fn test_enable_disable_cycle() {
        let (mut manager, _dir) = setup_manager();

        // Manually insert a plugin for testing
        let info = PluginInfo {
            id: "test".to_string(),
            manifest: PluginManifest {
                name: "test".to_string(),
                version: "1.0".to_string(),
                description: "".to_string(),
                author: "".to_string(),
                homepage: None,
                min_app_version: None,
                capabilities: Default::default(),
                wasm: crate::plugin::loader::WasmConfig {
                    entry: "p.wasm".to_string(),
                    memory_pages: 128,
                },
                dependencies: Default::default(),
            },
            enabled: false,
            installed_at: "now".to_string(),
            path: "/tmp".to_string(),
        };
        manager.installed.insert("test".to_string(), info);

        // Initially disabled
        assert_eq!(manager.enabled_count(), 0);

        // Enable
        manager.enable("test").unwrap();
        assert_eq!(manager.enabled_count(), 1);
        assert!(manager.get("test").unwrap().enabled);

        // Disable
        manager.disable("test").unwrap();
        assert_eq!(manager.enabled_count(), 0);
        assert!(!manager.get("test").unwrap().enabled);
    }

    #[test]
    fn test_uninstall() {
        let (mut manager, _dir) = setup_manager();

        let info = PluginInfo {
            id: "test".to_string(),
            manifest: PluginManifest {
                name: "test".to_string(),
                version: "1.0".to_string(),
                description: "".to_string(),
                author: "".to_string(),
                homepage: None,
                min_app_version: None,
                capabilities: Default::default(),
                wasm: crate::plugin::loader::WasmConfig {
                    entry: "p.wasm".to_string(),
                    memory_pages: 128,
                },
                dependencies: Default::default(),
            },
            enabled: false,
            installed_at: "now".to_string(),
            path: "/tmp".to_string(),
        };
        manager.installed.insert("test".to_string(), info);

        manager.uninstall("test").unwrap();
        assert!(!manager.installed.contains_key("test"));
    }
}
