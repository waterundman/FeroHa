import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import AiTaskStrip, { aiTaskStripPreviewLabel } from "../AiTaskStrip";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

describe("AiTaskStrip", () => {
  it("keeps command-card task entry separate from terminal CLI input", () => {
    render(<AiTaskStrip isTauri={false} />);

    expect(screen.getByRole("button", { name: /打开指令卡/ })).toBeTruthy();
    expect(screen.queryByPlaceholderText(/agent/i)).toBeNull();
    expect(screen.queryByRole("button", { name: /切换到 CLI/ })).toBeNull();
  });

  it("opens command cards from the slash shortcut", () => {
    render(<AiTaskStrip isTauri={false} />);

    fireEvent.keyDown(window, { key: "/" });

    expect(screen.getByRole("dialog", { name: /指令卡/ })).toBeTruthy();
  });

  it("shows compact risk and write-policy preview", () => {
    expect(aiTaskStripPreviewLabel("auto", "research")).toContain("自动");
    render(<AiTaskStrip isTauri={false} />);

    expect(screen.getByLabelText("AI 任务类型")).toBeTruthy();
    const status = screen.getByRole("status", { name: "任务调度策略" });
    expect(status.textContent).toMatch(/Bridge/);
    expect(status.textContent).toMatch(/研究简报/);
    expect(screen.queryByText(/写权：/)).toBeNull();
    expect(screen.queryByRole("option", { name: /Legacy MDT/ })).toBeNull();
  });
});
