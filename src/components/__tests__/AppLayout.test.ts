import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import {
  panelTabsForMode,
  resolvePanelForMode,
  agentResultToastMessage,
  feedbackRecordedToastMessage,
  settingsButtonStyleForState,
  sidebarNavStyleForState,
  sidebarPanelSizing,
  snapshotDriftToastMessage,
  taskCheckedToastMessage,
  windowControlDefinitions,
} from "../../App";
import { useAppStore } from "../../hooks/useAppStore";

const appSource = readFileSync("src/App.tsx", "utf8");
const themeSource = readFileSync("src/styles/feroha-theme.css", "utf8");
const desktopCapability = JSON.parse(readFileSync("src-tauri/capabilities/default.json", "utf8"));
const tauriMainSource = readFileSync("src-tauri/src/main.rs", "utf8");

function sourceFileExists(path: string): boolean {
  try {
    readFileSync(path, "utf8");
    return true;
  } catch {
    return false;
  }
}

describe("App sidebar layout", () => {
  it("uses percentage panel sizes so the navigation hit targets are not squeezed to pixels", () => {
    expect(sidebarPanelSizing).toEqual({
      defaultSize: "20%",
      minSize: "15%",
      maxSize: "35%",
      collapsedSize: "6%",
    });
  });

  it("stacks navigation buttons when the sidebar is collapsed", () => {
    expect(sidebarNavStyleForState(true)).toMatchObject({
      flexDirection: "column",
      alignItems: "center",
      overflow: "visible",
      position: "relative",
      zIndex: 3,
    });
  });

  it("wraps expanded AI navigation so settings cannot be clipped offscreen", () => {
    expect(sidebarNavStyleForState(false)).toMatchObject({
      flexWrap: "wrap",
      overflow: "visible",
      alignItems: "center",
    });
    expect(settingsButtonStyleForState(false, false)).toMatchObject({
      marginLeft: 0,
    });
  });

  it("keeps review-only panels out of the AI face navigation", () => {
    expect(panelTabsForMode("ai").map((tab) => tab.panel)).toEqual([
      "editor",
      "graph",
      "tasks",
      "cards",
      "pipeline",
      "plugins",
    ]);
  });

  it("keeps the retired legacy TaskPanel out of the task surface", () => {
    expect(sourceFileExists("src/components/TaskPanel.tsx")).toBe(false);
    expect(appSource).not.toContain("TaskPanel");
    expect(appSource).toContain("<AgentDashboard />");
  });

  it("keeps Bridge and Diff in the human review path", () => {
    expect(panelTabsForMode("human").map((tab) => tab.panel)).toEqual([
      "editor",
      "task-intake",
      "inspiration",
      "bridge",
      "diff",
    ]);
  });

  it("redirects stale review panels when returning to the AI face", () => {
    expect(resolvePanelForMode("diff", "ai")).toBe("graph");
    expect(resolvePanelForMode("bridge", "ai")).toBe("graph");
    expect(resolvePanelForMode("tasks", "human")).toBe("inspiration");
  });

  it("moves direct diff navigation to the human face at the store boundary", () => {
    useAppStore.setState({ mode: "ai", activePanel: "tasks" });

    useAppStore.getState().setActivePanel("diff");

    expect(useAppStore.getState().mode).toBe("human");
    expect(useAppStore.getState().activePanel).toBe("diff");
  });

  it("opens settings without changing the current face", () => {
    useAppStore.setState({ mode: "ai", activePanel: "tasks" });

    useAppStore.getState().setActivePanel("settings");

    expect(useAppStore.getState().mode).toBe("ai");
    expect(useAppStore.getState().activePanel).toBe("settings");
  });

  it("keeps the settings button reachable in collapsed navigation", () => {
    expect(settingsButtonStyleForState(true, false)).toMatchObject({
      marginTop: "auto",
      marginLeft: 0,
    });
  });

  it("localizes system toast messages", () => {
    expect(taskCheckedToastMessage("task-1")).toBe("任务 task-1 已批准");
    expect(agentResultToastMessage("research", "贝叶斯更新")).toBe("AI 结果已就绪：research - \"贝叶斯更新...\"");
    expect(feedbackRecordedToastMessage("accepted", 2)).toBe("反馈已记录：accepted，涉及 2 个区块");
    expect(snapshotDriftToastMessage({ snapshot_type: "global", note_id: "A.md", avg_cosine_distance: 0.1234 })).toBe(
      "全局快照漂移：\"A.md\"，平均距离 0.123",
    );
  });

  it("integrates window controls into the app chrome instead of native decoration", () => {
    const config = JSON.parse(readFileSync("src-tauri/tauri.conf.json", "utf8"));

    expect(config.app.windows[0].decorations).toBe(false);
    expect(tauriMainSource).toContain(".decorations(false)");
    expect(config.productName).toBe("贝叶斯笔记");
    expect(config.app.windows[0].title).toBe("贝叶斯笔记");
    expect(windowControlDefinitions.map((control) => control.id)).toEqual([
      "minimize",
      "maximize",
      "close",
    ]);
  });

  it("keeps the top chrome free of diagnostic labels", () => {
    expect(appSource).not.toContain("backendStatus");
    expect(appSource).not.toContain("appTitle");
    expect(appSource).not.toContain("FeroHa</span>");
    expect(appSource).not.toContain("ping");
  });

  it("can bootstrap a Tauri vault from the page query string for repeatable desktop loops", () => {
    expect(appSource).toContain('new URLSearchParams(window.location.search).get("vault")');
    expect(appSource).toContain("const bootstrapVaultPath = urlVaultPath || vaultPath");
    expect(appSource).toContain('await invoke("open_vault", { path: bootstrapVaultPath })');
  });

  it("loads human notes and AI workspace files through separate backend contracts", () => {
    expect(appSource).toContain('"list_notes"');
    expect(appSource).toContain('"list_ai_workspace_files"');
    expect(tauriMainSource).toContain("fs::commands::list_ai_workspace_files");
  });

  it("routes the editor empty-state new-note action to the vault template picker", () => {
    expect(appSource).toContain("newNotePickerOpen");
    expect(appSource).toContain("handleRequestNewNote");
    expect(appSource).toContain("onCreateNote={handleRequestNewNote}");
    expect(appSource).toContain("templatePickerOpen={newNotePickerOpen}");
  });

  it("exposes window controls through a direct Tauri action helper", () => {
    expect(appSource).toContain("export async function runWindowControlAction");
    expect(appSource).toContain("onMouseDown={(event) => event.stopPropagation()}");
    expect(appSource).not.toContain("function WindowControls({ isTauri }");
  });

  it("grants Tauri window permissions for the custom chrome buttons", () => {
    expect(desktopCapability.windows).toContain("main");
    expect(desktopCapability.permissions).toEqual(
      expect.arrayContaining([
        "core:window:allow-close",
        "core:window:allow-minimize",
        "core:window:allow-toggle-maximize",
      ]),
    );
  });

  it("routes the AI pipeline tab to the read-only orchestrator workflow view", () => {
    expect(appSource).toContain("<OrchestratorWorkflowView />");
    expect(appSource).not.toContain("<PipelineEditor");
    expect(appSource).not.toContain("handleRunPipeline");
  });

  it("keeps the API debug success glow wired to the app shell", () => {
    expect(appSource).toContain("feroha:api-debug-success");
    expect(appSource).toContain("data-api-debug-success");
    expect(themeSource).toContain("[data-api-debug-success]::before");
    expect(themeSource).toContain("api-debug-particles");
  });
});
