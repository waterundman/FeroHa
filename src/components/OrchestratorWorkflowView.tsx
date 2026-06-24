import { useEffect, useMemo, useState } from "react";
import type { CSSProperties, FormEvent, ReactNode } from "react";
import { useAppStore } from "../hooks/useAppStore";
import {
  workflowEventDetail,
  workflowEventLabel,
} from "../lib/orchestratorEventPresentation";
import type {
  ArtifactRef,
  VerificationFinding,
  WorkflowRuntimeBundle,
  WorkflowStepStatus,
} from "../types/orchestrator";
import FeroHaIcon from "./FeroHaIcon";

const stepStatusLabels: Record<WorkflowStepStatus, string> = {
  pending: "待处理",
  ready: "已就绪",
  running: "运行中",
  reported: "结果已记录",
  verified: "已验证",
  failed: "失败",
  blocked: "阻塞",
  skipped: "已跳过",
};

export default function OrchestratorWorkflowView() {
  const vaultPath = useAppStore((state) => state.vaultPath);
  const workflowRuns = useAppStore((state) => state.workflowRuns);
  const activeWorkflowRun = useAppStore((state) => state.activeWorkflowRun);
  const workflowRunLoading = useAppStore((state) => state.workflowRunLoading);
  const workflowRunError = useAppStore((state) => state.workflowRunError);
  const workflowEvents = useAppStore((state) => state.workflowRuntimeEvents);
  const eventRunId = useAppStore((state) => state.workflowRuntimeEventRunId);
  const eventError = useAppStore((state) => state.workflowRuntimeEventError);
  const createAndStartWorkflow = useAppStore((state) => state.createAndStartWorkflow);
  const fetchWorkflowRuns = useAppStore((state) => state.fetchWorkflowRuns);
  const fetchWorkflowRun = useAppStore((state) => state.fetchWorkflowRun);
  const fetchWorkflowRuntimeEvents = useAppStore(
    (state) => state.fetchWorkflowRuntimeEvents,
  );
  const [goal, setGoal] = useState("");
  const [criteria, setCriteria] = useState([""]);

  useEffect(() => {
    if (vaultPath) void fetchWorkflowRuns();
  }, [fetchWorkflowRuns, vaultPath]);

  useEffect(() => {
    const runId = activeWorkflowRun?.run.run_id;
    if (runId) void fetchWorkflowRuntimeEvents(runId, 200);
  }, [activeWorkflowRun?.run.run_id, fetchWorkflowRuntimeEvents]);

  const normalizedCriteria = criteria.map((criterion) => criterion.trim()).filter(Boolean);
  const canStart =
    Boolean(vaultPath) &&
    goal.trim().length > 0 &&
    normalizedCriteria.length > 0 &&
    !workflowRunLoading;

  const handleSubmit = async (event: FormEvent) => {
    event.preventDefault();
    if (!canStart) return;
    const result = await createAndStartWorkflow(goal.trim(), normalizedCriteria);
    if (result) {
      setGoal("");
      setCriteria([""]);
    }
  };

  return (
    <section style={styles.shell} aria-label="Orchestrator 编排中枢">
      <header style={styles.header}>
        <div style={styles.heading}>
          <span style={styles.headingIcon}>
            <FeroHaIcon name="Workflow" size={18} />
          </span>
          <div>
            <h2 style={styles.title}>编排中枢</h2>
            <p style={styles.subtitle}>目标进入现有 AI Manager，产物留在 Dream 文件系统。</p>
          </div>
        </div>
        {workflowRuns.length > 0 && (
          <select
            aria-label="工作流运行"
            value={activeWorkflowRun?.run.run_id ?? ""}
            onChange={(event) => void fetchWorkflowRun(event.target.value)}
            style={styles.select}
          >
            {workflowRuns.map((bundle) => (
              <option key={bundle.run.run_id} value={bundle.run.run_id}>
                {bundle.run.run_id} · {runStatusLabel(bundle.run.status)}
              </option>
            ))}
          </select>
        )}
      </header>

      <form style={styles.form} onSubmit={handleSubmit}>
        <label style={styles.field}>
          <span style={styles.label}>工作流目标</span>
          <textarea
            aria-label="工作流目标"
            value={goal}
            onChange={(event) => setGoal(event.target.value)}
            placeholder="描述要研究和验证的目标"
            rows={3}
            style={styles.textarea}
          />
        </label>
        <div style={styles.criteriaHeader}>
          <span style={styles.label}>验收条件</span>
          <button
            type="button"
            style={styles.secondaryButton}
            onClick={() => setCriteria((current) => [...current, ""])}
          >
            <FeroHaIcon name="Plus" size={13} /> 添加
          </button>
        </div>
        <div style={styles.criteriaList}>
          {criteria.map((criterion, index) => (
            <div key={index} style={styles.criterionRow}>
              <input
                aria-label={`验收条件 ${index + 1}`}
                value={criterion}
                onChange={(event) =>
                  setCriteria((current) =>
                    current.map((item, itemIndex) =>
                      itemIndex === index ? event.target.value : item,
                    ),
                  )
                }
                placeholder={`条件 ${index + 1}`}
                style={styles.input}
              />
              {criteria.length > 1 && (
                <button
                  type="button"
                  aria-label={`删除验收条件 ${index + 1}`}
                  title="删除"
                  style={styles.iconButton}
                  onClick={() =>
                    setCriteria((current) =>
                      current.filter((_, itemIndex) => itemIndex !== index),
                    )
                  }
                >
                  <FeroHaIcon name="X" size={14} />
                </button>
              )}
            </div>
          ))}
        </div>
        <div style={styles.formFooter}>
          {!vaultPath && <span style={styles.hint}>打开笔记库后可启动工作流</span>}
          <button type="submit" disabled={!canStart} style={styles.primaryButton}>
            <FeroHaIcon name="Play" size={14} />
            {workflowRunLoading ? "启动中" : "启动工作流"}
          </button>
        </div>
      </form>

      {workflowRunError && <div style={styles.error}>{workflowRunError}</div>}
      {activeWorkflowRun ? (
        <RunDetail
          bundle={activeWorkflowRun}
          events={
            eventRunId === activeWorkflowRun.run.run_id ? workflowEvents : []
          }
          eventError={eventError}
        />
      ) : (
        <div style={styles.empty}>
          <FeroHaIcon name="Activity" size={24} />
          <span>{workflowRunLoading ? "正在读取运行状态" : "暂无工作流运行"}</span>
        </div>
      )}
    </section>
  );
}

function RunDetail({
  bundle,
  events,
  eventError,
}: {
  bundle: WorkflowRuntimeBundle;
  events: ReturnType<typeof useAppStore.getState>["workflowRuntimeEvents"];
  eventError: string | null;
}) {
  const groupedArtifacts = useMemo(() => groupArtifacts(bundle.artifacts), [bundle.artifacts]);
  return (
    <div style={styles.detail}>
      <div style={styles.runHeader}>
        <div>
          <span style={styles.eyebrow}>当前运行</span>
          <strong style={styles.runId}>{bundle.run.run_id}</strong>
        </div>
        <span style={statusStyle(bundle.run.status)}>{runStatusLabel(bundle.run.status)}</span>
      </div>

      <Section title="步骤">
        <div style={styles.rows}>
          {bundle.workflow.steps.map((step) => (
            <div key={step.step_id} style={styles.row}>
              <span style={styles.rowId}>{step.step_id}</span>
              <span style={styles.rowMain}>{step.title}</span>
              <span style={styles.rowMeta}>{step.agent_type}</span>
              <span style={statusStyle(step.status)}>{stepStatusLabels[step.status]}</span>
            </div>
          ))}
        </div>
      </Section>

      <div style={styles.twoColumn}>
        <ArtifactGroup title="Working Memory" artifacts={groupedArtifacts.working} />
        <ArtifactGroup title="Semantic Memory" artifacts={groupedArtifacts.semantic} />
      </div>

      <Section title="验证">
        {bundle.verification_findings.length > 0 ? (
          <div style={styles.rows}>
            {bundle.verification_findings.map((finding) => (
              <FindingRow key={finding.verification_id} finding={finding} />
            ))}
          </div>
        ) : (
          <span style={styles.hint}>等待验证结果</span>
        )}
      </Section>

      <Section title="运行事件">
        {eventError && <div style={styles.error}>{eventError}</div>}
        {events.length > 0 ? (
          <div style={styles.rows}>
            {events.slice(-20).reverse().map((event, index) => (
              <div key={`${event.timestamp}-${event.event_name}-${index}`} style={styles.eventRow}>
                <span style={styles.eventName}>{workflowEventLabel(event.event_name)}</span>
                <span style={styles.eventDetail}>
                  {workflowEventDetail(event) ?? event.body}
                </span>
              </div>
            ))}
          </div>
        ) : (
          <span style={styles.hint}>等待运行事件</span>
        )}
      </Section>
    </div>
  );
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section style={styles.section}>
      <h3 style={styles.sectionTitle}>{title}</h3>
      {children}
    </section>
  );
}

function ArtifactGroup({ title, artifacts }: { title: string; artifacts: ArtifactRef[] }) {
  return (
    <Section title={title}>
      {artifacts.length > 0 ? (
        <div style={styles.rows}>
          {artifacts.map((artifact) => (
            <div key={artifact.artifact_id} style={styles.artifactRow}>
              <FeroHaIcon name="FileText" size={14} />
              <span style={styles.artifactUri}>{artifact.uri}</span>
            </div>
          ))}
        </div>
      ) : (
        <span style={styles.hint}>暂无产物</span>
      )}
    </Section>
  );
}

function FindingRow({ finding }: { finding: VerificationFinding }) {
  const passed = finding.result === "pass";
  return (
    <div style={styles.findingRow}>
      <span style={passed ? styles.passMark : styles.failMark}>
        <FeroHaIcon name={passed ? "Check" : "X"} size={13} />
      </span>
      <span style={styles.rowMain}>{finding.summary}</span>
      <span style={styles.rowMeta}>{finding.reason_code}</span>
    </div>
  );
}

function groupArtifacts(artifacts: ArtifactRef[]) {
  return {
    working: artifacts.filter(
      (artifact) =>
        artifact.uri.includes("/research/") ||
        artifact.uri.includes("/memory/working/"),
    ),
    semantic: artifacts.filter(
      (artifact) =>
        artifact.uri.includes("/memory/semantic/") ||
        artifact.uri.includes("/jsonld/") ||
        artifact.uri.includes("/mdt/"),
    ),
  };
}

function runStatusLabel(status: WorkflowRuntimeBundle["run"]["status"]) {
  return {
    queued: "排队中",
    running: "运行中",
    paused: "已暂停",
    failed: "失败",
    succeeded: "成功",
  }[status];
}

function statusStyle(status: string): CSSProperties {
  const isFailure = status === "failed" || status === "blocked" || status === "aborted";
  const isSuccess = status === "succeeded" || status === "verified" || status === "completed";
  return {
    ...styles.status,
    color: isFailure
      ? "var(--diff-delete)"
      : isSuccess
        ? "var(--diff-insert)"
        : "var(--accent-primary)",
  };
}

const styles: Record<string, CSSProperties> = {
  shell: { height: "100%", overflow: "auto", padding: 16, background: "var(--bg-primary)" },
  header: { display: "flex", justifyContent: "space-between", alignItems: "center", gap: 12, marginBottom: 14 },
  heading: { display: "flex", alignItems: "center", gap: 10, minWidth: 0 },
  headingIcon: { width: 34, height: 34, display: "inline-flex", alignItems: "center", justifyContent: "center", border: "1px solid var(--border-color)", borderRadius: 8, color: "var(--accent-primary)" },
  title: { margin: 0, fontSize: 17, color: "var(--text-primary)" },
  subtitle: { margin: "3px 0 0", fontSize: 12, color: "var(--text-muted)" },
  select: { minWidth: 190, padding: "7px 9px", border: "1px solid var(--border-color)", borderRadius: 6, color: "var(--text-primary)", background: "var(--bg-input)" },
  form: { display: "flex", flexDirection: "column", gap: 10, paddingBottom: 15, borderBottom: "1px solid var(--border-color)" },
  field: { display: "flex", flexDirection: "column", gap: 6 },
  label: { fontSize: 12, fontWeight: 700, color: "var(--text-primary)" },
  textarea: { width: "100%", resize: "vertical", boxSizing: "border-box", padding: 10, border: "1px solid var(--border-color)", borderRadius: 6, color: "var(--text-primary)", background: "var(--bg-input)", font: "inherit" },
  criteriaHeader: { display: "flex", justifyContent: "space-between", alignItems: "center" },
  criteriaList: { display: "flex", flexDirection: "column", gap: 7 },
  criterionRow: { display: "flex", gap: 7 },
  input: { flex: 1, minWidth: 0, padding: "8px 9px", border: "1px solid var(--border-color)", borderRadius: 6, color: "var(--text-primary)", background: "var(--bg-input)" },
  iconButton: { width: 34, border: "1px solid var(--border-color)", borderRadius: 6, color: "var(--text-secondary)", background: "var(--bg-secondary)", cursor: "pointer" },
  secondaryButton: { display: "inline-flex", alignItems: "center", gap: 5, padding: "5px 8px", border: "1px solid var(--border-color)", borderRadius: 6, color: "var(--text-secondary)", background: "var(--bg-secondary)", cursor: "pointer" },
  formFooter: { display: "flex", justifyContent: "space-between", alignItems: "center", gap: 10 },
  primaryButton: { display: "inline-flex", alignItems: "center", gap: 6, padding: "8px 12px", border: 0, borderRadius: 6, color: "var(--bg-primary)", background: "var(--accent-primary)", cursor: "pointer" },
  hint: { fontSize: 12, color: "var(--text-muted)" },
  error: { padding: "8px 10px", marginTop: 10, border: "1px solid var(--diff-delete)", borderRadius: 6, color: "var(--diff-delete)", fontSize: 12 },
  empty: { minHeight: 150, display: "flex", flexDirection: "column", justifyContent: "center", alignItems: "center", gap: 8, color: "var(--text-muted)" },
  detail: { display: "flex", flexDirection: "column", gap: 15, paddingTop: 15 },
  runHeader: { display: "flex", justifyContent: "space-between", alignItems: "center" },
  eyebrow: { display: "block", fontSize: 10, color: "var(--text-muted)", marginBottom: 3 },
  runId: { color: "var(--text-primary)", fontSize: 15 },
  status: { fontSize: 11, fontWeight: 700, whiteSpace: "nowrap" },
  section: { display: "flex", flexDirection: "column", gap: 7, minWidth: 0 },
  sectionTitle: { margin: 0, fontSize: 12, color: "var(--text-primary)" },
  twoColumn: { display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(260px, 1fr))", gap: 14 },
  rows: { display: "flex", flexDirection: "column", gap: 6 },
  row: { display: "grid", gridTemplateColumns: "56px minmax(0, 1fr) minmax(110px, .5fr) auto", gap: 8, alignItems: "center", padding: "8px 9px", border: "1px solid var(--border-muted)", borderRadius: 6, background: "var(--bg-secondary)" },
  rowId: { color: "var(--text-muted)", fontSize: 11 },
  rowMain: { color: "var(--text-primary)", fontSize: 12, overflow: "hidden", textOverflow: "ellipsis" },
  rowMeta: { color: "var(--text-muted)", fontSize: 10, overflow: "hidden", textOverflow: "ellipsis" },
  artifactRow: { display: "flex", alignItems: "center", gap: 7, padding: "8px 9px", border: "1px solid var(--border-muted)", borderRadius: 6, color: "var(--text-secondary)", background: "var(--bg-secondary)" },
  artifactUri: { minWidth: 0, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", fontSize: 11 },
  findingRow: { display: "grid", gridTemplateColumns: "22px minmax(0, 1fr) minmax(120px, .5fr)", gap: 8, alignItems: "center", padding: "8px 9px", border: "1px solid var(--border-muted)", borderRadius: 6, background: "var(--bg-secondary)" },
  passMark: { color: "var(--diff-insert)" },
  failMark: { color: "var(--diff-delete)" },
  eventRow: { display: "grid", gridTemplateColumns: "150px minmax(0, 1fr)", gap: 8, padding: "7px 9px", borderBottom: "1px solid var(--border-muted)" },
  eventName: { color: "var(--text-primary)", fontSize: 11, fontWeight: 700 },
  eventDetail: { color: "var(--text-secondary)", fontSize: 11, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" },
};
