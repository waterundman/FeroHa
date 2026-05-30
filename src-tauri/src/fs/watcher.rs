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
                        if let Some(ext) = path.extension() {
                            if ext != "md" {
                                continue;
                            }
                        }

                        let relative = path
                            .strip_prefix(&watch_path)
                            .unwrap_or(path)
                            .to_string_lossy()
                            .to_string();

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
