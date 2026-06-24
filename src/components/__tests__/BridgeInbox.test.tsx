import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import BridgeInbox, {
  bridgeRiskLabel,
  bridgeSourceLabel,
  bridgeStatusLabel,
  bridgeTaskTypeLabel,
} from "../BridgeInbox";
import type { BridgeProposal } from "../../types/bridge-proposal";
import { useAppStore } from "../../hooks/useAppStore";

const invokeMock = vi.hoisted(() => vi.fn());
const listenMock = vi.hoisted(() =>
  vi.fn(async (_eventName: string, _handler: () => Promise<void> | void) => vi.fn()),
);

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: listenMock,
}));

function proposal(overrides: Partial<BridgeProposal> = {}): BridgeProposal {
  return {
    id: "p1",
    source: "tool",
    source_ref: { kind: "task", id: "task_1" },
    intent: "批准研究任务",
    summary: "AI 准备执行一个需要确认的研究任务。",
    evidence: [],
    impact: {
      notes: ["Bayes.md"],
      creates_files: false,
      modifies_notes: false,
      exports_data: false,
      external_side_effect: false,
    },
    risk: "low",
    status: "pending",
    actions: [
      {
        id: "approve",
        label: "批准",
        kind: "approve_task",
        payload: { task_id: "task_1" },
      },
    ],
    trust_snapshot: {
      score: 0.7,
      acceptance_rate: 0.8,
      total_interactions: 5,
      recommended_mode: "manual",
    },
    created_at: 1,
    updated_at: 1,
    ...overrides,
  };
}

describe("BridgeInbox", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    listenMock.mockReset();
    listenMock.mockResolvedValue(vi.fn());
    useAppStore.setState({
      bridgeProposals: [],
      bridgeLoading: false,
      bridgeError: null,
      activePanel: "bridge",
      mode: "human",
    });
  });

  it("renders empty state after loading proposals", async () => {
    invokeMock.mockResolvedValueOnce([]);

    render(<BridgeInbox isTauri />);

    expect(await screen.findByText("暂无桥接提案")).toBeDefined();
    expect(invokeMock).toHaveBeenCalledWith("list_bridge_proposals", {});
  });

  it("explains that Bridge Review is a decision-first human review surface", () => {
    render(<BridgeInbox isTauri={false} />);

    expect(screen.getByText(/Bridge Review 审查 AI 交付物进入人类面前的关键决定/)).toBeDefined();
  });

  it("renders local preview proposals without Tauri so the mock review chain stays clickable", async () => {
    useAppStore.setState({
      bridgeProposals: [
        proposal({
          intent: "Preview proposal",
          summary: "Local simulation proposal",
          source: "scheduler",
          source_ref: { kind: "task", id: "sim-task-good" },
          actions: [
            {
              id: "open-diff",
              label: "Open Diff",
              kind: "open_diff",
              payload: { ghost_id: "sim-task-good" },
            },
          ],
        }),
      ],
    });
    invokeMock.mockRejectedValueOnce(new Error("Bridge store not initialized"));

    render(<BridgeInbox isTauri={false} />);

    expect(screen.getAllByText("Preview proposal").length).toBeGreaterThan(0);
    expect(screen.getByText(/浏览器预览使用本地模拟提案/)).toBeDefined();

    const buttons = screen.getAllByRole("button", { name: "Open Diff" });
    fireEvent.click(buttons[0]);

    await waitFor(() => {
      expect(useAppStore.getState().activePanel).toBe("diff");
      expect(useAppStore.getState().mode).toBe("human");
    });
  });

  it("groups pending and resolved proposals with readable human review copy", async () => {
    invokeMock.mockResolvedValueOnce([
      proposal(),
      proposal({
        id: "p2",
        intent: "归档梦境洞察",
        status: "archived",
        risk: "medium",
        source: "dream",
        source_ref: { kind: "dream_insight", id: "dream_1" },
      }),
    ]);

    render(<BridgeInbox isTauri />);

    expect(await screen.findByText("桥接审查")).toBeDefined();
    expect(screen.getAllByText("待审查 1").length).toBeGreaterThan(0);
    expect(screen.getAllByText("已处理 1").length).toBeGreaterThan(0);
    expect(screen.getAllByText("批准研究任务").length).toBeGreaterThan(0);
    expect(screen.getAllByText("归档梦境洞察").length).toBeGreaterThan(0);
    expect(screen.getAllByText("低风险").length).toBeGreaterThan(0);
    expect(screen.getAllByText("信任 70%").length).toBeGreaterThan(0);
  });

  it("shows typed task review metadata in the detail pane", async () => {
    invokeMock.mockResolvedValueOnce([
      proposal({
        task_type: "research",
        sandbox_summary: "tools: vector_search, web_search; network: allowlisted",
        expected_output: "research brief with sources",
        risk_reason: "medium risk because research may use network search",
        risk: "medium",
      }),
    ]);

    render(<BridgeInbox isTauri />);

    expect(await screen.findByText("任务类型")).toBeDefined();
    expect(screen.getByText("研究")).toBeDefined();
    expect(screen.getByText("沙箱")).toBeDefined();
    expect(screen.getByText("tools: vector_search, web_search; network: allowlisted")).toBeDefined();
    expect(screen.getByText("预期输出")).toBeDefined();
    expect(screen.getByText("research brief with sources")).toBeDefined();
    expect(screen.getByText("风险原因")).toBeDefined();
    expect(screen.getByText("medium risk because research may use network search")).toBeDefined();
  });

  it("labels scientist proposals as LeanLite proposition consistency", async () => {
    invokeMock.mockResolvedValueOnce([
      proposal({
        source: "scientist",
        source_ref: { kind: "scientist_output", id: "task_1" },
        evidence: [
          {
            label: "Proposition consistency",
            kind: "verification",
            ref: "task_1",
            excerpt: "kernel=PropositionKernel, claims=2, violations=1",
          },
        ],
      }),
    ]);

    render(<BridgeInbox isTauri />);

    expect(await screen.findByText("LeanLite")).toBeDefined();
    expect(screen.getByText("命题一致性")).toBeDefined();
    expect(screen.getAllByText("Proposition consistency").length).toBeGreaterThan(0);
    expect(screen.getByText("kernel=PropositionKernel, claims=2, violations=1")).toBeDefined();
  });

  it("executes proposal actions through the bridge command", async () => {
    invokeMock
      .mockResolvedValueOnce([proposal()])
      .mockResolvedValueOnce({ status: "success", proposal: proposal({ status: "approved" }) })
      .mockResolvedValueOnce([proposal({ status: "approved" })]);

    render(<BridgeInbox isTauri />);

    const buttons = await screen.findAllByRole("button", { name: "批准" });
    fireEvent.click(buttons[0]);

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("execute_bridge_action", {
        id: "p1",
        actionId: "approve",
      });
    });
  });

  it("refreshes proposals when the backend emits a bridge update event", async () => {
    let updateHandler: (() => Promise<void> | void) | null = null;
    listenMock.mockImplementation(async (eventName: string, handler: () => Promise<void> | void) => {
      if (eventName === "bridge-proposal-updated") {
        updateHandler = handler;
      }
      return vi.fn();
    });
    invokeMock
      .mockResolvedValueOnce([proposal()])
      .mockResolvedValueOnce([proposal({ id: "p2", intent: "Updated proposal" })]);

    render(<BridgeInbox isTauri />);

    expect((await screen.findAllByText("待审查 1")).length).toBeGreaterThan(0);
    await waitFor(() => {
      expect(updateHandler).not.toBeNull();
    });
    await act(async () => {
      await updateHandler?.();
    });

    expect((await screen.findAllByText("Updated proposal")).length).toBeGreaterThan(0);
    expect(invokeMock).toHaveBeenCalledTimes(2);
  });

  it("moves diff review navigation to the human face", async () => {
    invokeMock
      .mockResolvedValueOnce([
        proposal({
          source: "ghost",
          source_ref: { kind: "ghost", id: "ghost_1", path: "Target.md" },
          actions: [
            {
              id: "open-diff",
              label: "Open Diff",
              kind: "open_diff",
              payload: { ghost_id: "ghost_1" },
            },
          ],
        }),
      ])
      .mockResolvedValueOnce({
        status: "navigate",
        proposal: proposal(),
        metadata: {
          effect: "navigate",
          target_panel: "diff",
          target_id: "ghost_1",
        },
      })
      .mockResolvedValueOnce([]);

    render(<BridgeInbox isTauri />);

    const buttons = await screen.findAllByRole("button", { name: "Open Diff" });
    fireEvent.click(buttons[0]);

    await waitFor(() => {
      expect(useAppStore.getState().activePanel).toBe("diff");
      expect(useAppStore.getState().mode).toBe("human");
    });
  });

  it("falls back to local diff navigation when the bridge store is unavailable", async () => {
    invokeMock
      .mockResolvedValueOnce([
        proposal({
          source: "scheduler",
          source_ref: { kind: "task", id: "sim-task-good" },
          actions: [
            {
              id: "open-diff",
              label: "Open Diff",
              kind: "open_diff",
              payload: { ghost_id: "sim-task-good" },
            },
          ],
        }),
      ])
      .mockRejectedValueOnce(new Error("Bridge store not initialized"));

    render(<BridgeInbox isTauri />);

    const buttons = await screen.findAllByRole("button", { name: "Open Diff" });
    fireEvent.click(buttons[0]);

    await waitFor(() => {
      expect(useAppStore.getState().activePanel).toBe("diff");
      expect(useAppStore.getState().mode).toBe("human");
    });
  });

  it("prevents duplicate action execution while the first click is in flight", async () => {
    let resolveAction: (value: unknown) => void = () => undefined;
    const actionPromise = new Promise((resolve) => {
      resolveAction = resolve;
    });
    invokeMock
      .mockResolvedValueOnce([
        proposal({
          actions: [
            {
              id: "open-diff",
              label: "Open Diff",
              kind: "open_diff",
              payload: { ghost_id: "ghost_1" },
            },
          ],
        }),
      ])
      .mockReturnValueOnce(actionPromise)
      .mockResolvedValueOnce([]);

    render(<BridgeInbox isTauri />);

    const buttons = await screen.findAllByRole("button", { name: "Open Diff" });
    fireEvent.click(buttons[0]);
    await waitFor(() => {
      expect((buttons[0] as HTMLButtonElement).disabled).toBe(true);
    });
    fireEvent.click(buttons[0]);

    expect(invokeMock).toHaveBeenCalledTimes(2);
    expect(invokeMock).toHaveBeenCalledWith("execute_bridge_action", {
      id: "p1",
      actionId: "open-diff",
    });

    resolveAction({ status: "navigate", proposal: proposal(), metadata: { effect: "navigate" } });
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledTimes(3);
    });
  });
});

describe("BridgeInbox localized metadata", () => {
  it("localizes backend proposal enums for the human review surface", () => {
    expect(bridgeSourceLabel("tool")).toBe("工具");
    expect(bridgeStatusLabel("pending")).toBe("待审查");
    expect(bridgeRiskLabel("high")).toBe("高风险");
    expect(bridgeTaskTypeLabel("write_proposal")).toBe("写作提案");
    expect(bridgeTaskTypeLabel("mdt_index")).toBe("JSON-LD 索引（MDT 兼容）");
    expect(bridgeTaskTypeLabel("workflow_patch")).toBe("Workflow 补丁");
  });
});
