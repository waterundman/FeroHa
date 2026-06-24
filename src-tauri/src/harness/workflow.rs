use crate::ai::sandbox::{NetworkPolicy, SandboxPolicy};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GoalContract {
    pub goal_id: String,
    pub goal_text: String,
    #[serde(default)]
    pub success_definition: Vec<String>,
    #[serde(default)]
    pub non_goals: Vec<String>,
    #[serde(default = "empty_object")]
    pub constraints: Value,
    #[serde(default)]
    pub context_scope: Vec<String>,
    #[serde(default = "empty_object")]
    pub approval_policy: Value,
    #[serde(default = "empty_object")]
    pub budget: Value,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowStatus {
    Draft,
    Running,
    Paused,
    Completed,
    Aborted,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowStepKind {
    Research,
    Implement,
    Test,
    Review,
    Verify,
    Merge,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowStepMode {
    ReadOnly,
    WritePatch,
    TestOnly,
    ReviewOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowStepStatus {
    Pending,
    Ready,
    Running,
    Reported,
    Verified,
    Failed,
    Blocked,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RetryPolicy {
    pub max_attempts: usize,
    pub backoff_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GoalAlignment {
    #[serde(default)]
    pub success_clauses: Vec<usize>,
    pub why_necessary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkflowStep {
    pub step_id: String,
    pub title: String,
    pub kind: WorkflowStepKind,
    pub agent_type: String,
    pub mode: WorkflowStepMode,
    pub task: String,
    #[serde(default = "empty_object")]
    pub inputs: Value,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub acceptance_criteria: Vec<String>,
    pub goal_alignment: GoalAlignment,
    pub retry_policy: RetryPolicy,
    pub status: WorkflowStepStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ControlPolicy {
    pub max_parallel_steps: usize,
    pub replan_on_verification_fail: bool,
    pub max_patch_chain: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkflowIr {
    pub workflow_id: String,
    pub goal_id: String,
    pub version: usize,
    pub parent_version: Option<usize>,
    pub status: WorkflowStatus,
    #[serde(default = "empty_object")]
    pub global_context: Value,
    pub control_policy: ControlPolicy,
    #[serde(default)]
    pub steps: Vec<WorkflowStep>,
    pub created_by: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentRegistryEntry {
    pub agent_type: String,
    #[serde(default)]
    pub allowed_tools: Vec<String>,
    #[serde(default)]
    pub denied_tools: Vec<String>,
    pub default_mode: WorkflowStepMode,
    pub max_parallelism: usize,
    #[serde(default)]
    pub can_delegate: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentRegistry {
    agents: HashMap<String, AgentRegistryEntry>,
}

impl AgentRegistry {
    pub fn from_agents(agents: Vec<AgentRegistryEntry>) -> Self {
        Self {
            agents: agents
                .into_iter()
                .map(|agent| (agent.agent_type.clone(), agent))
                .collect(),
        }
    }

    pub fn get(&self, agent_type: &str) -> Option<&AgentRegistryEntry> {
        self.agents.get(agent_type)
    }

    pub fn contains_agent(&self, agent_type: &str) -> bool {
        self.agents.contains_key(agent_type)
    }

    pub fn allows_tool(&self, agent_type: &str, tool: &str) -> bool {
        let Some(agent) = self.get(agent_type) else {
            return false;
        };
        if agent.denied_tools.iter().any(|denied| denied == tool) {
            return false;
        }
        agent.allowed_tools.iter().any(|allowed| allowed == tool)
    }

    pub fn can_delegate(&self, agent_type: &str) -> bool {
        self.get(agent_type)
            .map(|agent| agent.can_delegate)
            .unwrap_or(false)
    }

    pub fn tools_for_agent(&self, agent_type: &str) -> Result<Vec<String>, WorkflowError> {
        let Some(agent) = self.get(agent_type) else {
            return Err(WorkflowError::UnknownAgentType(agent_type.to_string()));
        };

        Ok(agent
            .allowed_tools
            .iter()
            .filter(|tool| !agent.denied_tools.iter().any(|denied| denied == *tool))
            .filter(|tool| agent.can_delegate || !is_delegation_tool(tool.as_str()))
            .cloned()
            .collect())
    }

    pub fn sandbox_for_step(
        &self,
        run: &WorkflowRunState,
        step: &WorkflowStep,
    ) -> Result<SandboxPolicy, WorkflowError> {
        let tools = self.tools_for_agent(&step.agent_type)?;
        let read_roots = step_input_roots(step);
        let write_roots = step_write_roots(run, step);

        Ok(SandboxPolicy {
            network_policy: infer_network_policy(&tools),
            max_runtime_secs: runtime_secs_for_mode(&step.mode),
            requires_bridge: !write_roots.is_empty(),
            tool_allowlist: tools,
            read_roots,
            write_roots,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkflowPatch {
    pub patch_id: String,
    pub workflow_id: String,
    pub from_version: usize,
    pub to_version: usize,
    pub basis: PatchBasis,
    #[serde(default)]
    pub ops: Vec<WorkflowPatchOp>,
    pub rationale: String,
    #[serde(default = "empty_object")]
    pub predicted_impact: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PatchBasis {
    #[serde(default)]
    pub failed_steps: Vec<String>,
    #[serde(default)]
    pub failed_goal_clauses: Vec<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum WorkflowPatchOp {
    AddStep {
        after: Option<String>,
        step: WorkflowStep,
    },
    ReplaceStepStatus {
        step_id: String,
        status: WorkflowStepStatus,
    },
    ReplaceControlPolicy {
        control_policy: ControlPolicy,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StepReportStatus {
    Completed,
    Partial,
    Failed,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactType {
    Patch,
    TestReport,
    VerificationReport,
    Log,
    Screenshot,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RetentionPolicy {
    Ephemeral,
    Run,
    Workflow,
    Permanent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArtifactRef {
    pub artifact_id: String,
    #[serde(rename = "type")]
    pub artifact_type: ArtifactType,
    pub uri: String,
    pub hash: String,
    pub mime_type: String,
    pub producer_step_id: String,
    pub retention_policy: RetentionPolicy,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvidenceRef {
    pub file: String,
    #[serde(default)]
    pub lines: Vec<usize>,
    pub claim: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SuggestedNextStep {
    pub proposal: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StepReport {
    pub report_id: String,
    pub step_id: String,
    pub attempt: usize,
    pub status: StepReportStatus,
    pub summary: String,
    #[serde(default)]
    pub artifacts: Vec<ArtifactRef>,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub risks: Vec<String>,
    #[serde(default)]
    pub blocked_by: Vec<String>,
    #[serde(default)]
    pub suggested_next_steps: Vec<SuggestedNextStep>,
    #[serde(default = "empty_object")]
    pub resource_usage: Value,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StepReportsDigest {
    pub entries: Vec<StepReportDigestEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StepReportDigestEntry {
    pub report_id: String,
    pub step_id: String,
    pub status: StepReportStatus,
    pub summary: String,
    #[serde(default)]
    pub artifact_ids: Vec<String>,
    #[serde(default)]
    pub evidence_claims: Vec<String>,
    #[serde(default)]
    pub risks: Vec<String>,
    #[serde(default)]
    pub blocked_by: Vec<String>,
    #[serde(default)]
    pub suggested_next_steps: Vec<String>,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VerificationLevel {
    Step,
    Integration,
    Goal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VerificationOutcome {
    Pass,
    Fail,
    CannotVerify,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VerificationFinding {
    pub verification_id: String,
    pub level: VerificationLevel,
    pub target: String,
    pub result: VerificationOutcome,
    #[serde(default)]
    pub failed_clauses: Vec<usize>,
    pub reason_code: String,
    pub summary: String,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
    #[serde(default)]
    pub minimal_fix_surface: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OrchestratorOutput {
    WorkflowCreate {
        workflow: WorkflowIr,
    },
    WorkflowPatch {
        patch: WorkflowPatch,
    },
    CannotProceed {
        reason_code: String,
        summary: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HarnessEvent {
    pub timestamp: String,
    pub severity_text: String,
    pub event_name: String,
    pub body: String,
    #[serde(default = "empty_object")]
    pub attributes: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Queued,
    Running,
    Paused,
    Failed,
    Succeeded,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkflowRunState {
    pub run_id: String,
    pub workflow_id: String,
    pub workflow_version: usize,
    pub status: RunStatus,
    pub started_at: String,
    pub ended_at: Option<String>,
    #[serde(default)]
    pub active_step_ids: Vec<String>,
    #[serde(default)]
    pub worktree_map: HashMap<String, PathBuf>,
    #[serde(default = "empty_object")]
    pub metrics: Value,
    pub context_digest_version: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ArtifactContract {
    pub expected_output: String,
    #[serde(default)]
    pub acceptance_criteria: Vec<String>,
    #[serde(default)]
    pub success_clauses: Vec<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StepDispatch {
    pub workflow_id: String,
    pub run_id: String,
    pub step_id: String,
    pub agent_type: String,
    pub capability: WorkflowStepKind,
    pub mode: WorkflowStepMode,
    pub attempt: usize,
    #[serde(default = "empty_object")]
    pub inputs: Value,
    pub artifact_contract: ArtifactContract,
    pub sandbox_policy: SandboxPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkflowStateSummary {
    pub workflow_id: String,
    pub workflow_version: usize,
    pub workflow_status: WorkflowStatus,
    pub run_id: String,
    pub run_status: RunStatus,
    #[serde(default)]
    pub active_step_ids: Vec<String>,
    #[serde(default)]
    pub pending_step_ids: Vec<String>,
    #[serde(default)]
    pub verified_step_ids: Vec<String>,
    #[serde(default)]
    pub failed_step_ids: Vec<String>,
    #[serde(default)]
    pub blocked_step_ids: Vec<String>,
    pub context_digest_version: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RuntimePolicies {
    pub max_parallel_steps: usize,
    pub max_patch_chain: usize,
    pub replan_on_verification_fail: bool,
    pub allow_subagent_delegation: bool,
    #[serde(default)]
    pub allowed_orchestrator_outputs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OrchestratorReplanRequest {
    pub goal_contract: GoalContract,
    pub workflow_state_summary: WorkflowStateSummary,
    pub step_reports_digest: StepReportsDigest,
    #[serde(default)]
    pub verifier_findings: Vec<VerificationFinding>,
    pub agent_registry: AgentRegistry,
    pub runtime_policies: RuntimePolicies,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkflowRuntimeEventChain {
    pub workflow_id: String,
    pub run_id: String,
    #[serde(default)]
    pub events: Vec<HarnessEvent>,
    pub replan_requested: bool,
}

pub struct WorkflowRuntimeEventStore;

impl WorkflowIr {
    pub fn validate(
        &self,
        goal: &GoalContract,
        registry: &AgentRegistry,
    ) -> Result<(), WorkflowError> {
        if goal.success_definition.is_empty() {
            return Err(WorkflowError::GoalHasNoSuccessDefinition);
        }
        if self.goal_id != goal.goal_id {
            return Err(WorkflowError::GoalMismatch {
                workflow_goal_id: self.goal_id.clone(),
                contract_goal_id: goal.goal_id.clone(),
            });
        }
        if self.version == 0 {
            return Err(WorkflowError::InvalidVersion(0));
        }
        if self.steps.is_empty() {
            return Err(WorkflowError::EmptyWorkflow);
        }
        if self.control_policy.max_parallel_steps == 0 {
            return Err(WorkflowError::InvalidControlPolicy(
                "max_parallel_steps must be greater than zero".to_string(),
            ));
        }
        if self.control_policy.max_patch_chain == 0 {
            return Err(WorkflowError::InvalidControlPolicy(
                "max_patch_chain must be greater than zero".to_string(),
            ));
        }

        let mut ids = HashSet::new();
        for step in &self.steps {
            if step.step_id.trim().is_empty() {
                return Err(WorkflowError::EmptyStepId);
            }
            if !ids.insert(step.step_id.clone()) {
                return Err(WorkflowError::DuplicateStepId(step.step_id.clone()));
            }
        }

        for step in &self.steps {
            if !registry.contains_agent(&step.agent_type) {
                return Err(WorkflowError::UnknownAgentType(step.agent_type.clone()));
            }
            if step.acceptance_criteria.is_empty() {
                return Err(WorkflowError::MissingAcceptanceCriteria(
                    step.step_id.clone(),
                ));
            }
            if step.goal_alignment.success_clauses.is_empty() {
                return Err(WorkflowError::MissingGoalAlignment(step.step_id.clone()));
            }
            for clause in &step.goal_alignment.success_clauses {
                if *clause == 0 || *clause > goal.success_definition.len() {
                    return Err(WorkflowError::GoalClauseOutOfRange {
                        step_id: step.step_id.clone(),
                        clause: *clause,
                    });
                }
            }
            for dep in &step.dependencies {
                if !ids.contains(dep) {
                    return Err(WorkflowError::UnknownDependency {
                        step_id: step.step_id.clone(),
                        dependency: dep.clone(),
                    });
                }
            }
        }

        self.validate_acyclic_dependencies()
    }

    pub fn apply_patch(
        &self,
        patch: &WorkflowPatch,
        goal: &GoalContract,
        registry: &AgentRegistry,
    ) -> Result<Self, WorkflowError> {
        if patch.workflow_id != self.workflow_id {
            return Err(WorkflowError::PatchWorkflowMismatch {
                expected: self.workflow_id.clone(),
                actual: patch.workflow_id.clone(),
            });
        }
        if patch.from_version != self.version {
            return Err(WorkflowError::PatchVersionMismatch {
                expected: self.version,
                actual: patch.from_version,
            });
        }
        if patch.to_version <= patch.from_version {
            return Err(WorkflowError::InvalidPatchVersion {
                from_version: patch.from_version,
                to_version: patch.to_version,
            });
        }
        if patch.ops.is_empty() {
            return Err(WorkflowError::EmptyPatch(patch.patch_id.clone()));
        }
        if patch.to_version.saturating_sub(1) > self.control_policy.max_patch_chain {
            return Err(WorkflowError::PatchChainLimitExceeded {
                patch_id: patch.patch_id.clone(),
                max_patch_chain: self.control_policy.max_patch_chain,
                attempted_to_version: patch.to_version,
            });
        }

        let mut next = self.clone();
        next.parent_version = Some(self.version);
        next.version = patch.to_version;

        for op in &patch.ops {
            match op {
                WorkflowPatchOp::AddStep { after, step } => {
                    if next
                        .steps
                        .iter()
                        .any(|existing| existing.step_id == step.step_id)
                    {
                        return Err(WorkflowError::DuplicateStepId(step.step_id.clone()));
                    }
                    if let Some(after_id) = after {
                        let Some(pos) = next.steps.iter().position(|s| s.step_id == *after_id)
                        else {
                            return Err(WorkflowError::PatchTargetMissing(after_id.clone()));
                        };
                        next.steps.insert(pos + 1, step.clone());
                    } else {
                        next.steps.push(step.clone());
                    }
                }
                WorkflowPatchOp::ReplaceStepStatus { step_id, status } => {
                    let Some(step) = next.steps.iter_mut().find(|s| s.step_id == *step_id) else {
                        return Err(WorkflowError::PatchTargetMissing(step_id.clone()));
                    };
                    step.status = status.clone();
                }
                WorkflowPatchOp::ReplaceControlPolicy { control_policy } => {
                    next.control_policy = control_policy.clone();
                }
            }
        }

        next.validate(goal, registry)?;
        Ok(next)
    }

    fn validate_acyclic_dependencies(&self) -> Result<(), WorkflowError> {
        #[derive(Clone, Copy, PartialEq, Eq)]
        enum Visit {
            Visiting,
            Done,
        }

        let by_id: HashMap<&str, &WorkflowStep> = self
            .steps
            .iter()
            .map(|step| (step.step_id.as_str(), step))
            .collect();
        let mut state: HashMap<&str, Visit> = HashMap::new();

        fn visit<'a>(
            step_id: &'a str,
            by_id: &HashMap<&'a str, &'a WorkflowStep>,
            state: &mut HashMap<&'a str, Visit>,
        ) -> Result<(), WorkflowError> {
            match state.get(step_id).copied() {
                Some(Visit::Done) => return Ok(()),
                Some(Visit::Visiting) => {
                    return Err(WorkflowError::CyclicDependency(step_id.to_string()))
                }
                None => {}
            }

            state.insert(step_id, Visit::Visiting);
            if let Some(step) = by_id.get(step_id) {
                for dependency in &step.dependencies {
                    visit(dependency, by_id, state)?;
                }
            }
            state.insert(step_id, Visit::Done);
            Ok(())
        }

        for step in &self.steps {
            visit(step.step_id.as_str(), &by_id, &mut state)?;
        }

        Ok(())
    }
}

impl WorkflowRunState {
    pub fn for_workflow(
        run_id: impl Into<String>,
        workflow: &WorkflowIr,
        started_at: impl Into<String>,
    ) -> Self {
        let run_id = run_id.into();
        let worktree_map = workflow
            .steps
            .iter()
            .filter(|step| matches!(step.mode, WorkflowStepMode::WritePatch))
            .map(|step| {
                (
                    step.step_id.clone(),
                    default_worktree_path(&run_id, &step.step_id),
                )
            })
            .collect();

        Self {
            run_id,
            workflow_id: workflow.workflow_id.clone(),
            workflow_version: workflow.version,
            status: RunStatus::Running,
            started_at: started_at.into(),
            ended_at: None,
            active_step_ids: Vec::new(),
            worktree_map,
            metrics: empty_object(),
            context_digest_version: 1,
        }
    }

    pub fn ready_dispatches(
        &self,
        workflow: &WorkflowIr,
        registry: &AgentRegistry,
    ) -> Result<Vec<StepDispatch>, WorkflowError> {
        if self.workflow_id != workflow.workflow_id || self.workflow_version != workflow.version {
            return Err(WorkflowError::RunWorkflowMismatch {
                run_workflow_id: self.workflow_id.clone(),
                run_workflow_version: self.workflow_version,
                workflow_id: workflow.workflow_id.clone(),
                workflow_version: workflow.version,
            });
        }
        if !matches!(self.status, RunStatus::Running)
            || !matches!(workflow.status, WorkflowStatus::Running)
        {
            return Ok(Vec::new());
        }

        let global_budget = workflow
            .control_policy
            .max_parallel_steps
            .saturating_sub(self.active_step_ids.len());
        if global_budget == 0 {
            return Ok(Vec::new());
        }

        let active: HashSet<&str> = self.active_step_ids.iter().map(String::as_str).collect();
        let mut agent_counts = active_agent_counts(workflow, &active);
        let mut dispatches = Vec::new();

        for step in &workflow.steps {
            if dispatches.len() >= global_budget {
                break;
            }
            if !matches!(step.status, WorkflowStepStatus::Ready) {
                continue;
            }
            if active.contains(step.step_id.as_str()) {
                continue;
            }
            if !dependencies_verified(workflow, step) {
                continue;
            }

            let Some(agent) = registry.get(&step.agent_type) else {
                return Err(WorkflowError::UnknownAgentType(step.agent_type.clone()));
            };
            let current_for_agent = agent_counts.get(&step.agent_type).copied().unwrap_or(0);
            if current_for_agent >= agent.max_parallelism {
                continue;
            }

            dispatches.push(StepDispatch {
                workflow_id: workflow.workflow_id.clone(),
                run_id: self.run_id.clone(),
                step_id: step.step_id.clone(),
                agent_type: step.agent_type.clone(),
                capability: step.kind.clone(),
                mode: step.mode.clone(),
                attempt: 1,
                inputs: step.inputs.clone(),
                artifact_contract: ArtifactContract {
                    expected_output: step.task.clone(),
                    acceptance_criteria: step.acceptance_criteria.clone(),
                    success_clauses: step.goal_alignment.success_clauses.clone(),
                },
                sandbox_policy: registry.sandbox_for_step(self, step)?,
            });
            agent_counts.insert(step.agent_type.clone(), current_for_agent + 1);
        }

        Ok(dispatches)
    }
}

impl WorkflowStateSummary {
    pub fn from_runtime(workflow: &WorkflowIr, run: &WorkflowRunState) -> Self {
        Self {
            workflow_id: workflow.workflow_id.clone(),
            workflow_version: workflow.version,
            workflow_status: workflow.status.clone(),
            run_id: run.run_id.clone(),
            run_status: run.status.clone(),
            active_step_ids: run.active_step_ids.clone(),
            pending_step_ids: step_ids_with_statuses(
                workflow,
                &[WorkflowStepStatus::Pending, WorkflowStepStatus::Ready],
            ),
            verified_step_ids: step_ids_with_statuses(workflow, &[WorkflowStepStatus::Verified]),
            failed_step_ids: step_ids_with_statuses(workflow, &[WorkflowStepStatus::Failed]),
            blocked_step_ids: step_ids_with_statuses(workflow, &[WorkflowStepStatus::Blocked]),
            context_digest_version: run.context_digest_version,
        }
    }
}

impl RuntimePolicies {
    pub fn from_workflow(workflow: &WorkflowIr, registry: &AgentRegistry) -> Self {
        Self {
            max_parallel_steps: workflow.control_policy.max_parallel_steps,
            max_patch_chain: workflow.control_policy.max_patch_chain,
            replan_on_verification_fail: workflow.control_policy.replan_on_verification_fail,
            allow_subagent_delegation: registry.agents.values().any(|agent| agent.can_delegate),
            allowed_orchestrator_outputs: vec![
                "workflow_create".to_string(),
                "workflow_patch".to_string(),
                "cannot_proceed".to_string(),
            ],
        }
    }
}

impl OrchestratorReplanRequest {
    pub fn from_runtime(
        goal: &GoalContract,
        workflow: &WorkflowIr,
        run: &WorkflowRunState,
        reports: &[StepReport],
        findings: &[VerificationFinding],
        registry: &AgentRegistry,
    ) -> Self {
        Self {
            goal_contract: goal.clone(),
            workflow_state_summary: WorkflowStateSummary::from_runtime(workflow, run),
            step_reports_digest: StepReportsDigest::from_reports(reports),
            verifier_findings: findings.to_vec(),
            agent_registry: registry.clone(),
            runtime_policies: RuntimePolicies::from_workflow(workflow, registry),
        }
    }
}

impl WorkflowRuntimeEventChain {
    pub fn from_run_created(
        workflow: &WorkflowIr,
        run: &WorkflowRunState,
        timestamp: impl Into<String>,
    ) -> Self {
        let workflow_id = workflow.workflow_id.clone();
        let run_id = run.run_id.clone();
        Self {
            workflow_id: workflow_id.clone(),
            run_id: run_id.clone(),
            events: vec![HarnessEvent::new(
                timestamp,
                "INFO",
                "workflow.run.created",
                format!(
                    "Workflow run {} created for {}@v{}.",
                    run_id, workflow_id, workflow.version
                ),
                serde_json::json!({
                    "workflow_id": workflow_id,
                    "run_id": run_id,
                    "workflow_version": workflow.version,
                    "workflow_status": workflow.status,
                    "run_status": run.status,
                    "active_step_ids": run.active_step_ids,
                    "context_digest_version": run.context_digest_version,
                }),
            )],
            replan_requested: false,
        }
    }

    pub fn from_run_resumed(
        workflow: &WorkflowIr,
        run: &WorkflowRunState,
        timestamp: impl Into<String>,
    ) -> Self {
        let workflow_id = workflow.workflow_id.clone();
        let run_id = run.run_id.clone();
        Self {
            workflow_id: workflow_id.clone(),
            run_id: run_id.clone(),
            events: vec![HarnessEvent::new(
                timestamp,
                "INFO",
                "workflow.run.resumed",
                format!(
                    "Workflow run {} resumed for {}@v{}.",
                    run_id, workflow_id, workflow.version
                ),
                serde_json::json!({
                    "workflow_id": workflow_id,
                    "run_id": run_id,
                    "workflow_version": workflow.version,
                    "workflow_status": workflow.status,
                    "run_status": run.status,
                    "active_step_ids": run.active_step_ids,
                    "context_digest_version": run.context_digest_version,
                }),
            )],
            replan_requested: false,
        }
    }

    pub fn from_step_dispatches(dispatches: &[StepDispatch], timestamp: impl Into<String>) -> Self {
        let timestamp = timestamp.into();
        let workflow_id = dispatches
            .first()
            .map(|dispatch| dispatch.workflow_id.clone())
            .unwrap_or_default();
        let run_id = dispatches
            .first()
            .map(|dispatch| dispatch.run_id.clone())
            .unwrap_or_default();
        let events = dispatches
            .iter()
            .map(|dispatch| {
                HarnessEvent::new(
                    timestamp.clone(),
                    "INFO",
                    "workflow.step.dispatched",
                    format!(
                        "Workflow step {} dispatched to {}.",
                        dispatch.step_id, dispatch.agent_type
                    ),
                    step_event_attributes(dispatch, None, None),
                )
            })
            .collect();

        Self {
            workflow_id,
            run_id,
            events,
            replan_requested: false,
        }
    }

    pub fn from_step_queued(
        dispatch: &StepDispatch,
        task_id: impl Into<String>,
        timestamp: impl Into<String>,
    ) -> Self {
        let task_id = task_id.into();
        Self {
            workflow_id: dispatch.workflow_id.clone(),
            run_id: dispatch.run_id.clone(),
            events: vec![HarnessEvent::new(
                timestamp,
                "INFO",
                "workflow.step.queued",
                format!(
                    "Workflow step {} queued as task {}.",
                    dispatch.step_id, task_id
                ),
                step_event_attributes(dispatch, Some(task_id.as_str()), None),
            )],
            replan_requested: false,
        }
    }

    pub fn from_step_running(
        dispatch: &StepDispatch,
        task_id: impl Into<String>,
        timestamp: impl Into<String>,
    ) -> Self {
        let task_id = task_id.into();
        Self {
            workflow_id: dispatch.workflow_id.clone(),
            run_id: dispatch.run_id.clone(),
            events: vec![HarnessEvent::new(
                timestamp,
                "INFO",
                "workflow.step.running",
                format!(
                    "Workflow step {} is running as task {}.",
                    dispatch.step_id, task_id
                ),
                step_event_attributes(dispatch, Some(task_id.as_str()), None),
            )],
            replan_requested: false,
        }
    }

    pub fn from_step_reported(
        dispatch: &StepDispatch,
        task_id: impl Into<String>,
        summary: impl Into<String>,
        timestamp: impl Into<String>,
    ) -> Self {
        let task_id = task_id.into();
        let summary = summary.into();
        let mut attributes = step_event_attributes(dispatch, Some(task_id.as_str()), None);
        if let Value::Object(map) = &mut attributes {
            map.insert("summary".to_string(), serde_json::json!(summary));
        }
        Self {
            workflow_id: dispatch.workflow_id.clone(),
            run_id: dispatch.run_id.clone(),
            events: vec![HarnessEvent::new(
                timestamp,
                "INFO",
                "workflow.step.reported",
                format!(
                    "Workflow step {} reported from task {}.",
                    dispatch.step_id, task_id
                ),
                attributes,
            )],
            replan_requested: false,
        }
    }

    pub fn from_step_assessed(
        workflow_id: impl Into<String>,
        run_id: impl Into<String>,
        report: &StepReport,
        findings: &[VerificationFinding],
        timestamp: impl Into<String>,
    ) -> Self {
        let workflow_id = workflow_id.into();
        let run_id = run_id.into();
        let timestamp = timestamp.into();
        let mut events = vec![HarnessEvent::new(
            timestamp.clone(),
            step_report_event_severity(&report.status),
            "workflow.step_report.recorded",
            format!(
                "Step report {} recorded for {}.",
                report.report_id, report.step_id
            ),
            serde_json::json!({
                "workflow_id": workflow_id,
                "run_id": run_id,
                "report_id": report.report_id,
                "step_id": report.step_id,
                "status": report.status,
                "confidence": report.confidence,
                "artifact_ids": report
                    .artifacts
                    .iter()
                    .map(|artifact| artifact.artifact_id.clone())
                    .collect::<Vec<_>>(),
                "evidence_count": report.evidence.len(),
            }),
        )];
        for finding in findings {
            events.push(HarnessEvent::new(
                timestamp.clone(),
                verification_event_severity(&finding.result),
                verification_event_name(&finding.result),
                finding.summary.clone(),
                serde_json::json!({
                    "workflow_id": workflow_id,
                    "run_id": run_id,
                    "verification_id": finding.verification_id,
                    "step_id": report.step_id,
                    "level": finding.level,
                    "target": finding.target,
                    "result": finding.result,
                    "failed_clauses": finding.failed_clauses,
                    "reason_code": finding.reason_code,
                    "evidence_refs": finding.evidence_refs,
                    "minimal_fix_surface": finding.minimal_fix_surface,
                }),
            ));
        }

        Self {
            workflow_id,
            run_id,
            events,
            replan_requested: false,
        }
    }

    pub fn from_step_verified(
        dispatch: &StepDispatch,
        task_id: impl Into<String>,
        working_artifact: &ArtifactRef,
        timestamp: impl Into<String>,
    ) -> Self {
        let task_id = task_id.into();
        Self {
            workflow_id: dispatch.workflow_id.clone(),
            run_id: dispatch.run_id.clone(),
            events: vec![HarnessEvent::new(
                timestamp,
                "INFO",
                "workflow.step.verified",
                format!("Workflow step {} passed verification.", dispatch.step_id),
                serde_json::json!({
                    "workflow_id": dispatch.workflow_id,
                    "run_id": dispatch.run_id,
                    "step_id": dispatch.step_id,
                    "attempt": dispatch.attempt,
                    "task_id": task_id,
                    "artifact_id": working_artifact.artifact_id,
                    "artifact_uri": working_artifact.uri,
                    "artifact_hash": working_artifact.hash,
                }),
            )],
            replan_requested: false,
        }
    }

    pub fn from_semantic_promoted(
        workflow_id: impl Into<String>,
        run_id: impl Into<String>,
        step_id: impl Into<String>,
        artifact: &ArtifactRef,
        timestamp: impl Into<String>,
    ) -> Self {
        let workflow_id = workflow_id.into();
        let run_id = run_id.into();
        let step_id = step_id.into();
        Self {
            workflow_id: workflow_id.clone(),
            run_id: run_id.clone(),
            events: vec![HarnessEvent::new(
                timestamp,
                "INFO",
                "workflow.semantic.promoted",
                format!("Verified workflow knowledge was promoted from step {step_id}."),
                serde_json::json!({
                    "workflow_id": workflow_id,
                    "run_id": run_id,
                    "step_id": step_id,
                    "artifact_id": artifact.artifact_id,
                    "artifact_uri": artifact.uri,
                    "artifact_hash": artifact.hash,
                }),
            )],
            replan_requested: false,
        }
    }

    pub fn from_run_succeeded(
        workflow: &WorkflowIr,
        run: &WorkflowRunState,
        timestamp: impl Into<String>,
    ) -> Self {
        Self {
            workflow_id: workflow.workflow_id.clone(),
            run_id: run.run_id.clone(),
            events: vec![HarnessEvent::new(
                timestamp,
                "INFO",
                "workflow.run.succeeded",
                format!("Workflow run {} succeeded.", run.run_id),
                serde_json::json!({
                    "workflow_id": workflow.workflow_id,
                    "run_id": run.run_id,
                    "workflow_version": workflow.version,
                    "workflow_status": workflow.status,
                    "run_status": run.status,
                    "ended_at": run.ended_at,
                }),
            )],
            replan_requested: false,
        }
    }

    pub fn from_run_failed(
        workflow: &WorkflowIr,
        run: &WorkflowRunState,
        reason_code: impl Into<String>,
        summary: impl Into<String>,
        timestamp: impl Into<String>,
    ) -> Self {
        let reason_code = reason_code.into();
        let summary = summary.into();
        Self {
            workflow_id: workflow.workflow_id.clone(),
            run_id: run.run_id.clone(),
            events: vec![HarnessEvent::new(
                timestamp,
                "ERROR",
                "workflow.run.failed",
                summary.clone(),
                serde_json::json!({
                    "workflow_id": workflow.workflow_id,
                    "run_id": run.run_id,
                    "workflow_version": workflow.version,
                    "workflow_status": workflow.status,
                    "run_status": run.status,
                    "ended_at": run.ended_at,
                    "reason_code": reason_code,
                }),
            )],
            replan_requested: false,
        }
    }

    pub fn from_step_failed(
        dispatch: &StepDispatch,
        task_id: impl Into<String>,
        reason_code: impl Into<String>,
        summary: impl Into<String>,
        timestamp: impl Into<String>,
    ) -> Self {
        let task_id = task_id.into();
        let reason_code = reason_code.into();
        let summary = summary.into();
        Self {
            workflow_id: dispatch.workflow_id.clone(),
            run_id: dispatch.run_id.clone(),
            events: vec![HarnessEvent::new(
                timestamp,
                "ERROR",
                "workflow.step.failed",
                summary.clone(),
                step_event_attributes(
                    dispatch,
                    Some(task_id.as_str()),
                    Some((reason_code.as_str(), summary.as_str())),
                ),
            )],
            replan_requested: false,
        }
    }

    pub fn from_step_unsupported(
        dispatch: &StepDispatch,
        reason_code: impl Into<String>,
        summary: impl Into<String>,
        timestamp: impl Into<String>,
    ) -> Self {
        let reason_code = reason_code.into();
        let summary = summary.into();
        Self {
            workflow_id: dispatch.workflow_id.clone(),
            run_id: dispatch.run_id.clone(),
            events: vec![HarnessEvent::new(
                timestamp,
                "WARN",
                "workflow.step.unsupported",
                summary.clone(),
                step_event_attributes(
                    dispatch,
                    None,
                    Some((reason_code.as_str(), summary.as_str())),
                ),
            )],
            replan_requested: false,
        }
    }

    pub fn from_replan_request(
        request: &OrchestratorReplanRequest,
        timestamp: impl Into<String>,
    ) -> Self {
        let timestamp = timestamp.into();
        let workflow_id = request.workflow_state_summary.workflow_id.clone();
        let run_id = request.workflow_state_summary.run_id.clone();
        let mut events = Vec::new();

        for report in &request.step_reports_digest.entries {
            events.push(HarnessEvent::new(
                timestamp.clone(),
                step_report_event_severity(&report.status),
                "workflow.step_report.recorded",
                format!(
                    "Step report {} recorded for {}.",
                    report.report_id, report.step_id
                ),
                serde_json::json!({
                    "workflow_id": workflow_id.clone(),
                    "run_id": run_id.clone(),
                    "report_id": report.report_id,
                    "step_id": report.step_id,
                    "status": report.status.clone(),
                    "confidence": report.confidence,
                    "artifact_count": report.artifact_ids.len(),
                    "risk_count": report.risks.len(),
                    "blocked_by_count": report.blocked_by.len(),
                }),
            ));
        }

        for finding in &request.verifier_findings {
            events.push(HarnessEvent::new(
                timestamp.clone(),
                verification_event_severity(&finding.result),
                verification_event_name(&finding.result),
                finding.summary.clone(),
                serde_json::json!({
                    "workflow_id": workflow_id.clone(),
                    "run_id": run_id.clone(),
                    "verification_id": finding.verification_id,
                    "level": finding.level.clone(),
                    "target": finding.target,
                    "result": finding.result.clone(),
                    "failed_clauses": finding.failed_clauses,
                    "reason_code": finding.reason_code,
                    "evidence_refs": finding.evidence_refs,
                    "minimal_fix_surface": finding.minimal_fix_surface,
                }),
            ));
        }

        let replan_requested = request.runtime_policies.replan_on_verification_fail
            && request
                .verifier_findings
                .iter()
                .any(|finding| finding.result != VerificationOutcome::Pass);

        if replan_requested {
            let mut failed_goal_clauses = request
                .verifier_findings
                .iter()
                .flat_map(|finding| finding.failed_clauses.iter().copied())
                .collect::<Vec<_>>();
            failed_goal_clauses.sort_unstable();
            failed_goal_clauses.dedup();

            events.push(HarnessEvent::new(
                timestamp,
                "WARN",
                "workflow.replan.requested",
                format!(
                    "Workflow verifier requested orchestrator replan for {}@v{}.",
                    workflow_id, request.workflow_state_summary.workflow_version
                ),
                serde_json::json!({
                    "workflow_id": workflow_id.clone(),
                    "run_id": run_id.clone(),
                    "workflow_version": request.workflow_state_summary.workflow_version,
                    "finding_count": request.verifier_findings.len(),
                    "failed_step_ids": request.workflow_state_summary.failed_step_ids,
                    "failed_goal_clauses": failed_goal_clauses,
                    "allowed_orchestrator_outputs": request.runtime_policies.allowed_orchestrator_outputs,
                }),
            ));
        }

        Self {
            workflow_id,
            run_id,
            events,
            replan_requested,
        }
    }

    pub fn from_patch_decision(
        workflow_id: impl Into<String>,
        run_id: impl Into<String>,
        patch: &WorkflowPatch,
        accepted: bool,
        timestamp: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        let workflow_id = workflow_id.into();
        let run_id = run_id.into();
        let reason = reason.into();
        let event_name = if accepted {
            "workflow.patch.accepted"
        } else {
            "workflow.patch.rejected"
        };
        let severity = if accepted { "INFO" } else { "WARN" };
        let body = if reason.trim().is_empty() {
            format!(
                "Workflow patch {} {}.",
                patch.patch_id,
                patch_decision_verb(accepted)
            )
        } else {
            format!(
                "Workflow patch {} {}: {}",
                patch.patch_id,
                patch_decision_verb(accepted),
                reason
            )
        };

        Self {
            workflow_id: workflow_id.clone(),
            run_id: run_id.clone(),
            events: vec![HarnessEvent::new(
                timestamp,
                severity,
                event_name,
                body,
                serde_json::json!({
                    "workflow_id": workflow_id,
                    "run_id": run_id,
                    "patch_id": patch.patch_id,
                    "from_version": patch.from_version,
                    "to_version": patch.to_version,
                    "accepted": accepted,
                    "reason": reason,
                    "failed_steps": patch.basis.failed_steps,
                    "failed_goal_clauses": patch.basis.failed_goal_clauses,
                    "op_count": patch.ops.len(),
                }),
            )],
            replan_requested: false,
        }
    }

    pub fn from_patch_review_request(
        run_id: impl Into<String>,
        patch: &WorkflowPatch,
        proposal_id: impl Into<String>,
        timestamp: impl Into<String>,
    ) -> Self {
        let run_id = run_id.into();
        let proposal_id = proposal_id.into();
        Self {
            workflow_id: patch.workflow_id.clone(),
            run_id: run_id.clone(),
            events: vec![HarnessEvent::new(
                timestamp,
                "WARN",
                "workflow.patch.review_requested",
                format!(
                    "Workflow patch {} was routed to Bridge for human review.",
                    patch.patch_id
                ),
                serde_json::json!({
                    "workflow_id": patch.workflow_id,
                    "run_id": run_id,
                    "patch_id": patch.patch_id,
                    "proposal_id": proposal_id,
                    "from_version": patch.from_version,
                    "to_version": patch.to_version,
                    "failed_steps": patch.basis.failed_steps,
                    "failed_goal_clauses": patch.basis.failed_goal_clauses,
                    "op_count": patch.ops.len(),
                }),
            )],
            replan_requested: false,
        }
    }
}

fn step_event_attributes(
    dispatch: &StepDispatch,
    task_id: Option<&str>,
    error: Option<(&str, &str)>,
) -> Value {
    let mut attributes = serde_json::json!({
        "workflow_id": dispatch.workflow_id,
        "run_id": dispatch.run_id,
        "step_id": dispatch.step_id,
        "agent_type": dispatch.agent_type,
        "capability": dispatch.capability,
        "mode": dispatch.mode,
        "attempt": dispatch.attempt,
        "inputs": dispatch.inputs,
        "artifact_contract": dispatch.artifact_contract,
        "sandbox_summary": dispatch.sandbox_policy.summary(),
    });

    if let Value::Object(map) = &mut attributes {
        if let Some(task_id) = task_id {
            map.insert("task_id".to_string(), serde_json::json!(task_id));
        }
        if let Some((reason_code, summary)) = error {
            map.insert("reason_code".to_string(), serde_json::json!(reason_code));
            map.insert("summary".to_string(), serde_json::json!(summary));
        }
    }

    attributes
}

impl WorkflowRuntimeEventStore {
    pub fn append_chain(
        root: impl AsRef<Path>,
        chain: &WorkflowRuntimeEventChain,
    ) -> Result<PathBuf, WorkflowError> {
        Self::append_events(root, &chain.run_id, &chain.events)
    }

    pub fn append_events(
        root: impl AsRef<Path>,
        run_id: &str,
        events: &[HarnessEvent],
    ) -> Result<PathBuf, WorkflowError> {
        let path = Self::event_log_path(root, run_id)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| WorkflowError::RuntimeEventIo(err.to_string()))?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|err| WorkflowError::RuntimeEventIo(err.to_string()))?;

        for event in events {
            serde_json::to_writer(&mut file, event)
                .map_err(|err| WorkflowError::RuntimeEventParse(err.to_string()))?;
            file.write_all(b"\n")
                .map_err(|err| WorkflowError::RuntimeEventIo(err.to_string()))?;
        }

        Ok(path)
    }

    pub fn read_recent(
        root: impl AsRef<Path>,
        run_id: &str,
        limit: usize,
    ) -> Result<Vec<HarnessEvent>, WorkflowError> {
        let path = Self::event_log_path(root, run_id)?;
        if !path.exists() {
            return Ok(Vec::new());
        }

        let file = OpenOptions::new()
            .read(true)
            .open(&path)
            .map_err(|err| WorkflowError::RuntimeEventIo(err.to_string()))?;
        let reader = BufReader::new(file);
        let mut events = Vec::new();
        for line in reader.lines() {
            let line = line.map_err(|err| WorkflowError::RuntimeEventIo(err.to_string()))?;
            if line.trim().is_empty() {
                continue;
            }
            let event = serde_json::from_str::<HarnessEvent>(&line)
                .map_err(|err| WorkflowError::RuntimeEventParse(err.to_string()))?;
            events.push(event);
        }

        if limit > 0 && events.len() > limit {
            Ok(events.split_off(events.len() - limit))
        } else {
            Ok(events)
        }
    }

    pub fn event_log_path(root: impl AsRef<Path>, run_id: &str) -> Result<PathBuf, WorkflowError> {
        let run_id = safe_runtime_component(run_id)?;
        Ok(root
            .as_ref()
            .join(".harness")
            .join("runs")
            .join(run_id)
            .join("events.jsonl"))
    }
}

impl StepReportsDigest {
    pub fn from_reports(reports: &[StepReport]) -> Self {
        Self {
            entries: reports
                .iter()
                .map(|report| StepReportDigestEntry {
                    report_id: report.report_id.clone(),
                    step_id: report.step_id.clone(),
                    status: report.status.clone(),
                    summary: report.summary.clone(),
                    artifact_ids: report
                        .artifacts
                        .iter()
                        .map(|artifact| artifact.artifact_id.clone())
                        .collect(),
                    evidence_claims: report
                        .evidence
                        .iter()
                        .map(|evidence| evidence.claim.clone())
                        .collect(),
                    risks: report.risks.clone(),
                    blocked_by: report.blocked_by.clone(),
                    suggested_next_steps: report
                        .suggested_next_steps
                        .iter()
                        .map(|step| step.proposal.clone())
                        .collect(),
                    confidence: report.confidence,
                })
                .collect(),
        }
    }
}

fn step_report_event_severity(status: &StepReportStatus) -> &'static str {
    match status {
        StepReportStatus::Completed => "INFO",
        StepReportStatus::Partial | StepReportStatus::Blocked => "WARN",
        StepReportStatus::Failed => "ERROR",
    }
}

fn verification_event_name(result: &VerificationOutcome) -> &'static str {
    match result {
        VerificationOutcome::Pass => "workflow.verification.passed",
        VerificationOutcome::Fail => "workflow.verification.failed",
        VerificationOutcome::CannotVerify => "workflow.verification.cannot_verify",
    }
}

fn verification_event_severity(result: &VerificationOutcome) -> &'static str {
    match result {
        VerificationOutcome::Pass => "INFO",
        VerificationOutcome::Fail => "ERROR",
        VerificationOutcome::CannotVerify => "WARN",
    }
}

impl OrchestratorOutput {
    pub fn validate(
        &self,
        goal: &GoalContract,
        registry: &AgentRegistry,
    ) -> Result<(), WorkflowError> {
        match self {
            OrchestratorOutput::WorkflowCreate { workflow } => workflow.validate(goal, registry),
            OrchestratorOutput::WorkflowPatch { patch } => {
                if patch.ops.is_empty() {
                    return Err(WorkflowError::EmptyPatch(patch.patch_id.clone()));
                }
                Ok(())
            }
            OrchestratorOutput::CannotProceed {
                reason_code,
                summary,
            } => {
                if reason_code.trim().is_empty() || summary.trim().is_empty() {
                    return Err(WorkflowError::InvalidCannotProceed);
                }
                Ok(())
            }
        }
    }
}

impl HarnessEvent {
    pub fn new(
        timestamp: impl Into<String>,
        severity_text: impl Into<String>,
        event_name: impl Into<String>,
        body: impl Into<String>,
        attributes: Value,
    ) -> Self {
        Self {
            timestamp: timestamp.into(),
            severity_text: severity_text.into(),
            event_name: event_name.into(),
            body: body.into(),
            attributes,
        }
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum WorkflowError {
    #[error("goal contract has no success definition")]
    GoalHasNoSuccessDefinition,
    #[error(
        "workflow goal id {workflow_goal_id} does not match contract goal id {contract_goal_id}"
    )]
    GoalMismatch {
        workflow_goal_id: String,
        contract_goal_id: String,
    },
    #[error("workflow version {0} is invalid")]
    InvalidVersion(usize),
    #[error("workflow must contain at least one step")]
    EmptyWorkflow,
    #[error("invalid control policy: {0}")]
    InvalidControlPolicy(String),
    #[error("step id cannot be empty")]
    EmptyStepId,
    #[error("duplicate step id {0}")]
    DuplicateStepId(String),
    #[error("unknown agent type {0}")]
    UnknownAgentType(String),
    #[error("step {0} has no acceptance criteria")]
    MissingAcceptanceCriteria(String),
    #[error("step {0} has no goal alignment")]
    MissingGoalAlignment(String),
    #[error("step {step_id} references goal clause {clause} outside success definition")]
    GoalClauseOutOfRange { step_id: String, clause: usize },
    #[error("step {step_id} depends on unknown step {dependency}")]
    UnknownDependency { step_id: String, dependency: String },
    #[error("cyclic dependency around step {0}")]
    CyclicDependency(String),
    #[error("patch targets workflow {actual}, expected {expected}")]
    PatchWorkflowMismatch { expected: String, actual: String },
    #[error(
        "run targets workflow {run_workflow_id}@v{run_workflow_version}, expected {workflow_id}@v{workflow_version}"
    )]
    RunWorkflowMismatch {
        run_workflow_id: String,
        run_workflow_version: usize,
        workflow_id: String,
        workflow_version: usize,
    },
    #[error("patch from_version {actual} does not match workflow version {expected}")]
    PatchVersionMismatch { expected: usize, actual: usize },
    #[error("patch version must increase, got {from_version}->{to_version}")]
    InvalidPatchVersion {
        from_version: usize,
        to_version: usize,
    },
    #[error("patch target step {0} does not exist")]
    PatchTargetMissing(String),
    #[error("patch {0} must contain at least one operation")]
    EmptyPatch(String),
    #[error("patch {patch_id} exceeds max patch chain {max_patch_chain} at version {attempted_to_version}")]
    PatchChainLimitExceeded {
        patch_id: String,
        max_patch_chain: usize,
        attempted_to_version: usize,
    },
    #[error("cannot_proceed output must include reason_code and summary")]
    InvalidCannotProceed,
    #[error("unsafe workflow runtime path component {0}")]
    UnsafeRuntimeComponent(String),
    #[error("workflow runtime event io error: {0}")]
    RuntimeEventIo(String),
    #[error("workflow runtime event parse error: {0}")]
    RuntimeEventParse(String),
    #[error("workflow runtime state io error: {0}")]
    RuntimeStateIo(String),
    #[error("workflow runtime state parse error: {0}")]
    RuntimeStateParse(String),
    #[error("workflow runtime state missing: {0}")]
    RuntimeStateMissing(String),
}

fn empty_object() -> Value {
    Value::Object(serde_json::Map::new())
}

fn active_agent_counts(workflow: &WorkflowIr, active: &HashSet<&str>) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for step in &workflow.steps {
        if active.contains(step.step_id.as_str()) {
            *counts.entry(step.agent_type.clone()).or_insert(0) += 1;
        }
    }
    counts
}

fn dependencies_verified(workflow: &WorkflowIr, step: &WorkflowStep) -> bool {
    step.dependencies.iter().all(|dependency| {
        workflow.steps.iter().any(|candidate| {
            candidate.step_id == *dependency
                && matches!(
                    candidate.status,
                    WorkflowStepStatus::Verified | WorkflowStepStatus::Skipped
                )
        })
    })
}

fn step_ids_with_statuses(workflow: &WorkflowIr, statuses: &[WorkflowStepStatus]) -> Vec<String> {
    workflow
        .steps
        .iter()
        .filter(|step| statuses.iter().any(|status| status == &step.status))
        .map(|step| step.step_id.clone())
        .collect()
}

fn step_input_roots(step: &WorkflowStep) -> Vec<PathBuf> {
    let roots = step
        .inputs
        .get("files")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter_map(root_from_glob)
        .collect::<Vec<_>>();

    if roots.is_empty() {
        vec![PathBuf::from(".")]
    } else {
        roots
    }
}

fn step_write_roots(run: &WorkflowRunState, step: &WorkflowStep) -> Vec<PathBuf> {
    match step.mode {
        WorkflowStepMode::WritePatch => vec![run
            .worktree_map
            .get(&step.step_id)
            .cloned()
            .unwrap_or_else(|| default_worktree_path(&run.run_id, &step.step_id))],
        WorkflowStepMode::TestOnly => vec![PathBuf::from(format!(
            ".dualtrack/memory/working/workflows/{}/{}",
            run.run_id, step.step_id
        ))],
        WorkflowStepMode::ReadOnly | WorkflowStepMode::ReviewOnly => Vec::new(),
    }
}

fn default_worktree_path(run_id: &str, step_id: &str) -> PathBuf {
    PathBuf::from(format!(".harness/worktrees/{}/{}", run_id, step_id))
}

fn root_from_glob(pattern: &str) -> Option<PathBuf> {
    let trimmed = pattern.trim();
    if trimmed.is_empty() {
        return None;
    }

    let wildcard_at = trimmed
        .char_indices()
        .find_map(|(idx, ch)| matches!(ch, '*' | '?' | '[').then_some(idx))
        .unwrap_or(trimmed.len());
    let prefix = trimmed[..wildcard_at].trim_end_matches(['/', '\\']);

    if prefix.is_empty() {
        Some(PathBuf::from("."))
    } else {
        Some(PathBuf::from(prefix))
    }
}

fn infer_network_policy(tools: &[String]) -> NetworkPolicy {
    if tools.iter().any(|tool| tool == "web_search") {
        NetworkPolicy::Allowed
    } else if tools
        .iter()
        .any(|tool| tool == "arxiv_search" || tool == "semantic_scholar_search")
    {
        NetworkPolicy::AcademicOnly
    } else {
        NetworkPolicy::Disabled
    }
}

fn runtime_secs_for_mode(mode: &WorkflowStepMode) -> u64 {
    match mode {
        WorkflowStepMode::ReadOnly | WorkflowStepMode::ReviewOnly => 300,
        WorkflowStepMode::TestOnly => 600,
        WorkflowStepMode::WritePatch => 900,
    }
}

fn is_delegation_tool(tool: &str) -> bool {
    matches!(
        tool,
        "delegate" | "spawn_subagent" | "agent_team" | "direct_agent_message"
    )
}

pub(crate) fn safe_runtime_component(value: &str) -> Result<&str, WorkflowError> {
    let trimmed = value.trim();
    if trimmed != value
        || trimmed.is_empty()
        || trimmed == "."
        || trimmed == ".."
        || trimmed.contains("..")
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || !trimmed
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-'))
    {
        return Err(WorkflowError::UnsafeRuntimeComponent(value.to_string()));
    }
    Ok(trimmed)
}

fn patch_decision_verb(accepted: bool) -> &'static str {
    if accepted {
        "accepted"
    } else {
        "rejected"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn goal_contract() -> GoalContract {
        GoalContract {
            goal_id: "goal_demo".to_string(),
            goal_text: "Improve harness workflow runtime".to_string(),
            success_definition: vec![
                "Workflow is schema-valid".to_string(),
                "Verifier findings can request a patch".to_string(),
            ],
            non_goals: vec![],
            constraints: json!({"allow_new_dependency": false}),
            context_scope: vec!["src-tauri/src/harness/**".to_string()],
            approval_policy: json!({"before_shell": true}),
            budget: json!({"max_iterations": 3}),
            created_at: "2026-06-03T00:00:00Z".to_string(),
        }
    }

    fn registry() -> AgentRegistry {
        AgentRegistry::from_agents(vec![
            AgentRegistryEntry {
                agent_type: "repo_researcher".to_string(),
                allowed_tools: vec!["Read".to_string(), "Grep".to_string()],
                denied_tools: vec!["Write".to_string()],
                default_mode: WorkflowStepMode::ReadOnly,
                max_parallelism: 2,
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
            AgentRegistryEntry {
                agent_type: "test_runner".to_string(),
                allowed_tools: vec!["Read".to_string(), "Shell".to_string()],
                denied_tools: vec!["Edit".to_string()],
                default_mode: WorkflowStepMode::TestOnly,
                max_parallelism: 1,
                can_delegate: false,
            },
        ])
    }

    fn step(step_id: &str, agent_type: &str, dependencies: Vec<String>) -> WorkflowStep {
        WorkflowStep {
            step_id: step_id.to_string(),
            title: format!("Step {}", step_id),
            kind: WorkflowStepKind::Implement,
            agent_type: agent_type.to_string(),
            mode: WorkflowStepMode::WritePatch,
            task: "Produce a patch-shaped result, not a direct global decision.".to_string(),
            inputs: json!({"files": ["src-tauri/src/harness/**"]}),
            dependencies,
            acceptance_criteria: vec!["Return a structured step report".to_string()],
            goal_alignment: GoalAlignment {
                success_clauses: vec![1],
                why_necessary: "Defines the executable workflow contract".to_string(),
            },
            retry_policy: RetryPolicy {
                max_attempts: 2,
                backoff_ms: 1000,
            },
            status: WorkflowStepStatus::Pending,
        }
    }

    fn workflow() -> WorkflowIr {
        WorkflowIr {
            workflow_id: "wf_goal_demo".to_string(),
            goal_id: "goal_demo".to_string(),
            version: 1,
            parent_version: None,
            status: WorkflowStatus::Running,
            global_context: json!({"repo_summary": "FeroHa harness"}),
            control_policy: ControlPolicy {
                max_parallel_steps: 2,
                replan_on_verification_fail: true,
                max_patch_chain: 3,
            },
            steps: vec![
                step("S001", "repo_researcher", vec![]),
                step("S002", "code_writer", vec!["S001".to_string()]),
            ],
            created_by: "orchestrator@v1".to_string(),
            created_at: "2026-06-03T00:01:00Z".to_string(),
        }
    }

    #[test]
    fn workflow_validation_rejects_missing_goal_alignment() {
        let mut wf = workflow();
        wf.steps[1].goal_alignment.success_clauses.clear();

        let error = wf.validate(&goal_contract(), &registry()).unwrap_err();

        assert_eq!(
            error,
            WorkflowError::MissingGoalAlignment("S002".to_string())
        );
    }

    #[test]
    fn workflow_validation_rejects_unknown_agent_type() {
        let mut wf = workflow();
        wf.steps[1].agent_type = "unregistered_agent".to_string();

        let error = wf.validate(&goal_contract(), &registry()).unwrap_err();

        assert_eq!(
            error,
            WorkflowError::UnknownAgentType("unregistered_agent".to_string())
        );
    }

    #[test]
    fn agent_registry_deny_list_wins_and_subagents_do_not_delegate_by_default() {
        let registry = registry();

        assert!(registry.allows_tool("repo_researcher", "Read"));
        assert!(!registry.allows_tool("repo_researcher", "Write"));
        assert!(!registry.can_delegate("repo_researcher"));
    }

    #[test]
    fn workflow_patch_adds_steps_and_preserves_version_chain() {
        let wf = workflow();
        let patch = WorkflowPatch {
            patch_id: "patch_wf_goal_demo_v1_to_v2".to_string(),
            workflow_id: "wf_goal_demo".to_string(),
            from_version: 1,
            to_version: 2,
            basis: PatchBasis {
                failed_steps: vec!["S002".to_string()],
                failed_goal_clauses: vec![2],
            },
            ops: vec![WorkflowPatchOp::AddStep {
                after: Some("S002".to_string()),
                step: step("S003", "code_writer", vec!["S002".to_string()]),
            }],
            rationale: "Only add the missing verifier-driven repair step.".to_string(),
            predicted_impact: json!({"additional_runtime_minutes": 6}),
        };

        let patched = wf
            .apply_patch(&patch, &goal_contract(), &registry())
            .unwrap();

        assert_eq!(patched.version, 2);
        assert_eq!(patched.parent_version, Some(1));
        assert_eq!(
            patched
                .steps
                .iter()
                .map(|s| s.step_id.as_str())
                .collect::<Vec<_>>(),
            vec!["S001", "S002", "S003"]
        );
        assert_eq!(patched.steps[1].status, WorkflowStepStatus::Pending);
    }

    #[test]
    fn workflow_patch_rejects_empty_ops_and_patch_chain_over_budget() {
        let wf = workflow();
        let empty_patch = WorkflowPatch {
            patch_id: "patch_empty".to_string(),
            workflow_id: "wf_goal_demo".to_string(),
            from_version: 1,
            to_version: 2,
            basis: PatchBasis {
                failed_steps: vec![],
                failed_goal_clauses: vec![],
            },
            ops: vec![],
            rationale: "No operation".to_string(),
            predicted_impact: json!({}),
        };

        assert_eq!(
            wf.apply_patch(&empty_patch, &goal_contract(), &registry())
                .unwrap_err(),
            WorkflowError::EmptyPatch("patch_empty".to_string())
        );

        let over_budget = WorkflowPatch {
            patch_id: "patch_over_budget".to_string(),
            workflow_id: "wf_goal_demo".to_string(),
            from_version: 1,
            to_version: 5,
            basis: PatchBasis {
                failed_steps: vec!["S002".to_string()],
                failed_goal_clauses: vec![2],
            },
            ops: vec![WorkflowPatchOp::AddStep {
                after: Some("S002".to_string()),
                step: step("S003", "code_writer", vec!["S002".to_string()]),
            }],
            rationale: "Exceeds configured max patch chain".to_string(),
            predicted_impact: json!({}),
        };

        assert_eq!(
            wf.apply_patch(&over_budget, &goal_contract(), &registry())
                .unwrap_err(),
            WorkflowError::PatchChainLimitExceeded {
                patch_id: "patch_over_budget".to_string(),
                max_patch_chain: 3,
                attempted_to_version: 5
            }
        );
    }

    #[test]
    fn step_reports_digest_keeps_evidence_risks_and_blockers_without_embedding_artifacts() {
        let report = StepReport {
            report_id: "sr_S002_a1".to_string(),
            step_id: "S002".to_string(),
            attempt: 1,
            status: StepReportStatus::Partial,
            summary: "Implemented the first patch but route guard evidence is missing.".to_string(),
            artifacts: vec![ArtifactRef {
                artifact_id: "art_test_report".to_string(),
                artifact_type: ArtifactType::TestReport,
                uri: ".harness/runs/run_1/artifacts/test-report.json".to_string(),
                hash: "sha256:abc".to_string(),
                mime_type: "application/json".to_string(),
                producer_step_id: "S002".to_string(),
                retention_policy: RetentionPolicy::Workflow,
                created_at: "2026-06-03T00:02:00Z".to_string(),
            }],
            evidence: vec![EvidenceRef {
                file: "src/main.rs".to_string(),
                lines: vec![12, 20],
                claim: "Patch touched runtime entrypoint".to_string(),
            }],
            risks: vec!["Route guard is still unverified".to_string()],
            blocked_by: vec!["Need verifier finding".to_string()],
            suggested_next_steps: vec![SuggestedNextStep {
                proposal: "Add a verifier step for route guard".to_string(),
                reason: "Goal clause 2 lacks evidence".to_string(),
            }],
            resource_usage: json!({"token_total": 1200}),
            confidence: 0.72,
        };

        let digest = StepReportsDigest::from_reports(&[report]);

        assert_eq!(digest.entries.len(), 1);
        assert_eq!(digest.entries[0].artifact_ids, vec!["art_test_report"]);
        assert_eq!(
            digest.entries[0].evidence_claims,
            vec!["Patch touched runtime entrypoint"]
        );
        assert_eq!(
            digest.entries[0].risks,
            vec!["Route guard is still unverified"]
        );
        assert_eq!(digest.entries[0].blocked_by, vec!["Need verifier finding"]);
    }

    #[test]
    fn orchestrator_output_accepts_only_schema_valid_create_patch_or_cannot_proceed() {
        let create = OrchestratorOutput::WorkflowCreate {
            workflow: workflow(),
        };
        assert!(create.validate(&goal_contract(), &registry()).is_ok());

        let empty_patch = OrchestratorOutput::WorkflowPatch {
            patch: WorkflowPatch {
                patch_id: "patch_empty".to_string(),
                workflow_id: "wf_goal_demo".to_string(),
                from_version: 1,
                to_version: 2,
                basis: PatchBasis {
                    failed_steps: vec![],
                    failed_goal_clauses: vec![],
                },
                ops: vec![],
                rationale: "No real runtime change".to_string(),
                predicted_impact: json!({}),
            },
        };
        assert_eq!(
            empty_patch
                .validate(&goal_contract(), &registry())
                .unwrap_err(),
            WorkflowError::EmptyPatch("patch_empty".to_string())
        );

        let blocked = OrchestratorOutput::CannotProceed {
            reason_code: "budget_exceeded".to_string(),
            summary: "The requested patch chain exceeds max_patch_chain.".to_string(),
        };
        assert!(blocked.validate(&goal_contract(), &registry()).is_ok());
    }

    #[test]
    fn verifier_finding_and_harness_event_are_auditable_runtime_records() {
        let finding = VerificationFinding {
            verification_id: "vf_goal_demo_001".to_string(),
            level: VerificationLevel::Goal,
            target: "wf_goal_demo@v1".to_string(),
            result: VerificationOutcome::Fail,
            failed_clauses: vec![2],
            reason_code: "missing_verifier_evidence".to_string(),
            summary: "Goal clause 2 has no attached evidence artifact.".to_string(),
            evidence_refs: vec!["art_test_report".to_string()],
            minimal_fix_surface: vec!["Add a verification step with evidence refs".to_string()],
        };
        let value = serde_json::to_value(&finding).unwrap();

        assert_eq!(value["level"], "goal");
        assert_eq!(value["result"], "fail");
        assert_eq!(value["failed_clauses"], json!([2]));

        let event = HarnessEvent::new(
            "2026-06-03T00:03:00Z",
            "WARN",
            "verification.failed",
            "Goal verifier failed clause 2",
            json!({
                "workflow_id": "wf_goal_demo",
                "verification_id": "vf_goal_demo_001"
            }),
        );

        assert_eq!(event.severity_text, "WARN");
        assert_eq!(event.event_name, "verification.failed");
        assert_eq!(event.attributes["workflow_id"], "wf_goal_demo");
    }

    #[test]
    fn run_state_dispatches_only_ready_steps_with_verified_dependencies_and_sandbox() {
        let mut wf = workflow();
        wf.steps[0].status = WorkflowStepStatus::Verified;
        wf.steps[1].status = WorkflowStepStatus::Ready;

        let mut second_writer = step("S003", "code_writer", vec!["S001".to_string()]);
        second_writer.status = WorkflowStepStatus::Ready;
        wf.steps.push(second_writer);

        let mut blocked_by_unverified_dep = step("S004", "code_writer", vec!["S002".to_string()]);
        blocked_by_unverified_dep.status = WorkflowStepStatus::Ready;
        wf.steps.push(blocked_by_unverified_dep);

        let mut read_only = step("S005", "repo_researcher", vec!["S001".to_string()]);
        read_only.mode = WorkflowStepMode::ReadOnly;
        read_only.status = WorkflowStepStatus::Ready;
        wf.steps.push(read_only);

        let run = WorkflowRunState::for_workflow("run_demo", &wf, "2026-06-03T00:04:00Z");

        let dispatches = run.ready_dispatches(&wf, &registry()).unwrap();

        assert_eq!(
            dispatches
                .iter()
                .map(|dispatch| dispatch.step_id.as_str())
                .collect::<Vec<_>>(),
            vec!["S002", "S005"]
        );

        let writer = dispatches
            .iter()
            .find(|dispatch| dispatch.step_id == "S002")
            .unwrap();
        assert_eq!(writer.workflow_id, "wf_goal_demo");
        assert_eq!(writer.run_id, "run_demo");
        assert_eq!(writer.capability, WorkflowStepKind::Implement);
        assert_eq!(
            writer.inputs,
            json!({"files": ["src-tauri/src/harness/**"]})
        );
        assert_eq!(
            writer.artifact_contract.expected_output,
            "Produce a patch-shaped result, not a direct global decision."
        );
        assert_eq!(
            writer.artifact_contract.acceptance_criteria,
            vec!["Return a structured step report"]
        );
        assert_eq!(writer.artifact_contract.success_clauses, vec![1]);
        assert!(writer.sandbox_policy.allows_tool("Edit"));
        assert!(!writer.sandbox_policy.allows_tool("Shell"));
        assert_eq!(
            writer.sandbox_policy.write_roots,
            vec![std::path::PathBuf::from(".harness/worktrees/run_demo/S002")]
        );

        let researcher = dispatches
            .iter()
            .find(|dispatch| dispatch.step_id == "S005")
            .unwrap();
        assert!(researcher.sandbox_policy.write_roots.is_empty());
        assert!(researcher.sandbox_policy.allows_tool("Read"));
        assert!(!researcher.sandbox_policy.allows_tool("Write"));

        let event_chain =
            WorkflowRuntimeEventChain::from_step_dispatches(&dispatches, "2026-06-03T00:04:01Z");
        assert_eq!(event_chain.events.len(), 2);
        assert_eq!(event_chain.events[0].event_name, "workflow.step.dispatched");
        assert_eq!(
            event_chain.events[0].attributes["workflow_id"],
            "wf_goal_demo"
        );
        assert_eq!(event_chain.events[0].attributes["run_id"], "run_demo");
        assert_eq!(event_chain.events[0].attributes["step_id"], "S002");
        assert_eq!(event_chain.events[0].attributes["capability"], "implement");
    }

    #[test]
    fn replan_request_uses_six_digest_inputs_instead_of_raw_step_outputs() {
        let mut wf = workflow();
        wf.steps[0].status = WorkflowStepStatus::Verified;
        wf.steps[1].status = WorkflowStepStatus::Failed;
        let run = WorkflowRunState::for_workflow("run_demo", &wf, "2026-06-03T00:04:00Z");
        let report = StepReport {
            report_id: "sr_S002_a1".to_string(),
            step_id: "S002".to_string(),
            attempt: 1,
            status: StepReportStatus::Failed,
            summary: "Patch failed verifier; raw log is retained as an artifact only.".to_string(),
            artifacts: vec![ArtifactRef {
                artifact_id: "art_raw_log".to_string(),
                artifact_type: ArtifactType::Log,
                uri: ".harness/runs/run_demo/artifacts/S002/raw.log".to_string(),
                hash: "sha256:def".to_string(),
                mime_type: "text/plain".to_string(),
                producer_step_id: "S002".to_string(),
                retention_policy: RetentionPolicy::Run,
                created_at: "2026-06-03T00:05:00Z".to_string(),
            }],
            evidence: vec![EvidenceRef {
                file: "src-tauri/src/harness/workflow.rs".to_string(),
                lines: vec![1],
                claim: "Verifier finding can be tied to a workflow contract".to_string(),
            }],
            risks: vec!["Needs a verifier-driven repair step".to_string()],
            blocked_by: vec![],
            suggested_next_steps: vec![SuggestedNextStep {
                proposal: "Ask orchestrator for a workflow_patch".to_string(),
                reason: "Harness owns state; verifier only reports".to_string(),
            }],
            resource_usage: json!({"token_total": 900}),
            confidence: 0.61,
        };
        let finding = VerificationFinding {
            verification_id: "vf_S002".to_string(),
            level: VerificationLevel::Step,
            target: "S002".to_string(),
            result: VerificationOutcome::Fail,
            failed_clauses: vec![2],
            reason_code: "missing_runtime_state".to_string(),
            summary: "Step failed because runtime state cannot yet request a patch.".to_string(),
            evidence_refs: vec!["art_raw_log".to_string()],
            minimal_fix_surface: vec!["Create a structured replan request".to_string()],
        };

        let request = OrchestratorReplanRequest::from_runtime(
            &goal_contract(),
            &wf,
            &run,
            &[report],
            &[finding],
            &registry(),
        );
        let value = serde_json::to_value(&request).unwrap();

        for key in [
            "goal_contract",
            "workflow_state_summary",
            "step_reports_digest",
            "verifier_findings",
            "agent_registry",
            "runtime_policies",
        ] {
            assert!(value.get(key).is_some(), "missing {key}");
        }

        assert_eq!(request.workflow_state_summary.failed_step_ids, vec!["S002"]);
        assert_eq!(
            request.step_reports_digest.entries[0].artifact_ids,
            vec!["art_raw_log"]
        );
        assert!(!value.to_string().contains("raw.log"));
        assert!(!request.runtime_policies.allow_subagent_delegation);
        assert!(request
            .runtime_policies
            .allowed_orchestrator_outputs
            .contains(&"workflow_patch".to_string()));
    }

    #[test]
    fn runtime_event_chain_records_reports_findings_and_replan_request() {
        let mut wf = workflow();
        wf.steps[0].status = WorkflowStepStatus::Verified;
        wf.steps[1].status = WorkflowStepStatus::Failed;
        let run = WorkflowRunState::for_workflow("run_demo", &wf, "2026-06-07T00:04:00Z");
        let report = StepReport {
            report_id: "sr_S002_a1".to_string(),
            step_id: "S002".to_string(),
            attempt: 1,
            status: StepReportStatus::Failed,
            summary: "Runtime patch failed verifier.".to_string(),
            artifacts: vec![],
            evidence: vec![],
            risks: vec!["Needs runtime replan".to_string()],
            blocked_by: vec![],
            suggested_next_steps: vec![],
            resource_usage: json!({"token_total": 700}),
            confidence: 0.62,
        };
        let finding = VerificationFinding {
            verification_id: "vf_S002".to_string(),
            level: VerificationLevel::Step,
            target: "S002".to_string(),
            result: VerificationOutcome::Fail,
            failed_clauses: vec![2],
            reason_code: "missing_runtime_state".to_string(),
            summary: "Verifier cannot confirm runtime state persistence.".to_string(),
            evidence_refs: vec!["sr_S002_a1".to_string()],
            minimal_fix_surface: vec!["Persist runtime state before patch request".to_string()],
        };
        let request = OrchestratorReplanRequest::from_runtime(
            &goal_contract(),
            &wf,
            &run,
            &[report],
            &[finding],
            &registry(),
        );

        let chain =
            WorkflowRuntimeEventChain::from_replan_request(&request, "2026-06-07T00:05:00Z");

        assert_eq!(chain.workflow_id, "wf_goal_demo");
        assert_eq!(chain.run_id, "run_demo");
        assert!(chain.replan_requested);
        assert_eq!(
            chain
                .events
                .iter()
                .map(|event| event.event_name.as_str())
                .collect::<Vec<_>>(),
            vec![
                "workflow.step_report.recorded",
                "workflow.verification.failed",
                "workflow.replan.requested"
            ]
        );
        assert_eq!(chain.events[1].severity_text, "ERROR");
        assert_eq!(
            chain.events[1].attributes["reason_code"],
            "missing_runtime_state"
        );
        assert_eq!(chain.events[2].attributes["finding_count"], 1);
        assert_eq!(
            chain.events[2].attributes["failed_step_ids"],
            json!(["S002"])
        );
    }

    #[test]
    fn runtime_event_store_appends_replan_and_patch_decision_events_to_run_ledger() {
        let mut wf = workflow();
        wf.steps[0].status = WorkflowStepStatus::Verified;
        wf.steps[1].status = WorkflowStepStatus::Failed;
        let run = WorkflowRunState::for_workflow("run_demo", &wf, "2026-06-07T00:04:00Z");
        let report = StepReport {
            report_id: "sr_S002_a1".to_string(),
            step_id: "S002".to_string(),
            attempt: 1,
            status: StepReportStatus::Failed,
            summary: "Runtime patch failed verifier.".to_string(),
            artifacts: vec![],
            evidence: vec![],
            risks: vec!["Needs runtime replan".to_string()],
            blocked_by: vec![],
            suggested_next_steps: vec![],
            resource_usage: json!({"token_total": 700}),
            confidence: 0.62,
        };
        let finding = VerificationFinding {
            verification_id: "vf_S002".to_string(),
            level: VerificationLevel::Step,
            target: "S002".to_string(),
            result: VerificationOutcome::Fail,
            failed_clauses: vec![2],
            reason_code: "missing_runtime_state".to_string(),
            summary: "Verifier cannot confirm runtime state persistence.".to_string(),
            evidence_refs: vec!["sr_S002_a1".to_string()],
            minimal_fix_surface: vec!["Persist runtime state before patch request".to_string()],
        };
        let request = OrchestratorReplanRequest::from_runtime(
            &goal_contract(),
            &wf,
            &run,
            &[report],
            &[finding],
            &registry(),
        );
        let replan_chain =
            WorkflowRuntimeEventChain::from_replan_request(&request, "2026-06-07T00:05:00Z");
        let patch = WorkflowPatch {
            patch_id: "patch_wf_goal_demo_v1_to_v2".to_string(),
            workflow_id: "wf_goal_demo".to_string(),
            from_version: 1,
            to_version: 2,
            basis: PatchBasis {
                failed_steps: vec!["S002".to_string()],
                failed_goal_clauses: vec![2],
            },
            ops: vec![WorkflowPatchOp::AddStep {
                after: Some("S002".to_string()),
                step: step("S003", "code_writer", vec!["S002".to_string()]),
            }],
            rationale: "Add verifier-driven repair step.".to_string(),
            predicted_impact: json!({"additional_runtime_minutes": 6}),
        };
        let patch_chain = WorkflowRuntimeEventChain::from_patch_decision(
            "wf_goal_demo",
            "run_demo",
            &patch,
            true,
            "2026-06-07T00:06:00Z",
            "Patch passed bridge review.",
        );
        let temp = tempfile::tempdir().unwrap();

        let ledger_path = WorkflowRuntimeEventStore::append_chain(temp.path(), &replan_chain)
            .expect("replan chain should persist");
        WorkflowRuntimeEventStore::append_chain(temp.path(), &patch_chain)
            .expect("patch decision should append");
        let events = WorkflowRuntimeEventStore::read_recent(temp.path(), "run_demo", 10)
            .expect("events should reload");

        assert_eq!(
            ledger_path,
            temp.path()
                .join(".harness")
                .join("runs")
                .join("run_demo")
                .join("events.jsonl")
        );
        assert_eq!(
            events
                .iter()
                .map(|event| event.event_name.as_str())
                .collect::<Vec<_>>(),
            vec![
                "workflow.step_report.recorded",
                "workflow.verification.failed",
                "workflow.replan.requested",
                "workflow.patch.accepted"
            ]
        );
        assert_eq!(
            events[3].attributes["patch_id"],
            "patch_wf_goal_demo_v1_to_v2"
        );
        assert_eq!(events[3].attributes["op_count"], 1);
    }

    #[test]
    fn runtime_event_store_rejects_unsafe_run_ids() {
        let temp = tempfile::tempdir().unwrap();
        let chain = WorkflowRuntimeEventChain {
            workflow_id: "wf_goal_demo".to_string(),
            run_id: "../escape".to_string(),
            events: vec![HarnessEvent::new(
                "2026-06-07T00:05:00Z",
                "INFO",
                "workflow.step_report.recorded",
                "Unsafe run id should never choose the ledger path.",
                json!({}),
            )],
            replan_requested: false,
        };

        assert_eq!(
            WorkflowRuntimeEventStore::append_chain(temp.path(), &chain).unwrap_err(),
            WorkflowError::UnsafeRuntimeComponent("../escape".to_string())
        );
    }

    #[test]
    fn runtime_event_store_rejects_padded_run_ids() {
        let temp = tempfile::tempdir().unwrap();

        assert_eq!(
            WorkflowRuntimeEventStore::event_log_path(temp.path(), " run_demo "),
            Err(WorkflowError::UnsafeRuntimeComponent(
                " run_demo ".to_string()
            ))
        );
    }

    #[test]
    fn runtime_event_store_rejects_dotted_run_ids() {
        let temp = tempfile::tempdir().unwrap();

        for run_id in ["run.demo", "run_demo.", ".run_demo"] {
            assert_eq!(
                WorkflowRuntimeEventStore::event_log_path(temp.path(), run_id),
                Err(WorkflowError::UnsafeRuntimeComponent(run_id.to_string()))
            );
        }
    }
}
