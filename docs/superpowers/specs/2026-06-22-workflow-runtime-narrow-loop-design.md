# Workflow Runtime Narrow Loop Design

**Date:** 2026-06-22

**Status:** Approved for implementation planning

## Goal

Turn `OrchestratorOutput::WorkflowCreate` from a schema-only value into a recoverable production runtime. The first executable slice must persist workflow state, dispatch ready research steps through the existing real Agent task worker, record an auditable lifecycle, and refuse to pretend that unsupported capabilities have executed.

## Scope

This iteration includes:

- validating and starting a workflow run from a Goal, Workflow IR, Agent Registry, and run identifier;
- persisting the runtime bundle under the vault-level `.harness/runs` operational directory;
- converting ready `research` steps into approved `AgentTask` work that uses the existing task worker;
- recording queued, reported, failed, and unsupported lifecycle events;
- restoring an interrupted running workflow without duplicating tasks;
- exposing the persisted run state through thin Tauri commands and existing orchestrator status/event views.

This iteration does not include:

- a general LLM tool runner for implement, test, review, verify, or merge steps;
- direct writes from the Orchestrator or a Subagent into human notes;
- storing workflow runtime records in the Dream working, semantic, or long-term memory zones;
- automatic merge or approval of Bridge proposals;
- a visual workflow editor redesign.

## Architectural Choice

The runtime reuses the existing `AgentTask` worker rather than adding a second executor. That worker already owns real vector retrieval, Subagent execution, LLM routing, cancellation, task status, Scientist refinement, trace persistence, and Bridge proposal generation. A focused adapter maps supported workflow dispatches into this established execution contract.

The runtime logic is split into focused units:

1. `WorkflowRuntimeStore` owns path-safe, atomic persistence and recovery.
2. `WorkflowTaskAdapter` converts a supported `StepDispatch` into an `AgentTask` and rejects unsupported capabilities explicitly.
3. `WorkflowRuntimeService` validates inputs, creates or resumes runs, prepares dispatches, submits idempotent tasks, and records lifecycle transitions.
4. Tauri commands remain thin wrappers around the service.

`AgentScheduler` remains the task queue and execution status owner. It must not become the persistence or workflow-recovery service.

## Runtime Data Model

Each run is stored at:

```text
<vault>/.harness/runs/<run_id>/runtime.json
<vault>/.harness/runs/<run_id>/events.jsonl
```

`runtime.json` contains one `WorkflowRuntimeBundle`:

```rust
pub struct WorkflowRuntimeBundle {
    pub goal: GoalContract,
    pub workflow: WorkflowIr,
    pub run: WorkflowRunState,
    pub registry: AgentRegistry,
    pub dispatches: Vec<WorkflowDispatchRecord>,
    pub updated_at: String,
}
```

Each dispatch record contains:

```rust
pub struct WorkflowDispatchRecord {
    pub step_id: String,
    pub attempt: usize,
    pub task_id: Option<String>,
    pub status: WorkflowDispatchStatus,
    pub detail: Option<String>,
}
```

The status enum is serialized as snake case and has these values:

- `dispatched`: the ready step was materialized as a `StepDispatch`;
- `queued`: the corresponding `AgentTask` is present in the scheduler;
- `running`: the existing worker has started the task;
- `reported`: execution completed and a StepReport-shaped result was recorded;
- `failed`: execution failed with a durable error detail;
- `unsupported`: this runtime intentionally has no executor for the capability.

Runtime persistence is operational state, not AI memory. It stays under `.harness` and must never appear as a Dream memory zone in the AI file browser.

## Deterministic Identity And Idempotency

Research task IDs use this deterministic format:

```text
workflow__<run_id>__<step_id>__attempt_<attempt>
```

Starting or resuming a run checks both the persisted dispatch record and the scheduler task map. A task is not submitted again when the deterministic task ID already exists in a non-terminal scheduler state. A terminal `reported` dispatch is never requeued. A failed dispatch is retried only when its workflow step retry policy permits another attempt; automatic retry policy expansion is outside this iteration.

Run IDs, workflow IDs, and step IDs must pass the existing safe-component rules before they become filesystem paths. Path traversal, separators, empty identifiers, and reserved parent components are rejected before any write.

## Start And Recovery Flow

The production start command accepts `goal`, `workflow`, `registry`, and `run_id`.

1. Validate `OrchestratorOutput::WorkflowCreate` against the Goal and Registry.
2. Create `WorkflowRunState::for_workflow` with status `running`.
3. Persist the initial runtime bundle atomically.
4. Ask the existing dispatch planner for ready dispatches.
5. Persist one dispatch record per ready step and emit `workflow.step.dispatched`.
6. Adapt supported research steps into `AgentTask` values, submit them, and approve them as `checked_by = "orchestrator"`.
7. Mark those dispatch records `queued`, emit `workflow.step.queued`, and notify the existing worker.
8. Mark other capabilities `unsupported` and emit `workflow.step.unsupported`; do not enqueue substitute work.

Recovery loads the persisted bundle and repeats the idempotent dispatch reconciliation. It recreates missing queued research tasks only when the dispatch is non-terminal and the deterministic task ID is absent. Corrupt runtime JSON produces a typed error and does not overwrite the file.

## Research Task Adaptation

Only `WorkflowStepKind::Research` is executable in this slice.

The adapter creates a deep-research `AgentTask` using:

- the deterministic workflow task ID;
- `CliCommand::DeepResearch`;
- the dispatch artifact contract as the research question, augmented by string keywords from `inputs.keywords` when present;
- the dispatch sandbox policy without widening tools, read roots, write roots, network policy, or runtime limit;
- low priority and an orchestrator approval identity;
- a `WorkflowTaskContext` linking workflow ID, run ID, step ID, attempt, and acceptance criteria.

The existing worker opens the real vector store, runs the configured Subagent and LLM path, persists research traces, and invokes Scientist/Bridge logic. The adapter does not call network APIs directly and does not duplicate worker logic.

## Completion And Failure Flow

When the worker finishes a task carrying `WorkflowTaskContext`:

1. Build a StepReport-shaped record from the output, task trace, retrieval evidence, and task status.
2. Update the dispatch record to `reported` or `failed`.
3. Update the matching workflow step to `reported` or `failed`.
4. Remove the step from `active_step_ids` and persist the bundle atomically.
5. Emit `workflow.step.reported` or `workflow.step.failed` into the existing JSONL event ledger.
6. Re-run ready-step reconciliation. A dependent step remains blocked until a separate verification transition marks its dependency `verified`.

Completion does not automatically mark a step `verified`. Verification remains a separate Orchestrator/Scientist decision. The run becomes succeeded only when every workflow step is verified or skipped; that terminal transition is not synthesized from a research task merely completing.

## Event Contract

The lifecycle uses these event names:

- `workflow.run.created`
- `workflow.run.resumed`
- `workflow.step.dispatched`
- `workflow.step.queued`
- `workflow.step.running`
- `workflow.step.reported`
- `workflow.step.failed`
- `workflow.step.unsupported`

Every step event includes `workflow_id`, `run_id`, `step_id`, `agent_type`, `capability`, `attempt`, and `task_id` when a task exists. Error events include a stable reason code and a human-readable summary. The frontend presentation helper maps these names to concise Chinese labels while unknown events continue to fall back to the raw name.

## Commands And Status

Add thin commands:

```text
start_workflow_run(goal, workflow, registry, run_id)
get_workflow_run(run_id)
resume_workflow_run(run_id)
```

`start_workflow_run` returns the persisted runtime bundle after dispatch reconciliation. `get_workflow_run` is read-only. `resume_workflow_run` performs the same idempotent reconciliation used during vault startup.

The existing `orchestrator_status` remains the primary visual status surface. Runtime lifecycle events continue to flow through `recent_workflow_events`, and the workflow view derives counts from those events. No second competing frontend store is introduced.

## Error Handling

- Invalid workflow, run mismatch, unknown agent type, or unsafe path component returns a typed domain error.
- Unsupported capability is a durable dispatch status, not a command failure and not a fake success.
- Persistence uses write-to-temporary-file followed by atomic rename in the same run directory.
- A failed atomic replacement leaves the last valid runtime file intact.
- Corrupt runtime data is reported without deletion or silent reset.
- Scheduler submission is reconciled with persisted state to avoid duplicate work after partial failure.
- Worker errors are written both to the task status and workflow event ledger.

## Testing Strategy

Rust unit and integration tests cover:

- safe run-path construction and traversal rejection;
- runtime bundle round-trip persistence;
- corrupt JSON recovery errors;
- atomic replacement preserving the previous valid bundle on failure;
- deterministic task IDs;
- Research dispatch conversion preserving the sandbox and workflow context;
- non-Research dispatch becoming `unsupported` without scheduler submission;
- repeated start and resume producing one scheduler task;
- missing queued task recovery;
- successful worker completion updating dispatch/workflow state and events;
- worker failure updating durable state and events;
- existing snapshot, Bridge, Dream, scheduler, and workflow tests remaining green.

Frontend tests cover readable labels and counts for queued, reported, failed, and unsupported runtime events. A final application-level test starts a local run, reloads status, and verifies that the runtime event chain is visible without exposing raw internal payloads as primary UI text.

## Acceptance Criteria

The iteration is complete only when:

1. A valid WorkflowCreate can start and persist a run in a real vault.
2. A ready Research step enters the existing AgentTask worker exactly once.
3. The worker uses the configured vector, Subagent, LLM, Scientist, trace, and Bridge paths.
4. Restart/resume does not duplicate an existing task.
5. Non-Research steps are visibly unsupported and never reported as successful.
6. Task completion or failure is durable and visible in the orchestrator event UI.
7. Existing human/AI file separation, Dream memory zones, note editing, snapshots, command cards, and Bridge review behavior do not regress.
