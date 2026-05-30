import { useAppStore } from "../hooks/useAppStore";
import { useState, useEffect, useCallback } from "react";
import type { OrchestratorEvent, AgentState, TrackInfo } from "../types/orchestrator";
import FeroHaIcon from "./FeroHaIcon";

export default function OrchestratorPanel() {
  const {
    orchestratorStatus,
    fetchOrchestratorStatus,
    terminateAgent,
    reinstateAgent,
  } = useAppStore();
  const [expanded, setExpanded] = useState(false);

  useEffect(() => {
    fetchOrchestratorStatus();
    const interval = setInterval(fetchOrchestratorStatus, 5000);
    return () => clearInterval(interval);
  }, [fetchOrchestratorStatus]);

  const handleTerminate = useCallback(
    async (agentId: string) => {
      await terminateAgent(agentId);
      fetchOrchestratorStatus();
    },
    [terminateAgent, fetchOrchestratorStatus],
  );

  const handleReinstate = useCallback(
    async (agentId: string) => {
      await reinstateAgent(agentId);
      fetchOrchestratorStatus();
    },
    [reinstateAgent, fetchOrchestratorStatus],
  );

  if (!orchestratorStatus) {
    return (
      <div style={styles.bar}>
        <span style={styles.statusItem}>
          <span style={{ ...styles.dot, backgroundColor: "#6c7086" }} />
          Orchestrator: No data
        </span>
      </div>
    );
  }

  const s = orchestratorStatus;
  const activeCount = s.active_agents;
  const degradedCount = s.degraded_agents.length;
  const totalAgents = s.agent_states.length;
  const terminatedCount = s.agent_states.filter(
    (a) => a.status === "Terminated",
  ).length;

  return (
    <div style={styles.container}>
      {/* Collapsed status bar */}
      <div style={styles.bar} onClick={() => setExpanded(!expanded)} role="button" tabIndex={0} onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") { setExpanded(!expanded); e.preventDefault(); } }}>
        <div style={styles.badges}>
          <span style={styles.badge} title="Active agents">
            <span style={{ ...styles.dot, backgroundColor: successColor }} />
            Active: {activeCount}
          </span>
          <span style={styles.badge} title="Degraded agents">
            <span style={{ ...styles.dot, backgroundColor: warningColor }} />
            Degraded: {degradedCount}
          </span>
          <span style={styles.badge} title="Terminated agents">
            <span style={{ ...styles.dot, backgroundColor: errorColor }} />
            Terminated: {terminatedCount}
          </span>
        </div>
        <span style={styles.expandBtn}><FeroHaIcon name={expanded ? "ChevronDown" : "ChevronUp"} size={12} /></span>
      </div>

      {/* Expanded panel */}
      {expanded && (
        <div style={styles.panel}>
          <div style={styles.summaryRow}>
            <span>Epoch: {s.epoch_count}</span>
            <span>|</span>
            <span>Tracks: {s.track_count}</span>
            <span>|</span>
            <span>Total agents: {totalAgents}</span>
          </div>

          {/* Agent list */}
          {s.agent_states.length > 0 && (
            <div style={styles.section}>
              <div style={styles.sectionTitle}>Agents</div>
              <table style={styles.table}>
                <thead>
                  <tr style={styles.tableHeaderRow}>
                    <th style={styles.th}>Status</th>
                    <th style={styles.th}>Agent ID</th>
                    <th style={styles.th}>Regressions</th>
                    <th style={styles.th}>Last Epoch</th>
                    <th style={styles.th}>Actions</th>
                  </tr>
                </thead>
                <tbody>
                  {s.agent_states.map((agent) => (
                    <AgentRow
                      key={agent.agent_id}
                      agent={agent}
                      onTerminate={handleTerminate}
                      onReinstate={handleReinstate}
                    />
                  ))}
                </tbody>
              </table>
            </div>
          )}

          {/* Event timeline */}
          {s.recent_events.length > 0 && (
            <div style={styles.section}>
              <div style={styles.sectionTitle}>Recent Events</div>
              <div style={styles.eventList}>
                {s.recent_events.slice(-20).reverse().map((event, i) => (
                  <EventRow key={`${event.agent_id}-${event.timestamp}-${i}`} event={event} />
                ))}
              </div>
            </div>
          )}

          {/* Track details */}
          {s.track_details.length > 0 && (
            <div style={styles.section}>
              <div style={styles.sectionTitle}>Tracks</div>
              <div style={styles.trackList}>
                {s.track_details.map((track) => (
                  <TrackRow key={track.track_id} track={track} />
                ))}
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

function AgentRow({
  agent,
  onTerminate,
  onReinstate,
}: {
  agent: AgentState;
  onTerminate: (id: string) => void;
  onReinstate: (id: string) => void;
}) {
  const statusColor = agentStatusColor(agent.status);
  return (
    <tr style={styles.tableRow}>
      <td style={styles.td}>
        <span style={{ display: "inline-block", width: 8, height: 8, borderRadius: "50%", backgroundColor: statusColor }} />{" "}
        <span style={{ color: statusColor }}>{agent.status}</span>
      </td>
      <td style={styles.td}>{agent.agent_id}</td>
      <td style={styles.td}>{agent.regression_count}</td>
      <td style={styles.td}>{agent.last_epoch}</td>
      <td style={styles.td}>
        <div style={styles.actionBtns}>
          {(agent.status === "Healthy" || agent.status === "Degraded") && (
            <button
              style={styles.terminateBtn}
              onClick={() => onTerminate(agent.agent_id)}
              title="Terminate agent"
            >
              Terminate
            </button>
          )}
          {(agent.status === "Degraded" || agent.status === "Terminated" || agent.status === "Cooldown") && (
            <button
              style={styles.reinstateBtn}
              onClick={() => onReinstate(agent.agent_id)}
              title="Reinstate agent"
            >
              Reinstate
            </button>
          )}
        </div>
      </td>
    </tr>
  );
}

function EventRow({ event }: { event: OrchestratorEvent }) {
  const color = eventTypeColor(event.event_type);
  const time = new Date(event.timestamp).toLocaleTimeString();
  return (
    <div style={styles.eventRow}>
      <span style={{ ...styles.eventType, color }}>{event.event_type}</span>
      <span style={styles.eventAgent}>{event.agent_id}</span>
      <span style={styles.eventTime}>{time}</span>
      <span style={styles.eventEpoch}>#{event.epoch}</span>
      {event.detail && <span style={styles.eventDetail}>{event.detail}</span>}
    </div>
  );
}

function TrackRow({ track }: { track: TrackInfo }) {
  const color = track.status === "completed" ? successColor : track.status === "failed" ? errorColor : "#6c7086";
  return (
    <div style={styles.trackRow}>
      <span style={{ ...styles.trackDot, backgroundColor: color }} />
      <span style={styles.trackId}>{track.track_id}</span>
      <span style={{ ...styles.trackStatus, color }}>{track.status}</span>
      <span style={styles.trackFocus}>{track.focus}</span>
    </div>
  );
}

const successColor = "#a6e3a1";
const warningColor = "#f9e2af";
const errorColor = "#f38ba8";

function agentStatusColor(status: string): string {
  switch (status) {
    case "Healthy":
      return successColor;
    case "Degraded":
      return warningColor;
    case "Terminated":
      return errorColor;
    case "Cooldown":
      return "#89b4fa";
    default:
      return "#6c7086";
  }
}

function eventTypeColor(eventType: string): string {
  switch (eventType) {
    case "AuditPassed":
      return successColor;
    case "RegressionDetected":
      return warningColor;
    case "AgentDegraded":
      return errorColor;
    case "CleanKnowledgeExtracted":
      return "#89b4fa";
    case "ParallelTracksSpawned":
      return "#cba6f7";
    case "TrackCompleted":
      return successColor;
    case "TrackFailed":
      return errorColor;
    case "TrackCancelled":
      return "#6c7086";
    default:
      return "#a6adc8";
  }
}

const styles: Record<string, React.CSSProperties> = {
  container: {
    borderTop: "1px solid #313244",
    backgroundColor: "#1e1e2e",
    userSelect: "none",
  },
  bar: {
    display: "flex",
    justifyContent: "space-between",
    alignItems: "center",
    padding: "4px 12px",
    cursor: "pointer",
    minHeight: "24px",
    backgroundColor: "#181825",
    borderBottom: "1px solid #313244",
  },
  badges: {
    display: "flex",
    gap: "14px",
  },
  badge: {
    fontSize: "11px",
    color: "#a6adc8",
    display: "inline-flex",
    alignItems: "center",
    gap: "4px",
  },
  dot: {
    width: "8px",
    height: "8px",
    borderRadius: "50%",
    flexShrink: 0,
  },
  expandBtn: {
    fontSize: "10px",
    color: "#6c7086",
  },
  panel: {
    padding: "8px 12px",
    maxHeight: "320px",
    overflow: "auto",
    display: "flex",
    flexDirection: "column",
    gap: "8px",
  },
  summaryRow: {
    display: "flex",
    gap: "10px",
    fontSize: "11px",
    color: "#bac2de",
  },
  section: {
    display: "flex",
    flexDirection: "column",
    gap: "4px",
  },
  sectionTitle: {
    fontSize: "11px",
    fontWeight: 700,
    color: "#cdd6f4",
    textTransform: "uppercase" as const,
    letterSpacing: "0.05em",
  },
  table: {
    width: "100%",
    borderCollapse: "collapse" as const,
    fontSize: "11px",
  },
  tableHeaderRow: {
    borderBottom: "1px solid #313244",
  },
  th: {
    textAlign: "left" as const,
    padding: "2px 6px",
    color: "#6c7086",
    fontWeight: 600,
    fontSize: "10px",
    textTransform: "uppercase" as const,
  },
  tableRow: {
    borderBottom: "1px solid #252536",
  },
  td: {
    padding: "3px 6px",
    color: "#a6adc8",
    whiteSpace: "nowrap" as const,
    overflow: "hidden",
    textOverflow: "ellipsis",
    maxWidth: "180px",
  },
  actionBtns: {
    display: "flex",
    gap: "4px",
  },
  terminateBtn: {
    backgroundColor: "transparent",
    border: "1px solid #f38ba8",
    borderRadius: "3px",
    color: "#f38ba8",
    cursor: "pointer",
    fontSize: "10px",
    padding: "1px 6px",
  },
  reinstateBtn: {
    backgroundColor: "transparent",
    border: "1px solid #a6e3a1",
    borderRadius: "3px",
    color: "#a6e3a1",
    cursor: "pointer",
    fontSize: "10px",
    padding: "1px 6px",
  },
  eventList: {
    display: "flex",
    flexDirection: "column",
    gap: "2px",
    maxHeight: "160px",
    overflow: "auto",
  },
  eventRow: {
    display: "flex",
    gap: "8px",
    alignItems: "center",
    fontSize: "10px",
    padding: "2px 0",
    borderBottom: "1px solid #252536",
  },
  eventType: {
    fontWeight: 600,
    minWidth: "120px",
    fontSize: "10px",
  },
  eventAgent: {
    color: "#a6adc8",
    minWidth: "100px",
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap" as const,
  },
  eventTime: {
    color: "#6c7086",
    minWidth: "70px",
  },
  eventEpoch: {
    color: "#6c7086",
    minWidth: "30px",
  },
  eventDetail: {
    color: "#585b70",
    fontSize: "9px",
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap" as const,
    flex: 1,
  },
  trackList: {
    display: "flex",
    flexDirection: "column",
    gap: "2px",
  },
  trackRow: {
    display: "flex",
    gap: "8px",
    alignItems: "center",
    fontSize: "10px",
  },
  trackDot: {
    width: "6px",
    height: "6px",
    borderRadius: "50%",
    flexShrink: 0,
  },
  trackId: {
    color: "#a6adc8",
  },
  trackStatus: {
    fontWeight: 600,
  },
  trackFocus: {
    color: "#6c7086",
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap" as const,
  },
  statusItem: {
    fontSize: "11px",
    color: "#a6adc8",
    display: "inline-flex",
    alignItems: "center",
    gap: "6px",
  },
};
