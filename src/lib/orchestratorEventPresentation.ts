import type { HarnessEvent } from "../types/orchestrator";

const EVENT_LABELS: Readonly<Record<string, string>> = {
  "workflow.run.created": "运行已创建",
  "workflow.run.resumed": "运行已恢复",
  "workflow.step.dispatched": "步骤已派发",
  "workflow.step.queued": "步骤已排队",
  "workflow.step.running": "步骤运行中",
  "workflow.step.reported": "结果已记录",
  "workflow.step.verified": "步骤已验证",
  "workflow.semantic.promoted": "知识已晋升",
  "workflow.run.succeeded": "运行成功",
  "workflow.run.failed": "运行失败",
  "workflow.replan.requested": "请求重规划",
  "workflow.patch.review_requested": "等待 Bridge 审查",
  "workflow.patch.accepted": "补丁已接受",
  "workflow.patch.rejected": "补丁已拒绝",
  "workflow.verification.passed": "验证通过",
  "workflow.verification.failed": "验证失败",
  "workflow.verification.cannot_verify": "无法验证",
};

export function workflowEventLabel(eventName: string): string {
  return EVENT_LABELS[eventName] ?? eventName;
}

export function workflowEventDetail(
  event: Pick<HarnessEvent, "event_name" | "attributes">,
): string | null {
  const attributes = event.attributes ?? {};

  if (event.event_name === "workflow.step.dispatched") {
    const dispatchParts = ["step_id", "agent_type", "capability"]
      .map((key) => nonEmptyString(attributes[key]))
      .filter((value): value is string => value !== null);
    return dispatchParts.length > 0 ? dispatchParts.join(" · ") : null;
  }

  const failedSteps = attributes.failed_step_ids;
  if (Array.isArray(failedSteps)) {
    const stepIds = failedSteps
      .map(nonEmptyString)
      .filter((value): value is string => value !== null);
    if (stepIds.length > 0) return `失败步骤 ${stepIds.join(", ")}`;
  }

  const target = nonEmptyString(attributes.target);
  if (target) return target;
  const artifactUri = nonEmptyString(attributes.artifact_uri);
  if (artifactUri) return artifactUri;
  const stepId = nonEmptyString(attributes.step_id);
  if (stepId) return stepId;
  const runId = nonEmptyString(attributes.run_id);
  if (runId) return runId;

  const findingCount = attributes.finding_count;
  if (typeof findingCount === "number" && Number.isFinite(findingCount)) {
    return `${findingCount} 条 finding`;
  }
  return null;
}

export function workflowDispatchedCount(
  events: readonly Pick<HarnessEvent, "event_name">[],
): number {
  return events.filter((event) => event.event_name === "workflow.step.dispatched").length;
}

function nonEmptyString(value: unknown): string | null {
  if (typeof value !== "string") return null;
  const normalized = value.trim();
  return normalized.length > 0 ? normalized : null;
}
