import { useState, useRef, useEffect, KeyboardEvent } from "react";
import { useAppStore, TaskStatus } from "../hooks/useAppStore";

interface CliBarProps {
  isTauri: boolean;
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
  const [tasks, setTasks] = useState<TaskStatus[]>([]);
  const [suggestions, setSuggestions] = useState<string[]>([]);
  const inputRef = useRef<HTMLInputElement>(null);
  const [cursorIdx, setCursorIdx] = useState(0);

  // Available commands (autocomplete)
  const commands = [
    "/agent search --query ",
    "/agent summarize --target ",
    "/agent fetch-papers --topic ",
    "/agent deep-dive ",
    "/agent explain ",
    "/agent diff --review",
    "/agent status",
    "/agent config --model ",
  ];

  // Global `/` key listener
  useEffect(() => {
    const handler = (e: globalThis.KeyboardEvent) => {
      if (e.key === "/" && !active && !isInputFocused()) {
        e.preventDefault();
        setActive(true);
        setInput("/");
        setTimeout(() => inputRef.current?.focus(), 0);
      }
      if (e.key === "Escape" && active) {
        setActive(false);
        setInput("");
        setSuggestions([]);
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [active]);

  const isInputFocused = () => {
    const el = document.activeElement;
    return el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement;
  };

  const handleInput = (value: string) => {
    setInput(value);
    if (value.startsWith("/")) {
      const matches = commands.filter((c) => c.startsWith(value));
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
    setTasks((prev) => [
      ...prev,
      { id: taskId, command: cmd, status: "running" },
    ]);
    setHistory((prev) => [...prev, cmd]);
    setInput("");
    setActive(false);

    if (isTauri) {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        const result = await invoke<string>("execute_cli", { command: cmd });
        setTasks((prev) =>
          prev.map((t) =>
            t.id === taskId ? { ...t, status: "done", result } : t
          )
        );
      } catch (e) {
        setTasks((prev) =>
          prev.map((t) =>
            t.id === taskId
              ? { ...t, status: "error", result: String(e) }
              : t
          )
        );
      }
    } else {
      // Browser mock
      setTimeout(() => {
        setTasks((prev) =>
          prev.map((t) =>
            t.id === taskId
              ? { ...t, status: "done", result: "[Browser mode] Command simulated" }
              : t
          )
        );
      }, 1500);
    }
  };

  return (
    <div style={styles.container}>
      {/* Task status indicators */}
      {tasks.length > 0 && (
        <div style={styles.taskList}>
          {tasks.slice(-5).map((task) => (
            <div key={task.id} style={styles.taskItem}>
              <span style={{
                ...styles.taskDot,
                backgroundColor: task.status === "running" ? "#f9e2af"
                  : task.status === "done" ? "#a6e3a1" : "#f38ba8",
              }} />
              <span style={styles.taskCmd}>{task.command}</span>
              {task.result && (
                <span style={styles.taskResult}>{task.result}</span>
              )}
            </div>
          ))}
        </div>
      )}

      {/* CLI input */}
      <div style={styles.inputRow}>
        <span style={styles.prompt}>❯</span>
        <input
          ref={inputRef}
          style={styles.input}
          value={active ? input : ""}
          onChange={(e) => handleInput(e.target.value)}
          onKeyDown={handleKeyDown}
          onFocus={() => setActive(true)}
          onBlur={() => !input && setActive(false)}
          placeholder={active ? "" : "Press / for AI commands..."}
        />

        {/* Autocomplete suggestions */}
        {suggestions.length > 0 && (
          <div style={styles.autocomplete}>
            {suggestions.slice(0, 5).map((s, i) => (
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
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  container: {
    padding: "4px 12px",
    position: "relative" as const,
  },
  taskList: {
    display: "flex",
    flexDirection: "column",
    gap: "2px",
    marginBottom: "4px",
    maxHeight: "60px",
    overflow: "auto",
  },
  taskItem: {
    display: "flex",
    alignItems: "center",
    gap: "6px",
    fontSize: "11px",
    color: "#a6adc8",
  },
  taskDot: {
    width: "6px",
    height: "6px",
    borderRadius: "50%",
    flexShrink: 0,
  },
  taskCmd: {
    color: "#bac2de",
  },
  taskResult: {
    color: "#6c7086",
    fontSize: "10px",
  },
  inputRow: {
    display: "flex",
    alignItems: "center",
    gap: "8px",
    position: "relative" as const,
  },
  prompt: {
    color: "#cba6f7",
    fontWeight: 700,
    fontSize: "13px",
  },
  input: {
    flex: 1,
    backgroundColor: "transparent",
    border: "none",
    outline: "none",
    color: "#cdd6f4",
    fontSize: "13px",
    fontFamily: "'JetBrains Mono', 'Fira Code', monospace",
    padding: "6px 0",
  },
  autocomplete: {
    position: "absolute" as const,
    bottom: "100%",
    left: "24px",
    backgroundColor: "#313244",
    border: "1px solid #45475a",
    borderRadius: "6px",
    padding: "4px",
    minWidth: "300px",
    zIndex: 100,
  },
  suggestionItem: {
    padding: "4px 8px",
    fontSize: "12px",
    color: "#cdd6f4",
    fontFamily: "'JetBrains Mono', monospace",
    cursor: "pointer",
    borderRadius: "4px",
  },
  suggestionActive: {
    backgroundColor: "#45475a",
  },
};
