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
    })),
    {
      updateListener: { of: vi.fn().mockReturnValue([]) },
      theme: vi.fn().mockReturnValue([]),
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
    })),
    {
      updateListener: { of: vi.fn().mockReturnValue([]) },
      theme: vi.fn().mockReturnValue([]),
    }
  ),
  keymap: { of: vi.fn().mockReturnValue([]) },
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
