// Test: Editor component
import { describe, it, expect, vi, beforeEach } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { readFileSync } from "node:fs";
import { EditorView } from "codemirror";
import Editor from "../Editor";
import { useAppStore } from "../../hooks/useAppStore";

const editorSource = readFileSync("src/components/Editor.tsx", "utf8");

vi.mock("codemirror", () => ({
  basicSetup: [],
  EditorView: Object.assign(
    vi.fn().mockImplementation(() => ({
      state: { doc: { toString: () => "" } },
      destroy: vi.fn(),
      dispatch: vi.fn(),
      dom: document.createElement("div"),
    })),
    {
      updateListener: { of: vi.fn().mockReturnValue([]) },
      theme: vi.fn().mockReturnValue([]),
      domEventHandlers: vi.fn().mockReturnValue([]),
      editable: { of: vi.fn().mockReturnValue([]) },
    }
  ),
}));

vi.mock("@codemirror/lang-markdown", () => ({
  markdown: vi.fn().mockReturnValue([]),
}));

vi.mock("@codemirror/view", () => ({
  EditorView: Object.assign(
    vi.fn().mockImplementation(() => ({
      state: { doc: { toString: () => "" } },
      destroy: vi.fn(),
      dispatch: vi.fn(),
      dom: document.createElement("div"),
    })),
    {
      updateListener: { of: vi.fn().mockReturnValue([]) },
      theme: vi.fn().mockReturnValue([]),
      domEventHandlers: vi.fn().mockReturnValue([]),
    }
  ),
  ViewPlugin: {
    fromClass: vi.fn().mockReturnValue([]),
  },
  Decoration: {
    mark: vi.fn().mockReturnValue({}),
  },
  DecorationSet: {} as any,
  ViewUpdate: {} as any,
  keymap: { of: vi.fn().mockReturnValue([]) },
}));

vi.mock("@codemirror/state", () => ({
  RangeSetBuilder: vi.fn().mockImplementation(() => ({
    add: vi.fn(),
    finish: vi.fn().mockReturnValue({}),
  })),
  Compartment: vi.fn().mockImplementation(() => ({
    of: vi.fn().mockReturnValue([]),
    reconfigure: vi.fn().mockReturnValue({}),
  })),
  EditorState: {
    readOnly: { of: vi.fn().mockReturnValue([]) },
  },
}));

describe("Editor", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useAppStore.setState({
      vaultPath: "/test-vault",
      currentNote: null,
      isDirty: false,
      tabs: [],
      activeTabIndex: -1,
    });
  });

  it("shows a new document prompt instead of an untitled draft when no note is open", () => {
    render(<Editor isTauri={false} onCreateNote={vi.fn()} />);

    expect(screen.getByText("新建文档")).toBeDefined();
    expect(screen.getByText("还没有打开笔记")).toBeDefined();
    expect(screen.queryByText("untitled.md")).toBeNull();
    expect(EditorView).not.toHaveBeenCalled();
  });

  it("routes the empty editor create action to the shared new-note flow", () => {
    const onCreateNote = vi.fn();
    render(<Editor isTauri={false} onCreateNote={onCreateNote} />);

    fireEvent.click(screen.getByRole("button", { name: "新建文档" }));

    expect(onCreateNote).toHaveBeenCalledTimes(1);
  });

  it("renders word count display", () => {
    render(<Editor isTauri={false} note={{ path: "Human.md", content: "# Human", isDirty: false }} />);
    expect(screen.getByText(/词/)).toBeDefined();
  });

  it("renders save button", () => {
    render(<Editor isTauri={false} note={{ path: "Human.md", content: "# Human", isDirty: false }} />);
    expect(screen.getByText("保存")).toBeDefined();
  });

  it("does not expose a save action when Tauri has no open note", () => {
    render(<Editor isTauri onCreateNote={vi.fn()} />);

    const realCreateElement = document.createElement.bind(document);
    const anchorClick = vi.fn();
    const originalCreateObjectURL = URL.createObjectURL;
    const originalRevokeObjectURL = URL.revokeObjectURL;
    Object.defineProperty(URL, "createObjectURL", {
      configurable: true,
      value: vi.fn(() => "blob:test-download"),
    });
    Object.defineProperty(URL, "revokeObjectURL", {
      configurable: true,
      value: vi.fn(),
    });
    const createElementSpy = vi
      .spyOn(document, "createElement")
      .mockImplementation((tagName, options) => {
        const element = realCreateElement(tagName, options);
        if (tagName.toLowerCase() === "a") {
          element.click = anchorClick;
        }
        return element;
      });

    expect(screen.queryByText("保存")).toBeNull();

    expect(anchorClick).not.toHaveBeenCalled();
    createElementSpy.mockRestore();
    Object.defineProperty(URL, "createObjectURL", {
      configurable: true,
      value: originalCreateObjectURL,
    });
    Object.defineProperty(URL, "revokeObjectURL", {
      configurable: true,
      value: originalRevokeObjectURL,
    });
  });

  it("routes selection AI actions through the shared task dispatch IPC", () => {
    expect(editorSource).toContain("sendTaskToAgent");
    expect(editorSource).not.toContain('"submit_task"');
    expect(editorSource).toContain('contextNote: currentNote?.path ?? null');
    expect(editorSource).toContain('scope: "selected_text"');
    expect(editorSource).toContain('source: "human_editor_context_menu"');
    expect(editorSource).toContain('"read_only_auto_queue"');
    expect(editorSource).toContain('"draft_only"');
    expect(editorSource).toContain("expectedOutput:");
  });

  it("keeps the editor instance stable when a writable tab content update re-renders the note prop", () => {
    const initialNote = { path: "Human Note.md", content: "# Human Note", isDirty: false };
    const { rerender } = render(<Editor isTauri={false} note={initialNote} />);
    expect(EditorView).toHaveBeenCalledTimes(1);

    rerender(
      <Editor
        isTauri={false}
        note={{ path: "Human Note.md", content: "# Human Note\n\nnew text", isDirty: true }}
      />,
    );

    expect(EditorView).toHaveBeenCalledTimes(1);
  });

  it("captures a selection snapshot before dispatching selection AI work", () => {
    expect(editorSource).toContain("captureSelectionSnapshot");
    expect(editorSource).toContain("await emitSelectionSubmit(notePath, text, start, end, isTauri)");
    expect(editorSource).toContain("await captureSelectionSnapshot(trimmed)");
  });

  it("exposes the human text-selection task flow through a right-click context menu", () => {
    expect(editorSource).toContain("ContextMenu");
    expect(editorSource).toContain("selection-task-context-menu");
    expect(editorSource).toContain("handleEditorContextMenu");
    expect(editorSource).toContain("submitSelectionTaskToAi");
  });

  it("opens AI workspace files with a read-only editor extension", () => {
    expect(editorSource).toContain("isReadOnlyNote");
    expect(editorSource).toContain("EditorView.editable.of(!isReadOnlyNote)");
    expect(editorSource).toContain("AI 工作区只读");
  });

  it("can make a normal note read-only when it is opened from the AI face", () => {
    render(
      <Editor
        isTauri={false}
        readOnly
        readOnlyLabel="AI 面只读"
        note={{ path: "Human Note.md", content: "# Human Note", isDirty: false }}
      />,
    );

    expect(screen.getByText("AI 面只读")).toBeDefined();
    expect(screen.getByRole("button", { name: "只读" })).toHaveAttribute("disabled");
    expect(editorSource).toContain("readOnly={isReadOnlyNote}");
  });
});
