// File Watcher — Real-time vault change monitoring using notify

use notify::{Event, EventKind, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};
use std::time::{Duration, Instant};
use tokio::sync::broadcast;

use crate::ai::search_engine::SearchEngine;
use std::sync::Arc;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FileEvent {
    pub path: String,
    pub kind: FileEventKind,
    pub timestamp: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileEventKind {
    Created,
    Modified,
    Deleted,
    Renamed { from: String },
}

/// Debounced file watcher for vault changes
pub struct FileWatcher {
    #[allow(dead_code)]
    vault_path: PathBuf,
    event_tx: broadcast::Sender<FileEvent>,
    #[allow(dead_code)]
    shutdown_tx: Sender<()>,
    watcher: Box<dyn notify::Watcher + Send>,
    #[allow(dead_code)]
    search_engine: Option<Arc<SearchEngine>>,
}

const DEBOUNCE_MS: u64 = 300;

impl FileWatcher {
    /// Start watching a vault directory
    pub fn watch<P: AsRef<Path>>(
        vault_path: P,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let vault_path = vault_path.as_ref().to_path_buf();
        let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>();
        let (event_tx, _) = broadcast::channel::<FileEvent>(256);
        let broadcast_tx = event_tx.clone();

        let watch_path = vault_path.clone();
        let mut watcher: Box<dyn Watcher + Send> = Box::new(notify::recommended_watcher(
            move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    let kind = match event.kind {
                        EventKind::Create(_) => FileEventKind::Created,
                        EventKind::Modify(_) => FileEventKind::Modified,
                        EventKind::Remove(_) => FileEventKind::Deleted,
                        _ => return,
                    };

                    for path in &event.paths {
                        let relative = path
                            .strip_prefix(&watch_path)
                            .unwrap_or(path)
                            .to_string_lossy()
                            .to_string();
                        if !is_content_markdown_path(&relative) {
                            continue;
                        }

                        let event = FileEvent {
                            path: relative,
                            kind: kind.clone(),
                            timestamp: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_millis() as u64,
                        };
                        let _ = broadcast_tx.send(event);
                    }
                }
            },
        )?);

        watcher.watch(&vault_path, RecursiveMode::Recursive)?;

        // Spawn debounce + shutdown handler thread
        std::thread::spawn(move || {
            let _last_event: Option<(Instant, FileEvent)> = None;

            loop {
                std::thread::sleep(Duration::from_millis(DEBOUNCE_MS));

                if shutdown_rx.try_recv().is_ok() {
                    break;
                }

                // Debounce logic can be expanded here
                // For now, events are emitted immediately via the watcher callback
            }
        });

        Ok(FileWatcher {
            vault_path,
            event_tx,
            shutdown_tx,
            watcher,
            search_engine: None,
        })
    }

    /// Subscribe to file change events
    pub fn subscribe(&self) -> broadcast::Receiver<FileEvent> {
        self.event_tx.subscribe()
    }

    /// Set the search engine for FTS index updates
    pub fn set_search_engine(&mut self, engine: Arc<SearchEngine>) {
        self.search_engine = Some(engine);
    }
}

impl Drop for FileWatcher {
    fn drop(&mut self) {
        // Unwatch the path to clean up resources
        let _ = self.watcher.unwatch(&self.vault_path);
    }
}

pub(crate) fn is_content_markdown_path(path: &str) -> bool {
    let path = std::path::Path::new(path);
    if path.is_absolute() {
        return false;
    }

    let mut has_component = false;
    for component in path.components() {
        match component {
            std::path::Component::Normal(part) => {
                has_component = true;
                let name = part.to_string_lossy();
                if name.starts_with('.') || name.starts_with('_') {
                    return false;
                }
            }
            std::path::Component::CurDir => {}
            _ => return false,
        }
    }

    has_component && path.extension().map(|ext| ext == "md").unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::{is_content_markdown_path, FileEventKind, FileWatcher};
    use std::time::Duration;

    #[test]
    fn content_markdown_path_filter_excludes_internal_and_unsafe_paths() {
        assert!(is_content_markdown_path("notes/source.md"));
        assert!(!is_content_markdown_path(".dualtrack/research/result.md"));
        assert!(!is_content_markdown_path("notes/.hidden.md"));
        assert!(!is_content_markdown_path("_templates/source.md"));
        assert!(!is_content_markdown_path("notes/image.png"));
        assert!(!is_content_markdown_path("../outside.md"));
    }

    #[test]
    fn file_watcher_emits_content_markdown_events_and_skips_internal_paths() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let dir = tempfile::tempdir().unwrap();
            let watcher = FileWatcher::watch(dir.path()).unwrap();
            let mut rx = watcher.subscribe();

            let internal_dir = dir.path().join(".dualtrack").join("research");
            std::fs::create_dir_all(&internal_dir).unwrap();
            std::fs::write(internal_dir.join("result.md"), "# Internal\n").unwrap();
            assert!(tokio::time::timeout(Duration::from_millis(500), rx.recv())
                .await
                .is_err());

            let notes_dir = dir.path().join("notes");
            std::fs::create_dir_all(&notes_dir).unwrap();
            std::fs::write(notes_dir.join("source.md"), "# Source\n").unwrap();

            let event = tokio::time::timeout(Duration::from_secs(5), async {
                loop {
                    let event = rx.recv().await.unwrap();
                    if event.path.replace('\\', "/") == "notes/source.md" {
                        break event;
                    }
                }
            })
            .await
            .expect("watcher did not emit notes/source.md event");

            assert_eq!(event.path.replace('\\', "/"), "notes/source.md");
            assert!(matches!(
                event.kind,
                FileEventKind::Created | FileEventKind::Modified
            ));

            drop(watcher);
        });
    }
}
