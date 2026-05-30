import { useState, useEffect, useMemo } from "react";
import { useAppStore, NoteMeta } from "../hooks/useAppStore";

interface BacklinkContext {
  from: string;
  from_title: string;
  text: string;
  position: number;
}

interface OutgoingLink {
  title: string;
  path: string;
  exists: boolean;
}

interface BacklinksPanelProps {
  currentNotePath: string | null;
  isTauri: boolean;
}

const S = {
  panel: {
    borderBottom: "1px solid var(--border-color)",
  } as React.CSSProperties,
  header: {
    display: "flex",
    alignItems: "center",
    gap: "6px",
    padding: "8px 10px",
    cursor: "pointer",
    userSelect: "none",
    fontSize: "12px",
    fontWeight: 600,
    color: "var(--text-secondary)",
  } as React.CSSProperties,
  chevron: {
    display: "inline-flex",
    alignItems: "center",
    justifyContent: "center",
    width: "16px",
    height: "16px",
    transition: "transform 0.15s",
  } as React.CSSProperties,
  chevronOpen: {
    transform: "rotate(90deg)",
  } as React.CSSProperties,
  body: {
    padding: "0 8px 8px",
    maxHeight: "300px",
    overflowY: "auto",
  } as React.CSSProperties,
  tabBar: {
    display: "flex",
    gap: "2px",
    padding: "0 8px 4px",
    borderBottom: "1px solid var(--border-color)",
  } as React.CSSProperties,
  tab: {
    padding: "4px 10px",
    fontSize: "11px",
    fontWeight: 600,
    cursor: "pointer",
    border: "none",
    borderRadius: "4px 4px 0 0",
    color: "var(--text-muted)",
    background: "transparent",
    borderBottom: "2px solid transparent",
    transition: "color 0.15s, border-color 0.15s",
  } as React.CSSProperties,
  tabActive: {
    color: "var(--accent-primary)",
    borderBottomColor: "var(--accent-primary)",
  } as React.CSSProperties,
  empty: {
    fontSize: "11px",
    color: "var(--text-muted)",
    padding: "4px 4px 8px",
    fontStyle: "italic",
    lineHeight: "1.5",
  } as React.CSSProperties,
  noNote: {
    fontSize: "11px",
    color: "var(--text-muted)",
    padding: "8px 12px",
    fontStyle: "italic",
  } as React.CSSProperties,
  entry: {
    display: "flex",
    flexDirection: "column",
    padding: "5px 6px",
    borderRadius: "4px",
    cursor: "pointer",
    borderBottom: "1px solid var(--border-color)",
    transition: "background-color 0.1s",
  } as React.CSSProperties,
  entryTitle: {
    fontSize: "12px",
    fontWeight: 600,
    color: "var(--accent-primary)",
  } as React.CSSProperties,
  entryTitleGhost: {
    fontSize: "12px",
    fontWeight: 600,
    color: "var(--text-muted)",
  } as React.CSSProperties,
  entryText: {
    fontSize: "11px",
    color: "var(--text-secondary)",
    marginTop: "2px",
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  } as React.CSSProperties,
  outgoingRow: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    padding: "5px 6px",
    borderRadius: "4px",
    cursor: "pointer",
    borderBottom: "1px solid var(--border-color)",
    transition: "background-color 0.1s",
  } as React.CSSProperties,
  createBtn: {
    fontSize: "10px",
    padding: "2px 8px",
    borderRadius: "3px",
    border: "1px solid var(--border-color)",
    background: "var(--bg-input)",
    color: "var(--text-secondary)",
    cursor: "pointer",
  } as React.CSSProperties,
};

function truncate(s: string, max: number) {
  if (s.length <= max) return s;
  return s.slice(0, max) + "...";
}

function hasTauriRuntime() {
  return typeof window !== "undefined" && Boolean(window.__TAURI_INTERNALS__);
}

function parseOutgoingLinks(content: string, notes: NoteMeta[]): OutgoingLink[] {
  const regex = /\[\[([^\]|#]+)(?:[|#][^\]]+)?\]\]/g;
  const unique = new Set<string>();
  const links: OutgoingLink[] = [];
  let match;
  while ((match = regex.exec(content)) !== null) {
    const target = match[1].trim();
    if (!unique.has(target)) {
      unique.add(target);
      const note = notes.find(n =>
        n.title === target ||
        n.path === target ||
        n.path === `${target}.md` ||
        n.path.endsWith(`/${target}.md`)
      );
      links.push({
        title: target,
        path: note?.path || target,
        exists: !!note
      });
    }
  }
  return links;
}

export default function BacklinksPanel({ currentNotePath, isTauri }: BacklinksPanelProps) {
  const [collapsed, setCollapsed] = useState(true);
  const [backlinks, setBacklinks] = useState<BacklinkContext[]>([]);
  const [loading, setLoading] = useState(false);
  const [activeTab, setActiveTab] = useState<"incoming" | "outgoing">("incoming");

  const currentNote = useAppStore(s => s.currentNote);
  const notes = useAppStore(s => s.notes);

  const outgoingLinks = useMemo(() => {
    if (!currentNote) return [];
    return parseOutgoingLinks(currentNote.content, notes);
  }, [currentNote, notes]);

  useEffect(() => {
    if (!currentNotePath || collapsed || !isTauri || !hasTauriRuntime()) {
      return;
    }

    let cancelled = false;

    (async () => {
      if (cancelled) return;
      setLoading(true);
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        const result = await invoke<BacklinkContext[]>("get_backlinks", {
          noteId: currentNotePath,
        });
        if (!cancelled) {
          setBacklinks(result);
          setLoading(false);
        }
      } catch {
        if (!cancelled) {
          setBacklinks([]);
          setLoading(false);
        }
      }
    })();

    return () => {
      cancelled = true;
      setBacklinks([]);
      setLoading(false);
    };
  }, [currentNotePath, collapsed, isTauri]);

  if (!currentNotePath) {
    return (
      <div style={S.noNote}>No note selected</div>
    );
  }

  return (
    <div style={S.panel}>
      <div
        style={S.header}
        onClick={() => setCollapsed(!collapsed)}
        role="button"
        aria-expanded={!collapsed}
        tabIndex={0}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            setCollapsed(!collapsed);
          }
        }}
      >
        <span style={{ ...S.chevron, ...(!collapsed ? S.chevronOpen : {}) }}>
          <svg width="10" height="10" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round">
            <path d="M6 3l6 5-6 5" />
          </svg>
        </span>
        Links
      </div>
      {!collapsed && (
        <>
          <div style={S.tabBar}>
            <button
              style={{
                ...S.tab,
                ...(activeTab === "incoming" ? S.tabActive : {}),
              }}
              onClick={(e) => { e.stopPropagation(); setActiveTab("incoming"); }}
            >
              Incoming ({backlinks.length})
            </button>
            <button
              style={{
                ...S.tab,
                ...(activeTab === "outgoing" ? S.tabActive : {}),
              }}
              onClick={(e) => { e.stopPropagation(); setActiveTab("outgoing"); }}
            >
              Outgoing ({outgoingLinks.length})
            </button>
          </div>
          <div style={S.body}>
            {activeTab === "incoming" && (
              <>
                {loading && (
                  <div style={S.empty}>Loading backlinks...</div>
                )}
                {!loading && backlinks.length === 0 && (
                  <div style={S.empty}>
                    No notes link to this one yet. Try adding [[links]] from other notes.
                  </div>
                )}
                {!loading &&
                  backlinks.map((entry, i) => (
                    <div
                      key={`${entry.from}-${i}`}
                      style={S.entry}
                      onClick={async () => {
                        try {
                          const { invoke } = await import("@tauri-apps/api/core");
                          const content = await invoke<string>("read_note", {
                            path: entry.from,
                          });
                          useAppStore.getState().openNote(entry.from, content);
                        } catch {
                          useAppStore.getState().openNote(
                            entry.from,
                            `# ${entry.from_title}\n\n`
                          );
                        }
                      }}
                      onMouseEnter={(e) => {
                        (e.currentTarget as HTMLDivElement).style.backgroundColor =
                          "var(--bg-input)";
                      }}
                      onMouseLeave={(e) => {
                        (e.currentTarget as HTMLDivElement).style.backgroundColor =
                          "transparent";
                      }}
                    >
                      <span style={S.entryTitle}>{entry.from_title}</span>
                      <span style={S.entryText} title={entry.text}>
                        {truncate(entry.text, 60)}
                      </span>
                    </div>
                  ))}
              </>
            )}

            {activeTab === "outgoing" && (
              <>
                {outgoingLinks.length === 0 && (
                  <div style={S.empty}>
                    No outgoing links. Add [[wiki links]] in your note content.
                  </div>
                )}
                {outgoingLinks.map((link, i) => (
                  <div
                    key={`${link.path}-${i}`}
                    style={S.outgoingRow}
                    onClick={link.exists ? async () => {
                      try {
                        const { invoke } = await import("@tauri-apps/api/core");
                        const content = await invoke<string>("read_note", {
                          path: link.path,
                        });
                        useAppStore.getState().openNote(link.path, content);
                      } catch {
                        useAppStore.getState().openNote(
                          link.path,
                          `# ${link.title}\n\n`
                        );
                      }
                    } : undefined}
                    onMouseEnter={(e) => {
                      (e.currentTarget as HTMLDivElement).style.backgroundColor =
                        "var(--bg-input)";
                    }}
                    onMouseLeave={(e) => {
                      (e.currentTarget as HTMLDivElement).style.backgroundColor =
                        "transparent";
                    }}
                  >
                    <span style={link.exists ? S.entryTitle : S.entryTitleGhost}>
                      {link.title}
                    </span>
                    {!link.exists && (
                      <button
                        style={S.createBtn}
                        onClick={async (e) => {
                          e.stopPropagation();
                          const newContent = `# ${link.title}\n\n`;
                          try {
                            const { invoke } = await import("@tauri-apps/api/core");
                            await invoke("write_note", {
                              path: link.path.endsWith(".md") ? link.path : `${link.path}.md`,
                              content: newContent,
                            });
                            useAppStore.getState().openNote(
                              link.path.endsWith(".md") ? link.path : `${link.path}.md`,
                              newContent
                            );
                            const newNotes = useAppStore.getState().notes;
                            useAppStore.setState({ notes: newNotes });
                          } catch {
                            useAppStore.getState().openNote(
                              link.path,
                              newContent
                            );
                          }
                        }}
                      >
                        Create
                      </button>
                    )}
                  </div>
                ))}
              </>
            )}
          </div>
        </>
      )}
    </div>
  );
}
