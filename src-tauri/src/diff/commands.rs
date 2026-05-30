use crate::diff::ast_diff::DiffOp;
use crate::diff::ghost_store::{GhostOp, GhostStatus};
use crate::diff::merge_engine::apply_merge;
use crate::AiState;
use crate::AppState;
use std::sync::Mutex;
use tauri::State;

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DiffBlock {
    ghost_id: String,
    id: String,
    #[serde(rename = "type")]
    block_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    old_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    new_text: Option<String>,
    accepted: bool,
    rejected: bool,
}

#[tauri::command]
pub(crate) fn get_diff_blocks(
    ai_state: State<'_, Mutex<AiState>>,
) -> Result<Vec<DiffBlock>, String> {
    let ai = ai_state.lock().map_err(|e| e.to_string())?;
    let ghosts = ai.ghost_store.list_all();

    let mut blocks = Vec::new();
    for ghost in &ghosts {
        for block in &ghost.suggested_blocks {
            let (block_type, old_text, new_text) = match block.operation {
                GhostOp::Insert | GhostOp::Suggestion => {
                    ("inserted".to_string(), None, Some(block.content.clone()))
                }
                GhostOp::Modify => (
                    "modified".to_string(),
                    Some(block.heading_context.clone()),
                    Some(block.content.clone()),
                ),
                GhostOp::Delete => ("deleted".to_string(), Some(block.content.clone()), None),
            };

            blocks.push(DiffBlock {
                ghost_id: ghost.id.clone(),
                id: block.block_id.clone(),
                block_type,
                old_text,
                new_text,
                accepted: ghost.accepted_blocks.contains(&block.block_id),
                rejected: ghost.rejected_blocks.contains(&block.block_id),
            });
        }
    }

    Ok(blocks)
}

#[tauri::command]
pub(crate) fn review_diff(
    ghost_id: String,
    ai_state: State<'_, Mutex<AiState>>,
) -> Result<serde_json::Value, String> {
    let ai = ai_state.lock().map_err(|e| e.to_string())?;
    let ghost = ai
        .ghost_store
        .get(&ghost_id)
        .ok_or_else(|| format!("Ghost note not found: {}", ghost_id))?;

    serde_json::to_value(&ghost).map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn list_ghosts(
    ai_state: State<'_, Mutex<AiState>>,
) -> Result<Vec<serde_json::Value>, String> {
    let ai = ai_state.lock().map_err(|e| e.to_string())?;
    let ghosts = ai.ghost_store.list_all();
    Ok(ghosts
        .iter()
        .map(|g| serde_json::to_value(g).unwrap())
        .collect())
}

#[tauri::command]
pub(crate) fn accept_diff(
    ghost_id: String,
    block_ids: Vec<String>,
    state: State<'_, Mutex<AppState>>,
    ai_state: State<'_, Mutex<AiState>>,
) -> Result<String, String> {
    let ai = ai_state.lock().map_err(|e| e.to_string())?;
    let ghost = ai
        .ghost_store
        .get(&ghost_id)
        .ok_or_else(|| format!("Ghost note not found: {}", ghost_id))?;

    let mut diff_ops: Vec<DiffOp> = Vec::new();
    for block in &ghost.suggested_blocks {
        if block_ids.contains(&block.block_id) {
            match block.operation {
                GhostOp::Insert | GhostOp::Suggestion => {
                    diff_ops.push(DiffOp::InsertBlock {
                        position: 0,
                        block_id: block.block_id.clone(),
                        text: block.content.clone(),
                    });
                }
                GhostOp::Delete => {
                    diff_ops.push(DiffOp::DeleteBlock {
                        block_id: block.block_id.clone(),
                    });
                }
                GhostOp::Modify => {
                    diff_ops.push(DiffOp::ModifyBlock {
                        block_id: block.block_id.clone(),
                        old_text: block.heading_context.clone(),
                        new_text: block.content.clone(),
                    });
                }
            }
        }
    }

    let app = state.lock().map_err(|e| e.to_string())?;
    let vault = app.vault.as_ref().ok_or("No vault open")?;
    let original_content = vault.read_note(&ghost.source_note).unwrap_or_default();
    let merged_content = apply_merge(&original_content, &diff_ops).map_err(|e| e.to_string())?;

    vault
        .write_note(&ghost.source_note, &merged_content)
        .map_err(|e| e.to_string())?;

    drop(ai);
    drop(app);

    let ai = ai_state.lock().map_err(|e| e.to_string())?;
    let mut updated_ghost = ai
        .ghost_store
        .get(&ghost_id)
        .ok_or_else(|| format!("Ghost note not found: {}", ghost_id))?;
    for block_id in &block_ids {
        if !updated_ghost.accepted_blocks.contains(block_id) {
            updated_ghost.accepted_blocks.push(block_id.clone());
        }
        updated_ghost.rejected_blocks.retain(|id| id != block_id);
    }
    update_ghost_status(&mut updated_ghost);
    ai.ghost_store.save(&updated_ghost)?;

    Ok(format!(
        "Accepted {} blocks into {}",
        block_ids.len(),
        ghost.source_note
    ))
}

#[tauri::command]
pub(crate) fn reject_diff(
    ghost_id: String,
    block_ids: Option<Vec<String>>,
    ai_state: State<'_, Mutex<AiState>>,
) -> Result<String, String> {
    let ai = ai_state.lock().map_err(|e| e.to_string())?;
    let mut ghost = ai
        .ghost_store
        .get(&ghost_id)
        .ok_or_else(|| format!("Ghost note not found: {}", ghost_id))?;
    let ids = block_ids.unwrap_or_else(|| {
        ghost
            .suggested_blocks
            .iter()
            .map(|block| block.block_id.clone())
            .collect()
    });
    for block_id in &ids {
        if !ghost.rejected_blocks.contains(block_id) {
            ghost.rejected_blocks.push(block_id.clone());
        }
        ghost.accepted_blocks.retain(|id| id != block_id);
    }
    update_ghost_status(&mut ghost);
    ai.ghost_store.save(&ghost)?;
    Ok(format!("Rejected {} blocks from {}", ids.len(), ghost_id))
}

fn update_ghost_status(ghost: &mut crate::diff::ghost_store::GhostNote) {
    let suggested_count = ghost.suggested_blocks.len();
    let accepted_count = ghost.accepted_blocks.len();
    let rejected_count = ghost.rejected_blocks.len();
    let total = accepted_count + rejected_count;

    ghost.confidence = if total > 0 {
        accepted_count as f32 / total as f32
    } else {
        0.0
    };

    ghost.status = if accepted_count >= suggested_count && suggested_count > 0 {
        GhostStatus::Accepted
    } else if rejected_count >= suggested_count && suggested_count > 0 {
        GhostStatus::Rejected
    } else if total > 0 {
        GhostStatus::PartiallyAccepted
    } else {
        GhostStatus::Pending
    };
}

#[allow(dead_code)]
pub fn register_commands(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    builder.invoke_handler(tauri::generate_handler![
        get_diff_blocks,
        review_diff,
        list_ghosts,
        accept_diff,
        reject_diff,
    ])
}
