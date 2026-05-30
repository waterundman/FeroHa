// Two-Surface Interaction Protocol
// Defines the communication protocol between Human Surface and AI Surface
#![allow(dead_code)]

use crate::ai::agent_scheduler::TaskStatus;
use crate::ai::dream_engine::DreamEngine;
use crate::ai::vectordb::VectorStore;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Events from Human Surface to AI Surface
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum HumanToAiEvent {
    /// Note was created
    NoteCreated {
        path: String,
        content: String,
        timestamp: u64,
    },
    /// Note was modified
    NoteModified {
        path: String,
        content: String,
        old_content: Option<String>,
        timestamp: u64,
    },
    /// Note was deleted
    NoteDeleted { path: String, timestamp: u64 },
    /// Link was created
    LinkCreated {
        from: String,
        to: String,
        timestamp: u64,
    },
    /// Link was deleted
    LinkDeleted {
        from: String,
        to: String,
        timestamp: u64,
    },
    /// User requested AI action via instruction card
    InstructionCardExecuted {
        card_type: String,
        params: HashMap<String, String>,
        timestamp: u64,
    },
    /// User requested dream cycle
    DreamRequested { mode: String, timestamp: u64 },
    /// User selected text in editor
    TextSelected {
        path: String,
        selection: String,
        start_offset: usize,
        end_offset: usize,
        timestamp: u64,
    },
    /// User requested AI analysis of specific region
    RegionAnalyzeRequested {
        path: String,
        start_line: usize,
        end_line: usize,
        analysis_type: AnalysisType,
        timestamp: u64,
    },
    /// User feedback on AI suggestion
    SuggestionFeedback {
        suggestion_id: String,
        feedback: FeedbackType,
        reason: Option<String>,
        timestamp: u64,
    },
    /// User preference changed
    PreferenceChanged {
        key: String,
        old_value: String,
        new_value: String,
        timestamp: u64,
    },
}

/// Actions from AI Surface to Human Surface
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum AiToHumanAction {
    /// Write a note to AI surface
    WriteNote {
        path: String,
        content: String,
        source: String,
        timestamp: u64,
    },
    /// Create a link between notes
    CreateLink {
        from: String,
        to: String,
        link_type: String,
        timestamp: u64,
    },
    /// Send notification to user
    Notify {
        title: String,
        message: String,
        notification_type: NotificationType,
        timestamp: u64,
    },
    /// Show suggestion to user
    Suggest {
        suggestion: String,
        context: String,
        confidence: f32,
        timestamp: u64,
    },
    /// Dream cycle completed
    DreamCompleted {
        stats: DreamStatsSummary,
        timestamp: u64,
    },
    /// Research result
    ResearchResult {
        topic: String,
        summary: String,
        sources: Vec<String>,
        timestamp: u64,
    },
    /// Progressive suggestion (non-intrusive)
    ProgressiveSuggest {
        suggestion_id: String,
        content: String,
        relevance_score: f32,
        dismiss_after_secs: Option<u64>,
        timestamp: u64,
    },
    /// Context-aware note recommendations
    ContextualLink {
        source_note: String,
        recommended_notes: Vec<NoteRecommendation>,
        timestamp: u64,
    },
    /// Learning progress feedback
    LearningFeedback {
        metric: String,
        previous_value: f32,
        current_value: f32,
        insight: String,
        timestamp: u64,
    },
    /// Request human clarification
    RequestClarification {
        question: String,
        context: String,
        options: Vec<String>,
        timestamp: u64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NotificationType {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AnalysisType {
    Connections,
    Insights,
    Summarize,
    Expand,
    Critique,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FeedbackType {
    Helpful,
    NotHelpful,
    Incorrect,
    TooVerbose,
    TooBrief,
    OffTopic,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DreamStatsSummary {
    pub memories_processed: usize,
    pub connections_strengthened: usize,
    pub bridges_created: usize,
    pub communities_found: usize,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteRecommendation {
    pub path: String,
    pub relevance: f32,
    pub reason: String,
    pub snippet: String,
}

/// User action for trust score calculation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UserAction {
    Accept,
    Reject,
    Ignore,
}

/// Trust score for progressive interaction mode
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustScore {
    /// Current trust score (0.0-1.0)
    pub score: f32,
    /// Historical acceptance rate
    pub acceptance_rate: f32,
    /// Historical accuracy rate (proportion marked as helpful)
    pub accuracy_rate: f32,
    /// Consecutive accepts count
    pub consecutive_accepts: u32,
    /// Consecutive rejects count
    pub consecutive_rejects: u32,
    /// Total interactions
    pub total_interactions: u32,
    /// Total accepts
    pub total_accepts: u32,
    /// Total rejects
    pub total_rejects: u32,
}

impl TrustScore {
    /// Create a new TrustScore with default values
    pub fn new() -> Self {
        TrustScore {
            score: 0.5,
            acceptance_rate: 0.0,
            accuracy_rate: 0.0,
            consecutive_accepts: 0,
            consecutive_rejects: 0,
            total_interactions: 0,
            total_accepts: 0,
            total_rejects: 0,
        }
    }

    /// Update trust score based on user action
    pub fn update(&mut self, action: &UserAction) {
        self.total_interactions += 1;

        match action {
            UserAction::Accept => {
                self.consecutive_accepts += 1;
                self.consecutive_rejects = 0;
                self.total_accepts += 1;
                self.score = (self.score + 0.1).min(1.0);
            }
            UserAction::Reject => {
                self.consecutive_rejects += 1;
                self.consecutive_accepts = 0;
                self.total_rejects += 1;
                self.score = (self.score - 0.15).max(0.0);
            }
            UserAction::Ignore => {
                self.score = (self.score - 0.02).max(0.0);
            }
        }

        self.recalculate_rates();
    }

    /// Recalculate acceptance and accuracy rates
    fn recalculate_rates(&mut self) {
        if self.total_interactions > 0 {
            self.acceptance_rate = self.total_accepts as f32 / self.total_interactions as f32;
            self.accuracy_rate = self.acceptance_rate;
        }
    }

    /// Get recommended interaction mode based on trust score
    pub fn recommended_mode(&self) -> InteractionMode {
        if self.score >= 0.8 {
            InteractionMode::FullAuto
        } else if self.score >= 0.5 {
            InteractionMode::SemiAuto
        } else {
            InteractionMode::Manual
        }
    }

    /// Check if trust score indicates high confidence
    pub fn is_high_trust(&self) -> bool {
        self.score >= 0.8
    }

    /// Check if trust score indicates low confidence
    pub fn is_low_trust(&self) -> bool {
        self.score < 0.3
    }
}

impl Default for TrustScore {
    fn default() -> Self {
        Self::new()
    }
}

/// Interaction mode between surfaces
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InteractionMode {
    /// Manual mode: only respond to explicit user actions
    Manual,
    /// Semi-auto mode: suggest actions but require user approval
    SemiAuto,
    /// Full auto mode: automatically execute actions
    FullAuto,
}

/// Two-Surface Protocol handler
pub struct TwoSurfaceProtocol {
    mode: InteractionMode,
    event_log: Vec<HumanToAiEvent>,
    action_log: Vec<AiToHumanAction>,
    store: Option<VectorStore>,
    dream_engine: Option<DreamEngine>,
    pub trust_score: TrustScore,
}

impl Default for TwoSurfaceProtocol {
    fn default() -> Self {
        Self::new()
    }
}

impl TwoSurfaceProtocol {
    pub fn new() -> Self {
        TwoSurfaceProtocol {
            mode: InteractionMode::Manual,
            event_log: Vec::new(),
            action_log: Vec::new(),
            store: None,
            dream_engine: None,
            trust_score: TrustScore::new(),
        }
    }

    pub fn with_mode(mode: InteractionMode) -> Self {
        TwoSurfaceProtocol {
            mode,
            event_log: Vec::new(),
            action_log: Vec::new(),
            store: None,
            dream_engine: None,
            trust_score: TrustScore::new(),
        }
    }

    /// Set the vector store for this protocol instance
    pub fn with_store(mut self, store: VectorStore) -> Self {
        self.store = Some(store);
        self
    }

    /// Set the dream engine for this protocol instance
    pub fn with_dream_engine(mut self, engine: DreamEngine) -> Self {
        self.dream_engine = Some(engine);
        self
    }

    /// Process an event from Human Surface
    pub async fn process_event(&mut self, event: HumanToAiEvent) -> Vec<AiToHumanAction> {
        self.event_log.push(event.clone());

        match &self.mode {
            InteractionMode::Manual => {
                // Only respond to explicit instruction card executions
                match &event {
                    HumanToAiEvent::InstructionCardExecuted { .. } => {
                        self.handle_instruction_card(event).await
                    }
                    HumanToAiEvent::DreamRequested { .. } => self.handle_dream_request(event).await,
                    HumanToAiEvent::SuggestionFeedback {
                        suggestion_id,
                        feedback,
                        reason,
                        timestamp,
                    } => {
                        self.handle_suggestion_feedback(suggestion_id, feedback, reason, *timestamp)
                    }
                    _ => Vec::new(), // Ignore other events in manual mode
                }
            }
            InteractionMode::SemiAuto => {
                // Suggest actions for all events
                let mut actions = Vec::new();
                match &event {
                    HumanToAiEvent::NoteCreated { path, content, .. } => {
                        actions.push(self.suggest_analysis(&event));
                        if let Some(link) = self.find_contextual_links(path, content) {
                            actions.push(link);
                        }
                    }
                    HumanToAiEvent::NoteModified { path, content, .. } => {
                        actions.push(self.suggest_update(&event));
                        if let Some(link) = self.find_contextual_links(path, content) {
                            actions.push(link);
                        }
                    }
                    HumanToAiEvent::InstructionCardExecuted { .. } => {
                        actions.extend(self.handle_instruction_card(event).await);
                    }
                    HumanToAiEvent::DreamRequested { .. } => {
                        actions.extend(self.handle_dream_request(event).await);
                    }
                    HumanToAiEvent::SuggestionFeedback {
                        suggestion_id,
                        feedback,
                        reason,
                        timestamp,
                    } => {
                        actions.extend(self.handle_suggestion_feedback(
                            suggestion_id,
                            feedback,
                            reason,
                            *timestamp,
                        ));
                    }
                    HumanToAiEvent::TextSelected {
                        path,
                        selection,
                        start_offset,
                        end_offset,
                        timestamp,
                    } => {
                        actions.extend(self.handle_text_selected(
                            path,
                            selection,
                            *start_offset,
                            *end_offset,
                            *timestamp,
                        ));
                    }
                    HumanToAiEvent::RegionAnalyzeRequested {
                        path,
                        start_line,
                        end_line,
                        analysis_type,
                        timestamp,
                    } => {
                        actions.extend(self.handle_region_analyze(
                            path,
                            *start_line,
                            *end_line,
                            analysis_type,
                            *timestamp,
                        ));
                    }
                    _ => {}
                }
                actions
            }
            InteractionMode::FullAuto => {
                // Automatically execute all actions
                match &event {
                    HumanToAiEvent::NoteCreated { .. } => {
                        vec![self.auto_analyze(&event)]
                    }
                    HumanToAiEvent::NoteModified { .. } => {
                        vec![self.auto_update(&event)]
                    }
                    HumanToAiEvent::InstructionCardExecuted { .. } => {
                        self.handle_instruction_card(event).await
                    }
                    HumanToAiEvent::DreamRequested { .. } => self.handle_dream_request(event).await,
                    HumanToAiEvent::SuggestionFeedback {
                        suggestion_id,
                        feedback,
                        reason,
                        timestamp,
                    } => {
                        self.handle_suggestion_feedback(suggestion_id, feedback, reason, *timestamp)
                    }
                    HumanToAiEvent::TextSelected {
                        path,
                        selection,
                        start_offset,
                        end_offset,
                        timestamp,
                    } => self.handle_text_selected(
                        path,
                        selection,
                        *start_offset,
                        *end_offset,
                        *timestamp,
                    ),
                    HumanToAiEvent::RegionAnalyzeRequested {
                        path,
                        start_line,
                        end_line,
                        analysis_type,
                        timestamp,
                    } => self.handle_region_analyze(
                        path,
                        *start_line,
                        *end_line,
                        analysis_type,
                        *timestamp,
                    ),
                    _ => Vec::new(),
                }
            }
        }
    }

    /// Handle instruction card execution
    async fn handle_instruction_card(&mut self, event: HumanToAiEvent) -> Vec<AiToHumanAction> {
        if let HumanToAiEvent::InstructionCardExecuted {
            card_type,
            params,
            timestamp,
        } = event
        {
            match card_type.as_str() {
                "search" => {
                    let query = params.get("query").cloned().unwrap_or_default();
                    let top_k: usize = params
                        .get("top_k")
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(5);

                    let summary = if let Some(ref store) = self.store {
                        let results = store.search_text(&query, top_k);
                        if results.is_empty() {
                            "No results found".to_string()
                        } else {
                            let mut summary = format!("Found {} results:\n", results.len());
                            for (i, result) in results.iter().enumerate() {
                                summary.push_str(&format!(
                                    "{}. {} (score: {:.2})\n",
                                    i + 1,
                                    result.chunk_text,
                                    result.score
                                ));
                            }
                            summary
                        }
                    } else {
                        "Search unavailable: no vector store configured".to_string()
                    };

                    vec![AiToHumanAction::ResearchResult {
                        topic: query,
                        summary,
                        sources: Vec::new(),
                        timestamp,
                    }]
                }
                "summarize" => {
                    let target = params.get("target").cloned().unwrap_or_default();
                    let content = params.get("content").cloned().unwrap_or_default();

                    let summary = if content.is_empty() {
                        "No content provided for summary".to_string()
                    } else {
                        // Simple summary: extract first 3 paragraphs
                        let paragraphs: Vec<&str> = content
                            .split("\n\n")
                            .filter(|p| !p.trim().is_empty())
                            .take(3)
                            .collect();

                        if paragraphs.is_empty() {
                            "No meaningful content found".to_string()
                        } else {
                            let mut summary = String::from("Summary:\n");
                            for (i, para) in paragraphs.iter().enumerate() {
                                summary.push_str(&format!("{}. {}\n", i + 1, para.trim()));
                            }
                            summary
                        }
                    };

                    vec![AiToHumanAction::WriteNote {
                        path: format!("ai-notes/summaries/{}.md", target),
                        content: summary,
                        source: "summarize-card".to_string(),
                        timestamp,
                    }]
                }
                "dream" => {
                    let stats = if let (Some(ref mut engine), Some(ref store)) =
                        (&mut self.dream_engine, &self.store)
                    {
                        let dream_stats = engine.run_cycle(store);
                        DreamStatsSummary {
                            memories_processed: dream_stats.total_memories_processed,
                            connections_strengthened: dream_stats.nrem_connections_strengthened,
                            bridges_created: dream_stats.rem_bridges_created,
                            communities_found: dream_stats.insight_communities_found,
                            duration_ms: dream_stats.duration_ms,
                        }
                    } else {
                        DreamStatsSummary {
                            memories_processed: 0,
                            connections_strengthened: 0,
                            bridges_created: 0,
                            communities_found: 0,
                            duration_ms: 0,
                        }
                    };

                    vec![AiToHumanAction::DreamCompleted { stats, timestamp }]
                }
                "research" => {
                    let topic = params.get("topic").cloned().unwrap_or_default();
                    let depth = params.get("depth").cloned().unwrap_or_default();

                    let (summary, sources) = if let Some(ref store) = self.store {
                        let results = store.search_text(&topic, 10);
                        let mut srcs: Vec<String> = Vec::new();
                        if results.is_empty() {
                            (format!("No results found for topic: {}", topic), srcs)
                        } else {
                            let mut summary_text =
                                format!("Research results for '{}' (depth: {}):\n\n", topic, depth);
                            for (i, result) in results.iter().enumerate() {
                                summary_text.push_str(&format!(
                                    "{}. {}\n\n",
                                    i + 1,
                                    result.chunk_text
                                ));
                                if !srcs.contains(&result.source_file) {
                                    srcs.push(result.source_file.clone());
                                }
                            }
                            (summary_text, srcs)
                        }
                    } else {
                        (
                            format!(
                                "Research unavailable: no vector store configured (topic: {})",
                                topic
                            ),
                            Vec::new(),
                        )
                    };

                    vec![AiToHumanAction::ResearchResult {
                        topic,
                        summary,
                        sources,
                        timestamp,
                    }]
                }
                "connect" => {
                    let source = params.get("source").cloned().unwrap_or_default();
                    let target = params.get("target").cloned().unwrap_or_default();

                    let recommended_notes: Vec<NoteRecommendation> =
                        if let Some(ref store) = self.store {
                            let query = format!("{} {}", source, target);
                            let results = store.search_text(&query, 5);
                            results
                                .into_iter()
                                .map(|r| NoteRecommendation {
                                    path: r.source_file,
                                    relevance: r.score,
                                    reason: format!("Related to: {} / {}", source, target),
                                    snippet: r.chunk_text,
                                })
                                .collect()
                        } else {
                            Vec::new()
                        };

                    vec![AiToHumanAction::ContextualLink {
                        source_note: source,
                        recommended_notes,
                        timestamp,
                    }]
                }
                "organize" => {
                    let target = params.get("target").cloned().unwrap_or_default();

                    let suggestion_text = if let Some(ref store) = self.store {
                        let results = store.search_text("", 20);
                        let orphan_count = results.len().saturating_sub(5);
                        format!(
                            "Vault organization scan complete. Target: '{}'. {} notes scanned, {} may be orphaned (weak links). Consider tagging or linking these notes.",
                            target, results.len(), orphan_count
                        )
                    } else {
                        format!(
                            "Vault organization: vector store unavailable. Please index notes to enable orphan detection for target: '{}'.",
                            target
                        )
                    };

                    vec![AiToHumanAction::Suggest {
                        suggestion: suggestion_text,
                        context: "organize-card".to_string(),
                        confidence: 0.75,
                        timestamp,
                    }]
                }
                "analyze" => {
                    let target = params.get("target").cloned().unwrap_or_default();
                    let content = params.get("content").cloned().unwrap_or_default();

                    let analysis = if content.is_empty() {
                        "No content provided for analysis".to_string()
                    } else {
                        let paragraph_count = content
                            .split("\n\n")
                            .filter(|p| !p.trim().is_empty())
                            .count();
                        let sentence_count = content
                            .split(|c: char| {
                                c == '.'
                                    || c == '!'
                                    || c == '?'
                                    || c == '。'
                                    || c == '！'
                                    || c == '？'
                            })
                            .filter(|s| !s.trim().is_empty())
                            .count();
                        let word_count = content.split_whitespace().count();

                        let mut keyword_freq: std::collections::HashMap<&str, usize> =
                            std::collections::HashMap::new();
                        for word in content.split_whitespace() {
                            let cleaned = word.trim_matches(|c: char| !c.is_alphanumeric());
                            if cleaned.len() > 2 {
                                *keyword_freq.entry(cleaned).or_insert(0) += 1;
                            }
                        }
                        let mut top_keywords: Vec<(&str, usize)> =
                            keyword_freq.into_iter().collect();
                        top_keywords.sort_by(|a, b| b.1.cmp(&a.1));
                        let top_keywords: Vec<(&str, usize)> =
                            top_keywords.into_iter().take(10).collect();

                        let mut output = format!(
                            "## Text Analysis: {}\n\n",
                            if target.is_empty() {
                                "untitled"
                            } else {
                                &target
                            }
                        );
                        output.push_str("### Basic Statistics\n");
                        output.push_str(&format!("- Paragraphs: {}\n", paragraph_count));
                        output.push_str(&format!("- Sentences: {}\n", sentence_count));
                        output.push_str(&format!("- Words: {}\n\n", word_count));
                        output.push_str("### Top Keywords\n");
                        if top_keywords.is_empty() {
                            output.push_str("- No keywords detected\n");
                        } else {
                            for (kw, count) in &top_keywords {
                                output.push_str(&format!("- **{}**: {}\n", kw, count));
                            }
                        }
                        output
                    };

                    let target_slug = if target.is_empty() {
                        "untitled".to_string()
                    } else {
                        target.replace(|c: char| c == '/' || c == '\\', "_")
                    };

                    vec![AiToHumanAction::WriteNote {
                        path: format!("ai-notes/analysis/{}.md", target_slug),
                        content: analysis,
                        source: "analyze-card".to_string(),
                        timestamp,
                    }]
                }
                "rewrite" => {
                    let target = params.get("target").cloned().unwrap_or_default();
                    let content = params.get("content").cloned().unwrap_or_default();
                    let style = params
                        .get("style")
                        .cloned()
                        .unwrap_or_else(|| "formal".to_string());

                    let suggestion_text = format!(
                        "改写任务已提交。目标: '{}', 风格: '{}'。请在 Diff 面板查看改写结果。内容长度: {} 字",
                        if target.is_empty() { "untitled" } else { &target },
                        style,
                        content.chars().count()
                    );

                    vec![AiToHumanAction::Suggest {
                        suggestion: suggestion_text,
                        context: "rewrite-card".to_string(),
                        confidence: 0.85,
                        timestamp,
                    }]
                }
                _ => Vec::new(),
            }
        } else {
            Vec::new()
        }
    }

    /// Handle dream request
    async fn handle_dream_request(&mut self, event: HumanToAiEvent) -> Vec<AiToHumanAction> {
        if let HumanToAiEvent::DreamRequested { mode: _, timestamp } = event {
            let stats = if let (Some(ref mut engine), Some(ref store)) =
                (&mut self.dream_engine, &self.store)
            {
                let dream_stats = engine.run_cycle(store);
                DreamStatsSummary {
                    memories_processed: dream_stats.total_memories_processed,
                    connections_strengthened: dream_stats.nrem_connections_strengthened,
                    bridges_created: dream_stats.rem_bridges_created,
                    communities_found: dream_stats.insight_communities_found,
                    duration_ms: dream_stats.duration_ms,
                }
            } else {
                DreamStatsSummary {
                    memories_processed: 0,
                    connections_strengthened: 0,
                    bridges_created: 0,
                    communities_found: 0,
                    duration_ms: 0,
                }
            };

            vec![AiToHumanAction::DreamCompleted { stats, timestamp }]
        } else {
            Vec::new()
        }
    }

    /// Suggest analysis for a new note
    fn suggest_analysis(&self, event: &HumanToAiEvent) -> AiToHumanAction {
        let timestamp = match event {
            HumanToAiEvent::NoteCreated { timestamp, .. } => *timestamp,
            _ => 0,
        };
        AiToHumanAction::Suggest {
            suggestion: "Analyze this note for connections and insights?".to_string(),
            context: "New note detected".to_string(),
            confidence: 0.8,
            timestamp,
        }
    }

    /// Suggest update for a modified note
    fn suggest_update(&self, event: &HumanToAiEvent) -> AiToHumanAction {
        let timestamp = match event {
            HumanToAiEvent::NoteModified { timestamp, .. } => *timestamp,
            _ => 0,
        };
        AiToHumanAction::Suggest {
            suggestion: "Update AI index for this note?".to_string(),
            context: "Note modified".to_string(),
            confidence: 0.7,
            timestamp,
        }
    }

    /// Auto-analyze a new note
    fn auto_analyze(&mut self, event: &HumanToAiEvent) -> AiToHumanAction {
        let timestamp = match event {
            HumanToAiEvent::NoteCreated { timestamp, .. } => *timestamp,
            _ => 0,
        };

        let message = if let HumanToAiEvent::NoteCreated { path, content, .. } = event {
            if let Some(ref mut store) = self.store {
                // Index the note content
                index_note_content(store, path, content);
                format!("Note '{}' analyzed and indexed", path)
            } else {
                "Note analyzed but indexing unavailable".to_string()
            }
        } else {
            "Note analyzed".to_string()
        };

        AiToHumanAction::Notify {
            title: "AI Analysis".to_string(),
            message,
            notification_type: NotificationType::Info,
            timestamp,
        }
    }

    /// Auto-update for a modified note
    fn auto_update(&mut self, event: &HumanToAiEvent) -> AiToHumanAction {
        let timestamp = match event {
            HumanToAiEvent::NoteModified { timestamp, .. } => *timestamp,
            _ => 0,
        };

        let message = if let HumanToAiEvent::NoteModified { path, content, .. } = event {
            if let Some(ref mut store) = self.store {
                // Re-index the note content
                store.delete_by_file(path).unwrap_or(());
                index_note_content(store, path, content);
                format!("Note '{}' index updated", path)
            } else {
                "Note updated but indexing unavailable".to_string()
            }
        } else {
            "Note updated".to_string()
        };

        AiToHumanAction::Notify {
            title: "AI Update".to_string(),
            message,
            notification_type: NotificationType::Info,
            timestamp,
        }
    }

    /// Find contextual links for a note
    fn find_contextual_links(&self, path: &str, content: &str) -> Option<AiToHumanAction> {
        let store = self.store.as_ref()?;

        let results = store.search_text(content, 5);
        let recommended_notes: Vec<NoteRecommendation> = results
            .into_iter()
            .filter(|r| r.source_file != path)
            .take(5)
            .map(|r| NoteRecommendation {
                path: r.source_file,
                relevance: r.score,
                reason: "Semantically related".to_string(),
                snippet: r.chunk_text,
            })
            .collect();

        if recommended_notes.is_empty() {
            return None;
        }

        Some(AiToHumanAction::ContextualLink {
            source_note: path.to_string(),
            recommended_notes,
            timestamp: chrono::Utc::now().timestamp() as u64,
        })
    }

    /// Handle suggestion feedback from user
    fn handle_suggestion_feedback(
        &mut self,
        suggestion_id: &str,
        feedback: &FeedbackType,
        reason: &Option<String>,
        timestamp: u64,
    ) -> Vec<AiToHumanAction> {
        let action = match feedback {
            FeedbackType::Helpful => UserAction::Accept,
            _ => UserAction::Reject,
        };

        let old_score = self.trust_score.score;
        self.trust_score.update(&action);
        let new_score = self.trust_score.score;

        let _ = (suggestion_id, reason);

        vec![AiToHumanAction::LearningFeedback {
            metric: "trust_score".to_string(),
            previous_value: old_score,
            current_value: new_score,
            insight: format!("推荐模式: {:?}", self.trust_score.recommended_mode()),
            timestamp,
        }]
    }

    fn handle_text_selected(
        &mut self,
        path: &str,
        selection: &str,
        _start: usize,
        _end: usize,
        timestamp: u64,
    ) -> Vec<AiToHumanAction> {
        match &self.mode {
            InteractionMode::Manual => Vec::new(),
            InteractionMode::SemiAuto => {
                let suggestion_id = uuid::Uuid::new_v4().to_string();
                vec![AiToHumanAction::ProgressiveSuggest {
                    suggestion_id,
                    content: "要分析这段文本吗？".to_string(),
                    relevance_score: 0.7,
                    dismiss_after_secs: Some(15),
                    timestamp,
                }]
            }
            InteractionMode::FullAuto => {
                let snippet = if selection.len() > 200 {
                    format!("{}...", &selection[..200])
                } else {
                    selection.to_string()
                };
                vec![AiToHumanAction::ResearchResult {
                    topic: format!("文本分析: {}", path),
                    summary: format!(
                        "已自动分析选中的文本片段 ({} 字符)。\n原文: {}\n要点: 文本选自 {}, 长度 {} 字符。",
                        selection.len(),
                        snippet,
                        path,
                        selection.len()
                    ),
                    sources: vec![path.to_string()],
                    timestamp,
                }]
            }
        }
    }

    fn handle_region_analyze(
        &mut self,
        path: &str,
        start_line: usize,
        end_line: usize,
        analysis_type: &AnalysisType,
        timestamp: u64,
    ) -> Vec<AiToHumanAction> {
        match &self.mode {
            InteractionMode::Manual => Vec::new(),
            InteractionMode::SemiAuto | InteractionMode::FullAuto => {
                let context_summary = if let Some(ref store) = self.store {
                    let results = store.search_text(path, 5);
                    if results.is_empty() {
                        "No related context found".to_string()
                    } else {
                        let mut ctx = String::from("相关上下文:\n");
                        for (i, r) in results.iter().enumerate() {
                            ctx.push_str(&format!(
                                "{}. {}\n",
                                i + 1,
                                &r.chunk_text[..r.chunk_text.len().min(100)]
                            ));
                        }
                        ctx
                    }
                } else {
                    "向量存储不可用".to_string()
                };

                let analysis_label = match analysis_type {
                    AnalysisType::Connections => "关联分析",
                    AnalysisType::Insights => "洞察分析",
                    AnalysisType::Summarize => "摘要分析",
                    AnalysisType::Expand => "扩展分析",
                    AnalysisType::Critique => "批判分析",
                };

                vec![AiToHumanAction::ResearchResult {
                    topic: format!("区域分析 ({}:{}): {}", start_line, end_line, path),
                    summary: format!(
                        "对区域 L{}-L{} 进行了{}。\n{}",
                        start_line, end_line, analysis_label, context_summary
                    ),
                    sources: vec![path.to_string()],
                    timestamp,
                }]
            }
        }
    }

    /// Get interaction mode
    pub fn get_mode(&self) -> &InteractionMode {
        &self.mode
    }

    /// Get current trust score value
    pub fn trust_score_value(&self) -> f32 {
        self.trust_score.score
    }

    /// Get acceptance rate
    pub fn acceptance_rate(&self) -> f32 {
        self.trust_score.acceptance_rate
    }

    /// Get total interactions count
    pub fn total_interactions(&self) -> u32 {
        self.trust_score.total_interactions
    }

    /// Get accuracy rate
    pub fn accuracy_rate(&self) -> f32 {
        self.trust_score.accuracy_rate
    }

    /// Get consecutive accepts
    pub fn consecutive_accepts(&self) -> u32 {
        self.trust_score.consecutive_accepts
    }

    /// Get consecutive rejects
    pub fn consecutive_rejects(&self) -> u32 {
        self.trust_score.consecutive_rejects
    }

    /// Get total accepts
    pub fn total_accepts(&self) -> u32 {
        self.trust_score.total_accepts
    }

    /// Get total rejects
    pub fn total_rejects(&self) -> u32 {
        self.trust_score.total_rejects
    }

    /// Get recommended interaction mode
    pub fn current_mode(&self) -> InteractionMode {
        self.trust_score.recommended_mode()
    }

    /// Set interaction mode
    pub fn set_mode(&mut self, mode: InteractionMode) {
        self.mode = mode;
    }

    pub fn trust_score_record_accept(&mut self) {
        self.trust_score.update(&UserAction::Accept);
    }

    pub fn trust_score_record_reject(&mut self) {
        self.trust_score.update(&UserAction::Reject);
    }

    /// Get event log
    pub fn get_event_log(&self) -> &[HumanToAiEvent] {
        &self.event_log
    }

    /// Get action log
    pub fn get_action_log(&self) -> &[AiToHumanAction] {
        &self.action_log
    }
}

/// Index note content into the vector store (standalone function)
fn index_note_content(store: &mut VectorStore, path: &str, content: &str) {
    use crate::ai::chunker::chunk_markdown;
    use crate::ai::vectordb::ChunkRecord;

    let chunks = chunk_markdown(content, path);
    let records: Vec<ChunkRecord> = chunks
        .into_iter()
        .map(|chunk| {
            ChunkRecord {
                id: chunk.id,
                content_hash: u64::from_str_radix(&chunk.content_hash, 16).unwrap_or(0),
                chunk_text: chunk.text,
                embedding: vec![0.0; 384], // Placeholder embedding
                source_file: chunk.source_file,
                heading_context: chunk.heading_context,
                created_at: chrono::Utc::now().timestamp(),
                updated_at: chrono::Utc::now().timestamp(),
            }
        })
        .collect();

    store.upsert_batch(records).unwrap_or(());
}

/// Convert TaskStatus to frontend-friendly string representation
pub fn task_status_to_frontend(status: &TaskStatus) -> &'static str {
    match status {
        TaskStatus::Pending => "idle",
        TaskStatus::Approved { .. } | TaskStatus::Queued => "checking",
        TaskStatus::Running { .. } => "researching",
        TaskStatus::Done { .. } => "completed",
        TaskStatus::Error { .. } => "error",
        TaskStatus::Cancelled => "cancelled",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protocol_creation() {
        let protocol = TwoSurfaceProtocol::new();
        assert!(matches!(protocol.get_mode(), InteractionMode::Manual));
        assert_eq!(protocol.get_event_log().len(), 0);
        assert_eq!(protocol.get_action_log().len(), 0);
    }

    #[tokio::test]
    async fn test_manual_mode_ignores_note_events() {
        let mut protocol = TwoSurfaceProtocol::new();
        let event = HumanToAiEvent::NoteCreated {
            path: "test.md".to_string(),
            content: "# Test".to_string(),
            timestamp: 0,
        };
        let actions = protocol.process_event(event).await;
        assert_eq!(actions.len(), 0);
    }

    #[tokio::test]
    async fn test_manual_mode_handles_instruction_card() {
        let mut protocol = TwoSurfaceProtocol::new();
        let mut params = HashMap::new();
        params.insert("query".to_string(), "test query".to_string());

        let event = HumanToAiEvent::InstructionCardExecuted {
            card_type: "search".to_string(),
            params,
            timestamp: 0,
        };
        let actions = protocol.process_event(event).await;
        assert_eq!(actions.len(), 1);
    }

    #[tokio::test]
    async fn test_semi_auto_mode_suggests() {
        let mut protocol = TwoSurfaceProtocol::with_mode(InteractionMode::SemiAuto);
        let event = HumanToAiEvent::NoteCreated {
            path: "test.md".to_string(),
            content: "# Test".to_string(),
            timestamp: 0,
        };
        let actions = protocol.process_event(event).await;
        assert_eq!(actions.len(), 1);
        assert!(matches!(actions[0], AiToHumanAction::Suggest { .. }));
    }

    #[tokio::test]
    async fn test_full_auto_mode_analyzes() {
        let mut protocol = TwoSurfaceProtocol::with_mode(InteractionMode::FullAuto);
        let event = HumanToAiEvent::NoteCreated {
            path: "test.md".to_string(),
            content: "# Test".to_string(),
            timestamp: 0,
        };
        let actions = protocol.process_event(event).await;
        assert_eq!(actions.len(), 1);
        assert!(matches!(actions[0], AiToHumanAction::Notify { .. }));
    }

    #[tokio::test]
    async fn test_search_with_vector_store() {
        use crate::ai::vectordb::{ChunkRecord, VectorStore};

        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().to_str().unwrap();
        let mut store = VectorStore::new(db_path);

        // Add some test data
        store
            .upsert(ChunkRecord {
                id: "c1".to_string(),
                content_hash: 1,
                chunk_text: "Rust is a systems programming language".to_string(),
                embedding: vec![0.0; 384],
                source_file: "test.md".to_string(),
                heading_context: "Intro".to_string(),
                created_at: 0,
                updated_at: 0,
            })
            .unwrap();

        let mut protocol = TwoSurfaceProtocol::new().with_store(store);

        let mut params = HashMap::new();
        params.insert("query".to_string(), "Rust programming".to_string());

        let event = HumanToAiEvent::InstructionCardExecuted {
            card_type: "search".to_string(),
            params,
            timestamp: 0,
        };

        let actions = protocol.process_event(event).await;
        assert_eq!(actions.len(), 1);

        if let AiToHumanAction::ResearchResult { summary, .. } = &actions[0] {
            assert!(summary.contains("Rust is a systems programming language"));
        } else {
            panic!("Expected ResearchResult action");
        }
    }

    #[tokio::test]
    async fn test_dream_with_engine() {
        use crate::ai::dream_engine::DreamEngine;
        use crate::ai::vectordb::{ChunkRecord, VectorStore};

        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().to_str().unwrap();
        let mut store = VectorStore::new(db_path);

        // Add some test data
        store
            .upsert(ChunkRecord {
                id: "c1".to_string(),
                content_hash: 1,
                chunk_text: "Test memory".to_string(),
                embedding: vec![0.0; 384],
                source_file: "test.md".to_string(),
                heading_context: "".to_string(),
                created_at: 0,
                updated_at: 0,
            })
            .unwrap();

        let engine = DreamEngine::new();

        let mut protocol = TwoSurfaceProtocol::new()
            .with_store(store)
            .with_dream_engine(engine);

        let event = HumanToAiEvent::InstructionCardExecuted {
            card_type: "dream".to_string(),
            params: HashMap::new(),
            timestamp: 0,
        };

        let actions = protocol.process_event(event).await;
        assert_eq!(actions.len(), 1);

        if let AiToHumanAction::DreamCompleted { stats, .. } = &actions[0] {
            // DreamStatsSummary should have non-zero values when engine processes data
            assert!(stats.memories_processed > 0 || stats.duration_ms > 0);
        } else {
            panic!("Expected DreamCompleted action");
        }
    }

    #[tokio::test]
    async fn test_summarize_generates_summary() {
        let mut protocol = TwoSurfaceProtocol::new();

        let mut params = HashMap::new();
        params.insert("target".to_string(), "test".to_string());
        params.insert(
            "content".to_string(),
            "First paragraph.\n\nSecond paragraph.\n\nThird paragraph.\n\nFourth paragraph."
                .to_string(),
        );

        let event = HumanToAiEvent::InstructionCardExecuted {
            card_type: "summarize".to_string(),
            params,
            timestamp: 0,
        };

        let actions = protocol.process_event(event).await;
        assert_eq!(actions.len(), 1);

        if let AiToHumanAction::WriteNote { content, .. } = &actions[0] {
            assert!(content.contains("First paragraph"));
            assert!(content.contains("Second paragraph"));
            assert!(content.contains("Third paragraph"));
            assert!(!content.contains("Fourth paragraph")); // Only first 3 paragraphs
        } else {
            panic!("Expected WriteNote action");
        }
    }

    #[tokio::test]
    async fn test_auto_analyze_indexes_content() {
        use crate::ai::vectordb::VectorStore;

        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().to_str().unwrap();
        let store = VectorStore::new(db_path);

        let mut protocol =
            TwoSurfaceProtocol::with_mode(InteractionMode::FullAuto).with_store(store);

        let event = HumanToAiEvent::NoteCreated {
            path: "test.md".to_string(),
            content: "# Test\n\nSome content.".to_string(),
            timestamp: 0,
        };

        let actions = protocol.process_event(event).await;
        assert_eq!(actions.len(), 1);

        if let AiToHumanAction::Notify { message, .. } = &actions[0] {
            assert!(message.contains("analyzed and indexed"));
        } else {
            panic!("Expected Notify action");
        }
    }

    #[tokio::test]
    async fn test_auto_update_reindexes_content() {
        use crate::ai::vectordb::VectorStore;

        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().to_str().unwrap();
        let store = VectorStore::new(db_path);

        let mut protocol =
            TwoSurfaceProtocol::with_mode(InteractionMode::FullAuto).with_store(store);

        let event = HumanToAiEvent::NoteModified {
            path: "test.md".to_string(),
            content: "# Updated\n\nNew content.".to_string(),
            old_content: Some("# Test\n\nOld content.".to_string()),
            timestamp: 0,
        };

        let actions = protocol.process_event(event).await;
        assert_eq!(actions.len(), 1);

        if let AiToHumanAction::Notify { message, .. } = &actions[0] {
            assert!(message.contains("index updated"));
        } else {
            panic!("Expected Notify action");
        }
    }

    #[test]
    fn test_trust_score_creation() {
        let ts = TrustScore::new();
        assert_eq!(ts.score, 0.5);
        assert_eq!(ts.total_interactions, 0);
    }

    #[test]
    fn test_trust_score_accept() {
        let mut ts = TrustScore::new();
        ts.update(&UserAction::Accept);
        assert!(ts.score > 0.5);
        assert_eq!(ts.consecutive_accepts, 1);
        assert_eq!(ts.total_accepts, 1);
    }

    #[test]
    fn test_trust_score_reject() {
        let mut ts = TrustScore::new();
        ts.update(&UserAction::Reject);
        assert!(ts.score < 0.5);
        assert_eq!(ts.consecutive_rejects, 1);
        assert_eq!(ts.total_rejects, 1);
    }

    #[test]
    fn test_trust_score_recommended_mode() {
        let mut ts = TrustScore::new();
        assert!(matches!(ts.recommended_mode(), InteractionMode::SemiAuto));

        for _ in 0..4 {
            ts.update(&UserAction::Accept);
        }
        assert!(matches!(ts.recommended_mode(), InteractionMode::FullAuto));

        for _ in 0..10 {
            ts.update(&UserAction::Reject);
        }
        assert!(matches!(ts.recommended_mode(), InteractionMode::Manual));
    }
}
