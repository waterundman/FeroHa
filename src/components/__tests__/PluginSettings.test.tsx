import { beforeEach, describe, it, expect, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import PluginSettings from "../PluginSettings";

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
}));

describe("PluginSettings", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("shows backend plugin status instead of a placeholder", async () => {
    invokeMock.mockResolvedValueOnce({
      status: "ready",
      message: "Plugin manager initialized",
      available_plugins: 3,
      enabled_plugins: 1,
      plugins_dir: "D:/vault/.dualtrack/plugins",
    });

    render(<PluginSettings />);

    expect(await screen.findByText("插件运行状态")).toBeDefined();
    expect(screen.getByText("已接通")).toBeDefined();
    expect(screen.getByText("3")).toBeDefined();
    expect(screen.getByText("1")).toBeDefined();
    expect(screen.getByText("D:/vault/.dualtrack/plugins")).toBeDefined();
    expect(screen.queryByText("插件系统正在建设中")).toBeNull();
  });

  it("shows a browser preview state when Tauri plugin status is unavailable", async () => {
    invokeMock.mockRejectedValueOnce(new Error("not in tauri"));

    render(<PluginSettings />);

    await waitFor(() => expect(screen.getByText("浏览器预览")).toBeDefined());
    expect(screen.getByText("插件后端只在 Tauri 应用中可用")).toBeDefined();
    expect(screen.queryByText("not in tauri")).toBeNull();
  });
});
