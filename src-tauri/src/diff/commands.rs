use crate::bridge::proposal::{BridgeProposal, BridgeProposalStatus, SourceRefKind};
use crate::diff::ghost_store::{GhostOp, GhostStatus};
use crate::AiState;
use crate::AppState;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, State};

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

fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn bridge_status_for_ghost_status(ghost_status: &GhostStatus) -> Option<BridgeProposalStatus> {
    match ghost_status {
        GhostStatus::Accepted => Some(BridgeProposalStatus::Approved),
        GhostStatus::Rejected => Some(BridgeProposalStatus::Rejected),
        _ => None,
    }
}

fn sync_bridge_status_for_ghost(
    state: &Mutex<AppState>,
    ghost_id: &str,
    ghost_status: &GhostStatus,
) -> Option<BridgeProposal> {
    let bridge_status = bridge_status_for_ghost_status(ghost_status)?;
    let store = match state.lock() {
        Ok(app) => app.bridge_store.clone(),
        Err(error) => {
            tracing::warn!("Failed to sync ghost bridge status: {}", error);
            return None;
        }
    }?;

    match store.update_status_by_source_ref(
        &SourceRefKind::Ghost,
        ghost_id,
        bridge_status,
        now_millis(),
    ) {
        Ok(updated) => updated,
        Err(error) => {
            tracing::warn!(
                "Failed to update bridge proposal for ghost {}: {}",
                ghost_id,
                error
            );
            None
        }
    }
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

fn accept_ghost_feedback(
    ghost: &mut crate::diff::ghost_store::GhostNote,
    block_ids: &[String],
) -> usize {
    let valid_block_ids = ghost
        .suggested_blocks
        .iter()
        .map(|block| block.block_id.as_str())
        .collect::<std::collections::HashSet<_>>();
    let mut accepted_count = 0;

    for block_id in block_ids {
        if !valid_block_ids.contains(block_id.as_str()) {
            continue;
        }
        if !ghost.accepted_blocks.contains(block_id) {
            ghost.accepted_blocks.push(block_id.clone());
            accepted_count += 1;
        }
        ghost.rejected_blocks.retain(|id| id != block_id);
    }
    update_ghost_status(ghost);
    accepted_count
}

#[tauri::command]
pub(crate) fn accept_diff(
    ghost_id: String,
    block_ids: Vec<String>,
    state: State<'_, Mutex<AppState>>,
    ai_state: State<'_, Mutex<AiState>>,
    app_handle: AppHandle,
) -> Result<String, String> {
    let (ghost_update, updated_status, accepted_count, source_note) = {
        let ai = ai_state.lock().map_err(|e| e.to_string())?;
        let mut updated_ghost = ai
            .ghost_store
            .get(&ghost_id)
            .ok_or_else(|| format!("Ghost note not found: {}", ghost_id))?;
        let accepted_count = accept_ghost_feedback(&mut updated_ghost, &block_ids);
        ai.ghost_store.save(&updated_ghost)?;
        let updated_status = updated_ghost.status.clone();
        (
            serde_json::json!({
                "ghost_id": ghost_id,
                "status": updated_status,
                "accepted_blocks": updated_ghost.accepted_blocks,
                "rejected_blocks": updated_ghost.rejected_blocks,
            }),
            updated_status,
            accepted_count,
            updated_ghost.source_note,
        )
    };
    let bridge_update = sync_bridge_status_for_ghost(&state, &ghost_id, &updated_status);
    let _ = app_handle.emit("ghost-updated", ghost_update);
    if let Some(proposal) = bridge_update {
        let _ = app_handle.emit("bridge-proposal-updated", &proposal);
    }

    Ok(format!(
        "Recorded feedback for {} blocks from {}; human note content was not modified",
        accepted_count, source_note
    ))
}

#[tauri::command]
pub(crate) fn reject_diff(
    ghost_id: String,
    block_ids: Option<Vec<String>>,
    state: State<'_, Mutex<AppState>>,
    ai_state: State<'_, Mutex<AiState>>,
    app_handle: AppHandle,
) -> Result<String, String> {
    let (ghost_update, updated_status, rejected_count) = {
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
        let updated_status = ghost.status.clone();
        (
            serde_json::json!({
                "ghost_id": ghost_id.clone(),
                "status": updated_status,
                "accepted_blocks": ghost.accepted_blocks,
                "rejected_blocks": ghost.rejected_blocks,
            }),
            updated_status,
            ids.len(),
        )
    };
    let bridge_update = sync_bridge_status_for_ghost(&state, &ghost_id, &updated_status);
    let _ = app_handle.emit("ghost-updated", ghost_update);
    if let Some(proposal) = bridge_update {
        let _ = app_handle.emit("bridge-proposal-updated", &proposal);
    }
    Ok(format!("Rejected {} blocks from {}", rejected_count, ghost_id))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepting_ghost_blocks_updates_feedback_without_merging_note_text() {
        let mut ghost = crate::diff::ghost_store::GhostNote {
            id: "ghost-feedback".to_string(),
            task_id: Some("task-feedback".to_string()),
            source_note: "human.md".to_string(),
            task_description: "Review AI suggestion".to_string(),
            suggested_blocks: vec![crate::diff::ghost_store::GhostBlock {
                block_id: "block-1".to_string(),
                content: "AI suggestion".to_string(),
                operation: GhostOp::Suggestion,
                after_block_id: None,
                heading_context: String::new(),
                context: vec![],
                verified: None,
                verification_result: None,
            }],
            created_at: 1,
            status: GhostStatus::Pending,
            priority: 50,
            expires_at: None,
            related_ghosts: vec![],
            confidence: 0.7,
            feedback_history: vec![],
            accepted_blocks: vec![],
            rejected_blocks: vec![],
        };

        let accepted = accept_ghost_feedback(&mut ghost, &["block-1".to_string()]);

        assert_eq!(accepted, 1);
        assert_eq!(ghost.accepted_blocks, vec!["block-1"]);
        assert!(matches!(ghost.status, GhostStatus::Accepted));
    }

    #[test]
    fn final_ghost_statuses_map_to_bridge_resolution_statuses() {
        assert_eq!(
            bridge_status_for_ghost_status(&GhostStatus::Accepted),
            Some(BridgeProposalStatus::Approved)
        );
        assert_eq!(
            bridge_status_for_ghost_status(&GhostStatus::Rejected),
            Some(BridgeProposalStatus::Rejected)
        );
        assert_eq!(bridge_status_for_ghost_status(&GhostStatus::Pending), None);
        assert_eq!(
            bridge_status_for_ghost_status(&GhostStatus::PartiallyAccepted),
            None
        );
    }
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
