use crate::ai::agent_scheduler::{AgentTask, SubTaskStatus};
use crate::ai::llm_router::LlmRouter;
use crate::harness::context::ContextFragment;
use crate::harness::lean_translator::{LeanShapedTranslator, TranslationResult};
use crate::harness::output_hook::{HookTrigger, OutputManager};
use crate::harness::proposition_kernel::{PropositionGraph, PropositionKernel, VerificationResult};
use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use std::collections::{HashMap, HashSet};
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
pub struct EvidenceChainItem {
    pub claim: String,
    pub source_refs: Vec<String>,
    pub confidence: f32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ScientistResult {
    pub graph: Option<PropositionGraph>,
    pub verification: Option<VerificationResult>,
    pub clean_knowledge: CleanKnowledge,
    pub translation: Option<TranslationResult>,
    pub overall_confidence: f32,
    pub claims: Vec<String>,
    pub sources: Vec<String>,
    pub evidence_chain: Vec<EvidenceChainItem>,
    pub kernel_name: String,
}

impl Scientist {
    pub fn extract_knowledge(task: &AgentTask) -> CleanKnowledge {
        let report_claims = match &task.status {
            crate::ai::agent_scheduler::TaskStatus::Done { result, .. } => {
                extract_report_claims(result)
            }
            _ => Vec::new(),
        };
        let claims = if report_claims.is_empty() {
            task.sub_tasks
                .iter()
                .filter(|st| matches!(st.status, SubTaskStatus::Done))
                .map(|st| st.description.clone())
                .collect()
        } else {
            report_claims
        };

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
            let result = Self::build_result(knowledge, None, None, None, 0.0);
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
                let v = PropositionKernel::verify(g);
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

        let result = Self::build_result(
            knowledge,
            graph,
            verification,
            Some(translation),
            overall_confidence,
        );

        if let (Some(om), Some(dir)) = (output_manager, dualtrack_dir) {
            om.trigger(&HookTrigger::OnRefineComplete, &result, &task.intent, dir)
                .await;
        }

        result
    }

    pub(crate) fn build_result(
        knowledge: CleanKnowledge,
        graph: Option<PropositionGraph>,
        verification: Option<VerificationResult>,
        translation: Option<TranslationResult>,
        overall_confidence: f32,
    ) -> ScientistResult {
        let claims = knowledge.claims.clone();
        let sources: Vec<String> = knowledge
            .sources
            .iter()
            .map(|source| source.key.clone())
            .collect();
        let evidence_chain = Self::build_evidence_chain(&knowledge);

        ScientistResult {
            graph,
            verification,
            clean_knowledge: knowledge,
            translation,
            overall_confidence,
            claims,
            sources,
            evidence_chain,
            kernel_name: PropositionKernel::NAME.to_string(),
        }
    }

    fn build_evidence_chain(knowledge: &CleanKnowledge) -> Vec<EvidenceChainItem> {
        let source_refs: Vec<String> = knowledge
            .sources
            .iter()
            .map(|source| source.key.clone())
            .collect();
        let fallback_confidence = Self::fallback_confidence(knowledge);

        knowledge
            .claims
            .iter()
            .map(|claim| EvidenceChainItem {
                claim: claim.clone(),
                source_refs: source_refs.clone(),
                confidence: knowledge
                    .confidence_map
                    .get(claim)
                    .copied()
                    .unwrap_or(fallback_confidence),
            })
            .collect()
    }

    fn fallback_confidence(knowledge: &CleanKnowledge) -> f32 {
        if knowledge.confidence_map.is_empty() {
            return 0.5;
        }

        knowledge
            .confidence_map
            .values()
            .copied()
            .fold(0.0_f32, f32::max)
            .clamp(0.0, 1.0)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ClaimBlock {
    Paragraph,
    Item,
}

fn extract_report_claims(markdown: &str) -> Vec<String> {
    let mut excluded_section = false;
    let mut heading = None::<String>;
    let mut block = None::<(ClaimBlock, String)>;
    let mut claims = Vec::new();
    let mut seen = HashSet::new();

    for event in Parser::new(markdown) {
        match event {
            Event::Start(Tag::Heading { .. }) => heading = Some(String::new()),
            Event::End(TagEnd::Heading(_)) => {
                let title = heading.take().unwrap_or_default();
                let normalized = normalize_claim_text(&title).to_ascii_lowercase();
                excluded_section = matches!(
                    normalized.as_str(),
                    "acceptance check" | "citations" | "excluded sources"
                );
            }
            Event::Start(Tag::Item) if !excluded_section && block.is_none() => {
                block = Some((ClaimBlock::Item, String::new()));
            }
            Event::Start(Tag::Paragraph) if !excluded_section && block.is_none() => {
                block = Some((ClaimBlock::Paragraph, String::new()));
            }
            Event::End(TagEnd::Paragraph) => {
                if matches!(block, Some((ClaimBlock::Paragraph, _))) {
                    push_claim(&mut claims, &mut seen, block.take());
                }
            }
            Event::End(TagEnd::Item) => {
                if matches!(block, Some((ClaimBlock::Item, _))) {
                    push_claim(&mut claims, &mut seen, block.take());
                }
            }
            Event::Text(text) | Event::Code(text) => {
                if let Some(heading) = heading.as_mut() {
                    append_text(heading, &text);
                } else if let Some((_, content)) = block.as_mut() {
                    append_text(content, &text);
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if let Some(heading) = heading.as_mut() {
                    append_text(heading, " ");
                } else if let Some((_, content)) = block.as_mut() {
                    append_text(content, " ");
                }
            }
            _ => {}
        }
    }

    push_claim(&mut claims, &mut seen, block);
    claims
}

fn append_text(target: &mut String, text: &str) {
    if target
        .chars()
        .last()
        .map(|character| !character.is_whitespace())
        .unwrap_or(false)
    {
        target.push(' ');
    }
    target.push_str(text);
}

fn push_claim(
    claims: &mut Vec<String>,
    seen: &mut HashSet<String>,
    block: Option<(ClaimBlock, String)>,
) {
    let Some((_, content)) = block else {
        return;
    };
    let claim = normalize_claim_text(&content);
    if claim.is_empty() || claim.eq_ignore_ascii_case("n/a") || !seen.insert(claim.clone()) {
        return;
    }
    claims.push(claim);
}

fn normalize_claim_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::agent_scheduler::{
        AgentTask, SynthesizePhase, TaskPriority, TaskStatus, TaskType,
    };
    use crate::ai::task_intent::TaskIntentType;
    use crate::harness::context::{ContextFragment, ContextLayer, ContextSource};
    use serde_json::json;

    fn completed_task(result: &str) -> AgentTask {
        AgentTask {
            id: "scientist-report".to_string(),
            command: crate::cli::parser::CliCommand::Custom("research".to_string()),
            task_type: TaskType::DeepDive,
            task_intent: Some(TaskIntentType::Research),
            sandbox_policy: None,
            priority: TaskPriority::Low,
            priority_score: 0,
            status: TaskStatus::Done {
                completed_at: 100,
                result: result.to_string(),
            },
            anchor_note: None,
            created_at: 1,
            max_retries: 0,
            retry_count: 0,
            synthesize_phase: SynthesizePhase::Idle,
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

    #[test]
    fn scientist_result_exposes_claims_sources_evidence_chain_and_kernel_name() {
        let source = ContextFragment {
            id: "frag_1".to_string(),
            key: "Bayes.md#claim".to_string(),
            value: json!({"text": "Bayesian evidence"}),
            source: ContextSource::RAG,
            layer: ContextLayer::Note,
            created_at: 1,
            ttl: None,
            hash: ContextFragment::compute_hash(
                "Bayes.md#claim",
                &json!({"text": "Bayesian evidence"}),
            ),
        };
        let mut confidence_map = HashMap::new();
        confidence_map.insert("Claim A".to_string(), 0.82);
        let knowledge = CleanKnowledge {
            claims: vec!["Claim A".to_string()],
            sources: vec![source],
            confidence_map,
        };

        let result = Scientist::build_result(knowledge, None, None, None, 0.5);

        assert_eq!(result.kernel_name, "PropositionKernel");
        assert_eq!(result.claims, vec!["Claim A"]);
        assert_eq!(result.sources, vec!["Bayes.md#claim"]);
        assert_eq!(result.evidence_chain.len(), 1);
        assert_eq!(result.evidence_chain[0].claim, "Claim A");
        assert_eq!(result.evidence_chain[0].source_refs, vec!["Bayes.md#claim"]);
        assert!((result.evidence_chain[0].confidence - 0.82).abs() < 0.001);
    }

    #[test]
    fn scientist_uses_best_source_confidence_when_claim_has_no_exact_evidence_key() {
        let source = ContextFragment {
            id: "frag_1".to_string(),
            key: "Bayes.md#retrieval".to_string(),
            value: json!({"text": "Retrieved support"}),
            source: ContextSource::RAG,
            layer: ContextLayer::Note,
            created_at: 1,
            ttl: None,
            hash: ContextFragment::compute_hash(
                "Bayes.md#retrieval",
                &json!({"text": "Retrieved support"}),
            ),
        };
        let mut confidence_map = HashMap::new();
        confidence_map.insert("Bayes.md".to_string(), 0.91);
        let knowledge = CleanKnowledge {
            claims: vec!["Execute: audit research".to_string()],
            sources: vec![source],
            confidence_map,
        };

        let result = Scientist::build_result(knowledge, None, None, None, 0.5);

        assert!((result.evidence_chain[0].confidence - 0.91).abs() < 0.001);
    }
}
