use crate::ai::subagent::{ResearchPlan, StageType, SubagentEntry};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ResearchConfidence {
    Extracted,
    Inferred,
    Ambiguous,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResearchNodeKind {
    Question,
    Stage,
    Source,
    Hypothesis,
    Synthesis,
    Report,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResearchGraphNode {
    pub id: String,
    pub label: String,
    pub kind: ResearchNodeKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_location: Option<String>,
    #[serde(default)]
    pub attributes: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResearchGraphEdge {
    pub source: String,
    pub target: String,
    pub relation: String,
    pub confidence: ResearchConfidence,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ResearchGraphSummary {
    pub stage_count: usize,
    pub source_count: usize,
    pub hypothesis_count: usize,
    pub synthesis_count: usize,
    pub extracted_edges: usize,
    pub inferred_edges: usize,
    pub ambiguous_edges: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResearchGraph {
    pub schema_version: String,
    pub task_id: String,
    pub question: String,
    pub nodes: Vec<ResearchGraphNode>,
    pub edges: Vec<ResearchGraphEdge>,
    pub summary: ResearchGraphSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResearchGraphArtifacts {
    pub graph_json: String,
    pub graph_report: String,
}

pub fn build_deep_research_graph(
    task_id: &str,
    question: &str,
    plan: &ResearchPlan,
    sources: &[SubagentEntry],
    hypotheses: &[String],
    syntheses: &[String],
) -> ResearchGraph {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    nodes.push(ResearchGraphNode {
        id: "question".to_string(),
        label: question.to_string(),
        kind: ResearchNodeKind::Question,
        source_file: None,
        source_location: None,
        attributes: BTreeMap::new(),
    });

    let mut retrieve_stage_ids = Vec::new();
    for (idx, stage) in plan.stages.iter().enumerate() {
        let stage_id = format!("stage_{}_{}", idx, stage_type_name(&stage.stage_type));
        if matches!(stage.stage_type, StageType::Retrieve) {
            retrieve_stage_ids.push(stage_id.clone());
        }

        let mut attributes = BTreeMap::new();
        attributes.insert("goal".to_string(), stage.goal.clone());
        attributes.insert("status".to_string(), format!("{:?}", stage.status));
        if let Some(result) = &stage.result {
            attributes.insert("result".to_string(), truncate(result, 600));
        }

        nodes.push(ResearchGraphNode {
            id: stage_id.clone(),
            label: format!("{}: {}", stage_type_name(&stage.stage_type), stage.goal),
            kind: ResearchNodeKind::Stage,
            source_file: None,
            source_location: None,
            attributes,
        });
        edges.push(ResearchGraphEdge {
            source: "question".to_string(),
            target: stage_id,
            relation: "has_stage".to_string(),
            confidence: ResearchConfidence::Extracted,
            weight: None,
        });
    }

    let retrieval_anchor = retrieve_stage_ids
        .first()
        .cloned()
        .unwrap_or_else(|| "question".to_string());

    for (idx, entry) in sources.iter().enumerate() {
        let source_id = format!("source_{}_{}", idx, stable_suffix(&entry.title));
        let mut attributes = BTreeMap::new();
        attributes.insert("source".to_string(), entry.source.clone());
        attributes.insert("snippet".to_string(), truncate(&entry.snippet, 800));
        attributes.insert(
            "relevance_score".to_string(),
            format!("{:.3}", entry.relevance_score),
        );
        if !entry.authors.is_empty() {
            attributes.insert("authors".to_string(), entry.authors.join(", "));
        }
        if let Some(year) = entry.year {
            attributes.insert("year".to_string(), year.to_string());
        }
        if let Some(url) = &entry.url {
            attributes.insert("url".to_string(), url.clone());
        }

        nodes.push(ResearchGraphNode {
            id: source_id.clone(),
            label: entry.title.clone(),
            kind: ResearchNodeKind::Source,
            source_file: Some(entry.source.clone()),
            source_location: entry.url.clone(),
            attributes,
        });
        edges.push(ResearchGraphEdge {
            source: retrieval_anchor.clone(),
            target: source_id.clone(),
            relation: "retrieved_source".to_string(),
            confidence: ResearchConfidence::Extracted,
            weight: Some(entry.relevance_score.clamp(0.0, 1.0)),
        });
        edges.push(ResearchGraphEdge {
            source: source_id,
            target: "question".to_string(),
            relation: "provides_context".to_string(),
            confidence: ResearchConfidence::Extracted,
            weight: Some(entry.relevance_score.clamp(0.0, 1.0)),
        });
    }

    for (idx, synthesis) in syntheses.iter().enumerate() {
        let synthesis_id = format!("synthesis_{}", idx);
        let mut attributes = BTreeMap::new();
        attributes.insert("content".to_string(), truncate(synthesis, 1200));
        nodes.push(ResearchGraphNode {
            id: synthesis_id.clone(),
            label: format!("Synthesis {}", idx + 1),
            kind: ResearchNodeKind::Synthesis,
            source_file: None,
            source_location: None,
            attributes,
        });
        edges.push(ResearchGraphEdge {
            source: synthesis_id,
            target: "question".to_string(),
            relation: "summarizes".to_string(),
            confidence: ResearchConfidence::Inferred,
            weight: None,
        });
    }

    let top_source_ids: Vec<String> = sources
        .iter()
        .enumerate()
        .take(5)
        .map(|(idx, entry)| format!("source_{}_{}", idx, stable_suffix(&entry.title)))
        .collect();

    for (idx, hypothesis) in hypotheses.iter().enumerate() {
        let hypothesis_id = format!("hypothesis_{}", idx);
        let mut attributes = BTreeMap::new();
        attributes.insert("statement".to_string(), truncate(hypothesis, 1200));
        nodes.push(ResearchGraphNode {
            id: hypothesis_id.clone(),
            label: format!("Hypothesis {}", idx + 1),
            kind: ResearchNodeKind::Hypothesis,
            source_file: None,
            source_location: None,
            attributes,
        });
        edges.push(ResearchGraphEdge {
            source: "question".to_string(),
            target: hypothesis_id.clone(),
            relation: "proposes".to_string(),
            confidence: ResearchConfidence::Inferred,
            weight: None,
        });
        for source_id in &top_source_ids {
            edges.push(ResearchGraphEdge {
                source: source_id.clone(),
                target: hypothesis_id.clone(),
                relation: "supports_hypothesis".to_string(),
                confidence: ResearchConfidence::Inferred,
                weight: None,
            });
        }
    }

    nodes.push(ResearchGraphNode {
        id: "report".to_string(),
        label: "Deep research report".to_string(),
        kind: ResearchNodeKind::Report,
        source_file: None,
        source_location: None,
        attributes: BTreeMap::new(),
    });
    edges.push(ResearchGraphEdge {
        source: "question".to_string(),
        target: "report".to_string(),
        relation: "produces".to_string(),
        confidence: ResearchConfidence::Extracted,
        weight: None,
    });

    let summary = summarize_graph(&nodes, &edges);

    ResearchGraph {
        schema_version: "feroha.research_graph.v1".to_string(),
        task_id: task_id.to_string(),
        question: question.to_string(),
        nodes,
        edges,
        summary,
    }
}

pub fn write_research_graph_artifacts(
    dualtrack_dir: &Path,
    task_id: &str,
    graph: &ResearchGraph,
) -> io::Result<ResearchGraphArtifacts> {
    let relative_dir = format!("research/graphs/{}", task_id);
    let dir = dualtrack_dir.join(&relative_dir);
    fs::create_dir_all(&dir)?;

    let graph_json = format!("{}/graph.json", relative_dir);
    let graph_report = format!("{}/GRAPH_REPORT.md", relative_dir);
    let graph_path = dualtrack_dir.join(&graph_json);
    let report_path = dualtrack_dir.join(&graph_report);

    let json = serde_json::to_string_pretty(graph).map_err(io::Error::other)?;
    fs::write(graph_path, json)?;
    fs::write(report_path, render_graph_report(graph))?;

    Ok(ResearchGraphArtifacts {
        graph_json,
        graph_report,
    })
}

pub fn render_graph_report(graph: &ResearchGraph) -> String {
    let mut report = String::new();
    report.push_str(&format!("# Research Graph: {}\n\n", graph.question));
    report.push_str("## Summary\n\n");
    report.push_str(&format!("- Stages: {}\n", graph.summary.stage_count));
    report.push_str(&format!("- Sources: {}\n", graph.summary.source_count));
    report.push_str(&format!(
        "- Hypotheses: {}\n",
        graph.summary.hypothesis_count
    ));
    report.push_str(&format!("- Syntheses: {}\n", graph.summary.synthesis_count));
    report.push_str(&format!(
        "- Audit labels: {} EXTRACTED, {} INFERRED, {} AMBIGUOUS\n\n",
        graph.summary.extracted_edges, graph.summary.inferred_edges, graph.summary.ambiguous_edges
    ));

    report.push_str("## Top Sources\n\n");
    let mut source_nodes = graph
        .nodes
        .iter()
        .filter(|node| node.kind == ResearchNodeKind::Source)
        .take(10)
        .peekable();
    if source_nodes.peek().is_none() {
        report.push_str("No source nodes were captured.\n\n");
    } else {
        for node in source_nodes {
            let source = node
                .attributes
                .get("source")
                .map(String::as_str)
                .unwrap_or("unknown");
            report.push_str(&format!("- {} ({})\n", node.label, source));
        }
        report.push('\n');
    }

    report.push_str("## Iteration Backbone\n\n");
    for edge in graph
        .edges
        .iter()
        .filter(|edge| edge.relation == "has_stage")
    {
        if let Some(stage) = graph.nodes.iter().find(|node| node.id == edge.target) {
            let status = stage
                .attributes
                .get("status")
                .map(String::as_str)
                .unwrap_or("unknown");
            report.push_str(&format!("- {} [{}]\n", stage.label, status));
        }
    }

    report
}

fn summarize_graph(
    nodes: &[ResearchGraphNode],
    edges: &[ResearchGraphEdge],
) -> ResearchGraphSummary {
    ResearchGraphSummary {
        stage_count: count_kind(nodes, ResearchNodeKind::Stage),
        source_count: count_kind(nodes, ResearchNodeKind::Source),
        hypothesis_count: count_kind(nodes, ResearchNodeKind::Hypothesis),
        synthesis_count: count_kind(nodes, ResearchNodeKind::Synthesis),
        extracted_edges: edges
            .iter()
            .filter(|edge| edge.confidence == ResearchConfidence::Extracted)
            .count(),
        inferred_edges: edges
            .iter()
            .filter(|edge| edge.confidence == ResearchConfidence::Inferred)
            .count(),
        ambiguous_edges: edges
            .iter()
            .filter(|edge| edge.confidence == ResearchConfidence::Ambiguous)
            .count(),
    }
}

fn count_kind(nodes: &[ResearchGraphNode], kind: ResearchNodeKind) -> usize {
    nodes.iter().filter(|node| node.kind == kind).count()
}

fn stage_type_name(stage_type: &StageType) -> &'static str {
    match stage_type {
        StageType::Retrieve => "retrieve",
        StageType::Synthesize => "synthesize",
        StageType::Hypothesize => "hypothesize",
        StageType::Verify => "verify",
    }
}

fn stable_suffix(value: &str) -> String {
    format!("{:x}", md5::compute(value))
        .chars()
        .take(10)
        .collect()
}

fn truncate(value: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for (idx, ch) in value.chars().enumerate() {
        if idx >= max_chars {
            out.push_str("...");
            return out;
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::subagent::{ResearchStage, StageStatus};

    fn make_plan() -> ResearchPlan {
        ResearchPlan {
            plan_id: "plan_test".to_string(),
            question: "How should memory graphs work?".to_string(),
            stages: vec![
                ResearchStage {
                    stage_type: StageType::Retrieve,
                    goal: "Collect evidence".to_string(),
                    status: StageStatus::Done,
                    result: Some("Retrieved 1 results".to_string()),
                },
                ResearchStage {
                    stage_type: StageType::Verify,
                    goal: "Check gaps".to_string(),
                    status: StageStatus::Done,
                    result: Some("Completeness: 7/10".to_string()),
                },
            ],
            current_stage: 2,
            max_iterations: 3,
            iteration_count: 2,
        }
    }

    #[test]
    fn deep_research_graph_uses_graphify_style_audit_labels() {
        let sources = vec![SubagentEntry {
            title: "Graph memory".to_string(),
            snippet: "Knowledge graphs keep provenance.".to_string(),
            url: Some("https://example.test/paper".to_string()),
            authors: vec!["Ada".to_string()],
            year: Some(2026),
            source: "SemanticScholar".to_string(),
            relevance_score: 0.91,
        }];

        let graph = build_deep_research_graph(
            "task_1",
            "How should memory graphs work?",
            &make_plan(),
            &sources,
            &["Hypothesis: provenance improves review.".to_string()],
            &["Synthesis text".to_string()],
        );

        assert_eq!(graph.summary.stage_count, 2);
        assert_eq!(graph.summary.source_count, 1);
        assert_eq!(graph.summary.hypothesis_count, 1);
        assert!(graph.summary.extracted_edges > 0);
        assert!(graph.summary.inferred_edges > 0);

        let value = serde_json::to_value(&graph.edges[0].confidence).unwrap();
        assert_eq!(value, serde_json::json!("EXTRACTED"));
    }

    #[test]
    fn graph_report_mentions_audit_label_counts() {
        let graph = build_deep_research_graph("task_1", "Question?", &make_plan(), &[], &[], &[]);
        let report = render_graph_report(&graph);
        assert!(report.contains("Audit labels"));
        assert!(report.contains("EXTRACTED"));
    }

    #[test]
    fn graph_artifacts_use_passed_dualtrack_dir_without_nesting_dualtrack() {
        let temp = tempfile::tempdir().unwrap();
        let dualtrack_dir = temp.path().join(".dualtrack");
        let graph =
            build_deep_research_graph("task_graph", "Question?", &make_plan(), &[], &[], &[]);

        let artifacts =
            write_research_graph_artifacts(&dualtrack_dir, "task_graph", &graph).unwrap();

        assert_eq!(
            artifacts.graph_json,
            "research/graphs/task_graph/graph.json"
        );
        assert!(dualtrack_dir
            .join("research/graphs/task_graph/graph.json")
            .exists());
        assert!(!dualtrack_dir
            .join(".dualtrack/research/graphs/task_graph/graph.json")
            .exists());
    }
}
