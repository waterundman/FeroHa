use super::llm_router::LlmRouter;
use super::research_trace;
use super::vectordb::VectorStore;
use crate::graph::manifest::{GraphManifest, GraphManifestBuilder};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentJob {
    pub search_type: SearchType,
    pub keywords: Vec<String>,
    pub data_sources: Vec<DataSource>,
    pub max_results_per_source: usize,
    pub max_hops: u32,
    pub current_hop: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SearchType {
    Semantic,
    Keyword,
    AuthorSearch,
    TitleSearch,
    All,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DataSource {
    LocalVector,
    WebSearch,
    Arxiv,
    SemanticScholar,
}

impl DataSource {
    pub fn required_tool(&self) -> &'static str {
        match self {
            DataSource::LocalVector => "vector_search",
            DataSource::WebSearch => "web_search",
            DataSource::Arxiv => "arxiv_search",
            DataSource::SemanticScholar => "semantic_scholar_search",
        }
    }

    pub fn allowed_by_network_policy(&self, policy: &crate::ai::sandbox::NetworkPolicy) -> bool {
        match policy {
            crate::ai::sandbox::NetworkPolicy::Disabled => matches!(self, DataSource::LocalVector),
            crate::ai::sandbox::NetworkPolicy::AcademicOnly => {
                matches!(
                    self,
                    DataSource::LocalVector | DataSource::Arxiv | DataSource::SemanticScholar
                )
            }
            crate::ai::sandbox::NetworkPolicy::Allowed => true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentResult {
    pub source: DataSource,
    pub entries: Vec<SubagentEntry>,
    pub hop: u32,
    pub generated_keywords: Vec<String>,
    pub total_found: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub graph_manifest: Option<GraphManifest>,
}

impl SubagentJob {
    pub fn filtered_by_policy(&self, policy: &crate::ai::sandbox::SandboxPolicy) -> Self {
        let mut job = self.clone();
        job.data_sources = self
            .data_sources
            .iter()
            .filter(|source| {
                policy.allows_tool(source.required_tool())
                    && source.allowed_by_network_policy(&policy.network_policy)
            })
            .cloned()
            .collect();
        job
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentEntry {
    pub title: String,
    pub snippet: String,
    pub url: Option<String>,
    pub authors: Vec<String>,
    pub year: Option<u32>,
    pub source: String,
    pub relevance_score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StageType {
    Retrieve,
    Synthesize,
    Hypothesize,
    Verify,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StageStatus {
    Pending,
    Running,
    Done,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchStage {
    pub stage_type: StageType,
    pub goal: String,
    pub status: StageStatus,
    pub result: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchPlan {
    pub plan_id: String,
    pub question: String,
    pub stages: Vec<ResearchStage>,
    pub current_stage: usize,
    pub max_iterations: usize,
    pub iteration_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HypothesisStatus {
    Proposed,
    Verified,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hypothesis {
    pub id: String,
    pub statement: String,
    pub confidence: f32,
    pub supporting_chunks: Vec<String>,
    pub status: HypothesisStatus,
    pub verification_result: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StepType {
    LiteratureSearch,
    DataAnalysis,
    Synthesis,
    HypothesisTest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    pub step_type: StepType,
    pub description: String,
    pub estimated_sources: usize,
    pub depends_on: Vec<usize>,
}

#[derive(Clone)]
pub struct Subagent {
    http_client: Client,
    serper_api_key: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SerperResponse {
    organic: Vec<SerperOrganicResult>,
}

#[derive(Debug, Deserialize)]
struct SerperOrganicResult {
    title: String,
    snippet: String,
    link: String,
}

#[derive(Debug, Deserialize)]
struct SemanticScholarResponse {
    data: Vec<S2Paper>,
    #[serde(default)]
    #[allow(dead_code)]
    total: usize,
}

#[derive(Debug, Deserialize)]
struct S2Paper {
    #[serde(rename = "paperId")]
    paper_id: String,
    title: String,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    #[serde(rename = "abstract")]
    paper_abstract: Option<String>,
    #[serde(default)]
    authors: Vec<S2Author>,
    #[serde(default)]
    year: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct S2Author {
    name: String,
}

impl Subagent {
    pub fn new(serper_api_key: Option<String>) -> Self {
        let http_client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("FeroHa/2.5")
            .build()
            .expect("Failed to create HTTP client");
        Self {
            http_client,
            serper_api_key,
        }
    }

    async fn search_web(&self, keywords: &[String], max_results: usize) -> Vec<SubagentEntry> {
        let api_key = match &self.serper_api_key {
            Some(k) => k.clone(),
            None => {
                tracing::warn!("Serper API key not configured, skipping web search");
                return vec![];
            }
        };

        if keywords.is_empty() {
            return vec![];
        }

        let query = keywords.join(" ");
        let body = serde_json::json!({
            "q": query,
            "num": max_results.min(10),
        });

        let response = match self
            .http_client
            .post("https://google.serper.dev/search")
            .header("X-API-KEY", &api_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .timeout(std::time::Duration::from_secs(15))
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("Serper API request failed: {}", e);
                return vec![];
            }
        };

        let serper: SerperResponse = match response.json().await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("Serper API response parse failed: {}", e);
                return vec![];
            }
        };

        serper
            .organic
            .into_iter()
            .map(|r| SubagentEntry {
                title: r.title,
                snippet: r.snippet,
                url: Some(r.link),
                authors: vec![],
                year: None,
                source: "web".to_string(),
                relevance_score: 0.7,
            })
            .collect()
    }

    pub async fn search_semantic_scholar(
        &self,
        keywords: &[String],
        max_results: usize,
    ) -> Vec<SubagentEntry> {
        if keywords.is_empty() {
            return vec![];
        }

        let query = keywords.join(" ");

        let response = match self
            .http_client
            .get("https://api.semanticscholar.org/graph/v1/paper/search")
            .query(&[
                ("query", &query),
                ("limit", &max_results.to_string()),
                ("fields", &"title,url,abstract,authors,year".to_string()),
            ])
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("Semantic Scholar API request failed: {}", e);
                return vec![];
            }
        };

        let s2_response: SemanticScholarResponse = match response.json().await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("Semantic Scholar API response parse failed: {}", e);
                return vec![];
            }
        };

        s2_response
            .data
            .into_iter()
            .enumerate()
            .map(|(i, paper)| SubagentEntry {
                title: paper.title,
                snippet: paper.paper_abstract.unwrap_or_default(),
                url: paper.url,
                authors: paper.authors.into_iter().map(|a| a.name).collect(),
                year: paper.year,
                source: paper.paper_id,
                relevance_score: 1.0 - (i as f32 * 0.05).min(0.95),
            })
            .collect()
    }

    pub async fn search_arxiv(
        &self,
        keywords: &[String],
        max_results: usize,
    ) -> Vec<SubagentEntry> {
        if keywords.is_empty() || max_results == 0 {
            return vec![];
        }

        let encoded: Vec<String> = keywords
            .iter()
            .map(|kw| {
                kw.chars()
                    .map(|c| {
                        if c.is_alphanumeric() || c.is_whitespace() {
                            c
                        } else {
                            ' '
                        }
                    })
                    .collect::<String>()
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join("+")
            })
            .collect();

        if encoded.iter().all(|s| s.is_empty()) {
            return vec![];
        }

        let query = format!("all:{}", encoded.join("+AND+"));
        let url = format!(
            "http://export.arxiv.org/api/query?search_query={}&max_results={}&sortBy=relevance",
            query, max_results
        );

        let response = match self
            .http_client
            .get(&url)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("arXiv API request failed: {}", e);
                return vec![];
            }
        };

        let body = match response.text().await {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!("Failed to read arXiv response body: {}", e);
                return vec![];
            }
        };

        let entries = parse_arxiv_xml(&body);
        let n = entries.len();
        entries
            .into_iter()
            .enumerate()
            .map(|(i, mut entry)| {
                if n > 1 {
                    entry.relevance_score = 1.0 - (i as f32 / (n - 1) as f32) * 0.5;
                } else {
                    entry.relevance_score = 1.0;
                }
                entry
            })
            .collect()
    }

    pub async fn execute(
        &self,
        job: &SubagentJob,
        vector_store: Option<&VectorStore>,
    ) -> Vec<SubagentResult> {
        let mut results = Vec::new();

        for source in &job.data_sources {
            match source {
                DataSource::LocalVector => {
                    if let Some(vs) = vector_store {
                        for keyword in &job.keywords {
                            let search_results =
                                vs.search_text(keyword, job.max_results_per_source);
                            let entries: Vec<SubagentEntry> = search_results
                                .into_iter()
                                .map(|sr| SubagentEntry {
                                    title: if sr.heading_context.is_empty() {
                                        sr.source_file.clone()
                                    } else {
                                        sr.heading_context.clone()
                                    },
                                    snippet: sr.chunk_text,
                                    url: None,
                                    authors: vec![],
                                    year: None,
                                    source: sr.source_file,
                                    relevance_score: sr.score,
                                })
                                .collect();
                            let total_found = entries.len();
                            results.push(SubagentResult {
                                source: DataSource::LocalVector,
                                entries,
                                hop: job.current_hop,
                                generated_keywords: vec![],
                                total_found,
                                graph_manifest: None,
                            });
                        }
                    }
                }
                DataSource::WebSearch => {
                    let entries = self
                        .search_web(&job.keywords, job.max_results_per_source)
                        .await;
                    let total_found = entries.len();
                    results.push(SubagentResult {
                        source: DataSource::WebSearch,
                        entries,
                        hop: job.current_hop,
                        generated_keywords: vec![],
                        total_found,
                        graph_manifest: None,
                    });
                }
                DataSource::Arxiv => {
                    let entries = self
                        .search_arxiv(&job.keywords, job.max_results_per_source)
                        .await;
                    let total_found = entries.len();
                    results.push(SubagentResult {
                        source: DataSource::Arxiv,
                        entries,
                        hop: job.current_hop,
                        generated_keywords: vec![],
                        total_found,
                        graph_manifest: None,
                    });
                }
                DataSource::SemanticScholar => {
                    let entries = self
                        .search_semantic_scholar(&job.keywords, job.max_results_per_source)
                        .await;
                    let total_found = entries.len();
                    results.push(SubagentResult {
                        source: DataSource::SemanticScholar,
                        entries,
                        hop: job.current_hop,
                        generated_keywords: vec![],
                        total_found,
                        graph_manifest: None,
                    });
                }
            }
        }

        results
    }

    pub async fn execute_with_policy(
        &self,
        job: &SubagentJob,
        vector_store: Option<&VectorStore>,
        policy: &crate::ai::sandbox::SandboxPolicy,
    ) -> Vec<SubagentResult> {
        let filtered = job.filtered_by_policy(policy);
        self.execute(&filtered, vector_store).await
    }

    async fn execute_maybe_filtered(
        &self,
        job: &SubagentJob,
        vector_store: Option<&VectorStore>,
        policy: Option<&crate::ai::sandbox::SandboxPolicy>,
    ) -> Vec<SubagentResult> {
        if let Some(policy) = policy {
            self.execute_with_policy(job, vector_store, policy).await
        } else {
            self.execute(job, vector_store).await
        }
    }

    pub async fn multi_hop_execute(
        &self,
        job: &SubagentJob,
        vector_store: Option<&VectorStore>,
    ) -> Vec<SubagentResult> {
        let mut all_results = Vec::new();

        let first_hop = self.execute(job, vector_store).await;

        let all_entries: Vec<SubagentEntry> =
            first_hop.iter().flat_map(|r| r.entries.clone()).collect();

        all_results.extend(first_hop);

        let new_keywords = Self::extract_keywords(&all_entries, 5);

        if !new_keywords.is_empty() && job.current_hop < job.max_hops {
            let new_job = SubagentJob {
                keywords: new_keywords,
                current_hop: job.current_hop + 1,
                ..job.clone()
            };
            let recursive_results = Box::pin(self.multi_hop_execute(&new_job, vector_store)).await;
            all_results.extend(recursive_results);
        }

        all_results
    }

    pub fn extract_keywords(entries: &[SubagentEntry], max_keywords: usize) -> Vec<String> {
        let stop_words: &[&str] = &[
            "a", "an", "the", "is", "are", "of", "in", "to", "for", "and", "or", "on", "at",
            "with", "by", "from", "this", "that", "it", "as", "be", "been", "being", "was", "were",
            "will", "would", "can", "could", "has", "have", "had", "do", "does", "did", "not",
            "but", "if", "so", "we", "they", "he", "she", "no", "all", "about", "also", "which",
            "their", "its", "more", "some", "than", "other", "each", "only", "most", "well", "may",
            "should",
        ];

        let mut word_freq: HashMap<String, usize> = HashMap::new();

        for entry in entries {
            let text = format!("{} {}", entry.title, entry.snippet);
            for word in text.split_whitespace() {
                let cleaned: String = word
                    .to_lowercase()
                    .chars()
                    .filter(|c| c.is_alphanumeric())
                    .collect();
                if cleaned.len() < 3 {
                    continue;
                }
                if stop_words.contains(&cleaned.as_str()) {
                    continue;
                }
                *word_freq.entry(cleaned).or_insert(0) += 1;
            }
        }

        let mut freq_vec: Vec<(String, usize)> = word_freq.into_iter().collect();
        freq_vec.sort_by_key(|b| std::cmp::Reverse(b.1));

        freq_vec
            .into_iter()
            .take(max_keywords)
            .map(|(word, _)| word)
            .collect()
    }

    pub fn build_graph_manifest(
        link_graph: &crate::graph::link_graph::LinkGraph,
        target_paths: &[String],
        max_hops: u32,
        token_budget: usize,
        vault: Option<&crate::fs::vault::VaultManager>,
        vector_store: Option<&crate::ai::vectordb::VectorStore>,
    ) -> GraphManifest {
        let mut builder = GraphManifestBuilder::new(link_graph);
        if let Some(vs) = vector_store {
            builder = builder.with_vector_store(vs);
        }
        if let Some(v) = vault {
            builder = builder.with_vault(v);
        }
        builder.build(target_paths, max_hops, token_budget)
    }

    pub async fn execute_deep_research(
        &self,
        question: &str,
        vector_store: Option<&VectorStore>,
        mut llm_router: Option<LlmRouter>,
        task_id: &str,
        dualtrack_dir: &std::path::Path,
        sandbox_policy: Option<&crate::ai::sandbox::SandboxPolicy>,
    ) -> Result<(String, Vec<String>), String> {
        let max_iterations = 3;
        let plan_id = format!("plan_{}", uuid::Uuid::new_v4().to_string().replace('-', ""));

        // Phase 1: Planning
        let plan = if let Some(ref mut router) = llm_router {
            let planning_prompt = format!(
                "Create a structured research plan to answer the following question. \
                 Respond with a JSON array of stages, each with 'stage_type' (Retrieve/Synthesize/Hypothesize/Verify) \
                 and 'goal' (short description).\n\nQuestion: {}\n\nJSON:",
                question
            );
            match router
                .complete(
                    "You are a research planner. Return only valid JSON.",
                    &planning_prompt,
                    None,
                )
                .await
            {
                Ok(response) => {
                    let stages = parse_plan_stages(&response.text, max_iterations);
                    ResearchPlan {
                        plan_id: plan_id.clone(),
                        question: question.to_string(),
                        stages,
                        current_stage: 0,
                        max_iterations,
                        iteration_count: 0,
                    }
                }
                Err(_) => {
                    // Fallback: single-stage plan
                    ResearchPlan {
                        plan_id: plan_id.clone(),
                        question: question.to_string(),
                        stages: vec![
                            ResearchStage {
                                stage_type: StageType::Retrieve,
                                goal: "Search for information".to_string(),
                                status: StageStatus::Pending,
                                result: None,
                            },
                            ResearchStage {
                                stage_type: StageType::Synthesize,
                                goal: "Synthesize findings".to_string(),
                                status: StageStatus::Pending,
                                result: None,
                            },
                        ],
                        current_stage: 0,
                        max_iterations,
                        iteration_count: 0,
                    }
                }
            }
        } else {
            // No LLM: fallback single-round plan
            ResearchPlan {
                plan_id: plan_id.clone(),
                question: question.to_string(),
                stages: vec![
                    ResearchStage {
                        stage_type: StageType::Retrieve,
                        goal: "Search local notes".to_string(),
                        status: StageStatus::Pending,
                        result: None,
                    },
                    ResearchStage {
                        stage_type: StageType::Synthesize,
                        goal: "Compile results".to_string(),
                        status: StageStatus::Pending,
                        result: None,
                    },
                ],
                current_stage: 0,
                max_iterations,
                iteration_count: 0,
            }
        };

        let mut all_results: Vec<SubagentEntry> = Vec::new();
        let mut intermediate_reports: Vec<String> = Vec::new();
        let mut hypotheses: Vec<String> = Vec::new();
        let ghost_ids: Vec<String> = Vec::new();
        let mut current_plan = plan;

        // Research loop
        while current_plan.iteration_count < current_plan.max_iterations {
            let stage_idx = current_plan.current_stage;
            if stage_idx >= current_plan.stages.len() {
                break;
            }

            let stage_type = current_plan.stages[stage_idx].stage_type.clone();

            match stage_type {
                StageType::Retrieve => {
                    let keywords: Vec<String> = question
                        .split_whitespace()
                        .filter(|w| w.len() > 2)
                        .map(|w| w.to_lowercase())
                        .collect();

                    let job = SubagentJob {
                        search_type: SearchType::All,
                        keywords: keywords.clone(),
                        data_sources: vec![DataSource::LocalVector, DataSource::WebSearch],
                        max_results_per_source: 10,
                        max_hops: 2,
                        current_hop: 0,
                    };

                    let results = self
                        .execute_maybe_filtered(&job, vector_store, sandbox_policy)
                        .await;
                    for r in &results {
                        all_results.extend(r.entries.clone());
                    }

                    current_plan.stages[stage_idx].status = StageStatus::Done;
                    current_plan.stages[stage_idx].result =
                        Some(format!("Retrieved {} results", all_results.len()));
                }
                StageType::Synthesize => {
                    if let Some(ref mut router) = llm_router {
                        let context: Vec<String> = all_results
                            .iter()
                            .map(|e| format!("[{}] {}", e.source, e.snippet))
                            .collect();
                        let synth_prompt = format!(
                            "Synthesize the following research findings into a coherent intermediate report. \
                             Include key themes, contradictions, and gaps.\n\nFindings:\n{}\n\nQuestion: {}\n\nReport:",
                            context.join("\n\n"), question
                        );
                        match router
                            .complete("You are a research synthesizer.", &synth_prompt, None)
                            .await
                        {
                            Ok(response) => {
                                intermediate_reports.push(response.text.clone());
                                current_plan.stages[stage_idx].result = Some(response.text);
                            }
                            Err(_) => {
                                let fallback = format!("## Intermediate Report\n\n{} results found. Key themes not available (LLM unavailable).", all_results.len());
                                intermediate_reports.push(fallback.clone());
                                current_plan.stages[stage_idx].result = Some(fallback);
                            }
                        }
                    } else {
                        let fallback = format!(
                            "## Intermediate Report\n\n{} results found from local search.",
                            all_results.len()
                        );
                        intermediate_reports.push(fallback.clone());
                        current_plan.stages[stage_idx].result = Some(fallback);
                    }
                    current_plan.stages[stage_idx].status = StageStatus::Done;
                }
                StageType::Hypothesize => {
                    if let Some(ref mut router) = llm_router {
                        let context: Vec<String> = all_results
                            .iter()
                            .take(20)
                            .map(|e| {
                                format!(
                                    "- {}: {}",
                                    e.title,
                                    e.snippet.chars().take(200).collect::<String>()
                                )
                            })
                            .collect();
                        let hyp_prompt = format!(
                            "Based on the following research findings, generate 3-5 testable hypotheses. \
                             For each, provide: 1) The hypothesis statement 2) Confidence level (0-1) \
                             3) Supporting evidence from the findings.\n\nFindings:\n{}\n\nHypotheses:",
                            context.join("\n")
                        );
                        match router
                            .complete(
                                "You are a scientific hypothesis generator.",
                                &hyp_prompt,
                                None,
                            )
                            .await
                        {
                            Ok(response) => {
                                hypotheses.push(response.text.clone());
                                current_plan.stages[stage_idx].result = Some(response.text);
                            }
                            Err(_) => {
                                current_plan.stages[stage_idx].result = Some(
                                    "Hypothesis generation unavailable (LLM error)".to_string(),
                                );
                            }
                        }
                    } else {
                        current_plan.stages[stage_idx].result =
                            Some("Hypothesis generation requires LLM.".to_string());
                    }
                    current_plan.stages[stage_idx].status = StageStatus::Done;
                }
                StageType::Verify => {
                    // Evaluate completeness
                    if let Some(ref mut router) = llm_router {
                        let eval_prompt = format!(
                            "Rate the completeness of this research on a scale of 1-10. \
                             Question: {}\nResults count: {}\nIntermediate reports: {}\n\n\
                             Reply with just a number 1-10 and a one-line reason.",
                            question,
                            all_results.len(),
                            intermediate_reports.len()
                        );
                        match router
                            .complete("You are a research evaluator.", &eval_prompt, None)
                            .await
                        {
                            Ok(response) => {
                                let score = response
                                    .text
                                    .trim()
                                    .chars()
                                    .next()
                                    .and_then(|c| c.to_digit(10))
                                    .unwrap_or(5);
                                current_plan.stages[stage_idx].result =
                                    Some(format!("Completeness: {}/10", score));
                                if score < 6
                                    && current_plan.iteration_count
                                        < current_plan.max_iterations - 1
                                {
                                    // Need more iterations: generate new keywords
                                    let kw_job = SubagentJob {
                                        search_type: SearchType::All,
                                        keywords: extract_iteration_keywords(&all_results),
                                        data_sources: vec![
                                            DataSource::WebSearch,
                                            DataSource::Arxiv,
                                        ],
                                        max_results_per_source: 10,
                                        max_hops: 1,
                                        current_hop: 0,
                                    };
                                    let more_results = self
                                        .execute_maybe_filtered(
                                            &kw_job,
                                            vector_store,
                                            sandbox_policy,
                                        )
                                        .await;
                                    for r in &more_results {
                                        all_results.extend(r.entries.clone());
                                    }
                                    // Reset to Retrieve stage for next iteration
                                    current_plan.stages[stage_idx].status = StageStatus::Done;
                                    // Add a new Retrieve stage
                                    current_plan.stages.push(ResearchStage {
                                        stage_type: StageType::Retrieve,
                                        goal: format!(
                                            "Additional search (iteration {})",
                                            current_plan.iteration_count + 1
                                        ),
                                        status: StageStatus::Pending,
                                        result: None,
                                    });
                                    current_plan.stages.push(ResearchStage {
                                        stage_type: StageType::Synthesize,
                                        goal: "Update synthesis with new findings".to_string(),
                                        status: StageStatus::Pending,
                                        result: None,
                                    });
                                }
                            }
                            Err(_) => {
                                current_plan.stages[stage_idx].result =
                                    Some("Evaluation unavailable".to_string());
                            }
                        }
                    } else {
                        current_plan.stages[stage_idx].result =
                            Some("Completeness evaluation requires LLM.".to_string());
                    }
                    current_plan.stages[stage_idx].status = StageStatus::Done;
                }
            }

            current_plan.current_stage += 1;
            current_plan.iteration_count += 1;
        }

        // Phase 6: Report — build final output
        let mut report = format!("# Deep Research: {}\n\n", question);
        report.push_str(&format!("**Plan ID**: {}\n", plan_id));
        report.push_str(&format!(
            "**Iterations**: {}\n",
            current_plan.iteration_count
        ));
        report.push_str(&format!("**Total Sources**: {}\n\n", all_results.len()));

        report.push_str("## Research Stages\n\n");
        for stage in &current_plan.stages {
            let status_mark = match stage.status {
                StageStatus::Done => "✅",
                StageStatus::Running => "🔄",
                StageStatus::Skipped => "⏭️",
                StageStatus::Pending => "⏳",
            };
            let stage_type_debug = format!("{:?}", stage.stage_type);
            report.push_str(&format!(
                "- {} **{}**: {}\n",
                status_mark, stage_type_debug, stage.goal
            ));
        }

        report.push_str("\n## Key Findings\n\n");
        for (i, entry) in all_results.iter().take(15).enumerate() {
            report.push_str(&format!(
                "{}. **{}** (from {})\n   {}\n\n",
                i + 1,
                entry.title,
                entry.source,
                entry.snippet.chars().take(300).collect::<String>()
            ));
        }

        if !hypotheses.is_empty() {
            report.push_str("## Hypotheses\n\n");
            for h in &hypotheses {
                report.push_str(h);
                report.push_str("\n\n");
            }
        }

        if !intermediate_reports.is_empty() {
            report.push_str("## Synthesis\n\n");
            report.push_str(intermediate_reports.last().unwrap_or(&String::new()));
        }

        // Write trace
        let _ = research_trace::write_cot_log(
            dualtrack_dir,
            task_id,
            &format!(
                "## Deep Research\n\nQuestion: {}\nPlan: {}\nIterations: {}\nResults: {}",
                question,
                plan_id,
                current_plan.iteration_count,
                all_results.len()
            ),
            None,
        );

        Ok((report, ghost_ids))
    }

    pub async fn generate_hypotheses(
        &self,
        entries: &[SubagentEntry],
        question: &str,
        llm_router: Option<&mut LlmRouter>,
    ) -> Vec<Hypothesis> {
        if entries.is_empty() {
            return vec![];
        }

        if let Some(router) = llm_router {
            let context: Vec<String> = entries
                .iter()
                .take(15)
                .map(|e| {
                    format!(
                        "- {} (source: {}): {}",
                        e.title,
                        e.source,
                        e.snippet.chars().take(300).collect::<String>()
                    )
                })
                .collect();

            let prompt = format!(
                "Based on the following research findings, generate 3-5 testable hypotheses. \
                 For each hypothesis, provide: 1) A clear, testable statement 2) Confidence level (0.0-1.0) \
                 3) Which finding(s) support it (reference by index number).\n\n\
                 Research question: {}\n\nFindings:\n{}\n\n\
                 Respond with a JSON array: [{{\"statement\": \"...\", \"confidence\": 0.X, \"supporting_indices\": [0, 2]}}, ...]",
                question, context.join("\n")
            );

            let mut router_clone = router.clone();
            match router_clone
                .complete(
                    "You are a scientific hypothesis generator. Return only valid JSON array.",
                    &prompt,
                    None,
                )
                .await
            {
                Ok(response) => {
                    if let Ok(hypotheses_json) =
                        serde_json::from_str::<Vec<serde_json::Value>>(&response.text)
                    {
                        return hypotheses_json
                            .into_iter()
                            .enumerate()
                            .map(|(_i, v)| {
                                let statement =
                                    v["statement"].as_str().unwrap_or("Unknown").to_string();
                                let confidence = v["confidence"].as_f64().unwrap_or(0.5) as f32;
                                let indices: Vec<usize> = v["supporting_indices"]
                                    .as_array()
                                    .map(|a| {
                                        a.iter()
                                            .filter_map(|v| v.as_u64().map(|n| n as usize))
                                            .collect()
                                    })
                                    .unwrap_or_default();
                                let supporting: Vec<String> = indices
                                    .iter()
                                    .filter_map(|&idx| entries.get(idx).map(|e| e.source.clone()))
                                    .collect();
                                Hypothesis {
                                    id: format!(
                                        "hyp_{}",
                                        uuid::Uuid::new_v4().to_string().replace('-', "")
                                    ),
                                    statement,
                                    confidence: confidence.clamp(0.0, 1.0),
                                    supporting_chunks: supporting,
                                    status: HypothesisStatus::Proposed,
                                    verification_result: None,
                                }
                            })
                            .collect();
                    }
                }
                Err(e) => {
                    tracing::warn!("Hypothesis generation LLM call failed: {}", e);
                }
            }
        }

        // Fallback: generate simple hypotheses from entry titles
        entries
            .iter()
            .take(3)
            .enumerate()
            .map(|(i, entry)| Hypothesis {
                id: format!("hyp_fb_{}", i),
                statement: format!("Further investigation needed: {}", entry.title),
                confidence: entry.relevance_score * 0.5,
                supporting_chunks: vec![entry.source.clone()],
                status: HypothesisStatus::Proposed,
                verification_result: None,
            })
            .collect()
    }

    pub async fn plan_research(
        &self,
        question: &str,
        llm_router: Option<&mut LlmRouter>,
    ) -> Vec<PlanStep> {
        if let Some(router) = llm_router {
            let prompt = format!(
                "Create a step-by-step research plan to answer: {}\n\
                 Available: local notes (vector search), web, arXiv, Semantic Scholar.\n\
                 Respond with JSON array: [{{\"step_type\": \"LiteratureSearch\", \"description\": \"...\", \"estimated_sources\": N, \"depends_on\": []}}]",
                question
            );
            let mut router_clone = router.clone();
            if let Ok(response) = router_clone
                .complete(
                    "You are a research planner. Return only valid JSON array.",
                    &prompt,
                    None,
                )
                .await
            {
                if let Ok(steps) = serde_json::from_str::<Vec<serde_json::Value>>(&response.text) {
                    return steps
                        .into_iter()
                        .enumerate()
                        .map(|(_i, v)| {
                            let st = v["step_type"].as_str().unwrap_or("LiteratureSearch");
                            PlanStep {
                                step_type: match st.to_lowercase().as_str() {
                                    "literaturesearch" => StepType::LiteratureSearch,
                                    "dataanalysis" => StepType::DataAnalysis,
                                    "synthesis" => StepType::Synthesis,
                                    "hypothesistest" => StepType::HypothesisTest,
                                    _ => StepType::LiteratureSearch,
                                },
                                description: v["description"]
                                    .as_str()
                                    .unwrap_or("Research step")
                                    .to_string(),
                                estimated_sources: v["estimated_sources"].as_u64().unwrap_or(5)
                                    as usize,
                                depends_on: v["depends_on"]
                                    .as_array()
                                    .map(|a| {
                                        a.iter()
                                            .filter_map(|x| x.as_u64().map(|n| n as usize))
                                            .collect()
                                    })
                                    .unwrap_or_default(),
                            }
                        })
                        .collect();
                }
            }
        }
        vec![
            PlanStep {
                step_type: StepType::LiteratureSearch,
                description: "Search for relevant sources".to_string(),
                estimated_sources: 10,
                depends_on: vec![],
            },
            PlanStep {
                step_type: StepType::Synthesis,
                description: "Synthesize findings".to_string(),
                estimated_sources: 0,
                depends_on: vec![0],
            },
            PlanStep {
                step_type: StepType::HypothesisTest,
                description: "Verify key claims".to_string(),
                estimated_sources: 5,
                depends_on: vec![1],
            },
        ]
    }
}

impl Default for SubagentJob {
    fn default() -> Self {
        Self {
            search_type: SearchType::All,
            keywords: vec![],
            data_sources: vec![DataSource::LocalVector],
            max_results_per_source: 10,
            max_hops: 3,
            current_hop: 0,
        }
    }
}

fn parse_plan_stages(json_str: &str, max_iterations: usize) -> Vec<ResearchStage> {
    // Try to parse JSON array
    if let Ok(stages) = serde_json::from_str::<Vec<serde_json::Value>>(json_str) {
        return stages
            .into_iter()
            .take(max_iterations * 2)
            .map(|v| {
                let stage_type = v["stage_type"].as_str().unwrap_or("Retrieve");
                let goal = v["goal"].as_str().unwrap_or("Research step").to_string();
                ResearchStage {
                    stage_type: match stage_type.to_lowercase().as_str() {
                        "retrieve" => StageType::Retrieve,
                        "synthesize" => StageType::Synthesize,
                        "hypothesize" => StageType::Hypothesize,
                        "verify" => StageType::Verify,
                        _ => StageType::Retrieve,
                    },
                    goal,
                    status: StageStatus::Pending,
                    result: None,
                }
            })
            .collect();
    }
    // Fallback: default stages
    vec![
        ResearchStage {
            stage_type: StageType::Retrieve,
            goal: "Search for information".to_string(),
            status: StageStatus::Pending,
            result: None,
        },
        ResearchStage {
            stage_type: StageType::Synthesize,
            goal: "Synthesize findings".to_string(),
            status: StageStatus::Pending,
            result: None,
        },
        ResearchStage {
            stage_type: StageType::Verify,
            goal: "Evaluate completeness".to_string(),
            status: StageStatus::Pending,
            result: None,
        },
    ]
}

fn extract_iteration_keywords(entries: &[SubagentEntry]) -> Vec<String> {
    Subagent::extract_keywords(entries, 5)
}

fn decode_xml_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
}

fn extract_between(xml: &str, tag: &str) -> Option<String> {
    let start_tag = format!("<{}>", tag);
    let end_tag = format!("</{}>", tag);
    let start = xml.find(&start_tag)? + start_tag.len();
    let end = xml[start..].find(&end_tag)?;
    Some(decode_xml_entities(xml[start..start + end].trim()))
}

fn extract_all_between(xml: &str, tag: &str) -> Vec<String> {
    let start_tag = format!("<{}>", tag);
    let end_tag = format!("</{}>", tag);
    let mut results = Vec::new();
    let mut offset = 0;
    while let Some(start) = xml[offset..].find(&start_tag) {
        let abs = offset + start + start_tag.len();
        if let Some(end) = xml[abs..].find(&end_tag) {
            results.push(decode_xml_entities(xml[abs..abs + end].trim()));
            offset = abs + end + end_tag.len();
        } else {
            break;
        }
    }
    results
}

fn parse_arxiv_xml(xml: &str) -> Vec<SubagentEntry> {
    let mut entries = Vec::new();
    let parts: Vec<&str> = xml.split("<entry>").collect();
    for part in parts.iter().skip(1) {
        let end = part.find("</entry>").unwrap_or(part.len());
        let entry_xml = &part[..end];

        let title = extract_between(entry_xml, "title").unwrap_or_default();
        let snippet = extract_between(entry_xml, "summary").unwrap_or_default();

        let url = extract_between(entry_xml, "id");
        let authors = extract_all_between(entry_xml, "name");

        let year = extract_between(entry_xml, "published").and_then(|s| {
            s.trim()
                .split('-')
                .next()
                .and_then(|y| y.parse::<u32>().ok())
        });

        entries.push(SubagentEntry {
            title,
            snippet,
            url,
            authors,
            year,
            source: "arXiv".to_string(),
            relevance_score: 0.0,
        });
    }
    entries
}

#[cfg(test)]
mod sandbox_tests {
    use super::{DataSource, SearchType, SubagentJob};
    use crate::ai::sandbox::{NetworkPolicy, SandboxPolicy};

    #[test]
    fn data_sources_are_filtered_by_sandbox_policy() {
        let job = SubagentJob {
            search_type: SearchType::All,
            keywords: vec!["bayes".to_string()],
            data_sources: vec![
                DataSource::LocalVector,
                DataSource::WebSearch,
                DataSource::Arxiv,
                DataSource::SemanticScholar,
            ],
            max_results_per_source: 5,
            max_hops: 1,
            current_hop: 0,
        };
        let policy = SandboxPolicy::read_only(&["vector_search"]);

        let filtered = job.filtered_by_policy(&policy);

        assert_eq!(filtered.data_sources, vec![DataSource::LocalVector]);
    }

    #[test]
    fn network_policy_still_blocks_network_sources_when_tool_is_allowlisted() {
        let job = SubagentJob {
            search_type: SearchType::All,
            keywords: vec!["bayes".to_string()],
            data_sources: vec![
                DataSource::WebSearch,
                DataSource::Arxiv,
                DataSource::SemanticScholar,
            ],
            max_results_per_source: 5,
            max_hops: 1,
            current_hop: 0,
        };
        let mut policy =
            SandboxPolicy::read_only(&["web_search", "arxiv_search", "semantic_scholar_search"]);
        policy.network_policy = NetworkPolicy::Disabled;

        let filtered = job.filtered_by_policy(&policy);

        assert!(filtered.data_sources.is_empty());
    }
}
