# Dual Surface Task Memory Stage 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first implementation stage for the dual-surface task and Dream-memory spec: separate AI task strip from floating CLI, add human task intake, unify core control styling, add context menus, and normalize graph memory regions to Dream's three zones plus bridge overlay.

**Architecture:** This stage keeps the current React/Tauri architecture and changes surface ownership at the store/navigation boundary. Frontend behavior is driven by small exported helpers so tests can verify task payloads, panel placement, graph region mapping, and context-menu item routing without requiring Tauri. Backend changes are limited to memory-zone normalization contracts and Dream graph export mapping already present in the Rust modules.

**Tech Stack:** React 18, Zustand, Vitest, Testing Library, Vite, Rust/Tauri commands, D3 force graph canvas.

---

## File Structure

- Create `src/components/AiTaskStrip.tsx`: AI footer task-card launcher and task intent preview; no terminal-like CLI input.
- Create `src/components/HumanTaskIntake.tsx`: human-surface page for creating human-to-AI task requests.
- Create `src/components/ContextMenu.tsx`: shared accessible context-menu primitive for right-click workflows.
- Create `src/components/__tests__/AiTaskStrip.test.tsx`: footer separation and task payload helper tests.
- Create `src/components/__tests__/HumanTaskIntake.test.tsx`: request payload and review-mode tests.
- Create `src/components/__tests__/ContextMenu.test.tsx`: right-click menu rendering and Escape dismissal tests.
- Modify `src/components/CliBar.tsx`: keep exported task-intent helpers, remove inline CLI UI responsibility, or leave as compatibility wrapper around `AiTaskStrip`.
- Modify `src/App.tsx`: replace AI footer `CliBar` with `AiTaskStrip`; add human panel `task-intake`; use shared resize-handle class.
- Modify `src/hooks/useAppStore.ts`: add `task-intake` panel and optional `GraphNode.memory_zone`.
- Modify `src/styles/feroha-theme.css`: add shared control and resize tokens/classes.
- Modify `src/components/GraphView.tsx` and `src/components/GraphView.css`: Dream three-zone regions, focus/overview/triad mode helpers, integrated controls, and non-stalling interaction loop.
- Modify `src/components/BridgeInbox.tsx`: clarify downstream proposal review and add proposal context-menu hooks.
- Modify `src/components/DiffView.tsx`: clarify concrete text-change review and add block context-menu hooks.
- Modify `src/components/VaultBrowser.tsx`: adopt shared context menu and richer right-click actions.
- Modify backend files only if frontend/graph tests prove contract gaps: `src-tauri/src/ai/dream_engine.rs`, `src-tauri/src/graph/link_graph.rs`, `src-tauri/src/fs/commands.rs`.

No git commit is planned in this stage because the user asked to continue iterating directly in the current workspace and did not request a commit.

## Task 1: AI Task Strip Replaces Footer CLI Toggle

**Files:**
- Create: `src/components/AiTaskStrip.tsx`
- Create: `src/components/__tests__/AiTaskStrip.test.tsx`
- Modify: `src/App.tsx`
- Modify: `src/components/CliBar.tsx`

- [ ] **Step 1: Write the failing test**

Create `src/components/__tests__/AiTaskStrip.test.tsx` with these assertions:

```tsx
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import AiTaskStrip, { aiTaskStripPreviewLabel } from "../AiTaskStrip";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

describe("AiTaskStrip", () => {
  it("keeps command-card task entry separate from terminal CLI input", () => {
    render(<AiTaskStrip isTauri={false} />);

    expect(screen.getByRole("button", { name: /打开指令卡/ })).toBeTruthy();
    expect(screen.queryByPlaceholderText(/agent/i)).toBeNull();
    expect(screen.queryByRole("button", { name: /切换到 CLI/ })).toBeNull();
  });

  it("opens command cards from the slash shortcut", () => {
    render(<AiTaskStrip isTauri={false} />);

    fireEvent.keyDown(window, { key: "/" });

    expect(screen.getByRole("dialog", { name: /指令卡/ })).toBeTruthy();
  });

  it("shows compact risk and write-policy preview", () => {
    expect(aiTaskStripPreviewLabel("auto", "research")).toContain("自动");
    render(<AiTaskStrip isTauri={false} />);

    expect(screen.getByLabelText("AI 任务类型")).toBeTruthy();
    expect(screen.getByText(/Bridge/)).toBeTruthy();
    expect(screen.getByText(/写权/)).toBeTruthy();
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```powershell
npm test -- --run src/components/__tests__/AiTaskStrip.test.tsx
```

Expected: fails because `AiTaskStrip.tsx` does not exist.

- [ ] **Step 3: Implement minimal component**

Create `AiTaskStrip.tsx` by moving the command-card launcher and task-intent preview out of `CliBar.tsx`. Keep these exported helpers imported from `CliBar.tsx`: `TASK_INTENT_SELECT_OPTIONS`, `TaskIntentSelection`, `TaskIntentType`, `resolveTaskIntentSelectionForCard`, `taskIntentReviewInfo`, `stringifyCommandParams`, `renderCommandCardPrompt`, and `buildCommandCardDispatchPayload`.

The rendered shape must include:

```tsx
<section className="ai-task-strip" aria-label="AI 任务条">
  <button type="button" aria-label="打开指令卡" onClick={() => setShowCommandPanel(true)}>
    <FeroHaIcon name="LayoutGrid" size={15} />
    <span>打开指令卡</span>
  </button>
  <select aria-label="AI 任务类型" className="feroha-select" />
  <div aria-label="任务写入策略预览">
    <span>{previewLabel}</span>
    <span>Bridge 风险：{previewInfo.risk}</span>
    <span>写权：{previewInfo.writePolicy}</span>
  </div>
</section>
```

The `/` key handler opens `CommandCardPanel` unless an input or textarea is focused.

- [ ] **Step 4: Wire App footer**

Modify `src/App.tsx`:

```tsx
import AiTaskStrip from "./components/AiTaskStrip";
```

Replace:

```tsx
{mode === "ai" && <CliBar isTauri={isTauri} />}
```

With:

```tsx
{mode === "ai" && <AiTaskStrip isTauri={isTauri} />}
```

Leave `CliMiniWindow` mounted globally.

- [ ] **Step 5: Make `CliBar` a compatibility export holder**

Keep task-intent helper exports in `CliBar.tsx`. If the component remains exported, make it render `<AiTaskStrip isTauri={isTauri} />` or remove only the inline CLI UI while preserving tests that import helpers.

- [ ] **Step 6: Run verification**

Run:

```powershell
npm test -- --run src/components/__tests__/AiTaskStrip.test.tsx src/components/__tests__/CliBar.test.tsx
```

Expected: both test files pass.

## Task 2: Human Task Intake Panel

**Files:**
- Create: `src/components/HumanTaskIntake.tsx`
- Create: `src/components/__tests__/HumanTaskIntake.test.tsx`
- Modify: `src/App.tsx`
- Modify: `src/hooks/useAppStore.ts`
- Modify: `src/components/__tests__/AppLayout.test.ts`

- [ ] **Step 1: Write failing tests**

Add tests:

```tsx
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import HumanTaskIntake, { buildHumanTaskDispatchPayload } from "../HumanTaskIntake";
import { panelTabsForMode, resolvePanelForMode } from "../../App";

describe("HumanTaskIntake", () => {
  it("builds the shared AI task dispatch payload", () => {
    expect(buildHumanTaskDispatchPayload({
      title: "整理 Dream 三区",
      taskType: "dream",
      scope: "current_note",
      expectedOutput: "给出桥接提案",
      reviewMode: "manual_bridge",
      contextNote: "Dream.md",
      timestamp: 99,
    })).toMatchObject({
      intent: "整理 Dream 三区",
      task_type: "dream",
      scope: "current_note",
      expected_output: "给出桥接提案",
      review_mode: "manual_bridge",
      context_note: "Dream.md",
      timestamp: 99,
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

  it("keeps task intake in the human surface navigation", () => {
    expect(panelTabsForMode("human").map((tab) => tab.panel)).toContain("task-intake");
    expect(resolvePanelForMode("task-intake", "ai")).toBe("graph");
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```powershell
npm test -- --run src/components/__tests__/HumanTaskIntake.test.tsx src/components/__tests__/AppLayout.test.ts
```

Expected: fails because panel/component/type do not exist.

- [ ] **Step 3: Add store and navigation panel**

Modify `ActivePanel` in `src/hooks/useAppStore.ts`:

```ts
export type ActivePanel =
  | "editor"
  | "task-intake"
  | "graph"
  ...
```

Add `"task-intake"` to `activePanels` and `humanPanels`; do not add it to `aiPanels`.

Modify `src/App.tsx` human tabs:

```ts
{ panel: "task-intake", title: "向 AI 提任务" },
```

Add icon mapping:

```ts
"task-intake": "Send",
```

Add panel render:

```tsx
<div role="tabpanel" id="panel-task-intake" aria-label="向 AI 提任务面板" hidden={activePanel !== "task-intake"} style={styles.panelShell}>
  {activePanel === "task-intake" && <HumanTaskIntake isTauri={isTauri} />}
</div>
```

- [ ] **Step 4: Implement task intake**

`HumanTaskIntake.tsx` exports:

```ts
export type HumanTaskScope = "current_note" | "selected_text" | "folder" | "graph_focus" | "freeform";
export type HumanTaskReviewMode = "manual_bridge" | "read_only_auto_queue" | "draft_only";
export interface HumanTaskDraft {
  title: string;
  taskType: TaskIntentType;
  scope: HumanTaskScope;
  expectedOutput: string;
  reviewMode: HumanTaskReviewMode;
  contextNote: string | null;
  timestamp: number;
}
```

`buildHumanTaskDispatchPayload` returns the shared backend payload with `source: "human_task_intake"`.

Browser mode adds a local task via `useAppStore.addTask`. Tauri mode calls `dispatch_agent_task` with `{ payload }`.

- [ ] **Step 5: Run verification**

Run:

```powershell
npm test -- --run src/components/__tests__/HumanTaskIntake.test.tsx src/components/__tests__/AppLayout.test.ts
```

Expected: both test files pass.

## Task 3: Unified Control And Resize Styling

**Files:**
- Modify: `src/styles/feroha-theme.css`
- Modify: `src/App.tsx`
- Modify: `src/components/GraphView.css`
- Modify: `src/components/CliMiniWindow.css`
- Modify: representative inline styles in `VaultBrowser.tsx`, `QuickSwitcher.tsx`, `CommandCardLibrary.tsx`, `CommandCardPanel.tsx`, `SettingsPanel.tsx`, and `PipelineEditor.css`

- [ ] **Step 1: Write failing style contract test**

Create or extend a test file with:

```ts
import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";

const theme = readFileSync("src/styles/feroha-theme.css", "utf8");

describe("shared control styling contract", () => {
  it("defines integrated control and resize tokens", () => {
    for (const token of [
      "--control-bg",
      "--control-bg-hover",
      "--control-border",
      "--control-border-strong",
      "--control-placeholder",
      "--control-shadow-focus",
      "--resize-handle-bg",
      "--resize-handle-active",
    ]) {
      expect(theme).toContain(token);
    }
  });

  it("defines reusable control classes", () => {
    for (const className of [
      ".feroha-control",
      ".feroha-input",
      ".feroha-select",
      ".feroha-textarea",
      ".feroha-search",
      ".feroha-resize-handle",
    ]) {
      expect(theme).toContain(className);
    }
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```powershell
npm test -- --run src/components/__tests__/ControlSurface.test.ts
```

Expected: fails until tokens/classes exist.

- [ ] **Step 3: Add theme tokens**

For each theme block, define control tokens near `--bg-input`. Example for `feroha`:

```css
--control-bg: color-mix(in srgb, var(--bg-secondary) 78%, var(--bg-input));
--control-bg-hover: color-mix(in srgb, var(--bg-hover) 80%, var(--bg-input));
--control-border: color-mix(in srgb, var(--border-color) 72%, transparent);
--control-border-strong: var(--accent-primary);
--control-placeholder: var(--text-muted);
--control-shadow-focus: 0 0 0 2px var(--accent-glow);
--resize-handle-bg: color-mix(in srgb, var(--border-color) 42%, transparent);
--resize-handle-active: color-mix(in srgb, var(--accent-primary) 52%, transparent);
```

Repeat equivalent definitions for `classic`, `macchiato`, `frappe`, and `latte` using existing variables so classic icons remain legible.

- [ ] **Step 4: Add shared classes**

Append:

```css
.feroha-control,
.feroha-input,
.feroha-select,
.feroha-textarea,
.feroha-search {
  background: var(--control-bg);
  border: 1px solid var(--control-border);
  color: var(--text-primary);
  border-radius: 6px;
  outline: none;
  transition: border-color var(--transition-fast), background var(--transition-fast), box-shadow var(--transition-fast);
}

.feroha-input,
.feroha-select,
.feroha-search {
  min-height: 30px;
  padding: 6px 9px;
}

.feroha-textarea {
  padding: 8px 10px;
  resize: vertical;
}

.feroha-input::placeholder,
.feroha-textarea::placeholder,
.feroha-search::placeholder {
  color: var(--control-placeholder);
}

.feroha-control:hover,
.feroha-input:hover,
.feroha-select:hover,
.feroha-textarea:hover,
.feroha-search:hover {
  background: var(--control-bg-hover);
}

.feroha-control:focus,
.feroha-control:focus-visible,
.feroha-input:focus,
.feroha-input:focus-visible,
.feroha-select:focus,
.feroha-select:focus-visible,
.feroha-textarea:focus,
.feroha-textarea:focus-visible,
.feroha-search:focus,
.feroha-search:focus-visible {
  border-color: var(--control-border-strong);
  box-shadow: var(--control-shadow-focus);
}

.feroha-resize-handle {
  background: var(--resize-handle-bg);
}

.feroha-resize-handle:hover,
.feroha-resize-handle:active {
  background: var(--resize-handle-active);
}
```

- [ ] **Step 5: Apply classes to representative controls**

At minimum:

```tsx
<Separator className="feroha-resize-handle" style={styles.resizeHandle} />
<input className="graph-search-input feroha-search" />
<select className="feroha-select" />
<textarea className="cli-input feroha-textarea" />
```

For inline styles that cannot use classes, replace `var(--bg-input)` field backgrounds with `var(--control-bg)` and border with `var(--control-border)`.

- [ ] **Step 6: Run verification**

Run:

```powershell
npm test -- --run src/components/__tests__/ControlSurface.test.ts
```

Expected: style contract passes.

## Task 4: Shared Context Menus For Mouse Right-Click Workflows

**Files:**
- Create: `src/components/ContextMenu.tsx`
- Create: `src/components/__tests__/ContextMenu.test.tsx`
- Modify: `src/components/VaultBrowser.tsx`
- Modify: `src/components/GraphView.tsx`
- Modify: `src/components/BridgeInbox.tsx`
- Modify: `src/components/DiffView.tsx`

- [ ] **Step 1: Write failing primitive test**

```tsx
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import ContextMenu from "../ContextMenu";

describe("ContextMenu", () => {
  it("renders menu items at the requested point", () => {
    render(
      <ContextMenu
        point={{ x: 20, y: 30 }}
        items={[{ id: "ask-ai", label: "以此向 AI 提任务", icon: "Send", onSelect: vi.fn() }]}
        onClose={vi.fn()}
      />,
    );

    expect(screen.getByRole("menu")).toBeTruthy();
    expect(screen.getByRole("menuitem", { name: /以此向 AI 提任务/ })).toBeTruthy();
  });

  it("closes on Escape", () => {
    const onClose = vi.fn();
    render(<ContextMenu point={{ x: 1, y: 1 }} items={[]} onClose={onClose} />);

    fireEvent.keyDown(document, { key: "Escape" });

    expect(onClose).toHaveBeenCalled();
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```powershell
npm test -- --run src/components/__tests__/ContextMenu.test.tsx
```

Expected: fails because component does not exist.

- [ ] **Step 3: Implement primitive**

`ContextMenu.tsx`:

```tsx
export interface ContextMenuItem {
  id: string;
  label: string;
  icon?: string;
  disabled?: boolean;
  onSelect: () => void;
}

export default function ContextMenu({ point, items, onClose }: {
  point: { x: number; y: number };
  items: ContextMenuItem[];
  onClose: () => void;
}) {
  ...
}
```

Use `role="menu"`, `role="menuitem"`, Escape listener, outside click listener, and `FeroHaIcon`.

- [ ] **Step 4: Replace VaultBrowser local menu**

Use `ContextMenu` and provide:

For file:

```ts
[
  "以此向 AI 提任务",
  "在图谱中聚焦",
  "复制路径",
  favoriteLabel,
  "重命名",
  "删除",
]
```

For folder:

```ts
[
  "新建笔记",
  "新建文件夹",
  "以此文件夹建立 AI 任务上下文",
]
```

- [ ] **Step 5: Add basic graph/bridge/diff context menu hooks**

Graph node menu labels:

```ts
["打开笔记", "设为图谱焦点", "以节点提问 AI", "隐藏非相邻节点"]
```

Bridge proposal menu labels:

```ts
["查看 trace", "打开差异审查", "复制证据", "归档"]
```

Diff block menu labels:

```ts
["接受此块", "拒绝此块", "复制原文", "复制新文", "打开来源提案"]
```

These first-stage handlers can call existing actions or show a toast when the backend path does not exist yet.

- [ ] **Step 6: Run verification**

Run:

```powershell
npm test -- --run src/components/__tests__/ContextMenu.test.tsx src/components/__tests__/VaultBrowser.test.tsx src/components/__tests__/BridgeInbox.test.tsx src/components/__tests__/DiffView.test.tsx
```

Expected: targeted frontend tests pass.

## Task 5: Dream Three-Zone Graph Normalization

**Files:**
- Modify: `src/hooks/useAppStore.ts`
- Modify: `src/components/GraphView.tsx`
- Modify: `src/components/GraphView.css`
- Modify: `src/components/__tests__/GraphView.test.ts`
- Modify if needed: `src-tauri/src/ai/dream_engine.rs`

- [ ] **Step 1: Write failing graph helper tests**

Extend `GraphView.test.ts`:

```ts
it("maps Dream memory regions to the three-zone protocol plus bridge overlay", () => {
  expect(graphMemoryRegion({ memory_region: "working" })).toBe("working");
  expect(graphMemoryRegion({ memory_region: "semantic" })).toBe("semantic");
  expect(graphMemoryRegion({ memory_region: "long_term" })).toBe("long_term");
  expect(graphMemoryRegion({ memory_region: "dream_bridge" })).toBe("bridge");
});

it("maps edge origins into Dream zones", () => {
  expect(graphMemoryRegion({ origin: "jsonld", edge_type: "related" })).toBe("semantic");
  expect(graphMemoryRegion({ origin: "wikilink", edge_type: "reference" })).toBe("working");
  expect(graphMemoryRegion({ origin: "archive", edge_type: "source" })).toBe("long_term");
});

it("keeps bridge as overlay instead of a fourth memory zone", () => {
  expect(memoryRegionVisualPlan("bridge")).toMatchObject({
    label: "跨区桥",
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```powershell
npm test -- --run src/components/__tests__/GraphView.test.ts
```

Expected: fails because current regions include `core`, `dream`, and `archive`.

- [ ] **Step 3: Update types and normalization**

In `useAppStore.ts`:

```ts
export type MemoryZone = "working" | "semantic" | "long_term";

export interface GraphNode {
  ...
  memory_zone?: MemoryZone;
}
```

In `GraphView.tsx`:

```ts
export type GraphMemoryRegion = "working" | "semantic" | "long_term" | "bridge";
```

Normalize:

```ts
if (["semantic", "jsonld", "structure", "structural", "semantic_core"].includes(normalized)) return "semantic";
if (["working", "work", "hot", "wikilink", "human", "scratch", "temporal"].includes(normalized)) return "working";
if (["long_term", "longterm", "archive", "cold", "source", "storage"].includes(normalized)) return "long_term";
if (["bridge", "dream_bridge", "cross_region", "cross"].includes(normalized)) return "bridge";
```

Fallback:

```ts
if (edgeType === "bridge") return "bridge";
if (origin === "dream" && edgeType === "temporal") return "working";
if (origin === "dream" || edgeType === "semantic") return "semantic";
if (origin === "archive" || origin === "cold" || edgeType === "source") return "long_term";
if (origin === "jsonld" || origin === "frontmatter" || edgeType === "parent" || edgeType === "related" || edgeType === "sequence") return "semantic";
return "working";
```

- [ ] **Step 4: Add graph modes**

Export:

```ts
export type GraphViewMode = "focus" | "three-zone" | "ai-triad";
```

Add UI segmented controls with labels:

```ts
[
  ["focus", "聚焦"],
  ["three-zone", "三区"],
  ["ai-triad", "三主体"],
]
```

Default is `focus`.

- [ ] **Step 5: Reduce irrelevant graph in focus mode**

Export helper:

```ts
export function filterGraphEdgesByViewMode<T extends GraphEdgeLike>(
  edges: T[],
  mode: GraphViewMode,
  focusNodeId?: string,
): T[] {
  if (mode !== "focus" || !focusNodeId) return edges;
  return edges.filter((edge) => edge.from === focusNodeId || edge.to === focusNodeId || graphMemoryRegion(edge) === "bridge");
}
```

Use it before building canvas links.

- [ ] **Step 6: Keep animation alive after simulation cools**

Keep the existing interval render loop but change it from a low-rate interval to an animation loop when visible:

```ts
const animate = () => {
  if (disposed) return;
  draw();
  frameId = window.requestAnimationFrame(animate);
};
frameId = window.requestAnimationFrame(animate);
```

Stop it in cleanup. This keeps dashed/animated memory edges moving after the D3 simulation cools.

- [ ] **Step 7: Run verification**

Run:

```powershell
npm test -- --run src/components/__tests__/GraphView.test.ts
```

Expected: graph helper tests pass.

## Task 6: Bridge/Diff Copy And Review Boundaries

**Files:**
- Modify: `src/components/BridgeInbox.tsx`
- Modify: `src/components/DiffView.tsx`
- Modify: `src/components/__tests__/BridgeInbox.test.tsx`
- Modify: `src/components/__tests__/DiffView.test.tsx`

- [ ] **Step 1: Write/extend tests**

Assert these strings render:

```ts
"Bridge Review 是 AI 输出进入人类权限前的提案审查"
"Diff Review 只处理已经形成 ghost/text block 的具体文本改动"
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```powershell
npm test -- --run src/components/__tests__/BridgeInbox.test.tsx src/components/__tests__/DiffView.test.tsx
```

Expected: fails until copy exists.

- [ ] **Step 3: Add concise header copy**

Bridge header copy:

```tsx
<p>Bridge Review 是 AI 输出进入人类权限前的提案审查；通过后才会进入写入、工作流补丁或 Diff Review。</p>
```

Diff empty/header copy:

```tsx
<p>Diff Review 只处理已经形成 ghost/text block 的具体文本改动；上游来源通常是 Bridge 提案。</p>
```

- [ ] **Step 4: Run verification**

Run:

```powershell
npm test -- --run src/components/__tests__/BridgeInbox.test.tsx src/components/__tests__/DiffView.test.tsx
```

Expected: copy boundary tests pass.

## Task 7: Backend Contract Audit And Targeted Rust Checks

**Files:**
- Inspect: `src-tauri/src/ai/dream_engine.rs`
- Inspect: `src-tauri/src/graph/link_graph.rs`
- Inspect: `src-tauri/src/fs/commands.rs`
- Modify only if tests or source contradict frontend contract.

- [ ] **Step 1: Search backend memory region emitters**

Run:

```powershell
rg -n "memory_region|dream_bridge|long_term|semantic|working|GraphEdge" src-tauri/src
```

Expected: identify every current memory-region producer.

- [ ] **Step 2: Add or confirm backend tests**

If Rust tests already cover Dream export mapping, run them. If not, add a test near `dream_engine.rs` that confirms:

```rust
assert_eq!(dream_memory_region(DreamConnectionType::Semantic), "semantic");
assert_eq!(dream_memory_region(DreamConnectionType::Temporal), "working");
assert_eq!(dream_memory_region(DreamConnectionType::Reference), "long_term");
assert_eq!(dream_memory_region(DreamConnectionType::Bridge), "dream_bridge");
```

- [ ] **Step 3: Run targeted backend verification**

Run:

```powershell
cargo test dream_memory_region --manifest-path src-tauri/Cargo.toml
```

Expected: targeted Rust test passes.

## Task 8: Browser Verification And Cleanup

**Files:**
- No intended source edits unless verification exposes a bug.

- [ ] **Step 1: Run frontend tests**

Run:

```powershell
npm test -- --run src/components/__tests__/AiTaskStrip.test.tsx src/components/__tests__/HumanTaskIntake.test.tsx src/components/__tests__/ContextMenu.test.tsx src/components/__tests__/GraphView.test.ts src/components/__tests__/ControlSurface.test.ts src/components/__tests__/AppLayout.test.ts
```

Expected: targeted tests pass.

- [ ] **Step 2: Run full frontend verification**

Run:

```powershell
npm test
npm run build
```

Expected: all frontend tests and Vite build pass.

- [ ] **Step 3: Use browser verification**

Open `http://127.0.0.1:4173/` or start preview/dev server if needed. Verify:

- AI footer has no CLI/card toggle and no inline terminal input.
- Floating CLI trigger remains visible and opens the CLI window.
- Human surface has visible task intake, bridge review, and diff review entries.
- GraphView can pan after waiting at least 20 seconds.
- GraphView modes show focus, three-zone, and AI-triad controls.
- Search/select/textarea fields no longer render as isolated white boxes in `classic`, `feroha`, and `latte`.

- [ ] **Step 4: Clean generated artifacts**

After verification, remove generated build artifacts only:

```powershell
Remove-Item -Recurse -Force dist
Remove-Item -Recurse -Force src-tauri/target
Remove-Item -Force vite-node.err.log,vite-node.out.log
```

Expected: no `dist`, `src-tauri/target`, or Vite log files remain unless they existed as user-owned unrelated artifacts and must be preserved.

## Self-Review Checklist

- Spec coverage: tasks cover footer CLI separation, unified controls, right-click behavior, human task intake, Bridge/Diff distinction, Dream three-zone graph, AI triad graph mode, backend contract audit, browser verification, and artifact cleanup.
- Placeholder scan: the plan contains no incomplete task markers beyond checkbox syntax.
- Type consistency: `TaskIntentType` remains imported from the existing CLI/task-intent helper module; `GraphMemoryRegion` is changed to `working | semantic | long_term | bridge`; `task-intake` is the new `ActivePanel` id.
- Execution mode: because the user requested continued iteration in the current workspace, implement inline with `superpowers:executing-plans`; do not create a new branch or commit unless explicitly requested.
