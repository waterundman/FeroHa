export type BridgeProposalSource = "tool" | "scientist" | "dream" | "ghost" | "scheduler";

export type SourceRefKind =
  | "task"
  | "ghost"
  | "dream_insight"
  | "scientist_output"
  | "scheduler_job"
  | "tool_call";

export interface SourceRef {
  kind: SourceRefKind;
  id: string;
  path?: string;
}

export type EvidenceKind = "note" | "chunk" | "trace" | "tool_result" | "verification" | "diff";

export interface EvidenceRef {
  label: string;
  kind: EvidenceKind;
  ref: string;
  confidence?: number;
  excerpt?: string;
}

export interface ImpactScope {
  notes: string[];
  creates_files: boolean;
  modifies_notes: boolean;
  exports_data: boolean;
  external_side_effect: boolean;
}

export type ProposalRisk = "low" | "medium" | "high";

export type BridgeProposalStatus = "pending" | "approved" | "rejected" | "applied" | "archived";

export type ProposalActionKind =
  | "approve_task"
  | "approve_workflow_patch"
  | "open_diff"
  | "open_trace"
  | "apply_ghost"
  | "reject_workflow_patch"
  | "reject"
  | "archive";

export interface ProposalAction {
  id: string;
  label: string;
  kind: ProposalActionKind;
  payload?: Record<string, unknown>;
}

export interface TrustSnapshot {
  score: number;
  acceptance_rate: number;
  total_interactions: number;
  recommended_mode: string;
}

export interface BridgeProposal {
  id: string;
  source: BridgeProposalSource;
  source_ref: SourceRef;
  intent: string;
  summary: string;
  task_type?: string;
  sandbox_summary?: string;
  expected_output?: string;
  risk_reason?: string;
  evidence: EvidenceRef[];
  impact: ImpactScope;
  risk: ProposalRisk;
  status: BridgeProposalStatus;
  actions: ProposalAction[];
  trust_snapshot: TrustSnapshot;
  created_at: number;
  updated_at: number;
}

export interface BridgeProposalActionResult {
  status: string;
  message: string;
  proposal: BridgeProposal;
  metadata?: Record<string, unknown>;
}
