import { describe, expect, it } from "vitest";
import {
  buildCommandCardDispatchPayload,
  buildSubmitTaskArgs,
  resolveTaskIntentSelectionForCard,
  resolveTaskIntentSelectionForCommand,
  taskIntentReviewInfo,
  taskIntentForCommandCard,
} from "../CliBar";
import type { LegacyCommandCardDefinition } from "../../types/command-card";

function card(overrides: Partial<LegacyCommandCardDefinition> = {}): LegacyCommandCardDefinition {
  const base: LegacyCommandCardDefinition = {
    id: "verify-card",
    type: "verify",
    category: "analysis",
    label: "Verify",
    description: "Check a claim",
    icon: "Shield",
    params: { topic: "Bayes" },
    promptTemplate: "Verify {{topic}}",
    version: "1.0.0",
    tags: [],
    isCustom: false,
  };
  return { ...base, ...overrides } as LegacyCommandCardDefinition;
}

describe("CliBar task intent payloads", () => {
  it("passes explicit task type to submit_task", () => {
    expect(buildSubmitTaskArgs("/agent search bayes", "verify")).toEqual({
      command: "/agent search bayes",
      taskType: "verify",
    });
  });

  it("maps command cards to v3 task intents", () => {
    expect(taskIntentForCommandCard(card({ type: "verify" }))).toBe("verify");
    expect(taskIntentForCommandCard(card({ type: "format" }))).toBe("write_proposal");
    expect(taskIntentForCommandCard(card({ type: "custom" }))).toBe("research");
  });

  it("uses automatic task intent selection for command cards unless overridden", () => {
    expect(resolveTaskIntentSelectionForCard("auto", card({ type: "verify" }))).toBe("verify");
    expect(resolveTaskIntentSelectionForCard("auto", card({ type: "format" }))).toBe("write_proposal");
    expect(resolveTaskIntentSelectionForCard("jsonld_index", card({ type: "verify" }))).toBe("jsonld_index");
  });

  it("infers typed CLI commands with JSON-LD as the AI-face memory default", () => {
    expect(resolveTaskIntentSelectionForCommand("auto", "/dream")).toBe("dream");
    expect(resolveTaskIntentSelectionForCommand("auto", "/verify Bayes")).toBe("verify");
    expect(resolveTaskIntentSelectionForCommand("auto", "/agent import web page")).toBe("external_import");
    expect(resolveTaskIntentSelectionForCommand("auto", "/agent search bayes")).toBe("research");
    expect(resolveTaskIntentSelectionForCommand("auto", "/agent mdt index memory")).toBe("jsonld_index");
    expect(resolveTaskIntentSelectionForCommand("code_assist", "/verify Bayes")).toBe("code_assist");
  });

  it("describes task intent risk and permissions before submission", () => {
    expect(taskIntentReviewInfo("external_import")).toMatchObject({
      label: "外部导入",
      risk: "高",
      writePolicy: "只写入导入暂存与 Bridge 提案",
    });
    expect(taskIntentReviewInfo("verify").tools).toContain("PropositionKernel");
    expect(taskIntentReviewInfo("mdt_index")).toMatchObject({
      label: "JSON-LD 索引（MDT 兼容）",
      writePolicy: "兼容入口；优先重建 JSON-LD 语义索引",
      expectedOutput: "JSON-LD graph artifacts",
    });
  });

  it("passes selected task type through dispatch_agent_task payload", () => {
    const payload = buildCommandCardDispatchPayload({
      card: card(),
      params: { topic: "Dream memory" },
      renderedPrompt: "Verify Dream memory",
      taskType: "jsonld_index",
      contextNote: "Notes/Dream.md",
      timestamp: 123,
    });

    expect(payload.task_type).toBe("jsonld_index");
    expect(payload.context_note).toBe("Notes/Dream.md");
    expect(payload.params.topic).toBe("Dream memory");
  });
});
