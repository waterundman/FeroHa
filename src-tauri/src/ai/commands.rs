use crate::ai::agent_scheduler::{
    AgentScheduler, AgentTask, AiFaceDataFlow, AiManagerSnapshot, SynthesizePhase, TaskPriority,
    TaskStatus, TaskSuggestion, TaskType,
};
use crate::ai::dream_engine::{DreamEngine, DreamInsight, DreamStats};
use crate::ai::embedding::{EmbeddingBackend, EmbeddingPipeline};
use crate::ai::custom_research;
use crate::ai::manager::AiManagerService;
use crate::ai::rag::{RagPipeline, RagQuery};
use crate::ai::research_trace;
use crate::ai::subagent::{Subagent, SubagentEntry, SubagentResult};
use crate::ai::task_intent::TaskIntentType;
use crate::ai::vectordb::VectorStore;
use crate::ai::workflow_runtime_service::WorkflowRuntimeService;
use crate::harness::context::{ContextFragment, ContextLayer, ContextSource};
use crate::harness::lean_translator::{LeanShapedTranslator, TranslationResult};
use crate::harness::orchestrator::{OrchestratorEvent, OrchestratorStatus};
use crate::harness::output_hook::{HookTrigger, OutputHook};
use crate::harness::proposition_kernel::{PropositionGraph, PropositionKernel, VerificationResult};
use crate::harness::scientist::{CleanKnowledge, Scientist};
use crate::harness::workflow::{HarnessEvent, OrchestratorOutput, WorkflowPatch};
use crate::jsonld::types::{
    JsonLdContextBundle, JsonLdMigrationReport, JsonLdProjectIndex, JsonLdValidationReport,
};
use crate::mdt::archive::MdtArchiveManifest;
use crate::mdt::types::{MdtContextBundle, MdtProjectIndex, MdtValidationReport};
use crate::plugin::manager::{PluginManager, PluginManagerConfig};

use crate::ai::task_scheduler::CronJobStatus;
use crate::bridge::proposal::{BridgeProposal, TrustSnapshot};
use crate::bridge::store::BridgeProposalStore;
use crate::diff::ghost_store::{
    ConflictReport, FeedbackEntry, GhostBlock, GhostNote, GhostOp, GhostStatus,
};
use crate::AiState;
use crate::AppConfig;
use crate::AppState;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio_util::sync::CancellationToken;

fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn task_type_for_cli_command(cmd: &crate::cli::parser::CliCommand, fallback: &str) -> TaskType {
    match cmd {
        crate::cli::parser::CliCommand::Search { .. } => TaskType::Search,
        crate::cli::parser::CliCommand::Summarize { .. } => TaskType::Summarize,
        crate::cli::parser::CliCommand::FetchPapers { .. } => TaskType::FetchPapers,
        crate::cli::parser::CliCommand::DeepDive { .. } => TaskType::DeepDive,
        crate::cli::parser::CliCommand::Explain { .. } => TaskType::Explain,
        _ => TaskType::Custom(fallback.to_string()),
    }
}

fn task_intent_for_cli_command(
    cmd: &crate::cli::parser::CliCommand,
    explicit: Option<&str>,
) -> TaskIntentType {
    if let Some(intent) = explicit.and_then(TaskIntentType::parse) {
        return intent;
    }

    match cmd {
        crate::cli::parser::CliCommand::Summarize { .. } => TaskIntentType::Summarize,
        crate::cli::parser::CliCommand::Dream => TaskIntentType::Dream,
        crate::cli::parser::CliCommand::DiffReview | crate::cli::parser::CliCommand::Status => {
            TaskIntentType::Verify
        }
        crate::cli::parser::CliCommand::CustomCard { card_type, .. } => card_type
            .as_deref()
            .and_then(TaskIntentType::parse)
            .unwrap_or(TaskIntentType::WriteProposal),
        _ => TaskIntentType::Research,
    }
}

fn task_intent_from_payload(payload: &serde_json::Value) -> TaskIntentType {
    payload["task_type"]
        .as_str()
        .or_else(|| payload["card_type"].as_str())
        .and_then(TaskIntentType::parse)
        .unwrap_or(TaskIntentType::Research)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DispatchReviewAction {
    PendingBridgeReview,
    AutoApprove,
}

fn dispatch_review_action_for_policy(
    policy: &crate::ai::sandbox::SandboxPolicy,
) -> DispatchReviewAction {
    if policy.requires_bridge {
        DispatchReviewAction::PendingBridgeReview
    } else {
        DispatchReviewAction::AutoApprove
    }
}

fn sandbox_policy_for_dispatch_payload(
    task_intent: TaskIntentType,
    payload: &serde_json::Value,
) -> crate::ai::sandbox::SandboxPolicy {
    let mut policy = task_intent.default_sandbox_policy();
    match payload["review_mode"].as_str() {
        Some("read_only_auto_queue") if policy.write_roots.is_empty() => {
            policy.requires_bridge = false;
        }
        Some("draft_only") => {
            policy.requires_bridge = false;
            policy.write_roots.clear();
            policy.tool_allowlist.retain(|tool| {
                !matches!(
                    tool.as_str(),
                    "ghost_write"
                        | "bridge_proposal"
                        | "dream_cycle"
                        | "graph_index"
                        | "mdt_index"
                        | "mdt_pack"
                        | "jsonld_index"
                        | "jsonld_migrate"
                )
            });
            if policy.tool_allowlist.is_empty() {
                policy.tool_allowlist.push("llm_complete".to_string());
            }
        }
        _ => {}
    }
    policy
}

fn ensure_bridge_store_exists_for_review_action(
    review_action: DispatchReviewAction,
    has_bridge_store: bool,
) -> Result<(), String> {
    if review_action == DispatchReviewAction::PendingBridgeReview && !has_bridge_store {
        return Err(
            "Bridge review requires you to open a vault before submitting tasks that need approval."
                .to_string(),
        );
    }
    Ok(())
}

fn ensure_bridge_store_for_review_action(
    state: &Mutex<AppState>,
    review_action: DispatchReviewAction,
) -> Result<(), String> {
    if review_action == DispatchReviewAction::AutoApprove {
        return Ok(());
    }
    ensure_bridge_store_exists_for_review_action(review_action, bridge_context(state)?.is_some())
}

fn should_store_card_result_ghost(policy: &crate::ai::sandbox::SandboxPolicy) -> bool {
    policy.allows_tool("ghost_write")
        && policy.allows_write(std::path::Path::new(".dualtrack/ghosts/card-result.md"))
}

fn normalize_legacy_submit_task_payload(mut payload: serde_json::Value) -> serde_json::Value {
    let Some(obj) = payload.as_object_mut() else {
        return serde_json::json!({
            "intent": "legacy submit task",
            "content": payload.to_string(),
            "timestamp": now_millis(),
        });
    };

    let prompt = obj
        .get("prompt")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .to_string();
    let card_type = obj
        .get("card_type")
        .and_then(|value| value.as_str())
        .unwrap_or("selection")
        .to_string();

    if !obj.contains_key("content") {
        obj.insert(
            "content".to_string(),
            serde_json::Value::String(prompt.clone()),
        );
    }
    if !obj.contains_key("intent") {
        let preview: String = prompt.chars().take(80).collect();
        obj.insert(
            "intent".to_string(),
            serde_json::Value::String(format!("{}: {}", card_type, preview)),
        );
    }
    if !obj.contains_key("timestamp") {
        obj.insert(
            "timestamp".to_string(),
            serde_json::Value::Number(serde_json::Number::from(now_millis())),
        );
    }

    payload
}

fn sandbox_policy_for_task(
    task_id: &str,
    cmd: &crate::cli::parser::CliCommand,
    ai_state: &Mutex<AiState>,
) -> Result<crate::ai::sandbox::SandboxPolicy, String> {
    let ai = ai_state.lock().map_err(|e| e.to_string())?;
    Ok(ai
        .agent_scheduler
        .get_task(task_id)
        .and_then(|task| task.sandbox_policy.clone())
        .unwrap_or_else(|| task_intent_for_cli_command(cmd, None).default_sandbox_policy()))
}

fn bridge_context(
    state: &Mutex<AppState>,
) -> Result<Option<(BridgeProposalStore, TrustSnapshot)>, String> {
    let app = state.lock().map_err(|e| e.to_string())?;
    Ok(app
        .bridge_store
        .as_ref()
        .cloned()
        .map(|store| (store, TrustSnapshot::from_protocol(app.protocol.as_ref()))))
}

fn emit_bridge_proposal_update<R: tauri::Runtime>(
    app_handle: Option<&AppHandle<R>>,
    proposal: &BridgeProposal,
) {
    if let Some(app_handle) = app_handle {
        let _ = app_handle.emit("bridge-proposal-updated", proposal);
    }
}

fn upsert_prepared_bridge_proposal<R: tauri::Runtime>(
    store: &BridgeProposalStore,
    app_handle: Option<&AppHandle<R>>,
    proposal: BridgeProposal,
) {
    match store.upsert(proposal) {
        Ok(saved) => emit_bridge_proposal_update(app_handle, &saved),
        Err(error) => tracing::warn!("Failed to create bridge proposal: {}", error),
    }
}

pub(crate) fn try_upsert_bridge_proposal<R, F>(
    state: &Mutex<AppState>,
    app_handle: Option<&AppHandle<R>>,
    build: F,
) where
    R: tauri::Runtime,
    F: FnOnce(TrustSnapshot, u64) -> BridgeProposal,
{
    match bridge_context(state) {
        Ok(Some((store, trust_snapshot))) => {
            upsert_prepared_bridge_proposal(
                &store,
                app_handle,
                build(trust_snapshot, now_millis()),
            );
        }
        Ok(None) => {}
        Err(error) => tracing::warn!("Failed to prepare bridge proposal: {}", error),
    }
}

pub(crate) fn store_workflow_patch_review_proposal(
    store: &BridgeProposalStore,
    run_id: &str,
    patch: &WorkflowPatch,
    trust_snapshot: TrustSnapshot,
    now: u64,
) -> Result<BridgeProposal, String> {
    store
        .upsert(BridgeProposal::for_workflow_patch(
            run_id,
            patch,
            trust_snapshot,
            now,
        ))
        .map_err(|error| error.to_string())
}

pub(crate) fn store_orchestrator_output_bridge_proposal(
    store: &BridgeProposalStore,
    run_id: &str,
    output: &OrchestratorOutput,
    trust_snapshot: TrustSnapshot,
    now: u64,
) -> Result<Option<BridgeProposal>, String> {
    match output {
        OrchestratorOutput::WorkflowPatch { patch } => {
            store_workflow_patch_review_proposal(store, run_id, patch, trust_snapshot, now)
                .map(Some)
        }
        OrchestratorOutput::WorkflowCreate { .. } | OrchestratorOutput::CannotProceed { .. } => {
            Ok(None)
        }
    }
}

pub(crate) fn route_orchestrator_output_review_to_bridge(
    store: &BridgeProposalStore,
    scheduler: Option<&mut AgentScheduler>,
    run_id: &str,
    output: &OrchestratorOutput,
    trust_snapshot: TrustSnapshot,
    now: u64,
) -> Result<Option<BridgeProposal>, String> {
    let saved =
        store_orchestrator_output_bridge_proposal(store, run_id, output, trust_snapshot, now)?;
    if let (Some(scheduler), Some(proposal), OrchestratorOutput::WorkflowPatch { patch }) =
        (scheduler, saved.as_ref(), output)
    {
        scheduler.record_workflow_patch_review_request(run_id, patch, &proposal.id);
    }
    Ok(saved)
}

#[tauri::command]
pub(crate) fn search_notes(
    query: String,
    top_k: u32,
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<crate::ai::vectordb::SearchResult>, String> {
    let app = state.lock().map_err(|e| e.to_string())?;

    // Use RAG hybrid search if sync_engine is available
    if let Some(ref sync_engine) = app.sync_engine {
        let store = sync_engine.store();
        let pipeline = RagPipeline::new(store);
        let rag_query = RagQuery {
            query_text: query.clone(),
            max_results: top_k as usize,
            ..Default::default()
        };
        let results = pipeline.retrieve(&rag_query);
        if !results.is_empty() {
            return Ok(results.into_iter().map(|r| r.chunk).collect());
        }
    }

    // Fallback: brute-force vault scan
    let vault = app.vault.as_ref().ok_or("No vault open")?;
    let notes = vault.list_notes().map_err(|e| e.to_string())?;
    let mut results = Vec::new();

    for note in notes.iter().take(top_k as usize * 3) {
        if let Ok(content) = vault.read_note(&note.path) {
            if content.to_lowercase().contains(&query.to_lowercase()) {
                results.push(crate::ai::vectordb::SearchResult {
                    chunk_id: note.path.clone(),
                    chunk_text: content.chars().take(200).collect(),
                    source_file: note.path.clone(),
                    heading_context: String::new(),
                    score: 0.7,
                    similarity: 0.7,
                });
            }
        }
        if results.len() >= top_k as usize {
            break;
        }
    }

    Ok(results)
}

#[tauri::command]
pub(crate) async fn execute_cli(
    command: String,
    state: State<'_, Mutex<AppState>>,
    ai_state: State<'_, Mutex<AiState>>,
    app_handle: AppHandle,
) -> Result<String, String> {
    let cmd = crate::cli::parser::parse(&command).map_err(|e| e.to_string())?;
    let task_id = format!("task_{}", uuid::Uuid::new_v4().to_string().replace('-', ""));
    let task_intent = task_intent_for_cli_command(&cmd, None);
    let sandbox_policy = task_intent.default_sandbox_policy();
    ensure_bridge_store_for_review_action(&state, DispatchReviewAction::PendingBridgeReview)?;
    let task_type = task_type_for_cli_command(&cmd, &command);

    let task = AgentTask {
        id: task_id.clone(),
        command: cmd,
        task_type,
        task_intent: Some(task_intent),
        sandbox_policy: Some(sandbox_policy.clone()),
        priority: TaskPriority::Medium,
        priority_score: 50,
        status: TaskStatus::Pending,
        anchor_note: None,
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
        max_retries: 2,
        retry_count: 0,
        synthesize_phase: SynthesizePhase::Idle,
        subagent_results: vec![],
        graph_manifest: None,
        has_trace: false,
        source_block_id: None,
        card_id: None,
        card_type: None,
        prompt: None,
        params: None,
        context_note: None,
        intent: command.clone(),
        content: command.clone(),
        max_iterations: 30,
        sub_tasks: vec![],
        material_packet: None,
        context_fragments: vec![],
        regression_metrics: None,
        retry_delay_ms: 1000,
        retry_backoff_multiplier: 2.0,
        last_retry_at: None,
        consecutive_failures: 0,
    };

    {
        let mut ai = ai_state.lock().map_err(|e| e.to_string())?;
        AiManagerService::new(&mut ai.agent_scheduler).submit(task);
    }

    try_upsert_bridge_proposal(&state, Some(&app_handle), |trust_snapshot, now| {
        BridgeProposal::for_typed_task(
            &task_id,
            &command,
            task_intent,
            &sandbox_policy,
            trust_snapshot,
            now,
        )
    });

    Ok(serde_json::json!({
        "task_id": task_id,
        "status": "pending",
        "message": "Task submitted. Pending approval."
    })
    .to_string())
}

#[tauri::command]
pub(crate) fn submit_task(
    command: Option<String>,
    task_type: Option<String>,
    task: Option<serde_json::Value>,
    state: State<'_, Mutex<AppState>>,
    ai_state: State<'_, Mutex<AiState>>,
    app_handle: AppHandle,
) -> Result<serde_json::Value, String> {
    if let Some(task_payload) = task {
        return dispatch_agent_task(
            normalize_legacy_submit_task_payload(task_payload),
            state,
            ai_state,
            app_handle,
        );
    }

    let command =
        command.ok_or_else(|| "submit_task requires command or task payload".to_string())?;
    let cmd = crate::cli::parser::parse(&command).map_err(|e| e.to_string())?;
    let task_id = format!("task_{}", uuid::Uuid::new_v4().to_string().replace('-', ""));
    let explicit_task_type = task_type.as_deref();
    let task_intent = task_intent_for_cli_command(&cmd, explicit_task_type);
    let sandbox_policy = task_intent.default_sandbox_policy();
    ensure_bridge_store_for_review_action(&state, DispatchReviewAction::PendingBridgeReview)?;
    let scheduler_task_type = if explicit_task_type.is_some() {
        TaskType::Custom(task_intent.as_str().to_string())
    } else {
        task_type_for_cli_command(&cmd, &command)
    };

    let task = AgentTask {
        id: task_id.clone(),
        command: cmd,
        task_type: scheduler_task_type,
        task_intent: Some(task_intent),
        sandbox_policy: Some(sandbox_policy.clone()),
        priority: TaskPriority::Medium,
        priority_score: 50,
        status: TaskStatus::Pending,
        anchor_note: None,
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
        max_retries: 2,
        retry_count: 0,
        synthesize_phase: SynthesizePhase::Idle,
        subagent_results: vec![],
        graph_manifest: None,
        has_trace: false,
        source_block_id: None,
        card_id: None,
        card_type: None,
        prompt: None,
        params: None,
        context_note: None,
        intent: command.clone(),
        content: command.clone(),
        max_iterations: 30,
        sub_tasks: vec![],
        material_packet: None,
        context_fragments: vec![],
        regression_metrics: None,
        retry_delay_ms: 1000,
        retry_backoff_multiplier: 2.0,
        last_retry_at: None,
        consecutive_failures: 0,
    };

    let handle = {
        let mut ai = ai_state.lock().map_err(|e| e.to_string())?;
        AiManagerService::new(&mut ai.agent_scheduler).submit(task)
    };

    try_upsert_bridge_proposal(&state, Some(&app_handle), |trust_snapshot, now| {
        BridgeProposal::for_typed_task(
            &task_id,
            &command,
            task_intent,
            &sandbox_policy,
            trust_snapshot,
            now,
        )
    });

    Ok(serde_json::json!({
        "id": handle.id,
        "status": "pending"
    }))
}

#[tauri::command]
pub(crate) fn approve_task(
    task_id: String,
    ai_state: State<'_, Mutex<AiState>>,
    app_handle: AppHandle,
) -> Result<serde_json::Value, String> {
    {
        let mut ai = ai_state.lock().map_err(|e| e.to_string())?;
        AiManagerService::new(&mut ai.agent_scheduler).approve(&task_id, "human")?;
        ai.task_notifier.notify_one();
    }

    let _ = app_handle.emit(
        "task-updated",
        serde_json::json!({
            "task_id": task_id,
            "status": "approved"
        }),
    );

    let _ = app_handle.emit(
        "task-checked",
        serde_json::json!({
            "task_id": task_id,
            "checked_at": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            "checked_by": "human"
        }),
    );

    Ok(serde_json::json!({
        "task_id": task_id,
        "status": "approved"
    }))
}

#[tauri::command]
pub(crate) fn cancel_task(
    task_id: String,
    ai_state: State<'_, Mutex<AiState>>,
    app_handle: AppHandle,
) -> Result<serde_json::Value, String> {
    {
        let mut ai = ai_state.lock().map_err(|e| e.to_string())?;
        AiManagerService::new(&mut ai.agent_scheduler).cancel(&task_id);
    }

    let _ = app_handle.emit(
        "task-updated",
        serde_json::json!({
            "task_id": task_id,
            "status": "cancelled"
        }),
    );

    Ok(serde_json::json!({
        "task_id": task_id,
        "status": "cancelled"
    }))
}

#[tauri::command]
pub(crate) fn get_task_manifest(
    task_id: String,
    ai_state: tauri::State<'_, std::sync::Mutex<AiState>>,
) -> Result<serde_json::Value, String> {
    let ai = ai_state.lock().map_err(|e| e.to_string())?;
    let manifest = ai
        .agent_scheduler
        .get_task_manifest(&task_id)
        .map_err(|e| e.to_string())?;
    Ok(manifest.to_json())
}

#[tauri::command]
pub(crate) fn get_task_trace(
    task_id: String,
    state: State<'_, Mutex<AppState>>,
) -> Result<serde_json::Value, String> {
    let app = state.lock().map_err(|e| e.to_string())?;
    let trace = research_trace::get_task_trace(&app.dualtrack_dir, &task_id)?;
    serde_json::to_value(&trace).map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn list_tasks(
    status_filter: Option<String>,
    ai_state: State<'_, Mutex<AiState>>,
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<AgentTask>, String> {
    let mut ai = ai_state.lock().map_err(|e| e.to_string())?;
    let filter = status_filter.as_deref();
    let app = state.lock().map_err(|e| e.to_string())?;
    let dualtrack_dir = &app.dualtrack_dir;
    let tasks: Vec<AgentTask> = AiManagerService::new(&mut ai.agent_scheduler)
        .list_tasks(filter)
        .into_iter()
        .map(|mut t| {
            t.has_trace = research_trace::has_trace(dualtrack_dir, &t.id);
            t
        })
        .collect();
    Ok(tasks)
}

#[tauri::command]
pub(crate) fn list_ai_face_data_flows(
    ai_state: State<'_, Mutex<AiState>>,
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<AiFaceDataFlow>, String> {
    let mut ai = ai_state.lock().map_err(|e| e.to_string())?;
    let app = state.lock().map_err(|e| e.to_string())?;
    let dualtrack_dir = &app.dualtrack_dir;
    let mut flows = AiManagerService::new(&mut ai.agent_scheduler).list_ai_face_data_flows();
    for flow in &mut flows {
        flow.manager_has_trace |= research_trace::has_trace(dualtrack_dir, &flow.task_id);
    }
    Ok(flows)
}

#[tauri::command]
pub(crate) fn get_ai_manager_snapshot(
    ai_state: State<'_, Mutex<AiState>>,
) -> Result<AiManagerSnapshot, String> {
    let mut ai = ai_state.lock().map_err(|e| e.to_string())?;
    Ok(AiManagerService::new(&mut ai.agent_scheduler).snapshot())
}

fn resolve_template(template: &str, params: &std::collections::HashMap<String, String>) -> String {
    let mut result = template.to_string();
    for (key, value) in params {
        result = result.replace(&format!("{{{{{}}}}}", key), value);
    }
    result
}

fn evidence_entry_from_subagent(entry: &SubagentEntry) -> research_trace::TaskEvidenceEntry {
    research_trace::TaskEvidenceEntry {
        title: entry.title.clone(),
        snippet: entry.snippet.clone(),
        url: entry.url.clone(),
        authors: entry.authors.clone(),
        year: entry.year,
        source: entry.source.clone(),
        relevance_score: entry.relevance_score,
    }
}

fn task_evidence_from_entries(
    source: &str,
    entries: &[SubagentEntry],
    hop: u32,
    generated_keywords: Vec<String>,
) -> research_trace::TaskEvidence {
    research_trace::TaskEvidence {
        source: source.to_string(),
        entries: entries.iter().map(evidence_entry_from_subagent).collect(),
        hop,
        generated_keywords,
        total_found: entries.len(),
    }
}

fn task_evidence_from_subagent_result(result: &SubagentResult) -> research_trace::TaskEvidence {
    research_trace::TaskEvidence {
        source: format!("{:?}", result.source),
        entries: result
            .entries
            .iter()
            .map(evidence_entry_from_subagent)
            .collect(),
        hop: result.hop,
        generated_keywords: result.generated_keywords.clone(),
        total_found: result.total_found,
    }
}

fn note_task_evidence(
    source_note: &str,
    content: &str,
    relevance_score: f32,
) -> research_trace::TaskEvidence {
    let entries = if content.is_empty() {
        vec![]
    } else {
        vec![SubagentEntry {
            title: source_note.to_string(),
            snippet: content.chars().take(800).collect(),
            url: None,
            authors: vec![],
            year: None,
            source: source_note.to_string(),
            relevance_score,
        }]
    };
    task_evidence_from_entries("local_note", &entries, 0, vec![source_note.to_string()])
}

fn task_context_fragment(
    task_id: &str,
    context: &research_trace::TaskContext,
) -> Option<ContextFragment> {
    let key = format!("task.{}.trace_context", task_id);
    let value = serde_json::to_value(context).ok()?;
    Some(ContextFragment {
        id: format!("{}_trace_context", task_id),
        key: key.clone(),
        value: value.clone(),
        source: ContextSource::Agent,
        layer: ContextLayer::Transient,
        created_at: now_millis(),
        ttl: None,
        hash: ContextFragment::compute_hash(&key, &value),
    })
}

fn enrich_task_with_execution_context(task: &mut AgentTask, context: &research_trace::TaskContext) {
    for sub_task in &mut task.sub_tasks {
        if matches!(
            sub_task.status,
            crate::ai::agent_scheduler::SubTaskStatus::Pending
                | crate::ai::agent_scheduler::SubTaskStatus::Running
        ) {
            sub_task.status = crate::ai::agent_scheduler::SubTaskStatus::Done;
        }
    }

    for evidence in &context.retrieval_evidence {
        let result = AgentScheduler::subagent_result_from_task_evidence(evidence);
        task.subagent_results.retain(|existing| {
            !(existing.source == result.source
                && existing.hop == result.hop
                && existing.generated_keywords == result.generated_keywords)
        });
        task.subagent_results.push(result);
    }

    if let Some(fragment) = task_context_fragment(&task.id, context) {
        task.context_fragments
            .retain(|existing| existing.key != fragment.key);
        task.context_fragments.push(fragment);
    }
    task.has_trace = true;
}

fn mdt_task_query(task: Option<&AgentTask>, cmd: &crate::cli::parser::CliCommand) -> String {
    if let Some(task) = task {
        if let Some(params) = &task.params {
            for key in ["query", "q", "topic", "target"] {
                if let Some(value) = params.get(key).map(|value| value.trim()) {
                    if !value.is_empty() {
                        return value.to_string();
                    }
                }
            }
        }

        let content = task.content.trim();
        if !content.is_empty() && !content.starts_with("/agent") && !content.starts_with('/') {
            return content.to_string();
        }

        let intent = task.intent.trim();
        if !intent.is_empty() && !intent.starts_with("/agent") && !intent.starts_with('/') {
            return intent.to_string();
        }
    }

    match cmd {
        crate::cli::parser::CliCommand::Search { query, .. } => query.clone(),
        crate::cli::parser::CliCommand::Explain { concept, .. }
        | crate::cli::parser::CliCommand::DeepDive { concept, .. } => concept.clone(),
        crate::cli::parser::CliCommand::Summarize { target, .. } => target.clone(),
        crate::cli::parser::CliCommand::FetchPapers { topic, .. } => topic.clone(),
        crate::cli::parser::CliCommand::DeepResearch { question, .. } => question.clone(),
        crate::cli::parser::CliCommand::Custom(intent) => intent.clone(),
        crate::cli::parser::CliCommand::CustomCard { prompt, .. } => prompt.clone(),
        _ => String::new(),
    }
}

fn mdt_token_budget(task: Option<&AgentTask>) -> usize {
    task.and_then(|task| task.params.as_ref())
        .and_then(|params| {
            params
                .get("token_budget")
                .or_else(|| params.get("tokenBudget"))
                .or_else(|| params.get("budget"))
        })
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(2000)
}

fn mdt_archive_path(
    task: Option<&AgentTask>,
    dualtrack_dir: &Path,
    timestamp: u64,
) -> Result<PathBuf, String> {
    let archive_param = task
        .and_then(|task| task.params.as_ref())
        .and_then(|params| {
            params
                .get("archive_path")
                .or_else(|| params.get("archivePath"))
                .or_else(|| params.get("output"))
        })
        .map(|value| value.trim())
        .filter(|value| !value.is_empty());

    if let Some(path) = archive_param {
        let path = PathBuf::from(path);
        if path.is_absolute() {
            return Err("MDT task archive_path must be relative".to_string());
        }
        let mut safe_path = PathBuf::new();
        for component in path.components() {
            match component {
                std::path::Component::Normal(part) => safe_path.push(part),
                std::path::Component::CurDir => {}
                std::path::Component::ParentDir
                | std::path::Component::RootDir
                | std::path::Component::Prefix(_) => {
                    return Err(format!("unsafe MDT archive path: {}", path.display()));
                }
            }
        }
        if safe_path.as_os_str().is_empty() {
            return Err("MDT task archive_path is empty".to_string());
        }
        return Ok(dualtrack_dir.join("mdt").join("snapshots").join(safe_path));
    }

    Ok(dualtrack_dir
        .join("mdt")
        .join("snapshots")
        .join(format!("mdt_{}.mdtz", timestamp)))
}

fn render_mdt_bundle(bundle: &MdtContextBundle) -> String {
    let mut output = format!(
        "## MDT Context\n\n- Query: `{}`\n- Items: {}\n- Remaining token budget: {}\n\n",
        bundle.query,
        bundle.items.len(),
        bundle.remaining_budget
    );

    for item in &bundle.items {
        output.push_str(&format!(
            "### {} ({:?})\n\n{}\n\n{}\n\n",
            item.node_id, item.level, item.reason, item.content
        ));
    }

    output
}

fn render_jsonld_bundle(bundle: &JsonLdContextBundle) -> String {
    let mut output = format!(
        "## JSON-LD Context\n\n- Query: `{}`\n- Items: {}\n- Remaining token budget: {}\n\n",
        bundle.query,
        bundle.items.len(),
        bundle.remaining_budget
    );

    for item in &bundle.items {
        output.push_str(&format!(
            "### {} ({:?})\n\n{}\n\n{}\n\n",
            item.node_id, item.level, item.reason, item.content
        ));
    }

    output
}

fn render_mdt_compat_index_output(
    jsonld_node_count: usize,
    jsonld_edge_count: usize,
    jsonld_dir: &Path,
    mdt_node_count: usize,
    mdt_edge_count: usize,
    mdt_dir: &Path,
) -> String {
    format!(
        "## JSON-LD Index (MDT compatibility mirror)\n\n- JSON-LD Nodes: {}\n- JSON-LD Edges: {}\n- JSON-LD Artifacts: `{}`\n- MDT Mirror Nodes: {}\n- MDT Mirror Edges: {}\n- MDT Mirror Artifacts: `{}`",
        jsonld_node_count,
        jsonld_edge_count,
        jsonld_dir.display(),
        mdt_node_count,
        mdt_edge_count,
        mdt_dir.display()
    )
}

fn memory_task_trace_heading(task_intent: TaskIntentType) -> &'static str {
    match task_intent {
        TaskIntentType::JsonLdIndex | TaskIntentType::JsonLdRead => "JSON-LD Memory Task",
        TaskIntentType::MdtIndex | TaskIntentType::MdtRead => {
            "JSON-LD Memory Task (MDT compatibility)"
        }
        TaskIntentType::MdtPack => "MDT Archive Task",
        _ => "AI Memory Task",
    }
}

fn mdt_bundle_evidence(bundle: &MdtContextBundle) -> research_trace::TaskEvidence {
    let entries = bundle
        .items
        .iter()
        .map(|item| research_trace::TaskEvidenceEntry {
            title: item.node_id.clone(),
            snippet: item.content.chars().take(500).collect(),
            url: None,
            authors: vec![],
            year: None,
            source: item.node_id.clone(),
            relevance_score: 1.0,
        })
        .collect::<Vec<_>>();

    research_trace::TaskEvidence {
        source: "mdt_read".to_string(),
        total_found: entries.len(),
        entries,
        hop: 0,
        generated_keywords: if bundle.query.is_empty() {
            vec![]
        } else {
            vec![bundle.query.clone()]
        },
    }
}

fn execute_mdt_task_if_requested<R: tauri::Runtime>(
    task_id: &str,
    task_intent: TaskIntentType,
    task: Option<&AgentTask>,
    cmd: &crate::cli::parser::CliCommand,
    state: &Mutex<AppState>,
    app_handle: &AppHandle<R>,
) -> Result<Option<(String, research_trace::TaskContext)>, String> {
    if !matches!(
        task_intent,
        TaskIntentType::JsonLdIndex
            | TaskIntentType::JsonLdRead
            | TaskIntentType::MdtIndex
            | TaskIntentType::MdtRead
            | TaskIntentType::MdtPack
    ) {
        return Ok(None);
    }

    let (vault_path, dualtrack_dir) = {
        let app = state.lock().map_err(|e| e.to_string())?;
        if app.vault.is_none() || app.vault_path.is_empty() {
            return Err("No vault open".to_string());
        }
        (PathBuf::from(&app.vault_path), app.dualtrack_dir.clone())
    };

    let started = std::time::Instant::now();
    let (output, mut context, trace_query, trace_source, trace_reason) = match task_intent {
        TaskIntentType::JsonLdIndex => {
            let index = crate::jsonld::indexer::index_vault_with_artifacts(&vault_path)?;
            let artifact_dir = dualtrack_dir.join("jsonld").join("indexes");
            let output = format!(
                "## JSON-LD Index\n\n- Nodes: {}\n- Edges: {}\n- Artifacts: `{}`",
                index.nodes.len(),
                index.edges.len(),
                artifact_dir.display()
            );
            let context = research_trace::TaskContext {
                intent: "jsonld_index".to_string(),
                ..research_trace::TaskContext::default()
            };
            (
                output,
                context,
                "jsonld_index".to_string(),
                "jsonld_index".to_string(),
                format!(
                    "Indexed {} JSON-LD nodes and {} edges",
                    index.nodes.len(),
                    index.edges.len()
                ),
            )
        }
        TaskIntentType::JsonLdRead => {
            let query = mdt_task_query(task, cmd);
            let token_budget = mdt_token_budget(task);
            let bundle = crate::jsonld::reader::JsonLdReader::load_context(
                &vault_path,
                &query,
                token_budget,
            )?;
            let legacy_bundle = crate::jsonld::reader::to_legacy_mdt_bundle(bundle.clone());
            let evidence = mdt_bundle_evidence(&legacy_bundle);
            let output = render_jsonld_bundle(&bundle);
            let context = research_trace::TaskContext {
                intent: format!("jsonld_read: {}", query),
                retrieval_evidence: vec![evidence],
                ..research_trace::TaskContext::default()
            };
            let item_count = bundle.items.len();
            (
                output,
                context,
                query,
                "jsonld_read".to_string(),
                format!("Read {} JSON-LD context items", item_count),
            )
        }
        TaskIntentType::MdtIndex => {
            let jsonld_index = crate::jsonld::indexer::index_vault_with_artifacts(&vault_path)?;
            let index = crate::mdt::indexer::index_vault_with_artifacts(&vault_path)?;
            let artifact_dir = dualtrack_dir.join("mdt").join("indexes");
            let jsonld_dir = dualtrack_dir.join("jsonld").join("indexes");
            let output = render_mdt_compat_index_output(
                jsonld_index.nodes.len(),
                jsonld_index.edges.len(),
                &jsonld_dir,
                index.nodes.len(),
                index.edges.len(),
                &artifact_dir,
            );
            let context = research_trace::TaskContext {
                intent: "mdt_index".to_string(),
                ..research_trace::TaskContext::default()
            };
            (
                output,
                context,
                "mdt_index".to_string(),
                "mdt_index".to_string(),
                format!(
                    "Indexed {} MDT nodes and {} edges",
                    index.nodes.len(),
                    index.edges.len()
                ),
            )
        }
        TaskIntentType::MdtRead => {
            let query = mdt_task_query(task, cmd);
            let token_budget = mdt_token_budget(task);
            let bundle = crate::jsonld::reader::JsonLdReader::load_context(
                &vault_path,
                &query,
                token_budget,
            )
            .map(crate::jsonld::reader::to_legacy_mdt_bundle)
            .or_else(|_| {
                crate::mdt::reader::MdtReader::load_context(&vault_path, &query, token_budget)
            })?;
            let evidence = mdt_bundle_evidence(&bundle);
            let output = render_mdt_bundle(&bundle);
            let context = research_trace::TaskContext {
                intent: format!("mdt_read: {}", query),
                retrieval_evidence: vec![evidence],
                ..research_trace::TaskContext::default()
            };
            let item_count = bundle.items.len();
            (
                output,
                context,
                query,
                "mdt_read".to_string(),
                format!("Read {} MDT context items", item_count),
            )
        }
        TaskIntentType::MdtPack => {
            let _ = crate::jsonld::indexer::index_vault_with_artifacts(&vault_path);
            let archive_path = mdt_archive_path(task, &dualtrack_dir, now_millis())?;
            let manifest = crate::mdt::archive::pack_mdtz(&vault_path, &archive_path)?;
            let output = format!(
                "## MDT Archive\n\n- Archive: `{}`\n- Files: {}\n- Nodes: {}\n- Edges: {}",
                archive_path.display(),
                manifest.files.len(),
                manifest.project.nodes.len(),
                manifest.project.edges.len()
            );
            let context = research_trace::TaskContext {
                intent: format!("mdt_pack: {}", archive_path.display()),
                ..research_trace::TaskContext::default()
            };
            (
                output,
                context,
                "mdt_pack".to_string(),
                "mdt_pack".to_string(),
                format!("Packed {} files into MDT archive", manifest.files.len()),
            )
        }
        _ => unreachable!(),
    };

    context.phase_timings.insert(
        format!("{}_ms", task_intent.as_str()),
        started.elapsed().as_millis() as u64,
    );

    let _ = research_trace::write_path_log(
        &dualtrack_dir,
        task_id,
        0,
        &trace_query,
        &trace_source,
        &[],
        &[],
        &trace_reason,
        Some(&context),
    );
    let _ = research_trace::write_cot_log(
        &dualtrack_dir,
        task_id,
        &format!(
            "## {}\n\nIntent: {}\n\n{}",
            memory_task_trace_heading(task_intent),
            task_intent.as_str(),
            trace_reason
        ),
        Some(&context),
    );

    let _ = app_handle.emit(
        "agent-result",
        serde_json::json!({
            "type": task_intent.as_str(),
            "result": output,
        }),
    );

    Ok(Some((output, context)))
}

async fn execute_agent_task_async<R: tauri::Runtime>(
    task_id: &str,
    cmd: crate::cli::parser::CliCommand,
    state: &Mutex<AppState>,
    ai_state: &Mutex<AiState>,
    app_handle: AppHandle<R>,
    cancel_token: Option<CancellationToken>,
) -> Result<(String, research_trace::TaskContext), String> {
    let task_snapshot = {
        let ai = ai_state.lock().map_err(|e| e.to_string())?;
        ai.agent_scheduler.get_task(task_id).cloned()
    };
    let task_intent = task_snapshot
        .as_ref()
        .and_then(|task| task.task_intent)
        .unwrap_or_else(|| task_intent_for_cli_command(&cmd, None));
    let _task_intent = match &cmd {
        crate::cli::parser::CliCommand::Search { query, .. } => query.clone(),
        crate::cli::parser::CliCommand::Explain { concept, .. }
        | crate::cli::parser::CliCommand::DeepDive { concept, .. } => concept.clone(),
        crate::cli::parser::CliCommand::Summarize { target, .. } => target.clone(),
        crate::cli::parser::CliCommand::FetchPapers { topic, .. } => topic.clone(),
        crate::cli::parser::CliCommand::DeepResearch { question, .. } => question.clone(),
        crate::cli::parser::CliCommand::Custom(intent) => intent.clone(),
        _ => String::new(),
    };
    let sandbox_policy = sandbox_policy_for_task(task_id, &cmd, ai_state)?;

    if let Some(result) = execute_mdt_task_if_requested(
        task_id,
        task_intent,
        task_snapshot.as_ref(),
        &cmd,
        state,
        &app_handle,
    )? {
        return Ok(result);
    }

    // Orchestrator verification track — use Scientist flow
    {
        let ai_locked = ai_state.lock().map_err(|e| e.to_string())?;
        if let Some(task) = ai_locked.agent_scheduler.get_task(task_id) {
            if task.card_type.as_deref() == Some("orchestrator-track") {
                let task_clone = task.clone();
                let router = ai_locked.llm_router.clone();
                drop(ai_locked);
                if router.is_available() {
                    let result = Scientist::refine(&task_clone, &router, None, None).await;
                    let related_notes = result
                        .clean_knowledge
                        .sources
                        .iter()
                        .map(|source| source.key.clone())
                        .collect::<Vec<_>>();
                    let violation_count = result
                        .verification
                        .as_ref()
                        .map(|verification| verification.violations.len())
                        .unwrap_or(0);
                    try_upsert_bridge_proposal(state, Some(&app_handle), |trust_snapshot, now| {
                        BridgeProposal::for_scientist_result(
                            task_id,
                            &task_clone.intent,
                            result.clean_knowledge.claims.len(),
                            violation_count,
                            &result.kernel_name,
                            related_notes,
                            trust_snapshot,
                            now,
                        )
                    });
                    let output = format!(
                        "## Orchestrator Track Verification\n\n\
                         **Confidence**: {:.1}%\n\
                         **Claims analyzed**: {}\n\
                         **Sources**: {}\n\
                         **Violations found**: {}\n\n\
                         {}",
                        result.overall_confidence * 100.0,
                        result.clean_knowledge.claims.len(),
                        result.clean_knowledge.sources.len(),
                        result
                            .verification
                            .as_ref()
                            .map(|v| v.violations.len())
                            .unwrap_or(0),
                        result
                            .verification
                            .as_ref()
                            .map(|v| format!("{:?}", v.violations))
                            .unwrap_or_default(),
                    );
                    let ctx = research_trace::TaskContext {
                        intent: format!("track: {}", task_id),
                        ..research_trace::TaskContext::default()
                    };
                    return Ok((output, ctx));
                } else {
                    let output = format!(
                        "## Orchestrator Track (no LLM)\n\n\
                         Track {} has {} claims from parent. Configure an LLM API key for full verification.",
                        task_id,
                        task_clone.sub_tasks.iter().filter(|st| matches!(st.status, crate::ai::agent_scheduler::SubTaskStatus::Done)).count(),
                    );
                    let ctx = research_trace::TaskContext {
                        intent: format!("track: {}", task_id),
                        ..research_trace::TaskContext::default()
                    };
                    return Ok((output, ctx));
                }
            }
        }
    }

    match &cmd {
        crate::cli::parser::CliCommand::Search { query, top_k } => {
            let retrieve_start = std::time::Instant::now();
            let app = state.lock().map_err(|e| e.to_string())?;
            let dualtrack_dir = app.dualtrack_dir.clone();
            let entries: Vec<SubagentEntry> = if let Some(ref sync_engine) = app.sync_engine {
                let store = sync_engine.store();
                let sr = store.search_text(query, *top_k);
                sr.into_iter()
                    .map(|r| SubagentEntry {
                        title: if r.heading_context.is_empty() {
                            r.source_file.clone()
                        } else {
                            r.heading_context.clone()
                        },
                        snippet: r.chunk_text,
                        url: None,
                        authors: vec![],
                        year: None,
                        source: r.source_file,
                        relevance_score: r.score,
                    })
                    .collect()
            } else {
                vec![]
            };
            drop(app);

            let _ = research_trace::write_path_log(
                &dualtrack_dir,
                task_id,
                0,
                query,
                "local_vector",
                &[],
                &[],
                &format!(
                    "Vector search for '{}' returned {} results",
                    query,
                    entries.len()
                ),
                None,
            );

            let cot_content = format!(
                "## Query\n\n```\n{}\n```\n\n## Results\n- Count: {}\n- Source: local_vector search",
                query,
                entries.len()
            );
            let _ = research_trace::write_cot_log(&dualtrack_dir, task_id, &cot_content, None);

            let retrieve_ms = retrieve_start.elapsed().as_millis() as u64;
            let mut phase_timings = std::collections::HashMap::new();
            phase_timings.insert("retrieve_ms".to_string(), retrieve_ms);
            let retrieval_evidence = vec![task_evidence_from_entries(
                "local_vector",
                &entries,
                0,
                vec![query.clone()],
            )];

            if entries.is_empty() {
                let ctx = research_trace::TaskContext {
                    intent: query.clone(),
                    phase_timings,
                    retrieval_evidence,
                    ..research_trace::TaskContext::default()
                };
                return Ok((format!("No results found for: \"{}\"", query), ctx));
            }

            let mut output = format!("## Search Results: \"{}\"\n\n", query);
            for (i, entry) in entries.iter().enumerate() {
                output.push_str(&format!(
                    "{}. **{}** (from `{}`)\n   {}\n\n",
                    i + 1,
                    entry.title,
                    entry.source,
                    entry.snippet.chars().take(150).collect::<String>()
                ));
            }

            let _ = app_handle.emit(
                "agent-result",
                serde_json::json!({
                    "type": "search",
                    "query": query,
                    "results": entries,
                }),
            );

            let ctx = research_trace::TaskContext {
                intent: query.clone(),
                phase_timings,
                retrieval_evidence,
                ..research_trace::TaskContext::default()
            };
            Ok((output, ctx))
        }

        crate::cli::parser::CliCommand::Explain { concept, .. }
        | crate::cli::parser::CliCommand::DeepDive { concept, .. } => {
            let concept = concept.clone();
            let retrieve_start = std::time::Instant::now();
            let (dualtrack_dir, entries, contexts) = {
                let app = state.lock().map_err(|e| e.to_string())?;
                let dualtrack_dir = app.dualtrack_dir.clone();
                let entries: Vec<SubagentEntry> = if let Some(ref sync_engine) = app.sync_engine {
                    let store = sync_engine.store();
                    store
                        .search_text(&concept, 5)
                        .into_iter()
                        .map(|r| SubagentEntry {
                            title: if r.heading_context.is_empty() {
                                r.source_file.clone()
                            } else {
                                r.heading_context.clone()
                            },
                            snippet: r.chunk_text,
                            url: None,
                            authors: vec![],
                            year: None,
                            source: r.source_file,
                            relevance_score: r.score,
                        })
                        .collect()
                } else {
                    vec![]
                };
                let contexts: Vec<String> = entries
                    .iter()
                    .map(|e| format!("[{}] {}", e.source, e.snippet))
                    .collect();
                let _ = research_trace::write_path_log(
                    &dualtrack_dir,
                    task_id,
                    0,
                    &concept,
                    "local_vector",
                    &[],
                    &[],
                    &format!(
                        "Context retrieval for '{}' returned {} chunks",
                        concept,
                        contexts.len()
                    ),
                    None,
                );
                let _link_graph = app.link_graph.clone();
                drop(_link_graph);
                (dualtrack_dir, entries, contexts)
            };
            let retrieve_ms = retrieve_start.elapsed().as_millis() as u64;
            let retrieval_evidence = vec![task_evidence_from_entries(
                "local_vector",
                &entries,
                0,
                vec![concept.clone()],
            )];

            let mut router = {
                let ai = ai_state.lock().map_err(|e| e.to_string())?;
                if !ai.llm_router.is_available() {
                    let content = if contexts.is_empty() {
                        format!(
                            "## {}\n\nNo local notes found. To get AI-powered analysis, configure an LLM API key in Settings.\n\n\
                             Supported providers: Google Gemini, OpenAI.\n\
                             The app works fully offline for note-taking and search.",
                            concept
                        )
                    } else {
                        let mut buf = format!("## {}\n\n### From your notes:\n\n", concept);
                        for ctx in &contexts {
                            buf.push_str(&format!(
                                "- {}\n",
                                ctx.chars().take(200).collect::<String>()
                            ));
                        }
                        buf.push_str(
                            "\n\n*Add an LLM API key to enable AI-powered deep analysis.*",
                        );
                        buf
                    };

                    let _ = app_handle.emit(
                        "agent-result",
                        serde_json::json!({
                            "type": "analysis",
                            "concept": concept,
                            "result": content,
                        }),
                    );

                    if !contexts.is_empty() {
                        let ghost_blocks: Vec<GhostBlock> = contexts
                            .iter()
                            .enumerate()
                            .map(|(i, c)| GhostBlock {
                                block_id: format!("ghost-block-{}", i),
                                content: c.clone(),
                                operation: GhostOp::Suggestion,
                                after_block_id: None,
                                heading_context: concept.clone(),
                                context: vec![],
                                verified: None,
                                verification_result: None,
                            })
                            .collect();

                        let target_note = format!("{}.md", concept);
                        let block_count = ghost_blocks.len();
                        let ghost_id = {
                            let ai = ai_state.lock().map_err(|e| e.to_string())?;
                            match ai.ghost_store.create(
                                &target_note,
                                &format!("AI analysis for: {}", concept),
                                ghost_blocks,
                                Some(task_id.to_string()),
                            ) {
                                Ok(ghost_note) => Some(ghost_note.id),
                                Err(e) => {
                                    tracing::warn!("Failed to create ghost note: {}", e);
                                    None
                                }
                            }
                        };
                        if let Some(ghost_id) = ghost_id.as_deref() {
                            try_upsert_bridge_proposal(
                                state,
                                Some(&app_handle),
                                |trust_snapshot, now| {
                                    BridgeProposal::for_ghost(
                                        ghost_id,
                                        &target_note,
                                        block_count,
                                        trust_snapshot,
                                        now,
                                    )
                                },
                            );
                        }

                        let mut phase_timings = std::collections::HashMap::new();
                        phase_timings.insert("retrieve_ms".to_string(), retrieve_ms);

                        let mut ghost_ids = Vec::new();
                        if let Some(id) = ghost_id {
                            ghost_ids.push(id);
                        }

                        let ctx = research_trace::TaskContext {
                            intent: concept.clone(),
                            ghost_ids,
                            phase_timings,
                            retrieval_evidence,
                            ..research_trace::TaskContext::default()
                        };
                        return Ok((content, ctx));
                    }

                    let mut phase_timings = std::collections::HashMap::new();
                    phase_timings.insert("retrieve_ms".to_string(), retrieve_ms);
                    let ctx = research_trace::TaskContext {
                        intent: concept.clone(),
                        phase_timings,
                        retrieval_evidence,
                        ..research_trace::TaskContext::default()
                    };
                    return Ok((content, ctx));
                }
                ai.llm_router.clone()
            };

            let synthesize_start = std::time::Instant::now();
            let system = "You are a knowledge assistant. Analyze the concept based on the user's notes and your knowledge. Use Markdown formatting. Include a ## Summary section.";
            let prompt = if contexts.is_empty() {
                format!("Explain: {}\n\n(The user has no local notes on this topic. Provide a general explanation.)", concept)
            } else {
                format!(
                    "Explain: {}\n\nRelevant notes from user's vault:\n{}\n\nProvide analysis that connects these notes with broader knowledge.",
                    concept,
                    contexts.join("\n\n")
                )
            };

            match router.complete(system, &prompt, cancel_token.clone()).await {
                Ok(response) => {
                    let synthesize_ms = synthesize_start.elapsed().as_millis() as u64;
                    let cot_content = format!(
                        "## Prompt\n\n```\n{}\n```\n\n## Tokens\n- Input: {}\n- Output: {}\n## Model\n{}",
                        prompt, response.tokens_in, response.tokens_out, response.model_used,
                    );
                    let _ =
                        research_trace::write_cot_log(&dualtrack_dir, task_id, &cot_content, None);

                    let _ = app_handle.emit(
                        "agent-result",
                        serde_json::json!({
                            "type": "analysis",
                            "concept": concept,
                            "result": response.text,
                            "model": response.model_used,
                            "tokens": response.tokens_in + response.tokens_out,
                        }),
                    );

                    let blocks: Vec<GhostBlock> = response
                        .text
                        .split("\n\n")
                        .enumerate()
                        .map(|(i, para)| GhostBlock {
                            block_id: format!("ghost-block-{}", i),
                            content: para.to_string(),
                            operation: GhostOp::Suggestion,
                            after_block_id: None,
                            heading_context: concept.clone(),
                            context: vec![],
                            verified: None,
                            verification_result: None,
                        })
                        .collect();

                    let write_ghost_start = std::time::Instant::now();
                    let mut ghost_ids = Vec::new();
                    let target_note = format!("{}.md", concept);
                    let block_count = blocks.len();
                    let ghost_id = {
                        let ai = ai_state.lock().map_err(|e| e.to_string())?;
                        match ai.ghost_store.create(
                            &target_note,
                            &format!("AI analysis: {}", concept),
                            blocks,
                            Some(task_id.to_string()),
                        ) {
                            Ok(ghost_note) => Some(ghost_note.id),
                            Err(e) => {
                                tracing::warn!("Failed to store ghost: {}", e);
                                None
                            }
                        }
                    };
                    if let Some(ghost_id) = ghost_id {
                        try_upsert_bridge_proposal(
                            state,
                            Some(&app_handle),
                            |trust_snapshot, now| {
                                BridgeProposal::for_ghost(
                                    &ghost_id,
                                    &target_note,
                                    block_count,
                                    trust_snapshot,
                                    now,
                                )
                            },
                        );
                        ghost_ids.push(ghost_id);
                    }
                    let write_ghost_ms = write_ghost_start.elapsed().as_millis() as u64;

                    let mut phase_timings = std::collections::HashMap::new();
                    phase_timings.insert("retrieve_ms".to_string(), retrieve_ms);
                    phase_timings.insert("synthesize_ms".to_string(), synthesize_ms);
                    phase_timings.insert("write_ghost_ms".to_string(), write_ghost_ms);

                    let ctx = research_trace::TaskContext {
                        intent: concept.clone(),
                        ghost_ids,
                        phase_timings,
                        retrieval_evidence,
                        ..research_trace::TaskContext::default()
                    };
                    Ok((response.text, ctx))
                }
                Err(e) => {
                    tracing::error!("LLM call failed: {}", e);
                    Err(format!(
                        "AI service error: {}. Please check your API key in Settings.",
                        e
                    ))
                }
            }
        }

        crate::cli::parser::CliCommand::Summarize { target, .. } => {
            let target = target.clone();
            let retrieve_start = std::time::Instant::now();
            let (dualtrack_dir, content) = {
                let app = state.lock().map_err(|e| e.to_string())?;
                let dualtrack_dir = app.dualtrack_dir.clone();
                let content = if !target.is_empty() {
                    let vault = app.vault.as_ref().ok_or("No vault open")?;
                    vault.read_note(&target).unwrap_or_default()
                } else {
                    "No target specified for summarization.".to_string()
                };
                (dualtrack_dir, content)
            };

            if content.is_empty() {
                let retrieve_ms = retrieve_start.elapsed().as_millis() as u64;
                let mut phase_timings = std::collections::HashMap::new();
                phase_timings.insert("retrieve_ms".to_string(), retrieve_ms);
                let ctx = research_trace::TaskContext {
                    intent: target.clone(),
                    phase_timings,
                    retrieval_evidence: vec![note_task_evidence(&target, "", 0.0)],
                    ..research_trace::TaskContext::default()
                };
                return Ok(("Target note is empty or not found.".to_string(), ctx));
            }

            let _ = research_trace::write_path_log(
                &dualtrack_dir,
                task_id,
                0,
                &target,
                "local_note",
                &[],
                &[],
                &format!("Read note '{}' ({} chars)", target, content.len()),
                None,
            );

            let mut router = {
                let ai = ai_state.lock().map_err(|e| e.to_string())?;
                if !ai.llm_router.is_available() {
                    let first_line = content.lines().next().unwrap_or("");
                    let retrieve_ms = retrieve_start.elapsed().as_millis() as u64;
                    let mut phase_timings = std::collections::HashMap::new();
                    phase_timings.insert("retrieve_ms".to_string(), retrieve_ms);
                    let ctx = research_trace::TaskContext {
                        intent: target.clone(),
                        phase_timings,
                        retrieval_evidence: vec![note_task_evidence(&target, &content, 1.0)],
                        ..research_trace::TaskContext::default()
                    };
                    return Ok((format!(
                        "## Summary (offline)\n\nNote: `{}` ({:?} chars)\nFirst line: {}\n\n*Configure an LLM API key for AI summaries.*",
                        target, content.len(), first_line
                    ), ctx));
                }
                ai.llm_router.clone()
            };

            let synthesize_start = std::time::Instant::now();
            let retrieve_ms = retrieve_start.elapsed().as_millis() as u64;
            let system = "Summarize the following note in Markdown. Include key points, main arguments, and a TL;DR.";
            match router
                .complete(system, &content, cancel_token.clone())
                .await
            {
                Ok(response) => {
                    let synthesize_ms = synthesize_start.elapsed().as_millis() as u64;
                    let cot_content = format!(
                        "## Prompt\n\n```\n{}\n```\n\n## Tokens\n- Input: {}\n- Output: {}\n## Model\n{}",
                        content, response.tokens_in, response.tokens_out, response.model_used,
                    );
                    let _ =
                        research_trace::write_cot_log(&dualtrack_dir, task_id, &cot_content, None);

                    let _ = app_handle.emit(
                        "agent-result",
                        serde_json::json!({
                            "type": "summary",
                            "target": target,
                            "result": response.text,
                        }),
                    );

                    let mut phase_timings = std::collections::HashMap::new();
                    phase_timings.insert("retrieve_ms".to_string(), retrieve_ms);
                    phase_timings.insert("synthesize_ms".to_string(), synthesize_ms);

                    let ctx = research_trace::TaskContext {
                        intent: target.clone(),
                        phase_timings,
                        retrieval_evidence: vec![note_task_evidence(&target, &content, 1.0)],
                        ..research_trace::TaskContext::default()
                    };
                    Ok((response.text, ctx))
                }
                Err(e) => Err(e),
            }
        }

        crate::cli::parser::CliCommand::FetchPapers { topic, .. } => {
            let topic = topic.clone();
            let retrieve_start = std::time::Instant::now();
            let dualtrack_dir = {
                let app = state.lock().map_err(|e| e.to_string())?;
                app.dualtrack_dir.clone()
            };

            let keywords: Vec<String> = topic.split_whitespace().map(|s| s.to_string()).collect();

            // Try subagent (arXiv + Semantic Scholar) for real papers
            let subagent = {
                let ai = ai_state.lock().map_err(|e| e.to_string())?;
                ai.subagent.clone()
            };

            let (arxiv_results, s2_results) = if let Some(ref sub) = subagent {
                let arxiv_fut = async {
                    if sandbox_policy.allows_tool("arxiv_search") {
                        sub.search_arxiv(&keywords, 10).await
                    } else {
                        vec![]
                    }
                };
                let s2_fut = async {
                    if sandbox_policy.allows_tool("semantic_scholar_search") {
                        sub.search_semantic_scholar(&keywords, 10).await
                    } else {
                        vec![]
                    }
                };
                tokio::join!(arxiv_fut, s2_fut)
            } else {
                (vec![], vec![])
            };

            let retrieval_evidence = vec![
                task_evidence_from_entries("arxiv", &arxiv_results, 0, keywords.clone()),
                task_evidence_from_entries("semantic_scholar", &s2_results, 0, keywords.clone()),
            ];
            let mut seen_titles: std::collections::HashSet<String> =
                std::collections::HashSet::new();
            let mut all_results: Vec<crate::ai::subagent::SubagentEntry> = Vec::new();

            for entry in arxiv_results.iter().chain(s2_results.iter()) {
                let key = entry.title.to_lowercase().trim().to_string();
                if seen_titles.insert(key) {
                    all_results.push(entry.clone());
                }
            }

            let _ = research_trace::write_path_log(
                &dualtrack_dir,
                task_id,
                0,
                &topic,
                if all_results.is_empty() {
                    "llm_suggest"
                } else {
                    "arxiv_s2"
                },
                &[],
                &[],
                &format!(
                    "Fetched {} papers for topic '{}' via arXiv + Semantic Scholar",
                    all_results.len(),
                    topic
                ),
                None,
            );

            let retrieve_ms = retrieve_start.elapsed().as_millis() as u64;

            if !all_results.is_empty() {
                let synthesize_start = std::time::Instant::now();
                let mut output = format!("## Papers: \"{}\"\n\n", topic);
                output.push_str("*Results from arXiv & Semantic Scholar*\n\n");
                for (i, entry) in all_results.iter().enumerate() {
                    output.push_str(&format!(
                        "{}. **{}** ({})\n",
                        i + 1,
                        entry.title,
                        entry.year.map_or("n.d.".to_string(), |y| y.to_string())
                    ));
                    if !entry.authors.is_empty() {
                        output.push_str(&format!("   Authors: {}\n", entry.authors.join(", ")));
                    }
                    if !entry.snippet.is_empty() {
                        output.push_str(&format!(
                            "   {}\n",
                            entry.snippet.chars().take(300).collect::<String>()
                        ));
                    }
                    if let Some(ref url) = entry.url {
                        output.push_str(&format!("   [View paper]({})\n", url));
                    }
                    output.push('\n');
                }

                let synthesize_ms = synthesize_start.elapsed().as_millis() as u64;
                let _ = research_trace::write_cot_log(
                    &dualtrack_dir,
                    task_id,
                    &format!(
                        "## FetchPapers via arXiv + Semantic Scholar\n\nTopic: {}\nResults: {}",
                        topic,
                        all_results.len()
                    ),
                    None,
                );

                let _ = app_handle.emit(
                    "agent-result",
                    serde_json::json!({
                        "type": "fetch_papers",
                        "topic": topic,
                        "result": output,
                        "source": "arxiv_s2",
                    }),
                );

                let mut phase_timings = std::collections::HashMap::new();
                phase_timings.insert("retrieve_ms".to_string(), retrieve_ms);
                phase_timings.insert("synthesize_ms".to_string(), synthesize_ms);

                let ctx = research_trace::TaskContext {
                    intent: topic.clone(),
                    phase_timings,
                    retrieval_evidence,
                    ..research_trace::TaskContext::default()
                };
                return Ok((output, ctx));
            }

            // Fallback: LLM-suggested with disclaimer
            let mut router = {
                let ai = ai_state.lock().map_err(|e| e.to_string())?;
                if !ai.llm_router.is_available() {
                    let mut phase_timings = std::collections::HashMap::new();
                    phase_timings.insert("retrieve_ms".to_string(), retrieve_ms);
                    let ctx = research_trace::TaskContext {
                        intent: topic.clone(),
                        phase_timings,
                        retrieval_evidence,
                        ..research_trace::TaskContext::default()
                    };
                    return Ok((
                        format!(
                            "Paper fetching requires an LLM API key. Topic: \"{}\"\n\
                         The agent will search your local notes for related content.",
                            topic
                        ),
                        ctx,
                    ));
                }
                ai.llm_router.clone()
            };

            let synthesize_start = std::time::Instant::now();
            let system = "You are a research assistant. Based on the topic, suggest 3-5 relevant papers or resources. For each, provide: title, authors (if known), key contribution, and relevance. Format in Markdown.";
            let prompt = format!(
                "Topic: {}\n\nSuggest relevant academic papers and resources.",
                topic
            );

            match router.complete(system, &prompt, cancel_token.clone()).await {
                Ok(response) => {
                    let synthesize_ms = synthesize_start.elapsed().as_millis() as u64;
                    let cot_content = format!(
                        "## Prompt (LLM Fallback)\n\n```\n{}\n```\n\n## Tokens\n- Input: {}\n- Output: {}\n## Model\n{}",
                        prompt, response.tokens_in, response.tokens_out, response.model_used,
                    );
                    let _ =
                        research_trace::write_cot_log(&dualtrack_dir, task_id, &cot_content, None);

                    let disclaimer = "\n\n---\n*AI-generated suggestions, not verified \u{2014} use real API (arXiv, Semantic Scholar) for confirmation.*\n";
                    let full_text = format!(
                        "## Papers: \"{}\" (LLM Suggestions)\n\n{}\n{}",
                        topic, response.text, disclaimer
                    );

                    let _ = app_handle.emit(
                        "agent-result",
                        serde_json::json!({
                            "type": "fetch_papers",
                            "topic": topic,
                            "result": full_text,
                            "source": "llm_fallback",
                        }),
                    );

                    let mut phase_timings = std::collections::HashMap::new();
                    phase_timings.insert("retrieve_ms".to_string(), retrieve_ms);
                    phase_timings.insert("synthesize_ms".to_string(), synthesize_ms);

                    let ctx = research_trace::TaskContext {
                        intent: topic.clone(),
                        phase_timings,
                        retrieval_evidence,
                        ..research_trace::TaskContext::default()
                    };
                    Ok((full_text, ctx))
                }
                Err(e) => Err(e),
            }
        }

        crate::cli::parser::CliCommand::Status => {
            let ai = ai_state.lock().map_err(|e| e.to_string())?;
            let stats = ai.agent_scheduler.stats();
            let llm_ready = ai.llm_router.is_available();
            let budget = ai.llm_router.budget_remaining();
            let ctx = research_trace::TaskContext::default();
            Ok((
                format!(
                    "Agent: {} tasks | Queued: {} | Running: {} | Done: {} | Failed: {}\n\
                 LLM: {} | Budget remaining: ${:.4}",
                    stats.queued + stats.running + stats.done + stats.failed,
                    stats.queued,
                    stats.running,
                    stats.done,
                    stats.failed,
                    if llm_ready { "connected" } else { "no API key" },
                    budget,
                ),
                ctx,
            ))
        }

        crate::cli::parser::CliCommand::DiffReview => {
            let ai = ai_state.lock().map_err(|e| e.to_string())?;
            let ghosts = ai.ghost_store.list_all();
            let ctx = research_trace::TaskContext::default();
            if ghosts.is_empty() {
                Ok(("No pending AI suggestions to review.".to_string(), ctx))
            } else {
                let mut output = format!("## Pending AI Suggestions ({})\n\n", ghosts.len());
                for g in &ghosts {
                    output.push_str(&format!(
                        "- **{}**: {} ({} blocks, {})\n",
                        g.id,
                        g.task_description,
                        g.suggested_blocks.len(),
                        g.source_note,
                    ));
                }
                output.push_str("\nUse the Diff panel to review and merge suggestions.");
                Ok((output, ctx))
            }
        }

        crate::cli::parser::CliCommand::Config { model } => {
            let ctx = research_trace::TaskContext::default();
            if let Some(m) = model {
                let mut ai = ai_state.lock().map_err(|e| e.to_string())?;
                let mut cfg = ai.llm_router.config();
                cfg.llm_model = m.clone();
                ai.llm_router.update_config(cfg);
                Ok((format!("Model set to: {}", m), ctx))
            } else {
                let ai = ai_state.lock().map_err(|e| e.to_string())?;
                Ok((
                    format!(
                        "Current model: {} | Provider: {} | LLM: {}",
                        if ai.llm_router.is_available() {
                            "API connected"
                        } else {
                            "no API key"
                        },
                        "gemini/openai",
                        if ai.llm_router.is_available() {
                            "available"
                        } else {
                            "unconfigured"
                        },
                    ),
                    ctx,
                ))
            }
        }

        crate::cli::parser::CliCommand::Dream => {
            let app = state.lock().map_err(|e| e.to_string())?;
            let dualtrack_dir = app.dualtrack_dir.clone();
            let store = if let Some(ref sync_engine) = app.sync_engine {
                sync_engine.store()
            } else {
                return Err("No sync engine available. Open a vault first.".to_string());
            };

            let mut dream_engine = match app.dream_engine.clone() {
                Some(de) => de,
                None => {
                    let mut de = DreamEngine::new();
                    de.set_dualtrack_dir(&dualtrack_dir);
                    de
                }
            };

            let stats = dream_engine.run_cycle(store);
            let insights = dream_engine.get_insights().to_vec();
            let dream_edges = dream_engine.export_graph_edges();
            let om = app.output_manager.clone();
            let dream_dualtrack = dualtrack_dir.clone();
            drop(app);

            {
                let mut app = state.lock().map_err(|e| e.to_string())?;
                for edge in dream_edges {
                    app.link_graph.add_graph_edge(edge);
                }
                app.dream_engine = Some(dream_engine);
            }

            if let Some(ref om) = om {
                let claims: Vec<String> = insights.iter().map(|i| i.summary.clone()).collect();
                let confidence_map: std::collections::HashMap<String, f32> = insights
                    .iter()
                    .map(|i| (i.id.clone(), i.confidence))
                    .collect();
                let knowledge = CleanKnowledge {
                    claims,
                    sources: vec![],
                    confidence_map,
                };
                let overall_confidence = if insights.is_empty() {
                    0.0
                } else {
                    insights.iter().map(|i| i.confidence).sum::<f32>() / insights.len() as f32
                };
                let scientist_result =
                    Scientist::build_result(knowledge, None, None, None, overall_confidence);
                om.trigger(
                    &HookTrigger::OnDreamCycle,
                    &scientist_result,
                    "dream-cycle",
                    &dream_dualtrack,
                )
                .await;
            }

            let _ = app_handle.emit(
                "agent-result",
                serde_json::json!({
                    "type": "dream",
                    "stats": stats,
                    "insights": insights,
                }),
            );

            let ctx = research_trace::TaskContext::default();
            Ok((
                format!(
                    "## Dream Cycle Complete\n\n\
                 **NREM Phase**\n\
                 - Connections strengthened: {}\n\
                 - Connections pruned: {}\n\
                 - Memories processed: {}\n\n\
                 **REM Phase**\n\
                 - Bridges created: {}\n\
                 - Memories processed: {}\n\n\
                 **Insight Phase**\n\
                 - Communities found: {}\n\
                 - Summaries generated: {}\n\n\
                 **Total**\n\
                 - Memories processed: {}\n\
                 - Duration: {}ms\n\
                 - Insights discovered: {}",
                    stats.nrem_connections_strengthened,
                    stats.nrem_connections_pruned,
                    stats.total_memories_processed,
                    stats.rem_bridges_created,
                    stats.total_memories_processed,
                    stats.insight_communities_found,
                    stats.insight_summaries_generated,
                    stats.total_memories_processed,
                    stats.duration_ms,
                    insights.len()
                ),
                ctx,
            ))
        }
        crate::cli::parser::CliCommand::DeepResearch {
            question,
            max_iterations: _,
        } => {
            if !sandbox_policy.allows_tool("deep_research") {
                return Err("Deep research blocked by sandbox policy".to_string());
            }
            let question = question.clone();
            let (dualtrack_dir, vector_store_path) = {
                let app = state.lock().map_err(|e| e.to_string())?;
                (app.dualtrack_dir.clone(), app.vector_store_path.clone())
            };
            let vector_store = vector_store_path
                .to_str()
                .and_then(|path| VectorStore::open(path).ok());

            let (sub, router) = {
                let ai = ai_state.lock().map_err(|e| e.to_string())?;
                let router = ai.llm_router.clone();
                let sub = ai.subagent.as_ref().cloned();
                (sub, router)
            };

            let sub = sub.unwrap_or_else(|| Subagent::new(None));
            let outcome = sub
                .execute_deep_research(
                    &question,
                    vector_store.as_ref(),
                    Some(router),
                    task_id,
                    &dualtrack_dir,
                    Some(&sandbox_policy),
                )
                .await
                .map_err(|e| format!("Deep research failed: {}", e))?;
            let crate::ai::subagent::DeepResearchOutcome {
                report,
                ghost_ids,
                graph_artifacts,
                retrieval_results,
            } = outcome;
            let retrieval_evidence = retrieval_results
                .iter()
                .map(task_evidence_from_subagent_result)
                .collect::<Vec<_>>();
            let ctx = research_trace::TaskContext {
                intent: question.clone(),
                ghost_ids,
                research_graph_path: graph_artifacts
                    .as_ref()
                    .map(|artifacts| artifacts.graph_json.clone()),
                research_graph_report_path: graph_artifacts
                    .as_ref()
                    .map(|artifacts| artifacts.graph_report.clone()),
                retrieval_evidence,
                ..research_trace::TaskContext::default()
            };

            let (task_for_scientist, router_for_scientist) = {
                let ai = ai_state.lock().map_err(|e| e.to_string())?;
                let task = ai.agent_scheduler.get_task(task_id).cloned();
                let router = ai.llm_router.clone();
                (task, router)
            };
            let bridge_for_scientist = match bridge_context(state) {
                Ok(context) => context,
                Err(error) => {
                    tracing::warn!("Failed to prepare scientist bridge proposal: {}", error);
                    None
                }
            };

            if let Some(mut task) = task_for_scientist {
                enrich_task_with_execution_context(&mut task, &ctx);
                let app_for_bridge = app_handle.clone();
                tauri::async_runtime::spawn(async move {
                    let result = crate::harness::scientist::Scientist::refine(
                        &task,
                        &router_for_scientist,
                        None,
                        None,
                    )
                    .await;
                    tracing::info!(
                        "Scientist refined {} claims, conf={:.2}",
                        result.clean_knowledge.claims.len(),
                        result.overall_confidence
                    );
                    if let Some((store, trust_snapshot)) = bridge_for_scientist {
                        let related_notes = result
                            .clean_knowledge
                            .sources
                            .iter()
                            .map(|source| source.key.clone())
                            .collect::<Vec<_>>();
                        let violation_count = result
                            .verification
                            .as_ref()
                            .map(|verification| verification.violations.len())
                            .unwrap_or(0);
                        let proposal = BridgeProposal::for_scientist_result(
                            &task.id,
                            &task.intent,
                            result.clean_knowledge.claims.len(),
                            violation_count,
                            &result.kernel_name,
                            related_notes,
                            trust_snapshot,
                            now_millis(),
                        );
                        upsert_prepared_bridge_proposal(&store, Some(&app_for_bridge), proposal);
                    }
                });
            }

            let _ = app_handle.emit(
                "agent-result",
                serde_json::json!({
                    "type": "deep_research",
                    "question": question,
                    "result": report,
                }),
            );

            Ok((report, ctx))
        }
        crate::cli::parser::CliCommand::CustomCard {
            prompt: card_prompt,
            params: card_params,
            card_type: _card_type,
            card_id,
        } => {
            let dualtrack_dir = {
                let app = state.lock().map_err(|e| e.to_string())?;
                app.dualtrack_dir.clone()
            };

            let rendered_prompt = if let Some(ref params_map) = card_params {
                resolve_template(card_prompt, params_map)
            } else {
                card_prompt.clone()
            };

            let max_iterations = {
                let ai = ai_state.lock().map_err(|e| e.to_string())?;
                // Try to find task by id to get max_iterations
                let tasks = ai.agent_scheduler.list_tasks(None);
                let task = tasks.iter().find(|t| t.id == *task_id).cloned();
                task.map(|t| t.max_iterations).unwrap_or(30)
            };

            let mut router = {
                let ai = ai_state.lock().map_err(|e| e.to_string())?;
                if !ai.llm_router.is_available() {
                    let ctx = research_trace::TaskContext::default();
                    return Ok((format!(
                        "## Card Result\n\n**Card**: {}\n\nLLM API key not configured. Add an API key in Settings.\n\nPrompt: {}",
                        card_id.as_deref().unwrap_or("unknown"),
                        rendered_prompt
                    ), ctx));
                }
                ai.llm_router.clone()
            };

            let tool_registry = crate::ai::tool_registry::create_default_registry();
            let tool_prompt = tool_registry.build_tool_prompt_for_policy(&sandbox_policy);

            let system = format!(
                "You are a note-taking assistant with access to tools. Process the user's request and use tools when needed.\n\n{}",
                tool_prompt
            );

            let mut conversation = format!("User request: {}", rendered_prompt);
            let mut final_response_text = String::new();
            let mut final_model = String::new();
            let mut final_tokens_in: u64 = 0;
            let mut final_tokens_out: u64 = 0;

            for _iter in 0..max_iterations {
                match router
                    .complete(&system, &conversation, cancel_token.clone())
                    .await
                {
                    Ok(response) => {
                        final_response_text = response.text.clone();
                        final_model = response.model_used.clone();
                        final_tokens_in = response.tokens_in;
                        final_tokens_out = response.tokens_out;

                        if let Some(tool_call) =
                            crate::ai::tool_registry::parse_tool_call(&response.text)
                        {
                            tracing::info!(
                                "Tool call detected: {} (params: {})",
                                tool_call.tool,
                                tool_call.params
                            );

                            let tool_result = match tool_registry
                                .get_allowed(&tool_call.tool, &sandbox_policy)
                            {
                                Ok(tool) => {
                                    match tool_registry.validate_tool_call_params(
                                        &tool_call.tool,
                                        &tool_call.params,
                                        &sandbox_policy,
                                    ) {
                                        Ok(()) => {
                                            match tool
                                                .execute(
                                                    tool_call.params.clone(),
                                                    state,
                                                    ai_state,
                                                    &router,
                                                )
                                                .await
                                            {
                                                Ok(result) => result,
                                                Err(e) => crate::ai::tool_registry::ToolResult {
                                                    tool_name: tool_call.tool.clone(),
                                                    content: format!("Tool error: {}", e),
                                                    metadata: serde_json::json!({"error": e}),
                                                },
                                            }
                                        }
                                        Err(e) => crate::ai::tool_registry::ToolResult {
                                            tool_name: tool_call.tool.clone(),
                                            content: e.clone(),
                                            metadata: serde_json::json!({"error": e, "blocked_by_sandbox": true}),
                                        },
                                    }
                                }
                                Err(e) => crate::ai::tool_registry::ToolResult {
                                    tool_name: tool_call.tool.clone(),
                                    content: e.clone(),
                                    metadata: serde_json::json!({"error": e, "blocked_by_sandbox": true}),
                                },
                            };

                            if tool_result.tool_name == "ghost_write" {
                                if let Some(ghost_id) = tool_result
                                    .metadata
                                    .get("ghost_id")
                                    .and_then(|value| value.as_str())
                                {
                                    let target_note = tool_result
                                        .metadata
                                        .get("target_note")
                                        .and_then(|value| value.as_str())
                                        .unwrap_or("untitled.md")
                                        .to_string();
                                    let blocks_count = tool_result
                                        .metadata
                                        .get("blocks_count")
                                        .and_then(|value| value.as_u64())
                                        .unwrap_or(0)
                                        as usize;
                                    let ghost_id = ghost_id.to_string();
                                    try_upsert_bridge_proposal(
                                        state,
                                        Some(&app_handle),
                                        |trust_snapshot, now| {
                                            BridgeProposal::for_ghost(
                                                &ghost_id,
                                                &target_note,
                                                blocks_count,
                                                trust_snapshot,
                                                now,
                                            )
                                        },
                                    );
                                }
                            }

                            let tool_result_str = format!(
                                "Tool result from {}: {}",
                                tool_result.tool_name, tool_result.content
                            );
                            conversation.push_str(&format!(
                                "\n\n[Assistant used tool: {}]\n\nTool result:\n{}\n\nContinue analyzing and provide your next step or final answer.",
                                tool_call.tool, tool_result_str
                            ));
                        } else {
                            // No tool call — this is the final answer
                            break;
                        }
                    }
                    Err(e) => {
                        tracing::error!("Card LLM call failed in tool loop: {}", e);
                        return Err(format!(
                            "Card execution error: {}. Check your API key in Settings.",
                            e
                        ));
                    }
                }
            }

            let _ = research_trace::write_cot_log(
                &dualtrack_dir,
                task_id,
                &format!(
                    "## Tool-Call Card\n\nPrompt: {}\n\nModel: {}\nTokens: {}/{}",
                    rendered_prompt, final_model, final_tokens_in, final_tokens_out
                ),
                None,
            );

            let _ = app_handle.emit(
                "agent-result",
                serde_json::json!({
                    "type": "card_execution",
                    "card_id": card_id,
                    "result": final_response_text,
                    "model": final_model,
                }),
            );

            let mut ghost_ids = Vec::new();
            if should_store_card_result_ghost(&sandbox_policy) {
                let blocks: Vec<GhostBlock> = final_response_text
                    .split("\n\n")
                    .enumerate()
                    .map(|(i, para)| GhostBlock {
                        block_id: format!("ghost-block-{}", i),
                        content: para.to_string(),
                        operation: GhostOp::Suggestion,
                        after_block_id: None,
                        heading_context: card_id
                            .clone()
                            .unwrap_or_else(|| "card-result".to_string()),
                        context: vec![],
                        verified: None,
                        verification_result: None,
                    })
                    .collect();

                let target_note = "card-result.md".to_string();
                let block_count = blocks.len();
                let ghost_id = {
                    let ai = ai_state.lock().map_err(|e| e.to_string())?;
                    match ai.ghost_store.create(
                        &target_note,
                        &format!(
                            "Card execution: {}",
                            card_id.as_deref().unwrap_or("unknown")
                        ),
                        blocks,
                        Some(task_id.to_string()),
                    ) {
                        Ok(ghost_note) => Some(ghost_note.id),
                        Err(e) => {
                            tracing::warn!("Failed to store ghost: {}", e);
                            None
                        }
                    }
                };
                if let Some(ghost_id) = ghost_id {
                    try_upsert_bridge_proposal(state, Some(&app_handle), |trust_snapshot, now| {
                        BridgeProposal::for_ghost(
                            &ghost_id,
                            &target_note,
                            block_count,
                            trust_snapshot,
                            now,
                        )
                    });
                    ghost_ids.push(ghost_id);
                }
            } else {
                tracing::info!("Card result ghost storage blocked by sandbox policy");
            }

            let ctx = research_trace::TaskContext {
                intent: format!("card:{}", card_id.as_deref().unwrap_or("unknown")),
                ghost_ids,
                ..research_trace::TaskContext::default()
            };
            Ok((final_response_text, ctx))
        }
        crate::cli::parser::CliCommand::Custom(_intent) => {
            let query = _intent.clone();
            let retrieve_start = std::time::Instant::now();
            let (entries, contexts) = {
                let app = state.lock().map_err(|e| e.to_string())?;
                let dualtrack_dir_clone = app.dualtrack_dir.clone();
                let entries: Vec<SubagentEntry> = if let Some(ref sync_engine) = app.sync_engine {
                    let store = sync_engine.store();
                    store
                        .search_text(&query, 5)
                        .into_iter()
                        .map(|r| SubagentEntry {
                            title: if r.heading_context.is_empty() {
                                r.source_file.clone()
                            } else {
                                r.heading_context.clone()
                            },
                            snippet: r.chunk_text,
                            url: None,
                            authors: vec![],
                            year: None,
                            source: r.source_file,
                            relevance_score: r.score,
                        })
                        .collect()
                } else {
                    vec![]
                };
                let contexts: Vec<String> = entries
                    .iter()
                    .map(|e| format!("[{}] {}", e.source, e.snippet))
                    .collect();
                let _ = research_trace::write_path_log(
                    &dualtrack_dir_clone,
                    task_id,
                    0,
                    &query,
                    "local_vector",
                    &[],
                    &[],
                    &format!("Custom dispatch: {}", query),
                    None,
                );

                let cot_content = format!(
                    "## Custom Query\n\n```\n{}\n```\n\n## Results\n- Count: {}\n- Source: local_vector search",
                    query,
                    contexts.len()
                );
                let _ = research_trace::write_cot_log(
                    &dualtrack_dir_clone,
                    task_id,
                    &cot_content,
                    None,
                );

                (entries, contexts)
            };

            let retrieve_ms = retrieve_start.elapsed().as_millis() as u64;
            let mut phase_timings = std::collections::HashMap::new();
            phase_timings.insert("retrieve_ms".to_string(), retrieve_ms);

            let router = {
                let ai = ai_state.lock().map_err(|e| e.to_string())?;
                if ai.llm_router.is_available() {
                    Some(ai.llm_router.clone())
                } else {
                    None
                }
            };

            let content = if let Some(mut router) = router {
                let synthesize_start = std::time::Instant::now();
                let system = "You are a research assistant inside a note-taking app. Return concise, useful Markdown. Respect the user's requested output shape and do not claim to have modified files.";
                let expected_output = task_snapshot
                    .as_ref()
                    .map(|task| task.content.as_str())
                    .filter(|value| !value.trim().is_empty());
                let prompt = custom_research::llm_user_prompt(&query, &contexts, expected_output);
                match router.complete(system, &prompt, cancel_token.clone()).await {
                    Ok(response) => {
                        phase_timings.insert(
                            "synthesize_ms".to_string(),
                            response.latency_ms.max(synthesize_start.elapsed().as_millis() as u64),
                        );
                        custom_research::llm_content(&query, &response.text, &response.model_used)
                    }
                    Err(error) => {
                        phase_timings.insert(
                            "synthesize_ms".to_string(),
                            synthesize_start.elapsed().as_millis() as u64,
                        );
                        let fallback = custom_research::local_context_content(&query, &contexts);
                        format!(
                            "{}\n\n---\n\nLLM synthesis failed: {}",
                            fallback, error
                        )
                    }
                }
            } else {
                custom_research::no_llm_content(&query, &contexts)
            };

            let _ = app_handle.emit(
                "agent-result",
                serde_json::json!({"type": "research","query": query,"result": content}),
            );

            let ctx = research_trace::TaskContext {
                intent: query.clone(),
                phase_timings,
                retrieval_evidence: vec![task_evidence_from_entries(
                    "local_vector",
                    &entries,
                    0,
                    vec![query.clone()],
                )],
                ..research_trace::TaskContext::default()
            };
            Ok((content, ctx))
        }
    }
}

pub fn start_task_worker<R: tauri::Runtime>(app_handle: AppHandle<R>) {
    tauri::async_runtime::spawn(async move {
        loop {
            let (maybe_task, cancel_token) = {
                let ai_state = app_handle.state::<Mutex<AiState>>();
                let mut ai = match ai_state.lock() {
                    Ok(ai) => ai,
                    Err(_) => break,
                };
                let task = ai.agent_scheduler.dequeue();
                let token = task
                    .as_ref()
                    .map(|t| ai.agent_scheduler.create_cancel_token(&t.id));
                (task, token)
            };

            if let Some(task) = maybe_task {
                if let Some(root) = workflow_runtime_root(&app_handle) {
                    match WorkflowRuntimeService::new(&root).record_task_running(
                        &task,
                        now_millis(),
                    ) {
                        Ok(Some(bundle)) => {
                            let _ = app_handle.emit("workflow-run-updated", &bundle);
                        }
                        Ok(None) => {}
                        Err(error) => {
                            tracing::warn!(
                                "Failed to record workflow task {} running: {}",
                                task.id,
                                error
                            );
                        }
                    }
                }
                let task_id = task.id.clone();
                let cmd = task.command.clone();
                let app_for_state = app_handle.clone();
                let app_for_fn = app_handle.clone();
                let task_id_for_closure = task_id.clone();
                let cancel_token_for_closure = cancel_token;

                let result = tokio::task::spawn_blocking(move || {
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .map_err(|e| e.to_string())?;
                    let state = app_for_state.state::<Mutex<AppState>>();
                    let ai_state = app_for_state.state::<Mutex<AiState>>();
                    rt.block_on(execute_agent_task_async(
                        &task_id_for_closure,
                        cmd,
                        &state,
                        &ai_state,
                        app_for_fn,
                        cancel_token_for_closure,
                    ))
                })
                .await;

                match result {
                    Ok(inner) => match inner {
                        Ok((output, task_context)) => {
                            tracing::info!("Task {} completed", task_id);
                            let dream_snapshot = {
                                let state = app_handle.state::<Mutex<AppState>>();
                                state.lock().ok().and_then(|app| {
                                    app.dream_engine
                                        .as_ref()
                                        .map(|engine| engine.audit_snapshot())
                                })
                            };
                            {
                                let handle = app_handle.state::<Mutex<AiState>>();
                                let mut ai = handle
                                    .lock()
                                    .map_err(|e| e.to_string())
                                    .expect("AiState poisoned");
                                ai.agent_scheduler.complete_with_context_and_dream_snapshot(
                                    &task_id,
                                    output.clone(),
                                    Some(&task_context),
                                    dream_snapshot,
                                );
                            }
                            {
                                let state = app_handle.state::<Mutex<AppState>>();
                                let lock = state.lock();
                                if let Ok(app) = lock {
                                    let _ = research_trace::write_result_md(
                                        &app.dualtrack_dir,
                                        &task_id,
                                        &output,
                                        Some(&task_context),
                                    );
                                    let _ = research_trace::write_context(
                                        &app.dualtrack_dir,
                                        &task_id,
                                        &task_context,
                                    );
                                }
                            }
                            let workflow_update = {
                                let completed_task = {
                                    let handle = app_handle.state::<Mutex<AiState>>();
                                    handle
                                        .lock()
                                        .ok()
                                        .and_then(|ai| ai.agent_scheduler.get_task(&task_id).cloned())
                                };
                                match (workflow_runtime_root(&app_handle), completed_task) {
                                    (Some(root), Some(completed_task)) => {
                                        let handle = app_handle.state::<Mutex<AiState>>();
                                        let result = match handle.lock() {
                                            Ok(mut ai) => WorkflowRuntimeService::new(&root)
                                                .record_task_completion(
                                                    &completed_task,
                                                    now_millis(),
                                                    &mut ai.agent_scheduler,
                                                )
                                                .map_err(|error| error.to_string()),
                                            Err(error) => Err(error.to_string()),
                                        };
                                        result
                                    }
                                    _ => Ok(None),
                                }
                            };
                            match workflow_update {
                                Ok(Some(bundle)) => {
                                    let _ = app_handle.emit("workflow-run-updated", &bundle);
                                }
                                Ok(None) => {}
                                Err(error) => {
                                    tracing::error!(
                                        "Failed to complete workflow runtime for task {}: {}",
                                        task_id,
                                        error
                                    );
                                }
                            }
                            let _ = app_handle.emit(
                                "feroha_research_completed",
                                serde_json::json!({
                                    "task_id": &task_id,
                                    "source_block_id": null,
                                    "intent": task_context.intent,
                                    "content": output,
                                    "result": output,
                                }),
                            );
                            let _ = app_handle.emit(
                                "task-updated",
                                serde_json::json!({
                                    "task_id": &task_id,
                                    "status": "done",
                                    "result": output,
                                }),
                            );
                        }
                        Err(e) => {
                            tracing::error!("Task {} failed: {}", task_id, e);
                            let failed_task = {
                                let handle = app_handle.state::<Mutex<AiState>>();
                                let mut ai = handle
                                    .lock()
                                    .map_err(|e| e.to_string())
                                    .expect("AiState poisoned");
                                ai.agent_scheduler.fail(&task_id, e.clone());
                                ai.agent_scheduler.get_task(&task_id).cloned()
                            };
                            if let (Some(root), Some(failed_task)) =
                                (workflow_runtime_root(&app_handle), failed_task)
                            {
                                match WorkflowRuntimeService::new(&root).record_task_failure(
                                    &failed_task,
                                    "task_execution_failed",
                                    &e,
                                    now_millis(),
                                ) {
                                    Ok(Some(bundle)) => {
                                        let _ =
                                            app_handle.emit("workflow-run-updated", &bundle);
                                    }
                                    Ok(None) => {}
                                    Err(error) => tracing::warn!(
                                        "Failed to record workflow task {} failure: {}",
                                        task_id,
                                        error
                                    ),
                                }
                            }
                            let _ = app_handle.emit(
                                "task-updated",
                                serde_json::json!({
                                    "task_id": &task_id,
                                    "status": "error",
                                    "error": e,
                                }),
                            );
                        }
                    },
                    Err(join_err) => {
                        tracing::error!("Task {} spawn failed: {:?}", task_id, join_err);
                        let join_summary = format!("{:?}", join_err);
                        let failed_task = {
                            let handle = app_handle.state::<Mutex<AiState>>();
                            let failed_task = match handle.lock() {
                                Ok(mut ai) => {
                                    ai.agent_scheduler.fail(&task_id, join_summary.clone());
                                    ai.agent_scheduler.get_task(&task_id).cloned()
                                }
                                Err(_) => None,
                            };
                            failed_task
                        };
                        if let (Some(root), Some(failed_task)) =
                            (workflow_runtime_root(&app_handle), failed_task)
                        {
                            match WorkflowRuntimeService::new(&root).record_task_failure(
                                &failed_task,
                                "task_join_failed",
                                &join_summary,
                                now_millis(),
                            ) {
                                Ok(Some(bundle)) => {
                                    let _ = app_handle.emit("workflow-run-updated", &bundle);
                                }
                                Ok(None) => {}
                                Err(error) => tracing::warn!(
                                    "Failed to record workflow task {} join failure: {}",
                                    task_id,
                                    error
                                ),
                            }
                        }
                        let _ = app_handle.emit(
                            "task-updated",
                            serde_json::json!({
                                "task_id": &task_id,
                                "status": "error",
                                "error": join_summary,
                            }),
                        );
                    }
                }
            } else {
                let notifier = {
                    let ai_state = app_handle.state::<Mutex<AiState>>();
                    let ai = match ai_state.lock() {
                        Ok(ai) => ai,
                        Err(_) => break,
                    };
                    ai.task_notifier.clone()
                };
                tokio::select! {
                    _ = notifier.notified() => {},
                    _ = tokio::time::sleep(std::time::Duration::from_secs(10)) => {},
                }
            }
        }
    });
}

fn workflow_runtime_root<R: tauri::Runtime>(app_handle: &AppHandle<R>) -> Option<PathBuf> {
    let state = app_handle.state::<Mutex<AppState>>();
    state.lock().ok().and_then(|app| {
        (!app.vault_path.trim().is_empty()).then(|| PathBuf::from(&app.vault_path))
    })
}

#[tauri::command]
pub(crate) fn trigger_dream(
    state: State<'_, Mutex<AppState>>,
    app_handle: AppHandle,
) -> Result<String, String> {
    let app = state.lock().map_err(|e| e.to_string())?;
    let dualtrack_dir = app.dualtrack_dir.clone();
    let bridge_store = app.bridge_store.clone();
    let bridge_trust = TrustSnapshot::from_protocol(app.protocol.as_ref());
    let store = if let Some(ref sync_engine) = app.sync_engine {
        sync_engine.store()
    } else {
        return Err("No sync engine available. Open a vault first.".to_string());
    };

    let mut dream_engine = match app.dream_engine.clone() {
        Some(de) => de,
        None => {
            let mut de = DreamEngine::new();
            de.set_dualtrack_dir(&dualtrack_dir);
            de
        }
    };

    let stats = dream_engine.run_cycle(store);
    let insights = dream_engine.get_insights().to_vec();
    let dream_edges = dream_engine.export_graph_edges();
    drop(app);

    {
        let mut app = state.lock().map_err(|e| e.to_string())?;
        for edge in dream_edges {
            app.link_graph.add_graph_edge(edge);
        }
        app.dream_engine = Some(dream_engine);
    }

    if !insights.is_empty() {
        if let Some(store) = bridge_store {
            let related_notes = insights
                .iter()
                .flat_map(|insight| insight.related_chunks.iter().cloned())
                .take(12)
                .collect::<Vec<_>>();
            let proposal = BridgeProposal::for_dream_cycle(
                &format!("dream_{}", now_millis()),
                insights.len(),
                related_notes,
                bridge_trust,
                now_millis(),
            );
            upsert_prepared_bridge_proposal(&store, Some(&app_handle), proposal);
        }
    }

    let _ = app_handle.emit(
        "agent-result",
        serde_json::json!({
            "type": "dream",
            "stats": stats,
            "insights": insights,
        }),
    );

    Ok(format!(
        "## Dream Cycle Complete\n\n\
         **NREM Phase**\n\
         - Connections strengthened: {}\n\
         - Connections pruned: {}\n\
         - Memories processed: {}\n\n\
         **REM Phase**\n\
         - Bridges created: {}\n\
         - Memories processed: {}\n\n\
         **Insight Phase**\n\
         - Communities found: {}\n\
         - Summaries generated: {}\n\n\
         **Total**\n\
         - Memories processed: {}\n\
         - Duration: {}ms\n\
         - Insights discovered: {}",
        stats.nrem_connections_strengthened,
        stats.nrem_connections_pruned,
        stats.total_memories_processed,
        stats.rem_bridges_created,
        stats.total_memories_processed,
        stats.insight_communities_found,
        stats.insight_summaries_generated,
        stats.total_memories_processed,
        stats.duration_ms,
        insights.len()
    ))
}

#[tauri::command]
pub(crate) fn get_vectordb_stats(
    state: State<'_, Mutex<AppState>>,
) -> Result<crate::ai::vectordb::VectorDbStats, String> {
    let app = state.lock().map_err(|e| e.to_string())?;
    if let Some(ref sync_engine) = app.sync_engine {
        let store = sync_engine.store();
        store.get_stats().map_err(|e| e.to_string())
    } else {
        Err("No sync engine available. Open a vault first.".to_string())
    }
}

#[tauri::command]
pub(crate) fn get_config(state: State<'_, Mutex<AppConfig>>) -> Result<AppConfig, String> {
    state.lock().map(|c| c.clone()).map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn set_config(
    config: AppConfig,
    state: State<'_, Mutex<AppConfig>>,
    ai_state: State<'_, Mutex<AiState>>,
    app_state: State<'_, Mutex<AppState>>,
) -> Result<(), String> {
    let mut current = state.lock().map_err(|e| e.to_string())?;
    *current = config.clone();

    let mut ai = ai_state.lock().map_err(|e| e.to_string())?;
    let router_config = config.to_router_config();
    ai.llm_router.update_config(router_config);
    let embedding_backend = embedding_backend_from_config(&config);
    ai.embedding_pipeline = EmbeddingPipeline::new(embedding_backend.clone());
    drop(ai);

    if let Ok(mut app) = app_state.lock() {
        if let Some(sync_engine) = app.sync_engine.as_mut() {
            sync_engine.set_embedder(EmbeddingPipeline::new(embedding_backend));
        }
    }
    Ok(())
}

fn embedding_backend_from_config(config: &AppConfig) -> EmbeddingBackend {
    match config.embedding_provider.as_str() {
        "openai" => EmbeddingBackend::OpenAi {
            api_key: config.embedding_api_key.clone(),
            model: "text-embedding-3-small".to_string(),
        },
        "gemini" => EmbeddingBackend::Gemini {
            api_key: config.embedding_api_key.clone(),
        },
        _ => EmbeddingBackend::None,
    }
}

fn sanitize_debug_message(message: &str) -> String {
    message
        .split_whitespace()
        .map(|part| {
            let suspicious_secret = part.starts_with("sk-")
                || part.starts_with("sk_")
                || (part.len() > 72
                    && part
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')));
            if suspicious_secret {
                "[redacted]"
            } else {
                part
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn debug_llm_config_status(
    provider: &str,
    model: &str,
    ok: bool,
    latency_ms: u64,
    error: Option<String>,
) -> serde_json::Value {
    let sanitized_error = error.map(|message| sanitize_debug_message(&message));
    serde_json::json!({
        "ok": ok,
        "provider": provider,
        "model": model,
        "latency_ms": latency_ms,
        "message": if ok { "api debug completed" } else { "api debug failed" },
        "error": sanitized_error,
    })
}

#[tauri::command]
pub(crate) async fn debug_llm_config(
    ai_state: State<'_, Mutex<AiState>>,
) -> Result<serde_json::Value, String> {
    let mut router = {
        let ai = ai_state.lock().map_err(|e| e.to_string())?;
        ai.llm_router.clone()
    };
    let config = router.config();
    let provider = config.llm_provider.clone();
    let model = config.llm_model.clone();

    if provider != "ollama" && config.llm_api_key.trim().is_empty() {
        return Ok(debug_llm_config_status(
            &provider,
            &model,
            false,
            0,
            Some("missing API key".to_string()),
        ));
    }

    let start = std::time::Instant::now();
    let result = router
        .complete(
            "You are an API health check. Reply with exactly: ok",
            "ok",
            None,
        )
        .await;
    let latency_ms = start.elapsed().as_millis() as u64;

    Ok(match result {
        Ok(_) => debug_llm_config_status(&provider, &model, true, latency_ms, None),
        Err(error) => debug_llm_config_status(&provider, &model, false, latency_ms, Some(error)),
    })
}

#[tauri::command]
pub(crate) fn record_ghost_feedback(
    ghost_id: String,
    block_ids: Vec<String>,
    action: String,
    reason: Option<String>,
    ai_state: State<'_, Mutex<AiState>>,
    app_state: State<'_, Mutex<AppState>>,
    app_handle: AppHandle,
) -> Result<serde_json::Value, String> {
    let ai = ai_state.lock().map_err(|e| e.to_string())?;
    let mut ghost: GhostNote = ai
        .ghost_store
        .get(&ghost_id)
        .ok_or_else(|| "ghost not found".to_string())?;

    let timestamp = chrono::Utc::now().timestamp_millis();

    match action.as_str() {
        "accept" => {
            let new_blocks: Vec<String> = block_ids
                .iter()
                .filter(|id| !ghost.accepted_blocks.contains(id))
                .cloned()
                .collect();
            ghost.accepted_blocks.extend(new_blocks);
        }
        "reject" => {
            let new_blocks: Vec<String> = block_ids
                .iter()
                .filter(|id| !ghost.rejected_blocks.contains(id))
                .cloned()
                .collect();
            ghost.rejected_blocks.extend(new_blocks);
        }
        _ => {}
    }

    ghost.feedback_history.push(FeedbackEntry {
        timestamp,
        action: action.clone(),
        block_ids: block_ids.clone(),
        reason: reason.clone(),
    });

    let total = ghost.accepted_blocks.len() + ghost.rejected_blocks.len();
    ghost.confidence = if total > 0 {
        ghost.accepted_blocks.len() as f32 / total as f32
    } else {
        0.0
    };

    let suggested_count = ghost.suggested_blocks.len();
    let accepted_count = ghost.accepted_blocks.len();
    let rejected_count = ghost.rejected_blocks.len();

    ghost.status = if accepted_count >= suggested_count && suggested_count > 0 {
        GhostStatus::Accepted
    } else if rejected_count >= suggested_count && suggested_count > 0 {
        GhostStatus::Rejected
    } else if accepted_count > 0 || rejected_count > 0 {
        GhostStatus::PartiallyAccepted
    } else {
        GhostStatus::Pending
    };

    ai.ghost_store.save(&ghost)?;

    drop(ai);

    let mut app = app_state.lock().map_err(|e| e.to_string())?;
    if let Some(ref mut protocol) = app.protocol {
        match action.as_str() {
            "accept" => protocol.trust_score_record_accept(),
            "reject" => protocol.trust_score_record_reject(),
            _ => {}
        }
    }
    drop(app);

    let _ = app_handle.emit(
        "feroha_feedback_recorded",
        serde_json::json!({
            "ghost_id": ghost_id,
            "action": action,
            "block_ids": block_ids,
            "confidence": ghost.confidence,
            "status": ghost.status,
        }),
    );

    serde_json::to_value(&ghost).map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn inspect_ghost(
    ghost_id: String,
    ai_state: State<'_, Mutex<AiState>>,
) -> Result<serde_json::Value, String> {
    let ai = ai_state.lock().map_err(|e| e.to_string())?;
    let ghost = ai
        .ghost_store
        .get(&ghost_id)
        .ok_or_else(|| format!("ghost not found: {}", ghost_id))?;

    serde_json::to_value(&ghost).map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn dispatch_agent_task(
    payload: serde_json::Value,
    state: State<'_, Mutex<AppState>>,
    ai_state: State<'_, Mutex<AiState>>,
    app_handle: AppHandle,
) -> Result<serde_json::Value, String> {
    let content = payload["content"].as_str().unwrap_or("").to_string();
    let intent = payload["intent"].as_str().unwrap_or("").to_string();
    let timestamp = payload["timestamp"].as_u64().unwrap_or(0);
    let card_id = payload["card_id"].as_str().map(|s| s.to_string());
    let card_type = payload["card_type"].as_str().map(|s| s.to_string());
    let prompt = payload["prompt"].as_str().map(|s| s.to_string());
    let source_block_id = payload["source_block_id"]
        .as_str()
        .or_else(|| payload["blockId"].as_str())
        .map(|s| s.to_string());
    let params: Option<std::collections::HashMap<String, String>> =
        payload["params"].as_object().map(|obj| {
            obj.iter()
                .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                .collect()
        });
    let context_note = payload["context_note"].as_str().map(|s| s.to_string());
    let task_intent = task_intent_from_payload(&payload);
    let sandbox_policy = sandbox_policy_for_dispatch_payload(task_intent, &payload);
    let review_action = dispatch_review_action_for_policy(&sandbox_policy);
    ensure_bridge_store_for_review_action(&state, review_action)?;

    let cmd = if prompt.is_some() {
        crate::cli::parser::CliCommand::CustomCard {
            prompt: prompt.clone().unwrap_or_default(),
            params: params.clone(),
            card_type: card_type.clone(),
            card_id: card_id.clone(),
        }
    } else {
        crate::cli::parser::CliCommand::Custom(format!("/agent research {}", intent))
    };
    let task_id = format!("task_{}", uuid::Uuid::new_v4().to_string().replace('-', ""));

    let task = AgentTask {
        id: task_id.clone(),
        command: cmd,
        task_type: TaskType::Custom(task_intent.as_str().to_string()),
        task_intent: Some(task_intent),
        sandbox_policy: Some(sandbox_policy.clone()),
        priority: TaskPriority::Medium,
        priority_score: 50,
        status: TaskStatus::Pending,
        anchor_note: None,
        created_at: timestamp,
        max_retries: 2,
        retry_count: 0,
        synthesize_phase: SynthesizePhase::Idle,
        subagent_results: vec![],
        graph_manifest: None,
        has_trace: false,
        source_block_id,
        card_id,
        card_type,
        prompt,
        params,
        context_note,
        intent: intent.clone(),
        content: content.clone(),
        max_iterations: 30,
        sub_tasks: vec![],
        material_packet: None,
        context_fragments: vec![],
        regression_metrics: None,
        retry_delay_ms: 1000,
        retry_backoff_multiplier: 2.0,
        last_retry_at: None,
        consecutive_failures: 0,
    };

    let handle = {
        let mut ai = ai_state.lock().map_err(|e| e.to_string())?;
        AiManagerService::new(&mut ai.agent_scheduler).submit(task)
    };

    if review_action == DispatchReviewAction::PendingBridgeReview {
        try_upsert_bridge_proposal(&state, Some(&app_handle), |trust_snapshot, now| {
            BridgeProposal::for_typed_task(
                &handle.id,
                &intent,
                task_intent,
                &sandbox_policy,
                trust_snapshot,
                now,
            )
        });

        let _ = app_handle.emit(
            "task-updated",
            serde_json::json!({
                "task_id": handle.id,
                "status": "pending"
            }),
        );

        return Ok(serde_json::json!({
            "task_id": handle.id,
            "status": "pending",
            "message": "Task submitted. Pending Bridge approval."
        }));
    }

    {
        let mut ai = ai_state.lock().map_err(|e| e.to_string())?;
        AiManagerService::new(&mut ai.agent_scheduler).approve(&handle.id, "dispatch")?;
        ai.task_notifier.notify_one();
    }

    let _ = app_handle.emit(
        "task-checked",
        serde_json::json!({
            "task_id": handle.id,
            "checked_at": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64,
            "checked_by": "dispatch"
        }),
    );

    let _ = app_handle.emit(
        "task-updated",
        serde_json::json!({
            "task_id": handle.id,
            "status": "approved"
        }),
    );

    Ok(serde_json::json!({
        "task_id": handle.id,
        "status": "researching"
    }))
}

#[tauri::command]
pub(crate) async fn plan_research(
    question: String,
    ai_state: State<'_, Mutex<AiState>>,
) -> Result<serde_json::Value, String> {
    let mut router = {
        let ai = ai_state.lock().map_err(|e| e.to_string())?;
        ai.llm_router.clone()
    };
    let subagent = Subagent::new(None);
    let steps = subagent.plan_research(&question, Some(&mut router)).await;
    serde_json::to_value(&steps).map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn check_ghost_conflicts(
    source_note: Option<String>,
    ai_state: State<'_, Mutex<AiState>>,
) -> Result<Vec<ConflictReport>, String> {
    let ai = ai_state.lock().map_err(|e| e.to_string())?;
    Ok(ai.ghost_store.detect_conflicts(source_note.as_deref()))
}

#[tauri::command]
pub(crate) fn get_suggestions(
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<TaskSuggestion>, String> {
    let app = state.lock().map_err(|e| e.to_string())?;
    let recently_edited: Vec<String> = app
        .vault
        .as_ref()
        .map(|v| {
            v.list_notes()
                .unwrap_or_default()
                .into_iter()
                .take(10)
                .map(|n| n.path)
                .collect()
        })
        .unwrap_or_default();
    let suggestions = crate::ai::agent_scheduler::analyze_activity(
        &recently_edited,
        &app.link_graph,
        app.vault.as_ref(),
    );
    Ok(suggestions)
}

#[tauri::command]
pub(crate) fn orchestrator_status(
    ai_state: State<'_, Mutex<AiState>>,
) -> Result<OrchestratorStatus, String> {
    let state = ai_state.lock().map_err(|e| e.to_string())?;
    let status = state
        .agent_scheduler
        .orchestrator_status()
        .unwrap_or(OrchestratorStatus {
            active_agents: 0,
            degraded_agents: vec![],
            epoch_count: 0,
            track_count: 0,
            track_event_count: 0,
            material_packet_count: 0,
            active_track_count: 0,
            completed_track_count: 0,
            failed_track_count: 0,
            cancelled_track_count: 0,
            last_event: None,
            recent_events: vec![],
            agent_states: vec![],
            track_details: vec![],
            diagnostics: vec![],
            workflow_event_count: 0,
            workflow_replan_request_count: 0,
            recent_workflow_events: vec![],
            workflow_event_log_path: None,
        });
    Ok(status)
}

#[tauri::command]
pub(crate) fn read_workflow_runtime_events(
    run_id: String,
    limit: Option<usize>,
    ai_state: State<'_, Mutex<AiState>>,
) -> Result<Vec<HarnessEvent>, String> {
    let state = ai_state.lock().map_err(|e| e.to_string())?;
    state
        .agent_scheduler
        .workflow_runtime_events_for_run(&run_id, limit.unwrap_or(200))
}

#[tauri::command]
pub(crate) fn submit_workflow_patch_review(
    run_id: String,
    patch: WorkflowPatch,
    state: State<'_, Mutex<AppState>>,
    app_handle: AppHandle,
) -> Result<BridgeProposal, String> {
    let (store, trust_snapshot) = bridge_context(&state)?
        .ok_or_else(|| "Bridge proposal store is not initialized".to_string())?;
    let saved = store_workflow_patch_review_proposal(
        &store,
        &run_id,
        &patch,
        trust_snapshot,
        now_millis(),
    )?;
    emit_bridge_proposal_update(Some(&app_handle), &saved);
    Ok(saved)
}

#[tauri::command]
pub(crate) fn submit_orchestrator_output_review(
    run_id: String,
    output: OrchestratorOutput,
    state: State<'_, Mutex<AppState>>,
    ai_state: State<'_, Mutex<AiState>>,
    app_handle: AppHandle,
) -> Result<Option<BridgeProposal>, String> {
    let (store, trust_snapshot) = bridge_context(&state)?
        .ok_or_else(|| "Bridge proposal store is not initialized".to_string())?;
    let saved = {
        let mut ai = ai_state.lock().map_err(|e| e.to_string())?;
        route_orchestrator_output_review_to_bridge(
        &store,
            Some(&mut ai.agent_scheduler),
        &run_id,
        &output,
        trust_snapshot,
        now_millis(),
        )?
    };
    if let Some(proposal) = saved.as_ref() {
        emit_bridge_proposal_update(Some(&app_handle), proposal);
    }
    Ok(saved)
}

#[tauri::command]
pub(crate) async fn translate_research(
    agent_id: String,
    ai_state: State<'_, Mutex<AiState>>,
) -> Result<TranslationResult, String> {
    let (task, router) = {
        let state = ai_state.lock().map_err(|e| e.to_string())?;
        let task = state
            .agent_scheduler
            .list_tasks(None)
            .into_iter()
            .find(|t| t.id == agent_id)
            .cloned()
            .ok_or(format!("Agent task {} not found", agent_id))?;
        let router = state.llm_router.clone();
        (task, router)
    };

    let knowledge = Scientist::extract_knowledge(&task);

    let translator = LeanShapedTranslator::new(Arc::new(tokio::sync::Mutex::new(router)), None);

    Ok(translator.translate(&knowledge, &agent_id).await)
}

#[tauri::command]
pub(crate) fn verify_proposition_graph(
    graph: PropositionGraph,
) -> Result<VerificationResult, String> {
    Ok(PropositionKernel::verify(&graph))
}

#[tauri::command]
pub(crate) fn orchestrator_events(
    ai_state: State<'_, Mutex<AiState>>,
) -> Result<Vec<OrchestratorEvent>, String> {
    let state = ai_state.lock().map_err(|e| e.to_string())?;
    let events = state.agent_scheduler.orchestrator_events();
    Ok(events)
}

#[tauri::command]
pub(crate) fn orchestrator_terminate(
    agent_id: String,
    ai_state: State<'_, Mutex<AiState>>,
) -> Result<bool, String> {
    let mut state = ai_state.lock().map_err(|e| e.to_string())?;
    let result = state.agent_scheduler.terminate_agent(&agent_id);
    Ok(result)
}

#[tauri::command]
pub(crate) fn orchestrator_reinstate(
    agent_id: String,
    ai_state: State<'_, Mutex<AiState>>,
) -> Result<bool, String> {
    let mut state = ai_state.lock().map_err(|e| e.to_string())?;
    let result = state.agent_scheduler.reinstate_agent(&agent_id);
    Ok(result)
}

#[tauri::command]
pub(crate) fn list_skills(
    ai_state: State<'_, Mutex<AiState>>,
) -> Result<Vec<crate::ai::skill_manager::SkillDef>, String> {
    let ai = ai_state.lock().map_err(|e| e.to_string())?;
    let manager = ai
        .skill_manager
        .as_ref()
        .ok_or("Skill manager not initialized")?;
    Ok(manager.list_skills().into_iter().cloned().collect())
}

#[tauri::command]
pub(crate) fn plugin_status(
    app_state: State<'_, Mutex<AppState>>,
) -> Result<serde_json::Value, String> {
    let plugins_dir = {
        let app = app_state.lock().map_err(|e| e.to_string())?;
        let base = if app.dualtrack_dir.as_os_str().is_empty() {
            PathBuf::from(".dualtrack")
        } else {
            app.dualtrack_dir.clone()
        };
        base.join("plugins")
    };

    let mut manager = PluginManager::new(
        PluginManagerConfig {
            plugins_dir: plugins_dir.to_string_lossy().to_string(),
            ..Default::default()
        },
        env!("CARGO_PKG_VERSION"),
    );
    let available_plugins = manager.initialize().map_err(|e| e.to_string())?;
    let enabled_plugins = manager.enabled_count();

    Ok(serde_json::json!({
        "status": "ready",
        "message": if available_plugins == 0 {
            "Plugin manager initialized; no plugins installed"
        } else {
            "Plugin manager initialized"
        },
        "available_plugins": available_plugins,
        "enabled_plugins": enabled_plugins,
        "plugins_dir": plugins_dir
    }))
}

#[tauri::command]
pub(crate) fn list_agent_tools() -> Result<Vec<crate::ai::tool_registry::ToolInfo>, String> {
    let registry = crate::ai::tool_registry::create_default_registry();
    Ok(registry.list_tools())
}

#[tauri::command]
pub(crate) fn search_fulltext(
    query: String,
    limit: Option<usize>,
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<crate::ai::search_engine::SearchResult>, String> {
    let app = state.lock().map_err(|e| e.to_string())?;
    let engine = app
        .search_engine
        .as_ref()
        .ok_or("Search engine not initialized")?;
    engine
        .search(&query, limit.unwrap_or(20))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn jsonld_validate(project_root: String) -> Result<JsonLdValidationReport, String> {
    Ok(crate::jsonld::indexer::validate_vault(&PathBuf::from(
        project_root,
    )))
}

#[tauri::command]
pub(crate) fn jsonld_migrate(project_root: String) -> Result<JsonLdMigrationReport, String> {
    crate::jsonld::indexer::migrate_vault_with_artifacts(&PathBuf::from(project_root))
}

#[tauri::command]
pub(crate) fn jsonld_index(project_root: String) -> Result<JsonLdProjectIndex, String> {
    crate::jsonld::indexer::index_vault_with_artifacts(&PathBuf::from(project_root))
}

#[tauri::command]
pub(crate) fn jsonld_read(
    project_root: String,
    query: String,
    token_budget: usize,
) -> Result<JsonLdContextBundle, String> {
    crate::jsonld::reader::JsonLdReader::load_context(
        &PathBuf::from(project_root),
        &query,
        token_budget,
    )
}

#[tauri::command]
pub(crate) fn mdt_validate(project_root: String) -> Result<MdtValidationReport, String> {
    let root = PathBuf::from(project_root);
    let _ = crate::jsonld::indexer::validate_vault(&root);
    Ok(crate::mdt::indexer::validate_vault(&root))
}

#[tauri::command]
pub(crate) fn mdt_index(project_root: String) -> Result<MdtProjectIndex, String> {
    let root = PathBuf::from(project_root);
    let _ = crate::jsonld::indexer::index_vault_with_artifacts(&root)?;
    crate::mdt::indexer::index_vault_with_artifacts(&root)
}

#[tauri::command]
pub(crate) fn mdt_read(
    project_root: String,
    query: String,
    token_budget: usize,
) -> Result<MdtContextBundle, String> {
    let root = PathBuf::from(project_root);
    crate::jsonld::reader::JsonLdReader::load_context(&root, &query, token_budget)
        .map(crate::jsonld::reader::to_legacy_mdt_bundle)
        .or_else(|_| crate::mdt::reader::MdtReader::load_context(&root, &query, token_budget))
}

#[tauri::command]
pub(crate) fn mdt_pack(
    project_root: String,
    archive_path: String,
) -> Result<MdtArchiveManifest, String> {
    let root = PathBuf::from(project_root);
    let _ = crate::jsonld::indexer::index_vault_with_artifacts(&root)?;
    crate::mdt::archive::pack_mdtz(&root, &PathBuf::from(archive_path))
}

#[tauri::command]
pub(crate) fn mdt_unpack(
    archive_path: String,
    output_root: String,
) -> Result<MdtArchiveManifest, String> {
    crate::mdt::archive::unpack_mdtz(&PathBuf::from(archive_path), &PathBuf::from(output_root))
}

#[tauri::command]
pub(crate) fn list_output_hooks(
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<OutputHook>, String> {
    let app = state.lock().map_err(|e| e.to_string())?;
    if let Some(ref om) = app.output_manager {
        Ok(om.list_hooks())
    } else {
        Ok(vec![])
    }
}

#[tauri::command]
pub(crate) fn add_output_hook(
    hook: OutputHook,
    state: State<'_, Mutex<AppState>>,
) -> Result<bool, String> {
    let mut app = state.lock().map_err(|e| e.to_string())?;
    if app.output_manager.is_none() {
        app.output_manager = Some(std::sync::Arc::new(
            crate::harness::output_hook::OutputManager::load_defaults(&app.dualtrack_dir),
        ));
    }
    if let Some(ref mut om) = app.output_manager {
        add_hook_to_output_manager(om, hook);
        Ok(true)
    } else {
        Ok(false)
    }
}

fn add_hook_to_output_manager(
    manager: &mut Arc<crate::harness::output_hook::OutputManager>,
    hook: OutputHook,
) {
    Arc::make_mut(manager).add_hook(hook);
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DreamStatus {
    pub last_stats: Option<DreamStats>,
    pub insights: Vec<DreamInsight>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TrustScoreInfo {
    pub score: f32,
    pub acceptance_rate: f32,
    pub accuracy_rate: f32,
    pub consecutive_accepts: u32,
    pub consecutive_rejects: u32,
    pub total_interactions: u32,
    pub total_accepts: u32,
    pub total_rejects: u32,
    pub mode: String,
    pub recommended_mode: String,
}

#[tauri::command]
pub(crate) fn get_scheduler_status(
    ai_state: State<'_, Mutex<AiState>>,
) -> Result<Vec<CronJobStatus>, String> {
    let ai = ai_state.lock().map_err(|e| e.to_string())?;
    if let Some(ref scheduler) = ai.scheduler {
        Ok(scheduler.status())
    } else {
        Ok(vec![])
    }
}

#[tauri::command]
pub(crate) fn get_dream_status(state: State<'_, Mutex<AppState>>) -> Result<DreamStatus, String> {
    let app = state.lock().map_err(|e| e.to_string())?;
    if let Some(ref engine) = app.dream_engine {
        Ok(DreamStatus {
            last_stats: engine.last_stats().cloned(),
            insights: engine.last_insights().to_vec(),
        })
    } else {
        Ok(DreamStatus {
            last_stats: None,
            insights: vec![],
        })
    }
}

#[tauri::command]
pub(crate) fn get_trust_score_info(
    state: State<'_, Mutex<AppState>>,
) -> Result<TrustScoreInfo, String> {
    let app = state.lock().map_err(|e| e.to_string())?;
    if let Some(ref protocol) = app.protocol {
        let rec_mode = protocol.current_mode();
        Ok(TrustScoreInfo {
            score: protocol.trust_score_value(),
            acceptance_rate: protocol.acceptance_rate(),
            accuracy_rate: protocol.accuracy_rate(),
            consecutive_accepts: protocol.consecutive_accepts(),
            consecutive_rejects: protocol.consecutive_rejects(),
            total_interactions: protocol.total_interactions(),
            total_accepts: protocol.total_accepts(),
            total_rejects: protocol.total_rejects(),
            mode: format!("{:?}", rec_mode).to_lowercase(),
            recommended_mode: format!("{:?}", rec_mode).to_lowercase(),
        })
    } else {
        Ok(TrustScoreInfo {
            score: 0.5,
            acceptance_rate: 0.0,
            accuracy_rate: 0.0,
            consecutive_accepts: 0,
            consecutive_rejects: 0,
            total_interactions: 0,
            total_accepts: 0,
            total_rejects: 0,
            mode: "manual".to_string(),
            recommended_mode: "manual".to_string(),
        })
    }
}

#[cfg(test)]
mod mdt_command_tests {
    use super::*;

    #[test]
    fn mdt_commands_index_validate_and_read_project() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("root.md"),
            "---\nmdt_version: \"0.1.0\"\nid: root\ntitle: Root Reader\ntree:\n  parent: null\n  order: 0\narea: mdt\nimportance: 5\nsummary: \"Root reader summary\"\nlinks:\n  - target: child\n    type: related\n---\n# Root Reader\n\nFull root reader body.\n",
        )
        .unwrap();
        std::fs::write(
            temp.path().join("child.md"),
            "---\nmdt_version: \"0.1.0\"\nid: child\ntitle: Child Node\ntree:\n  parent: root\n  order: 1\narea: mdt\n---\n# Child Node\n",
        )
        .unwrap();

        let root = temp.path().to_string_lossy().to_string();
        let report = mdt_validate(root.clone()).unwrap();
        assert!(report.valid);
        assert_eq!(report.node_count, 2);
        assert_eq!(report.edge_count, 1);

        let index = mdt_index(root.clone()).unwrap();
        assert_eq!(index.nodes.len(), 2);
        assert_eq!(index.edges[0].edge_type, "related");

        let bundle = mdt_read(root, "reader".to_string(), 1000).unwrap();
        assert_eq!(bundle.items[0].node_id, "urn:feroha:node:root");
        assert!(bundle.items[0].content.contains("Full root reader body"));
    }

    #[test]
    fn mdt_index_writes_release_artifacts() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("root.md"),
            "---\nmdt_version: \"0.1.0\"\nid: root\ntitle: Root\ntree:\n  parent: null\n  order: 0\nlinks:\n  - target: child\n    type: parent\n---\n# Root\n",
        )
        .unwrap();
        std::fs::write(
            temp.path().join("child.md"),
            "---\nmdt_version: \"0.1.0\"\nid: child\ntitle: Child\ntree:\n  parent: root\n  order: 1\n---\n# Child\n",
        )
        .unwrap();

        let root = temp.path().to_string_lossy().to_string();
        let index = mdt_index(root).unwrap();

        let index_dir = temp.path().join(".dualtrack").join("mdt").join("indexes");
        let nodes: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(index_dir.join("nodes.json")).unwrap())
                .unwrap();
        let edges: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(index_dir.join("edges.json")).unwrap())
                .unwrap();
        let project: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(index_dir.join("project.json")).unwrap())
                .unwrap();

        assert_eq!(index.nodes.len(), 2);
        assert_eq!(nodes.as_array().unwrap().len(), 2);
        assert_eq!(edges.as_array().unwrap().len(), 1);
        assert_eq!(project["node_count"], 2);
        assert_eq!(project["edge_count"], 1);
    }
}

#[cfg(test)]
mod output_hook_command_tests {
    use super::*;
    use crate::harness::output_hook::{HookTarget, HookTrigger, OutputHook, OutputManager};
    use std::sync::Arc;

    #[test]
    fn add_output_hook_to_shared_manager_does_not_panic() {
        let temp = tempfile::tempdir().unwrap();
        let mut manager = Arc::new(OutputManager::load_defaults(&temp.path().to_path_buf()));
        let shared_reader = manager.clone();
        let hook = OutputHook {
            id: "custom".to_string(),
            name: "Custom".to_string(),
            trigger: HookTrigger::OnDreamCycle,
            target: HookTarget::FileSink {
                path: temp.path().join("custom-output"),
            },
            filter: Default::default(),
            enabled: true,
        };

        add_hook_to_output_manager(&mut manager, hook);

        assert_eq!(manager.list_hooks().len(), 2);
        assert_eq!(shared_reader.list_hooks().len(), 1);
    }
}

#[cfg(test)]
mod task_intent_command_tests {
    use super::*;
    use crate::ai::task_intent::TaskIntentType;

    fn minimal_test_task(task_intent: TaskIntentType) -> AgentTask {
        AgentTask {
            id: "task_test".to_string(),
            command: crate::cli::parser::CliCommand::Custom(String::new()),
            task_type: TaskType::Custom(task_intent.as_str().to_string()),
            task_intent: Some(task_intent),
            sandbox_policy: Some(task_intent.default_sandbox_policy()),
            priority: TaskPriority::Medium,
            priority_score: 50,
            status: TaskStatus::Pending,
            anchor_note: None,
            created_at: 0,
            max_retries: 0,
            retry_count: 0,
            synthesize_phase: SynthesizePhase::Idle,
            subagent_results: vec![],
            graph_manifest: None,
            has_trace: false,
            source_block_id: None,
            card_id: None,
            card_type: None,
            prompt: None,
            params: None,
            context_note: None,
            intent: task_intent.as_str().to_string(),
            content: String::new(),
            max_iterations: 1,
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
    fn cli_commands_map_to_task_intents() {
        let research = crate::cli::parser::CliCommand::DeepResearch {
            question: "How should Dream memory work?".to_string(),
            max_iterations: Some(5),
        };
        let dream = crate::cli::parser::CliCommand::Dream;
        let summarize = crate::cli::parser::CliCommand::Summarize {
            target: "Note.md".to_string(),
            style: crate::cli::parser::SummarizeStyle::Bullet,
        };

        assert_eq!(
            task_intent_for_cli_command(&research, None),
            TaskIntentType::Research
        );
        assert_eq!(
            task_intent_for_cli_command(&dream, None),
            TaskIntentType::Dream
        );
        assert_eq!(
            task_intent_for_cli_command(&summarize, None),
            TaskIntentType::Summarize
        );
    }

    #[test]
    fn dispatch_payload_can_select_task_intent_type() {
        let payload = serde_json::json!({
            "intent": "Rebuild MDT index",
            "content": "Index this vault",
            "task_type": "mdt_index"
        });

        assert_eq!(task_intent_from_payload(&payload), TaskIntentType::JsonLdIndex);
    }

    #[test]
    fn mdt_compat_index_output_keeps_jsonld_as_primary_memory_contract() {
        let output = render_mdt_compat_index_output(
            3,
            4,
            std::path::Path::new("vault/.dualtrack/jsonld/indexes"),
            1,
            2,
            std::path::Path::new("vault/.dualtrack/mdt/indexes"),
        );

        assert!(output.starts_with("## JSON-LD Index"));
        assert!(output.contains("MDT compatibility mirror"));
        assert!(output.contains("JSON-LD Nodes: 3"));
        assert!(output.contains("MDT Mirror Nodes: 1"));
        assert!(!output.contains("Legacy MDT"));
        assert!(
            output.find("JSON-LD Nodes").unwrap() < output.find("MDT Mirror Nodes").unwrap()
        );
    }

    #[test]
    fn memory_task_trace_headings_match_jsonld_migration_contract() {
        assert_eq!(
            memory_task_trace_heading(TaskIntentType::JsonLdIndex),
            "JSON-LD Memory Task"
        );
        assert_eq!(
            memory_task_trace_heading(TaskIntentType::MdtIndex),
            "JSON-LD Memory Task (MDT compatibility)"
        );
        assert_eq!(
            memory_task_trace_heading(TaskIntentType::MdtPack),
            "MDT Archive Task"
        );
    }

    #[test]
    fn mdt_read_query_uses_cli_query_when_intent_is_selected() {
        let cmd = crate::cli::parser::CliCommand::Search {
            query: "bayesian graph notes".to_string(),
            top_k: 5,
        };

        assert_eq!(mdt_task_query(None, &cmd), "bayesian graph notes");
    }

    #[test]
    fn mdt_pack_defaults_archive_under_dualtrack_snapshots() {
        let dualtrack_dir = std::path::PathBuf::from("vault/.dualtrack");

        let archive_path = mdt_archive_path(None, &dualtrack_dir, 1234).unwrap();

        assert_eq!(
            archive_path,
            dualtrack_dir
                .join("mdt")
                .join("snapshots")
                .join("mdt_1234.mdtz")
        );
    }

    #[test]
    fn mdt_pack_rejects_unsafe_task_archive_path() {
        let dualtrack_dir = std::path::PathBuf::from("vault/.dualtrack");
        let mut task = minimal_test_task(TaskIntentType::MdtPack);
        task.params = Some(std::collections::HashMap::from([(
            "archive_path".to_string(),
            "../outside.mdtz".to_string(),
        )]));

        let error = mdt_archive_path(Some(&task), &dualtrack_dir, 1234).unwrap_err();

        assert!(error.contains("unsafe MDT archive path"));
    }

    #[test]
    fn legacy_submit_task_payload_is_normalized_for_dispatch() {
        let payload = serde_json::json!({
            "card_type": "rewrite",
            "prompt": "rewrite selected paragraph",
            "params": { "target": "Note.md" }
        });

        let normalized = normalize_legacy_submit_task_payload(payload);

        assert_eq!(
            normalized["content"].as_str(),
            Some("rewrite selected paragraph")
        );
        assert!(normalized["intent"].as_str().unwrap().contains("rewrite"));
        assert!(normalized["timestamp"].as_u64().is_some());
        assert_eq!(
            task_intent_from_payload(&normalized),
            TaskIntentType::WriteProposal
        );
    }

    #[test]
    fn dispatch_review_action_respects_bridge_requirement() {
        let write_policy = TaskIntentType::WriteProposal.default_sandbox_policy();
        assert_eq!(
            dispatch_review_action_for_policy(&write_policy),
            DispatchReviewAction::PendingBridgeReview
        );

        let mut auto_policy = TaskIntentType::Verify.default_sandbox_policy();
        auto_policy.requires_bridge = false;
        assert_eq!(
            dispatch_review_action_for_policy(&auto_policy),
            DispatchReviewAction::AutoApprove
        );
    }

    #[test]
    fn bridge_review_preflight_rejects_missing_bridge_store() {
        let error =
            ensure_bridge_store_exists_for_review_action(DispatchReviewAction::PendingBridgeReview, false)
                .unwrap_err();

        assert!(error.contains("open a vault"));
        assert!(ensure_bridge_store_exists_for_review_action(
            DispatchReviewAction::PendingBridgeReview,
            true
        )
        .is_ok());
        assert!(ensure_bridge_store_exists_for_review_action(
            DispatchReviewAction::AutoApprove,
            false
        )
        .is_ok());
    }

    #[test]
    fn dispatch_payload_review_mode_can_force_safe_non_bridge_policy() {
        let read_only_payload = serde_json::json!({
            "task_type": "research",
            "review_mode": "read_only_auto_queue"
        });
        let read_only_policy =
            sandbox_policy_for_dispatch_payload(TaskIntentType::Research, &read_only_payload);

        assert!(!read_only_policy.requires_bridge);
        assert!(read_only_policy.write_roots.is_empty());

        let draft_payload = serde_json::json!({
            "task_type": "write_proposal",
            "review_mode": "draft_only"
        });
        let draft_policy =
            sandbox_policy_for_dispatch_payload(TaskIntentType::WriteProposal, &draft_payload);

        assert!(!draft_policy.requires_bridge);
        assert!(draft_policy.write_roots.is_empty());
        assert!(!draft_policy.allows_tool("ghost_write"));
        assert!(draft_policy.allows_tool("llm_complete"));
    }

    #[test]
    fn workflow_patch_review_proposal_is_stored_for_bridge_inbox() {
        let dir = tempfile::tempdir().unwrap();
        let store = BridgeProposalStore::new(dir.path().join("bridge"));
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
            predicted_impact: serde_json::json!({ "risk": "medium" }),
        };

        let saved = store_workflow_patch_review_proposal(
            &store,
            "run_runtime",
            &patch,
            TrustSnapshot::default(),
            1,
        )
        .expect("workflow patch review should be stored");

        assert_eq!(
            saved.source,
            crate::bridge::proposal::BridgeProposalSource::Scheduler
        );
        assert!(saved.actions.iter().any(|action| action.kind
            == crate::bridge::proposal::ProposalActionKind::ApproveWorkflowPatch));
        assert!(saved.actions.iter().any(|action| action.kind
            == crate::bridge::proposal::ProposalActionKind::RejectWorkflowPatch));

        let proposals = store.list(None).unwrap();
        assert_eq!(proposals.len(), 1);
        assert_eq!(proposals[0].source_ref.id, "run_runtime");
        assert_eq!(proposals[0].source_ref.path.as_deref(), Some("wf_runtime"));
    }

    #[test]
    fn orchestrator_output_workflow_patch_is_routed_to_bridge_inbox() {
        let dir = tempfile::tempdir().unwrap();
        let store = BridgeProposalStore::new(dir.path().join("bridge"));
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
            predicted_impact: serde_json::json!({ "risk": "medium" }),
        };
        let output = crate::harness::workflow::OrchestratorOutput::WorkflowPatch { patch };

        let saved = store_orchestrator_output_bridge_proposal(
            &store,
            "run_runtime",
            &output,
            TrustSnapshot::default(),
            1,
        )
        .expect("orchestrator output routing should not fail")
        .expect("workflow patch output should create a bridge proposal");

        assert_eq!(saved.source_ref.id, "run_runtime");
        assert_eq!(saved.source_ref.path.as_deref(), Some("wf_runtime"));
        assert!(saved.actions.iter().any(|action| action.kind
            == crate::bridge::proposal::ProposalActionKind::ApproveWorkflowPatch));
        assert_eq!(store.list(None).unwrap().len(), 1);
    }

    #[test]
    fn orchestrator_output_workflow_patch_routes_bridge_review_into_runtime_ledger() {
        let dir = tempfile::tempdir().unwrap();
        let store = BridgeProposalStore::new(dir.path().join("bridge"));
        let event_root = tempfile::tempdir().unwrap();
        let mut scheduler = crate::ai::agent_scheduler::AgentScheduler::new(2);
        scheduler.set_workflow_event_root(event_root.path());
        let patch = crate::harness::workflow::WorkflowPatch {
            patch_id: "patch_wf_runtime_v1_to_v2".to_string(),
            workflow_id: "wf_runtime".to_string(),
            from_version: 1,
            to_version: 2,
            basis: crate::harness::workflow::PatchBasis {
                failed_steps: vec!["S001".to_string()],
                failed_goal_clauses: vec![2],
            },
            ops: vec![crate::harness::workflow::WorkflowPatchOp::ReplaceStepStatus {
                step_id: "S001".to_string(),
                status: crate::harness::workflow::WorkflowStepStatus::Pending,
            }],
            rationale: "Retry verifier step after human review.".to_string(),
            predicted_impact: serde_json::json!({ "risk": "medium" }),
        };
        let output = crate::harness::workflow::OrchestratorOutput::WorkflowPatch { patch };

        let saved = route_orchestrator_output_review_to_bridge(
            &store,
            Some(&mut scheduler),
            "run_runtime",
            &output,
            TrustSnapshot::default(),
            1,
        )
        .expect("orchestrator output review routing should not fail")
        .expect("workflow patch output should create a bridge proposal");

        let events = scheduler
            .workflow_runtime_events_for_run("run_runtime", 10)
            .expect("routing should persist workflow runtime events");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_name, "workflow.patch.review_requested");
        assert_eq!(events[0].attributes["proposal_id"], saved.id);
        assert_eq!(events[0].attributes["patch_id"], "patch_wf_runtime_v1_to_v2");
    }

    #[test]
    fn card_result_ghost_storage_respects_sandbox_policy() {
        let write_policy = TaskIntentType::WriteProposal.default_sandbox_policy();
        assert!(should_store_card_result_ghost(&write_policy));

        let code_policy = TaskIntentType::CodeAssist.default_sandbox_policy();
        assert!(!should_store_card_result_ghost(&code_policy));
    }

    #[test]
    fn debug_llm_config_status_is_sanitized() {
        let status = debug_llm_config_status(
            "openai",
            "gpt-4o-mini",
            true,
            42,
            Some("sk-secret-value should not leak".to_string()),
        );

        assert_eq!(status["provider"], "openai");
        assert_eq!(status["model"], "gpt-4o-mini");
        assert_eq!(status["ok"], true);
        assert_eq!(status["latency_ms"], 42);
        assert!(!status.to_string().contains("sk-secret-value"));
    }

    #[test]
    fn custom_research_llm_prompt_carries_task_output_contract() {
        let prompt = custom_research::llm_user_prompt(
            "/agent research analyze seed",
            &[],
            Some("Output exactly 3 concise Chinese bullets."),
        );

        assert!(prompt.contains("/agent research analyze seed"));
        assert!(prompt.contains("Output exactly 3 concise Chinese bullets."));
        assert!(prompt.contains("No local note context matched"));
    }

    #[test]
    fn custom_research_missing_llm_message_is_reserved_for_unavailable_router() {
        let content = custom_research::no_llm_content("/agent research analyze seed", &[]);

        assert!(content.contains("No local matches found"));
        assert!(content.contains("Add an LLM API key"));
    }
}

#[allow(dead_code)]
pub fn register_commands(builder: tauri::Builder<tauri::Wry>) -> tauri::Builder<tauri::Wry> {
    builder.invoke_handler(tauri::generate_handler![
        search_notes,
        execute_cli,
        submit_task,
        approve_task,
        cancel_task,
        list_tasks,
        list_ai_face_data_flows,
        get_ai_manager_snapshot,
        get_task_manifest,
        get_task_trace,
        trigger_dream,
        get_vectordb_stats,
        get_config,
        set_config,
        debug_llm_config,
        dispatch_agent_task,
        record_ghost_feedback,
        inspect_ghost,
        plan_research,
        check_ghost_conflicts,
        get_suggestions,
        orchestrator_status,
        read_workflow_runtime_events,
        submit_workflow_patch_review,
        submit_orchestrator_output_review,
        orchestrator_events,
        orchestrator_terminate,
        orchestrator_reinstate,
        get_dream_status,
        get_scheduler_status,
        get_trust_score_info,
        translate_research,
        verify_proposition_graph,
        list_skills,
        plugin_status,
        search_fulltext,
        jsonld_validate,
        jsonld_migrate,
        jsonld_index,
        jsonld_read,
        mdt_validate,
        mdt_index,
        mdt_read,
        mdt_pack,
        mdt_unpack,
    ])
}
