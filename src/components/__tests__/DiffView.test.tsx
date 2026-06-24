import { render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import DiffView from "../DiffView";
import { useAppStore } from "../../hooks/useAppStore";

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
}));

describe("DiffView browser preview", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    useAppStore.setState({ diffBlocks: [] });
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("does not show fake pending edits when Tauri is unavailable", async () => {
    render(<DiffView isTauri={false} />);

    expect(await screen.findByText("浏览器预览无法读取真实差异")).toBeDefined();
    expect(screen.queryByText(/Rust's ownership system/)).toBeNull();
  });

  it("explains that Diff Review only handles concrete text blocks", async () => {
    render(<DiffView isTauri={false} />);

    expect(await screen.findByText(/Diff Review 只处理已经形成 ghost\/text block 的具体文本改动/)).toBeDefined();
  });

  it("keeps locally staged simulation diffs in browser preview", async () => {
    useAppStore.setState({
      diffBlocks: [
        {
          ghostId: "sim-task-good",
          id: "sim-task-good-diff",
          type: "inserted",
          newText: "browser preview keeps local simulation diff",
          accepted: false,
          rejected: false,
        },
      ],
    });

    render(<DiffView isTauri={false} />);

    expect(await screen.findByText(/browser preview keeps local simulation diff/)).toBeDefined();
    expect(screen.queryByText("浏览器预览无法读取真实差异")).toBeNull();
  });

  it("keeps locally staged simulation diffs when the Tauri diff store is unavailable", async () => {
    invokeMock.mockRejectedValueOnce(new Error("diff store not initialized"));
    useAppStore.setState({
      diffBlocks: [
        {
          ghostId: "sim-task-good",
          id: "sim-task-good-diff",
          type: "inserted",
          newText: "局部仿射坍缩模拟建议",
          accepted: false,
          rejected: false,
        },
      ],
    });

    render(<DiffView isTauri />);

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("get_diff_blocks");
    });

    expect(await screen.findByText(/局部仿射坍缩模拟建议/)).toBeDefined();
  });

  it("keeps locally staged simulation diffs when the Tauri diff store is empty", async () => {
    invokeMock.mockResolvedValueOnce([]);
    useAppStore.setState({
      diffBlocks: [
        {
          ghostId: "sim-task-good",
          id: "sim-task-good-diff",
          type: "inserted",
          newText: "empty backend should not erase local simulation diff",
          accepted: false,
          rejected: false,
        },
      ],
    });

    render(<DiffView isTauri />);

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("get_diff_blocks");
    });
    expect(screen.getByText(/empty backend should not erase local simulation diff/)).toBeDefined();
  });

  it("presents acceptance as feedback that does not modify human notes", async () => {
    useAppStore.setState({
      diffBlocks: [{
        ghostId: "ghost-feedback",
        id: "block-1",
        type: "inserted",
        newText: "AI suggestion",
        accepted: false,
        rejected: false,
      }],
    });

    render(<DiffView isTauri={false} />);

    expect(screen.getByText("采纳反馈")).toBeDefined();
    expect(screen.getByText(/不会修改人类笔记正文/)).toBeDefined();
  });
});
