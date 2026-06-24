use crate::harness::workflow::{
    safe_runtime_component, AgentRegistry, ArtifactRef, GoalContract, StepReport,
    VerificationFinding, WorkflowError, WorkflowIr, WorkflowRunState, WorkflowRuntimeEventStore,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

const RUNTIME_FILE_NAME: &str = "runtime.json";

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
    #[serde(default)]
    pub artifacts: Vec<ArtifactRef>,
    #[serde(default)]
    pub step_reports: Vec<StepReport>,
    #[serde(default)]
    pub verification_findings: Vec<VerificationFinding>,
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

#[derive(Debug, Clone)]
pub struct WorkflowRuntimeStore {
    root: PathBuf,
}

impl WorkflowRuntimeStore {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }

    pub fn runtime_path(&self, run_id: &str) -> Result<PathBuf, WorkflowError> {
        WorkflowRuntimeEventStore::event_log_path(&self.root, run_id)
            .map(|path| path.with_file_name(RUNTIME_FILE_NAME))
    }

    pub fn write(&self, bundle: &WorkflowRuntimeBundle) -> Result<PathBuf, WorkflowError> {
        self.write_with_persist(bundle, |temp, path| {
            temp.persist(path)
                .map(|_| ())
                .map_err(|err| format!("persist {}: {}", path.display(), err.error))
        })
    }

    fn write_with_persist<F>(
        &self,
        bundle: &WorkflowRuntimeBundle,
        persist: F,
    ) -> Result<PathBuf, WorkflowError>
    where
        F: FnOnce(NamedTempFile, &Path) -> Result<(), String>,
    {
        let path = self.runtime_path(&bundle.run.run_id)?;
        validate_bundle_contract(bundle)?;
        let encoded = serde_json::to_vec_pretty(bundle)
            .map_err(|err| WorkflowError::RuntimeStateParse(err.to_string()))?;
        let run_dir = path
            .parent()
            .expect("runtime path always has a run directory");
        fs::create_dir_all(run_dir).map_err(|err| {
            WorkflowError::RuntimeStateIo(format!(
                "create runtime directory {}: {err}",
                run_dir.display()
            ))
        })?;

        let mut temp = NamedTempFile::new_in(run_dir).map_err(|err| {
            WorkflowError::RuntimeStateIo(format!(
                "create temporary runtime state in {}: {err}",
                run_dir.display()
            ))
        })?;
        temp.write_all(&encoded).map_err(|err| {
            WorkflowError::RuntimeStateIo(format!("write {}: {err}", path.display()))
        })?;
        temp.flush().map_err(|err| {
            WorkflowError::RuntimeStateIo(format!("flush {}: {err}", path.display()))
        })?;
        temp.as_file().sync_all().map_err(|err| {
            WorkflowError::RuntimeStateIo(format!("sync {}: {err}", path.display()))
        })?;
        persist(temp, &path).map_err(WorkflowError::RuntimeStateIo)?;

        Ok(path)
    }

    pub fn read(&self, run_id: &str) -> Result<WorkflowRuntimeBundle, WorkflowError> {
        let path = self.runtime_path(run_id)?;
        let encoded = match fs::read(&path) {
            Ok(encoded) => encoded,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Err(WorkflowError::RuntimeStateMissing(run_id.to_string()))
            }
            Err(err) => {
                return Err(WorkflowError::RuntimeStateIo(format!(
                    "read {}: {err}",
                    path.display()
                )))
            }
        };
        let bundle: WorkflowRuntimeBundle = serde_json::from_slice(&encoded).map_err(|err| {
            WorkflowError::RuntimeStateParse(format!("parse {}: {err}", path.display()))
        })?;
        if bundle.run.run_id != run_id {
            return Err(WorkflowError::RuntimeStateParse(format!(
                "runtime state at {} contains run id {}, expected {run_id}",
                path.display(),
                bundle.run.run_id
            )));
        }
        validate_bundle_contract(&bundle).map_err(|err| {
            WorkflowError::RuntimeStateParse(format!(
                "runtime state contract at {} is invalid: {err}",
                path.display()
            ))
        })?;
        Ok(bundle)
    }

    pub fn list_run_ids(&self) -> Result<Vec<String>, WorkflowError> {
        let runs_dir = self.root.join(".harness").join("runs");
        let entries = match fs::read_dir(&runs_dir) {
            Ok(entries) => entries,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(err) => {
                return Err(WorkflowError::RuntimeStateIo(format!(
                    "list {}: {err}",
                    runs_dir.display()
                )))
            }
        };
        let mut run_ids = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|err| {
                WorkflowError::RuntimeStateIo(format!("list {}: {err}", runs_dir.display()))
            })?;
            let file_type = entry.file_type().map_err(|err| {
                WorkflowError::RuntimeStateIo(format!(
                    "inspect runtime entry {}: {err}",
                    entry.path().display()
                ))
            })?;
            if !file_type.is_dir() {
                continue;
            }
            let Some(run_id) = entry.file_name().to_str().map(str::to_string) else {
                continue;
            };
            if safe_runtime_component(&run_id).is_err()
                || !entry.path().join(RUNTIME_FILE_NAME).is_file()
            {
                continue;
            }
            run_ids.push(run_id);
        }
        run_ids.sort();
        Ok(run_ids)
    }
}

fn validate_bundle_contract(bundle: &WorkflowRuntimeBundle) -> Result<(), WorkflowError> {
    if bundle.workflow.goal_id != bundle.goal.goal_id {
        return Err(WorkflowError::GoalMismatch {
            workflow_goal_id: bundle.workflow.goal_id.clone(),
            contract_goal_id: bundle.goal.goal_id.clone(),
        });
    }
    if bundle.run.workflow_id != bundle.workflow.workflow_id
        || bundle.run.workflow_version != bundle.workflow.version
    {
        return Err(WorkflowError::RunWorkflowMismatch {
            run_workflow_id: bundle.run.workflow_id.clone(),
            run_workflow_version: bundle.run.workflow_version,
            workflow_id: bundle.workflow.workflow_id.clone(),
            workflow_version: bundle.workflow.version,
        });
    }
    bundle.workflow.validate(&bundle.goal, &bundle.registry)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::workflow::{
        AgentRegistry, AgentRegistryEntry, ArtifactRef, ArtifactType, ControlPolicy, GoalAlignment,
        GoalContract, RetentionPolicy, RetryPolicy, RunStatus, WorkflowError, WorkflowIr,
        WorkflowRunState, WorkflowStatus, WorkflowStep, WorkflowStepKind, WorkflowStepMode,
        WorkflowStepStatus,
    };
    use serde_json::json;
    use std::fs;

    fn bundle(run_id: &str, updated_at: &str) -> WorkflowRuntimeBundle {
        let goal = GoalContract {
            goal_id: "goal_runtime".to_string(),
            goal_text: "Persist workflow runtime state".to_string(),
            success_definition: vec!["Runtime state survives restart".to_string()],
            non_goals: vec![],
            constraints: json!({"vault_backed": true}),
            context_scope: vec!["src-tauri/src/harness/**".to_string()],
            approval_policy: json!({}),
            budget: json!({"max_iterations": 2}),
            created_at: "2026-06-22T00:00:00Z".to_string(),
        };
        let workflow = WorkflowIr {
            workflow_id: "wf_runtime".to_string(),
            goal_id: goal.goal_id.clone(),
            version: 1,
            parent_version: None,
            status: WorkflowStatus::Running,
            global_context: json!({}),
            control_policy: ControlPolicy {
                max_parallel_steps: 1,
                replan_on_verification_fail: true,
                max_patch_chain: 2,
            },
            steps: vec![WorkflowStep {
                step_id: "S001".to_string(),
                title: "Persist runtime".to_string(),
                kind: WorkflowStepKind::Implement,
                agent_type: "code_writer".to_string(),
                mode: WorkflowStepMode::WritePatch,
                task: "Write the runtime bundle".to_string(),
                inputs: json!({}),
                dependencies: vec![],
                acceptance_criteria: vec!["runtime.json can be read back".to_string()],
                goal_alignment: GoalAlignment {
                    success_clauses: vec![1],
                    why_necessary: "Durable state is the goal".to_string(),
                },
                retry_policy: RetryPolicy {
                    max_attempts: 2,
                    backoff_ms: 10,
                },
                status: WorkflowStepStatus::Running,
            }],
            created_by: "test".to_string(),
            created_at: "2026-06-22T00:00:01Z".to_string(),
        };
        let run = WorkflowRunState {
            run_id: run_id.to_string(),
            workflow_id: workflow.workflow_id.clone(),
            workflow_version: workflow.version,
            status: RunStatus::Running,
            started_at: "2026-06-22T00:00:02Z".to_string(),
            ended_at: None,
            active_step_ids: vec!["S001".to_string()],
            worktree_map: Default::default(),
            metrics: json!({}),
            context_digest_version: 1,
        };
        let registry = AgentRegistry::from_agents(vec![AgentRegistryEntry {
            agent_type: "code_writer".to_string(),
            allowed_tools: vec!["Read".to_string(), "Edit".to_string()],
            denied_tools: vec![],
            default_mode: WorkflowStepMode::WritePatch,
            max_parallelism: 1,
            can_delegate: false,
        }]);

        WorkflowRuntimeBundle {
            goal,
            workflow,
            run,
            registry,
            dispatches: vec![WorkflowDispatchRecord {
                step_id: "S001".to_string(),
                attempt: 1,
                task_id: Some("task-1".to_string()),
                status: WorkflowDispatchStatus::Running,
                detail: Some("worker accepted task".to_string()),
            }],
            artifacts: Vec::new(),
            step_reports: Vec::new(),
            verification_findings: Vec::new(),
            updated_at: updated_at.to_string(),
        }
    }

    #[test]
    fn runtime_types_serialize_dispatch_status_as_snake_case() {
        assert_eq!(
            serde_json::to_value(WorkflowDispatchStatus::Unsupported).unwrap(),
            json!("unsupported")
        );

        let context = WorkflowTaskContext {
            workflow_id: "wf_runtime".to_string(),
            run_id: "run-a".to_string(),
            step_id: "S001".to_string(),
            attempt: 2,
            acceptance_criteria: vec!["tests pass".to_string()],
        };
        assert_eq!(
            serde_json::from_value::<WorkflowTaskContext>(serde_json::to_value(&context).unwrap())
                .unwrap(),
            context
        );
    }

    #[test]
    fn runtime_bundle_defaults_missing_reference_collections_to_empty() {
        let mut encoded =
            serde_json::to_value(bundle("run-legacy", "2026-06-22T00:01:00Z")).unwrap();
        let object = encoded.as_object_mut().unwrap();
        object.remove("dispatches").unwrap();
        object.remove("artifacts").unwrap();
        object.remove("step_reports").unwrap();
        object.remove("verification_findings").unwrap();

        let decoded: WorkflowRuntimeBundle = serde_json::from_value(encoded).unwrap();

        assert!(decoded.dispatches.is_empty());
        assert!(decoded.artifacts.is_empty());
        assert!(decoded.step_reports.is_empty());
        assert!(decoded.verification_findings.is_empty());
    }

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

    #[test]
    fn runtime_store_round_trips_bundle_and_lists_only_valid_runtime_runs() {
        let vault = tempfile::tempdir().unwrap();
        let store = WorkflowRuntimeStore::new(vault.path());
        let expected = bundle("run-b", "2026-06-22T00:01:00Z");

        let path = store.write(&expected).unwrap();
        store
            .write(&bundle("run-a", "2026-06-22T00:02:00Z"))
            .unwrap();
        fs::create_dir_all(vault.path().join(".harness/runs/no-runtime")).unwrap();
        fs::create_dir_all(vault.path().join(".harness/runs/unsafe name")).unwrap();
        fs::write(
            vault.path().join(".harness/runs/unsafe name/runtime.json"),
            b"{}",
        )
        .unwrap();

        assert_eq!(path, vault.path().join(".harness/runs/run-b/runtime.json"));
        assert_eq!(store.read("run-b").unwrap(), expected);
        assert_eq!(store.list_run_ids().unwrap(), vec!["run-a", "run-b"]);
    }

    #[test]
    fn runtime_store_rejects_unsafe_run_id_before_writing() {
        let vault = tempfile::tempdir().unwrap();
        let store = WorkflowRuntimeStore::new(vault.path());

        assert_eq!(
            store.write(&bundle("../escape", "2026-06-22T00:01:00Z")),
            Err(WorkflowError::UnsafeRuntimeComponent(
                "../escape".to_string()
            ))
        );
        assert!(!vault.path().join(".harness/escape").exists());
        assert!(!vault.path().join("escape/runtime.json").exists());
    }

    #[test]
    fn runtime_store_rejects_padded_run_id_before_writing() {
        let vault = tempfile::tempdir().unwrap();
        let store = WorkflowRuntimeStore::new(vault.path());

        assert_eq!(
            store.write(&bundle(" run-padded ", "2026-06-22T00:01:00Z")),
            Err(WorkflowError::UnsafeRuntimeComponent(
                " run-padded ".to_string()
            ))
        );
        assert!(!vault.path().join(".harness/runs/run-padded").exists());
    }

    #[test]
    fn runtime_store_rejects_run_workflow_mismatch_before_writing() {
        let vault = tempfile::tempdir().unwrap();
        let store = WorkflowRuntimeStore::new(vault.path());
        let mut invalid = bundle("run-mismatch", "2026-06-22T00:01:00Z");
        invalid.run.workflow_id = "different-workflow".to_string();

        assert_eq!(
            store.write(&invalid),
            Err(WorkflowError::RunWorkflowMismatch {
                run_workflow_id: "different-workflow".to_string(),
                run_workflow_version: 1,
                workflow_id: "wf_runtime".to_string(),
                workflow_version: 1,
            })
        );
        assert!(!store.runtime_path("run-mismatch").unwrap().exists());
    }

    #[test]
    fn runtime_store_rejects_invalid_workflow_before_writing() {
        let vault = tempfile::tempdir().unwrap();
        let store = WorkflowRuntimeStore::new(vault.path());
        let mut invalid = bundle("run-invalid", "2026-06-22T00:01:00Z");
        invalid.workflow.steps[0].goal_alignment.success_clauses = vec![2];

        assert_eq!(
            store.write(&invalid),
            Err(WorkflowError::GoalClauseOutOfRange {
                step_id: "S001".to_string(),
                clause: 2,
            })
        );
        assert!(!store.runtime_path("run-invalid").unwrap().exists());
    }

    #[test]
    fn runtime_store_reports_corrupt_json_without_changing_it() {
        let vault = tempfile::tempdir().unwrap();
        let store = WorkflowRuntimeStore::new(vault.path());
        let path = store.runtime_path("run-corrupt").unwrap();
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let corrupt = b"{not-json";
        fs::write(&path, corrupt).unwrap();

        assert!(matches!(
            store.read("run-corrupt"),
            Err(WorkflowError::RuntimeStateParse(_))
        ));
        assert_eq!(fs::read(path).unwrap(), corrupt);
    }

    #[test]
    fn runtime_store_reports_missing_run() {
        let vault = tempfile::tempdir().unwrap();
        let store = WorkflowRuntimeStore::new(vault.path());

        assert_eq!(
            store.read("missing-run"),
            Err(WorkflowError::RuntimeStateMissing(
                "missing-run".to_string()
            ))
        );
    }

    #[test]
    fn runtime_store_replaces_existing_bundle() {
        let vault = tempfile::tempdir().unwrap();
        let store = WorkflowRuntimeStore::new(vault.path());
        store
            .write(&bundle("run-replace", "2026-06-22T00:01:00Z"))
            .unwrap();
        let replacement = bundle("run-replace", "2026-06-22T00:02:00Z");

        store.write(&replacement).unwrap();

        assert_eq!(store.read("run-replace").unwrap(), replacement);
    }

    #[test]
    fn runtime_store_keeps_existing_bundle_when_replacement_persist_fails() {
        let vault = tempfile::tempdir().unwrap();
        let store = WorkflowRuntimeStore::new(vault.path());
        let original = bundle("run-replace-fail", "2026-06-22T00:01:00Z");
        store.write(&original).unwrap();
        let replacement = bundle("run-replace-fail", "2026-06-22T00:02:00Z");

        let error = store
            .write_with_persist(&replacement, |_temp, path| {
                Err(format!(
                    "forced persist failure for {}",
                    path.display()
                ))
            })
            .unwrap_err();

        assert!(matches!(
            error,
            WorkflowError::RuntimeStateIo(message)
                if message.contains("forced persist failure")
        ));
        assert_eq!(store.read("run-replace-fail").unwrap(), original);
    }
}
