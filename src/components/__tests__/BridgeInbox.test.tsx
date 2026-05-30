import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import BridgeInbox from "../BridgeInbox";
import type { BridgeProposal } from "../../types/bridge-proposal";
import { useAppStore } from "../../hooks/useAppStore";

const invokeMock = vi.hoisted(() => vi.fn());
const listenMock = vi.hoisted(() =>
  vi.fn(async (_eventName: string, _handler: () => Promise<void> | void) => vi.fn())
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
    });
  });

  it("renders empty state after loading proposals", async () => {
    invokeMock.mockResolvedValueOnce([]);

    render(<BridgeInbox isTauri />);

    expect(await screen.findByText("No bridge proposals")).toBeDefined();
    expect(invokeMock).toHaveBeenCalledWith("list_bridge_proposals", {});
  });

  it("groups pending and resolved proposals", async () => {
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

    expect(await screen.findByText("Pending Review")).toBeDefined();
    expect(screen.getByText("Resolved")).toBeDefined();
    expect(screen.getAllByText("批准研究任务").length).toBeGreaterThan(0);
    expect(screen.getAllByText("归档梦境洞察").length).toBeGreaterThan(0);
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

    expect(await screen.findByText("Task Type")).toBeDefined();
    expect(screen.getByText("research")).toBeDefined();
    expect(screen.getByText("Sandbox")).toBeDefined();
    expect(screen.getByText("tools: vector_search, web_search; network: allowlisted")).toBeDefined();
    expect(screen.getByText("Expected Output")).toBeDefined();
    expect(screen.getByText("research brief with sources")).toBeDefined();
    expect(screen.getByText("Risk Reason")).toBeDefined();
    expect(screen.getByText("medium risk because research may use network search")).toBeDefined();
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

    expect(await screen.findByText("Pending Review")).toBeDefined();
    await waitFor(() => {
      expect(updateHandler).not.toBeNull();
    });
    await act(async () => {
      await updateHandler?.();
    });

    expect((await screen.findAllByText("Updated proposal")).length).toBeGreaterThan(0);
    expect(invokeMock).toHaveBeenCalledTimes(2);
  });

  it("navigates to the target panel when an action returns navigation metadata", async () => {
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
