import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import FeroHaIcon from "../FeroHaIcon";

describe("FeroHaIcon", () => {
  it("does not inject style text into icon-only buttons", () => {
    render(
      <button type="button" aria-label="设置">
        <FeroHaIcon name="Settings" size={16} />
      </button>,
    );

    const button = screen.getByRole("button", { name: "设置" });
    expect(button.textContent?.trim()).toBe("");
    expect(button.querySelector("style")).toBeNull();
  });

  it("lets navigation icons inherit button color and defines classic contrast variables", () => {
    const css = readFileSync(join(process.cwd(), "src/styles/feroha-theme.css"), "utf8");

    expect(css).toContain("stroke: currentColor");
    expect(css).toContain(':root[data-theme="classic"]');
    expect(css).toContain("--icon-default: #bac2de");
    expect(css).toContain("button .feroha-icon");
  });

  it("maps the legacy Edit icon name without warning", () => {
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});

    render(<FeroHaIcon name="Edit" size={16} />);

    expect(warn).not.toHaveBeenCalled();
    warn.mockRestore();
  });
});
