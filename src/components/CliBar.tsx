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

  const commands = useMemo(
    () => getAllCommandCompletions().map((c) => `/agent ${c.label.slice(1)}`),
    []
  );

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
        const result = await invoke<{ id: string; status: string }>("submit_task", { command: cmd });
        updateTask(taskId, { id: result.id, status: "pending" as const });
      } catch (e) {
        updateTask(taskId, { status: "error", result: String(e) });
      }
    } else {
      setTimeout(() => {
        updateTask(taskId, { status: "done", result: "[Browser mode] Command simulated" });
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

    const stringParams = Object.fromEntries(
      Object.entries(params).map(([key, value]) => [key, Array.isArray(value) ? value.join(", ") : String(value)])
    );
    const renderedPrompt = Object.entries(stringParams).reduce(
      (prompt, [key, value]) => prompt.replaceAll(`{{${key}}}`, value),
      card.promptTemplate
    );

    if (isTauri) {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        const result = await invoke<{ task_id: string; status: string }>("dispatch_agent_task", {
          payload: {
            intent: card.description || card.label,
            content: renderedPrompt,
            card_id: card.id,
            card_type: card.type,
            prompt: renderedPrompt,
            params: stringParams,
            context_note: useAppStore.getState().currentNote?.path ?? null,
            timestamp: Date.now(),
          },
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
          result: `[Browser mode] ${renderedPrompt}`,
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
          title={mode === "cards" ? "Switch to CLI mode" : "Switch to Card mode"}
          aria-label={mode === "cards" ? "Switch to CLI mode" : "Switch to Card mode"}
        >
          {mode === "cards" ? <FeroHaIcon name="LayoutGrid" size={16} /> : <FeroHaIcon name="Terminal" size={16} />}
        </button>

        {mode === "cli" ? (
          <>
            <span style={{ color: "#cba6f7", fontWeight: 700 }}>▸</span>
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
          </>
        ) : (
          <button
            style={styles.cardModeBtn}
            onClick={() => setShowCommandPanel(true)}
          >
            Click or press / to open Command Cards
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
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  container: {
    padding: "4px 12px",
    position: "relative" as const,
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
  cardModeBtn: {
    flex: 1,
    backgroundColor: "transparent",
    border: "1px dashed #45475a",
    borderRadius: "6px",
    color: "var(--text-muted)",
    fontSize: "13px",
    padding: "6px 12px",
    cursor: "pointer",
    textAlign: "left" as const,
    transition: "all 0.15s",
  },
  autocomplete: {
    position: "absolute" as const,
    bottom: "100%",
    left: "48px",
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
    color: "var(--text-primary)",
    fontFamily: "'JetBrains Mono', monospace",
    cursor: "pointer",
    borderRadius: "4px",
  },
  suggestionActive: {
    backgroundColor: "#45475a",
  },
};
