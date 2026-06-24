export type AiFaceMemoryRole =
  | "HumanTask"
  | "AiMemoryExpansion"
  | "OrchestratorVerification";

export type AiScientistVerificationState = "NoClaims" | "NotRun" | "Passed" | "Failed";

export type AiScientistConfidenceBasis =
  | "None"
  | "EvidenceFallback"
  | "KernelVerification";

export interface AiScientistVerificationSummary {
  state: AiScientistVerificationState;
  passed: boolean | null;
  violation_count: number;
  overall_confidence: number;
  confidence_basis: AiScientistConfidenceBasis;
  evidence_chain_count: number;
  kernel_name: string;
  kernel_scope: string;
  is_truth_proof: boolean;
}

export interface AiFaceDataFlow {
  task_id: string;
  manager_status: string;
  manager_phase: string;
  memory_role: AiFaceMemoryRole;
  manager_has_trace: boolean;
  orchestrator_enabled: boolean;
  scientist_claim_count: number;
  scientist_source_count: number;
  scientist_verification: AiScientistVerificationSummary;
  context_fragment_count: number;
  subagent_result_count: number;
  sandbox_summary?: string | null;
  material_packet_focus?: string | null;
}

export type AiManagerControlAction =
  | "OrchestratorTrackPending"
  | "BridgeReviewPending"
  | "RunningTasks"
  | "DispatchReady"
  | "Idle";

export interface AiManagerSnapshot {
  total_tasks: number;
  pending_review_count: number;
  execution_queue_count: number;
  running_count: number;
  completed_count: number;
  failed_count: number;
  human_task_count: number;
  memory_expansion_count: number;
  verification_track_count: number;
  bridge_required_count: number;
  read_only_count: number;
  write_capable_count: number;
  network_enabled_count: number;
  scientist_payload_count: number;
  orchestrator_packet_count: number;
  latest_control_action: AiManagerControlAction;
}
