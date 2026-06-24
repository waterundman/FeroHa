import { useState, useEffect, useMemo, useCallback, useRef } from "react";
import { useAppStore } from "../hooks/useAppStore";
import FeroHaIcon from "./FeroHaIcon";

interface QuickSwitcherProps {
  isTauri: boolean;
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
}

interface SearchResult {
  type: "note" | "create";
  path: string;
  title: string;
}

function highlightMatches(text: string, query: string): React.ReactNode {
  if (!query.trim()) return text;
  const lowerText = text.toLowerCase();
  const tokens = query.toLowerCase().split(/\s+/).filter(Boolean);
  if (tokens.length === 0) return text;

  const ranges: Array<{ start: number; end: number }> = [];
  for (const token of tokens) {
    let idx = 0;
    while (idx < lowerText.length) {
      const pos = lowerText.indexOf(token, idx);
      if (pos === -1) break;
      ranges.push({ start: pos, end: pos + token.length });
      idx = pos + 1;
    }
  }

  ranges.sort((a, b) => a.start - b.start);

  const merged: Array<{ start: number; end: number }> = [];
  for (const r of ranges) {
    if (merged.length > 0 && r.start <= merged[merged.length - 1].end) {
      merged[merged.length - 1].end = Math.max(merged[merged.length - 1].end, r.end);
    } else {
      merged.push(r);
    }
  }

  if (merged.length === 0) return text;

  const parts: React.ReactNode[] = [];
  let last = 0;
  for (const r of merged) {
    if (r.start > last) {
      parts.push(text.slice(last, r.start));
    }
    parts.push(
      <mark key={r.start} style={styles.mark}>
        {text.slice(r.start, r.end)}
      </mark>
    );
    last = r.end;
  }
  if (last < text.length) {
    parts.push(text.slice(last));
  }
  return parts;
}

export default function QuickSwitcher({ isTauri, open: openProp, onOpenChange }: QuickSwitcherProps) {
  const isControlled = openProp !== undefined;
  const [internalOpen, setInternalOpen] = useState(false);
  const isOpen = isControlled ? openProp : internalOpen;

  const setOpen = useCallback(
    (value: boolean) => {
      if (isControlled) {
        onOpenChange?.(value);
      } else {
        setInternalOpen(value);
      }
    },
    [isControlled, onOpenChange]
  );

  const [query, setQuery] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  const notes = useAppStore((s) => s.notes);
  const recentNotes = useAppStore((s) => s.recentNotes);

  const results = useMemo((): SearchResult[] => {
    if (!query.trim()) {
      return recentNotes.slice(0, 5).map((path) => {
        const note = notes.find((n) => n.path === path);
        return { type: "note" as const, path, title: note?.title ?? path };
      });
    }

    const tokens = query
      .toLowerCase()
      .split(/\s+/)
      .filter(Boolean);

    const matched: SearchResult[] = [];
    for (const note of notes) {
      const lowerTitle = note.title.toLowerCase();
      const allMatch = tokens.every((t) => lowerTitle.includes(t));
      if (allMatch) {
        matched.push({ type: "note", path: note.path, title: note.title });
      }
      if (matched.length >= 15) break;
    }

    if (matched.length === 0 && query.trim()) {
      matched.push({ type: "create", path: query, title: query });
    }

    return matched;
  }, [query, notes, recentNotes]);

  const prevQueryRef = useRef(query);
  useEffect(() => {
    if (query !== prevQueryRef.current) {
      setSelectedIndex(0);
      prevQueryRef.current = query;
    }
  }, [query]);

  useEffect(() => {
    if (isOpen && inputRef.current) {
      inputRef.current.focus();
    }
  }, [isOpen]);

  const closeSwitcher = useCallback(() => {
    setOpen(false);
    setQuery("");
    setSelectedIndex(0);
  }, [setOpen]);

  const selectResult = useCallback(
    async (result: SearchResult) => {
      if (result.type === "note") {
        if (isTauri) {
          try {
            const { invoke } = await import("@tauri-apps/api/core");
            const content = await invoke<string>("read_note", { path: result.path });
            useAppStore.getState().openNote(result.path, content);
          } catch {
            // Fallback: open without content
          }
        }
      } else {
        if (isTauri) {
          try {
            const { invoke } = await import("@tauri-apps/api/core");
            const generatedPath = result.path.endsWith(".md")
              ? result.path
              : result.path + ".md";
            await invoke("create_note", { path: generatedPath });
            const content = await invoke<string>("read_note", { path: generatedPath });
            useAppStore.getState().openNote(generatedPath, content);
          } catch {
            // Silently fail
          }
        }
      }
      closeSwitcher();
    },
    [closeSwitcher, isTauri]
  );

  const handleEnter = useCallback(() => {
    if (results.length === 0) return;
    const idx = Math.min(selectedIndex, results.length - 1);
    selectResult(results[idx]);
  }, [results, selectedIndex, selectResult]);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "p" && (e.ctrlKey || e.metaKey)) {
        if (!isControlled) {
          e.preventDefault();
          setOpen(!isOpen);
        }
        return;
      }

      if (!isOpen) return;

      if (e.key === "Escape") {
        e.preventDefault();
        closeSwitcher();
        return;
      }

      if (e.key === "ArrowDown") {
        e.preventDefault();
        setSelectedIndex((prev) =>
          results.length > 0 ? (prev + 1) % results.length : 0
        );
        return;
      }

      if (e.key === "ArrowUp") {
        e.preventDefault();
        setSelectedIndex((prev) =>
          results.length > 0 ? (prev - 1 + results.length) % results.length : 0
        );
        return;
      }

      if (e.key === "Enter") {
        e.preventDefault();
        handleEnter();
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [isOpen, results, selectedIndex, handleEnter, closeSwitcher, isControlled, setOpen]);

  if (!isOpen) return null;

  return (
    <div style={styles.backdrop} onClick={closeSwitcher}>
      <div style={styles.modal} onClick={(e) => e.stopPropagation()}>
        <div style={styles.header}>
          <div style={styles.headerTitleRow}>
            <FeroHaIcon name="Search" size={16} />
            <span style={styles.headerTitle}>快速切换</span>
          </div>
          <kbd style={styles.shortcut}>Ctrl+P</kbd>
        </div>
        <input
          ref={inputRef}
          type="text"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="搜索笔记或创建新笔记..."
          style={styles.input}
          autoFocus
        />
        <div style={styles.results}>
          {results.map((result, i) =>
            result.type === "create" ? (
              <div
                key="create"
                style={{
                  ...styles.resultItem,
                  ...(i === selectedIndex ? styles.resultItemSelected : {}),
                }}
                onClick={() => selectResult(result)}
              >
                <FeroHaIcon name="Plus" size={14} />
                <span style={{ color: "var(--diff-insert)" }}>创建笔记：{result.title}</span>
              </div>
            ) : (
              <div
                key={result.path}
                style={{
                  ...styles.resultItem,
                  ...(i === selectedIndex ? styles.resultItemSelected : {}),
                }}
                onClick={() => selectResult(result)}
              >
                <span style={styles.resultTitle}>
                  {highlightMatches(result.title, query)}
                </span>
                <span style={styles.resultPath}>{result.path}</span>
              </div>
            )
          )}
          {results.length === 0 && !query.trim() && (
            <div style={styles.empty}>输入关键词搜索或创建笔记</div>
          )}
          {results.length === 0 && query.trim() && (
            <div style={styles.empty}>无结果</div>
          )}
        </div>
      </div>
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  backdrop: {
    position: "fixed",
    top: 0,
    left: 0,
    right: 0,
    bottom: 0,
    backgroundColor: "rgba(0, 0, 0, 0.38)",
    zIndex: 1000,
    display: "flex",
    justifyContent: "center",
    padding: "14vh 16px 16px",
    backdropFilter: "blur(4px)",
  },
  modal: {
    width: "min(620px, 100%)",
    maxHeight: "min(520px, 78vh)",
    backgroundColor: "var(--bg-primary)",
    border: "1px solid var(--border-color)",
    borderRadius: "10px",
    overflow: "hidden",
    alignSelf: "flex-start",
    boxShadow: "0 24px 70px rgba(0, 0, 0, 0.46)",
    display: "flex",
    flexDirection: "column",
  },
  header: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    gap: "12px",
    padding: "12px 14px",
    borderBottom: "1px solid var(--border-muted)",
    backgroundColor: "var(--bg-secondary)",
  },
  headerTitleRow: {
    display: "inline-flex",
    alignItems: "center",
    gap: "8px",
    minWidth: 0,
    color: "var(--text-primary)",
  },
  headerTitle: {
    fontSize: "14px",
    fontWeight: 700,
    color: "var(--text-primary)",
  },
  shortcut: {
    flex: "0 0 auto",
    border: "1px solid var(--border-color)",
    borderRadius: "5px",
    backgroundColor: "var(--bg-input)",
    color: "var(--text-secondary)",
    fontSize: "11px",
    padding: "2px 7px",
    fontFamily: "var(--font-mono)",
  },
  input: {
    width: "100%",
    padding: "13px 16px",
    backgroundColor: "var(--bg-input)",
    color: "var(--text-primary)",
    border: "none",
    borderBottom: "1px solid var(--border-color)",
    fontSize: "14px",
    outline: "none",
    boxSizing: "border-box",
  },
  results: {
    maxHeight: "360px",
    overflowY: "auto",
    padding: "6px",
  },
  resultItem: {
    display: "flex",
    alignItems: "center",
    padding: "10px 12px",
    cursor: "pointer",
    gap: "12px",
    borderRadius: "7px",
    minWidth: 0,
  },
  resultItemSelected: {
    backgroundColor: "var(--bg-input)",
  },
  resultTitle: {
    fontWeight: "bold",
    color: "var(--text-primary)",
    whiteSpace: "nowrap",
    overflow: "hidden",
    textOverflow: "ellipsis",
    flex: "1 1 auto",
    minWidth: 0,
  },
  resultPath: {
    fontSize: "12px",
    color: "var(--text-muted)",
    whiteSpace: "nowrap",
    overflow: "hidden",
    textOverflow: "ellipsis",
    marginLeft: "auto",
    maxWidth: "42%",
    minWidth: 0,
  },
  mark: {
    backgroundColor: "var(--diff-warn)",
    color: "var(--bg-primary)",
    borderRadius: "2px",
    padding: "0 1px",
  },
  empty: {
    padding: "16px",
    color: "var(--text-muted)",
    textAlign: "center",
    fontSize: "14px",
  },
};
