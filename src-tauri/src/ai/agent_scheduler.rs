// Agent Scheduler — Manage AI agent task lifecycle
// v2.4: Task Handoff state machine (Pending→Approved→Running→Done|Error)

use super::sandbox::SandboxPolicy;
use super::subagent::{DataSource, SearchType, Subagent, SubagentJob, SubagentResult};
use super::task_intent::TaskIntentType;
use crate::cli::parser::CliCommand;
use crate::graph::manifest::GraphManifest;
use crate::harness::context::ContextFragment;
use crate::harness::orchestrator::{
    AuditAction, Orchestrator, OrchestratorMaterialPacket, RegressionMetrics,
};
use crate::harness::regression::DreamAuditSnapshot;
use crate::harness::scientist::Scientist;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
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
        self.complete_with_dream_snapshot(task_id, result, None);
    }

    pub fn complete_with_dream_snapshot(
        &mut self,
        task_id: &str,
        result: String,
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
                self.tasks.remove(task_id);
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

    /// Get status update receiver for external listeners
    pub fn status_receiver(&mut self) -> &mut mpsc::UnboundedReceiver<TaskStatusUpdate> {
        &mut self.status_rx
    }

    pub fn set_subagent(&mut self, subagent: Subagent) {
        self.subagent = Some(subagent);
    }

    pub fn orchestrator_status(&self) -> Option<crate::harness::orchestrator::OrchestratorStatus> {
        self.orchestrator.as_ref().map(|o| o.status())
    }

    pub fn orchestrator_events(&self) -> Vec<crate::harness::orchestrator::OrchestratorEvent> {
        self.orchestrator
            .as_ref()
            .map(|o| o.event_log.clone())
            .unwrap_or_default()
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

    pub fn get_task_manifest(&self, task_id: &str) -> Result<&GraphManifest, String> {
        self.tasks
            .get(task_id)
            .and_then(|t| t.graph_manifest.as_ref())
            .ok_or_else(|| format!("No manifest for task: {}", task_id))
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
