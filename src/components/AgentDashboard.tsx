import { useState, useEffect, useCallback } from "react";
import { useAppStore } from "../hooks/useAppStore";
import FeroHaIcon from "./FeroHaIcon";
function hasTauriRuntime() {
  return typeof window !== "undefined" && Boolean(window.__TAURI_INTERNALS__);
}

interface TaskItem {
  id: string;
  command?: string;
  task_type?: string;
  priority?: string;
  priority_score?: number;
  status?: string;
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

interface OrchestratorStatus {
  active_agents: number;
  degraded_agents: string[];
  epoch_count: number;
  track_count: number;
  agent_states: Array<{ agent_id: string; status: string }>;
}

interface DreamStatus {
  last_stats: {
    nrem_connections_strengthened: number;
    nrem_connections_pruned: number;
    rem_bridges_created: number;
    insight_communities_found: number;
    insight_summaries_generated: number;
    total_memories_processed: number;
    duration_ms: number;
  } | null;
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

export default function AgentDashboard() {
  const [isTauri] = useState(hasTauriRuntime);
  const updateTask = useAppStore((s) => s.updateTask);
  const clearCompletedTasks = useAppStore((s) => s.clearCompletedTasks);
  const agentTools = useAppStore((s) => s.agentTools);
  const fetchAgentTools = useAppStore((s) => s.fetchAgentTools);

  const [orchestratorStatus, setOrchestratorStatus] = useState<OrchestratorStatus | null>(null);
  const [dreamStatus, setDreamStatus] = useState<DreamStatus | null>(null);
  const [trustScore, setTrustScore] = useState<TrustScoreInfo | null>(null);
  const [backendTasks, setBackendTasks] = useState<TaskItem[]>([]);
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
      setError("Failed to load tasks");
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

  const refreshAll = useCallback(async () => {
    setError(null);
    await Promise.all([
      fetchOrchestrator(),
      fetchTasks(),
      fetchDreamStatus(),
      fetchTrustScore(),
      fetchAgentTools(),
    ]);
  }, [fetchOrchestrator, fetchTasks, fetchDreamStatus, fetchTrustScore, fetchAgentTools]);

  /* eslint-disable react-hooks/set-state-in-effect */
  useEffect(() => {
    fetchOrchestrator();
    fetchTasks();
    fetchDreamStatus();
    fetchTrustScore();
    fetchAgentTools();
    const interval = setInterval(() => {
      fetchOrchestrator();
      fetchTasks();
    }, 5000);
    return () => clearInterval(interval);
  }, [fetchOrchestrator, fetchTasks, fetchDreamStatus, fetchTrustScore, fetchAgentTools]);
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
        }
      );
    };
    setup();
    return () => { unlisten?.(); };
  }, [isTauri, updateTask, fetchTasks]);

  const approveTask = async (taskId: string) => {
    if (!isTauri) return;
    try {
      await invoke("approve_task", { taskId });
      fetchTasks();
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
    } catch (e) {
      console.error("Retry failed:", e);
    }
  };

  const triggerDream = async () => {
    setDreamLoading(true);
    try {
      await invoke("trigger_dream");
      setTimeout(() => fetchDreamStatus(), 1000);
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
      const s = t.status;
      return s === "Pending";
    }),
    Approved: backendTasks.filter((t) => {
      const s = t.status;
      if (!s) return false;
      return s.startsWith("Approved") || s === "Queued";
    }),
    Running: backendTasks.filter((t) => {
      const s = t.status;
      if (!s) return false;
      return s.startsWith("Running");
    }),
    Done: backendTasks.filter((t) => {
      const s = t.status;
      if (!s) return false;
      return s.startsWith("Done");
    }),
    Error: backendTasks.filter((t) => {
      const s = t.status;
      if (!s) return false;
      return s.startsWith("Error");
    }),
  };

  const orchestratorHealthy = !orchestratorStatus
    ? "gray"
    : orchestratorStatus.degraded_agents.length === 0
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

  return (
    <div className="agent-dashboard">
      <div className="dashboard-header">
        <h3 className="dashboard-title">
          <FeroHaIcon name="Bot" size={18} />
          Agent Dashboard
        </h3>
        <div className="dashboard-header-actions">
          <button className="dashboard-btn" onClick={refreshAll} title="Refresh all data">
            <FeroHaIcon name="RefreshCw" size={14} />
            Refresh
          </button>
          <button className="dashboard-btn" onClick={clearCompletedTasks} title="Clear completed and failed tasks">
            <FeroHaIcon name="Trash2" size={14} />
            Clear
          </button>
        </div>
      </div>

      {error && <div className="dashboard-error">{error}</div>}

      <div className="dashboard-grid">
        {/* Orchestrator Status Card */}
        <div className="card orchestrator-card">
          <div className="card-header">
            <FeroHaIcon name="Activity" size={16} />
            <h4 className="card-title">Orchestrator</h4>
            <span className={`status-indicator status-${orchestratorHealthy}`} />
          </div>
          {orchestratorStatus ? (
            <div className="card-body">
              <div className="stat-row">
                <span className="stat-label">Active Agents</span>
                <span className="stat-value">{orchestratorStatus.active_agents}</span>
              </div>
              <div className="stat-row">
                <span className="stat-label">Degraded</span>
                <span className="stat-value" style={{ color: orchestratorStatus.degraded_agents.length > 0 ? "var(--status-error-color)" : "inherit" }}>
                  {orchestratorStatus.degraded_agents.length}
                </span>
              </div>
              <div className="stat-row">
                <span className="stat-label">Epochs</span>
                <span className="stat-value">{orchestratorStatus.epoch_count}</span>
              </div>
              <div className="stat-row">
                <span className="stat-label">Tracks</span>
                <span className="stat-value">{orchestratorStatus.track_count}</span>
              </div>
              {orchestratorStatus.degraded_agents.length > 0 && (
                <div className="degraded-list">
                  <span className="degraded-label">Degraded agents:</span>
                  {orchestratorStatus.degraded_agents.map((id) => (
                    <span key={id} className="degraded-agent-tag">{id.slice(0, 12)}</span>
                  ))}
                </div>
              )}
            </div>
          ) : (
            <div className="card-muted">
              {isTauri ? "Loading..." : "Tauri backend not available"}
            </div>
          )}
        </div>

        {/* Trust Score Gauge */}
        <div className="card trust-card">
          <div className="card-header">
            <FeroHaIcon name="Shield" size={16} />
            <h4 className="card-title">Trust Score</h4>
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
                  <span className="stat-label">Accept rate</span>
                  <span className="stat-value">{(trustScore.acceptance_rate * 100).toFixed(0)}%</span>
                </div>
                <div className="stat-row">
                  <span className="stat-label">Interactions</span>
                  <span className="stat-value">{trustScore.total_interactions}</span>
                </div>
              </div>
            )}
          </div>
        </div>

        {/* Dream Panel */}
        <div className="card dream-card">
          <div className="card-header">
            <FeroHaIcon name="Moon" size={16} />
            <h4 className="card-title">Dream Engine</h4>
            <button
              className={`dashboard-btn dashboard-btn-accent ${dreamLoading ? "loading" : ""}`}
              onClick={triggerDream}
              disabled={dreamLoading}
              title="Trigger dream cycle"
            >
              <FeroHaIcon name={dreamLoading ? "Loader" : "Sparkles"} size={14} />
              {dreamLoading ? "Dreaming..." : "Dream"}
            </button>
          </div>
          {dreamStatus?.last_stats ? (
            <div className="card-body">
              <div className="stat-group">
                <span className="stat-group-label">NREM Phase</span>
                <div className="stat-row">
                  <span className="stat-label">Strengthened</span>
                  <span className="stat-value">{dreamStatus.last_stats.nrem_connections_strengthened}</span>
                </div>
                <div className="stat-row">
                  <span className="stat-label">Pruned</span>
                  <span className="stat-value">{dreamStatus.last_stats.nrem_connections_pruned}</span>
                </div>
              </div>
              <div className="stat-group">
                <span className="stat-group-label">REM Phase</span>
                <div className="stat-row">
                  <span className="stat-label">Bridges</span>
                  <span className="stat-value">{dreamStatus.last_stats.rem_bridges_created}</span>
                </div>
              </div>
              <div className="stat-group">
                <span className="stat-group-label">Insight Phase</span>
                <div className="stat-row">
                  <span className="stat-label">Communities</span>
                  <span className="stat-value">{dreamStatus.last_stats.insight_communities_found}</span>
                </div>
              </div>
              {dreamStatus.insights.length > 0 && (
                <div className="insight-list">
                  <span className="stat-group-label">Insights ({dreamStatus.insights.length})</span>
                  {dreamStatus.insights.slice(0, 5).map((insight) => (
                    <div key={insight.id} className="insight-item">
                      <div className="insight-header">
                        <span className="insight-title">{insight.title}</span>
                        <span className="insight-confidence">
                          {Math.round(insight.confidence * 100)}%
                        </span>
                      </div>
                      <div className="insight-summary">
                        {insight.summary.slice(0, 100)}{insight.summary.length > 100 ? "..." : ""}
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </div>
          ) : (
            <div className="card-muted">
              {dreamStatus && !dreamStatus.last_stats
                ? "No dream cycle run yet. Click Dream to start."
                : isTauri
                  ? "Loading..."
                  : "Tauri backend not available"}
            </div>
          )}
        </div>

        {/* Agent Tools */}
        {agentTools && agentTools.length > 0 && (
          <div className="card tools-card">
            <div className="card-header">
              <FeroHaIcon name="Wrench" size={16} />
              <h4 className="card-title">Agent Tools ({agentTools.length})</h4>
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
              Memory Insights
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
                      Confidence: {(insight.confidence * 100).toFixed(0)}%
                    </span>
                  </div>
                  <div className="insight-detail-summary">{insight.summary}</div>
                  {insight.related_chunks && insight.related_chunks.length > 0 && (
                    <div className="insight-detail-related">
                      <span className="insight-detail-label">Related chunks: </span>
                      {insight.related_chunks.slice(0, 5).join(", ")}
                      {insight.related_chunks.length > 5 && ` +${insight.related_chunks.length - 5} more`}
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
            Task Queue
          </h4>
          <span className="task-count-badge">{backendTasks.length} total</span>
        </div>

        {backendTasks.length === 0 ? (
          <div className="card-muted" style={{ padding: "24px", textAlign: "center" }}>
            <FeroHaIcon name="Coffee" size={24} />
            <div style={{ marginTop: 8, fontSize: 13 }}>No active tasks</div>
            <div style={{ fontSize: 11, marginTop: 4 }}>Use /agent in the CLI to start tasks</div>
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
                      {status}
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
                            <span className="task-type">{task.task_type || "Task"}</span>
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
                                    title="Approve"
                                  >
                                    <FeroHaIcon name="Check" size={10} />
                                  </button>
                                  <button
                                    className="task-action-btn cancel"
                                    onClick={() => cancelTask(task.id)}
                                    title="Cancel"
                                  >
                                    <FeroHaIcon name="X" size={10} />
                                  </button>
                                </>
                              )}
                              {status === "Approved" && (
                                <button
                                  className="task-action-btn cancel"
                                  onClick={() => cancelTask(task.id)}
                                  title="Cancel"
                                >
                                  <FeroHaIcon name="X" size={10} />
                                </button>
                              )}
                              {status === "Error" && (
                                <button
                                  className="task-action-btn retry"
                                  onClick={() => retryTask(task.id)}
                                  title="Retry"
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
                                  <span className="task-detail-label">Intent:</span>
                                  <span>{task.intent}</span>
                                </div>
                              )}
                              {task.retry_count !== undefined && task.retry_count > 0 && (
                                <div className="task-detail-row">
                                  <span className="task-detail-label">Retries:</span>
                                  <span>{task.retry_count}</span>
                                </div>
                              )}

                              {/* Subagent Results */}
                              {task.subagent_results && task.subagent_results.length > 0 && (
                                <div className="subagent-section">
                                  <div className="subagent-header">Subagent Results</div>
                                  {task.subagent_results.map((result, ri) => (
                                    <div key={ri} className="subagent-source">
                                      <span
                                        className="subagent-source-label"
                                        style={{ backgroundColor: DATA_SOURCE_COLORS[result.source] || "#6c7086" }}
                                      >
                                        {result.source}
                                      </span>
                                      <span className="subagent-source-count">
                                        {result.entries.length} results (hop {result.hop})
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
                                              Relevance: {(entry.relevance_score * 100).toFixed(0)}%
                                            </span>
                                            <span className="subagent-entry-source">{entry.source}</span>
                                          </div>
                                        </div>
                                      ))}
                                      {result.entries.length > 5 && (
                                        <div className="subagent-more">
                                          +{result.entries.length - 5} more results
                                        </div>
                                      )}
                                    </div>
                                  ))}
                                </div>
                              )}

                              {/* Context Fragments */}
                              {task.context_fragments && task.context_fragments.length > 0 && (
                                <div className="context-section">
                                  <div className="subagent-header">Context Fragments</div>
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
                        <div className="task-section-empty">No tasks</div>
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
