// Agent Scheduler — Manage AI agent task lifecycle
// v2.4: Task Handoff state machine (Pending→Approved→Running→Done|Error)

use super::research_trace::TaskContext;
use super::sandbox::{NetworkPolicy, SandboxPolicy};
use super::subagent::{DataSource, SearchType, Subagent, SubagentJob, SubagentResult};
use super::task_intent::TaskIntentType;
use crate::cli::parser::CliCommand;
use crate::graph::manifest::GraphManifest;
use crate::harness::context::{ContextFragment, ContextLayer, ContextSource};
use crate::harness::orchestrator::{
    AuditAction, Orchestrator, OrchestratorMaterialPacket, OrchestratorStatus, RegressionMetrics,
    TrackInfo,
};
use crate::harness::orchestrator_runtime::ControlledSubagentJob;
use crate::harness::proposition_kernel::PropositionKernel;
use crate::harness::regression::DreamAuditSnapshot;
use crate::harness::scientist::{CleanKnowledge, Scientist};
use crate::harness::workflow::{
    AgentRegistry, GoalContract, HarnessEvent, OrchestratorReplanRequest, StepReport,
    VerificationFinding, WorkflowError, WorkflowIr, WorkflowPatch, WorkflowRunState,
    WorkflowRuntimeEventChain, WorkflowRuntimeEventStore,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskType {
    Search,
    Summarize,
    FetchPapers,
    DeepDive,
    Explain,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskPriority {
    High,   // User-initiated, expecting immediate response
    Medium, // User-initiated, can wait
    Low,    // Background/automatic task
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    Approved { checked_at: u64, checked_by: String },
    Queued,
    Running { started_at: u64, progress: f32 },
    Done { completed_at: u64, result: String },
    Error { message: String },
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SynthesizePhase {
    Idle,
    Retrieving,
    RetrievalDone,
    Synthesizing,
    Writing,
    Done,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTask {
    pub id: String,
    pub command: CliCommand,
    pub task_type: TaskType,
    #[serde(default)]
    pub task_intent: Option<TaskIntentType>,
    #[serde(default)]
    pub sandbox_policy: Option<SandboxPolicy>,
    pub priority: TaskPriority,
    pub priority_score: u8,
    pub status: TaskStatus,
    pub anchor_note: Option<String>,
    pub created_at: u64,
    pub max_retries: u32,
    pub retry_count: u32,
    pub synthesize_phase: SynthesizePhase,
    pub subagent_results: Vec<SubagentResult>,
    pub graph_manifest: Option<GraphManifest>,
    pub has_trace: bool,
    pub source_block_id: Option<String>,
    pub card_id: Option<String>,
    pub card_type: Option<String>,
    pub prompt: Option<String>,
    pub params: Option<std::collections::HashMap<String, String>>,
    pub context_note: Option<String>,
    pub intent: String,
    pub content: String,
    pub max_iterations: usize,
    pub sub_tasks: Vec<SubTask>,
    #[serde(default)]
    pub material_packet: Option<OrchestratorMaterialPacket>,
    #[serde(default)]
    pub context_fragments: Vec<ContextFragment>,
    #[serde(default)]
    pub regression_metrics: Option<RegressionMetrics>,
    #[serde(default)]
    pub retry_delay_ms: u64,
    #[serde(default)]
    pub retry_backoff_multiplier: f32,
    #[serde(default)]
    pub last_retry_at: Option<u64>,
    #[serde(default)]
    pub consecutive_failures: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskHandle {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SubTaskStatus {
    Pending,
    Running,
    Done,
    Failed,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentRole {
    Scientist,
    Retriever,
    Synthesizer,
    Manager,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubTask {
    pub id: String,
    pub parent_task_id: String,
    pub description: String,
    pub status: SubTaskStatus,
    pub depends_on: Vec<String>,
    pub assigned_agent: AgentRole,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SuggestionType {
    ExpandNote,
    ConnectNotes,
    ResearchTopic,
    CreateMissingNote,
    SummarizeCluster,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSuggestion {
    pub id: String,
    pub suggestion_type: SuggestionType,
    pub title: String,
    pub description: String,
    pub relevance_score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerStats {
    pub queued: usize,
    pub running: usize,
    pub done: usize,
    pub failed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AiFaceMemoryRole {
    HumanTask,
    AiMemoryExpansion,
    OrchestratorVerification,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AiScientistVerificationState {
    NoClaims,
    NotRun,
    Passed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AiScientistConfidenceBasis {
    None,
    EvidenceFallback,
    KernelVerification,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AiScientistVerificationSummary {
    pub state: AiScientistVerificationState,
    pub passed: Option<bool>,
    pub violation_count: usize,
    pub overall_confidence: f32,
    pub confidence_basis: AiScientistConfidenceBasis,
    pub evidence_chain_count: usize,
    pub kernel_name: String,
    pub kernel_scope: String,
    pub is_truth_proof: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AiFaceDataFlow {
    pub task_id: String,
    pub manager_status: String,
    pub manager_phase: SynthesizePhase,
    pub memory_role: AiFaceMemoryRole,
    pub manager_has_trace: bool,
    pub orchestrator_enabled: bool,
    pub scientist_claim_count: usize,
    pub scientist_source_count: usize,
    pub scientist_verification: AiScientistVerificationSummary,
    pub context_fragment_count: usize,
    pub subagent_result_count: usize,
    pub sandbox_summary: Option<String>,
    pub material_packet_focus: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AiManagerControlAction {
    OrchestratorTrackPending,
    BridgeReviewPending,
    RunningTasks,
    DispatchReady,
    Idle,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AiManagerSnapshot {
    pub total_tasks: usize,
    pub pending_review_count: usize,
    pub execution_queue_count: usize,
    pub running_count: usize,
    pub completed_count: usize,
    pub failed_count: usize,
    pub human_task_count: usize,
    pub memory_expansion_count: usize,
    pub verification_track_count: usize,
    pub bridge_required_count: usize,
    pub read_only_count: usize,
    pub write_capable_count: usize,
    pub network_enabled_count: usize,
    pub scientist_payload_count: usize,
    pub orchestrator_packet_count: usize,
    pub latest_control_action: AiManagerControlAction,
}

/// Background agent task scheduler
///
/// Architecture:
///   CLI Input → parse → AgentTask → Scheduler queue → Worker → LLM → Result
///
/// Tasks run asynchronously on tokio runtime. Status updates are broadcast
/// to the frontend via Tauri events (Stage 4 full implementation).
pub struct AgentScheduler {
    /// Pending approval queue (tasks awaiting human sign-off)
    pending_queue: VecDeque<AgentTask>,
    /// High priority queue (always processed first)
    high_queue: VecDeque<AgentTask>,
    /// Standard queue
    queue: VecDeque<AgentTask>,
    /// All tasks indexed by ID
    tasks: HashMap<String, AgentTask>,
    /// Max concurrent running tasks
    max_concurrent: usize,
    /// Currently running task count
    running_count: usize,
    /// Task completion channel for status updates
    status_tx: mpsc::UnboundedSender<TaskStatusUpdate>,
    status_rx: mpsc::UnboundedReceiver<TaskStatusUpdate>,
    /// CancellationToken map for interrupting in-flight HTTP calls
    cancel_tokens: HashMap<String, CancellationToken>,
    pub subagent: Option<Subagent>,
    pub orchestrator: Option<Orchestrator>,
    workflow_runtime_events: Vec<HarnessEvent>,
    workflow_replan_request_count: usize,
    workflow_event_root: Option<PathBuf>,
    workflow_event_log_path: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TaskStatusUpdate {
    pub task_id: String,
    pub status: TaskStatus,
}

impl AgentScheduler {
    /// Create a new scheduler
    pub fn new(max_concurrent: usize) -> Self {
        let (status_tx, status_rx) = mpsc::unbounded_channel();
        AgentScheduler {
            pending_queue: VecDeque::new(),
            high_queue: VecDeque::new(),
            queue: VecDeque::new(),
            tasks: HashMap::new(),
            max_concurrent,
            running_count: 0,
            status_tx,
            status_rx,
            cancel_tokens: HashMap::new(),
            subagent: None,
            orchestrator: Some(Orchestrator::new(max_concurrent)),
            workflow_runtime_events: Vec::new(),
            workflow_replan_request_count: 0,
            workflow_event_root: None,
            workflow_event_log_path: None,
        }
    }

    /// Submit a task to the scheduler (goes to pending queue awaiting approval)
    pub fn submit(&mut self, task: AgentTask) -> TaskHandle {
        let handle = TaskHandle {
            id: task.id.clone(),
        };
        let mut pending_task = task;
        pending_task.status = TaskStatus::Pending;
        pending_task.priority_score = self.calculate_priority(&pending_task, &None);
        if pending_task.sub_tasks.is_empty() {
            pending_task.sub_tasks = self.decompose_task(&pending_task);
        }
        self.tasks
            .insert(pending_task.id.clone(), pending_task.clone());
        self.pending_queue.push_back(pending_task);
        handle
    }

    /// Approve a pending task — moves it to the execution queue
    pub fn approve(&mut self, task_id: &str, checked_by: &str) -> Result<(), String> {
        self.pending_queue.retain(|t| t.id != task_id);

        if let Some(task) = self.tasks.get_mut(task_id) {
            if matches!(task.status, TaskStatus::Pending) {
                task.status = TaskStatus::Approved {
                    checked_at: now_millis(),
                    checked_by: checked_by.to_string(),
                };
                let approved_task = task.clone();
                match approved_task.priority {
                    TaskPriority::High => self.high_queue.push_back(approved_task),
                    _ => self.queue.push_back(approved_task),
                }
                return Ok(());
            }
        }

        Err(format!(
            "Task {} not found or not in Pending state",
            task_id
        ))
    }

    /// Reject a pending task
    pub fn reject(&mut self, task_id: &str) -> Result<(), String> {
        self.pending_queue.retain(|t| t.id != task_id);

        if let Some(task) = self.tasks.get_mut(task_id) {
            if matches!(task.status, TaskStatus::Pending) {
                task.status = TaskStatus::Cancelled;
                let _ = self.status_tx.send(TaskStatusUpdate {
                    task_id: task_id.to_string(),
                    status: TaskStatus::Cancelled,
                });
                return Ok(());
            }
        }

        Err(format!(
            "Task {} not found or not in Pending state",
            task_id
        ))
    }

    /// Dequeue the next task (high priority first, only Approved tasks)
    pub fn dequeue(&mut self) -> Option<AgentTask> {
        if self.running_count >= self.max_concurrent {
            return None;
        }

        self.high_queue
            .make_contiguous()
            .sort_by_key(|t| std::cmp::Reverse(t.priority_score));
        self.queue
            .make_contiguous()
            .sort_by_key(|t| std::cmp::Reverse(t.priority_score));

        let task = self
            .high_queue
            .pop_front()
            .or_else(|| self.queue.pop_front());

        if let Some(ref t) = task {
            if !matches!(t.status, TaskStatus::Approved { .. }) {
                return None;
            }
            self.running_count += 1;
            self.update_task_status(
                &t.id,
                TaskStatus::Running {
                    started_at: now_millis(),
                    progress: 0.0,
                },
            );
        }

        task
    }

    /// Mark a task as completed
    pub fn complete(&mut self, task_id: &str, result: String) {
        self.complete_with_context_and_dream_snapshot(task_id, result, None, None);
    }

    pub fn complete_with_dream_snapshot(
        &mut self,
        task_id: &str,
        result: String,
        dream_snapshot: Option<DreamAuditSnapshot>,
    ) {
        self.complete_with_context_and_dream_snapshot(task_id, result, None, dream_snapshot);
    }

    pub fn complete_with_context_and_dream_snapshot(
        &mut self,
        task_id: &str,
        result: String,
        task_context: Option<&TaskContext>,
        dream_snapshot: Option<DreamAuditSnapshot>,
    ) {
        self.running_count = self.running_count.saturating_sub(1);
        self.update_task_status(
            task_id,
            TaskStatus::Done {
                completed_at: now_millis(),
                result,
            },
        );
        self.record_completion_context(task_id, task_context);

        if let Some(ref mut orch) = self.orchestrator {
            if self.tasks.get(task_id).is_some() {
                let task_clone = self.tasks.get(task_id).unwrap().clone();
                let audit_result =
                    orch.audit_epoch_with_dream(task_id, &task_clone, dream_snapshot);
                let audit_action = audit_result.action.clone();
                let audit_metrics = audit_result.metrics.clone();
                if let Some(t) = self.tasks.get_mut(task_id) {
                    t.regression_metrics = Some(audit_metrics);
                }
                if matches!(
                    audit_action,
                    AuditAction::Degraded | AuditAction::Terminated
                ) {
                    let track_tasks = if let Some(t) = self.tasks.get_mut(task_id) {
                        t.consecutive_failures = 0;
                        orch.degrade_agent(task_id);
                        let knowledge = Scientist::extract_knowledge(t);
                        let packets = orch.spawn_parallel_track_packets(&knowledge, t);
                        packets
                            .into_iter()
                            .map(|packet| Self::track_task_from_packet(t, packet))
                            .collect::<Vec<_>>()
                    } else {
                        Vec::new()
                    };

                    for new_task in track_tasks {
                        self.tasks.insert(new_task.id.clone(), new_task.clone());
                        self.high_queue.push_back(new_task);
                    }
                }
            }
        }
    }

    fn record_completion_context(&mut self, task_id: &str, task_context: Option<&TaskContext>) {
        if let Some(task) = self.tasks.get_mut(task_id) {
            for sub_task in &mut task.sub_tasks {
                if matches!(
                    sub_task.status,
                    SubTaskStatus::Pending | SubTaskStatus::Running
                ) {
                    sub_task.status = SubTaskStatus::Done;
                }
            }
            task.has_trace = true;

            if let Some(context) = task_context {
                let retrieval_results = context
                    .retrieval_evidence
                    .iter()
                    .map(Self::subagent_result_from_task_evidence)
                    .collect::<Vec<_>>();
                for result in retrieval_results {
                    task.subagent_results.retain(|existing| {
                        !(existing.source == result.source
                            && existing.hop == result.hop
                            && existing.generated_keywords == result.generated_keywords)
                    });
                    task.subagent_results.push(result);
                }

                let key = format!("task.{}.trace_context", task_id);
                if let Ok(value) = serde_json::to_value(context) {
                    let fragment = ContextFragment {
                        id: format!("{}_trace_context", task_id),
                        key: key.clone(),
                        value: value.clone(),
                        source: ContextSource::Agent,
                        layer: ContextLayer::Transient,
                        created_at: now_millis(),
                        ttl: None,
                        hash: ContextFragment::compute_hash(&key, &value),
                    };
                    task.context_fragments.retain(|frag| frag.key != key);
                    task.context_fragments.push(fragment);
                }
            }
        }
    }

    pub(crate) fn subagent_result_from_task_evidence(
        evidence: &super::research_trace::TaskEvidence,
    ) -> SubagentResult {
        let entries = evidence
            .entries
            .iter()
            .map(|entry| super::subagent::SubagentEntry {
                title: entry.title.clone(),
                snippet: entry.snippet.clone(),
                url: entry.url.clone(),
                authors: entry.authors.clone(),
                year: entry.year,
                source: entry.source.clone(),
                relevance_score: entry.relevance_score,
            })
            .collect::<Vec<_>>();

        SubagentResult {
            source: Self::data_source_from_evidence_label(&evidence.source),
            entries,
            hop: evidence.hop,
            generated_keywords: evidence.generated_keywords.clone(),
            total_found: evidence.total_found,
            graph_manifest: None,
        }
    }

    pub(crate) fn data_source_from_evidence_label(source: &str) -> DataSource {
        let normalized = source.to_ascii_lowercase();
        if normalized.contains("semantic") || normalized.contains("s2") {
            DataSource::SemanticScholar
        } else if normalized.contains("arxiv") {
            DataSource::Arxiv
        } else if normalized.contains("web") {
            DataSource::WebSearch
        } else {
            DataSource::LocalVector
        }
    }

    /// Mark a task as failed
    pub fn fail(&mut self, task_id: &str, error: String) {
        self.running_count = self.running_count.saturating_sub(1);

        // Check if retry is possible
        if let Some(task) = self.tasks.get(task_id) {
            if task.retry_count < task.max_retries {
                // Compute exponential backoff delay
                let delay = task.retry_delay_ms as f64
                    * task.retry_backoff_multiplier.powf(task.retry_count as f32) as f64;
                let _delay_ms = delay as u64;

                // Check cooldown before retry
                if let Some(ref orchestrator) = self.orchestrator {
                    if orchestrator.is_in_cooldown(task_id) {
                        return;
                    }
                }

                let mut retry_task = task.clone();
                retry_task.retry_count += 1;
                retry_task.last_retry_at = Some(now_millis());
                retry_task.consecutive_failures += 1;

                // If consecutive failures >= 3, trigger orchestrator audit
                if retry_task.consecutive_failures >= 3 {
                    if let Some(ref mut orchestrator) = self.orchestrator {
                        let result = orchestrator.audit_epoch(&retry_task.id, &retry_task);
                        if result.action == AuditAction::Cooldown {
                            if let Some(t) = self.tasks.get_mut(task_id) {
                                t.last_retry_at = retry_task.last_retry_at;
                                t.consecutive_failures = retry_task.consecutive_failures;
                            }
                            return;
                        }
                    }
                }

                retry_task.status = TaskStatus::Approved {
                    checked_at: now_millis(),
                    checked_by: "auto-retry".to_string(),
                };
                retry_task.priority = TaskPriority::Low; // Downgrade on retry
                self.queue.push_back(retry_task);
                if let Some(queued_retry) = self.queue.back().cloned() {
                    self.tasks.insert(task_id.to_string(), queued_retry);
                }
                return;
            }
        }

        self.update_task_status(
            task_id,
            TaskStatus::Error {
                message: error.clone(),
            },
        );

        if let Some(ref mut orch) = self.orchestrator {
            if self.tasks.get(task_id).is_some() {
                let task_clone = self.tasks.get(task_id).unwrap().clone();
                let audit_result = orch.audit_epoch(task_id, &task_clone);
                if matches!(
                    audit_result.action,
                    AuditAction::Degraded | AuditAction::Terminated
                ) {
                    if let Some(t) = self.tasks.get_mut(task_id) {
                        t.regression_metrics = Some(audit_result.metrics);
                    }
                }
            }
        }
    }

    /// Cancel a pending, approved, or running task
    pub fn cancel(&mut self, task_id: &str) -> bool {
        // Remove from all queues
        self.pending_queue.retain(|t| t.id != task_id);
        self.high_queue.retain(|t| t.id != task_id);
        self.queue.retain(|t| t.id != task_id);

        // Signal cancellation to interrupt in-flight HTTP calls
        if let Some(token) = self.cancel_tokens.remove(task_id) {
            token.cancel();
        }

        if let Some(task) = self.tasks.get(task_id) {
            if let TaskStatus::Running { .. } = task.status {
                self.running_count = self.running_count.saturating_sub(1);
            }
        }

        self.update_task_status(task_id, TaskStatus::Cancelled);
        true
    }

    /// Get task status
    pub fn status(&self, task_id: &str) -> Option<&TaskStatus> {
        self.tasks.get(task_id).map(|t| &t.status)
    }

    /// Get scheduler statistics
    pub fn stats(&self) -> SchedulerStats {
        let mut stats = SchedulerStats {
            queued: self.pending_queue.len() + self.high_queue.len() + self.queue.len(),
            running: 0,
            done: 0,
            failed: 0,
        };
        for task in self.tasks.values() {
            match task.status {
                TaskStatus::Pending | TaskStatus::Approved { .. } | TaskStatus::Queued => {}
                TaskStatus::Running { .. } => stats.running += 1,
                TaskStatus::Done { .. } => stats.done += 1,
                TaskStatus::Error { .. } => stats.failed += 1,
                TaskStatus::Cancelled => {}
            }
        }
        stats
    }

    /// List all tasks, optionally filtered by status
    pub fn list_tasks(&self, status_filter: Option<&str>) -> Vec<&AgentTask> {
        self.tasks
            .values()
            .filter(|t| {
                if let Some(filter) = status_filter {
                    matches!(
                        (filter, &t.status),
                        ("pending", TaskStatus::Pending)
                            | ("approved", TaskStatus::Approved { .. })
                            | ("running", TaskStatus::Running { .. })
                            | ("done", TaskStatus::Done { .. })
                            | ("error", TaskStatus::Error { .. })
                            | ("cancelled", TaskStatus::Cancelled)
                    )
                } else {
                    true
                }
            })
            .collect()
    }

    pub fn list_ai_face_data_flows(&self) -> Vec<AiFaceDataFlow> {
        let mut tasks = self.tasks.values().collect::<Vec<_>>();
        tasks.sort_by(|a, b| {
            a.created_at
                .cmp(&b.created_at)
                .then_with(|| a.id.cmp(&b.id))
        });
        tasks
            .into_iter()
            .filter_map(|task| self.ai_face_data_flow(&task.id))
            .collect()
    }

    pub fn ai_manager_snapshot(&self) -> AiManagerSnapshot {
        let mut running_count = 0;
        let mut completed_count = 0;
        let mut failed_count = 0;
        let mut human_task_count = 0;
        let mut memory_expansion_count = 0;
        let mut verification_track_count = 0;
        let mut bridge_required_count = 0;
        let mut read_only_count = 0;
        let mut write_capable_count = 0;
        let mut network_enabled_count = 0;
        let mut scientist_payload_count = 0;
        let mut orchestrator_packet_count = 0;

        for task in self.tasks.values() {
            match task.status {
                TaskStatus::Running { .. } => running_count += 1,
                TaskStatus::Done { .. } => completed_count += 1,
                TaskStatus::Error { .. } => failed_count += 1,
                _ => {}
            }

            match memory_role_for_task(task) {
                AiFaceMemoryRole::HumanTask => human_task_count += 1,
                AiFaceMemoryRole::AiMemoryExpansion => memory_expansion_count += 1,
                AiFaceMemoryRole::OrchestratorVerification => verification_track_count += 1,
            }

            if let Some(policy) = task.sandbox_policy.as_ref() {
                if policy.requires_bridge {
                    bridge_required_count += 1;
                }
                if policy.write_roots.is_empty() {
                    read_only_count += 1;
                } else {
                    write_capable_count += 1;
                }
                if policy.network_policy != NetworkPolicy::Disabled {
                    network_enabled_count += 1;
                }
            }

            scientist_payload_count += task.context_fragments.len() + task.subagent_results.len();
            if task.material_packet.is_some() {
                orchestrator_packet_count += 1;
            }
        }

        AiManagerSnapshot {
            total_tasks: self.tasks.len(),
            pending_review_count: self.pending_queue.len(),
            execution_queue_count: self.high_queue.len() + self.queue.len(),
            running_count,
            completed_count,
            failed_count,
            human_task_count,
            memory_expansion_count,
            verification_track_count,
            bridge_required_count,
            read_only_count,
            write_capable_count,
            network_enabled_count,
            scientist_payload_count,
            orchestrator_packet_count,
            latest_control_action: self.latest_manager_control_action(),
        }
    }

    fn latest_manager_control_action(&self) -> AiManagerControlAction {
        if self.high_queue.iter().any(|task| {
            task.material_packet.is_some()
                || task.card_type.as_deref() == Some("orchestrator-track")
        }) {
            AiManagerControlAction::OrchestratorTrackPending
        } else if !self.pending_queue.is_empty() {
            AiManagerControlAction::BridgeReviewPending
        } else if self.running_count > 0 {
            AiManagerControlAction::RunningTasks
        } else if !self.high_queue.is_empty() || !self.queue.is_empty() {
            AiManagerControlAction::DispatchReady
        } else {
            AiManagerControlAction::Idle
        }
    }

    /// Get status update receiver for external listeners
    pub fn status_receiver(&mut self) -> &mut mpsc::UnboundedReceiver<TaskStatusUpdate> {
        &mut self.status_rx
    }

    pub fn set_subagent(&mut self, subagent: Subagent) {
        self.subagent = Some(subagent);
    }

    pub fn set_workflow_event_root(&mut self, root: impl AsRef<Path>) {
        self.workflow_event_root = Some(root.as_ref().to_path_buf());
    }

    pub fn orchestrator_status(&self) -> Option<OrchestratorStatus> {
        self.orchestrator.as_ref().map(|o| {
            let mut status = o.status();
            self.apply_runtime_track_status(&mut status);
            self.apply_workflow_runtime_events(&mut status);
            status
        })
    }

    pub fn orchestrator_events(&self) -> Vec<crate::harness::orchestrator::OrchestratorEvent> {
        self.orchestrator
            .as_ref()
            .map(|o| o.event_log.clone())
            .unwrap_or_default()
    }

    pub fn build_orchestrator_replan_request(
        &mut self,
        agent_id: &str,
        goal: &GoalContract,
        workflow: &WorkflowIr,
        run: &WorkflowRunState,
        reports: &[StepReport],
        findings: &[VerificationFinding],
        registry: &AgentRegistry,
    ) -> OrchestratorReplanRequest {
        if let Some(orchestrator) = self.orchestrator.as_mut() {
            orchestrator.record_verification_findings(agent_id, findings);
        }

        let request = OrchestratorReplanRequest::from_runtime(
            goal, workflow, run, reports, findings, registry,
        );
        let event_chain =
            WorkflowRuntimeEventChain::from_replan_request(&request, now_millis().to_string());
        self.record_workflow_event_chain(&event_chain);

        request
    }

    pub fn prepare_workflow_subagent_jobs(
        &mut self,
        workflow: &WorkflowIr,
        run: &WorkflowRunState,
        registry: &AgentRegistry,
    ) -> Result<Vec<ControlledSubagentJob>, WorkflowError> {
        let dispatches = run.ready_dispatches(workflow, registry)?;
        let event_chain = WorkflowRuntimeEventChain::from_step_dispatches(
            &dispatches,
            now_millis().to_string(),
        );
        self.record_workflow_event_chain(&event_chain);
        Ok(dispatches
            .into_iter()
            .map(ControlledSubagentJob::from_dispatch)
            .collect())
    }

    fn apply_runtime_track_status(&self, status: &mut OrchestratorStatus) {
        let mut track_details = self
            .tasks
            .values()
            .filter_map(runtime_track_info_for_task)
            .collect::<Vec<_>>();

        if track_details.is_empty() {
            return;
        }

        track_details.sort_by(|a, b| a.track_id.cmp(&b.track_id));

        let material_packet_count = track_details.len();
        let completed_track_count = track_details
            .iter()
            .filter(|track| track.status == "completed")
            .count();
        let failed_track_count = track_details
            .iter()
            .filter(|track| track.status == "failed")
            .count();
        let cancelled_track_count = track_details
            .iter()
            .filter(|track| track.status == "cancelled")
            .count();
        let active_track_count = material_packet_count
            .saturating_sub(completed_track_count + failed_track_count + cancelled_track_count);

        status.material_packet_count = material_packet_count;
        status.active_track_count = active_track_count;
        status.completed_track_count = completed_track_count;
        status.failed_track_count = failed_track_count;
        status.cancelled_track_count = cancelled_track_count;
        status.track_count = active_track_count;
        status.track_details = track_details;
    }

    fn apply_workflow_runtime_events(&self, status: &mut OrchestratorStatus) {
        status.workflow_event_count = self.workflow_runtime_events.len();
        status.workflow_replan_request_count = self.workflow_replan_request_count;
        status.recent_workflow_events = self
            .workflow_runtime_events
            .iter()
            .rev()
            .take(20)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        status.workflow_event_log_path = self.workflow_event_log_path.clone();
    }

    pub(crate) fn record_workflow_event_chain(&mut self, chain: &WorkflowRuntimeEventChain) {
        if chain.replan_requested {
            self.workflow_replan_request_count += 1;
        }
        self.workflow_runtime_events.extend(chain.events.clone());
        const MAX_WORKFLOW_RUNTIME_EVENTS: usize = 50;
        if self.workflow_runtime_events.len() > MAX_WORKFLOW_RUNTIME_EVENTS {
            let overflow = self.workflow_runtime_events.len() - MAX_WORKFLOW_RUNTIME_EVENTS;
            self.workflow_runtime_events.drain(0..overflow);
        }
        if let Some(root) = self.workflow_event_root.as_ref() {
            if let Ok(path) = WorkflowRuntimeEventStore::append_chain(root, chain) {
                self.workflow_event_log_path = Some(path.to_string_lossy().to_string());
            }
        }
    }

    pub fn record_workflow_patch_decision(
        &mut self,
        workflow_id: &str,
        run_id: &str,
        patch: &WorkflowPatch,
        accepted: bool,
        reason: &str,
    ) {
        let event_chain = WorkflowRuntimeEventChain::from_patch_decision(
            workflow_id,
            run_id,
            patch,
            accepted,
            now_millis().to_string(),
            reason,
        );
        self.record_workflow_event_chain(&event_chain);
    }

    pub fn record_workflow_patch_review_request(
        &mut self,
        run_id: &str,
        patch: &WorkflowPatch,
        proposal_id: &str,
    ) {
        let event_chain = WorkflowRuntimeEventChain::from_patch_review_request(
            run_id,
            patch,
            proposal_id,
            now_millis().to_string(),
        );
        self.record_workflow_event_chain(&event_chain);
    }

    pub fn workflow_runtime_events_for_run(
        &self,
        run_id: &str,
        limit: usize,
    ) -> Result<Vec<HarnessEvent>, String> {
        if let Some(root) = self.workflow_event_root.as_ref() {
            return WorkflowRuntimeEventStore::read_recent(root, run_id, limit)
                .map_err(|err| err.to_string());
        }

        let mut events = self
            .workflow_runtime_events
            .iter()
            .filter(|event| {
                event
                    .attributes
                    .get("run_id")
                    .and_then(|value| value.as_str())
                    == Some(run_id)
            })
            .cloned()
            .collect::<Vec<_>>();
        if limit > 0 && events.len() > limit {
            Ok(events.split_off(events.len() - limit))
        } else {
            Ok(events)
        }
    }

    pub fn terminate_agent(&mut self, agent_id: &str) -> bool {
        self.orchestrator
            .as_mut()
            .map(|o| o.terminate_agent(agent_id))
            .unwrap_or(false)
    }

    pub fn reinstate_agent(&mut self, agent_id: &str) -> bool {
        self.orchestrator
            .as_mut()
            .map(|o| o.reinstate_agent(agent_id))
            .unwrap_or(false)
    }

    pub fn build_subagent_job(&self, task: &AgentTask) -> SubagentJob {
        let mut keywords: Vec<String> = Vec::new();

        match &task.command {
            CliCommand::Search { query, .. } => {
                keywords.push(query.clone());
            }
            CliCommand::Explain { concept, .. } | CliCommand::DeepDive { concept, .. } => {
                keywords.push(concept.clone());
            }
            CliCommand::Summarize { target, .. } => {
                keywords.push(target.clone());
            }
            CliCommand::FetchPapers { topic, .. } => {
                keywords.push(topic.clone());
            }
            _ => {}
        }

        let data_sources = match task.task_type {
            TaskType::Search | TaskType::Summarize => {
                vec![DataSource::LocalVector, DataSource::WebSearch]
            }
            TaskType::FetchPapers | TaskType::DeepDive | TaskType::Explain => {
                vec![
                    DataSource::LocalVector,
                    DataSource::WebSearch,
                    DataSource::Arxiv,
                    DataSource::SemanticScholar,
                ]
            }
            _ => vec![DataSource::LocalVector],
        };

        let job = SubagentJob {
            search_type: SearchType::All,
            keywords,
            data_sources,
            max_results_per_source: 10,
            max_hops: 3,
            current_hop: 0,
        };

        if let Some(policy) = task.sandbox_policy.as_ref() {
            job.filtered_by_policy(policy)
        } else {
            job
        }
    }

    pub fn track_task_from_packet(
        parent: &AgentTask,
        packet: OrchestratorMaterialPacket,
    ) -> AgentTask {
        let packet_sub_tasks = packet
            .claims
            .iter()
            .enumerate()
            .map(|(index, claim)| SubTask {
                id: format!("{}_claim_{}", packet.track_id, index),
                parent_task_id: packet.track_id.clone(),
                description: claim.clone(),
                status: SubTaskStatus::Done,
                depends_on: vec![],
                assigned_agent: AgentRole::Scientist,
            })
            .collect();

        AgentTask {
            id: packet.track_id.clone(),
            command: parent.command.clone(),
            task_type: parent.task_type.clone(),
            task_intent: Some(TaskIntentType::Verify),
            sandbox_policy: Some(packet.sandbox_policy.clone()),
            priority: TaskPriority::Low,
            priority_score: 0,
            status: TaskStatus::Approved {
                checked_at: now_millis(),
                checked_by: "orchestrator".to_string(),
            },
            regression_metrics: None,
            anchor_note: parent.anchor_note.clone(),
            created_at: now_millis(),
            max_retries: parent.max_retries,
            retry_count: 0,
            synthesize_phase: SynthesizePhase::Idle,
            subagent_results: parent.subagent_results.clone(),
            graph_manifest: parent.graph_manifest.clone(),
            has_trace: false,
            source_block_id: parent.source_block_id.clone(),
            card_id: parent.card_id.clone(),
            card_type: Some("orchestrator-track".to_string()),
            prompt: Some(packet.prompt.clone()),
            params: parent.params.clone(),
            context_note: parent.context_note.clone(),
            intent: format!("track: {}", packet.focus),
            content: packet.prompt.clone(),
            max_iterations: parent.max_iterations,
            sub_tasks: packet_sub_tasks,
            material_packet: Some(packet),
            context_fragments: parent.context_fragments.clone(),
            retry_delay_ms: 1000,
            retry_backoff_multiplier: 2.0,
            last_retry_at: None,
            consecutive_failures: 0,
        }
    }

    /// Create a CancellationToken for a task about to run.
    /// Stores it internally so cancel() can signal it later.
    pub fn create_cancel_token(&mut self, task_id: &str) -> CancellationToken {
        let token = CancellationToken::new();
        self.cancel_tokens
            .insert(task_id.to_string(), token.clone());
        token
    }

    pub fn calculate_priority(&self, task: &AgentTask, active_note: &Option<String>) -> u8 {
        let now = now_millis();
        let urgency = ((now.saturating_sub(task.created_at)).min(3_600_000) as f32 / 3_600_000.0
            * 30.0) as u8;
        let relevance = if let (Some(anchor), Some(active)) = (&task.anchor_note, active_note) {
            if anchor == active {
                25
            } else {
                0
            }
        } else {
            0
        };
        let user_waiting: u8 = 20;
        let score = urgency + relevance + user_waiting;
        score.min(100)
    }

    pub fn resort_queues(&mut self, active_note: &Option<String>) {
        let active = active_note.clone();
        for task in self.high_queue.iter_mut() {
            let now = now_millis();
            let urgency = ((now.saturating_sub(task.created_at)).min(3_600_000) as f32
                / 3_600_000.0
                * 30.0) as u8;
            let relevance = if let (Some(anchor), Some(a)) = (&task.anchor_note, &active) {
                if anchor == a {
                    25
                } else {
                    0
                }
            } else {
                0
            };
            task.priority_score = (urgency + relevance + 20).min(100);
        }
        for task in self.queue.iter_mut() {
            let now = now_millis();
            let urgency = ((now.saturating_sub(task.created_at)).min(3_600_000) as f32
                / 3_600_000.0
                * 30.0) as u8;
            let relevance = if let (Some(anchor), Some(a)) = (&task.anchor_note, &active) {
                if anchor == a {
                    25
                } else {
                    0
                }
            } else {
                0
            };
            task.priority_score = (urgency + relevance + 20).min(100);
        }
    }

    pub fn decompose_task(&self, task: &AgentTask) -> Vec<SubTask> {
        let parent_id = task.id.clone();
        match &task.command {
            crate::cli::parser::CliCommand::DeepResearch { .. } => {
                vec![
                    SubTask {
                        id: format!("{}_0", parent_id),
                        parent_task_id: parent_id.clone(),
                        description: "Search sources".to_string(),
                        status: SubTaskStatus::Pending,
                        depends_on: vec![],
                        assigned_agent: AgentRole::Retriever,
                    },
                    SubTask {
                        id: format!("{}_1", parent_id),
                        parent_task_id: parent_id.clone(),
                        description: "Filter relevant results".to_string(),
                        status: SubTaskStatus::Pending,
                        depends_on: vec![format!("{}_0", parent_id)],
                        assigned_agent: AgentRole::Scientist,
                    },
                    SubTask {
                        id: format!("{}_2", parent_id),
                        parent_task_id: parent_id.clone(),
                        description: "Analyze and synthesize".to_string(),
                        status: SubTaskStatus::Pending,
                        depends_on: vec![format!("{}_1", parent_id)],
                        assigned_agent: AgentRole::Synthesizer,
                    },
                    SubTask {
                        id: format!("{}_3", parent_id),
                        parent_task_id: parent_id.clone(),
                        description: "Generate hypotheses".to_string(),
                        status: SubTaskStatus::Pending,
                        depends_on: vec![format!("{}_2", parent_id)],
                        assigned_agent: AgentRole::Scientist,
                    },
                    SubTask {
                        id: format!("{}_4", parent_id),
                        parent_task_id: parent_id.clone(),
                        description: "Write final report".to_string(),
                        status: SubTaskStatus::Pending,
                        depends_on: vec![format!("{}_3", parent_id)],
                        assigned_agent: AgentRole::Synthesizer,
                    },
                ]
            }
            crate::cli::parser::CliCommand::FetchPapers { .. } => {
                vec![
                    SubTask {
                        id: format!("{}_0", parent_id),
                        parent_task_id: parent_id.clone(),
                        description: "Search arXiv".to_string(),
                        status: SubTaskStatus::Pending,
                        depends_on: vec![],
                        assigned_agent: AgentRole::Retriever,
                    },
                    SubTask {
                        id: format!("{}_1", parent_id),
                        parent_task_id: parent_id.clone(),
                        description: "Filter by relevance".to_string(),
                        status: SubTaskStatus::Pending,
                        depends_on: vec![format!("{}_0", parent_id)],
                        assigned_agent: AgentRole::Scientist,
                    },
                    SubTask {
                        id: format!("{}_2", parent_id),
                        parent_task_id: parent_id.clone(),
                        description: "Summarize papers".to_string(),
                        status: SubTaskStatus::Pending,
                        depends_on: vec![format!("{}_1", parent_id)],
                        assigned_agent: AgentRole::Synthesizer,
                    },
                ]
            }
            _ => {
                vec![SubTask {
                    id: format!("{}_0", parent_id),
                    parent_task_id: parent_id.clone(),
                    description: format!("Execute: {}", task.intent),
                    status: SubTaskStatus::Pending,
                    depends_on: vec![],
                    assigned_agent: AgentRole::Manager,
                }]
            }
        }
    }

    /// Internal status update
    fn update_task_status(&mut self, task_id: &str, status: TaskStatus) {
        if let Some(task) = self.tasks.get_mut(task_id) {
            task.status = status.clone();
        }
        let _ = self.status_tx.send(TaskStatusUpdate {
            task_id: task_id.to_string(),
            status,
        });
    }

    pub fn get_task(&self, task_id: &str) -> Option<&AgentTask> {
        self.tasks.get(task_id)
    }

    pub fn ai_face_data_flow(&self, task_id: &str) -> Option<AiFaceDataFlow> {
        let task = self.tasks.get(task_id)?;
        let knowledge = Scientist::extract_knowledge(task);

        Some(AiFaceDataFlow {
            task_id: task.id.clone(),
            manager_status: task_status_label(&task.status).to_string(),
            manager_phase: task.synthesize_phase.clone(),
            memory_role: memory_role_for_task(task),
            manager_has_trace: task.has_trace,
            orchestrator_enabled: self.orchestrator.is_some(),
            scientist_claim_count: knowledge.claims.len(),
            scientist_source_count: knowledge.sources.len(),
            scientist_verification: scientist_verification_summary(&knowledge),
            context_fragment_count: task.context_fragments.len(),
            subagent_result_count: task.subagent_results.len(),
            sandbox_summary: task.sandbox_policy.as_ref().map(SandboxPolicy::summary),
            material_packet_focus: task
                .material_packet
                .as_ref()
                .map(|packet| packet.focus.clone()),
        })
    }

    pub fn get_task_manifest(&self, task_id: &str) -> Result<&GraphManifest, String> {
        self.tasks
            .get(task_id)
            .and_then(|t| t.graph_manifest.as_ref())
            .ok_or_else(|| format!("No manifest for task: {}", task_id))
    }
}

fn task_status_label(status: &TaskStatus) -> &'static str {
    match status {
        TaskStatus::Pending => "pending",
        TaskStatus::Approved { .. } => "approved",
        TaskStatus::Queued => "queued",
        TaskStatus::Running { .. } => "running",
        TaskStatus::Done { .. } => "done",
        TaskStatus::Error { .. } => "error",
        TaskStatus::Cancelled => "cancelled",
    }
}

fn memory_role_for_task(task: &AgentTask) -> AiFaceMemoryRole {
    if task.material_packet.is_some() || task.card_type.as_deref() == Some("orchestrator-track") {
        AiFaceMemoryRole::OrchestratorVerification
    } else if task.intent.contains("dream")
        || task.card_type.as_deref() == Some("dream")
        || task.card_type.as_deref() == Some("memory-expansion")
    {
        AiFaceMemoryRole::AiMemoryExpansion
    } else {
        AiFaceMemoryRole::HumanTask
    }
}

fn runtime_track_info_for_task(task: &AgentTask) -> Option<TrackInfo> {
    let packet = task.material_packet.as_ref()?;
    Some(TrackInfo {
        track_id: task.id.clone(),
        focus: packet.focus.clone(),
        status: runtime_track_status_label(&task.status).to_string(),
        parent_agent: packet.parent_task_id.clone(),
        reason: Some(packet.instruction.clone()),
        claim_count: packet.claims.len(),
        source_ref_count: packet.source_refs.len(),
    })
}

fn runtime_track_status_label(status: &TaskStatus) -> &'static str {
    match status {
        TaskStatus::Pending => "pending",
        TaskStatus::Approved { .. } => "approved",
        TaskStatus::Queued => "queued",
        TaskStatus::Running { .. } => "running",
        TaskStatus::Done { .. } => "completed",
        TaskStatus::Error { .. } => "failed",
        TaskStatus::Cancelled => "cancelled",
    }
}

fn scientist_verification_summary(knowledge: &CleanKnowledge) -> AiScientistVerificationSummary {
    let has_claims = !knowledge.claims.is_empty();
    let confidence = if !has_claims {
        0.0
    } else if knowledge.confidence_map.is_empty() {
        0.5
    } else {
        knowledge
            .confidence_map
            .values()
            .copied()
            .fold(0.0_f32, f32::max)
            .clamp(0.0, 1.0)
    };

    AiScientistVerificationSummary {
        state: if has_claims {
            AiScientistVerificationState::NotRun
        } else {
            AiScientistVerificationState::NoClaims
        },
        passed: None,
        violation_count: 0,
        overall_confidence: confidence,
        confidence_basis: if has_claims {
            AiScientistConfidenceBasis::EvidenceFallback
        } else {
            AiScientistConfidenceBasis::None
        },
        evidence_chain_count: knowledge.claims.len(),
        kernel_name: PropositionKernel::NAME.to_string(),
        kernel_scope: if has_claims {
            "not_run".to_string()
        } else {
            "no_claims".to_string()
        },
        is_truth_proof: false,
    }
}

#[allow(dead_code)]
pub fn inject_manifest_into_prompt(manifest: &GraphManifest, base_prompt: &str) -> String {
    let manifest_toml = manifest.to_toml();
    format!(
        "## Knowledge Graph Context\n\
         The following is a structured overview of relevant notes in your vault:\n\n\
         {}\n\
         ## Tool: read_file\n\
         You may request the full content of any note by using the path shown above:\n\
         Call `read_file(\"path/to/note.md\")` to get the note's full content.\n\n\
         ---\n\n\
         {}",
        manifest_toml, base_prompt
    )
}

fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub fn analyze_activity(
    recently_edited: &[String],
    link_graph: &crate::graph::link_graph::LinkGraph,
    vault: Option<&crate::fs::vault::VaultManager>,
) -> Vec<TaskSuggestion> {
    let mut suggestions = Vec::new();

    for note_path in recently_edited.iter().take(5) {
        if let Some(v) = vault {
            if let Ok(content) = v.read_note(note_path) {
                if content.chars().count() < 500 {
                    suggestions.push(TaskSuggestion {
                        id: format!(
                            "sug_expand_{}",
                            uuid::Uuid::new_v4().to_string().replace('-', "")
                        ),
                        suggestion_type: SuggestionType::ExpandNote,
                        title: format!("Expand: {}", note_path),
                        description: format!(
                            "This note is short ({} chars). Generate more detailed content.",
                            content.len()
                        ),
                        relevance_score: 0.7,
                    });
                }
            }
        }

        let backlinks = link_graph.get_backlinks(note_path);
        if backlinks.len() >= 2 {
            suggestions.push(TaskSuggestion {
                id: format!(
                    "sug_connect_{}",
                    uuid::Uuid::new_v4().to_string().replace('-', "")
                ),
                suggestion_type: SuggestionType::ConnectNotes,
                title: format!("Connect: {}", note_path),
                description: format!(
                    "Found {} backlinks. Consider creating a hub note.",
                    backlinks.len()
                ),
                relevance_score: 0.6 + backlinks.len() as f32 * 0.05,
            });
        }
    }

    suggestions.sort_by(|a, b| {
        b.relevance_score
            .partial_cmp(&a.relevance_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    suggestions.truncate(5);
    suggestions
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::sandbox::SandboxPolicy;
    use crate::cli::parser::CliCommand;

    fn make_task(id: &str, priority: TaskPriority) -> AgentTask {
        AgentTask {
            id: id.to_string(),
            command: CliCommand::Status,
            task_type: TaskType::Search,
            task_intent: Some(TaskIntentType::Research),
            sandbox_policy: Some(TaskIntentType::Research.default_sandbox_policy()),
            priority,
            priority_score: 50,
            status: TaskStatus::Pending,
            anchor_note: None,
            created_at: now_millis(),
            max_retries: 1,
            retry_count: 0,
            synthesize_phase: SynthesizePhase::Idle,
            subagent_results: vec![],
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

    #[test]
    fn test_submit_goes_to_pending() {
        let mut sched = AgentScheduler::new(2);
        sched.submit(make_task("t1", TaskPriority::Medium));
        sched.submit(make_task("t2", TaskPriority::High));

        // Both should be Pending, not in dequeue queue
        assert!(matches!(sched.status("t1"), Some(TaskStatus::Pending)));
        assert!(matches!(sched.status("t2"), Some(TaskStatus::Pending)));
        assert!(sched.dequeue().is_none()); // Nothing approved yet
    }

    #[test]
    fn test_approve_and_dequeue() {
        let mut sched = AgentScheduler::new(2);
        sched.submit(make_task("t1", TaskPriority::Medium));
        sched.submit(make_task("t2", TaskPriority::High));
        sched.submit(make_task("t3", TaskPriority::Medium));

        // Approve tasks
        assert!(sched.approve("t1", "human").is_ok());
        assert!(sched.approve("t2", "human").is_ok());
        assert!(sched.approve("t3", "human").is_ok());

        // High priority first
        let t = sched.dequeue().unwrap();
        assert_eq!(t.id, "t2"); // High priority
        assert!(matches!(
            sched.status("t2"),
            Some(TaskStatus::Running { .. })
        ));

        let t = sched.dequeue().unwrap();
        assert_eq!(t.id, "t1"); // First medium

        // Max concurrent = 2
        assert!(sched.dequeue().is_none());
    }

    #[test]
    fn test_reject_task() {
        let mut sched = AgentScheduler::new(2);
        sched.submit(make_task("t1", TaskPriority::Medium));

        assert!(sched.reject("t1").is_ok());
        assert!(matches!(sched.status("t1"), Some(TaskStatus::Cancelled)));
    }

    #[test]
    fn test_approve_idempotent() {
        let mut sched = AgentScheduler::new(2);
        sched.submit(make_task("t1", TaskPriority::Medium));
        sched.approve("t1", "human").unwrap();

        // Second approve should fail (already approved)
        assert!(sched.approve("t1", "human").is_err());
    }

    #[test]
    fn test_complete_and_fail() {
        let mut sched = AgentScheduler::new(2);
        sched.submit(make_task("t1", TaskPriority::Medium));
        sched.approve("t1", "human").unwrap();

        let t = sched.dequeue().unwrap();
        sched.complete(&t.id, "done".to_string());

        match sched.status("t1") {
            Some(TaskStatus::Done { result, .. }) => assert_eq!(result, "done"),
            _ => panic!("Expected Done"),
        }
    }

    #[test]
    fn submit_decomposes_empty_task_for_audit_claims() {
        let mut sched = AgentScheduler::new(2);
        sched.submit(make_task("t1", TaskPriority::Medium));

        let stored = sched.get_task("t1").expect("task should be stored");
        assert!(!stored.sub_tasks.is_empty());
        assert_eq!(stored.sub_tasks[0].parent_task_id, "t1");
    }

    #[test]
    fn complete_records_trace_context_and_marks_subtasks_done() {
        let mut sched = AgentScheduler::new(2);
        let mut task = make_task("t1", TaskPriority::Medium);
        task.intent = "audit research".to_string();
        sched.submit(task);
        sched.approve("t1", "human").unwrap();

        let running = sched.dequeue().unwrap();
        let mut phase_timings = std::collections::HashMap::new();
        phase_timings.insert("retrieve_ms".to_string(), 12);
        let context = TaskContext {
            intent: "audit research".to_string(),
            ghost_ids: vec!["ghost_1".to_string()],
            phase_timings,
            ..TaskContext::default()
        };

        sched.complete_with_context_and_dream_snapshot(
            &running.id,
            "done".to_string(),
            Some(&context),
            None,
        );

        let stored = sched.get_task("t1").expect("task should be stored");
        assert!(stored.has_trace);
        assert!(stored
            .sub_tasks
            .iter()
            .all(|sub_task| matches!(sub_task.status, SubTaskStatus::Done)));
        let trace_fragment = stored
            .context_fragments
            .iter()
            .find(|fragment| fragment.key == "task.t1.trace_context")
            .expect("trace context should be recorded as evidence");
        assert_eq!(
            trace_fragment.source,
            crate::harness::context::ContextSource::Agent
        );
        assert_eq!(
            trace_fragment.value["intent"].as_str(),
            Some("audit research")
        );

        let knowledge = crate::harness::scientist::Scientist::extract_knowledge(stored);
        assert!(!knowledge.claims.is_empty());
        assert_eq!(knowledge.sources.len(), 1);
    }

    #[test]
    fn complete_converts_trace_retrieval_evidence_into_subagent_results() {
        let mut sched = AgentScheduler::new(2);
        let task = make_task("t1", TaskPriority::Medium);
        sched.submit(task);
        sched.approve("t1", "human").unwrap();

        let running = sched.dequeue().unwrap();
        let context = TaskContext {
            intent: "bayes search".to_string(),
            retrieval_evidence: vec![crate::ai::research_trace::TaskEvidence {
                source: "local_vector".to_string(),
                hop: 0,
                generated_keywords: vec!["bayes".to_string()],
                total_found: 1,
                entries: vec![crate::ai::research_trace::TaskEvidenceEntry {
                    title: "Bayes.md".to_string(),
                    snippet: "Bayesian evidence".to_string(),
                    url: None,
                    authors: vec![],
                    year: None,
                    source: "Bayes.md".to_string(),
                    relevance_score: 0.91,
                }],
            }],
            ..TaskContext::default()
        };

        sched.complete_with_context_and_dream_snapshot(
            &running.id,
            "done".to_string(),
            Some(&context),
            None,
        );

        let stored = sched.get_task("t1").expect("task should be stored");
        assert_eq!(stored.subagent_results.len(), 1);
        assert_eq!(stored.subagent_results[0].source, DataSource::LocalVector);
        assert_eq!(stored.subagent_results[0].total_found, 1);
        assert_eq!(stored.subagent_results[0].entries[0].title, "Bayes.md");

        let knowledge = crate::harness::scientist::Scientist::extract_knowledge(stored);
        assert_eq!(
            knowledge.confidence_map.get("Bayes.md").copied(),
            Some(0.91)
        );
    }

    #[test]
    fn ai_face_data_flow_exposes_manager_scientist_and_orchestrator_contract() {
        let mut sched = AgentScheduler::new(2);
        let mut task = make_task("t1", TaskPriority::Medium);
        task.intent = "audit research".to_string();
        sched.submit(task);
        sched.approve("t1", "human").unwrap();

        let running = sched.dequeue().unwrap();
        let context = TaskContext {
            intent: "audit research".to_string(),
            retrieval_evidence: vec![crate::ai::research_trace::TaskEvidence {
                source: "local_vector".to_string(),
                hop: 0,
                generated_keywords: vec!["bayes".to_string()],
                total_found: 1,
                entries: vec![crate::ai::research_trace::TaskEvidenceEntry {
                    title: "Bayes.md".to_string(),
                    snippet: "Bayesian evidence".to_string(),
                    url: None,
                    authors: vec![],
                    year: None,
                    source: "Bayes.md".to_string(),
                    relevance_score: 0.88,
                }],
            }],
            ..TaskContext::default()
        };

        sched.complete_with_context_and_dream_snapshot(
            &running.id,
            "done".to_string(),
            Some(&context),
            None,
        );

        let flow = sched
            .ai_face_data_flow("t1")
            .expect("AI face data flow should be available for stored tasks");
        assert_eq!(flow.task_id, "t1");
        assert_eq!(flow.manager_status, "done");
        assert_eq!(flow.memory_role, AiFaceMemoryRole::HumanTask);
        assert!(flow.manager_has_trace);
        assert!(flow.orchestrator_enabled);
        assert!(flow.scientist_claim_count > 0);
        assert_eq!(flow.scientist_source_count, 1);
        assert_eq!(
            flow.scientist_verification.state,
            AiScientistVerificationState::NotRun
        );
        assert_eq!(flow.scientist_verification.passed, None);
        assert_eq!(flow.scientist_verification.violation_count, 0);
        assert_eq!(
            flow.scientist_verification.evidence_chain_count,
            flow.scientist_claim_count
        );
        assert_eq!(
            flow.scientist_verification.confidence_basis,
            AiScientistConfidenceBasis::EvidenceFallback
        );
        assert_eq!(flow.scientist_verification.kernel_name, "PropositionKernel");
        assert_eq!(flow.scientist_verification.kernel_scope, "not_run");
        assert!(!flow.scientist_verification.is_truth_proof);
        assert!((flow.scientist_verification.overall_confidence - 0.88).abs() < f32::EPSILON);
        assert_eq!(flow.context_fragment_count, 1);
        assert_eq!(flow.subagent_result_count, 1);
        assert!(flow
            .sandbox_summary
            .as_deref()
            .unwrap_or_default()
            .contains("bridge=true"));
    }

    #[test]
    fn test_retry_on_failure() {
        let mut sched = AgentScheduler::new(2);
        let mut task = make_task("t1", TaskPriority::Medium);
        task.max_retries = 2;

        sched.submit(task);
        sched.approve("t1", "human").unwrap();
        let t = sched.dequeue().unwrap();
        sched.fail(&t.id, "temporary error".to_string());

        // Should be re-queued with low priority
        let retry = sched.dequeue().unwrap();
        assert_eq!(retry.id, "t1");
        assert!(matches!(retry.priority, TaskPriority::Low));
        assert_eq!(retry.retry_count, 1);
        assert!(matches!(
            sched.status("t1"),
            Some(TaskStatus::Running { .. })
        ));
    }

    #[test]
    fn test_cancel() {
        let mut sched = AgentScheduler::new(2);
        sched.submit(make_task("t1", TaskPriority::Medium));
        sched.approve("t1", "human").unwrap();
        sched.submit(make_task("t2", TaskPriority::Medium));
        sched.approve("t2", "human").unwrap();

        sched.cancel("t1");
        match sched.status("t1") {
            Some(TaskStatus::Cancelled) => {}
            _ => panic!("Expected Cancelled"),
        }

        // t2 should still be queued
        let t = sched.dequeue().unwrap();
        assert_eq!(t.id, "t2");
    }

    #[test]
    fn test_cancel_pending() {
        let mut sched = AgentScheduler::new(2);
        sched.submit(make_task("t1", TaskPriority::Medium));
        sched.cancel("t1");

        match sched.status("t1") {
            Some(TaskStatus::Cancelled) => {}
            _ => panic!("Expected Cancelled for pending task"),
        }
    }

    #[test]
    fn test_list_tasks() {
        let mut sched = AgentScheduler::new(2);
        sched.submit(make_task("t1", TaskPriority::Medium));
        sched.submit(make_task("t2", TaskPriority::High));
        sched.approve("t2", "human").unwrap();

        let pending = sched.list_tasks(Some("pending"));
        assert_eq!(pending.len(), 1);

        let approved = sched.list_tasks(Some("approved"));
        assert_eq!(approved.len(), 1);

        let all = sched.list_tasks(None);
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn list_ai_face_data_flows_orders_tasks_and_marks_memory_roles() {
        let mut sched = AgentScheduler::new(2);
        let mut human_task = make_task("z-human", TaskPriority::Medium);
        human_task.created_at = 300;
        let mut dream_task = make_task("a-dream", TaskPriority::Low);
        dream_task.created_at = 100;
        dream_task.intent = "dream memory expansion".to_string();
        dream_task.card_type = Some("dream".to_string());
        let parent = make_task("parent", TaskPriority::Medium);
        let packet = crate::harness::orchestrator::OrchestratorMaterialPacket {
            track_id: "parent_track_0".to_string(),
            parent_task_id: "parent".to_string(),
            focus: "correctness".to_string(),
            instruction: "Verify claims".to_string(),
            prompt: "Focus: correctness\nClaim A".to_string(),
            claims: vec!["Claim A".to_string()],
            source_refs: vec!["Bayes.md#intro".to_string()],
            sandbox_policy: crate::ai::task_intent::TaskIntentType::Verify.default_sandbox_policy(),
        };
        let mut track_task = AgentScheduler::track_task_from_packet(&parent, packet);
        track_task.created_at = 200;

        sched.submit(human_task);
        sched.submit(dream_task);
        sched.tasks.insert(track_task.id.clone(), track_task);

        let flows = sched.list_ai_face_data_flows();

        assert_eq!(
            flows
                .iter()
                .map(|flow| flow.task_id.as_str())
                .collect::<Vec<_>>(),
            vec!["a-dream", "parent_track_0", "z-human"]
        );
        assert_eq!(flows[0].memory_role, AiFaceMemoryRole::AiMemoryExpansion);
        assert_eq!(
            flows[1].memory_role,
            AiFaceMemoryRole::OrchestratorVerification
        );
        assert_eq!(
            flows[1].material_packet_focus.as_deref(),
            Some("correctness")
        );
        assert_eq!(flows[2].memory_role, AiFaceMemoryRole::HumanTask);
    }

    #[test]
    fn ai_manager_snapshot_exposes_control_surface_and_memory_outputs() {
        let mut sched = AgentScheduler::new(2);
        let pending_human = make_task("human", TaskPriority::Medium);
        let mut dream_task = make_task("dream", TaskPriority::Low);
        dream_task.task_intent = Some(TaskIntentType::Dream);
        dream_task.sandbox_policy = Some(TaskIntentType::Dream.default_sandbox_policy());
        dream_task.intent = "dream memory expansion".to_string();
        dream_task.card_type = Some("dream".to_string());

        sched.submit(pending_human);
        sched.submit(dream_task);
        sched.approve("dream", "human").unwrap();
        let running = sched.dequeue().unwrap();
        let context = TaskContext {
            intent: "dream memory expansion".to_string(),
            retrieval_evidence: vec![crate::ai::research_trace::TaskEvidence {
                source: "local_vector".to_string(),
                hop: 0,
                generated_keywords: vec!["dream".to_string()],
                total_found: 1,
                entries: vec![crate::ai::research_trace::TaskEvidenceEntry {
                    title: "Dream.md".to_string(),
                    snippet: "Dream insight".to_string(),
                    url: None,
                    authors: vec![],
                    year: None,
                    source: "Dream.md".to_string(),
                    relevance_score: 0.8,
                }],
            }],
            ..TaskContext::default()
        };
        sched.complete_with_context_and_dream_snapshot(
            &running.id,
            "done".to_string(),
            Some(&context),
            None,
        );

        let parent = make_task("parent", TaskPriority::Medium);
        let packet = crate::harness::orchestrator::OrchestratorMaterialPacket {
            track_id: "parent_track_0".to_string(),
            parent_task_id: "parent".to_string(),
            focus: "correctness".to_string(),
            instruction: "Verify claims".to_string(),
            prompt: "Focus: correctness\nClaim A".to_string(),
            claims: vec!["Claim A".to_string()],
            source_refs: vec!["Dream.md#claim".to_string()],
            sandbox_policy: crate::ai::task_intent::TaskIntentType::Verify.default_sandbox_policy(),
        };
        let track_task = AgentScheduler::track_task_from_packet(&parent, packet);
        sched.high_queue.push_back(track_task.clone());
        sched.tasks.insert(track_task.id.clone(), track_task);

        let snapshot = sched.ai_manager_snapshot();

        assert_eq!(snapshot.total_tasks, 3);
        assert_eq!(snapshot.pending_review_count, 1);
        assert_eq!(snapshot.execution_queue_count, 1);
        assert_eq!(snapshot.completed_count, 1);
        assert_eq!(snapshot.human_task_count, 1);
        assert_eq!(snapshot.memory_expansion_count, 1);
        assert_eq!(snapshot.verification_track_count, 1);
        assert_eq!(snapshot.bridge_required_count, 2);
        assert_eq!(snapshot.read_only_count, 2);
        assert_eq!(snapshot.write_capable_count, 1);
        assert_eq!(snapshot.network_enabled_count, 1);
        assert_eq!(snapshot.scientist_payload_count, 2);
        assert_eq!(snapshot.orchestrator_packet_count, 1);
        assert_eq!(
            snapshot.latest_control_action,
            AiManagerControlAction::OrchestratorTrackPending
        );
    }

    #[test]
    fn build_subagent_job_respects_task_sandbox_policy() {
        let sched = AgentScheduler::new(2);
        let mut task = make_task("t1", TaskPriority::Medium);
        task.command = CliCommand::FetchPapers {
            topic: "bayesian inference".to_string(),
            max: 5,
            link_to: None,
        };
        task.task_type = TaskType::FetchPapers;
        task.sandbox_policy = Some(SandboxPolicy::read_only(&["vector_search"]));

        let job = sched.build_subagent_job(&task);

        assert_eq!(job.data_sources, vec![DataSource::LocalVector]);
    }

    #[test]
    fn track_task_from_material_packet_preserves_packet_and_sandbox() {
        let parent = make_task("parent", TaskPriority::Medium);
        let packet = crate::harness::orchestrator::OrchestratorMaterialPacket {
            track_id: "parent_track_0".to_string(),
            parent_task_id: "parent".to_string(),
            focus: "correctness".to_string(),
            instruction: "Verify claims".to_string(),
            prompt: "Focus: correctness\nClaim A".to_string(),
            claims: vec!["Claim A".to_string()],
            source_refs: vec!["Bayes.md#intro".to_string()],
            sandbox_policy: crate::ai::task_intent::TaskIntentType::Verify.default_sandbox_policy(),
        };

        let task = AgentScheduler::track_task_from_packet(&parent, packet.clone());

        assert_eq!(task.id, "parent_track_0");
        assert_eq!(task.card_type.as_deref(), Some("orchestrator-track"));
        assert_eq!(task.prompt.as_deref(), Some(packet.prompt.as_str()));
        assert_eq!(task.content, packet.prompt);
        assert_eq!(task.sandbox_policy, Some(packet.sandbox_policy.clone()));
        assert_eq!(task.material_packet.as_ref().unwrap().focus, "correctness");
        assert_eq!(task.sub_tasks.len(), 1);
        assert_eq!(task.sub_tasks[0].description, "Claim A");
        assert!(matches!(task.sub_tasks[0].status, SubTaskStatus::Done));
        assert!(matches!(
            task.status,
            TaskStatus::Approved {
                checked_by,
                ..
            } if checked_by == "orchestrator"
        ));
    }

    #[test]
    fn complete_records_orchestrator_tracks_in_task_map() {
        let mut sched = AgentScheduler::new(4);
        let mut parent = make_task("parent", TaskPriority::Medium);
        parent.subagent_results = vec![
            SubagentResult {
                source: DataSource::LocalVector,
                entries: vec![],
                hop: 0,
                generated_keywords: vec![],
                total_found: 0,
                graph_manifest: None,
            },
            SubagentResult {
                source: DataSource::LocalVector,
                entries: vec![],
                hop: 1,
                generated_keywords: vec![],
                total_found: 0,
                graph_manifest: None,
            },
        ];

        sched.submit(parent);
        sched.approve("parent", "human").unwrap();
        if let Some(orch) = sched.orchestrator.as_mut() {
            orch.regression_thresholds.min_epoch_before_audit = 1;
            orch.regression_thresholds.auto_terminate = false;
        }

        let task = sched.dequeue().unwrap();
        sched.complete(&task.id, "done".to_string());

        let track_id = sched
            .high_queue
            .front()
            .expect("orchestrator should enqueue verification track")
            .id
            .clone();
        let stored = sched
            .tasks
            .get(&track_id)
            .expect("orchestrator track should be available by id");

        assert_eq!(stored.card_type.as_deref(), Some("orchestrator-track"));
        assert_eq!(stored.task_intent, Some(TaskIntentType::Verify));
        assert!(stored
            .sandbox_policy
            .as_ref()
            .unwrap()
            .allows_tool("proposition_kernel"));
    }

    #[test]
    fn orchestrator_status_uses_runtime_track_tasks_for_packet_and_active_counts() {
        let mut sched = AgentScheduler::new(4);
        let parent = make_task("parent", TaskPriority::Medium);

        for (index, (focus, status)) in [
            (
                "correctness",
                TaskStatus::Approved {
                    checked_at: 1,
                    checked_by: "orchestrator".to_string(),
                },
            ),
            (
                "consistency",
                TaskStatus::Done {
                    completed_at: 2,
                    result: "ok".to_string(),
                },
            ),
            (
                "verification",
                TaskStatus::Error {
                    message: "missing source".to_string(),
                },
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let packet = crate::harness::orchestrator::OrchestratorMaterialPacket {
                track_id: format!("parent_track_{}", index),
                parent_task_id: "parent".to_string(),
                focus: focus.to_string(),
                instruction: format!("Verify {}", focus),
                prompt: format!("Focus: {}", focus),
                claims: vec!["Claim A".to_string()],
                source_refs: vec!["Bayes.md#intro".to_string()],
                sandbox_policy: crate::ai::task_intent::TaskIntentType::Verify
                    .default_sandbox_policy(),
            };
            let mut track_task = AgentScheduler::track_task_from_packet(&parent, packet);
            track_task.status = status;
            sched.tasks.insert(track_task.id.clone(), track_task);
        }

        let status = sched
            .orchestrator_status()
            .expect("scheduler should expose orchestrator status");

        assert_eq!(status.track_event_count, 0);
        assert_eq!(status.material_packet_count, 3);
        assert_eq!(status.active_track_count, 1);
        assert_eq!(status.completed_track_count, 1);
        assert_eq!(status.failed_track_count, 1);
        assert_eq!(status.cancelled_track_count, 0);
        assert_eq!(status.track_count, status.active_track_count);
        assert_eq!(status.track_details.len(), 3);
        assert_eq!(status.track_details[0].track_id, "parent_track_0");
        assert_eq!(status.track_details[0].focus, "correctness");
        assert_eq!(status.track_details[0].status, "approved");
        assert_eq!(status.track_details[0].parent_agent, "parent");
        assert_eq!(status.track_details[0].claim_count, 1);
        assert_eq!(status.track_details[0].source_ref_count, 1);
    }

    #[test]
    fn workflow_replan_request_records_verifier_findings_in_orchestrator_status() {
        let mut sched = AgentScheduler::new(2);
        let event_root = tempfile::tempdir().unwrap();
        sched.set_workflow_event_root(event_root.path());
        let goal = crate::harness::workflow::GoalContract {
            goal_id: "goal_runtime".to_string(),
            goal_text: "Connect verifier findings to orchestrator diagnostics".to_string(),
            success_definition: vec![
                "Runtime replan request is schema-valid".to_string(),
                "Verifier findings are visible in orchestrator status".to_string(),
            ],
            non_goals: vec![],
            constraints: serde_json::json!({"allow_direct_file_edit": false}),
            context_scope: vec!["src-tauri/src/harness/**".to_string()],
            approval_policy: serde_json::json!({"bridge_required": true}),
            budget: serde_json::json!({"max_patch_chain": 3}),
            created_at: "2026-06-05T16:30:00Z".to_string(),
        };
        let registry = crate::harness::workflow::AgentRegistry::from_agents(vec![
            crate::harness::workflow::AgentRegistryEntry {
                agent_type: "code_writer".to_string(),
                allowed_tools: vec!["Read".to_string(), "Edit".to_string()],
                denied_tools: vec!["Shell".to_string()],
                default_mode: crate::harness::workflow::WorkflowStepMode::WritePatch,
                max_parallelism: 1,
                can_delegate: false,
            },
        ]);
        let workflow = crate::harness::workflow::WorkflowIr {
            workflow_id: "wf_runtime".to_string(),
            goal_id: "goal_runtime".to_string(),
            version: 1,
            parent_version: None,
            status: crate::harness::workflow::WorkflowStatus::Running,
            global_context: serde_json::json!({"surface": "scheduler"}),
            control_policy: crate::harness::workflow::ControlPolicy {
                max_parallel_steps: 1,
                replan_on_verification_fail: true,
                max_patch_chain: 3,
            },
            steps: vec![crate::harness::workflow::WorkflowStep {
                step_id: "S001".to_string(),
                title: "Persist verifier finding".to_string(),
                kind: crate::harness::workflow::WorkflowStepKind::Verify,
                agent_type: "code_writer".to_string(),
                mode: crate::harness::workflow::WorkflowStepMode::WritePatch,
                task: "Record verifier finding through orchestrator status".to_string(),
                inputs: serde_json::json!({"files": ["src-tauri/src/ai/agent_scheduler.rs"]}),
                dependencies: vec![],
                acceptance_criteria: vec![
                    "Status contains workflow verifier diagnostic".to_string()
                ],
                goal_alignment: crate::harness::workflow::GoalAlignment {
                    success_clauses: vec![2],
                    why_necessary: "AI face needs the runtime diagnosis, not only a replan payload"
                        .to_string(),
                },
                retry_policy: crate::harness::workflow::RetryPolicy {
                    max_attempts: 1,
                    backoff_ms: 0,
                },
                status: crate::harness::workflow::WorkflowStepStatus::Failed,
            }],
            created_by: "orchestrator@v1".to_string(),
            created_at: "2026-06-05T16:31:00Z".to_string(),
        };
        let run = crate::harness::workflow::WorkflowRunState::for_workflow(
            "run_runtime",
            &workflow,
            "2026-06-05T16:32:00Z",
        );
        let report = crate::harness::workflow::StepReport {
            report_id: "sr_S001_a1".to_string(),
            step_id: "S001".to_string(),
            attempt: 1,
            status: crate::harness::workflow::StepReportStatus::Failed,
            summary: "Verifier failed before orchestrator status could show the finding."
                .to_string(),
            artifacts: vec![],
            evidence: vec![],
            risks: vec!["AI face would not see the minimal fix surface".to_string()],
            blocked_by: vec![],
            suggested_next_steps: vec![],
            resource_usage: serde_json::json!({"token_total": 300}),
            confidence: 0.66,
        };
        let finding = crate::harness::workflow::VerificationFinding {
            verification_id: "vf_S001_runtime".to_string(),
            level: crate::harness::workflow::VerificationLevel::Step,
            target: "S001".to_string(),
            result: crate::harness::workflow::VerificationOutcome::Fail,
            failed_clauses: vec![2],
            reason_code: "missing_runtime_state".to_string(),
            summary: "Workflow verifier finding was not persisted into orchestrator status."
                .to_string(),
            evidence_refs: vec!["sr_S001_a1".to_string()],
            minimal_fix_surface: vec![
                "Record verifier findings while building replan requests".to_string()
            ],
        };

        let request = sched.build_orchestrator_replan_request(
            "workflow_runtime",
            &goal,
            &workflow,
            &run,
            &[report],
            &[finding],
            &registry,
        );

        assert_eq!(request.verifier_findings.len(), 1);
        let status = sched
            .orchestrator_status()
            .expect("scheduler should expose orchestrator status");
        let diagnostic = status
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.reason_code == "missing_runtime_state")
            .expect("workflow verifier finding should be visible as orchestrator diagnostic");
        assert_eq!(
            diagnostic.source,
            crate::harness::orchestrator::OrchestratorDiagnosticSource::WorkflowVerifier
        );
        assert_eq!(diagnostic.severity, "error");
        assert_eq!(
            diagnostic.minimal_fix_surface,
            vec!["Record verifier findings while building replan requests"]
        );
        assert_eq!(status.workflow_event_count, 3);
        assert_eq!(status.workflow_replan_request_count, 1);
        assert_eq!(
            status
                .recent_workflow_events
                .iter()
                .map(|event| event.event_name.as_str())
                .collect::<Vec<_>>(),
            vec![
                "workflow.step_report.recorded",
                "workflow.verification.failed",
                "workflow.replan.requested"
            ]
        );
        let event_log_path = status
            .workflow_event_log_path
            .as_ref()
            .expect("status should expose the latest persisted workflow event log path");
        assert!(event_log_path.ends_with("events.jsonl"));
        let persisted = crate::harness::workflow::WorkflowRuntimeEventStore::read_recent(
            event_root.path(),
            "run_runtime",
            10,
        )
        .expect("scheduler should persist workflow runtime events");
        assert_eq!(persisted.len(), 3);
        assert_eq!(persisted[2].event_name, "workflow.replan.requested");
    }

    #[test]
    fn workflow_patch_decision_is_recorded_in_runtime_event_chain() {
        let mut sched = AgentScheduler::new(2);
        let event_root = tempfile::tempdir().unwrap();
        sched.set_workflow_event_root(event_root.path());
        let patch = crate::harness::workflow::WorkflowPatch {
            patch_id: "patch_wf_runtime_v1_to_v2".to_string(),
            workflow_id: "wf_runtime".to_string(),
            from_version: 1,
            to_version: 2,
            basis: crate::harness::workflow::PatchBasis {
                failed_steps: vec!["S001".to_string()],
                failed_goal_clauses: vec![2],
            },
            ops: vec![
                crate::harness::workflow::WorkflowPatchOp::ReplaceStepStatus {
                    step_id: "S001".to_string(),
                    status: crate::harness::workflow::WorkflowStepStatus::Pending,
                },
            ],
            rationale: "Retry verifier step after human review.".to_string(),
            predicted_impact: serde_json::json!({"risk": "low"}),
        };

        sched.record_workflow_patch_decision(
            "wf_runtime",
            "run_runtime",
            &patch,
            false,
            "Bridge reviewer rejected the retry surface.",
        );

        let status = sched
            .orchestrator_status()
            .expect("scheduler should expose orchestrator status");
        assert_eq!(status.workflow_event_count, 1);
        assert_eq!(status.workflow_replan_request_count, 0);
        assert_eq!(
            status.recent_workflow_events[0].event_name,
            "workflow.patch.rejected"
        );
        assert_eq!(
            status.recent_workflow_events[0].attributes["patch_id"],
            "patch_wf_runtime_v1_to_v2"
        );
        let persisted = crate::harness::workflow::WorkflowRuntimeEventStore::read_recent(
            event_root.path(),
            "run_runtime",
            10,
        )
        .expect("patch decision should persist to the same workflow runtime ledger");
        assert_eq!(persisted.len(), 1);
        assert_eq!(persisted[0].event_name, "workflow.patch.rejected");
    }

    #[test]
    fn scheduler_prepares_controlled_subagent_jobs_and_records_dispatch_events() {
        let mut sched = AgentScheduler::new(2);
        let event_root = tempfile::tempdir().unwrap();
        sched.set_workflow_event_root(event_root.path());
        let registry = crate::harness::workflow::AgentRegistry::from_agents(vec![
            crate::harness::workflow::AgentRegistryEntry {
                agent_type: "research_subagent".to_string(),
                allowed_tools: vec![
                    "vector_search".to_string(),
                    "arxiv_search".to_string(),
                ],
                denied_tools: vec!["Edit".to_string()],
                default_mode: crate::harness::workflow::WorkflowStepMode::ReadOnly,
                max_parallelism: 1,
                can_delegate: false,
            },
        ]);
        let workflow = crate::harness::workflow::WorkflowIr {
            workflow_id: "wf_dispatch".to_string(),
            goal_id: "goal_dispatch".to_string(),
            version: 1,
            parent_version: None,
            status: crate::harness::workflow::WorkflowStatus::Running,
            global_context: serde_json::json!({}),
            control_policy: crate::harness::workflow::ControlPolicy {
                max_parallel_steps: 1,
                replan_on_verification_fail: true,
                max_patch_chain: 3,
            },
            steps: vec![crate::harness::workflow::WorkflowStep {
                step_id: "S001".to_string(),
                title: "Expand the evidence frontier".to_string(),
                kind: crate::harness::workflow::WorkflowStepKind::Research,
                agent_type: "research_subagent".to_string(),
                mode: crate::harness::workflow::WorkflowStepMode::ReadOnly,
                task: "Return a cited evidence packet".to_string(),
                inputs: serde_json::json!({"keywords": ["bayesian memory"]}),
                dependencies: vec![],
                acceptance_criteria: vec!["Every claim has a source".to_string()],
                goal_alignment: crate::harness::workflow::GoalAlignment {
                    success_clauses: vec![1],
                    why_necessary: "Scientist needs frontier evidence".to_string(),
                },
                retry_policy: crate::harness::workflow::RetryPolicy {
                    max_attempts: 2,
                    backoff_ms: 1000,
                },
                status: crate::harness::workflow::WorkflowStepStatus::Ready,
            }],
            created_by: "orchestrator@v1".to_string(),
            created_at: "2026-06-19T00:00:00Z".to_string(),
        };
        let run = crate::harness::workflow::WorkflowRunState::for_workflow(
            "run_dispatch",
            &workflow,
            "2026-06-19T00:01:00Z",
        );

        let jobs = sched
            .prepare_workflow_subagent_jobs(&workflow, &run, &registry)
            .unwrap();

        assert_eq!(jobs.len(), 1);
        assert!(jobs[0].retrieval_job.is_some());
        assert_eq!(jobs[0].dispatch.step_id, "S001");
        let events = sched
            .workflow_runtime_events_for_run("run_dispatch", 10)
            .unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_name, "workflow.step.dispatched");
    }

    #[test]
    fn workflow_runtime_events_for_run_reads_persisted_ledger() {
        let mut sched = AgentScheduler::new(2);
        let event_root = tempfile::tempdir().unwrap();
        sched.set_workflow_event_root(event_root.path());
        let patch = crate::harness::workflow::WorkflowPatch {
            patch_id: "patch_wf_runtime_v1_to_v2".to_string(),
            workflow_id: "wf_runtime".to_string(),
            from_version: 1,
            to_version: 2,
            basis: crate::harness::workflow::PatchBasis {
                failed_steps: vec!["S001".to_string()],
                failed_goal_clauses: vec![2],
            },
            ops: vec![
                crate::harness::workflow::WorkflowPatchOp::ReplaceStepStatus {
                    step_id: "S001".to_string(),
                    status: crate::harness::workflow::WorkflowStepStatus::Pending,
                },
            ],
            rationale: "Retry verifier step after human review.".to_string(),
            predicted_impact: serde_json::json!({}),
        };

        sched.record_workflow_patch_decision(
            "wf_runtime",
            "run_runtime",
            &patch,
            true,
            "Bridge reviewer accepted the retry surface.",
        );

        let events = sched
            .workflow_runtime_events_for_run("run_runtime", 5)
            .expect("scheduler should read persisted workflow runtime events by run");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_name, "workflow.patch.accepted");
        assert_eq!(
            sched
                .workflow_runtime_events_for_run("run_missing", 5)
                .unwrap(),
            Vec::<HarnessEvent>::new()
        );
    }

    #[test]
    fn complete_with_dream_snapshot_records_dream_metrics() {
        let mut sched = AgentScheduler::new(4);
        let parent = make_task("dream_parent", TaskPriority::Medium);

        sched.submit(parent);
        sched.approve("dream_parent", "human").unwrap();
        if let Some(orch) = sched.orchestrator.as_mut() {
            orch.regression_thresholds.min_epoch_before_audit = 1;
            orch.regression_thresholds.auto_terminate = false;
        }

        let task = sched.dequeue().unwrap();
        sched.complete_with_dream_snapshot(
            &task.id,
            "done".to_string(),
            Some(crate::harness::regression::DreamAuditSnapshot {
                community_coverage: 0.96,
                salience_shift: 0.01,
                contradiction_risk: 0.0,
            }),
        );

        let metrics = sched
            .get_task("dream_parent")
            .and_then(|task| task.regression_metrics.as_ref())
            .expect("audit metrics should be stored on completed task");
        assert_eq!(metrics.dream_coverage, 0.96);
        assert_eq!(metrics.salience_shift, 0.01);

        let detail = sched
            .orchestrator
            .as_ref()
            .and_then(|orch| orch.event_log.last())
            .and_then(|event| event.detail.as_ref())
            .expect("Dream reason should be logged");
        assert!(detail.contains("dream_coverage_reached"));
    }
}
