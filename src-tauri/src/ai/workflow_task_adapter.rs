use super::agent_scheduler::{AgentTask, SynthesizePhase, TaskPriority, TaskStatus, TaskType};
use super::task_intent::TaskIntentType;
use crate::cli::parser::CliCommand;
use crate::harness::context::{ContextFragment, ContextLayer, ContextSource};
use crate::harness::workflow::{
    safe_runtime_component, StepDispatch, WorkflowError, WorkflowStepKind,
};
use crate::harness::workflow_runtime::WorkflowTaskContext;

const WORKFLOW_DISPATCH_CONTEXT_KEY: &str = "workflow.dispatch";
const DEFAULT_MAX_ITERATIONS: usize = 30;

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
    pub fn task_id(dispatch: &StepDispatch) -> Result<String, WorkflowError> {
        let run_id = safe_runtime_component(&dispatch.run_id)?;
        let step_id = safe_runtime_component(&dispatch.step_id)?;
        Ok(format!(
            "workflow__{}__{}__attempt_{}",
            run_id, step_id, dispatch.attempt
        ))
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

        Ok(AdaptedWorkflowTask::Task(research_task(
            dispatch, created_at,
        )?))
    }
}

fn research_task(dispatch: &StepDispatch, created_at: u64) -> Result<AgentTask, WorkflowError> {
    let task_id = WorkflowTaskAdapter::task_id(dispatch)?;
    let question = research_question(dispatch);
    let command_max_iterations = dispatch_max_iterations(dispatch);
    let max_iterations = command_max_iterations.unwrap_or(DEFAULT_MAX_ITERATIONS);
    let context = WorkflowTaskContext {
        workflow_id: dispatch.workflow_id.clone(),
        run_id: dispatch.run_id.clone(),
        step_id: dispatch.step_id.clone(),
        attempt: dispatch.attempt,
        acceptance_criteria: dispatch.artifact_contract.acceptance_criteria.clone(),
    };
    let context_fragment = workflow_context_fragment(&task_id, &context, created_at)?;

    Ok(AgentTask {
        id: task_id,
        command: CliCommand::DeepResearch {
            question: question.clone(),
            max_iterations: command_max_iterations,
        },
        task_type: TaskType::DeepDive,
        task_intent: Some(TaskIntentType::Research),
        sandbox_policy: Some(dispatch.sandbox_policy.clone()),
        priority: TaskPriority::Low,
        priority_score: 0,
        status: TaskStatus::Pending,
        anchor_note: None,
        created_at,
        max_retries: 0,
        retry_count: 0,
        synthesize_phase: SynthesizePhase::Idle,
        subagent_results: Vec::new(),
        graph_manifest: None,
        has_trace: false,
        source_block_id: None,
        card_id: None,
        card_type: None,
        prompt: Some(question.clone()),
        params: None,
        context_note: None,
        intent: format!("workflow research {}", dispatch.step_id),
        content: question,
        max_iterations,
        sub_tasks: Vec::new(),
        material_packet: None,
        context_fragments: vec![context_fragment],
        regression_metrics: None,
        retry_delay_ms: 0,
        retry_backoff_multiplier: 1.0,
        last_retry_at: None,
        consecutive_failures: 0,
    })
}

pub fn workflow_task_context(task: &AgentTask) -> Option<WorkflowTaskContext> {
    task.context_fragments
        .iter()
        .find(|fragment| fragment.key == WORKFLOW_DISPATCH_CONTEXT_KEY)
        .and_then(|fragment| serde_json::from_value(fragment.value.clone()).ok())
}

fn workflow_context_fragment(
    task_id: &str,
    context: &WorkflowTaskContext,
    created_at: u64,
) -> Result<ContextFragment, WorkflowError> {
    let value = serde_json::to_value(context)
        .map_err(|err| WorkflowError::RuntimeStateParse(err.to_string()))?;
    Ok(ContextFragment {
        id: format!("{}__workflow_dispatch", task_id),
        key: WORKFLOW_DISPATCH_CONTEXT_KEY.to_string(),
        hash: ContextFragment::compute_hash(WORKFLOW_DISPATCH_CONTEXT_KEY, &value),
        value,
        source: ContextSource::Pipeline,
        layer: ContextLayer::Project,
        created_at,
        ttl: None,
    })
}

fn research_question(dispatch: &StepDispatch) -> String {
    let question = dispatch
        .inputs
        .get("question")
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| research_keywords(dispatch))
        .unwrap_or_else(|| dispatch.artifact_contract.expected_output.clone());
    let checklist = dispatch
        .artifact_contract
        .acceptance_criteria
        .iter()
        .map(|criterion| criterion.trim())
        .filter(|criterion| !criterion.is_empty())
        .map(|criterion| format!("- [ ] {criterion}"))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        "{question}\n\n\
         Return a Markdown research report. Include an exact `## Acceptance Check` \
         section using the checklist below. Mark an item `[x]` only when the report's \
         cited evidence supports it; otherwise leave it `[ ]`.\n\n\
         ## Acceptance Check\n{checklist}"
    )
}

fn research_keywords(dispatch: &StepDispatch) -> Option<String> {
    let keywords = dispatch
        .inputs
        .get("keywords")
        .and_then(|value| value.as_array())?
        .iter()
        .filter_map(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();

    if keywords.is_empty() {
        None
    } else {
        Some(keywords.join(", "))
    }
}

fn dispatch_max_iterations(dispatch: &StepDispatch) -> Option<usize> {
    dispatch
        .inputs
        .get("max_iterations")
        .or_else(|| dispatch.inputs.get("maxIterations"))
        .and_then(|value| {
            value
                .as_u64()
                .and_then(|number| usize::try_from(number).ok())
                .or_else(|| {
                    value
                        .as_str()
                        .and_then(|text| text.trim().parse::<usize>().ok())
                })
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::sandbox::SandboxPolicy;
    use crate::cli::parser::CliCommand;
    use crate::harness::workflow::{
        ArtifactContract, StepDispatch, WorkflowStepKind, WorkflowStepMode,
    };
    use serde_json::json;

    fn research_dispatch(run_id: &str, step_id: &str) -> StepDispatch {
        let mut dispatch = dispatch_with_kind(WorkflowStepKind::Research);
        dispatch.run_id = run_id.to_string();
        dispatch.step_id = step_id.to_string();
        dispatch
    }

    fn dispatch_with_kind(capability: WorkflowStepKind) -> StepDispatch {
        StepDispatch {
            workflow_id: "wf_demo".to_string(),
            run_id: "run_demo".to_string(),
            step_id: "S001".to_string(),
            agent_type: "research_subagent".to_string(),
            capability,
            mode: WorkflowStepMode::ReadOnly,
            attempt: 1,
            inputs: json!({"question": "What evidence supports the claim?"}),
            artifact_contract: ArtifactContract {
                expected_output: "Return a cited evidence packet".to_string(),
                acceptance_criteria: vec!["Every claim has a source".to_string()],
                success_clauses: vec![1],
            },
            sandbox_policy: SandboxPolicy::read_only_research(),
        }
    }

    #[test]
    fn research_dispatch_becomes_path_safe_deep_research_task() {
        let dispatch = research_dispatch("run_demo", "S001");
        let adapted = WorkflowTaskAdapter::adapt(&dispatch, 100).unwrap();
        let AdaptedWorkflowTask::Task(task) = adapted else {
            panic!("expected task")
        };

        assert_eq!(task.id, "workflow__run_demo__S001__attempt_1");
        assert!(matches!(
            task.command,
            CliCommand::DeepResearch {
                ref question,
                max_iterations: None
            } if question.starts_with("What evidence supports the claim?")
                && question.contains("## Acceptance Check")
        ));
        assert!(matches!(task.task_type, TaskType::DeepDive));
        assert_eq!(task.task_intent, Some(TaskIntentType::Research));
        assert_eq!(task.sandbox_policy, Some(dispatch.sandbox_policy.clone()));
        assert!(matches!(task.priority, TaskPriority::Low));
        assert_eq!(task.priority_score, 0);
        assert!(matches!(task.status, TaskStatus::Pending));
        assert_eq!(task.created_at, 100);
        assert_eq!(
            task.prompt.as_deref().map(|prompt| prompt.starts_with(
                "What evidence supports the claim?"
            )),
            Some(true)
        );
        assert!(task.content.starts_with("What evidence supports the claim?"));
        assert!(task
            .content
            .contains("- [ ] Every claim has a source"));
        assert_eq!(task.max_iterations, DEFAULT_MAX_ITERATIONS);

        let context = workflow_task_context(&task).unwrap();
        assert_eq!(context.workflow_id, "wf_demo");
        assert_eq!(context.run_id, "run_demo");
        assert_eq!(context.step_id, "S001");
        assert_eq!(context.attempt, 1);
        assert_eq!(
            context.acceptance_criteria,
            vec!["Every claim has a source".to_string()]
        );

        let fragment = task
            .context_fragments
            .iter()
            .find(|fragment| fragment.key == WORKFLOW_DISPATCH_CONTEXT_KEY)
            .unwrap();
        assert_eq!(fragment.source, ContextSource::Pipeline);
        assert_eq!(fragment.layer, ContextLayer::Project);
        assert_eq!(fragment.created_at, 100);
        assert_eq!(fragment.ttl, None);
        assert_eq!(fragment.value, serde_json::to_value(&context).unwrap());
        assert_eq!(
            fragment.hash,
            ContextFragment::compute_hash(WORKFLOW_DISPATCH_CONTEXT_KEY, &fragment.value)
        );
    }

    #[test]
    fn implement_dispatch_is_explicitly_unsupported() {
        let dispatch = dispatch_with_kind(WorkflowStepKind::Implement);

        let AdaptedWorkflowTask::Unsupported {
            capability,
            reason_code,
            summary,
        } = WorkflowTaskAdapter::adapt(&dispatch, 100).unwrap()
        else {
            panic!("expected unsupported")
        };

        assert_eq!(capability, WorkflowStepKind::Implement);
        assert_eq!(reason_code, "unsupported_workflow_capability");
        assert_eq!(summary, "No narrow-loop executor for Implement".to_string());
    }

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

    #[test]
    fn research_dispatch_rejects_unsafe_task_id_components() {
        let unsafe_step = research_dispatch("run_demo", "../escape");
        assert!(matches!(
            WorkflowTaskAdapter::adapt(&unsafe_step, 100),
            Err(WorkflowError::UnsafeRuntimeComponent(value)) if value == "../escape"
        ));

        let unsafe_run = research_dispatch("run.demo", "S001");
        assert!(matches!(
            WorkflowTaskAdapter::adapt(&unsafe_run, 100),
            Err(WorkflowError::UnsafeRuntimeComponent(value)) if value == "run.demo"
        ));
    }
}
