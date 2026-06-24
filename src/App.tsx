import { useEffect, useState, useCallback, useRef } from "react";
import { Group, Panel, Separator } from "react-resizable-panels";
import {
  resolveActivePanelForMode,
  useAppStore,
  type ActivePanel,
  type AppMode,
  type NoteMeta,
  type GraphData,
} from "./hooks/useAppStore";
import Editor from "./components/Editor";
import TabBar from "./components/TabBar";
import VaultBrowser, { mergeVaultNoteLists } from "./components/VaultBrowser";
import GraphView from "./components/GraphView";
import DiffView from "./components/DiffView";
import AiTaskStrip from "./components/AiTaskStrip";
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
import OrchestratorWorkflowView from "./components/OrchestratorWorkflowView";
import PluginSettings from "./components/PluginSettings";
import BridgeInbox from "./components/BridgeInbox";
import HumanTaskIntake from "./components/HumanTaskIntake";
import { ToastContainer } from "./components/Toast";
import { showToast } from "./components/toastBus";
import { listenForResearchCompletion, type ResearchCompletedPayload } from "./lib/ipc";
import { useKeyboardShortcuts } from "./hooks/useKeyboardShortcuts";
import { ShortcutHelpModal } from "./components/ShortcutTooltip";
import { useSettings, loadSettingsFromBackend } from "./hooks/useSettings";
import type { WorkflowRuntimeBundle } from "./types/orchestrator";
import "./styles/feroha-theme.css";

export const sidebarPanelSizing = {
  defaultSize: "20%",
  minSize: "15%",
  maxSize: "35%",
  collapsedSize: "6%",
} as const;

export const windowControlDefinitions = [
  { id: "minimize", label: "最小化", icon: "Minus" },
  { id: "maximize", label: "最大化", icon: "Square" },
  { id: "close", label: "关闭", icon: "X" },
] as const;

type WindowControlId = (typeof windowControlDefinitions)[number]["id"];

export async function runWindowControlAction(id: WindowControlId) {
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    const appWindow = getCurrentWindow();
    if (id === "minimize") await appWindow.minimize();
    if (id === "maximize") await appWindow.toggleMaximize();
    if (id === "close") await appWindow.close();
  } catch (error) {
    console.error("Window control action failed:", error);
    showToast("error", "窗口控制暂不可用");
  }
}

interface SnapshotDriftPayload {
  snapshot_type?: "global" | "local" | string;
  note_id?: string;
  avg_cosine_distance?: number;
}

function hasTauriRuntime() {
  return typeof window !== "undefined" && Boolean(window.__TAURI_INTERNALS__ || (window as any).__TAURI__);
}

export function snapshotDriftToastMessage(drift: SnapshotDriftPayload): string {
  const typeLabel = drift.snapshot_type === "global" ? "全局快照" : "局部快照";
  const distance = drift.avg_cosine_distance;
  const distanceLabel = typeof distance === "number" ? distance.toFixed(3) : "无数据";
  return `${typeLabel}漂移："${drift.note_id}"，平均距离 ${distanceLabel}`;
}

export function taskCheckedToastMessage(taskId: string): string {
  return `任务 ${taskId} 已批准`;
}

export function agentResultToastMessage(type: string, query?: string): string {
  return `AI 结果已就绪：${type} - "${query?.slice(0, 50) || ""}..."`;
}

export function feedbackRecordedToastMessage(action: string, blockCount = 0): string {
  return `反馈已记录：${action}，涉及 ${blockCount} 个区块`;
}

function setupSnapshotDriftListener() {
  let unlisten: (() => void) | undefined;
  (async () => {
    try {
      const { listen } = await import("@tauri-apps/api/event");
      unlisten = await listen<SnapshotDriftPayload>("snapshot-drift", (event) => {
        showToast("warning", snapshotDriftToastMessage(event.payload));
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
        showToast("info", taskCheckedToastMessage(event.payload.task_id));
      });
      unlistens.push(u1);

      const u2 = await listen<{ type: string; query: string }>("agent-result", (event) => {
        const p = event.payload;
        showToast("success", agentResultToastMessage(p.type, p.query));
      });
      unlistens.push(u2);

      const u3 = await listen<{ action: string; block_ids?: string[] }>("feroha_feedback_recorded", (event) => {
        const p = event.payload;
        showToast("info", feedbackRecordedToastMessage(p.action, p.block_ids?.length || 0));
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
  const [showShortcutHelp, setShowShortcutHelp] = useState(false);
  const [newNotePickerOpen, setNewNotePickerOpen] = useState(false);
  const [settings] = useSettings();
  const mode = useAppStore((s) => s.mode);
  const [isModeTransitioning, setIsModeTransitioning] = useState(false);
  const [apiDebugGlowActive, setApiDebugGlowActive] = useState(false);
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

  useEffect(() => {
    const handleApiDebugSuccess = () => {
      setApiDebugGlowActive(true);
      window.setTimeout(() => setApiDebugGlowActive(false), 5200);
    };
    window.addEventListener("feroha:api-debug-success", handleApiDebugSuccess);
    return () => window.removeEventListener("feroha:api-debug-success", handleApiDebugSuccess);
  }, []);

  const setVaultPath = useAppStore((s) => s.setVaultPath);
  const setNotes = useAppStore((s) => s.setNotes);
  const setGraph = useAppStore((s) => s.setGraph);
  const applyWorkflowRunUpdate = useAppStore((s) => s.applyWorkflowRunUpdate);
  const vaultPath = useAppStore((s) => s.vaultPath);
  const currentNote = useAppStore((s) => s.currentNote);
  const activePanel = useAppStore((s) => s.activePanel);
  const setActivePanel = useAppStore((s) => s.setActivePanel);
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

  useEffect(() => {
    const resolved = resolvePanelForMode(activePanel, mode);
    if (resolved !== activePanel) setActivePanel(resolved);
  }, [activePanel, mode, setActivePanel]);

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
        const urlVaultPath = new URLSearchParams(window.location.search).get("vault");
        const bootstrapVaultPath = urlVaultPath || vaultPath;
        if (bootstrapVaultPath) {
          await invoke("open_vault", { path: bootstrapVaultPath });
          path = bootstrapVaultPath;
        }
      }
      if (!path) return;

      setVaultPath(path);
      const [humanNotes, aiWorkspaceNotes] = await Promise.all([
        invoke<NoteMeta[]>("list_notes"),
        invoke<NoteMeta[]>("list_ai_workspace_files").catch(() => [] as NoteMeta[]),
      ]);
      const notes = mergeVaultNoteLists(humanNotes, aiWorkspaceNotes);
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

    loadVaultState();
    loadSettingsFromBackend().catch(() => {});
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

  useEffect(() => {
    if (!isTauri) return;
    let unlisten: (() => void) | undefined;
    (async () => {
      try {
        const { listen } = await import("@tauri-apps/api/event");
        unlisten = await listen<WorkflowRuntimeBundle>(
          "workflow-run-updated",
          async (event) => {
            applyWorkflowRunUpdate(event.payload);
            try {
              const { invoke } = await import("@tauri-apps/api/core");
              const [humanNotes, aiWorkspaceNotes] = await Promise.all([
                invoke<NoteMeta[]>("list_notes"),
                invoke<NoteMeta[]>("list_ai_workspace_files").catch(() => [] as NoteMeta[]),
              ]);
              setNotes(mergeVaultNoteLists(humanNotes, aiWorkspaceNotes));
            } catch {
              // Runtime state is still applied even if the file browser refresh fails.
            }
          },
        );
      } catch {
        // best-effort
      }
    })();
    return () => { unlisten?.(); };
  }, [applyWorkflowRunUpdate, isTauri, setNotes]);

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

  const handlePanelSelect = useCallback(
    (panel: ActivePanel) => {
      setActivePanel(resolvePanelForMode(panel, mode));
    },
    [mode, setActivePanel],
  );

  const handleRequestNewNote = useCallback(() => {
    setSidebarCollapsed(false);
    setNewNotePickerOpen(true);
  }, [setSidebarCollapsed]);

  return (
    <div
      style={styles.app}
      data-mode-transition={isModeTransitioning ? "" : undefined}
      data-mode-transition-active={isModeTransitioning ? undefined : ""}
      data-api-debug-success={apiDebugGlowActive ? "" : undefined}
    >
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
        <div style={styles.headerDragRegion} data-tauri-drag-region />
        <div style={styles.headerControls}>
          <button
            style={styles.sidebarToggle}
            onClick={() => setSidebarCollapsed(!sidebarCollapsed)}
            aria-label={sidebarCollapsed ? "展开侧栏" : "折叠侧栏"}
            aria-expanded={!sidebarCollapsed}
            title={sidebarCollapsed ? "展开侧栏" : "折叠侧栏"}
          >
            <FeroHaIcon name={sidebarCollapsed ? "PanelLeftOpen" : "PanelLeftClose"} size={16} />
          </button>
          <WindowControls />
        </div>
      </header>

      <div style={styles.main}>
        <Group orientation="horizontal" style={styles.panelGroup}>
          <Panel
            defaultSize={sidebarPanelSizing.defaultSize}
            minSize={sidebarPanelSizing.minSize}
            maxSize={sidebarPanelSizing.maxSize}
            collapsedSize={sidebarPanelSizing.collapsedSize}
            collapsible
            onResize={(panelSize) => {
              if (panelSize.asPercentage <= 7) {
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
              aria-label="侧栏导航"
            >
              <div style={sidebarNavStyleForState(sidebarCollapsed)} role="tablist" aria-label="面板标签" onKeyDown={handleSidebarNavKeyDown}>
                <ModeToggle collapsed={sidebarCollapsed} />
                {panelTabsForMode(mode).map((tab) => (
                  <TabBtn key={tab.panel} panel={tab.panel} title={tab.title} onSelect={handlePanelSelect} />
                ))}
                <button
                  onClick={() => handlePanelSelect("settings")}
                  title="设置"
                  aria-label="设置"
                  aria-selected={activePanel === "settings"}
                  aria-controls="panel-settings"
                  data-sidebar-tab
                  style={settingsButtonStyleForState(sidebarCollapsed, activePanel === "settings")}
                >
                  <FeroHaIcon name="Settings" size={16} />
                </button>
              </div>
              {!sidebarCollapsed && (
                <>
                  <div className="sidebar-panel-content">
                    <VaultBrowser
                      vaultPath={vaultPath}
                      onSelectVault={setVaultPath}
                      isTauri={isTauri}
                      templatePickerOpen={newNotePickerOpen}
                      onTemplatePickerClose={() => setNewNotePickerOpen(false)}
                    />
                  </div>
                  {mode === "ai" && <div className="sidebar-panel-content"><TagsPanel isTauri={isTauri} /></div>}
                  {mode === "ai" && <div className="sidebar-panel-content"><BacklinksPanel currentNotePath={currentNote?.path ?? null} isTauri={isTauri} /></div>}
                </>
              )}
            </aside>
          </Panel>

          <Separator className="app-resize-separator feroha-resize-handle" style={styles.resizeHandle} />

          <Panel>
            <main className="main-panel" style={styles.content} role="main" aria-label="主内容">
              <div role="tabpanel" id="panel-editor" aria-label="编辑器面板" hidden={activePanel !== "editor"} style={styles.editorPanel}>
                {activePanel === "editor" && (
                  <div style={styles.editorContainer}>
                    <TabBar />
                    <div style={{
                      ...styles.editorSplitArea,
                      flexDirection: splitActive && splitDirection === "vertical" ? "column" : "row",
                    }}>
                      <div style={styles.editorPane}>
                        <Editor
                          isTauri={isTauri}
                          note={primaryNoteProps}
                          readOnly={mode === "ai"}
                          readOnlyLabel="AI 面只读"
                          onCreateNote={handleRequestNewNote}
                        />
                      </div>
                      {splitActive && splitTab && (
                        <>
                          <div
                            className="editor-split-divider feroha-resize-handle"
                            style={{
                              ...styles.splitDivider,
                              ...(splitDirection === "vertical"
                                ? { height: "4px", width: "100%", cursor: "row-resize" }
                                : { width: "4px", height: "100%", cursor: "col-resize" }),
                            }}
                          />
                          <div style={styles.editorPane}>
                            <Editor
                              isTauri={isTauri}
                              note={splitNoteProps}
                              readOnly={mode === "ai"}
                              readOnlyLabel="AI 面只读"
                              onCreateNote={handleRequestNewNote}
                            />
                          </div>
                        </>
                      )}
                    </div>
                  </div>
                )}
              </div>
              <div role="tabpanel" id="panel-graph" aria-label="图谱面板" hidden={activePanel !== "graph"} style={styles.panelShell}>
                {activePanel === "graph" && <GraphView focusNotePath={currentNote?.path} />}
              </div>
              <div role="tabpanel" id="panel-diff" aria-label="差异审查面板" hidden={activePanel !== "diff"} style={styles.panelShell}>
                {activePanel === "diff" && <DiffView isTauri={isTauri} />}
              </div>
              <div role="tabpanel" id="panel-tasks" aria-label="Agent 任务面板" hidden={activePanel !== "tasks"} style={styles.panelShell}>
                {activePanel === "tasks" && <AgentDashboard />}
              </div>
              <div role="tabpanel" id="panel-bridge" aria-label="桥接收件箱面板" hidden={activePanel !== "bridge"} style={styles.panelShell}>
                {activePanel === "bridge" && <BridgeInbox isTauri={isTauri} />}
              </div>
              <div role="tabpanel" id="panel-cards" aria-label="指令卡面板" hidden={activePanel !== "cards"} style={styles.panelShell}>
                {activePanel === "cards" && (
                  <CommandCardLibrary
                    isOpen
                    mode="manage"
                    onClose={() => useAppStore.getState().setActivePanel("editor")}
                  />
                )}
              </div>
              <div role="tabpanel" id="panel-pipeline" aria-label="编排面板" hidden={activePanel !== "pipeline"} style={styles.panelShell}>
                {activePanel === "pipeline" && <OrchestratorWorkflowView />}
              </div>
              <div role="tabpanel" id="panel-plugins" aria-label="插件面板" hidden={activePanel !== "plugins"} style={styles.panelShell}>
                {activePanel === "plugins" && <PluginSettings />}
              </div>
              <div role="tabpanel" id="panel-inspiration" aria-label="灵感画布面板" hidden={activePanel !== "inspiration"} style={styles.panelShell}>
                {activePanel === "inspiration" && <InspirationCanvas />}
              </div>
              <div role="tabpanel" id="panel-task-intake" aria-label="向 AI 提任务面板" hidden={activePanel !== "task-intake"} style={styles.panelShell}>
                {activePanel === "task-intake" && <HumanTaskIntake isTauri={isTauri} />}
              </div>
              <div role="tabpanel" id="panel-settings" aria-label="设置面板" hidden={activePanel !== "settings"} style={styles.panelShell}>
                {activePanel === "settings" && <SettingsPanel />}
              </div>
            </main>
          </Panel>
        </Group>
      </div>

      <footer style={styles.footer} role="contentinfo">
        {mode === "ai" && <OrchestratorPanel />}
        {mode === "ai" && <AiTaskStrip isTauri={isTauri} />}
        <StatusBar />
      </footer>
      <CliMiniWindow vaultPath={vaultPath ?? ""} isTauri={isTauri} />
    </div>
  );
}

const tabIcons: Record<string, string> = {
  editor: "FileText",
  "task-intake": "Send",
  graph: "GitGraph",
  diff: "GitCompare",
  tasks: "ListTodo",
  bridge: "Inbox",
  cards: "LayoutGrid",
  pipeline: "Workflow",
  plugins: "Plug",
  inspiration: "Lightbulb",
  settings: "Settings",
};

export interface PanelTabDefinition {
  panel: ActivePanel;
  title: string;
}

export function panelTabsForMode(mode: AppMode): PanelTabDefinition[] {
  if (mode === "human") {
    return [
      { panel: "editor", title: "编辑器" },
      { panel: "task-intake", title: "向 AI 提任务" },
      { panel: "inspiration", title: "灵感画布" },
      { panel: "bridge", title: "桥接审查" },
      { panel: "diff", title: "差异审查" },
    ];
  }

  return mode === "ai"
    ? [
        { panel: "editor", title: "编辑器" },
        { panel: "graph", title: "知识图谱" },
        { panel: "tasks", title: "Agent 任务" },
        { panel: "cards", title: "指令卡" },
        { panel: "pipeline", title: "编排" },
        { panel: "plugins", title: "插件" },
      ]
    : [
        { panel: "task-intake", title: "向 AI 提任务" },
        { panel: "editor", title: "编辑器" },
        { panel: "inspiration", title: "灵感画布" },
        { panel: "bridge", title: "桥接审查" },
        { panel: "diff", title: "差异审查" },
      ];
}

export function resolvePanelForMode(panel: ActivePanel, mode: AppMode): ActivePanel {
  return resolveActivePanelForMode(panel, mode);
}

function TabBtn({
  panel,
  title,
  onSelect,
}: {
  panel: ActivePanel;
  title: string;
  onSelect: (panel: ActivePanel) => void;
}) {
  const activePanel = useAppStore((s) => s.activePanel);
  const isActive = activePanel === panel;

  return (
    <button
      onClick={() => onSelect(panel)}
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

function WindowControls() {
  return (
    <div style={styles.windowControls} aria-label="窗口控制">
      {windowControlDefinitions.map((control) => (
        <button
          key={control.id}
          type="button"
          style={{
            ...styles.windowControlBtn,
            ...(control.id === "close" ? styles.windowCloseBtn : {}),
          }}
          title={control.label}
          aria-label={control.label}
          onMouseDown={(event) => event.stopPropagation()}
          onClick={() => { void runWindowControlAction(control.id); }}
        >
          <FeroHaIcon name={control.icon} size={13} />
        </button>
      ))}
    </div>
  );
}

export function sidebarNavStyleForState(collapsed: boolean): React.CSSProperties {
  return {
    ...styles.sidebarNav,
    ...(collapsed ? styles.sidebarNavCollapsed : {}),
  };
}

export function settingsButtonStyleForState(collapsed: boolean, active: boolean): React.CSSProperties {
  return {
    ...styles.sidebarTabBtn,
    ...(active ? styles.sidebarTabBtnActive : {}),
    ...(collapsed ? { marginTop: "auto", marginLeft: 0 } : { marginLeft: 0 }),
  };
}

const styles: Record<string, React.CSSProperties> = {
  app: {
    display: "flex",
    flexDirection: "column",
    height: "100vh",
    fontFamily: "system-ui, -apple-system, sans-serif",
    backgroundColor: "var(--bg-primary)",
    color: "var(--text-primary)",
    position: "relative",
  },
  header: {
    display: "flex",
    justifyContent: "space-between",
    alignItems: "center",
    padding: "4px 8px 4px 14px",
    backgroundColor: "var(--bg-secondary)",
    borderBottom: "1px solid var(--border-color)",
    fontSize: "13px",
    userSelect: "none",
    minHeight: "34px",
  },
  headerDragRegion: {
    display: "flex",
    alignItems: "center",
    flex: 1,
    minWidth: 0,
    height: "100%",
  },
  headerControls: {
    display: "flex",
    alignItems: "center",
    gap: "6px",
    flexShrink: 0,
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
  windowControls: {
    display: "inline-flex",
    alignItems: "center",
    gap: "2px",
    paddingLeft: "4px",
    borderLeft: "1px solid var(--border-muted)",
  },
  windowControlBtn: {
    display: "inline-flex",
    alignItems: "center",
    justifyContent: "center",
    width: "30px",
    height: "26px",
    backgroundColor: "transparent",
    color: "var(--text-muted)",
    border: "1px solid transparent",
    borderRadius: "5px",
    cursor: "pointer",
    transition: "background 120ms ease, color 120ms ease",
  },
  windowCloseBtn: {
    color: "var(--text-secondary)",
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
    position: "relative",
    zIndex: 2,
  },
  sidebarCollapsed: {
    width: "44px",
    minWidth: "44px",
    maxWidth: "56px",
  },
  sidebarInner: {
    height: "100%",
    overflow: "auto",
    display: "flex",
    flexDirection: "column",
  },
  resizeHandle: {
    width: "4px",
    background: "var(--resize-handle-bg)",
    cursor: "col-resize",
    transition: "background 200ms",
    outline: "none",
    position: "relative",
    zIndex: 1,
  },
  content: {
    flex: 1,
    overflow: "hidden",
    display: "flex",
    flexDirection: "column",
    position: "relative",
    zIndex: 0,
  },
  editorPanel: {
    height: "100%",
    overflow: "hidden",
  },
  panelShell: {
    height: "100%",
    minHeight: 0,
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
    background: "var(--resize-handle-bg)",
    transition: "background 0.15s",
  },
  footer: {
    borderTop: "1px solid var(--border-color)",
    backgroundColor: "var(--bg-secondary)",
  },
  sidebarNav: {
    display: "flex",
    flexWrap: "wrap",
    gap: "2px",
    padding: "6px 8px",
    borderBottom: "1px solid var(--border-color)",
    overflow: "visible",
    alignItems: "center",
  },
  sidebarNavCollapsed: {
    flexWrap: "nowrap",
    flexDirection: "column",
    alignItems: "center",
    padding: "8px 6px",
    position: "relative",
    zIndex: 3,
  },
  sidebarTabBtn: {
    display: "inline-flex",
    alignItems: "center",
    justifyContent: "center",
    width: "30px",
    height: "28px",
    flexShrink: 0,
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
