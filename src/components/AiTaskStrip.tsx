import { useEffect, useMemo, useState } from "react";
import { useAppStore } from "../hooks/useAppStore";
import type { LegacyCommandCardDefinition, ParamValue } from "../types/command-card";
import {
  buildCommandCardDispatchPayload,
  renderCommandCardPrompt,
  resolveTaskIntentSelectionForCard,
  stringifyCommandParams,
  taskIntentReviewInfo,
  TASK_INTENT_SELECT_OPTIONS,
  type TaskIntentSelection,
  type TaskIntentType,
} from "./CliBar";
import CommandCardPanel from "./CommandCardPanel";
import FeroHaIcon from "./FeroHaIcon";

type CommandParams = Record<string, ParamValue>;

interface AiTaskStripProps {
  isTauri: boolean;
}

function isInputFocused() {
  const el = document.activeElement;
  return el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement || el instanceof HTMLSelectElement;
}

export function aiTaskStripPreviewLabel(selection: TaskIntentSelection, preview: TaskIntentType): string {
  if (selection === "auto") return `自动 · ${taskIntentReviewInfo(preview).label}`;
  return taskIntentReviewInfo(preview).label;
}

export default function AiTaskStrip({ isTauri }: AiTaskStripProps) {
  const [showCommandPanel, setShowCommandPanel] = useState(false);
  const [selectedTaskType, setSelectedTaskType] = useState<TaskIntentSelection>("auto");
  const addTask = useAppStore((s) => s.addTask);
  const updateTask = useAppStore((s) => s.updateTask);
  const currentNotePath = useAppStore((s) => s.currentNote?.path ?? null);

  const previewTaskType = useMemo<TaskIntentType>(
    () => (selectedTaskType === "auto" ? "research" : selectedTaskType),
    [selectedTaskType],
  );
  const previewInfo = taskIntentReviewInfo(previewTaskType);
  const previewLabel = aiTaskStripPreviewLabel(selectedTaskType, previewTaskType);
  const statusText = `${previewInfo.risk}风险 · ${previewInfo.expectedOutput}`;
  const statusTitle = `Bridge 风险：${previewInfo.risk}；写权：${previewInfo.writePolicy}；产物：${previewInfo.expectedOutput}`;

  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      if (event.key === "/" && !showCommandPanel && !isInputFocused()) {
        event.preventDefault();
        setShowCommandPanel(true);
      }
      if (event.key === "Escape" && showCommandPanel) {
        setShowCommandPanel(false);
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [showCommandPanel]);

  const handleCommandCardExecute = async (card: LegacyCommandCardDefinition, params: CommandParams) => {
    setShowCommandPanel(false);
    const commandLabel = `card:${card.id}`;
    const localTaskId = `task_${Date.now()}`;
    addTask({ id: localTaskId, command: commandLabel, status: "pending" });

    const stringParams = stringifyCommandParams(params);
    const renderedPrompt = renderCommandCardPrompt(card, stringParams);
    const taskType = resolveTaskIntentSelectionForCard(selectedTaskType, card);

    if (isTauri) {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        const result = await invoke<{ task_id: string; status: string }>("dispatch_agent_task", {
          payload: buildCommandCardDispatchPayload({
            card,
            params: stringParams,
            renderedPrompt,
            taskType,
            contextNote: currentNotePath,
            timestamp: Date.now(),
          }),
        });
        updateTask(localTaskId, {
          id: result.task_id,
          status: result.status === "researching" ? "approved" : "pending",
        });
      } catch (error) {
        updateTask(localTaskId, { status: "error", result: String(error) });
      }
      return;
    }

    window.setTimeout(() => {
      updateTask(localTaskId, {
        status: "done",
        result: `[Browser preview] ${renderedPrompt}`,
      });
    }, 250);
  };

  return (
    <section className="ai-task-strip" aria-label="AI 任务条" style={styles.container}>
      <CommandCardPanel
        onExecute={handleCommandCardExecute}
        isTauri={isTauri}
        isOpen={showCommandPanel}
        onClose={() => setShowCommandPanel(false)}
      />

      <div style={styles.mainRow}>
        <button
          type="button"
          aria-label="打开指令卡"
          title="打开指令卡（/）"
          style={styles.primaryButton}
          onClick={() => setShowCommandPanel(true)}
        >
          <FeroHaIcon name="LayoutGrid" size={15} />
          <span>打开指令卡</span>
        </button>

        <select
          aria-label="AI 任务类型"
          className="feroha-select"
          style={styles.select}
          value={selectedTaskType}
          onChange={(event) => setSelectedTaskType(event.target.value as TaskIntentSelection)}
        >
          {TASK_INTENT_SELECT_OPTIONS.map((option) => (
            <option key={option.value} value={option.value}>
              {option.label}
            </option>
          ))}
        </select>

        <div
          role="status"
          aria-label="任务调度策略"
          title={statusTitle}
          style={styles.preview}
        >
          <span style={styles.intentChip}>{previewLabel}</span>
          <span style={styles.statusRail}>
            <FeroHaIcon name="ShieldCheck" size={13} />
            <span>Bridge</span>
            <span style={styles.statusDot} />
            <span style={styles.statusText}>{statusText}</span>
          </span>
        </div>
      </div>
    </section>
  );
}

const styles: Record<string, React.CSSProperties> = {
  container: {
    padding: "7px 12px",
    borderTop: "1px solid var(--border-muted)",
  },
  mainRow: {
    display: "flex",
    alignItems: "center",
    gap: "8px",
    minWidth: 0,
    flexWrap: "wrap",
  },
  primaryButton: {
    display: "inline-flex",
    alignItems: "center",
    justifyContent: "center",
    gap: "6px",
    minHeight: "30px",
    padding: "5px 10px",
    border: "1px solid var(--control-border, var(--border-color))",
    borderRadius: "6px",
    background: "var(--control-bg, var(--bg-input))",
    color: "var(--text-primary)",
    cursor: "pointer",
    fontSize: "12px",
    fontWeight: 700,
  },
  select: {
    width: "138px",
    minWidth: "124px",
    fontSize: "12px",
  },
  preview: {
    display: "flex",
    alignItems: "center",
    gap: "7px",
    flex: "1 1 280px",
    minWidth: 0,
    flexWrap: "nowrap",
    color: "var(--text-muted)",
    fontSize: "11px",
    lineHeight: 1.35,
  },
  intentChip: {
    display: "inline-flex",
    alignItems: "center",
    minHeight: "22px",
    padding: "2px 8px",
    border: "1px solid var(--accent-primary)",
    borderRadius: "999px",
    color: "var(--accent-primary)",
    background: "var(--accent-glow)",
    fontWeight: 700,
    whiteSpace: "nowrap",
  },
  statusRail: {
    display: "inline-flex",
    alignItems: "center",
    gap: "6px",
    minHeight: "24px",
    minWidth: 0,
    padding: "2px 9px",
    border: "1px solid var(--control-border)",
    borderRadius: "999px",
    background: "color-mix(in srgb, var(--control-bg) 82%, transparent)",
    color: "var(--text-secondary)",
    overflow: "hidden",
    whiteSpace: "nowrap",
    boxShadow: "inset 0 0 0 1px color-mix(in srgb, var(--bg-primary) 48%, transparent)",
  },
  statusDot: {
    width: "4px",
    height: "4px",
    borderRadius: "50%",
    background: "var(--accent-primary)",
    flex: "0 0 auto",
  },
  statusText: {
    minWidth: 0,
    overflow: "hidden",
    textOverflow: "ellipsis",
  },
};
