use super::agent_scheduler::AgentScheduler;
use super::workflow_runtime_service::WorkflowRuntimeService;
use super::workflow_template::WorkflowTemplate;
use crate::harness::workflow_runtime::WorkflowRuntimeBundle;
use crate::{AiState, AppState};
use std::path::Path;
use std::sync::Mutex;
use tauri::State;

pub fn create_and_start_for_root(
    root: &Path,
    goal_text: String,
    acceptance_criteria: Vec<String>,
    scheduler: &mut AgentScheduler,
    now: u64,
) -> Result<WorkflowRuntimeBundle, String> {
    let template = WorkflowTemplate::build(&goal_text, acceptance_criteria, now)
        .map_err(|error| error.to_string())?;
    WorkflowRuntimeService::new(root)
        .start(
            template.goal,
            template.workflow,
            template.registry,
            &template.run_id,
            scheduler,
            now,
        )
        .map_err(|error| error.to_string())
}

pub fn get_workflow_run_for_root(
    root: &Path,
    run_id: &str,
) -> Result<WorkflowRuntimeBundle, String> {
    WorkflowRuntimeService::new(root)
        .get(run_id)
        .map_err(|error| error.to_string())
}

pub fn list_workflow_runs_for_root(root: &Path) -> Result<Vec<WorkflowRuntimeBundle>, String> {
    WorkflowRuntimeService::new(root)
        .list()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn create_and_start_workflow(
    goal_text: String,
    acceptance_criteria: Vec<String>,
    state: State<'_, Mutex<AppState>>,
    ai_state: State<'_, Mutex<AiState>>,
) -> Result<WorkflowRuntimeBundle, String> {
    let vault_root = open_vault_root(&state)?;
    let bundle = {
        let mut ai = ai_state.lock().map_err(|error| error.to_string())?;
        create_and_start_for_root(
            &vault_root,
            goal_text,
            acceptance_criteria,
            &mut ai.agent_scheduler,
            now_millis(),
        )?
    };
    if let Ok(ai) = ai_state.lock() {
        ai.task_notifier.notify_one();
    }
    Ok(bundle)
}

#[tauri::command]
pub(crate) fn get_workflow_run(
    run_id: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<WorkflowRuntimeBundle, String> {
    let vault_root = open_vault_root(&state)?;
    get_workflow_run_for_root(&vault_root, &run_id)
}

#[tauri::command]
pub(crate) fn list_workflow_runs(
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<WorkflowRuntimeBundle>, String> {
    let vault_root = open_vault_root(&state)?;
    list_workflow_runs_for_root(&vault_root)
}

fn open_vault_root(state: &State<'_, Mutex<AppState>>) -> Result<std::path::PathBuf, String> {
    let app = state.lock().map_err(|error| error.to_string())?;
    if app.vault.is_none() || app.vault_path.trim().is_empty() {
        return Err("No vault open".to_string());
    }
    Ok(std::path::PathBuf::from(&app.vault_path))
}

fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::agent_scheduler::AgentScheduler;

    #[test]
    fn create_and_start_routes_template_into_existing_runtime_and_scheduler() {
        let root = tempfile::tempdir().unwrap();
        let mut scheduler = AgentScheduler::new(1);

        let bundle = create_and_start_for_root(
            root.path(),
            "Map Bayesian memory evidence".to_string(),
            vec!["Every conclusion cites evidence".to_string()],
            &mut scheduler,
            100,
        )
        .unwrap();

        assert_eq!(bundle.workflow.steps.len(), 1);
        assert!(scheduler
            .get_task("workflow__run_100__S001__attempt_1")
            .is_some());
    }

    #[test]
    fn list_workflow_runs_reads_existing_runtime_store() {
        let root = tempfile::tempdir().unwrap();
        let mut scheduler = AgentScheduler::new(1);
        create_and_start_for_root(
            root.path(),
            "Goal".to_string(),
            vec!["Evidence".to_string()],
            &mut scheduler,
            100,
        )
        .unwrap();

        let runs = list_workflow_runs_for_root(root.path()).unwrap();

        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].run.run_id, "run_100");
    }
}
