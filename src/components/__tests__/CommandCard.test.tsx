import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import CommandCard from "../CommandCard";
import type { LegacyCommandCardDefinition } from "../../types/command-card";
import { useSettingsStore } from "../../hooks/useSettings";
import { commandCardSkillDescriptor } from "../../lib/commandCardSkill";

describe("CommandCard", () => {
  it("uses localized category badges and custom labels", () => {
    render(
      <CommandCard
        card={card({ category: "content", isCustom: true })}
        isSelected={false}
        onClick={vi.fn()}
      />,
    );

    expect(screen.getByText("内容")).toBeDefined();
    expect(screen.getByText("自定义")).toBeDefined();
  });

  it("shows when a skill-wrapped card is waiting for an API key", () => {
    useSettingsStore.setState({
      settings: { ...useSettingsStore.getState().settings, llmProvider: "deepseek", llmApiKey: "" },
    });

    render(
      <CommandCard
        card={card({ type: "rewrite", category: "content" })}
        isSelected={false}
        onClick={vi.fn()}
      />,
    );

    expect(screen.getByText("等待 API")).toBeDefined();
  });

  it("describes command cards as skill wrappers with execution capabilities", () => {
    expect(commandCardSkillDescriptor(card({ type: "search", category: "content" }))).toMatchObject({
      executionPath: "local",
      status: "ready",
    });
    expect(
      commandCardSkillDescriptor(
        card({ id: "dream-cycle", type: "dream", category: "agent", promptTemplate: "/agent dream" }),
        { llmReady: true },
      ),
    ).toMatchObject({
      skillId: "skill:dream-cycle",
      executionPath: "orchestrator",
      status: "ready",
    });
  });
});

function card(overrides: Partial<LegacyCommandCardDefinition> = {}): LegacyCommandCardDefinition {
  return {
    id: "test",
    type: "search",
    category: "content",
    label: "搜索笔记",
    description: "按关键词检索笔记",
    icon: "Search",
    params: {},
    promptTemplate: "",
    version: "1.0.0",
    tags: ["搜索"],
    isCustom: false,
    ...overrides,
  };
}
