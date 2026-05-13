import { useState, useEffect } from "react";
import { useAppStore, DiffBlock } from "../hooks/useAppStore";

interface DiffViewProps {
  isTauri: boolean;
}

/**
 * DiffView — Visual comparison for AI-generated suggestions
 * Shows a GitHub-style split view with accept/reject per block.
 */
export default function DiffView({ isTauri }: DiffViewProps) {
  const [activeTab, setActiveTab] = useState<"pending" | "history">("pending");
  const diffBlocks = useAppStore((s) => s.diffBlocks);
  const setDiffBlocks = useAppStore((s) => s.setDiffBlocks);
  const updateDiffBlock = useAppStore((s) => s.updateDiffBlock);

  // Mock diff data for browser mode fallback
  const mockDiffBlocks: DiffBlock[] = [
    {
      id: "diff-1",
      type: "added",
      newText: "Rust's ownership system prevents data races at compile time by enforcing strict borrowing rules.",
      accepted: false,
      rejected: false,
    },
    {
      id: "diff-2",
      type: "modified",
      oldText: "Tauri is an Electron alternative.",
      newText: "Tauri is a lightweight framework for building desktop apps with web frontends and a Rust backend, consuming 10x less memory than Electron.",
      accepted: false,
      rejected: false,
    },
    {
      id: "diff-3",
      type: "removed",
      oldText: "This paragraph is redundant since the concept is already explained elsewhere.",
      accepted: false,
      rejected: false,
    },
  ];

  // Load diff data from backend when isTauri, otherwise use mock
  useEffect(() => {
    const loadDiffData = async () => {
      if (isTauri) {
        try {
          const { invoke } = await import("@tauri-apps/api/core");
          const blocks = await invoke<DiffBlock[]>("get_diff_blocks");
          setDiffBlocks(blocks);
        } catch (e) {
          console.error("Failed to load diff data:", e);
          setDiffBlocks(mockDiffBlocks);
        }
      } else {
        // Browser mode fallback
        setDiffBlocks(mockDiffBlocks);
      }
    };
    loadDiffData();
  }, [isTauri, setDiffBlocks]);

  const handleAccept = async (blockId: string) => {
    updateDiffBlock(blockId, { accepted: true });

    if (isTauri) {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        await invoke("accept_diff", { ghostId: "current", blockIds: [blockId] });
      } catch (e) {
        console.error(e);
      }
    }
  };

  const handleReject = async (blockId: string) => {
    updateDiffBlock(blockId, { rejected: true });

    if (isTauri) {
      try {
        const { invoke } = await import("@tauri-apps/api/core");
        await invoke("reject_diff", { ghostId: blockId });
      } catch (e) {
        console.error(e);
      }
    }
  };

  const pendingCount = diffBlocks.filter((b) => !b.accepted && !b.rejected).length;
  const acceptedCount = diffBlocks.filter((b) => b.accepted).length;
  const rejectedCount = diffBlocks.filter((b) => b.rejected).length;

  return (
    <div style={styles.container}>
      <div style={styles.header}>
        <div style={styles.tabs}>
          <button
            style={{ ...styles.tabBtn, ...(activeTab === "pending" ? styles.tabActive : {}) }}
            onClick={() => setActiveTab("pending")}
          >
            Pending ({pendingCount})
          </button>
          <button
            style={{ ...styles.tabBtn, ...(activeTab === "history" ? styles.tabActive : {}) }}
            onClick={() => setActiveTab("history")}
          >
            History ({acceptedCount + rejectedCount})
          </button>
        </div>
        <div style={styles.actions}>
          {pendingCount > 0 && (
            <>
              <button style={styles.acceptAllBtn} onClick={() => {
                diffBlocks.forEach((b) => { if (!b.accepted && !b.rejected) handleAccept(b.id); });
              }}>
                Accept All
              </button>
              <button style={styles.rejectAllBtn} onClick={() => {
                diffBlocks.forEach((b) => { if (!b.accepted && !b.rejected) handleReject(b.id); });
              }}>
                Reject All
              </button>
            </>
          )}
        </div>
      </div>

      <div style={styles.diffList}>
        {activeTab === "pending" &&
          diffBlocks
            .filter((b) => !b.accepted && !b.rejected)
            .map((block) => (
              <DiffBlockCard
                key={block.id}
                block={block}
                onAccept={() => handleAccept(block.id)}
                onReject={() => handleReject(block.id)}
              />
            ))}

        {activeTab === "history" &&
          diffBlocks
            .filter((b) => b.accepted || b.rejected)
            .map((block) => (
              <DiffBlockCard
                key={block.id}
                block={block}
                readOnly
              />
            ))}

        {activeTab === "pending" && pendingCount === 0 && (
          <div style={styles.empty}>
            <span style={styles.emptyIcon}>✓</span>
            <p>No pending suggestions</p>
            <p style={styles.emptyHint}>
              Run an AI command like <code>/agent deep-dive [[Concept]]</code> to generate suggestions
            </p>
          </div>
        )}

        {activeTab === "history" && acceptedCount + rejectedCount === 0 && (
          <div style={styles.empty}>
            <p>No review history yet</p>
          </div>
        )}
      </div>
    </div>
  );
}

function DiffBlockCard({
  block,
  onAccept,
  onReject,
  readOnly,
}: {
  block: DiffBlock;
  onAccept?: () => void;
  onReject?: () => void;
  readOnly?: boolean;
}) {
  const isAccepted = block.accepted;
  const isRejected = block.rejected;

  return (
    <div
      style={{
        ...styles.blockCard,
        ...(isAccepted ? styles.blockAccepted : {}),
        ...(isRejected ? styles.blockRejected : {}),
      }}
    >
      <div style={styles.blockHeader}>
        <span style={styles.blockType}>
          {block.type === "added" && "+ Added"}
          {block.type === "removed" && "- Removed"}
          {block.type === "modified" && "~ Modified"}
        </span>
        {isAccepted && <span style={styles.acceptedLabel}>Accepted ✓</span>}
        {isRejected && <span style={styles.rejectedLabel}>Rejected ✗</span>}
      </div>

      {block.oldText && block.type === "modified" && (
        <div style={styles.oldText}>
          <span style={styles.oldLabel}>— Old:</span>
          <div style={styles.textContent}>{block.oldText}</div>
        </div>
      )}

      {block.oldText && block.type === "removed" && (
        <div style={styles.removedText}>
          <span style={styles.removedLabel}>— Removing:</span>
          <div style={styles.textContent}>{block.oldText}</div>
        </div>
      )}

      {block.newText && (block.type === "added" || block.type === "modified") && (
        <div style={styles.newText}>
          <span style={styles.newLabel}>+ New:</span>
          <div style={styles.textContent}>{block.newText}</div>
        </div>
      )}

      {!readOnly && (
        <div style={styles.blockActions}>
          <button style={styles.acceptBtn} onClick={onAccept}>Accept</button>
          <button style={styles.rejectBtn} onClick={onReject}>Reject</button>
        </div>
      )}
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  container: {
    display: "flex",
    flexDirection: "column",
    height: "100%",
  },
  header: {
    display: "flex",
    justifyContent: "space-between",
    alignItems: "center",
    padding: "6px 0",
    borderBottom: "1px solid #313244",
    marginBottom: "16px",
  },
  tabs: {
    display: "flex",
    gap: "4px",
  },
  tabBtn: {
    padding: "3px 12px",
    background: "transparent",
    color: "#6c7086",
    border: "none",
    borderRadius: "4px",
    cursor: "pointer",
    fontSize: "12px",
  },
  tabActive: {
    backgroundColor: "#313244",
    color: "#cdd6f4",
  },
  actions: {
    display: "flex",
    gap: "8px",
  },
  acceptAllBtn: {
    padding: "4px 12px",
    backgroundColor: "#a6e3a1",
    color: "#1e1e2e",
    border: "none",
    borderRadius: "4px",
    cursor: "pointer",
    fontSize: "11px",
    fontWeight: 600,
  },
  rejectAllBtn: {
    padding: "4px 12px",
    backgroundColor: "#f38ba8",
    color: "#1e1e2e",
    border: "none",
    borderRadius: "4px",
    cursor: "pointer",
    fontSize: "11px",
    fontWeight: 600,
  },
  diffList: {
    flex: 1,
    overflow: "auto",
    display: "flex",
    flexDirection: "column",
    gap: "12px",
  },
  blockCard: {
    border: "1px solid #313244",
    borderRadius: "8px",
    padding: "12px",
    backgroundColor: "#181825",
  },
  blockAccepted: {
    borderColor: "#a6e3a1",
    opacity: 0.7,
  },
  blockRejected: {
    borderColor: "#f38ba8",
    opacity: 0.5,
  },
  blockHeader: {
    display: "flex",
    justifyContent: "space-between",
    alignItems: "center",
    marginBottom: "8px",
  },
  blockType: {
    fontSize: "11px",
    fontWeight: 600,
    color: "#cba6f7",
    textTransform: "uppercase" as const,
    letterSpacing: "0.5px",
  },
  acceptedLabel: {
    fontSize: "11px",
    color: "#a6e3a1",
  },
  rejectedLabel: {
    fontSize: "11px",
    color: "#f38ba8",
  },
  oldText: {
    marginBottom: "8px",
  },
  removedText: {
    marginBottom: "8px",
  },
  newText: {
    marginBottom: "8px",
  },
  oldLabel: {
    fontSize: "11px",
    color: "#f38ba8",
    marginBottom: "4px",
    display: "block",
  },
  newLabel: {
    fontSize: "11px",
    color: "#a6e3a1",
    marginBottom: "4px",
    display: "block",
  },
  removedLabel: {
    fontSize: "11px",
    color: "#f38ba8",
    marginBottom: "4px",
    display: "block",
  },
  textContent: {
    fontSize: "13px",
    color: "#cdd6f4",
    lineHeight: "1.6",
    padding: "8px",
    backgroundColor: "#11111b",
    borderRadius: "4px",
    whiteSpace: "pre-wrap" as const,
  },
  blockActions: {
    display: "flex",
    gap: "8px",
    marginTop: "8px",
  },
  acceptBtn: {
    padding: "4px 16px",
    backgroundColor: "#a6e3a1",
    color: "#1e1e2e",
    border: "none",
    borderRadius: "4px",
    cursor: "pointer",
    fontSize: "12px",
    fontWeight: 600,
  },
  rejectBtn: {
    padding: "4px 16px",
    backgroundColor: "#45475a",
    color: "#cdd6f4",
    border: "none",
    borderRadius: "4px",
    cursor: "pointer",
    fontSize: "12px",
  },
  empty: {
    textAlign: "center" as const,
    padding: "60px 20px",
    color: "#6c7086",
  },
  emptyIcon: {
    fontSize: "36px",
    color: "#a6e3a1",
    display: "block",
    marginBottom: "12px",
  },
  emptyHint: {
    fontSize: "12px",
    marginTop: "8px",
    color: "#585b70",
  },
};
