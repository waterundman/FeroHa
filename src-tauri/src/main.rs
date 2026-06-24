// Dual-Track Note IDE — Rust backend entry point
// Architecture: Tauri 2.0 + Microkernel pattern
// v3.0.0 - Memory architecture line with Bridge closure baseline

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ai;
mod bridge;
mod cli;
mod diff;
mod fs;
mod graph;
mod harness;
mod ipc;
mod jsonld;
mod mdt;
mod parser;
mod plugin;
mod snapshot;
mod state;

use ai::agent_scheduler::AgentScheduler;
use ai::embedding::EmbeddingPipeline;
use ai::llm_router::LlmRouter;
use diff::ghost_store::GhostStore;
use graph::link_graph::LinkGraph;
use ipc::protocol::TwoSurfaceProtocol;
pub use state::{AiState, AppConfig, AppState};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::Manager;
use tokio::sync::Notify;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt::init();
    let initial_config = AppConfig::from_env();
    let initial_router_config = initial_config.to_router_config();
    let initial_embedding_backend = initial_config.to_embedding_backend();

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(Mutex::new(AppState {
            vault: None,
            file_watcher: None,
            link_graph: LinkGraph::new(),
            vault_path: String::new(),
            sync_engine: None,
            vector_store_path: PathBuf::new(),
            dualtrack_dir: PathBuf::new(),
            snapshot_engine: None,
            dream_engine: None,
            protocol: Some(TwoSurfaceProtocol::default()),
            search_engine: None,
            output_manager: None,
            bridge_store: None,
            snapshot_listeners_started: false,
        }))
        .manage(Mutex::new(AiState {
            agent_scheduler: AgentScheduler::new(3),
            llm_router: LlmRouter::new(initial_router_config),
            embedding_pipeline: EmbeddingPipeline::new(initial_embedding_backend),
            ghost_store: GhostStore::new(&PathBuf::from(".dualtrack")),
            subagent: None,
            skill_manager: None,
            scheduler: None,
            task_notifier: Arc::new(Notify::new()),
            task_worker_started: false,
        }))
        .manage(Mutex::new(initial_config))
        .setup(|app| {
            if let Some(bootstrap_vault) = std::env::var_os("FEROHA_BOOTSTRAP_VAULT") {
                let bootstrap_vault = PathBuf::from(bootstrap_vault);
                eprintln!("FEROHA setup: bootstrapping vault at {:?}", bootstrap_vault);
                let bootstrap_path = bootstrap_vault.to_string_lossy().to_string();
                let app_handle = app.handle().clone();
                let app_state = app.state::<Mutex<AppState>>();
                let ai_state = app.state::<Mutex<AiState>>();
                if let Err(error) =
                    fs::commands::open_vault_runtime(&bootstrap_path, app_handle, app_state, ai_state)
                {
                    eprintln!("FEROHA setup: failed to bootstrap vault: {}", error);
                }
            }

            let webview_data_dir = std::env::var_os("FEROHA_WEBVIEW_DATA_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| std::env::temp_dir().join("feroha-manual-main-webview"));
            eprintln!("FEROHA setup: creating main webview window at {:?}", webview_data_dir);
            tracing::info!("Creating main webview window at {:?}", webview_data_dir);

            #[cfg(debug_assertions)]
            let main_url = tauri::WebviewUrl::External(
                std::env::var("FEROHA_WEBVIEW_URL")
                    .unwrap_or_else(|_| "http://127.0.0.1:1420".to_string())
                    .parse()
                    .expect("valid dev url"),
            );
            #[cfg(not(debug_assertions))]
            let main_url = tauri::WebviewUrl::App("index.html".into());

            tauri::WebviewWindowBuilder::new(app, "main", main_url)
                .title("贝叶斯笔记")
                .inner_size(1400.0, 900.0)
                .min_inner_size(900.0, 600.0)
                .resizable(true)
                .visible(true)
                .decorations(false)
                .shadow(true)
                .data_directory(webview_data_dir)
                .build()?;
            eprintln!("FEROHA setup: main webview window created");

            Ok(())
        });

    builder
        .invoke_handler(tauri::generate_handler![
            fs::commands::ping,
            fs::commands::open_vault,
            fs::commands::get_vault_path,
            fs::commands::list_notes,
            fs::commands::list_folders,
            fs::commands::list_ai_workspace_files,
            fs::commands::read_note,
            fs::commands::save_note,
            fs::commands::save_asset,
            fs::commands::create_note,
            fs::commands::delete_note,
            fs::commands::rename_note,
            fs::commands::create_folder,
            fs::commands::list_templates,
            fs::commands::list_tags,
            fs::commands::get_note_tags,
            graph::commands::get_graph,
            graph::commands::get_graph_with_focus,
            graph::commands::get_backlinks,
            graph::commands::get_note_links,
            ai::commands::search_notes,
            ai::commands::execute_cli,
            ai::commands::submit_task,
            ai::commands::approve_task,
            ai::commands::cancel_task,
            ai::commands::list_tasks,
            ai::commands::list_ai_face_data_flows,
            ai::commands::get_ai_manager_snapshot,
            ai::commands::get_task_manifest,
            ai::commands::get_task_trace,
            ai::commands::trigger_dream,
            ai::commands::get_vectordb_stats,
            ai::commands::get_config,
            ai::commands::set_config,
            ai::commands::debug_llm_config,
            ai::commands::dispatch_agent_task,
            ai::commands::record_ghost_feedback,
            ai::commands::inspect_ghost,
            ai::commands::plan_research,
            ai::commands::check_ghost_conflicts,
            ai::commands::get_suggestions,
            ai::commands::orchestrator_status,
            ai::commands::read_workflow_runtime_events,
            ai::commands::submit_workflow_patch_review,
            ai::commands::submit_orchestrator_output_review,
            ai::commands::orchestrator_events,
            ai::commands::orchestrator_terminate,
            ai::commands::orchestrator_reinstate,
            ai::workflow_commands::create_and_start_workflow,
            ai::workflow_commands::get_workflow_run,
            ai::workflow_commands::list_workflow_runs,
            ai::commands::get_dream_status,
            ai::commands::get_scheduler_status,
            ai::commands::get_trust_score_info,
            ai::commands::translate_research,
            ai::commands::verify_proposition_graph,
            ai::commands::list_skills,
            ai::commands::plugin_status,
            ai::commands::search_fulltext,
            ai::commands::jsonld_validate,
            ai::commands::jsonld_migrate,
            ai::commands::jsonld_index,
            ai::commands::jsonld_read,
            ai::commands::mdt_validate,
            ai::commands::mdt_index,
            ai::commands::mdt_read,
            ai::commands::mdt_pack,
            ai::commands::mdt_unpack,
            ai::commands::list_agent_tools,
            bridge::commands::list_bridge_proposals,
            bridge::commands::get_bridge_proposal,
            bridge::commands::update_bridge_proposal_status,
            bridge::commands::execute_bridge_action,
            diff::commands::get_diff_blocks,
            diff::commands::review_diff,
            diff::commands::list_ghosts,
            diff::commands::accept_diff,
            diff::commands::reject_diff,
            ai::commands::list_output_hooks,
            ai::commands::add_output_hook,
            snapshot::commands::get_current_snapshot,
            snapshot::commands::get_snapshot_diff,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn main() {
    crate::run();
}
