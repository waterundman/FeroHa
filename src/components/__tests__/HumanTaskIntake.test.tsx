import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { readFileSync } from "node:fs";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { panelTabsForMode, resolvePanelForMode } from "../../App";
import { useAppStore } from "../../hooks/useAppStore";
import AgentDashboard from "../AgentDashboard";
import HumanTaskIntake, { buildHumanTaskDispatchPayload } from "../HumanTaskIntake";

const humanTaskIntakeSource = readFileSync("src/components/HumanTaskIntake.tsx", "utf8");
const aiTaskStripSource = readFileSync("src/components/AiTaskStrip.tsx", "utf8");

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (command: string) => {
    if (command === "dispatch_agent_task") return { task_id: "sim-backend-task", status: "pending" };
    if (command === "orchestrator_status") return useAppStore.getState().orchestratorStatus;
    if (command === "list_tasks") {
      return [
        {
          id: "sim-backend-task",
          command: { verb: "Custom", params: "mock simulation" },
          status: "Pending",
          task_type: { Custom: "research" },
          priority_score: 50,
          created_at: 1_781_500_001_000,
          retry_count: 0,
          intent: "研究线性空间中的维度坍缩问题",
          content: "模拟后端任务",
          subagent_results: [],
          context_fragments: [],
        },
      ];
    }
    if (command === "get_dream_status") return { last_stats: null, insights: [] };
    if (command === "get_trust_score_info") {
      return { score: 0.8, mode: "manual_bridge", acceptance_rate: 0.8, total_interactions: 1 };
    }
    if (command === "list_ai_face_data_flows") return [];
    if (command === "get_ai_manager_snapshot") return null;
    if (command === "list_agent_tools") return [];
    return null;
  }),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => vi.fn()),
}));

describe("HumanTaskIntake", () => {
  beforeEach(() => {
    useAppStore.setState({
      vaultPath: null,
      currentNote: null,
      tasks: [],
      diffBlocks: [],
      bridgeProposals: [],
      orchestratorStatus: null,
      mode: "human",
      activePanel: "task-intake",
    });
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: undefined,
    });
  });

  it("builds the shared AI task dispatch payload", () => {
    expect(
      buildHumanTaskDispatchPayload({
        title: "整理 Dream 三区",
        taskType: "dream",
        scope: "current_note",
        expectedOutput: "给出桥接提案",
        reviewMode: "manual_bridge",
        contextNote: "Dream.md",
        timestamp: 99,
      }),
    ).toMatchObject({
      intent: "整理 Dream 三区",
      task_type: "dream",
      scope: "current_note",
      expected_output: "给出桥接提案",
      review_mode: "manual_bridge",
      context_note: "Dream.md",
      timestamp: 99,
      source: "human_task_intake",
    });
  });

  it("renders upstream task intake separate from downstream review", () => {
    render(<HumanTaskIntake isTauri={false} />);

    expect(screen.getByRole("heading", { name: /向 AI 提任务/ })).toBeTruthy();
    expect(screen.getByLabelText("任务标题")).toBeTruthy();
    expect(screen.getByLabelText("任务类型")).toBeTruthy();
    expect(screen.getByLabelText("任务范围")).toBeTruthy();
    expect(screen.getByLabelText("审查方式")).toBeTruthy();
  });

  it("blocks manual Bridge submission in Tauri until a vault is open", () => {
    render(<HumanTaskIntake isTauri={true} />);

    fireEvent.change(screen.getByLabelText("任务标题"), {
      target: { value: "真实 Bridge 前置条件测试" },
    });
    fireEvent.change(screen.getByLabelText("期望输出"), {
      target: { value: "必须先有 vault 才能进入 Bridge inbox" },
    });

    expect(screen.getByText(/先打开笔记库/)).toBeTruthy();
    expect(screen.getByRole("button", { name: /提交给 AI Manager/ })).toHaveAttribute(
      "disabled",
    );
  });

  it("keeps task intake in the human surface navigation", () => {
    expect(panelTabsForMode("human").map((tab) => tab.panel)).toContain("task-intake");
    expect(resolvePanelForMode("task-intake", "ai")).toBe("graph");
  });

  it("feeds the deterministic simulation suite into the AI surface", async () => {
    render(<HumanTaskIntake isTauri={false} />);

    fireEvent.click(screen.getByRole("button", { name: "投喂退行警告" }));

    await waitFor(() => {
      expect(useAppStore.getState().activePanel).toBe("tasks");
    });

    const state = useAppStore.getState();
    expect(state.tasks.some((task) => task.id === "sim-task-buggy")).toBe(true);
    expect(state.orchestratorStatus?.degraded_agents).toContain("Master-Alpha");
    expect(state.bridgeProposals[0].risk).toBe("high");
    expect(state.diffBlocks[0].newText).toContain("高风险推论");
    expect(screen.getByText(/模拟投喂已完成/)).toBeTruthy();
  });

  it("keeps the AI dashboard renderable after a Tauri mock simulation", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: {},
    });

    function Harness() {
      const activePanel = useAppStore((s) => s.activePanel);
      return activePanel === "tasks" ? <AgentDashboard /> : <HumanTaskIntake isTauri />;
    }

    render(<Harness />);

    fireEvent.click(screen.getByRole("button", { name: "投喂成功闭环" }));

    await waitFor(() => {
      expect(useAppStore.getState().activePanel).toBe("tasks");
    });

    expect(screen.getByText(/任务队列/)).toBeTruthy();
  });

  it("keeps human task intake on the shared dispatch path without duplicating AI command-card entrypoints", () => {
    expect(humanTaskIntakeSource).toContain('"dispatch_agent_task"');
    expect(humanTaskIntakeSource).toContain("buildHumanTaskDispatchPayload");
    expect(humanTaskIntakeSource).not.toContain("<CommandCardPanel");
    expect(humanTaskIntakeSource).not.toContain("<CommandCardLibrary");

    expect(aiTaskStripSource).toContain("<CommandCardPanel");
    expect(aiTaskStripSource).toContain("buildCommandCardDispatchPayload");
  });
});
