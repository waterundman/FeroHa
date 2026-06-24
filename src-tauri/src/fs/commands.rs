use crate::ai::agent_scheduler::{AgentTask, SynthesizePhase, TaskPriority, TaskStatus, TaskType};
use crate::ai::dream_engine::DreamEngine;
use crate::ai::embedding::EmbeddingPipeline;
use crate::ai::search_engine::SearchEngine;
use crate::ai::skill_manager::SkillManager;
use crate::ai::subagent::Subagent;
use crate::ai::sync_engine::SyncEngine;
use crate::ai::task_scheduler::{CronJob, TaskScheduler};
use crate::ai::vectordb::VectorStore;
use crate::ai::workflow_runtime_service::WorkflowRuntimeService;
use crate::cli::parser::CliCommand;
use crate::diff::ghost_store::GhostStore;
use crate::fs::vault::{NoteMeta, VaultManager};
use crate::fs::watcher::{FileEvent, FileEventKind, FileWatcher};
use crate::graph::link_graph::{GraphEdge, GraphEdgeType, LinkGraph};
use crate::snapshot::engine::SnapshotEngine;
use crate::snapshot::store::SnapshotStore;
use crate::AiState;
use crate::AppState;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Listener, Manager, State};

#[tauri::command]
pub(crate) fn ping() -> String {
    "pong".to_string()
}

#[tauri::command]
pub(crate) fn open_vault<R: tauri::Runtime>(
    path: String,
    state: State<'_, Mutex<AppState>>,
    ai_state: State<'_, Mutex<AiState>>,
    app_handle: AppHandle<R>,
) -> Result<(), String> {
    open_vault_runtime(&path, app_handle, state, ai_state)
}

pub(crate) fn open_vault_runtime<R: tauri::Runtime>(
    path: &str,
    app_handle: AppHandle<R>,
    state: State<'_, Mutex<AppState>>,
    ai_state: State<'_, Mutex<AiState>>,
) -> Result<(), String> {
    let mut app = state.lock().map_err(|e| e.to_string())?;
    let mut ai = ai_state.lock().map_err(|e| e.to_string())?;
    let _dualtrack_dir = initialize_vault_services(&mut app, &mut ai, path)?;
    drop(ai);

    let watch_path = path.to_string();
    let worker_handle = app_handle.clone();

    if let Ok(watcher) = FileWatcher::watch(&watch_path) {
        let mut rx = watcher.subscribe();
        tauri::async_runtime::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        {
                            let state = worker_handle.state::<Mutex<AppState>>();
                            if let Ok(mut app) = state.lock() {
                                if let Err(e) = process_file_event(&mut app, &event) {
                                    tracing::warn!("Failed to process file event: {}", e);
                                }
                            };
                        }
                        let _ = worker_handle.emit("file-changed", &event);
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("FileWatcher event lagged by {} messages", n);
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }
        });
        app.file_watcher = Some(watcher);
        let search_engine = app.search_engine.clone();
        if let (Some(ref mut watcher), Some(engine)) = (app.file_watcher.as_mut(), search_engine) {
            watcher.set_search_engine(engine);
        }
        tracing::info!("File watcher started for: {}", watch_path);
    } else {
        tracing::warn!("Failed to start file watcher");
    }

    let should_start_snapshot_listeners = if app.snapshot_listeners_started {
        false
    } else {
        app.snapshot_listeners_started = true;
        true
    };

    if should_start_snapshot_listeners {
        // Listen for NOTE_OPENED -> global snapshot
        let note_handle = app_handle.clone();
        let _ = note_handle.listen_any("note-opened", {
            let h = note_handle.clone();
            move |event| {
                if let Ok(data) = serde_json::from_str::<serde_json::Value>(event.payload()) {
                    let note_id = data["note_id"].as_str().unwrap_or("");
                    let p = h
                        .state::<Mutex<AppState>>()
                        .lock()
                        .ok()
                        .map(|app| app.vault_path.clone())
                        .unwrap_or_default();
                    if p.is_empty() {
                        return;
                    }
                    crate::snapshot::commands::handle_global_snapshot(note_id, &p, &h);
                }
            }
        });

        // Listen for SELECTION_SUBMIT -> local snapshot
        let sel_handle = app_handle.clone();
        let _ = sel_handle.listen_any("selection-submit", {
            let h = sel_handle.clone();
            move |event| {
                if let Ok(data) = serde_json::from_str::<serde_json::Value>(event.payload()) {
                    let note_id = data["note_id"].as_str().unwrap_or("");
                    let start = data["start"].as_u64().unwrap_or(0) as usize;
                    let end = data["end"].as_u64().unwrap_or(0) as usize;
                    let p = h
                        .state::<Mutex<AppState>>()
                        .lock()
                        .ok()
                        .map(|app| app.vault_path.clone())
                        .unwrap_or_default();
                    if p.is_empty() {
                        return;
                    }
                    crate::snapshot::commands::handle_local_snapshot(note_id, start, end, &p, &h);
                }
            }
        });
    }

    // Start the task worker once; it reads current state on every loop.
    let should_start_worker = {
        let mut ai = ai_state.lock().map_err(|e| e.to_string())?;
        if ai.task_worker_started {
            false
        } else {
            ai.task_worker_started = true;
            true
        }
    };
    if should_start_worker {
        crate::ai::commands::start_task_worker(app_handle.clone());
    }

    {
        let mut ai = ai_state.lock().map_err(|e| e.to_string())?;
        if let Some(existing_scheduler) = ai.scheduler.take() {
            existing_scheduler.stop();
        }
    }

    // Initialize task scheduler for periodic auto-jobs
    let jobs = vec![CronJob {
        id: "dream-auto".to_string(),
        interval_secs: 6 * 3600,
        command: CliCommand::Dream,
        last_run: None,
        enabled: true,
    }];
    let scheduler = TaskScheduler::new(jobs);
    {
        let app_handle_sched = app_handle.clone();
        let on_tick: std::sync::Arc<dyn Fn() + Send + Sync> = std::sync::Arc::new(move || {
            let task_id = format!(
                "dream_auto_{}",
                uuid::Uuid::new_v4().to_string().replace('-', "")
            );
            let bridge_task_id = task_id.clone();
            let task_intent = crate::ai::task_intent::TaskIntentType::Dream;
            let sandbox_policy = task_intent.default_sandbox_policy();
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            let task = AgentTask {
                id: task_id,
                command: CliCommand::Dream,
                task_type: TaskType::Custom("dream-auto".to_string()),
                task_intent: Some(task_intent),
                sandbox_policy: Some(sandbox_policy),
                priority: TaskPriority::Low,
                priority_score: 10,
                status: TaskStatus::Pending,
                anchor_note: None,
                created_at: now,
                max_retries: 0,
                retry_count: 0,
                synthesize_phase: SynthesizePhase::Idle,
                subagent_results: vec![],
                graph_manifest: None,
                has_trace: false,
                source_block_id: None,
                card_id: None,
                card_type: None,
                prompt: None,
                params: None,
                context_note: None,
                intent: "dream-auto".to_string(),
                content: "dream-auto".to_string(),
                max_iterations: 30,
                sub_tasks: vec![],
                material_packet: None,
                context_fragments: vec![],
                regression_metrics: None,
                retry_delay_ms: 1000,
                retry_backoff_multiplier: 2.0,
                last_retry_at: None,
                consecutive_failures: 0,
            };
            {
                let state_handle = app_handle_sched.state::<Mutex<AiState>>();
                let mut ai = state_handle.lock().unwrap();
                ai.agent_scheduler.submit(task);
                ai.task_notifier.notify_one();
            }
            let app_state = app_handle_sched.state::<Mutex<AppState>>();
            crate::ai::commands::try_upsert_bridge_proposal(
                &app_state,
                Some(&app_handle_sched),
                |trust_snapshot, bridge_now| {
                    crate::bridge::proposal::BridgeProposal::for_scheduler_task(
                        &bridge_task_id,
                        "dream-auto",
                        trust_snapshot,
                        bridge_now,
                    )
                },
            );
        });
        scheduler.start(on_tick);
        let mut ai = ai_state.lock().map_err(|e| e.to_string())?;
        ai.scheduler = Some(std::sync::Arc::new(scheduler));
    }

    Ok(())
}

pub(crate) fn initialize_vault_services(
    app: &mut AppState,
    ai: &mut AiState,
    path: &str,
) -> Result<PathBuf, String> {
    let dualtrack_dir = PathBuf::from(&path).join(".dualtrack");
    std::fs::create_dir_all(&dualtrack_dir).map_err(|e| e.to_string())?;
    crate::ai::dream_memory::ensure_dream_memory_layout(&dualtrack_dir)?;

    let vault = VaultManager::open(&path).map_err(|e| e.to_string())?;
    app.vault_path = path.to_string();
    app.dualtrack_dir = dualtrack_dir.clone();
    app.bridge_store = Some(crate::bridge::store::store_for_dualtrack_dir(
        &dualtrack_dir,
    ));
    app.vector_store_path = dualtrack_dir.join("vectors");
    app.vault = Some(vault);

    let db_path = app.vector_store_path.to_str().unwrap_or(":memory:");
    let mut vector_store = VectorStore::open(db_path).map_err(|e| e.to_string())?;

    let embed_cfg = &ai.embedding_pipeline;
    let dim = embed_cfg.dimension();
    vector_store.set_dimension(dim);

    let embed_pipe = EmbeddingPipeline::new(ai.embedding_pipeline.backend_config());
    let sync_engine = SyncEngine::new(vector_store, embed_pipe);
    app.sync_engine = Some(sync_engine);

    ai.ghost_store = GhostStore::new(&dualtrack_dir);
    ai.ghost_store.init().map_err(|e| e.to_string())?;

    let serper_key = ai.llm_router.config().llm_api_key.clone();
    let subagent = if !serper_key.is_empty() {
        Subagent::new(Some(serper_key))
    } else {
        Subagent::new(None)
    };
    ai.subagent = Some(subagent);

    let vault_path = std::path::PathBuf::from(&path);
    ai.agent_scheduler.set_workflow_event_root(&vault_path);
    ai.skill_manager = Some(SkillManager::new(vault_path, "feroha".to_string()));
    match WorkflowRuntimeService::new(path).resume_all(&mut ai.agent_scheduler, now_millis()) {
        Ok(result) => {
            if !result.resumed.is_empty() {
                tracing::info!(
                    "Workflow runtime auto-resumed {} run(s)",
                    result.resumed.len()
                );
                ai.task_notifier.notify_one();
            }
            for error in result.errors {
                tracing::warn!(
                    "Workflow runtime auto-resume skipped {}: {}",
                    error.run_id,
                    error.error
                );
            }
        }
        Err(error) => {
            tracing::warn!("Workflow runtime auto-resume failed: {}", error);
        }
    }

    let embed_backend = ai.embedding_pipeline.backend_config();

    let snapshot_store = SnapshotStore::new(&dualtrack_dir);
    let snapshot_engine =
        SnapshotEngine::new(snapshot_store, EmbeddingPipeline::new(embed_backend));
    snapshot_engine.init().map_err(|e| e.to_string())?;
    let deleted = snapshot_engine.cleanup_expired();
    if deleted > 0 {
        tracing::info!(
            "Snapshot TTL cleanup: removed {} expired snapshots",
            deleted
        );
    }
    app.snapshot_engine = Some(snapshot_engine);

    // Load previous dream communities
    {
        let mut dream_engine = DreamEngine::new();
        dream_engine.set_dualtrack_dir(&dualtrack_dir);
        if let Some(ref sync) = app.sync_engine {
            dream_engine.load_from_db(sync.store());
        }
        app.dream_engine = Some(dream_engine);
    }

    sync_all_notes(app)?;
    rebuild_link_graph(app)?;

    // Initialize full-text search independently from file watching. Search should still work
    // when notify cannot start, while watcher updates can attach to the same engine when present.
    let vault_path_buf = PathBuf::from(&path);
    match SearchEngine::new(&vault_path_buf) {
        Ok(engine) => {
            let engine = Arc::new(engine);
            match engine.index_all_md_files() {
                Ok(count) => tracing::info!("FTS indexed {} markdown files", count),
                Err(e) => tracing::warn!("FTS initial indexing error: {}", e),
            }
            if let Some(ref mut watcher) = app.file_watcher {
                watcher.set_search_engine(engine.clone());
            }
            app.search_engine = Some(engine);
        }
        Err(e) => tracing::warn!("Failed to create search engine: {}", e),
    }

    let protocol = crate::ipc::protocol::TwoSurfaceProtocol::new();
    app.protocol = Some(protocol);

    let output_manager = crate::harness::output_hook::OutputManager::load_defaults(&dualtrack_dir);
    app.output_manager = Some(std::sync::Arc::new(output_manager));

    Ok(dualtrack_dir)
}

#[tauri::command]
pub(crate) fn get_vault_path(state: State<'_, Mutex<AppState>>) -> Result<String, String> {
    let app = state.lock().map_err(|e| e.to_string())?;
    if app.vault_path.is_empty() {
        Err("No vault open".to_string())
    } else {
        Ok(app.vault_path.clone())
    }
}

#[tauri::command]
pub(crate) fn list_notes(
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<NoteMeta>, String> {
    let app = state.lock().map_err(|e| e.to_string())?;
    let vault = app.vault.as_ref().ok_or("No vault open")?;
    vault.list_notes().map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn list_ai_workspace_files(
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<NoteMeta>, String> {
    let app = state.lock().map_err(|e| e.to_string())?;
    let vault = app.vault.as_ref().ok_or("No vault open")?;
    list_ai_workspace_files_for_root(&vault.root_path)
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FolderMeta {
    pub path: String,
    pub name: String,
}

#[tauri::command]
pub(crate) fn list_folders(state: State<'_, Mutex<AppState>>) -> Result<Vec<FolderMeta>, String> {
    let app = state.lock().map_err(|e| e.to_string())?;
    let vault = app.vault.as_ref().ok_or("No vault open")?;
    list_content_folders_for_root(&vault.root_path)
}

pub(crate) fn list_content_folders_for_root(root_path: &Path) -> Result<Vec<FolderMeta>, String> {
    let mut folders = Vec::new();
    scan_content_folders(root_path, "", &mut folders)?;
    folders.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(folders)
}

fn scan_content_folders(
    dir: &Path,
    prefix: &str,
    folders: &mut Vec<FolderMeta>,
) -> Result<(), String> {
    for entry in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name.starts_with('_') {
            continue;
        }

        let relative = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{}/{}", prefix, name)
        };

        folders.push(FolderMeta {
            path: relative.clone(),
            name,
        });
        scan_content_folders(&path, &relative, folders)?;
    }
    Ok(())
}

pub(crate) fn list_ai_workspace_files_for_root(root_path: &Path) -> Result<Vec<NoteMeta>, String> {
    let ai_root = root_path.join(".dualtrack");
    let mut notes = Vec::new();
    if !ai_root.exists() {
        return Ok(notes);
    }
    scan_ai_workspace_dir(root_path, &ai_root, &mut notes)?;
    notes.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(notes)
}

fn scan_ai_workspace_dir(
    root_path: &Path,
    dir: &Path,
    notes: &mut Vec<NoteMeta>,
) -> Result<(), String> {
    for entry in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            scan_ai_workspace_dir(root_path, &path, notes)?;
        } else if path.extension().map(|e| e == "md").unwrap_or(false) {
            notes.push(note_meta_for_workspace_file(root_path, &path)?);
        }
    }
    Ok(())
}

fn note_meta_for_workspace_file(root_path: &Path, path: &Path) -> Result<NoteMeta, String> {
    let metadata = std::fs::metadata(path).map_err(|e| e.to_string())?;
    let relative = path
        .strip_prefix(root_path)
        .map_err(|e| e.to_string())?
        .to_string_lossy()
        .replace('\\', "/");
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let tags = std::fs::read_to_string(path)
        .ok()
        .and_then(|content| {
            crate::parser::frontmatter::parse_frontmatter(&content).map(|(fm, body_offset)| {
                let mut all_tags = fm.tags;
                let inline_tags =
                    crate::parser::frontmatter::extract_inline_tags(&content, body_offset);
                for tag in inline_tags {
                    if !all_tags.contains(&tag) {
                        all_tags.push(tag);
                    }
                }
                all_tags
            })
        })
        .unwrap_or_default();

    Ok(NoteMeta {
        path: relative,
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
    })
}

#[tauri::command]
pub(crate) fn read_note(path: String, state: State<'_, Mutex<AppState>>) -> Result<String, String> {
    let app = state.lock().map_err(|e| e.to_string())?;
    let vault = app.vault.as_ref().ok_or("No vault open")?;
    vault.read_note(&path).map_err(|e| e.to_string())
}

fn validate_human_surface_path(path: &str) -> Result<(), String> {
    if path.is_empty() {
        return Err("Human note path cannot be empty".to_string());
    }
    if path.contains('\\') {
        return Err(format!(
            "Human note path must use forward slash separators: {}",
            path
        ));
    }

    let normalized = path.replace('\\', "/");
    let bytes = path.as_bytes();
    let has_windows_prefix = bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':';
    let candidate = Path::new(path);
    if candidate.is_absolute()
        || candidate.has_root()
        || normalized.starts_with('/')
        || has_windows_prefix
    {
        return Err(format!("Human note path must be relative: {}", path));
    }

    for segment in normalized.split('/').filter(|segment| !segment.is_empty()) {
        if segment == ".." {
            return Err(format!("Human note path cannot escape the vault: {}", path));
        }
        if segment.starts_with('.') || segment.starts_with('_') {
            return Err(format!(
                "Human note path cannot use an internal namespace: {}",
                path
            ));
        }
    }

    Ok(())
}

fn validate_human_surface_path_for_vault(
    vault: &VaultManager,
    path: &str,
) -> Result<(), String> {
    validate_human_surface_path(path)?;

    let canonical_root = std::fs::canonicalize(&vault.root_path).map_err(|error| {
        format!(
            "Failed to resolve vault root {}: {}",
            vault.root_path.display(),
            error
        )
    })?;
    let target = vault.root_path.join(path);
    let mut existing_ancestor = target.as_path();
    while !existing_ancestor.exists() {
        existing_ancestor = existing_ancestor
            .parent()
            .ok_or_else(|| format!("Human note path has no existing vault ancestor: {}", path))?;
    }

    let canonical_ancestor = std::fs::canonicalize(existing_ancestor).map_err(|error| {
        format!(
            "Failed to resolve human note path ancestor {}: {}",
            existing_ancestor.display(),
            error
        )
    })?;
    let resolved_relative = canonical_ancestor
        .strip_prefix(&canonical_root)
        .map_err(|_| format!("Human note path resolves outside the vault: {}", path))?;

    for component in resolved_relative.components() {
        if let std::path::Component::Normal(segment) = component {
            let segment = segment.to_string_lossy();
            if segment.starts_with('.') || segment.starts_with('_') {
                return Err(format!(
                    "Human note path resolves into an internal namespace: {}",
                    path
                ));
            }
        }
    }

    Ok(())
}

#[tauri::command]
pub(crate) fn save_note(
    path: String,
    content: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    validate_human_surface_path(&path)?;
    let mut app = state.lock().map_err(|e| e.to_string())?;
    save_note_for_app(&mut app, &path, &content)
}

pub(crate) fn save_note_for_app(
    app: &mut AppState,
    path: &str,
    content: &str,
) -> Result<(), String> {
    validate_human_surface_path(path)?;
    let vault = app.vault.as_ref().ok_or("No vault open")?;
    validate_human_surface_path_for_vault(vault, path)?;
    vault.write_note(path, content).map_err(|e| e.to_string())?;
    process_file_event(
        app,
        &FileEvent {
            path: path.to_string(),
            kind: FileEventKind::Modified,
            timestamp: now_millis(),
        },
    )?;
    Ok(())
}

#[tauri::command]
pub(crate) fn save_asset(
    path: String,
    content: Vec<u8>,
    state: State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    let app = state.lock().map_err(|e| e.to_string())?;
    let vault = app.vault.as_ref().ok_or("No vault open")?;
    vault.save_asset(&path, &content).map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn delete_note(path: String, state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    validate_human_surface_path(&path)?;
    let mut app = state.lock().map_err(|e| e.to_string())?;
    let vault = app.vault.as_ref().ok_or("No vault open")?;
    validate_human_surface_path_for_vault(vault, &path)?;
    vault.delete_note(&path).map_err(|e| e.to_string())?;
    process_file_event(
        &mut app,
        &FileEvent {
            path,
            kind: FileEventKind::Deleted,
            timestamp: now_millis(),
        },
    )?;
    Ok(())
}

#[tauri::command]
pub(crate) fn create_note(path: String, state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    validate_human_surface_path(&path)?;
    let mut app = state.lock().map_err(|e| e.to_string())?;
    let vault = app.vault.as_ref().ok_or("No vault open")?;
    validate_human_surface_path_for_vault(vault, &path)?;
    let title = path
        .rsplit('/')
        .next()
        .unwrap_or(&path)
        .trim_end_matches(".md");
    let template = format!("# {}\n\n", title);
    vault
        .write_note(&path, &template)
        .map_err(|e| e.to_string())?;
    process_file_event(
        &mut app,
        &FileEvent {
            path,
            kind: FileEventKind::Created,
            timestamp: now_millis(),
        },
    )?;
    Ok(())
}

#[tauri::command]
pub(crate) fn create_folder(path: String, state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    validate_human_surface_path(&path)?;
    let app = state.lock().map_err(|e| e.to_string())?;
    let vault = app.vault.as_ref().ok_or("No vault open")?;
    validate_human_surface_path_for_vault(vault, &path)?;
    vault.create_folder(&path).map_err(|e| e.to_string())
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TemplateMeta {
    pub path: String,
    pub title: String,
    pub preview: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TagSummary {
    pub name: String,
    pub count: usize,
    pub notes: Vec<String>,
}

#[tauri::command]
pub(crate) fn list_tags(state: State<'_, Mutex<AppState>>) -> Result<Vec<TagSummary>, String> {
    let app = state.lock().map_err(|e| e.to_string())?;
    let vault = app.vault.as_ref().ok_or("No vault open")?;
    let notes = vault.list_notes().map_err(|e| e.to_string())?;

    let mut tag_map: std::collections::HashMap<String, (usize, Vec<String>)> =
        std::collections::HashMap::new();
    for note in &notes {
        for tag in &note.tags {
            let entry = tag_map.entry(tag.clone()).or_insert((0, vec![]));
            entry.0 += 1;
            entry.1.push(note.path.clone());
        }
    }

    let mut summaries: Vec<TagSummary> = tag_map
        .into_iter()
        .map(|(name, (count, notes))| TagSummary { name, count, notes })
        .collect();
    summaries.sort_by_key(|s| std::cmp::Reverse(s.count));

    Ok(summaries)
}

#[tauri::command]
pub(crate) fn get_note_tags(
    note_id: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<String>, String> {
    let app = state.lock().map_err(|e| e.to_string())?;
    let vault = app.vault.as_ref().ok_or("No vault open")?;
    let notes = vault.list_notes().map_err(|e| e.to_string())?;
    let note = notes
        .iter()
        .find(|n| n.path == note_id)
        .ok_or_else(|| format!("Note not found: {}", note_id))?;
    Ok(note.tags.clone())
}

#[tauri::command]
pub(crate) fn list_templates(
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<TemplateMeta>, String> {
    let app = state.lock().map_err(|e| e.to_string())?;
    let vault = app.vault.as_ref().ok_or("No vault open")?;

    let templates_dir = vault.root_path.join("templates");
    if !templates_dir.exists() {
        return Ok(vec![]);
    }

    let mut templates = Vec::new();
    for entry in std::fs::read_dir(&templates_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.extension().map(|e| e == "md").unwrap_or(false) {
            let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
            let relative = path
                .strip_prefix(&vault.root_path)
                .map_err(|e| e.to_string())?;
            let title = relative
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let preview = content.lines().take(3).collect::<Vec<_>>().join("\n");
            templates.push(TemplateMeta {
                path: relative.to_string_lossy().to_string(),
                title,
                preview,
            });
        }
    }
    Ok(templates)
}

#[tauri::command]
pub(crate) fn rename_note(
    old_path: String,
    new_path: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    validate_human_surface_path(&old_path)?;
    validate_human_surface_path(&new_path)?;
    let mut app = state.lock().map_err(|e| e.to_string())?;
    rename_note_for_app(&mut app, &old_path, &new_path)
}

fn rename_note_for_app(app: &mut AppState, old_path: &str, new_path: &str) -> Result<(), String> {
    validate_human_surface_path(old_path)?;
    validate_human_surface_path(new_path)?;
    let vault = app.vault.as_ref().ok_or("No vault open")?;
    validate_human_surface_path_for_vault(vault, old_path)?;
    validate_human_surface_path_for_vault(vault, new_path)?;
    vault.rename_note(old_path, new_path)?;

    app.link_graph.rename_note(old_path, new_path);

    process_file_event(
        app,
        &FileEvent {
            path: old_path.to_string(),
            kind: FileEventKind::Deleted,
            timestamp: now_millis(),
        },
    )?;
    process_file_event(
        app,
        &FileEvent {
            path: new_path.to_string(),
            kind: FileEventKind::Created,
            timestamp: now_millis(),
        },
    )?;

    Ok(())
}

fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub(crate) fn process_file_event(app: &mut AppState, event: &FileEvent) -> Result<(), String> {
    if !crate::fs::watcher::is_content_markdown_path(&event.path) {
        return Ok(());
    }

    if let Some(vault) = app.vault.as_ref() {
        vault.refresh_cache().map_err(|e| e.to_string())?;
    }

    if let Some(sync_engine) = app.sync_engine.as_mut() {
        sync_engine.process_event_sync(event, &app.vault_path)?;
    }

    // Update full-text search index
    if let Some(ref engine) = app.search_engine {
        let vault_path = std::path::PathBuf::from(&app.vault_path);
        let full_path = vault_path.join(&event.path);
        let content = std::fs::read_to_string(&full_path).unwrap_or_default();
        let title = {
            let path = &event.path;
            std::path::Path::new(path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Untitled")
                .to_string()
        };
        let modified = full_path
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        match event.kind {
            FileEventKind::Created | FileEventKind::Modified => {
                if full_path.extension().map(|e| e == "md").unwrap_or(false) {
                    if let Err(e) = engine.add_document(&event.path, &title, &content, modified) {
                        tracing::warn!("FTS add_document error for {}: {}", event.path, e);
                    }
                }
            }
            FileEventKind::Deleted => {
                if let Err(e) = engine.delete_document(&event.path) {
                    tracing::warn!("FTS delete_document error for {}: {}", event.path, e);
                }
            }
            _ => {}
        }
    }

    rebuild_link_graph(app)
}

fn sync_all_notes(app: &mut AppState) -> Result<(), String> {
    let notes = if let Some(vault) = app.vault.as_ref() {
        vault.list_notes().map_err(|e| e.to_string())?
    } else {
        return Ok(());
    };

    if let Some(sync_engine) = app.sync_engine.as_mut() {
        for note in notes {
            let event = FileEvent {
                path: note.path,
                kind: FileEventKind::Modified,
                timestamp: now_millis(),
            };
            if let Err(e) = sync_engine.process_event_sync(&event, &app.vault_path) {
                tracing::warn!("Failed to index note: {}", e);
            }
        }
    }

    Ok(())
}

fn rebuild_link_graph(app: &mut AppState) -> Result<(), String> {
    let vault = if let Some(vault) = app.vault.as_ref() {
        vault
    } else {
        app.link_graph = LinkGraph::new();
        return Ok(());
    };

    let notes = vault.list_notes().map_err(|e| e.to_string())?;
    let known_paths: std::collections::HashSet<String> =
        notes.iter().map(|note| note.path.clone()).collect();
    let mut graph = LinkGraph::new();
    let mut mdt_id_to_path = std::collections::HashMap::new();
    let mut note_contents = Vec::new();

    for note in &notes {
        graph.set_title(&note.path, &note.title);
        let content = match vault.read_note(&note.path) {
            Ok(content) => content,
            Err(_) => continue,
        };
        if let Some((frontmatter, _)) = crate::parser::frontmatter::parse_frontmatter(&content) {
            if let Some(mdt) = frontmatter.mdt {
                if let Some(id) = mdt.id {
                    mdt_id_to_path.insert(id, note.path.clone());
                }
            }
        }
        note_contents.push((note.path.clone(), content));
    }

    for (note_path, content) in note_contents {
        for link in crate::parser::ast::extract_wikilinks(&content, &note_path) {
            let target = resolve_wikilink_target(&link.target, &known_paths);
            graph.add_link(&note_path, &target);
        }
        add_mdt_frontmatter_links(
            &mut graph,
            &note_path,
            &content,
            &known_paths,
            &mdt_id_to_path,
        );
    }

    add_jsonld_structure_edges(&mut graph, &vault.root_path);

    if let Some(dream_engine) = app.dream_engine.as_ref() {
        for edge in dream_engine.export_graph_edges() {
            graph.add_graph_edge(edge);
        }
    }

    app.link_graph = graph;
    Ok(())
}

fn add_jsonld_structure_edges(graph: &mut LinkGraph, vault_root: &std::path::Path) {
    let Ok(index) = crate::jsonld::indexer::index_vault(vault_root) else {
        return;
    };
    let id_to_source = index
        .nodes
        .iter()
        .map(|node| (node.id.clone(), node.source_path.clone()))
        .collect::<std::collections::HashMap<_, _>>();

    for edge in index.edges {
        if !matches!(edge.origin.as_str(), "tree") {
            continue;
        }
        let Some(source) = id_to_source.get(&edge.source) else {
            continue;
        };
        let target = id_to_source
            .get(&edge.target)
            .cloned()
            .unwrap_or(edge.target.clone());
        graph.add_graph_edge(GraphEdge {
            from: source.clone(),
            to: target,
            edge_type: graph_edge_type_from_mdt(&edge.edge_type),
            origin: "jsonld".to_string(),
            confidence: edge.confidence,
            weight: None,
            memory_region: Some("semantic".to_string()),
        });
    }
}

fn add_mdt_frontmatter_links(
    graph: &mut LinkGraph,
    source_path: &str,
    content: &str,
    known_paths: &std::collections::HashSet<String>,
    id_to_path: &std::collections::HashMap<String, String>,
) {
    let Some((frontmatter, _)) = crate::parser::frontmatter::parse_frontmatter(content) else {
        return;
    };
    let Some(mdt) = frontmatter.mdt else {
        return;
    };

    for link in mdt.links {
        let target = resolve_mdt_link_target(&link.target, known_paths, id_to_path);
        graph.add_typed_link(
            source_path,
            &target,
            graph_edge_type_from_mdt(&link.edge_type),
            "frontmatter",
            link.confidence.unwrap_or(1.0),
        );
    }
}

fn resolve_mdt_link_target(
    target: &str,
    known_paths: &std::collections::HashSet<String>,
    id_to_path: &std::collections::HashMap<String, String>,
) -> String {
    let normalized = target.trim().replace('\\', "/");
    if let Some(path) = id_to_path.get(&normalized) {
        return path.clone();
    }
    if known_paths.contains(&normalized) {
        return normalized;
    }
    resolve_wikilink_target(&normalized, known_paths)
}

fn graph_edge_type_from_mdt(edge_type: &str) -> GraphEdgeType {
    match edge_type.trim().to_lowercase().as_str() {
        "parent" => GraphEdgeType::Parent,
        "reference" => GraphEdgeType::Reference,
        "related" => GraphEdgeType::Related,
        "source" => GraphEdgeType::Source,
        "sequence" => GraphEdgeType::Sequence,
        "semantic" => GraphEdgeType::Semantic,
        "temporal" => GraphEdgeType::Temporal,
        "bridge" => GraphEdgeType::Bridge,
        _ => GraphEdgeType::Reference,
    }
}

fn resolve_wikilink_target(
    target: &str,
    known_paths: &std::collections::HashSet<String>,
) -> String {
    let normalized = target.trim().replace('\\', "/");
    if normalized.is_empty() {
        return normalized;
    }

    let with_ext = if normalized.ends_with(".md") {
        normalized
    } else {
        format!("{}.md", normalized)
    };

    if known_paths.contains(&with_ext) {
        return with_ext;
    }

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
mod mdt_graph_tests {
    use super::*;
    use crate::graph::link_graph::{GraphEdgeType, LinkGraph};
    use std::collections::{HashMap, HashSet};

    #[test]
    fn frontmatter_links_are_added_as_typed_edges() {
        let content = "---\nmdt_version: \"0.1.0\"\nid: root\ntitle: Root\ntree:\n  parent: null\n  order: 0\nlinks:\n  - target: child\n    type: related\n    confidence: 0.7\n---\n# Root\n";
        let known_paths = HashSet::from(["root.md".to_string(), "child.md".to_string()]);
        let id_to_path = HashMap::from([("child".to_string(), "child.md".to_string())]);
        let mut graph = LinkGraph::new();

        add_mdt_frontmatter_links(&mut graph, "root.md", content, &known_paths, &id_to_path);

        let graph_data = graph.to_frontend_json();
        assert_eq!(graph_data.edges.len(), 1);
        assert_eq!(graph_data.edges[0].from, "root.md");
        assert_eq!(graph_data.edges[0].to, "child.md");
        assert_eq!(graph_data.edges[0].edge_type, GraphEdgeType::Related);
        assert_eq!(graph_data.edges[0].origin, "frontmatter");
        assert_eq!(graph_data.edges[0].confidence, 0.7);
    }

    #[test]
    fn jsonld_structure_edges_are_tagged_as_semantic_memory_region() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("root.md"),
            "---\nmdt_version: \"0.1.0\"\nid: root\ntitle: Root\ntree:\n  parent: null\n  order: 0\nlinks:\n  - target: child\n    type: parent\n---\n# Root\n",
        )
        .unwrap();
        std::fs::write(
            temp.path().join("child.md"),
            "---\nmdt_version: \"0.1.0\"\nid: child\ntitle: Child\ntree:\n  parent: root\n  order: 1\n---\n# Child\n",
        )
        .unwrap();
        let mut graph = LinkGraph::new();

        add_jsonld_structure_edges(&mut graph, temp.path());

        let graph_data = graph.to_frontend_json();
        assert!(graph_data.edges.iter().any(|edge| {
            edge.origin == "jsonld" && edge.memory_region.as_deref() == Some("semantic")
        }));
    }
}

#[cfg(test)]
mod process_file_event_tests {
    use super::*;
    use crate::ai::agent_scheduler::AgentScheduler;
    use crate::ai::embedding::{EmbeddingBackend, EmbeddingPipeline};
    use crate::ai::llm_router::LlmRouter;
    use crate::ai::search_engine::SearchEngine;
    use crate::ai::vectordb::VectorStore;
    use crate::harness::workflow::{
        AgentRegistry, AgentRegistryEntry, ControlPolicy, GoalAlignment, GoalContract, RetryPolicy,
        WorkflowIr, WorkflowStatus, WorkflowStep, WorkflowStepKind, WorkflowStepMode,
        WorkflowStepStatus,
    };
    use crate::harness::workflow_runtime::WorkflowDispatchStatus;
    use crate::ipc::protocol::TwoSurfaceProtocol;
    use serde_json::json;
    use tempfile::TempDir;
    use tokio::sync::Notify;

    #[cfg(unix)]
    fn create_directory_link(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::unix::fs::symlink(target, link)
    }

    #[cfg(windows)]
    fn create_directory_link(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::windows::fs::symlink_dir(target, link).or_else(|symlink_error| {
            let output = std::process::Command::new("cmd")
                .arg("/C")
                .arg("mklink")
                .arg("/J")
                .arg(link)
                .arg(target)
                .output()?;
            if output.status.success() {
                Ok(())
            } else {
                Err(symlink_error)
            }
        })
    }

    #[cfg(not(any(unix, windows)))]
    fn create_directory_link(_target: &Path, _link: &Path) -> std::io::Result<()> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "directory links are not supported on this platform",
        ))
    }

    fn empty_app_state() -> AppState {
        AppState {
            vault: None,
            file_watcher: None,
            link_graph: LinkGraph::new(),
            vault_path: String::new(),
            sync_engine: None,
            vector_store_path: PathBuf::new(),
            dualtrack_dir: PathBuf::new(),
            snapshot_engine: None,
            dream_engine: None,
            protocol: Some(TwoSurfaceProtocol::default()),
            search_engine: None,
            output_manager: None,
            bridge_store: None,
            snapshot_listeners_started: false,
        }
    }

    fn empty_ai_state() -> AiState {
        AiState {
            agent_scheduler: AgentScheduler::new(3),
            llm_router: LlmRouter::new(Default::default()),
            embedding_pipeline: EmbeddingPipeline::new(EmbeddingBackend::None),
            ghost_store: GhostStore::new(&PathBuf::from(".dualtrack")),
            subagent: None,
            skill_manager: None,
            scheduler: None,
            task_notifier: Arc::new(Notify::new()),
            task_worker_started: false,
        }
    }

    fn workflow_goal() -> GoalContract {
        GoalContract {
            goal_id: "goal_boot".to_string(),
            goal_text: "Resume a persisted workflow run at vault startup".to_string(),
            success_definition: vec!["A queued workflow task is restored".to_string()],
            non_goals: vec![],
            constraints: json!({}),
            context_scope: vec!["content/**".to_string()],
            approval_policy: json!({}),
            budget: json!({"max_iterations": 1}),
            created_at: "2026-06-23T00:00:00Z".to_string(),
        }
    }

    fn workflow_registry() -> AgentRegistry {
        AgentRegistry::from_agents(vec![AgentRegistryEntry {
            agent_type: "research_subagent".to_string(),
            allowed_tools: vec!["Read".to_string(), "arxiv_search".to_string()],
            denied_tools: vec!["Write".to_string()],
            default_mode: WorkflowStepMode::ReadOnly,
            max_parallelism: 1,
            can_delegate: false,
        }])
    }

    fn workflow_with_ready_research() -> WorkflowIr {
        WorkflowIr {
            workflow_id: "wf_boot".to_string(),
            goal_id: "goal_boot".to_string(),
            version: 1,
            parent_version: None,
            status: WorkflowStatus::Running,
            global_context: json!({}),
            control_policy: ControlPolicy {
                max_parallel_steps: 1,
                replan_on_verification_fail: true,
                max_patch_chain: 1,
            },
            steps: vec![WorkflowStep {
                step_id: "S001".to_string(),
                title: "Restore queued research task".to_string(),
                kind: WorkflowStepKind::Research,
                agent_type: "research_subagent".to_string(),
                mode: WorkflowStepMode::ReadOnly,
                task: "Return a startup recovery report".to_string(),
                inputs: json!({"question": "Can startup restore this run?"}),
                dependencies: vec![],
                acceptance_criteria: vec!["Task is visible in scheduler".to_string()],
                goal_alignment: GoalAlignment {
                    success_clauses: vec![1],
                    why_necessary: "Startup recovery is the narrow runtime loop".to_string(),
                },
                retry_policy: RetryPolicy {
                    max_attempts: 1,
                    backoff_ms: 0,
                },
                status: WorkflowStepStatus::Ready,
            }],
            created_by: "orchestrator@v1".to_string(),
            created_at: "2026-06-23T00:00:01Z".to_string(),
        }
    }

    fn test_app_state(vault_dir: &std::path::Path) -> AppState {
        let dualtrack_dir = vault_dir.join(".dualtrack");
        std::fs::create_dir_all(&dualtrack_dir).unwrap();
        let vault = VaultManager::open(vault_dir).unwrap();
        let mut vector_store =
            VectorStore::open(dualtrack_dir.join("vectors").to_str().unwrap()).unwrap();
        vector_store.set_dimension(384);
        let sync_engine =
            SyncEngine::new(vector_store, EmbeddingPipeline::new(EmbeddingBackend::None));
        let search_engine = Arc::new(SearchEngine::new(vault_dir).unwrap());

        AppState {
            vault: Some(vault),
            file_watcher: None,
            link_graph: LinkGraph::new(),
            vault_path: vault_dir.to_string_lossy().to_string(),
            sync_engine: Some(sync_engine),
            vector_store_path: dualtrack_dir.join("vectors"),
            dualtrack_dir,
            snapshot_engine: None,
            dream_engine: Some(DreamEngine::new()),
            protocol: Some(TwoSurfaceProtocol::default()),
            search_engine: Some(search_engine),
            output_manager: None,
            bridge_store: None,
            snapshot_listeners_started: false,
        }
    }

    #[test]
    fn initialize_vault_services_wires_core_backend_state_and_indexes_content_only() {
        let dir = TempDir::new().unwrap();
        let vault_dir = dir.path();
        std::fs::write(vault_dir.join("target.md"), "# Target\n").unwrap();
        std::fs::write(
            vault_dir.join("source.md"),
            "# Source\n\nneedle-open-vault [[target]]\n",
        )
        .unwrap();
        std::fs::create_dir_all(vault_dir.join("_private")).unwrap();
        std::fs::write(
            vault_dir.join("_private").join("secret.md"),
            "needle-secret",
        )
        .unwrap();
        std::fs::create_dir_all(vault_dir.join(".dualtrack").join("research")).unwrap();
        std::fs::write(
            vault_dir
                .join(".dualtrack")
                .join("research")
                .join("result.md"),
            "needle-internal",
        )
        .unwrap();

        let mut app = empty_app_state();
        let mut ai = empty_ai_state();

        initialize_vault_services(&mut app, &mut ai, &vault_dir.to_string_lossy()).unwrap();

        assert_eq!(app.vault_path, vault_dir.to_string_lossy());
        assert_eq!(app.dualtrack_dir, vault_dir.join(".dualtrack"));
        assert_eq!(
            app.vector_store_path,
            vault_dir.join(".dualtrack").join("vectors")
        );
        assert!(app.vault.is_some());
        assert!(app.sync_engine.is_some());
        assert!(app.bridge_store.is_some());
        assert!(app.snapshot_engine.is_some());
        assert!(app.dream_engine.is_some());
        assert!(app.protocol.is_some());
        assert!(app.search_engine.is_some());
        assert!(app.output_manager.is_some());
        assert!(ai.subagent.is_some());
        assert!(ai.skill_manager.is_some());
        assert!(vault_dir.join(".dualtrack").join("ghosts").exists());
        assert!(vault_dir
            .join(".dualtrack")
            .join("memory")
            .join("working")
            .exists());
        assert!(vault_dir
            .join(".dualtrack")
            .join("memory")
            .join("semantic")
            .exists());
        assert!(vault_dir
            .join(".dualtrack")
            .join("memory")
            .join("long_term")
            .exists());

        assert!(!app
            .sync_engine
            .as_ref()
            .unwrap()
            .store()
            .chunks_for_file("source.md")
            .is_empty());
        assert_eq!(
            app.search_engine
                .as_ref()
                .unwrap()
                .search("needle-open-vault", 5)
                .unwrap()[0]
                .path,
            "source.md"
        );
        assert!(app
            .search_engine
            .as_ref()
            .unwrap()
            .search("needle-secret", 5)
            .unwrap()
            .is_empty());
        assert!(app
            .search_engine
            .as_ref()
            .unwrap()
            .search("needle-internal", 5)
            .unwrap()
            .is_empty());
        let graph = app.link_graph.to_frontend_json();
        assert!(graph
            .edges
            .iter()
            .any(|edge| edge.from == "source.md" && edge.to == "target.md"));
    }

    #[test]
    fn initialize_vault_services_auto_resumes_workflow_runtime_runs_and_skips_corrupt() {
        let dir = TempDir::new().unwrap();
        let vault_dir = dir.path();
        std::fs::write(vault_dir.join("source.md"), "# Source\n").unwrap();
        let mut seeded_scheduler = AgentScheduler::new(1);
        WorkflowRuntimeService::new(vault_dir)
            .start(
                workflow_goal(),
                workflow_with_ready_research(),
                workflow_registry(),
                "run_boot",
                &mut seeded_scheduler,
                100,
            )
            .unwrap();
        let bad_run_dir = vault_dir.join(".harness").join("runs").join("run_bad");
        std::fs::create_dir_all(&bad_run_dir).unwrap();
        std::fs::write(bad_run_dir.join("runtime.json"), b"{\"broken\": true").unwrap();
        let mut app = empty_app_state();
        let mut ai = empty_ai_state();

        initialize_vault_services(&mut app, &mut ai, &vault_dir.to_string_lossy()).unwrap();

        let task = ai
            .agent_scheduler
            .get_task("workflow__run_boot__S001__attempt_1")
            .expect("startup should recover the persisted workflow task");
        assert!(matches!(task.status, TaskStatus::Approved { .. }));
        let bundle = WorkflowRuntimeService::new(vault_dir)
            .get("run_boot")
            .unwrap();
        assert_eq!(bundle.dispatches.len(), 1);
        assert_eq!(bundle.dispatches[0].status, WorkflowDispatchStatus::Queued);
        assert_eq!(
            bundle.dispatches[0].task_id.as_deref(),
            Some("workflow__run_boot__S001__attempt_1")
        );
        let events = crate::harness::workflow::WorkflowRuntimeEventStore::read_recent(
            vault_dir, "run_boot", 20,
        )
        .unwrap();
        assert!(events
            .iter()
            .any(|event| event.event_name == "workflow.run.resumed"));
    }

    #[test]
    fn process_file_event_updates_vector_fts_and_graph_for_content_note() {
        let dir = TempDir::new().unwrap();
        let vault_dir = dir.path();
        std::fs::write(vault_dir.join("target.md"), "# Target\n").unwrap();
        std::fs::write(
            vault_dir.join("source.md"),
            "# Source\n\nneedle-runtime-flow [[target]]\n",
        )
        .unwrap();
        let mut app = test_app_state(vault_dir);

        process_file_event(
            &mut app,
            &FileEvent {
                path: "source.md".to_string(),
                kind: FileEventKind::Modified,
                timestamp: 1,
            },
        )
        .unwrap();

        assert!(!app
            .sync_engine
            .as_ref()
            .unwrap()
            .store()
            .chunks_for_file("source.md")
            .is_empty());
        assert_eq!(
            app.search_engine
                .as_ref()
                .unwrap()
                .search("needle-runtime-flow", 5)
                .unwrap()[0]
                .path,
            "source.md"
        );
        let graph = app.link_graph.to_frontend_json();
        assert!(graph
            .edges
            .iter()
            .any(|edge| edge.from == "source.md" && edge.to == "target.md"));
    }

    #[test]
    fn process_file_event_ignores_dualtrack_internal_markdown() {
        let dir = TempDir::new().unwrap();
        let vault_dir = dir.path();
        let internal_dir = vault_dir.join(".dualtrack").join("research");
        std::fs::create_dir_all(&internal_dir).unwrap();
        std::fs::write(
            internal_dir.join("result.md"),
            "# Internal\n\nneedle-internal\n",
        )
        .unwrap();
        let mut app = test_app_state(vault_dir);

        process_file_event(
            &mut app,
            &FileEvent {
                path: ".dualtrack/research/result.md".to_string(),
                kind: FileEventKind::Modified,
                timestamp: 1,
            },
        )
        .unwrap();

        assert!(app
            .sync_engine
            .as_ref()
            .unwrap()
            .store()
            .chunks_for_file(".dualtrack/research/result.md")
            .is_empty());
        assert!(app
            .search_engine
            .as_ref()
            .unwrap()
            .search("needle-internal", 5)
            .unwrap()
            .is_empty());
        assert!(app.link_graph.to_frontend_json().nodes.is_empty());
    }

    #[test]
    fn list_ai_workspace_files_exposes_dualtrack_without_polluting_human_notes() {
        let dir = TempDir::new().unwrap();
        let vault_dir = dir.path();
        std::fs::write(vault_dir.join("human.md"), "# Human\n").unwrap();
        let working_dir = vault_dir.join(".dualtrack").join("memory").join("working");
        let semantic_dir = vault_dir.join(".dualtrack").join("jsonld").join("indexes");
        std::fs::create_dir_all(&working_dir).unwrap();
        std::fs::create_dir_all(&semantic_dir).unwrap();
        std::fs::write(working_dir.join("task.md"), "# Task\n").unwrap();
        std::fs::write(semantic_dir.join("claims.md"), "# Claims\n").unwrap();

        let vault = VaultManager::open(vault_dir).unwrap();
        let human_notes = vault.list_notes().unwrap();
        assert_eq!(human_notes.len(), 1);
        assert_eq!(human_notes[0].path, "human.md");

        let ai_notes = list_ai_workspace_files_for_root(vault_dir).unwrap();
        let paths = ai_notes
            .iter()
            .map(|note| note.path.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            paths,
            vec![
                ".dualtrack/jsonld/indexes/claims.md",
                ".dualtrack/memory/working/task.md",
            ]
        );
    }

    #[test]
    fn list_content_folders_exposes_empty_human_folders_only() {
        let dir = TempDir::new().unwrap();
        let vault_dir = dir.path();
        std::fs::create_dir_all(vault_dir.join("Projects").join("Ideas")).unwrap();
        std::fs::create_dir_all(vault_dir.join(".dualtrack").join("memory")).unwrap();
        std::fs::create_dir_all(vault_dir.join("_private")).unwrap();

        let folders = list_content_folders_for_root(vault_dir).unwrap();
        let paths = folders
            .iter()
            .map(|folder| folder.path.as_str())
            .collect::<Vec<_>>();

        assert_eq!(paths, vec!["Projects", "Projects/Ideas"]);
    }

    #[test]
    fn save_note_for_human_surface_writes_and_reads_back_content() {
        let dir = TempDir::new().unwrap();
        let vault_dir = dir.path();
        let mut app = test_app_state(vault_dir);

        save_note_for_app(&mut app, "human-edit.md", "# Human\n\nroundtrip").unwrap();

        let saved = app
            .vault
            .as_ref()
            .unwrap()
            .read_note("human-edit.md")
            .unwrap();
        assert_eq!(saved, "# Human\n\nroundtrip");
    }

    #[test]
    fn validate_human_surface_path_allows_normal_relative_paths() {
        assert!(validate_human_surface_path("notes/project.md").is_ok());
        assert!(validate_human_surface_path("Projects/Ideas").is_ok());
    }

    #[test]
    fn validate_human_surface_path_rejects_internal_namespaces() {
        assert!(validate_human_surface_path(".dualtrack/research/result.md").is_err());
        assert!(validate_human_surface_path(".harness/runs/state.md").is_err());
    }

    #[test]
    fn validate_human_surface_path_rejects_nested_hidden_and_private_segments() {
        assert!(validate_human_surface_path("notes/.archive/result.md").is_err());
        assert!(validate_human_surface_path("notes/_private/result.md").is_err());
    }

    #[test]
    fn validate_human_surface_path_rejects_absolute_parent_and_empty_paths() {
        assert!(validate_human_surface_path(r"C:\vault\note.md").is_err());
        assert!(validate_human_surface_path("/vault/note.md").is_err());
        assert!(validate_human_surface_path("notes/../secret.md").is_err());
        assert!(validate_human_surface_path("").is_err());
    }

    #[test]
    fn validate_human_surface_path_rejects_backslash_separators() {
        assert!(validate_human_surface_path(r"notes\project.md").is_err());
    }

    #[test]
    fn save_note_for_human_surface_rejects_backslash_without_writing() {
        let dir = TempDir::new().unwrap();
        let vault_dir = dir.path();
        let mut app = test_app_state(vault_dir);

        let result = save_note_for_app(&mut app, r"notes\blocked.md", "# Blocked\n");

        assert!(result.is_err());
        assert!(!vault_dir.join(r"notes\blocked.md").exists());
    }

    #[test]
    fn save_note_for_human_surface_allows_new_nested_relative_path() {
        let dir = TempDir::new().unwrap();
        let vault_dir = dir.path();
        let mut app = test_app_state(vault_dir);

        save_note_for_app(&mut app, "new/branch/note.md", "# Human\n").unwrap();

        assert!(vault_dir.join("new").join("branch").join("note.md").exists());
    }

    #[test]
    fn save_note_for_human_surface_rejects_external_directory_link_without_writing() {
        let vault_dir = TempDir::new().unwrap();
        let outside_dir = TempDir::new().unwrap();
        let link = vault_dir.path().join("alias");
        if let Err(error) = create_directory_link(outside_dir.path(), &link) {
            eprintln!("skipping directory-link test: {}", error);
            return;
        }
        let mut app = test_app_state(vault_dir.path());

        let result = save_note_for_app(&mut app, "alias/generated.md", "# Blocked\n");

        assert!(result.is_err());
        assert!(!outside_dir.path().join("generated.md").exists());
    }

    #[test]
    fn save_note_for_human_surface_rejects_link_to_internal_namespace_without_writing() {
        let vault_dir = TempDir::new().unwrap();
        let internal_dir = vault_dir.path().join(".dualtrack");
        std::fs::create_dir_all(&internal_dir).unwrap();
        let link = vault_dir.path().join("alias");
        if let Err(error) = create_directory_link(&internal_dir, &link) {
            eprintln!("skipping directory-link test: {}", error);
            return;
        }
        let mut app = test_app_state(vault_dir.path());

        let result = save_note_for_app(&mut app, "alias/generated.md", "# Blocked\n");

        assert!(result.is_err());
        assert!(!internal_dir.join("generated.md").exists());
    }

    #[test]
    fn rename_note_for_app_rejects_invalid_human_surface_old_path_before_moving() {
        let dir = TempDir::new().unwrap();
        let vault_dir = dir.path();
        let internal_dir = vault_dir.join(".dualtrack");
        std::fs::create_dir_all(&internal_dir).unwrap();
        std::fs::write(internal_dir.join("source.md"), "# Internal\n").unwrap();
        let mut app = empty_app_state();
        app.vault = Some(VaultManager::open(vault_dir).unwrap());

        let result = rename_note_for_app(&mut app, ".dualtrack/source.md", "renamed-human.md");

        assert!(result.is_err());
        assert!(internal_dir.join("source.md").exists());
        assert!(!vault_dir.join("renamed-human.md").exists());
    }

    #[test]
    fn rename_note_for_app_rejects_invalid_human_surface_new_path_before_moving() {
        let dir = TempDir::new().unwrap();
        let vault_dir = dir.path();
        std::fs::write(vault_dir.join("source.md"), "# Human\n").unwrap();
        let mut app = empty_app_state();
        app.vault = Some(VaultManager::open(vault_dir).unwrap());

        let result = rename_note_for_app(&mut app, "source.md", ".harness/moved.md");

        assert!(result.is_err());
        assert!(vault_dir.join("source.md").exists());
        assert!(!vault_dir.join(".harness").join("moved.md").exists());
    }

    #[test]
    fn rename_note_for_human_surface_rejects_linked_new_path_before_moving() {
        let vault_dir = TempDir::new().unwrap();
        let outside_dir = TempDir::new().unwrap();
        std::fs::write(vault_dir.path().join("source.md"), "# Human\n").unwrap();
        let link = vault_dir.path().join("alias");
        if let Err(error) = create_directory_link(outside_dir.path(), &link) {
            eprintln!("skipping directory-link test: {}", error);
            return;
        }
        let mut app = empty_app_state();
        app.vault = Some(VaultManager::open(vault_dir.path()).unwrap());

        let result = rename_note_for_app(&mut app, "source.md", "alias/moved.md");

        assert!(result.is_err());
        assert!(vault_dir.path().join("source.md").exists());
        assert!(!outside_dir.path().join("moved.md").exists());
    }
}

#[allow(dead_code)]
pub fn register_commands(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    builder.invoke_handler(tauri::generate_handler![
        ping,
        open_vault,
        get_vault_path,
        list_notes,
        list_folders,
        list_ai_workspace_files,
        read_note,
        save_note,
        save_asset,
        create_note,
        delete_note,
        create_folder,
        rename_note,
        list_templates,
        list_tags,
        get_note_tags,
    ])
}
