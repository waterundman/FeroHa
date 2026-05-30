import { useEffect, useState, useCallback, useRef } from "react";
import { Group, Panel, Separator } from "react-resizable-panels";
import { useAppStore, type NoteMeta, type GraphData } from "./hooks/useAppStore";
import Editor from "./components/Editor";
import TabBar from "./components/TabBar";
import VaultBrowser from "./components/VaultBrowser";
import GraphView from "./components/GraphView";
import DiffView from "./components/DiffView";
import CliBar from "./components/CliBar";
import CliMiniWindow from "./components/CliMiniWindow";
import ModeToggle from "./components/ModeToggle";
import SettingsPanel from "./components/SettingsPanel";
import BacklinksPanel from "./components/BacklinksPanel";
import TagsPanel from "./components/TagsPanel";
import StatusBar from "./components/StatusBar";
import OrchestratorPanel from "./components/OrchestratorPanel";
import QuickSwitcher from "./components/QuickSwitcher";
import FeroHaIcon from "./components/FeroHaIcon";
import AgentDashboard from "./components/AgentDashboard";
import InspirationCanvas from "./components/InspirationCanvas";
import CommandCardLibrary from "./components/CommandCardLibrary";
import PipelineEditor from "./components/PipelineEditor";
import PluginSettings from "./components/PluginSettings";
import BridgeInbox from "./components/BridgeInbox";
import { ToastContainer } from "./components/Toast";
import { showToast } from "./components/toastBus";
import { listenForResearchCompletion, type ResearchCompletedPayload } from "./lib/ipc";
import { pipelineEngine, type PipelineDefinition } from "./lib/commandCardPipeline";
import { useKeyboardShortcuts } from "./hooks/useKeyboardShortcuts";
import { ShortcutHelpModal } from "./components/ShortcutTooltip";
import { useSettings, loadSettingsFromBackend } from "./hooks/useSettings";
import "./styles/feroha-theme.css";

interface SnapshotDriftPayload {
  snapshot_type?: "global" | "local" | string;
  note_id?: string;
  avg_cosine_distance?: number;
}

function hasTauriRuntime() {
  return typeof window !== "undefined" && Boolean(window.__TAURI_INTERNALS__);
}

function setupSnapshotDriftListener() {
  let unlisten: (() => void) | undefined;
  (async () => {
    try {
      const { listen } = await import("@tauri-apps/api/event");
      unlisten = await listen<SnapshotDriftPayload>("snapshot-drift", (event) => {
        const drift = event.payload;
        const typeLabel = drift.snapshot_type === "global" ? "Global" : "Local";
        const distance = drift.avg_cosine_distance;
        showToast(
          "warning",
          `${typeLabel} drift detected in "${drift.note_id}": avg distance ${
            typeof distance === "number" ? distance.toFixed(3) : "n/a"
          }`
        );
      });
    } catch {
      // Browser mode has no Tauri event bus.
    }
  })();
  return () => {
    unlisten?.();
  };
}

function setupTaskEventListeners() {
  const unlistens: (() => void)[] = [];
  (async () => {
    try {
      const { listen } = await import("@tauri-apps/api/event");

      const u1 = await listen<{ task_id: string }>("task-checked", (event) => {
        showToast("info", `Task ${event.payload.task_id} approved`);
      });
      unlistens.push(u1);

      const u2 = await listen<{ type: string; query: string }>("agent-result", (event) => {
        const p = event.payload;
        showToast("success", `AI result ready: ${p.type} — "${p.query?.slice(0, 50) || ''}..."`);
      });
      unlistens.push(u2);

      const u3 = await listen<{ action: string; block_ids?: string[] }>("feroha_feedback_recorded", (event) => {
        const p = event.payload;
        showToast("info", `Feedback recorded: ${p.action} on ${p.block_ids?.length || 0} blocks`);
      });
      unlistens.push(u3);
    } catch {
      // Browser mode has no Tauri event bus.
    }
  })();
  return () => { unlistens.forEach(fn => fn?.()); };
}

export default function App() {
  const [isTauri] = useState(hasTauriRuntime);
  const [backendStatus, setBackendStatus] = useState(() =>
    hasTauriRuntime() ? "Initializing..." : "Browser mode - Tauri not detected"
  );
  const [showSettings, setShowSettings] = useState(false);
  const [showShortcutHelp, setShowShortcutHelp] = useState(false);
  const [settings] = useSettings();
  const mode = useAppStore((s) => s.mode);
  const [isModeTransitioning, setIsModeTransitioning] = useState(false);
  const prevModeRef = useRef(mode);

  useEffect(() => {
    if (prevModeRef.current === mode) return;
    prevModeRef.current = mode;
    setIsModeTransitioning(true);
    const timer = setTimeout(() => setIsModeTransitioning(false), 150);
    return () => clearTimeout(timer);
  }, [mode]);

  useEffect(() => {
    document.documentElement.setAttribute("data-theme", settings.theme);
  }, [settings.theme]);

  const setVaultPath = useAppStore((s) => s.setVaultPath);
  const setNotes = useAppStore((s) => s.setNotes);
  const setGraph = useAppStore((s) => s.setGraph);
  const vaultPath = useAppStore((s) => s.vaultPath);
  const currentNote = useAppStore((s) => s.currentNote);
  const activePanel = useAppStore((s) => s.activePanel);
  const sidebarCollapsed = useAppStore((s) => s.sidebarCollapsed);
  const setSidebarCollapsed = useAppStore((s) => s.setSidebarCollapsed);
  const tabs = useAppStore((s) => s.tabs);
  const activeTabIndex = useAppStore((s) => s.activeTabIndex);
  const splitActive = useAppStore((s) => s.splitActive);
  const splitDirection = useAppStore((s) => s.splitDirection);
  const splitTabIndex = useAppStore((s) => s.splitTabIndex);

  const activeTab = tabs[activeTabIndex];
  const splitTab = splitActive && splitTabIndex >= 0 && splitTabIndex < tabs.length ? tabs[splitTabIndex] : undefined;

  const primaryNoteProps = activeTab ? { path: activeTab.path, content: activeTab.content, isDirty: activeTab.isDirty } : undefined;
  const splitNoteProps = splitTab ? { path: splitTab.path, content: splitTab.content, isDirty: splitTab.isDirty } : undefined;

  useKeyboardShortcuts({
    onToggleSidebar: () => setSidebarCollapsed(!sidebarCollapsed),
    onShowHelp: () => setShowShortcutHelp(true),
  });

  const loadVaultState = useCallback(async () => {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      let path = "";
      try {
        path = await invoke<string>("get_vault_path");
      } catch {
        if (vaultPath) {
          await invoke("open_vault", { path: vaultPath });
          path = vaultPath;
        }
      }
      if (!path) return;

      setVaultPath(path);
      const notes = await invoke<NoteMeta[]>("list_notes");
      setNotes(notes);
      const graph = await invoke<GraphData>("get_graph");
      setGraph(graph);
      const tags = await invoke<{ name: string; count: number }[]>("list_tags");
      useAppStore.getState().setAllTags(tags);
      const notePath = useAppStore.getState().currentNote?.path;
      if (notePath && notes.some((note) => note.path === notePath)) {
        const content = await invoke<string>("read_note", { path: notePath });
        useAppStore.getState().openNote(notePath, content);
      }
    } catch {
      // No vault is open yet during first launch.
    }
  }, [setGraph, setNotes, setVaultPath, vaultPath]);

  useEffect(() => {
    if (!isTauri) return;
    return setupSnapshotDriftListener();
  }, [isTauri]);

  useEffect(() => {
    if (!isTauri) return;
    return setupTaskEventListeners();
  }, [isTauri]);

  useEffect(() => {
    if (!isTauri) return;
    let unlisten: (() => void) | undefined;
    (async () => {
      try {
        unlisten = await listenForResearchCompletion((payload: ResearchCompletedPayload) => {
          showToast("success", `Research completed: ${payload.intent || payload.task_id}`);
        });
      } catch {
        // best-effort
      }
    })();
    return () => { unlisten?.(); };
  }, [isTauri]);

  useEffect(() => {
    const mq = window.matchMedia("(max-width: 768px)");
    const handleChange = (event: MediaQueryListEvent | MediaQueryList) => {
      if (event.matches) setSidebarCollapsed(true);
    };

    handleChange(mq);
    mq.addEventListener("change", handleChange);
    return () => mq.removeEventListener("change", handleChange);
  }, [setSidebarCollapsed]);

  useEffect(() => {
    if (!isTauri) return;

    (async () => {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        const res = await invoke<string>("ping");
        setBackendStatus(`Backend ready: ${res}`);
        loadVaultState();
        loadSettingsFromBackend().catch(() => {});
      } catch {
        setBackendStatus("Backend connection failed");
      }
    })();
  }, [isTauri, loadVaultState]);

  useEffect(() => {
    if (!isTauri) return;
    let unlisten: (() => void) | undefined;
    (async () => {
      try {
        const { listen } = await import("@tauri-apps/api/event");
        unlisten = await listen("file-changed", () => {
          loadVaultState();
        });
      } catch {
        // best-effort
      }
    })();
    return () => { unlisten?.(); };
  }, [isTauri, loadVaultState]);

  const handleRunPipeline = useCallback(async (pipeline: PipelineDefinition) => {
    try {
      showToast("info", `Running pipeline: ${pipeline.name}`);
      await pipelineEngine.execute(pipeline, {}, (execution) => {
        if (execution.status === "completed") {
          showToast("success", `Pipeline completed: ${pipeline.name}`);
        }
        if (execution.status === "failed") {
          showToast("error", `Pipeline failed: ${pipeline.name}`);
        }
      });
    } catch (error) {
      showToast("error", `Pipeline failed: ${String(error)}`);
    }
  }, []);

  const handleSidebarNavKeyDown = useCallback((e: React.KeyboardEvent) => {
    const buttons = Array.from(
      (e.currentTarget as HTMLElement).querySelectorAll<HTMLButtonElement>("[data-sidebar-tab]")
    );
    const idx = buttons.indexOf(document.activeElement as HTMLButtonElement);
    if (e.key === "ArrowDown" || e.key === "ArrowRight") {
      e.preventDefault();
      buttons[(idx + 1) % buttons.length]?.focus();
    } else if (e.key === "ArrowUp" || e.key === "ArrowLeft") {
      e.preventDefault();
      buttons[(idx - 1 + buttons.length) % buttons.length]?.focus();
    }
  }, []);

  return (
    <div style={styles.app} data-mode-transition={isModeTransitioning ? "" : undefined} data-mode-transition-active={isModeTransitioning ? undefined : ""}>
      <style>{`
        [data-separator]:hover {
          background: var(--accent-glow) !important;
        }
        [data-separator]:active {
          background: color-mix(in srgb, var(--accent-primary) 60%, transparent) !important;
        }
        [data-mode-transition] .sidebar-panel-content {
          opacity: 0;
          transition: opacity 150ms ease-in;
        }
        [data-mode-transition-active] .sidebar-panel-content {
          opacity: 1;
          transition: opacity 150ms ease-out;
        }
        [data-mode-transition] .main-panel {
          opacity: 0;
          transition: opacity 150ms ease-in;
        }
        [data-mode-transition-active] .main-panel {
          opacity: 1;
          transition: opacity 150ms ease-out;
        }
        @media (prefers-reduced-motion: reduce) {
          [data-mode-transition] .sidebar-panel-content,
          [data-mode-transition-active] .sidebar-panel-content,
          [data-mode-transition] .main-panel,
          [data-mode-transition-active] .main-panel {
            transition: none !important;
          }
        }
      `}</style>
      <ToastContainer />
      <QuickSwitcher isTauri={isTauri} />
      <ShortcutHelpModal
        isOpen={showShortcutHelp}
        onClose={() => setShowShortcutHelp(false)}
      />
      <header style={styles.header} role="banner">
        <span style={styles.status} aria-label={`Backend status: ${backendStatus}`}>
          {backendStatus}
        </span>
        <button
          style={styles.sidebarToggle}
          onClick={() => setSidebarCollapsed(!sidebarCollapsed)}
          aria-label={sidebarCollapsed ? "Expand sidebar" : "Collapse sidebar"}
          aria-expanded={!sidebarCollapsed}
          title={sidebarCollapsed ? "Expand sidebar" : "Collapse sidebar"}
        >
          <FeroHaIcon name={sidebarCollapsed ? "PanelLeftOpen" : "PanelLeftClose"} size={16} />
        </button>
      </header>

      <div style={styles.main}>
        <Group orientation="horizontal" style={styles.panelGroup}>
          <Panel
            defaultSize={20}
            minSize={15}
            maxSize={35}
            collapsedSize={3}
            collapsible
            onResize={(panelSize) => {
              if (panelSize.asPercentage <= 5) {
                if (!sidebarCollapsed) setSidebarCollapsed(true);
              } else {
                if (sidebarCollapsed) setSidebarCollapsed(false);
              }
            }}
            style={{
              ...styles.sidebar,
              ...(sidebarCollapsed ? styles.sidebarCollapsed : {}),
            }}
          >
            <aside
              style={styles.sidebarInner}
              role="navigation"
              aria-label="Sidebar navigation"
            >
              <div style={styles.sidebarNav} role="tablist" aria-label="Panel tabs" onKeyDown={handleSidebarNavKeyDown}>
                <ModeToggle />
                <TabBtn panel="editor" title="Editor" />
                {mode === "human" && <TabBtn panel="inspiration" title="Inspiration" />}
                {mode === "ai" && <TabBtn panel="graph" title="Graph" />}
                <TabBtn panel="diff" title="Diff" />
                {mode === "ai" && <TabBtn panel="tasks" title="Tasks" />}
                {mode === "ai" && <TabBtn panel="bridge" title="Bridge" />}
                {mode === "ai" && <TabBtn panel="cards" title="Cards" />}
                {mode === "ai" && <TabBtn panel="pipeline" title="Pipeline" />}
                {mode === "ai" && <TabBtn panel="plugins" title="Plugins" />}
                <button
                  onClick={() => setShowSettings(!showSettings)}
                  title="Settings"
                  aria-label="Settings"
                  aria-pressed={showSettings}
                  data-sidebar-tab
                  style={{
                    ...styles.sidebarTabBtn,
                    ...(showSettings ? styles.sidebarTabBtnActive : {}),
                    marginLeft: "auto",
                  }}
                >
                  <FeroHaIcon name="Settings" size={16} />
                </button>
              </div>
              {!sidebarCollapsed && (
                showSettings ? (
                  <SettingsPanel />
                ) : (
                  <>
                    <div className="sidebar-panel-content"><VaultBrowser vaultPath={vaultPath} onSelectVault={setVaultPath} isTauri={isTauri} /></div>
                    {mode === "ai" && <div className="sidebar-panel-content"><TagsPanel isTauri={isTauri} /></div>}
                    {mode === "ai" && <div className="sidebar-panel-content"><BacklinksPanel currentNotePath={currentNote?.path ?? null} isTauri={isTauri} /></div>}
                  </>
                )
              )}
            </aside>
          </Panel>

          <Separator style={styles.resizeHandle} />

          <Panel>
            <main className="main-panel" style={styles.content} role="main" aria-label="Main content">
              <div role="tabpanel" id="panel-editor" aria-label="Editor panel" hidden={activePanel !== "editor"} style={styles.editorPanel}>
                {activePanel === "editor" && (
                  <div style={styles.editorContainer}>
                    <TabBar />
                    <div style={{
                      ...styles.editorSplitArea,
                      flexDirection: splitActive && splitDirection === "vertical" ? "column" : "row",
                    }}>
                      <div style={styles.editorPane}>
                        <Editor isTauri={isTauri} note={primaryNoteProps} />
                      </div>
                      {splitActive && splitTab && (
                        <>
                          <div
                            style={{
                              ...styles.splitDivider,
                              ...(splitDirection === "vertical"
                                ? { height: "4px", width: "100%", cursor: "row-resize" }
                                : { width: "4px", height: "100%", cursor: "col-resize" }),
                            }}
                          />
                          <div style={styles.editorPane}>
                            <Editor isTauri={isTauri} note={splitNoteProps} />
                          </div>
                        </>
                      )}
                    </div>
                  </div>
                )}
              </div>
              <div role="tabpanel" id="panel-graph" aria-label="Graph panel" hidden={activePanel !== "graph"}>
                {activePanel === "graph" && <GraphView focusNotePath={currentNote?.path} />}
              </div>
              <div role="tabpanel" id="panel-diff" aria-label="Diff panel" hidden={activePanel !== "diff"}>
                {activePanel === "diff" && <DiffView isTauri={isTauri} />}
              </div>
              <div role="tabpanel" id="panel-tasks" aria-label="Agent Dashboard panel" hidden={activePanel !== "tasks"}>
                {activePanel === "tasks" && <AgentDashboard />}
              </div>
              <div role="tabpanel" id="panel-bridge" aria-label="Bridge Inbox panel" hidden={activePanel !== "bridge"}>
                {activePanel === "bridge" && <BridgeInbox isTauri={isTauri} />}
              </div>
              <div role="tabpanel" id="panel-cards" aria-label="Command cards panel" hidden={activePanel !== "cards"}>
                {activePanel === "cards" && (
                  <CommandCardLibrary
                    isOpen
                    mode="manage"
                    onClose={() => useAppStore.getState().setActivePanel("editor")}
                  />
                )}
              </div>
              <div role="tabpanel" id="panel-pipeline" aria-label="Pipeline panel" hidden={activePanel !== "pipeline"}>
                {activePanel === "pipeline" && <PipelineEditor onRun={handleRunPipeline} />}
              </div>
              <div role="tabpanel" id="panel-plugins" aria-label="Plugins panel" hidden={activePanel !== "plugins"}>
                {activePanel === "plugins" && <PluginSettings />}
              </div>
              <div role="tabpanel" id="panel-inspiration" aria-label="Inspiration panel" hidden={activePanel !== "inspiration"}>
                {activePanel === "inspiration" && <InspirationCanvas />}
              </div>
            </main>
          </Panel>
        </Group>
      </div>

      <footer style={styles.footer} role="contentinfo">
        {mode === "ai" && <OrchestratorPanel />}
        {mode === "ai" && <CliBar isTauri={isTauri} />}
        <StatusBar />
      </footer>
      <CliMiniWindow vaultPath={vaultPath ?? ""} />
    </div>
  );
}

const tabIcons: Record<string, string> = {
  editor: "FileText",
  graph: "GitGraph",
  diff: "GitCompare",
  tasks: "ListTodo",
  bridge: "Inbox",
  cards: "LayoutGrid",
  pipeline: "Workflow",
  plugins: "Plug",
  inspiration: "Lightbulb",
};

function TabBtn({ panel, title }: { panel: "editor" | "graph" | "diff" | "tasks" | "bridge" | "cards" | "pipeline" | "plugins" | "inspiration"; title: string }) {
  const activePanel = useAppStore((s) => s.activePanel);
  const setActivePanel = useAppStore((s) => s.setActivePanel);
  const isActive = activePanel === panel;

  return (
    <button
      onClick={() => setActivePanel(panel)}
      title={title}
      role="tab"
      aria-selected={isActive}
      aria-controls={`panel-${panel}`}
      data-sidebar-tab
      tabIndex={isActive ? 0 : -1}
      style={{
        ...styles.sidebarTabBtn,
        ...(isActive ? styles.sidebarTabBtnActive : {}),
      }}
    >
      <FeroHaIcon name={tabIcons[panel]} size={16} />
    </button>
  );
}

const styles: Record<string, React.CSSProperties> = {
  app: {
    display: "flex",
    flexDirection: "column",
    height: "100vh",
    fontFamily: "system-ui, -apple-system, sans-serif",
    backgroundColor: "var(--bg-primary)",
    color: "var(--text-primary)",
  },
  header: {
    display: "flex",
    justifyContent: "space-between",
    alignItems: "center",
    padding: "6px 16px",
    backgroundColor: "var(--bg-secondary)",
    borderBottom: "1px solid var(--border-color)",
    fontSize: "13px",
    userSelect: "none",
  },
  status: {
    fontSize: "11px",
    color: "var(--text-secondary)",
    maxWidth: "240px",
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  sidebarToggle: {
    display: "inline-flex",
    alignItems: "center",
    justifyContent: "center",
    width: "26px",
    height: "26px",
    backgroundColor: "transparent",
    color: "var(--text-muted)",
    border: "1px solid transparent",
    borderRadius: "4px",
    cursor: "pointer",
    transition: "all 0.15s",
  },
  main: {
    display: "flex",
    flex: 1,
    overflow: "hidden",
  },
  panelGroup: {
    flex: 1,
  },
  sidebar: {
    backgroundColor: "var(--bg-secondary)",
    borderRight: "1px solid var(--border-color)",
    overflow: "hidden",
  },
  sidebarCollapsed: {
    width: "44px",
    minWidth: "44px",
  },
  sidebarInner: {
    height: "100%",
    overflow: "auto",
    display: "flex",
    flexDirection: "column",
  },
  resizeHandle: {
    width: "4px",
    background: "transparent",
    transition: "background 200ms",
    outline: "none",
  },
  content: {
    flex: 1,
    overflow: "hidden",
    display: "flex",
    flexDirection: "column",
  },
  editorPanel: {
    height: "100%",
    overflow: "hidden",
  },
  editorContainer: {
    display: "flex",
    flexDirection: "column",
    height: "100%",
    overflow: "hidden",
  },
  editorSplitArea: {
    display: "flex",
    flex: 1,
    overflow: "hidden",
  },
  editorPane: {
    flex: 1,
    overflow: "hidden",
    padding: "24px",
  },
  splitDivider: {
    flexShrink: 0,
    background: "var(--border-color)",
    transition: "background 0.15s",
  },
  footer: {
    borderTop: "1px solid var(--border-color)",
    backgroundColor: "var(--bg-secondary)",
  },
  sidebarNav: {
    display: "flex",
    gap: "2px",
    padding: "6px 8px",
    borderBottom: "1px solid var(--border-color)",
  },
  sidebarTabBtn: {
    display: "inline-flex",
    alignItems: "center",
    justifyContent: "center",
    width: "30px",
    height: "28px",
    backgroundColor: "transparent",
    color: "var(--text-muted)",
    border: "1px solid transparent",
    borderRadius: "4px",
    cursor: "pointer",
    transition: "all 0.15s",
    outline: "none",
  },
  sidebarTabBtnActive: {
    backgroundColor: "var(--bg-input)",
    color: "var(--text-primary)",
  },
};
