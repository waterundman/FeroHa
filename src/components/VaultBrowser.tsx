import { useState, useEffect, useCallback, useMemo, useRef } from "react";
import { useAppStore, type NoteMeta, type GraphData } from "../hooks/useAppStore";
import { showToast } from "./toastBus";
import TemplatePicker from "./TemplatePicker";
import FeroHaIcon from "./FeroHaIcon";

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
  isFolder: boolean;
  depth: number;
  children: TreeNode[];
  meta?: NoteMeta;
}

interface FileChangedPayload {
  path: string;
  kind: "created" | "modified" | "deleted" | string;
}

type SortMode = "title-asc" | "title-desc" | "modified-desc" | "modified-asc";

const SKELETON_LINE_WIDTHS = ["68%", "84%", "72%", "91%", "76%"];

const Icon = ({ d, s = 14 }: { d: string; s?: number }) => (
  <svg width={s} height={s} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
    <path d={d} />
  </svg>
);

function buildTree(notes: NoteMeta[], sortMode: SortMode): TreeNode[] {
  const folderMap = new Map<string, TreeNode>();

  const getFolder = (parts: string[]): TreeNode => {
    let parent: TreeNode | null = null;
    for (let i = 0; i < parts.length; i++) {
      const segment = parts[i];
      const depth = i;
      const key = depth === 0 ? segment : `${i > 0 ? `${parts.slice(0, i).join("/")}/` : ""}${segment}`;
      if (!folderMap.has(key)) {
        const node: TreeNode = {
          name: segment,
          path: key + "/",
          isFolder: true,
          depth,
          children: [],
        };
        folderMap.set(key, node);
        if (parent) {
          if (!parent.children.find(c => c.path === node.path)) {
            parent.children.push(node);
          }
        }
        parent = node;
      } else {
        parent = folderMap.get(key)!;
      }
    }
    return parent!;
  };

  const roots: TreeNode[] = [];
  const rootFolderMap = new Map<string, TreeNode>();

  for (const note of notes) {
    const parts = note.path.split("/");
    if (parts.length === 1) {
      roots.push({
        name: note.title,
        path: note.path,
        isFolder: false,
        depth: 0,
        children: [],
        meta: note,
      });
    } else {
      const folderParts = parts.slice(0, -1);
      const folderPath = folderParts.join("/");

      if (!rootFolderMap.has(folderParts[0])) {
        const parentFolder = getFolder(folderParts);
        rootFolderMap.set(folderParts[0], parentFolder);
        if (!roots.find(r => r.path === parentFolder.path)) {
          roots.push(parentFolder);
        }
      } else {
        getFolder(folderParts);
      }

      const fileNode: TreeNode = {
        name: note.title,
        path: note.path,
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

export default function VaultBrowser({ vaultPath, onSelectVault, isTauri, templatePickerOpen: tpo, onTemplatePickerClose }: VaultBrowserProps) {
  const [activeNote, setActiveNote] = useState<string | null>(null);
  const [hoveredNote, setHoveredNote] = useState<string | null>(null);
  const [sortBy, setSortBy] = useState<SortMode>("title-asc");
  const [isLoading, setIsLoading] = useState(false);
  const [loadingOperations, setLoadingOperations] = useState<Set<string>>(new Set());
  const [recentCollapsed, setRecentCollapsed] = useState(false);
  const [favoritesCollapsed, setFavoritesCollapsed] = useState(false);
  const [humanCollapsed, setHumanCollapsed] = useState(false);
  const [aiCollapsed, setAiCollapsed] = useState(true);
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
    if (filterTags.length === 0) return storeNotes;
    return storeNotes.filter(note => {
      const noteTags = note.tags || [];
      return filterTags.every(ft => noteTags.includes(ft));
    });
  }, [storeNotes, filterTags]);

  const humanNotes = useMemo(() => filteredNotes.filter(n => !n.path.startsWith(".dualtrack/")), [filteredNotes]);
  const aiNotes = useMemo(() => filteredNotes.filter(n => n.path.startsWith(".dualtrack/")), [filteredNotes]);

  const humanTree = useMemo(() => buildTree(humanNotes, sortBy), [humanNotes, sortBy]);
  const aiTree = useMemo(() => buildTree(aiNotes, sortBy), [aiNotes, sortBy]);

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
      const noteList = await invoke<NoteMeta[]>("list_notes");
      setStoreNotes(noteList);
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
        showToast("error", "Failed to open vault");
      }
    } else {
      onSelectVault("/demo-vault");
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
      openNoteInEditor(note.path, note.name);
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
        showToast("error", `Failed to create ${fullPath}`);
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

  const handleDeleteNote = async (notePath: string, e: React.MouseEvent) => {
    e.stopPropagation();
    if (!confirm(`Delete "${notePath}"?`)) return;

    const operationId = `delete-${notePath}`;
    setLoadingOperations(prev => new Set(prev).add(operationId));

    if (isTauri) {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        await invoke("delete_note", { path: notePath });
        setStoreNotes(useAppStore.getState().notes.filter((n) => n.path !== notePath));
        if (activeNote === notePath) setActiveNote(null);
        showToast("success", `Deleted ${notePath}`);
      } catch (e) {
        console.error("Failed to delete note:", e);
        showToast("error", `Failed to delete ${notePath}`);
      }
    } else {
      setStoreNotes(useAppStore.getState().notes.filter((n) => n.path !== notePath));
      if (activeNote === notePath) setActiveNote(null);
      showToast("success", `Deleted ${notePath}`);
    }

    setLoadingOperations(prev => {
      const next = new Set(prev);
      next.delete(operationId);
      return next;
    });
  };

  const handleCreateFolder = async (prefix?: string) => {
    setContextMenu(null);
    const name = prompt("New folder name:");
    if (!name) return;
    const safeName = name.replace(/[^a-zA-Z0-9-_]/g, "-");
    const folderPath = prefix ? `${prefix}/${safeName}` : safeName;

    const operationId = `createFolder-${folderPath}`;
    setLoadingOperations(prev => new Set(prev).add(operationId));

    if (isTauri) {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        await invoke("create_folder", { path: folderPath });
        refreshNotes();
        showToast("success", `Created folder ${folderPath}`);
      } catch (e) {
        console.error("Failed to create folder:", e);
        showToast("error", `Failed to create folder ${folderPath}`);
      }
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
        showToast("success", `Renamed to ${newPath}`);
      } catch (e) {
        console.error("Failed to rename note:", e);
        showToast("error", `Failed to rename: ${e}`);
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
    const isActive = node.meta && node.path === activeNote;
    const isHovered = node.path === (hoveredNote === null ? undefined : hoveredNote);
    const depthIndent = node.depth * 14;

    if (node.isFolder) {
      return (
        <div key={node.path}>
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
        key={node.path}
        style={{
          ...styles.fileItem,
          ...(isActive ? styles.fileItemActive : {}),
          ...(isHovered ? styles.fileItemHovered : {}),
          ...(loadingOperations.has(`delete-${node.path}`) ? styles.fileItemLoading : {}),
          paddingLeft: `${8 + depthIndent}px`,
        }}
        onClick={() => handleNoteClick(node)}
        onDoubleClick={() => startRename(node)}
        onMouseEnter={() => setHoveredNote(node.path)}
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
                "--icon-default": favorites.includes(node.path) ? "var(--diff-warn)" : "var(--text-muted)",
                opacity: favorites.includes(node.path) || hoveredNote === node.path ? 1 : 0,
              } as React.CSSProperties}
              onClick={(e) => { e.stopPropagation(); toggleFavorite(node.path); }}
              title={favorites.includes(node.path) ? "Unfavorite" : "Favorite"}
            >
              <FeroHaIcon name="Star" size={12} />
            </button>
            {loadingOperations.has(`delete-${node.path}`) ? (
              <span style={styles.spinner}><FeroHaIcon name="Loader" size={14} className="animate-spin" /></span>
            ) : (
              (hoveredNote === node.path) && (
                <button
                  style={styles.deleteBtn}
                  onClick={(e) => handleDeleteNote(node.path, e)}
                  title="Delete note"
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

  const renderTreeSection = (tree: TreeNode[], sectionTitle: string, collapsed: boolean, setCollapsed: (v: boolean) => void, iconName: string, accentColor: string) => {
    if (tree.length === 0) return null;
    const totalFiles = tree.reduce((acc, node) => {
      const count = (n: TreeNode): number => n.isFolder ? n.children.reduce((s, c) => s + count(c), 0) : 1;
      return acc + count(node);
    }, 0);

    return (
      <div style={{ marginBottom: "4px" }}>
        <div
          style={styles.sectionHeader}
          onClick={() => setCollapsed(!collapsed)}
        >
          <span style={styles.recentChevron}>
            <FeroHaIcon name={collapsed ? "ChevronRight" : "ChevronDown"} size={14} />
          </span>
          <FeroHaIcon name={iconName} size={14} />
          <span style={{ ...styles.sectionTitle, color: accentColor }}>{sectionTitle}</span>
          <span style={styles.sectionBadge}>{totalFiles}</span>
        </div>
        {!collapsed && (
          <div style={styles.sectionList}>
            {tree.map(renderTreeNode)}
          </div>
        )}
      </div>
    );
  };

  const sortLabels: Record<SortMode, string> = {
    "title-asc": "Title A-Z",
    "title-desc": "Title Z-A",
    "modified-desc": "Modified (newest)",
    "modified-asc": "Modified (oldest)",
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
        <span style={styles.title}>Vault</span>
        <button style={styles.iconBtn} onClick={handleOpenVault} title={vaultPath ? "Switch Vault" : "Open Vault"}>
          <Icon d="M2 5v8h12V7H7L5 5H2z" />
        </button>
      </div>

      {vaultPath && (
        <div style={styles.path}>{vaultPath}</div>
      )}

      {vaultPath && (
        <div style={styles.actions}>
          <button style={styles.iconBtn} onClick={() => { setNewNoteFolder(null); setTemplatePickerOpen(true); }} title="New Note">
            <Icon d="M9 1H4v14h8V5l-3-4z M9 1v4h4" />
          </button>
          <button style={styles.iconBtn} onClick={() => handleCreateFolder()} title="New Folder">
            <Icon d="M2 4h4l2 2h6v7H2V4z M8 9v4 M6 11h4" />
          </button>
          <select
            style={styles.sortSelect}
            value={sortBy}
            onChange={(e) => setSortBy(e.target.value as SortMode)}
            title="Sort order"
          >
            {Object.entries(sortLabels).map(([key, label]) => (
              <option key={key} value={key}>{label}</option>
            ))}
          </select>
          <button style={styles.iconBtn} onClick={refreshNotes} title="Refresh">
            ⟳
          </button>
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
                  <span style={styles.recentTitle}>Recent</span>
                  <button
                    style={styles.recentClearBtn}
                    onClick={(e) => { e.stopPropagation(); clearRecentNotes(); }}
                    title="Clear recent notes"
                  >
                    Clear
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
                <span style={styles.filterLabel}>Filtering:</span>
                {filterTags.map((tag) => (
                  <span key={tag} style={styles.filterTag}>#{tag}</span>
                ))}
                <button
                  style={styles.filterClearBtn}
                  onClick={() => setFilterTags([])}
                >
                  Clear
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
                  <span style={{ "--icon-default": "var(--diff-warn)" } as React.CSSProperties}>
                    <FeroHaIcon name="Star" size={14} />
                  </span>
                  <span style={styles.recentTitle}>Favorites</span>
                  <span style={styles.sectionBadge}>{favoriteMeta.length}</span>
                </div>
                {!favoritesCollapsed && (
                  <div style={styles.recentList}>
                    {favoriteMeta.length === 0 ? (
                      <div style={styles.empty}>Star a note to pin it here</div>
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
              "Human Notes",
              humanCollapsed,
              setHumanCollapsed,
              "User",
              "var(--text-primary)"
            )}
            {renderTreeSection(
              aiTree,
              "AI Workspace",
              aiCollapsed,
              setAiCollapsed,
              "Bot",
              "var(--accent-primary)"
            )}
            {filteredNotes.length === 0 && vaultPath && (
              <div style={styles.empty}>{filterTags.length > 0 ? "No notes match all selected tags" : "No .md files found"}</div>
            )}
          </>
        )}
      </div>

      {contextMenu && (
        <div
          className="feroha-context-menu"
          style={{
            position: "fixed",
            left: contextMenu.x,
            top: contextMenu.y,
            zIndex: 1000,
          }}
        >
          {contextMenu.item.isFolder ? (
            <>
              <div
                className="feroha-context-menu-item"
                onClick={() => {
                  const folderPath = contextMenu.item.path.replace(/\/$/, "");
                  setNewNoteFolder(folderPath);
                  setTemplatePickerOpen(true);
                  setContextMenu(null);
                }}
              >
                New Note
              </div>
              <div
                className="feroha-context-menu-item"
                onClick={() => handleCreateFolder(contextMenu.item.path.replace(/\/$/, ""))}
              >
                New Folder
              </div>
            </>
          ) : (
            <>
              <div
                className="feroha-context-menu-item"
                onClick={() => {
                  startRename(contextMenu.item);
                  setContextMenu(null);
                }}
              >
                Rename
              </div>
              <div
                className="feroha-context-menu-item"
                onClick={(e) => {
                  handleDeleteNote(contextMenu.item.path, e as unknown as React.MouseEvent);
                  setContextMenu(null);
                }}
              >
                Delete
              </div>
            </>
          )}
        </div>
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
  container: { padding: "12px", height: "100%", display: "flex", flexDirection: "column" },
  header: { display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "8px" },
  title: { fontSize: "13px", fontWeight: 600, color: "var(--text-primary)" },
  iconBtn: { display: "inline-flex", alignItems: "center", justifyContent: "center", width: "26px", height: "26px", backgroundColor: "transparent", color: "var(--text-secondary)", border: "none", borderRadius: "4px", cursor: "pointer", transition: "all 0.15s" },
  path: { fontSize: "10px", color: "var(--text-muted)", marginBottom: "4px", wordBreak: "break-all" as const, padding: "4px 6px", backgroundColor: "var(--bg-primary)", borderRadius: "4px" },
  actions: { display: "flex", gap: "2px", marginBottom: "8px", alignItems: "center" },
  actionBtn: { padding: "2px 6px", backgroundColor: "var(--bg-input)", color: "var(--text-primary)", border: "1px solid var(--border-color)", borderRadius: "4px", cursor: "pointer", fontSize: "10px" },
  sortSelect: { fontSize: "10px", backgroundColor: "var(--bg-input)", color: "var(--text-secondary)", border: "1px solid var(--border-color)", borderRadius: "4px", padding: "2px 4px", height: "22px", cursor: "pointer", outline: "none" },
  fileList: { flex: 1, overflow: "auto", display: "flex", flexDirection: "column", gap: "2px" },
  fileItem: { display: "flex", alignItems: "center", gap: "6px", padding: "4px 8px", borderRadius: "4px", cursor: "pointer", fontSize: "13px", color: "var(--text-secondary)", position: "relative" as const },
  fileItemActive: { backgroundColor: "var(--bg-input)", color: "var(--text-primary)" },
  fileItemHovered: { backgroundColor: "var(--border-color)22" },
  fileItemLoading: { opacity: 0.6, pointerEvents: "none" as const },
  fileIcon: { fontSize: "12px", flexShrink: 0 },
  fileName: { overflow: "hidden" as const, textOverflow: "ellipsis" as const, whiteSpace: "nowrap" as const, flex: 1 },
  deleteBtn: { padding: "0 4px", backgroundColor: "transparent", color: "#f38ba8", border: "none", cursor: "pointer", fontSize: "12px", lineHeight: "1", opacity: 0.7 },
  starBtn: { padding: "0 2px", backgroundColor: "transparent", border: "none", cursor: "pointer", fontSize: "12px", lineHeight: "1", transition: "opacity 0.15s" },
  spinner: { fontSize: "12px", animation: "spin 1s linear infinite" },
  empty: { fontSize: "12px", color: "var(--text-muted)", fontStyle: "italic", padding: "8px" },
  treeFolder: { display: "flex", alignItems: "center", gap: "4px", padding: "3px 8px", borderRadius: "4px", cursor: "pointer", fontSize: "13px", color: "var(--text-secondary)" },
  chevron: { fontSize: "10px", width: "12px", textAlign: "center" as const, flexShrink: 0, color: "var(--text-muted)" },
  folderIcon: { fontSize: "12px", flexShrink: 0, color: "var(--text-muted)" },
  countBadge: { fontSize: "10px", color: "var(--text-muted)", marginLeft: "4px" },
  inlineInput: { flex: 1, padding: "1px 4px", fontSize: "13px", backgroundColor: "var(--bg-input)", color: "var(--text-primary)", border: "1px solid var(--accent-primary)", borderRadius: "3px", outline: "none" },
  recentHeader: { display: "flex", alignItems: "center", gap: "4px", padding: "4px 8px", cursor: "pointer", borderRadius: "4px", fontSize: "11px", fontWeight: 600, color: "var(--text-secondary)", textTransform: "uppercase" as const, letterSpacing: "0.5px" },
  recentChevron: { fontSize: "10px", width: "12px", textAlign: "center" as const, color: "var(--text-muted)" },
  recentTitle: { flex: 1 },
  recentClearBtn: { padding: "1px 6px", backgroundColor: "transparent", color: "var(--text-muted)", border: "none", borderRadius: "3px", cursor: "pointer", fontSize: "10px" },
  recentList: { display: "flex", flexDirection: "column", gap: "2px", marginBottom: "8px" },
  sectionHeader: { display: "flex", alignItems: "center", gap: "6px", padding: "4px 8px", cursor: "pointer", borderRadius: "4px", fontSize: "11px", fontWeight: 600, textTransform: "uppercase" as const, letterSpacing: "0.5px" },
  sectionTitle: { flex: 1, fontSize: "11px" },
  sectionBadge: { fontSize: "10px", color: "var(--text-muted)", backgroundColor: "var(--bg-input)", padding: "1px 6px", borderRadius: "8px", fontWeight: 500 },
  sectionList: { display: "flex", flexDirection: "column", gap: "2px" },
  filterBar: { display: "flex", alignItems: "center", gap: "4px", padding: "4px 8px", marginBottom: "4px", flexWrap: "wrap" as const },
  filterLabel: { fontSize: "10px", color: "var(--text-muted)", marginRight: "2px" },
  filterTag: { fontSize: "10px", color: "var(--accent-primary)", backgroundColor: "var(--accent-primary)18", padding: "1px 5px", borderRadius: "3px" },
  filterClearBtn: { padding: "1px 6px", backgroundColor: "transparent", color: "#f38ba8", border: "none", borderRadius: "3px", cursor: "pointer", fontSize: "10px" },
};
