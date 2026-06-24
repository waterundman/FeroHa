# FeroHa Dual Surface Task And Memory Interaction Design

Date: 2026-06-08

Scope: next design stage for AI surface, human surface, task intake, command entry, mouse interaction, shared control styling, Dream three-zone memory folders, and graph/AI-triad data flow.

## Current Evidence

The current codebase already contains strong pieces, but their product boundaries are not yet clean.

- `src/App.tsx` keeps AI-only panels as editor, graph, tasks, cards, pipeline, plugins, settings; human panels as editor, inspiration, bridge, diff, settings.
- `src/components/CliBar.tsx` is mounted in the AI footer and mixes two jobs: command-card launcher and inline CLI input. `src/components/CliMiniWindow.tsx` already provides a separate floating CLI, so the footer creates duplicate command paths.
- `src/components/BridgeInbox.tsx` is downstream approval for AI outputs and workflow patches. `src/components/DiffView.tsx` is downstream block-level review for Ghost/text edits. There is no first-class human page for submitting a task to the AI surface.
- Many controls directly use `--bg-input` plus visible borders. In light or high-contrast themes this can read as isolated white boxes instead of an integrated app surface.
- `src-tauri/src/ai/dream_engine.rs` exports Dream graph edges with `memory_region` values `semantic`, `working`, `long_term`, and `dream_bridge`.
- `src/components/GraphView.tsx` currently maps graph display into `core`, `working`, `dream`, `archive`, and `bridge`. This is useful visually, but it diverges from Dream's three-zone memory intent and allows unrelated/demo information to crowd the graph.
- `docs/ai-face-triad-data-flow-2026-06-04.md` defines the AI triad as Manager, Scientist, and Orchestrator. It already states that human tasks and AI self-generated tasks flow through Manager, Scientist extracts claims/evidence, and Orchestrator audits memory/regression and emits verification tracks.

## Design Goal

Make FeroHa feel like one dual-surface note system:

- Human surface: authoring, visual thinking, task request, final approval, and text-change review.
- AI surface: memory organization, task execution state, triad observability, graph exploration, command-card/tool management, and workflow design.
- Bridge: the boundary where AI outputs ask for human authority.
- Dream memory: a three-zone protocol that drives folders, graph filters, and AI triad explanations.

## Design Alternatives

### Alternative A: Local UI Cleanup

Only restyle the bottom bar, search inputs, and resize handles. This is fast and low risk, but it does not solve duplicate task entry, missing human task intake, or Dream/Graph model drift.

### Alternative B: Interaction And Memory Convergence

Unify the command entry model, add a human task intake page, define shared control-surface tokens, add a cross-surface context-menu system, and normalize graph memory regions to Dream's three zones plus bridge overlay. This is the recommended path because it fixes the user's five reported symptoms at their shared cause: unclear surface boundaries and inconsistent memory protocol.

### Alternative C: Full Navigation Rewrite

Redesign every surface, sidebar, footer, and graph from scratch around a new navigation architecture. This may produce the cleanest end state, but it is too disruptive while the project has a large active 3.0.0 worktree.

Recommended approach: Alternative B.

## Surface Ownership

### Human Surface

Human surface owns actions where the human is the originator or final authority.

- Editor: write and read notes.
- Inspiration canvas: human thinking layout and spatial linking.
- AI Task Intake: human-to-AI request page. This is a new first-class human panel.
- Bridge Review: review AI output proposals before they affect notes, workflows, imports, or persistent memory.
- Diff Review: review concrete text modifications from Ghost/diff proposals after the Bridge path has identified a change as editable text.
- Settings: shared configuration, visible from both surfaces.

### AI Surface

AI surface owns AI operations, memory inspection, and tool/workflow configuration.

- GraphView: memory graph with Dream-zone filters and focus mode.
- AgentDashboard: AI Manager, Scientist, and Orchestrator status, task queue, Dream state, and data-flow inspection.
- Command Cards: AI capability library and instruction-card management.
- Pipeline/Workflow: AI workflow authoring and verifier flow.
- Plugins: AI tool extension status and capability management.
- Orchestrator Panel: compact AI runtime status.

## Footer Command Entry

The footer must stop behaving like a mode-switching terminal.

### New Footer Shape

When the app is in AI surface:

- Keep Orchestrator compact status.
- Replace the current `CliBar` inline CLI/card toggle with an "AI Task Strip".
- The strip has:
  - primary button: "指令卡任务" or "打开指令卡";
  - task type selector or compact intent chip;
  - brief risk/write-policy preview;
  - no inline command text field.
- `/` opens the command-card/task composer, not the inline CLI.
- `Ctrl+\`` and the floating terminal button open `CliMiniWindow`.

When the app is in human surface:

- The footer only shows global status plus the floating CLI trigger.
- Human-to-AI requests are made through the new AI Task Intake page, not through a hidden footer control.

### CLI Role

`CliMiniWindow` is the advanced console. It remains global, floating, draggable, and keyboard-driven. It is not the primary path for normal task submission.

## Unified Control Surface

Introduce a shared control style contract in `src/styles/feroha-theme.css`.

Required tokens:

- `--control-bg`: integrated field surface.
- `--control-bg-hover`: hover field surface.
- `--control-border`: low-contrast border.
- `--control-border-strong`: active/focused border.
- `--control-placeholder`: placeholder text color.
- `--control-shadow-focus`: focus ring.
- `--resize-handle-bg`: passive splitter surface.
- `--resize-handle-active`: active splitter surface.

Required classes:

- `.feroha-control`
- `.feroha-input`
- `.feroha-select`
- `.feroha-textarea`
- `.feroha-search`
- `.feroha-resize-handle`

Adoption targets:

- Graph search input.
- Command card search, sort, and parameter fields.
- Quick switcher input.
- Settings fields.
- Vault browser search/sort/inline rename.
- Pipeline property editor fields.
- App resize separators and editor split dividers.
- CLI mini-window input area.

Acceptance rule: no core search/input/select/textarea or split handle should appear as an isolated white rectangle in `feroha`, `classic`, or `latte`.

## Mouse Interaction Model

The app should treat mouse buttons as a note tool, not merely browser defaults.

### Global Rules

- Left click: select, open, drag, or execute the primary visible action.
- Double left click: open deeper view when an item has a detail page or note target.
- Right click: open a context menu relevant to the item under the pointer.
- Right click on empty space: open a surface-level menu for creation or navigation.
- Context menus must be keyboard dismissible with Escape and must not replace native text selection in text fields.

### Editor

- Left click: normal caret placement and selection.
- Right click on selected text: menu with "发送到 AI 任务", "总结选区", "改写为提案", "建立链接", "复制 Markdown".
- Right click without selection: keep native editing expectations and add note actions only when safe.

### Vault Browser

- Left click: open note.
- Double click: open note and focus editor.
- Right click file: "以此笔记提 AI 任务", "在图谱中聚焦", "复制路径", "收藏/取消收藏", "重命名".
- Right click folder: "新建笔记", "新建文件夹", "以此文件夹建立 AI 任务上下文".

### GraphView

- Left click node: select/focus.
- Double click node: open note.
- Drag node/canvas: layout interaction.
- Right click node: "打开笔记", "设为图谱焦点", "以节点提问 AI", "隐藏非相邻节点".
- Right click edge: "查看连接来源", "只看此记忆区", "隐藏此类连接".
- Right click empty graph: "重置视图", "显示三区总览", "清除过滤".

### Bridge Review

- Left click proposal: select and show detail.
- Right click proposal: "查看 trace", "打开差异审查", "复制证据", "归档".
- Approval and rejection remain explicit buttons.

### Diff Review

- Left click block: focus block.
- Right click block: "接受此块", "拒绝此块", "复制原文", "复制新文", "打开来源提案".
- Bulk accept/reject remains explicit toolbar action.

### Canvas And Pipeline

These tools already have mode-specific interactions. They should adopt the shared context-menu component but keep their tool-mode semantics.

## Human-To-AI Task Intake

Add a new human panel named "AI 委托" or "向 AI 提任务".

Purpose: make the human-to-AI flow visible and separate from downstream review.

Fields:

- Task title.
- Task type selector using the same `TaskIntentType` choices as the AI task system.
- Scope: current note, selected text, folder, graph focus, or freeform context.
- Expected output.
- Risk/write policy preview.
- Review mode:
  - manual Bridge review;
  - auto queue for read-only tasks;
  - draft only.

Data flow:

1. Human creates a task request.
2. Frontend builds the same dispatch payload currently used by command cards or `submit_task`.
3. Backend Manager receives the request through `dispatch_agent_task` or `submit_task`.
4. `TaskIntentType` chooses sandbox policy and Bridge requirement.
5. If review is required, the result appears in Bridge Review.
6. If the result is concrete text modification, Bridge can navigate to Diff Review.

Distinction:

- AI Task Intake is upstream request creation.
- Bridge Review is downstream approval of AI output or workflow patch.
- Diff Review is downstream line/block-level acceptance of concrete text changes.

## Dream Three-Zone Memory Protocol

Normalize the product model to three memory zones and a bridge overlay.

### Zone 1: Working Memory

Meaning: active notes, current task context, recent human edits, temporal Dream connections, short-lived context fragments.

Folder:

- `.dualtrack/memory/working/`

Typical producers:

- Human Task Intake.
- AI Manager task queue.
- recent context fragments.
- temporal Dream links.

Graph behavior:

- high motion speed;
- visible by default around the focused note/task;
- fades when outside the focus radius.

### Zone 2: Semantic Memory

Meaning: structured concepts, JSON-LD indexes, extracted claims, evidence chains, proposition consistency records, stable entity/relation graph.

Folder:

- `.dualtrack/memory/semantic/`

Typical producers:

- JSON-LD indexer.
- AI Scientist claim/evidence extraction.
- PropositionKernel or LeanLite consistency summaries.

Graph behavior:

- stable layout anchors;
- parent/related/semantic edges;
- default visible when graph is in "结构" or "AI 三主体" mode.

### Zone 3: Long-Term Memory

Meaning: consolidated Dream insights, reference/source edges, archived or cooled material, older but retained task outputs.

Folder:

- `.dualtrack/memory/long_term/`

Typical producers:

- Dream cycle after consolidation.
- source/reference edges.
- archived MDT/JSON-LD compatibility artifacts.

Graph behavior:

- slow motion;
- muted by default unless focus node connects into it;
- visible in "长期记忆" and "三区总览" modes.

### Bridge Overlay

`dream_bridge` is not a fourth memory zone. It is a cross-zone connection type.

Folder:

- `.dualtrack/memory/bridges/`

Graph behavior:

- animated cross-zone edge;
- color distinct from the three zones;
- shown when it explains why two zones are linked.

## GraphView Requirements

GraphView must stop treating all available graph data as equally relevant.

Required modes:

- Focus Mode: default. Shows current note/task, direct neighbors, and zone bridge edges.
- Three-Zone Overview: shows working, semantic, and long-term clusters.
- AI Triad Mode: overlays Manager, Scientist, and Orchestrator roles on graph regions.

Required filters:

- zone filters: working, semantic, long-term;
- bridge overlay toggle;
- edge-type filters remain available but are secondary;
- "hide demo graph when real graph has nodes";
- "hide low-confidence unrelated edges" default on.

Backend normalization:

- Convert Dream `memory_region` values:
  - `working` -> working;
  - `semantic` -> semantic;
  - `long_term` -> long-term;
  - `dream_bridge` -> bridge overlay.
- Convert JSON-LD structure edges to semantic memory unless explicitly tagged otherwise.
- Convert wikilinks around active human notes to working memory.
- Convert source/reference/archive edges to long-term memory.

AI triad relation:

- Manager primarily reads and writes working memory task state.
- Scientist primarily produces semantic memory claims/evidence.
- Orchestrator primarily audits semantic plus long-term memory and produces bridge/verification tracks.
- Dream connects all three by moving material from working through semantic into long-term and by creating bridge overlay edges.

## Backend Data Contracts

Add or normalize the following contracts before UI relies on them:

- `MemoryZone`: `working`, `semantic`, `long_term`.
- `MemoryBridgeKind`: at minimum `dream_bridge`, with room for future cross-zone bridges.
- `GraphEdge.memory_region`: should carry zone value or bridge overlay value.
- `GraphNode.memory_zone`: optional first-class field derived from folder, frontmatter, JSON-LD metadata, or graph source.
- `TaskIntentType` should remain the shared intake type for command cards, CLI, and Human Task Intake.
- AI Manager snapshot should include counts grouped by memory zone when available.
- Dream status should expose latest zone movement summary: working processed, semantic consolidated, long-term retained, bridge edges created.

## Frontend Components To Add Or Change

Add:

- `src/components/HumanTaskIntake.tsx`
- `src/components/ContextMenu.tsx`
- `src/components/context-menu.css` or theme-level classes
- tests for Human Task Intake payloads and context menu routing

Change:

- `src/App.tsx`: add human panel for AI Task Intake and update human navigation.
- `src/components/CliBar.tsx`: rename/reframe as AI Task Strip or split into a smaller component; remove inline CLI mode.
- `src/components/CliMiniWindow.tsx`: keep as sole CLI command surface; fix labels and integrate control styles.
- `src/components/GraphView.tsx`: zone modes, focus mode, Dream zone mapping, triad overlay.
- `src/components/BridgeInbox.tsx`: add explanatory copy showing it is downstream output approval.
- `src/components/DiffView.tsx`: add explanatory copy showing it is concrete text change review.
- `src/styles/feroha-theme.css`: add control and resize tokens/classes.
- `src-tauri/src/ai/dream_engine.rs`: keep Dream exports but align region names or document mapping.
- `src-tauri/src/fs/commands.rs` and `src-tauri/src/graph/link_graph.rs`: add node/edge zone normalization where graph data is built.

## Testing And Verification

Frontend tests:

- `AppLayout.test.ts`: human panel list includes AI Task Intake; Bridge and Diff remain review-only human panels.
- `CliBar.test.tsx` or new `AiTaskStrip.test.tsx`: footer no longer shows inline CLI input; `/` opens command-card/task composer; `Ctrl+\`` is the CLI path.
- `HumanTaskIntake.test.tsx`: task type, scope, expected output, review mode produce correct dispatch payload.
- `GraphView.test.ts`: Dream three-zone mapping, bridge overlay mapping, focus-mode edge reduction, no demo graph when real nodes exist.
- `ContextMenu.test.tsx`: right-click on note/graph/proposal/diff block opens correct menu items.
- style contract test: shared control classes exist and are used by key search/input components.

Backend tests:

- Dream export maps connection types into `working`, `semantic`, `long_term`, and `dream_bridge`.
- Graph rebuild tags JSON-LD structure as semantic memory.
- Human task dispatch payload still routes through `TaskIntentType` and sandbox policy.
- AI Manager snapshot reports memory-zone task counts when data exists.

Browser verification:

- AI footer shows task strip and no inline terminal input.
- Floating CLI opens independently.
- Search inputs and split handles blend into `feroha`, `classic`, and `latte`.
- Human surface shows AI Task Intake, Bridge Review, and Diff Review as separate entries.
- Graph default view is focused and not crowded by unrelated demo nodes.
- Three-zone overview shows working, semantic, long-term, and bridge overlay distinctly.

## Acceptance Criteria

1. The footer has no CLI/card mode toggle. Command cards/task intake and CLI are separate.
2. The floating CLI is the only terminal-like command input.
3. Core controls use shared control tokens and do not appear as isolated white boxes.
4. Right-click context menus exist for editor selections, vault files, graph nodes/edges, bridge proposals, and diff blocks.
5. The human surface contains a visible AI Task Intake page.
6. Bridge Review and Diff Review have distinct names, descriptions, and data-flow boundaries.
7. Dream memory is normalized to working, semantic, and long-term zones, with dream bridges as overlays.
8. GraphView defaults to a focused, relevant view and offers a three-zone overview.
9. AI triad display explains how Manager, Scientist, and Orchestrator relate to the three memory zones.
10. The implementation is covered by targeted frontend and backend tests and verified in the browser.

## Implementation Order

1. Add shared control tokens and update a small set of representative controls.
2. Split footer command entry from floating CLI.
3. Add Human Task Intake panel and dispatch payload tests.
4. Add context menu infrastructure and implement the first five target menus.
5. Normalize Dream three-zone graph mapping in tests and backend/frontend helpers.
6. Add GraphView focus/overview modes and triad overlay.
7. Update Bridge/Diff explanatory copy and navigation labels.
8. Run full frontend tests, targeted backend tests, build, browser verification, and cleanup of generated artifacts.

## Spec Self-Review

- Scope is large but cohesive: every section serves the same dual-surface and memory-protocol convergence.
- No implementation code should be written before this spec is reviewed.
- No placeholder sections remain.
- The design preserves existing backend contracts where possible and defines explicit normalization instead of replacing the whole architecture.
- The implementation should be split into separate TDD tasks before editing production code.
