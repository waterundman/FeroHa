use crate::snapshot;
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

pub fn handle_global_snapshot(note_id: &str, _vault_path: &str, app_handle: &AppHandle) {
    let state = app_handle.state::<Mutex<AppState>>();
    let app = match state.lock() {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("Failed to lock state: {}", e);
            return;
        }
    };

    let snapshot_engine = match app.snapshot_engine.as_ref() {
        Some(e) => e,
        None => {
            tracing::warn!("No snapshot engine");
            return;
        }
    };

    let vault = match app.vault.as_ref() {
        Some(v) => v,
        None => {
            tracing::warn!("No vault open");
            return;
        }
    };

    let content = match vault.read_note(note_id) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Failed to read note {}: {}", note_id, e);
            return;
        }
    };

    let backlinks: Vec<String> = app.link_graph.get_backlinks(note_id);
    let backlink_contents: Vec<String> = backlinks
        .iter()
        .filter_map(|bl| vault.read_note(bl).ok())
        .collect();
    let mut all_backlinks = backlinks.clone();
    all_backlinks.extend(backlink_contents);

    if let Err(e) =
        snapshot_engine.create_global_snapshot(note_id, &content, &all_backlinks, app_handle)
    {
        tracing::warn!("Failed to create global snapshot for {}: {}", note_id, e);
    }
}

pub fn handle_local_snapshot(
    note_id: &str,
    start: usize,
    end: usize,
    _vault_path: &str,
    app_handle: &AppHandle,
) {
    let state = app_handle.state::<Mutex<AppState>>();
    let app = match state.lock() {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("Failed to lock state: {}", e);
            return;
        }
    };

    let snapshot_engine = match app.snapshot_engine.as_ref() {
        Some(e) => e,
        None => {
            tracing::warn!("No snapshot engine");
            return;
        }
    };

    let vault = match app.vault.as_ref() {
        Some(v) => v,
        None => {
            tracing::warn!("No vault open");
            return;
        }
    };

    let full_content = match vault.read_note(note_id) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Failed to read note {}: {}", note_id, e);
            return;
        }
    };

    let chars: Vec<char> = full_content.chars().collect();
    let safe_start = start.min(chars.len());
    let safe_end = end.min(chars.len()).max(safe_start);
    let selected: String = chars[safe_start..safe_end].iter().collect();

    if let Err(e) = snapshot_engine.create_local_snapshot(
        note_id,
        &selected,
        (safe_start, safe_end),
        app_handle,
    ) {
        tracing::warn!("Failed to create local snapshot for {}: {}", note_id, e);
    }
}

#[allow(dead_code)]
pub fn register_commands(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    builder.invoke_handler(tauri::generate_handler![
        get_current_snapshot,
        get_snapshot_diff,
    ])
}
