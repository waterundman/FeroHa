// Agent Scheduler — Manage AI agent task lifecycle
// Stage 4: Background task queue with priority, status tracking, cancellation

use std::collections::{HashMap, VecDeque};
use tokio::sync::mpsc;
use serde::{Serialize, Deserialize};
use crate::cli::parser::CliCommand;

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
    High,    // User-initiated, expecting immediate response
    Medium,  // User-initiated, can wait
    Low,     // Background/automatic task
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskStatus {
    Queued,
    Running { started_at: u64, progress: f32 },
    Done { completed_at: u64, result: String },
    Error { message: String },
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTask {
    pub id: String,
    pub command: CliCommand,
    pub task_type: TaskType,
    pub priority: TaskPriority,
    pub status: TaskStatus,
    pub anchor_note: Option<String>,
    pub created_at: u64,
    pub max_retries: u32,
    pub retry_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskHandle {
    pub id: String,
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
            high_queue: VecDeque::new(),
            queue: VecDeque::new(),
            tasks: HashMap::new(),
            max_concurrent,
            running_count: 0,
            status_tx,
            status_rx,
        }
    }

    /// Submit a task to the scheduler
    pub fn submit(&mut self, task: AgentTask) -> TaskHandle {
        let handle = TaskHandle { id: task.id.clone() };
        self.tasks.insert(task.id.clone(), task.clone());

        match task.priority {
            TaskPriority::High => self.high_queue.push_back(task),
            _ => self.queue.push_back(task),
        }

        handle
    }

    /// Dequeue the next task (high priority first)
    pub fn dequeue(&mut self) -> Option<AgentTask> {
        if self.running_count >= self.max_concurrent {
            return None;
        }

        let task = self.high_queue.pop_front()
            .or_else(|| self.queue.pop_front());

        if let Some(ref t) = task {
            self.running_count += 1;
            self.update_task_status(&t.id, TaskStatus::Running {
                started_at: now_millis(),
                progress: 0.0,
            });
        }

        task
    }

    /// Mark a task as completed
    pub fn complete(&mut self, task_id: &str, result: String) {
        self.running_count = self.running_count.saturating_sub(1);
        self.update_task_status(task_id, TaskStatus::Done {
            completed_at: now_millis(),
            result,
        });
    }

    /// Mark a task as failed
    pub fn fail(&mut self, task_id: &str, error: String) {
        self.running_count = self.running_count.saturating_sub(1);

        // Check if retry is possible
        if let Some(task) = self.tasks.get(task_id) {
            if task.retry_count < task.max_retries {
                let mut retry_task = task.clone();
                retry_task.retry_count += 1;
                retry_task.status = TaskStatus::Queued;
                retry_task.priority = TaskPriority::Low; // Downgrade on retry
                self.queue.push_back(retry_task);
                self.tasks.remove(task_id);
                return;
            }
        }

        self.update_task_status(task_id, TaskStatus::Error { message: error });
    }

    /// Cancel a queued or running task
    pub fn cancel(&mut self, task_id: &str) -> bool {
        // Remove from queues
        self.high_queue.retain(|t| t.id != task_id);
        self.queue.retain(|t| t.id != task_id);

        if let Some(task) = self.tasks.get(task_id) {
            match task.status {
                TaskStatus::Running { .. } => {
                    self.running_count = self.running_count.saturating_sub(1);
                }
                _ => {}
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
            queued: 0,
            running: 0,
            done: 0,
            failed: 0,
        };
        for task in self.tasks.values() {
            match task.status {
                TaskStatus::Queued => stats.queued += 1,
                TaskStatus::Running { .. } => stats.running += 1,
                TaskStatus::Done { .. } => stats.done += 1,
                TaskStatus::Error { .. } => stats.failed += 1,
                TaskStatus::Cancelled => {}
            }
        }
        stats
    }

    /// Get status update receiver for external listeners
    pub fn status_receiver(&mut self) -> &mut mpsc::UnboundedReceiver<TaskStatusUpdate> {
        &mut self.status_rx
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
}

fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::parser::CliCommand;

    fn make_task(id: &str, priority: TaskPriority) -> AgentTask {
        AgentTask {
            id: id.to_string(),
            command: CliCommand::Status,
            task_type: TaskType::Search,
            priority,
            status: TaskStatus::Queued,
            anchor_note: None,
            created_at: now_millis(),
            max_retries: 1,
            retry_count: 0,
        }
    }

    #[test]
    fn test_submit_and_dequeue() {
        let mut sched = AgentScheduler::new(2);
        sched.submit(make_task("t1", TaskPriority::Medium));
        sched.submit(make_task("t2", TaskPriority::High));
        sched.submit(make_task("t3", TaskPriority::Medium));

        // High priority first
        let t = sched.dequeue().unwrap();
        assert_eq!(t.id, "t2"); // High priority

        let t = sched.dequeue().unwrap();
        assert_eq!(t.id, "t1"); // First medium

        // Max concurrent = 2, so third won't dequeue
        assert!(sched.dequeue().is_none());
    }

    #[test]
    fn test_complete_and_fail() {
        let mut sched = AgentScheduler::new(2);
        sched.submit(make_task("t1", TaskPriority::Medium));

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
        sched.submit(make_task("t2", TaskPriority::Medium));

        sched.cancel("t1");
        match sched.status("t1") {
            Some(TaskStatus::Cancelled) => {}
            _ => panic!("Expected Cancelled"),
        }

        // t2 should still be queued
        let t = sched.dequeue().unwrap();
        assert_eq!(t.id, "t2");
    }
}
