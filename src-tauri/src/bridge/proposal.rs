#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BridgeProposalSource {
    Tool,
    Scientist,
    Dream,
    Ghost,
    Scheduler,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceRefKind {
    Task,
    Ghost,
    DreamInsight,
    ScientistOutput,
    SchedulerJob,
    ToolCall,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SourceRef {
    pub kind: SourceRefKind,
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    Note,
    Chunk,
    Trace,
    ToolResult,
    Verification,
    Diff,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EvidenceRef {
    pub label: String,
    pub kind: EvidenceKind,
    #[serde(rename = "ref")]
    pub reference: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub excerpt: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ImpactScope {
    #[serde(default)]
    pub notes: Vec<String>,
    #[serde(default)]
    pub creates_files: bool,
    #[serde(default)]
    pub modifies_notes: bool,
    #[serde(default)]
    pub exports_data: bool,
    #[serde(default)]
    pub external_side_effect: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalRisk {
    Low,
    Medium,
    High,
}

impl From<crate::ai::task_intent::BridgeRisk> for ProposalRisk {
    fn from(value: crate::ai::task_intent::BridgeRisk) -> Self {
        match value {
            crate::ai::task_intent::BridgeRisk::Low => Self::Low,
            crate::ai::task_intent::BridgeRisk::Medium => Self::Medium,
            crate::ai::task_intent::BridgeRisk::High => Self::High,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BridgeProposalStatus {
    Pending,
    Approved,
    Rejected,
    Applied,
    Archived,
}

impl BridgeProposalStatus {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.to_ascii_lowercase().as_str() {
            "pending" => Ok(Self::Pending),
            "approved" => Ok(Self::Approved),
            "rejected" => Ok(Self::Rejected),
            "applied" => Ok(Self::Applied),
            "archived" => Ok(Self::Archived),
            _ => Err(format!("Unknown bridge proposal status: {value}")),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Rejected => "rejected",
            Self::Applied => "applied",
            Self::Archived => "archived",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalActionKind {
    ApproveTask,
    OpenDiff,
    OpenTrace,
    ApplyGhost,
    Reject,
    Archive,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProposalAction {
    pub id: String,
    pub label: String,
    pub kind: ProposalActionKind,
    #[serde(default)]
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TrustSnapshot {
    pub score: f32,
    pub acceptance_rate: f32,
    pub total_interactions: u64,
    pub recommended_mode: String,
}

impl Default for TrustSnapshot {
    fn default() -> Self {
        Self {
            score: 0.5,
            acceptance_rate: 0.0,
            total_interactions: 0,
            recommended_mode: "manual".to_string(),
        }
    }
}

impl TrustSnapshot {
    pub fn from_protocol(protocol: Option<&crate::ipc::protocol::TwoSurfaceProtocol>) -> Self {
        if let Some(protocol) = protocol {
            Self {
                score: protocol.trust_score_value(),
                acceptance_rate: protocol.acceptance_rate(),
                total_interactions: protocol.total_interactions() as u64,
                recommended_mode: format!("{:?}", protocol.current_mode()).to_lowercase(),
            }
        } else {
            Self::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BridgeProposal {
    pub id: String,
    pub intent: String,
    pub summary: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox_summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_output: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risk_reason: Option<String>,
    pub source: BridgeProposalSource,
    pub source_ref: SourceRef,
    pub status: BridgeProposalStatus,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub impact: ImpactScope,
    pub risk: ProposalRisk,
    #[serde(default)]
    pub actions: Vec<ProposalAction>,
    #[serde(default)]
    pub trust_snapshot: TrustSnapshot,
    pub created_at: u64,
    pub updated_at: u64,
}

impl BridgeProposal {
    fn next_id() -> String {
        format!(
            "bridge_{}",
            uuid::Uuid::new_v4().to_string().replace('-', "")
        )
    }

    pub fn classify_risk(impact: &ImpactScope, has_verification_violations: bool) -> ProposalRisk {
        if impact.modifies_notes
            || impact.exports_data
            || impact.external_side_effect
            || has_verification_violations
        {
            ProposalRisk::High
        } else if impact.notes.len() > 1 || impact.creates_files {
            ProposalRisk::Medium
        } else {
            ProposalRisk::Low
        }
    }

    pub fn for_task(task_id: &str, intent: &str, trust_snapshot: TrustSnapshot, now: u64) -> Self {
        let impact = ImpactScope::default();
        Self {
            id: Self::next_id(),
            source: BridgeProposalSource::Tool,
            source_ref: SourceRef {
                kind: SourceRefKind::Task,
                id: task_id.to_string(),
                path: None,
            },
            intent: intent.to_string(),
            summary: "AI 准备执行一个需要人类确认的任务。".to_string(),
            task_type: None,
            sandbox_summary: None,
            expected_output: None,
            risk_reason: None,
            evidence: vec![EvidenceRef {
                label: "Agent task".to_string(),
                kind: EvidenceKind::Trace,
                reference: task_id.to_string(),
                confidence: None,
                excerpt: Some(intent.to_string()),
            }],
            risk: Self::classify_risk(&impact, false),
            impact,
            status: BridgeProposalStatus::Pending,
            actions: vec![
                ProposalAction {
                    id: "approve".to_string(),
                    label: "批准".to_string(),
                    kind: ProposalActionKind::ApproveTask,
                    payload: serde_json::json!({ "task_id": task_id }),
                },
                ProposalAction {
                    id: "reject".to_string(),
                    label: "拒绝".to_string(),
                    kind: ProposalActionKind::Reject,
                    payload: serde_json::json!({ "task_id": task_id }),
                },
            ],
            trust_snapshot,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn for_typed_task(
        task_id: &str,
        intent: &str,
        task_type: crate::ai::task_intent::TaskIntentType,
        sandbox_policy: &crate::ai::sandbox::SandboxPolicy,
        trust_snapshot: TrustSnapshot,
        now: u64,
    ) -> Self {
        let mut proposal = Self::for_task(task_id, intent, trust_snapshot, now);
        proposal.task_type = Some(task_type.as_str().to_string());
        proposal.sandbox_summary = Some(sandbox_policy.summary());
        proposal.expected_output = Some(task_type.expected_output().to_string());
        proposal.risk_reason = Some(task_type.risk_reason().to_string());
        proposal.risk = ProposalRisk::from(task_type.default_bridge_risk());
        proposal
    }

    pub fn for_scheduler_task(
        task_id: &str,
        job_id: &str,
        trust_snapshot: TrustSnapshot,
        now: u64,
    ) -> Self {
        let impact = ImpactScope::default();
        Self {
            id: Self::next_id(),
            source: BridgeProposalSource::Scheduler,
            source_ref: SourceRef {
                kind: SourceRefKind::SchedulerJob,
                id: task_id.to_string(),
                path: Some(job_id.to_string()),
            },
            intent: format!("审查定时任务: {}", job_id),
            summary: format!("Scheduler 触发了 `{}`，需要人类确认后执行。", job_id),
            task_type: None,
            sandbox_summary: None,
            expected_output: None,
            risk_reason: None,
            evidence: vec![EvidenceRef {
                label: "Scheduler job".to_string(),
                kind: EvidenceKind::Trace,
                reference: task_id.to_string(),
                confidence: None,
                excerpt: Some(job_id.to_string()),
            }],
            risk: Self::classify_risk(&impact, false),
            impact,
            status: BridgeProposalStatus::Pending,
            actions: vec![
                ProposalAction {
                    id: "approve".to_string(),
                    label: "批准".to_string(),
                    kind: ProposalActionKind::ApproveTask,
                    payload: serde_json::json!({ "task_id": task_id, "job_id": job_id }),
                },
                ProposalAction {
                    id: "reject".to_string(),
                    label: "拒绝".to_string(),
                    kind: ProposalActionKind::Reject,
                    payload: serde_json::json!({ "task_id": task_id, "job_id": job_id }),
                },
            ],
            trust_snapshot,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn for_ghost(
        ghost_id: &str,
        target_note: &str,
        block_count: usize,
        trust_snapshot: TrustSnapshot,
        now: u64,
    ) -> Self {
        let impact = ImpactScope {
            notes: vec![target_note.to_string()],
            creates_files: false,
            modifies_notes: true,
            exports_data: false,
            external_side_effect: false,
        };
        Self {
            id: Self::next_id(),
            source: BridgeProposalSource::Ghost,
            source_ref: SourceRef {
                kind: SourceRefKind::Ghost,
                id: ghost_id.to_string(),
                path: Some(target_note.to_string()),
            },
            intent: format!("审阅 {} 个 Ghost 建议块", block_count),
            summary: format!(
                "AI 为 `{}` 创建了 {} 个待审阅建议块。",
                target_note, block_count
            ),
            task_type: None,
            sandbox_summary: None,
            expected_output: None,
            risk_reason: None,
            evidence: vec![EvidenceRef {
                label: "Ghost diff".to_string(),
                kind: EvidenceKind::Diff,
                reference: ghost_id.to_string(),
                confidence: None,
                excerpt: None,
            }],
            risk: Self::classify_risk(&impact, false),
            impact,
            status: BridgeProposalStatus::Pending,
            actions: vec![
                ProposalAction {
                    id: "open-diff".to_string(),
                    label: "打开 Diff".to_string(),
                    kind: ProposalActionKind::OpenDiff,
                    payload: serde_json::json!({ "ghost_id": ghost_id }),
                },
                ProposalAction {
                    id: "reject".to_string(),
                    label: "拒绝".to_string(),
                    kind: ProposalActionKind::Reject,
                    payload: serde_json::json!({ "ghost_id": ghost_id }),
                },
            ],
            trust_snapshot,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn for_scientist_result(
        task_id: &str,
        topic: &str,
        claim_count: usize,
        violation_count: usize,
        related_notes: Vec<String>,
        trust_snapshot: TrustSnapshot,
        now: u64,
    ) -> Self {
        let impact = ImpactScope {
            notes: related_notes,
            creates_files: false,
            modifies_notes: false,
            exports_data: false,
            external_side_effect: false,
        };
        Self {
            id: Self::next_id(),
            source: BridgeProposalSource::Scientist,
            source_ref: SourceRef {
                kind: SourceRefKind::ScientistOutput,
                id: task_id.to_string(),
                path: None,
            },
            intent: format!("审阅 Scientist 精炼结果: {}", topic),
            summary: format!(
                "Scientist 提取了 {} 条 claims，发现 {} 个验证问题。",
                claim_count, violation_count
            ),
            task_type: None,
            sandbox_summary: None,
            expected_output: None,
            risk_reason: None,
            evidence: vec![EvidenceRef {
                label: "Scientist verification".to_string(),
                kind: EvidenceKind::Verification,
                reference: task_id.to_string(),
                confidence: None,
                excerpt: Some(format!(
                    "claims={}, violations={}",
                    claim_count, violation_count
                )),
            }],
            risk: Self::classify_risk(&impact, violation_count > 0),
            impact,
            status: BridgeProposalStatus::Pending,
            actions: vec![
                ProposalAction {
                    id: "open-trace".to_string(),
                    label: "打开 Trace".to_string(),
                    kind: ProposalActionKind::OpenTrace,
                    payload: serde_json::json!({ "task_id": task_id }),
                },
                ProposalAction {
                    id: "archive".to_string(),
                    label: "归档".to_string(),
                    kind: ProposalActionKind::Archive,
                    payload: serde_json::json!({}),
                },
            ],
            trust_snapshot,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn for_dream_cycle(
        cycle_id: &str,
        insight_count: usize,
        related_notes: Vec<String>,
        trust_snapshot: TrustSnapshot,
        now: u64,
    ) -> Self {
        let impact = ImpactScope {
            notes: related_notes,
            creates_files: false,
            modifies_notes: false,
            exports_data: false,
            external_side_effect: false,
        };
        Self {
            id: Self::next_id(),
            source: BridgeProposalSource::Dream,
            source_ref: SourceRef {
                kind: SourceRefKind::DreamInsight,
                id: cycle_id.to_string(),
                path: None,
            },
            intent: "审阅 Dream 洞察".to_string(),
            summary: format!("Dream cycle 生成了 {} 条可回看洞察。", insight_count),
            task_type: None,
            sandbox_summary: None,
            expected_output: None,
            risk_reason: None,
            evidence: vec![EvidenceRef {
                label: "Dream insights".to_string(),
                kind: EvidenceKind::Trace,
                reference: cycle_id.to_string(),
                confidence: None,
                excerpt: Some(format!("insights={}", insight_count)),
            }],
            risk: Self::classify_risk(&impact, false),
            impact,
            status: BridgeProposalStatus::Pending,
            actions: vec![ProposalAction {
                id: "archive".to_string(),
                label: "归档".to_string(),
                kind: ProposalActionKind::Archive,
                payload: serde_json::json!({}),
            }],
            trust_snapshot,
            created_at: now,
            updated_at: now,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_frontend_contract_field_names() {
        let proposal = BridgeProposal {
            id: "proposal-1".to_string(),
            intent: "Review AI intent".to_string(),
            summary: "AI wants to do something reviewable.".to_string(),
            task_type: None,
            sandbox_summary: None,
            expected_output: None,
            risk_reason: None,
            source: BridgeProposalSource::Tool,
            source_ref: SourceRef {
                kind: SourceRefKind::Task,
                id: "task-1".to_string(),
                path: None,
            },
            status: BridgeProposalStatus::Pending,
            evidence: vec![],
            impact: ImpactScope::default(),
            risk: ProposalRisk::Low,
            actions: vec![],
            trust_snapshot: TrustSnapshot::default(),
            created_at: 1,
            updated_at: 1,
        };

        let value = serde_json::to_value(proposal).unwrap();

        assert!(value.get("intent").is_some());
        assert!(value.get("trust_snapshot").is_some());
        assert!(value.get("title").is_none());
        assert!(value.get("trust").is_none());
    }

    #[test]
    fn task_proposal_contains_approve_action() {
        let proposal =
            BridgeProposal::for_task("task_1", "Research Bayes", TrustSnapshot::default(), 1);

        assert_eq!(proposal.source, BridgeProposalSource::Tool);
        assert_eq!(proposal.source_ref.id, "task_1");
        assert!(proposal
            .actions
            .iter()
            .any(|action| action.kind == ProposalActionKind::ApproveTask));
    }

    #[test]
    fn typed_task_proposal_contains_task_review_metadata() {
        let policy = crate::ai::task_intent::TaskIntentType::Research.default_sandbox_policy();
        let proposal = BridgeProposal::for_typed_task(
            "task_1",
            "Research Bayes",
            crate::ai::task_intent::TaskIntentType::Research,
            &policy,
            TrustSnapshot::default(),
            1,
        );

        assert_eq!(proposal.task_type.as_deref(), Some("research"));
        assert_eq!(
            proposal.expected_output.as_deref(),
            Some("research brief with sources")
        );
        assert!(proposal
            .sandbox_summary
            .as_deref()
            .unwrap()
            .contains("vector_search"));
        assert!(proposal.risk_reason.as_deref().unwrap().contains("network"));
        assert_eq!(proposal.risk, ProposalRisk::Medium);
    }

    #[test]
    fn ghost_proposal_is_high_risk_when_it_modifies_note() {
        let proposal =
            BridgeProposal::for_ghost("ghost_1", "Target.md", 3, TrustSnapshot::default(), 1);

        assert_eq!(proposal.source, BridgeProposalSource::Ghost);
        assert_eq!(proposal.risk, ProposalRisk::High);
        assert!(proposal
            .actions
            .iter()
            .any(|action| action.kind == ProposalActionKind::OpenDiff));
    }

    #[test]
    fn scientist_proposal_is_high_risk_with_violations() {
        let proposal = BridgeProposal::for_scientist_result(
            "task_1",
            "Deep research",
            4,
            1,
            vec!["Note.md".to_string()],
            TrustSnapshot::default(),
            1,
        );

        assert_eq!(proposal.source, BridgeProposalSource::Scientist);
        assert_eq!(proposal.risk, ProposalRisk::High);
    }

    #[test]
    fn dream_proposal_groups_insights_without_write_impact() {
        let proposal = BridgeProposal::for_dream_cycle(
            "dream_1",
            2,
            vec!["A.md".to_string()],
            TrustSnapshot::default(),
            1,
        );

        assert_eq!(proposal.source, BridgeProposalSource::Dream);
        assert_eq!(proposal.risk, ProposalRisk::Low);
        assert!(!proposal.impact.modifies_notes);
    }

    #[test]
    fn scheduler_proposal_uses_scheduler_source_with_task_actions() {
        let proposal =
            BridgeProposal::for_scheduler_task("task_1", "dream-auto", TrustSnapshot::default(), 1);

        assert_eq!(proposal.source, BridgeProposalSource::Scheduler);
        assert_eq!(proposal.source_ref.kind, SourceRefKind::SchedulerJob);
        assert_eq!(proposal.source_ref.id, "task_1");
        assert!(proposal
            .actions
            .iter()
            .any(|action| action.kind == ProposalActionKind::ApproveTask));
        assert!(proposal
            .actions
            .iter()
            .any(|action| action.kind == ProposalActionKind::Reject));
    }
}
