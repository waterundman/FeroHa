import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import CliMiniWindow from "../CliMiniWindow";

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
}));

describe("CliMiniWindow", () => {
  it("uses browser preview output instead of exposing Tauri invoke errors", async () => {
    invokeMock.mockImplementation(() => {
      throw new TypeError("Cannot read properties of undefined (reading 'invoke')");
    });

    render(<CliMiniWindow vaultPath="" isTauri={false} />);

    fireEvent.click(screen.getByRole("button", { name: "打开 CLI 浮窗" }));

    const input = screen.getByPlaceholderText("/agent ...");
    fireEvent.change(input, { target: { value: "/agent 浏览器回归检查" } });
    fireEvent.keyDown(input, { key: "Enter", code: "Enter" });

    expect(await screen.findByText("浏览器预览")).toBeInTheDocument();
    expect(await screen.findByText(/CLI 命令已模拟执行/)).toBeInTheDocument();
    expect(invokeMock).not.toHaveBeenCalled();
    expect(
      screen.queryByText(/Cannot read properties of undefined/)
    ).not.toBeInTheDocument();
  });
});
