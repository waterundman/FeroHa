use crate::snapshot;
use crate::snapshot::Snapshot;
use crate::AppState;
use std::sync::Mutex;
use tauri::{AppHandle, Manager, State};

#[tauri::command]
pub(crate) fn get_current_snapshot(
    note_id: String,
    snapshot_type: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<Option<snapshot::Snapshot>, String> {
    let app = state.lock().map_err(|e| e.to_string())?;
    let engine = app.snapshot_engine.as_ref().ok_or("No snapshot engine")?;

    let st = match snapshot_type.as_str() {
        "global" => snapshot::SnapshotType::Global,
        "local" => snapshot::SnapshotType::Local,
        _ => return Err("Invalid snapshot_type: use 'global' or 'local'".to_string()),
    };

    Ok(engine.get_current(&note_id, &st))
}

#[tauri::command]
pub(crate) fn get_snapshot_diff(
    note_id: String,
    snapshot_type: String,
    ts1: Option<i64>,
    ts2: Option<i64>,
    state: State<'_, Mutex<AppState>>,
) -> Result<snapshot::DriftResult, String> {
    let app = state.lock().map_err(|e| e.to_string())?;
    let engine = app.snapshot_engine.as_ref().ok_or("No snapshot engine")?;

    let st = match snapshot_type.as_str() {
        "global" => snapshot::SnapshotType::Global,
        "local" => snapshot::SnapshotType::Local,
        _ => return Err("Invalid snapshot_type: use 'global' or 'local'".to_string()),
    };

    engine.get_diff(&note_id, &st, ts1, ts2)
}

pub fn handle_global_snapshot<R: tauri::Runtime>(
    note_id: &str,
    _vault_path: &str,
    app_handle: &AppHandle<R>,
) {
    let state = app_handle.state::<Mutex<AppState>>();
    let app = match state.lock() {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("Failed to lock state: {}", e);
            return;
        }
    };

    if let Err(e) = create_global_snapshot_from_state(&app, note_id, Some(app_handle)) {
        tracing::warn!("Failed to create global snapshot for {}: {}", note_id, e);
    }
}

pub(crate) fn create_global_snapshot_from_state<R: tauri::Runtime>(
    app: &AppState,
    note_id: &str,
    app_handle: Option<&AppHandle<R>>,
) -> Result<Snapshot, String> {
    let snapshot_engine = app.snapshot_engine.as_ref().ok_or("No snapshot engine")?;
    let vault = app.vault.as_ref().ok_or("No vault open")?;
    let content = vault
        .read_note(note_id)
        .map_err(|e| format!("Failed to read note {}: {}", note_id, e))?;

    let backlinks: Vec<String> = app.link_graph.get_backlinks(note_id);
    let backlink_contents: Vec<String> = backlinks
        .iter()
        .filter_map(|bl| vault.read_note(bl).ok())
        .collect();
    let mut all_backlinks = backlinks.clone();
    all_backlinks.extend(backlink_contents);

    snapshot_engine.create_global_snapshot_with_emitter(
        note_id,
        &content,
        &all_backlinks,
        app_handle,
    )
}

pub fn handle_local_snapshot<R: tauri::Runtime>(
    note_id: &str,
    start: usize,
    end: usize,
    _vault_path: &str,
    app_handle: &AppHandle<R>,
) {
    let state = app_handle.state::<Mutex<AppState>>();
    let app = match state.lock() {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("Failed to lock state: {}", e);
            return;
        }
    };

    if let Err(e) = create_local_snapshot_from_state(&app, note_id, start, end, Some(app_handle)) {
        tracing::warn!("Failed to create local snapshot for {}: {}", note_id, e);
    }
}

pub(crate) fn create_local_snapshot_from_state<R: tauri::Runtime>(
    app: &AppState,
    note_id: &str,
    start: usize,
    end: usize,
    app_handle: Option<&AppHandle<R>>,
) -> Result<Snapshot, String> {
    let snapshot_engine = app.snapshot_engine.as_ref().ok_or("No snapshot engine")?;
    let vault = app.vault.as_ref().ok_or("No vault open")?;
    let full_content = vault
        .read_note(note_id)
        .map_err(|e| format!("Failed to read note {}: {}", note_id, e))?;

    let chars: Vec<char> = full_content.chars().collect();
    let safe_start = start.min(chars.len());
    let safe_end = end.min(chars.len()).max(safe_start);
    let selected: String = chars[safe_start..safe_end].iter().collect();

    snapshot_engine.create_local_snapshot_with_emitter(
        note_id,
        &selected,
        (safe_start, safe_end),
        app_handle,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::dream_engine::DreamEngine;
    use crate::ai::embedding::{EmbeddingBackend, EmbeddingPipeline};
    use crate::ai::search_engine::SearchEngine;
    use crate::ai::sync_engine::SyncEngine;
    use crate::ai::vectordb::VectorStore;
    use crate::fs::vault::VaultManager;
    use crate::graph::link_graph::LinkGraph;
    use crate::ipc::protocol::TwoSurfaceProtocol;
    use crate::snapshot::engine::SnapshotEngine;
    use crate::snapshot::store::SnapshotStore;
    use std::sync::Arc;
    use tempfile::TempDir;

    fn app_state_with_snapshots(vault_dir: &std::path::Path) -> AppState {
        let dualtrack_dir = vault_dir.join(".dualtrack");
        std::fs::create_dir_all(&dualtrack_dir).unwrap();
        let vault = VaultManager::open(vault_dir).unwrap();
        let mut vector_store =
            VectorStore::open(dualtrack_dir.join("vectors").to_str().unwrap()).unwrap();
        vector_store.set_dimension(384);
        let snapshot_engine = SnapshotEngine::new(
            SnapshotStore::new(&dualtrack_dir),
            EmbeddingPipeline::new(EmbeddingBackend::None),
        );
        snapshot_engine.init().unwrap();

        let mut link_graph = LinkGraph::new();
        link_graph.add_link("backlink.md", "note.md");

        AppState {
            vault: Some(vault),
            file_watcher: None,
            link_graph,
            vault_path: vault_dir.to_string_lossy().to_string(),
            sync_engine: Some(SyncEngine::new(
                vector_store,
                EmbeddingPipeline::new(EmbeddingBackend::None),
            )),
            vector_store_path: dualtrack_dir.join("vectors"),
            dualtrack_dir,
            snapshot_engine: Some(snapshot_engine),
            dream_engine: Some(DreamEngine::new()),
            protocol: Some(TwoSurfaceProtocol::default()),
            search_engine: Some(Arc::new(SearchEngine::new(vault_dir).unwrap())),
            output_manager: None,
            bridge_store: None,
            snapshot_listeners_started: false,
        }
    }

    #[test]
    fn global_snapshot_event_logic_stores_note_and_backlink_context() {
        let dir = TempDir::new().unwrap();
        let vault_dir = dir.path();
        std::fs::write(vault_dir.join("note.md"), "# Note\n\nbody").unwrap();
        std::fs::write(vault_dir.join("backlink.md"), "# Backlink\n\ncontext").unwrap();
        let app = app_state_with_snapshots(vault_dir);

        let snapshot =
            create_global_snapshot_from_state::<tauri::Wry>(&app, "note.md", None).unwrap();

        assert_eq!(snapshot.note_id, "note.md");
        assert!(matches!(
            snapshot.snapshot_type,
            snapshot::SnapshotType::Global
        ));
        assert!(snapshot
            .metadata
            .backlinks
            .contains(&"backlink.md".to_string()));
        assert!(snapshot
            .metadata
            .backlinks
            .iter()
            .any(|entry| entry.contains("context")));
        assert!(app
            .snapshot_engine
            .as_ref()
            .unwrap()
            .get_current("note.md", &snapshot::SnapshotType::Global)
            .is_some());
    }

    #[test]
    fn local_snapshot_event_logic_clamps_selection_and_stores_snapshot() {
        let dir = TempDir::new().unwrap();
        let vault_dir = dir.path();
        std::fs::write(vault_dir.join("note.md"), "abcdef").unwrap();
        let app = app_state_with_snapshots(vault_dir);

        let snapshot =
            create_local_snapshot_from_state::<tauri::Wry>(&app, "note.md", 2, 99, None).unwrap();

        assert!(matches!(
            snapshot.snapshot_type,
            snapshot::SnapshotType::Local
        ));
        assert_eq!(snapshot.metadata.selection_range, Some((2, 6)));
        assert_eq!(snapshot.metadata.total_chars, 4);
        assert!(app
            .snapshot_engine
            .as_ref()
            .unwrap()
            .get_current("note.md", &snapshot::SnapshotType::Local)
            .is_some());
    }
}

#[allow(dead_code)]
pub fn register_commands(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    builder.invoke_handler(tauri::generate_handler![
        get_current_snapshot,
        get_snapshot_diff,
    ])
}
