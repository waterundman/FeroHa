// Global state management for Dual-Track Note IDE
// Uses Zustand for lightweight, hook-based state

import { create } from "zustand";
import { persist } from "zustand/middleware";

export interface NoteMeta {
  path: string;
  title: string;
  size: number;
  modified: string;
  created: string;
  links: string[];
}

export interface TaskStatus {
  id: string;
  command: string;
  status: "queued" | "running" | "done" | "error";
  result?: string;
}

export interface GraphNode {
  id: string;
  title: string;
  outgoing: number;
  incoming: number;
}

export interface GraphEdge {
  from: string;
  to: string;
}

export interface GraphData {
  nodes: GraphNode[];
  edges: GraphEdge[];
}

export interface DiffBlock {
  id: string;
  type: "added" | "removed" | "modified";
  oldText?: string;
  newText?: string;
  accepted: boolean;
  rejected: boolean;
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

  // Graph state
  graph: GraphData;
  setGraph: (graph: GraphData) => void;

  // CLI / Agent state
  tasks: TaskStatus[];
  addTask: (task: TaskStatus) => void;
  updateTask: (id: string, updates: Partial<TaskStatus>) => void;

  // Diff state
  diffBlocks: DiffBlock[];
  setDiffBlocks: (blocks: DiffBlock[]) => void;
  updateDiffBlock: (id: string, updates: Partial<DiffBlock>) => void;

  // UI state
  activePanel: "editor" | "graph" | "diff";
  setActivePanel: (panel: "editor" | "graph" | "diff") => void;
  isCliActive: boolean;
  setCliActive: (active: boolean) => void;
  mode: "human" | "ai";
  setMode: (mode: "human" | "ai") => void;
}

export const useAppStore = create<AppStore>()(
  persist(
    (set) => ({
      // Vault state
      vaultPath: null,
      notes: [],
      currentNote: null,
      isDirty: false,

      setVaultPath: (path) => set({ vaultPath: path }),
      setNotes: (notes) => set({ notes }),
      openNote: (path, content) => set({ currentNote: { path, content }, isDirty: false }),
      setCurrentContent: (content) =>
        set((state) => ({
          currentNote: state.currentNote
            ? { ...state.currentNote, content }
            : null,
          isDirty: true,
        })),
      markClean: () => set({ isDirty: false }),

      // Graph state
      graph: { nodes: [], edges: [] },
      setGraph: (graph) => set({ graph }),

      // CLI / Agent state
      tasks: [],
      addTask: (task) => set((state) => ({ tasks: [...state.tasks, task] })),
      updateTask: (id, updates) =>
        set((state) => ({
          tasks: state.tasks.map((t) => (t.id === id ? { ...t, ...updates } : t)),
        })),

      // Diff state
      diffBlocks: [],
      setDiffBlocks: (blocks) => set({ diffBlocks: blocks }),
      updateDiffBlock: (id, updates) =>
        set((state) => ({
          diffBlocks: state.diffBlocks.map((b) => (b.id === id ? { ...b, ...updates } : b)),
        })),

      // UI state
      activePanel: "editor",
      setActivePanel: (panel) => set({ activePanel: panel }),
      isCliActive: false,
      setCliActive: (active) => set({ isCliActive: active }),
      mode: "ai",
      setMode: (mode) => set({ mode }),
    }),
    {
      name: "bayesian-notes-store",
      partialize: (state) => ({
        vaultPath: state.vaultPath,
        currentNote: state.currentNote,
      }),
    }
  )
);
