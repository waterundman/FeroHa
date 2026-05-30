import { useState, useEffect, useCallback } from "react";
import { useAppStore, DiffBlock } from "../hooks/useAppStore";
import FeroHaIcon from "./FeroHaIcon";

interface DiffViewProps {
  isTauri: boolean;
}

const MOCK_DIFF_BLOCKS: DiffBlock[] = [
  {
    ghostId: "mock",
    id: "diff-1",
    type: "inserted",
    newText: "Rust's ownership system prevents data races at compile time by enforcing strict borrowing rules.",
    accepted: false,
    rejected: false,
  },
  {
    ghostId: "mock",
    id: "diff-2",
    type: "modified",
    oldText: "Tauri is an Electron alternative.",
    newText: "Tauri is a lightweight framework for building desktop apps with web frontends and a Rust backend, consuming 10x less memory than Electron.",
    accepted: false,
    rejected: false,
  },
  {
    ghostId: "mock",
    id: "diff-3",
    type: "deleted",
    oldText: "This paragraph is redundant since the concept is already explained elsewhere.",
    accepted: false,
    rejected: false,
  },
];

type ViewMode = "unified" | "side-by-side";

function getLineColor(type: DiffBlock["type"], isOld: boolean): string | null {
  if (type === "inserted") return "var(--diff-insert)";
  if (type === "deleted") return "var(--diff-delete)";
  if (type === "modified") {
    return isOld ? "var(--diff-delete)" : "var(--diff-insert)";
  }
  return null;
}

function getLineBg(type: DiffBlock["type"], isOld: boolean): string | null {
  const color = getLineColor(type, isOld);
  if (!color) return null;
  return `${color}15`;
}

function getLinePrefix(type: DiffBlock["type"], isOld: boolean): string {
  if (type === "inserted") return "+";
  if (type === "deleted") return "-";
  if (type === "modified") return isOld ? "-" : "+";
  return " ";
}

export default function DiffView({ isTauri }: DiffViewProps) {
  const [activeTab, setActiveTab] = useState<"pending" | "history">("pending");
  const [viewMode, setViewMode] = useState<ViewMode>("unified");
  const [focusedIndex, setFocusedIndex] = useState(-1);
  const diffBlocks = useAppStore((s) => s.diffBlocks);
  const setDiffBlocks = useAppStore((s) => s.setDiffBlocks);
  const updateDiffBlock = useAppStore((s) => s.updateDiffBlock);

  useEffect(() => {
    const loadDiffData = async () => {
      if (isTauri) {
        try {
          const { invoke } = await import("@tauri-apps/api/core");
          const blocks = await invoke<DiffBlock[]>("get_diff_blocks");
          setDiffBlocks(blocks);
        } catch (e) {
          console.error("Failed to load diff data:", e);
          setDiffBlocks(MOCK_DIFF_BLOCKS);
        }
      } else {
        setDiffBlocks(MOCK_DIFF_BLOCKS);
      }
    };
    loadDiffData();
  }, [isTauri, setDiffBlocks]);

  const handleAccept = useCallback(async (block: DiffBlock) => {
    updateDiffBlock(block.id, { accepted: true, rejected: false });

    if (isTauri) {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        await invoke("accept_diff", { ghostId: block.ghostId, blockIds: [block.id] });
      } catch (e) {
        console.error(e);
      }
    }
  }, [isTauri, updateDiffBlock]);

  const handleReject = useCallback(async (block: DiffBlock) => {
    updateDiffBlock(block.id, { rejected: true, accepted: false });

    if (isTauri) {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        await invoke("reject_diff", { ghostId: block.ghostId, blockIds: [block.id] });
      } catch (e) {
        console.error(e);
      }
    }
  }, [isTauri, updateDiffBlock]);

  const pendingBlocks = diffBlocks.filter((b) => !b.accepted && !b.rejected);
  const historyBlocks = diffBlocks.filter((b) => b.accepted || b.rejected);
  const pendingCount = pendingBlocks.length;
  const acceptedCount = diffBlocks.filter((b) => b.accepted).length;
  const rejectedCount = diffBlocks.filter((b) => b.rejected).length;
  const visibleBlocks = activeTab === "pending" ? pendingBlocks : historyBlocks;

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;

      const isMod = e.metaKey || e.ctrlKey;
      if (isMod) return;

      if (e.key === "ArrowLeft" && !e.shiftKey) {
        e.preventDefault();
        setFocusedIndex((prev) => Math.max(-1, prev - 1));
      } else if (e.key === "ArrowRight" && !e.shiftKey) {
        e.preventDefault();
        setFocusedIndex((prev) => Math.min(visibleBlocks.length - 1, prev + 1));
      }

      if (e.shiftKey) {
        if (e.key === "A" || e.key === "a") {
          e.preventDefault();
          pendingBlocks.forEach((b) => { handleAccept(b); });
        } else if (e.key === "R" || e.key === "r") {
          e.preventDefault();
          pendingBlocks.forEach((b) => { handleReject(b); });
        }
        return;
      }

      if (activeTab === "pending") {
        if (e.key === "a" || e.key === "Enter") {
          e.preventDefault();
          const idx = focusedIndex >= 0 && focusedIndex < pendingBlocks.length
            ? focusedIndex : 0;
          if (pendingBlocks[idx]) handleAccept(pendingBlocks[idx]);
        } else if (e.key === "r") {
          e.preventDefault();
          const idx = focusedIndex >= 0 && focusedIndex < pendingBlocks.length
            ? focusedIndex : 0;
          if (pendingBlocks[idx]) handleReject(pendingBlocks[idx]);
        }
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [activeTab, pendingBlocks, focusedIndex, visibleBlocks.length, handleAccept, handleReject]);

  return (
    <div className="diff-view-container">
      <style>{diffViewCSS}</style>
      <div className="diff-header">
        <div className="diff-tabs">
          <button
            className={`diff-tab-btn ${activeTab === "pending" ? "diff-tab-active" : ""}`}
            onClick={() => setActiveTab("pending")}
          >
            Pending {pendingCount > 0 && <span className="diff-count-badge">{pendingCount}</span>}
          </button>
          <button
            className={`diff-tab-btn ${activeTab === "history" ? "diff-tab-active" : ""}`}
            onClick={() => setActiveTab("history")}
          >
            History {acceptedCount + rejectedCount > 0 && <span className="diff-count-badge">{acceptedCount + rejectedCount}</span>}
          </button>
        </div>
        <div className="diff-header-right">
          <div className="diff-view-toggle">
            <button
              className={`diff-view-btn ${viewMode === "unified" ? "diff-view-active" : ""}`}
              onClick={() => setViewMode("unified")}
              title="Unified view"
            >
              <FeroHaIcon name="AlignJustify" size={14} />
            </button>
            <button
              className={`diff-view-btn ${viewMode === "side-by-side" ? "diff-view-active" : ""}`}
              onClick={() => setViewMode("side-by-side")}
              title="Side-by-side view"
            >
              <FeroHaIcon name="Columns2" size={14} />
            </button>
          </div>
          {pendingCount > 0 && (
            <div className="diff-bulk-actions">
              <button className="diff-accept-all-btn" onClick={() => {
                pendingBlocks.forEach((b) => { handleAccept(b); });
              }}>
                <FeroHaIcon name="Check" size={12} /> Accept All
              </button>
              <button className="diff-reject-all-btn" onClick={() => {
                pendingBlocks.forEach((b) => { handleReject(b); });
              }}>
                <FeroHaIcon name="X" size={12} /> Reject All
              </button>
            </div>
          )}
        </div>
      </div>

      <div className="diff-list">
        {activeTab === "pending" && pendingCount === 0 && (
          <div className="diff-empty">
            <FeroHaIcon name="CheckCircle2" size={36} />
            <p className="diff-empty-title">No pending suggestions</p>
            <p className="diff-empty-hint">
              Use <code>/agent</code> commands or instruction cards to generate AI suggestions
            </p>
          </div>
        )}

        {activeTab === "history" && historyBlocks.length === 0 && (
          <div className="diff-empty">
            <FeroHaIcon name="List" size={36} />
            <p className="diff-empty-title">No review history</p>
          </div>
        )}

        {diffBlocks.length === 0 && activeTab === "pending" && !isTauri && (
          <div className="diff-empty">
            <FeroHaIcon name="Search" size={36} />
            <p className="diff-empty-title">Diff panel displays AI modification suggestions</p>
            <p className="diff-empty-hint">
              Accept or reject suggestions to incorporate them into notes
            </p>
          </div>
        )}

        {visibleBlocks.map((block, index) => (
          <DiffBlockCard
            key={block.id}
            block={block}
            viewMode={viewMode}
            isFocused={index === focusedIndex}
            index={index}
            onAccept={activeTab === "pending" ? () => handleAccept(block) : undefined}
            onReject={activeTab === "pending" ? () => handleReject(block) : undefined}
            onClick={() => setFocusedIndex(index)}
            readOnly={activeTab === "history"}
          />
        ))}
      </div>
    </div>
  );
}

function DiffBlockCard({
  block,
  viewMode,
  isFocused,
  index,
  onAccept,
  onReject,
  onClick,
  readOnly,
}: {
  block: DiffBlock;
  viewMode: ViewMode;
  isFocused: boolean;
  index: number;
  onAccept?: () => void;
  onReject?: () => void;
  onClick?: () => void;
  readOnly?: boolean;
}) {
  const isAccepted = block.accepted;
  const isRejected = block.rejected;
  const oldLines = (block.oldText ?? "").split("\n");
  const newLines = (block.newText ?? "").split("\n");

  const typeLabel = block.type === "inserted" ? "Inserted"
    : block.type === "deleted" ? "Deleted"
    : "Modified";
  const typeColor = block.type === "inserted" ? "var(--diff-insert)"
    : block.type === "deleted" ? "var(--diff-delete)"
    : "var(--diff-modify)";

  const cardClasses = [
    "diff-block-card",
    isFocused ? "diff-block-focused" : "",
    isAccepted ? "diff-block-accepted" : "",
    isRejected ? "diff-block-rejected" : "",
    readOnly ? "diff-block-readonly" : "",
  ].filter(Boolean).join(" ");

  const renderLines = (lines: string[], isOld: boolean, prefixOverride?: string) => {
    return lines.map((line, i) => {
      const bg = getLineBg(block.type, isOld);
      const prefix = prefixOverride ?? getLinePrefix(block.type, isOld);
      return (
        <div
          key={i}
          className="diff-line"
          style={{ backgroundColor: bg ?? undefined }}
        >
          <span className="diff-line-number">{prefix}{i + 1}</span>
          <span className="diff-line-content">{line || " "}</span>
        </div>
      );
    });
  };

  let content: React.ReactNode;
  if (viewMode === "side-by-side" && (block.type === "modified" || (block.oldText && block.newText))) {
    content = (
      <div className="diff-side-by-side">
        <div className="diff-side-left">
          <div className="diff-side-label">Old</div>
          {renderLines(oldLines, true)}
        </div>
        <div className="diff-side-right">
          <div className="diff-side-label">New</div>
          {renderLines(newLines, false)}
        </div>
      </div>
    );
  } else if (viewMode === "side-by-side" && block.type === "inserted") {
    content = (
      <div className="diff-side-by-side">
        <div className="diff-side-left diff-side-empty">
          <div className="diff-side-label">&nbsp;</div>
        </div>
        <div className="diff-side-right">
          <div className="diff-side-label">New</div>
          {renderLines(newLines, false)}
        </div>
      </div>
    );
  } else if (viewMode === "side-by-side" && block.type === "deleted") {
    content = (
      <div className="diff-side-by-side">
        <div className="diff-side-left">
          <div className="diff-side-label">Old</div>
          {renderLines(oldLines, true)}
        </div>
        <div className="diff-side-right diff-side-empty">
          <div className="diff-side-label">&nbsp;</div>
        </div>
      </div>
    );
  } else {
    content = (
      <div className="diff-unified">
        {block.oldText && (block.type === "deleted" || block.type === "modified") && (
          <div className="diff-section">
            <div className="diff-section-label" style={{ color: "var(--diff-delete)" }}>
              — Removing:
            </div>
            {renderLines(oldLines, true, (block.type === "modified") ? "-" : "-")}
          </div>
        )}
        {block.newText && (block.type === "inserted" || block.type === "modified") && (
          <div className="diff-section">
            <div className="diff-section-label" style={{ color: "var(--diff-insert)" }}>
              + Adding:
            </div>
            {renderLines(newLines, false, (block.type === "modified") ? "+" : "+")}
          </div>
        )}
      </div>
    );
  }

  return (
    <div
      className={cardClasses}
      style={{ animationDelay: `${index * 60}ms` }}
      onClick={onClick}
    >
      <div className="diff-card-header">
        <div className="diff-card-header-left">
          <span className="diff-ghost-id" title={block.ghostId}>
            {block.ghostId.length > 12 ? block.ghostId.slice(0, 12) + "…" : block.ghostId}
          </span>
          <span className="diff-type-tag" style={{ color: typeColor }}>
            <span className="diff-type-dot" style={{ backgroundColor: typeColor }} />
            {typeLabel}
          </span>
        </div>
        <div className="diff-card-header-right">
          {isAccepted && (
            <span className="diff-status-label diff-status-accepted">
              <FeroHaIcon name="Check" size={10} /> Accepted
            </span>
          )}
          {isRejected && (
            <span className="diff-status-label diff-status-rejected">
              <FeroHaIcon name="X" size={10} /> Rejected
            </span>
          )}
        </div>
      </div>

      {content}

      {!readOnly && (
        <div className="diff-card-actions">
          <button className="diff-accept-btn" onClick={(e) => { e.stopPropagation(); onAccept?.(); }}>
            <FeroHaIcon name="Check" size={14} /> Accept
          </button>
          <button className="diff-reject-btn" onClick={(e) => { e.stopPropagation(); onReject?.(); }}>
            <FeroHaIcon name="X" size={14} /> Reject
          </button>
        </div>
      )}
    </div>
  );
}

const diffViewCSS = `
@keyframes diffCardIn {
  from { opacity: 0; transform: translateY(8px); }
  to { opacity: 1; transform: translateY(0); }
}

.diff-view-container {
  display: flex;
  flex-direction: column;
  height: 100%;
}

/* Header */
.diff-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 6px 0;
  border-bottom: 1px solid var(--border-color);
  margin-bottom: 16px;
  flex-wrap: wrap;
  gap: 8px;
}

.diff-header-right {
  display: flex;
  align-items: center;
  gap: 8px;
}

/* Tabs */
.diff-tabs {
  display: flex;
  gap: 4px;
}

.diff-tab-btn {
  padding: 3px 12px;
  background: transparent;
  color: var(--text-muted);
  border: none;
  border-radius: 4px;
  cursor: pointer;
  font-size: 12px;
  transition: all var(--transition-fast, 200ms) var(--easing-smooth, ease);
  display: inline-flex;
  align-items: center;
  gap: 6px;
}

.diff-tab-btn:hover {
  color: var(--text-primary);
}

.diff-tab-active {
  background-color: var(--bg-input);
  color: var(--text-primary);
}

/* Count Badge */
.diff-count-badge {
  background: #89b4fa;
  color: #1e1e2e;
  border-radius: 10px;
  padding: 1px 8px;
  font-size: 10px;
  font-weight: 700;
  min-width: 8px;
  text-align: center;
}

/* View Mode Toggle */
.diff-view-toggle {
  display: flex;
  border: 1px solid var(--border-color);
  border-radius: 4px;
  overflow: hidden;
}

.diff-view-btn {
  padding: 4px 8px;
  background: transparent;
  border: none;
  cursor: pointer;
  color: var(--text-muted);
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all var(--transition-fast, 200ms) var(--easing-smooth, ease);
}

.diff-view-btn:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.diff-view-active {
  background: var(--bg-input);
  color: var(--text-primary);
}

/* Bulk Actions */
.diff-bulk-actions {
  display: flex;
  gap: 6px;
}

.diff-accept-all-btn,
.diff-reject-all-btn {
  padding: 4px 12px;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  font-size: 11px;
  font-weight: 600;
  display: inline-flex;
  align-items: center;
  gap: 4px;
  transition: transform var(--transition-fast, 200ms) var(--easing-smooth, ease);
}

.diff-accept-all-btn:hover,
.diff-reject-all-btn:hover {
  transform: scale(1.03);
}

.diff-accept-all-btn {
  background-color: var(--diff-insert);
  color: var(--bg-primary);
}

.diff-reject-all-btn {
  background-color: var(--diff-delete);
  color: var(--bg-primary);
}

/* Diff List */
.diff-list {
  flex: 1;
  overflow: auto;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

/* Diff Card */
.diff-block-card {
  border: 1px solid var(--border-color);
  border-radius: 8px;
  padding: 12px;
  background-color: var(--bg-secondary);
  cursor: default;
  animation: diffCardIn 300ms var(--easing-smooth, ease) both;
  transition: border-color var(--transition-fast, 200ms) var(--easing-smooth, ease),
              box-shadow var(--transition-fast, 200ms) var(--easing-smooth, ease);
}

.diff-block-focused {
  border-color: var(--accent-primary);
  box-shadow: 0 0 0 1px var(--accent-primary);
}

.diff-block-accepted {
  border-left: 3px solid var(--diff-insert);
  opacity: 0.65;
}

.diff-block-rejected {
  border-left: 3px solid var(--diff-delete);
  opacity: 0.55;
}

.diff-block-rejected .diff-line-content {
  text-decoration: line-through;
}

.diff-block-readonly {
  pointer-events: none;
}

/* Card Header */
.diff-card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 10px;
}

.diff-card-header-left {
  display: flex;
  align-items: center;
  gap: 10px;
}

.diff-card-header-right {
  display: flex;
  align-items: center;
  gap: 6px;
}

.diff-ghost-id {
  font-size: 10px;
  color: var(--text-muted);
  font-family: var(--font-mono, monospace);
  opacity: 0.7;
}

.diff-type-tag {
  font-size: 11px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  display: inline-flex;
  align-items: center;
  gap: 5px;
}

.diff-type-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  display: inline-block;
}

.diff-status-label {
  font-size: 10px;
  font-weight: 600;
  display: inline-flex;
  align-items: center;
  gap: 3px;
}

.diff-status-accepted {
  color: var(--diff-insert);
}

.diff-status-rejected {
  color: var(--diff-delete);
}

/* Diff Lines */
.diff-line {
  display: flex;
  align-items: flex-start;
  font-family: var(--font-mono, monospace);
  font-size: 12px;
  line-height: 1.7;
  min-height: 20px;
}

.diff-line-number {
  width: 36px;
  min-width: 36px;
  text-align: right;
  padding-right: 8px;
  color: var(--text-muted);
  font-size: 11px;
  user-select: none;
  opacity: 0.6;
}

.diff-line-content {
  flex: 1;
  padding: 0 4px;
  white-space: pre-wrap;
  word-break: break-word;
  color: var(--text-primary);
}

.diff-section {
  margin-bottom: 8px;
}

.diff-section:last-child {
  margin-bottom: 0;
}

.diff-section-label {
  font-size: 11px;
  font-weight: 600;
  margin-bottom: 4px;
}

/* Unified View */
.diff-unified {
  border-radius: 4px;
  overflow: hidden;
}

/* Side-by-Side View */
.diff-side-by-side {
  display: flex;
  gap: 1px;
  border-radius: 4px;
  overflow: hidden;
  border: 1px solid var(--border-color);
}

.diff-side-left,
.diff-side-right {
  flex: 1;
  min-width: 0;
  overflow: hidden;
}

.diff-side-left {
  border-right: 1px solid var(--border-color);
}

.diff-side-empty {
  background: var(--bg-input);
  opacity: 0.3;
}

.diff-side-label {
  font-size: 10px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.5px;
  color: var(--text-muted);
  padding: 2px 8px;
  background: var(--bg-input);
  border-bottom: 1px solid var(--border-color);
}

/* Card Actions */
.diff-card-actions {
  display: flex;
  gap: 8px;
  margin-top: 10px;
  padding-top: 10px;
  border-top: 1px solid var(--border-muted, rgba(15, 45, 34, 0.4));
}

.diff-accept-btn,
.diff-reject-btn {
  padding: 5px 16px;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  font-size: 12px;
  font-weight: 600;
  display: inline-flex;
  align-items: center;
  gap: 5px;
  transition: transform var(--transition-fast, 200ms) var(--easing-smooth, ease),
              opacity var(--transition-fast, 200ms) var(--easing-smooth, ease);
  pointer-events: auto;
}

.diff-accept-btn:hover,
.diff-reject-btn:hover {
  transform: scale(1.03);
}

.diff-accept-btn {
  background-color: var(--diff-insert);
  color: var(--bg-primary);
}

.diff-reject-btn {
  background-color: var(--bg-hover);
  color: var(--text-primary);
  border: 1px solid var(--border-color);
}

.diff-accept-btn:active,
.diff-reject-btn:active {
  transform: scale(0.97);
}

/* Empty State */
.diff-empty {
  text-align: center;
  padding: 60px 20px;
  color: var(--text-muted);
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 8px;
}

.diff-empty-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-secondary);
  margin: 0;
}

.diff-empty-hint {
  font-size: 12px;
  color: var(--text-muted);
  opacity: 0.7;
  margin: 0;
}

.diff-empty code {
  background: var(--bg-input);
  padding: 1px 6px;
  border-radius: 3px;
  font-family: var(--font-mono, monospace);
  font-size: 11px;
}
`;
