// Test: VaultBrowser component
import { beforeEach, describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, within } from "@testing-library/react";
import VaultBrowser, {
  aiWorkspaceDisplayPath,
  aiWorkspaceZoneForPath,
  humanFolderPathFromInput,
  mergeVaultNoteLists,
  vaultContextMenuActionIdsForNode,
} from "../VaultBrowser";
import { useAppStore } from "../../hooks/useAppStore";

// Mock window.prompt (still used by handleCreateFolder)
const mockPrompt = vi.spyOn(window, "prompt");
const mockConfirm = vi.spyOn(window, "confirm");

describe("VaultBrowser", () => {
  beforeEach(() => {
    mockPrompt.mockReturnValue(null);
    mockConfirm.mockReturnValue(true);
    useAppStore.setState({
      notes: [],
      currentNote: null,
      isDirty: false,
      favorites: [],
      recentNotes: [],
      filterTags: [],
      tabs: [],
      activeTabIndex: -1,
      splitActive: false,
      splitTabIndex: -1,
    });
  });

  it("renders with Vault title and open button", () => {
    render(<VaultBrowser vaultPath={null} onSelectVault={vi.fn()} isTauri={false} />);
    
    expect(screen.getByText("笔记库")).toBeDefined();
    expect(screen.getByTitle("打开笔记库")).toBeDefined();
  });

  it("shows empty state when no notes in browser mode", () => {
    render(<VaultBrowser vaultPath={null} onSelectVault={vi.fn()} isTauri={false} />);
    
    // In browser mode with null vaultPath, no actions or file list content
    expect(screen.getByText("笔记库")).toBeDefined();
    expect(screen.queryByText("未找到 .md 文件")).toBeNull(); // file list hidden without vaultPath
  });

  it("shows a manual vault path opener in Tauri before a vault is selected", () => {
    render(<VaultBrowser vaultPath={null} onSelectVault={vi.fn()} isTauri />);

    expect(screen.getByPlaceholderText("Vault path")).toBeDefined();
    expect(screen.getByRole("button", { name: "Open" })).toBeDefined();
  });

  it("displays actions with sort dropdown when vault is open", () => {
    render(<VaultBrowser vaultPath="/test-vault" onSelectVault={vi.fn()} isTauri={false} />);
    
    expect(screen.getByText("/test-vault")).toBeDefined();
    expect(screen.getByTitle("新建笔记")).toBeDefined();
    expect(screen.getByTitle("新建文件夹")).toBeDefined();
    expect(screen.getByTitle("排序方式")).toBeDefined();
    expect(screen.getByTitle("刷新")).toBeDefined();
  });

  it("opens template picker when new note button clicked", () => {
    const onSelectVault = vi.fn();
    
    render(<VaultBrowser vaultPath="/test-vault" onSelectVault={onSelectVault} isTauri={false} />);
    
    fireEvent.click(screen.getByTitle("新建笔记"));
    expect(screen.getByText("新建笔记 - 选择模板")).toBeDefined();
    expect(screen.getAllByText(/空白笔记/).length).toBeGreaterThan(0);
  });

  it("creates a new folder when prompted", () => {
    mockPrompt.mockReturnValue("test-folder");
    
    render(<VaultBrowser vaultPath="/test-vault" onSelectVault={vi.fn()} isTauri={false} />);
    
    fireEvent.click(screen.getByTitle("新建文件夹"));
    expect(mockPrompt).toHaveBeenCalledWith("新文件夹名称：");
  });

  it("shows a newly created empty human folder before it has notes", () => {
    mockPrompt.mockReturnValue("研究 资料");
    useAppStore.setState({
      notes: [],
      favorites: [],
      recentNotes: [],
      filterTags: [],
    });

    render(<VaultBrowser vaultPath="/test-vault" onSelectVault={vi.fn()} isTauri={false} />);

    fireEvent.click(screen.getByTitle("新建文件夹"));

    const humanSection = within(screen.getByTestId("human-notes-section"));
    expect(humanSection.getByText("研究 资料")).toBeDefined();
    expect(screen.queryByText(/\.md 文件/)).toBeNull();
  });

  it("closes an open human note when it is deleted from the library", () => {
    useAppStore.setState({
      notes: [
        {
          path: "Draft.md",
          title: "Draft",
          size: 10,
          modified: "1",
          created: "1",
          links: [],
          tags: [],
        },
      ],
      favorites: ["Draft.md"],
      recentNotes: ["Draft.md"],
    });
    useAppStore.getState().openNote("Draft.md", "# Draft");

    render(<VaultBrowser vaultPath="/test-vault" onSelectVault={vi.fn()} isTauri={false} />);

    const humanSection = within(screen.getByTestId("human-notes-section"));
    fireEvent.contextMenu(humanSection.getByText("Draft"));
    fireEvent.click(screen.getByRole("menuitem", { name: /删除/ }));

    expect(useAppStore.getState().currentNote).toBeNull();
    expect(useAppStore.getState().tabs).toHaveLength(0);
    expect(useAppStore.getState().favorites).not.toContain("Draft.md");
    expect(useAppStore.getState().recentNotes).not.toContain("Draft.md");
  });

  it("preserves readable human folder names while rejecting internal paths", () => {
    expect(humanFolderPathFromInput(undefined, "研究 资料")).toBe("研究 资料");
    expect(humanFolderPathFromInput("Projects", "阶段 一")).toBe("Projects/阶段 一");
    expect(humanFolderPathFromInput(undefined, ".dualtrack")).toBeNull();
    expect(humanFolderPathFromInput(undefined, "../escape")).toBeNull();
  });

  it("sort dropdown shows all sort options", () => {
    render(<VaultBrowser vaultPath="/test-vault" onSelectVault={vi.fn()} isTauri={false} />);
    
    const sortSelect = screen.getByTitle("排序方式") as HTMLSelectElement;
    expect(sortSelect.value).toBe("title-asc");
    
    const options = screen.getAllByRole("option");
    expect(options).toHaveLength(4);
    expect(options[0].textContent).toBe("标题 A-Z");
    expect(options[1].textContent).toBe("标题 Z-A");
    expect(options[2].textContent).toBe("修改时间（最新）");
    expect(options[3].textContent).toBe("修改时间（最旧）");
    
    fireEvent.change(sortSelect, { target: { value: "title-desc" } });
    expect(sortSelect.value).toBe("title-desc");
  });

  it("shows note workflow actions on right click", () => {
    useAppStore.setState({
      notes: [
        {
          path: "Dream.md",
          title: "Dream",
          size: 10,
          modified: "1",
          created: "1",
          links: [],
          tags: [],
        },
      ],
      favorites: [],
    });

    render(<VaultBrowser vaultPath="/test-vault" onSelectVault={vi.fn()} isTauri={false} />);

    fireEvent.contextMenu(screen.getByText("Dream"));

    expect(screen.getByRole("menuitem", { name: /以此向 AI 提任务/ })).toBeDefined();
    expect(screen.getByRole("menuitem", { name: /在图谱中聚焦/ })).toBeDefined();
    expect(screen.getByRole("menuitem", { name: /复制路径/ })).toBeDefined();
  });
  it("keeps AI workspace files read-only while preserving quick task actions", () => {
    const ids = vaultContextMenuActionIdsForNode({
      path: ".dualtrack/Research.md",
      isFolder: false,
    });

    expect(ids).toEqual(expect.arrayContaining([
      "open-readonly",
      "ask-ai",
      "focus-graph",
      "copy-path",
    ]));
    expect(ids).not.toContain("rename");
    expect(ids).not.toContain("delete");
  });

  it("classifies AI workspace files by the Dream three-zone memory protocol", () => {
    expect(aiWorkspaceZoneForPath(".dualtrack/memory/working/task.md")).toBe("working");
    expect(aiWorkspaceZoneForPath(".dualtrack/research/results/task_1/result.md")).toBe("working");
    expect(aiWorkspaceZoneForPath(".dualtrack/memory/semantic/claims.md")).toBe("semantic");
    expect(aiWorkspaceZoneForPath(".dualtrack/jsonld/indexes/graph.md")).toBe("semantic");
    expect(aiWorkspaceZoneForPath(".dualtrack/memory/long_term/insight.md")).toBe("long_term");
    expect(aiWorkspaceZoneForPath(".dualtrack/dream/insight.md")).toBe("long_term");
    expect(aiWorkspaceZoneForPath("Human/Dream.md")).toBeNull();
  });

  it("merges human notes with the read-only AI workspace listing", () => {
    const human = {
      path: "Human/Dream.md",
      title: "Dream",
      size: 10,
      modified: "1",
      created: "1",
      links: [],
      tags: [],
    };
    const ai = {
      path: ".dualtrack/memory/working/task.md",
      title: "task",
      size: 12,
      modified: "2",
      created: "2",
      links: [],
      tags: [],
    };

    expect(mergeVaultNoteLists([human], [ai]).map((note) => note.path)).toEqual([
      "Human/Dream.md",
      ".dualtrack/memory/working/task.md",
    ]);
  });

  it("keeps Dream memory zones visible even before AI workspace files exist", () => {
    useAppStore.setState({
      notes: [],
      favorites: [],
      recentNotes: [],
      filterTags: [],
    });

    render(<VaultBrowser vaultPath="/test-vault" onSelectVault={vi.fn()} isTauri={false} />);

    expect(screen.getByTestId("human-notes-section")).toBeDefined();
    expect(screen.getByTestId("ai-zone-working")).toBeDefined();
    expect(screen.getByTestId("ai-zone-semantic")).toBeDefined();
    expect(screen.getByTestId("ai-zone-long_term")).toBeDefined();
  });

  it("renders AI workspace files under Dream memory zones instead of one flat dualtrack group", () => {
    useAppStore.setState({
      notes: [
        {
          path: ".dualtrack/research/results/task_1/result.md",
          title: "result",
          size: 10,
          modified: "1",
          created: "1",
          links: [],
          tags: [],
        },
        {
          path: ".dualtrack/jsonld/indexes/claims.md",
          title: "claims",
          size: 10,
          modified: "1",
          created: "1",
          links: [],
          tags: [],
        },
        {
          path: ".dualtrack/dream/insight.md",
          title: "insight",
          size: 10,
          modified: "1",
          created: "1",
          links: [],
          tags: [],
        },
      ],
    });

    render(<VaultBrowser vaultPath="/test-vault" onSelectVault={vi.fn()} isTauri={false} />);

    expect(aiWorkspaceDisplayPath(".dualtrack/research/results/task_1/result.md")).toBe("research/results/task_1/result.md");
    const workingZone = within(screen.getByTestId("ai-zone-working"));
    fireEvent.click(workingZone.getByText("research"));
    fireEvent.click(workingZone.getByText("results"));
    fireEvent.click(workingZone.getByText("task_1"));
    expect(workingZone.getByText("result")).toBeDefined();

    const semanticZone = within(screen.getByTestId("ai-zone-semantic"));
    fireEvent.click(semanticZone.getByText("jsonld"));
    fireEvent.click(semanticZone.getByText("indexes"));
    expect(semanticZone.getByText("claims")).toBeDefined();

    const longTermZone = within(screen.getByTestId("ai-zone-long_term"));
    fireEvent.click(longTermZone.getByText("dream"));
    expect(longTermZone.getByText("insight")).toBeDefined();
  });

  it("keeps human files writable and taskable from the same context menu policy", () => {
    const ids = vaultContextMenuActionIdsForNode({
      path: "Human/Dream.md",
      isFolder: false,
    });

    expect(ids).toEqual(expect.arrayContaining([
      "ask-ai",
      "focus-graph",
      "copy-path",
      "favorite",
      "rename",
      "delete",
    ]));
  });

  it("filters notes by title or path for faster file viewing", () => {
    useAppStore.setState({
      notes: [
        {
          path: "Dream.md",
          title: "Dream",
          size: 10,
          modified: "1",
          created: "1",
          links: [],
          tags: [],
        },
        {
          path: "Archive/Other.md",
          title: "Other",
          size: 10,
          modified: "1",
          created: "1",
          links: [],
          tags: [],
        },
      ],
    });

    render(<VaultBrowser vaultPath="/test-vault" onSelectVault={vi.fn()} isTauri={false} />);

    fireEvent.change(screen.getByPlaceholderText("搜索笔记、文件夹或标签"), {
      target: { value: "dream" },
    });

    expect(screen.getByText("Dream")).toBeDefined();
    expect(screen.queryByText("Other")).toBeNull();
  });
});
