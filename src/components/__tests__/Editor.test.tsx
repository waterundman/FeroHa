// Test: Editor component
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import Editor from "../Editor";

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
}));

describe("Editor", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders with default filename", () => {
    render(<Editor isTauri={false} />);
    expect(screen.getByText("untitled.md")).toBeDefined();
  });

  it("renders word count display", () => {
    render(<Editor isTauri={false} />);
    expect(screen.getByText(/words/)).toBeDefined();
  });

  it("renders save button", () => {
    render(<Editor isTauri={false} />);
    expect(screen.getByText("Save")).toBeDefined();
  });
});
