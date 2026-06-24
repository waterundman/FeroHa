import { useState, useRef, useEffect, useCallback, useMemo } from "react";
import FeroHaIcon from "./FeroHaIcon";
import "./CliMiniWindow.css";

interface CliOutputLine {
  id: string;
  type: "prompt" | "response" | "error" | "loading";
  text: string;
  timestamp: number;
}

interface CliMiniWindowProps {
  vaultPath: string;
  isTauri: boolean;
}

const CLS_KEY = "feroha-cli-history";
const MAX_HISTORY = 50;
const MIN_W = 300; const MIN_H = 200;
const MAX_W = 800; const MAX_H = 600;
const INITIAL_W = 420; const INITIAL_H = 480;
const DEFAULT_POS = { x: 0, y: 0 };
const INITIAL_PINNED = { x: 0, y: 0 };

function loadHistory(): string[] {
  try {
    const raw = localStorage.getItem(CLS_KEY);
    return raw ? JSON.parse(raw) : [];
  } catch { return []; }
}

function saveHistory(h: string[]) {
  try {
    const trimmed = h.slice(-MAX_HISTORY);
    localStorage.setItem(CLS_KEY, JSON.stringify(trimmed));
  } catch { /* quota exceeded */ }
}

function simpleMarkdown(text: string): string {
  let out = text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
  out = out.replace(/^### (.+)$/gm, "<h3>$1</h3>");
  out = out.replace(/^## (.+)$/gm, "<h2>$1</h2>");
  out = out.replace(/^# (.+)$/gm, "<h1>$1</h1>");
  out = out.replace(/\*\*(.+?)\*\*/g, "<strong>$1</strong>");
  out = out.replace(/\*(.+?)\*/g, "<em>$1</em>");
  out = out.replace(/`([^`]+)`/g, "<code>$1</code>");
  out = out.replace(/^- (.+)$/gm, "<li>$1</li>");
  out = out.replace(/(<li>.*<\/li>)/gs, (match) => `<ul>${match}</ul>`);
  out = out.replace(/\n/g, "<br>");
  return out;
}

function useLocalStore<T>(key: string, fallback: T): [T, (v: T | ((prev: T) => T)) => void] {
  const [value, setValue] = useState<T>(() => {
    try {
      const raw = localStorage.getItem(key);
      return raw ? JSON.parse(raw) : fallback;
    } catch { return fallback; }
  });
  useEffect(() => {
    try { localStorage.setItem(key, JSON.stringify(value)); } catch { /* */ }
  }, [key, value]);
  return [value, setValue];
}

export default function CliMiniWindow({ vaultPath: _vaultPath, isTauri }: CliMiniWindowProps) {
  const [isOpen, setIsOpen] = useState(false);
  const [isMinimized, setIsMinimized] = useState(false);
  const [output, setOutput] = useState<CliOutputLine[]>([]);
  const [inputValue, setInputValue] = useState("");
  const [historyIndex, setHistoryIndex] = useState(-1);

  const [position, setPosition] = useLocalStore("feroha-cli-window-pos", DEFAULT_POS);
  const [size, setSize] = useLocalStore("feroha-cli-window-size", { width: INITIAL_W, height: INITIAL_H });
  const [pinned, setPinned] = useLocalStore("feroha-cli-window-pinned", INITIAL_PINNED);
  const [pinActive, setPinActive] = useState(false);

  const inputRef = useRef<HTMLTextAreaElement>(null);
  const outputRef = useRef<HTMLDivElement>(null);
  const dragging = useRef<{ startX: number; startY: number; offsetX: number; offsetY: number } | null>(null);
  const resizing = useRef<{ startX: number; startY: number; startW: number; startH: number } | null>(null);
  const startedNearEdge = useRef(false);
  const [isExecuting, setIsExecuting] = useState(false);

  const commandHistory = useMemo(() => loadHistory(), [isOpen]);

  // Pin: persist window position when pinned
  useEffect(() => {
    if (pinActive) {
      setPinned(position);
    }
  }, [pinActive, position, setPinned]);

  // Auto-scroll output
  useEffect(() => {
    if (outputRef.current) {
      outputRef.current.scrollTop = outputRef.current.scrollHeight;
    }
  }, [output]);

  // Ctrl+` toggle
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.ctrlKey && !e.shiftKey && !e.altKey && e.key === "`") {
        e.preventDefault();
        setIsOpen((prev) => !prev);
      }
      if (e.key === "Escape" && isOpen && !isMinimized) {
        if (document.activeElement === inputRef.current) {
          setInputValue("");
        } else {
          setIsMinimized(true);
        }
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [isOpen, isMinimized]);

  // Dragging
  const onDragStart = useCallback((e: React.MouseEvent) => {
    if (e.button !== 0) return;
    initializedPosition();
    dragging.current = {
      startX: e.clientX,
      startY: e.clientY,
      offsetX: position.x,
      offsetY: position.y,
    };
    startedNearEdge.current = true;
  }, [position]);

  const onResizeStart = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    if (e.button !== 0) return;
    resizing.current = {
      startX: e.clientX,
      startY: e.clientY,
      startW: size.width,
      startH: size.height,
    };
    startedNearEdge.current = true;
  }, [size]);

  useEffect(() => {
    if (!startedNearEdge.current) return;
    const onMove = (e: MouseEvent) => {
      if (dragging.current) {
        const dx = e.clientX - dragging.current.startX;
        const dy = e.clientY - dragging.current.startY;
        setPosition({
          x: dragging.current.offsetX + dx,
          y: dragging.current.offsetY + dy,
        });
      }
      if (resizing.current) {
        const dx = e.clientX - resizing.current.startX;
        const dy = e.clientY - resizing.current.startY;
        setSize({
          width: Math.min(MAX_W, Math.max(MIN_W, resizing.current.startW + dx)),
          height: Math.min(MAX_H, Math.max(MIN_H, resizing.current.startH + dy)),
        });
      }
    };
    const onUp = () => {
      dragging.current = null;
      resizing.current = null;
      startedNearEdge.current = false;
    };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
    return () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    };
  }, [setPosition, setSize]);

  function initializedPosition() {
    const el = document.getElementById("feroha-cli-window");
    if (el && position.x === DEFAULT_POS.x && position.y === DEFAULT_POS.y) {
      const rect = el.getBoundingClientRect();
      const x = window.innerWidth - rect.width - 20;
      const y = window.innerHeight - rect.height - 80;
      setPosition({ x, y });
    }
  }

  const addOutputLine = useCallback((line: CliOutputLine) => {
    setOutput((prev) => [...prev, line]);
  }, []);

  const executeCommand = useCallback(async (cmd: string) => {
    const trimmed = cmd.trim();
    if (!trimmed) return;

    const promptId = `p_${Date.now()}`;
    addOutputLine({ id: promptId, type: "prompt", text: trimmed, timestamp: Date.now() });

    const loadingId = `l_${Date.now()}`;
    addOutputLine({ id: loadingId, type: "loading", text: "⏳ Running...", timestamp: Date.now() });

    setIsExecuting(true);
    setInputValue("");
    setHistoryIndex(-1);

    const updatedHistory = [...commandHistory, trimmed];
    saveHistory(updatedHistory);

    if (!isTauri) {
      setOutput((prev) =>
        prev.filter((l) => l.id !== loadingId).concat({
          id: `r_${Date.now()}`,
          type: "response",
          text: `## 浏览器预览\n\nCLI 命令已模拟执行：\`${trimmed}\`\n\n真实 \`execute_cli\` 只在 Tauri 应用中可用。`,
          timestamp: Date.now(),
        })
      );
      setIsExecuting(false);
      return;
    }

    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const resultJson = await invoke<string>("execute_cli", { command: trimmed });
      let parsed: { task_id?: string; status?: string; message?: string } = {};
      try { parsed = JSON.parse(resultJson); } catch { parsed = { message: resultJson }; }

      const taskId = parsed.task_id || "unknown";
      setOutput((prev) =>
        prev.filter((l) => l.id !== loadingId).concat({
          id: `r_${Date.now()}`,
          type: "response",
          text: `## Task submitted\n\n**Status**: ${parsed.status || "pending"}\n**Task ID**: \`${taskId}\`\n${parsed.message || ""}`,
          timestamp: Date.now(),
        })
      );

      // Poll for task completion via task-updated events
      try {
        const { listen } = await import("@tauri-apps/api/event");
        const unlisten = await listen<{ task_id: string; status: string; result?: string }>(
          "task-updated",
          (event) => {
            if (event.payload.task_id === taskId) {
              const { status: tStatus, result: tResult } = event.payload;
              if (tStatus === "done" && tResult) {
                addOutputLine({
                  id: `tr_${Date.now()}`,
                  type: "response",
                  text: tResult,
                  timestamp: Date.now(),
                });
                unlisten();
              } else if (tStatus === "error") {
                addOutputLine({
                  id: `tr_${Date.now()}`,
                  type: "error",
                  text: `## Error\n\n${tResult || "Task failed"}`,
                  timestamp: Date.now(),
                });
                unlisten();
              }
            }
          }
        );

        // Timeout after 120s
        setTimeout(() => {
          unlisten();
        }, 120000);
      } catch {
        // Listen setup failed; result already shown
      }
    } catch (e) {
      setOutput((prev) =>
        prev.filter((l) => l.id !== loadingId).concat({
          id: `e_${Date.now()}`,
          type: "error",
          text: `## Execution error\n\n\`\`\`\n${String(e)}\n\`\`\``,
          timestamp: Date.now(),
        })
      );
    } finally {
      setIsExecuting(false);
    }
  }, [commandHistory, addOutputLine, isTauri]);

  const handleInputKeyDown = useCallback((e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      if (inputValue.trim()) {
        executeCommand(inputValue);
      }
    } else if (e.key === "Enter" && e.shiftKey) {
      return;
    } else if (e.key === "ArrowUp" && !inputValue) {
      e.preventDefault();
      const newIdx = historyIndex + 1;
      if (newIdx < commandHistory.length) {
        setHistoryIndex(newIdx);
        setInputValue(commandHistory[commandHistory.length - 1 - newIdx]);
      }
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      if (historyIndex <= 0) {
        setHistoryIndex(-1);
        setInputValue("");
      } else {
        const newIdx = historyIndex - 1;
        setHistoryIndex(newIdx);
        setInputValue(commandHistory[commandHistory.length - 1 - newIdx]);
      }
    }
  }, [inputValue, historyIndex, commandHistory, executeCommand]);

  // Trigger button
  if (!isOpen) {
    return (
      <button
        className="cli-trigger"
        onClick={() => { setIsOpen(true); setIsMinimized(false); }}
        title="打开 CLI（Ctrl+`）"
        aria-label="打开 CLI 浮窗"
      >
        <FeroHaIcon name="Terminal" size={20} />
      </button>
    );
  }

  const windowStyle: React.CSSProperties = pinActive
    ? { position: "absolute" as const, left: pinned.x, top: pinned.y, width: size.width, height: size.height }
    : { position: "fixed" as const, left: position.x, top: position.y, width: size.width, height: size.height };

  if (!position.x && !position.y && !pinActive) {
    windowStyle.bottom = "80px";
    windowStyle.right = "20px";
    windowStyle.left = "auto";
    windowStyle.top = "auto";
  }

  return (
    <div
      id="feroha-cli-window"
      className={`cli-window${isMinimized ? " cli-minimized" : ""}`}
      style={windowStyle}
    >
      <div className="cli-header" onMouseDown={onDragStart}>
        <span className="cli-header-title">
          <FeroHaIcon name="Terminal" size={14} />
          <span>CLI</span>
        </span>
        <div className="cli-header-actions">
          <button
            className="cli-header-btn"
            onClick={() => setPinActive((p) => !p)}
            title={pinActive ? "Unpin window" : "Pin window"}
          >
            <FeroHaIcon name={pinActive ? "Pin" : "PinOff"} size={12} />
          </button>
          <button
            className="cli-header-btn"
            onClick={() => setIsMinimized(true)}
            title="最小化"
          >
            <FeroHaIcon name="Minus" size={12} />
          </button>
          <button
            className="cli-header-btn"
            onClick={() => { setIsOpen(false); setIsMinimized(false); }}
            title="关闭"
          >
            <FeroHaIcon name="X" size={12} />
          </button>
        </div>
      </div>

      {!isMinimized && (
        <>
          <div className="cli-output" ref={outputRef}>
            {output.length === 0 && (
              <div className="cli-output-empty">
                <FeroHaIcon name="Terminal" size={32} />
                <p>输入 <code>/agent ...</code> 开始</p>
                <div className="cli-shortcut-hints">
                  <span><kbd>Enter</kbd> 执行</span>
                  <span><kbd>Shift+Enter</kbd> 新行</span>
                  <span><kbd>↑</kbd><kbd>↓</kbd> 历史</span>
                  <span><kbd>Esc</kbd> 最小化</span>
                </div>
              </div>
            )}
            {output.map((line) => (
              <div key={line.id} className={`cli-output-line cli-output-${line.type}`}>
                {line.type === "prompt" && <span className="cli-prompt-arrow">&gt;</span>}
                {line.type === "loading" ? (
                  <span className="cli-loading">{line.text}</span>
                ) : (
                  <span dangerouslySetInnerHTML={{ __html: simpleMarkdown(line.text) }} />
                )}
              </div>
            ))}
          </div>

          <div className="cli-input-area">
            <span className="cli-input-prompt">&gt;</span>
            <textarea
              ref={inputRef}
              className="cli-input feroha-textarea"
              value={inputValue}
              onChange={(e) => { setInputValue(e.target.value); setHistoryIndex(-1); }}
              onKeyDown={handleInputKeyDown}
              placeholder="/agent ..."
              rows={1}
              disabled={isExecuting}
            />
          </div>

          <div className="cli-resize-handle feroha-resize-handle" onMouseDown={onResizeStart} />
        </>
      )}
    </div>
  );
}
