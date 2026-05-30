use crate::ai::agent_scheduler::{AgentTask, TaskHandle};
use crate::ai::sandbox::SandboxPolicy;
use crate::ai::task_intent::TaskIntentType;
use crate::harness::regression::{DreamAuditSnapshot, EpochEndReason};
use crate::harness::scientist::CleanKnowledge;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegressionThresholds {
    pub semantic_repeat_max: f32,
    pub tool_loop_max: f32,
    pub convergence_min: f32,
    pub min_epoch_before_audit: usize,
    #[serde(default = "default_auto_terminate")]
    pub auto_terminate: bool,
    #[serde(default = "default_max_consecutive")]
    pub max_consecutive_regressions: usize,
    #[serde(default = "default_cooldown_ms")]
    pub termination_cooldown_ms: u64,
    #[serde(default = "default_evidence_gain_min")]
    pub evidence_gain_min: f32,
    #[serde(default = "default_dream_coverage_min")]
    pub dream_coverage_min: f32,
    #[serde(default = "default_salience_shift_min")]
    pub salience_shift_min: f32,
    #[serde(default = "default_contradiction_risk_max")]
    pub contradiction_risk_max: f32,
}

impl Default for RegressionThresholds {
    fn default() -> Self {
        Self {
            semantic_repeat_max: 0.85,
            tool_loop_max: 0.6,
            convergence_min: 0.05,
            min_epoch_before_audit: 3,
            auto_terminate: true,
            max_consecutive_regressions: 3,
            termination_cooldown_ms: 60000,
            evidence_gain_min: 0.05,
            dream_coverage_min: 0.9,
            salience_shift_min: 0.05,
            contradiction_risk_max: 0.8,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegressionMetrics {
    pub semantic_repeat_rate: f32,
    pub tool_call_loop_rate: f32,
    pub convergence_rate: f32,
    pub epoch: usize,
    #[serde(default)]
    pub novelty_delta: f32,
    #[serde(default)]
    pub evidence_gain: f32,
    #[serde(default)]
    pub dream_coverage: f32,
    #[serde(default)]
    pub salience_shift: f32,
    #[serde(default)]
    pub contradiction_risk: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AuditAction {
    Healthy,
    Degraded,
    Terminated,
    Cooldown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditResult {
    pub metrics: RegressionMetrics,
    pub action: AuditAction,
    #[serde(default)]
    pub reasons: Vec<EpochEndReason>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AgentStatus {
    Healthy,
    Degraded,
    Terminated,
    Cooldown,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentState {
    pub agent_id: String,
    pub status: AgentStatus,
    pub regression_count: usize,
    pub last_epoch: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackInfo {
    pub track_id: String,
    pub focus: String,
    pub status: String,
    pub parent_agent: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OrchestratorMaterialPacket {
    pub track_id: String,
    pub parent_task_id: String,
    pub focus: String,
    pub instruction: String,
    pub prompt: String,
    pub claims: Vec<String>,
    pub source_refs: Vec<String>,
    pub sandbox_policy: SandboxPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OrchestratorEventType {
    AuditPassed,
    RegressionDetected,
    AgentDegraded,
    CleanKnowledgeExtracted,
    ParallelTracksSpawned,
    TrackCompleted,
    TrackFailed,
    TrackCancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestratorEvent {
    pub epoch: usize,
    pub agent_id: String,
    pub event_type: OrchestratorEventType,
    pub timestamp: u64,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestratorStatus {
    pub active_agents: usize,
    pub degraded_agents: Vec<String>,
    pub epoch_count: usize,
    pub track_count: usize,
    pub last_event: Option<OrchestratorEvent>,
    #[serde(default)]
    pub recent_events: Vec<OrchestratorEvent>,
    #[serde(default)]
    pub agent_states: Vec<AgentState>,
    #[serde(default)]
    pub track_details: Vec<TrackInfo>,
}

pub struct Orchestrator {
    pub epoch_count: usize,
    pub max_parallel_tracks: usize,
    pub degraded_agents: HashSet<String>,
    pub event_log: Vec<OrchestratorEvent>,
    pub regression_thresholds: RegressionThresholds,
    agent_epochs: HashMap<String, usize>,
    agent_previous_results: HashMap<String, Vec<String>>,
    agent_previous_result_len: HashMap<String, usize>,
    agent_regression_count: HashMap<String, usize>,
    agent_cooldown_until: HashMap<String, u64>,
}

impl Orchestrator {
    pub fn new(max_parallel_tracks: usize) -> Self {
        Self {
            epoch_count: 0,
            max_parallel_tracks,
            degraded_agents: HashSet::new(),
            event_log: Vec::new(),
            regression_thresholds: RegressionThresholds::default(),
            agent_epochs: HashMap::new(),
            agent_previous_results: HashMap::new(),
            agent_previous_result_len: HashMap::new(),
            agent_regression_count: HashMap::new(),
            agent_cooldown_until: HashMap::new(),
        }
    }

    pub fn audit_epoch(&mut self, agent_id: &str, task: &AgentTask) -> AuditResult {
        self.audit_epoch_with_dream(agent_id, task, None)
    }

    pub fn audit_epoch_with_dream(
        &mut self,
        agent_id: &str,
        task: &AgentTask,
        dream: Option<DreamAuditSnapshot>,
    ) -> AuditResult {
        self.epoch_count += 1;
        let dream = dream.unwrap_or_default();

        let epoch = self
            .agent_epochs
            .entry(agent_id.to_string())
            .and_modify(|e| *e += 1)
            .or_insert(1);
        let current_epoch = *epoch;

        if current_epoch < self.regression_thresholds.min_epoch_before_audit {
            self.store_snapshot(agent_id, task);
            self.log_event(agent_id, OrchestratorEventType::AuditPassed, None);
            return AuditResult {
                metrics: RegressionMetrics {
                    semantic_repeat_rate: 0.0,
                    tool_call_loop_rate: 0.0,
                    convergence_rate: 0.0,
                    epoch: current_epoch,
                    novelty_delta: 1.0,
                    evidence_gain: 1.0,
                    dream_coverage: dream.community_coverage,
                    salience_shift: dream.salience_shift,
                    contradiction_risk: dream.contradiction_risk,
                },
                action: AuditAction::Healthy,
                reasons: Vec::new(),
            };
        }

        let semantic = self.compute_semantic_repeat(agent_id, task);
        let tool_loop = self.compute_tool_loop_rate(task);
        let convergence = self.compute_convergence_rate(agent_id, task);
        let evidence_gain = self.compute_evidence_gain(agent_id, task);

        self.store_snapshot(agent_id, task);

        let metrics = RegressionMetrics {
            semantic_repeat_rate: semantic,
            tool_call_loop_rate: tool_loop,
            convergence_rate: convergence,
            epoch: current_epoch,
            novelty_delta: 1.0 - semantic,
            evidence_gain,
            dream_coverage: dream.community_coverage,
            salience_shift: dream.salience_shift,
            contradiction_risk: dream.contradiction_risk,
        };

        let reasons = self.classify_epoch_reasons(&metrics);
        let threshold_exceeded = convergence < self.regression_thresholds.convergence_min
            || reasons.iter().any(Self::is_regressive_reason);

        if self.is_in_cooldown(agent_id) {
            return AuditResult {
                metrics,
                action: AuditAction::Cooldown,
                reasons,
            };
        }

        if threshold_exceeded {
            self.log_event(
                agent_id,
                OrchestratorEventType::RegressionDetected,
                Some(format!(
                    "semantic={:.3} tool_loop={:.3} convergence={:.3} evidence_gain={:.3} dream_coverage={:.3} contradiction_risk={:.3} reasons={}",
                    metrics.semantic_repeat_rate,
                    metrics.tool_call_loop_rate,
                    metrics.convergence_rate,
                    metrics.evidence_gain,
                    metrics.dream_coverage,
                    metrics.contradiction_risk,
                    Self::format_reasons(&reasons)
                )),
            );

            if self.regression_thresholds.auto_terminate {
                let count = self
                    .agent_regression_count
                    .entry(agent_id.to_string())
                    .and_modify(|c| *c += 1)
                    .or_insert(1);
                if *count >= self.regression_thresholds.max_consecutive_regressions {
                    self.degrade_agent(agent_id);
                    let cooldown_until =
                        now_millis() + self.regression_thresholds.termination_cooldown_ms;
                    self.agent_cooldown_until
                        .insert(agent_id.to_string(), cooldown_until);
                    return AuditResult {
                        metrics,
                        action: AuditAction::Terminated,
                        reasons,
                    };
                } else {
                    self.degrade_agent(agent_id);
                    return AuditResult {
                        metrics,
                        action: AuditAction::Degraded,
                        reasons,
                    };
                }
            } else {
                return AuditResult {
                    metrics,
                    action: AuditAction::Degraded,
                    reasons,
                };
            }
        } else {
            self.agent_regression_count.insert(agent_id.to_string(), 0);
            let detail = if reasons.is_empty() {
                None
            } else {
                Some(format!("reasons={}", Self::format_reasons(&reasons)))
            };
            self.log_event(agent_id, OrchestratorEventType::AuditPassed, detail);
            return AuditResult {
                metrics,
                action: AuditAction::Healthy,
                reasons,
            };
        }
    }

    pub fn degrade_agent(&mut self, agent_id: &str) -> bool {
        let newly_degraded = self.degraded_agents.insert(agent_id.to_string());
        if newly_degraded {
            self.log_event(
                agent_id,
                OrchestratorEventType::AgentDegraded,
                Some("Agent marked as degraded due to regression".to_string()),
            );
        }
        newly_degraded
    }

    pub fn agent_states(&self) -> Vec<AgentState> {
        let all_agents: HashSet<&str> = self.agent_epochs.keys().map(|s| s.as_str()).collect();
        all_agents
            .iter()
            .map(|&id| {
                let status = if self.degraded_agents.contains(id) {
                    AgentStatus::Degraded
                } else {
                    AgentStatus::Healthy
                };
                let reg_count = self.agent_regression_count.get(id).copied().unwrap_or(0);
                let last_epoch = self.agent_epochs.get(id).copied().unwrap_or(0);
                AgentState {
                    agent_id: id.to_string(),
                    status,
                    regression_count: reg_count,
                    last_epoch,
                }
            })
            .collect()
    }

    pub fn terminate_agent(&mut self, agent_id: &str) -> bool {
        if self.degraded_agents.contains(agent_id) {
            return false;
        }
        self.degraded_agents.insert(agent_id.to_string());
        let cooldown_until = now_millis() + self.regression_thresholds.termination_cooldown_ms;
        self.agent_cooldown_until
            .insert(agent_id.to_string(), cooldown_until);
        self.log_event(
            agent_id,
            OrchestratorEventType::AgentDegraded,
            Some("Manually terminated".to_string()),
        );
        true
    }

    pub fn reinstate_agent(&mut self, agent_id: &str) -> bool {
        let was_degraded = self.degraded_agents.remove(agent_id);
        self.agent_regression_count.remove(agent_id);
        self.agent_cooldown_until.remove(agent_id);
        if was_degraded {
            self.log_event(
                agent_id,
                OrchestratorEventType::AuditPassed,
                Some("Manually reinstated".to_string()),
            );
        }
        was_degraded
    }

    pub fn is_in_cooldown(&self, agent_id: &str) -> bool {
        if let Some(&until) = self.agent_cooldown_until.get(agent_id) {
            now_millis() < until
        } else {
            false
        }
    }

    pub fn spawn_parallel_tracks(
        &mut self,
        knowledge: &CleanKnowledge,
        original_task: &AgentTask,
    ) -> Vec<TaskHandle> {
        self.spawn_parallel_track_packets(knowledge, original_task)
            .into_iter()
            .map(|packet| TaskHandle {
                id: packet.track_id,
            })
            .collect()
    }

    pub fn spawn_parallel_track_packets(
        &mut self,
        knowledge: &CleanKnowledge,
        original_task: &AgentTask,
    ) -> Vec<OrchestratorMaterialPacket> {
        let packets: Vec<OrchestratorMaterialPacket> = Self::track_focuses()
            .into_iter()
            .enumerate()
            .map(|(i, (focus, instruction))| {
                let track_id = format!("{}_track_{}", original_task.id, i);
                OrchestratorMaterialPacket {
                    track_id,
                    parent_task_id: original_task.id.clone(),
                    prompt: Self::build_track_prompt(
                        &focus,
                        &instruction,
                        knowledge,
                        original_task,
                    ),
                    focus,
                    instruction,
                    claims: knowledge.claims.clone(),
                    source_refs: knowledge
                        .sources
                        .iter()
                        .map(|source| source.key.clone())
                        .collect(),
                    sandbox_policy: TaskIntentType::Verify.default_sandbox_policy(),
                }
            })
            .collect();

        let track_count = packets.len();
        self.log_event(
            &original_task.id,
            OrchestratorEventType::ParallelTracksSpawned,
            Some(format!("Spawned {} parallel tracks", track_count)),
        );

        packets
    }

    pub fn status(&self) -> OrchestratorStatus {
        let active = self
            .agent_epochs
            .len()
            .saturating_sub(self.degraded_agents.len());

        let mut spawned = 0usize;
        let mut completed = 0usize;
        let mut failed = 0usize;
        let mut cancelled = 0usize;

        for event in &self.event_log {
            match event.event_type {
                OrchestratorEventType::ParallelTracksSpawned => spawned += 1,
                OrchestratorEventType::TrackCompleted => completed += 1,
                OrchestratorEventType::TrackFailed => failed += 1,
                OrchestratorEventType::TrackCancelled => cancelled += 1,
                _ => {}
            }
        }

        let track_count = spawned.saturating_sub(completed + failed + cancelled);

        let recent_events: Vec<OrchestratorEvent> =
            self.event_log.iter().rev().take(20).cloned().collect();

        let agent_states = self.agent_states();

        let track_details: Vec<TrackInfo> = self
            .event_log
            .iter()
            .filter_map(|event| match event.event_type {
                OrchestratorEventType::TrackCompleted => Some(TrackInfo {
                    track_id: event.agent_id.clone(),
                    focus: String::new(),
                    status: "completed".to_string(),
                    parent_agent: event.agent_id.clone(),
                }),
                OrchestratorEventType::TrackFailed => Some(TrackInfo {
                    track_id: event.agent_id.clone(),
                    focus: String::new(),
                    status: "failed".to_string(),
                    parent_agent: event.agent_id.clone(),
                }),
                _ => None,
            })
            .collect();

        OrchestratorStatus {
            active_agents: active,
            degraded_agents: self.degraded_agents.iter().cloned().collect(),
            epoch_count: self.epoch_count,
            track_count,
            last_event: self.event_log.last().cloned(),
            recent_events,
            agent_states,
            track_details,
        }
    }

    fn compute_semantic_repeat(&self, agent_id: &str, task: &AgentTask) -> f32 {
        let current_snippets: Vec<String> = task
            .subagent_results
            .iter()
            .flat_map(|r| r.entries.iter())
            .map(|e| e.snippet.clone())
            .collect();

        if let Some(prev_snippets) = self.agent_previous_results.get(agent_id) {
            let current_joined = current_snippets.join(" ");
            let prev_joined = prev_snippets.join(" ");
            let current_tokens: HashSet<&str> = current_joined.split_whitespace().collect();
            let prev_tokens: HashSet<&str> = prev_joined.split_whitespace().collect();

            let intersection = current_tokens.intersection(&prev_tokens).count();
            let union = current_tokens.union(&prev_tokens).count();

            if union > 0 {
                intersection as f32 / union as f32
            } else {
                0.0
            }
        } else {
            0.0
        }
    }

    fn compute_tool_loop_rate(&self, task: &AgentTask) -> f32 {
        let results = &task.subagent_results;
        if results.len() < 2 {
            return 0.0;
        }

        let source_names: Vec<String> = results.iter().map(|r| format!("{:?}", r.source)).collect();

        let mut same_consecutive = 0usize;
        for i in 0..source_names.len() - 1 {
            if source_names[i] == source_names[i + 1] {
                same_consecutive += 1;
            }
        }

        same_consecutive as f32 / (source_names.len() - 1) as f32
    }

    fn compute_convergence_rate(&self, agent_id: &str, task: &AgentTask) -> f32 {
        let current_len: usize = task
            .subagent_results
            .iter()
            .flat_map(|r| r.entries.iter())
            .map(|e| e.snippet.len())
            .sum();

        if let Some(&prev_len) = self.agent_previous_result_len.get(agent_id) {
            let max_len = current_len.max(prev_len).max(1);
            let delta = (current_len as isize - prev_len as isize).unsigned_abs();
            1.0 - (delta as f32 / max_len as f32)
        } else {
            1.0
        }
    }

    fn compute_evidence_gain(&self, agent_id: &str, task: &AgentTask) -> f32 {
        let current_len: usize = task
            .subagent_results
            .iter()
            .flat_map(|r| r.entries.iter())
            .map(|e| e.snippet.len())
            .sum();

        if let Some(&prev_len) = self.agent_previous_result_len.get(agent_id) {
            if current_len <= prev_len {
                return 0.0;
            }
            let max_len = current_len.max(prev_len).max(1);
            (current_len - prev_len) as f32 / max_len as f32
        } else {
            1.0
        }
    }

    fn classify_epoch_reasons(&self, metrics: &RegressionMetrics) -> Vec<EpochEndReason> {
        let mut reasons = Vec::new();

        if metrics.semantic_repeat_rate > self.regression_thresholds.semantic_repeat_max {
            reasons.push(EpochEndReason::NoveltyPlateau);
        }

        if metrics.evidence_gain <= self.regression_thresholds.evidence_gain_min
            || metrics.convergence_rate < self.regression_thresholds.convergence_min
        {
            reasons.push(EpochEndReason::EvidencePlateau);
        }

        if metrics.tool_call_loop_rate > self.regression_thresholds.tool_loop_max {
            reasons.push(EpochEndReason::ToolLoop);
        }

        if metrics.dream_coverage >= self.regression_thresholds.dream_coverage_min
            && metrics.salience_shift <= self.regression_thresholds.salience_shift_min
        {
            reasons.push(EpochEndReason::DreamCoverageReached);
        }

        if metrics.contradiction_risk >= self.regression_thresholds.contradiction_risk_max {
            reasons.push(EpochEndReason::ContradictionRiskHigh);
        }

        reasons
    }

    fn is_regressive_reason(reason: &EpochEndReason) -> bool {
        matches!(
            reason,
            EpochEndReason::NoveltyPlateau
                | EpochEndReason::EvidencePlateau
                | EpochEndReason::ContradictionRiskHigh
                | EpochEndReason::ToolLoop
                | EpochEndReason::BudgetExhausted
        )
    }

    fn format_reasons(reasons: &[EpochEndReason]) -> String {
        if reasons.is_empty() {
            "none".to_string()
        } else {
            reasons
                .iter()
                .map(EpochEndReason::as_str)
                .collect::<Vec<_>>()
                .join(",")
        }
    }

    fn store_snapshot(&mut self, agent_id: &str, task: &AgentTask) {
        let snippets: Vec<String> = task
            .subagent_results
            .iter()
            .flat_map(|r| r.entries.iter())
            .map(|e| e.snippet.clone())
            .collect();

        let total_len: usize = task
            .subagent_results
            .iter()
            .flat_map(|r| r.entries.iter())
            .map(|e| e.snippet.len())
            .sum();

        self.agent_previous_results
            .insert(agent_id.to_string(), snippets);
        self.agent_previous_result_len
            .insert(agent_id.to_string(), total_len);
    }

    fn log_event(
        &mut self,
        agent_id: &str,
        event_type: OrchestratorEventType,
        detail: Option<String>,
    ) {
        let event = OrchestratorEvent {
            epoch: self.epoch_count,
            agent_id: agent_id.to_string(),
            event_type,
            timestamp: now_millis(),
            detail,
        };
        self.event_log.push(event);
    }

    fn track_focuses() -> Vec<(String, String)> {
        vec![
            (
                "correctness".to_string(),
                "Verify factual correctness of each claim, check for contradictions with known facts"
                    .to_string(),
            ),
            (
                "consistency".to_string(),
                "Check internal logical consistency, ensure no self-contradiction in conclusions"
                    .to_string(),
            ),
            (
                "verification".to_string(),
                "Cross-reference claims against external sources, flag unsupported assertions"
                    .to_string(),
            ),
        ]
    }

    fn build_track_prompt(
        focus: &str,
        instruction: &str,
        knowledge: &CleanKnowledge,
        original_task: &AgentTask,
    ) -> String {
        let claims = if knowledge.claims.is_empty() {
            "- No claims extracted".to_string()
        } else {
            knowledge
                .claims
                .iter()
                .map(|claim| format!("- {}", claim))
                .collect::<Vec<_>>()
                .join("\n")
        };
        let sources = if knowledge.sources.is_empty() {
            "- No source fragments attached".to_string()
        } else {
            knowledge
                .sources
                .iter()
                .map(|source| format!("- {}", source.key))
                .collect::<Vec<_>>()
                .join("\n")
        };
        format!(
            "Orchestrator material packet\n\nParent task: {}\nIntent: {}\nFocus: {}\nInstruction: {}\n\nClaims:\n{}\n\nSources:\n{}\n\nReturn a verification report. Do not edit files directly.",
            original_task.id, original_task.intent, focus, instruction, claims, sources
        )
    }
}

fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn default_auto_terminate() -> bool {
    true
}
fn default_max_consecutive() -> usize {
    3
}
fn default_cooldown_ms() -> u64 {
    60000
}
fn default_evidence_gain_min() -> f32 {
    0.05
}
fn default_dream_coverage_min() -> f32 {
    0.9
}
fn default_salience_shift_min() -> f32 {
    0.05
}
fn default_contradiction_risk_max() -> f32 {
    0.8
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::agent_scheduler::{
        AgentRole, AgentTask, SubTask, SubTaskStatus, SynthesizePhase, TaskPriority, TaskStatus,
        TaskType,
    };
    use crate::ai::subagent::{DataSource, SubagentEntry, SubagentResult};
    use crate::cli::parser::CliCommand;
    use crate::harness::context::{ContextFragment, ContextLayer, ContextSource};
    use serde_json::json;

    fn make_task(id: &str, snippets: &[&str]) -> AgentTask {
        let entries: Vec<SubagentEntry> = snippets
            .iter()
            .enumerate()
            .map(|(i, s)| SubagentEntry {
                title: format!("Entry {}", i),
                snippet: s.to_string(),
                url: None,
                authors: vec![],
                year: None,
                source: "test".to_string(),
                relevance_score: 0.5 + i as f32 * 0.1,
            })
            .collect();

        let subagent_results = if entries.is_empty() {
            vec![]
        } else {
            vec![SubagentResult {
                source: DataSource::LocalVector,
                entries,
                hop: 0,
                generated_keywords: vec![],
                total_found: snippets.len(),
                graph_manifest: None,
            }]
        };

        AgentTask {
            id: id.to_string(),
            command: CliCommand::Status,
            task_type: TaskType::Search,
            task_intent: Some(crate::ai::task_intent::TaskIntentType::Research),
            sandbox_policy: Some(
                crate::ai::task_intent::TaskIntentType::Research.default_sandbox_policy(),
            ),
            priority: TaskPriority::Medium,
            priority_score: 50,
            status: TaskStatus::Pending,
            anchor_note: None,
            created_at: now_millis(),
            max_retries: 1,
            retry_count: 0,
            synthesize_phase: SynthesizePhase::Idle,
            subagent_results,
            graph_manifest: None,
            has_trace: false,
            source_block_id: None,
            card_type: None,
            prompt: None,
            params: None,
            context_note: None,
            card_id: None,
            intent: String::new(),
            content: String::new(),
            max_iterations: 30,
            sub_tasks: vec![],
            material_packet: None,
            context_fragments: vec![],
            regression_metrics: None,
            retry_delay_ms: 1000,
            retry_backoff_multiplier: 2.0,
            last_retry_at: None,
            consecutive_failures: 0,
        }
    }

    fn make_task_with_subs(
        id: &str,
        snippets: &[&str],
        sub_tasks: Vec<SubTask>,
        fragments: Vec<ContextFragment>,
    ) -> AgentTask {
        let mut task = make_task(id, snippets);
        task.sub_tasks = sub_tasks;
        task.context_fragments = fragments;
        task
    }

    fn make_looping_task(id: &str, snippets: &[&str]) -> AgentTask {
        let mut task = make_task(id, snippets);
        let entries: Vec<SubagentEntry> = snippets
            .iter()
            .enumerate()
            .map(|(i, s)| SubagentEntry {
                title: format!("Loop Entry {}", i),
                snippet: s.to_string(),
                url: None,
                authors: vec![],
                year: None,
                source: "loop".to_string(),
                relevance_score: 0.8,
            })
            .collect();

        task.subagent_results = vec![
            SubagentResult {
                source: DataSource::LocalVector,
                entries: entries.clone(),
                hop: 0,
                generated_keywords: vec!["same".to_string()],
                total_found: entries.len(),
                graph_manifest: None,
            },
            SubagentResult {
                source: DataSource::LocalVector,
                entries,
                hop: 1,
                generated_keywords: vec!["same".to_string()],
                total_found: snippets.len(),
                graph_manifest: None,
            },
        ];
        task
    }

    #[test]
    fn test_regression_reason_codes_include_plateaus_and_tool_loop() {
        let task = make_looping_task("agent_reason", &["repeat repeat repeat"]);
        let mut orch = Orchestrator::new(4);
        orch.regression_thresholds.auto_terminate = false;
        orch.regression_thresholds.min_epoch_before_audit = 1;
        orch.regression_thresholds.tool_loop_max = 0.5;

        let first = orch.audit_epoch("agent_reason", &task);
        assert_eq!(first.action, AuditAction::Degraded);
        assert!(first.reasons.contains(&EpochEndReason::ToolLoop));

        let second = orch.audit_epoch("agent_reason", &task);
        assert_eq!(second.action, AuditAction::Degraded);
        assert!(second.reasons.contains(&EpochEndReason::NoveltyPlateau));
        assert!(second.reasons.contains(&EpochEndReason::EvidencePlateau));
        assert_eq!(second.metrics.novelty_delta, 0.0);
        assert_eq!(second.metrics.evidence_gain, 0.0);
    }

    #[test]
    fn test_audit_epoch_with_dream_returns_dream_reason_codes() {
        let task = make_task("agent_dream", &["stable memory"]);
        let mut orch = Orchestrator::new(4);
        orch.regression_thresholds.min_epoch_before_audit = 1;
        orch.regression_thresholds.auto_terminate = false;

        let result = orch.audit_epoch_with_dream(
            "agent_dream",
            &task,
            Some(DreamAuditSnapshot {
                community_coverage: 0.96,
                salience_shift: 0.01,
                contradiction_risk: 0.91,
            }),
        );

        assert!(result
            .reasons
            .contains(&EpochEndReason::DreamCoverageReached));
        assert!(result
            .reasons
            .contains(&EpochEndReason::ContradictionRiskHigh));
        assert_eq!(result.metrics.dream_coverage, 0.96);
        assert_eq!(result.metrics.salience_shift, 0.01);
        assert_eq!(result.metrics.contradiction_risk, 0.91);
    }

    #[test]
    fn test_audit_pass_early_epoch() {
        let task = make_task("agent1", &["hello world"]);
        let mut orch = Orchestrator::new(4);

        let result = orch.audit_epoch("agent1", &task);
        assert_eq!(
            result.action,
            AuditAction::Healthy,
            "Early epoch should return Healthy"
        );
        assert_eq!(orch.epoch_count, 1);

        let result = orch.audit_epoch("agent1", &task);
        assert_eq!(
            result.action,
            AuditAction::Healthy,
            "Second early epoch should return Healthy"
        );
        assert_eq!(orch.epoch_count, 2);
    }

    #[test]
    fn test_regression_detected() {
        let task = make_task("agent1", &["hello world foo bar"]);
        let mut orch = Orchestrator::new(4);

        // epoch 1 — skip
        orch.audit_epoch("agent1", &task);
        // epoch 2 — skip
        orch.audit_epoch("agent1", &task);
        // epoch 3 — should compute and detect (identical content → Jaccard=1.0)
        let result = orch.audit_epoch("agent1", &task);

        assert!(
            matches!(result.action, AuditAction::Degraded),
            "Third epoch should return Degraded"
        );
        assert!(
            result.metrics.semantic_repeat_rate > 0.85,
            "Expected high semantic repeat, got {}",
            result.metrics.semantic_repeat_rate
        );

        // Verify regression event was logged
        let reg_events: Vec<_> = orch
            .event_log
            .iter()
            .filter(|e| matches!(e.event_type, OrchestratorEventType::RegressionDetected))
            .collect();
        assert!(
            !reg_events.is_empty(),
            "Should have RegressionDetected event"
        );
    }

    #[test]
    fn test_degrade_idempotent() {
        let mut orch = Orchestrator::new(4);

        let first = orch.degrade_agent("agent1");
        assert!(first, "First degrade should return true");

        let second = orch.degrade_agent("agent1");
        assert!(!second, "Second degrade on same agent should return false");

        assert!(orch.degraded_agents.contains("agent1"));

        let deg_events: Vec<_> = orch
            .event_log
            .iter()
            .filter(|e| matches!(e.event_type, OrchestratorEventType::AgentDegraded))
            .collect();
        assert_eq!(
            deg_events.len(),
            1,
            "Should only log one AgentDegraded event"
        );
    }

    #[test]
    fn test_extract_clean_knowledge() {
        let sub_tasks = vec![
            SubTask {
                id: "sub_0".to_string(),
                parent_task_id: "task1".to_string(),
                description: "Claim A: gravity exists".to_string(),
                status: SubTaskStatus::Done,
                depends_on: vec![],
                assigned_agent: AgentRole::Scientist,
            },
            SubTask {
                id: "sub_1".to_string(),
                parent_task_id: "task1".to_string(),
                description: "Claim B: mass bends spacetime".to_string(),
                status: SubTaskStatus::Done,
                depends_on: vec![],
                assigned_agent: AgentRole::Scientist,
            },
            SubTask {
                id: "sub_2".to_string(),
                parent_task_id: "task1".to_string(),
                description: "Claim C: yet to finish".to_string(),
                status: SubTaskStatus::Running,
                depends_on: vec![],
                assigned_agent: AgentRole::Retriever,
            },
        ];

        let fragments = vec![ContextFragment {
            id: "frag_0".to_string(),
            key: "research.gravity".to_string(),
            value: json!({"topic": "gravity"}),
            source: ContextSource::RAG,
            layer: ContextLayer::Note,
            created_at: 1,
            ttl: None,
            hash: ContextFragment::compute_hash("research.gravity", &json!({"topic": "gravity"})),
        }];

        let task = make_task_with_subs("task1", &["snippet A", "snippet B"], sub_tasks, fragments);

        let knowledge = crate::harness::scientist::Scientist::extract_knowledge(&task);

        assert_eq!(knowledge.claims.len(), 2);
        assert!(knowledge.claims.iter().any(|c| c.contains("Claim A")));
        assert!(knowledge.claims.iter().any(|c| c.contains("Claim B")));
        assert!(!knowledge.claims.iter().any(|c| c.contains("Claim C")));

        assert_eq!(knowledge.sources.len(), 1);
        assert_eq!(knowledge.sources[0].id, "frag_0");

        assert!(!knowledge.confidence_map.is_empty());
    }

    #[test]
    fn test_spawn_parallel_tracks() {
        let task = make_task("agent1", &["hello"]);
        let mut orch = Orchestrator::new(4);

        let knowledge = CleanKnowledge {
            claims: vec!["claim1".to_string()],
            sources: vec![],
            confidence_map: HashMap::new(),
        };

        let handles = orch.spawn_parallel_tracks(&knowledge, &task);
        assert_eq!(handles.len(), 3, "Should spawn exactly 3 tracks");
        assert_eq!(handles[0].id, "agent1_track_0");
        assert_eq!(handles[1].id, "agent1_track_1");
        assert_eq!(handles[2].id, "agent1_track_2");

        let spawn_events: Vec<_> = orch
            .event_log
            .iter()
            .filter(|e| matches!(e.event_type, OrchestratorEventType::ParallelTracksSpawned))
            .collect();
        assert_eq!(spawn_events.len(), 1);
    }

    #[test]
    fn test_spawn_parallel_track_packets_include_materials_and_sandbox() {
        let mut task = make_task("agent1", &["hello"]);
        task.intent = "Audit Bayesian note".to_string();
        let mut orch = Orchestrator::new(4);

        let knowledge = CleanKnowledge {
            claims: vec!["Claim A: posterior updates prior".to_string()],
            sources: vec![ContextFragment {
                id: "frag_1".to_string(),
                key: "Bayes.md#intro".to_string(),
                value: json!({"text": "Bayes source"}),
                source: ContextSource::RAG,
                layer: ContextLayer::Note,
                created_at: 1,
                ttl: None,
                hash: ContextFragment::compute_hash(
                    "Bayes.md#intro",
                    &json!({"text": "Bayes source"}),
                ),
            }],
            confidence_map: HashMap::new(),
        };

        let packets = orch.spawn_parallel_track_packets(&knowledge, &task);

        assert_eq!(packets.len(), 3);
        assert_eq!(packets[0].track_id, "agent1_track_0");
        assert_eq!(packets[0].parent_task_id, "agent1");
        assert_eq!(packets[0].focus, "correctness");
        assert!(packets[0].prompt.contains("Claim A"));
        assert_eq!(packets[0].source_refs, vec!["Bayes.md#intro"]);
        assert!(packets[0].sandbox_policy.allows_tool("proposition_kernel"));
        assert!(!packets[0].sandbox_policy.allows_tool("ghost_write"));
        assert!(packets[0].sandbox_policy.write_roots.is_empty());
    }

    #[test]
    fn test_status_track_count() {
        let task = make_task("agent1", &["hello"]);
        let mut orch = Orchestrator::new(4);

        let knowledge = CleanKnowledge {
            claims: vec!["claim1".to_string()],
            sources: vec![],
            confidence_map: HashMap::new(),
        };

        orch.spawn_parallel_tracks(&knowledge, &task);

        let status = orch.status();
        assert_eq!(
            status.track_count, 1,
            "Track count should reflect spawned tracks"
        );
        assert_eq!(status.epoch_count, 0);
    }

    #[test]
    fn test_tool_loop_zero_with_one_result() {
        let task = make_task("agent1", &["single snippet"]);
        let orch = Orchestrator::new(4);
        let rate = orch.compute_tool_loop_rate(&task);
        assert_eq!(rate, 0.0);
    }

    #[test]
    fn test_convergence_first_epoch() {
        let task = make_task("agent1", &["hello world"]);
        let orch = Orchestrator::new(4);
        let rate = orch.compute_convergence_rate("agent1", &task);
        assert_eq!(rate, 1.0);
    }

    #[test]
    fn test_auto_terminate_enabled() {
        let mut orch = Orchestrator::new(4);
        orch.regression_thresholds.auto_terminate = true;
        orch.regression_thresholds.max_consecutive_regressions = 2;
        orch.regression_thresholds.min_epoch_before_audit = 1;

        let task = make_task("agent_auto", &["hello world foo bar"]);

        // Epoch 1: warmup — nothing to compare against, stores snapshot
        let result = orch.audit_epoch("agent_auto", &task);
        assert_eq!(result.action, AuditAction::Healthy);

        // Epoch 2: regression detected, first strike → Degraded
        let result = orch.audit_epoch("agent_auto", &task);
        assert_eq!(result.action, AuditAction::Degraded);
        assert!(orch.degraded_agents.contains("agent_auto"));

        // Epoch 3: regression detected again since same content → Terminated
        let result = orch.audit_epoch("agent_auto", &task);
        assert_eq!(result.action, AuditAction::Terminated);
        assert!(orch.degraded_agents.contains("agent_auto"));
    }

    #[test]
    fn test_auto_terminate_disabled() {
        let mut orch = Orchestrator::new(4);
        orch.regression_thresholds.auto_terminate = false;
        orch.regression_thresholds.min_epoch_before_audit = 1;

        let task = make_task("agent_noauto", &["hello world foo bar"]);

        // Epoch 1: warmup — stores snapshot, no regression (nothing to compare)
        let result = orch.audit_epoch("agent_noauto", &task);
        assert_eq!(result.action, AuditAction::Healthy);

        // Epochs 2-5: always Degraded, never Terminated
        for _ in 0..4 {
            let result = orch.audit_epoch("agent_noauto", &task);
            assert_eq!(result.action, AuditAction::Degraded);
        }
    }

    #[test]
    fn test_cooldown_prevents_audit() {
        let mut orch = Orchestrator::new(4);
        orch.regression_thresholds.auto_terminate = true;
        orch.regression_thresholds.max_consecutive_regressions = 1;
        orch.regression_thresholds.min_epoch_before_audit = 1;
        orch.regression_thresholds.termination_cooldown_ms = 60000;

        let task = make_task("agent_cool", &["hello world foo bar"]);

        // Epoch 1: warmup — stores snapshot
        orch.audit_epoch("agent_cool", &task);

        // Epoch 2: regression detected → Terminated (max=1)
        let result = orch.audit_epoch("agent_cool", &task);
        assert_eq!(result.action, AuditAction::Terminated);

        // Immediately again → Cooldown
        let result = orch.audit_epoch("agent_cool", &task);
        assert_eq!(result.action, AuditAction::Cooldown);
    }

    #[test]
    fn test_regression_count_reset_on_pass() {
        let mut orch = Orchestrator::new(4);
        orch.regression_thresholds.auto_terminate = true;
        orch.regression_thresholds.max_consecutive_regressions = 3;
        orch.regression_thresholds.min_epoch_before_audit = 1;

        let task_a = make_task("agent_reset", &["hello world foo bar"]);
        let task_b = make_task("agent_reset", &["completely different content here"]);

        // Epoch 1: warmup with task_a — stores snapshot, no regression (first epoch)
        orch.audit_epoch("agent_reset", &task_a);

        // Epoch 2: regression with same task_a → Degraded
        let result = orch.audit_epoch("agent_reset", &task_a);
        assert!(matches!(result.action, AuditAction::Degraded));

        // Epoch 3: submit different content (task_b), no regression → Healthy, reset count
        let result = orch.audit_epoch("agent_reset", &task_b);
        assert_eq!(result.action, AuditAction::Healthy);
        assert_eq!(
            orch.agent_regression_count
                .get("agent_reset")
                .copied()
                .unwrap_or(99),
            0
        );
    }

    #[test]
    fn test_agent_states() {
        let mut orch = Orchestrator::new(4);

        // Register agent_healthy via audit
        let task = make_task("agent_healthy", &["hello world"]);
        orch.audit_epoch("agent_healthy", &task);

        // Register agent_degraded and degrade it
        orch.audit_epoch("agent_degraded", &task);
        orch.degrade_agent("agent_degraded");

        let states = orch.agent_states();
        assert_eq!(states.len(), 2);

        let healthy = states
            .iter()
            .find(|s| s.agent_id == "agent_healthy")
            .unwrap();
        assert_eq!(healthy.status, AgentStatus::Healthy);

        let degraded = states
            .iter()
            .find(|s| s.agent_id == "agent_degraded")
            .unwrap();
        assert_eq!(degraded.status, AgentStatus::Degraded);
    }

    #[test]
    fn test_status_extended() {
        let mut orch = Orchestrator::new(4);
        let task = make_task("agent1", &["hello world"]);
        orch.audit_epoch("agent1", &task);

        let knowledge = CleanKnowledge {
            claims: vec!["claim1".to_string()],
            sources: vec![],
            confidence_map: HashMap::new(),
        };
        orch.spawn_parallel_tracks(&knowledge, &task);

        let status = orch.status();

        assert!(
            !status.recent_events.is_empty(),
            "recent_events should have entries"
        );
        assert!(
            !status.agent_states.is_empty(),
            "agent_states should have entries"
        );
        // track_details may be empty since no TrackCompleted/TrackFailed events
    }

    #[test]
    fn test_terminate_and_reinstate() {
        let mut orch = Orchestrator::new(4);

        let terminated = orch.terminate_agent("agent_tr");
        assert!(terminated);
        assert!(orch.degraded_agents.contains("agent_tr"));

        // Cannot terminate again
        let again = orch.terminate_agent("agent_tr");
        assert!(!again);

        let reinstated = orch.reinstate_agent("agent_tr");
        assert!(reinstated);
        assert!(!orch.degraded_agents.contains("agent_tr"));

        // Reinstating again returns false
        let again = orch.reinstate_agent("agent_tr");
        assert!(!again);
    }

    #[test]
    fn test_reinstate_clears_cooldown() {
        let mut orch = Orchestrator::new(4);
        orch.terminate_agent("agent_clr");
        assert!(orch.is_in_cooldown("agent_clr"));

        orch.reinstate_agent("agent_clr");
        assert!(!orch.is_in_cooldown("agent_clr"));
    }
}
