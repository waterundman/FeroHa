use crate::ai::agent_scheduler::{AgentTask, SubTaskStatus};
use crate::ai::llm_router::LlmRouter;
use crate::harness::context::ContextFragment;
use crate::harness::lean_kernel::{HybridLeanKernel, PropositionGraph, VerificationResult};
use crate::harness::lean_translator::{LeanShapedTranslator, TranslationResult};
use crate::harness::output_hook::{HookTrigger, OutputManager};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CleanKnowledge {
    pub claims: Vec<String>,
    pub sources: Vec<ContextFragment>,
    pub confidence_map: HashMap<String, f32>,
}

pub struct Scientist;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ScientistResult {
    pub graph: Option<PropositionGraph>,
    pub verification: Option<VerificationResult>,
    pub clean_knowledge: CleanKnowledge,
    pub translation: Option<TranslationResult>,
    pub overall_confidence: f32,
}

impl Scientist {
    pub fn extract_knowledge(task: &AgentTask) -> CleanKnowledge {
        let claims: Vec<String> = task
            .sub_tasks
            .iter()
            .filter(|st| matches!(st.status, SubTaskStatus::Done))
            .map(|st| st.description.clone())
            .collect();

        let sources = task.context_fragments.clone();

        let mut confidence_map: HashMap<String, f32> = HashMap::new();
        for result in &task.subagent_results {
            for entry in &result.entries {
                let key = entry.title.clone();
                let existing = confidence_map.entry(key).or_insert(0.0);
                *existing = (*existing).max(entry.relevance_score);
            }
        }

        CleanKnowledge {
            claims,
            sources,
            confidence_map,
        }
    }

    pub async fn refine(
        task: &AgentTask,
        router: &LlmRouter,
        output_manager: Option<&OutputManager>,
        dualtrack_dir: Option<&PathBuf>,
    ) -> ScientistResult {
        let knowledge = Self::extract_knowledge(task);

        if knowledge.claims.is_empty() {
            let result = ScientistResult {
                graph: None,
                verification: None,
                clean_knowledge: knowledge,
                translation: None,
                overall_confidence: 0.0,
            };
            if let (Some(om), Some(dir)) = (output_manager, dualtrack_dir) {
                om.trigger(&HookTrigger::OnRefineComplete, &result, &task.intent, dir)
                    .await;
            }
            return result;
        }

        let translator =
            LeanShapedTranslator::new(Arc::new(tokio::sync::Mutex::new(router.clone())), None);
        let translation = translator.translate(&knowledge, &task.id).await;

        let (graph, verification) =
            if let (true, Some(ref g)) = (translation.success, &translation.graph) {
                let v = HybridLeanKernel::verify(g);
                (Some(g.clone()), Some(v))
            } else {
                (None, None)
            };

        let overall_confidence = if let Some(ref v) = verification {
            let total = knowledge.claims.len().max(1) as f32;
            let violations = v.violations.len() as f32;
            1.0 - (violations / total).min(1.0)
        } else {
            0.5
        };

        let result = ScientistResult {
            graph,
            verification,
            clean_knowledge: knowledge,
            translation: Some(translation),
            overall_confidence,
        };

        if let (Some(om), Some(dir)) = (output_manager, dualtrack_dir) {
            om.trigger(&HookTrigger::OnRefineComplete, &result, &task.intent, dir)
                .await;
        }

        result
    }
}
