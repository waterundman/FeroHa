import type { CommandCategory, CommandType, LegacyCommandCardDefinition } from "../types/command-card";

export type CommandCardSkillStatus = "ready" | "needs-api";

export interface CommandCardSkillDescriptor {
  skillId: string;
  executionPath: "local" | "llm" | "orchestrator";
  capabilities: string[];
  requiresLlmApi: boolean;
  status: CommandCardSkillStatus;
  statusLabel: string;
}

type CardShape = Pick<LegacyCommandCardDefinition, "id" | "type" | "category" | "promptTemplate">;

export function commandCardRequiresLlm(type: CommandType): boolean {
  return !["search", "orchestrator-check"].includes(type);
}

export function commandCardExecutionPath(card: Pick<CardShape, "type" | "category" | "promptTemplate">): CommandCardSkillDescriptor["executionPath"] {
  if (!commandCardRequiresLlm(card.type)) return "local";
  if (card.category === "agent" || card.promptTemplate.trim().startsWith("/agent")) return "orchestrator";
  return "llm";
}

export function commandCardCapabilities(type: CommandType, category: CommandCategory): string[] {
  const shared = category === "agent" ? ["orchestrator"] : [];
  switch (type) {
    case "search":
    case "multi-search":
      return [...shared, "retrieval", "source-scan"];
    case "dream":
      return [...shared, "dream-cycle", "memory-consolidation"];
    case "orchestrator-check":
      return ["orchestrator-status", "runtime-health"];
    case "graph-analysis":
    case "connect":
    case "analyze":
      return [...shared, "knowledge-graph", "llm-reasoning"];
    case "rewrite":
    case "translate":
    case "expand":
    case "simplify":
    case "format":
    case "extract":
      return [...shared, "draft-proposal", "llm-transform"];
    case "summarize":
    case "review":
    case "verify":
    case "compare":
    case "question":
    case "suggest":
    case "research":
    case "deep-research":
    case "brainstorm":
    case "outline":
    case "organize":
    case "visualize":
    case "custom":
    default:
      return [...shared, "llm-reasoning"];
  }
}

export function commandCardSkillDescriptor(
  card: CardShape,
  runtime: { llmReady?: boolean } = {},
): CommandCardSkillDescriptor {
  const requiresLlmApi = commandCardRequiresLlm(card.type);
  const llmReady = Boolean(runtime.llmReady);
  const status: CommandCardSkillStatus = requiresLlmApi && !llmReady ? "needs-api" : "ready";

  return {
    skillId: `skill:${card.id}`,
    executionPath: commandCardExecutionPath(card),
    capabilities: commandCardCapabilities(card.type, card.category),
    requiresLlmApi,
    status,
    statusLabel: status === "ready" ? "能力就绪" : "等待 API",
  };
}
