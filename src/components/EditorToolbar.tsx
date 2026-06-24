import React from "react";
import { EditorView } from "@codemirror/view";
import FeroHaIcon from "./FeroHaIcon";

interface EditorToolbarProps {
  viewRef: React.MutableRefObject<EditorView | null>;
  viewMode?: "edit" | "preview";
  onToggleViewMode?: () => void;
  onToggleLineWrap?: () => void;
  lineWrapActive?: boolean;
  readOnly?: boolean;
}

function wrapSelection(view: EditorView, before: string, after: string) {
  const selection = view.state.selection.main;
  const text = view.state.sliceDoc(selection.from, selection.to);
  view.dispatch({
    changes: {
      from: selection.from,
      to: selection.to,
      insert: `${before}${text || "text"}${after}`,
    },
  });
}

function insertAtLineStart(view: EditorView, text: string) {
  const selection = view.state.selection.main;
  const line = view.state.doc.lineAt(selection.from);
  view.dispatch({
    changes: { from: line.from, to: line.from, insert: text },
  });
}

function insertAtEachLineStart(view: EditorView, text: string) {
  const selection = view.state.selection.main;
  const doc = view.state.doc;
  const fromLine = doc.lineAt(selection.from);
  const toLine = doc.lineAt(selection.to);
  const changes = [];
  for (let i = fromLine.number; i <= toLine.number; i++) {
    const line = doc.line(i);
    changes.push({ from: line.from, to: line.from, insert: text });
  }
  view.dispatch({ changes });
}

const buttons = [
  {
    icon: "Bold",
    label: "加粗",
    action: (view: EditorView) => wrapSelection(view, "**", "**"),
  },
  {
    icon: "Italic",
    label: "斜体",
    action: (view: EditorView) => wrapSelection(view, "*", "*"),
  },
  {
    icon: "Heading",
    label: "标题",
    action: (view: EditorView) => insertAtLineStart(view, "## "),
  },
  {
    icon: "Link",
    label: "链接",
    action: (view: EditorView) => wrapSelection(view, "[", "](url)"),
  },
  {
    icon: "List",
    label: "列表",
    action: (view: EditorView) => insertAtLineStart(view, "- "),
  },
  {
    icon: "Code",
    label: "代码",
    action: (view: EditorView) => wrapSelection(view, "`", "`"),
  },
  {
    icon: "Quote",
    label: "引用",
    action: (view: EditorView) => insertAtEachLineStart(view, "> "),
  },
  {
    icon: "ListOrdered",
    label: "有序列表",
    action: (view: EditorView) => insertAtEachLineStart(view, "1. "),
  },
  {
    icon: "Strikethrough",
    label: "删除线",
    action: (view: EditorView) => {
      const sel = view.state.selection.main;
      if (sel.empty) {
        view.dispatch({
          changes: { from: sel.from, insert: "~~~~" },
          selection: { anchor: sel.from + 2 },
        });
      } else {
        wrapSelection(view, "~~", "~~");
      }
    },
  },
  {
    icon: "Image",
    label: "图片",
    action: (view: EditorView) => {
      const sel = view.state.selection.main;
      view.dispatch({
        changes: { from: sel.from, insert: "![alt](url)" },
        selection: { anchor: sel.from + 2, head: sel.from + 5 },
      });
    },
  },
];

export default function EditorToolbar({
  viewRef,
  viewMode,
  onToggleViewMode,
  onToggleLineWrap,
  lineWrapActive,
  readOnly = false,
}: EditorToolbarProps) {
  const handleClick = (action: (view: EditorView) => void) => {
    if (readOnly) return;
    if (!viewRef.current) return;
    viewRef.current.focus();
    action(viewRef.current);
  };

  return (
    <div style={styles.toolbar}>
      {viewMode !== "preview" && !readOnly && buttons.map((btn) => (
        <button
          key={btn.icon}
          style={styles.btn}
          title={btn.label}
          aria-label={btn.label}
          onClick={() => handleClick(btn.action)}
        >
          <FeroHaIcon name={btn.icon} size={16} />
        </button>
      ))}
      {viewMode !== "preview" && onToggleLineWrap && (
        <button
          style={lineWrapActive ? { ...styles.btn, color: "var(--accent-primary)" } : styles.btn}
          title="切换自动换行"
          aria-label="切换自动换行"
          onClick={onToggleLineWrap}
        >
          <FeroHaIcon name="WrapText" size={16} />
        </button>
      )}
      {onToggleViewMode && (
        <button
          style={{ ...styles.btn, marginLeft: "auto" }}
          title={viewMode === "preview" ? "编辑模式" : "预览模式"}
          aria-label={viewMode === "preview" ? "编辑模式" : "预览模式"}
          onClick={onToggleViewMode}
        >
          <FeroHaIcon name={viewMode === "preview" ? "Edit" : "Eye"} size={16} />
        </button>
      )}
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  toolbar: {
    display: "flex",
    gap: "2px",
    padding: "4px 0",
    borderBottom: "1px solid var(--border-color)",
  },
  btn: {
    width: 28,
    height: 28,
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    backgroundColor: "transparent",
    color: "var(--icon-default)",
    border: "none",
    borderRadius: "4px",
    cursor: "pointer",
    padding: 0,
  },
};
