import { useMemo, useState } from "react";
import { useAppStore } from "../hooks/useAppStore";
import {
  TASK_INTENT_OPTIONS,
  taskIntentReviewInfo,
  type TaskIntentType,
} from "./CliBar";
import FeroHaIcon from "./FeroHaIcon";
import {
  buildMockSimulationRun,
  mockSimulationCaseOptions,
  type MockSimulationCaseKey,
} from "../lib/mockSimulationSuite";

export type HumanTaskScope = "current_note" | "selected_text" | "folder" | "graph_focus" | "freeform";
export type HumanTaskReviewMode = "manual_bridge" | "read_only_auto_queue" | "draft_only";

export interface HumanTaskDraft {
  title: string;
  taskType: TaskIntentType;
  scope: HumanTaskScope;
  expectedOutput: string;
  reviewMode: HumanTaskReviewMode;
  contextNote: string | null;
  timestamp: number;
}

const scopeOptions: Array<{ value: HumanTaskScope; label: string }> = [
  { value: "current_note", label: "当前笔记" },
  { value: "selected_text", label: "选中文本" },
  { value: "folder", label: "文件夹" },
  { value: "graph_focus", label: "图谱焦点" },
  { value: "freeform", label: "自由上下文" },
];

const reviewModeOptions: Array<{ value: HumanTaskReviewMode; label: string; description: string }> = [
  { value: "manual_bridge", label: "人工 Bridge 审查", description: "AI 输出进入 Bridge，由人类确认后再写入。" },
  { value: "read_only_auto_queue", label: "只读自动入队", description: "适合检索、总结、验证等无写入任务。" },
  { value: "draft_only", label: "仅生成草稿", description: "只产生草稿结果，不自动进入写入路径。" },
];

export function buildHumanTaskDispatchPayload(draft: HumanTaskDraft) {
  return {
    intent: draft.title,
    content: draft.expectedOutput,
    task_type: draft.taskType,
    scope: draft.scope,
    expected_output: draft.expectedOutput,
    review_mode: draft.reviewMode,
    context_note: draft.contextNote,
    source: "human_task_intake",
    timestamp: draft.timestamp,
  };
}

export default function HumanTaskIntake({ isTauri }: { isTauri: boolean }) {
  const vaultPath = useAppStore((s) => s.vaultPath);
  const currentNotePath = useAppStore((s) => s.currentNote?.path ?? null);
  const addTask = useAppStore((s) => s.addTask);
  const updateTask = useAppStore((s) => s.updateTask);
  const applyMockSimulationRun = useAppStore((s) => s.applyMockSimulationRun);
  const [title, setTitle] = useState("");
  const [taskType, setTaskType] = useState<TaskIntentType>("research");
  const [scope, setScope] = useState<HumanTaskScope>("current_note");
  const [expectedOutput, setExpectedOutput] = useState("");
  const [reviewMode, setReviewMode] = useState<HumanTaskReviewMode>("manual_bridge");
  const [statusText, setStatusText] = useState("");
  const [simulationStatus, setSimulationStatus] = useState("");

  const intentInfo = taskIntentReviewInfo(taskType);
  const reviewDescription = useMemo(
    () => reviewModeOptions.find((item) => item.value === reviewMode)?.description ?? "",
    [reviewMode],
  );
  const needsBridgeVault = isTauri && reviewMode === "manual_bridge" && !vaultPath;
  const canSubmit =
    title.trim().length > 0 && expectedOutput.trim().length > 0 && !needsBridgeVault;

  const submit = async () => {
    if (!canSubmit) return;
    const timestamp = Date.now();
    const payload = buildHumanTaskDispatchPayload({
      title: title.trim(),
      taskType,
      scope,
      expectedOutput: expectedOutput.trim(),
      reviewMode,
      contextNote: currentNotePath,
      timestamp,
    });
    const localTaskId = `human_task_${timestamp}`;
    addTask({ id: localTaskId, command: payload.intent, status: "pending" });

    if (isTauri) {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        const result = await invoke<{ task_id: string; status: string }>("dispatch_agent_task", {
          payload,
        });
        updateTask(localTaskId, {
          id: result.task_id,
          status: result.status === "researching" ? "approved" : "pending",
        });
        setStatusText("任务已提交到 AI Manager。");
      } catch (error) {
        updateTask(localTaskId, { status: "error", result: String(error) });
        setStatusText(`提交失败：${String(error)}`);
      }
      return;
    }

    updateTask(localTaskId, {
      status: "done",
      result: `[Browser preview] ${payload.intent}`,
    });
    setStatusText("浏览器预览：任务 payload 已生成。");
  };

  const runMockSimulation = async (caseKey: MockSimulationCaseKey) => {
    const run = buildMockSimulationRun(caseKey);
    let backendStatus = "浏览器预览";

    if (isTauri) {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        const result = await invoke<{ task_id: string; status: string }>("dispatch_agent_task", {
          payload: run.dispatchPayload,
        });
        backendStatus = `后端已接收 ${result.task_id} / ${result.status}`;
      } catch (error) {
        backendStatus = `后端未接收：${String(error)}`;
      }
    }

    applyMockSimulationRun(run);
    setSimulationStatus(
      `模拟投喂已完成：${run.task.id} · ${run.kernelStatus} · ${backendStatus}`,
    );
  };

  return (
    <section className="human-task-intake" style={styles.container}>
      <header style={styles.header}>
        <div style={styles.titleMark}>
          <FeroHaIcon name="Send" size={18} />
        </div>
        <div style={styles.titleGroup}>
          <h2 style={styles.title}>向 AI 提任务</h2>
          <p style={styles.subtitle}>
            上游任务创建入口。Bridge Review 审 AI 输出，Diff Review 审具体文本改动。
          </p>
        </div>
      </header>

      <div style={styles.layout}>
        <form style={styles.form} onSubmit={(event) => { event.preventDefault(); submit(); }}>
          <label style={styles.field}>
            <span style={styles.label}>任务标题</span>
            <input
              aria-label="任务标题"
              className="feroha-input"
              value={title}
              onChange={(event) => setTitle(event.target.value)}
              placeholder="例如：整理 Dream 三区记忆"
            />
          </label>

          <div style={styles.twoCol}>
            <label style={styles.field}>
              <span style={styles.label}>任务类型</span>
              <select
                aria-label="任务类型"
                className="feroha-select"
                value={taskType}
                onChange={(event) => setTaskType(event.target.value as TaskIntentType)}
              >
                {TASK_INTENT_OPTIONS.map((option) => (
                  <option key={option.value} value={option.value}>{option.label}</option>
                ))}
              </select>
            </label>

            <label style={styles.field}>
              <span style={styles.label}>任务范围</span>
              <select
                aria-label="任务范围"
                className="feroha-select"
                value={scope}
                onChange={(event) => setScope(event.target.value as HumanTaskScope)}
              >
                {scopeOptions.map((option) => (
                  <option key={option.value} value={option.value}>{option.label}</option>
                ))}
              </select>
            </label>
          </div>

          <label style={styles.field}>
            <span style={styles.label}>期望输出</span>
            <textarea
              aria-label="期望输出"
              className="feroha-textarea"
              value={expectedOutput}
              onChange={(event) => setExpectedOutput(event.target.value)}
              placeholder="说明你希望 AI 交付的内容、格式和审查边界。"
              rows={5}
            />
          </label>

          <label style={styles.field}>
            <span style={styles.label}>审查方式</span>
            <select
              aria-label="审查方式"
              className="feroha-select"
              value={reviewMode}
              onChange={(event) => setReviewMode(event.target.value as HumanTaskReviewMode)}
            >
              {reviewModeOptions.map((option) => (
                <option key={option.value} value={option.value}>{option.label}</option>
              ))}
            </select>
          </label>

          <button type="submit" style={styles.submitButton} disabled={!canSubmit}>
            <FeroHaIcon name="Send" size={14} />
            <span>提交给 AI Manager</span>
          </button>
          {needsBridgeVault && (
            <p role="status" style={styles.status}>
              先打开笔记库后才能提交 Bridge 审查任务；也可以切换为只读自动入队或仅生成草稿。
            </p>
          )}
          {statusText && <p style={styles.status}>{statusText}</p>}

          <section aria-label="mockSimulationSuite" style={styles.simulationPanel}>
            <div style={styles.simulationHeader}>
              <strong style={styles.simulationTitle}>模拟测试</strong>
              <span style={styles.simulationHint}>固定数据走同一任务入口</span>
            </div>
            <div style={styles.simulationButtons}>
              {mockSimulationCaseOptions.map((option) => (
                <button
                  key={option.key}
                  aria-label={option.label}
                  type="button"
                  style={styles.simulationButton}
                  onClick={() => {
                    void runMockSimulation(option.key);
                  }}
                >
                  <span style={styles.simulationButtonLabel}>{option.label}</span>
                  <span style={styles.simulationButtonMeta}>{option.description}</span>
                </button>
              ))}
            </div>
            {simulationStatus && (
              <p role="status" style={styles.status}>
                {simulationStatus}
              </p>
            )}
          </section>
        </form>

        <aside style={styles.preview} aria-label="任务策略预览">
          <div style={styles.previewSection}>
            <span style={styles.previewEyebrow}>Task Intent</span>
            <strong style={styles.previewTitle}>{intentInfo.label}</strong>
            <p style={styles.previewText}>{intentInfo.description}</p>
          </div>
          <div style={styles.previewGrid}>
            <span>Bridge 风险</span>
            <strong>{intentInfo.risk}</strong>
            <span>写入策略</span>
            <strong>{intentInfo.writePolicy}</strong>
            <span>推荐工具</span>
            <strong>{intentInfo.tools.join(" / ")}</strong>
            <span>审查方式</span>
            <strong>{reviewDescription}</strong>
            <span>上下文</span>
            <strong>{currentNotePath ?? "未绑定当前笔记"}</strong>
          </div>
        </aside>
      </div>
    </section>
  );
}

const styles: Record<string, React.CSSProperties> = {
  container: {
    height: "100%",
    minHeight: 0,
    overflow: "auto",
    padding: "18px",
    background: "var(--bg-primary)",
    color: "var(--text-primary)",
  },
  header: {
    display: "flex",
    alignItems: "center",
    gap: "12px",
    paddingBottom: "14px",
    borderBottom: "1px solid var(--border-color)",
  },
  titleMark: {
    width: "34px",
    height: "34px",
    display: "inline-flex",
    alignItems: "center",
    justifyContent: "center",
    border: "1px solid var(--border-color)",
    borderRadius: "7px",
    background: "var(--bg-secondary)",
    color: "var(--accent-primary)",
    flex: "0 0 auto",
  },
  titleGroup: {
    minWidth: 0,
  },
  title: {
    margin: 0,
    fontSize: "18px",
    lineHeight: 1.25,
    letterSpacing: 0,
  },
  subtitle: {
    margin: "3px 0 0",
    color: "var(--text-muted)",
    fontSize: "12px",
    lineHeight: 1.45,
  },
  layout: {
    display: "grid",
    gridTemplateColumns: "minmax(280px, 1fr) minmax(260px, 360px)",
    gap: "16px",
    paddingTop: "16px",
    alignItems: "start",
  },
  form: {
    display: "flex",
    flexDirection: "column",
    gap: "12px",
    minWidth: 0,
  },
  twoCol: {
    display: "grid",
    gridTemplateColumns: "repeat(2, minmax(0, 1fr))",
    gap: "10px",
  },
  field: {
    display: "flex",
    flexDirection: "column",
    gap: "6px",
    minWidth: 0,
  },
  label: {
    fontSize: "12px",
    color: "var(--text-secondary)",
    fontWeight: 700,
  },
  submitButton: {
    display: "inline-flex",
    alignItems: "center",
    justifyContent: "center",
    gap: "7px",
    width: "fit-content",
    minHeight: "32px",
    padding: "7px 12px",
    border: "1px solid var(--accent-primary)",
    borderRadius: "6px",
    background: "var(--accent-glow)",
    color: "var(--accent-primary)",
    cursor: "pointer",
    fontSize: "12px",
    fontWeight: 800,
  },
  status: {
    margin: 0,
    color: "var(--text-secondary)",
    fontSize: "12px",
  },
  simulationPanel: {
    display: "flex",
    flexDirection: "column",
    gap: "10px",
    paddingTop: "2px",
  },
  simulationHeader: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    gap: "8px",
  },
  simulationTitle: {
    color: "var(--text-primary)",
    fontSize: "12px",
  },
  simulationHint: {
    color: "var(--text-muted)",
    fontSize: "11px",
  },
  simulationButtons: {
    display: "grid",
    gridTemplateColumns: "repeat(2, minmax(0, 1fr))",
    gap: "8px",
  },
  simulationButton: {
    display: "flex",
    flexDirection: "column",
    alignItems: "flex-start",
    gap: "4px",
    minHeight: "54px",
    padding: "9px 10px",
    border: "1px solid var(--border-color)",
    borderRadius: "7px",
    background: "var(--bg-secondary)",
    color: "var(--text-primary)",
    cursor: "pointer",
    textAlign: "left",
  },
  simulationButtonLabel: {
    fontSize: "12px",
    fontWeight: 800,
  },
  simulationButtonMeta: {
    color: "var(--text-muted)",
    fontSize: "11px",
    lineHeight: 1.35,
  },
  preview: {
    border: "1px solid var(--border-color)",
    borderRadius: "8px",
    background: "var(--bg-secondary)",
    padding: "13px",
    display: "flex",
    flexDirection: "column",
    gap: "12px",
    minWidth: 0,
  },
  previewSection: {
    display: "flex",
    flexDirection: "column",
    gap: "4px",
  },
  previewEyebrow: {
    color: "var(--text-muted)",
    fontSize: "10px",
    textTransform: "uppercase",
    letterSpacing: 0,
  },
  previewTitle: {
    color: "var(--accent-primary)",
    fontSize: "15px",
  },
  previewText: {
    margin: 0,
    color: "var(--text-secondary)",
    fontSize: "12px",
    lineHeight: 1.5,
  },
  previewGrid: {
    display: "grid",
    gridTemplateColumns: "max-content minmax(0, 1fr)",
    gap: "8px 10px",
    fontSize: "12px",
    lineHeight: 1.45,
  },
};
