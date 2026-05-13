import { useEffect, useRef, useState } from "react";
import { EditorView, basicSetup } from "codemirror";
import { markdown } from "@codemirror/lang-markdown";
import { keymap } from "@codemirror/view";
import { useAppStore } from "../hooks/useAppStore";

interface EditorProps {
  isTauri: boolean;
}

export default function Editor({ isTauri }: EditorProps) {
  const editorRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);
  const currentNote = useAppStore((s) => s.currentNote);
  const setCurrentContent = useAppStore((s) => s.setCurrentContent);
  const markClean = useAppStore((s) => s.markClean);
  const isDirty = useAppStore((s) => s.isDirty);
  const [wordCount, setWordCount] = useState(0);
  const [saveStatus, setSaveStatus] = useState("");

  useEffect(() => {
    if (!editorRef.current) return;

    const editorView = new EditorView({
      doc: currentNote?.content || "# Welcome to Dual-Track Note IDE\n\nStart writing...\n\nTry `/agent` to invoke AI commands.",
      extensions: [
        basicSetup,
        markdown(),
        keymap.of([]),
        EditorView.updateListener.of((update) => {
          if (update.docChanged) {
            const text = update.state.doc.toString();
            setCurrentContent(text);
            setWordCount(text.split(/\s+/).filter(Boolean).length);
          }
        }),
        EditorView.theme({
          "&": { height: "100%", fontSize: "15px", lineHeight: "1.7" },
          ".cm-content": {
            fontFamily: "'JetBrains Mono', 'Fira Code', monospace",
            padding: "0",
          },
          ".cm-gutters": { display: "none" },
          ".cm-activeLine": { backgroundColor: "#31324444" },
          ".cm-selectionBackground": { backgroundColor: "#45475a88" },
        }),
      ],
      parent: editorRef.current,
    });

    viewRef.current = editorView;

    // Ctrl+S handler
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === "s") {
        e.preventDefault();
        handleSave();
      }
    };
    window.addEventListener("keydown", handleKeyDown);

    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      editorView.destroy();
    };
  }, []);

  // Sync editor content when switching notes externally
  useEffect(() => {
    if (viewRef.current && currentNote) {
      const currentDoc = viewRef.current.state.doc.toString();
      if (currentDoc !== currentNote.content) {
        viewRef.current.dispatch({
          changes: {
            from: 0,
            to: currentDoc.length,
            insert: currentNote.content,
          },
        });
      }
    }
  }, [currentNote?.path]);

  const handleSave = async () => {
    if (!viewRef.current) return;
    const content = viewRef.current.state.doc.toString();

    if (isTauri && currentNote) {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        await invoke("save_note", {
          path: currentNote.path,
          content,
        });
        markClean();
        setSaveStatus("Saved ✓");
        setTimeout(() => setSaveStatus(""), 2000);
      } catch (e) {
        setSaveStatus("Save failed!");
        console.error(e);
      }
    } else {
      // Browser fallback
      const blob = new Blob([content], { type: "text/markdown" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = currentNote?.path || "untitled.md";
      a.click();
      URL.revokeObjectURL(url);
      setSaveStatus("Downloaded ✓");
      setTimeout(() => setSaveStatus(""), 2000);
    }
  };

  return (
    <div style={styles.wrapper}>
      <div style={styles.toolbar}>
        <div style={styles.fileInfo}>
          <span style={styles.fileName}>
            {currentNote?.path || "untitled.md"}
          </span>
          {isDirty && <span style={styles.dirtyMark}>●</span>}
        </div>
        <div style={styles.toolbarRight}>
          <span style={styles.wordCount}>{wordCount} words</span>
          <span style={styles.saveStatus}>{saveStatus}</span>
          <button style={styles.saveBtn} onClick={handleSave}>
            Save
          </button>
        </div>
      </div>
      <div ref={editorRef} style={styles.editor} />
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  wrapper: {
    display: "flex",
    flexDirection: "column",
    height: "100%",
  },
  toolbar: {
    display: "flex",
    justifyContent: "space-between",
    alignItems: "center",
    padding: "6px 0",
    borderBottom: "1px solid #313244",
    marginBottom: "12px",
    fontSize: "12px",
  },
  fileInfo: {
    display: "flex",
    alignItems: "center",
    gap: "6px",
  },
  fileName: {
    color: "#cdd6f4",
    fontWeight: 600,
  },
  dirtyMark: {
    color: "#f9e2af",
    fontSize: "14px",
  },
  toolbarRight: {
    display: "flex",
    alignItems: "center",
    gap: "10px",
  },
  wordCount: {
    color: "#6c7086",
  },
  saveStatus: {
    color: "#a6e3a1",
    fontSize: "11px",
    minWidth: "70px",
    textAlign: "right",
  },
  saveBtn: {
    padding: "4px 12px",
    backgroundColor: "#45475a",
    color: "#cdd6f4",
    border: "1px solid #585b70",
    borderRadius: "4px",
    cursor: "pointer",
    fontSize: "12px",
  },
  editor: {
    flex: 1,
    overflow: "auto",
  },
};
