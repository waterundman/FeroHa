import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { panelTabsForMode } from "../../App";

const main = readFileSync("src-tauri/src/main.rs", "utf8");
const app = readFileSync("src/App.tsx", "utf8");
const store = readFileSync("src/hooks/useAppStore.ts", "utf8");
const agentDashboard = readFileSync("src/components/AgentDashboard.tsx", "utf8");
const bridgeInbox = readFileSync("src/components/BridgeInbox.tsx", "utf8");
const diffView = readFileSync("src/components/DiffView.tsx", "utf8");
const graphView = readFileSync("src/components/GraphView.tsx", "utf8");
const cliBar = readFileSync("src/components/CliBar.tsx", "utf8");
const aiCommands = readFileSync("src-tauri/src/ai/commands.rs", "utf8");

describe("cross-module surface coherence", () => {
  it("connects the AI manager/scientist/orchestrator command contract to the AI face", () => {
    for (const command of [
      "list_ai_face_data_flows",
      "get_ai_manager_snapshot",
      "orchestrator_status",
      "get_dream_status",
    ]) {
      expect(main).toContain(`ai::commands::${command}`);
      expect(agentDashboard).toContain(command);
    }
    expect(main).toContain("ai::commands::list_agent_tools");
    expect(store).toContain("list_agent_tools");
    expect(agentDashboard).toContain("fetchAgentTools");

    expect(panelTabsForMode("ai").map((tab) => tab.panel)).toContain("tasks");
    expect(agentDashboard).toContain("AI Manager");
    expect(agentDashboard).toContain("AI Scientist");
    expect(agentDashboard).toContain("AI Orchestrator");
  });

  it("keeps AI face and orchestrator contracts in shared type modules", () => {
    const aiFaceTypes = readFileSync("src/types/ai-face.ts", "utf8");
    const orchestratorTypes = readFileSync("src/types/orchestrator.ts", "utf8");

    expect(agentDashboard).toContain('from "../types/ai-face"');
    expect(agentDashboard).toContain('from "../types/orchestrator"');
    expect(agentDashboard).not.toContain("interface AiFaceDataFlow");
    expect(agentDashboard).not.toContain("interface AiManagerSnapshot");
    expect(agentDashboard).not.toContain("interface OrchestratorStatus");
    expect(aiFaceTypes).toContain("export interface AiFaceDataFlow");
    expect(aiFaceTypes).toContain("export interface AiManagerSnapshot");
    expect(orchestratorTypes).toContain("export interface OrchestratorStatus");
  });

  it("keeps human review surfaces downstream of bridge and diff commands", () => {
    for (const command of ["list_bridge_proposals", "execute_bridge_action"]) {
      expect(main).toContain(`bridge::commands::${command}`);
      expect(store).toContain(command);
    }
    expect(bridgeInbox).toContain("fetchBridgeProposals");
    expect(bridgeInbox).toContain("executeBridgeAction");
    for (const command of ["get_diff_blocks", "accept_diff", "reject_diff"]) {
      expect(main).toContain(`diff::commands::${command}`);
      expect(diffView).toContain(command);
    }

    expect(panelTabsForMode("human").map((tab) => tab.panel)).toEqual([
      "editor",
      "task-intake",
      "inspiration",
      "bridge",
      "diff",
    ]);
    expect(panelTabsForMode("ai").map((tab) => tab.panel)).not.toContain("bridge");
    expect(panelTabsForMode("ai").map((tab) => tab.panel)).not.toContain("diff");
  });

  it("keeps JSON-LD memory as the primary AI memory contract with MDT only as compatibility", () => {
    for (const command of ["jsonld_index", "jsonld_read", "mdt_index", "mdt_read", "mdt_pack"]) {
      expect(main).toContain(`ai::commands::${command}`);
    }

    expect(cliBar).toContain("JSON-LD 索引（MDT 兼容）");
    expect(bridgeInbox).toContain("JSON-LD 索引（MDT 兼容）");
    expect(aiCommands).toContain("JSON-LD Index (MDT compatibility mirror)");
    expect(aiCommands).toContain("JSON-LD Memory Task (MDT compatibility)");
    expect(cliBar).not.toContain("Legacy MDT");
    expect(bridgeInbox).not.toContain("Legacy MDT");
    expect(aiCommands).not.toContain("## Legacy MDT");
  });

  it("routes graph memory display through Dream zones and backend graph commands", () => {
    expect(main).toContain("graph::commands::get_graph");
    expect(main).toContain("graph::commands::get_graph_with_focus");
    expect(app).toContain("get_graph");
    expect(graphView).toContain("get_graph_with_focus");
    expect(graphView).toContain("working");
    expect(graphView).toContain("semantic");
    expect(graphView).toContain("long_term");
    expect(graphView).toContain("bridge");
  });
});
