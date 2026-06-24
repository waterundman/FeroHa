# Backend Flow Audit - 2026-06-01

## Scope

Objective: audit whether backend logic flows are wired end to end and whether they converge into durable state, events, proposals, traces, or explicit errors.

Current evidence inspected:

- Runtime entry: `src-tauri/src/main.rs`
- Shared state: `src-tauri/src/state.rs`
- Vault/file flow: `src-tauri/src/fs/commands.rs`
- Agent/task flow: `src-tauri/src/ai/commands.rs`, `src-tauri/src/ai/agent_scheduler.rs`
- Research flow: `src-tauri/src/ai/subagent.rs`, `src-tauri/src/ai/research_graph.rs`, `src-tauri/src/ai/research_trace.rs`
- Bridge/Ghost flow: `src-tauri/src/bridge/commands.rs`, `src-tauri/src/bridge/proposal.rs`, `src-tauri/src/diff/ghost_store.rs`
- Memory graph flow: `src-tauri/src/graph/link_graph.rs`, `src-tauri/src/ai/dream_engine.rs`, `src-tauri/src/mdt/*`
- IPC call sites sampled from `src/**/*.tsx` and `src/lib/*.ts`

## Flow Matrix

| Flow | Entry | State/Service Path | Convergence Target | Status |
|---|---|---|---|---|
| Tauri command surface | `main.rs::invoke_handler` | Direct command registration | Callable IPC commands | Connected; module `register_commands` helpers are not runtime entry |
| Vault open | `open_vault` | `initialize_vault_services` -> `VaultManager`, `.dualtrack`, `VectorStore`, `SyncEngine`, `GhostStore`, `SnapshotEngine`, `DreamEngine`, `SearchEngine`, `BridgeProposalStore`, `OutputManager`, then watcher/listeners/worker/scheduler | Initialized `AppState`/`AiState` plus runtime side effects | Hardened this pass; core service wiring is regression-tested, snapshot listeners are registered synchronously before return, task worker is single-start, old scheduler is stopped before replacement, and FTS initializes independently from watcher startup |
| Note save/create/delete/rename/assets | `save_note/create_note/delete_note/rename_note/save_asset` | `VaultManager` then `process_file_event` for note writes | note/asset file, vector sync, FTS update, graph rebuild | Hardened this pass; all external paths are vault-relative, existing files can be overwritten on Windows, and content note events now converge into vector, FTS, and graph state immediately |
| Missing outgoing note creation | `BacklinksPanel` -> `save_note` | `save_note` | note file + graph/vector refresh | Fixed this pass; was calling non-existent `write_note` |
| Graph build | `rebuild_link_graph` | wikilinks + MDT frontmatter + Dream projected edges | `AppState.link_graph` then `get_graph/get_graph_with_focus` | Connected |
| MDT read/index/archive | `mdt_validate/mdt_index/mdt_read/mdt_pack/mdt_unpack`, typed scheduler intents | `mdt::{indexer,reader,archive}` | validation report, generated indexes, context bundle, `.mdtz`, task trace | Hardened this pass; direct commands and approved `MdtIndex/MdtRead/MdtPack` worker tasks now converge |
| Task submit via CLI bar | `submit_task(command, taskType)` | parse CLI -> task intent -> sandbox -> scheduler pending -> Bridge proposal | pending task and proposal | Connected; intent/content now preserved |
| Legacy selection submit | `submit_task(task)` | legacy payload normalized -> `dispatch_agent_task` | task/proposal via standard dispatch path | Fixed this pass; old IPC shape was not accepted |
| Command card dispatch | `dispatch_agent_task` | task intent -> sandbox -> pending Bridge or auto-approve | scheduler queue and events | Connected; `source_block_id/blockId` now preserved |
| Bridge approval/reject | `execute_bridge_action` | proposal store -> scheduler approve/reject or ghost reject | task queue notification, proposal update, UI events | Hardened this pass; missing approval handler no longer mutates proposal state |
| Worker execution | `start_task_worker` | scheduler dequeue -> `execute_agent_task_async` | result trace, task status, completion event, task evidence fragment, normalized retrieval evidence | Hardened this pass; completion now marks subtasks done and stores trace context/evidence on the task, with trace files under the active `.dualtrack/research` root |
| Retry path | `AgentScheduler::fail` | requeue approved low-priority retry | task remains discoverable and can converge | Fixed this pass; retry task was previously removed from task map |
| Search/explain/summarize/fetch papers | `execute_agent_task_async` branches | local vector/LLM/subagent -> `TaskContext.retrieval_evidence` -> scheduler `subagent_results` | markdown output, trace, events, optional ghost proposal, task evidence for Scientist | Hardened this pass; retrieval evidence now converges into task records |
| Deep research | `CliCommand::DeepResearch` | subagent stages -> local vector/web -> research graph -> trace/evidence -> Scientist proposal | report, graph artifacts, trace, Bridge proposal, task evidence | Hardened this pass; local vector store and retrieval results now flow into Scientist-ready task evidence, with graph artifacts under `.dualtrack/research/graphs` |
| Deep research tool | `ToolRegistry::DeepResearchTool` | tool call -> subagent stages -> local vector/web -> research graph | tool result metadata and report | Fixed this pass; old tuple return and missing vector store were disconnected |
| Dream cycle | `trigger_dream` and CLI `Dream` | `DreamEngine::run_cycle` -> graph edge projection -> Bridge proposal/output hook | Dream stats, insights, graph edges, proposal | Connected |
| Scientist/PropositionKernel | `translate_research`, `verify_proposition_graph`, deep research refine | translator -> PropositionKernel | verification result and Bridge proposal | Connected |
| Output hooks | `list_output_hooks/add_output_hook`, Scientist and Dream triggers | `OutputManager` -> file sink, webhook, or stdio target | hook list or external output sink | Hardened this pass; adding hooks now uses `Arc` copy-on-write instead of panicking when the manager is shared |
| Skill manager | `open_vault` initializes, `list_skills` exposes definitions, internal `execute_skill` paths | `SkillManager` -> skill-specific vault artifacts | skill list or vault-scoped artifacts/errors | Hardened this pass; write/research path parameters are constrained to safe vault-relative paths |
| Plugin manager | `plugin_status`, internal install/discover/load lifecycle | `PluginManager` -> `PluginLoader` -> plugin manifests/WASM | status JSON or installed plugin metadata/errors | Hardened this pass; archive install validates against temp/extracted plugin dir, plugin names and WASM entries are path-safe |
| Ghost/Diff review | ghost creation paths, `record_ghost_feedback`, diff commands | `GhostStore`, merge/diff commands, Bridge actions, file event processing | ghost state, trust score, diff events, vector/FTS/graph refresh | Hardened this pass; accepted diffs now converge through the same file event path as note saves, and ghost IDs/source notes are path-safe |
| Snapshot | `note-opened`, `selection-submit`, snapshot commands | listener -> state-backed snapshot handler -> snapshot engine/store | current snapshot and diffs | Hardened this pass; event listeners are single-start and synchronously registered across repeated vault opens, and global/local snapshot event handlers now have regression coverage for store convergence |
| File watcher | `FileWatcher::watch` | notify event -> content-path filter -> `process_file_event` -> `file-changed` event | content-note cache/vector/FTS/graph refresh | Hardened this pass; hidden/internal `.dualtrack` and underscore-prefixed markdown artifacts no longer enter user-note indexes, and real notify events now have runtime smoke coverage |
| Tool registry sandbox | Custom card tool loop | `ToolRegistry::get_allowed` + param validation | allowed tool result or blocked metadata | Hardened this pass; `ghost_write` target notes and sandbox dot-root reads reject path escapes |
| Scheduled Dream | `TaskScheduler` from `open_vault` | periodic task -> scheduler -> Bridge proposal -> scheduler status | pending auto dream task and observable run status | Hardened this pass; repeated vault opens stop the old scheduler before replacement, and scheduler status now reports real last-run/next-run data |

## Fixes Applied In This Audit Pass

1. `submit_task` now accepts both modern `{ command, taskType }` and legacy `{ task: {...} }` payloads. Legacy payloads are normalized into the standard `dispatch_agent_task` path.
2. Write-oriented card aliases such as `rewrite`, `correct`, `expand`, `translate`, `simplify`, `format`, and `extract` now map to `TaskIntentType::WriteProposal`, so they require Bridge review instead of falling through to generic research.
3. `AgentScheduler::fail` now keeps retry tasks in the task map while requeueing, so retries can still be listed, cancelled, completed, audited, and traced.
4. `dispatch_agent_task` now preserves `source_block_id` / `blockId`.
5. `BacklinksPanel` now calls the registered `save_note` command instead of the missing `write_note`.
6. Deep Research now opens the current vector store and passes it into `Subagent::execute_deep_research`, restoring local-note retrieval inside the research loop.
7. `DeepResearchTool` now consumes the new `DeepResearchOutcome` and returns graph/ghost metadata instead of trying to destructure the old `(report, ghost_ids)` tuple.
8. `open_vault` now starts the task worker only once and stops any previous periodic scheduler before installing a new one.
9. Scheduler submit now decomposes ordinary tasks into subtasks, giving Scientist/Orchestrator claim material instead of empty audit inputs.
10. Worker completion now records `TaskContext` as an agent context fragment, marks subtasks done, and sets `has_trace` on the task before orchestration audit runs.
11. `TaskContext` now carries normalized `retrieval_evidence`; scheduler completion converts that evidence into task `subagent_results`.
12. Search, Explain/DeepDive, Summarize, FetchPapers, Custom research, and DeepResearch now attach their actual retrieval evidence to task context.
13. DeepResearch's Scientist proposal path now enriches the cloned task with execution context before refinement, avoiding stale empty claim/evidence inputs.
14. Bridge `ApproveTask` now checks that an approval handler exists before mutating proposal status, preventing failed actions from leaving proposals stuck as approved.
15. `accept_diff` now calls the shared `process_file_event` path after writing a note, so accepted ghost changes refresh cache, vector sync, full-text search, and graph state immediately.
16. Direct `accept_diff`/`reject_diff` now emit `ghost-updated`; accepted diffs also emit `file-changed`.
17. Snapshot event listeners for `note-opened` and `selection-submit` are now started once instead of being duplicated on every `open_vault`.
18. `add_output_hook` now mutates `OutputManager` through `Arc::make_mut`, preventing a panic when another runtime path already holds an `Arc` clone.
19. Research trace paths now write/read under the passed `.dualtrack/research/...` root instead of nesting a second `.dualtrack/.dualtrack/...` directory.
20. Approved `MdtIndex`, `MdtRead`, and `MdtPack` scheduler tasks now execute MDT index/read/archive work directly instead of falling through to unrelated CLI command branches.
21. `MdtPack` worker tasks constrain custom archive paths to safe relative paths under `.dualtrack/mdt/snapshots`, rejecting absolute paths and `..` escapes.
22. DeepResearch graph artifacts now write under the same `.dualtrack/research/graphs/...` root instead of `.dualtrack/.dualtrack/research/graphs/...`.
23. `SkillManager` now rejects unsafe vault-relative paths for markdown write skills and deep-research path topics, preventing `../` and absolute-path traversal outside the vault.
24. `VaultManager::save_asset` and `VaultManager::rename_note` now use the same safe relative path normalization as note read/write/delete/create-folder.
25. `VaultManager` now replaces existing note and asset files through a shared helper, avoiding Windows `fs::rename(temp, existing_target)` failures during ordinary overwrite saves.
26. `PluginLoader::install_from_archive` now validates manifests against the extracted temporary plugin directory before copying, so new plugin installs can pass validation without an already-installed copy.
27. Plugin manifest names and WASM entry paths now reject path separators, absolute paths, prefixes, and parent-directory traversal.
28. `GhostStore` now validates ghost IDs before read/write/status paths, preventing crafted `ghost_id` values from escaping `.dualtrack/ghosts`.
29. `GhostStore` now normalizes and validates `source_note` during ghost creation/list/conflict filtering, so invalid target notes cannot create reviewable-but-unacceptable ghost proposals.
30. `ToolRegistry` now validates `ghost_write.target_note` as a safe vault-relative path before execution, and the sandbox no longer treats `.` read roots as permission to read parent or absolute paths.
31. `FileWatcher` and `process_file_event` now ignore unsafe, hidden, underscore-prefixed, non-Markdown, and `.dualtrack` internal paths, preventing generated research traces and other backend artifacts from polluting user-note vector/FTS/graph indexes.
32. `SearchEngine::commit` now explicitly reloads its reader after writer commit, so a note saved through `process_file_event` is immediately searchable instead of depending on delayed reader reload timing.
33. `SearchEngine::index_all_md_files` now uses the same content-note path filter as `FileWatcher` and `process_file_event`, so initial FTS scans cannot index `.dualtrack`, hidden, underscore-prefixed, unsafe, or non-content Markdown paths.
34. `open_vault` now initializes `SearchEngine` outside the watcher-startup branch, so full-text search remains available even if filesystem watching cannot start.
35. `open_vault` now delegates pure backend state setup to `initialize_vault_services`, separating testable service convergence from AppHandle-dependent runtime side effects. A regression test now proves vault open wires the vault manager, vector store, FTS, link graph, bridge store, snapshots, dream engine, protocol, output manager, ghost store, subagent, and skill manager while keeping internal/private Markdown out of user indexes.
36. `TaskScheduler` now records each cron job run and reports real `last_run_at` plus remaining `next_run_secs` through `get_scheduler_status`, so scheduled Dream has an observable runtime convergence signal instead of static placeholder status.
37. Snapshot event handling now routes through state-backed helpers that can run with or without an `AppHandle`, so `note-opened` and `selection-submit` convergence into snapshot storage is directly testable while runtime drift events still emit when an app handle is available.
38. `FileWatcher` now has runtime smoke coverage proving the real notify backend emits vault-relative events for content Markdown while suppressing internal `.dualtrack` Markdown before it can reach `process_file_event`.
39. `open_vault` now registers `note-opened` and `selection-submit` listeners synchronously instead of spawning listener registration onto the async runtime, eliminating the race where frontend events emitted immediately after vault open could be lost.

## Verification Performed

- `rustfmt --edition 2021 --check` passed for changed backend Rust files, including the new GhostStore/ToolRegistry/Sandbox hardening.
- `cmd /c 'call "C:\BuildTools\Common7\Tools\VsDevCmd.bat" -arch=x64 && set "CARGO_TARGET_DIR=target-audit-nodebug" && set "RUSTFLAGS=-C debuginfo=0" && cargo test --manifest-path src-tauri\Cargo.toml -- --test-threads=1'` passed after installing Visual Studio Build Tools: lib tests, bin tests, and doc-tests completed successfully. The main backend library suite reported 285 passing tests; the second binary test target reported 279 passing tests and 0 failures.
- New scheduler observability regression passed: `status_reports_last_run_and_remaining_interval`.
- New snapshot event convergence regressions passed: `global_snapshot_event_logic_stores_note_and_backlink_context` and `local_snapshot_event_logic_clamps_selection_and_stores_snapshot`.
- New file watcher runtime smoke passed: `file_watcher_emits_content_markdown_events_and_skips_internal_paths`.
- New runtime convergence regressions passed: `initialize_vault_services_wires_core_backend_state_and_indexes_content_only`, `process_file_event_updates_vector_fts_and_graph_for_content_note`, `process_file_event_ignores_dualtrack_internal_markdown`, `content_markdown_path_filter_excludes_internal_and_unsafe_paths`, and `index_all_md_files_excludes_internal_and_private_markdown`.
- `npm.cmd run test -- src/components/__tests__/CliBar.test.tsx src/components/__tests__/BridgeInbox.test.tsx` passed: 11 tests.
- `node_modules\.bin\tsc.cmd --noEmit` passed.
- `git diff --check` exited successfully; it only reported Git LF/CRLF normalization warnings.
- Command-surface cross-check found no frontend `invoke(...)` calls missing from `main.rs::invoke_handler`, and no `#[tauri::command]` functions missing from the runtime handler.

Verification notes:

- The earlier `RC.EXE`/`link.exe` blocker has been resolved by installing Visual Studio Build Tools under `C:\BuildTools` and loading `VsDevCmd.bat` before Cargo commands.
- A no-window Wry `AppHandle` unit smoke was attempted, but this Windows environment fails before Rust test execution with `STATUS_ENTRYPOINT_NOT_FOUND`: the test exe imports `WaitOnAddress` / `WakeByAddress*` from `api-ms-win-core-synch-l1-2-0.dll`, while `C:\Windows\System32\api-ms-win-core-synch-l1-2-0.dll` does not export those symbols. `C:\Windows\System32\downlevel` and Build Tools contain forwarder DLLs, but Windows API-set resolution still loads the incompatible system DLL. This is an environment/runtime loader limit, not a backend logic failure.
- Parallel Cargo test runs on Windows can still contend on `.pdb` files and fail with `LNK1201`. Serial runs with `RUSTFLAGS=-C debuginfo=0` avoid generating huge PDBs and produced the successful full backend test evidence above.

## Remaining Audit Risks

1. `execute_cli` intentionally submits pending Bridge-reviewed tasks rather than executing immediately. The UX name may be misleading, but the backend flow is coherent if Bridge approval is expected.
2. Regression auditing now receives trace context fragments, subtasks, and normalized retrieval evidence, and the corresponding scheduler/research trace tests now pass locally.
3. Core open-vault backend service convergence, snapshot event handling, scheduler status, and notify-backed file watcher event conversion are covered by Rust tests. Full no-window Wry `AppHandle` unit proof is blocked on the Windows API-set loader issue above; the remaining runtime check should be performed through the actual Tauri app on a compatible Windows runtime or after repairing the system API-set/UCRT installation.
