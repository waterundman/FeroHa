use super::agent_scheduler::{AgentScheduler, TaskStatus};
use super::workflow_task_adapter::{AdaptedWorkflowTask, WorkflowTaskAdapter};
use crate::harness::workflow::{
    safe_runtime_component, AgentRegistry, ArtifactContract, GoalContract, OrchestratorOutput,
    RunStatus, StepDispatch, WorkflowError, WorkflowIr, WorkflowRunState,
    WorkflowRuntimeEventChain, WorkflowRuntimeEventStore, WorkflowStep, WorkflowStepStatus,
};
use crate::harness::workflow_runtime::{
    WorkflowDispatchRecord, WorkflowDispatchStatus, WorkflowRuntimeBundle, WorkflowRuntimeStore,
};
use std::path::{Path, PathBuf};

pub struct WorkflowRuntimeService {
    store: WorkflowRuntimeStore,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkflowRuntimeResumeAllResult {
    pub resumed: Vec<WorkflowRuntimeBundle>,
    pub errors: Vec<WorkflowRuntimeResumeError>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowRuntimeResumeError {
    pub run_id: String,
    pub error: String,
}

impl WorkflowRuntimeService {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            store: WorkflowRuntimeStore::new(root),
        }
    }

    pub fn get(&self, run_id: &str) -> Result<WorkflowRuntimeBundle, WorkflowError> {
        self.store.read(run_id)
    }

    pub fn start(
        &self,
        goal: GoalContract,
        workflow: WorkflowIr,
        registry: AgentRegistry,
        run_id: &str,
        scheduler: &mut AgentScheduler,
        now: u64,
    ) -> Result<WorkflowRuntimeBundle, WorkflowError> {
        safe_runtime_component(run_id)?;
        OrchestratorOutput::WorkflowCreate {
            workflow: workflow.clone(),
        }
        .validate(&goal, &registry)?;
        let runtime_path = self.store.runtime_path(run_id)?;
        if runtime_path.exists() {
            return Err(WorkflowError::RuntimeStateIo(format!(
                "workflow runtime already exists for run {run_id}"
            )));
        }

        let run = WorkflowRunState::for_workflow(run_id, &workflow, now.to_string());
        let mut bundle = WorkflowRuntimeBundle {
            goal,
            workflow,
            run,
            registry,
            dispatches: Vec::new(),
            updated_at: now.to_string(),
        };
        self.store.write(&bundle)?;
        self.ensure_event_chain(&WorkflowRuntimeEventChain::from_run_created(
            &bundle.workflow,
            &bundle.run,
            now.to_string(),
        ))?;

        self.reconcile(&mut bundle, scheduler, now)
    }

    pub fn resume(
        &self,
        run_id: &str,
        scheduler: &mut AgentScheduler,
        now: u64,
    ) -> Result<WorkflowRuntimeBundle, WorkflowError> {
        let mut bundle = self.store.read(run_id)?;
        self.repair_runtime_events(&bundle)?;
        self.record_event_chain(&WorkflowRuntimeEventChain::from_run_resumed(
            &bundle.workflow,
            &bundle.run,
            now.to_string(),
        ))?;

        self.reconcile(&mut bundle, scheduler, now)
    }

    pub fn resume_all(
        &self,
        scheduler: &mut AgentScheduler,
        now: u64,
    ) -> Result<WorkflowRuntimeResumeAllResult, WorkflowError> {
        let mut resumed = Vec::new();
        let mut errors = Vec::new();
        for run_id in self.store.list_run_ids()? {
            match self.resume(&run_id, scheduler, now) {
                Ok(bundle) => resumed.push(bundle),
                Err(error) => errors.push(WorkflowRuntimeResumeError {
                    run_id,
                    error: error.to_string(),
                }),
            }
        }
        Ok(WorkflowRuntimeResumeAllResult { resumed, errors })
    }

    fn reconcile(
        &self,
        bundle: &mut WorkflowRuntimeBundle,
        scheduler: &mut AgentScheduler,
        now: u64,
    ) -> Result<WorkflowRuntimeBundle, WorkflowError> {
        self.sync_scheduler_statuses(bundle, scheduler, now)?;
        self.recover_missing_scheduler_tasks(bundle, scheduler, now)?;

        for dispatch in bundle
            .run
            .ready_dispatches(&bundle.workflow, &bundle.registry)?
        {
            if dispatch_record(bundle, &dispatch.step_id, dispatch.attempt)
                .map(dispatch_record_is_terminal)
                .unwrap_or(false)
            {
                continue;
            }
            if dispatch_record(bundle, &dispatch.step_id, dispatch.attempt).is_some() {
                continue;
            }

            bundle.dispatches.push(WorkflowDispatchRecord {
                step_id: dispatch.step_id.clone(),
                attempt: dispatch.attempt,
                task_id: None,
                status: WorkflowDispatchStatus::Dispatched,
                detail: None,
            });
            bundle.updated_at = now.to_string();
            self.store.write(bundle)?;
            self.ensure_event_chain(&WorkflowRuntimeEventChain::from_step_dispatches(
                std::slice::from_ref(&dispatch),
                now.to_string(),
            ))?;

            self.advance_dispatch(bundle, scheduler, &dispatch, now)?;
        }

        Ok(bundle.clone())
    }

    fn recover_missing_scheduler_tasks(
        &self,
        bundle: &mut WorkflowRuntimeBundle,
        scheduler: &mut AgentScheduler,
        now: u64,
    ) -> Result<(), WorkflowError> {
        let recoverable = bundle
            .dispatches
            .iter()
            .filter(|record| !dispatch_record_is_terminal(record))
            .filter(|record| {
                record
                    .task_id
                    .as_deref()
                    .map(|task_id| scheduler.get_task(task_id).is_none())
                    .unwrap_or(true)
            })
            .map(|record| (record.step_id.clone(), record.attempt))
            .collect::<Vec<_>>();

        for (step_id, attempt) in recoverable {
            let dispatch = self.dispatch_for_step(bundle, &step_id, attempt)?;
            self.advance_dispatch(bundle, scheduler, &dispatch, now)?;
        }

        Ok(())
    }

    fn sync_scheduler_statuses(
        &self,
        bundle: &mut WorkflowRuntimeBundle,
        scheduler: &AgentScheduler,
        now: u64,
    ) -> Result<(), WorkflowError> {
        let task_statuses = bundle
            .dispatches
            .iter()
            .enumerate()
            .filter_map(|(index, record)| {
                let task_id = record.task_id.as_ref()?;
                let task = scheduler.get_task(task_id)?;
                Some((index, task_id.clone(), task.status.clone()))
            })
            .collect::<Vec<_>>();

        for (index, task_id, status) in task_statuses {
            self.sync_task_status(bundle, index, &task_id, &status, now)?;
        }

        Ok(())
    }

    fn sync_task_status(
        &self,
        bundle: &mut WorkflowRuntimeBundle,
        record_index: usize,
        task_id: &str,
        status: &TaskStatus,
        now: u64,
    ) -> Result<(), WorkflowError> {
        let (step_id, attempt) = {
            let record = &bundle.dispatches[record_index];
            (record.step_id.clone(), record.attempt)
        };
        let dispatch = self.dispatch_for_step(bundle, &step_id, attempt)?;
        let mut changed = false;

        match status {
            TaskStatus::Pending | TaskStatus::Approved { .. } | TaskStatus::Queued => {
                changed |= set_dispatch_state(
                    &mut bundle.dispatches[record_index],
                    WorkflowDispatchStatus::Queued,
                    Some("submitted and approved by orchestrator".to_string()),
                );
                changed |= set_workflow_step_status(
                    &mut bundle.workflow,
                    &step_id,
                    WorkflowStepStatus::Running,
                )?;
                changed |= add_active_step(&mut bundle.run.active_step_ids, &step_id);
                if changed {
                    bundle.updated_at = now.to_string();
                    self.store.write(bundle)?;
                }
                self.ensure_event_chain(&WorkflowRuntimeEventChain::from_step_queued(
                    &dispatch,
                    task_id,
                    now.to_string(),
                ))?;
            }
            TaskStatus::Running { .. } => {
                changed |= set_dispatch_state(
                    &mut bundle.dispatches[record_index],
                    WorkflowDispatchStatus::Running,
                    Some("scheduler task is running".to_string()),
                );
                changed |= set_workflow_step_status(
                    &mut bundle.workflow,
                    &step_id,
                    WorkflowStepStatus::Running,
                )?;
                changed |= add_active_step(&mut bundle.run.active_step_ids, &step_id);
                if changed {
                    bundle.updated_at = now.to_string();
                    self.store.write(bundle)?;
                }
                self.ensure_event_chain(&WorkflowRuntimeEventChain::from_step_running(
                    &dispatch,
                    task_id,
                    now.to_string(),
                ))?;
            }
            TaskStatus::Done { result, .. } => {
                changed |= set_dispatch_state(
                    &mut bundle.dispatches[record_index],
                    WorkflowDispatchStatus::Reported,
                    Some(result.clone()),
                );
                changed |= set_workflow_step_status(
                    &mut bundle.workflow,
                    &step_id,
                    WorkflowStepStatus::Reported,
                )?;
                changed |= remove_active_step(&mut bundle.run.active_step_ids, &step_id);
                if changed {
                    bundle.updated_at = now.to_string();
                    self.store.write(bundle)?;
                }
                self.ensure_event_chain(&WorkflowRuntimeEventChain::from_step_reported(
                    &dispatch,
                    task_id,
                    result.clone(),
                    now.to_string(),
                ))?;
            }
            TaskStatus::Error { message } => {
                changed |= set_dispatch_state(
                    &mut bundle.dispatches[record_index],
                    WorkflowDispatchStatus::Failed,
                    Some(message.clone()),
                );
                changed |= set_workflow_step_status(
                    &mut bundle.workflow,
                    &step_id,
                    WorkflowStepStatus::Failed,
                )?;
                changed |= remove_active_step(&mut bundle.run.active_step_ids, &step_id);
                if !matches!(bundle.run.status, RunStatus::Failed) {
                    bundle.run.status = RunStatus::Failed;
                    changed = true;
                }
                if changed {
                    bundle.updated_at = now.to_string();
                    self.store.write(bundle)?;
                }
                self.ensure_event_chain(&WorkflowRuntimeEventChain::from_step_failed(
                    &dispatch,
                    task_id,
                    "task_failed",
                    message.clone(),
                    now.to_string(),
                ))?;
            }
            TaskStatus::Cancelled => {
                let message = "scheduler task was cancelled".to_string();
                changed |= set_dispatch_state(
                    &mut bundle.dispatches[record_index],
                    WorkflowDispatchStatus::Failed,
                    Some(message.clone()),
                );
                changed |= set_workflow_step_status(
                    &mut bundle.workflow,
                    &step_id,
                    WorkflowStepStatus::Failed,
                )?;
                changed |= remove_active_step(&mut bundle.run.active_step_ids, &step_id);
                if !matches!(bundle.run.status, RunStatus::Failed) {
                    bundle.run.status = RunStatus::Failed;
                    changed = true;
                }
                if changed {
                    bundle.updated_at = now.to_string();
                    self.store.write(bundle)?;
                }
                self.ensure_event_chain(&WorkflowRuntimeEventChain::from_step_failed(
                    &dispatch,
                    task_id,
                    "task_cancelled",
                    message,
                    now.to_string(),
                ))?;
            }
        }

        Ok(())
    }

    fn advance_dispatch(
        &self,
        bundle: &mut WorkflowRuntimeBundle,
        scheduler: &mut AgentScheduler,
        dispatch: &StepDispatch,
        now: u64,
    ) -> Result<(), WorkflowError> {
        match WorkflowTaskAdapter::adapt(dispatch, now)? {
            AdaptedWorkflowTask::Task(task) => {
                let task_id = task.id.clone();
                if scheduler.get_task(&task_id).is_none() {
                    scheduler.submit(task);
                    scheduler
                        .approve(&task_id, "orchestrator")
                        .map_err(WorkflowError::RuntimeStateIo)?;
                }
                mark_dispatch_queued(bundle, dispatch, &task_id, now);
                set_workflow_step_status(
                    &mut bundle.workflow,
                    &dispatch.step_id,
                    WorkflowStepStatus::Running,
                )?;
                add_active_step(&mut bundle.run.active_step_ids, &dispatch.step_id);
                self.store.write(bundle)?;
                self.ensure_event_chain(&WorkflowRuntimeEventChain::from_step_queued(
                    dispatch,
                    task_id,
                    now.to_string(),
                ))?;
            }
            AdaptedWorkflowTask::Unsupported {
                reason_code,
                summary,
                ..
            } => {
                mark_dispatch_unsupported(bundle, dispatch, &reason_code, &summary, now);
                self.store.write(bundle)?;
                self.ensure_event_chain(&WorkflowRuntimeEventChain::from_step_unsupported(
                    dispatch,
                    reason_code,
                    summary,
                    now.to_string(),
                ))?;
            }
        }

        Ok(())
    }

    fn dispatch_for_step(
        &self,
        bundle: &WorkflowRuntimeBundle,
        step_id: &str,
        attempt: usize,
    ) -> Result<StepDispatch, WorkflowError> {
        let step = bundle
            .workflow
            .steps
            .iter()
            .find(|step| step.step_id == step_id)
            .ok_or_else(|| {
                WorkflowError::RuntimeStateParse(format!(
                    "dispatch record references missing step {step_id}"
                ))
            })?;
        dispatch_from_step(&bundle.run, &bundle.registry, step, attempt)
    }

    fn record_event_chain(&self, chain: &WorkflowRuntimeEventChain) -> Result<(), WorkflowError> {
        WorkflowRuntimeEventStore::append_chain(self.runtime_root(&chain.run_id)?, chain)?;
        Ok(())
    }

    fn ensure_event_chain(&self, chain: &WorkflowRuntimeEventChain) -> Result<(), WorkflowError> {
        let existing = WorkflowRuntimeEventStore::read_recent(
            self.runtime_root(&chain.run_id)?,
            &chain.run_id,
            0,
        )?;
        let missing = chain
            .events
            .iter()
            .filter(|event| {
                !existing
                    .iter()
                    .any(|candidate| runtime_event_matches(candidate, event))
            })
            .cloned()
            .collect::<Vec<_>>();

        if !missing.is_empty() {
            WorkflowRuntimeEventStore::append_events(
                self.runtime_root(&chain.run_id)?,
                &chain.run_id,
                &missing,
            )?;
        }
        Ok(())
    }

    fn repair_runtime_events(&self, bundle: &WorkflowRuntimeBundle) -> Result<(), WorkflowError> {
        self.ensure_event_chain(&WorkflowRuntimeEventChain::from_run_created(
            &bundle.workflow,
            &bundle.run,
            bundle.run.started_at.clone(),
        ))?;

        for record in &bundle.dispatches {
            let dispatch = self.dispatch_for_step(bundle, &record.step_id, record.attempt)?;
            self.ensure_event_chain(&WorkflowRuntimeEventChain::from_step_dispatches(
                std::slice::from_ref(&dispatch),
                bundle.updated_at.clone(),
            ))?;
            match record.status {
                WorkflowDispatchStatus::Queued => {
                    if let Some(task_id) = record.task_id.as_deref() {
                        self.ensure_event_chain(&WorkflowRuntimeEventChain::from_step_queued(
                            &dispatch,
                            task_id,
                            bundle.updated_at.clone(),
                        ))?;
                    }
                }
                WorkflowDispatchStatus::Running => {
                    if let Some(task_id) = record.task_id.as_deref() {
                        self.ensure_event_chain(&WorkflowRuntimeEventChain::from_step_queued(
                            &dispatch,
                            task_id,
                            bundle.updated_at.clone(),
                        ))?;
                        self.ensure_event_chain(&WorkflowRuntimeEventChain::from_step_running(
                            &dispatch,
                            task_id,
                            bundle.updated_at.clone(),
                        ))?;
                    }
                }
                WorkflowDispatchStatus::Reported => {
                    if let Some(task_id) = record.task_id.as_deref() {
                        self.ensure_event_chain(&WorkflowRuntimeEventChain::from_step_queued(
                            &dispatch,
                            task_id,
                            bundle.updated_at.clone(),
                        ))?;
                        self.ensure_event_chain(&WorkflowRuntimeEventChain::from_step_reported(
                            &dispatch,
                            task_id,
                            record.detail.clone().unwrap_or_default(),
                            bundle.updated_at.clone(),
                        ))?;
                    }
                }
                WorkflowDispatchStatus::Failed => {
                    if let Some(task_id) = record.task_id.as_deref() {
                        let (reason_code, summary) = failed_dispatch_detail(record);
                        self.ensure_event_chain(&WorkflowRuntimeEventChain::from_step_queued(
                            &dispatch,
                            task_id,
                            bundle.updated_at.clone(),
                        ))?;
                        self.ensure_event_chain(&WorkflowRuntimeEventChain::from_step_failed(
                            &dispatch,
                            task_id,
                            reason_code,
                            summary,
                            bundle.updated_at.clone(),
                        ))?;
                    }
                }
                WorkflowDispatchStatus::Unsupported => {
                    let (reason_code, summary) = unsupported_dispatch_detail(record);
                    self.ensure_event_chain(&WorkflowRuntimeEventChain::from_step_unsupported(
                        &dispatch,
                        reason_code,
                        summary,
                        bundle.updated_at.clone(),
                    ))?;
                }
                WorkflowDispatchStatus::Dispatched => {}
            }
        }

        Ok(())
    }

    fn runtime_root(&self, run_id: &str) -> Result<PathBuf, WorkflowError> {
        runtime_root_from_path(self.store.runtime_path(run_id)?)
    }
}

fn runtime_event_matches(
    candidate: &crate::harness::workflow::HarnessEvent,
    expected: &crate::harness::workflow::HarnessEvent,
) -> bool {
    if candidate.event_name != expected.event_name {
        return false;
    }
    for key in [
        "workflow_id",
        "run_id",
        "step_id",
        "attempt",
        "task_id",
        "reason_code",
    ] {
        if let Some(expected_value) = expected.attributes.get(key) {
            if candidate.attributes.get(key) != Some(expected_value) {
                return false;
            }
        }
    }
    true
}

fn unsupported_dispatch_detail(record: &WorkflowDispatchRecord) -> (String, String) {
    let Some(detail) = record.detail.as_deref() else {
        return (
            "unsupported_workflow_capability".to_string(),
            "Workflow step is not supported by the current runtime adapter.".to_string(),
        );
    };
    if let Some((reason_code, summary)) = detail.split_once(": ") {
        (reason_code.to_string(), summary.to_string())
    } else {
        (
            "unsupported_workflow_capability".to_string(),
            detail.to_string(),
        )
    }
}

fn failed_dispatch_detail(record: &WorkflowDispatchRecord) -> (String, String) {
    let summary = record.detail.clone().unwrap_or_default();
    if summary == "scheduler task was cancelled" {
        ("task_cancelled".to_string(), summary)
    } else {
        ("task_failed".to_string(), summary)
    }
}

fn dispatch_from_step(
    run: &WorkflowRunState,
    registry: &AgentRegistry,
    step: &WorkflowStep,
    attempt: usize,
) -> Result<StepDispatch, WorkflowError> {
    Ok(StepDispatch {
        workflow_id: run.workflow_id.clone(),
        run_id: run.run_id.clone(),
        step_id: step.step_id.clone(),
        agent_type: step.agent_type.clone(),
        capability: step.kind.clone(),
        mode: step.mode.clone(),
        attempt,
        inputs: step.inputs.clone(),
        artifact_contract: ArtifactContract {
            expected_output: step.task.clone(),
            acceptance_criteria: step.acceptance_criteria.clone(),
            success_clauses: step.goal_alignment.success_clauses.clone(),
        },
        sandbox_policy: registry.sandbox_for_step(run, step)?,
    })
}

fn runtime_root_from_path(runtime_path: PathBuf) -> Result<PathBuf, WorkflowError> {
    runtime_path
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            WorkflowError::RuntimeStateIo(format!(
                "runtime path {} has no vault root",
                runtime_path.display()
            ))
        })
}

fn dispatch_record<'a>(
    bundle: &'a WorkflowRuntimeBundle,
    step_id: &str,
    attempt: usize,
) -> Option<&'a WorkflowDispatchRecord> {
    bundle
        .dispatches
        .iter()
        .find(|record| record.step_id == step_id && record.attempt == attempt)
}

fn dispatch_record_mut<'a>(
    bundle: &'a mut WorkflowRuntimeBundle,
    step_id: &str,
    attempt: usize,
) -> Option<&'a mut WorkflowDispatchRecord> {
    bundle
        .dispatches
        .iter_mut()
        .find(|record| record.step_id == step_id && record.attempt == attempt)
}

fn dispatch_record_is_terminal(record: &WorkflowDispatchRecord) -> bool {
    matches!(
        record.status,
        WorkflowDispatchStatus::Reported
            | WorkflowDispatchStatus::Failed
            | WorkflowDispatchStatus::Unsupported
    )
}

fn mark_dispatch_queued(
    bundle: &mut WorkflowRuntimeBundle,
    dispatch: &StepDispatch,
    task_id: &str,
    now: u64,
) {
    if let Some(record) = dispatch_record_mut(bundle, &dispatch.step_id, dispatch.attempt) {
        record.task_id = Some(task_id.to_string());
        record.status = WorkflowDispatchStatus::Queued;
        record.detail = Some("submitted and approved by orchestrator".to_string());
    } else {
        bundle.dispatches.push(WorkflowDispatchRecord {
            step_id: dispatch.step_id.clone(),
            attempt: dispatch.attempt,
            task_id: Some(task_id.to_string()),
            status: WorkflowDispatchStatus::Queued,
            detail: Some("submitted and approved by orchestrator".to_string()),
        });
    }
    bundle.updated_at = now.to_string();
}

fn mark_dispatch_unsupported(
    bundle: &mut WorkflowRuntimeBundle,
    dispatch: &StepDispatch,
    reason_code: &str,
    summary: &str,
    now: u64,
) {
    let detail = format!("{reason_code}: {summary}");
    if let Some(record) = dispatch_record_mut(bundle, &dispatch.step_id, dispatch.attempt) {
        record.task_id = None;
        record.status = WorkflowDispatchStatus::Unsupported;
        record.detail = Some(detail);
    } else {
        bundle.dispatches.push(WorkflowDispatchRecord {
            step_id: dispatch.step_id.clone(),
            attempt: dispatch.attempt,
            task_id: None,
            status: WorkflowDispatchStatus::Unsupported,
            detail: Some(detail),
        });
    }
    bundle.updated_at = now.to_string();
}

fn set_dispatch_state(
    record: &mut WorkflowDispatchRecord,
    status: WorkflowDispatchStatus,
    detail: Option<String>,
) -> bool {
    let changed = record.status != status || record.detail != detail;
    if changed {
        record.status = status;
        record.detail = detail;
    }
    changed
}

fn set_workflow_step_status(
    workflow: &mut WorkflowIr,
    step_id: &str,
    status: WorkflowStepStatus,
) -> Result<bool, WorkflowError> {
    let step = workflow
        .steps
        .iter_mut()
        .find(|step| step.step_id == step_id)
        .ok_or_else(|| {
            WorkflowError::RuntimeStateParse(format!(
                "dispatch record references missing step {step_id}"
            ))
        })?;
    if step.status == status {
        Ok(false)
    } else {
        step.status = status;
        Ok(true)
    }
}

fn add_active_step(active_step_ids: &mut Vec<String>, step_id: &str) -> bool {
    if !active_step_ids.iter().any(|active| active == step_id) {
        active_step_ids.push(step_id.to_string());
        true
    } else {
        false
    }
}

fn remove_active_step(active_step_ids: &mut Vec<String>, step_id: &str) -> bool {
    let original_len = active_step_ids.len();
    active_step_ids.retain(|active| active != step_id);
    active_step_ids.len() != original_len
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::agent_scheduler::AgentScheduler;
    use crate::harness::workflow::{
        AgentRegistry, AgentRegistryEntry, ControlPolicy, GoalAlignment, GoalContract, RetryPolicy,
        WorkflowIr, WorkflowStatus, WorkflowStep, WorkflowStepKind, WorkflowStepMode,
        WorkflowStepStatus,
    };
    use crate::harness::workflow_runtime::WorkflowDispatchStatus;
    use serde_json::json;

    fn goal() -> GoalContract {
        GoalContract {
            goal_id: "goal_demo".to_string(),
            goal_text: "Dispatch the first narrow-loop workflow step".to_string(),
            success_definition: vec!["A ready step is durably reconciled".to_string()],
            non_goals: vec![],
            constraints: json!({}),
            context_scope: vec!["src-tauri/src/ai/**".to_string()],
            approval_policy: json!({}),
            budget: json!({"max_iterations": 2}),
            created_at: "2026-06-22T00:00:00Z".to_string(),
        }
    }

    fn registry() -> AgentRegistry {
        AgentRegistry::from_agents(vec![
            AgentRegistryEntry {
                agent_type: "research_subagent".to_string(),
                allowed_tools: vec!["Read".to_string(), "arxiv_search".to_string()],
                denied_tools: vec!["Write".to_string()],
                default_mode: WorkflowStepMode::ReadOnly,
                max_parallelism: 1,
                can_delegate: false,
            },
            AgentRegistryEntry {
                agent_type: "code_writer".to_string(),
                allowed_tools: vec!["Read".to_string(), "Edit".to_string()],
                denied_tools: vec!["Shell".to_string()],
                default_mode: WorkflowStepMode::WritePatch,
                max_parallelism: 1,
                can_delegate: false,
            },
        ])
    }

    fn workflow_with_ready_research() -> WorkflowIr {
        workflow_with_ready_step(
            WorkflowStepKind::Research,
            "research_subagent",
            WorkflowStepMode::ReadOnly,
        )
    }

    fn workflow_with_ready_implement() -> WorkflowIr {
        workflow_with_ready_step(
            WorkflowStepKind::Implement,
            "code_writer",
            WorkflowStepMode::WritePatch,
        )
    }

    fn workflow_with_ready_step(
        kind: WorkflowStepKind,
        agent_type: &str,
        mode: WorkflowStepMode,
    ) -> WorkflowIr {
        WorkflowIr {
            workflow_id: "wf_demo".to_string(),
            goal_id: "goal_demo".to_string(),
            version: 1,
            parent_version: None,
            status: WorkflowStatus::Running,
            global_context: json!({}),
            control_policy: ControlPolicy {
                max_parallel_steps: 1,
                replan_on_verification_fail: true,
                max_patch_chain: 2,
            },
            steps: vec![WorkflowStep {
                step_id: "S001".to_string(),
                title: "Reconcile first ready step".to_string(),
                kind,
                agent_type: agent_type.to_string(),
                mode,
                task: "Return a narrow-loop step report".to_string(),
                inputs: json!({"question": "What evidence should the workflow capture?"}),
                dependencies: vec![],
                acceptance_criteria: vec!["A dispatch record is persisted".to_string()],
                goal_alignment: GoalAlignment {
                    success_clauses: vec![1],
                    why_necessary: "The run cannot progress without dispatch reconciliation"
                        .to_string(),
                },
                retry_policy: RetryPolicy {
                    max_attempts: 1,
                    backoff_ms: 0,
                },
                status: WorkflowStepStatus::Ready,
            }],
            created_by: "orchestrator@v1".to_string(),
            created_at: "2026-06-22T00:00:01Z".to_string(),
        }
    }

    fn runtime_events(
        root: &std::path::Path,
        run_id: &str,
    ) -> Vec<crate::harness::workflow::HarnessEvent> {
        WorkflowRuntimeEventStore::read_recent(root, run_id, 10).unwrap()
    }

    #[test]
    fn start_persists_run_and_queues_research_once() {
        let root = tempfile::tempdir().unwrap();
        let service = WorkflowRuntimeService::new(root.path());
        let mut scheduler = AgentScheduler::new(2);

        let bundle = service
            .start(
                goal(),
                workflow_with_ready_research(),
                registry(),
                "run_demo",
                &mut scheduler,
                100,
            )
            .unwrap();

        assert_eq!(bundle.dispatches[0].status, WorkflowDispatchStatus::Queued);
        let task_id = bundle.dispatches[0].task_id.as_deref().unwrap();
        assert!(scheduler.get_task(task_id).is_some());
        assert_eq!(service.get("run_demo").unwrap(), bundle);
        let events = runtime_events(root.path(), "run_demo");
        assert_eq!(
            events
                .iter()
                .map(|event| event.event_name.as_str())
                .collect::<Vec<_>>(),
            vec![
                "workflow.run.created",
                "workflow.step.dispatched",
                "workflow.step.queued"
            ]
        );
        assert_eq!(events[2].attributes["task_id"], task_id);
    }

    #[test]
    fn resume_is_idempotent_when_scheduler_already_has_task() {
        let root = tempfile::tempdir().unwrap();
        let service = WorkflowRuntimeService::new(root.path());
        let mut scheduler = AgentScheduler::new(2);
        service
            .start(
                goal(),
                workflow_with_ready_research(),
                registry(),
                "run_demo",
                &mut scheduler,
                100,
            )
            .unwrap();

        service.resume("run_demo", &mut scheduler, 101).unwrap();

        assert_eq!(
            scheduler
                .list_tasks(None)
                .iter()
                .filter(|task| task.id == "workflow__run_demo__S001__attempt_1")
                .count(),
            1
        );
        let events = runtime_events(root.path(), "run_demo");
        assert!(events
            .iter()
            .any(|event| event.event_name == "workflow.run.resumed"));
    }

    #[test]
    fn resume_repairs_missing_state_events_without_duplicate_tasks() {
        let root = tempfile::tempdir().unwrap();
        let service = WorkflowRuntimeService::new(root.path());
        let mut scheduler = AgentScheduler::new(2);
        service
            .start(
                goal(),
                workflow_with_ready_research(),
                registry(),
                "run_demo",
                &mut scheduler,
                100,
            )
            .unwrap();
        let event_log = WorkflowRuntimeEventStore::event_log_path(root.path(), "run_demo").unwrap();
        std::fs::write(&event_log, "").unwrap();

        service.resume("run_demo", &mut scheduler, 101).unwrap();
        service.resume("run_demo", &mut scheduler, 102).unwrap();

        assert_eq!(
            scheduler
                .list_tasks(None)
                .iter()
                .filter(|task| task.id == "workflow__run_demo__S001__attempt_1")
                .count(),
            1
        );
        let events = runtime_events(root.path(), "run_demo");
        assert!(events
            .iter()
            .any(|event| event.event_name == "workflow.run.created"));
        assert_eq!(
            events
                .iter()
                .filter(|event| event.event_name == "workflow.step.dispatched")
                .count(),
            1
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| event.event_name == "workflow.step.queued")
                .count(),
            1
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| event.event_name == "workflow.run.resumed")
                .count(),
            2
        );
    }

    #[test]
    fn resume_recovers_missing_scheduler_task_without_duplicate_dispatch() {
        let root = tempfile::tempdir().unwrap();
        let service = WorkflowRuntimeService::new(root.path());
        let mut scheduler = AgentScheduler::new(2);
        service
            .start(
                goal(),
                workflow_with_ready_research(),
                registry(),
                "run_demo",
                &mut scheduler,
                100,
            )
            .unwrap();
        let mut recovered_scheduler = AgentScheduler::new(2);

        let bundle = service
            .resume("run_demo", &mut recovered_scheduler, 101)
            .unwrap();

        assert!(recovered_scheduler
            .get_task("workflow__run_demo__S001__attempt_1")
            .is_some());
        assert_eq!(
            bundle
                .dispatches
                .iter()
                .filter(|record| record.step_id == "S001" && record.attempt == 1)
                .count(),
            1
        );
        assert_eq!(bundle.dispatches[0].status, WorkflowDispatchStatus::Queued);
        assert_eq!(bundle.run.active_step_ids, vec!["S001".to_string()]);
    }

    #[test]
    fn resume_syncs_running_and_reported_scheduler_task() {
        let root = tempfile::tempdir().unwrap();
        let service = WorkflowRuntimeService::new(root.path());
        let mut scheduler = AgentScheduler::new(2);
        service
            .start(
                goal(),
                workflow_with_ready_research(),
                registry(),
                "run_demo",
                &mut scheduler,
                100,
            )
            .unwrap();
        let task = scheduler.dequeue().unwrap();

        let running = service.resume("run_demo", &mut scheduler, 101).unwrap();

        assert_eq!(
            running.dispatches[0].status,
            WorkflowDispatchStatus::Running
        );
        assert_eq!(
            running.workflow.steps[0].status,
            WorkflowStepStatus::Running
        );
        assert_eq!(running.run.active_step_ids, vec!["S001".to_string()]);
        scheduler.complete(&task.id, "research report body".to_string());

        let reported = service.resume("run_demo", &mut scheduler, 102).unwrap();

        assert_eq!(
            reported.dispatches[0].status,
            WorkflowDispatchStatus::Reported
        );
        assert_eq!(
            reported.dispatches[0].detail.as_deref(),
            Some("research report body")
        );
        assert_eq!(
            reported.workflow.steps[0].status,
            WorkflowStepStatus::Reported
        );
        assert!(reported.run.active_step_ids.is_empty());
        let events = runtime_events(root.path(), "run_demo");
        assert!(events
            .iter()
            .any(|event| event.event_name == "workflow.step.running"));
        assert!(events
            .iter()
            .any(|event| event.event_name == "workflow.step.reported"));
    }

    #[test]
    fn resume_syncs_failed_scheduler_task() {
        let root = tempfile::tempdir().unwrap();
        let service = WorkflowRuntimeService::new(root.path());
        let mut scheduler = AgentScheduler::new(2);
        service
            .start(
                goal(),
                workflow_with_ready_research(),
                registry(),
                "run_demo",
                &mut scheduler,
                100,
            )
            .unwrap();
        let task = scheduler.dequeue().unwrap();
        scheduler.fail(&task.id, "retrieval failed".to_string());

        let failed = service.resume("run_demo", &mut scheduler, 101).unwrap();

        assert_eq!(failed.dispatches[0].status, WorkflowDispatchStatus::Failed);
        assert_eq!(
            failed.dispatches[0].detail.as_deref(),
            Some("retrieval failed")
        );
        assert_eq!(failed.workflow.steps[0].status, WorkflowStepStatus::Failed);
        assert!(matches!(failed.run.status, RunStatus::Failed));
        assert!(failed.run.active_step_ids.is_empty());
        let events = runtime_events(root.path(), "run_demo");
        let failed_event = events
            .iter()
            .find(|event| event.event_name == "workflow.step.failed")
            .unwrap();
        assert_eq!(failed_event.attributes["reason_code"], "task_failed");
    }

    #[test]
    fn resume_all_recovers_each_persisted_run() {
        let root = tempfile::tempdir().unwrap();
        let service = WorkflowRuntimeService::new(root.path());
        let mut scheduler = AgentScheduler::new(2);
        service
            .start(
                goal(),
                workflow_with_ready_research(),
                registry(),
                "run_alpha",
                &mut scheduler,
                100,
            )
            .unwrap();
        service
            .start(
                goal(),
                workflow_with_ready_research(),
                registry(),
                "run_beta",
                &mut scheduler,
                101,
            )
            .unwrap();
        let mut recovered_scheduler = AgentScheduler::new(2);

        let bundles = service.resume_all(&mut recovered_scheduler, 200).unwrap();

        assert!(bundles.errors.is_empty());
        assert_eq!(bundles.resumed.len(), 2);
        assert!(recovered_scheduler
            .get_task("workflow__run_alpha__S001__attempt_1")
            .is_some());
        assert!(recovered_scheduler
            .get_task("workflow__run_beta__S001__attempt_1")
            .is_some());
    }

    #[test]
    fn resume_all_continues_after_corrupt_runtime_state() {
        let root = tempfile::tempdir().unwrap();
        let service = WorkflowRuntimeService::new(root.path());
        let mut scheduler = AgentScheduler::new(2);
        service
            .start(
                goal(),
                workflow_with_ready_research(),
                registry(),
                "run_alpha",
                &mut scheduler,
                100,
            )
            .unwrap();
        service
            .start(
                goal(),
                workflow_with_ready_research(),
                registry(),
                "run_beta",
                &mut scheduler,
                101,
            )
            .unwrap();
        std::fs::write(
            service.store.runtime_path("run_alpha").unwrap(),
            b"{\"broken\": true",
        )
        .unwrap();
        let mut recovered_scheduler = AgentScheduler::new(2);

        let result = service.resume_all(&mut recovered_scheduler, 200).unwrap();

        assert_eq!(result.resumed.len(), 1);
        assert_eq!(result.resumed[0].run.run_id, "run_beta");
        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.errors[0].run_id, "run_alpha");
        assert!(result.errors[0].error.contains("parse"));
        assert!(recovered_scheduler
            .get_task("workflow__run_beta__S001__attempt_1")
            .is_some());
    }

    #[test]
    fn unsupported_step_is_durable_and_never_submitted() {
        let root = tempfile::tempdir().unwrap();
        let service = WorkflowRuntimeService::new(root.path());
        let mut scheduler = AgentScheduler::new(2);

        let bundle = service
            .start(
                goal(),
                workflow_with_ready_implement(),
                registry(),
                "run_demo",
                &mut scheduler,
                100,
            )
            .unwrap();

        assert_eq!(
            bundle.dispatches[0].status,
            WorkflowDispatchStatus::Unsupported
        );
        assert!(bundle.dispatches[0].task_id.is_none());
        assert!(scheduler.list_tasks(None).is_empty());
        let events = runtime_events(root.path(), "run_demo");
        assert_eq!(
            events
                .iter()
                .map(|event| event.event_name.as_str())
                .collect::<Vec<_>>(),
            vec![
                "workflow.run.created",
                "workflow.step.dispatched",
                "workflow.step.unsupported"
            ]
        );
        assert_eq!(
            events[2].attributes["reason_code"],
            "unsupported_workflow_capability"
        );
    }
}
