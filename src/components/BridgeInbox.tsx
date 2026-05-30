import { useEffect, useMemo, useRef, useState } from "react";
import { useAppStore } from "../hooks/useAppStore";
import type { BridgeProposal } from "../types/bridge-proposal";
import FeroHaIcon from "./FeroHaIcon";

interface BridgeInboxProps {
  isTauri: boolean;
}

const resolvedStatuses = new Set(["approved", "rejected", "applied", "archived"]);
const actionKey = (proposalId: string, actionId: string) => `${proposalId}:${actionId}`;

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

  if (!isTauri) {
    return (
      <section className="bridge-inbox">
        <BridgeHeader onRefresh={fetchBridgeProposals} />
        <div className="bridge-empty">Bridge Inbox requires the local FeroHa runtime.</div>
      </section>
    );
  }

  return (
    <section className="bridge-inbox">
      <BridgeHeader onRefresh={fetchBridgeProposals} />
      {error && <div className="bridge-error">{error}</div>}
      {loading && proposals.length === 0 ? (
        <div className="bridge-empty">Loading bridge proposals...</div>
      ) : proposals.length === 0 ? (
        <div className="bridge-empty">No bridge proposals</div>
      ) : (
        <div className="bridge-layout">
          <div className="bridge-list">
            <BridgeGroup
              title="Pending Review"
              items={groups.pending}
              selectedId={selected?.id ?? null}
              onSelect={setSelectedId}
              onAction={runAction}
              isActionPending={isActionPending}
            />
            <BridgeGroup
              title="Resolved"
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
      <style>{bridgeStyles}</style>
    </section>
  );
}

function BridgeHeader({ onRefresh }: { onRefresh: () => void }) {
  return (
    <header className="bridge-header">
      <h3>
        <FeroHaIcon name="Inbox" size={18} />
        Bridge Inbox
      </h3>
      <button type="button" onClick={onRefresh} title="Refresh bridge proposals" aria-label="Refresh bridge proposals">
        <FeroHaIcon name="RefreshCw" size={14} />
      </button>
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
      <h4>{title}</h4>
      {items.length === 0 ? (
        <div className="bridge-muted">No items</div>
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
          <span className={`bridge-risk bridge-risk-${proposal.risk}`}>{proposal.risk}</span>
        </div>
        <div className="bridge-card-summary">{proposal.summary}</div>
        <div className="bridge-card-meta">
          <span>{proposal.source}</span>
          <span>{proposal.status}</span>
          <span>trust {Math.round(proposal.trust_snapshot.score * 100)}%</span>
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
          >
            {action.label}
          </button>
        )})}
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
    return <aside className="bridge-detail empty">Select a proposal</aside>;
  }

  const taskReviewRows = [
    ["Task Type", proposal.task_type],
    ["Sandbox", proposal.sandbox_summary],
    ["Expected Output", proposal.expected_output],
    ["Risk Reason", proposal.risk_reason],
  ].filter((entry): entry is [string, string] => Boolean(entry[1]));

  return (
    <aside className="bridge-detail">
      <div>
        <h4>{proposal.intent}</h4>
        <p>{proposal.summary}</p>
      </div>
      <div className="bridge-detail-grid">
        <span>Source</span>
        <strong>{proposal.source}</strong>
        <span>Status</span>
        <strong>{proposal.status}</strong>
        <span>Risk</span>
        <strong>{proposal.risk}</strong>
        <span>Trust</span>
        <strong>{Math.round(proposal.trust_snapshot.score * 100)}%</strong>
      </div>
      {taskReviewRows.length > 0 && (
        <section className="bridge-task-review">
          <h5>Task Review</h5>
          <div className="bridge-detail-grid bridge-task-review-grid">
            {taskReviewRows.map(([label, value]) => (
              <div className="bridge-detail-row" key={label}>
                <span>{label}</span>
                <strong>{value}</strong>
              </div>
            ))}
          </div>
        </section>
      )}
      <section>
        <h5>Impact</h5>
        {proposal.impact.notes.length > 0 ? (
          <div className="bridge-note-list">{proposal.impact.notes.join(", ")}</div>
        ) : (
          <div className="bridge-muted">No note impact</div>
        )}
      </section>
      <section>
        <h5>Evidence</h5>
        {proposal.evidence.length === 0 ? (
          <div className="bridge-muted">No evidence attached</div>
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
      <div className="bridge-detail-actions">
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
            {action.label}
          </button>
        )})}
      </div>
    </aside>
  );
}

const bridgeStyles = `
.bridge-inbox { height: 100%; overflow: hidden; display: flex; flex-direction: column; background: var(--bg-primary); color: var(--text-primary); }
.bridge-header { display: flex; align-items: center; justify-content: space-between; padding: 12px 16px; border-bottom: 1px solid var(--border-color); }
.bridge-header h3 { display: flex; align-items: center; gap: 8px; margin: 0; font-size: 16px; }
.bridge-header button, .bridge-card-actions button, .bridge-detail-actions button { display: inline-flex; align-items: center; justify-content: center; border: 1px solid var(--border-color); background: var(--bg-input); color: var(--text-primary); border-radius: 4px; padding: 4px 8px; cursor: pointer; min-height: 26px; }
.bridge-layout { display: grid; grid-template-columns: minmax(320px, 44%) 1fr; min-height: 0; flex: 1; }
.bridge-list { overflow: auto; padding: 12px; border-right: 1px solid var(--border-color); }
.bridge-group { margin-bottom: 16px; }
.bridge-group h4 { margin: 0 0 8px; color: var(--text-secondary); font-size: 13px; }
.bridge-card { display: flex; gap: 10px; padding: 10px; border: 1px solid var(--border-color); border-radius: 6px; background: var(--bg-secondary); margin-bottom: 8px; cursor: pointer; }
.bridge-card.selected { border-color: var(--accent-primary); background: var(--bg-input); }
.bridge-card-main { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 4px; }
.bridge-card-title-row { display: flex; align-items: center; gap: 8px; justify-content: space-between; }
.bridge-card-title-row strong { font-size: 13px; overflow-wrap: anywhere; }
.bridge-card-summary { color: var(--text-secondary); font-size: 12px; line-height: 1.4; overflow-wrap: anywhere; }
.bridge-card-meta { display: flex; gap: 6px; flex-wrap: wrap; color: var(--text-muted); font-size: 11px; }
.bridge-risk { font-size: 10px; padding: 1px 6px; border-radius: 3px; border: 1px solid var(--border-color); }
.bridge-risk-high { color: var(--status-error-color); }
.bridge-risk-medium { color: var(--diff-warn); }
.bridge-risk-low { color: var(--accent-primary); }
.bridge-card-actions { display: flex; flex-direction: column; gap: 4px; flex: 0 0 auto; }
.bridge-detail { overflow: auto; padding: 16px; display: flex; flex-direction: column; gap: 12px; }
.bridge-detail h4, .bridge-detail h5 { margin: 0 0 6px; }
.bridge-detail p { margin: 0; color: var(--text-secondary); line-height: 1.5; }
.bridge-detail.empty, .bridge-empty, .bridge-muted { color: var(--text-muted); }
.bridge-empty { padding: 32px; text-align: center; }
.bridge-error { color: var(--status-error-color); border-bottom: 1px solid var(--border-color); padding: 8px 16px; font-size: 12px; }
.bridge-detail-grid { display: grid; grid-template-columns: max-content 1fr; gap: 6px 12px; font-size: 12px; }
.bridge-detail-grid span { color: var(--text-muted); }
.bridge-task-review-grid { display: flex; flex-direction: column; gap: 6px; }
.bridge-detail-row { display: grid; grid-template-columns: max-content 1fr; gap: 6px 12px; align-items: start; }
.bridge-detail-row strong { overflow-wrap: anywhere; }
.bridge-note-list { color: var(--text-secondary); font-size: 12px; overflow-wrap: anywhere; }
.bridge-evidence { border: 1px solid var(--border-color); border-radius: 4px; padding: 8px; margin-bottom: 6px; background: var(--bg-secondary); display: flex; flex-direction: column; gap: 4px; font-size: 12px; }
.bridge-evidence span, .bridge-evidence small { color: var(--text-secondary); overflow-wrap: anywhere; }
.bridge-detail-actions { display: flex; gap: 6px; flex-wrap: wrap; }
@media (max-width: 760px) { .bridge-layout { grid-template-columns: 1fr; } .bridge-list { border-right: 0; border-bottom: 1px solid var(--border-color); max-height: 52vh; } }
`;
