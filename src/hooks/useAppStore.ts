// Global state management for Dual-Track Note IDE
// Uses Zustand for lightweight, hook-based state

import { create } from "zustand";
import { persist } from "zustand/middleware";
import type {
  HarnessEvent,
  OrchestratorOutput,
  OrchestratorStatus,
  WorkflowPatch,
  WorkflowRuntimeBundle,
} from "../types/orchestrator";
import type { ToolInfo } from "../types/command-card";
import type { BridgeProposal, BridgeProposalActionResult } from "../types/bridge-proposal";
import type { MockSimulationRun } from "../lib/mockSimulationSuite";

export type ActivePanel =
  | "editor"
  | "task-intake"
  | "graph"
  | "diff"
  | "tasks"
  | "cards"
  | "pipeline"
  | "plugins"
  | "inspiration"
  | "bridge"
  | "settings";

export type AppMode = "human" | "ai";

const activePanels: ActivePanel[] = [
  "editor",
  "task-intake",
  "graph",
  "diff",
  "tasks",
  "cards",
  "pipeline",
  "plugins",
  "inspiration",
  "bridge",
  "settings",
];

function isActivePanel(panel: unknown): panel is ActivePanel {
  return typeof panel === "string" && activePanels.includes(panel as ActivePanel);
}

const aiPanels = new Set<ActivePanel>([
  "editor",
  "graph",
  "tasks",
  "cards",
  "pipeline",
  "plugins",
  "settings",
]);

const humanPanels = new Set<ActivePanel>([
  "editor",
  "task-intake",
  "inspiration",
  "bridge",
  "diff",
  "settings",
]);

export function modeForPanel(panel: ActivePanel, currentMode: AppMode): AppMode {
  if (panel === "settings" || panel === "editor") return currentMode;
  if (humanPanels.has(panel)) return "human";
  return "ai";
}

export function resolveActivePanelForMode(panel: ActivePanel, mode: AppMode): ActivePanel {
  if (mode === "ai") return aiPanels.has(panel) ? panel : "graph";
  return humanPanels.has(panel) ? panel : "inspiration";
}

function navigationUpdateFromMetadata(
  metadata: Record<string, unknown> | undefined,
  currentMode: AppMode,
): { mode: AppMode; activePanel: ActivePanel } | null {
  if (!metadata?.effect || metadata.effect !== "navigate" || !isActivePanel(metadata.target_panel)) {
    return null;
  }
  const targetPanel = metadata.target_panel;
  const targetMode = modeForPanel(targetPanel, currentMode);
  return {
    mode: targetMode,
    activePanel: resolveActivePanelForMode(targetPanel, targetMode),
  };
}

function payloadString(payload: Record<string, unknown> | undefined, key: string): string | null {
  const value = payload?.[key];
  return typeof value === "string" && value.trim() ? value : null;
}

function localBridgeNavigationResult(
  proposal: BridgeProposal | undefined,
  actionId: string,
  message: string,
): BridgeProposalActionResult | null {
  const action = proposal?.actions.find((candidate) => candidate.id === actionId);
  if (!proposal || !action) {
    return null;
  }

  if (action.kind === "open_diff") {
    return {
      status: "navigate",
      message,
      proposal,
      metadata: {
        effect: "navigate",
        target_panel: "diff",
        target_id: payloadString(action.payload, "ghost_id") ?? proposal.source_ref.id,
      },
    };
  }

  if (action.kind === "open_trace") {
    return {
      status: "navigate",
      message,
      proposal,
      metadata: {
        effect: "navigate",
        target_panel: "tasks",
        target_id: payloadString(action.payload, "task_id") ?? proposal.source_ref.id,
      },
    };
  }

  return null;
}

export interface NoteMeta {
  path: string;
  title: string;
  size: number;
  modified: string;
  created: string;
  links: string[];
  tags: string[];
}

export interface TaskStatus {
  id: string;
  command: string;
  status: "pending" | "approved" | "running" | "done" | "error" | "cancelled";
  result?: string;
  has_trace?: boolean;
}

export type MemoryZone = "working" | "semantic" | "long_term";

export interface GraphNode {
  id: string;
  title: string;
  outgoing: number;
  incoming: number;
  activation?: number;
  memory_zone?: MemoryZone;
}

export type GraphEdgeType =
  | "parent"
  | "reference"
  | "related"
  | "source"
  | "sequence"
  | "semantic"
  | "temporal"
  | "bridge";

export interface GraphEdge {
  from: string;
  to: string;
  edge_type?: GraphEdgeType;
  origin?: string;
  confidence?: number;
  weight?: number;
  memory_region?: string;
}

export interface GraphData {
  nodes: GraphNode[];
  edges: GraphEdge[];
}

export interface DiffBlock {
  ghostId: string;
  id: string;
  type: "inserted" | "deleted" | "modified";
  oldText?: string;
  newText?: string;
  accepted: boolean;
  rejected: boolean;
}

export interface TabState {
  path: string;
  title: string;
  content: string;
  viewMode: 'edit' | 'preview';
  isDirty: boolean;
}

interface AppStore {
  // Vault state
  vaultPath: string | null;
  notes: NoteMeta[];
  currentNote: { path: string; content: string } | null;
  isDirty: boolean;

  // Vault actions
  setVaultPath: (path: string | null) => void;
  setNotes: (notes: NoteMeta[]) => void;
  openNote: (path: string, content: string) => void;
  setCurrentContent: (content: string) => void;
  markClean: () => void;

  // Tab state
  tabs: TabState[];
  activeTabIndex: number;
  closedTabs: TabState[];
  splitActive: boolean;
  splitDirection: 'horizontal' | 'vertical';
  splitTabIndex: number;

  // Tab actions
  openTab: (path: string, title: string, content: string) => void;
  closeTab: (index: number) => void;
  switchTab: (index: number) => void;
  toggleSplit: (direction?: 'right' | 'down') => void;
  closeAllTabs: () => void;
  closeOtherTabs: (index: number) => void;
  restoreTab: () => void;
  updateTabDirty: (path: string, isDirty: boolean) => void;
  updateTabViewMode: (path: string, mode: 'edit' | 'preview') => void;
  setTabContent: (path: string, content: string) => void;
  markTabClean: (path: string) => void;

  // Graph state
  graph: GraphData;
  setGraph: (graph: GraphData) => void;

  // Navigation breadcrumb path for graph
  navigationPath: string[];
  addToNavigationPath: (path: string) => void;
  clearNavigationPath: () => void;

  // CLI / Agent state
  tasks: TaskStatus[];
  addTask: (task: TaskStatus) => void;
  updateTask: (id: string, updates: Partial<TaskStatus>) => void;
  clearCompletedTasks: () => void;
  applyMockSimulationRun: (run: MockSimulationRun) => void;

  // Diff state
  diffBlocks: DiffBlock[];
  setDiffBlocks: (blocks: DiffBlock[]) => void;
  updateDiffBlock: (id: string, updates: Partial<DiffBlock>) => void;

  // Cursor state (for StatusBar)
  cursorLine: number;
  cursorCol: number;
  setCursorPos: (line: number, col: number) => void;

  // Save status (for StatusBar)
  saveStatus: "idle" | "saving" | "success" | "error";
  setSaveStatus: (status: "idle" | "saving" | "success" | "error") => void;

  // Sidebar state (responsive)
  sidebarCollapsed: boolean;
  setSidebarCollapsed: (collapsed: boolean) => void;

  // Recent notes history
  recentNotes: string[];
  addRecentNote: (path: string) => void;
  clearRecentNotes: () => void;

  // Favorites
  favorites: string[];
  toggleFavorite: (path: string) => void;
  isFavorite: (path: string) => boolean;

  // Tags
  allTags: { name: string; count: number }[];
  setAllTags: (tags: { name: string; count: number }[]) => void;
  filterTags: string[];
  setFilterTags: (tags: string[]) => void;
  toggleFilterTag: (tag: string) => void;

  // Orchestrator state
  orchestratorStatus: OrchestratorStatus | null;
  fetchOrchestratorStatus: () => Promise<void>;
  workflowRuntimeEvents: HarnessEvent[];
  workflowRuntimeEventRunId: string | null;
  workflowRuntimeEventError: string | null;
  fetchWorkflowRuntimeEvents: (runId: string, limit?: number) => Promise<HarnessEvent[]>;
  workflowRuns: WorkflowRuntimeBundle[];
  activeWorkflowRun: WorkflowRuntimeBundle | null;
  workflowRunLoading: boolean;
  workflowRunError: string | null;
  createAndStartWorkflow: (
    goalText: string,
    acceptanceCriteria: string[],
  ) => Promise<WorkflowRuntimeBundle | null>;
  fetchWorkflowRuns: () => Promise<WorkflowRuntimeBundle[]>;
  fetchWorkflowRun: (runId: string) => Promise<WorkflowRuntimeBundle | null>;
  applyWorkflowRunUpdate: (bundle: WorkflowRuntimeBundle) => void;
  submitWorkflowPatchReview: (runId: string, patch: WorkflowPatch) => Promise<BridgeProposal | null>;
  submitOrchestratorOutputReview: (
    runId: string,
    output: OrchestratorOutput,
  ) => Promise<BridgeProposal | null>;
  terminateAgent: (agentId: string) => Promise<boolean>;
  reinstateAgent: (agentId: string) => Promise<boolean>;

  // Agent tools
  agentTools: ToolInfo[];
  fetchAgentTools: () => Promise<void>;

  // Bridge proposal inbox
  bridgeProposals: BridgeProposal[];
  bridgeLoading: boolean;
  bridgeError: string | null;
  fetchBridgeProposals: () => Promise<void>;
  executeBridgeAction: (proposalId: string, actionId: string) => Promise<BridgeProposalActionResult | null>;

  // UI state
  activePanel: ActivePanel;
  setActivePanel: (panel: ActivePanel) => void;
  isCliActive: boolean;
  setCliActive: (active: boolean) => void;
  mode: AppMode;
  setMode: (mode: AppMode) => void;

  // Navigation history
  _isNavigatingHistory: boolean;
  navigationStack: string[];
  navigationIndex: number;
  hasBack: boolean;
  hasForward: boolean;
  goBack: () => void;
  goForward: () => void;
}

function getTitleFromPath(path: string): string {
  return path.split('/').pop()?.replace(/\.md$/, '') || path;
}

function sortWorkflowRuns(runs: WorkflowRuntimeBundle[]): WorkflowRuntimeBundle[] {
  return [...runs].sort((left, right) => {
    const leftNumeric = Number(left.updated_at);
    const rightNumeric = Number(right.updated_at);
    if (Number.isFinite(leftNumeric) && Number.isFinite(rightNumeric)) {
      return rightNumeric - leftNumeric;
    }
    return right.updated_at.localeCompare(left.updated_at);
  });
}

export const useAppStore = create<AppStore>()(
  persist(
    (set, get) => ({
      // Vault state
      vaultPath: null,
      notes: [],
      currentNote: null,
      isDirty: false,

      // Recent notes
      recentNotes: [],

      // Favorites
      favorites: [],

      // Tab state
      tabs: [],
      activeTabIndex: -1,
      closedTabs: [],
      splitActive: false,
      splitDirection: 'horizontal',
      splitTabIndex: -1,

      setVaultPath: (path) => set({
        vaultPath: path,
        navigationStack: [],
        navigationIndex: -1,
        hasBack: false,
        hasForward: false,
        favorites: [],
        tabs: [],
        activeTabIndex: -1,
        splitActive: false,
        workflowRuns: [],
        activeWorkflowRun: null,
        workflowRunError: null,
      }),
      setNotes: (notes) => set({ notes }),
      openNote: (path, content) => {
        const state = get();
        const currentPath = state.currentNote?.path ?? null;

        const tabs = state.tabs;
        const existingIdx = tabs.findIndex(t => t.path === path);
        const title = getTitleFromPath(path);

        if (existingIdx >= 0) {
          const updatedTabs = tabs.map((t, i) =>
            i === existingIdx ? { ...t, content: content || t.content, isDirty: false } : t
          );
          set({
            tabs: updatedTabs,
            activeTabIndex: existingIdx,
            currentNote: { path, content: content || tabs[existingIdx].content },
            isDirty: false,
          });
        } else {
          const newTab: TabState = {
            path, title,
            content: content || '',
            viewMode: 'edit',
            isDirty: false,
          };
          const newTabs = [...tabs, newTab];
          set({
            tabs: newTabs,
            activeTabIndex: newTabs.length - 1,
            currentNote: { path, content: newTab.content },
            isDirty: false,
          });
        }

        get().addRecentNote(path);

        if (!state._isNavigatingHistory) {
          if (currentPath !== path) {
            const newStack = state.navigationStack.slice(0, state.navigationIndex + 1);
            newStack.push(path);
            const newIndex = newStack.length - 1;
            set({
              navigationStack: newStack,
              navigationIndex: newIndex,
              hasBack: newIndex > 0,
              hasForward: newIndex < newStack.length - 1,
            });
          }
        }
      },
      setCurrentContent: (content) =>
        set((state) => {
          const tabs = [...state.tabs];
          const idx = state.activeTabIndex;
          if (idx >= 0 && idx < tabs.length) {
            tabs[idx] = { ...tabs[idx], content, isDirty: true };
          }
          return {
            currentNote: state.currentNote
              ? { ...state.currentNote, content }
              : null,
            isDirty: true,
            tabs,
          };
        }),
      markClean: () =>
        set((state) => {
          const tabs = [...state.tabs];
          const idx = state.activeTabIndex;
          if (idx >= 0 && idx < tabs.length) {
            tabs[idx] = { ...tabs[idx], isDirty: false };
          }
          return { isDirty: false, tabs };
        }),

      // Tab actions
      openTab: (path, title, content) => {
        const tabs = get().tabs;
        const existingIdx = tabs.findIndex(t => t.path === path);
        if (existingIdx >= 0) {
          const updatedTabs = tabs.map((t, i) =>
            i === existingIdx ? { ...t, content, isDirty: false } : t
          );
          set({
            tabs: updatedTabs,
            activeTabIndex: existingIdx,
            currentNote: { path, content },
            isDirty: false,
          });
        } else {
          const newTab: TabState = { path, title, content, viewMode: 'edit', isDirty: false };
          const newTabs = [...tabs, newTab];
          set({
            tabs: newTabs,
            activeTabIndex: newTabs.length - 1,
            currentNote: { path, content },
            isDirty: false,
          });
        }
        get().addRecentNote(path);
      },
      closeTab: (index) => {
        const state = get();
        const tabs = state.tabs;
        if (index < 0 || index >= tabs.length) return;
        const closedTab = tabs[index];
        const newClosed = [closedTab, ...state.closedTabs].slice(0, 10);
        const newTabs = tabs.filter((_, i) => i !== index);
        if (newTabs.length === 0) {
          set({
            tabs: [],
            activeTabIndex: -1,
            closedTabs: newClosed,
            currentNote: null,
            isDirty: false,
            splitActive: false,
            splitTabIndex: -1,
          });
          return;
        }
        let newIndex = state.activeTabIndex;
        if (newIndex >= newTabs.length) newIndex = newTabs.length - 1;
        if (index < newIndex) newIndex--;
        const activeTab = newTabs[newIndex];
        set({
          tabs: newTabs,
          activeTabIndex: newIndex,
          closedTabs: newClosed,
          currentNote: { path: activeTab.path, content: activeTab.content },
          isDirty: activeTab.isDirty,
          splitActive: false,
          splitTabIndex: -1,
        });
      },
      switchTab: (index) => {
        const tabs = get().tabs;
        if (index < 0 || index >= tabs.length) return;
        const tab = tabs[index];
        set({
          activeTabIndex: index,
          currentNote: { path: tab.path, content: tab.content },
          isDirty: tab.isDirty,
        });
      },
      toggleSplit: (direction) => {
        const state = get();
        if (state.splitActive) {
          set({ splitActive: false, splitTabIndex: -1 });
          return;
        }
        if (state.tabs.length < 2) return;
        if (direction === 'right') {
          set({ splitActive: true, splitDirection: 'horizontal', splitTabIndex: state.tabs.length > 1 ? (state.activeTabIndex === 0 ? 1 : 0) : -1 });
        } else if (direction === 'down') {
          set({ splitActive: true, splitDirection: 'vertical', splitTabIndex: state.tabs.length > 1 ? (state.activeTabIndex === 0 ? 1 : 0) : -1 });
        } else {
          set({ splitActive: true, splitDirection: 'horizontal', splitTabIndex: state.tabs.length > 1 ? (state.activeTabIndex === 0 ? 1 : 0) : -1 });
        }
      },
      closeAllTabs: () => {
        set({ tabs: [], activeTabIndex: -1, currentNote: null, isDirty: false, splitActive: false, splitTabIndex: -1 });
      },
      closeOtherTabs: (index) => {
        const tabs = get().tabs;
        if (index < 0 || index >= tabs.length) return;
        const kept = tabs[index];
        set({
          tabs: [kept],
          activeTabIndex: 0,
          currentNote: { path: kept.path, content: kept.content },
          isDirty: kept.isDirty,
          splitActive: false,
        });
      },
      restoreTab: () => {
        const closedTabs = get().closedTabs;
        if (closedTabs.length === 0) return;
        const [restored, ...remaining] = closedTabs;
        const tabs = get().tabs;
        const existingIdx = tabs.findIndex(t => t.path === restored.path);
        if (existingIdx >= 0) {
          set({ closedTabs: remaining, activeTabIndex: existingIdx });
          return;
        }
        const newTabs = [...tabs, restored];
        set({
          tabs: newTabs,
          activeTabIndex: newTabs.length - 1,
          closedTabs: remaining,
          currentNote: { path: restored.path, content: restored.content },
          isDirty: restored.isDirty,
        });
      },
      updateTabDirty: (path, isDirty) =>
        set((state) => {
          const tabs = state.tabs.map(t => t.path === path ? { ...t, isDirty } : t);
          const isActive = state.tabs[state.activeTabIndex]?.path === path;
          return { tabs, ...(isActive ? { isDirty } : {}) };
        }),
      updateTabViewMode: (path, mode) =>
        set((state) => ({
          tabs: state.tabs.map(t => t.path === path ? { ...t, viewMode: mode } : t),
        })),
      setTabContent: (path, content) =>
        set((state) => {
          const tabs = state.tabs.map(t => t.path === path ? { ...t, content, isDirty: true } : t);
          const isActive = state.tabs[state.activeTabIndex]?.path === path;
          return {
            tabs,
            ...(isActive ? { currentNote: { path, content }, isDirty: true } : {}),
          };
        }),
      markTabClean: (path) =>
        set((state) => {
          const tabs = state.tabs.map(t => t.path === path ? { ...t, isDirty: false } : t);
          const isActive = state.tabs[state.activeTabIndex]?.path === path;
          return { tabs, ...(isActive ? { isDirty: false } : {}) };
        }),

      // Recent notes actions
      addRecentNote: (path) => set((state) => {
        const filtered = state.recentNotes.filter((p) => p !== path);
        return { recentNotes: [path, ...filtered].slice(0, 20) };
      }),
      clearRecentNotes: () => set({ recentNotes: [] }),

      // Favorites actions
      toggleFavorite: (path) => set((state) => {
        if (state.favorites.includes(path)) {
          return { favorites: state.favorites.filter((p) => p !== path) };
        }
        return { favorites: [path, ...state.favorites] };
      }),
      isFavorite: (path) => get().favorites.includes(path),

      // Graph state
      graph: { nodes: [], edges: [] },
      setGraph: (graph) => set({ graph }),

      // Navigation breadcrumb path for graph
      navigationPath: [],
      addToNavigationPath: (path) =>
        set((state) => {
          const without = state.navigationPath.filter((p) => p !== path);
          const updated = [...without, path].slice(-10);
          return { navigationPath: updated };
        }),
      clearNavigationPath: () => set({ navigationPath: [] }),

      // CLI / Agent state
      tasks: [],
      addTask: (task) => set((state) => ({ tasks: [...state.tasks, task] })),
      updateTask: (id, updates) =>
        set((state) => ({
          tasks: state.tasks.map((t) => (t.id === id ? { ...t, ...updates } : t)),
        })),
      clearCompletedTasks: () =>
        set((state) => ({
          tasks: state.tasks.filter(
            (t) => t.status !== "done" && t.status !== "error" && t.status !== "cancelled"
          ),
        })),
      applyMockSimulationRun: (run) =>
        set((state) => ({
          tasks: [
            ...state.tasks.filter((task) => task.id !== run.task.id),
            run.task,
          ],
          diffBlocks: [
            ...state.diffBlocks.filter(
              (block) =>
                block.ghostId !== run.task.id &&
                !run.diffBlocks.some((incoming) => incoming.id === block.id),
            ),
            ...run.diffBlocks,
          ],
          orchestratorStatus: run.orchestratorStatus,
          bridgeProposals: [
            run.bridgeProposal,
            ...state.bridgeProposals.filter(
              (proposal) => proposal.id !== run.bridgeProposal.id,
            ),
          ],
          mode: "ai",
          activePanel: "tasks",
        })),

      // Diff state
      diffBlocks: [],
      setDiffBlocks: (blocks) => set({ diffBlocks: blocks }),
      updateDiffBlock: (id, updates) =>
        set((state) => ({
          diffBlocks: state.diffBlocks.map((b) => (b.id === id ? { ...b, ...updates } : b)),
        })),

      // Cursor state
      cursorLine: 1,
      cursorCol: 1,
      setCursorPos: (line, col) => set({ cursorLine: line, cursorCol: col }),

      // Save status
      saveStatus: "idle",
      setSaveStatus: (status) => set({ saveStatus: status }),

      // Sidebar state
      sidebarCollapsed: false,
      setSidebarCollapsed: (collapsed) => set({ sidebarCollapsed: collapsed }),

      // Tags
      allTags: [],
      setAllTags: (tags) => set({ allTags: tags }),
      filterTags: [],
      setFilterTags: (tags) => set({ filterTags: tags }),
      toggleFilterTag: (tag) =>
        set((state) => {
          const has = state.filterTags.includes(tag);
          return {
            filterTags: has
              ? state.filterTags.filter((t) => t !== tag)
              : [...state.filterTags, tag]
          };
        }),

      // Orchestrator state
      orchestratorStatus: null,
      workflowRuntimeEvents: [],
      workflowRuntimeEventRunId: null,
      workflowRuntimeEventError: null,
      workflowRuns: [],
      activeWorkflowRun: null,
      workflowRunLoading: false,
      workflowRunError: null,

      fetchOrchestratorStatus: async () => {
        try {
          const { invoke } = await import("@tauri-apps/api/core");
          const status = await invoke<OrchestratorStatus>("orchestrator_status");
          set({ orchestratorStatus: status });
        } catch {
          // Tauri not available or command failed
        }
      },

      fetchWorkflowRuntimeEvents: async (runId: string, limit = 200) => {
        try {
          const { invoke } = await import("@tauri-apps/api/core");
          const events = await invoke<HarnessEvent[]>("read_workflow_runtime_events", {
            runId,
            limit,
          });
          set({
            workflowRuntimeEvents: events,
            workflowRuntimeEventRunId: runId,
            workflowRuntimeEventError: null,
          });
          return events;
        } catch (error) {
          const message = error instanceof Error ? error.message : String(error);
          set({
            workflowRuntimeEvents: [],
            workflowRuntimeEventRunId: runId,
            workflowRuntimeEventError: message,
          });
          return [];
        }
      },

      applyWorkflowRunUpdate: (bundle) =>
        set((state) => ({
          workflowRuns: sortWorkflowRuns([
            bundle,
            ...state.workflowRuns.filter(
              (existing) => existing.run.run_id !== bundle.run.run_id,
            ),
          ]),
          activeWorkflowRun: bundle,
          workflowRunError: null,
        })),

      createAndStartWorkflow: async (goalText, acceptanceCriteria) => {
        set({ workflowRunLoading: true, workflowRunError: null });
        try {
          const { invoke } = await import("@tauri-apps/api/core");
          const bundle = await invoke<WorkflowRuntimeBundle>("create_and_start_workflow", {
            goalText,
            acceptanceCriteria,
          });
          get().applyWorkflowRunUpdate(bundle);
          set({ workflowRunLoading: false });
          return bundle;
        } catch (error) {
          set({
            workflowRunLoading: false,
            workflowRunError: error instanceof Error ? error.message : String(error),
          });
          return null;
        }
      },

      fetchWorkflowRuns: async () => {
        set({ workflowRunLoading: true, workflowRunError: null });
        try {
          const { invoke } = await import("@tauri-apps/api/core");
          const runs = sortWorkflowRuns(
            await invoke<WorkflowRuntimeBundle[]>("list_workflow_runs"),
          );
          set({
            workflowRuns: runs,
            activeWorkflowRun: runs[0] ?? null,
            workflowRunLoading: false,
          });
          return runs;
        } catch (error) {
          set({
            workflowRuns: [],
            activeWorkflowRun: null,
            workflowRunLoading: false,
            workflowRunError: error instanceof Error ? error.message : String(error),
          });
          return [];
        }
      },

      fetchWorkflowRun: async (runId) => {
        set({ workflowRunLoading: true, workflowRunError: null });
        try {
          const { invoke } = await import("@tauri-apps/api/core");
          const bundle = await invoke<WorkflowRuntimeBundle>("get_workflow_run", {
            runId,
          });
          get().applyWorkflowRunUpdate(bundle);
          set({ workflowRunLoading: false });
          return bundle;
        } catch (error) {
          set({
            workflowRunLoading: false,
            workflowRunError: error instanceof Error ? error.message : String(error),
          });
          return null;
        }
      },

      submitWorkflowPatchReview: async (runId: string, patch: WorkflowPatch) => {
        try {
          const { invoke } = await import("@tauri-apps/api/core");
          const proposal = await invoke<BridgeProposal>("submit_workflow_patch_review", {
            runId,
            patch,
          });
          set((state) => ({
            bridgeProposals: [
              proposal,
              ...state.bridgeProposals.filter((item) => item.id !== proposal.id),
            ],
            bridgeError: null,
          }));
          return proposal;
        } catch (error) {
          set({ bridgeError: error instanceof Error ? error.message : String(error) });
          return null;
        }
      },

      submitOrchestratorOutputReview: async (
        runId: string,
        output: OrchestratorOutput,
      ) => {
        try {
          const { invoke } = await import("@tauri-apps/api/core");
          const proposal = await invoke<BridgeProposal | null>(
            "submit_orchestrator_output_review",
            {
              runId,
              output,
            },
          );
          if (proposal) {
            set((state) => ({
              bridgeProposals: [
                proposal,
                ...state.bridgeProposals.filter((item) => item.id !== proposal.id),
              ],
              bridgeError: null,
            }));
          }
          return proposal;
        } catch (error) {
          set({ bridgeError: error instanceof Error ? error.message : String(error) });
          return null;
        }
      },

      terminateAgent: async (agentId: string) => {
        try {
          const { invoke } = await import("@tauri-apps/api/core");
          return await invoke<boolean>("orchestrator_terminate", { agent_id: agentId });
        } catch {
          return false;
        }
      },

      reinstateAgent: async (agentId: string) => {
        try {
          const { invoke } = await import("@tauri-apps/api/core");
          return await invoke<boolean>("orchestrator_reinstate", { agent_id: agentId });
        } catch {
          return false;
        }
      },

      // Agent tools
      agentTools: [],
      fetchAgentTools: async () => {
        try {
          const { invoke } = await import("@tauri-apps/api/core");
          const tools = await invoke<ToolInfo[]>("list_agent_tools");
          set({ agentTools: tools });
        } catch {
          // non-critical
        }
      },

      // Bridge proposal inbox
      bridgeProposals: [],
      bridgeLoading: false,
      bridgeError: null,
      fetchBridgeProposals: async () => {
        set({ bridgeLoading: true, bridgeError: null });
        try {
          const { invoke } = await import("@tauri-apps/api/core");
          const proposals = await invoke<BridgeProposal[]>("list_bridge_proposals", {});
          set({ bridgeProposals: proposals, bridgeLoading: false });
        } catch (error) {
          set({
            bridgeError: error instanceof Error ? error.message : String(error),
            bridgeLoading: false,
          });
        }
      },
      executeBridgeAction: async (proposalId, actionId) => {
        try {
          const { invoke } = await import("@tauri-apps/api/core");
          const result = await invoke<BridgeProposalActionResult>("execute_bridge_action", {
            id: proposalId,
            actionId,
          });
          const navigationUpdate = navigationUpdateFromMetadata(result.metadata, get().mode);
          if (navigationUpdate) {
            set(navigationUpdate);
          }
          await get().fetchBridgeProposals();
          return result;
        } catch (error) {
          const message = error instanceof Error ? error.message : String(error);
          const fallbackResult = localBridgeNavigationResult(
            get().bridgeProposals.find((proposal) => proposal.id === proposalId),
            actionId,
            message,
          );
          const navigationUpdate = navigationUpdateFromMetadata(fallbackResult?.metadata, get().mode);
          if (fallbackResult && navigationUpdate) {
            set({ bridgeError: message, ...navigationUpdate });
            return fallbackResult;
          }
          set({ bridgeError: message });
          return null;
        }
      },

      // UI state
      activePanel: "editor",
      setActivePanel: (panel) => set((state) => {
        const mode = modeForPanel(panel, state.mode);
        return {
          mode,
          activePanel: resolveActivePanelForMode(panel, mode),
        };
      }),
      isCliActive: false,
      setCliActive: (active) => set({ isCliActive: active }),
      mode: "ai",
      setMode: (mode) => set((state) => ({
        mode,
        activePanel: resolveActivePanelForMode(state.activePanel, mode),
      })),

      // Navigation history
      _isNavigatingHistory: false,
      navigationStack: [],
      navigationIndex: -1,
      hasBack: false,
      hasForward: false,
      goBack: () => {
        const state = get();
        if (state.navigationIndex <= 0) return;
        const newIndex = state.navigationIndex - 1;
        const path = state.navigationStack[newIndex];
        if (!path) return;
        set({
          _isNavigatingHistory: true,
          navigationIndex: newIndex,
          hasBack: newIndex > 0,
          hasForward: newIndex < state.navigationStack.length - 1,
        });
        get().openNote(path, "");
        set({ _isNavigatingHistory: false });
      },
      goForward: () => {
        const state = get();
        if (state.navigationIndex >= state.navigationStack.length - 1) return;
        const newIndex = state.navigationIndex + 1;
        const path = state.navigationStack[newIndex];
        if (!path) return;
        set({
          _isNavigatingHistory: true,
          navigationIndex: newIndex,
          hasBack: newIndex > 0,
          hasForward: newIndex < state.navigationStack.length - 1,
        });
        get().openNote(path, "");
        set({ _isNavigatingHistory: false });
      },
    }),
    {
      name: "bayesian-notes-store",
      partialize: (state) => ({
        vaultPath: state.vaultPath,
        currentNote: state.currentNote,
        recentNotes: state.recentNotes,
        navigationStack: state.navigationStack,
        favorites: state.favorites,
        tabs: state.tabs,
        activeTabIndex: state.activeTabIndex,
      }),
      onRehydrateStorage: () => () => {
        if (typeof window !== "undefined" && !(window as any).__TAURI_INTERNALS__) {
          const store = useAppStore.getState();
          if (!store.vaultPath) {
            store.setNotes([
              { path: "Welcome.md", title: "Welcome", size: 256, modified: new Date().toISOString(), created: new Date().toISOString(), links: ["getting-started"], tags: ["welcome"] },
              { path: "Getting Started.md", title: "Getting Started", size: 512, modified: new Date().toISOString(), created: new Date().toISOString(), links: ["architecture"], tags: ["guide"] },
              { path: "Architecture.md", title: "Architecture", size: 1024, modified: new Date().toISOString(), created: new Date().toISOString(), links: ["dual-track", "rust"], tags: ["design"] },
              { path: "Dual Track.md", title: "Dual Track", size: 768, modified: new Date().toISOString(), created: new Date().toISOString(), links: ["architecture", "llm"], tags: ["design"] },
            ]);
            store.setVaultPath("/demo-vault");
          }
        }
      },
    }
  )
);
