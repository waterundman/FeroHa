import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import QuickSwitcher from "../QuickSwitcher";

describe("QuickSwitcher", () => {
  it("uses localized chrome for the search entry point", () => {
    render(<QuickSwitcher isTauri={false} open onOpenChange={vi.fn()} />);

    expect(screen.getByText("快速切换")).toBeDefined();
    expect(screen.getByPlaceholderText("搜索笔记或创建新笔记...")).toBeDefined();
    expect(screen.getByText("输入关键词搜索或创建笔记")).toBeDefined();
  });
});
