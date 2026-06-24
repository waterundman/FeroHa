use super::agent_scheduler::{AgentScheduler, AgentTask, TaskStatus};
use super::dream_memory;
use super::manager::AiManagerService;
use super::research_trace;
use super::workflow_task_adapter::{
    workflow_task_context, AdaptedWorkflowTask, WorkflowTaskAdapter,
};
use crate::harness::scientist::{Scientist, ScientistResult};
use crate::harness::workflow::{
    safe_runtime_component, AgentRegistry, ArtifactContract, ArtifactRef, EvidenceRef, GoalContract,
    OrchestratorOutput, RunStatus, StepDispatch, StepReport, StepReportStatus,
    VerificationFinding, VerificationLevel, VerificationOutcome, WorkflowError, WorkflowIr,
    WorkflowRunState, WorkflowRuntimeEventChain, WorkflowRuntimeEventStore, WorkflowStatus,
    WorkflowStep, WorkflowStepStatus,
};
use crate::harness::workflow_runtime::{
    WorkflowDispatchRecord, WorkflowDispatchStatus, WorkflowRuntimeBundle, WorkflowRuntimeStore,
    WorkflowTaskContext,
};
use std::cmp::Ordering;
use std::collections::HashSet;
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

    pub fn list(&self) -> Result<Vec<WorkflowRuntimeBundle>, WorkflowError> {
        let mut bundles = self
            .store
            .list_run_ids()?
            .into_iter()
            .map(|run_id| self.store.read(&run_id))
            .collect::<Result<Vec<_>, _>>()?;
        bundles.sort_by(|left, right| compare_updated_at_desc(&left.updated_at, &right.updated_at));
        Ok(bundles)
    }

    pub fn record_task_running(
        &self,
        task: &AgentTask,
        now: u64,
    ) -> Result<Option<WorkflowRuntimeBundle>, WorkflowError> {
        let Some(context) = workflow_task_context(task) else {
            return Ok(None);
        };
        let mut bundle = self.store.read(&context.run_id)?;
        validate_task_runtime_context(&bundle, task, &context)?;
        let dispatch =
            self.dispatch_for_step(&bundle, &context.step_id, context.attempt)?;
        let record = dispatch_record_mut(&mut bundle, &context.step_id, context.attempt)
            .ok_or_else(|| missing_task_dispatch_error(task, &context.step_id))?;
        set_dispatch_state(
            record,
            WorkflowDispatchStatus::Running,
            Some("scheduler task is running".to_string()),
        );
        set_workflow_step_status(
            &mut bundle.workflow,
            &context.step_id,
            WorkflowStepStatus::Running,
        )?;
        add_active_step(&mut bundle.run.active_step_ids, &context.step_id);
        bundle.updated_at = now.to_string();
        self.store.write(&bundle)?;
        self.ensure_event_chain(&WorkflowRuntimeEventChain::from_step_running(
            &dispatch,
            &task.id,
            now.to_string(),
        ))?;
        Ok(Some(bundle))
    }

    pub fn record_task_completion(
        &self,
        task: &AgentTask,
        now: u64,
        _scheduler: &mut AgentScheduler,
    ) -> Result<Option<WorkflowRuntimeBundle>, WorkflowError> {
        let Some(context) = workflow_task_context(task) else {
            return Ok(None);
        };
        if !matches!(task.status, TaskStatus::Done { .. }) {
            return Err(WorkflowError::RuntimeStateParse(format!(
                "workflow task {} is not completed",
                task.id
            )));
        }

        let mut bundle = self.store.read(&context.run_id)?;
        validate_task_runtime_context(&bundle, task, &context)?;
        let dispatch =
            self.dispatch_for_step(&bundle, &context.step_id, context.attempt)?;
        let runtime_root = self.runtime_root(&context.run_id)?;
        let dualtrack_dir = runtime_root.join(".dualtrack");
        let task_id = safe_runtime_component(&task.id)?;
        let result_path = dualtrack_dir
            .join("research")
            .join("results")
            .join(task_id)
            .join("result.md");
        let result_content = std::fs::read_to_string(&result_path).ok();
        let working_artifact = dream_memory::working_result_artifact(
            &runtime_root,
            task_id,
            &context.step_id,
            &now.to_string(),
        )
        .ok();
        if let Some(artifact) = working_artifact.clone() {
            upsert_artifact(&mut bundle.artifacts, artifact);
        }

        let trace = research_trace::get_task_trace(&dualtrack_dir, task_id).ok();
        let knowledge = Scientist::extract_knowledge(task);
        let source_refs = task_source_refs(task);
        let confidence = task_evidence_confidence(task);
        let checked_criteria =
            checked_acceptance_criteria(result_content.as_deref().unwrap_or_default());
        let evidence_refs = working_artifact
            .iter()
            .map(|artifact| artifact.uri.clone())
            .chain(source_refs.iter().cloned())
            .collect::<Vec<_>>();
        let step = bundle
            .workflow
            .steps
            .iter()
            .find(|step| step.step_id == context.step_id)
            .cloned()
            .ok_or_else(|| missing_task_dispatch_error(task, &context.step_id))?;
        let findings = build_completion_findings(CompletionChecks {
            run_id: &context.run_id,
            task_id,
            step: &step,
            working_artifact: working_artifact.as_ref(),
            result_content: result_content.as_deref(),
            trace_complete: trace.is_some(),
            context_present: trace
                .as_ref()
                .and_then(|trace| trace.context.as_ref())
                .is_some(),
            evidence_present: !source_refs.is_empty(),
            claims_present: !knowledge.claims.is_empty(),
            checked_criteria: &checked_criteria,
            evidence_refs: &evidence_refs,
        });
        let all_pass = findings
            .iter()
            .all(|finding| finding.result == VerificationOutcome::Pass);
        let report = StepReport {
            report_id: format!(
                "report_{}_{}_a{}",
                context.run_id, context.step_id, context.attempt
            ),
            step_id: context.step_id.clone(),
            attempt: context.attempt,
            status: if working_artifact.is_some()
                && result_content
                    .as_deref()
                    .map(str::trim)
                    .is_some_and(|result| !result.is_empty())
            {
                StepReportStatus::Completed
            } else {
                StepReportStatus::Failed
            },
            summary: "Research result recorded in Dream Working Memory".to_string(),
            artifacts: working_artifact.iter().cloned().collect(),
            evidence: source_refs
                .iter()
                .map(|source| EvidenceRef {
                    file: source.clone(),
                    lines: Vec::new(),
                    claim: "retrieval evidence reference".to_string(),
                })
                .collect(),
            risks: findings
                .iter()
                .filter(|finding| finding.result != VerificationOutcome::Pass)
                .map(|finding| finding.reason_code.clone())
                .collect(),
            blocked_by: Vec::new(),
            suggested_next_steps: Vec::new(),
            resource_usage: serde_json::json!({
                "claim_count": knowledge.claims.len(),
                "evidence_reference_count": source_refs.len(),
            }),
            confidence,
        };
        bundle
            .step_reports
            .retain(|existing| existing.report_id != report.report_id);
        bundle.step_reports.push(report.clone());
        bundle
            .verification_findings
            .retain(|finding| finding.target != context.step_id);
        bundle.verification_findings.extend(findings.clone());

        let record = dispatch_record_mut(&mut bundle, &context.step_id, context.attempt)
            .ok_or_else(|| missing_task_dispatch_error(task, &context.step_id))?;
        set_dispatch_state(
            record,
            WorkflowDispatchStatus::Reported,
            Some("research result recorded".to_string()),
        );
        remove_active_step(&mut bundle.run.active_step_ids, &context.step_id);
        bundle.updated_at = now.to_string();

        let semantic_artifact = if all_pass {
            let working_artifact = working_artifact.as_ref().ok_or_else(|| {
                WorkflowError::RuntimeStateIo(
                    "verified workflow result has no Working artifact".to_string(),
                )
            })?;
            let scientist_result =
                Scientist::build_result(knowledge, None, None, None, confidence);
            let semantic_content = render_semantic_workflow_memory(
                &bundle,
                task,
                working_artifact,
                &scientist_result,
                &source_refs,
                &step,
            );
            let artifact = dream_memory::write_semantic_workflow_memory(
                &dualtrack_dir,
                &bundle.workflow.workflow_id,
                &bundle.run.run_id,
                &context.step_id,
                &semantic_content,
                &now.to_string(),
            )
            .map_err(WorkflowError::RuntimeStateIo)?;
            upsert_artifact(&mut bundle.artifacts, artifact.clone());
            set_workflow_step_status(
                &mut bundle.workflow,
                &context.step_id,
                WorkflowStepStatus::Verified,
            )?;
            bundle.workflow.status = WorkflowStatus::Completed;
            bundle.run.status = RunStatus::Succeeded;
            bundle.run.ended_at = Some(now.to_string());
            Some(artifact)
        } else {
            set_workflow_step_status(
                &mut bundle.workflow,
                &context.step_id,
                WorkflowStepStatus::Failed,
            )?;
            bundle.workflow.status = WorkflowStatus::Aborted;
            bundle.run.status = RunStatus::Failed;
            bundle.run.ended_at = Some(now.to_string());
            None
        };

        self.store.write(&bundle)?;
        self.ensure_event_chain(&WorkflowRuntimeEventChain::from_step_reported(
            &dispatch,
            &task.id,
            "Research result persisted in Dream Working Memory",
            now.to_string(),
        ))?;
        self.ensure_event_chain(&WorkflowRuntimeEventChain::from_step_assessed(
            &bundle.workflow.workflow_id,
            &bundle.run.run_id,
            &report,
            &findings,
            now.to_string(),
        ))?;

        if let (Some(working_artifact), Some(semantic_artifact)) =
            (working_artifact.as_ref(), semantic_artifact.as_ref())
        {
            self.ensure_event_chain(&WorkflowRuntimeEventChain::from_step_verified(
                &dispatch,
                &task.id,
                working_artifact,
                now.to_string(),
            ))?;
            self.ensure_event_chain(&WorkflowRuntimeEventChain::from_semantic_promoted(
                &bundle.workflow.workflow_id,
                &bundle.run.run_id,
                &context.step_id,
                semantic_artifact,
                now.to_string(),
            ))?;
            self.ensure_event_chain(&WorkflowRuntimeEventChain::from_run_succeeded(
                &bundle.workflow,
                &bundle.run,
                now.to_string(),
            ))?;
        } else {
            let reason_code = findings
                .iter()
                .find(|finding| finding.result != VerificationOutcome::Pass)
                .map(|finding| finding.reason_code.as_str())
                .unwrap_or("verification_failed");
            self.ensure_event_chain(&WorkflowRuntimeEventChain::from_run_failed(
                &bundle.workflow,
                &bundle.run,
                reason_code,
                "Workflow verification failed; Working artifacts were retained.",
                now.to_string(),
            ))?;
        }

        Ok(Some(bundle))
    }

    pub fn record_task_failure(
        &self,
        task: &AgentTask,
        reason_code: &str,
        summary: &str,
        now: u64,
    ) -> Result<Option<WorkflowRuntimeBundle>, WorkflowError> {
        let Some(context) = workflow_task_context(task) else {
            return Ok(None);
        };
        let mut bundle = self.store.read(&context.run_id)?;
        validate_task_runtime_context(&bundle, task, &context)?;
        let dispatch =
            self.dispatch_for_step(&bundle, &context.step_id, context.attempt)?;
        let record = dispatch_record_mut(&mut bundle, &context.step_id, context.attempt)
            .ok_or_else(|| missing_task_dispatch_error(task, &context.step_id))?;
        set_dispatch_state(
            record,
            WorkflowDispatchStatus::Failed,
            Some(summary.to_string()),
        );
        set_workflow_step_status(
            &mut bundle.workflow,
            &context.step_id,
            WorkflowStepStatus::Failed,
        )?;
        remove_active_step(&mut bundle.run.active_step_ids, &context.step_id);
        bundle.workflow.status = WorkflowStatus::Aborted;
        bundle.run.status = RunStatus::Failed;
        bundle.run.ended_at = Some(now.to_string());
        bundle.updated_at = now.to_string();
        self.store.write(&bundle)?;
        self.ensure_event_chain(&WorkflowRuntimeEventChain::from_step_failed(
            &dispatch,
            &task.id,
            reason_code,
            summary,
            now.to_string(),
        ))?;
        self.ensure_event_chain(&WorkflowRuntimeEventChain::from_run_failed(
            &bundle.workflow,
            &bundle.run,
            reason_code,
            summary,
            now.to_string(),
        ))?;
        Ok(Some(bundle))
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
            artifacts: Vec::new(),
            step_reports: Vec::new(),
            verification_findings: Vec::new(),
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
            TaskStatus::Done { .. } => {
                changed |= set_dispatch_state(
                    &mut bundle.dispatches[record_index],
                    WorkflowDispatchStatus::Reported,
                    Some("research result recorded".to_string()),
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
                    "Research result persisted in Dream Working Memory".to_string(),
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
                    let mut manager = AiManagerService::new(scheduler);
                    manager.submit(task);
                    manager
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

fn compare_updated_at_desc(left: &str, right: &str) -> Ordering {
    match (left.parse::<u128>(), right.parse::<u128>()) {
        (Ok(left), Ok(right)) => right.cmp(&left),
        _ => right.cmp(left),
    }
}

fn validate_task_runtime_context(
    bundle: &WorkflowRuntimeBundle,
    task: &AgentTask,
    context: &WorkflowTaskContext,
) -> Result<(), WorkflowError> {
    if context.workflow_id != bundle.workflow.workflow_id {
        return Err(WorkflowError::RuntimeStateParse(format!(
            "workflow task {} references workflow {}, expected {}",
            task.id, context.workflow_id, bundle.workflow.workflow_id
        )));
    }
    let record = dispatch_record(bundle, &context.step_id, context.attempt)
        .ok_or_else(|| missing_task_dispatch_error(task, &context.step_id))?;
    if record.task_id.as_deref() != Some(task.id.as_str()) {
        return Err(WorkflowError::RuntimeStateParse(format!(
            "workflow task {} does not match dispatch task {:?}",
            task.id, record.task_id
        )));
    }
    Ok(())
}

fn missing_task_dispatch_error(task: &AgentTask, step_id: &str) -> WorkflowError {
    WorkflowError::RuntimeStateParse(format!(
        "workflow task {} references missing dispatch for step {}",
        task.id, step_id
    ))
}

fn upsert_artifact(artifacts: &mut Vec<ArtifactRef>, artifact: ArtifactRef) {
    artifacts.retain(|existing| {
        existing.artifact_id != artifact.artifact_id && existing.uri != artifact.uri
    });
    artifacts.push(artifact);
}

fn task_source_refs(task: &AgentTask) -> Vec<String> {
    let mut refs = Vec::new();
    let mut seen = HashSet::new();
    for result in &task.subagent_results {
        for entry in &result.entries {
            let source_ref = entry
                .url
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .or_else(|| {
                    let title = entry.title.trim();
                    (!title.is_empty()).then(|| format!("{}:{}", entry.source, title))
                });
            if let Some(source_ref) = source_ref {
                if seen.insert(source_ref.clone()) {
                    refs.push(source_ref);
                }
            }
        }
    }
    refs
}

fn task_evidence_confidence(task: &AgentTask) -> f32 {
    task.subagent_results
        .iter()
        .flat_map(|result| result.entries.iter())
        .map(|entry| entry.relevance_score)
        .fold(0.0_f32, f32::max)
        .clamp(0.0, 1.0)
}

fn checked_acceptance_criteria(markdown: &str) -> HashSet<String> {
    let mut in_acceptance_section = false;
    let mut checked = HashSet::new();

    for line in markdown.lines() {
        let trimmed = line.trim();
        if let Some(heading) = trimmed.strip_prefix("## ") {
            in_acceptance_section = heading.trim().eq_ignore_ascii_case("Acceptance Check");
            continue;
        }
        if !in_acceptance_section {
            continue;
        }
        let criterion = trimmed
            .strip_prefix("- [x] ")
            .or_else(|| trimmed.strip_prefix("- [X] "));
        if let Some(criterion) = criterion {
            checked.insert(normalize_contract_text(criterion));
        }
    }

    checked
}

fn normalize_contract_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[derive(Clone, Copy)]
struct CompletionChecks<'a> {
    run_id: &'a str,
    task_id: &'a str,
    step: &'a WorkflowStep,
    working_artifact: Option<&'a ArtifactRef>,
    result_content: Option<&'a str>,
    trace_complete: bool,
    context_present: bool,
    evidence_present: bool,
    claims_present: bool,
    checked_criteria: &'a HashSet<String>,
    evidence_refs: &'a [String],
}

fn build_completion_findings(checks: CompletionChecks<'_>) -> Vec<VerificationFinding> {
    let all_clauses = checks.step.goal_alignment.success_clauses.clone();
    let expected_working_uri = format!(
        ".dualtrack/research/results/{}/result.md",
        checks.task_id
    );
    let mut findings = Vec::new();
    push_completion_finding(
        &mut findings,
        checks,
        "working_artifact",
        checks.working_artifact.is_some(),
        "working_artifact_present",
        "working_artifact_missing",
        "Working result artifact is present.",
        "Working result artifact is missing.",
        &all_clauses,
        vec![expected_working_uri.clone()],
    );
    let result_present = checks
        .result_content
        .map(str::trim)
        .is_some_and(|result| !result.is_empty());
    push_completion_finding(
        &mut findings,
        checks,
        "result_content",
        result_present,
        "research_result_present",
        "empty_research_result",
        "Research result contains report content.",
        "Research result is empty or unreadable.",
        &all_clauses,
        vec![expected_working_uri.clone()],
    );
    push_completion_finding(
        &mut findings,
        checks,
        "trace",
        checks.trace_complete,
        "research_trace_present",
        "research_trace_missing",
        "Research path and trace files are present.",
        "Research path or trace files are missing.",
        &all_clauses,
        vec![format!(
            ".dualtrack/research/paths/{}",
            checks.task_id
        )],
    );
    push_completion_finding(
        &mut findings,
        checks,
        "context",
        checks.context_present,
        "research_context_present",
        "research_context_missing",
        "Research context is present.",
        "Research context is missing.",
        &all_clauses,
        vec![expected_working_uri.clone()],
    );
    push_completion_finding(
        &mut findings,
        checks,
        "evidence",
        checks.evidence_present,
        "evidence_present",
        "evidence_missing",
        "Retrieval evidence references are present.",
        "Retrieval evidence references are missing.",
        &all_clauses,
        vec![expected_working_uri.clone()],
    );
    push_completion_finding(
        &mut findings,
        checks,
        "claims",
        checks.claims_present,
        "claims_present",
        "claims_missing",
        "Scientist extracted claims from the research report.",
        "Scientist could not extract claims from the research report.",
        &all_clauses,
        vec![expected_working_uri],
    );

    for (index, criterion) in checks.step.acceptance_criteria.iter().enumerate() {
        let normalized = normalize_contract_text(criterion);
        let satisfied = checks.checked_criteria.contains(&normalized);
        let clause = checks
            .step
            .goal_alignment
            .success_clauses
            .get(index)
            .copied()
            .into_iter()
            .collect::<Vec<_>>();
        push_completion_finding(
            &mut findings,
            checks,
            &format!("acceptance_{}", index + 1),
            satisfied,
            "acceptance_criterion_checked",
            "acceptance_criterion_unchecked",
            &format!("Acceptance criterion {} is checked.", index + 1),
            &format!("Acceptance criterion {} is not checked.", index + 1),
            &clause,
            vec![criterion.clone()],
        );
    }

    findings
}

#[allow(clippy::too_many_arguments)]
fn push_completion_finding(
    findings: &mut Vec<VerificationFinding>,
    checks: CompletionChecks<'_>,
    suffix: &str,
    passed: bool,
    pass_reason_code: &str,
    fail_reason_code: &str,
    pass_summary: &str,
    fail_summary: &str,
    failed_clauses: &[usize],
    minimal_fix_surface: Vec<String>,
) {
    findings.push(VerificationFinding {
        verification_id: format!(
            "verify_{}_{}_{}",
            checks.run_id, checks.step.step_id, suffix
        ),
        level: VerificationLevel::Step,
        target: checks.step.step_id.clone(),
        result: if passed {
            VerificationOutcome::Pass
        } else {
            VerificationOutcome::Fail
        },
        failed_clauses: if passed {
            Vec::new()
        } else {
            failed_clauses.to_vec()
        },
        reason_code: if passed {
            pass_reason_code.to_string()
        } else {
            fail_reason_code.to_string()
        },
        summary: if passed {
            pass_summary.to_string()
        } else {
            fail_summary.to_string()
        },
        evidence_refs: checks.evidence_refs.to_vec(),
        minimal_fix_surface: if passed {
            Vec::new()
        } else {
            minimal_fix_surface
        },
    });
}

fn render_semantic_workflow_memory(
    bundle: &WorkflowRuntimeBundle,
    task: &AgentTask,
    working_artifact: &ArtifactRef,
    scientist: &ScientistResult,
    source_refs: &[String],
    step: &WorkflowStep,
) -> String {
    let claims = scientist
        .claims
        .iter()
        .map(|claim| format!("- {claim}"))
        .collect::<Vec<_>>()
        .join("\n");
    let evidence_chain = scientist
        .claims
        .iter()
        .map(|claim| {
            if source_refs.is_empty() {
                format!("- {claim} -> no source reference")
            } else {
                format!("- {claim} -> {}", source_refs.join(", "))
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let acceptance = step
        .acceptance_criteria
        .iter()
        .map(|criterion| format!("- [x] {criterion}"))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "# Verified Workflow Knowledge\n\n\
         - Workflow ID: `{}`\n\
         - Run ID: `{}`\n\
         - Task ID: `{}`\n\
         - Working artifact: `{}`\n\
         - Working artifact hash: `{}`\n\
         - Confidence: {:.3}\n\
         - Verification kernel: `{}`\n\n\
         ## Claims\n\n{}\n\n\
         ## Evidence Chain\n\n{}\n\n\
         ## Acceptance Criteria\n\n{}\n",
        bundle.workflow.workflow_id,
        bundle.run.run_id,
        task.id,
        working_artifact.uri,
        working_artifact.hash,
        scientist.overall_confidence,
        scientist.kernel_name,
        claims,
        evidence_chain,
        acceptance,
    )
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
        "report_id",
        "verification_id",
        "artifact_id",
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
    use crate::ai::research_trace::{self, TaskContext, TaskEvidence, TaskEvidenceEntry};
    use crate::harness::workflow::{
        AgentRegistry, AgentRegistryEntry, ControlPolicy, GoalAlignment, GoalContract, RetryPolicy,
        RunStatus, VerificationOutcome, WorkflowIr, WorkflowStatus, WorkflowStep,
        WorkflowStepKind, WorkflowStepMode, WorkflowStepStatus,
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
            Some("research result recorded")
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
    fn reported_dispatch_keeps_short_detail_instead_of_scheduler_result_body() {
        let root = tempfile::tempdir().unwrap();
        let service = WorkflowRuntimeService::new(root.path());
        let mut scheduler = AgentScheduler::new(1);
        let started = service
            .start(
                goal(),
                workflow_with_ready_research(),
                registry(),
                "run_reference_only",
                &mut scheduler,
                100,
            )
            .unwrap();
        let task_id = started.dispatches[0].task_id.clone().unwrap();
        scheduler.complete(
            &task_id,
            "full model response body that must stay in Dream working memory".to_string(),
        );

        let bundle = service
            .resume("run_reference_only", &mut scheduler, 200)
            .unwrap();

        assert_eq!(
            bundle.dispatches[0].detail.as_deref(),
            Some("research result recorded")
        );
        assert!(!serde_json::to_string(&bundle)
            .unwrap()
            .contains("full model response body"));
        let ledger = std::fs::read_to_string(
            root.path()
                .join(".harness/runs/run_reference_only/events.jsonl"),
        )
        .unwrap();
        assert!(!ledger.contains("full model response body"));
    }

    #[test]
    fn completed_research_promotes_verified_semantic_memory_and_succeeds_run() {
        let root = tempfile::tempdir().unwrap();
        let service = WorkflowRuntimeService::new(root.path());
        let mut scheduler = AgentScheduler::new(1);
        let bundle = service
            .start(
                goal(),
                workflow_with_ready_research(),
                registry(),
                "run_completion",
                &mut scheduler,
                100,
            )
            .unwrap();
        let task_id = bundle.dispatches[0].task_id.clone().unwrap();
        let dualtrack = root.path().join(".dualtrack");
        let context = TaskContext {
            intent: "Map Bayesian evidence".to_string(),
            retrieval_evidence: vec![TaskEvidence {
                source: "web".to_string(),
                entries: vec![TaskEvidenceEntry {
                    title: "Bayesian evidence".to_string(),
                    snippet: "Posterior updates preserve uncertainty.".to_string(),
                    url: Some("https://example.test/evidence".to_string()),
                    authors: vec![],
                    year: Some(2026),
                    source: "web".to_string(),
                    relevance_score: 0.9,
                }],
                hop: 0,
                generated_keywords: vec!["bayesian evidence".to_string()],
                total_found: 1,
            }],
            ..TaskContext::default()
        };
        research_trace::write_path_log(
            &dualtrack,
            &task_id,
            0,
            "bayesian evidence",
            "web",
            &["https://example.test/evidence".to_string()],
            &[],
            "accepted source",
            Some(&context),
        )
        .unwrap();
        research_trace::write_cot_log(
            &dualtrack,
            &task_id,
            "research trace",
            Some(&context),
        )
        .unwrap();
        let result = "## Findings\n\nEvidence-backed conclusion.\n\n\
                      ## Acceptance Check\n\n- [x] A dispatch record is persisted";
        research_trace::write_result_md(&dualtrack, &task_id, result, Some(&context)).unwrap();
        research_trace::write_context(&dualtrack, &task_id, &context).unwrap();
        scheduler.complete_with_context_and_dream_snapshot(
            &task_id,
            result.to_string(),
            Some(&context),
            None,
        );
        let task = scheduler.get_task(&task_id).unwrap().clone();

        let bundle = service
            .record_task_completion(&task, 200, &mut scheduler)
            .unwrap()
            .unwrap();

        assert_eq!(
            bundle.workflow.steps[0].status,
            WorkflowStepStatus::Verified
        );
        assert_eq!(bundle.run.status, RunStatus::Succeeded);
        assert!(bundle.run.ended_at.is_some());
        assert_eq!(bundle.step_reports.len(), 1);
        assert!(bundle
            .verification_findings
            .iter()
            .all(|finding| finding.result == VerificationOutcome::Pass));
        assert!(bundle.artifacts.iter().any(|artifact| {
            artifact
                .uri
                .starts_with(".dualtrack/memory/semantic/workflows/")
        }));
    }

    #[test]
    fn missing_evidence_keeps_working_artifact_and_blocks_semantic_promotion() {
        let root = tempfile::tempdir().unwrap();
        let service = WorkflowRuntimeService::new(root.path());
        let mut scheduler = AgentScheduler::new(1);
        let bundle = service
            .start(
                goal(),
                workflow_with_ready_research(),
                registry(),
                "run_missing_evidence",
                &mut scheduler,
                100,
            )
            .unwrap();
        let task_id = bundle.dispatches[0].task_id.clone().unwrap();
        let dualtrack = root.path().join(".dualtrack");
        let context = TaskContext::default();
        research_trace::write_path_log(
            &dualtrack,
            &task_id,
            0,
            "unsupported conclusion",
            "local",
            &[],
            &[],
            "no evidence",
            Some(&context),
        )
        .unwrap();
        research_trace::write_cot_log(
            &dualtrack,
            &task_id,
            "research trace",
            Some(&context),
        )
        .unwrap();
        let result = "## Findings\n\nUnsupported conclusion.\n\n\
                      ## Acceptance Check\n\n- [x] A dispatch record is persisted";
        research_trace::write_result_md(&dualtrack, &task_id, result, Some(&context)).unwrap();
        research_trace::write_context(&dualtrack, &task_id, &context).unwrap();
        scheduler.complete_with_context_and_dream_snapshot(
            &task_id,
            result.to_string(),
            Some(&context),
            None,
        );
        let task = scheduler.get_task(&task_id).unwrap().clone();

        let bundle = service
            .record_task_completion(&task, 200, &mut scheduler)
            .unwrap()
            .unwrap();

        assert_eq!(bundle.run.status, RunStatus::Failed);
        assert!(bundle
            .verification_findings
            .iter()
            .any(|finding| finding.reason_code == "evidence_missing"));
        assert!(bundle
            .artifacts
            .iter()
            .any(|artifact| artifact.uri.ends_with("/result.md")));
        assert!(!root
            .path()
            .join(".dualtrack/memory/semantic/workflows")
            .exists());
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
