# Orchestrator Real Data Flow Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let a user start a deterministic research workflow from the existing orchestration view, execute it through the existing AI Manager worker, persist raw output only in Dream Working Memory, promote verified knowledge to Semantic Memory, and keep human notes free of automatic AI writes.

**Architecture:** Extend the existing `WorkflowRuntimeService`, `AgentScheduler`, research trace, Scientist, Dream memory, Zustand store, and orchestration view. `.harness` remains the runtime/event control plane and stores references only; `.dualtrack` remains the AI content plane. No second scheduler, artifact store, workflow database, frontend store, or orchestration page is introduced.

**Tech Stack:** Rust 2021, Tauri 2, Serde, SHA-256, React 18, TypeScript, Zustand, Vitest, Testing Library.

---

## File Structure

- Modify `src-tauri/src/fs/commands.rs`: enforce human-surface path guards for all human note mutations.
- Modify `src-tauri/src/diff/commands.rs`: convert Ghost acceptance into feedback-only state changes.
- Modify `src-tauri/src/harness/workflow_runtime.rs`: add runtime artifact/report/finding references without storing generated body text.
- Modify `src-tauri/src/harness/workflow.rs`: correct runtime write roots and add lifecycle event constructors.
- Create `src-tauri/src/ai/workflow_template.rs`: deterministic Goal, Workflow, Registry, workflow ID, and run ID construction.
- Modify `src-tauri/src/ai/workflow_task_adapter.rs`: include a machine-checkable acceptance section in the existing Research prompt.
- Modify `src-tauri/src/ai/dream_memory.rs`: canonical Working/Semantic paths, safe semantic promotion, and SHA-256 artifact refs.
- Modify `src-tauri/src/ai/workflow_runtime_service.rs`: immediate lifecycle callbacks, contract evaluation, semantic promotion, terminal state, and run listing.
- Modify `src-tauri/src/harness/scientist.rs`: extract claim material from the completed research report before falling back to task decomposition labels.
- Create `src-tauri/src/ai/workflow_commands.rs`: thin Tauri commands for create/start, get, and list.
- Modify `src-tauri/src/ai/commands.rs`: invoke runtime callbacks from the existing worker after trace writes and on every failure path.
- Modify `src-tauri/src/ai/mod.rs`: export the focused workflow modules.
- Modify `src-tauri/src/main.rs`: register the workflow commands.
- Modify `src/types/orchestrator.ts`: mirror the concrete Rust runtime contract.
- Modify `src/hooks/useAppStore.ts`: add workflow run state and IPC actions to the existing store.
- Modify `src/hooks/__tests__/useAppStore.workflow.test.ts`: cover create/get/list IPC and state convergence.
- Modify `src/components/OrchestratorWorkflowView.tsx`: add the goal form, active run, steps, artifacts, verification, and event timeline.
- Modify `src/components/OrchestratorPanel.tsx`: keep only macro workflow health and remove duplicate full details.
- Modify `src/components/DiffView.tsx`: present Ghost acceptance as feedback, not note application.
- Modify focused component and presentation tests under `src/components/__tests__` and `src/lib/__tests__`.

### Task 1: Lock Human Note Writes To The Human Namespace

**Files:**
- Modify: `src-tauri/src/fs/commands.rs:525-615`
- Modify: `src-tauri/src/fs/commands.rs:709-735`
- Test: `src-tauri/src/fs/commands.rs:1482-1497`

- [ ] **Step 1: Write failing path-boundary tests**

Add these tests beside `save_note_for_human_surface_writes_and_reads_back_content`:

```rust
#[test]
fn human_note_mutations_reject_ai_and_runtime_namespaces() {
    for path in [
        ".dualtrack/memory/working/ai.md",
        ".harness/runs/run-a/runtime.md",
        ".hidden/note.md",
        "_private/note.md",
    ] {
        assert_eq!(
            validate_human_surface_path(path).unwrap_err(),
            format!("Human note operations cannot access private path: {path}")
        );
    }
}

#[test]
fn save_note_rejects_private_path_before_touching_disk() {
    let dir = TempDir::new().unwrap();
    let mut app = test_app_state(dir.path());

    let error = save_note_for_app(
        &mut app,
        ".dualtrack/memory/working/ai.md",
        "machine output",
    )
    .unwrap_err();

    assert!(error.contains("cannot access private path"));
    assert!(!dir
        .path()
        .join(".dualtrack/memory/working/ai.md")
        .exists());
}
```

- [ ] **Step 2: Run the path tests and verify RED**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml human_note_mutations_reject_ai_and_runtime_namespaces -- --nocapture
```

Expected: compilation fails because `validate_human_surface_path` does not exist.

- [ ] **Step 3: Implement one shared human path guard**

Add the helper near the note commands:

```rust
fn validate_human_surface_path(path: &str) -> Result<(), String> {
    let normalized = path.replace('\\', "/");
    let components = normalized
        .split('/')
        .filter(|component| !component.is_empty())
        .collect::<Vec<_>>();
    let private = normalized.starts_with('/')
        || components.is_empty()
        || components.iter().any(|component| {
            *component == "."
                || *component == ".."
                || component.starts_with('.')
                || component.starts_with('_')
        });
    if private {
        return Err(format!(
            "Human note operations cannot access private path: {path}"
        ));
    }
    Ok(())
}
```

Call it before acquiring a vault write in:

```rust
save_note_for_app(app, path, content)
create_note(path, state)
delete_note(path, state)
create_folder(path, state)
rename_note(old_path, new_path, state)
```

For rename, validate both paths before mutating:

```rust
validate_human_surface_path(&old_path)?;
validate_human_surface_path(&new_path)?;
```

- [ ] **Step 4: Run focused filesystem tests**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml human_surface -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml list_ai_workspace_files_exposes_dualtrack_without_polluting_human_notes -- --nocapture
```

Expected: all matching tests pass and the AI workspace listing remains available.

- [ ] **Step 5: Commit the human namespace boundary**

```powershell
git add src-tauri/src/fs/commands.rs
git commit -m "fix: isolate human note write commands"
```

### Task 2: Make Diff Acceptance Feedback-Only

**Files:**
- Modify: `src-tauri/src/diff/commands.rs:137-235`
- Modify: `src/components/DiffView.tsx`
- Test: `src-tauri/src/diff/commands.rs:310-330`
- Test: `src/components/__tests__/DiffView.test.tsx`

- [ ] **Step 1: Write failing backend regression test**

Extract the feedback mutation into a pure helper and first write:

```rust
#[test]
fn accepting_ghost_blocks_updates_feedback_without_merging_note_text() {
    let mut ghost = crate::diff::ghost_store::GhostNote {
        id: "ghost-feedback".to_string(),
        task_id: Some("task-feedback".to_string()),
        source_note: "human.md".to_string(),
        task_description: "Review AI suggestion".to_string(),
        suggested_blocks: vec![crate::diff::ghost_store::GhostBlock {
            block_id: "block-1".to_string(),
            content: "AI suggestion".to_string(),
            operation: GhostOp::Suggestion,
            after_block_id: None,
            heading_context: String::new(),
            context: vec![],
            verified: None,
            verification_result: None,
        }],
        created_at: 1,
        status: GhostStatus::Pending,
        priority: 50,
        expires_at: None,
        related_ghosts: vec![],
        confidence: 0.7,
        feedback_history: vec![],
        accepted_blocks: vec![],
        rejected_blocks: vec![],
    };

    let accepted = accept_ghost_feedback(&mut ghost, &["block-1".to_string()]);

    assert_eq!(accepted, 1);
    assert_eq!(ghost.accepted_blocks, vec!["block-1"]);
    assert!(matches!(ghost.status, GhostStatus::Accepted));
}
```

- [ ] **Step 2: Run the test and verify RED**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml accepting_ghost_blocks_updates_feedback_without_merging_note_text -- --nocapture
```

Expected: compilation fails because `accept_ghost_feedback` does not exist.

- [ ] **Step 3: Remove note merging from `accept_diff`**

Implement:

```rust
fn accept_ghost_feedback(
    ghost: &mut crate::diff::ghost_store::GhostNote,
    block_ids: &[String],
) -> usize {
    for block_id in block_ids {
        if !ghost.accepted_blocks.contains(block_id) {
            ghost.accepted_blocks.push(block_id.clone());
        }
        ghost.rejected_blocks.retain(|id| id != block_id);
    }
    update_ghost_status(ghost);
    block_ids.len()
}
```

Change `accept_diff` to:

```rust
let (ghost_update, updated_status, accepted_count, source_note) = {
    let ai = ai_state.lock().map_err(|e| e.to_string())?;
    let mut ghost = ai
        .ghost_store
        .get(&ghost_id)
        .ok_or_else(|| format!("Ghost note not found: {ghost_id}"))?;
    let accepted_count = accept_ghost_feedback(&mut ghost, &block_ids);
    ai.ghost_store.save(&ghost)?;
    (
        serde_json::json!({
            "ghost_id": ghost_id,
            "status": ghost.status,
            "accepted_blocks": ghost.accepted_blocks,
            "rejected_blocks": ghost.rejected_blocks,
            "effect": "feedback_only",
        }),
        ghost.status.clone(),
        accepted_count,
        ghost.source_note,
    )
};
```

Delete the `DiffOp`, `apply_merge`, `FileEvent`, `VaultManager::write_note`, `process_file_event`, and `file-changed` path from this command. Return:

```rust
Ok(format!(
    "Recorded feedback for {accepted_count} blocks from {source_note}; human note content was not modified"
))
```

- [ ] **Step 4: Write and run frontend wording test**

Add:

```typescript
it("presents acceptance as feedback that does not modify human notes", async () => {
  useAppStore.setState({
    diffBlocks: [{
      ghostId: "ghost-feedback",
      id: "block-1",
      type: "inserted",
      newText: "AI suggestion",
      accepted: false,
      rejected: false,
    }],
  });

  render(<DiffView isTauri={false} />);

  expect(screen.getByText("采纳反馈")).toBeDefined();
  expect(screen.getByText(/不会修改人类笔记正文/)).toBeDefined();
});
```

Replace action/status copy with `采纳反馈`、`已采纳`、`全部采纳` and add the persistent explanation.

Run:

```powershell
npm.cmd test -- src/components/__tests__/DiffView.test.tsx
```

Expected: all DiffView tests pass.

- [ ] **Step 5: Commit feedback-only Diff behavior**

```powershell
git add src-tauri/src/diff/commands.rs src/components/DiffView.tsx src/components/__tests__/DiffView.test.tsx
git commit -m "fix: keep ai diff acceptance feedback only"
```

### Task 3: Store Workflow References Instead Of Generated Body Text

**Files:**
- Modify: `src-tauri/src/harness/workflow_runtime.rs:13-51`
- Modify: `src-tauri/src/ai/workflow_runtime_service.rs:211-293`
- Modify: `src-tauri/src/harness/workflow.rs:1632-1644`

- [ ] **Step 1: Write failing runtime serialization tests**

Extend `WorkflowRuntimeBundle` fixture tests:

```rust
#[test]
fn runtime_bundle_serializes_artifact_refs_without_generated_body_text() {
    let mut expected = bundle("run-artifacts", "200");
    expected.artifacts = vec![ArtifactRef {
        artifact_id: "working-result".to_string(),
        artifact_type: ArtifactType::Other,
        uri: ".dualtrack/research/results/task/result.md".to_string(),
        hash: "sha256:abc".to_string(),
        mime_type: "text/markdown".to_string(),
        producer_step_id: "S001".to_string(),
        retention_policy: RetentionPolicy::Workflow,
        created_at: "200".to_string(),
    }];
    expected.dispatches[0].detail = Some("research result recorded".to_string());

    let encoded = serde_json::to_string(&expected).unwrap();

    assert!(encoded.contains(".dualtrack/research/results/task/result.md"));
    assert!(!encoded.contains("full model response body"));
}
```

Add a service test:

```rust
#[test]
fn reported_dispatch_keeps_short_detail_instead_of_scheduler_result_body() {
    let root = tempfile::tempdir().unwrap();
    let service = WorkflowRuntimeService::new(root.path());
    let mut scheduler = AgentScheduler::new(1);
    let started = service
        .start(
            goal(),
            workflow_with_ready_research(),
            registry(),
            "run_demo",
            &mut scheduler,
            100,
        )
        .unwrap();
    let task_id = started.dispatches[0].task_id.clone().unwrap();
    scheduler.complete(
        &task_id,
        "full model response body that must stay in Dream working memory".to_string(),
    );

    let bundle = service
        .resume("run_demo", &mut scheduler, 200)
        .unwrap();

    assert_eq!(
        bundle.dispatches[0].detail.as_deref(),
        Some("research result recorded")
    );
    assert!(!serde_json::to_string(&bundle)
        .unwrap()
        .contains("full model response body"));
    let ledger = std::fs::read_to_string(
        root.path().join(".harness/runs/run_demo/events.jsonl"),
    )
    .unwrap();
    assert!(!ledger.contains("full model response body"));
}
```

- [ ] **Step 2: Run tests and verify RED**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml runtime_bundle_serializes_artifact_refs_without_generated_body_text -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml reported_dispatch_keeps_short_detail_instead_of_scheduler_result_body -- --nocapture
```

Expected: `WorkflowRuntimeBundle` has no artifact/report/finding fields and dispatch detail still contains the result body.

- [ ] **Step 3: Extend the existing runtime bundle**

Import existing workflow types and add defaulted fields:

```rust
pub struct WorkflowRuntimeBundle {
    pub goal: GoalContract,
    pub workflow: WorkflowIr,
    pub run: WorkflowRunState,
    pub registry: AgentRegistry,
    #[serde(default)]
    pub dispatches: Vec<WorkflowDispatchRecord>,
    #[serde(default)]
    pub artifacts: Vec<ArtifactRef>,
    #[serde(default)]
    pub step_reports: Vec<StepReport>,
    #[serde(default)]
    pub verification_findings: Vec<VerificationFinding>,
    pub updated_at: String,
}
```

Initialize all three vectors in `WorkflowRuntimeService::start`. In the `TaskStatus::Done` sync branch replace:

```rust
Some(result.clone())
```

with:

```rust
Some("research result recorded".to_string())
```

and emit only a summary:

```rust
WorkflowRuntimeEventChain::from_step_reported(
    &dispatch,
    task_id,
    "Research result persisted in Dream Working Memory".to_string(),
    now.to_string(),
)
```

Change `WorkflowStepMode::TestOnly` write roots from `.harness/runs/.../artifacts` to:

```rust
PathBuf::from(format!(
    ".dualtrack/memory/working/workflows/{}/{}",
    run.run_id, step.step_id
))
```

Update every existing `WorkflowRuntimeBundle` Rust literal in runtime/service/filesystem tests with:

```rust
artifacts: Vec::new(),
step_reports: Vec::new(),
verification_findings: Vec::new(),
```

In `advance_dispatch`, route task submission through the existing Manager facade instead of calling scheduler control methods directly:

```rust
let mut manager = AiManagerService::new(scheduler);
manager.submit(task);
manager
    .approve(&task_id, "orchestrator")
    .map_err(WorkflowError::RuntimeStateIo)?;
```

- [ ] **Step 4: Run runtime compatibility tests**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml workflow_runtime -- --nocapture
```

Expected: existing legacy bundle/default tests and the new reference-only tests pass.

- [ ] **Step 5: Commit the runtime data-plane separation**

```powershell
git add src-tauri/src/harness/workflow_runtime.rs src-tauri/src/harness/workflow.rs src-tauri/src/ai/workflow_runtime_service.rs
git commit -m "fix: keep workflow runtime reference only"
```

### Task 4: Build The Deterministic Workflow And Canonical Dream Artifacts

**Files:**
- Create: `src-tauri/src/ai/workflow_template.rs`
- Modify: `src-tauri/src/ai/workflow_task_adapter.rs`
- Modify: `src-tauri/src/ai/dream_memory.rs`
- Modify: `src-tauri/src/ai/mod.rs`

- [ ] **Step 1: Write failing workflow template tests**

Create `workflow_template.rs` with tests first:

```rust
#[test]
fn template_builds_one_ready_read_only_research_step() {
    let template = WorkflowTemplate::build(
        "Map the evidence for Bayesian memory",
        vec!["Every conclusion cites evidence".to_string()],
        100,
    )
    .unwrap();

    assert_eq!(template.workflow.steps.len(), 1);
    let step = &template.workflow.steps[0];
    assert_eq!(step.kind, WorkflowStepKind::Research);
    assert_eq!(step.mode, WorkflowStepMode::ReadOnly);
    assert_eq!(step.status, WorkflowStepStatus::Ready);
    assert_eq!(step.acceptance_criteria, vec!["Every conclusion cites evidence"]);
    assert!(template.registry.contains_agent("research_subagent"));
}

#[test]
fn template_rejects_empty_goal_and_empty_acceptance_contract() {
    assert_eq!(
        WorkflowTemplate::build(" ", vec!["evidence".to_string()], 100),
        Err(WorkflowTemplateError::GoalRequired)
    );
    assert_eq!(
        WorkflowTemplate::build("goal", vec![" ".to_string()], 100),
        Err(WorkflowTemplateError::AcceptanceCriteriaRequired)
    );
}
```

- [ ] **Step 2: Run template tests and verify RED**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml workflow_template -- --nocapture
```

Expected: the module and types do not exist.

- [ ] **Step 3: Implement the deterministic builder**

Define:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct WorkflowTemplate {
    pub goal: GoalContract,
    pub workflow: WorkflowIr,
    pub registry: AgentRegistry,
    pub run_id: String,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum WorkflowTemplateError {
    #[error("goal_required")]
    GoalRequired,
    #[error("acceptance_criteria_required")]
    AcceptanceCriteriaRequired,
}
```

Normalize acceptance criteria by trimming and removing empty/duplicate entries. Use deterministic safe IDs based on `now`:

```rust
let workflow_id = format!("wf_{now}");
let run_id = format!("run_{now}");
```

Build one `research_subagent` registry entry with the tools from `SandboxPolicy::read_only_research().tool_allowlist`, and one `S001` Research step with `inputs.question = goal_text`.

- [ ] **Step 4: Write failing acceptance-prompt test**

Add to `workflow_task_adapter.rs`:

```rust
#[test]
fn research_prompt_requires_an_exact_machine_checkable_acceptance_section() {
    let dispatch = research_dispatch("run_demo", "S001");

    let AdaptedWorkflowTask::Task(task) =
        WorkflowTaskAdapter::adapt(&dispatch, 100).unwrap()
    else {
        panic!("expected task");
    };

    assert!(task.content.contains("## Acceptance Check"));
    assert!(task.content.contains("- [ ] Every claim has a source"));
}
```

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml research_prompt_requires_an_exact_machine_checkable_acceptance_section -- --nocapture
```

Expected: the existing research question contains only the question text.

- [ ] **Step 5: Extend the existing Research prompt contract**

Append the acceptance contract in `research_question`:

```rust
let checklist = dispatch
    .artifact_contract
    .acceptance_criteria
    .iter()
    .map(|criterion| format!("- [ ] {}", criterion.trim()))
    .collect::<Vec<_>>()
    .join("\n");

format!(
    "{question}\n\n\
     Return a Markdown research report. Include an exact `## Acceptance Check` \
     section using the checklist below. Mark an item `[x]` only when the report's \
     cited evidence supports it; otherwise leave it `[ ]`.\n\n\
     ## Acceptance Check\n{checklist}"
)
```

The deterministic verifier later requires the exact normalized criterion to appear on a checked line. This is a contract-compliance check, not a claim of external truth.

- [ ] **Step 6: Write failing Dream artifact tests**

Add to `dream_memory.rs`:

```rust
#[test]
fn semantic_workflow_memory_writes_only_under_canonical_semantic_root() {
    let root = tempfile::tempdir().unwrap();
    let dualtrack = root.path().join(".dualtrack");
    ensure_dream_memory_layout(&dualtrack).unwrap();

    let artifact = write_semantic_workflow_memory(
        &dualtrack,
        "wf_100",
        "run_100",
        "S001",
        "# Verified knowledge\n",
        "100",
    )
    .unwrap();

    assert_eq!(
        artifact.uri,
        ".dualtrack/memory/semantic/workflows/wf_100/run_100.md"
    );
    assert_eq!(
        classify_ai_memory_path(&artifact.uri),
        Some(DreamMemoryZone::Semantic)
    );
    assert!(artifact.hash.starts_with("sha256:"));
    assert!(!dualtrack
        .join("memory/long_term/workflows/wf_100/run_100.md")
        .exists());
}
```

- [ ] **Step 7: Implement safe Working and Semantic artifact helpers**

Add:

```rust
pub fn working_result_artifact(
    vault_root: &Path,
    task_id: &str,
    step_id: &str,
    created_at: &str,
) -> Result<ArtifactRef, String>;

pub fn write_semantic_workflow_memory(
    dualtrack_dir: &Path,
    workflow_id: &str,
    run_id: &str,
    step_id: &str,
    content: &str,
    created_at: &str,
) -> Result<ArtifactRef, String>;
```

Validate all ID components with `safe_runtime_component`. The semantic writer must construct:

```rust
dualtrack_dir
    .join("memory")
    .join("semantic")
    .join("workflows")
    .join(workflow_id)
    .join(format!("{run_id}.md"))
```

Hash file bytes with SHA-256 and return an existing `ArtifactRef` with `RetentionPolicy::Workflow`.

- [ ] **Step 8: Run and commit template/Dream tests**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml workflow_template -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml workflow_task_adapter -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml dream_memory -- --nocapture
```

Then:

```powershell
git add src-tauri/src/ai/workflow_template.rs src-tauri/src/ai/workflow_task_adapter.rs src-tauri/src/ai/dream_memory.rs src-tauri/src/ai/mod.rs
git commit -m "feat: build deterministic dream workflow"
```

### Task 5: Complete, Verify, Promote, And Terminate The Existing Runtime

**Files:**
- Modify: `src-tauri/src/ai/workflow_runtime_service.rs`
- Modify: `src-tauri/src/harness/workflow.rs`
- Modify: `src-tauri/src/harness/scientist.rs`
- Test: `src-tauri/src/ai/workflow_runtime_service.rs`
- Test: `src-tauri/src/harness/scientist.rs`

- [ ] **Step 1: Write failing successful-completion test**

Add a fixture that writes a real trace/result under a temporary `.dualtrack`, completes the scheduler task with `TaskContext.retrieval_evidence`, then asserts:

```rust
#[test]
fn completed_research_promotes_verified_semantic_memory_and_succeeds_run() {
    let root = tempfile::tempdir().unwrap();
    let service = WorkflowRuntimeService::new(root.path());
    let mut scheduler = AgentScheduler::new(1);
    let bundle = service
        .start(
            goal(),
            workflow_with_ready_research(),
            registry(),
            "run_demo",
            &mut scheduler,
            100,
        )
        .unwrap();
    let task_id = bundle.dispatches[0].task_id.clone().unwrap();
    let dualtrack = root.path().join(".dualtrack");
    let context = TaskContext {
        intent: "Map Bayesian evidence".to_string(),
        retrieval_evidence: vec![TaskEvidence {
            source: "web".to_string(),
            entries: vec![TaskEvidenceEntry {
                title: "Bayesian evidence".to_string(),
                snippet: "Posterior updates preserve uncertainty.".to_string(),
                url: Some("https://example.test/evidence".to_string()),
                authors: vec![],
                year: Some(2026),
                source: "web".to_string(),
                relevance_score: 0.9,
            }],
            hop: 0,
            generated_keywords: vec!["bayesian evidence".to_string()],
            total_found: 1,
        }],
        ..TaskContext::default()
    };
    research_trace::write_path_log(
        &dualtrack,
        &task_id,
        0,
        "bayesian evidence",
        "web",
        &["https://example.test/evidence".to_string()],
        &[],
        "accepted source",
        Some(&context),
    )
    .unwrap();
    research_trace::write_cot_log(&dualtrack, &task_id, "research trace", Some(&context))
        .unwrap();
    let result = "## Findings\n\nEvidence-backed conclusion.\n\n\
                  ## Acceptance Check\n\n- [x] A dispatch record is persisted";
    research_trace::write_result_md(&dualtrack, &task_id, result, Some(&context)).unwrap();
    research_trace::write_context(&dualtrack, &task_id, &context).unwrap();
    scheduler.complete_with_context_and_dream_snapshot(
        &task_id,
        result.to_string(),
        Some(&context),
        None,
    );
    let task = scheduler.get_task(&task_id).unwrap().clone();

    let bundle = service
        .record_task_completion(&task, 200, &mut scheduler)
        .unwrap()
        .unwrap();

    assert_eq!(bundle.workflow.steps[0].status, WorkflowStepStatus::Verified);
    assert_eq!(bundle.run.status, RunStatus::Succeeded);
    assert!(bundle.run.ended_at.is_some());
    assert_eq!(bundle.step_reports.len(), 1);
    assert!(bundle
        .verification_findings
        .iter()
        .all(|finding| finding.result == VerificationOutcome::Pass));
    assert!(bundle.artifacts.iter().any(|artifact| {
        artifact.uri.starts_with(".dualtrack/memory/semantic/workflows/")
    }));
}
```

- [ ] **Step 2: Write failing verification-error test**

```rust
#[test]
fn missing_evidence_keeps_working_artifact_and_blocks_semantic_promotion() {
    let root = tempfile::tempdir().unwrap();
    let service = WorkflowRuntimeService::new(root.path());
    let mut scheduler = AgentScheduler::new(1);
    let bundle = service
        .start(
            goal(),
            workflow_with_ready_research(),
            registry(),
            "run_demo",
            &mut scheduler,
            100,
        )
        .unwrap();
    let task_id = bundle.dispatches[0].task_id.clone().unwrap();
    let dualtrack = root.path().join(".dualtrack");
    let context = TaskContext::default();
    research_trace::write_path_log(
        &dualtrack,
        &task_id,
        0,
        "unsupported conclusion",
        "local",
        &[],
        &[],
        "no evidence",
        Some(&context),
    )
    .unwrap();
    research_trace::write_cot_log(&dualtrack, &task_id, "research trace", Some(&context))
        .unwrap();
    let result = "## Findings\n\nUnsupported conclusion.\n\n\
                  ## Acceptance Check\n\n- [x] A dispatch record is persisted";
    research_trace::write_result_md(&dualtrack, &task_id, result, Some(&context)).unwrap();
    research_trace::write_context(&dualtrack, &task_id, &context).unwrap();
    scheduler.complete_with_context_and_dream_snapshot(
        &task_id,
        result.to_string(),
        Some(&context),
        None,
    );
    let task = scheduler.get_task(&task_id).unwrap().clone();

    let bundle = service
        .record_task_completion(&task, 200, &mut scheduler)
        .unwrap()
        .unwrap();

    assert_eq!(bundle.run.status, RunStatus::Failed);
    assert!(bundle
        .verification_findings
        .iter()
        .any(|finding| finding.reason_code == "evidence_missing"));
    assert!(!root
        .path()
        .join(".dualtrack/memory/semantic/workflows")
        .exists());
}
```

- [ ] **Step 3: Run both tests and verify RED**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml completed_research_promotes_verified_semantic_memory_and_succeeds_run -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml missing_evidence_keeps_working_artifact_and_blocks_semantic_promotion -- --nocapture
```

Expected: `record_task_completion` and verification/promotion logic do not exist.

- [ ] **Step 4: Write a failing real-report claim extraction test**

Add to `scientist.rs`:

```rust
fn completed_task(result: &str) -> AgentTask {
    AgentTask {
        id: "scientist-report".to_string(),
        command: crate::cli::parser::CliCommand::Custom("research".to_string()),
        task_type: crate::ai::agent_scheduler::TaskType::DeepDive,
        task_intent: Some(crate::ai::task_intent::TaskIntentType::Research),
        sandbox_policy: None,
        priority: crate::ai::agent_scheduler::TaskPriority::Low,
        priority_score: 0,
        status: crate::ai::agent_scheduler::TaskStatus::Done {
            completed_at: 100,
            result: result.to_string(),
        },
        anchor_note: None,
        created_at: 1,
        max_retries: 0,
        retry_count: 0,
        synthesize_phase: crate::ai::agent_scheduler::SynthesizePhase::Idle,
        subagent_results: vec![],
        graph_manifest: None,
        has_trace: true,
        source_block_id: None,
        card_id: None,
        card_type: None,
        prompt: None,
        params: None,
        context_note: None,
        intent: "research".to_string(),
        content: "research".to_string(),
        max_iterations: 1,
        sub_tasks: vec![],
        material_packet: None,
        context_fragments: vec![],
        regression_metrics: None,
        retry_delay_ms: 0,
        retry_backoff_multiplier: 1.0,
        last_retry_at: None,
        consecutive_failures: 0,
    }
}

#[test]
fn completed_research_report_becomes_scientist_claim_material() {
    let task = completed_task(
        "## Findings\n\nBayesian updates preserve uncertainty.\n\n\
         ## Acceptance Check\n\n- [x] Every claim has a source",
    );

    let knowledge = Scientist::extract_knowledge(&task);

    assert!(knowledge
        .claims
        .contains(&"Bayesian updates preserve uncertainty.".to_string()));
    assert!(!knowledge
        .claims
        .contains(&"Every claim has a source".to_string()));
}
```

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml completed_research_report_becomes_scientist_claim_material -- --nocapture
```

Expected: the current Scientist only uses completed subtask descriptions.

- [ ] **Step 5: Extract claims from the completed Markdown report**

Use `pulldown_cmark::Parser` to collect paragraph and list-item text from the completed task result. Exclude content under `Acceptance Check`, `Citations`, and `Excluded Sources`. Prefer report claims when non-empty; use completed subtask descriptions only as a compatibility fallback.

The extraction entry remains the existing API:

```rust
pub fn extract_knowledge(task: &AgentTask) -> CleanKnowledge
```

Do not add a second Scientist service or a second model call.

- [ ] **Step 6: Implement immediate runtime callbacks**

Add public methods returning `Ok(None)` for non-workflow tasks:

```rust
pub fn record_task_running(
    &self,
    task: &AgentTask,
    now: u64,
) -> Result<Option<WorkflowRuntimeBundle>, WorkflowError>;

pub fn record_task_completion(
    &self,
    task: &AgentTask,
    now: u64,
    scheduler: &mut AgentScheduler,
) -> Result<Option<WorkflowRuntimeBundle>, WorkflowError>;

pub fn record_task_failure(
    &self,
    task: &AgentTask,
    reason_code: &str,
    summary: &str,
    now: u64,
) -> Result<Option<WorkflowRuntimeBundle>, WorkflowError>;

pub fn list(&self) -> Result<Vec<WorkflowRuntimeBundle>, WorkflowError>;
```

Use `workflow_task_context(task)` to locate the run. Completion must:

1. load the bundle;
2. locate the existing Working result/context/trace;
3. create a Working `ArtifactRef`;
4. create one `StepReport`;
5. extract `CleanKnowledge` from the completed scheduler task;
6. parse `## Acceptance Check` and require every exact normalized acceptance criterion on a checked `- [x]` line;
7. create one `VerificationFinding` per failed contract check, or pass findings for all acceptance clauses;
8. write Semantic memory only when every finding passes;
9. mark step `Verified`, workflow `Completed`, run `Succeeded`, and set `ended_at`;
10. otherwise mark step/run failed without deleting Working artifacts;
11. persist and append report/verification/terminal events.

The semantic Markdown renderer must include workflow/run/task IDs, Working artifact URI/hash, claims, evidence chain, confidence, acceptance criteria, and kernel name.

`list` must reuse `WorkflowRuntimeStore::list_run_ids`, load every valid bundle, and sort descending by numeric-or-lexical `updated_at`. Corrupt runs return a typed error from the focused command instead of being silently converted into empty state.

- [ ] **Step 7: Add terminal event constructors**

Add constructors to `WorkflowRuntimeEventChain` for:

```text
workflow.step.verified
workflow.semantic.promoted
workflow.run.succeeded
workflow.run.failed
```

Event attributes contain IDs, reason codes, and artifact refs only. Event bodies contain short summaries, never research body text.

- [ ] **Step 8: Run service tests and commit**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml workflow_runtime_service -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml scientist -- --nocapture
```

Then:

```powershell
git add src-tauri/src/ai/workflow_runtime_service.rs src-tauri/src/harness/workflow.rs src-tauri/src/harness/scientist.rs
git commit -m "feat: verify and promote workflow results"
```

### Task 6: Wire The Existing Worker And Expose Focused Commands

**Files:**
- Create: `src-tauri/src/ai/workflow_commands.rs`
- Modify: `src-tauri/src/ai/commands.rs:2669-2803`
- Modify: `src-tauri/src/ai/mod.rs`
- Modify: `src-tauri/src/main.rs`

- [ ] **Step 1: Write failing command helper tests**

In `workflow_commands.rs` add pure root helpers and tests:

```rust
#[test]
fn create_and_start_routes_template_into_existing_runtime_and_scheduler() {
    let root = tempfile::tempdir().unwrap();
    let mut scheduler = AgentScheduler::new(1);

    let bundle = create_and_start_for_root(
        root.path(),
        "Map Bayesian memory evidence".to_string(),
        vec!["Every conclusion cites evidence".to_string()],
        &mut scheduler,
        100,
    )
    .unwrap();

    assert_eq!(bundle.workflow.steps.len(), 1);
    assert!(scheduler
        .get_task("workflow__run_100__S001__attempt_1")
        .is_some());
}

#[test]
fn list_workflow_runs_reads_existing_runtime_store() {
    let root = tempfile::tempdir().unwrap();
    let mut scheduler = AgentScheduler::new(1);
    create_and_start_for_root(
        root.path(),
        "Goal".to_string(),
        vec!["Evidence".to_string()],
        &mut scheduler,
        100,
    )
    .unwrap();

    let runs = list_workflow_runs_for_root(root.path()).unwrap();

    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].run.run_id, "run_100");
}
```

- [ ] **Step 2: Run command tests and verify RED**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml workflow_commands -- --nocapture
```

Expected: module and helper functions do not exist.

- [ ] **Step 3: Implement thin Tauri commands**

Define:

```rust
#[tauri::command]
pub(crate) fn create_and_start_workflow(
    goal_text: String,
    acceptance_criteria: Vec<String>,
    state: State<'_, Mutex<AppState>>,
    ai_state: State<'_, Mutex<AiState>>,
) -> Result<WorkflowRuntimeBundle, String>;

#[tauri::command]
pub(crate) fn get_workflow_run(
    run_id: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<WorkflowRuntimeBundle, String>;

#[tauri::command]
pub(crate) fn list_workflow_runs(
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<WorkflowRuntimeBundle>, String>;
```

Require an open vault, use `WorkflowTemplate::build`, call the existing Runtime service, notify the existing task worker once, and sort listed bundles by `updated_at` descending.

- [ ] **Step 4: Wire runtime callbacks into `start_task_worker`**

Before execution, call `record_task_running` with the dequeued task. On success:

1. complete the scheduler task;
2. write result/context into existing Working Memory;
3. clone the now-completed scheduler task;
4. call `record_task_completion`;
5. emit `workflow-run-updated` with the returned bundle;
6. emit existing task/research events.

On `execute_agent_task_async` error and `spawn_blocking` join error:

1. call `agent_scheduler.fail`;
2. clone the failed task;
3. call `record_task_failure` with `task_execution_failed` or `task_join_failed`;
4. emit `workflow-run-updated` when applicable.

- [ ] **Step 5: Register commands and run backend tests**

Export the module and register:

```rust
ai::workflow_commands::create_and_start_workflow,
ai::workflow_commands::get_workflow_run,
ai::workflow_commands::list_workflow_runs,
```

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml workflow_commands -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml workflow_runtime_service -- --nocapture
```

- [ ] **Step 6: Commit worker and command wiring**

```powershell
git add src-tauri/src/ai/workflow_commands.rs src-tauri/src/ai/commands.rs src-tauri/src/ai/mod.rs src-tauri/src/main.rs
git commit -m "feat: expose orchestrator workflow runtime"
```

### Task 7: Add Concrete Frontend Runtime State Without A Second Store

**Files:**
- Create: `src/test/workflowFixtures.ts`
- Modify: `src/types/orchestrator.ts`
- Modify: `src/hooks/useAppStore.ts`
- Modify: `src/hooks/__tests__/useAppStore.workflow.test.ts`

- [ ] **Step 1: Write failing Zustand IPC tests**

Create the shared typed fixture:

```typescript
import type { WorkflowRuntimeBundle } from "../types/orchestrator";

export const runtimeBundle: WorkflowRuntimeBundle = {
  goal: {
    goal_id: "goal_100",
    goal_text: "Map Bayesian evidence",
    success_definition: ["Every conclusion cites evidence"],
    non_goals: [],
    constraints: {},
    context_scope: [],
    approval_policy: {},
    budget: { max_iterations: 30 },
    created_at: "100",
  },
  workflow: {
    workflow_id: "wf_100",
    goal_id: "goal_100",
    version: 1,
    parent_version: null,
    status: "completed",
    global_context: {},
    control_policy: {
      max_parallel_steps: 1,
      replan_on_verification_fail: true,
      max_patch_chain: 1,
    },
    steps: [{
      step_id: "S001",
      title: "Research goal",
      kind: "research",
      agent_type: "research_subagent",
      mode: "read_only",
      task: "Map Bayesian evidence",
      inputs: { question: "Map Bayesian evidence" },
      dependencies: [],
      acceptance_criteria: ["Every conclusion cites evidence"],
      goal_alignment: {
        success_clauses: [1],
        why_necessary: "Research satisfies the goal",
      },
      retry_policy: { max_attempts: 1, backoff_ms: 0 },
      status: "verified",
    }],
    created_by: "orchestrator@template",
    created_at: "100",
  },
  run: {
    run_id: "run_100",
    workflow_id: "wf_100",
    workflow_version: 1,
    status: "succeeded",
    started_at: "100",
    ended_at: "200",
    active_step_ids: [],
    worktree_map: {},
    metrics: {},
    context_digest_version: 1,
  },
  registry: {
    agents: {
      research_subagent: {
        agent_type: "research_subagent",
        allowed_tools: ["vector_search", "web_search"],
        denied_tools: [],
        default_mode: "read_only",
        max_parallelism: 1,
        can_delegate: false,
      },
    },
  },
  dispatches: [{
    step_id: "S001",
    attempt: 1,
    task_id: "workflow__run_100__S001__attempt_1",
    status: "reported",
    detail: "research result recorded",
  }],
  artifacts: [
    {
      artifact_id: "working-result",
      type: "other",
      uri: ".dualtrack/research/results/workflow__run_100__S001__attempt_1/result.md",
      hash: "sha256:working",
      mime_type: "text/markdown",
      producer_step_id: "S001",
      retention_policy: "workflow",
      created_at: "200",
    },
    {
      artifact_id: "semantic-result",
      type: "verification_report",
      uri: ".dualtrack/memory/semantic/workflows/wf_100/run_100.md",
      hash: "sha256:semantic",
      mime_type: "text/markdown",
      producer_step_id: "S001",
      retention_policy: "workflow",
      created_at: "200",
    },
  ],
  step_reports: [{
    report_id: "report_run_100_S001",
    step_id: "S001",
    attempt: 1,
    status: "completed",
    summary: "Research contract passed",
    artifacts: [],
    evidence: [{
      file: ".dualtrack/research/results/workflow__run_100__S001__attempt_1/context.json",
      lines: [],
      claim: "Evidence exists",
    }],
    risks: [],
    blocked_by: [],
    suggested_next_steps: [],
    resource_usage: {},
    confidence: 0.9,
  }],
  verification_findings: [{
    verification_id: "verify_run_100_S001_1",
    level: "step",
    target: "S001",
    result: "pass",
    failed_clauses: [],
    reason_code: "acceptance_criterion_passed",
    summary: "Every conclusion cites evidence",
    evidence_refs: ["working-result"],
    minimal_fix_surface: [],
  }],
  updated_at: "200",
};
```

Import it into `useAppStore.workflow.test.ts`, then add:

```typescript
it("creates and stores a workflow run through the focused command", async () => {
  invokeMock.mockResolvedValueOnce(runtimeBundle);

  const result = await useAppStore.getState().createAndStartWorkflow(
    "Map Bayesian evidence",
    ["Every conclusion cites evidence"],
  );

  expect(invokeMock).toHaveBeenCalledWith("create_and_start_workflow", {
    goalText: "Map Bayesian evidence",
    acceptanceCriteria: ["Every conclusion cites evidence"],
  });
  expect(result).toEqual(runtimeBundle);
  expect(useAppStore.getState().activeWorkflowRun).toEqual(runtimeBundle);
});

it("lists workflow runs and keeps the newest run active", async () => {
  invokeMock.mockResolvedValueOnce([runtimeBundle]);

  await useAppStore.getState().fetchWorkflowRuns();

  expect(invokeMock).toHaveBeenCalledWith("list_workflow_runs");
  expect(useAppStore.getState().workflowRuns).toEqual([runtimeBundle]);
  expect(useAppStore.getState().activeWorkflowRun?.run.run_id).toBe("run_100");
});
```

- [ ] **Step 2: Run tests and verify RED**

Run:

```powershell
npm.cmd test -- src/hooks/__tests__/useAppStore.workflow.test.ts
```

Expected: concrete runtime types and actions do not exist.

- [ ] **Step 3: Mirror concrete Rust types**

Add concrete TypeScript interfaces for Goal, step, workflow, run, dispatch, artifact, report, finding, registry, and bundle. Use string unions matching Rust snake-case values:

```typescript
export type WorkflowStepStatus =
  | "pending"
  | "ready"
  | "running"
  | "reported"
  | "verified"
  | "failed"
  | "blocked"
  | "skipped";

export interface WorkflowRuntimeBundle {
  goal: GoalContract;
  workflow: WorkflowIr;
  run: WorkflowRunState;
  registry: AgentRegistry;
  dispatches: WorkflowDispatchRecord[];
  artifacts: ArtifactRef[];
  step_reports: StepReport[];
  verification_findings: VerificationFinding[];
  updated_at: string;
}
```

- [ ] **Step 4: Extend the existing Zustand store**

Add:

```typescript
workflowRuns: WorkflowRuntimeBundle[];
activeWorkflowRun: WorkflowRuntimeBundle | null;
workflowRunLoading: boolean;
workflowRunError: string | null;
createAndStartWorkflow: (
  goalText: string,
  acceptanceCriteria: string[],
) => Promise<WorkflowRuntimeBundle | null>;
fetchWorkflowRuns: () => Promise<WorkflowRuntimeBundle[]>;
fetchWorkflowRun: (runId: string) => Promise<WorkflowRuntimeBundle | null>;
applyWorkflowRunUpdate: (bundle: WorkflowRuntimeBundle) => void;
```

`applyWorkflowRunUpdate` replaces by `run_id`, sorts by `updated_at`, and updates `activeWorkflowRun`. Do not persist workflow run state to localStorage; disk Runtime remains the source of truth.

- [ ] **Step 5: Run store tests and commit**

Run:

```powershell
npm.cmd test -- src/hooks/__tests__/useAppStore.workflow.test.ts
```

Then:

```powershell
git add src/test/workflowFixtures.ts src/types/orchestrator.ts src/hooks/useAppStore.ts src/hooks/__tests__/useAppStore.workflow.test.ts
git commit -m "feat: track workflow runtime in app store"
```

### Task 8: Upgrade The Existing Orchestration View And Simplify Duplicate Status

**Files:**
- Modify: `src/components/OrchestratorWorkflowView.tsx`
- Modify: `src/components/OrchestratorPanel.tsx`
- Modify: `src/lib/orchestratorEventPresentation.ts`
- Move: `src/components/__tests__/OrchestratorWorkflowView.test.ts` to `src/components/__tests__/OrchestratorWorkflowView.test.tsx`
- Modify: `src/components/__tests__/OrchestratorPanel.test.tsx`
- Modify: `src/lib/__tests__/orchestratorEventPresentation.test.ts`
- Modify: `src/App.tsx`

- [ ] **Step 1: Write failing orchestration view tests**

Replace the helper-only test with component behavior:

```typescript
import { runtimeBundle } from "../../test/workflowFixtures";

it("submits a goal and acceptance criteria through the existing orchestration view", async () => {
  const createAndStartWorkflow = vi.fn(async () => runtimeBundle);
  useAppStore.setState({
    createAndStartWorkflow,
    workflowRuns: [],
    activeWorkflowRun: null,
    workflowRunLoading: false,
    workflowRunError: null,
  });

  render(<OrchestratorWorkflowView />);
  fireEvent.change(screen.getByLabelText("工作流目标"), {
    target: { value: "Map Bayesian evidence" },
  });
  fireEvent.change(screen.getByLabelText("验收条件 1"), {
    target: { value: "Every conclusion cites evidence" },
  });
  fireEvent.click(screen.getByRole("button", { name: "启动工作流" }));

  await waitFor(() => {
    expect(createAndStartWorkflow).toHaveBeenCalledWith(
      "Map Bayesian evidence",
      ["Every conclusion cites evidence"],
    );
  });
});

it("shows real run, artifact, and verification state", () => {
  useAppStore.setState({ activeWorkflowRun: runtimeBundle });

  render(<OrchestratorWorkflowView />);

  expect(screen.getByText("run_100")).toBeDefined();
  expect(screen.getByText("已验证")).toBeDefined();
  expect(screen.getByText(/Working Memory/)).toBeDefined();
  expect(screen.getByText(/Semantic Memory/)).toBeDefined();
});
```

- [ ] **Step 2: Run tests and verify RED**

Run:

```powershell
npm.cmd test -- src/components/__tests__/OrchestratorWorkflowView.test.tsx
```

Expected: the existing view has no form or concrete runtime rendering.

- [ ] **Step 3: Implement the orchestration main view**

Use the existing component file and add:

- one goal textarea;
- a list of acceptance inputs with add/remove controls;
- a start button disabled for empty goal/criteria, loading, or no vault;
- active run summary;
- one row per workflow step;
- artifact rows grouped by `working` and `semantic` from URI;
- verification finding rows;
- existing event timeline using `workflowEventLabel/detail`;
- empty, loading, and error states.

On mount call `fetchWorkflowRuns`. Listen to `workflow-run-updated` in `App.tsx` and pass the payload to `applyWorkflowRunUpdate`; after each update refresh the existing vault data so Dream files appear in the existing browser.

- [ ] **Step 4: Simplify the bottom Orchestrator panel**

Update tests first to assert the panel contains macro metrics but no full ledger button or artifact list. Then keep:

```text
current run status
active tasks
verification failures
replan count
latest abnormal summary
```

Remove duplicate full workflow ledger, track table, and detailed diagnostic lists from `OrchestratorPanel`; those remain in the main orchestration view and Agent dashboard.

- [ ] **Step 5: Add event labels and run frontend tests**

Add readable labels for:

```text
workflow.run.created
workflow.run.resumed
workflow.step.queued
workflow.step.running
workflow.step.reported
workflow.step.verified
workflow.semantic.promoted
workflow.run.succeeded
workflow.run.failed
```

Run:

```powershell
npm.cmd test -- src/components/__tests__/OrchestratorWorkflowView.test.tsx src/components/__tests__/OrchestratorPanel.test.tsx src/lib/__tests__/orchestratorEventPresentation.test.ts src/components/__tests__/AppLayout.test.ts
```

- [ ] **Step 6: Commit the frontend orchestration flow**

```powershell
git add src/components/OrchestratorWorkflowView.tsx src/components/OrchestratorPanel.tsx src/lib/orchestratorEventPresentation.ts src/components/__tests__/OrchestratorWorkflowView.test.ts src/components/__tests__/OrchestratorWorkflowView.test.tsx src/components/__tests__/OrchestratorPanel.test.tsx src/lib/__tests__/orchestratorEventPresentation.test.ts src/App.tsx
git commit -m "feat: operate workflows from orchestrator view"
```

### Task 9: Full Verification And Real Narrow Loop

**Files:**
- Verify: `src-tauri/src/ai/workflow_runtime_service.rs`
- Verify: `src-tauri/src/ai/workflow_commands.rs`
- Verify: `src-tauri/src/diff/commands.rs`
- Verify: `src/components/OrchestratorWorkflowView.tsx`
- Verify: `src/components/DiffView.tsx`

- [ ] **Step 1: Run the complete Rust suite**

Run:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
```

Expected: all library, binary, and doc tests pass with zero failures.

- [ ] **Step 2: Run the complete frontend suite**

Run:

```powershell
Set-Location -LiteralPath 'D:\新项目仓库\贝叶斯笔记'
npm.cmd test -- --run
```

Expected: all Vitest files pass with zero failures.

- [ ] **Step 3: Run production builds**

Run:

```powershell
npm.cmd run build
npm.cmd run tauri -- build --no-bundle
```

Expected: TypeScript/Vite and the Tauri executable build successfully. Existing large-chunk and unrelated dead-code warnings may remain.

- [ ] **Step 4: Execute a real temporary-vault workflow**

Start the app with a temporary vault and configured API, submit one goal through the orchestration view, and verify:

```text
.harness/runs/<run_id>/runtime.json exists
runtime.json does not contain the generated research paragraph
events.jsonl contains created, queued, running, reported, verified, semantic promoted, succeeded
.dualtrack/research/results/<task_id>/result.md contains the generated report
.dualtrack/research/results/<task_id>/context.json contains retrieval evidence
.dualtrack/memory/semantic/workflows/<workflow_id>/<run_id>.md exists
the run shows succeeded after refresh
the AI Working and Semantic files appear in the existing Dream zones
no human note file changes
resume/open-vault does not create a duplicate deterministic task
```

Use the in-app browser or Computer Use only after automated suites pass. Never expose API keys in screenshots, logs, runtime files, or the final report.

- [ ] **Step 5: Verify the Diff feedback boundary manually**

Open a real Ghost suggestion, click `采纳反馈`, and verify:

```text
Ghost/Bridge status changes
the human source note file hash remains unchanged
no file-changed event is emitted for the human note
the UI states that feedback was recorded
```

- [ ] **Step 6: Review scope and whitespace**

Run:

```powershell
git diff --check
git status --short
```

Expected: no whitespace errors. Unrelated pre-existing dirty files remain untouched.
