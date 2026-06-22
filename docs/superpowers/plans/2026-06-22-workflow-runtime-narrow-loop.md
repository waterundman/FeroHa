# Workflow Runtime Narrow Loop Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Persist `WorkflowCreate` runs and execute ready research steps exactly once through the existing real Agent task worker while unsupported capabilities remain explicit and auditable.

**Architecture:** Add a vault-backed runtime store in `harness`, a research-only task adapter and stateless runtime service in `ai`, then connect three thin Tauri commands and the existing worker lifecycle. Reuse `AgentScheduler`, vector retrieval, Subagent, LLM, Scientist, trace, and Bridge paths; do not build a second executor or store operational state in Dream memory.

**Tech Stack:** Rust, Serde, tempfile, Tauri 2 commands/events, Tokio task worker, TypeScript, Zustand, Vitest.

---

## File Structure

- Create `src-tauri/src/harness/workflow_runtime.rs`: runtime bundle types, dispatch statuses, task context, and vault-backed atomic store.
- Modify `src-tauri/src/harness/workflow.rs`: expose the existing safe runtime component validator and add lifecycle event constructors.
- Modify `src-tauri/src/harness/mod.rs`: export `workflow_runtime`.
- Create `src-tauri/src/ai/workflow_task_adapter.rs`: deterministic research task conversion and context extraction.
- Create `src-tauri/src/ai/workflow_runtime_service.rs`: start, resume, reconcile, completion, and failure transitions.
- Modify `src-tauri/src/ai/mod.rs`: export the two focused AI runtime modules.
- Modify `src-tauri/src/ai/agent_scheduler.rs`: expose the existing event recorder to the service; do not move persistence into this file.
- Modify `src-tauri/src/ai/commands.rs`: add thin commands and worker lifecycle callbacks.
- Modify `src-tauri/src/fs/commands.rs`: resume persisted running workflows after vault services initialize.
- Modify `src-tauri/src/main.rs`: register runtime commands.
- Modify `src/types/orchestrator.ts`: mirror runtime request/response types.
- Modify `src/hooks/useAppStore.ts`: add start/get/resume actions without a competing runtime store.
- Modify `src/hooks/__tests__/useAppStore.workflow.test.ts`: verify IPC payloads.
- Modify `src/lib/orchestratorEventPresentation.ts`: label new runtime events.
- Modify `src/lib/__tests__/orchestratorEventPresentation.test.ts`: verify labels and details.

### Task 1: Runtime Bundle And Atomic Store

**Files:**
- Create: `src-tauri/src/harness/workflow_runtime.rs`
- Modify: `src-tauri/src/harness/workflow.rs`
- Modify: `src-tauri/src/harness/mod.rs`

- [ ] **Step 1: Write failing store and path-safety tests**

Add tests in `workflow_runtime.rs` for a round trip, corrupt JSON, unsafe run IDs, and run listing:

```rust
#[test]
fn runtime_store_round_trips_bundle_and_lists_run() {
    let root = tempfile::tempdir().unwrap();
    let store = WorkflowRuntimeStore::new(root.path());
    let bundle = runtime_bundle("run_demo");

    store.write(&bundle).unwrap();

    assert_eq!(store.read("run_demo").unwrap(), bundle);
    assert_eq!(store.list_run_ids().unwrap(), vec!["run_demo"]);
}

#[test]
fn runtime_store_rejects_unsafe_run_id_before_writing() {
    let root = tempfile::tempdir().unwrap();
    let store = WorkflowRuntimeStore::new(root.path());
    let mut bundle = runtime_bundle("../escape");
    bundle.run.run_id = "../escape".to_string();

    assert!(matches!(
        store.write(&bundle),
        Err(WorkflowError::UnsafeRuntimeComponent(_))
    ));
    assert!(!root.path().join("escape").exists());
}

#[test]
fn runtime_store_reports_corrupt_json_without_overwriting_it() {
    let root = tempfile::tempdir().unwrap();
    let store = WorkflowRuntimeStore::new(root.path());
    let path = store.runtime_path("run_demo").unwrap();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "{not-json").unwrap();

    assert!(matches!(
        store.read("run_demo"),
        Err(WorkflowError::RuntimeStateParse(_))
    ));
    assert_eq!(std::fs::read_to_string(path).unwrap(), "{not-json");
}
```

- [ ] **Step 2: Run the store tests and verify RED**

Run: `cargo test -p feroha workflow_runtime`

Expected: compilation fails because `WorkflowRuntimeStore`, `WorkflowRuntimeBundle`, dispatch types, and runtime-state errors do not exist.

- [ ] **Step 3: Implement runtime types and store**

Create the following public contract in `workflow_runtime.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowDispatchStatus {
    Dispatched,
    Queued,
    Running,
    Reported,
    Failed,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkflowDispatchRecord {
    pub step_id: String,
    pub attempt: usize,
    pub task_id: Option<String>,
    pub status: WorkflowDispatchStatus,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkflowRuntimeBundle {
    pub goal: GoalContract,
    pub workflow: WorkflowIr,
    pub run: WorkflowRunState,
    pub registry: AgentRegistry,
    #[serde(default)]
    pub dispatches: Vec<WorkflowDispatchRecord>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkflowTaskContext {
    pub workflow_id: String,
    pub run_id: String,
    pub step_id: String,
    pub attempt: usize,
    pub acceptance_criteria: Vec<String>,
}
```

Implement `WorkflowRuntimeStore::{new,runtime_path,write,read,list_run_ids}`. Derive the run directory from `WorkflowRuntimeEventStore::event_log_path(...).parent()` so event and state path validation cannot drift. Serialize with `serde_json::to_vec_pretty`, write through `tempfile::NamedTempFile::new_in`, flush and `sync_all`, then `persist` to `runtime.json`.

In `workflow.rs`, add:

```rust
#[error("workflow runtime state io error: {0}")]
RuntimeStateIo(String),
#[error("workflow runtime state parse error: {0}")]
RuntimeStateParse(String),
#[error("workflow runtime state not found for run {0}")]
RuntimeStateMissing(String),
```

Expose `safe_runtime_component` as `pub(crate)` and export the new module from `harness/mod.rs`.

- [ ] **Step 4: Run the store tests and verify GREEN**

Run: `cargo test -p feroha workflow_runtime`

Expected: all new runtime store tests pass.

- [ ] **Step 5: Commit the store unit**

```bash
git add src-tauri/src/harness/workflow_runtime.rs src-tauri/src/harness/workflow.rs src-tauri/src/harness/mod.rs
git commit -m "feat: persist workflow runtime bundles"
```

### Task 2: Research-Only Workflow Task Adapter

**Files:**
- Create: `src-tauri/src/ai/workflow_task_adapter.rs`
- Modify: `src-tauri/src/ai/mod.rs`

- [ ] **Step 1: Write failing adapter tests**

```rust
#[test]
fn research_dispatch_becomes_path_safe_deep_research_task() {
    let dispatch = research_dispatch("run_demo", "S001");
    let adapted = WorkflowTaskAdapter::adapt(&dispatch, 100).unwrap();
    let AdaptedWorkflowTask::Task(task) = adapted else { panic!("expected task") };

    assert_eq!(task.id, "workflow__run_demo__S001__attempt_1");
    assert!(matches!(task.command, CliCommand::DeepResearch { .. }));
    assert_eq!(task.sandbox_policy, Some(dispatch.sandbox_policy.clone()));
    assert_eq!(workflow_task_context(&task).unwrap().run_id, "run_demo");
}

#[test]
fn implement_dispatch_is_explicitly_unsupported() {
    let dispatch = dispatch_with_kind(WorkflowStepKind::Implement);

    assert!(matches!(
        WorkflowTaskAdapter::adapt(&dispatch, 100).unwrap(),
        AdaptedWorkflowTask::Unsupported { capability: WorkflowStepKind::Implement, .. }
    ));
}
```

- [ ] **Step 2: Run adapter tests and verify RED**

Run: `cargo test -p feroha workflow_task_adapter`

Expected: compilation fails because the adapter API does not exist.

- [ ] **Step 3: Implement deterministic identity, task conversion, and context**

Implement:

```rust
pub enum AdaptedWorkflowTask {
    Task(AgentTask),
    Unsupported {
        capability: WorkflowStepKind,
        reason_code: String,
        summary: String,
    },
}

pub struct WorkflowTaskAdapter;

impl WorkflowTaskAdapter {
    pub fn task_id(dispatch: &StepDispatch) -> String {
        format!(
            "workflow__{}__{}__attempt_{}",
            dispatch.run_id, dispatch.step_id, dispatch.attempt
        )
    }

    pub fn adapt(
        dispatch: &StepDispatch,
        created_at: u64,
    ) -> Result<AdaptedWorkflowTask, WorkflowError> {
        if dispatch.capability != WorkflowStepKind::Research {
            return Ok(AdaptedWorkflowTask::Unsupported {
                capability: dispatch.capability.clone(),
                reason_code: "unsupported_workflow_capability".to_string(),
                summary: format!("No narrow-loop executor for {:?}", dispatch.capability),
            });
        }
        Ok(AdaptedWorkflowTask::Task(research_task(dispatch, created_at)?))
    }
}
```

`research_task` must construct one complete `AgentTask` with `TaskType::DeepDive`, `TaskIntentType::Research`, low priority, the unchanged dispatch sandbox, and `CliCommand::DeepResearch`. Store a serialized `WorkflowTaskContext` in a `ContextFragment` with key `workflow.dispatch`, source `Pipeline`, layer `Project`, and `ContextFragment::compute_hash`. Implement `workflow_task_context(&AgentTask)` by finding and deserializing that fragment.

- [ ] **Step 4: Run adapter tests and verify GREEN**

Run: `cargo test -p feroha workflow_task_adapter`

Expected: research conversion and unsupported-capability tests pass.

- [ ] **Step 5: Commit the adapter unit**

```bash
git add src-tauri/src/ai/workflow_task_adapter.rs src-tauri/src/ai/mod.rs
git commit -m "feat: adapt workflow research steps to agent tasks"
```

### Task 3: Start, Resume, And Idempotent Reconciliation Service

**Files:**
- Create: `src-tauri/src/ai/workflow_runtime_service.rs`
- Modify: `src-tauri/src/ai/mod.rs`
- Modify: `src-tauri/src/ai/agent_scheduler.rs`
- Modify: `src-tauri/src/harness/workflow.rs`

- [ ] **Step 1: Write failing service tests**

Cover start, duplicate resume, missing-task recovery, and unsupported dispatch:

```rust
#[test]
fn start_persists_run_and_queues_research_once() {
    let root = tempfile::tempdir().unwrap();
    let service = WorkflowRuntimeService::new(root.path());
    let mut scheduler = AgentScheduler::new(2);

    let bundle = service
        .start(goal(), workflow_with_ready_research(), registry(), "run_demo", &mut scheduler, 100)
        .unwrap();

    assert_eq!(bundle.dispatches[0].status, WorkflowDispatchStatus::Queued);
    let task_id = bundle.dispatches[0].task_id.as_deref().unwrap();
    assert!(scheduler.get_task(task_id).is_some());
    assert_eq!(service.get("run_demo").unwrap(), bundle);
}

#[test]
fn resume_is_idempotent_when_scheduler_already_has_task() {
    let root = tempfile::tempdir().unwrap();
    let service = WorkflowRuntimeService::new(root.path());
    let mut scheduler = AgentScheduler::new(2);
    service.start(goal(), workflow_with_ready_research(), registry(), "run_demo", &mut scheduler, 100).unwrap();

    service.resume("run_demo", &mut scheduler, 101).unwrap();

    assert_eq!(scheduler.list_tasks(None).iter().filter(|task| task.id == "workflow__run_demo__S001__attempt_1").count(), 1);
}

#[test]
fn unsupported_step_is_durable_and_never_submitted() {
    let root = tempfile::tempdir().unwrap();
    let service = WorkflowRuntimeService::new(root.path());
    let mut scheduler = AgentScheduler::new(2);

    let bundle = service.start(goal(), workflow_with_ready_implement(), registry(), "run_demo", &mut scheduler, 100).unwrap();

    assert_eq!(bundle.dispatches[0].status, WorkflowDispatchStatus::Unsupported);
    assert!(bundle.dispatches[0].task_id.is_none());
    assert!(scheduler.list_tasks(None).is_empty());
}
```

- [ ] **Step 2: Run service tests and verify RED**

Run: `cargo test -p feroha workflow_runtime_service`

Expected: compilation fails because the service and lifecycle event constructors do not exist.

- [ ] **Step 3: Implement service start/get/resume/reconcile**

Create a stateless service:

```rust
pub struct WorkflowRuntimeService {
    store: WorkflowRuntimeStore,
}

impl WorkflowRuntimeService {
    pub fn new(root: impl AsRef<Path>) -> Self;
    pub fn get(&self, run_id: &str) -> Result<WorkflowRuntimeBundle, WorkflowError>;
    pub fn start(
        &self,
        goal: GoalContract,
        workflow: WorkflowIr,
        registry: AgentRegistry,
        run_id: &str,
        scheduler: &mut AgentScheduler,
        now: u64,
    ) -> Result<WorkflowRuntimeBundle, WorkflowError>;
    pub fn resume(
        &self,
        run_id: &str,
        scheduler: &mut AgentScheduler,
        now: u64,
    ) -> Result<WorkflowRuntimeBundle, WorkflowError>;
    pub fn resume_all(
        &self,
        scheduler: &mut AgentScheduler,
        now: u64,
    ) -> Result<Vec<WorkflowRuntimeBundle>, WorkflowError>;
}
```

`start` validates `OrchestratorOutput::WorkflowCreate`, rejects an existing run ID, persists the initial bundle, emits `workflow.run.created`, then calls one private `reconcile`. `reconcile` calls `ready_dispatches`, skips existing terminal records, adapts each dispatch, submits and approves research tasks as `orchestrator`, adds the step to `active_step_ids`, persists after transitions, and records queued/unsupported events.

Make `AgentScheduler::record_workflow_event_chain` `pub(crate)` so the service updates in-memory status and the existing JSONL ledger through one path. Add focused `WorkflowRuntimeEventChain` constructors for run-created, run-resumed, queued, and unsupported events; every step event must carry the contract attributes from the design.

- [ ] **Step 4: Run service and scheduler workflow tests**

Run: `cargo test -p feroha workflow_runtime_service`

Run: `cargo test -p feroha scheduler_prepares_controlled_subagent_jobs_and_records_dispatch_events`

Expected: both commands pass; existing dispatch preparation remains compatible.

- [ ] **Step 5: Commit the service unit**

```bash
git add src-tauri/src/ai/workflow_runtime_service.rs src-tauri/src/ai/mod.rs src-tauri/src/ai/agent_scheduler.rs src-tauri/src/harness/workflow.rs
git commit -m "feat: start and resume workflow runtime"
```

### Task 4: Worker Running, Completion, And Failure Transitions

**Files:**
- Modify: `src-tauri/src/ai/workflow_runtime_service.rs`
- Modify: `src-tauri/src/ai/commands.rs`
- Modify: `src-tauri/src/harness/workflow.rs`

- [ ] **Step 1: Write failing lifecycle transition tests**

```rust
#[test]
fn task_completion_reports_step_and_clears_active_dispatch() {
    let fixture = started_research_fixture();
    let task = fixture.scheduler.get_task(&fixture.task_id).unwrap().clone();

    let bundle = fixture.service
        .record_task_completion(&task, "research result", 200, &mut fixture.scheduler)
        .unwrap()
        .unwrap();

    assert_eq!(bundle.dispatches[0].status, WorkflowDispatchStatus::Reported);
    assert!(bundle.run.active_step_ids.is_empty());
    assert_eq!(bundle.workflow.steps[0].status, WorkflowStepStatus::Reported);
}

#[test]
fn task_failure_is_durable_and_does_not_verify_step() {
    let fixture = started_research_fixture();
    let task = fixture.scheduler.get_task(&fixture.task_id).unwrap().clone();

    let bundle = fixture.service
        .record_task_failure(&task, "network timeout", 200, &mut fixture.scheduler)
        .unwrap()
        .unwrap();

    assert_eq!(bundle.dispatches[0].status, WorkflowDispatchStatus::Failed);
    assert_eq!(bundle.workflow.steps[0].status, WorkflowStepStatus::Failed);
    assert_ne!(bundle.workflow.steps[0].status, WorkflowStepStatus::Verified);
}
```

- [ ] **Step 2: Run lifecycle tests and verify RED**

Run: `cargo test -p feroha workflow_runtime_service::tests::task_`

Expected: compilation fails because completion/failure methods are absent.

- [ ] **Step 3: Implement transition methods and worker hooks**

Add service methods returning `Ok(None)` for ordinary non-workflow tasks:

```rust
pub fn record_task_running(
    &self,
    task: &AgentTask,
    now: u64,
    scheduler: &mut AgentScheduler,
) -> Result<Option<WorkflowRuntimeBundle>, WorkflowError>;

pub fn record_task_completion(
    &self,
    task: &AgentTask,
    output: &str,
    now: u64,
    scheduler: &mut AgentScheduler,
) -> Result<Option<WorkflowRuntimeBundle>, WorkflowError>;

pub fn record_task_failure(
    &self,
    task: &AgentTask,
    error: &str,
    now: u64,
    scheduler: &mut AgentScheduler,
) -> Result<Option<WorkflowRuntimeBundle>, WorkflowError>;
```

Each method obtains `WorkflowTaskContext` from the task fragment, loads the run, updates only the matching dispatch and step, persists, and records `workflow.step.running`, `workflow.step.reported`, or `workflow.step.failed`. Completion sets `Reported`, never `Verified`, then runs reconciliation; dependencies remain blocked until a later verifier marks them verified.

In `start_task_worker`, call `record_task_running` immediately after dequeue and before `execute_agent_task_async`. On success call `record_task_completion` after scheduler completion; on execution error call `record_task_failure` after scheduler failure. Construct the service from the current `AppState.vault_path`; skip the hook when no vault is open or the task has no workflow context.

- [ ] **Step 4: Run lifecycle and worker regression tests**

Run: `cargo test -p feroha workflow_runtime_service`

Run: `cargo test -p feroha test_approve_and_dequeue`

Expected: workflow transitions pass and ordinary task dequeue behavior remains green.

- [ ] **Step 5: Commit worker lifecycle integration**

```bash
git add src-tauri/src/ai/workflow_runtime_service.rs src-tauri/src/ai/commands.rs src-tauri/src/harness/workflow.rs
git commit -m "feat: persist workflow task lifecycle"
```

### Task 5: Tauri Commands And Vault Recovery

**Files:**
- Modify: `src-tauri/src/ai/commands.rs`
- Modify: `src-tauri/src/fs/commands.rs`
- Modify: `src-tauri/src/main.rs`

- [ ] **Step 1: Write failing command and recovery tests**

Add pure helper tests in `ai/commands.rs` for start/get/resume service routing and an initialization test in `fs/commands.rs` that persists a running bundle, recreates scheduler state, initializes the vault, and asserts one deterministic task exists.

```rust
#[test]
fn workflow_command_start_routes_to_runtime_service() {
    let root = tempfile::tempdir().unwrap();
    let mut scheduler = AgentScheduler::new(2);
    let bundle = start_workflow_run_for_root(
        root.path(), goal(), workflow(), registry(), "run_demo", &mut scheduler, 100
    ).unwrap();
    assert_eq!(bundle.run.run_id, "run_demo");
}
```

- [ ] **Step 2: Run command tests and verify RED**

Run: `cargo test -p feroha workflow_command_start_routes_to_runtime_service`

Expected: compilation fails because the helper and commands do not exist.

- [ ] **Step 3: Add thin commands and recovery wiring**

Add Tauri commands with camel-case IPC compatibility:

```rust
#[tauri::command]
pub(crate) fn start_workflow_run(
    goal: GoalContract,
    workflow: WorkflowIr,
    registry: AgentRegistry,
    run_id: String,
    state: State<'_, Mutex<AppState>>,
    ai_state: State<'_, Mutex<AiState>>,
) -> Result<WorkflowRuntimeBundle, String>;

#[tauri::command]
pub(crate) fn get_workflow_run(
    run_id: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<WorkflowRuntimeBundle, String>;

#[tauri::command]
pub(crate) fn resume_workflow_run(
    run_id: String,
    state: State<'_, Mutex<AppState>>,
    ai_state: State<'_, Mutex<AiState>>,
) -> Result<WorkflowRuntimeBundle, String>;
```

All commands reject an empty vault path. Register them in both `main.rs` and `commands::register_commands`. In `initialize_vault_services`, after `set_workflow_event_root`, construct the service and call `resume_all`; notify `task_notifier` once when recovery queued at least one task. Corrupt runs are logged and left untouched rather than blocking the vault from opening.

- [ ] **Step 4: Run command and vault initialization tests**

Run: `cargo test -p feroha workflow_command`

Run: `cargo test -p feroha initialize_vault_services`

Expected: commands and idempotent recovery pass.

- [ ] **Step 5: Commit command wiring**

```bash
git add src-tauri/src/ai/commands.rs src-tauri/src/fs/commands.rs src-tauri/src/main.rs
git commit -m "feat: expose workflow runtime commands"
```

### Task 6: Frontend Runtime Contract And Event Presentation

**Files:**
- Modify: `src/types/orchestrator.ts`
- Modify: `src/hooks/useAppStore.ts`
- Modify: `src/hooks/__tests__/useAppStore.workflow.test.ts`
- Modify: `src/lib/orchestratorEventPresentation.ts`
- Modify: `src/lib/__tests__/orchestratorEventPresentation.test.ts`

- [ ] **Step 1: Write failing IPC and event-label tests**

```typescript
it("starts a workflow run through the runtime command", async () => {
  invokeMock.mockResolvedValueOnce(runtimeBundle);
  const result = await useAppStore.getState().startWorkflowRun(
    goal, workflow, registry, "run_demo",
  );
  expect(invokeMock).toHaveBeenCalledWith("start_workflow_run", {
    goal,
    workflow,
    registry,
    runId: "run_demo",
  });
  expect(result).toEqual(runtimeBundle);
});

it.each([
  ["workflow.run.created", "运行已创建"],
  ["workflow.run.resumed", "运行已恢复"],
  ["workflow.step.queued", "步骤已入队"],
  ["workflow.step.running", "步骤执行中"],
  ["workflow.step.reported", "步骤已报告"],
  ["workflow.step.failed", "步骤失败"],
  ["workflow.step.unsupported", "能力暂不支持"],
])("maps %s", (name, label) => {
  expect(workflowEventLabel(name)).toBe(label);
});
```

- [ ] **Step 2: Run frontend tests and verify RED**

Run: `npm.cmd test -- --run src/hooks/__tests__/useAppStore.workflow.test.ts src/lib/__tests__/orchestratorEventPresentation.test.ts`

Expected: TypeScript compilation or assertions fail because runtime types/actions/labels are absent.

- [ ] **Step 3: Add types, actions, and labels**

Mirror Goal, Workflow, Registry, Run, DispatchRecord, and RuntimeBundle fields in `src/types/orchestrator.ts`. Add nullable actions to the Zustand interface and implementation:

```typescript
startWorkflowRun: async (goal, workflow, registry, runId) => invoke(
  "start_workflow_run",
  { goal, workflow, registry, runId },
),
getWorkflowRun: async (runId) => invoke("get_workflow_run", { runId }),
resumeWorkflowRun: async (runId) => invoke("resume_workflow_run", { runId }),
```

Catch errors consistently with existing workflow actions and return `null`. Add the seven approved Chinese lifecycle labels. Extend event detail to display `step_id`, `agent_type`, `capability`, and `task_id` when present without throwing on malformed attributes.

- [ ] **Step 4: Run frontend tests and build**

Run: `npm.cmd test -- --run src/hooks/__tests__/useAppStore.workflow.test.ts src/lib/__tests__/orchestratorEventPresentation.test.ts src/components/__tests__/OrchestratorPanel.test.tsx src/components/__tests__/OrchestratorWorkflowView.test.ts`

Run: `npm.cmd run build`

Expected: tests and TypeScript/Vite production build pass.

- [ ] **Step 5: Commit frontend contract**

```bash
git add src/types/orchestrator.ts src/hooks/useAppStore.ts src/hooks/__tests__/useAppStore.workflow.test.ts src/lib/orchestratorEventPresentation.ts src/lib/__tests__/orchestratorEventPresentation.test.ts
git commit -m "feat: expose workflow runtime lifecycle"
```

### Task 7: Full Regression And Real Runtime Verification

**Files:**
- Verify: `src-tauri/src/ai/workflow_runtime_service.rs`
- Verify: `src-tauri/src/harness/workflow_runtime.rs`
- Verify: `src/hooks/useAppStore.ts`
- Verify: `src/components/OrchestratorWorkflowView.tsx`

- [ ] **Step 1: Run the complete Rust suite**

Run: `cargo test -p feroha`

Expected: all library, binary, and doc tests pass with zero failures.

- [ ] **Step 2: Run the complete frontend suite**

Run: `npm.cmd test -- --run`

Expected: all Vitest files pass with zero failures.

- [ ] **Step 3: Build web and Tauri release targets**

Run: `npm.cmd run build`

Run: `npm.cmd run tauri -- build --no-bundle`

Expected: both exit successfully; the existing large-chunk and dead-code warnings may remain but no new compile errors are accepted.

- [ ] **Step 4: Exercise the real narrow loop**

Open a temporary vault, invoke `start_workflow_run` with one ready Research step and one ready Implement step, then verify:

```text
.harness/runs/<run_id>/runtime.json exists
Research dispatch status is queued/running/reported
Implement dispatch status is unsupported
exactly one workflow__<run>__<step>__attempt_1 task exists
events.jsonl contains created, dispatched, queued, and unsupported events
resume_workflow_run does not create a duplicate task
orchestrator UI shows readable event labels
```

Use Computer Use for the desktop path when the plugin is operational; otherwise use the in-app browser for UI presentation and report that desktop/API execution could not be independently automated. Never expose API keys in logs, screenshots, runtime JSON, or the final report.

- [ ] **Step 5: Review the final diff for scope and whitespace**

Run: `git diff --check`

Run: `git status --short`

Expected: no whitespace errors; unrelated existing dirty files remain untouched.

Do not create an empty commit when verification required no code changes. If verification exposes a defect, return to the task that owns that behavior, add a failing regression test there, and commit only that task's explicit files.
