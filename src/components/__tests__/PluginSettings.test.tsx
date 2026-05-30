import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import PluginSettings from "../PluginSettings";

describe("PluginSettings", () => {
  it("shows placeholder message", () => {
    render(<PluginSettings />);
    expect(screen.getByText("Plugin system is under development")).toBeDefined();
  });

  it("shows future release note", () => {
    render(<PluginSettings />);
    expect(screen.getByText(/Backend plugin infrastructure exists/)).toBeDefined();
  });
});
