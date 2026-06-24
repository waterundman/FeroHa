import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import OrchestratorWorkflowView from "../OrchestratorWorkflowView";
import { useAppStore } from "../../hooks/useAppStore";
import { runtimeBundle } from "../../test/workflowFixtures";

describe("OrchestratorWorkflowView", () => {
  beforeEach(() => {
    useAppStore.setState({
      vaultPath: "D:\\vault",
      createAndStartWorkflow: vi.fn(async () => runtimeBundle),
      fetchWorkflowRuns: vi.fn(async () => []),
      fetchWorkflowRuntimeEvents: vi.fn(async () => []),
      fetchOrchestratorStatus: vi.fn(async () => undefined),
      workflowRuns: [],
      activeWorkflowRun: null,
      workflowRunLoading: false,
      workflowRunError: null,
      workflowRuntimeEvents: [],
      workflowRuntimeEventRunId: null,
      workflowRuntimeEventError: null,
      orchestratorStatus: null,
    });
  });

  it("submits a goal and acceptance criteria through the existing orchestration view", async () => {
    const createAndStartWorkflow = vi.fn(async () => runtimeBundle);
    useAppStore.setState({ createAndStartWorkflow });

    render(<OrchestratorWorkflowView />);
    fireEvent.change(screen.getByLabelText("工作流目标"), {
      target: { value: "Map Bayesian evidence" },
    });
    fireEvent.change(screen.getByLabelText("验收条件 1"), {
      target: { value: "Every conclusion cites evidence" },
    });
    fireEvent.click(screen.getByRole("button", { name: "启动工作流" }));

    await waitFor(() => {
      expect(createAndStartWorkflow).toHaveBeenCalledWith(
        "Map Bayesian evidence",
        ["Every conclusion cites evidence"],
      );
    });
  });

  it("shows real run, artifact, and verification state", () => {
    useAppStore.setState({ activeWorkflowRun: runtimeBundle });

    render(<OrchestratorWorkflowView />);

    expect(screen.getByText("run_100")).toBeDefined();
    expect(screen.getByText("已验证")).toBeDefined();
    expect(screen.getByText("Working Memory")).toBeDefined();
    expect(screen.getByText("Semantic Memory")).toBeDefined();
    expect(screen.getByText("Every conclusion cites evidence")).toBeDefined();
  });
});
