import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import {
  aiFaceFlowNarrative,
  aiFaceRoleLabel,
  aiScientistKernelBoundaryLabel,
  aiScientistVerificationDetail,
  aiScientistVerificationLabel,
  aiFaceSubjectMetrics,
  aiOrchestratorDiagnosticSummary,
  aiOrchestratorFirstFixSurface,
  aiOrchestratorWorkloadDetail,
  aiManagerControlLabel,
  aiManagerControlPoints,
  dreamRunSummaryItems,
  dreamStageCardsFromStats,
  formatDreamDuration,
  normalizeTaskStatus,
  normalizeTaskType,
} from "../AgentDashboard";

describe("AgentDashboard task status normalization", () => {
  it("keeps string statuses stable", () => {
    expect(normalizeTaskStatus("Pending")).toBe("Pending");
    expect(normalizeTaskStatus("Running")).toBe("Running");
  });

  it("normalizes Rust enum object statuses", () => {
    expect(normalizeTaskStatus({ Approved: { approved_by: "human" } })).toBe("Approved");
    expect(normalizeTaskStatus({ Running: { started_at: 1, progress: 0.5 } })).toBe("Running");
    expect(normalizeTaskStatus({ Error: "boom" })).toBe("Error");
  });

  it("normalizes Rust enum object task types before rendering", () => {
    expect(normalizeTaskType({ Custom: "research" })).toBe("research");
    expect(normalizeTaskType("dream")).toBe("dream");
    expect(normalizeTaskType(null)).toBe("任务");
  });
});

describe("AgentDashboard Dream presentation", () => {
  const stats = {
    nrem_connections_strengthened: 8,
    nrem_connections_pruned: 2,
    rem_bridges_created: 5,
    insight_communities_found: 3,
    insight_summaries_generated: 4,
    total_memories_processed: 42,
    duration_ms: 1530,
  };

  it("groups Dream stats by memory phase", () => {
    const cards = dreamStageCardsFromStats(stats);

    expect(cards.map((card) => card.title)).toEqual(["NREM 整合", "REM 桥接", "洞察提炼"]);
    expect(cards[0].metrics).toEqual([
      { label: "强化连接", value: 8 },
      { label: "剪枝连接", value: 2 },
    ]);
    expect(cards[1].metrics).toEqual([{ label: "桥接连接", value: 5 }]);
    expect(cards[2].metrics).toEqual([{ label: "记忆社区", value: 3 }]);
  });

  it("formats Dream run summaries compactly", () => {
    expect(formatDreamDuration(1530)).toBe("1.5s");
    expect(dreamRunSummaryItems(stats)).toEqual([
      "处理 42 个记忆块",
      "生成 4 条洞察摘要",
      "耗时 1.5s",
    ]);
  });

  it("keeps the Dream engine layout responsive and bounded", () => {
    const css = readFileSync(join(process.cwd(), "src/components/AgentDashboard.css"), "utf8");

    expect(css).toContain(".dream-control-strip");
    expect(css).toContain("grid-template-columns: minmax(220px, 0.42fr) minmax(0, 1fr)");
    expect(css).toContain("max-height: min(260px, 36vh)");
    expect(css).toContain("@media (max-width: 720px)");
  });
});

describe("AgentDashboard AI face triad presentation", () => {
  const flows = [
    {
      task_id: "human-task",
      manager_status: "done",
      manager_phase: "Done",
      memory_role: "HumanTask" as const,
      manager_has_trace: true,
      orchestrator_enabled: true,
      scientist_claim_count: 3,
      scientist_source_count: 2,
      context_fragment_count: 1,
      subagent_result_count: 2,
      scientist_verification: {
        state: "NotRun" as const,
        passed: null,
        violation_count: 0,
        overall_confidence: 0.82,
        confidence_basis: "EvidenceFallback" as const,
        evidence_chain_count: 3,
        kernel_name: "PropositionKernel",
        kernel_scope: "not_run",
        is_truth_proof: false,
      },
      sandbox_summary: "read-only tools",
      material_packet_focus: null,
    },
    {
      task_id: "track-task",
      manager_status: "approved",
      manager_phase: "Idle",
      memory_role: "OrchestratorVerification" as const,
      manager_has_trace: false,
      orchestrator_enabled: true,
      scientist_claim_count: 1,
      scientist_source_count: 1,
      context_fragment_count: 1,
      subagent_result_count: 1,
      scientist_verification: {
        state: "NotRun" as const,
        passed: null,
        violation_count: 0,
        overall_confidence: 0.64,
        confidence_basis: "EvidenceFallback" as const,
        evidence_chain_count: 1,
        kernel_name: "PropositionKernel",
        kernel_scope: "not_run",
        is_truth_proof: false,
      },
      sandbox_summary: "verify-only",
      material_packet_focus: "correctness",
    },
  ];

  it("labels memory roles in Chinese", () => {
    expect(aiFaceRoleLabel("HumanTask")).toBe("人工任务");
    expect(aiFaceRoleLabel("AiMemoryExpansion")).toBe("AI 记忆拓展");
    expect(aiFaceRoleLabel("OrchestratorVerification")).toBe("编排验证轨道");
  });

  it("summarizes manager scientist and orchestrator workloads", () => {
    const metrics = aiFaceSubjectMetrics(
      flows,
      {
        active_agents: 2,
        degraded_agents: [],
        epoch_count: 4,
        track_count: 2,
        track_event_count: 1,
        material_packet_count: 3,
        active_track_count: 2,
        completed_track_count: 1,
        failed_track_count: 0,
        cancelled_track_count: 0,
        agent_states: [],
        diagnostics: [
          {
            source: "EpochReason" as const,
            reason_code: "tool_loop",
            summary: "Tool loop detected.",
            target: "agent_diag",
            minimal_fix_surface: ["Reduce repeated retrieval loops"],
            evidence_refs: [],
            failed_clauses: [],
            severity: "warning",
          },
          {
            source: "WorkflowVerifier" as const,
            reason_code: "missing_runtime_state",
            summary: "Goal clause 2 cannot be verified from runtime state.",
            target: "wf_goal_demo@v1",
            minimal_fix_surface: ["Persist runtime state before replan"],
            evidence_refs: ["report_runtime_state"],
            failed_clauses: [2],
            severity: "error",
          },
        ],
      },
      { last_stats: null, insights: [] }
    );

    expect(metrics.map((metric) => metric.title)).toEqual([
      "AI Manager",
      "AI Scientist",
      "AI Orchestrator",
    ]);
    expect(metrics[0].value).toBe("2");
    expect(metrics[1].value).toBe("4/3");
    expect(metrics[1].detail).toBe("0 一致性通过 / 2 待验证");
    expect(metrics[2].value).toBe("4");
    expect(metrics[2].detail).toBe("3 个材料包 / 2 条活跃轨道 / 1 次派生");
    expect(aiOrchestratorWorkloadDetail(null)).toBe("0 个材料包 / 0 条活跃轨道 / 0 次派生");
    expect(aiOrchestratorDiagnosticSummary(metrics[2].diagnostics)).toBe(
      "2 条诊断 / 最新 missing_runtime_state"
    );
    expect(aiOrchestratorFirstFixSurface(metrics[2].diagnostics)).toBe(
      "Persist runtime state before replan"
    );
  });

  it("keeps recent flow narratives compact", () => {
    expect(aiFaceFlowNarrative(flows[1])).toBe(
      "track-task · 编排验证轨道 · Manager: approved/Idle · Scientist: 1 claims, 1 context · 待验证 · focus correctness"
    );
  });

  it("does not present extracted scientist claims as verified knowledge", () => {
    expect(aiScientistVerificationLabel(flows[1].scientist_verification)).toBe("待验证");
    expect(aiFaceFlowNarrative(flows[1])).toContain("待验证");
    expect(aiFaceFlowNarrative(flows[1])).not.toContain("已验证");
    expect(aiScientistVerificationDetail(flows[1].scientist_verification)).toBe(
      "1 条候选证据 / 检索置信度 64%"
    );
    expect(aiScientistKernelBoundaryLabel(flows[1].scientist_verification)).toBe(
      "Kernel 未运行 / 非真理证明"
    );
    expect(aiScientistKernelBoundaryLabel(flows[1].scientist_verification)).not.toContain(
      "结构一致性"
    );
  });

  it("labels scientist kernel outcomes without implying truth proof", () => {
    const noClaims = {
      state: "NoClaims" as const,
      passed: null,
      violation_count: 0,
      overall_confidence: 0,
      confidence_basis: "None" as const,
      evidence_chain_count: 0,
      kernel_name: "PropositionKernel",
      kernel_scope: "no_claims",
      is_truth_proof: false,
    };
    const passed = {
      ...noClaims,
      state: "Passed" as const,
      passed: true,
      overall_confidence: 0.93,
      confidence_basis: "KernelVerification" as const,
      evidence_chain_count: 2,
      kernel_scope: "proposition_graph_consistency",
    };
    const failed = {
      ...passed,
      state: "Failed" as const,
      passed: false,
      violation_count: 2,
    };

    expect(aiScientistVerificationLabel(noClaims)).toBe("无命题");
    expect(aiScientistKernelBoundaryLabel(noClaims)).toBe("无 Kernel 输入");
    expect(aiScientistVerificationLabel(passed)).toBe("一致性通过");
    expect(aiScientistKernelBoundaryLabel(passed)).toBe("结构一致性通过");
    expect(aiScientistVerificationDetail(passed)).toBe("2 条证据链 / Kernel 置信度 93%");
    expect(aiScientistVerificationLabel(failed)).toBe("发现冲突");
    expect(aiScientistKernelBoundaryLabel(failed)).toBe("结构一致性冲突");
    expect(aiScientistVerificationDetail(failed)).toBe("2 个冲突 / Kernel 置信度 93%");
  });

  it("keeps the AI face triad layout responsive and bounded", () => {
    const css = readFileSync(join(process.cwd(), "src/components/AgentDashboard.css"), "utf8");

    expect(css).toContain(".ai-face-flow-card");
    expect(css).toContain(".ai-face-subject-grid");
    expect(css).toContain(".ai-face-flow-row");
  });

  it("turns AI Manager snapshot into control points", () => {
    const snapshot = {
      total_tasks: 6,
      pending_review_count: 2,
      execution_queue_count: 1,
      running_count: 1,
      completed_count: 2,
      failed_count: 0,
      human_task_count: 3,
      memory_expansion_count: 2,
      verification_track_count: 1,
      bridge_required_count: 4,
      read_only_count: 2,
      write_capable_count: 3,
      network_enabled_count: 1,
      scientist_payload_count: 5,
      orchestrator_packet_count: 1,
      latest_control_action: "BridgeReviewPending" as const,
    };

    expect(aiManagerControlLabel(snapshot.latest_control_action)).toBe("等待桥接审查");
    expect(aiManagerControlPoints(snapshot).map((point) => point.label)).toEqual([
      "入口",
      "审批",
      "调度",
      "输出",
    ]);
    expect(aiManagerControlPoints(snapshot)[1].detail).toBe("4 个任务需要 bridge");
  });
});
