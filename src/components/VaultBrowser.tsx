import { useState, useEffect, useCallback, useMemo, useRef } from "react";
import { useAppStore, type NoteMeta, type GraphData } from "../hooks/useAppStore";
import { showToast } from "./toastBus";
import TemplatePicker from "./TemplatePicker";
import FeroHaIcon from "./FeroHaIcon";
import ContextMenu, { type ContextMenuItem } from "./ContextMenu";

interface VaultBrowserProps {
  vaultPath: string | null;
  onSelectVault: (path: string) => void;
  isTauri: boolean;
  templatePickerOpen?: boolean;
  onTemplatePickerClose?: () => void;
}

interface TreeNode {
  name: string;
  path: string;
  sourcePath?: string;
  isFolder: boolean;
  depth: number;
  children: TreeNode[];
  meta?: NoteMeta;
}

interface FolderMeta {
  path: string;
  name: string;
}

interface FileChangedPayload {
  path: string;
  kind: "created" | "modified" | "deleted" | string;
}

type SortMode = "title-asc" | "title-desc" | "modified-desc" | "modified-asc";
export type AiWorkspaceZone = "working" | "semantic" | "long_term";

interface AiWorkspaceZoneDefinition {
  id: AiWorkspaceZone;
  title: string;
  description: string;
  icon: string;
  accentColor: string;
  canonicalRoot: string;
  legacyRoots: string[];
}

const SKELETON_LINE_WIDTHS = ["68%", "84%", "72%", "91%", "76%"];

const AI_WORKSPACE_ZONES: AiWorkspaceZoneDefinition[] = [
  {
    id: "working",
    title: "Working Memory",
    description: "当前任务、快照与近期研究上下文",
    icon: "Activity",
    accentColor: "#94e2d5",
    canonicalRoot: ".dualtrack/memory/working/",
    legacyRoots: [
      ".dualtrack/research/",
      ".dualtrack/snapshots/",
      ".dualtrack/output/",
      ".dualtrack/bridge/",
      ".dualtrack/ghosts/",
    ],
  },
  {
    id: "semantic",
    title: "Semantic Memory",
    description: "claims、JSON-LD、MDT 与结构索引",
    icon: "Network",
    accentColor: "#cba6f7",
    canonicalRoot: ".dualtrack/memory/semantic/",
    legacyRoots: [
      ".dualtrack/jsonld/",
      ".dualtrack/mdt/",
      ".dualtrack/fts/",
      ".dualtrack/vectors/",
    ],
  },
  {
    id: "long_term",
    title: "Long-Term Memory",
    description: "Dream 巩固洞见与冷却归档",
    icon: "Archive",
    accentColor: "#a6adc8",
    canonicalRoot: ".dualtrack/memory/long_term/",
    legacyRoots: [
      ".dualtrack/dream/",
      ".dualtrack/archive/",
      ".dualtrack/imports/",
    ],
  },
];

const Icon = ({ d, s = 14 }: { d: string; s?: number }) => (
  <svg width={s} height={s} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
    <path d={d} />
  </svg>
);

function normalizeVaultPath(path: string): string {
  return path.replace(/\\/g, "/");
}

export function normalizeHumanFolderPath(path: string): string | null {
  const normalized = normalizeVaultPath(path)
    .split("/")
    .map((part) => part.trim())
    .filter(Boolean)
    .join("/");
  if (!normalized) return null;
  const invalid = normalized.split("/").some((part) =>
    part === "." ||
    part === ".." ||
    part.startsWith(".") ||
    part.startsWith("_")
  );
  return invalid ? null : normalized;
}

export function humanFolderPathFromInput(parentPath: string | undefined, input: string): string | null {
  const child = normalizeHumanFolderPath(input);
  if (!child) return null;
  const parent = parentPath ? normalizeHumanFolderPath(parentPath) : null;
  return parent ? `${parent}/${child}` : child;
}

export function isAiWorkspacePath(path: string): boolean {
  return normalizeVaultPath(path).startsWith(".dualtrack/");
}

export function aiWorkspaceZoneForPath(path: string): AiWorkspaceZone | null {
  const normalized = normalizeVaultPath(path);
  if (!isAiWorkspacePath(normalized)) return null;

  for (const zone of AI_WORKSPACE_ZONES) {
    if (
      normalized.startsWith(zone.canonicalRoot) ||
      zone.legacyRoots.some((root) => normalized.startsWith(root))
    ) {
      return zone.id;
    }
  }

  return "working";
}

export function aiWorkspaceDisplayPath(path: string): string {
  const normalized = normalizeVaultPath(path);
  const zone = AI_WORKSPACE_ZONES.find((candidate) => normalized.startsWith(candidate.canonicalRoot));
  if (zone) return normalized.slice(zone.canonicalRoot.length) || zone.id;
  return normalized.replace(/^\.dualtrack\//, "");
}

export function mergeVaultNoteLists(humanNotes: NoteMeta[], aiWorkspaceNotes: NoteMeta[]): NoteMeta[] {
  const merged = new Map<string, NoteMeta>();
  for (const note of humanNotes) merged.set(note.path, note);
  for (const note of aiWorkspaceNotes) merged.set(note.path, note);
  return Array.from(merged.values());
}

function sourcePathForAiDisplayPath(zone: AiWorkspaceZone, displayPath: string): string {
  const zoneDef = AI_WORKSPACE_ZONES.find((candidate) => candidate.id === zone);
  return `${zoneDef?.canonicalRoot ?? ".dualtrack/memory/working/"}${displayPath}`.replace(/\/+$/, "/");
}

function treeNodeActualPath(node: TreeNode): string {
  return node.sourcePath ?? node.path;
}

function folderPathChain(path: string): string[] {
  const parts = normalizeVaultPath(path).split("/").filter(Boolean);
  return parts.map((_, index) => `${parts.slice(0, index + 1).join("/")}/`);
}

function buildTree(
  notes: NoteMeta[],
  sortMode: SortMode,
  options: {
    pathForNote?: (note: NoteMeta) => string;
    sourcePathForTreePath?: (treePath: string) => string;
    folderPaths?: string[];
  } = {},
): TreeNode[] {
  const folderMap = new Map<string, TreeNode>();
  const pathForNote = options.pathForNote ?? ((note: NoteMeta) => note.path);
  const roots: TreeNode[] = [];

  const getFolder = (parts: string[]): TreeNode => {
    let parent: TreeNode | null = null;
    for (let i = 0; i < parts.length; i++) {
      const segment = parts[i];
      const depth = i;
      const key = depth === 0 ? segment : `${i > 0 ? `${parts.slice(0, i).join("/")}/` : ""}${segment}`;
      if (!folderMap.has(key)) {
        const folderPath = key + "/";
        const node: TreeNode = {
          name: segment,
          path: folderPath,
          sourcePath: options.sourcePathForTreePath?.(folderPath),
          isFolder: true,
          depth,
          children: [],
        };
        folderMap.set(key, node);
        if (parent) {
          if (!parent.children.find(c => c.path === node.path)) {
            parent.children.push(node);
          }
        } else if (!roots.find(r => r.path === node.path)) {
          roots.push(node);
        }
        parent = node;
      } else {
        parent = folderMap.get(key)!;
      }
    }
    return parent!;
  };

  for (const folderPath of options.folderPaths ?? []) {
    const parts = folderPath.replace(/^\/+|\/+$/g, "").split("/").filter(Boolean);
    if (parts.length > 0) getFolder(parts);
  }

  for (const note of notes) {
    const treePath = pathForNote(note).replace(/^\/+/, "");
    const parts = treePath.split("/").filter(Boolean);
    if (parts.length === 0) continue;
    if (parts.length === 1) {
      roots.push({
        name: note.title,
        path: treePath,
        sourcePath: note.path,
        isFolder: false,
        depth: 0,
        children: [],
        meta: note,
      });
    } else {
      const folderParts = parts.slice(0, -1);
      const folderPath = folderParts.join("/");
      getFolder(folderParts);

      const fileNode: TreeNode = {
        name: note.title,
        path: treePath,
        sourcePath: note.path,
        isFolder: false,
        depth: folderParts.length,
        children: [],
        meta: note,
      };

      const parentKey = folderPath;
      const parentFolder = folderMap.get(parentKey);
      if (parentFolder) {
        if (!parentFolder.children.find(c => c.path === fileNode.path)) {
          parentFolder.children.push(fileNode);
        }
      } else {
        if (!roots.find(r => r.path === fileNode.path)) {
          roots.push(fileNode);
        }
      }
    }
  }

  const sortNodes = (nodes: TreeNode[]) => {
    nodes.sort((a, b) => {
      if (a.isFolder !== b.isFolder) return a.isFolder ? -1 : 1;
      if (sortMode === "title-asc") return a.name.localeCompare(b.name);
      if (sortMode === "title-desc") return b.name.localeCompare(a.name);
      if (sortMode === "modified-desc") {
        const aMod = a.meta ? parseInt(a.meta.modified || "0", 10) : 0;
        const bMod = b.meta ? parseInt(b.meta.modified || "0", 10) : 0;
        return bMod - aMod;
      }
      if (sortMode === "modified-asc") {
        const aMod = a.meta ? parseInt(a.meta.modified || "0", 10) : 0;
        const bMod = b.meta ? parseInt(b.meta.modified || "0", 10) : 0;
        return aMod - bMod;
      }
      return 0;
    });
    for (const node of nodes) {
      if (node.children.length > 0) sortNodes(node.children);
    }
  };

  sortNodes(roots);
  return roots;
}

export function vaultContextMenuActionIdsForNode(node: { path: string; isFolder: boolean }): string[] {
  const isAiNode = isAiWorkspacePath(node.path);
  if (node.isFolder) {
    return isAiNode
      ? ["open-readonly", "folder-ai-context", "focus-graph", "copy-path"]
      : ["new-note", "new-folder", "folder-ai-context", "copy-path"];
  }
  return isAiNode
    ? ["open-readonly", "ask-ai", "focus-graph", "copy-path"]
    : ["ask-ai", "focus-graph", "copy-path", "favorite", "rename", "delete"];
}

export default function VaultBrowser({ vaultPath, onSelectVault, isTauri, templatePickerOpen: tpo, onTemplatePickerClose }: VaultBrowserProps) {
  const [activeNote, setActiveNote] = useState<string | null>(null);
  const [hoveredNote, setHoveredNote] = useState<string | null>(null);
  const [sortBy, setSortBy] = useState<SortMode>("title-asc");
  const [searchQuery, setSearchQuery] = useState("");
  const [isLoading, setIsLoading] = useState(false);
  const [loadingOperations, setLoadingOperations] = useState<Set<string>>(new Set());
  const [folderPaths, setFolderPaths] = useState<string[]>([]);
  const [manualVaultPath, setManualVaultPath] = useState("");
  const [recentCollapsed, setRecentCollapsed] = useState(false);
  const [favoritesCollapsed, setFavoritesCollapsed] = useState(false);
  const [humanCollapsed, setHumanCollapsed] = useState(false);
  const [aiZoneCollapsed, setAiZoneCollapsed] = useState<Record<AiWorkspaceZone, boolean>>({
    working: false,
    semantic: false,
    long_term: false,
  });
  const [isTemplatePickerOpen, setTemplatePickerOpen] = useState(false);
  const [newNoteFolder, setNewNoteFolder] = useState<string | null>(null);
  const [expandedFolders, setExpandedFolders] = useState<Set<string>>(new Set());
  const [editingPath, setEditingPath] = useState<string | null>(null);
  const [editingValue, setEditingValue] = useState("");
  const [contextMenu, setContextMenu] = useState<{ x: number; y: number; item: TreeNode } | null>(null);
  const editInputRef = useRef<HTMLInputElement>(null);

  const recentNotes = useAppStore((s) => s.recentNotes);
  const clearRecentNotes = useAppStore((s) => s.clearRecentNotes);
  const favorites = useAppStore((s) => s.favorites);
  const toggleFavorite = useAppStore((s) => s.toggleFavorite);
  const storeNotes = useAppStore((s) => s.notes);
  const setStoreNotes = useAppStore((s) => s.setNotes);
  const setGraph = useAppStore((s) => s.setGraph);
  const setAllTags = useAppStore((s) => s.setAllTags);
  const filterTags = useAppStore((s) => s.filterTags);
  const setFilterTags = useAppStore((s) => s.setFilterTags);

  const filteredNotes = useMemo(() => {
    const tagFiltered = filterTags.length === 0 ? storeNotes : storeNotes.filter(note => {
      const noteTags = note.tags || [];
      return filterTags.every(ft => noteTags.includes(ft));
    });
    const query = searchQuery.trim().toLowerCase();
    if (!query) return tagFiltered;
    return tagFiltered.filter((note) =>
      note.title.toLowerCase().includes(query) ||
      note.path.toLowerCase().includes(query) ||
      note.tags?.some((tag) => tag.toLowerCase().includes(query))
    );
  }, [storeNotes, filterTags, searchQuery]);

  const humanNotes = useMemo(() => filteredNotes.filter(n => !isAiWorkspacePath(n.path)), [filteredNotes]);
  const aiNotes = useMemo(() => filteredNotes.filter(n => isAiWorkspacePath(n.path)), [filteredNotes]);
  const humanNoteTotal = useMemo(() => storeNotes.filter(n => !isAiWorkspacePath(n.path)).length, [storeNotes]);
  const aiNoteTotal = useMemo(() => storeNotes.filter(n => isAiWorkspacePath(n.path)).length, [storeNotes]);

  const filteredFolderPaths = useMemo(() => {
    const query = searchQuery.trim().toLowerCase();
    if (!query) return folderPaths;

    const noteFolderAncestors = new Set<string>();
    for (const note of humanNotes) {
      const parts = normalizeVaultPath(note.path).split("/").filter(Boolean);
      for (let index = 1; index < parts.length; index += 1) {
        noteFolderAncestors.add(parts.slice(0, index).join("/"));
      }
    }

    return folderPaths.filter((folderPath) =>
      folderPath.toLowerCase().includes(query) || noteFolderAncestors.has(folderPath)
    );
  }, [folderPaths, humanNotes, searchQuery]);

  const humanTree = useMemo(
    () => buildTree(humanNotes, sortBy, { folderPaths: filteredFolderPaths }),
    [humanNotes, sortBy, filteredFolderPaths],
  );
  const hasVisibleVaultItems = filteredNotes.length > 0 || filteredFolderPaths.length > 0;
  const aiZoneTrees = useMemo(() => {
    return AI_WORKSPACE_ZONES.reduce((acc, zone) => {
      const zoneNotes = aiNotes.filter((note) => aiWorkspaceZoneForPath(note.path) === zone.id);
      acc[zone.id] = buildTree(zoneNotes, sortBy, {
        pathForNote: (note) => aiWorkspaceDisplayPath(note.path),
        sourcePathForTreePath: (treePath) => sourcePathForAiDisplayPath(zone.id, treePath),
      });
      return acc;
    }, {} as Record<AiWorkspaceZone, TreeNode[]>);
  }, [aiNotes, sortBy]);

  const favoriteMeta = useMemo(() => {
    return favorites
      .map((path) => storeNotes.find((n) => n.path === path))
      .filter((n): n is NoteMeta => n != null);
  }, [favorites, storeNotes]);

  const refreshNotesRef = useRef(async () => {
    if (!isTauri) return;
    setIsLoading(true);
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const [humanNoteList, humanFolderList, aiWorkspaceNoteList] = await Promise.all([
        invoke<NoteMeta[]>("list_notes"),
        invoke<FolderMeta[]>("list_folders").catch(() => [] as FolderMeta[]),
        invoke<NoteMeta[]>("list_ai_workspace_files").catch(() => [] as NoteMeta[]),
      ]);
      setStoreNotes(mergeVaultNoteLists(humanNoteList, aiWorkspaceNoteList));
      setFolderPaths(humanFolderList.map((folder) => normalizeVaultPath(folder.path)));
      const graph = await invoke<GraphData>("get_graph");
      setGraph(graph);
      const tags = await invoke<{ name: string; count: number }[]>("list_tags");
      setAllTags(tags);
    } catch {
      // Vault not open
    } finally {
      setIsLoading(false);
    }
  });
  const refreshNotes = useCallback(() => refreshNotesRef.current(), []);

  useEffect(() => {
    if (!isTauri) return;
    let unlisten: (() => void) | undefined;
    const setupListener = async () => {
      try {
        const { listen } = await import("@tauri-apps/api/event");
        const unsub = await listen<FileChangedPayload>("file-changed", () => {
          refreshNotes();
        });
        unlisten = unsub;
      } catch {
        // Browser mode
      }
  };
  setupListener();
  return () => { unlisten?.(); };
  }, [isTauri, vaultPath, refreshNotes]);

  useEffect(() => {
    if (!isTauri || !vaultPath) return;
    refreshNotes();
  }, [isTauri, vaultPath, refreshNotes]);

  const handleOpenVault = async () => {
    if (isTauri) {
      try {
        const [{ invoke }, { open }] = await Promise.all([
          import("@tauri-apps/api/core"),
          import("@tauri-apps/plugin-dialog"),
        ]);
        const selected = await open({ directory: true, multiple: false });
        if (selected && typeof selected === "string") {
          await invoke("open_vault", { path: selected });
          onSelectVault(selected);
          refreshNotes();
        }
      } catch (e) {
        console.error("Failed to open vault:", e);
        showToast("error", "打开库失败");
      }
    } else {
      onSelectVault("/demo-vault");
    }
  };

  const handleOpenManualVault = async () => {
    const selected = manualVaultPath.trim();
    if (!selected) return;

    if (isTauri) {
      setIsLoading(true);
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        await invoke("open_vault", { path: selected });
        onSelectVault(selected);
        setManualVaultPath("");
        refreshNotes();
      } catch (e) {
        console.error("Failed to open vault path:", e);
        showToast("error", `Failed to open vault: ${selected}`);
      } finally {
        setIsLoading(false);
      }
    } else {
      onSelectVault(selected);
    }
  };

  const openNoteInEditor = useCallback((notePath: string, title: string) => {
    setActiveNote(notePath);
    if (isTauri) {
      import("@tauri-apps/api/core").then(({ invoke }) => {
        invoke<string>("read_note", { path: notePath }).then((content) => {
          useAppStore.getState().openNote(notePath, content);
        }).catch(console.error);
      });
    } else {
      useAppStore.getState().openNote(notePath, `# ${title}\n\n`);
    }
  }, [isTauri]);

  const handleNoteClick = (note: TreeNode) => {
    if (note.meta) {
      openNoteInEditor(treeNodeActualPath(note), note.name);
    }
  };

  const handleRecentClick = (notePath: string) => {
    const meta = storeNotes.find(n => n.path === notePath);
    const title = meta?.title || notePath.split("/").pop()?.replace(/\.md$/, "") || notePath;
    openNoteInEditor(notePath, title);
  };

  const handleTemplateSelect = async (content: string, fileName: string) => {
    setTemplatePickerOpen(false);
    onTemplatePickerClose?.();
    const fullPath = newNoteFolder ? `${newNoteFolder}/${fileName}` : fileName;
    setNewNoteFolder(null);

    const operationId = `create-${fullPath}`;
    setLoadingOperations(prev => new Set(prev).add(operationId));

    if (isTauri) {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        await invoke("create_note", { path: fullPath });
        await invoke("save_note", { path: fullPath, content });
        setActiveNote(fullPath);
        const readContent = await invoke<string>("read_note", { path: fullPath });
        useAppStore.getState().openNote(fullPath, readContent);
        refreshNotes();
        showToast("success", `Created ${fullPath}`);
      } catch (e) {
        console.error("Failed to create note:", e);
        showToast("error", `创建失败：${fullPath}`);
      }
    } else {
      const newNote: NoteMeta = {
        path: fullPath,
        title: fullPath.replace(/\.md$/, ""),
        size: content.length,
        modified: new Date().toISOString(),
        created: new Date().toISOString(),
        links: [],
        tags: [],
      };
      const prev = useAppStore.getState().notes;
      if (!prev.find((n) => n.path === fullPath)) {
        setStoreNotes([...prev, newNote]);
      }
      useAppStore.getState().openNote(fullPath, content);
      showToast("success", `Created ${fullPath}`);
    }

    setLoadingOperations(prev => {
      const next = new Set(prev);
      next.delete(operationId);
      return next;
    });
  };

  const removeDeletedNoteFromWorkspace = (notePath: string) => {
    const store = useAppStore.getState();
    const tabIndex = store.tabs.findIndex((tab) => tab.path === notePath);
    if (tabIndex >= 0) {
      store.closeTab(tabIndex);
    }
    useAppStore.setState((state) => ({
      currentNote: state.currentNote?.path === notePath ? null : state.currentNote,
      isDirty: state.currentNote?.path === notePath ? false : state.isDirty,
      closedTabs: state.closedTabs.filter((tab) => tab.path !== notePath),
      favorites: state.favorites.filter((path) => path !== notePath),
      recentNotes: state.recentNotes.filter((path) => path !== notePath),
    }));
  };

  const handleDeleteNote = async (notePath: string, e?: Pick<React.MouseEvent, "stopPropagation">) => {
    e?.stopPropagation();
    if (!confirm(`删除 "${notePath}"？`)) return;

    const operationId = `delete-${notePath}`;
    setLoadingOperations(prev => new Set(prev).add(operationId));

    if (isTauri) {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        await invoke("delete_note", { path: notePath });
        setStoreNotes(useAppStore.getState().notes.filter((n) => n.path !== notePath));
        if (activeNote === notePath) setActiveNote(null);
        removeDeletedNoteFromWorkspace(notePath);
        refreshNotes();
        showToast("success", `已删除 ${notePath}`);
      } catch (e) {
        console.error("Failed to delete note:", e);
        showToast("error", `删除失败：${notePath}`);
      }
    } else {
      setStoreNotes(useAppStore.getState().notes.filter((n) => n.path !== notePath));
      if (activeNote === notePath) setActiveNote(null);
      removeDeletedNoteFromWorkspace(notePath);
      showToast("success", `已删除 ${notePath}`);
    }

    setLoadingOperations(prev => {
      const next = new Set(prev);
      next.delete(operationId);
      return next;
    });
  };

  const addVisibleFolderPath = (folderPath: string) => {
    const normalized = normalizeHumanFolderPath(folderPath);
    if (!normalized) return;
    setFolderPaths((current) => (
      current.includes(normalized) ? current : [...current, normalized]
    ));
    setExpandedFolders((current) => {
      const next = new Set(current);
      for (const segment of folderPathChain(normalized)) {
        next.add(segment);
      }
      return next;
    });
  };

  const handleCreateFolder = async (prefix?: string) => {
    setContextMenu(null);
    const name = prompt("新文件夹名称：");
    if (!name) return;
    const folderPath = humanFolderPathFromInput(prefix, name);
    if (!folderPath) {
      showToast("error", "文件夹名称不能是隐藏目录、内部目录或上级路径");
      return;
    }

    const operationId = `createFolder-${folderPath}`;
    setLoadingOperations(prev => new Set(prev).add(operationId));

    if (isTauri) {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        await invoke("create_folder", { path: folderPath });
        addVisibleFolderPath(folderPath);
        refreshNotes();
        showToast("success", `Created folder ${folderPath}`);
      } catch (e) {
        console.error("Failed to create folder:", e);
        showToast("error", `创建文件夹失败：${folderPath}`);
      }
    } else {
      addVisibleFolderPath(folderPath);
      showToast("success", `Created folder ${folderPath}`);
    }

    setLoadingOperations(prev => {
      const next = new Set(prev);
      next.delete(operationId);
      return next;
    });
  };

  const startRename = (treeNode: TreeNode) => {
    if (treeNode.isFolder || !treeNode.meta) return;
    setEditingPath(treeNode.path);
    setEditingValue(treeNode.name);
    setTimeout(() => {
      if (editInputRef.current) {
        editInputRef.current.focus();
        editInputRef.current.select();
      }
    }, 0);
  };

  const confirmRename = async () => {
    if (!editingPath || !editingValue.trim()) {
      setEditingPath(null);
      return;
    }
    const newName = editingValue.trim();
    const parts = editingPath.split("/");
    parts[parts.length - 1] = newName + ".md";
    const newPath = parts.join("/");

    if (newPath === editingPath) {
      setEditingPath(null);
      return;
    }

    if (isTauri) {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        await invoke("rename_note", { oldPath: editingPath, newPath });
        refreshNotes();
        showToast("success", `已重命名为 ${newPath}`);
      } catch (e) {
        console.error("Failed to rename note:", e);
        showToast("error", `重命名失败：${e}`);
      }
    }
    setEditingPath(null);
  };

  const cancelRename = () => {
    setEditingPath(null);
  };

  const toggleFolder = (folderPath: string) => {
    setExpandedFolders(prev => {
      const next = new Set(prev);
      if (next.has(folderPath)) {
        next.delete(folderPath);
      } else {
        next.add(folderPath);
      }
      return next;
    });
  };

  const handleContextMenu = (e: React.MouseEvent, item: TreeNode) => {
    e.preventDefault();
    e.stopPropagation();
    setContextMenu({ x: e.clientX, y: e.clientY, item });
  };

  const openTaskIntakeForPath = (path: string) => {
    useAppStore.getState().setActivePanel("task-intake");
    showToast("info", `已将 ${path} 作为 AI 任务上下文`);
  };

  const focusPathInGraph = (path: string) => {
    const store = useAppStore.getState();
    store.addToNavigationPath(path);
    store.setActivePanel("graph");
    showToast("info", `图谱聚焦：${path}`);
  };

  const copyPath = (path: string) => {
    void navigator.clipboard?.writeText(path);
    showToast("info", `已复制路径：${path}`);
  };

  const openReadOnlyNote = (item: TreeNode) => {
    if (!item.meta) return;
    const itemPath = treeNodeActualPath(item);
    openNoteInEditor(itemPath, item.name);
    showToast("info", `AI 工作区以只读方式打开：${itemPath}`);
  };

  const contextMenuItemsForNode = (item: TreeNode): ContextMenuItem[] => {
    const itemPath = treeNodeActualPath(item);
    const folderPath = itemPath.replace(/\/$/, "");
    const builders: Record<string, () => ContextMenuItem> = {
      "open-readonly": () => ({
        id: "open-readonly",
        label: item.isFolder ? "展开查看" : "只读打开",
        icon: item.isFolder ? "FolderOpen" : "BookOpen",
        onSelect: () => {
          if (item.isFolder) toggleFolder(item.path);
          else openReadOnlyNote(item);
        },
      }),
      "new-note": () => ({
        id: "new-note",
        label: "新建笔记",
        icon: "FilePlus",
        onSelect: () => {
          setNewNoteFolder(folderPath);
          setTemplatePickerOpen(true);
        },
      }),
      "new-folder": () => ({
        id: "new-folder",
        label: "新建文件夹",
        icon: "FolderPlus",
        onSelect: () => handleCreateFolder(folderPath),
      }),
      "folder-ai-context": () => ({
        id: "folder-ai-context",
        label: "作为 AI 任务上下文",
        icon: "Send",
        onSelect: () => openTaskIntakeForPath(folderPath),
      }),
      "ask-ai": () => ({
        id: "ask-ai",
        label: "以此向 AI 提任务",
        icon: "Send",
        onSelect: () => openTaskIntakeForPath(itemPath),
      }),
      "focus-graph": () => ({
        id: "focus-graph",
        label: "在图谱中聚焦",
        icon: "GitGraph",
        onSelect: () => focusPathInGraph(itemPath),
      }),
      "copy-path": () => ({
        id: "copy-path",
        label: "复制路径",
        icon: "Copy",
        shortcut: "Ctrl+C",
        onSelect: () => copyPath(itemPath),
      }),
      favorite: () => ({
        id: "favorite",
        label: favorites.includes(itemPath) ? "取消收藏" : "收藏",
        icon: "Star",
        onSelect: () => toggleFavorite(itemPath),
      }),
      rename: () => ({
        id: "rename",
        label: "重命名",
        icon: "Pencil",
        onSelect: () => startRename(item),
      }),
      delete: () => ({
        id: "delete",
        label: "删除",
        icon: "Trash2",
        variant: "danger",
        onSelect: () => { void handleDeleteNote(itemPath); },
      }),
    };

    return vaultContextMenuActionIdsForNode({ path: itemPath, isFolder: item.isFolder })
      .map((id) => builders[id]?.())
      .filter((menuItem): menuItem is ContextMenuItem => Boolean(menuItem));
  };

  useEffect(() => {
    const closeMenu = () => setContextMenu(null);
    const escKey = (e: KeyboardEvent) => { if (e.key === "Escape") setContextMenu(null); };
    document.addEventListener("click", closeMenu);
    document.addEventListener("keydown", escKey);
    return () => {
      document.removeEventListener("click", closeMenu);
      document.removeEventListener("keydown", escKey);
    };
  }, []);

  const renderTreeNode = (node: TreeNode): React.ReactNode => {
    const isExpanded = expandedFolders.has(node.path);
    const actualPath = treeNodeActualPath(node);
    const isActive = node.meta && actualPath === activeNote;
    const isHovered = actualPath === (hoveredNote === null ? undefined : hoveredNote);
    const depthIndent = node.depth * 14;
    const isAiNode = isAiWorkspacePath(actualPath);

    if (node.isFolder) {
      return (
        <div key={actualPath}>
          <div
            style={{
              ...styles.treeFolder,
              paddingLeft: `${8 + depthIndent}px`,
            }}
            onClick={() => toggleFolder(node.path)}
            onContextMenu={(e) => handleContextMenu(e, node)}
          >
            <span style={styles.chevron}>
              <FeroHaIcon
                name={isExpanded ? "ChevronDown" : "ChevronRight"}
                size={12}
              />
            </span>
            <span style={styles.folderIcon}>
              <FeroHaIcon name={isExpanded ? "FolderOpen" : "Folder"} size={14} />
            </span>
            <span style={styles.fileName}>{node.name}</span>
            <span style={styles.countBadge}>
              {node.children.filter(c => !c.isFolder).length}
            </span>
          </div>
          {isExpanded && node.children.map(renderTreeNode)}
        </div>
      );
    }

    // File node
    return (
      <div
        key={actualPath}
        style={{
          ...styles.fileItem,
          ...(isActive ? styles.fileItemActive : {}),
          ...(isHovered ? styles.fileItemHovered : {}),
          ...(loadingOperations.has(`delete-${actualPath}`) ? styles.fileItemLoading : {}),
          paddingLeft: `${8 + depthIndent}px`,
        }}
        onClick={() => handleNoteClick(node)}
        onDoubleClick={() => { if (!isAiNode) startRename(node); }}
        onMouseEnter={() => setHoveredNote(actualPath)}
        onMouseLeave={() => setHoveredNote(null)}
        onContextMenu={(e) => handleContextMenu(e, node)}
      >
        {editingPath === node.path ? (
          <input
            ref={editInputRef}
            style={styles.inlineInput}
            value={editingValue}
            onChange={(e) => setEditingValue(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") { e.preventDefault(); confirmRename(); }
              if (e.key === "Escape") { e.preventDefault(); cancelRename(); }
            }}
            onBlur={confirmRename}
            onClick={(e) => e.stopPropagation()}
          />
        ) : (
          <>
            <span style={styles.fileIcon}><FeroHaIcon name="FileText" size={14} /></span>
            <span style={styles.fileName}>{node.name}</span>
            <button
              style={{
                ...styles.starBtn,
                color: favorites.includes(actualPath) ? "var(--diff-warn)" : "var(--text-muted)",
                opacity: favorites.includes(actualPath) || hoveredNote === actualPath ? 1 : 0,
              } as React.CSSProperties}
              onClick={(e) => { e.stopPropagation(); toggleFavorite(actualPath); }}
              title={favorites.includes(actualPath) ? "Unfavorite" : "Favorite"}
            >
              <FeroHaIcon name="Star" size={12} />
            </button>
            {loadingOperations.has(`delete-${actualPath}`) ? (
              <span style={styles.spinner}><FeroHaIcon name="Loader" size={14} className="animate-spin" /></span>
            ) : (
              (hoveredNote === actualPath && !isAiNode) && (
                <button
                  style={styles.deleteBtn}
                  onClick={(e) => handleDeleteNote(actualPath, e)}
                  title="删除笔记"
                >
                  <FeroHaIcon name="X" size={12} />
                </button>
              )
            )}
          </>
        )}
      </div>
    );
  };

  const renderTreeSection = (
    tree: TreeNode[],
    sectionTitle: string,
    collapsed: boolean,
    setCollapsed: (v: boolean) => void,
    iconName: string,
    accentColor: string,
    testId?: string,
    description?: string,
    showWhenEmpty = false,
  ) => {
    if (tree.length === 0 && !showWhenEmpty) return null;
    const totalFiles = tree.reduce((acc, node) => {
      const count = (n: TreeNode): number => n.isFolder ? n.children.reduce((s, c) => s + count(c), 0) : 1;
      return acc + count(node);
    }, 0);

    return (
      <div key={testId ?? sectionTitle} style={{ marginBottom: "4px" }} data-testid={testId}>
        <div
          style={styles.sectionHeader}
          onClick={() => setCollapsed(!collapsed)}
          title={description}
        >
          <span style={styles.recentChevron}>
            <FeroHaIcon name={collapsed ? "ChevronRight" : "ChevronDown"} size={14} />
          </span>
          <FeroHaIcon name={iconName} size={14} />
          <span style={{ ...styles.sectionTitle, color: accentColor }}>{sectionTitle}</span>
          <span style={styles.sectionBadge}>{totalFiles}</span>
        </div>
        {description && !collapsed && (
          <div style={styles.sectionDescription}>{description}</div>
        )}
        {!collapsed && (
          <div style={styles.sectionList}>
            {tree.length > 0 ? tree.map(renderTreeNode) : (
              <div style={styles.emptySection}>No files yet</div>
            )}
          </div>
        )}
      </div>
    );
  };

  const sortLabels: Record<SortMode, string> = {
    "title-asc": "标题 A-Z",
    "title-desc": "标题 Z-A",
    "modified-desc": "修改时间（最新）",
    "modified-asc": "修改时间（最旧）",
  };

  return (
    <div style={styles.container}>
      <style>{`
        @keyframes spin {
          from { transform: rotate(0deg); }
          to { transform: rotate(360deg); }
        }
        .animate-spin { animation: spin 1s linear infinite; }
      `}</style>
      <div style={styles.header}>
        <div style={styles.titleBlock}>
          <span style={styles.title}>笔记库</span>
          <span style={styles.subtitle}>人类面可写，AI 工作区只读</span>
        </div>
        <button style={styles.iconBtn} onClick={handleOpenVault} title={vaultPath ? "切换笔记库" : "打开笔记库"}>
          <Icon d="M2 5v8h12V7H7L5 5H2z" />
        </button>
      </div>

      {vaultPath && (
        <div style={styles.path}>{vaultPath}</div>
      )}

      {vaultPath && (
        <div style={styles.summaryRow} aria-label="笔记库摘要">
          <span style={styles.summaryChip}>人类笔记 {humanNoteTotal}</span>
          <span style={styles.summaryChip}>文件夹 {folderPaths.length}</span>
          <span style={styles.summaryChipMuted}>AI 只读 {aiNoteTotal}</span>
        </div>
      )}

      {isTauri && !vaultPath && (
        <div style={styles.manualVaultOpen}>
          <input
            className="feroha-search"
            style={styles.manualVaultInput}
            value={manualVaultPath}
            onChange={(e) => setManualVaultPath(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                void handleOpenManualVault();
              }
            }}
            placeholder="Vault path"
            aria-label="Vault path"
          />
          <button
            style={styles.manualVaultBtn}
            onClick={() => { void handleOpenManualVault(); }}
            disabled={!manualVaultPath.trim() || isLoading}
          >
            Open
          </button>
        </div>
      )}

      {vaultPath && (
        <div style={styles.actions}>
          <button style={styles.primaryActionBtn} onClick={() => { setNewNoteFolder(null); setTemplatePickerOpen(true); }} title="新建笔记">
            <FeroHaIcon name="FilePlus" size={13} />
            <span>笔记</span>
          </button>
          <button style={styles.actionBtnWithIcon} onClick={() => handleCreateFolder()} title="新建文件夹">
            <FeroHaIcon name="FolderPlus" size={13} />
            <span>文件夹</span>
          </button>
          <button style={styles.iconBtn} onClick={refreshNotes} title="刷新">
            <FeroHaIcon name="RefreshCw" size={14} />
          </button>
          <select
            style={styles.sortSelect}
            value={sortBy}
            onChange={(e) => setSortBy(e.target.value as SortMode)}
            title="排序方式"
          >
            {Object.entries(sortLabels).map(([key, label]) => (
              <option key={key} value={key}>{label}</option>
            ))}
          </select>
          <input
            className="feroha-search"
            style={styles.searchInput}
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder="搜索笔记、文件夹或标签"
            aria-label="搜索笔记、文件夹或标签"
          />
        </div>
      )}

      <div style={styles.fileList}>
        {isLoading ? (
          <SkeletonLoader />
        ) : (
          <>
            {vaultPath && recentNotes.length > 0 && (
              <div style={{ marginBottom: "8px" }}>
                <div
                  style={styles.recentHeader}
                  onClick={() => setRecentCollapsed(!recentCollapsed)}
                >
                  <span style={styles.recentChevron}>
                    <FeroHaIcon name={recentCollapsed ? "ChevronRight" : "ChevronDown"} size={14} />
                  </span>
                  <span style={styles.recentTitle}>最近</span>
                  <button
                    style={styles.recentClearBtn}
                    onClick={(e) => { e.stopPropagation(); clearRecentNotes(); }}
                    title="清除最近笔记"
                  >
                    清除
                  </button>
                </div>
                {!recentCollapsed && (
                  <div style={styles.recentList}>
                    {recentNotes.slice(0, 10).map((notePath) => {
                      const meta = storeNotes.find((n) => n.path === notePath);
                      const title = meta?.title || notePath.split("/").pop()?.replace(/\.md$/, "") || notePath;
                      return (
                        <div
                          key={notePath}
                          style={{
                            ...styles.fileItem,
                            ...(notePath === activeNote ? styles.fileItemActive : {}),
                          }}
                          onClick={() => handleRecentClick(notePath)}
                          onMouseEnter={() => setHoveredNote(notePath)}
                          onMouseLeave={() => setHoveredNote(null)}
                        >
                          <span style={styles.fileIcon}><FeroHaIcon name="FileText" size={14} /></span>
                          <span style={styles.fileName}>{title}</span>
                        </div>
                      );
                    })}
                  </div>
                )}
              </div>
            )}
            {filterTags.length > 0 && (
              <div style={styles.filterBar}>
                <span style={styles.filterLabel}>筛选：</span>
                {filterTags.map((tag) => (
                  <span key={tag} style={styles.filterTag}>#{tag}</span>
                ))}
                <button
                  style={styles.filterClearBtn}
                  onClick={() => setFilterTags([])}
                >
                  清除
                </button>
              </div>
            )}
            {vaultPath && (
              <div style={{ marginBottom: "8px" }}>
                <div
                  style={styles.recentHeader}
                  onClick={() => setFavoritesCollapsed(!favoritesCollapsed)}
                >
                  <span style={styles.recentChevron}>
                    <FeroHaIcon name={favoritesCollapsed ? "ChevronRight" : "ChevronDown"} size={14} />
                  </span>
                  <span style={{ color: "var(--diff-warn)" }}>
                    <FeroHaIcon name="Star" size={14} />
                  </span>
                  <span style={styles.recentTitle}>收藏</span>
                  <span style={styles.sectionBadge}>{favoriteMeta.length}</span>
                </div>
                {!favoritesCollapsed && (
                  <div style={styles.recentList}>
                    {favoriteMeta.length === 0 ? (
                      <div style={styles.empty}>给笔记加星后会固定在这里</div>
                    ) : (
                      favoriteMeta.map((meta) => {
                        const title = meta.title || meta.path.split("/").pop()?.replace(/\.md$/, "") || meta.path;
                        return (
                          <div
                            key={meta.path}
                            style={{
                              ...styles.fileItem,
                              ...(meta.path === activeNote ? styles.fileItemActive : {}),
                            }}
                            onClick={() => openNoteInEditor(meta.path, title)}
                            onMouseEnter={() => setHoveredNote(meta.path)}
                            onMouseLeave={() => setHoveredNote(null)}
                          >
                            <span style={styles.fileIcon}><FeroHaIcon name="FileText" size={14} /></span>
                            <span style={styles.fileName}>{title}</span>
                          </div>
                        );
                      })
                    )}
                  </div>
                )}
              </div>
            )}
            {renderTreeSection(
              humanTree,
              "人类笔记",
              humanCollapsed,
              setHumanCollapsed,
              "User",
              "var(--text-primary)",
              "human-notes-section",
              "可写 Markdown 文件与空文件夹",
              true,
            )}
            {AI_WORKSPACE_ZONES.map((zone) =>
              renderTreeSection(
                aiZoneTrees[zone.id],
                zone.title,
                aiZoneCollapsed[zone.id],
                (collapsed) => setAiZoneCollapsed((current) => ({ ...current, [zone.id]: collapsed })),
                zone.icon,
                zone.accentColor,
                `ai-zone-${zone.id}`,
                zone.description,
                true,
              )
            )}
            {!hasVisibleVaultItems && vaultPath && (
              <div style={styles.empty}>{filterTags.length > 0 ? "没有匹配全部标签的笔记" : "未找到 .md 文件"}</div>
            )}
          </>
        )}
      </div>

      {contextMenu && (
        <ContextMenu
          point={{ x: contextMenu.x, y: contextMenu.y }}
          items={contextMenuItemsForNode(contextMenu.item)}
          onClose={() => setContextMenu(null)}
        />
      )}

      <TemplatePicker
        isOpen={isTemplatePickerOpen || (tpo ?? false)}
        onClose={() => { setTemplatePickerOpen(false); setNewNoteFolder(null); onTemplatePickerClose?.(); }}
        onSelectTemplate={handleTemplateSelect}
        isTauri={isTauri}
      />
    </div>
  );
}

function SkeletonLoader() {
  return (
    <div style={skeletonStyles.container}>
      {SKELETON_LINE_WIDTHS.map((width, i) => (
        <div key={i} style={skeletonStyles.item}>
          <div style={skeletonStyles.icon} />
          <div style={{ ...skeletonStyles.line, width }} />
        </div>
      ))}
    </div>
  );
}

const skeletonStyles: Record<string, React.CSSProperties> = {
  container: {
    display: "flex",
    flexDirection: "column",
    gap: "2px",
    padding: "4px 0",
  },
  item: {
    display: "flex",
    alignItems: "center",
    gap: "6px",
    padding: "4px 8px",
    borderRadius: "4px",
  },
  icon: {
    width: "12px",
    height: "12px",
    backgroundColor: "var(--bg-input)",
    borderRadius: "2px",
    animation: "pulse 1.5s ease-in-out infinite",
  },
  line: {
    height: "12px",
    backgroundColor: "var(--bg-input)",
    borderRadius: "2px",
    animation: "pulse 1.5s ease-in-out infinite",
  },
};

const styles: Record<string, React.CSSProperties> = {
  container: { padding: "12px", height: "100%", minHeight: 0, display: "flex", flexDirection: "column", boxSizing: "border-box", overflow: "hidden" },
  header: { display: "flex", justifyContent: "space-between", alignItems: "center", gap: "8px", marginBottom: "8px", minWidth: 0 },
  titleBlock: { display: "flex", flexDirection: "column", minWidth: 0, gap: "1px" },
  title: { fontSize: "13px", fontWeight: 600, color: "var(--text-primary)" },
  subtitle: { fontSize: "10px", color: "var(--text-muted)", whiteSpace: "nowrap" as const, overflow: "hidden" as const, textOverflow: "ellipsis" as const },
  iconBtn: { display: "inline-flex", alignItems: "center", justifyContent: "center", width: "28px", height: "28px", backgroundColor: "transparent", color: "var(--text-secondary)", border: "1px solid transparent", borderRadius: "4px", cursor: "pointer", transition: "all 0.15s", flexShrink: 0 },
  path: { fontSize: "10px", color: "var(--text-muted)", marginBottom: "4px", wordBreak: "break-all" as const, padding: "4px 6px", backgroundColor: "var(--bg-primary)", borderRadius: "4px" },
  summaryRow: { display: "flex", alignItems: "center", gap: "4px", flexWrap: "wrap" as const, marginBottom: "8px", minWidth: 0 },
  summaryChip: { fontSize: "10px", color: "var(--text-secondary)", border: "1px solid var(--border-color)", borderRadius: "999px", padding: "2px 6px", backgroundColor: "var(--bg-primary)" },
  summaryChipMuted: { fontSize: "10px", color: "var(--text-muted)", border: "1px solid var(--border-muted)", borderRadius: "999px", padding: "2px 6px", backgroundColor: "transparent" },
  manualVaultOpen: { display: "flex", gap: "4px", marginBottom: "8px", alignItems: "center", minWidth: 0 },
  manualVaultInput: { flex: 1, minWidth: 0, height: "28px", fontSize: "12px" },
  manualVaultBtn: { height: "28px", padding: "0 8px", backgroundColor: "var(--bg-input)", color: "var(--text-primary)", border: "1px solid var(--border-color)", borderRadius: "4px", cursor: "pointer", fontSize: "12px", flexShrink: 0 },
  actions: { display: "flex", gap: "4px", marginBottom: "8px", alignItems: "center", flexWrap: "wrap" as const, minWidth: 0 },
  actionBtn: { padding: "2px 6px", backgroundColor: "var(--bg-input)", color: "var(--text-primary)", border: "1px solid var(--border-color)", borderRadius: "4px", cursor: "pointer", fontSize: "10px" },
  primaryActionBtn: { display: "inline-flex", alignItems: "center", justifyContent: "center", gap: "4px", height: "28px", padding: "0 9px", backgroundColor: "var(--bg-input)", color: "var(--text-primary)", border: "1px solid var(--control-border-strong)", borderRadius: "4px", cursor: "pointer", fontSize: "12px", flexShrink: 0 },
  actionBtnWithIcon: { display: "inline-flex", alignItems: "center", justifyContent: "center", gap: "4px", height: "28px", padding: "0 8px", backgroundColor: "transparent", color: "var(--text-secondary)", border: "1px solid var(--border-color)", borderRadius: "4px", cursor: "pointer", fontSize: "12px", flexShrink: 0 },
  sortSelect: { flex: "1 1 110px", minWidth: 0, fontSize: "10px", backgroundColor: "var(--control-bg)", color: "var(--text-secondary)", border: "1px solid var(--control-border)", borderRadius: "4px", padding: "2px 4px", height: "24px", cursor: "pointer", outline: "none" },
  searchInput: { flex: "1 1 100%", minWidth: "120px", height: "28px", fontSize: "12px" },
  fileList: { flex: 1, minHeight: 0, overflow: "auto", display: "flex", flexDirection: "column", gap: "2px" },
  fileItem: { display: "flex", alignItems: "center", gap: "6px", minHeight: "26px", minWidth: 0, padding: "4px 8px", borderRadius: "4px", cursor: "pointer", fontSize: "13px", color: "var(--text-secondary)", position: "relative" as const },
  fileItemActive: { backgroundColor: "var(--bg-input)", color: "var(--text-primary)" },
  fileItemHovered: { backgroundColor: "var(--border-color)22" },
  fileItemLoading: { opacity: 0.6, pointerEvents: "none" as const },
  fileIcon: { fontSize: "12px", flexShrink: 0 },
  fileName: { overflow: "hidden" as const, textOverflow: "ellipsis" as const, whiteSpace: "nowrap" as const, flex: 1 },
  deleteBtn: { padding: "0 4px", backgroundColor: "transparent", color: "#f38ba8", border: "none", cursor: "pointer", fontSize: "12px", lineHeight: "1", opacity: 0.7 },
  starBtn: { padding: "0 2px", backgroundColor: "transparent", border: "none", cursor: "pointer", fontSize: "12px", lineHeight: "1", transition: "opacity 0.15s" },
  spinner: { fontSize: "12px", animation: "spin 1s linear infinite" },
  empty: { fontSize: "12px", color: "var(--text-muted)", fontStyle: "italic", padding: "8px" },
  treeFolder: { display: "flex", alignItems: "center", gap: "4px", minHeight: "26px", minWidth: 0, padding: "3px 8px", borderRadius: "4px", cursor: "pointer", fontSize: "13px", color: "var(--text-secondary)" },
  chevron: { fontSize: "10px", width: "12px", textAlign: "center" as const, flexShrink: 0, color: "var(--text-muted)" },
  folderIcon: { fontSize: "12px", flexShrink: 0, color: "var(--text-muted)" },
  countBadge: { fontSize: "10px", color: "var(--text-muted)", marginLeft: "4px" },
  inlineInput: { flex: 1, padding: "1px 4px", fontSize: "13px", backgroundColor: "var(--control-bg)", color: "var(--text-primary)", border: "1px solid var(--control-border-strong)", borderRadius: "3px", outline: "none" },
  recentHeader: { display: "flex", alignItems: "center", gap: "4px", minWidth: 0, padding: "4px 8px", cursor: "pointer", borderRadius: "4px", fontSize: "11px", fontWeight: 600, color: "var(--text-secondary)", textTransform: "uppercase" as const, letterSpacing: "0.5px" },
  recentChevron: { fontSize: "10px", width: "12px", textAlign: "center" as const, color: "var(--text-muted)" },
  recentTitle: { flex: 1, minWidth: 0, overflow: "hidden" as const, textOverflow: "ellipsis" as const, whiteSpace: "nowrap" as const },
  recentClearBtn: { padding: "1px 6px", backgroundColor: "transparent", color: "var(--text-muted)", border: "none", borderRadius: "3px", cursor: "pointer", fontSize: "10px" },
  recentList: { display: "flex", flexDirection: "column", gap: "2px", marginBottom: "8px" },
  sectionHeader: { display: "flex", alignItems: "center", gap: "6px", minWidth: 0, padding: "4px 8px", cursor: "pointer", borderRadius: "4px", fontSize: "11px", fontWeight: 600, textTransform: "uppercase" as const, letterSpacing: "0.5px" },
  sectionTitle: { flex: 1, minWidth: 0, fontSize: "11px", overflow: "hidden" as const, textOverflow: "ellipsis" as const, whiteSpace: "nowrap" as const },
  sectionBadge: { fontSize: "10px", color: "var(--text-muted)", backgroundColor: "var(--bg-input)", padding: "1px 6px", borderRadius: "8px", fontWeight: 500 },
  sectionDescription: { fontSize: "10px", color: "var(--text-muted)", padding: "0 8px 4px 28px", lineHeight: 1.35 },
  sectionList: { display: "flex", flexDirection: "column", gap: "2px" },
  emptySection: { fontSize: "11px", color: "var(--text-muted)", padding: "4px 8px 6px 28px", fontStyle: "italic" },
  filterBar: { display: "flex", alignItems: "center", gap: "4px", padding: "4px 8px", marginBottom: "4px", flexWrap: "wrap" as const },
  filterLabel: { fontSize: "10px", color: "var(--text-muted)", marginRight: "2px" },
  filterTag: { fontSize: "10px", color: "var(--accent-primary)", backgroundColor: "var(--accent-primary)18", padding: "1px 5px", borderRadius: "3px" },
  filterClearBtn: { padding: "1px 6px", backgroundColor: "transparent", color: "#f38ba8", border: "none", borderRadius: "3px", cursor: "pointer", fontSize: "10px" },
};
