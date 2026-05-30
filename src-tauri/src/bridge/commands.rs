use crate::bridge::proposal::{
    BridgeProposal, BridgeProposalStatus, ProposalActionKind, SourceRefKind,
};
use crate::bridge::store::BridgeProposalStore;
use crate::diff::ghost_store::GhostStatus;
use crate::{AiState, AppState};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, State};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BridgeProposalActionResult {
    pub status: String,
    pub message: String,
    pub proposal: BridgeProposal,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

pub fn parse_status(status: &str) -> Result<BridgeProposalStatus, String> {
    BridgeProposalStatus::parse(status)
}

pub fn parse_direct_status_update(status: &str) -> Result<BridgeProposalStatus, String> {
    let parsed = parse_status(status)?;
    match parsed {
        BridgeProposalStatus::Archived => Ok(parsed),
        BridgeProposalStatus::Pending
        | BridgeProposalStatus::Rejected
        | BridgeProposalStatus::Approved
        | BridgeProposalStatus::Applied => Err(format!(
            "Bridge proposal status '{status}' requires a bridge action"
        )),
    }
}

fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn source_ref_is_task_like(kind: &SourceRefKind) -> bool {
    matches!(kind, SourceRefKind::Task | SourceRefKind::SchedulerJob)
}

pub fn execute_action_against_store(
    store: &BridgeProposalStore,
    id: &str,
    action_id: &str,
    mut approve_task: Option<&mut dyn FnMut(&str) -> Result<(), String>>,
    mut reject_task: Option<&mut dyn FnMut(&str) -> Result<(), String>>,
    mut reject_ghost: Option<&mut dyn FnMut(&str) -> Result<(), String>>,
) -> Result<BridgeProposalActionResult, String> {
    let proposal = store
        .get(id)?
        .ok_or_else(|| format!("Bridge proposal not found: {id}"))?;
    let action = proposal
        .actions
        .iter()
        .find(|action| action.id == action_id)
        .cloned()
        .ok_or_else(|| format!("Bridge action not found: {action_id}"))?;

    let is_archive = action.kind == ProposalActionKind::Archive;
    if proposal.status != BridgeProposalStatus::Pending && !is_archive {
        return Err(format!(
            "Bridge proposal {} is {:?}; cannot execute action {}",
            proposal.id, proposal.status, action_id
        ));
    }

    let updated = match action.kind {
        ProposalActionKind::Archive => {
            store.update_status(id, BridgeProposalStatus::Archived, now_millis())?
        }
        ProposalActionKind::Reject => {
            let updated = store.update_status(id, BridgeProposalStatus::Rejected, now_millis())?;
            match proposal.source_ref.kind {
                SourceRefKind::Task | SourceRefKind::SchedulerJob => {
                    if let Some(reject) = reject_task.as_mut() {
                        if let Err(error) = reject(&proposal.source_ref.id) {
                            let _ = store.update_status(
                                id,
                                BridgeProposalStatus::Pending,
                                now_millis(),
                            );
                            return Err(error);
                        }
                    }
                }
                SourceRefKind::Ghost => {
                    if let Some(reject) = reject_ghost.as_mut() {
                        if let Err(error) = reject(&proposal.source_ref.id) {
                            let _ = store.update_status(
                                id,
                                BridgeProposalStatus::Pending,
                                now_millis(),
                            );
                            return Err(error);
                        }
                    }
                }
                _ => {}
            }
            updated
        }
        ProposalActionKind::ApproveTask => {
            let task_id = action
                .payload
                .get("task_id")
                .and_then(|value| value.as_str())
                .ok_or_else(|| "approve_task action missing task_id".to_string())?;
            if !source_ref_is_task_like(&proposal.source_ref.kind)
                || proposal.source_ref.id != task_id
            {
                return Err(format!(
                    "approve_task payload task_id does not match proposal source: {} != {}",
                    task_id, proposal.source_ref.id
                ));
            }
            let updated = store.update_status(id, BridgeProposalStatus::Approved, now_millis())?;
            if let Some(approve) = approve_task.as_mut() {
                if let Err(error) = approve(&proposal.source_ref.id) {
                    let _ = store.update_status(id, BridgeProposalStatus::Pending, now_millis());
                    return Err(error);
                }
            } else {
                return Err("approve_task action requires an approval handler".to_string());
            }
            updated
        }
        ProposalActionKind::OpenDiff | ProposalActionKind::OpenTrace => {
            let (target_panel, target_id) = match action.kind {
                ProposalActionKind::OpenDiff => (
                    "diff",
                    action
                        .payload
                        .get("ghost_id")
                        .and_then(|value| value.as_str())
                        .unwrap_or(&proposal.source_ref.id)
                        .to_string(),
                ),
                ProposalActionKind::OpenTrace => (
                    "tasks",
                    action
                        .payload
                        .get("task_id")
                        .and_then(|value| value.as_str())
                        .unwrap_or(&proposal.source_ref.id)
                        .to_string(),
                ),
                _ => unreachable!(),
            };
            return Ok(BridgeProposalActionResult {
                status: "navigate".to_string(),
                message: format!("Bridge action requests navigation: {action_id}"),
                proposal,
                metadata: serde_json::json!({
                    "action_kind": action.kind,
                    "effect": "navigate",
                    "target_panel": target_panel,
                    "target_id": target_id,
                    "payload": action.payload,
                }),
            });
        }
        ProposalActionKind::ApplyGhost => {
            return Err(
                "apply_ghost action is not implemented; open the diff for review".to_string(),
            );
        }
    };

    Ok(BridgeProposalActionResult {
        status: "success".to_string(),
        message: format!("Executed bridge action: {action_id}"),
        proposal: updated,
        metadata: serde_json::json!({
            "action_kind": action.kind,
            "effect": "state_changed",
            "payload": action.payload,
        }),
    })
}

fn bridge_store(state: State<'_, Mutex<AppState>>) -> Result<BridgeProposalStore, String> {
    let app = state.lock().map_err(|e| e.to_string())?;
    app.bridge_store
        .as_ref()
        .cloned()
        .ok_or_else(|| "Bridge store not initialized".to_string())
}

#[tauri::command]
pub(crate) fn list_bridge_proposals(
    status_filter: Option<String>,
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<BridgeProposal>, String> {
    bridge_store(state)?.list(status_filter.as_deref())
}

#[tauri::command]
pub(crate) fn get_bridge_proposal(
    id: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<Option<BridgeProposal>, String> {
    bridge_store(state)?.get(&id)
}

#[tauri::command]
pub(crate) fn update_bridge_proposal_status(
    id: String,
    status: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<BridgeProposal, String> {
    bridge_store(state)?.update_status(&id, parse_direct_status_update(&status)?, now_millis())
}

#[tauri::command]
pub(crate) fn execute_bridge_action(
    id: String,
    action_id: String,
    state: State<'_, Mutex<AppState>>,
    ai_state: State<'_, Mutex<AiState>>,
    app_handle: AppHandle,
) -> Result<BridgeProposalActionResult, String> {
    let store = bridge_store(state)?;
    let mut approved_task_id: Option<String> = None;
    let mut rejected_task_id: Option<String> = None;
    let mut rejected_ghost_id: Option<String> = None;
    let mut approve_task = |task_id: &str| -> Result<(), String> {
        let mut ai = ai_state.lock().map_err(|e| e.to_string())?;
        ai.agent_scheduler.approve(task_id, "human")?;
        ai.task_notifier.notify_one();
        approved_task_id = Some(task_id.to_string());
        Ok(())
    };
    let mut reject_task = |task_id: &str| -> Result<(), String> {
        let mut ai = ai_state.lock().map_err(|e| e.to_string())?;
        ai.agent_scheduler.reject(task_id)?;
        rejected_task_id = Some(task_id.to_string());
        Ok(())
    };
    let mut reject_ghost = |ghost_id: &str| -> Result<(), String> {
        let ai = ai_state.lock().map_err(|e| e.to_string())?;
        ai.ghost_store
            .update_status(ghost_id, GhostStatus::Rejected)?;
        rejected_ghost_id = Some(ghost_id.to_string());
        Ok(())
    };

    let result = execute_action_against_store(
        &store,
        &id,
        &action_id,
        Some(&mut approve_task),
        Some(&mut reject_task),
        Some(&mut reject_ghost),
    )?;

    if let Some(task_id) = approved_task_id {
        let _ = app_handle.emit(
            "task-updated",
            serde_json::json!({
                "task_id": task_id,
                "status": "approved"
            }),
        );
        let _ = app_handle.emit(
            "task-checked",
            serde_json::json!({
                "task_id": task_id,
                "checked_at": now_millis(),
                "checked_by": "human"
            }),
        );
    }
    if let Some(task_id) = rejected_task_id {
        let _ = app_handle.emit(
            "task-updated",
            serde_json::json!({
                "task_id": task_id,
                "status": "cancelled"
            }),
        );
    }
    if let Some(ghost_id) = rejected_ghost_id {
        let _ = app_handle.emit(
            "ghost-updated",
            serde_json::json!({
                "ghost_id": ghost_id,
                "status": "rejected"
            }),
        );
    }
    let _ = app_handle.emit("bridge-proposal-updated", &result.proposal);

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::proposal::{
        BridgeProposal, BridgeProposalSource, BridgeProposalStatus, ImpactScope, ProposalAction,
        ProposalActionKind, ProposalRisk, SourceRef, SourceRefKind, TrustSnapshot,
    };
    use crate::bridge::store::BridgeProposalStore;

    fn sample() -> BridgeProposal {
        BridgeProposal {
            id: "p1".to_string(),
            source: BridgeProposalSource::Tool,
            source_ref: SourceRef {
                kind: SourceRefKind::Task,
                id: "task_1".to_string(),
                path: None,
            },
            intent: "Approve task".to_string(),
            summary: "Approve task".to_string(),
            task_type: None,
            sandbox_summary: None,
            expected_output: None,
            risk_reason: None,
            evidence: vec![],
            impact: ImpactScope::default(),
            risk: ProposalRisk::Low,
            status: BridgeProposalStatus::Pending,
            actions: vec![ProposalAction {
                id: "archive".to_string(),
                label: "Archive".to_string(),
                kind: ProposalActionKind::Archive,
                payload: serde_json::json!({}),
            }],
            trust_snapshot: TrustSnapshot::default(),
            created_at: 1,
            updated_at: 1,
        }
    }

    #[test]
    fn parse_status_rejects_unknown_values() {
        assert!(parse_status("pending").is_ok());
        assert!(parse_status("archived").is_ok());
        assert!(parse_status("nonsense").is_err());
    }

    #[test]
    fn execute_archive_action_updates_status() {
        let dir = tempfile::tempdir().unwrap();
        let store = BridgeProposalStore::new(dir.path().join("bridge"));
        store.upsert(sample()).unwrap();

        let result =
            execute_action_against_store(&store, "p1", "archive", None, None, None).unwrap();

        assert_eq!(result.status, "success");
        assert_eq!(result.proposal.status, BridgeProposalStatus::Archived);
    }

    #[test]
    fn approve_task_rejects_payload_source_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let store = BridgeProposalStore::new(dir.path().join("bridge"));
        let mut proposal = sample();
        proposal.actions = vec![ProposalAction {
            id: "approve".to_string(),
            label: "Approve".to_string(),
            kind: ProposalActionKind::ApproveTask,
            payload: serde_json::json!({ "task_id": "task_2" }),
        }];
        store.upsert(proposal).unwrap();
        let mut approve = |_task_id: &str| Ok(());

        let error =
            execute_action_against_store(&store, "p1", "approve", Some(&mut approve), None, None)
                .unwrap_err();

        assert!(error.contains("does not match proposal source"));
    }

    #[test]
    fn approved_proposals_cannot_execute_approval_again() {
        let dir = tempfile::tempdir().unwrap();
        let store = BridgeProposalStore::new(dir.path().join("bridge"));
        let mut proposal = sample();
        proposal.status = BridgeProposalStatus::Approved;
        proposal.actions = vec![ProposalAction {
            id: "approve".to_string(),
            label: "Approve".to_string(),
            kind: ProposalActionKind::ApproveTask,
            payload: serde_json::json!({ "task_id": "task_1" }),
        }];
        store.upsert(proposal).unwrap();
        let mut approve = |_task_id: &str| Ok(());

        let error =
            execute_action_against_store(&store, "p1", "approve", Some(&mut approve), None, None)
                .unwrap_err();

        assert!(error.contains("cannot execute action"));
    }

    #[test]
    fn direct_status_update_rejects_execution_states() {
        assert!(parse_direct_status_update("archived").is_ok());
        assert!(parse_direct_status_update("rejected").is_err());
        assert!(parse_direct_status_update("approved").is_err());
        assert!(parse_direct_status_update("applied").is_err());
    }

    #[test]
    fn reject_ghost_invokes_ghost_reject_handler() {
        let dir = tempfile::tempdir().unwrap();
        let store = BridgeProposalStore::new(dir.path().join("bridge"));
        let mut proposal = sample();
        proposal.source = BridgeProposalSource::Ghost;
        proposal.source_ref = SourceRef {
            kind: SourceRefKind::Ghost,
            id: "ghost_1".to_string(),
            path: Some("Target.md".to_string()),
        };
        proposal.actions = vec![ProposalAction {
            id: "reject".to_string(),
            label: "Reject".to_string(),
            kind: ProposalActionKind::Reject,
            payload: serde_json::json!({ "ghost_id": "ghost_1" }),
        }];
        store.upsert(proposal).unwrap();
        let mut rejected_ghost_id = String::new();
        let mut reject_ghost = |ghost_id: &str| {
            rejected_ghost_id = ghost_id.to_string();
            Ok(())
        };

        let result = execute_action_against_store(
            &store,
            "p1",
            "reject",
            None,
            None,
            Some(&mut reject_ghost),
        )
        .unwrap();

        assert_eq!(result.proposal.status, BridgeProposalStatus::Rejected);
        assert_eq!(rejected_ghost_id, "ghost_1");
    }

    #[test]
    fn open_diff_action_returns_navigation_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let store = BridgeProposalStore::new(dir.path().join("bridge"));
        let mut proposal = sample();
        proposal.source = BridgeProposalSource::Ghost;
        proposal.source_ref = SourceRef {
            kind: SourceRefKind::Ghost,
            id: "ghost_1".to_string(),
            path: Some("Target.md".to_string()),
        };
        proposal.actions = vec![ProposalAction {
            id: "open-diff".to_string(),
            label: "Open Diff".to_string(),
            kind: ProposalActionKind::OpenDiff,
            payload: serde_json::json!({ "ghost_id": "ghost_1" }),
        }];
        store.upsert(proposal).unwrap();

        let result =
            execute_action_against_store(&store, "p1", "open-diff", None, None, None).unwrap();

        assert_eq!(result.status, "navigate");
        assert_eq!(result.metadata["effect"], "navigate");
        assert_eq!(result.metadata["target_panel"], "diff");
        assert_eq!(result.metadata["target_id"], "ghost_1");
    }

    #[test]
    fn approve_scheduler_task_invokes_task_approval_handler() {
        let dir = tempfile::tempdir().unwrap();
        let store = BridgeProposalStore::new(dir.path().join("bridge"));
        let mut proposal = sample();
        proposal.source = BridgeProposalSource::Scheduler;
        proposal.source_ref = SourceRef {
            kind: SourceRefKind::SchedulerJob,
            id: "task_1".to_string(),
            path: Some("dream-auto".to_string()),
        };
        proposal.actions = vec![ProposalAction {
            id: "approve".to_string(),
            label: "Approve".to_string(),
            kind: ProposalActionKind::ApproveTask,
            payload: serde_json::json!({ "task_id": "task_1" }),
        }];
        store.upsert(proposal).unwrap();
        let mut approved_task_id = String::new();
        let mut approve = |task_id: &str| {
            approved_task_id = task_id.to_string();
            Ok(())
        };

        let result =
            execute_action_against_store(&store, "p1", "approve", Some(&mut approve), None, None)
                .unwrap();

        assert_eq!(result.proposal.status, BridgeProposalStatus::Approved);
        assert_eq!(approved_task_id, "task_1");
    }
}
