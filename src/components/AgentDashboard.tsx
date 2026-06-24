import { useState, useEffect, useCallback } from "react";
import { useAppStore } from "../hooks/useAppStore";
import type {
  AiFaceDataFlow,
  AiFaceMemoryRole,
  AiManagerControlAction,
  AiManagerSnapshot,
  AiScientistVerificationSummary,
} from "../types/ai-face";
import type {
  OrchestratorDiagnostic,
  OrchestratorStatus,
} from "../types/orchestrator";
import FeroHaIcon from "./FeroHaIcon";
import "./AgentDashboard.css";
function hasTauriRuntime() {
  return typeof window !== "undefined" && Boolean(window.__TAURI_INTERNALS__);
}

type OrchestratorDashboardStatus = Partial<OrchestratorStatus> &
  Pick<
    OrchestratorStatus,
    | "epoch_count"
    | "material_packet_count"
    | "active_track_count"
    | "track_event_count"
    | "diagnostics"
  >;

interface TaskItem {
  id: string;
  command?: string;
  task_type?: unknown;
  priority?: string;
  priority_score?: number;
  status?: unknown;
  created_at?: number;
  retry_count?: number;
  intent?: string;
  content?: string;
  subagent_results?: SubagentResult[];
  context_fragments?: ContextFragment[];
}

interface SubagentResult {
  source: string;
  entries: SubagentEntry[];
  hop: number;
  total_found: number;
}

interface SubagentEntry {
  title: string;
  snippet: string;
  url?: string;
  authors?: string[];
  year?: number;
  source: string;
  relevance_score: number;
}

interface ContextFragment {
  id: string;
  key: string;
  value: unknown;
}

export interface DreamStats {
  nrem_connections_strengthened: number;
  nrem_connections_pruned: number;
  rem_bridges_created: number;
  insight_communities_found: number;
  insight_summaries_generated: number;
  total_memories_processed: number;
  duration_ms: number;
}

interface DreamStatus {
  last_stats: DreamStats | null;
  insights: DreamInsight[];
}

interface DreamInsight {
  id: string;
  insight_type: string;
  title: string;
  summary: string;
  related_chunks: string[];
  confidence: number;
  created_at: number;
}

interface TrustScoreInfo {
  score: number;
  mode: string;
  acceptance_rate: number;
  total_interactions: number;
}

export interface AiFaceSubjectMetric {
  title: string;
  subtitle: string;
  icon: string;
  tone: "manager" | "scientist" | "orchestrator";
  value: string;
  detail: string;
  diagnostics: OrchestratorDiagnostic[];
}

export interface AiManagerControlPoint {
  label: string;
  value: string;
  detail: string;
  tone: "intake" | "guard" | "dispatch" | "output";
}

const DATA_SOURCE_COLORS: Record<string, string> = {
  LocalVector: "#2ae09a",
  WebSearch: "#89b4fa",
  Arxiv: "#f9e2af",
  SemanticScholar: "#cba6f7",
};

const STATUS_COLOR_MAP: Record<string, string> = {
  Pending: "var(--status-pending-color, #f9e2af)",
  Approved: "var(--status-running-color, #89b4fa)",
  Running: "var(--status-running-color, #a6e3a1)",
  Done: "var(--status-done-color, #a6e3a1)",
  Error: "var(--status-error-color, #f38ba8)",
  Cancelled: "var(--status-idle-color, #6c7086)",
};

const STATUS_LABEL_MAP: Record<string, string> = {
  Pending: "待处理",
  Approved: "已批准",
  Running: "运行中",
  Done: "已完成",
  Error: "错误",
  Cancelled: "已取消",
};

export interface DreamMetric {
  label: string;
  value: number;
}

export interface DreamStageCard {
  title: string;
  subtitle: string;
  icon: string;
  tone: "nrem" | "rem" | "insight";
  metrics: DreamMetric[];
}

export function formatDreamDuration(durationMs: number): string {
  if (!Number.isFinite(durationMs) || durationMs <= 0) return "0.0s";
  if (durationMs < 60_000) return `${(durationMs / 1000).toFixed(1)}s`;
  const minutes = Math.floor(durationMs / 60_000);
  const seconds = Math.round((durationMs % 60_000) / 1000);
  return `${minutes}m ${seconds}s`;
}

export function dreamStageCardsFromStats(stats: DreamStats): DreamStageCard[] {
  return [
    {
      title: "NREM 整合",
      subtitle: "稳定高价值连接",
      icon: "GitMerge",
      tone: "nrem",
      metrics: [
        { label: "强化连接", value: stats.nrem_connections_strengthened },
        { label: "剪枝连接", value: stats.nrem_connections_pruned },
      ],
    },
    {
      title: "REM 桥接",
      subtitle: "生成跨区域关系",
      icon: "Combine",
      tone: "rem",
      metrics: [{ label: "桥接连接", value: stats.rem_bridges_created }],
    },
    {
      title: "洞察提炼",
      subtitle: "聚合记忆社区",
      icon: "Brain",
      tone: "insight",
      metrics: [{ label: "记忆社区", value: stats.insight_communities_found }],
    },
  ];
}

export function dreamRunSummaryItems(stats: DreamStats): string[] {
  return [
    `处理 ${stats.total_memories_processed} 个记忆块`,
    `生成 ${stats.insight_summaries_generated} 条洞察摘要`,
    `耗时 ${formatDreamDuration(stats.duration_ms)}`,
  ];
}

function displayStatus(status: string): string {
  return STATUS_LABEL_MAP[status] ?? status;
}

export function normalizeTaskStatus(status: unknown): string {
  if (typeof status === "string") return status;
  if (status && typeof status === "object") {
    const keys = Object.keys(status as Record<string, unknown>);
    if (keys.length > 0) return keys[0];
  }
  return "";
}

export function normalizeTaskType(taskType: unknown): string {
  if (typeof taskType === "string" && taskType.trim()) return taskType;
  if (taskType && typeof taskType === "object") {
    const entries = Object.entries(taskType as Record<string, unknown>);
    if (entries.length > 0) {
      const [kind, payload] = entries[0];
      if (typeof payload === "string" && payload.trim()) return payload;
      return kind;
    }
  }
  return "任务";
}

export function aiFaceRoleLabel(role: AiFaceMemoryRole): string {
  switch (role) {
    case "AiMemoryExpansion":
      return "AI 记忆拓展";
    case "OrchestratorVerification":
      return "编排验证轨道";
    case "HumanTask":
    default:
      return "人工任务";
  }
}

export function aiScientistVerificationLabel(summary: AiScientistVerificationSummary): string {
  switch (summary.state) {
    case "Passed":
      return "一致性通过";
    case "Failed":
      return "发现冲突";
    case "NoClaims":
      return "无命题";
    case "NotRun":
    default:
      return "待验证";
  }
}

export function aiScientistConfidenceLabel(summary: AiScientistVerificationSummary): string {
  switch (summary.confidence_basis) {
    case "KernelVerification":
      return "Kernel 置信度";
    case "EvidenceFallback":
      return "检索置信度";
    case "None":
    default:
      return "无置信度";
  }
}

export function aiScientistKernelBoundaryLabel(summary: AiScientistVerificationSummary): string {
  if (summary.is_truth_proof) {
    return "真理证明";
  }
  switch (summary.state) {
    case "Passed":
      return "结构一致性通过";
    case "Failed":
      return "结构一致性冲突";
    case "NoClaims":
      return "无 Kernel 输入";
    case "NotRun":
    default:
      return "Kernel 未运行 / 非真理证明";
  }
}

export function aiScientistVerificationTone(summary: AiScientistVerificationSummary): string {
  switch (summary.state) {
    case "Passed":
      return "passed";
    case "Failed":
      return "failed";
    case "NoClaims":
      return "empty";
    case "NotRun":
    default:
      return "pending";
  }
}

export function aiScientistVerificationDetail(summary: AiScientistVerificationSummary): string {
  const confidence = `${Math.round(summary.overall_confidence * 100)}%`;
  const confidenceLabel = aiScientistConfidenceLabel(summary);
  if (summary.state === "Failed") {
    return `${summary.violation_count} 个冲突 / ${confidenceLabel} ${confidence}`;
  }
  if (summary.state === "Passed") {
    return `${summary.evidence_chain_count} 条证据链 / ${confidenceLabel} ${confidence}`;
  }
  if (summary.state === "NoClaims") {
    return "等待命题提炼";
  }
  return `${summary.evidence_chain_count} 条候选证据 / ${confidenceLabel} ${confidence}`;
}

export function aiOrchestratorWorkloadDetail(status: OrchestratorDashboardStatus | null): string {
  return `${status?.material_packet_count ?? 0} 个材料包 / ${status?.active_track_count ?? 0} 条活跃轨道 / ${status?.track_event_count ?? 0} 次派生`;
}

export function aiOrchestratorDiagnosticSummary(
  diagnostics: OrchestratorDiagnostic[] | null | undefined
): string {
  if (!diagnostics || diagnostics.length === 0) return "0 条诊断";
  const latest = diagnostics[diagnostics.length - 1];
  return `${diagnostics.length} 条诊断 / 最新 ${latest.reason_code}`;
}

export function aiOrchestratorFirstFixSurface(
  diagnostics: OrchestratorDiagnostic[] | null | undefined
): string {
  if (!diagnostics || diagnostics.length === 0) return "暂无最小修复面";
  const latest = diagnostics[diagnostics.length - 1];
  return latest.minimal_fix_surface[0] ?? latest.summary;
}

export function aiFaceSubjectMetrics(
  flows: AiFaceDataFlow[],
  orchestratorStatus: OrchestratorDashboardStatus | null,
  dreamStatus: DreamStatus | null,
  managerSnapshot: AiManagerSnapshot | null = null
): AiFaceSubjectMetric[] {
  const totalClaims = flows.reduce((sum, flow) => sum + flow.scientist_claim_count, 0);
  const totalSources = flows.reduce((sum, flow) => sum + flow.scientist_source_count, 0);
  const tracedTasks = flows.filter((flow) => flow.manager_has_trace).length;
  const dreamProcessed = dreamStatus?.last_stats?.total_memories_processed ?? 0;
  const verifiedScientistFlows = flows.filter(
    (flow) => flow.scientist_verification.state === "Passed"
  ).length;
  const pendingScientistFlows = flows.filter(
    (flow) => flow.scientist_verification.state === "NotRun"
  ).length;
  const failedScientistFlows = flows.filter(
    (flow) => flow.scientist_verification.state === "Failed"
  ).length;
  const scientistVerificationDetail =
    failedScientistFlows > 0
      ? `${verifiedScientistFlows} 一致性通过 / ${pendingScientistFlows} 待验证 / ${failedScientistFlows} 冲突`
      : `${verifiedScientistFlows} 一致性通过 / ${pendingScientistFlows} 待验证`;

  return [
    {
      title: "AI Manager",
      subtitle: "任务编排与材料收集",
      icon: "Bot",
      tone: "manager",
      value: String(managerSnapshot?.total_tasks ?? flows.length),
      detail: managerSnapshot
        ? `${managerSnapshot.pending_review_count} 待审 / ${managerSnapshot.execution_queue_count} 待调度`
        : `${tracedTasks} 条 trace / ${dreamProcessed} 个 Dream 记忆块`,
      diagnostics: [],
    },
    {
      title: "AI Scientist",
      subtitle: "命题提炼与证据链",
      icon: "Brain",
      tone: "scientist",
      value: `${totalClaims}/${totalSources}`,
      detail: scientistVerificationDetail,
      diagnostics: [],
    },
    {
      title: "AI Orchestrator",
      subtitle: "全局调控与退行轨道",
      icon: "Activity",
      tone: "orchestrator",
      value: String(orchestratorStatus?.epoch_count ?? 0),
      detail: aiOrchestratorWorkloadDetail(orchestratorStatus),
      diagnostics: orchestratorStatus?.diagnostics ?? [],
    },
  ];
}

export function aiFaceFlowNarrative(flow: AiFaceDataFlow): string {
  const focus = flow.material_packet_focus ? ` · focus ${flow.material_packet_focus}` : "";
  const verification = aiScientistVerificationLabel(flow.scientist_verification);
  return `${flow.task_id} · ${aiFaceRoleLabel(flow.memory_role)} · Manager: ${flow.manager_status}/${flow.manager_phase} · Scientist: ${flow.scientist_claim_count} claims, ${flow.scientist_source_count} context · ${verification}${focus}`;
}

export function aiManagerControlLabel(action: AiManagerControlAction): string {
  switch (action) {
    case "OrchestratorTrackPending":
      return "编排验证待派发";
    case "BridgeReviewPending":
      return "等待桥接审查";
    case "RunningTasks":
      return "任务运行中";
    case "DispatchReady":
      return "队列待调度";
    case "Idle":
    default:
      return "空闲";
  }
}

export function aiManagerControlPoints(snapshot: AiManagerSnapshot | null): AiManagerControlPoint[] {
  if (!snapshot) {
    return [
      { label: "入口", value: "0", detail: "等待任务输入", tone: "intake" },
      { label: "审批", value: "0", detail: "无 bridge 审查", tone: "guard" },
      { label: "调度", value: "0", detail: "队列空闲", tone: "dispatch" },
      { label: "输出", value: "0", detail: "暂无记忆材料", tone: "output" },
    ];
  }

  return [
    {
      label: "入口",
      value: String(snapshot.total_tasks),
      detail: `${snapshot.human_task_count} 人工 / ${snapshot.memory_expansion_count} AI 拓展 / ${snapshot.verification_track_count} 验证`,
      tone: "intake",
    },
    {
      label: "审批",
      value: String(snapshot.pending_review_count),
      detail: `${snapshot.bridge_required_count} 个任务需要 bridge`,
      tone: "guard",
    },
    {
      label: "调度",
      value: String(snapshot.execution_queue_count + snapshot.running_count),
      detail: `${snapshot.execution_queue_count} 待派发 / ${snapshot.running_count} 运行中`,
      tone: "dispatch",
    },
    {
      label: "输出",
      value: String(snapshot.scientist_payload_count),
      detail: `${snapshot.orchestrator_packet_count} 个 orchestrator packet`,
      tone: "output",
    },
  ];
}

export default function AgentDashboard() {
  const [isTauri] = useState(hasTauriRuntime);
  const updateTask = useAppStore((s) => s.updateTask);
  const setGraph = useAppStore((s) => s.setGraph);
  const clearCompletedTasks = useAppStore((s) => s.clearCompletedTasks);
  const agentTools = useAppStore((s) => s.agentTools);
  const fetchAgentTools = useAppStore((s) => s.fetchAgentTools);

  const [orchestratorStatus, setOrchestratorStatus] = useState<OrchestratorStatus | null>(null);
  const [dreamStatus, setDreamStatus] = useState<DreamStatus | null>(null);
  const [trustScore, setTrustScore] = useState<TrustScoreInfo | null>(null);
  const [backendTasks, setBackendTasks] = useState<TaskItem[]>([]);
  const [aiFaceFlows, setAiFaceFlows] = useState<AiFaceDataFlow[]>([]);
  const [managerSnapshot, setManagerSnapshot] = useState<AiManagerSnapshot | null>(null);
  const [expandedSections, setExpandedSections] = useState<Record<string, boolean>>({
    Pending: true,
    Approved: true,
    Running: true,
    Done: false,
    Error: true,
    Insights: true,
  });
  const [expandedTasks, setExpandedTasks] = useState<Set<string>>(new Set());
  const [dreamLoading, setDreamLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const invoke = useCallback(async (cmd: string, args?: Record<string, unknown>) => {
    if (!isTauri) throw new Error("Not in Tauri");
    const { invoke: tauriInvoke } = await import("@tauri-apps/api/core");
    return tauriInvoke(cmd, args);
  }, [isTauri]);

  const fetchOrchestrator = useCallback(async () => {
    if (!isTauri) return;
    try {
      const status = await invoke("orchestrator_status") as OrchestratorStatus;
      setOrchestratorStatus(status);
    } catch {
      // best-effort
    }
  }, [isTauri, invoke]);

  const fetchTasks = useCallback(async () => {
    if (!isTauri) return;
    try {
      const all = await invoke("list_tasks") as TaskItem[];
      setBackendTasks(all || []);
    } catch {
      setError("任务加载失败");
    }
  }, [isTauri, invoke]);

  const fetchDreamStatus = useCallback(async () => {
    if (!isTauri) return;
    try {
      const status = await invoke("get_dream_status") as DreamStatus;
      setDreamStatus(status);
    } catch {
      // best-effort
    }
  }, [isTauri, invoke]);

  const fetchTrustScore = useCallback(async () => {
    if (!isTauri) return;
    try {
      const info = await invoke("get_trust_score_info") as TrustScoreInfo;
      setTrustScore(info);
    } catch {
      // best-effort
    }
  }, [isTauri, invoke]);

  const fetchAiFaceFlows = useCallback(async () => {
    if (!isTauri) return;
    try {
      const flows = await invoke("list_ai_face_data_flows") as AiFaceDataFlow[];
      setAiFaceFlows(flows || []);
    } catch {
      // best-effort
    }
  }, [isTauri, invoke]);

  const fetchManagerSnapshot = useCallback(async () => {
    if (!isTauri) return;
    try {
      const snapshot = await invoke("get_ai_manager_snapshot") as AiManagerSnapshot;
      setManagerSnapshot(snapshot);
    } catch {
      // best-effort
    }
  }, [isTauri, invoke]);

  const refreshAll = useCallback(async () => {
    setError(null);
    await Promise.all([
      fetchOrchestrator(),
      fetchTasks(),
      fetchDreamStatus(),
      fetchTrustScore(),
      fetchAiFaceFlows(),
      fetchManagerSnapshot(),
      fetchAgentTools(),
    ]);
  }, [fetchOrchestrator, fetchTasks, fetchDreamStatus, fetchTrustScore, fetchAiFaceFlows, fetchManagerSnapshot, fetchAgentTools]);

  /* eslint-disable react-hooks/set-state-in-effect */
  useEffect(() => {
    fetchOrchestrator();
    fetchTasks();
    fetchDreamStatus();
    fetchTrustScore();
    fetchAiFaceFlows();
    fetchManagerSnapshot();
    fetchAgentTools();
    const interval = setInterval(() => {
      fetchOrchestrator();
      fetchTasks();
      fetchAiFaceFlows();
      fetchManagerSnapshot();
    }, 5000);
    return () => clearInterval(interval);
  }, [fetchOrchestrator, fetchTasks, fetchDreamStatus, fetchTrustScore, fetchAiFaceFlows, fetchManagerSnapshot, fetchAgentTools]);
  /* eslint-enable react-hooks/set-state-in-effect */

  useEffect(() => {
    if (!isTauri) return;
    let unlisten: (() => void) | undefined;
    const setup = async () => {
      const { listen: tauriListen } = await import("@tauri-apps/api/event");
      unlisten = await tauriListen<{ task_id: string; status: string; result?: string }>(
        "task-updated",
        (event) => {
          const { task_id, status, result } = event.payload;
          updateTask(task_id, {
            id: task_id,
            status: status as "pending" | "approved" | "running" | "done" | "error" | "cancelled",
            result,
          });
          fetchTasks();
          fetchAiFaceFlows();
          fetchManagerSnapshot();
        }
      );
    };
    setup();
    return () => { unlisten?.(); };
  }, [isTauri, updateTask, fetchTasks, fetchAiFaceFlows, fetchManagerSnapshot]);

  const approveTask = async (taskId: string) => {
    if (!isTauri) return;
    try {
      await invoke("approve_task", { taskId });
      fetchTasks();
      fetchAiFaceFlows();
      fetchManagerSnapshot();
    } catch (e) {
      console.error("Approve failed:", e);
    }
  };

  const cancelTask = async (taskId: string) => {
    if (!isTauri) {
      updateTask(taskId, { status: "cancelled" });
      return;
    }
    try {
      await invoke("cancel_task", { taskId });
      fetchTasks();
      fetchAiFaceFlows();
      fetchManagerSnapshot();
    } catch (e) {
      console.error("Cancel failed:", e);
    }
  };

  const retryTask = async (taskId: string) => {
    if (!isTauri) return;
    try {
      await invoke("cancel_task", { taskId });
      await invoke("approve_task", { taskId });
      fetchTasks();
      fetchAiFaceFlows();
      fetchManagerSnapshot();
    } catch (e) {
      console.error("Retry failed:", e);
    }
  };

  const triggerDream = async () => {
    setDreamLoading(true);
    try {
      await invoke("trigger_dream");
      setTimeout(() => {
        fetchDreamStatus();
        fetchAiFaceFlows();
        fetchManagerSnapshot();
        invoke("get_graph")
          .then((graph) => setGraph(graph as Parameters<typeof setGraph>[0]))
          .catch(() => undefined);
      }, 1000);
    } catch (e) {
      console.error("Dream failed:", e);
    } finally {
      setDreamLoading(false);
    }
  };

  const toggleSection = (section: string) => {
    setExpandedSections((prev) => ({ ...prev, [section]: !prev[section] }));
  };

  const toggleTaskExpand = (taskId: string) => {
    setExpandedTasks((prev) => {
      const next = new Set(prev);
      if (next.has(taskId)) next.delete(taskId);
      else next.add(taskId);
      return next;
    });
  };

  const statusGroups = {
    Pending: backendTasks.filter((t) => {
      const s = normalizeTaskStatus(t.status);
      return s === "Pending";
    }),
    Approved: backendTasks.filter((t) => {
      const s = normalizeTaskStatus(t.status);
      if (!s) return false;
      return s.startsWith("Approved") || s === "Queued";
    }),
    Running: backendTasks.filter((t) => {
      const s = normalizeTaskStatus(t.status);
      if (!s) return false;
      return s.startsWith("Running");
    }),
    Done: backendTasks.filter((t) => {
      const s = normalizeTaskStatus(t.status);
      if (!s) return false;
      return s.startsWith("Done");
    }),
    Error: backendTasks.filter((t) => {
      const s = normalizeTaskStatus(t.status);
      if (!s) return false;
      return s.startsWith("Error");
    }),
  };

  const orchestratorDegradedAgents = orchestratorStatus?.degraded_agents ?? [];
  const orchestratorDiagnostics = orchestratorStatus?.diagnostics ?? [];
  const orchestratorHealthy = !orchestratorStatus
    ? "gray"
    : orchestratorDegradedAgents.length === 0
      ? "green"
      : orchestratorStatus.active_agents > 0
        ? "orange"
        : "red";

  const trustColor =
    !trustScore
      ? "var(--text-muted)"
      : trustScore.score < 0.4
        ? "var(--trust-low-color, #ef4444)"
        : trustScore.score < 0.7
          ? "var(--trust-mid-color, #f59e0b)"
          : "var(--trust-high-color, #22c55e)";

  const trustPercent = trustScore ? Math.round(trustScore.score * 100) : 0;
  const dreamStats = dreamStatus?.last_stats ?? null;
  const dreamInsights = dreamStatus?.insights ?? [];
  const dreamStageCards = dreamStats ? dreamStageCardsFromStats(dreamStats) : [];
  const dreamRunSummary = dreamStats ? dreamRunSummaryItems(dreamStats) : [];
  const aiFaceSubjectCards = aiFaceSubjectMetrics(aiFaceFlows, orchestratorStatus, dreamStatus, managerSnapshot);
  const managerControlPoints = aiManagerControlPoints(managerSnapshot);
  const managerActionText = managerSnapshot ? aiManagerControlLabel(managerSnapshot.latest_control_action) : "等待后端";
  const recentAiFaceFlows = aiFaceFlows.slice(-4).reverse();

  return (
    <div className="agent-dashboard">
      <div className="dashboard-header">
        <h3 className="dashboard-title">
          <FeroHaIcon name="Bot" size={18} />
          Agent 面板
        </h3>
        <div className="dashboard-header-actions">
          <button className="dashboard-btn" onClick={refreshAll} title="刷新全部数据">
            <FeroHaIcon name="RefreshCw" size={14} />
            刷新
          </button>
          <button className="dashboard-btn" onClick={clearCompletedTasks} title="清除已完成和失败任务">
            <FeroHaIcon name="Trash2" size={14} />
            清除
          </button>
        </div>
      </div>

      {error && <div className="dashboard-error">{error}</div>}

      <div className="dashboard-grid">
        {/* Orchestrator Status Card */}
        <div className="card orchestrator-card">
          <div className="card-header">
            <FeroHaIcon name="Activity" size={16} />
            <h4 className="card-title">编排器</h4>
            <span className={`status-indicator status-${orchestratorHealthy}`} />
          </div>
          {orchestratorStatus ? (
            <div className="card-body">
              <div className="stat-row">
                <span className="stat-label">活跃 Agent</span>
                <span className="stat-value">{orchestratorStatus.active_agents}</span>
              </div>
              <div className="stat-row">
                <span className="stat-label">退行</span>
                <span className="stat-value" style={{ color: orchestratorDegradedAgents.length > 0 ? "var(--status-error-color)" : "inherit" }}>
                  {orchestratorDegradedAgents.length}
                </span>
              </div>
              <div className="stat-row">
                <span className="stat-label">Epoch</span>
                <span className="stat-value">{orchestratorStatus.epoch_count}</span>
              </div>
              <div className="stat-row">
                <span className="stat-label">活跃轨道</span>
                <span className="stat-value">{orchestratorStatus.active_track_count}</span>
              </div>
              <div className="stat-row">
                <span className="stat-label">材料包</span>
                <span className="stat-value">{orchestratorStatus.material_packet_count}</span>
              </div>
              <div className="stat-row">
                <span className="stat-label">派生事件</span>
                <span className="stat-value">{orchestratorStatus.track_event_count}</span>
              </div>
              <div className="stat-row">
                <span className="stat-label">Workflow 事件</span>
                <span className="stat-value">{orchestratorStatus.workflow_event_count ?? 0}</span>
              </div>
              <div className="stat-row">
                <span className="stat-label">Replan</span>
                <span className="stat-value">{orchestratorStatus.workflow_replan_request_count ?? 0}</span>
              </div>
              {orchestratorDegradedAgents.length > 0 && (
                <div className="degraded-list">
                  <span className="degraded-label">退行 Agent：</span>
                  {orchestratorDegradedAgents.map((id) => (
                    <span key={id} className="degraded-agent-tag">{id.slice(0, 12)}</span>
                  ))}
                </div>
              )}
              {orchestratorDiagnostics.length > 0 && (
                <div className="orchestrator-diagnostic-strip">
                  <span className="orchestrator-diagnostic-label">
                    {aiOrchestratorDiagnosticSummary(orchestratorDiagnostics)}
                  </span>
                  <span className="orchestrator-diagnostic-fix">
                    {aiOrchestratorFirstFixSurface(orchestratorDiagnostics)}
                  </span>
                </div>
              )}
            </div>
          ) : (
            <div className="card-muted">
              {isTauri ? "正在加载..." : "Tauri 后端不可用"}
            </div>
          )}
        </div>

        {/* Trust Score Gauge */}
        <div className="card trust-card">
          <div className="card-header">
            <FeroHaIcon name="Shield" size={16} />
            <h4 className="card-title">信任评分</h4>
          </div>
          <div className="card-body trust-body">
            <div className="trust-gauge-container">
              <svg className="trust-gauge" viewBox="0 0 120 120">
                <circle
                  cx="60" cy="60" r="50"
                  fill="none"
                  stroke="var(--bg-input)"
                  strokeWidth="8"
                />
                <circle
                  cx="60" cy="60" r="50"
                  fill="none"
                  stroke={trustColor}
                  strokeWidth="8"
                  strokeDasharray={`${trustPercent * 3.14} 314`}
                  strokeLinecap="round"
                  transform="rotate(-90 60 60)"
                  style={{ transition: "stroke-dasharray 0.5s ease" }}
                />
                <text x="60" y="55" textAnchor="middle" fill={trustColor} fontSize="22" fontWeight="bold">
                  {trustPercent}%
                </text>
                <text x="60" y="72" textAnchor="middle" fill="var(--text-muted)" fontSize="11">
                  {trustScore?.mode ?? "n/a"}
                </text>
              </svg>
            </div>
            {trustScore && (
              <div className="trust-details">
                <div className="stat-row">
                  <span className="stat-label">接受率</span>
                  <span className="stat-value">{(trustScore.acceptance_rate * 100).toFixed(0)}%</span>
                </div>
                <div className="stat-row">
                  <span className="stat-label">交互数</span>
                  <span className="stat-value">{trustScore.total_interactions}</span>
                </div>
              </div>
            )}
          </div>
        </div>

        {/* AI Face Triad */}
        <div className="card ai-face-flow-card">
          <div className="card-header ai-face-flow-header">
            <div className="ai-face-flow-title">
              <FeroHaIcon name="Workflow" size={16} />
              <div>
                <h4 className="card-title">AI 面三主体</h4>
                <span className="ai-face-flow-subtitle">Manager / Scientist / Orchestrator 数据流</span>
              </div>
            </div>
            <span className="ai-face-flow-badge">{aiFaceFlows.length} 条流</span>
          </div>
          <div className="card-body ai-face-flow-body">
            <div className="ai-face-subject-grid">
              {aiFaceSubjectCards.map((subject) => (
                <div key={subject.title} className={`ai-face-subject ai-face-subject-${subject.tone}`}>
                  <div className="ai-face-subject-heading">
                    <span className="ai-face-subject-icon">
                      <FeroHaIcon name={subject.icon} size={15} />
                    </span>
                    <div>
                      <div className="ai-face-subject-title">{subject.title}</div>
                      <div className="ai-face-subject-subtitle">{subject.subtitle}</div>
                    </div>
                  </div>
                  <div className="ai-face-subject-value">{subject.value}</div>
                  <div className="ai-face-subject-detail">{subject.detail}</div>
                </div>
              ))}
            </div>

            <div className="ai-manager-control-panel">
              <div className="ai-manager-control-header">
                <span>Manager 控制面</span>
                <strong>{managerActionText}</strong>
              </div>
              <div className="ai-manager-control-grid">
                {managerControlPoints.map((point) => (
                  <div key={point.label} className={`ai-manager-control-point control-${point.tone}`}>
                    <span>{point.label}</span>
                    <strong>{point.value}</strong>
                    <small>{point.detail}</small>
                  </div>
                ))}
              </div>
              {managerSnapshot && (
                <div className="ai-manager-sandbox-row">
                  <span>只读 {managerSnapshot.read_only_count}</span>
                  <span>可写 {managerSnapshot.write_capable_count}</span>
                  <span>网络 {managerSnapshot.network_enabled_count}</span>
                  <span>失败 {managerSnapshot.failed_count}</span>
                </div>
              )}
            </div>

            <div className="ai-face-flow-list">
              <span className="stat-group-label">最近数据流</span>
              {recentAiFaceFlows.length === 0 ? (
                <div className="ai-face-flow-empty">
                  {isTauri ? "暂无 AI 面任务流。" : "Tauri 后端不可用"}
                </div>
              ) : (
                recentAiFaceFlows.map((flow) => (
                  <div key={flow.task_id} className="ai-face-flow-row" title={aiFaceFlowNarrative(flow)}>
                    <div className="ai-face-flow-row-main">
                      <span className={`ai-face-role-pill role-${flow.memory_role.toLowerCase()}`}>
                        {aiFaceRoleLabel(flow.memory_role)}
                      </span>
                      <span className="ai-face-flow-task-id">{flow.task_id}</span>
                    </div>
                    <div className="ai-face-flow-row-state">
                      <span>管控 {flow.manager_status}/{flow.manager_phase}</span>
                      <span>科学家 {flow.scientist_claim_count} 命题 / {flow.scientist_source_count} 来源</span>
                      <span className={`ai-face-verification-pill verification-${aiScientistVerificationTone(flow.scientist_verification)}`}>
                        {aiScientistVerificationLabel(flow.scientist_verification)}
                      </span>
                    </div>
                    <div className="ai-face-flow-row-meta">
                      <span>上下文 {flow.context_fragment_count}</span>
                      <span>子代理 {flow.subagent_result_count}</span>
                      <span>{aiScientistVerificationDetail(flow.scientist_verification)}</span>
                      <span>Kernel {flow.scientist_verification.kernel_scope}</span>
                      <span>{aiScientistKernelBoundaryLabel(flow.scientist_verification)}</span>
                      {flow.material_packet_focus && <span>焦点 {flow.material_packet_focus}</span>}
                      {flow.sandbox_summary && <span>{flow.sandbox_summary}</span>}
                    </div>
                  </div>
                ))
              )}
            </div>
          </div>
        </div>

        {/* Dream Panel */}
        <div className="card dream-card">
          <div className="card-header dream-header">
            <div className="dream-header-title">
              <FeroHaIcon name="Moon" size={16} />
              <div>
                <h4 className="card-title">Dream 引擎</h4>
                <span className="dream-header-subtitle">AI 记忆循环</span>
              </div>
            </div>
            <button
              className={`dashboard-btn dashboard-btn-accent ${dreamLoading ? "loading" : ""}`}
              onClick={triggerDream}
              disabled={dreamLoading}
              title="触发 Dream 循环"
            >
              <FeroHaIcon name={dreamLoading ? "Loader" : "Sparkles"} size={14} />
              {dreamLoading ? "Dream 中..." : "Dream"}
            </button>
          </div>
          {dreamStats ? (
            <div className="card-body dream-body">
              <div className="dream-control-strip">
                <div className="dream-control-main">
                  <span className="dream-control-label">最近一次循环</span>
                  <strong>Dream 已完成</strong>
                </div>
                <div className="dream-run-summary">
                  {dreamRunSummary.map((item) => (
                    <span key={item}>{item}</span>
                  ))}
                </div>
              </div>

              <div className="dream-stage-grid">
                {dreamStageCards.map((stage) => (
                  <div key={stage.title} className={`dream-stage-tile dream-stage-${stage.tone}`}>
                    <div className="dream-stage-heading">
                      <span className="dream-stage-icon">
                        <FeroHaIcon name={stage.icon} size={15} />
                      </span>
                      <div>
                        <div className="dream-stage-title">{stage.title}</div>
                        <div className="dream-stage-subtitle">{stage.subtitle}</div>
                      </div>
                    </div>
                    <div className="dream-stage-metrics">
                      {stage.metrics.map((metric) => (
                        <div key={metric.label} className="dream-stage-metric">
                          <span>{metric.label}</span>
                          <strong>{metric.value}</strong>
                        </div>
                      ))}
                    </div>
                  </div>
                ))}
              </div>

              <div className="insight-list">
                <span className="stat-group-label">近期洞察 ({dreamInsights.length})</span>
                {dreamInsights.length === 0 ? (
                  <div className="dream-empty-insight">本轮尚未形成洞察。</div>
                ) : (
                  dreamInsights.slice(0, 5).map((insight) => (
                    <div key={insight.id} className="insight-item">
                      <div className="insight-header">
                        <span className="insight-title">{insight.title}</span>
                        <span className="insight-confidence">
                          {Math.round(insight.confidence * 100)}%
                        </span>
                      </div>
                      <div className="insight-summary">
                        {insight.summary.slice(0, 120)}{insight.summary.length > 120 ? "..." : ""}
                      </div>
                    </div>
                  ))
                )}
                </div>
            </div>
          ) : (
            <div className="card-muted">
              {dreamStatus && !dreamStatus.last_stats
                ? "尚未运行 Dream 循环。"
                : isTauri
                  ? "正在加载..."
                  : "Tauri 后端不可用"}
            </div>
          )}
        </div>

        {/* Agent Tools */}
        {agentTools && agentTools.length > 0 && (
          <div className="card tools-card">
            <div className="card-header">
              <FeroHaIcon name="Wrench" size={16} />
              <h4 className="card-title">Agent 工具 ({agentTools.length})</h4>
            </div>
            <div className="card-body">
              <div className="tool-grid">
                {agentTools.map((tool) => (
                  <span key={tool.name} className="tool-badge" title={tool.description}>
                    {tool.name}
                  </span>
                ))}
              </div>
            </div>
          </div>
        )}
      </div>

      {/* Memory Insights */}
      {dreamStatus?.insights && dreamStatus.insights.length > 0 && (
        <div className="memory-insights-section">
          <button
            className="task-section-toggle"
            onClick={() => toggleSection("Insights")}
          >
            <FeroHaIcon
              name={expandedSections["Insights"] ? "ChevronDown" : "ChevronRight"}
              size={12}
            />
            <span className="task-section-status" style={{ color: "var(--highlight-color, #a78bfa)" }}>
              <FeroHaIcon name="Brain" size={12} />
              记忆洞察
            </span>
            <span className="task-section-count">{dreamStatus.insights.length}</span>
          </button>
          {expandedSections["Insights"] && (
            <div className="task-section-items">
              {dreamStatus.insights.map((insight) => (
                <div key={insight.id} className="insight-detail-card">
                  <div className="insight-detail-header">
                    <span className="insight-detail-type">
                      {insight.insight_type}
                    </span>
                    <span className="insight-detail-title">{insight.title}</span>
                    <span className="insight-detail-confidence">
                      置信度：{(insight.confidence * 100).toFixed(0)}%
                    </span>
                  </div>
                  <div className="insight-detail-summary">{insight.summary}</div>
                  {insight.related_chunks && insight.related_chunks.length > 0 && (
                    <div className="insight-detail-related">
                      <span className="insight-detail-label">相关记忆块：</span>
                      {insight.related_chunks.slice(0, 5).join(", ")}
                      {insight.related_chunks.length > 5 && ` +${insight.related_chunks.length - 5} 个`}
                    </div>
                  )}
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      {/* Task Queue */}
      <div className="task-queue-section">
        <div className="task-queue-header">
          <h4 className="section-title">
            <FeroHaIcon name="ListTodo" size={16} />
            任务队列
          </h4>
          <span className="task-count-badge">共 {backendTasks.length} 个</span>
        </div>

        {backendTasks.length === 0 ? (
          <div className="card-muted" style={{ padding: "24px", textAlign: "center" }}>
            <FeroHaIcon name="Coffee" size={24} />
            <div style={{ marginTop: 8, fontSize: 13 }}>暂无活动任务</div>
            <div style={{ fontSize: 11, marginTop: 4 }}>在底部指令栏使用 /agent 启动任务</div>
          </div>
        ) : (
          <div className="task-sections">
            {(["Pending", "Approved", "Running", "Done", "Error"] as const).map((status) => {
              const items = statusGroups[status];
              return (
                <div key={status} className="task-section">
                  <button
                    className="task-section-toggle"
                    onClick={() => toggleSection(status)}
                  >
                    <FeroHaIcon
                      name={expandedSections[status] ? "ChevronDown" : "ChevronRight"}
                      size={12}
                    />
                    <span className="task-section-status" style={{ color: STATUS_COLOR_MAP[status] }}>
                      {status === "Approved" ? <FeroHaIcon name="CheckCircle" size={12} /> :
                       status === "Running" ? <FeroHaIcon name="Loader" size={12} className="animate-spin" /> :
                       status === "Done" ? <FeroHaIcon name="Check" size={12} /> :
                       status === "Error" ? <FeroHaIcon name="AlertCircle" size={12} /> :
                       <FeroHaIcon name="Clock" size={12} />}
                      {displayStatus(status)}
                    </span>
                    <span className="task-section-count">{items.length}</span>
                  </button>
                  {expandedSections[status] && (
                    <div className="task-section-items">
                      {items.map((task) => (
                        <div key={task.id} className={`task-item task-item-${status.toLowerCase()}`}>
                          <div
                            className="task-item-header"
                            onClick={() => toggleTaskExpand(task.id)}
                          >
                            <span className="task-dot" style={{ backgroundColor: STATUS_COLOR_MAP[status] }} />
                            <span className="task-id">{task.id.slice(0, 16)}</span>
                            <span className="task-type">{normalizeTaskType(task.task_type)}</span>
                            {task.created_at && (
                              <span className="task-time">
                                {new Date(task.created_at).toLocaleTimeString()}
                              </span>
                            )}
                            {task.priority_score !== undefined && (
                              <span className="task-priority">P{task.priority_score}</span>
                            )}
                            <FeroHaIcon
                              name={expandedTasks.has(task.id) ? "ChevronUp" : "ChevronDown"}
                              size={12}
                            />
                            <div className="task-item-actions" onClick={(e) => e.stopPropagation()}>
                              {status === "Pending" && (
                                <>
                                  <button
                                    className="task-action-btn approve"
                                    onClick={() => approveTask(task.id)}
                                    title="批准"
                                  >
                                    <FeroHaIcon name="Check" size={10} />
                                  </button>
                                  <button
                                    className="task-action-btn cancel"
                                    onClick={() => cancelTask(task.id)}
                                    title="取消"
                                  >
                                    <FeroHaIcon name="X" size={10} />
                                  </button>
                                </>
                              )}
                              {status === "Approved" && (
                                <button
                                  className="task-action-btn cancel"
                                  onClick={() => cancelTask(task.id)}
                                  title="取消"
                                >
                                  <FeroHaIcon name="X" size={10} />
                                </button>
                              )}
                              {status === "Error" && (
                                <button
                                  className="task-action-btn retry"
                                  onClick={() => retryTask(task.id)}
                                  title="重试"
                                >
                                  <FeroHaIcon name="RotateCw" size={10} />
                                </button>
                              )}
                            </div>
                          </div>

                          {/* Task detail expand */}
                          {expandedTasks.has(task.id) && (
                            <div className="task-detail">
                              {task.intent && (
                                <div className="task-detail-row">
                                  <span className="task-detail-label">意图：</span>
                                  <span>{task.intent}</span>
                                </div>
                              )}
                              {task.retry_count !== undefined && task.retry_count > 0 && (
                                <div className="task-detail-row">
                                  <span className="task-detail-label">重试：</span>
                                  <span>{task.retry_count}</span>
                                </div>
                              )}

                              {/* Subagent Results */}
                              {task.subagent_results && task.subagent_results.length > 0 && (
                                <div className="subagent-section">
                                  <div className="subagent-header">Subagent 结果</div>
                                  {task.subagent_results.map((result, ri) => (
                                    <div key={ri} className="subagent-source">
                                      <span
                                        className="subagent-source-label"
                                        style={{ backgroundColor: DATA_SOURCE_COLORS[result.source] || "#6c7086" }}
                                      >
                                        {result.source}
                                      </span>
                                      <span className="subagent-source-count">
                                        {result.entries.length} 条结果（hop {result.hop}）
                                      </span>
                                      {result.entries.slice(0, 5).map((entry, ei) => (
                                        <div key={ei} className="subagent-entry">
                                          <div className="subagent-entry-title">
                                            {entry.url ? (
                                              <a href={entry.url} target="_blank" rel="noopener noreferrer" className="subagent-entry-link">
                                                {entry.title}
                                              </a>
                                            ) : (
                                              entry.title
                                            )}
                                          </div>
                                          <div className="subagent-entry-snippet">{entry.snippet.slice(0, 150)}</div>
                                          <div className="subagent-entry-meta">
                                            <span className="subagent-entry-score">
                                              相关度：{(entry.relevance_score * 100).toFixed(0)}%
                                            </span>
                                            <span className="subagent-entry-source">{entry.source}</span>
                                          </div>
                                        </div>
                                      ))}
                                      {result.entries.length > 5 && (
                                        <div className="subagent-more">
                                          还有 {result.entries.length - 5} 条结果
                                        </div>
                                      )}
                                    </div>
                                  ))}
                                </div>
                              )}

                              {/* Context Fragments */}
                              {task.context_fragments && task.context_fragments.length > 0 && (
                                <div className="context-section">
                                  <div className="subagent-header">上下文片段</div>
                                  {task.context_fragments.map((frag, fi) => (
                                    <div key={fi} className="context-fragment">
                                      <span className="context-fragment-key">{frag.key}</span>
                                    </div>
                                  ))}
                                </div>
                              )}
                            </div>
                          )}
                        </div>
                      ))}
                      {items.length === 0 && (
                        <div className="task-section-empty">暂无任务</div>
                      )}
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        )}
      </div>

      <style>{`
        .animate-spin { animation: dash-spin 1s linear infinite; }
        @keyframes dash-spin {
          from { transform: rotate(0deg); }
          to { transform: rotate(360deg); }
        }
      `}</style>
    </div>
  );
}
