import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { commandCardLibraryChromeClass } from "../CommandCardLibrary";

const pipelineEditor = readFileSync("src/components/PipelineEditor.tsx", "utf8");
const pipelineCss = readFileSync("src/components/PipelineEditor.css", "utf8");
const commandCardLibrary = readFileSync("src/components/CommandCardLibrary.tsx", "utf8");
const commandCardPreview = readFileSync("src/components/CommandCardPreview.tsx", "utf8");

describe("CommandCardLibrary chrome", () => {
  it("embeds the management library inside the AI panel", () => {
    expect(commandCardLibraryChromeClass("manage")).toBe("command-card-library embedded");
  });

  it("keeps browse and select uses as modal surfaces", () => {
    expect(commandCardLibraryChromeClass("browse")).toBe("command-card-library modal");
    expect(commandCardLibraryChromeClass("select")).toBe("command-card-library modal");
  });

  it("keeps shared themed search controls in the legacy pipeline editor", () => {
    expect(pipelineEditor).toContain('className="pipeline-card-search-input feroha-search"');
    expect(pipelineEditor).not.toContain('className="pipeline-property-row input"');
    expect(pipelineCss).toContain(".pipeline-card-search-input");
    expect(pipelineCss).toContain("var(--control-bg)");
    expect(pipelineCss).toContain("var(--control-border)");
    expect(pipelineCss).toContain("var(--control-shadow-focus)");
  });

  it("shows skill-wrapper capability state in the command card library", () => {
    expect(commandCardLibrary).toContain("CommandCardLibraryItem");
    const commandCardItem = readFileSync("src/components/CommandCardLibraryItem.tsx", "utf8");
    expect(commandCardItem).toContain("card-skill-line");
    expect(commandCardItem).toContain("skill.statusLabel");
  });

  it("opens a visible card preview when browsing cards instead of swallowing clicks", () => {
    expect(commandCardLibrary).toContain("selectedPreviewCard");
    expect(commandCardLibrary).toContain("CommandCardPreview");
    expect(commandCardPreview).toContain("command-card-preview");
    expect(commandCardLibrary).toContain("handleUsePreviewCard");
  });

  it("keeps individual command-card UI in a focused item component", () => {
    const commandCardItem = readFileSync("src/components/CommandCardLibraryItem.tsx", "utf8");

    expect(commandCardLibrary).toContain("CommandCardLibraryItem");
    expect(commandCardItem).toContain("commandCardSkillDescriptor");
    expect(commandCardLibrary).not.toContain("commandCardSkillDescriptor");
  });
});
