import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import ContextMenu from "../ContextMenu";

describe("ContextMenu", () => {
  it("renders menu items at the requested point", () => {
    render(
      <ContextMenu
        point={{ x: 20, y: 30 }}
        items={[{ id: "ask-ai", label: "以此向 AI 提任务", icon: "Send", onSelect: vi.fn() }]}
        onClose={vi.fn()}
      />,
    );

    expect(screen.getByRole("menu")).toBeTruthy();
    expect(screen.getByRole("menuitem", { name: /以此向 AI 提任务/ })).toBeTruthy();
  });

  it("closes on Escape", () => {
    const onClose = vi.fn();
    render(<ContextMenu point={{ x: 1, y: 1 }} items={[]} onClose={onClose} />);

    fireEvent.keyDown(document, { key: "Escape" });

    expect(onClose).toHaveBeenCalled();
  });

  it("renders separators, shortcuts, and destructive menu states", () => {
    render(
      <ContextMenu
        point={{ x: 4, y: 8 }}
        items={[
          { id: "copy", label: "Copy", icon: "Copy", shortcut: "Ctrl+C", onSelect: vi.fn() },
          { id: "split", type: "separator" },
          { id: "delete", label: "Delete", icon: "Trash2", variant: "danger", onSelect: vi.fn() },
        ]}
        onClose={vi.fn()}
      />,
    );

    expect(screen.getByText("Ctrl+C")).toBeTruthy();
    expect(screen.getByRole("separator")).toBeTruthy();
    expect(screen.getByRole("menuitem", { name: /Delete/ })).toHaveClass("is-danger");
  });
});
