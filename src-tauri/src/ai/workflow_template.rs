use crate::ai::sandbox::SandboxPolicy;
use crate::harness::workflow::{
    AgentRegistry, AgentRegistryEntry, ControlPolicy, GoalAlignment, GoalContract, RetryPolicy,
    WorkflowIr, WorkflowStatus, WorkflowStep, WorkflowStepKind, WorkflowStepMode,
    WorkflowStepStatus,
};
use serde_json::json;
use std::collections::HashSet;

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

impl WorkflowTemplate {
    pub fn build(
        goal_text: &str,
        acceptance_criteria: Vec<String>,
        now: u64,
    ) -> Result<Self, WorkflowTemplateError> {
        let goal_text = goal_text.trim();
        if goal_text.is_empty() {
            return Err(WorkflowTemplateError::GoalRequired);
        }

        let mut seen = HashSet::new();
        let acceptance_criteria = acceptance_criteria
            .into_iter()
            .map(|criterion| criterion.trim().to_string())
            .filter(|criterion| !criterion.is_empty())
            .filter(|criterion| seen.insert(criterion.clone()))
            .collect::<Vec<_>>();
        if acceptance_criteria.is_empty() {
            return Err(WorkflowTemplateError::AcceptanceCriteriaRequired);
        }

        let goal_id = format!("goal_{now}");
        let workflow_id = format!("wf_{now}");
        let run_id = format!("run_{now}");
        let created_at = now.to_string();
        let success_clauses = (1..=acceptance_criteria.len()).collect::<Vec<_>>();
        let research_policy = SandboxPolicy::read_only_research();
        let registry = AgentRegistry::from_agents(vec![AgentRegistryEntry {
            agent_type: "research_subagent".to_string(),
            allowed_tools: research_policy.tool_allowlist,
            denied_tools: vec!["Write".to_string(), "Edit".to_string()],
            default_mode: WorkflowStepMode::ReadOnly,
            max_parallelism: 1,
            can_delegate: false,
        }]);
        let goal = GoalContract {
            goal_id: goal_id.clone(),
            goal_text: goal_text.to_string(),
            success_definition: acceptance_criteria.clone(),
            non_goals: Vec::new(),
            constraints: json!({
                "ai_output_surface": "dream_only",
                "human_notes_mutable_by_ai": false,
            }),
            context_scope: vec!["**/*.md".to_string()],
            approval_policy: json!({"mode": "automatic_contract_verification"}),
            budget: json!({"max_iterations": 30}),
            created_at: created_at.clone(),
        };
        let workflow = WorkflowIr {
            workflow_id,
            goal_id,
            version: 1,
            parent_version: None,
            status: WorkflowStatus::Running,
            global_context: json!({"template": "deterministic_research_v1"}),
            control_policy: ControlPolicy {
                max_parallel_steps: 1,
                replan_on_verification_fail: true,
                max_patch_chain: 1,
            },
            steps: vec![WorkflowStep {
                step_id: "S001".to_string(),
                title: "Research goal".to_string(),
                kind: WorkflowStepKind::Research,
                agent_type: "research_subagent".to_string(),
                mode: WorkflowStepMode::ReadOnly,
                task: "Produce an evidence-backed Markdown research report".to_string(),
                inputs: json!({
                    "question": goal_text,
                    "max_iterations": 30,
                }),
                dependencies: Vec::new(),
                acceptance_criteria,
                goal_alignment: GoalAlignment {
                    success_clauses,
                    why_necessary: "The research report is the evidence source for the goal"
                        .to_string(),
                },
                retry_policy: RetryPolicy {
                    max_attempts: 1,
                    backoff_ms: 0,
                },
                status: WorkflowStepStatus::Ready,
            }],
            created_by: "orchestrator@deterministic-v1".to_string(),
            created_at,
        };

        Ok(Self {
            goal,
            workflow,
            registry,
            run_id,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::workflow::{
        WorkflowStepKind, WorkflowStepMode, WorkflowStepStatus,
    };

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
        assert_eq!(
            step.acceptance_criteria,
            vec!["Every conclusion cites evidence"]
        );
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
}
