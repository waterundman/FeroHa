import { useEffect } from "react";
import { useAppStore } from "../hooks/useAppStore";

interface TagsPanelProps {
  isTauri: boolean;
}

const S = {
  container: {
    padding: "12px",
    borderTop: "1px solid var(--border-color)",
  } as React.CSSProperties,
  header: {
    display: "flex",
    alignItems: "center",
    gap: "6px",
    marginBottom: "8px",
    fontSize: "12px",
    fontWeight: 600,
    color: "var(--text-secondary)",
  } as React.CSSProperties,
  badge: {
    display: "inline-flex",
    alignItems: "center",
    justifyContent: "center",
    minWidth: "18px",
    height: "16px",
    padding: "0 4px",
    backgroundColor: "var(--bg-input)",
    borderRadius: "8px",
    fontSize: "10px",
    color: "var(--text-secondary)",
  } as React.CSSProperties,
  list: {
    maxHeight: "200px",
    overflowY: "auto",
    display: "flex",
    flexDirection: "column",
    gap: "2px",
  } as React.CSSProperties,
  tagItem: {
    display: "flex",
    alignItems: "center",
    justifyContent: "space-between",
    padding: "4px 8px",
    borderRadius: "4px",
    cursor: "pointer",
    fontSize: "12px",
    color: "var(--text-secondary)",
    transition: "background-color 0.1s",
  } as React.CSSProperties,
  tagItemSelected: {
    backgroundColor: "var(--bg-input)",
  } as React.CSSProperties,
  tagName: {
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
    flex: 1,
  } as React.CSSProperties,
  countBadge: {
    display: "inline-flex",
    alignItems: "center",
    justifyContent: "center",
    minWidth: "16px",
    height: "14px",
    padding: "0 4px",
    backgroundColor: "var(--accent-primary)22",
    borderRadius: "7px",
    fontSize: "10px",
    color: "var(--accent-primary)",
    marginLeft: "6px",
  } as React.CSSProperties,
  empty: {
    fontSize: "11px",
    color: "var(--text-muted)",
    padding: "4px 0 8px",
    fontStyle: "italic",
    lineHeight: "1.5",
  } as React.CSSProperties,
};

export default function TagsPanel({ isTauri }: TagsPanelProps) {
  const allTags = useAppStore((s) => s.allTags);
  const filterTags = useAppStore((s) => s.filterTags);
  const toggleFilterTag = useAppStore((s) => s.toggleFilterTag);
  const setAllTags = useAppStore((s) => s.setAllTags);

  useEffect(() => {
    if (!isTauri) return;
    let cancelled = false;
    (async () => {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        const tags = await invoke<{ name: string; count: number }[]>("list_tags");
        if (!cancelled) setAllTags(tags);
      } catch {
        // tag listing not available
      }
    })();
    return () => { cancelled = true; };
  }, [isTauri, setAllTags]);

  const sorted = [...allTags].sort((a, b) => b.count - a.count);

  return (
    <div style={S.container}>
      <div style={S.header}>
        Tags
        <span style={S.badge}>{allTags.length}</span>
      </div>
      <div style={S.list}>
        {sorted.length === 0 && (
          <div style={S.empty}>
            No tags yet. Add tags in frontmatter or use #tag in notes.
          </div>
        )}
        {sorted.map((tag) => {
          const isSelected = filterTags.includes(tag.name);
          return (
            <div
              key={tag.name}
              style={{
                ...S.tagItem,
                ...(isSelected ? S.tagItemSelected : {}),
              }}
              onClick={() => toggleFilterTag(tag.name)}
              onMouseEnter={(e) => {
                if (!isSelected)
                  (e.currentTarget as HTMLDivElement).style.backgroundColor = "var(--bg-input)66";
              }}
              onMouseLeave={(e) => {
                if (!isSelected)
                  (e.currentTarget as HTMLDivElement).style.backgroundColor = "transparent";
              }}
            >
              <span style={S.tagName}>#{tag.name}</span>
              <span style={S.countBadge}>{tag.count}</span>
            </div>
          );
        })}
      </div>
    </div>
  );
}
