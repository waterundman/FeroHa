import { useAppStore } from "../hooks/useAppStore";
import { triggerGoToLine } from "./Editor";

interface StatusBarProps {
  onCursorClick?: () => void;
}

export default function StatusBar({ onCursorClick }: StatusBarProps) {
  const currentNote = useAppStore((s) => s.currentNote);
  const cursorLine = useAppStore((s) => s.cursorLine);
  const cursorCol = useAppStore((s) => s.cursorCol);
  const isDirty = useAppStore((s) => s.isDirty);
  const saveStatus = useAppStore((s) => s.saveStatus);
  const mode = useAppStore((s) => s.mode);

  const filePath = currentNote?.path || "No file open";
  const encoding = "UTF-8";

  const saveLabel =
    saveStatus === "saving"
      ? "Saving..."
      : saveStatus === "success"
        ? "Saved"
        : saveStatus === "error"
          ? "Save failed"
          : isDirty
            ? "Unsaved"
            : "Saved";

  const saveColor =
    saveStatus === "saving"
      ? "var(--diff-warn)"
      : saveStatus === "success"
        ? "var(--diff-insert)"
        : saveStatus === "error"
          ? "var(--diff-delete)"
          : isDirty
            ? "var(--diff-warn)"
            : "var(--text-muted)";

  return (
    <div style={styles.bar} role="status" aria-live="polite">
      <div style={styles.left}>
        <span style={styles.item} title={filePath}>
          {filePath}
        </span>
        {isDirty && <span style={{ width: 8, height: 8, borderRadius: "50%", backgroundColor: "var(--accent-primary)", display: "inline-block" }} />}
      </div>
      <div style={styles.right}>
        <span
          style={{ ...styles.item, cursor: "pointer" }}
          title="Click to go to line..."
          onClick={() => {
            if (onCursorClick) {
              onCursorClick();
            } else {
              const input = window.prompt("Go to line:");
              if (input !== null) {
                const line = parseInt(input, 10);
                if (!isNaN(line) && line > 0) {
                  triggerGoToLine(line);
                }
              }
            }
          }}
        >
          Ln {cursorLine}, Col {cursorCol}
        </span>
        <span style={styles.separator}>|</span>
        <span style={styles.item}>{encoding}</span>
        <span style={styles.separator}>|</span>
        <span style={{ ...styles.item, color: saveColor }}>{saveLabel}</span>
        <span style={styles.separator}>|</span>
        <span style={styles.item}>{mode === "ai" ? "AI" : "Human"}</span>
      </div>
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  bar: {
    display: "flex",
    justifyContent: "space-between",
    alignItems: "center",
    padding: "2px 12px",
    backgroundColor: "var(--bg-secondary)",
    borderTop: "1px solid var(--border-color)",
    fontSize: "11px",
    color: "var(--text-muted)",
    userSelect: "none",
    minHeight: "22px",
  },
  left: {
    display: "flex",
    alignItems: "center",
    gap: "6px",
    overflow: "hidden",
    flex: 1,
  },
  right: {
    display: "flex",
    alignItems: "center",
    gap: "8px",
    flexShrink: 0,
  },
  item: {
    whiteSpace: "nowrap",
    overflow: "hidden",
    textOverflow: "ellipsis",
  },
  separator: {
    color: "var(--border-color)",
  },
};
