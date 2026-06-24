import { describe, expect, it } from "vitest";
import type { HarnessEvent } from "../../types/orchestrator";
import {
  workflowDispatchedCount,
  workflowEventDetail,
  workflowEventLabel,
} from "../orchestratorEventPresentation";

function event(eventName: string, attributes: Record<string, unknown> = {}): HarnessEvent {
  return {
    timestamp: "2026-06-19T00:00:00Z",
    severity_text: "INFO",
    event_name: eventName,
    body: "runtime event",
    attributes,
  };
}

describe("orchestrator event presentation", () => {
  it.each([
    ["workflow.run.created", "运行已创建"],
    ["workflow.run.resumed", "运行已恢复"],
    ["workflow.step.dispatched", "步骤已派发"],
    ["workflow.step.queued", "步骤已排队"],
    ["workflow.step.running", "步骤运行中"],
    ["workflow.step.reported", "结果已记录"],
    ["workflow.step.verified", "步骤已验证"],
    ["workflow.semantic.promoted", "知识已晋升"],
    ["workflow.run.succeeded", "运行成功"],
    ["workflow.run.failed", "运行失败"],
    ["workflow.replan.requested", "请求重规划"],
    ["workflow.patch.review_requested", "等待 Bridge 审查"],
    ["workflow.patch.accepted", "补丁已接受"],
    ["workflow.patch.rejected", "补丁已拒绝"],
    ["workflow.verification.passed", "验证通过"],
    ["workflow.verification.failed", "验证失败"],
    ["workflow.verification.cannot_verify", "无法验证"],
  ])("maps %s to a readable label", (eventName, expected) => {
    expect(workflowEventLabel(eventName)).toBe(expected);
  });

  it("falls back to the original event name", () => {
    expect(workflowEventLabel("workflow.custom.created")).toBe("workflow.custom.created");
  });

  it("presents dispatch identity in step, agent, capability order", () => {
    expect(
      workflowEventDetail(
        event("workflow.step.dispatched", {
          step_id: "S001",
          agent_type: "scientist",
          capability: "research",
        }),
      ),
    ).toBe("S001 · scientist · research");
  });

  it("omits missing or invalid dispatch attributes without throwing", () => {
    expect(
      workflowEventDetail(
        event("workflow.step.dispatched", {
          step_id: " ",
          agent_type: "scientist",
          capability: null,
        }),
      ),
    ).toBe("scientist");
    expect(workflowEventDetail(event("workflow.step.dispatched"))).toBeNull();
  });

  it("counts dispatched events only", () => {
    expect(
      workflowDispatchedCount([
        event("workflow.step.dispatched"),
        event("workflow.replan.requested"),
        event("workflow.step.dispatched"),
      ]),
    ).toBe(2);
  });
});
