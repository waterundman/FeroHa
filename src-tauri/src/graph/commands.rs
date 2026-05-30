use crate::AppState;
use std::sync::Mutex;
use tauri::State;

#[tauri::command]
pub(crate) fn get_graph(
    state: State<'_, Mutex<AppState>>,
) -> Result<crate::graph::link_graph::GraphData, String> {
    let app = state.lock().map_err(|e| e.to_string())?;
    Ok(app.link_graph.to_frontend_json())
}

#[tauri::command]
pub(crate) fn get_backlinks(
    note_id: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<crate::graph::backlinks::BacklinkContext>, String> {
    let app = state.lock().map_err(|e| e.to_string())?;
    let vault = app.vault.as_ref().ok_or("No vault open")?;

    let provider = |path: &str| vault.read_note(path).ok();

    Ok(crate::graph::backlinks::get_contextual_backlinks(
        &app.link_graph,
        &note_id,
        &provider,
    ))
}

#[tauri::command]
pub(crate) fn get_note_links(
    note_id: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<String>, String> {
    let app = state.lock().map_err(|e| e.to_string())?;
    let graph_data = app.link_graph.to_frontend_json();
    let links: Vec<String> = graph_data
        .edges
        .iter()
        .filter(|e| e.from == note_id)
        .map(|e| e.to.clone())
        .collect();
    Ok(links)
}

#[tauri::command]
pub(crate) fn get_graph_with_focus(
    focus_path: Option<String>,
    state: State<'_, Mutex<AppState>>,
) -> Result<crate::graph::link_graph::GraphData, String> {
    let app = state.lock().map_err(|e| e.to_string())?;
    match focus_path {
        Some(path) => Ok(app.link_graph.get_graph_with_focus(&path)),
        None => Ok(app.link_graph.to_frontend_json()),
    }
}

#[allow(dead_code)]
pub fn register_commands(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    builder.invoke_handler(tauri::generate_handler![
        get_graph,
        get_graph_with_focus,
        get_backlinks,
        get_note_links,
    ])
}
