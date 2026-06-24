# AI Face Read-Only and Task Context Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the AI surface read-only for files, add human text-selection right-click task dispatch, integrate custom app chrome, and show a restrained API debug success effect.

**Architecture:** Keep the existing React/Tauri structure. Extend shared UI primitives such as `ContextMenu`, `VaultBrowser`, `Editor`, and `SettingsPanel`; add only the minimum Tauri command needed for API health testing.

**Tech Stack:** React 18, Zustand, CodeMirror, Tauri 2, Rust, Vitest, Cargo tests, Codex in-app Browser.

---

### Task 1: AI File Browser Read-Only Behavior

**Files:**
- Modify: `src/components/VaultBrowser.tsx`
- Test: `src/components/__tests__/VaultBrowser.test.tsx`

- [ ] **Step 1: Write failing tests**

Add tests that seed `.dualtrack/Research.md`, open its context menu, and assert AI files expose read-only actions while hiding create, rename, and delete actions. Add a second test that verifies a human note still exposes writable actions.

- [ ] **Step 2: Run red test**

Run: `npm.cmd test -- src/components/__tests__/VaultBrowser.test.tsx`

Expected: FAIL because AI file menus still include rename and delete.

- [ ] **Step 3: Implement read-only menu**

Detect AI notes by `.dualtrack/` path. For AI files, keep open/preview, copy path, graph focus, and task context actions. Remove rename/delete/new note/new folder for AI paths.

- [ ] **Step 4: Run green test**

Run: `npm.cmd test -- src/components/__tests__/VaultBrowser.test.tsx`

Expected: PASS.

### Task 2: Human Selection Right-Click Task Dispatch

**Files:**
- Modify: `src/components/Editor.tsx`
- Modify: `src/components/ContextMenu.tsx`
- Test: `src/components/__tests__/Editor.test.tsx`
- Test: `src/components/__tests__/ContextMenu.test.tsx`

- [ ] **Step 1: Write failing tests**

Assert `Editor.tsx` contains a context-menu selection dispatch path using `sendTaskToAgent`, and assert `ContextMenu` supports separators, shortcuts, and destructive item styles.

- [ ] **Step 2: Run red tests**

Run: `npm.cmd test -- src/components/__tests__/Editor.test.tsx src/components/__tests__/ContextMenu.test.tsx`

Expected: FAIL because the editor lacks a right-click task context menu and the menu primitive lacks separator/shortcut metadata.

- [ ] **Step 3: Implement right-click menu**

On editor right-click with selected text, show a context menu containing “提交给 AI”, “分析选区”, “改写选区”, and “复制选区”. Dispatch the main task through `sendTaskToAgent` with `taskType: "research"` and include note path in the intent.

- [ ] **Step 4: Run green tests**

Run: `npm.cmd test -- src/components/__tests__/Editor.test.tsx src/components/__tests__/ContextMenu.test.tsx`

Expected: PASS.

### Task 3: API Save and Debug Feedback

**Files:**
- Modify: `src/hooks/useSettings.ts`
- Modify: `src/components/SettingsPanel.tsx`
- Modify: `src/App.tsx`
- Modify: `src/styles/feroha-theme.css`
- Modify: `src-tauri/src/ai/commands.rs`
- Modify: `src-tauri/src/main.rs`
- Test: `src/hooks/__tests__/useSettings.test.ts`
- Test: `src/components/__tests__/SettingsPanel.test.tsx`

- [ ] **Step 1: Write failing tests**

Add frontend tests for a save-and-debug button and an exported API success event helper. Add Rust tests for a debug response sanitizer that never includes the API key.

- [ ] **Step 2: Run red tests**

Run: `npm.cmd test -- src/hooks/__tests__/useSettings.test.ts src/components/__tests__/SettingsPanel.test.tsx`

Expected: FAIL because the helpers and UI are missing.

- [ ] **Step 3: Implement API debug**

Expose `debug_llm_config` from Tauri. In settings, call `set_config`, then `debug_llm_config`. On success, emit a document event that `App` maps to a temporary success class. Add CSS for the subtle border particle effect and reduced-motion fallback.

- [ ] **Step 4: Run green tests**

Run: `npm.cmd test -- src/hooks/__tests__/useSettings.test.ts src/components/__tests__/SettingsPanel.test.tsx`

Expected: PASS.

### Task 4: Integrated App Chrome and Taste Cleanup

**Files:**
- Modify: `src-tauri/tauri.conf.json`
- Modify: `src/App.tsx`
- Modify: `src/styles/feroha-theme.css`
- Test: `src/components/__tests__/AppLayout.test.ts`

- [ ] **Step 1: Write failing tests**

Assert the app exports chrome control metadata and the Tauri config disables native decorations.

- [ ] **Step 2: Run red tests**

Run: `npm.cmd test -- src/components/__tests__/AppLayout.test.ts`

Expected: FAIL because chrome controls are not integrated and Tauri decorations remain default.

- [ ] **Step 3: Implement custom chrome**

Add an integrated top bar with drag region and window controls using `@tauri-apps/api/window`. Reduce icon hover glow, uppercase labels, and context-menu over-bright hover states.

- [ ] **Step 4: Run green test**

Run: `npm.cmd test -- src/components/__tests__/AppLayout.test.ts`

Expected: PASS.

### Task 5: Full Verification

**Files:**
- Existing test and app files only.

- [ ] **Step 1: Run frontend and backend checks**

Run: `npm.cmd run build`, `cargo test -p feroha --lib -- --test-threads=1`, and targeted Vitest suites.

- [ ] **Step 2: Browser simulation**

Start the full app if needed, use the Codex in-app Browser to verify file menus, editor right-click task dispatch, settings API debug feedback, and integrated chrome. Clear relevant browser storage/cache after testing.
