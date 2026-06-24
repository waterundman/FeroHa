import type { BridgeProposal, EvidenceRef, ProposalRisk } from "../types/bridge-proposal";
import type { HarnessEvent, OrchestratorEvent, OrchestratorStatus } from "../types/orchestrator";
import type { DiffBlock, TaskStatus } from "../hooks/useAppStore";

export const mockSimulationSuite = {
  caseSuccess: {
    taskId: "sim-task-good",
    humanText: "研究线性空间中的维度坍缩问题",
    orchestratorLog: [
      { epoch: 1, action: "Master Agent 启动 RAG 检索" },
      { epoch: 2, action: "构建一阶因果链" },
      { epoch: 3, action: "收敛完成，未触发退行" },
    ],
    kernelStatus: "PASSED",
    confidence: 0.96,
    citations: [
      "https://arxiv.org/abs/2604.1234",
      "local://vault/thesis_notes.md#^block-x12",
    ],
    diffContent:
      "【新增推论】基于你一手的调研数据，高维空间在特定算子作用下会发生局部仿射坍缩...",
  },
  caseRegressionAndWarning: {
    taskId: "sim-task-buggy",
    humanText: "尝试用纯几何方法证明非线性漂移的绝对消除",
    orchestratorLog: [
      { epoch: 1, action: "Master-Alpha 启动多跳推理" },
      { epoch: 2, action: "调用外部网络工具陷入死循环" },
      {
        epoch: 3,
        action:
          "🚨 检测到退行！健康度跌至 42%。Master-Alpha 降级为只读 Subagent-Alpha",
      },
      { epoch: 1, action: "⚡ 催生并行分裂代理：Alpha-mutated-v0 继承干净知识重新出发" },
      {
        epoch: 2,
        action: "Alpha-mutated-v0 放弃纯几何路线，引入混沌扰动，逻辑收敛成功",
      },
    ],
    kernelStatus: "WARNING_PASSED",
    confidence: 0.58,
    citations: ["https://wikipedia.org/wiki/Chaos_theory"],
    diffContent:
      "【高风险推论】非线性漂移无法静态消除，必须在 4D 管道中引入时间步长负反馈...",
  },
} as const;

export type MockSimulationCaseKey = keyof typeof mockSimulationSuite;

type KernelStatus = (typeof mockSimulationSuite)[MockSimulationCaseKey]["kernelStatus"];
type WarningTone = "stable" | "amber";

export interface MockSimulationRun {
  caseKey: MockSimulationCaseKey;
  task: TaskStatus;
  kernelStatus: KernelStatus;
  confidence: number;
  citations: readonly string[];
  warningTone: WarningTone;
  orchestratorStatus: OrchestratorStatus;
  bridgeProposal: BridgeProposal;
  diffBlocks: DiffBlock[];
  dispatchPayload: Record<string, unknown>;
}

export const mockSimulationCaseOptions: Array<{
  key: MockSimulationCaseKey;
  label: string;
  description: string;
}> = [
  {
    key: "caseSuccess",
    label: "投喂成功闭环",
    description: "PASSED · 置信度 96% · 低风险",
  },
  {
    key: "caseRegressionAndWarning",
    label: "投喂退行警告",
    description: "WARNING_PASSED · 置信度 58% · 琥珀警告",
  },
];

const baseTimestampByCase: Record<MockSimulationCaseKey, number> = {
  caseSuccess: 1_781_500_001_000,
  caseRegressionAndWarning: 1_781_500_002_000,
};

function agentIdForAction(action: string): string {
  if (action.includes("Alpha-mutated")) return "Alpha-mutated-v0";
  if (action.includes("Master-Alpha")) return "Master-Alpha";
  return "Master-Agent";
}

function eventTypeForAction(action: string): string {
  if (action.includes("退行")) return "regression_detected";
  if (action.includes("收敛成功") || action.includes("收敛完成")) return "converged";
  if (action.includes("死循环")) return "tool_loop_detected";
  return "progress";
}

function citationToEvidence(ref: string, index: number, confidence: number): EvidenceRef {
  const isLocal = ref.startsWith("local://");
  return {
    label: isLocal ? `本地材料 ${index + 1}` : `外部引用 ${index + 1}`,
    kind: isLocal ? "note" : "tool_result",
    ref,
    confidence,
  };
}

function riskForRun(kernelStatus: KernelStatus, confidence: number): ProposalRisk {
  if (kernelStatus === "WARNING_PASSED" || confidence < 0.7) return "high";
  if (confidence < 0.85) return "medium";
  return "low";
}

function warningToneForRun(kernelStatus: KernelStatus, confidence: number): WarningTone {
  return kernelStatus === "WARNING_PASSED" || confidence < 0.7 ? "amber" : "stable";
}

export function buildMockSimulationRun(caseKey: MockSimulationCaseKey): MockSimulationRun {
  const simulationCase = mockSimulationSuite[caseKey];
  const createdAt = baseTimestampByCase[caseKey];
  const warningTone = warningToneForRun(
    simulationCase.kernelStatus,
    simulationCase.confidence,
  );
  const isWarning = warningTone === "amber";
  const confidencePercent = Math.round(simulationCase.confidence * 100);
  const events: OrchestratorEvent[] = simulationCase.orchestratorLog.map((entry, index) => ({
    epoch: entry.epoch,
    agent_id: agentIdForAction(entry.action),
    event_type: eventTypeForAction(entry.action),
    timestamp: createdAt + index * 1_000,
    detail: entry.action,
  }));
  const workflowEvents: HarnessEvent[] = simulationCase.orchestratorLog.map((entry, index) => ({
    timestamp: new Date(createdAt + index * 1_000).toISOString(),
    severity_text: entry.action.includes("退行") || entry.action.includes("死循环")
      ? "warning"
      : "info",
    event_name: eventTypeForAction(entry.action),
    body: entry.action,
    attributes: {
      task_id: simulationCase.taskId,
      case_key: caseKey,
      epoch: entry.epoch,
      agent_id: agentIdForAction(entry.action),
    },
  }));
  const diagnostics = isWarning
    ? [
        {
          source: "EpochReason" as const,
          reason_code: "regression_health_drop",
          summary:
            "检测到退行，健康度跌至 42%。原 Master-Alpha 被降级为只读，突变代理完成收敛。",
          target: "Master-Alpha",
          minimal_fix_surface: [
            "Master-Alpha 降级为只读",
            "Alpha-mutated-v0 继承干净知识重新出发",
          ],
          evidence_refs: [...simulationCase.citations],
          failed_clauses: [1],
          severity: "warning",
        },
      ]
    : [];

  const orchestratorStatus: OrchestratorStatus = {
    active_agents: 0,
    degraded_agents: isWarning ? ["Master-Alpha"] : [],
    epoch_count: Math.max(...simulationCase.orchestratorLog.map((entry) => entry.epoch)),
    track_count: isWarning ? 2 : 1,
    track_event_count: simulationCase.orchestratorLog.length,
    material_packet_count: simulationCase.citations.length,
    active_track_count: 0,
    completed_track_count: 1,
    failed_track_count: isWarning ? 1 : 0,
    cancelled_track_count: 0,
    last_event: events[events.length - 1] ?? null,
    recent_events: events,
    agent_states: isWarning
      ? [
          {
            agent_id: "Master-Alpha",
            status: "read_only_degraded",
            regression_count: 1,
            last_epoch: 3,
          },
          {
            agent_id: "Alpha-mutated-v0",
            status: "completed",
            regression_count: 0,
            last_epoch: 2,
          },
        ]
      : [
          {
            agent_id: "Master-Agent",
            status: "completed",
            regression_count: 0,
            last_epoch: 3,
          },
        ],
    track_details: isWarning
      ? [
          {
            track_id: `${simulationCase.taskId}-geometry`,
            focus: "纯几何证明路线",
            status: "degraded",
            parent_agent: "Master-Alpha",
            reason: "外部工具调用陷入死循环并触发退行阈值",
            claim_count: 1,
            source_ref_count: 1,
          },
          {
            track_id: `${simulationCase.taskId}-mutated`,
            focus: "混沌扰动与时间步长负反馈",
            status: "completed",
            parent_agent: "Alpha-mutated-v0",
            reason: null,
            claim_count: 2,
            source_ref_count: simulationCase.citations.length,
          },
        ]
      : [
          {
            track_id: `${simulationCase.taskId}-main`,
            focus: "维度坍缩一阶因果链",
            status: "completed",
            parent_agent: "Master-Agent",
            reason: null,
            claim_count: 2,
            source_ref_count: simulationCase.citations.length,
          },
        ],
    diagnostics,
    workflow_event_count: workflowEvents.length,
    workflow_replan_request_count: isWarning ? 1 : 0,
    recent_workflow_events: workflowEvents,
    workflow_event_log_path: `mock://simulation-suite/${caseKey}`,
  };

  const risk = riskForRun(simulationCase.kernelStatus, simulationCase.confidence);
  const bridgeProposal: BridgeProposal = {
    id: `bridge-${simulationCase.taskId}`,
    source: "scheduler",
    source_ref: { kind: "task", id: simulationCase.taskId },
    intent: simulationCase.humanText,
    summary: isWarning
      ? "退行路线已隔离为只读，突变代理给出带风险标记的可审查推论。"
      : "AI 推理已通过内核校验，可进入人类审查或只读归档。",
    task_type: "research",
    sandbox_summary: `${simulationCase.kernelStatus} · 置信度 ${confidencePercent}%`,
    expected_output: simulationCase.diffContent,
    risk_reason: isWarning
      ? `置信度 ${confidencePercent}%，且出现退行降级，需要人类确认后再写入。`
      : `置信度 ${confidencePercent}%，未出现退行或外部副作用。`,
    evidence: simulationCase.citations.map((ref, index) =>
      citationToEvidence(ref, index, simulationCase.confidence),
    ),
    impact: {
      notes: [],
      creates_files: false,
      modifies_notes: true,
      exports_data: false,
      external_side_effect: false,
    },
    risk,
    status: "pending",
    actions: [
      {
        id: `${simulationCase.taskId}-open-diff`,
        label: "查看 Diff",
        kind: "open_diff",
        payload: { ghost_id: simulationCase.taskId },
      },
      {
        id: `${simulationCase.taskId}-open-trace`,
        label: "查看推理轨迹",
        kind: "open_trace",
        payload: { task_id: simulationCase.taskId },
      },
      {
        id: `${simulationCase.taskId}-archive`,
        label: "归档",
        kind: "archive",
      },
    ],
    trust_snapshot: {
      score: simulationCase.confidence,
      acceptance_rate: simulationCase.confidence,
      total_interactions: 1,
      recommended_mode: isWarning ? "manual_bridge" : "read_only_auto_queue",
    },
    created_at: createdAt,
    updated_at: createdAt,
  };

  return {
    caseKey,
    task: {
      id: simulationCase.taskId,
      command: simulationCase.humanText,
      status: "done",
      result: `${simulationCase.kernelStatus} · 置信度 ${confidencePercent}%`,
      has_trace: true,
    },
    kernelStatus: simulationCase.kernelStatus,
    confidence: simulationCase.confidence,
    citations: simulationCase.citations,
    warningTone,
    orchestratorStatus,
    bridgeProposal,
    diffBlocks: [
      {
        ghostId: simulationCase.taskId,
        id: `${simulationCase.taskId}-diff`,
        type: "inserted",
        newText: simulationCase.diffContent,
        accepted: false,
        rejected: false,
      },
    ],
    dispatchPayload: {
      intent: simulationCase.humanText,
      content: simulationCase.diffContent,
      task_type: "research",
      scope: "selected_text",
      expected_output: simulationCase.diffContent,
      review_mode: isWarning ? "manual_bridge" : "read_only_auto_queue",
      context_note: null,
      source: "mock_simulation_suite",
      simulation_case: caseKey,
      timestamp: createdAt,
    },
  };
}
