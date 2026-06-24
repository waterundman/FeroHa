use crate::ai::subagent::{DataSource, SearchType, SubagentJob};
use crate::harness::workflow::{StepDispatch, WorkflowStepKind};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlledSubagentJob {
    pub dispatch: StepDispatch,
    pub retrieval_job: Option<SubagentJob>,
}

impl ControlledSubagentJob {
    pub fn from_dispatch(dispatch: StepDispatch) -> Self {
        let retrieval_job = if dispatch.capability == WorkflowStepKind::Research {
            let mut keywords = dispatch
                .inputs
                .get("keywords")
                .and_then(|value| value.as_array())
                .map(|values| {
                    values
                        .iter()
                        .filter_map(|value| value.as_str().map(str::to_string))
                        .filter(|value| !value.trim().is_empty())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            if keywords.is_empty() {
                keywords.push(dispatch.artifact_contract.expected_output.clone());
            }

            Some(
                SubagentJob {
                    search_type: SearchType::All,
                    keywords,
                    data_sources: vec![
                        DataSource::LocalVector,
                        DataSource::WebSearch,
                        DataSource::Arxiv,
                        DataSource::SemanticScholar,
                    ],
                    max_results_per_source: 10,
                    max_hops: 3,
                    current_hop: 0,
                }
                .filtered_by_policy(&dispatch.sandbox_policy),
            )
        } else {
            None
        };

        Self {
            dispatch,
            retrieval_job,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::sandbox::SandboxPolicy;
    use crate::harness::workflow::{
        ArtifactContract, StepDispatch, WorkflowStepKind, WorkflowStepMode,
    };

    fn dispatch(kind: WorkflowStepKind) -> StepDispatch {
        StepDispatch {
            workflow_id: "wf_runtime".to_string(),
            run_id: "run_runtime".to_string(),
            step_id: "S001".to_string(),
            agent_type: "research_subagent".to_string(),
            capability: kind,
            mode: WorkflowStepMode::ReadOnly,
            attempt: 1,
            inputs: serde_json::json!({
                "keywords": ["bayesian memory", "knowledge frontier"]
            }),
            artifact_contract: ArtifactContract {
                expected_output: "Return an evidence packet".to_string(),
                acceptance_criteria: vec!["Cite every claim".to_string()],
                success_clauses: vec![1],
            },
            sandbox_policy: SandboxPolicy::read_only_research(),
        }
    }

    #[test]
    fn research_dispatch_becomes_a_sandbox_filtered_controlled_subagent_job() {
        let job = ControlledSubagentJob::from_dispatch(dispatch(WorkflowStepKind::Research));
        let retrieval = job
            .retrieval_job
            .expect("research dispatch should prepare retrieval work");

        assert_eq!(job.dispatch.workflow_id, "wf_runtime");
        assert_eq!(job.dispatch.run_id, "run_runtime");
        assert_eq!(retrieval.keywords, vec!["bayesian memory", "knowledge frontier"]);
        assert!(retrieval.data_sources.contains(&crate::ai::subagent::DataSource::LocalVector));
        assert!(retrieval.data_sources.contains(&crate::ai::subagent::DataSource::Arxiv));
    }

    #[test]
    fn non_research_dispatch_keeps_contract_without_faking_a_retrieval_job() {
        let job = ControlledSubagentJob::from_dispatch(dispatch(WorkflowStepKind::Implement));

        assert!(job.retrieval_job.is_none());
        assert_eq!(job.dispatch.artifact_contract.success_clauses, vec![1]);
    }
}
