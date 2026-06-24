import { beforeEach, describe, expect, it, vi } from "vitest";
import { useAppStore } from "../useAppStore";
import type { HarnessEvent, OrchestratorOutput } from "../../types/orchestrator";
import type { BridgeProposal } from "../../types/bridge-proposal";
import { runtimeBundle } from "../../test/workflowFixtures";

const invokeMock = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
}));

function workflowEvent(overrides: Partial<HarnessEvent> = {}): HarnessEvent {
  return {
    timestamp: "2026-06-07T00:00:00Z",
    severity_text: "INFO",
    event_name: "workflow.patch.accepted",
    body: "Workflow patch accepted by bridge reviewer.",
    attributes: {
      workflow_id: "wf_runtime",
      run_id: "run_runtime",
      patch_id: "patch_1",
    },
    ...overrides,
  };
}

describe("useAppStore workflow runtime events", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    useAppStore.setState({
      workflowRuntimeEvents: [],
      workflowRuntimeEventError: null,
      workflowRuns: [],
      activeWorkflowRun: null,
      workflowRunLoading: false,
      workflowRunError: null,
      bridgeProposals: [],
    } as Partial<ReturnType<typeof useAppStore.getState>>);
  });

  it("fetches and caches workflow runtime events by run", async () => {
    const events = [workflowEvent()];
    invokeMock.mockResolvedValueOnce(events);

    const result = await useAppStore
      .getState()
      .fetchWorkflowRuntimeEvents("run_runtime", 7);

    expect(invokeMock).toHaveBeenCalledWith("read_workflow_runtime_events", {
      runId: "run_runtime",
      limit: 7,
    });
    expect(result).toEqual(events);
    expect(useAppStore.getState().workflowRuntimeEvents).toEqual(events);
    expect(useAppStore.getState().workflowRuntimeEventError).toBeNull();
  });

  it("submits workflow patch review proposals through the bridge command", async () => {
    const patch = {
      patch_id: "patch_wf_runtime_v1_to_v2",
      workflow_id: "wf_runtime",
      from_version: 1,
      to_version: 2,
      basis: {
        failed_steps: ["S001"],
        failed_goal_clauses: [2],
      },
      ops: [],
      rationale: "Retry verifier step after human review.",
      predicted_impact: { risk: "medium" },
    };
    const proposal = {
      id: "bridge_1",
      source: "scheduler",
      source_ref: { kind: "scheduler_job", id: "run_runtime", path: "wf_runtime" },
      intent: "审查 Workflow patch",
      summary: "Workflow patch review",
      evidence: [],
      impact: {
        notes: [],
        creates_files: false,
        modifies_notes: false,
        exports_data: false,
        external_side_effect: false,
      },
      risk: "medium",
      status: "pending",
      actions: [],
      trust_snapshot: {
        score: 0.5,
        acceptance_rate: 0,
        total_interactions: 0,
        recommended_mode: "manual",
      },
      created_at: 1,
      updated_at: 1,
    } satisfies BridgeProposal;
    invokeMock.mockResolvedValueOnce(proposal);

    const result = await useAppStore
      .getState()
      .submitWorkflowPatchReview("run_runtime", patch);

    expect(invokeMock).toHaveBeenCalledWith("submit_workflow_patch_review", {
      runId: "run_runtime",
      patch,
    });
    expect(result).toEqual(proposal);
    expect(useAppStore.getState().bridgeProposals).toEqual([proposal]);
  });

  it("submits orchestrator workflow patch output through the bridge command", async () => {
    const output = {
      type: "workflow_patch",
      patch: {
        patch_id: "patch_wf_runtime_v1_to_v2",
        workflow_id: "wf_runtime",
        from_version: 1,
        to_version: 2,
        basis: {
          failed_steps: ["S001"],
          failed_goal_clauses: [2],
        },
        ops: [],
        rationale: "Retry verifier step after human review.",
        predicted_impact: { risk: "medium" },
      },
    } satisfies OrchestratorOutput;
    const proposal = {
      id: "bridge_2",
      source: "scheduler",
      source_ref: { kind: "scheduler_job", id: "run_runtime", path: "wf_runtime" },
      intent: "审查 Workflow patch",
      summary: "Workflow patch review",
      evidence: [],
      impact: {
        notes: [],
        creates_files: false,
        modifies_notes: false,
        exports_data: false,
        external_side_effect: false,
      },
      risk: "medium",
      status: "pending",
      actions: [],
      trust_snapshot: {
        score: 0.5,
        acceptance_rate: 0,
        total_interactions: 0,
        recommended_mode: "manual",
      },
      created_at: 1,
      updated_at: 1,
    } satisfies BridgeProposal;
    invokeMock.mockResolvedValueOnce(proposal);

    const result = await useAppStore
      .getState()
      .submitOrchestratorOutputReview("run_runtime", output);

    expect(invokeMock).toHaveBeenCalledWith("submit_orchestrator_output_review", {
      runId: "run_runtime",
      output,
    });
    expect(result).toEqual(proposal);
    expect(useAppStore.getState().bridgeProposals).toEqual([proposal]);
  });

  it("creates and stores a workflow run through the focused command", async () => {
    invokeMock.mockResolvedValueOnce(runtimeBundle);

    const result = await useAppStore.getState().createAndStartWorkflow(
      "Map Bayesian evidence",
      ["Every conclusion cites evidence"],
    );

    expect(invokeMock).toHaveBeenCalledWith("create_and_start_workflow", {
      goalText: "Map Bayesian evidence",
      acceptanceCriteria: ["Every conclusion cites evidence"],
    });
    expect(result).toEqual(runtimeBundle);
    expect(useAppStore.getState().activeWorkflowRun).toEqual(runtimeBundle);
  });

  it("lists workflow runs and keeps the newest run active", async () => {
    invokeMock.mockResolvedValueOnce([runtimeBundle]);

    await useAppStore.getState().fetchWorkflowRuns();

    expect(invokeMock).toHaveBeenCalledWith("list_workflow_runs");
    expect(useAppStore.getState().workflowRuns).toEqual([runtimeBundle]);
    expect(useAppStore.getState().activeWorkflowRun?.run.run_id).toBe("run_100");
  });
});
