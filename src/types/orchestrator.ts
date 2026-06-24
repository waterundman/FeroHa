export interface OrchestratorStatus {
  active_agents: number;
  degraded_agents: string[];
  epoch_count: number;
  track_count: number;
  track_event_count: number;
  material_packet_count: number;
  active_track_count: number;
  completed_track_count: number;
  failed_track_count: number;
  cancelled_track_count: number;
  last_event: OrchestratorEvent | null;
  recent_events: OrchestratorEvent[];
  agent_states: AgentState[];
  track_details: TrackInfo[];
  diagnostics: OrchestratorDiagnostic[];
  workflow_event_count?: number;
  workflow_replan_request_count?: number;
  recent_workflow_events?: HarnessEvent[];
  workflow_event_log_path?: string | null;
}

export interface OrchestratorEvent {
  epoch: number;
  agent_id: string;
  event_type: string;
  timestamp: number;
  detail: string | null;
}

export interface HarnessEvent {
  timestamp: string;
  severity_text: string;
  event_name: string;
  body: string;
  attributes: Record<string, unknown>;
}

export interface GoalContract {
  goal_id: string;
  goal_text: string;
  success_definition: string[];
  non_goals: string[];
  constraints: Record<string, unknown>;
  context_scope: string[];
  approval_policy: Record<string, unknown>;
  budget: Record<string, unknown>;
  created_at: string;
}

export type WorkflowStatus = "draft" | "running" | "paused" | "completed" | "aborted";
export type WorkflowStepKind = "research" | "implement" | "test" | "review" | "verify" | "merge";
export type WorkflowStepMode = "read_only" | "write_patch" | "test_only" | "review_only";
export type WorkflowStepStatus =
  | "pending"
  | "ready"
  | "running"
  | "reported"
  | "verified"
  | "failed"
  | "blocked"
  | "skipped";

export interface WorkflowStep {
  step_id: string;
  title: string;
  kind: WorkflowStepKind;
  agent_type: string;
  mode: WorkflowStepMode;
  task: string;
  inputs: Record<string, unknown>;
  dependencies: string[];
  acceptance_criteria: string[];
  goal_alignment: {
    success_clauses: number[];
    why_necessary: string;
  };
  retry_policy: {
    max_attempts: number;
    backoff_ms: number;
  };
  status: WorkflowStepStatus;
}

export interface WorkflowIr {
  workflow_id: string;
  goal_id: string;
  version: number;
  parent_version: number | null;
  status: WorkflowStatus;
  global_context: Record<string, unknown>;
  control_policy: {
    max_parallel_steps: number;
    replan_on_verification_fail: boolean;
    max_patch_chain: number;
  };
  steps: WorkflowStep[];
  created_by: string;
  created_at: string;
}

export type WorkflowRunStatus = "queued" | "running" | "paused" | "failed" | "succeeded";

export interface WorkflowRunState {
  run_id: string;
  workflow_id: string;
  workflow_version: number;
  status: WorkflowRunStatus;
  started_at: string;
  ended_at: string | null;
  active_step_ids: string[];
  worktree_map: Record<string, string>;
  metrics: Record<string, unknown>;
  context_digest_version: number;
}

export interface AgentRegistryEntry {
  agent_type: string;
  allowed_tools: string[];
  denied_tools: string[];
  default_mode: WorkflowStepMode;
  max_parallelism: number;
  can_delegate: boolean;
}

export interface AgentRegistry {
  agents: Record<string, AgentRegistryEntry>;
}

export type WorkflowDispatchStatus =
  | "dispatched"
  | "queued"
  | "running"
  | "reported"
  | "failed"
  | "unsupported";

export interface WorkflowDispatchRecord {
  step_id: string;
  attempt: number;
  task_id: string | null;
  status: WorkflowDispatchStatus;
  detail: string | null;
}

export type ArtifactType =
  | "patch"
  | "test_report"
  | "verification_report"
  | "log"
  | "screenshot"
  | "other";
export type RetentionPolicy = "ephemeral" | "run" | "workflow" | "permanent";

export interface ArtifactRef {
  artifact_id: string;
  type: ArtifactType;
  uri: string;
  hash: string;
  mime_type: string;
  producer_step_id: string;
  retention_policy: RetentionPolicy;
  created_at: string;
}

export interface EvidenceRef {
  file: string;
  lines: number[];
  claim: string;
}

export interface SuggestedNextStep {
  proposal: string;
  reason: string;
}

export type StepReportStatus = "completed" | "partial" | "failed" | "blocked";

export interface StepReport {
  report_id: string;
  step_id: string;
  attempt: number;
  status: StepReportStatus;
  summary: string;
  artifacts: ArtifactRef[];
  evidence: EvidenceRef[];
  risks: string[];
  blocked_by: string[];
  suggested_next_steps: SuggestedNextStep[];
  resource_usage: Record<string, unknown>;
  confidence: number;
}

export type VerificationLevel = "step" | "integration" | "goal";
export type VerificationOutcome = "pass" | "fail" | "cannot_verify";

export interface VerificationFinding {
  verification_id: string;
  level: VerificationLevel;
  target: string;
  result: VerificationOutcome;
  failed_clauses: number[];
  reason_code: string;
  summary: string;
  evidence_refs: string[];
  minimal_fix_surface: string[];
}

export interface WorkflowRuntimeBundle {
  goal: GoalContract;
  workflow: WorkflowIr;
  run: WorkflowRunState;
  registry: AgentRegistry;
  dispatches: WorkflowDispatchRecord[];
  artifacts: ArtifactRef[];
  step_reports: StepReport[];
  verification_findings: VerificationFinding[];
  updated_at: string;
}

export interface WorkflowPatch {
  patch_id: string;
  workflow_id: string;
  from_version: number;
  to_version: number;
  basis: {
    failed_steps: string[];
    failed_goal_clauses: number[];
  };
  ops: Array<Record<string, unknown>>;
  rationale: string;
  predicted_impact: Record<string, unknown>;
}

export type OrchestratorOutput =
  | {
      type: "workflow_create";
      workflow: Record<string, unknown>;
    }
  | {
      type: "workflow_patch";
      patch: WorkflowPatch;
    }
  | {
      type: "cannot_proceed";
      reason_code: string;
      summary: string;
    };

export interface AgentState {
  agent_id: string;
  status: string;
  regression_count: number;
  last_epoch: number;
}

export interface TrackInfo {
  track_id: string;
  focus: string;
  status: string;
  parent_agent: string;
  reason?: string | null;
  claim_count: number;
  source_ref_count: number;
}

export type OrchestratorDiagnosticSource = "EpochReason" | "WorkflowVerifier";

export interface OrchestratorDiagnostic {
  source: OrchestratorDiagnosticSource;
  reason_code: string;
  summary: string;
  target?: string | null;
  minimal_fix_surface: string[];
  evidence_refs: string[];
  failed_clauses: number[];
  severity: string;
}
