import { beforeEach, describe, expect, it } from "vitest";
import { buildMockSimulationRun } from "../../lib/mockSimulationSuite";
import { useAppStore } from "../useAppStore";

describe("useAppStore mock simulation intake", () => {
  beforeEach(() => {
    useAppStore.setState({
      tasks: [],
      diffBlocks: [],
      bridgeProposals: [],
      orchestratorStatus: null,
      mode: "human",
      activePanel: "task-intake",
    } as Partial<ReturnType<typeof useAppStore.getState>>);
  });

  it("applies a successful mock run to task, orchestrator, bridge, and diff state", () => {
    const run = buildMockSimulationRun("caseSuccess");

    useAppStore.getState().applyMockSimulationRun(run);

    const state = useAppStore.getState();
    expect(state.mode).toBe("ai");
    expect(state.activePanel).toBe("tasks");
    expect(state.tasks).toContainEqual(run.task);
    expect(state.orchestratorStatus?.last_event?.detail).toContain("收敛完成");
    expect(state.bridgeProposals[0]).toMatchObject({
      id: "bridge-sim-task-good",
      risk: "low",
    });
    expect(state.diffBlocks[0]).toMatchObject({
      ghostId: "sim-task-good",
      accepted: false,
      rejected: false,
    });
  });

  it("replaces existing simulation artifacts without duplicating stale rows", () => {
    const first = buildMockSimulationRun("caseSuccess");
    const second = buildMockSimulationRun("caseRegressionAndWarning");

    useAppStore.getState().applyMockSimulationRun(first);
    useAppStore.getState().applyMockSimulationRun(second);
    useAppStore.getState().applyMockSimulationRun(second);

    const state = useAppStore.getState();
    expect(state.tasks.filter((task) => task.id === "sim-task-buggy")).toHaveLength(1);
    expect(state.bridgeProposals.filter((proposal) => proposal.id === "bridge-sim-task-buggy")).toHaveLength(1);
    expect(state.diffBlocks.filter((block) => block.ghostId === "sim-task-buggy")).toHaveLength(1);
    expect(state.orchestratorStatus?.degraded_agents).toContain("Master-Alpha");
    expect(state.orchestratorStatus?.diagnostics[0].severity).toBe("warning");
  });
});
