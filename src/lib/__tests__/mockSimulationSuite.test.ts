import { describe, expect, it } from "vitest";
import {
  buildMockSimulationRun,
  mockSimulationSuite,
} from "../mockSimulationSuite";

describe("mockSimulationSuite", () => {
  it("maps the successful case into a passed, low-risk AI run", () => {
    const run = buildMockSimulationRun("caseSuccess");

    expect(mockSimulationSuite.caseSuccess.taskId).toBe("sim-task-good");
    expect(run.task).toMatchObject({
      id: "sim-task-good",
      status: "done",
      command: "研究线性空间中的维度坍缩问题",
    });
    expect(run.kernelStatus).toBe("PASSED");
    expect(run.warningTone).toBe("stable");
    expect(run.orchestratorStatus.degraded_agents).toEqual([]);
    expect(run.orchestratorStatus.completed_track_count).toBe(1);
    expect(run.orchestratorStatus.diagnostics).toEqual([]);
    expect(run.bridgeProposal.risk).toBe("low");
    expect(run.bridgeProposal.evidence.map((item) => item.ref)).toContain(
      "local://vault/thesis_notes.md#^block-x12",
    );
    expect(run.diffBlocks[0].newText).toContain("局部仿射坍缩");
  });

  it("maps the regression case into a warning run with a degraded read-only agent", () => {
    const run = buildMockSimulationRun("caseRegressionAndWarning");

    expect(run.task).toMatchObject({
      id: "sim-task-buggy",
      status: "done",
      command: "尝试用纯几何方法证明非线性漂移的绝对消除",
    });
    expect(run.kernelStatus).toBe("WARNING_PASSED");
    expect(run.warningTone).toBe("amber");
    expect(run.orchestratorStatus.degraded_agents).toContain("Master-Alpha");
    expect(run.orchestratorStatus.workflow_replan_request_count).toBe(1);
    expect(run.orchestratorStatus.diagnostics[0]).toMatchObject({
      source: "EpochReason",
      reason_code: "regression_health_drop",
      target: "Master-Alpha",
      severity: "warning",
    });
    expect(
      run.orchestratorStatus.recent_workflow_events?.some((event) =>
        String(event.body).includes("检测到退行"),
      ),
    ).toBe(true);
    expect(run.bridgeProposal.risk).toBe("high");
    expect(run.bridgeProposal.risk_reason).toContain("置信度 58%");
    expect(run.bridgeProposal.evidence.map((item) => item.ref)).toContain(
      "https://wikipedia.org/wiki/Chaos_theory",
    );
    expect(run.diffBlocks[0].newText).toContain("高风险推论");
  });
});
