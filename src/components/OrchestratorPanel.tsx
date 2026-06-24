import { useEffect, useState } from "react";
import type { CSSProperties } from "react";
import { useAppStore } from "../hooks/useAppStore";
import type { WorkflowRunStatus } from "../types/orchestrator";
import FeroHaIcon from "./FeroHaIcon";

export default function OrchestratorPanel() {
  const orchestratorStatus = useAppStore((state) => state.orchestratorStatus);
  const activeWorkflowRun = useAppStore((state) => state.activeWorkflowRun);
  const fetchOrchestratorStatus = useAppStore((state) => state.fetchOrchestratorStatus);
  const [expanded, setExpanded] = useState(false);

  useEffect(() => {
    void fetchOrchestratorStatus();
    const interval = window.setInterval(fetchOrchestratorStatus, 5000);
    return () => window.clearInterval(interval);
  }, [fetchOrchestratorStatus]);

  const runStatus = activeWorkflowRun?.run.status ?? null;
  const activeTasks = activeWorkflowRun?.run.active_step_ids.length ?? 0;
  const runtimeFailures =
    activeWorkflowRun?.verification_findings.filter((finding) => finding.result !== "pass")
      .length ?? 0;
  const diagnosticFailures =
    orchestratorStatus?.diagnostics?.filter(
      (diagnostic) =>
        diagnostic.source === "WorkflowVerifier" &&
        (diagnostic.severity === "error" || diagnostic.severity === "warning"),
    ).length ?? 0;
  const verificationFailures = Math.max(runtimeFailures, diagnosticFailures);
  const replanCount = orchestratorStatus?.workflow_replan_request_count ?? 0;
  const latestAbnormal =
    [...(orchestratorStatus?.diagnostics ?? [])].reverse().find(
      (diagnostic) => diagnostic.severity !== "info",
    )?.summary ??
    [...(activeWorkflowRun?.verification_findings ?? [])].reverse().find(
      (finding) => finding.result !== "pass",
    )?.summary ??
    "暂无异常";
  const statusLabel = runStatus ? runStatusText(runStatus) : "等待运行";

  return (
    <div style={styles.container}>
      <div
        role="button"
        tabIndex={0}
        aria-label={expanded ? "收起编排状态" : "展开编排状态"}
        style={styles.bar}
        onClick={() => setExpanded((value) => !value)}
        onKeyDown={(event) => {
          if (event.key === "Enter" || event.key === " ") {
            event.preventDefault();
            setExpanded((value) => !value);
          }
        }}
      >
        <div style={styles.brand}>
          <span style={styles.icon}><FeroHaIcon name="Workflow" size={14} /></span>
          <span>
            <strong style={styles.title}>编排中枢</strong>
            <span style={styles.statusText}>{statusLabel}</span>
          </span>
        </div>
        <div style={styles.metrics}>
          <Metric label={`任务 ${activeTasks}`} warn={false} />
          <Metric label={`验证失败 ${verificationFailures}`} warn={verificationFailures > 0} />
          <Metric label={`Replan ${replanCount}`} warn={replanCount > 0} />
        </div>
        <FeroHaIcon name={expanded ? "ChevronDown" : "ChevronUp"} size={14} />
      </div>

      {expanded && (
        <div style={styles.panel}>
          <div style={styles.summaryGrid}>
            <Summary label="当前运行" value={activeWorkflowRun?.run.run_id ?? "无"} />
            <Summary label="运行状态" value={statusLabel} />
            <Summary label="活跃任务" value={String(activeTasks)} />
            <Summary label="验证失败" value={String(verificationFailures)} />
            <Summary label="Replan" value={String(replanCount)} />
          </div>
          <div style={styles.abnormal}>
            <span style={styles.abnormalLabel}>最新异常</span>
            <span style={styles.abnormalText}>{latestAbnormal}</span>
          </div>
        </div>
      )}
    </div>
  );
}

function Metric({ label, warn }: { label: string; warn: boolean }) {
  return <span style={warn ? { ...styles.metric, ...styles.metricWarn } : styles.metric}>{label}</span>;
}

function Summary({ label, value }: { label: string; value: string }) {
  return (
    <div style={styles.summary}>
      <span style={styles.summaryLabel}>{label}</span>
      <strong style={styles.summaryValue}>{value}</strong>
    </div>
  );
}

function runStatusText(status: WorkflowRunStatus) {
  return {
    queued: "排队中",
    running: "运行中",
    paused: "已暂停",
    failed: "失败",
    succeeded: "成功",
  }[status];
}

const styles: Record<string, CSSProperties> = {
  container: { borderTop: "1px solid var(--border-color)", background: "var(--bg-primary)" },
  bar: { minHeight: 38, display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12, padding: "6px 12px", background: "var(--bg-secondary)", cursor: "pointer" },
  brand: { display: "flex", alignItems: "center", gap: 8, minWidth: 0 },
  icon: { width: 24, height: 24, display: "inline-flex", alignItems: "center", justifyContent: "center", border: "1px solid var(--border-color)", borderRadius: 6, color: "var(--accent-primary)" },
  title: { display: "block", fontSize: 12, color: "var(--text-primary)", lineHeight: 1.1 },
  statusText: { display: "block", marginTop: 2, fontSize: 10, color: "var(--text-muted)" },
  metrics: { display: "flex", alignItems: "center", gap: 7, marginLeft: "auto" },
  metric: { padding: "3px 7px", border: "1px solid var(--border-color)", borderRadius: 5, color: "var(--text-secondary)", fontSize: 10, whiteSpace: "nowrap" },
  metricWarn: { color: "var(--diff-delete)", borderColor: "var(--diff-delete)" },
  panel: { padding: 10, borderTop: "1px solid var(--border-color)", background: "var(--bg-primary)" },
  summaryGrid: { display: "grid", gridTemplateColumns: "repeat(5, minmax(90px, 1fr))", gap: 8 },
  summary: { padding: "7px 8px", border: "1px solid var(--border-muted)", borderRadius: 6, background: "var(--bg-secondary)" },
  summaryLabel: { display: "block", fontSize: 9, color: "var(--text-muted)" },
  summaryValue: { display: "block", marginTop: 3, fontSize: 11, color: "var(--text-primary)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" },
  abnormal: { display: "grid", gridTemplateColumns: "80px minmax(0, 1fr)", gap: 8, marginTop: 8, padding: "7px 8px", borderTop: "1px solid var(--border-muted)" },
  abnormalLabel: { fontSize: 10, color: "var(--text-muted)" },
  abnormalText: { fontSize: 11, color: "var(--text-secondary)", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" },
};
