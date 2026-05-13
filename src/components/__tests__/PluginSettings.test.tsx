import { describe, it, expect } from "vitest";
import { render, screen, act } from "@testing-library/react";
import PluginSettings from "../PluginSettings";

describe("PluginSettings", () => {
  it("shows installed plugins", () => {
    render(<PluginSettings />);
    expect(screen.getByText("Search Enhancer")).toBeDefined();
    expect(screen.getByText("Export to PDF")).toBeDefined();
  });

  it("allows toggle enable/disable", () => {
    render(<PluginSettings />);
    expect(screen.getByText("Disable")).toBeDefined();
    expect(screen.getByText("Enable")).toBeDefined();
  });

  it("allows install from marketplace", () => {
    const { container } = render(<PluginSettings />);
    const btns = container.querySelectorAll("button");
    let marketplaceBtn: HTMLElement | null = null;
    btns.forEach((b) => {
      if (b.textContent?.includes("Marketplace")) marketplaceBtn = b;
    });
    expect(marketplaceBtn).not.toBeNull();
    act(() => {
      (marketplaceBtn as HTMLElement).click();
    });
    expect(screen.getByText(/Arxiv Agent/)).toBeDefined();
    expect(screen.getAllByText("Install").length).toBeGreaterThan(0);
  });

  it("allows uninstall", () => {
    render(<PluginSettings />);
    const uninstallBtns = screen.getAllByText("Uninstall");
    expect(uninstallBtns.length).toBeGreaterThan(0);
  });
});
