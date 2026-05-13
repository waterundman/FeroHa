import { useState, useEffect, useCallback } from "react";
import { useAppStore } from "../hooks/useAppStore";

interface VaultBrowserProps {
  vaultPath: string | null;
  onSelectVault: (path: string) => void;
  isTauri: boolean;
}

interface NoteEntry {
  path: string;
  title: string;
}

const Icon = ({ d, s = 14 }: { d: string; s?: number }) => (
  <svg width={s} height={s} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
    <path d={d} />
  </svg>
);

/**
 * Vault Browser — File tree sidebar for navigating notes
 * Supports: open/create/delete notes, watcher event subscription, drag selection
 */
export default function VaultBrowser({ vaultPath, onSelectVault, isTauri }: VaultBrowserProps) {
  const [notes, setNotes] = useState<NoteEntry[]>(() => {
    if (!isTauri) {
      return [
        { path: "welcome.md", title: "Welcome" },
        { path: "concepts/architecture.md", title: "Architecture" },
        { path: "concepts/dual-track.md", title: "Dual Track" },
        { path: "research/llm-internals.md", title: "LLM Internals" },
      ];
    }
    return [];
  });
  const [activeNote, setActiveNote] = useState<string | null>(null);
  const [hoveredNote, setHoveredNote] = useState<string | null>(null);
  const [sortAsc, setSortAsc] = useState(true);

  // Subscribe to file watcher events
  useEffect(() => {
    if (!isTauri) return;

    let unlisten: (() => void) | undefined;

    const setupListener = async () => {
      try {
        const { listen } = await import("@tauri-apps/api/event");
        const unsub = await listen<any>("file-changed", (event) => {
          const { path, kind } = event.payload;
          if (kind === "created" || kind === "modified") {
            setNotes((prev) => {
              const existing = prev.find((n) => n.path === path);
              if (existing) return prev;
              const title = path.split("/").pop()?.replace(/\.md$/, "") || path;
              return [...prev, { path, title }];
            });
          } else if (kind === "deleted") {
            setNotes((prev) => prev.filter((n) => n.path !== path));
          }
        });
        unlisten = unsub;
      } catch {
        // Browser mode — no events
      }
    };

    setupListener();
    return () => { unlisten?.(); };
  }, [isTauri, vaultPath]);

  const handleOpenVault = async () => {
    if (isTauri) {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        const result = await invoke<string>("get_vault_path");
        if (result) {
          onSelectVault(result);
          refreshNotes();
        }
      } catch {
        const { open } = await import("@tauri-apps/plugin-dialog");
        const selected = await open({ directory: true, multiple: false });
        if (selected && typeof selected === "string") {
          onSelectVault(selected);
          refreshNotes();
        }
      }
    } else {
      onSelectVault("/demo-vault");
    }
  };

  const refreshNotes = useCallback(async () => {
    if (!isTauri) return;
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const noteList = await invoke<NoteEntry[]>("list_notes");
      setNotes(noteList);
    } catch {
      // Vault not open or other error
    }
  }, [isTauri]);

  const handleNoteClick = (note: NoteEntry) => {
    setActiveNote(note.path);
    if (isTauri) {
      import("@tauri-apps/api/core").then(({ invoke }) => {
        invoke<string>("read_note", { path: note.path }).then((content) => {
          useAppStore.getState().openNote(note.path, content);
        }).catch(console.error);
      });
    } else {
      useAppStore.getState().openNote(note.path, `# ${note.title}\n\n`);
    }
  };

  const handleCreateNote = async () => {
    const name = prompt("New note name (e.g., my-note.md):");
    if (!name) return;
    const safeName = name.endsWith(".md") ? name : `${name}.md`;

    if (isTauri) {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        await invoke("create_note", { path: safeName });
        refreshNotes();
      } catch (e) {
        console.error("Failed to create note:", e);
      }
    } else {
      setNotes((prev) => {
        if (prev.find((n) => n.path === safeName)) return prev;
        return [...prev, { path: safeName, title: name.replace(/\.md$/, "") }];
      });
    }
  };

  const handleDeleteNote = async (notePath: string, e: React.MouseEvent) => {
    e.stopPropagation();
    if (!confirm(`Delete "${notePath}"?`)) return;

    if (isTauri) {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        await invoke("delete_note", { path: notePath });
        setNotes((prev) => prev.filter((n) => n.path !== notePath));
        if (activeNote === notePath) setActiveNote(null);
      } catch (e) {
        console.error("Failed to delete note:", e);
      }
    } else {
      setNotes((prev) => prev.filter((n) => n.path !== notePath));
      if (activeNote === notePath) setActiveNote(null);
    }
  };

  const handleCreateFolder = async () => {
    const name = prompt("New folder name:");
    if (!name) return;
    const safeName = name.replace(/[^a-zA-Z0-9-_]/g, "-");
    if (isTauri) {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        await invoke("create_folder", { path: safeName });
        refreshNotes();
      } catch (e) {
        console.error("Failed to create folder:", e);
      }
    }
  };

  const handleSort = () => {
    setSortAsc(!sortAsc);
    setNotes((prev) =>
      [...prev].sort((a, b) =>
        sortAsc ? a.title.localeCompare(b.title) : b.title.localeCompare(a.title)
      )
    );
  };

  return (
    <div style={styles.container}>
      <div style={styles.header}>
        <span style={styles.title}>Vault</span>
        <button style={styles.iconBtn} onClick={handleOpenVault} title={vaultPath ? "Switch Vault" : "Open Vault"}>
          <Icon d="M2 5v8h12V7H7L5 5H2z" />
        </button>
      </div>

      {vaultPath && (
        <div style={styles.path}>{vaultPath}</div>
      )}

      {/* Action bar */}
      {vaultPath && (
        <div style={styles.actions}>
          <button style={styles.iconBtn} onClick={handleCreateNote} title="New Note">
            <Icon d="M9 1H4v14h8V5l-3-4z M9 1v4h4" />
          </button>
          <button style={styles.iconBtn} onClick={handleCreateFolder} title="New Folder">
            <Icon d="M2 4h4l2 2h6v7H2V4z M8 9v4 M6 11h4" />
          </button>
          <button style={styles.iconBtn} onClick={handleSort} title="Sort">
            <Icon d="M3 5h10 M3 9h6 M3 13h3" />
          </button>
          <button style={styles.iconBtn} onClick={refreshNotes} title="Refresh">
            ⟳
          </button>
        </div>
      )}

      <div style={styles.fileList}>
        {notes.map((note) => (
          <div
            key={note.path}
            style={{
              ...styles.fileItem,
              ...(note.path === activeNote ? styles.fileItemActive : {}),
              ...(note.path === hoveredNote ? styles.fileItemHovered : {}),
            }}
            onClick={() => handleNoteClick(note)}
            onMouseEnter={() => setHoveredNote(note.path)}
            onMouseLeave={() => setHoveredNote(null)}
          >
            <span style={styles.fileIcon}>📄</span>
            <span style={styles.fileName}>{note.title}</span>
            {hoveredNote === note.path && (
              <button
                style={styles.deleteBtn}
                onClick={(e) => handleDeleteNote(note.path, e)}
                title="Delete note"
              >
                ✕
              </button>
            )}
          </div>
        ))}

        {notes.length === 0 && vaultPath && (
          <div style={styles.empty}>No .md files found</div>
        )}
      </div>
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  container: { padding: "12px", height: "100%", display: "flex", flexDirection: "column" },
  header: { display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "8px" },
  title: { fontSize: "13px", fontWeight: 600, color: "#cdd6f4" },
  iconBtn: { display: "inline-flex", alignItems: "center", justifyContent: "center", width: "26px", height: "26px", backgroundColor: "transparent", color: "#a6adc8", border: "none", borderRadius: "4px", cursor: "pointer", transition: "all 0.15s" },
  path: { fontSize: "10px", color: "#6c7086", marginBottom: "4px", wordBreak: "break-all" as const, padding: "4px 6px", backgroundColor: "#11111b", borderRadius: "4px" },
  actions: { display: "flex", gap: "2px", marginBottom: "8px" },
  actionBtn: { padding: "2px 6px", backgroundColor: "#313244", color: "#cdd6f4", border: "1px solid #45475a", borderRadius: "4px", cursor: "pointer", fontSize: "10px" },
  fileList: { flex: 1, overflow: "auto", display: "flex", flexDirection: "column", gap: "2px" },
  fileItem: { display: "flex", alignItems: "center", gap: "6px", padding: "4px 8px", borderRadius: "4px", cursor: "pointer", fontSize: "13px", color: "#bac2de", position: "relative" as const },
  fileItemActive: { backgroundColor: "#313244", color: "#cdd6f4" },
  fileItemHovered: { backgroundColor: "#45475a22" },
  fileIcon: { fontSize: "12px" },
  fileName: { overflow: "hidden" as const, textOverflow: "ellipsis" as const, whiteSpace: "nowrap" as const, flex: 1 },
  deleteBtn: { padding: "0 4px", backgroundColor: "transparent", color: "#f38ba8", border: "none", cursor: "pointer", fontSize: "12px", lineHeight: "1", opacity: 0.7 },
  empty: { fontSize: "12px", color: "#6c7086", fontStyle: "italic", padding: "8px" },
};
