import { useState, useRef, useEffect, useMemo, KeyboardEvent } from "react";
import { useAppStore } from "../hooks/useAppStore";
import CommandCardPanel from "./CommandCardPanel";
import type { LegacyCommandCardDefinition, ParamValue } from "../types/command-card";
import { getAllCommandCompletions } from "../lib/completionSources";
import FeroHaIcon from "./FeroHaIcon";

type CommandParams = Record<string, ParamValue>;

interface CliBarProps {
  isTauri: boolean;
}

export type TaskIntentType =
  | "research"
  | "summarize"
  | "verify"
  | "dream"
  | "jsonld_index"
  | "jsonld_read"
  | "mdt_index"
  | "mdt_read"
  | "mdt_pack"
  | "write_proposal"
  | "external_import"
  | "code_assist";

export type TaskIntentSelection = "auto" | TaskIntentType;

export interface TaskIntentReviewInfo {
  value: TaskIntentType;
  label: string;
  risk: "低" | "中" | "高";
  tools: string[];
  writePolicy: string;
  expectedOutput: string;
  description: string;
}

const TASK_INTENT_REVIEW: Record<TaskIntentType, TaskIntentReviewInfo> = {
  research: {
    value: "research",
    label: "研究",
    risk: "中",
    tools: ["向量检索", "全文检索", "Web", "论文", "LLM"],
    writePolicy: "只生成 Bridge 提案",
    expectedOutput: "带来源的研究简报",
    description: "适合查找、比较和归纳资料。",
  },
  summarize: {
    value: "summarize",
    label: "总结",
    risk: "低",
    tools: ["读取笔记", "检索", "LLM"],
    writePolicy: "只生成 Bridge 提案",
    expectedOutput: "摘要提案",
    description: "适合压缩已有笔记或上下文。",
  },
  verify: {
    value: "verify",
    label: "验证",
    risk: "低",
    tools: ["读取笔记", "向量检索", "PropositionKernel"],
    writePolicy: "无源笔记写权",
    expectedOutput: "一致性验证报告",
    description: "适合检查 claim、证据链和结构一致性。",
  },
  dream: {
    value: "dream",
    label: "Dream",
    risk: "中",
    tools: ["Dream 引擎", "图谱索引"],
    writePolicy: "只写入 Dream 派生物与 Bridge 提案",
    expectedOutput: "Dream 洞察提案",
    description: "适合让记忆区块重组并产生桥接关系。",
  },
  jsonld_index: {
    value: "jsonld_index",
    label: "JSON-LD 索引",
    risk: "中",
    tools: ["读取 vault", "JSON-LD 构建器"],
    writePolicy: "只写入可重建 JSON-LD 迁移产物",
    expectedOutput: "JSON-LD graph artifacts",
    description: "适合重建 FeroHa Block Profile 的结构索引。",
  },
  jsonld_read: {
    value: "jsonld_read",
    label: "JSON-LD 读取",
    risk: "低",
    tools: ["JSON-LD Reader", "读取笔记"],
    writePolicy: "无写权",
    expectedOutput: "JSON-LD context bundle",
    description: "适合按 L0-L3 展开语义上下文。",
  },
  mdt_index: {
    value: "mdt_index",
    label: "JSON-LD 索引（MDT 兼容）",
    risk: "中",
    tools: ["JSON-LD 构建器", "MDT alias resolver"],
    writePolicy: "兼容入口；优先重建 JSON-LD 语义索引",
    expectedOutput: "JSON-LD graph artifacts",
    description: "保留旧 Markdown Tree 指令别名，但执行路径默认落到 JSON-LD 语义索引。",
  },
  mdt_read: {
    value: "mdt_read",
    label: "JSON-LD 读取（MDT 兼容）",
    risk: "低",
    tools: ["JSON-LD Reader", "MDT alias resolver"],
    writePolicy: "无写权",
    expectedOutput: "JSON-LD context bundle",
    description: "保留旧 L0-L3 读取别名，但优先使用 JSON-LD Reader。",
  },
  mdt_pack: {
    value: "mdt_pack",
    label: "MDT 归档兼容包",
    risk: "中",
    tools: ["MDT Archive", "读取 vault"],
    writePolicy: "只写入归档产物",
    expectedOutput: ".mdtz 归档包",
    description: "仅作为历史格式归档保留。",
  },
  write_proposal: {
    value: "write_proposal",
    label: "写作提案",
    risk: "中",
    tools: ["LLM", "Ghost Store"],
    writePolicy: "只写入 ghost，等待 Diff 审查",
    expectedOutput: "ghost 写作提案",
    description: "适合改写、扩写、翻译或格式化。",
  },
  external_import: {
    value: "external_import",
    label: "外部导入",
    risk: "高",
    tools: ["网络", "解析器", "Bridge"],
    writePolicy: "只写入导入暂存与 Bridge 提案",
    expectedOutput: "导入审查提案",
    description: "适合从外部来源导入并等待人工确认。",
  },
  code_assist: {
    value: "code_assist",
    label: "代码辅助",
    risk: "高",
    tools: ["受限文件读取", "输出 Hook", "Bridge"],
    writePolicy: "无直接源码写权",
    expectedOutput: "代码协助报告",
    description: "适合读取代码上下文并提出修改方案。",
  },
};

export const TASK_INTENT_OPTIONS: Array<{ value: TaskIntentType; label: string }> = [
  { value: "research", label: "研究" },
  { value: "summarize", label: "总结" },
  { value: "verify", label: "验证" },
  { value: "dream", label: "Dream" },
  { value: "jsonld_index", label: "JSON-LD 索引" },
  { value: "jsonld_read", label: "JSON-LD 读取" },
  { value: "write_proposal", label: "写作提案" },
  { value: "external_import", label: "外部导入" },
  { value: "code_assist", label: "代码辅助" },
];

export const TASK_INTENT_SELECT_OPTIONS: Array<{ value: TaskIntentSelection; label: string }> = [
  { value: "auto", label: "自动判断" },
  ...TASK_INTENT_OPTIONS,
];

export function taskIntentReviewInfo(taskType: TaskIntentType): TaskIntentReviewInfo {
  return TASK_INTENT_REVIEW[taskType];
}

export function taskIntentForCommandCard(card: Pick<LegacyCommandCardDefinition, "type">): TaskIntentType {
  switch (card.type) {
    case "summarize":
      return "summarize";
    case "dream":
      return "dream";
    case "verify":
    case "review":
    case "orchestrator-check":
      return "verify";
    case "format":
    case "rewrite":
    case "translate":
    case "expand":
    case "simplify":
    case "extract":
      return "write_proposal";
    case "deep-research":
    case "multi-search":
    case "research":
    case "search":
    case "analyze":
    case "graph-analysis":
      return "research";
    default:
      return "research";
  }
}

export function inferTaskIntentForCommand(command: string): TaskIntentType {
  const normalized = command.trim().toLowerCase().replace(/[-\s]+/g, "_");
  const tokens = new Set(command.toLowerCase().split(/[^a-z0-9]+/).filter(Boolean));
  const hasAny = (...values: string[]) => values.some((value) => tokens.has(value) || normalized.includes(value));

  if (hasAny("dream", "dream_cycle")) return "dream";
  if (hasAny("verify", "verification", "review", "check_claim")) return "verify";
  if (hasAny("mdt_pack", "pack_mdt", "archive")) return "mdt_pack";
  if (hasAny("jsonld_read", "json_ld_read", "memory_read", "mdt_read", "read_mdt")) return "jsonld_read";
  if (hasAny("jsonld_index", "json_ld_index", "memory_index", "build_graph", "mdt_index", "index_mdt")) {
    return "jsonld_index";
  }
  if (hasAny("import", "external_import", "fetch_url")) return "external_import";
  if (hasAny("code", "code_assist", "patch", "refactor")) return "code_assist";
  if (hasAny("rewrite", "correct", "expand", "translate", "simplify", "format", "extract", "write_proposal")) {
    return "write_proposal";
  }
  if (hasAny("summary", "summarize")) return "summarize";
  return "research";
}

export function resolveTaskIntentSelectionForCommand(
  selection: TaskIntentSelection,
  command: string
): TaskIntentType {
  return selection === "auto" ? inferTaskIntentForCommand(command) : selection;
}

export function resolveTaskIntentSelectionForCard(
  selection: TaskIntentSelection,
  card: Pick<LegacyCommandCardDefinition, "type">
): TaskIntentType {
  return selection === "auto" ? taskIntentForCommandCard(card) : selection;
}

export function buildSubmitTaskArgs(command: string, taskType: TaskIntentType) {
  return {
    command,
    taskType,
  };
}

export function stringifyCommandParams(params: CommandParams): Record<string, string> {
  return Object.fromEntries(
    Object.entries(params).map(([key, value]) => [
      key,
      Array.isArray(value) ? value.join(", ") : String(value),
    ])
  );
}

export function renderCommandCardPrompt(
  card: LegacyCommandCardDefinition,
  stringParams: Record<string, string>
): string {
  return Object.entries(stringParams).reduce(
    (prompt, [key, value]) => prompt.replaceAll(`{{${key}}}`, value),
    card.promptTemplate
  );
}

export function buildCommandCardDispatchPayload({
  card,
  params,
  renderedPrompt,
  taskType,
  contextNote,
  timestamp,
}: {
  card: LegacyCommandCardDefinition;
  params: Record<string, string>;
  renderedPrompt: string;
  taskType: TaskIntentType;
  contextNote: string | null;
  timestamp: number;
}) {
  return {
    intent: card.description || card.label,
    content: renderedPrompt,
    card_id: card.id,
    card_type: card.type,
    task_type: taskType,
    prompt: renderedPrompt,
    params,
    context_note: contextNote,
    timestamp,
  };
}

function isInputFocused() {
  const el = document.activeElement;
  return el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement;
}

/**
 * CLI Bar — Global AI command input
 * Activated by typing `/` anywhere in the app.
 * Dispatches commands to Rust backend via Tauri IPC.
 */
export default function CliBar({ isTauri }: CliBarProps) {
  const [active, setActive] = useState(false);
  const [input, setInput] = useState("");
  const [, setHistory] = useState<string[]>([]);
  const addTask = useAppStore((s) => s.addTask);
  const updateTask = useAppStore((s) => s.updateTask);
  const [suggestions, setSuggestions] = useState<string[]>([]);
  const inputRef = useRef<HTMLInputElement>(null);
  const [cursorIdx, setCursorIdx] = useState(0);
  const [mode, setMode] = useState<"cards" | "cli">("cards");
  const [showCommandPanel, setShowCommandPanel] = useState(false);
  const [selectedTaskType, setSelectedTaskType] = useState<TaskIntentSelection>("auto");

  const commands = useMemo(
    () => getAllCommandCompletions().map((c) => `/agent ${c.label.slice(1)}`),
    []
  );
  const previewTaskType = useMemo(
    () => resolveTaskIntentSelectionForCommand(selectedTaskType, input),
    [input, selectedTaskType]
  );
  const previewInfo = taskIntentReviewInfo(previewTaskType);
  const intentPreviewLabel =
    selectedTaskType === "auto" && mode === "cards" ? "自动 · 按指令卡判断" :
    selectedTaskType === "auto" ? `自动 · ${previewInfo.label}` :
    previewInfo.label;

  // Global `/` key listener
  useEffect(() => {
    const handler = (e: globalThis.KeyboardEvent) => {
      if (e.key === "/" && !active && !isInputFocused()) {
        e.preventDefault();
        if (mode === "cards") {
          setShowCommandPanel(true);
        } else {
          setActive(true);
          setInput("/");
          setTimeout(() => inputRef.current?.focus(), 0);
        }
      }
      if (e.key === "Escape" && active) {
        setActive(false);
        setInput("");
        setSuggestions([]);
      }
      if (e.key === "Escape" && showCommandPanel) {
        setShowCommandPanel(false);
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [active, mode, showCommandPanel]);

  const handleInput = (value: string) => {
    setInput(value);
    if (value.startsWith("/")) {
      const matches = commands.filter((c) => c.toLowerCase().includes(value.toLowerCase()));
      setSuggestions(matches);
      setCursorIdx(0);
    } else {
      setSuggestions([]);
    }
  };

  const handleKeyDown = (e: KeyboardEvent) => {
    if (e.key === "Tab" && suggestions.length > 0) {
      e.preventDefault();
      setInput(suggestions[cursorIdx % suggestions.length]);
      setSuggestions([]);
    }
    if (e.key === "ArrowDown") {
      setCursorIdx((i) => (i + 1) % Math.max(suggestions.length, 1));
    }
    if (e.key === "ArrowUp") {
      setCursorIdx((i) => (i - 1 + suggestions.length) % Math.max(suggestions.length, 1));
    }
    if (e.key === "Enter" && input.trim()) {
      e.preventDefault();
      executeCommand(input.trim());
    }
  };

  const executeCommand = async (cmd: string) => {
    const taskId = `task_${Date.now()}`;
    addTask({ id: taskId, command: cmd, status: "pending" });
    setHistory((prev) => [...prev, cmd]);
    setInput("");
    setActive(false);

    if (isTauri) {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        const taskType = resolveTaskIntentSelectionForCommand(selectedTaskType, cmd);
        const result = await invoke<{ id: string; status: string }>(
          "submit_task",
          buildSubmitTaskArgs(cmd, taskType)
        );
        updateTask(taskId, { id: result.id, status: "pending" as const });
      } catch (e) {
        updateTask(taskId, { status: "error", result: String(e) });
      }
    } else {
      setTimeout(() => {
        updateTask(taskId, { status: "done", result: "[浏览器预览] 指令已模拟执行" });
      }, 1500);
    }
  };

  // Listen for task-updated events from backend
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
            contextNote: useAppStore.getState().currentNote?.path ?? null,
            timestamp: Date.now(),
          }),
        });
        updateTask(localTaskId, {
          id: result.task_id,
          status: result.status === "researching" ? "approved" : "pending",
        });
      } catch (e) {
        updateTask(localTaskId, { status: "error", result: String(e) });
      }
    } else {
      setTimeout(() => {
        updateTask(localTaskId, {
          status: "done",
          result: `[浏览器预览] ${renderedPrompt}`,
        });
      }, 500);
    }
  };

  const toggleMode = () => {
    setMode((prev) => (prev === "cards" ? "cli" : "cards"));
  };

  return (
    <div style={styles.container}>
      {/* Command Card Panel */}
      <CommandCardPanel
        onExecute={handleCommandCardExecute}
        isTauri={isTauri}
        isOpen={showCommandPanel}
        onClose={() => setShowCommandPanel(false)}
      />

      {/* Mode toggle and input */}
      <div style={styles.inputRow}>
        {/* Mode toggle button */}
        <button
          style={styles.modeToggle}
          onClick={toggleMode}
          title={mode === "cards" ? "切换到 CLI 模式" : "切换到指令卡模式"}
          aria-label={mode === "cards" ? "切换到 CLI 模式" : "切换到指令卡模式"}
        >
          {mode === "cards" ? <FeroHaIcon name="LayoutGrid" size={16} /> : <FeroHaIcon name="Terminal" size={16} />}
        </button>

        <select
          style={styles.taskTypeSelect}
          value={selectedTaskType}
          onChange={(event) => setSelectedTaskType(event.target.value as TaskIntentSelection)}
          title="任务类型"
          aria-label="任务类型"
        >
          {TASK_INTENT_SELECT_OPTIONS.map((option) => (
            <option key={option.value} value={option.value}>
              {option.label}
            </option>
          ))}
        </select>

        {mode === "cli" ? (
          <>
            <span style={styles.promptGlyph}>&gt;</span>
            <input
              ref={inputRef}
              style={styles.input}
              value={active ? input : ""}
              onChange={(e) => handleInput(e.target.value)}
              onKeyDown={handleKeyDown}
              onFocus={() => setActive(true)}
              onBlur={() => !input && setActive(false)}
              placeholder={active ? "" : "输入 / 调用 AI 指令..."}
            />
          </>
        ) : (
          <button
            style={styles.cardModeBtn}
            onClick={() => setShowCommandPanel(true)}
          >
            打开指令卡（/）
          </button>
        )}

        {/* Autocomplete suggestions (CLI mode only) */}
        {mode === "cli" && suggestions.length > 0 && (
          <div style={styles.autocomplete}>
            {suggestions.slice(0, 8).map((s, i) => (
              <div
                key={s}
                style={{
                  ...styles.suggestionItem,
                  ...(i === cursorIdx ? styles.suggestionActive : {}),
                }}
                onMouseDown={() => {
                  setInput(s);
                  setSuggestions([]);
                }}
              >
                {s}
              </div>
            ))}
          </div>
        )}
      </div>

      <div style={styles.intentReview} aria-label="任务类型审查">
        <span style={styles.intentChip}>
          {intentPreviewLabel}
        </span>
        <span style={styles.intentMeta}>Bridge 风险：{previewInfo.risk}</span>
        <span style={styles.intentMeta}>写权：{previewInfo.writePolicy}</span>
        <span style={styles.intentMeta}>工具：{previewInfo.tools.join(" / ")}</span>
        <span style={styles.intentMeta}>产物：{previewInfo.expectedOutput}</span>
      </div>
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  container: {
    padding: "4px 12px",
    position: "relative" as const,
  },
  taskTypeSelect: {
    width: "132px",
    minWidth: "118px",
    backgroundColor: "var(--bg-input)",
    border: "1px solid var(--border-color)",
    borderRadius: "6px",
    color: "var(--text-primary)",
    fontSize: "12px",
    padding: "6px 8px",
    outline: "none",
  },
  inputRow: {
    display: "flex",
    alignItems: "center",
    gap: "8px",
    position: "relative" as const,
  },
  input: {
    flex: 1,
    backgroundColor: "transparent",
    border: "none",
    outline: "none",
    color: "var(--text-primary)",
    fontSize: "13px",
    fontFamily: "'JetBrains Mono', 'Fira Code', monospace",
    padding: "6px 0",
  },
  promptGlyph: {
    color: "var(--accent-primary)",
    fontWeight: 700,
  },
  cardModeBtn: {
    flex: 1,
    backgroundColor: "transparent",
    border: "1px dashed var(--border-color)",
    borderRadius: "6px",
    color: "var(--text-muted)",
    fontSize: "13px",
    padding: "6px 12px",
    cursor: "pointer",
    textAlign: "left" as const,
    transition: "all 0.15s",
  },
  intentReview: {
    display: "flex",
    alignItems: "center",
    gap: "6px",
    flexWrap: "wrap",
    marginTop: "5px",
    paddingLeft: "40px",
    color: "var(--text-muted)",
    fontSize: "11px",
    lineHeight: 1.45,
  },
  intentChip: {
    display: "inline-flex",
    alignItems: "center",
    minHeight: "20px",
    padding: "1px 8px",
    border: "1px solid var(--border-color)",
    borderRadius: "999px",
    color: "var(--accent-primary)",
    backgroundColor: "var(--bg-input)",
    fontWeight: 700,
  },
  intentMeta: {
    display: "inline-flex",
    alignItems: "center",
    minHeight: "20px",
    padding: "1px 7px",
    border: "1px solid var(--border-muted)",
    borderRadius: "5px",
    backgroundColor: "var(--bg-secondary)",
  },
  autocomplete: {
    position: "absolute" as const,
    bottom: "100%",
    left: "48px",
    backgroundColor: "var(--bg-secondary)",
    border: "1px solid var(--border-color)",
    borderRadius: "6px",
    padding: "4px",
    minWidth: "300px",
    zIndex: 100,
  },
  suggestionItem: {
    padding: "4px 8px",
    fontSize: "12px",
    color: "var(--text-primary)",
    fontFamily: "'JetBrains Mono', monospace",
    cursor: "pointer",
    borderRadius: "4px",
  },
  suggestionActive: {
    backgroundColor: "var(--bg-hover)",
  },
};
