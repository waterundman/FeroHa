import { useAppStore } from "../hooks/useAppStore";
import { triggerGoToLine } from "./Editor";
import FeroHaIcon from "./FeroHaIcon";

interface StatusBarProps {
  onCursorClick?: () => void;
}

type SaveStatus = "idle" | "saving" | "success" | "error";
type Mode = "human" | "ai";

export const emptyFileLabel = "未打开文件";

export function formatCursorLabel(line: number, col: number): string {
  return `行 ${line}，列 ${col}`;
}

export function saveStatusLabel(saveStatus: SaveStatus, isDirty: boolean): string {
  if (saveStatus === "saving") return "保存中";
  if (saveStatus === "success") return "已保存";
  if (saveStatus === "error") return "保存失败";
  return isDirty ? "未保存" : "已保存";
}

export function modeStatusLabel(mode: Mode): string {
  return mode === "ai" ? "AI 面" : "人类面";
}

function saveStatusIcon(saveStatus: SaveStatus, isDirty: boolean): string {
  if (saveStatus === "saving") return "Loader";
  if (saveStatus === "error") return "AlertCircle";
  if (isDirty) return "Circle";
  return "CircleCheck";
}

function saveStatusColor(saveStatus: SaveStatus, isDirty: boolean): string {
  if (saveStatus === "saving") return "var(--diff-warn)";
  if (saveStatus === "success") return "var(--diff-insert)";
  if (saveStatus === "error") return "var(--diff-delete)";
  return isDirty ? "var(--diff-warn)" : "var(--text-muted)";
}

function runCursorJump(onCursorClick?: () => void) {
  if (onCursorClick) {
    onCursorClick();
    return;
  }
  const input = window.prompt("跳转到行：");
  if (input !== null) {
    const line = parseInt(input, 10);
    if (!isNaN(line) && line > 0) triggerGoToLine(line);
  }
}

export default function StatusBar({ onCursorClick }: StatusBarProps) {
  const currentNote = useAppStore((s) => s.currentNote);
  const cursorLine = useAppStore((s) => s.cursorLine);
  const cursorCol = useAppStore((s) => s.cursorCol);
  const isDirty = useAppStore((s) => s.isDirty);
  const saveStatus = useAppStore((s) => s.saveStatus);
  const mode = useAppStore((s) => s.mode);

  const filePath = currentNote?.path || emptyFileLabel;
  const saveLabel = saveStatusLabel(saveStatus, isDirty);
  const surfaceLabel = modeStatusLabel(mode);
  const saveColor = saveStatusColor(saveStatus, isDirty);

  return (
    <div style={styles.bar} role="status" aria-live="polite">
      <div style={styles.left}>
        <span style={styles.fileChip} title={filePath}>
          <FeroHaIcon name="FileText" size={13} />
          <span style={styles.fileText}>{filePath}</span>
          {isDirty && <span style={styles.dirtyDot} title="未保存修改" />}
        </span>
      </div>
      <div style={styles.right}>
        <button
          type="button"
          style={{ ...styles.chip, ...styles.cursorChip }}
          title="跳转到行"
          aria-label="跳转到行"
          onClick={() => runCursorJump(onCursorClick)}
        >
          <FeroHaIcon name="MapPin" size={12} />
          <span>{formatCursorLabel(cursorLine, cursorCol)}</span>
        </button>
        <span style={{ ...styles.chip, color: saveColor }} aria-label="保存状态">
          <FeroHaIcon name={saveStatusIcon(saveStatus, isDirty)} size={12} />
          <span>{saveLabel}</span>
        </span>
        <span
          style={{
            ...styles.chip,
            ...styles.modeChip,
            color: mode === "ai" ? "var(--accent-primary)" : "var(--text-secondary)",
          }}
          aria-label="当前面向"
        >
          <FeroHaIcon name={mode === "ai" ? "Bot" : "User"} size={12} />
          <span>{surfaceLabel}</span>
        </span>
      </div>
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  bar: {
    display: "flex",
    justifyContent: "space-between",
    alignItems: "center",
    padding: "4px 10px",
    backgroundColor: "var(--bg-secondary)",
    borderTop: "1px solid var(--border-color)",
    fontSize: "11px",
    color: "var(--text-muted)",
    userSelect: "none",
    minHeight: "28px",
    gap: "10px",
  },
  left: {
    display: "flex",
    alignItems: "center",
    overflow: "hidden",
    flex: 1,
    minWidth: 0,
  },
  right: {
    display: "flex",
    alignItems: "center",
    justifyContent: "flex-end",
    gap: "6px",
    flexShrink: 0,
    minWidth: 0,
  },
  fileChip: {
    display: "inline-flex",
    alignItems: "center",
    gap: "6px",
    minWidth: 0,
    maxWidth: "100%",
    padding: "2px 7px",
    borderRadius: "999px",
    background: "color-mix(in srgb, var(--bg-primary) 72%, transparent)",
    border: "1px solid var(--border-muted)",
    color: "var(--text-secondary)",
  },
  fileText: {
    whiteSpace: "nowrap",
    overflow: "hidden",
    textOverflow: "ellipsis",
    minWidth: 0,
  },
  chip: {
    display: "inline-flex",
    alignItems: "center",
    justifyContent: "center",
    gap: "4px",
    minHeight: "20px",
    padding: "2px 7px",
    borderRadius: "999px",
    background: "color-mix(in srgb, var(--bg-primary) 68%, transparent)",
    border: "1px solid var(--border-muted)",
    color: "var(--text-muted)",
    lineHeight: 1,
    whiteSpace: "nowrap",
    letterSpacing: 0,
  },
  cursorChip: {
    font: "inherit",
    cursor: "pointer",
  },
  modeChip: {
    fontWeight: 600,
  },
  dirtyDot: {
    width: 6,
    height: 6,
    borderRadius: "50%",
    backgroundColor: "var(--accent-primary)",
    display: "inline-block",
    flexShrink: 0,
  },
};
