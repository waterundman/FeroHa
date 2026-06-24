use crate::ai::agent_scheduler::{
    AgentScheduler, AgentTask, AiFaceDataFlow, AiManagerSnapshot, TaskHandle,
};

pub struct AiManagerService<'a> {
    scheduler: &'a mut AgentScheduler,
}

impl<'a> AiManagerService<'a> {
    pub fn new(scheduler: &'a mut AgentScheduler) -> Self {
        Self { scheduler }
    }

    pub fn submit(&mut self, task: AgentTask) -> TaskHandle {
        self.scheduler.submit(task)
    }

    pub fn approve(&mut self, task_id: &str, checked_by: &str) -> Result<(), String> {
        self.scheduler.approve(task_id, checked_by)
    }

    pub fn cancel(&mut self, task_id: &str) {
        self.scheduler.cancel(task_id);
    }

    pub fn list_tasks(&self, status_filter: Option<&str>) -> Vec<AgentTask> {
        self.scheduler
            .list_tasks(status_filter)
            .into_iter()
            .cloned()
            .collect()
    }

    pub fn list_ai_face_data_flows(&self) -> Vec<AiFaceDataFlow> {
        self.scheduler.list_ai_face_data_flows()
    }

    pub fn snapshot(&self) -> AiManagerSnapshot {
        self.scheduler.ai_manager_snapshot()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::agent_scheduler::{
        AgentTask, SynthesizePhase, TaskPriority, TaskStatus, TaskType,
    };
    use crate::ai::task_intent::TaskIntentType;
    use crate::cli::parser::CliCommand;

    fn task(id: &str) -> AgentTask {
        AgentTask {
            id: id.to_string(),
            command: CliCommand::Custom("manager facade test".to_string()),
            task_type: TaskType::Custom("research".to_string()),
            task_intent: Some(TaskIntentType::Research),
            sandbox_policy: Some(TaskIntentType::Research.default_sandbox_policy()),
            priority: TaskPriority::Medium,
            priority_score: 50,
            status: TaskStatus::Pending,
            anchor_note: None,
            created_at: 1,
            max_retries: 2,
            retry_count: 0,
            synthesize_phase: SynthesizePhase::Idle,
            subagent_results: Vec::new(),
            graph_manifest: None,
            has_trace: false,
            source_block_id: None,
            card_id: None,
            card_type: None,
            prompt: None,
            params: None,
            context_note: Some("Human.md".to_string()),
            intent: "Research Human.md".to_string(),
            content: "Research Human.md".to_string(),
            max_iterations: 30,
            sub_tasks: Vec::new(),
            material_packet: None,
            context_fragments: Vec::new(),
            regression_metrics: None,
            retry_delay_ms: 1000,
            retry_backoff_multiplier: 2.0,
            last_retry_at: None,
            consecutive_failures: 0,
        }
    }

    #[test]
    fn manager_facade_owns_task_control_and_ai_face_queries() {
        let mut scheduler = crate::ai::agent_scheduler::AgentScheduler::new(2);
        let handle = {
            let mut manager = AiManagerService::new(&mut scheduler);
            manager.submit(task("manager-task"))
        };
        assert_eq!(handle.id, "manager-task");

        {
            let mut manager = AiManagerService::new(&mut scheduler);
            manager.approve("manager-task", "human").unwrap();
            assert_eq!(manager.list_tasks(Some("approved")).len(), 1);
            assert_eq!(manager.list_ai_face_data_flows().len(), 1);
            assert_eq!(manager.snapshot().total_tasks, 1);
        }
    }
}
