import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import OrchestratorPanel from "../OrchestratorPanel";
import { useAppStore } from "../../hooks/useAppStore";
import { runtimeBundle } from "../../test/workflowFixtures";

describe("OrchestratorPanel", () => {
  beforeEach(() => {
    useAppStore.setState({
      fetchOrchestratorStatus: vi.fn(async () => undefined),
      orchestratorStatus: {
        active_agents: 2,
        degraded_agents: ["agent_a"],
        epoch_count: 7,
        track_count: 0,
        track_event_count: 4,
        material_packet_count: 3,
        active_track_count: 1,
        completed_track_count: 0,
        failed_track_count: 0,
        cancelled_track_count: 0,
        workflow_event_count: 5,
        workflow_replan_request_count: 1,
        last_event: null,
        recent_events: [],
        agent_states: [],
        track_details: [],
        diagnostics: [{
          source: "WorkflowVerifier",
          reason_code: "evidence_missing",
          summary: "Evidence is missing",
          target: "S001",
          minimal_fix_surface: ["Working result"],
          evidence_refs: [],
          failed_clauses: [1],
          severity: "error",
        }],
      },
      activeWorkflowRun: {
        ...runtimeBundle,
        run: { ...runtimeBundle.run, status: "running", active_step_ids: ["S001"] },
      },
    });
  });

  it("shows macro workflow health in the bottom strip", () => {
    render(<OrchestratorPanel />);

    expect(screen.getByText("编排中枢")).toBeDefined();
    expect(screen.getByText("运行中")).toBeDefined();
    expect(screen.getByText("任务 1")).toBeDefined();
    expect(screen.getByText("验证失败 1")).toBeDefined();
    expect(screen.getByText("Replan 1")).toBeDefined();
  });

  it("keeps detailed ledger and artifacts out of the macro panel", () => {
    render(<OrchestratorPanel />);
    fireEvent.click(screen.getByRole("button", { name: "展开编排状态" }));

    expect(screen.getByText("Evidence is missing")).toBeDefined();
    expect(screen.queryByRole("button", { name: /ledger/i })).toBeNull();
    expect(screen.queryByText(/\.dualtrack\/research\/results/)).toBeNull();
  });
});
