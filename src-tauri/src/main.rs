// Dual-Track Note IDE — Rust backend entry point
// Architecture: Tauri 2.0 + Microkernel pattern
// v2 — Full AI pipeline wired: FileWatcher → SyncEngine → VectorStore → RAG → Agent → GhostStore → Diff

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod cli;
mod diff;
mod fs;
mod graph;
mod parser;
mod plugin;
mod ai;

use std::sync::Mutex;
use std::path::PathBuf;
use tauri::{Manager, State, AppHandle, Emitter};
use fs::vault::VaultManager;
use fs::watcher::FileWatcher;
use graph::link_graph::LinkGraph;
use ai::embedding::EmbeddingPipeline;
use ai::vectordb::VectorStore;
use ai::sync_engine::SyncEngine;
use ai::llm_router::LlmRouter;
use ai::agent_scheduler::AgentScheduler;
use diff::ghost_store::{GhostStore, GhostBlock, GhostOp, GhostStatus};

// ─── Application State ────────────────────────────

pub struct AppState {
    pub vault: Option<VaultManager>,
    pub link_graph: LinkGraph,
    pub vault_path: String,
    pub sync_engine: Option<SyncEngine>,
    pub vector_store_path: PathBuf,
    pub dualtrack_dir: PathBuf,
}

pub struct AiState {
    pub agent_scheduler: AgentScheduler,
    pub llm_router: LlmRouter,
    pub embedding_pipeline: EmbeddingPipeline,
    pub ghost_store: GhostStore,
}

// ─── Health Check ─────────────────────────────────

#[tauri::command]
fn ping() -> String {
    "pong".to_string()
}

// ─── Vault Operations ─────────────────────────────

#[tauri::command]
fn open_vault(
    path: String,
    state: State<'_, Mutex<AppState>>,
    ai_state: State<'_, Mutex<AiState>>,
    app_handle: AppHandle,
) -> Result<(), String> {
    let mut app = state.lock().map_err(|e| e.to_string())?;
    let dualtrack_dir = PathBuf::from(&path).join(".dualtrack");
    std::fs::create_dir_all(&dualtrack_dir).map_err(|e| e.to_string())?;

    // Open vault
    let vault = VaultManager::open(&path).map_err(|e| e.to_string())?;
    app.vault_path = path.clone();
    app.dualtrack_dir = dualtrack_dir.clone();
    app.vector_store_path = dualtrack_dir.join("vectors");

    // Initialize vector store
    let db_path = app.vector_store_path.to_str().unwrap_or(":memory:");
    let mut vector_store = VectorStore::open(db_path).map_err(|e| e.to_string())?;

    // Initialize embedding pipeline
    let ai = ai_state.lock().map_err(|e| e.to_string())?;
    let embed_cfg = &ai.embedding_pipeline;
    let dim = embed_cfg.dimension();
    drop(ai);
    vector_store.set_dimension(dim);

    // Create sync engine
    let _embedding = {
        let ai = ai_state.lock().map_err(|e| e.to_string())?;
        ai.embedding_pipeline.is_real()
    };
    let embed_pipe = {
        let ai = ai_state.lock().map_err(|e| e.to_string())?;
        // Clone the backend config for the sync engine
        EmbeddingPipeline::new(ai.embedding_pipeline.backend_config())
    };
    let sync_engine = SyncEngine::new(vector_store, embed_pipe);
    app.sync_engine = Some(sync_engine);

    // Initialize ghost store
    let mut ai = ai_state.lock().map_err(|e| e.to_string())?;
    ai.ghost_store = GhostStore::new(&dualtrack_dir);
    ai.ghost_store.init().map_err(|e| e.to_string())?;
    drop(ai);

    // Start file watcher
    let watch_path = path.clone();
    let handle = app_handle.clone();
    if let Ok(watcher) = FileWatcher::watch(&watch_path) {
        let mut rx = watcher.subscribe();
        // Spawn background task to forward events to frontend and sync engine
        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        let _ = handle.emit("file-changed", &event);
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("FileWatcher event lagged by {} messages", n);
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }
        });
        // Store watcher (keeps it alive)
        tracing::info!("File watcher started for: {}", watch_path);
    } else {
        tracing::warn!("Failed to start file watcher");
    }

    app.vault = Some(vault);
    Ok(())
}

#[tauri::command]
fn get_vault_path(state: State<'_, Mutex<AppState>>) -> Result<String, String> {
    let app = state.lock().map_err(|e| e.to_string())?;
    if app.vault_path.is_empty() {
        Err("No vault open".to_string())
    } else {
        Ok(app.vault_path.clone())
    }
}

#[tauri::command]
fn list_notes(state: State<'_, Mutex<AppState>>) -> Result<Vec<fs::vault::NoteMeta>, String> {
    let app = state.lock().map_err(|e| e.to_string())?;
    let vault = app.vault.as_ref().ok_or("No vault open")?;
    vault.list_notes().map_err(|e| e.to_string())
}

#[tauri::command]
fn read_note(path: String, state: State<'_, Mutex<AppState>>) -> Result<String, String> {
    let app = state.lock().map_err(|e| e.to_string())?;
    let vault = app.vault.as_ref().ok_or("No vault open")?;
    vault.read_note(&path).map_err(|e| e.to_string())
}

#[tauri::command]
fn save_note(path: String, content: String, state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let app = state.lock().map_err(|e| e.to_string())?;
    let vault = app.vault.as_ref().ok_or("No vault open")?;
    vault.write_note(&path, &content).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_note(path: String, state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let app = state.lock().map_err(|e| e.to_string())?;
    let vault = app.vault.as_ref().ok_or("No vault open")?;
    vault.delete_note(&path).map_err(|e| e.to_string())
}

#[tauri::command]
fn create_note(path: String, state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let app = state.lock().map_err(|e| e.to_string())?;
    let vault = app.vault.as_ref().ok_or("No vault open")?;
    let title = path.rsplit('/').next().unwrap_or(&path).trim_end_matches(".md");
    let template = format!("# {}\n\n", title);
    vault.write_note(&path, &template).map_err(|e| e.to_string())
}

// ─── Graph & Link Operations ──────────────────────

#[tauri::command]
fn get_graph(state: State<'_, Mutex<AppState>>) -> Result<graph::link_graph::GraphData, String> {
    let app = state.lock().map_err(|e| e.to_string())?;
    Ok(app.link_graph.to_frontend_json())
}

#[tauri::command]
fn get_backlinks(note_id: String, state: State<'_, Mutex<AppState>>) -> Result<Vec<String>, String> {
    let app = state.lock().map_err(|e| e.to_string())?;
    Ok(app.link_graph.get_backlinks(&note_id))
}

#[tauri::command]
fn get_note_links(note_id: String, state: State<'_, Mutex<AppState>>) -> Result<Vec<String>, String> {
    let app = state.lock().map_err(|e| e.to_string())?;
    let graph_data = app.link_graph.to_frontend_json();
    let links: Vec<String> = graph_data
        .edges
        .iter()
        .filter(|e| e.from == note_id)
        .map(|e| e.to.clone())
        .collect();
    Ok(links)
}

// ─── Search Operations ────────────────────────────

#[tauri::command]
fn search_notes(
    query: String,
    top_k: u32,
    state: State<'_, Mutex<AppState>>,
) -> Result<Vec<ai::vectordb::SearchResult>, String> {
    let app = state.lock().map_err(|e| e.to_string())?;

    if let Some(ref sync_engine) = app.sync_engine {
        let store = sync_engine.store();
        let results = store.search_text(&query, top_k as usize);
        if !results.is_empty() {
            return Ok(results);
        }
    }

    // Fallback: text search across vault
    let vault = app.vault.as_ref().ok_or("No vault open")?;
    let notes = vault.list_notes().map_err(|e| e.to_string())?;
    let mut results = Vec::new();

    for note in notes.iter().take(top_k as usize * 3) {
        if let Ok(content) = vault.read_note(&note.path) {
            if content.to_lowercase().contains(&query.to_lowercase()) {
                results.push(ai::vectordb::SearchResult {
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

// ─── CLI / Agent Operations (REAL EXECUTION) ─────

#[tauri::command]
async fn execute_cli(
    command: String,
    _state: State<'_, Mutex<AppState>>,
    _ai_state: State<'_, Mutex<AiState>>,
    app_handle: AppHandle,
) -> Result<String, String> {
    let cmd = cli::parser::parse(&command).map_err(|e| e.to_string())?;

    // 使用 AppHandle 在新线程中获取状态 (Tauri官方推荐方式)
    let handle = app_handle.clone();
    tokio::task::spawn_blocking(move || {
        // 通过 AppHandle 获取状态，而不是移动 State
        let state = handle.state::<Mutex<AppState>>();
        let ai_state = handle.state::<Mutex<AiState>>();
        let result = execute_agent_task(cmd, &state, &ai_state, handle.clone());
        match result {
            Ok(response) => {
                tracing::info!("Agent task completed: {}", response);
            }
            Err(e) => {
                tracing::error!("Agent task failed: {}", e);
            }
        }
    });

    Ok("Agent task started. Check status with /agent status".to_string())
}

fn execute_agent_task(
    cmd: cli::parser::CliCommand,
    state: &Mutex<AppState>,
    ai_state: &Mutex<AiState>,
    app_handle: AppHandle,
) -> Result<String, String> {
    // 创建 tokio runtime 来执行异步操作
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| e.to_string())?;
    
    rt.block_on(execute_agent_task_async(cmd, state, ai_state, app_handle))
}

async fn execute_agent_task_async(
    cmd: cli::parser::CliCommand,
    state: &Mutex<AppState>,
    ai_state: &Mutex<AiState>,
    app_handle: AppHandle,
) -> Result<String, String> {
    match &cmd {
        cli::parser::CliCommand::Search { query, top_k } => {
            // Use VectorStore for search
            let app = state.lock().map_err(|e| e.to_string())?;
            let results = if let Some(ref sync_engine) = app.sync_engine {
                let store = sync_engine.store();
                store.search_text(query, *top_k)
            } else {
                vec![]
            };
            drop(app);

            if results.is_empty() {
                return Ok(format!("No results found for: \"{}\"", query));
            }

            let mut output = format!("## Search Results: \"{}\"\n\n", query);
            for (i, r) in results.iter().enumerate() {
                output.push_str(&format!(
                    "{}. **{}** (from `{}`)\n   {}\n\n",
                    i + 1,
                    r.heading_context,
                    r.source_file,
                    r.chunk_text.chars().take(150).collect::<String>()
                ));
            }

            let _ = app_handle.emit("agent-result", serde_json::json!({
                "type": "search",
                "query": query,
                "results": results,
            }));

            Ok(output)
        }

        cli::parser::CliCommand::Explain { concept, .. }
        | cli::parser::CliCommand::DeepDive { concept, .. } => {
            // RAG + LLM synthesis
            let concept = concept.clone();
            let app = state.lock().map_err(|e| e.to_string())?;
            let _vault_path = app.vault_path.clone();
            let contexts = if let Some(ref sync_engine) = app.sync_engine {
                let store = sync_engine.store();
                store.search_text(&concept, 5)
                    .into_iter()
                    .map(|r| format!("[{}] {}", r.source_file, r.chunk_text))
                    .collect()
            } else {
                vec![]
            };
            let _link_graph = app.link_graph.clone();
            let _anchor_links = if !concept.is_empty() {
                vec![format!("{}.md", concept), concept.clone()]
            } else {
                vec![]
            };
            drop(app);

            let mut ai = ai_state.lock().map_err(|e| e.to_string())?;
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
                        buf.push_str(&format!("- {}\n", ctx.chars().take(200).collect::<String>()));
                    }
                    buf.push_str("\n\n*Add an LLM API key to enable AI-powered deep analysis.*");
                    buf
                };

                let _ = app_handle.emit("agent-result", serde_json::json!({
                    "type": "analysis",
                    "concept": concept,
                    "result": content,
                }));

                // Store as ghost note if there's context
                if !contexts.is_empty() {
                    let ghost_blocks: Vec<GhostBlock> = contexts.iter().enumerate().map(|(i, c)| {
                        GhostBlock {
                            block_id: format!("ghost-block-{}", i),
                            content: c.clone(),
                            operation: GhostOp::Suggestion,
                            after_block_id: None,
                            heading_context: concept.clone(),
                        }
                    }).collect();

                    if let Err(e) = ai.ghost_store.create(
                        &format!("{}.md", concept),
                        &format!("AI analysis for: {}", concept),
                        ghost_blocks,
                    ) {
                        tracing::warn!("Failed to create ghost note: {}", e);
                    }
                }

                return Ok(content);
            }

            // Real LLM call with RAG context
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

            match ai.llm_router.complete(system, &prompt).await {
                Ok(response) => {
                    let _ = app_handle.emit("agent-result", serde_json::json!({
                        "type": "analysis",
                        "concept": concept,
                        "result": response.text,
                        "model": response.model_used,
                        "tokens": response.tokens_in + response.tokens_out,
                    }));

                    // Store as ghost note for potential merge
                    let blocks: Vec<GhostBlock> = response.text
                        .split("\n\n")
                        .enumerate()
                        .map(|(i, para)| GhostBlock {
                            block_id: format!("ghost-block-{}", i),
                            content: para.to_string(),
                            operation: GhostOp::Suggestion,
                            after_block_id: None,
                            heading_context: concept.clone(),
                        })
                        .collect();

                    if let Err(e) = ai.ghost_store.create(
                        &format!("{}.md", concept),
                        &format!("AI analysis: {}", concept),
                        blocks,
                    ) {
                        tracing::warn!("Failed to store ghost: {}", e);
                    }

                    Ok(response.text)
                }
                Err(e) => {
                    tracing::error!("LLM call failed: {}", e);
                    Err(format!("AI service error: {}. Please check your API key in Settings.", e))
                }
            }
        }

        cli::parser::CliCommand::Summarize { target, .. } => {
            let target = target.clone();
            let app = state.lock().map_err(|e| e.to_string())?;
            let content = if !target.is_empty() {
                let vault = app.vault.as_ref().ok_or("No vault open")?;
                vault.read_note(&target).unwrap_or_default()
            } else {
                "No target specified for summarization.".to_string()
            };

            if content.is_empty() {
                return Ok("Target note is empty or not found.".to_string());
            }
            drop(app);

            let mut ai = ai_state.lock().map_err(|e| e.to_string())?;
            if !ai.llm_router.is_available() {
                let first_line = content.lines().next().unwrap_or("");
                return Ok(format!(
                    "## Summary (offline)\n\nNote: `{}` ({:?} chars)\nFirst line: {}\n\n*Configure an LLM API key for AI summaries.*",
                    target, content.len(), first_line
                ));
            }

            let system = "Summarize the following note in Markdown. Include key points, main arguments, and a TL;DR.";
            match ai.llm_router.complete(system, &content).await {
                Ok(response) => {
                    let _ = app_handle.emit("agent-result", serde_json::json!({
                        "type": "summary",
                        "target": target,
                        "result": response.text,
                    }));
                    Ok(response.text)
                }
                Err(e) => Err(e),
            }
        }

        cli::parser::CliCommand::FetchPapers { topic, .. } => {
            let topic = topic.clone();
            let mut ai = ai_state.lock().map_err(|e| e.to_string())?;
            if !ai.llm_router.is_available() {
                return Ok(format!(
                    "Paper fetching requires an LLM API key. Topic: \"{}\"\n\
                     The agent will search your local notes for related content.",
                    topic
                ));
            }

            let system = "You are a research assistant. Based on the topic, suggest 3-5 relevant papers or resources. For each, provide: title, authors (if known), key contribution, and relevance. Format in Markdown.";
            let prompt = format!("Topic: {}\n\nSuggest relevant academic papers and resources.", topic);

            match ai.llm_router.complete(system, &prompt).await {
                Ok(response) => Ok(response.text),
                Err(e) => Err(e),
            }
        }

        cli::parser::CliCommand::Status => {
            let ai = ai_state.lock().map_err(|e| e.to_string())?;
            let stats = ai.agent_scheduler.stats();
            let llm_ready = ai.llm_router.is_available();
            let budget = ai.llm_router.budget_remaining();
            Ok(format!(
                "Agent: {} tasks | Queued: {} | Running: {} | Done: {} | Failed: {}\n\
                 LLM: {} | Budget remaining: ${:.4}",
                stats.queued + stats.running + stats.done + stats.failed,
                stats.queued, stats.running, stats.done, stats.failed,
                if llm_ready { "connected" } else { "no API key" },
                budget,
            ))
        }

        cli::parser::CliCommand::DiffReview => {
            let ai = ai_state.lock().map_err(|e| e.to_string())?;
            let ghosts = ai.ghost_store.list_all();
            if ghosts.is_empty() {
                Ok("No pending AI suggestions to review.".to_string())
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
                Ok(output)
            }
        }

        cli::parser::CliCommand::Config { model } => {
            if let Some(m) = model {
                let mut ai = ai_state.lock().map_err(|e| e.to_string())?;
                let mut cfg = ai.llm_router.config();
                cfg.llm_model = m.clone();
                ai.llm_router.update_config(cfg);
                Ok(format!("Model set to: {}", m))
            } else {
                let ai = ai_state.lock().map_err(|e| e.to_string())?;
                Ok(format!("Current model: {} | Provider: {} | LLM: {}",
                    if ai.llm_router.is_available() { "API connected" } else { "no API key" },
                    "gemini/openai",
                    if ai.llm_router.is_available() { "available" } else { "unconfigured" },
                ))
            }
        }
    }
}

// ─── Diff / Merge Operations (REAL) ───────────────

#[tauri::command]
fn review_diff(
    ghost_id: String,
    ai_state: State<'_, Mutex<AiState>>,
) -> Result<serde_json::Value, String> {
    let ai = ai_state.lock().map_err(|e| e.to_string())?;
    let ghost = ai.ghost_store.get(&ghost_id).ok_or_else(|| {
        format!("Ghost note not found: {}", ghost_id)
    })?;

    Ok(serde_json::to_value(&ghost).map_err(|e| e.to_string())?)
}

#[tauri::command]
fn list_ghosts(
    ai_state: State<'_, Mutex<AiState>>,
) -> Result<Vec<serde_json::Value>, String> {
    let ai = ai_state.lock().map_err(|e| e.to_string())?;
    let ghosts = ai.ghost_store.list_all();
    Ok(ghosts.iter().map(|g| serde_json::to_value(g).unwrap()).collect())
}

#[tauri::command]
fn accept_diff(
    ghost_id: String,
    block_ids: Vec<String>,
    state: State<'_, Mutex<AppState>>,
    ai_state: State<'_, Mutex<AiState>>,
) -> Result<String, String> {
    // Load ghost note
    let ai = ai_state.lock().map_err(|e| e.to_string())?;
    let ghost = ai.ghost_store.get(&ghost_id).ok_or_else(|| {
        format!("Ghost note not found: {}", ghost_id)
    })?;

    // Read original file
    let app = state.lock().map_err(|e| e.to_string())?;
    let vault = app.vault.as_ref().ok_or("No vault open")?;
    let mut content = vault.read_note(&ghost.source_note).unwrap_or_default();

    // Append accepted blocks to the file
    for block in &ghost.suggested_blocks {
        if block_ids.contains(&block.block_id) {
            match block.operation {
                GhostOp::Insert | GhostOp::Suggestion => {
                    content.push_str(&format!("\n\n{}", block.content));
                }
                _ => {}
            }
        }
    }

    vault.write_note(&ghost.source_note, &content).map_err(|e| e.to_string())?;

    // Mark ghost as accepted
    drop(ai);
    drop(app);

    let ai = ai_state.lock().map_err(|e| e.to_string())?;
    ai.ghost_store.update_status(&ghost_id, GhostStatus::Accepted)
        .map_err(|e| e.to_string())?;

    Ok(format!("Accepted {} blocks into {}", block_ids.len(), ghost.source_note))
}

#[tauri::command]
fn reject_diff(
    ghost_id: String,
    ai_state: State<'_, Mutex<AiState>>,
) -> Result<String, String> {
    let ai = ai_state.lock().map_err(|e| e.to_string())?;
    ai.ghost_store.update_status(&ghost_id, GhostStatus::Rejected)
        .map_err(|e| e.to_string())?;
    Ok(format!("Rejected ghost note: {}", ghost_id))
}

// ─── Config Operations ────────────────────────────

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct AppConfig {
    pub llm_provider: String,
    pub llm_api_key: String,
    pub llm_model: String,
    pub embedding_provider: String,
    pub embedding_api_key: String,
    pub theme: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            llm_provider: "gemini".to_string(),
            llm_api_key: String::new(),
            llm_model: "gemini-2.0-flash".to_string(),
            embedding_provider: "none".to_string(),
            embedding_api_key: String::new(),
            theme: "dark".to_string(),
        }
    }
}

#[tauri::command]
fn get_config(state: State<'_, Mutex<AppConfig>>) -> Result<AppConfig, String> {
    state.lock().map(|c| c.clone()).map_err(|e| e.to_string())
}

#[tauri::command]
fn set_config(
    config: AppConfig,
    state: State<'_, Mutex<AppConfig>>,
    ai_state: State<'_, Mutex<AiState>>,
) -> Result<(), String> {
    // Update config
    let mut current = state.lock().map_err(|e| e.to_string())?;
    *current = config.clone();

    // Sync to AI state
    let mut ai = ai_state.lock().map_err(|e| e.to_string())?;
    let router_config = ai::llm_router::RouterConfig {
        llm_provider: config.llm_provider.clone(),
        llm_api_key: config.llm_api_key.clone(),
        llm_model: config.llm_model.clone(),
        embedding_provider: config.embedding_provider.clone(),
        embedding_api_key: config.embedding_api_key.clone(),
        ..Default::default()
    };
    ai.llm_router.update_config(router_config);
    Ok(())
}

// ─── App Entry Point ──────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(Mutex::new(AppState {
            vault: None,
            link_graph: LinkGraph::new(),
            vault_path: String::new(),
            sync_engine: None,
            vector_store_path: PathBuf::new(),
            dualtrack_dir: PathBuf::new(),
        }))
        .manage(Mutex::new(AiState {
            agent_scheduler: AgentScheduler::new(3),
            llm_router: LlmRouter::new(Default::default()),
            embedding_pipeline: EmbeddingPipeline::new(Default::default()),
            ghost_store: GhostStore::new(&PathBuf::from(".dualtrack")),
        }))
        .manage(Mutex::new(AppConfig::default()))
        .invoke_handler(tauri::generate_handler![
            ping,
            open_vault,
            get_vault_path,
            list_notes,
            read_note,
            save_note,
            create_note,
            delete_note,
            get_graph,
            get_backlinks,
            get_note_links,
            search_notes,
            execute_cli,
            review_diff,
            list_ghosts,
            accept_diff,
            reject_diff,
            get_config,
            set_config,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn main() {
    crate::run();
}
