import { useState, useEffect } from "react";
import { useAppStore } from "../hooks/useAppStore";
import FeroHaIcon from "./FeroHaIcon";

function hasTauriRuntime() {
  return typeof window !== "undefined" && Boolean(window.__TAURI_INTERNALS__);
}

export default function TaskPanel() {
  const [isTauri] = useState(hasTauriRuntime);
  const tasks = useAppStore((s) => s.tasks);
  const updateTask = useAppStore((s) => s.updateTask);
  const clearCompletedTasks = useAppStore((s) => s.clearCompletedTasks);

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
        }
      );
    };
    setup();
    return () => { unlisten?.(); };
  }, [isTauri, updateTask]);

  const approveTask = async (taskId: string) => {
    if (!isTauri) return;
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("approve_task", { taskId });
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
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("cancel_task", { taskId });
    } catch (e) {
      console.error("Cancel failed:", e);
    }
  };

  const getStatusColor = (status: string) => {
    switch (status) {
      case "pending": return "#f9e2af";
      case "approved": return "#89b4fa";
      case "running": return "#a6e3a1";
      case "done": return "#a6e3a1";
      case "error": return "#f38ba8";
      case "cancelled": return "#6c7086";
      default: return "#a6adc8";
    }
  };

  const running = tasks.filter((t) => t.status === "running");
  const pending = tasks.filter((t) => t.status === "pending" || t.status === "approved");
  const completed = tasks.filter((t) => t.status === "done");
  const failed = tasks.filter((t) => t.status === "error" || t.status === "cancelled");

  if (tasks.length === 0) {
    return (
      <div style={styles.emptyContainer}>
        <div style={styles.emptyContent}>
          <FeroHaIcon name="Coffee" size={32} />
          <div style={{ marginTop: 12, fontSize: 13, color: "var(--text-muted)" }}>No active tasks</div>
          <div style={{ fontSize: 11, marginTop: 4, color: "var(--text-muted)" }}>
            Use /agent in the CLI to start tasks
          </div>
        </div>
      </div>
    );
  }

  return (
    <div style={styles.panel}>
      <style>{`
        .animate-spin { animation: spin 1s linear infinite; }
        @keyframes spin {
          from { transform: rotate(0deg); }
          to { transform: rotate(360deg); }
        }
      `}</style>
      <div style={styles.header}>
        <span style={styles.headerTitle}>Tasks</span>
        {(completed.length > 0 || failed.length > 0) && (
          <button style={styles.clearBtn} onClick={clearCompletedTasks} title="Clear completed and failed tasks">
            Clear completed
          </button>
        )}
      </div>
      <div style={styles.list}>
        {running.length > 0 && (
          <div style={styles.section}>
            <div style={styles.sectionHeader}>
              <FeroHaIcon name="Loader" size={14} className="animate-spin" />
              <span style={styles.sectionTitle}>Active</span>
              <span style={styles.sectionCount}>{running.length}</span>
            </div>
            {running.map((task) => (
              <TaskItem key={task.id} task={task} getStatusColor={getStatusColor} approveTask={approveTask} cancelTask={cancelTask} isTauri={isTauri} />
            ))}
          </div>
        )}
        {pending.length > 0 && (
          <div style={styles.section}>
            <div style={styles.sectionHeader}>
              <FeroHaIcon name="Clock" size={14} />
              <span style={styles.sectionTitle}>Pending</span>
              <span style={styles.sectionCount}>{pending.length}</span>
            </div>
            {pending.map((task) => (
              <TaskItem key={task.id} task={task} getStatusColor={getStatusColor} approveTask={approveTask} cancelTask={cancelTask} isTauri={isTauri} />
            ))}
          </div>
        )}
        {completed.length > 0 && (
          <div style={styles.section}>
            <div style={styles.sectionHeader}>
              <FeroHaIcon name="CheckCircle" size={14} />
              <span style={styles.sectionTitle}>Done</span>
              <span style={styles.sectionCount}>{completed.length}</span>
            </div>
            {completed.map((task) => (
              <TaskItem key={task.id} task={task} getStatusColor={getStatusColor} approveTask={approveTask} cancelTask={cancelTask} isTauri={isTauri} />
            ))}
          </div>
        )}
        {failed.length > 0 && (
          <div style={styles.section}>
            <div style={styles.sectionHeader}>
              <FeroHaIcon name="AlertCircle" size={14} />
              <span style={styles.sectionTitle}>Failed</span>
              <span style={styles.sectionCount}>{failed.length}</span>
            </div>
            {failed.map((task) => (
              <TaskItem key={task.id} task={task} getStatusColor={getStatusColor} approveTask={approveTask} cancelTask={cancelTask} isTauri={isTauri} />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

interface TaskItemProps {
  task: { id: string; command: string; status: string; result?: string };
  getStatusColor: (status: string) => string;
  approveTask: (taskId: string) => void;
  cancelTask: (taskId: string) => void;
  isTauri: boolean;
}

function TaskItem({ task, getStatusColor, approveTask, cancelTask, isTauri }: TaskItemProps) {
  return (
    <div style={styles.taskItem}>
      <div style={styles.taskRow}>
        <span style={{ ...styles.taskDot, backgroundColor: getStatusColor(task.status) }} />
        <span style={styles.taskCmd} title={task.command}>{task.command}</span>
        <span style={styles.taskStatus}>{task.status}</span>
        {task.status === "pending" && isTauri && (
          <button
            style={styles.approveBtn}
            onClick={() => approveTask(task.id)}
            title="Approve task"
          >
            <FeroHaIcon name="Check" size={12} />
          </button>
        )}
        {(task.status === "pending" || task.status === "approved") && (
          <button
            style={styles.cancelBtn}
            onClick={() => cancelTask(task.id)}
            title="Cancel task"
          >
            <FeroHaIcon name="X" size={12} />
          </button>
        )}
      </div>
      {task.result && (
        <div style={styles.taskResult}>{task.result.slice(0, 100)}</div>
      )}
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  emptyContainer: {
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    height: "100%",
  },
  emptyContent: {
    textAlign: "center",
    padding: "40px 20px",
    color: "var(--text-muted)",
  },
  panel: {
    display: "flex",
    flexDirection: "column",
    height: "100%",
  },
  header: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    padding: "12px 0 8px 0",
    borderBottom: "1px solid var(--border-color)",
    marginBottom: "8px",
  },
  headerTitle: {
    fontSize: "16px",
    fontWeight: 600,
    color: "var(--text-primary)",
  },
  clearBtn: {
    backgroundColor: "transparent",
    border: "1px solid var(--border-color)",
    borderRadius: "4px",
    color: "var(--text-muted)",
    cursor: "pointer",
    fontSize: "11px",
    padding: "2px 8px",
  },
  list: {
    flex: 1,
    overflow: "auto",
  },
  section: {
    marginBottom: "12px",
  },
  sectionHeader: {
    display: "flex",
    alignItems: "center",
    gap: "6px",
    padding: "4px 0",
    marginBottom: "4px",
    color: "var(--text-secondary)",
    fontSize: "12px",
    fontWeight: 600,
    userSelect: "none",
  },
  sectionTitle: {
    flex: 1,
  },
  sectionCount: {
    fontSize: "10px",
    color: "var(--text-muted)",
    backgroundColor: "var(--bg-input)",
    borderRadius: "8px",
    padding: "0 6px",
    lineHeight: "16px",
  },
  taskItem: {
    padding: "6px 8px",
    borderBottom: "1px solid var(--border-color)",
    color: "var(--text-primary)",
    fontSize: "12px",
  },
  taskRow: {
    display: "flex",
    alignItems: "center",
    gap: "8px",
  },
  taskDot: {
    width: "8px",
    height: "8px",
    borderRadius: "50%",
    flexShrink: 0,
  },
  taskCmd: {
    flex: 1,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
    color: "var(--text-primary)",
  },
  taskStatus: {
    fontSize: "10px",
    fontStyle: "italic",
    color: "var(--text-muted)",
    flexShrink: 0,
  },
  approveBtn: {
    backgroundColor: "transparent",
    border: "1px solid #a6e3a1",
    borderRadius: "4px",
    color: "#a6e3a1",
    cursor: "pointer",
    fontSize: "11px",
    padding: "0 4px",
    display: "inline-flex",
    alignItems: "center",
    flexShrink: 0,
  },
  cancelBtn: {
    backgroundColor: "transparent",
    border: "1px solid #f38ba8",
    borderRadius: "4px",
    color: "#f38ba8",
    cursor: "pointer",
    fontSize: "11px",
    padding: "0 4px",
    display: "inline-flex",
    alignItems: "center",
    flexShrink: 0,
  },
  taskResult: {
    color: "var(--text-muted)",
    fontSize: "10px",
    marginTop: "4px",
    paddingLeft: "16px",
    wordBreak: "break-all",
    maxHeight: "40px",
    overflow: "hidden",
  },
};
