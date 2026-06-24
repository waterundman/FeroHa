import { useEffect, useMemo, useRef, useState } from "react";
import { useAppStore } from "../hooks/useAppStore";
import type { BridgeProposal, ProposalAction } from "../types/bridge-proposal";
import FeroHaIcon from "./FeroHaIcon";

interface BridgeInboxProps {
  isTauri: boolean;
}

const resolvedStatuses = new Set(["approved", "rejected", "applied", "archived"]);
const actionKey = (proposalId: string, actionId: string) => `${proposalId}:${actionId}`;

const bridgeSourceLabels: Record<string, string> = {
  tool: "工具",
  scientist: "AI Scientist",
  dream: "Dream",
  ghost: "Ghost",
  scheduler: "调度器",
};

const bridgeStatusLabels: Record<string, string> = {
  pending: "待审查",
  approved: "已批准",
  rejected: "已拒绝",
  applied: "已应用",
  archived: "已归档",
};

const bridgeRiskLabels: Record<string, string> = {
  low: "低风险",
  medium: "中风险",
  high: "高风险",
};

const bridgeTaskTypeLabels: Record<string, string> = {
  research: "研究",
  summarize: "总结",
  verify: "验证",
  dream: "Dream",
  jsonld_index: "JSON-LD 索引",
  jsonld_read: "JSON-LD 读取",
  mdt_index: "JSON-LD 索引（MDT 兼容）",
  mdt_read: "JSON-LD 读取（MDT 兼容）",
  mdt_pack: "MDT 归档兼容包",
  workflow_patch: "Workflow 补丁",
  write_proposal: "写作提案",
  external_import: "外部导入",
  code_assist: "代码协助",
};

export function bridgeSourceLabel(source: string): string {
  return bridgeSourceLabels[source] ?? source;
}

export function bridgeStatusLabel(status: string): string {
  return bridgeStatusLabels[status] ?? status;
}

export function bridgeRiskLabel(risk: string): string {
  return bridgeRiskLabels[risk] ?? risk;
}

export function bridgeTaskTypeLabel(taskType: string): string {
  return bridgeTaskTypeLabels[taskType] ?? taskType;
}

function trustPercent(proposal: BridgeProposal): number {
  return Math.round(proposal.trust_snapshot.score * 100);
}

function actionIcon(action: ProposalAction): string {
  switch (action.kind) {
    case "approve_task":
    case "approve_workflow_patch":
      return "Check";
    case "reject":
    case "reject_workflow_patch":
      return "X";
    case "open_diff":
      return "FileDiff";
    case "open_trace":
      return "Route";
    case "apply_ghost":
      return "WandSparkles";
    case "archive":
      return "Archive";
    default:
      return "Circle";
  }
}

export default function BridgeInbox({ isTauri }: BridgeInboxProps) {
  const proposals = useAppStore((state) => state.bridgeProposals);
  const loading = useAppStore((state) => state.bridgeLoading);
  const error = useAppStore((state) => state.bridgeError);
  const fetchBridgeProposals = useAppStore((state) => state.fetchBridgeProposals);
  const executeBridgeAction = useAppStore((state) => state.executeBridgeAction);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const pendingActionsRef = useRef(new Set<string>());
  const [pendingActions, setPendingActions] = useState<Record<string, boolean>>({});

  useEffect(() => {
    if (isTauri) {
      fetchBridgeProposals();
    }
  }, [fetchBridgeProposals, isTauri]);

  useEffect(() => {
    if (!isTauri) {
      return;
    }

    let disposed = false;
    let unlisten: (() => void) | null = null;

    import("@tauri-apps/api/event")
      .then(({ listen }) => listen("bridge-proposal-updated", () => fetchBridgeProposals()))
      .then((dispose) => {
        if (disposed) {
          dispose();
          return;
        }
        unlisten = dispose;
      })
      .catch(() => undefined);

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [fetchBridgeProposals, isTauri]);

  const groups = useMemo(() => {
    const pending = proposals.filter((proposal) => proposal.status === "pending");
    const resolved = proposals.filter((proposal) => resolvedStatuses.has(proposal.status));
    return { pending, resolved };
  }, [proposals]);

  const selected = proposals.find((proposal) => proposal.id === selectedId) ?? proposals[0] ?? null;
  const averageTrust = proposals.length
    ? Math.round(
        proposals.reduce((total, proposal) => total + proposal.trust_snapshot.score, 0) /
          proposals.length *
          100,
      )
    : 0;

  const runAction = async (proposalId: string, actionId: string) => {
    const key = actionKey(proposalId, actionId);
    if (pendingActionsRef.current.has(key)) {
      return;
    }
    pendingActionsRef.current.add(key);
    setPendingActions((current) => ({ ...current, [key]: true }));
    try {
      await executeBridgeAction(proposalId, actionId);
    } finally {
      pendingActionsRef.current.delete(key);
      setPendingActions((current) => {
        const next = { ...current };
        delete next[key];
        return next;
      });
    }
  };

  const isActionPending = (proposalId: string, actionId: string) =>
    Boolean(pendingActions[actionKey(proposalId, actionId)]);

  return (
    <section className="bridge-inbox">
      <BridgeHeader
        onRefresh={fetchBridgeProposals}
        pendingCount={groups.pending.length}
        resolvedCount={groups.resolved.length}
        averageTrust={averageTrust}
        disabled={!isTauri}
      />
      {!isTauri && proposals.length === 0 ? (
        <div className="bridge-empty">桥接审查需要 FeroHa 本地运行时。</div>
      ) : (
        <>
          {!isTauri && (
            <div className="bridge-error">浏览器预览使用本地模拟提案；真实审查需要 FeroHa 本地运行时。</div>
          )}
          {error && <div className="bridge-error">{error}</div>}
          {loading && proposals.length === 0 ? (
            <div className="bridge-empty">正在加载桥接提案...</div>
          ) : proposals.length === 0 ? (
            <div className="bridge-empty">暂无桥接提案</div>
          ) : (
            <div className="bridge-layout">
              <div className="bridge-list">
                <BridgeGroup
                  title="待审查"
                  items={groups.pending}
                  selectedId={selected?.id ?? null}
                  onSelect={setSelectedId}
                  onAction={runAction}
                  isActionPending={isActionPending}
                />
                <BridgeGroup
                  title="已处理"
                  items={groups.resolved}
                  selectedId={selected?.id ?? null}
                  onSelect={setSelectedId}
                  onAction={runAction}
                  isActionPending={isActionPending}
                />
              </div>
              <BridgeDetail proposal={selected} onAction={runAction} isActionPending={isActionPending} />
            </div>
          )}
        </>
      )}
      <style>{bridgeStyles}</style>
    </section>
  );
}

function BridgeHeader({
  onRefresh,
  pendingCount,
  resolvedCount,
  averageTrust,
  disabled,
}: {
  onRefresh: () => void;
  pendingCount: number;
  resolvedCount: number;
  averageTrust: number;
  disabled: boolean;
}) {
  return (
    <header className="bridge-header">
      <div className="bridge-header-title">
        <span className="bridge-header-mark">
          <FeroHaIcon name="Inbox" size={17} />
        </span>
      <div>
        <h3>桥接审查</h3>
          <p>Bridge Review 审查 AI 交付物进入人类面前的关键决定；先看风险、信任和动作，需要时再展开证据。</p>
      </div>
      </div>
      <div className="bridge-header-meta">
        <span className="bridge-summary-pill strong">待审查 {pendingCount}</span>
        <span className="bridge-summary-pill">已处理 {resolvedCount}</span>
        <span className="bridge-summary-pill">信任 {averageTrust}%</span>
        <button type="button" onClick={onRefresh} title="刷新桥接提案" aria-label="刷新桥接提案" disabled={disabled}>
          <FeroHaIcon name="RefreshCw" size={14} />
        </button>
      </div>
    </header>
  );
}

function BridgeGroup({
  title,
  items,
  selectedId,
  onSelect,
  onAction,
  isActionPending,
}: {
  title: string;
  items: BridgeProposal[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  onAction: (proposalId: string, actionId: string) => void;
  isActionPending: (proposalId: string, actionId: string) => boolean;
}) {
  return (
    <section className="bridge-group">
      <h4>{title} {items.length}</h4>
      {items.length === 0 ? (
        <div className="bridge-muted">暂无条目</div>
      ) : (
        items.map((proposal) => (
          <BridgeCard
            key={proposal.id}
            proposal={proposal}
            selected={proposal.id === selectedId}
            onSelect={() => onSelect(proposal.id)}
            onAction={onAction}
            isActionPending={isActionPending}
          />
        ))
      )}
    </section>
  );
}

function BridgeCard({
  proposal,
  selected,
  onSelect,
  onAction,
  isActionPending,
}: {
  proposal: BridgeProposal;
  selected: boolean;
  onSelect: () => void;
  onAction: (proposalId: string, actionId: string) => void;
  isActionPending: (proposalId: string, actionId: string) => boolean;
}) {
  return (
    <article className={`bridge-card ${selected ? "selected" : ""}`} onClick={onSelect}>
      <div className="bridge-card-main">
        <div className="bridge-card-title-row">
          <strong>{proposal.intent}</strong>
          <span className={`bridge-risk bridge-risk-${proposal.risk}`}>{bridgeRiskLabel(proposal.risk)}</span>
        </div>
        <div className="bridge-card-summary">{proposal.summary}</div>
        <div className="bridge-card-meta">
          <span>{bridgeSourceLabel(proposal.source)}</span>
          <span>{bridgeStatusLabel(proposal.status)}</span>
          <span>信任 {trustPercent(proposal)}%</span>
        </div>
      </div>
      <div className="bridge-card-actions" onClick={(event) => event.stopPropagation()}>
        {proposal.actions.slice(0, 2).map((action) => {
          const pending = isActionPending(proposal.id, action.id);
          return (
            <button
              key={action.id}
              type="button"
              onClick={() => onAction(proposal.id, action.id)}
              aria-label={action.label}
              aria-busy={pending}
              disabled={pending}
              title={action.label}
            >
              <FeroHaIcon name={actionIcon(action)} size={13} />
              <span>{action.label}</span>
            </button>
          );
        })}
      </div>
    </article>
  );
}

function BridgeDetail({
  proposal,
  onAction,
  isActionPending,
}: {
  proposal: BridgeProposal | null;
  onAction: (proposalId: string, actionId: string) => void;
  isActionPending: (proposalId: string, actionId: string) => boolean;
}) {
  if (!proposal) {
    return <aside className="bridge-detail empty">选择一个提案查看审查材料</aside>;
  }

  const taskReviewRows = [
    ["任务类型", proposal.task_type ? bridgeTaskTypeLabel(proposal.task_type) : undefined],
    ["沙箱", proposal.sandbox_summary],
    ["预期输出", proposal.expected_output],
    ["风险原因", proposal.risk_reason],
  ].filter((entry): entry is [string, string] => Boolean(entry[1]));
  const affectedNotes = proposal.impact.notes.length;

  return (
    <aside className="bridge-detail">
      <section className="bridge-decision-panel">
        <div className="bridge-decision-title">
          <span className={`bridge-risk bridge-risk-${proposal.risk}`}>{bridgeRiskLabel(proposal.risk)}</span>
          <span className="bridge-status-chip">{bridgeStatusLabel(proposal.status)}</span>
        </div>
        <h4>{proposal.intent}</h4>
        <p>{proposal.summary}</p>
        <div className="bridge-decision-meta">
          <span>{bridgeSourceLabel(proposal.source)}</span>
          <span>信任 {trustPercent(proposal)}%</span>
          <span>{affectedNotes > 0 ? `${affectedNotes} notes` : "no note writes"}</span>
        </div>
        <div className="bridge-detail-actions primary">
          {proposal.actions.map((action) => {
            const pending = isActionPending(proposal.id, action.id);
            return (
              <button
                key={action.id}
                type="button"
                onClick={() => onAction(proposal.id, action.id)}
                aria-busy={pending}
                disabled={pending}
              >
                <FeroHaIcon name={actionIcon(action)} size={14} />
                <span>{action.label}</span>
              </button>
            );
          })}
        </div>
      </section>
      {proposal.source === "scientist" && (
        <section className="bridge-kernel-review">
          <h5>LeanLite</h5>
          <div className="bridge-kernel-chip">命题一致性</div>
        </section>
      )}
      {taskReviewRows.length > 0 && (
        <details className="bridge-disclosure">
          <summary>任务合同与沙箱</summary>
          <div className="bridge-task-review-grid">
            {taskReviewRows.map(([label, value]) => (
              <div className="bridge-detail-row" key={label}>
                <span>{label}</span>
                <strong>{value}</strong>
              </div>
            ))}
          </div>
        </details>
      )}
      <details className="bridge-disclosure">
        <summary>影响与证据</summary>
        <section>
          <h5>影响</h5>
          {proposal.impact.notes.length > 0 ? (
            <div className="bridge-note-list">{proposal.impact.notes.join(", ")}</div>
          ) : (
            <div className="bridge-muted">暂无笔记影响</div>
          )}
        </section>
        <section>
          <h5>证据</h5>
          {proposal.evidence.length === 0 ? (
            <div className="bridge-muted">暂无附加证据</div>
          ) : (
            proposal.evidence.map((evidence) => (
              <div className="bridge-evidence" key={`${evidence.kind}-${evidence.ref}`}>
                <strong>{evidence.label}</strong>
                <span>{evidence.ref}</span>
                {evidence.excerpt && <small>{evidence.excerpt}</small>}
              </div>
            ))
          )}
        </section>
      </details>
      <details className="bridge-disclosure">
        <summary>来源细节</summary>
        <div className="bridge-detail-grid">
          <span>来源</span>
          <strong>{bridgeSourceLabel(proposal.source)}</strong>
          <span>状态</span>
          <strong>{bridgeStatusLabel(proposal.status)}</strong>
          <span>风险</span>
          <strong>{bridgeRiskLabel(proposal.risk)}</strong>
          <span>信任</span>
          <strong>信任 {trustPercent(proposal)}%</strong>
        </div>
      </details>
    </aside>
  );
}

const bridgeStyles = `
.bridge-inbox { height: 100%; min-height: 0; overflow: hidden; display: flex; flex-direction: column; background: var(--bg-primary); color: var(--text-primary); box-sizing: border-box; }
.bridge-header { display: flex; align-items: center; justify-content: space-between; gap: 14px; padding: 14px 18px; border-bottom: 1px solid var(--border-color); background: var(--bg-primary); min-width: 0; }
.bridge-header-title { display: flex; align-items: center; gap: 10px; min-width: 0; }
.bridge-header-title > div { min-width: 0; }
.bridge-header-mark { display: inline-flex; align-items: center; justify-content: center; width: 30px; height: 30px; border: 1px solid var(--border-color); border-radius: 6px; color: var(--accent-primary); background: var(--bg-secondary); flex: 0 0 auto; }
.bridge-header h3 { margin: 0; font-size: 15px; line-height: 1.25; }
.bridge-header p { margin: 2px 0 0; color: var(--text-muted); font-size: 11px; line-height: 1.35; max-width: 720px; overflow-wrap: anywhere; }
.bridge-header-meta { display: flex; align-items: center; justify-content: flex-end; gap: 7px; flex-wrap: wrap; flex: 0 1 420px; min-width: 210px; }
.bridge-summary-pill { display: inline-flex; align-items: center; min-height: 24px; padding: 2px 8px; border: 1px solid var(--border-muted); border-radius: 6px; background: var(--bg-secondary); color: var(--text-secondary); font-size: 11px; white-space: nowrap; }
.bridge-summary-pill.strong { color: var(--accent-primary); border-color: var(--accent-primary); background: var(--accent-glow); }
.bridge-header button, .bridge-card-actions button, .bridge-detail-actions button { display: inline-flex; align-items: center; justify-content: center; gap: 5px; border: 1px solid var(--control-border, var(--border-color)); background: var(--control-bg, var(--bg-input)); color: var(--text-primary); border-radius: 5px; padding: 5px 8px; cursor: pointer; min-height: 28px; max-width: 100%; transition: background 0.15s, border-color 0.15s, color 0.15s, transform 0.15s; font-size: 12px; line-height: 1; }
.bridge-card-actions button span, .bridge-detail-actions button span { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.bridge-header button:hover, .bridge-card-actions button:hover, .bridge-detail-actions button:hover { background: var(--bg-hover); border-color: var(--accent-primary); color: var(--accent-primary); }
.bridge-header button:disabled, .bridge-card-actions button:disabled, .bridge-detail-actions button:disabled { opacity: 0.55; cursor: default; transform: none; }
.bridge-layout { display: grid; grid-template-columns: minmax(300px, 34%) minmax(0, 1fr); min-height: 0; flex: 1; }
.bridge-list { min-height: 0; overflow: auto; padding: 14px; border-right: 1px solid var(--border-color); background: var(--bg-secondary); }
.bridge-group { margin-bottom: 18px; }
.bridge-group h4 { margin: 0 0 8px; color: var(--text-secondary); font-size: 12px; font-weight: 700; letter-spacing: 0; }
.bridge-card { display: grid; grid-template-columns: minmax(0, 1fr); gap: 10px; padding: 12px; border: 1px solid var(--border-color); border-left: 3px solid transparent; border-radius: 7px; background: var(--bg-primary); margin-bottom: 9px; cursor: pointer; transition: background 0.15s, border-color 0.15s, transform 0.15s; }
.bridge-card:hover { background: var(--bg-hover); transform: translateY(-1px); }
.bridge-card.selected { border-color: var(--accent-primary); border-left-color: var(--accent-primary); background: var(--bg-input); }
.bridge-card-main { min-width: 0; display: flex; flex-direction: column; gap: 5px; }
.bridge-card-title-row { display: flex; align-items: flex-start; gap: 8px; justify-content: space-between; }
.bridge-card-title-row strong { font-size: 13px; line-height: 1.35; overflow-wrap: anywhere; }
.bridge-card-summary { color: var(--text-secondary); font-size: 12px; line-height: 1.45; overflow-wrap: anywhere; }
.bridge-card-meta { display: flex; gap: 6px; flex-wrap: wrap; color: var(--text-muted); font-size: 11px; }
.bridge-card-meta span { border: 1px solid var(--border-muted); border-radius: 4px; padding: 1px 5px; background: var(--bg-secondary); }
.bridge-risk { display: inline-flex; align-items: center; width: fit-content; white-space: nowrap; font-size: 11px; padding: 2px 7px; border-radius: 999px; border: 1px solid var(--border-color); background: var(--bg-secondary); }
.bridge-risk-high { color: var(--status-error-color); border-color: color-mix(in srgb, var(--status-error-color) 42%, var(--border-color)); }
.bridge-risk-medium { color: var(--diff-warn); border-color: color-mix(in srgb, var(--diff-warn) 42%, var(--border-color)); }
.bridge-risk-low { color: var(--accent-primary); border-color: color-mix(in srgb, var(--accent-primary) 42%, var(--border-color)); }
.bridge-card-actions { display: flex; gap: 6px; flex-wrap: wrap; align-items: center; min-width: 0; }
.bridge-detail { min-width: 0; min-height: 0; overflow: auto; padding: 18px; display: flex; flex-direction: column; gap: 14px; background: linear-gradient(180deg, color-mix(in srgb, var(--bg-primary) 94%, var(--accent-primary)), var(--bg-primary) 180px); }
.bridge-detail h4, .bridge-detail h5 { margin: 0 0 6px; }
.bridge-detail h4 { font-size: 18px; line-height: 1.25; }
.bridge-detail p { margin: 0; color: var(--text-secondary); line-height: 1.55; }
.bridge-detail.empty, .bridge-empty, .bridge-muted { color: var(--text-muted); }
.bridge-empty { padding: 32px; text-align: center; }
.bridge-error { color: var(--status-error-color); border-bottom: 1px solid var(--border-color); padding: 8px 16px; font-size: 12px; }
.bridge-decision-panel { display: flex; flex-direction: column; align-items: flex-start; gap: 8px; padding: 14px; border: 1px solid var(--border-color); border-radius: 8px; background: var(--bg-secondary); box-shadow: 0 10px 26px rgba(0, 0, 0, 0.14); }
.bridge-decision-title { display: flex; flex-wrap: wrap; gap: 6px; align-items: center; }
.bridge-status-chip { display: inline-flex; align-items: center; width: fit-content; white-space: nowrap; font-size: 11px; padding: 2px 7px; border-radius: 999px; color: var(--text-secondary); border: 1px solid var(--border-color); background: var(--bg-primary); }
.bridge-decision-meta { display: flex; flex-wrap: wrap; gap: 6px; color: var(--text-muted); font-size: 11px; }
.bridge-decision-meta span { border: 1px solid var(--border-muted); border-radius: 4px; padding: 2px 6px; background: var(--bg-primary); }
.bridge-detail-actions.primary { margin-top: 2px; }
.bridge-detail-hero { display: flex; flex-direction: column; align-items: flex-start; gap: 6px; padding-bottom: 2px; }
.bridge-detail-grid { display: grid; grid-template-columns: max-content minmax(0, 1fr); gap: 7px 12px; font-size: 12px; padding: 12px; border: 1px solid var(--border-color); border-radius: 7px; background: var(--bg-secondary); }
.bridge-detail-grid span, .bridge-detail-row span { color: var(--text-muted); }
.bridge-detail-grid strong, .bridge-detail-row strong { min-width: 0; overflow-wrap: anywhere; }
.bridge-disclosure { border: 1px solid var(--border-color); border-radius: 7px; background: color-mix(in srgb, var(--bg-secondary) 70%, transparent); padding: 0; overflow: hidden; }
.bridge-disclosure summary { cursor: pointer; color: var(--text-secondary); font-size: 12px; font-weight: 700; padding: 10px 12px; list-style-position: inside; }
.bridge-disclosure[open] summary { border-bottom: 1px solid var(--border-color); color: var(--text-primary); }
.bridge-disclosure > section, .bridge-disclosure > .bridge-task-review-grid, .bridge-disclosure > .bridge-detail-grid { margin: 10px; }
.bridge-task-review-grid { display: flex; flex-direction: column; gap: 6px; padding: 10px; border: 1px solid var(--border-color); border-radius: 7px; background: var(--bg-secondary); }
.bridge-detail-row { display: grid; grid-template-columns: max-content 1fr; gap: 6px 12px; align-items: start; font-size: 12px; }
.bridge-kernel-review { display: flex; flex-direction: column; gap: 6px; }
.bridge-kernel-chip { display: inline-flex; width: fit-content; border: 1px solid var(--border-color); border-radius: 5px; padding: 4px 8px; color: var(--accent-primary); background: var(--bg-secondary); font-size: 12px; }
.bridge-note-list { color: var(--text-secondary); font-size: 12px; overflow-wrap: anywhere; }
.bridge-evidence { border: 1px solid var(--border-color); border-radius: 7px; padding: 9px; margin-bottom: 6px; background: var(--bg-secondary); display: flex; flex-direction: column; gap: 4px; font-size: 12px; }
.bridge-evidence span, .bridge-evidence small { color: var(--text-secondary); overflow-wrap: anywhere; }
.bridge-detail-actions { display: flex; gap: 7px; flex-wrap: wrap; min-width: 0; }
@media (max-width: 920px) { .bridge-layout { grid-template-columns: 1fr; } .bridge-list { border-right: 0; border-bottom: 1px solid var(--border-color); max-height: 42vh; } }
@media (max-width: 620px) { .bridge-header { align-items: flex-start; flex-direction: column; justify-content: flex-start; gap: 10px; padding: 12px; } .bridge-header-meta { justify-content: flex-start; flex: 0 1 auto; min-width: 0; width: 100%; } .bridge-list, .bridge-detail { padding: 12px; } .bridge-detail-grid, .bridge-detail-row { grid-template-columns: 1fr; } }
`;
