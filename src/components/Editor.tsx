import { useCallback, useEffect, useRef, useState } from "react";
import { EditorView, basicSetup } from "codemirror";
import { markdown } from "@codemirror/lang-markdown";
import { keymap, ViewPlugin, Decoration, DecorationSet, ViewUpdate } from "@codemirror/view";
import { EditorState, RangeSetBuilder, Compartment } from "@codemirror/state";
import { searchKeymap } from "@codemirror/search";
import { autocompletion, closeBrackets, closeBracketsKeymap } from "@codemirror/autocomplete";
import { slashCommandSource, wikiLinkSource, headingSource, tagSource } from "../lib/completionSources";
import { marked } from "marked";
import { useAppStore, type NoteMeta } from "../hooks/useAppStore";
import { useSettings, useSettingsStore } from "../hooks/useSettings";
import { sendTaskToAgent } from "../lib/ipc";
import { showToast } from "./toastBus";
import EditorToolbar from "./EditorToolbar";
import SelectionToolbar from "./SelectionToolbar";
import FeroHaIcon from "./FeroHaIcon";
import ContextMenu, { type ContextMenuItem } from "./ContextMenu";

const lineWrapCompartment = new Compartment();

let _goToLineHandler: ((line: number) => void) | null = null;
export function triggerGoToLine(line: number) {
  _goToLineHandler?.(line);
}

interface EditorProps {
  isTauri: boolean;
  note?: { path: string; content: string; isDirty: boolean } | null;
  readOnly?: boolean;
  readOnlyLabel?: string;
  onCreateNote?: () => void;
}

function countWords(content: string) {
  return content.split(/\s+/).filter(Boolean).length;
}

function getHeadings(content: string): { level: number; text: string; pos: number }[] {
  const headings: { level: number; text: string; pos: number }[] = [];
  const regex = /^(#{1,6})\s+(.+)$/gm;
  let match;
  while ((match = regex.exec(content)) !== null) {
    headings.push({
      level: match[1].length,
      text: match[2],
      pos: match.index,
    });
  }
  return headings;
}

async function emitNoteOpened(notePath: string, isTauri: boolean) {
  if (!isTauri || !notePath) return;
  try {
    const { emit } = await import("@tauri-apps/api/event");
    await emit("note-opened", { note_id: notePath });
  } catch {
    // Event emission is best-effort.
  }
}

async function emitSelectionSubmit(
  notePath: string,
  content: string,
  start: number,
  end: number,
  isTauri: boolean
) {
  if (!isTauri || !notePath) return;
  try {
    const { emit } = await import("@tauri-apps/api/event");
    await emit("selection-submit", {
      note_id: notePath,
      content,
      start,
      end,
    });
  } catch {
    // Event emission is best-effort.
  }
}

const tagRegex = /(?:^|\s)#([a-zA-Z\u4e00-\u9fff][\w\u4e00-\u9fff/-]*)/g;

function renderMarkdown(content: string): string {
  const withWikiLinks = content.replace(
    /\[\[([^\]]+)\]\]/g,
    (_match, target) => `[${target}](#/wiki/${encodeURIComponent(target)})`
  );
  return marked.parse(withWikiLinks) as string;
}

const tagHighlighter = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;
    constructor(view: EditorView) {
      this.decorations = this.buildDecorations(view);
    }
    update(update: ViewUpdate) {
      if (update.docChanged || update.viewportChanged)
        this.decorations = this.buildDecorations(update.view);
    }
    buildDecorations(view: EditorView) {
      const builder = new RangeSetBuilder<Decoration>();
      const doc = view.state.doc;
      let match;
      while ((match = tagRegex.exec(doc.toString())) !== null) {
        const start = match.index + match[0].indexOf('#');
        const end = start + match[0].trim().length;
        builder.add(start, end, Decoration.mark({ class: 'cm-tag' }));
      }
      return builder.finish();
    }
  },
  { decorations: (v) => v.decorations }
);

const listContinuationKeymap = keymap.of([
  {
    key: 'Enter',
    run: (view: EditorView) => {
      const { state, dispatch } = view;
      const pos = state.selection.main.head;
      const line = state.doc.lineAt(pos);
      const lineText = line.text;
      const listMatch = lineText.match(/^(\s*)([-*+]|\d+\.)\s+(.*)/);
      if (listMatch) {
        const [, indent, marker, content] = listMatch;
        if (content.trim() === '') {
          const changes = { from: line.from, to: line.to, insert: '' };
          dispatch(state.update({ changes, selection: { anchor: line.from } }));
          return true;
        } else {
          const newMarker = /^\d+\./.test(marker)
            ? `${parseInt(marker) + 1}. `
            : `${marker} `;
          dispatch(state.update({
            changes: { from: pos, insert: `\n${indent}${newMarker}` },
            selection: { anchor: pos + 1 + indent.length + newMarker.length }
          }));
          return true;
        }
      }
      return false;
    }
  },
  {
    key: 'Tab',
    run: (view: EditorView) => {
      const { state, dispatch } = view;
      const pos = state.selection.main.head;
      const line = state.doc.lineAt(pos);
      const lineText = line.text;
      const indentMatch = lineText.match(/^(\s*)([-*+]|\d+\.)\s/);
      if (indentMatch && state.selection.main.empty) {
        dispatch(state.update({
          changes: { from: line.from, insert: '  ' },
          selection: { anchor: pos + 2 }
        }));
        return true;
      }
      return false;
    }
  },
  {
    key: 'Shift-Tab',
    run: (view: EditorView) => {
      const { state, dispatch } = view;
      const pos = state.selection.main.head;
      const line = state.doc.lineAt(pos);
      const lineText = line.text;
      if (lineText.startsWith('  ') && state.selection.main.empty) {
        const leadingSpaces = lineText.match(/^ */)!
        const removeCount = Math.min(2, leadingSpaces[0].length);
        dispatch(state.update({
          changes: { from: line.from, to: line.from + removeCount },
          selection: { anchor: pos - removeCount }
        }));
        return true;
      }
      return false;
    }
  }
]);

export default function Editor({ isTauri, note, readOnly = false, readOnlyLabel, onCreateNote }: EditorProps) {
  const editorRef = useRef<HTMLDivElement>(null);
  const viewRef = useRef<EditorView | null>(null);
  const handleSaveRef = useRef<(() => Promise<void>) | null>(null);

  const storeCurrentNote = useAppStore((s) => s.currentNote);
  const vaultPath = useAppStore((s) => s.vaultPath);
  const storeIsDirty = useAppStore((s) => s.isDirty);
  const allNotes = useAppStore((s) => s.notes);
  const setCurrentContent = useAppStore((s) => s.setCurrentContent);
  const markClean = useAppStore((s) => s.markClean);
  const setCursorPos = useAppStore((s) => s.setCursorPos);
  const setSaveStatus = useAppStore((s) => s.setSaveStatus);
  const saveStatus = useAppStore((s) => s.saveStatus);
  const setTabContent = useAppStore((s) => s.setTabContent);
  const markTabClean = useAppStore((s) => s.markTabClean);

  const currentNote = note ?? storeCurrentNote;
  const hasOpenNote = Boolean(currentNote);
  const isDirty = note ? note.isDirty : storeIsDirty;
  const isReadOnlyNote = Boolean(readOnly || currentNote?.path.startsWith(".dualtrack/"));
  const readOnlyBadgeLabel = readOnlyLabel ?? "AI 工作区只读";
  const canCreateNoteFromEmptyState = Boolean(onCreateNote) && !readOnly && (!isTauri || Boolean(vaultPath));

  const [outlineVisible, setOutlineVisible] = useState(false);
  const [wordWrap, setWordWrap] = useState(false);
  const [showMeta, setShowMeta] = useState(false);
  const [backlinksCount, setBacklinksCount] = useState(0);
  const [settings] = useSettings();
  const defaultViewMode = useSettingsStore((s) => s.settings.defaultViewMode) || "edit";
  const [viewMode, setViewMode] = useState<"edit" | "preview">(defaultViewMode);
  const suppressChangeRef = useRef(false);

  const currentNoteRef = useRef(currentNote);
  const externalNoteModeRef = useRef(Boolean(note));
  const isTauriRef = useRef(isTauri);

  const [selToolbar, setSelToolbar] = useState<{ x: number; y: number; text: string } | null>(null);
  const [selectionContextMenu, setSelectionContextMenu] = useState<{
    x: number;
    y: number;
    text: string;
  } | null>(null);

  useEffect(() => {
    currentNoteRef.current = currentNote;
  }, [currentNote]);

  useEffect(() => {
    externalNoteModeRef.current = Boolean(note);
  }, [note]);

  useEffect(() => {
    isTauriRef.current = isTauri;
  }, [isTauri]);

  const handleSave = useCallback(async () => {
    if (isReadOnlyNote) {
      setSaveStatus("idle");
      showToast("info", `${readOnlyBadgeLabel}，未写入文件`);
      return;
    }

    if (!currentNote) {
      setSaveStatus("idle");
      showToast("info", isTauri && !vaultPath ? "先打开一个笔记库，再新建文档" : "先新建或打开一篇笔记");
      return;
    }

    const content = viewRef.current
      ? viewRef.current.state.doc.toString()
      : currentNote.content;

    setSaveStatus("saving");

    if (isTauri) {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        await invoke("save_note", {
          path: currentNote.path,
          content,
        });
        if (note) {
          markTabClean(currentNote.path);
        } else {
          markClean();
        }
        setSaveStatus("success");
        showToast("success", "Note saved successfully");
        setTimeout(() => setSaveStatus("idle"), 2000);
      } catch (e) {
        setSaveStatus("error");
        showToast("error", "Failed to save note");
        console.error(e);
        setTimeout(() => setSaveStatus("idle"), 3000);
      }
    } else {
      const blob = new Blob([content], { type: "text/markdown" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = currentNote.path;
      a.click();
      URL.revokeObjectURL(url);
      setSaveStatus("success");
      showToast("success", "Note downloaded successfully");
      setTimeout(() => setSaveStatus("idle"), 2000);
    }
  }, [currentNote, isReadOnlyNote, isTauri, markClean, readOnlyBadgeLabel, setSaveStatus, note, markTabClean, vaultPath]);

  useEffect(() => {
    handleSaveRef.current = handleSave;
  }, [handleSave]);

  const captureSelectionSnapshot = useCallback(async (text: string) => {
    const notePath = currentNote?.path;
    if (!notePath || !isTauri) return;

    const view = viewRef.current;
    let start = 0;
    let end = text.length;
    if (view && !view.state.selection.main.empty) {
      start = view.state.selection.main.from;
      end = view.state.selection.main.to;
    } else if (currentNote?.content) {
      const index = currentNote.content.indexOf(text);
      if (index >= 0) {
        start = index;
        end = index + text.length;
      }
    }

    await emitSelectionSubmit(notePath, text, start, end, isTauri);
  }, [currentNote?.content, currentNote?.path, isTauri]);

  const submitSelectionTaskToAi = useCallback(async (action: string, text: string) => {
    const trimmed = text.trim();
    if (!trimmed) return;

    if (!isTauri) {
      showToast("info", `${action} task staged in browser preview`);
      return;
    }

    const taskType = action === "analyze" || action === "submit" ? "research" : "write_proposal";
    const reviewMode = taskType === "research" ? "read_only_auto_queue" : "draft_only";
    try {
      await captureSelectionSnapshot(trimmed);
      const result = await sendTaskToAgent({
        content: trimmed,
        intent: `${action}: ${currentNote?.path ?? "selection"}`,
        contextNote: currentNote?.path ?? null,
        scope: "selected_text",
        source: "human_editor_context_menu",
        reviewMode,
        expectedOutput:
          taskType === "research"
            ? "Read the selected text and return an analysis for the human review surface."
            : "Create a draft proposal from the selected text without writing to files.",
        taskType,
      });
      showToast(
        "info",
        `${action} task submitted to AI Manager (${result.status})`
      );
    } catch (e) {
      console.error("Selection task context menu error:", e);
      const message = e instanceof Error ? e.message : String(e);
      showToast("error", `${action} task failed: ${message}`);
    }
  }, [captureSelectionSnapshot, currentNote?.path, isTauri]);

  const handleEditorContextMenu = useCallback((event: MouseEvent, view: EditorView) => {
    const sel = view.state.selection.main;
    if (sel.empty) return false;
    const selText = view.state.sliceDoc(sel.from, sel.to);
    if (!selText.trim()) return false;

    event.preventDefault();
    event.stopPropagation();
    setSelToolbar(null);
    setSelectionContextMenu({
      x: event.clientX,
      y: event.clientY,
      text: selText,
    });
    return true;
  }, []);

  const handleEmptyCreateClick = useCallback(() => {
    if (readOnly) return;
    if (isTauri && !vaultPath) {
      showToast("info", "先打开一个笔记库，再新建文档");
      return;
    }
    onCreateNote?.();
  }, [isTauri, onCreateNote, readOnly, vaultPath]);

  const handlePaste = useCallback(async (event: ClipboardEvent) => {
    const items = event.clipboardData?.items;
    if (!items) return;
    for (const item of Array.from(items)) {
      if (item.type.startsWith('image/')) {
        event.preventDefault();
        const file = item.getAsFile();
        if (!file) continue;
        const ext = item.type.split('/')[1] || 'png';
        const timestamp = Date.now();
        const filename = `paste-${timestamp}.${ext}`;
        const buffer = await file.arrayBuffer();
        const uint8Array = new Uint8Array(buffer);
        try {
          const { invoke } = await import('@tauri-apps/api/core');
          await invoke('save_asset', {
            path: `assets/${filename}`,
            content: Array.from(uint8Array)
          });
          const view = viewRef.current;
          if (view) {
            const pos = view.state.selection.main.head;
            const mdImage = `![${filename}](assets/${filename})`;
            view.dispatch({
              changes: { from: pos, insert: mdImage },
              selection: { anchor: pos + mdImage.length }
            });
          }
        } catch (e) {
          console.error('Failed to paste image:', e);
        }
        return;
      }
    }
  }, []);

  useEffect(() => {
    if (!isTauri || !currentNote?.path) return;
    let cancelled = false;
    (async () => {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        const result = await invoke<{ from: string }[]>("get_backlinks", {
          noteId: currentNote.path,
        });
        if (!cancelled) setBacklinksCount(result.length);
      } catch {
        if (!cancelled) setBacklinksCount(0);
      }
    })();
    return () => { cancelled = true; };
  }, [currentNote?.path, isTauri]);

  useEffect(() => {
    if (!viewRef.current) return;
    viewRef.current.dispatch({
      effects: lineWrapCompartment.reconfigure(
        wordWrap ? EditorView.lineWrapping : []
      ),
    });
  }, [wordWrap]);

  const handleGoToLine = useCallback((lineNum: number) => {
    const view = viewRef.current;
    if (!view) return;
    if (lineNum < 1 || lineNum > view.state.doc.lines) return;
    const line = view.state.doc.line(lineNum);
    view.dispatch({
      selection: { anchor: line.from, head: line.from },
      scrollIntoView: true,
    });
    view.focus();
  }, []);

  useEffect(() => {
    _goToLineHandler = handleGoToLine;
    return () => { _goToLineHandler = null; };
  }, [handleGoToLine]);

  useEffect(() => {
    if (!hasOpenNote || viewMode !== "edit" || !editorRef.current) return;

    const editorView = new EditorView({
      doc: currentNoteRef.current?.content ?? "",
      extensions: [
        basicSetup,
        autocompletion({
          override: [slashCommandSource, wikiLinkSource, headingSource, tagSource],
          activateOnTyping: true,
          defaultKeymap: true,
        }),
        closeBrackets(),
        keymap.of([...closeBracketsKeymap]),
        listContinuationKeymap,
        markdown(),
        tagHighlighter,
        EditorState.readOnly.of(isReadOnlyNote),
        EditorView.editable.of(!isReadOnlyNote),
        lineWrapCompartment.of([]),
        keymap.of([
          {
            key: "Ctrl-Shift-Enter",
            run: (view) => {
              const selection = view.state.selection;
              const note = currentNoteRef.current;
              if (!selection.main.empty && note) {
                const text = selection.ranges
                  .map((range) => view.state.sliceDoc(range.from, range.to))
                  .join("\n");
                emitSelectionSubmit(
                  note.path,
                  text,
                  selection.main.from,
                  selection.main.to,
                  isTauriRef.current
                );
                showToast("info", "Selection snapshot captured");
                return true;
              }
              return false;
            },
          },
        ]),
        keymap.of([...searchKeymap]),
        EditorView.domEventHandlers({
          click(event, view) {
            if (!isTauriRef.current) return false;

            if (!event.ctrlKey && !event.metaKey) return false;

            const pos = view.posAtCoords({ x: event.clientX, y: event.clientY });
            if (pos === null) return false;

            const line = view.state.doc.lineAt(pos);
            const lineText = line.text;

            const wikiLinkRegex = /\[\[([^\]]+)\]\]/g;
            let match;
            while ((match = wikiLinkRegex.exec(lineText)) !== null) {
              const linkStart = line.from + match.index;
              const linkEnd = linkStart + match[0].length;

              if (pos >= linkStart && pos <= linkEnd) {
                const linkTarget = match[1];

                const notes = useAppStore.getState().notes;
                const exists = notes.some((n) => {
                  const noteBaseName = n.path.split('/').pop()?.replace(/\.md$/, '') || '';
                  return noteBaseName.toLowerCase() === linkTarget.toLowerCase();
                });

                if (exists) {
                  return false;
                }

                event.preventDefault();
                event.stopPropagation();

                const generatedPath = linkTarget.endsWith('.md')
                  ? linkTarget
                  : linkTarget + '.md';

                (async () => {
                  try {
                    const { invoke } = await import('@tauri-apps/api/core');
                    await invoke('create_note', { path: generatedPath });
                    const content = await invoke<string>('read_note', { path: generatedPath });
                    useAppStore.getState().openNote(generatedPath, content);
                  } catch (err) {
                    console.error('Failed to create note from link:', err);
                  }
                })();

                return true;
              }
            }

            return false;
          },
          contextmenu(event, view) {
            return handleEditorContextMenu(event, view);
          },
          mouseup(_event, view) {
            if (!isTauriRef.current) return false;
            setTimeout(() => {
              const sel = view.state.selection.main;
              const selText = sel.empty ? "" : view.state.sliceDoc(sel.from, sel.to);
              if (selText.length > 0) {
                const coords = view.coordsAtPos(sel.head);
                if (coords) {
                  setSelToolbar({
                    x: coords.left + (coords.right - coords.left) / 2,
                    y: coords.bottom,
                    text: selText,
                  });
                }
              } else {
                setSelToolbar(null);
              }
            }, 0);
            return false;
          },
        }),
        EditorView.updateListener.of((update) => {
          if (update.docChanged && !isReadOnlyNote) {
            if (suppressChangeRef.current) {
              suppressChangeRef.current = false;
            } else {
              if (externalNoteModeRef.current) {
                setTabContent(currentNoteRef.current?.path ?? "", update.state.doc.toString());
              } else {
                setCurrentContent(update.state.doc.toString());
              }
            }
          }
          if (update.selectionSet || update.docChanged) {
            const pos = update.state.selection.main.head;
            const line = update.state.doc.lineAt(pos);
            setCursorPos(line.number, pos - line.from + 1);
          }
        }),
        EditorView.theme({
          "&": { height: "100%", fontSize: `${settings.editorFontSize}px`, lineHeight: "1.7" },
          ".cm-content": {
            fontFamily: "'JetBrains Mono', 'Fira Code', monospace",
            padding: "0",
          },
          ".cm-gutters": { display: "none" },
          ".cm-activeLine": { backgroundColor: "var(--bg-input)44" },
          ".cm-selectionBackground": { backgroundColor: "var(--border-color)88" },
          ".cm-tag": { color: "var(--accent-primary)", fontWeight: "500" },
        }),
      ],
      parent: editorRef.current,
    });

    viewRef.current = editorView;

    const pasteHandler = handlePaste as unknown as EventListener;
    editorView.dom.addEventListener('paste', pasteHandler);

    return () => {
      editorView.dom.removeEventListener('paste', pasteHandler);
      editorView.destroy();
      viewRef.current = null;
    };
  }, [handleEditorContextMenu, hasOpenNote, isReadOnlyNote, setCurrentContent, setCursorPos, settings.editorFontSize, viewMode, setTabContent]);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key === "s") {
        e.preventDefault();
        handleSaveRef.current?.();
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, []);

  useEffect(() => {
    if (!viewRef.current || !currentNote) return;

    const currentDoc = viewRef.current.state.doc.toString();
    if (currentDoc !== currentNote.content) {
      suppressChangeRef.current = true;
      viewRef.current.dispatch({
        changes: {
          from: 0,
          to: currentDoc.length,
          insert: currentNote.content,
        },
      });
    }

    emitNoteOpened(currentNote.path, isTauri);
  }, [currentNote, isTauri]);

  useEffect(() => {
    if (!settings.autoSaveInterval || settings.autoSaveInterval < 5) return;
    if (isReadOnlyNote) return;
    if (!isTauri || !currentNote || !isDirty) return;
    const timer = window.setTimeout(() => {
      handleSaveRef.current?.();
    }, settings.autoSaveInterval * 1000);
    return () => window.clearTimeout(timer);
  }, [currentNote, isDirty, isReadOnlyNote, isTauri, settings.autoSaveInterval]);

  const content = currentNote?.content ?? "";

  useEffect(() => {
    if (viewMode !== "preview") return;
    const previewEl = document.querySelector(".feroha-markdown-preview");
    if (!previewEl) return;
    const codeBlocks = previewEl.querySelectorAll("pre");
    codeBlocks.forEach((pre) => {
      if (pre.querySelector(".code-copy-btn")) return;
      (pre as HTMLElement).style.position = "relative";
      const btn = document.createElement("button");
      btn.className = "code-copy-btn";
      btn.textContent = "Copy";
      btn.onclick = async () => {
        const code = pre.querySelector("code")?.textContent || "";
        await navigator.clipboard.writeText(code);
        btn.textContent = "Copied!";
        setTimeout(() => { btn.textContent = "Copy"; }, 2000);
      };
      pre.appendChild(btn);
    });
  }, [content, viewMode]);
  const wordCount = countWords(content);
  const charCount = content.length;
  const headings = getHeadings(content);

  const handleOutlineScroll = (pos: number) => {
    if (!viewRef.current) return;
    const view = viewRef.current;
    view.dispatch({
      selection: { anchor: pos, head: pos },
      scrollIntoView: true,
    });
    view.focus();
  };

  const handlePreviewClick = useCallback(
    (e: React.MouseEvent<HTMLDivElement>) => {
      const target = e.target as HTMLElement;
      if (target.tagName !== "A") return;
      const href = target.getAttribute("href");
      if (!href) return;
      const wikiMatch = href.match(/^#\/wiki\/(.+)$/);
      if (!wikiMatch) return;
      e.preventDefault();
      const linkTarget = wikiMatch[1];
      useAppStore.getState().openNote(linkTarget, "");
    },
    []
  );

  const selectionContextMenuItems: ContextMenuItem[] = selectionContextMenu
    ? [
        {
          id: "submit-selection-task",
          label: "提交给 AI",
          icon: "Send",
          shortcut: "Ctrl+Shift+Enter",
          onSelect: () => {
            void submitSelectionTaskToAi("submit", selectionContextMenu.text);
          },
        },
        {
          id: "analyze-selection",
          label: "分析选区",
          icon: "FileSearch",
          onSelect: () => {
            void submitSelectionTaskToAi("analyze", selectionContextMenu.text);
          },
        },
        {
          id: "rewrite-selection",
          label: "改写选区",
          icon: "PenLine",
          onSelect: () => {
            void submitSelectionTaskToAi("rewrite", selectionContextMenu.text);
          },
        },
        { id: "selection-tools-separator", type: "separator" },
        {
          id: "copy-selection",
          label: "复制选区",
          icon: "Copy",
          shortcut: "Ctrl+C",
          onSelect: () => {
            void navigator.clipboard?.writeText(selectionContextMenu.text);
            showToast("info", "已复制选区");
          },
        },
      ]
    : [];

  if (!hasOpenNote) {
    return (
      <div style={styles.wrapper}>
        <div style={styles.emptyState} data-testid="editor-empty-state">
          <div style={styles.emptyIcon}>
            <FeroHaIcon name={readOnly ? "BookOpen" : "FilePlus"} size={28} />
          </div>
          <div style={styles.emptyTitle}>
            {readOnly ? readOnlyBadgeLabel : "还没有打开笔记"}
          </div>
          <div style={styles.emptyBody}>
            {readOnly
              ? "AI 面当前只读。切回人类面后，可以新建或编辑文档。"
              : vaultPath || !isTauri
                ? "从左侧笔记库选择一篇笔记，或新建文档开始记录。"
                : "先打开一个笔记库，然后新建文档开始记录。"}
          </div>
          {!readOnly && (
            <button
              type="button"
              style={{
                ...styles.emptyPrimaryBtn,
                ...(!canCreateNoteFromEmptyState ? styles.emptyPrimaryBtnDisabled : {}),
              }}
              onClick={handleEmptyCreateClick}
              disabled={!canCreateNoteFromEmptyState}
            >
              <FeroHaIcon name="FilePlus" size={14} />
              新建文档
            </button>
          )}
        </div>
      </div>
    );
  }

  return (
    <div style={styles.wrapper}>
      <div style={styles.toolbar}>
        <div style={styles.fileInfo}>
          <span style={styles.fileName}>
            {currentNote?.path ?? ""}
          </span>
          {isDirty && <span style={styles.dirtyMark}>*</span>}
          {isReadOnlyNote && <span style={styles.readOnlyBadge}>{readOnlyBadgeLabel}</span>}
        </div>
        <div style={styles.toolbarRight}>
          <button
            style={{
              ...styles.outlineBtn,
              ...(outlineVisible ? styles.outlineBtnActive : {}),
            }}
            onClick={() => setOutlineVisible(!outlineVisible)}
            title="切换大纲"
            aria-label="切换大纲"
          >
            <FeroHaIcon name="ListTree" size={14} />
          </button>
          <span style={styles.wordCount}>
            {charCount.toLocaleString()} 字符 · {wordCount} 词
          </span>
          <button
            style={{
              ...styles.outlineBtn,
              ...(showMeta ? styles.outlineBtnActive : {}),
            }}
            onClick={() => setShowMeta(!showMeta)}
            title="笔记元数据"
            aria-label="切换笔记元数据"
          >
            <FeroHaIcon name="Info" size={14} />
          </button>
          <button
            style={{
              ...styles.saveBtn,
              ...(saveStatus === "saving" ? styles.saveBtnLoading : {}),
              ...(saveStatus === "success" ? styles.saveBtnSuccess : {}),
              ...(saveStatus === "error" ? styles.saveBtnError : {}),
            }}
            onClick={handleSave}
            disabled={saveStatus === "saving" || isReadOnlyNote}
            aria-label={isReadOnlyNote ? "只读" : undefined}
          >
            {isReadOnlyNote
              ? "只读"
              : saveStatus === "saving"
              ? "保存中"
              : saveStatus === "success"
                ? "已保存"
                : saveStatus === "error"
                  ? "错误"
                  : "保存"}
          </button>
        </div>
      </div>
      <EditorToolbar
        viewRef={viewRef}
        viewMode={viewMode}
        onToggleViewMode={() => setViewMode(viewMode === "edit" ? "preview" : "edit")}
        onToggleLineWrap={() => setWordWrap(!wordWrap)}
        lineWrapActive={wordWrap}
        readOnly={isReadOnlyNote}
      />
      {showMeta && currentNote && (
        <MetaBar
          content={content}
          currentNotePath={currentNote.path}
          allNotes={allNotes}
          backlinksCount={backlinksCount}
        />
      )}
      <div style={styles.editorWrapper}>
        {viewMode === "preview" ? (
          <div
            className="feroha-markdown-preview"
            onClick={handlePreviewClick}
            dangerouslySetInnerHTML={{ __html: renderMarkdown(content) }}
          />
        ) : (
          <div
            ref={editorRef}
            style={styles.editor}
            onClick={(e) => {
              if (isReadOnlyNote) return;
              const view = viewRef.current;
              if (!view) return;
              const pos = view.posAtCoords({ x: e.clientX, y: e.clientY });
              if (pos === null) return;
              const line = view.state.doc.lineAt(pos);
              const text = line.text;
              const checkboxMatch = text.match(/^(\s*)-\s*\[([ x])\]\s/);
              if (checkboxMatch) {
                const checked = checkboxMatch[2] === "x";
                const newMark = checked ? " " : "x";
                const newText = text.replace(/^(\s*-)\s*\[[ x]\]/, `$1 [${newMark}]`);
                view.dispatch({
                  changes: { from: line.from, to: line.to, insert: newText },
                });
              }
            }}
          />
        )}
        {viewMode === "edit" && outlineVisible && headings.length > 0 && (
          <div style={styles.outlinePanel}>
            <div style={styles.outlineTitle}>Outline</div>
            {headings.map((h, i) => (
              <div
                key={i}
                style={{
                  ...styles.outlineItem,
                  paddingLeft: h.level * 8,
                  color:
                    h.level === 1
                      ? "var(--text-primary)"
                      : h.level === 2
                        ? "var(--text-secondary)"
                        : "var(--text-muted)",
                  fontSize: h.level <= 2 ? 12 : 11,
                }}
                onClick={() => handleOutlineScroll(h.pos)}
              >
                {h.text}
              </div>
            ))}
          </div>
        )}
        {selectionContextMenu && (
          <div data-testid="selection-task-context-menu">
            <ContextMenu
              point={{ x: selectionContextMenu.x, y: selectionContextMenu.y }}
              items={selectionContextMenuItems}
              onClose={() => setSelectionContextMenu(null)}
            />
          </div>
        )}
        <SelectionToolbar
          visible={selToolbar !== null}
          x={selToolbar?.x ?? 0}
          y={selToolbar?.y ?? 0}
          onAction={async (action) => {
            if (!selToolbar) return;
            await submitSelectionTaskToAi(action, selToolbar.text);
            setSelToolbar(null);
          }}
          onDismiss={() => setSelToolbar(null)}
        />
      </div>
    </div>
  );
}

interface MetaBarProps {
  content: string;
  currentNotePath: string;
  allNotes: NoteMeta[];
  backlinksCount: number;
}

function MetaBar({ content, currentNotePath, allNotes, backlinksCount }: MetaBarProps) {
  const wordCount = countWords(content);
  const charCount = content.length;
  const outgoingLinkCount = (content.match(/\[\[([^\]|#]+)/g) || []).length;
  const currentNoteMeta = allNotes.find((n) => n.path === currentNotePath);
  const tags = currentNoteMeta?.tags || [];
  const modifiedDate = currentNoteMeta?.modified
    ? new Date(currentNoteMeta.modified).toLocaleString()
    : null;

  return (
    <div style={metaStyles.bar}>
      <div style={metaStyles.group}>
        <span style={metaStyles.label}>词数：</span>
        <span style={metaStyles.value}>{wordCount}</span>
      </div>
      <div style={metaStyles.separator} />
      <div style={metaStyles.group}>
        <span style={metaStyles.label}>字符：</span>
        <span style={metaStyles.value}>{charCount}</span>
      </div>
      <div style={metaStyles.separator} />
      <div style={metaStyles.group}>
        <span style={metaStyles.label}>标签：</span>
        <span style={metaStyles.value}>
          {tags.length > 0 ? tags.join(", ") : <em style={{ color: "var(--text-muted)" }}>无</em>}
        </span>
      </div>
      {modifiedDate && (
        <>
          <div style={metaStyles.separator} />
          <div style={metaStyles.group}>
            <span style={metaStyles.label}>修改：</span>
            <span style={metaStyles.value}>{modifiedDate}</span>
          </div>
        </>
      )}
      <div style={metaStyles.spacer} />
      <div style={metaStyles.group}>
        <span style={metaStyles.label}>链接：</span>
        <span style={metaStyles.value}>
          入链：{backlinksCount} · 出链：{outgoingLinkCount}
        </span>
      </div>
    </div>
  );
}

const metaStyles: Record<string, React.CSSProperties> = {
  bar: {
    display: "flex",
    alignItems: "center",
    gap: "8px",
    padding: "4px 8px",
    backgroundColor: "var(--bg-secondary)",
    border: "1px solid var(--border-color)",
    borderRadius: "4px",
    marginBottom: "6px",
    fontSize: "11px",
    flexWrap: "wrap",
    minHeight: "24px",
  },
  group: {
    display: "flex",
    alignItems: "center",
    gap: "4px",
    whiteSpace: "nowrap",
  },
  label: {
    color: "var(--text-muted)",
    fontWeight: 600,
  },
  value: {
    color: "var(--text-secondary)",
  },
  separator: {
    width: "1px",
    height: "14px",
    backgroundColor: "var(--border-color)",
    flexShrink: 0,
  },
  spacer: {
    flex: 1,
  },
};

const styles: Record<string, React.CSSProperties> = {
  wrapper: {
    display: "flex",
    flexDirection: "column",
    height: "100%",
  },
  emptyState: {
    flex: 1,
    minHeight: 0,
    display: "flex",
    flexDirection: "column",
    alignItems: "center",
    justifyContent: "center",
    gap: "12px",
    textAlign: "center",
    color: "var(--text-secondary)",
    border: "1px dashed var(--border-muted)",
    borderRadius: "8px",
    backgroundColor: "var(--bg-secondary)",
    padding: "32px",
  },
  emptyIcon: {
    display: "inline-flex",
    alignItems: "center",
    justifyContent: "center",
    width: "48px",
    height: "48px",
    borderRadius: "8px",
    color: "var(--text-muted)",
    backgroundColor: "var(--bg-input)",
  },
  emptyTitle: {
    color: "var(--text-primary)",
    fontSize: "15px",
    fontWeight: 600,
  },
  emptyBody: {
    maxWidth: "360px",
    color: "var(--text-muted)",
    fontSize: "13px",
    lineHeight: 1.6,
  },
  emptyPrimaryBtn: {
    display: "inline-flex",
    alignItems: "center",
    justifyContent: "center",
    gap: "6px",
    minHeight: "32px",
    padding: "0 12px",
    border: "1px solid var(--control-border-strong)",
    borderRadius: "4px",
    backgroundColor: "var(--bg-input)",
    color: "var(--text-primary)",
    cursor: "pointer",
    fontSize: "13px",
  },
  emptyPrimaryBtnDisabled: {
    opacity: 0.55,
    cursor: "not-allowed",
  },
  toolbar: {
    display: "flex",
    justifyContent: "space-between",
    alignItems: "center",
    padding: "6px 0",
    borderBottom: "1px solid var(--border-color)",
    marginBottom: "12px",
    fontSize: "12px",
  },
  fileInfo: {
    display: "flex",
    alignItems: "center",
    gap: "6px",
  },
  fileName: {
    color: "var(--text-primary)",
    fontWeight: 600,
  },
  dirtyMark: {
    color: "var(--diff-warn)",
    fontSize: "14px",
  },
  readOnlyBadge: {
    padding: "2px 6px",
    border: "1px solid var(--border-color)",
    borderRadius: "999px",
    color: "var(--text-muted)",
    backgroundColor: "var(--bg-input)",
    fontSize: "11px",
    lineHeight: 1,
  },
  toolbarRight: {
    display: "flex",
    alignItems: "center",
    gap: "10px",
  },
  outlineBtn: {
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    width: 24,
    height: 24,
    backgroundColor: "transparent",
    border: "1px solid transparent",
    borderRadius: "4px",
    cursor: "pointer",
    padding: 0,
  },
  outlineBtnActive: {
    border: "1px solid var(--border-color)",
    backgroundColor: "var(--bg-input)",
  },
  wordCount: {
    color: "var(--text-muted)",
  },
  saveBtn: {
    padding: "4px 12px",
    backgroundColor: "var(--bg-input)",
    color: "var(--text-primary)",
    border: "1px solid var(--border-color)",
    borderRadius: "4px",
    cursor: "pointer",
    fontSize: "12px",
    minWidth: "68px",
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    transition: "all 0.2s ease",
  },
  saveBtnLoading: {
    backgroundColor: "var(--border-color)",
    color: "var(--text-muted)",
    cursor: "not-allowed",
  },
  saveBtnSuccess: {
    backgroundColor: "#a6e3a1",
    color: "#1e1e2e",
    borderColor: "#a6e3a1",
  },
  saveBtnError: {
    backgroundColor: "#f38ba8",
    color: "#1e1e2e",
    borderColor: "#f38ba8",
  },
  editorWrapper: {
    position: "relative",
    flex: 1,
    overflow: "hidden",
  },
  editor: {
    height: "100%",
    overflow: "auto",
  },
  outlinePanel: {
    position: "absolute",
    right: 8,
    top: 0,
    width: 200,
    maxHeight: 300,
    backgroundColor: "var(--bg-secondary)",
    border: "1px solid var(--border-color)",
    borderRadius: 6,
    overflowY: "auto",
    zIndex: 10,
    padding: 8,
  },
  outlineTitle: {
    fontSize: 11,
    fontWeight: 600,
    color: "var(--text-muted)",
    textTransform: "uppercase",
    letterSpacing: "0.5px",
    marginBottom: 6,
  },
  outlineItem: {
    cursor: "pointer",
    padding: "2px 4px",
    borderRadius: 3,
    lineHeight: "1.5",
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
};
