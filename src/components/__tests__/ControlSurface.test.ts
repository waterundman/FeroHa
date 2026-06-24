import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const theme = readFileSync("src/styles/feroha-theme.css", "utf8");
const appSource = readFileSync("src/App.tsx", "utf8");

describe("shared control styling contract", () => {
  it("defines integrated control and resize tokens", () => {
    for (const token of [
      "--control-bg",
      "--control-bg-hover",
      "--control-border",
      "--control-border-strong",
      "--control-placeholder",
      "--control-shadow-focus",
      "--resize-handle-bg",
      "--resize-handle-active",
      "--resize-handle-grip",
      "--scrollbar-thumb",
      "--scrollbar-thumb-hover",
    ]) {
      expect(theme).toContain(token);
    }
  });

  it("defines reusable control classes", () => {
    for (const className of [
      ".feroha-control",
      ".feroha-input",
      ".feroha-select",
      ".feroha-textarea",
      ".feroha-search",
      ".feroha-resize-handle",
    ]) {
      expect(theme).toContain(className);
    }
  });

  it("themes all app resize rails instead of leaving transparent system gaps", () => {
    expect(appSource).toContain("app-resize-separator feroha-resize-handle");
    expect(appSource).toContain("editor-split-divider feroha-resize-handle");
    expect(appSource).toMatch(/resizeHandle:\s*{[^}]*background:\s*"var\(--resize-handle-bg\)"/s);
    expect(appSource).toMatch(/resizeHandle:\s*{[^}]*cursor:\s*"col-resize"/s);
    expect(appSource).not.toMatch(/resizeHandle:\s*{[^}]*background:\s*"transparent"/s);
    expect(theme).toContain(".app-resize-separator[data-separator]::before");
    expect(theme).toContain("textarea::-webkit-resizer");
    expect(theme).toContain("*::-webkit-scrollbar-thumb");
  });
});
