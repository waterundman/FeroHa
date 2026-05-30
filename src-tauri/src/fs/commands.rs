use crate::ai::agent_scheduler::{AgentTask, SynthesizePhase, TaskPriority, TaskStatus, TaskType};
use crate::ai::dream_engine::DreamEngine;
use crate::ai::embedding::EmbeddingPipeline;
use crate::ai::search_engine::SearchEngine;
use crate::ai::skill_manager::SkillManager;
use crate::ai::subagent::Subagent;
use crate::ai::sync_engine::SyncEngine;
use crate::ai::task_scheduler::{CronJob, TaskScheduler};
use crate::ai::vectordb::VectorStore;
use crate::cli::parser::CliCommand;
use crate::diff::ghost_store::GhostStore;
use crate::fs::vault::VaultManager;
use crate::fs::watcher::{FileEvent, FileEventKind, FileWatcher};
use crate::graph::link_graph::{GraphEdgeType, LinkGraph};
use crate::snapshot::engine::SnapshotEngine;
use crate::snapshot::store::SnapshotStore;
use crate::AiState;
use crate::AppState;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Listener, Manager, State};

#[tauri::command]
pub(crate) fn ping() -> String {
    "pong".to_string()
}

#[tauri::command]
pub(crate) fn open_vault(
    path: String,
    state: State<'_, Mutex<AppState>>,
    ai_state: State<'_, Mutex<AiState>>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let mut app = state.lock().map_err(|e| e.to_string())?;
    let dualtrack_dir = PathBuf::from(&path).join(".dualtrack");
    std::fs::create_dir_all(&dualtrack_dir).map_err(|e| e.to_string())?;

    let vault = VaultManager::open(&path).map_err(|e| e.to_string())?;
    app.vault_path = path.clone();
    app.dualtrack_dir = dualtrack_dir.clone();
    app.bridge_store = Some(crate::bridge::store::store_for_dualtrack_dir(
        &dualtrack_dir,
    ));
    app.vector_store_path = dualtrack_dir.join("vectors");
    app.vault = Some(vault);

    let db_path = app.vector_store_path.to_str().unwrap_or(":memory:");
    let mut vector_store = VectorStore::open(db_path).map_err(|e| e.to_string())?;

    let ai = ai_state.lock().map_err(|e| e.to_string())?;
    let embed_cfg = &ai.embedding_pipeline;
    let dim = embed_cfg.dimension();
    drop(ai);
    vector_store.set_dimension(dim);

    let embed_pipe = {
        let ai = ai_state.lock().map_err(|e| e.to_string())?;
        EmbeddingPipeline::new(ai.embedding_pipeline.backend_config())
    };
    let sync_engine = SyncEngine::new(vector_store, embed_pipe);
    app.sync_engine = Some(sync_engine);

    let mut ai = ai_state.lock().map_err(|e| e.to_string())?;
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
    ai.skill_manager = Some(SkillManager::new(vault_path, "feroha".to_string()));

    let embed_backend = ai.embedding_pipeline.backend_config();
    drop(ai);

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

    let watch_path = path.clone();
    let worker_handle = app_handle.clone();
    sync_all_notes(&mut app)?;
    rebuild_link_graph(&mut app)?;

    if let Ok(watcher) = FileWatcher::watch(&watch_path) {
        let mut rx = watcher.subscribe();
        tokio::spawn(async move {
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
        tracing::info!("File watcher started for: {}", watch_path);

        // Initialize full-text search engine
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
    } else {
        tracing::warn!("Failed to start file watcher");
    }

    // Listen for NOTE_OPENED -> global snapshot
    let note_handle = app_handle.clone();
    let note_vault_path = path.clone();
    tauri::async_runtime::spawn(async move {
        let _ = note_handle.listen_any("note-opened", {
            let h = note_handle.clone();
            let p = note_vault_path.clone();
            move |event| {
                if let Ok(data) = serde_json::from_str::<serde_json::Value>(event.payload()) {
                    let note_id = data["note_id"].as_str().unwrap_or("");
                    crate::snapshot::commands::handle_global_snapshot(note_id, &p, &h);
                }
            }
        });
    });

    // Listen for SELECTION_SUBMIT -> local snapshot
    let sel_handle = app_handle.clone();
    let sel_vault_path = path.clone();
    tauri::async_runtime::spawn(async move {
        let _ = sel_handle.listen_any("selection-submit", {
            let h = sel_handle.clone();
            let p = sel_vault_path.clone();
            move |event| {
                if let Ok(data) = serde_json::from_str::<serde_json::Value>(event.payload()) {
                    let note_id = data["note_id"].as_str().unwrap_or("");
                    let start = data["start"].as_u64().unwrap_or(0) as usize;
                    let end = data["end"].as_u64().unwrap_or(0) as usize;
                    crate::snapshot::commands::handle_local_snapshot(note_id, start, end, &p, &h);
                }
            }
        });
    });

    // Start background task worker
    crate::ai::commands::start_task_worker(app_handle.clone());

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
        let mut ai = ai_state.lock().unwrap();
        ai.scheduler = Some(std::sync::Arc::new(scheduler));
    }

    let protocol = crate::ipc::protocol::TwoSurfaceProtocol::new();
    app.protocol = Some(protocol);

    let output_manager = crate::harness::output_hook::OutputManager::load_defaults(&dualtrack_dir);
    app.output_manager = Some(std::sync::Arc::new(output_manager));

    Ok(())
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
) -> Result<Vec<crate::fs::vault::NoteMeta>, String> {
    let app = state.lock().map_err(|e| e.to_string())?;
    let vault = app.vault.as_ref().ok_or("No vault open")?;
    vault.list_notes().map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn read_note(path: String, state: State<'_, Mutex<AppState>>) -> Result<String, String> {
    let app = state.lock().map_err(|e| e.to_string())?;
    let vault = app.vault.as_ref().ok_or("No vault open")?;
    vault.read_note(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn save_note(
    path: String,
    content: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    let mut app = state.lock().map_err(|e| e.to_string())?;
    let vault = app.vault.as_ref().ok_or("No vault open")?;
    vault
        .write_note(&path, &content)
        .map_err(|e| e.to_string())?;
    process_file_event(
        &mut app,
        &FileEvent {
            path,
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
    let mut app = state.lock().map_err(|e| e.to_string())?;
    let vault = app.vault.as_ref().ok_or("No vault open")?;
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
    let mut app = state.lock().map_err(|e| e.to_string())?;
    let vault = app.vault.as_ref().ok_or("No vault open")?;
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
    let app = state.lock().map_err(|e| e.to_string())?;
    let vault = app.vault.as_ref().ok_or("No vault open")?;
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
    let mut app = state.lock().map_err(|e| e.to_string())?;
    let vault = app.vault.as_ref().ok_or("No vault open")?;
    vault.rename_note(&old_path, &new_path)?;

    app.link_graph.rename_note(&old_path, &new_path);

    process_file_event(
        &mut app,
        &FileEvent {
            path: old_path.clone(),
            kind: FileEventKind::Deleted,
            timestamp: now_millis(),
        },
    )?;
    process_file_event(
        &mut app,
        &FileEvent {
            path: new_path,
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

fn process_file_event(app: &mut AppState, event: &FileEvent) -> Result<(), String> {
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

    app.link_graph = graph;
    Ok(())
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
}

#[allow(dead_code)]
pub fn register_commands(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    builder.invoke_handler(tauri::generate_handler![
        ping,
        open_vault,
        get_vault_path,
        list_notes,
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
