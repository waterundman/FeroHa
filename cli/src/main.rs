// FeroHa CLI — Standalone CLI binary for the FeroHa Dual-Track AI Note IDE
// v2.14.0 — Stage 1: Agent CLI + Kernel Facade

use clap::{Parser, Subcommand};

/// FeroHa — Dual-Track AI Note IDE CLI
#[derive(Parser)]
#[command(name = "feroha", version = "2.14.1", about = "FeroHa dual-track note IDE CLI")]
struct Cli {
    /// Path to the vault directory
    #[arg(short, long, env = "FERoha_VAULT")]
    vault: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Agent task management and execution
    Agent {
        #[command(subcommand)]
        action: AgentAction,
    },
    /// Note CRUD operations
    Note {
        #[command(subcommand)]
        action: NoteAction,
    },
    /// Knowledge graph operations
    Graph {
        #[command(subcommand)]
        action: GraphAction,
    },
}

#[derive(Subcommand)]
enum AgentAction {
    /// Search the vault using the agent system
    Search {
        /// Search query
        query: String,
        /// Data sources (local, web, arxiv, semantic-scholar)
        #[arg(short, long, value_delimiter = ',', default_value = "local")]
        sources: Vec<String>,
        /// Number of results
        #[arg(long, default_value = "5")]
        top_k: usize,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Deep research on a question
    Research {
        /// Research question
        question: String,
        /// Research depth
        #[arg(long, default_value = "2")]
        depth: usize,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Trigger a dream cycle (memory consolidation)
    Dream {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// List agent tasks
    Tasks {
        /// Filter tasks by status
        #[arg(long)]
        status: Option<String>,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Approve a pending agent task
    Approve {
        /// Task ID to approve
        task_id: String,
    },
    /// Reject a pending agent task
    Reject {
        /// Task ID to reject
        task_id: String,
    },
    /// Get status of a specific task
    Status {
        /// Task ID
        task_id: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Show orchestrator status
    Orchestrator {
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Run a pipeline
    Pipeline {
        #[command(subcommand)]
        action: PipelineAction,
    },
}

#[derive(Subcommand)]
enum PipelineAction {
    /// Run a pipeline by ID
    Run {
        /// Pipeline ID
        pipeline_id: String,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum NoteAction {
    /// Create a new note
    Create {
        /// Note title
        title: String,
    },
    /// List all notes in the vault
    List,
    /// Search notes by keyword
    Search {
        /// Search keyword
        keyword: String,
    },
    /// Read a note's content (outputs to stdout)
    Read {
        /// Note path relative to vault root
        path: String,
    },
    /// Delete a note
    Delete {
        /// Note path relative to vault root
        path: String,
    },
}

#[derive(Subcommand)]
enum GraphAction {
    /// Export the knowledge graph
    Export {
        /// Output format
        #[arg(long, default_value = "json")]
        format: String,
    },
}

fn resolve_vault(cli_vault: Option<String>) -> Result<String, String> {
    cli_vault.ok_or_else(|| {
        "No vault specified. Use --vault <PATH> or set FERoha_VAULT environment variable."
            .to_string()
    })
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Agent { action } => handle_agent(action, cli.vault),
        Commands::Note { action } => handle_note(action, cli.vault),
        Commands::Graph { action } => handle_graph(action, cli.vault),
    }
}

// ---------------------------------------------------------------------------
// Agent handlers
// ---------------------------------------------------------------------------

fn handle_agent(action: AgentAction, vault: Option<String>) {
    let _vault_path = match resolve_vault(vault) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    match action {
        AgentAction::Search {
            query,
            sources,
            top_k,
            json,
        } => {
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "status": "not_implemented",
                        "query": query,
                        "sources": sources,
                        "top_k": top_k,
                        "message": "Agent search requires an initialized kernel with sync engine"
                    })
                );
            } else {
                println!(
                    "Search: '{}' (sources: {:?}, top_k: {})",
                    query, sources, top_k
                );
                println!("Status: Not yet implemented — requires initialized kernel with sync engine");
            }
        }
        AgentAction::Research {
            question,
            depth,
            json,
        } => {
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "status": "not_implemented",
                        "question": question,
                        "depth": depth,
                        "message": "Deep research requires an LLM API key configured in the kernel"
                    })
                );
            } else {
                println!("Research: '{}' (depth: {})", question, depth);
                println!("Status: Not yet implemented — requires initialized kernel with LLM router");
            }
        }
        AgentAction::Dream { json } => {
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "status": "not_implemented",
                        "message": "Dream engine requires an initialized kernel with sync engine"
                    })
                );
            } else {
                println!("Dream: Not yet implemented — requires initialized kernel with sync engine");
            }
        }
        AgentAction::Tasks { status, json } => {
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "status": "not_implemented",
                        "filter": status,
                        "message": "Task listing requires an initialized kernel with agent scheduler"
                    })
                );
            } else {
                println!(
                    "Tasks (status filter: {:?}): Not yet implemented",
                    status
                );
            }
        }
        AgentAction::Approve { task_id } => {
            println!("Approve task '{}': Not yet implemented", task_id);
        }
        AgentAction::Reject { task_id } => {
            println!("Reject task '{}': Not yet implemented", task_id);
        }
        AgentAction::Status { task_id, json } => {
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "status": "not_implemented",
                        "task_id": task_id,
                        "message": "Task status requires an initialized kernel with agent scheduler"
                    })
                );
            } else {
                println!(
                    "Task status '{}': Not yet implemented",
                    task_id
                );
            }
        }
        AgentAction::Orchestrator { json } => {
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "status": "not_implemented",
                        "message": "Orchestrator status requires an initialized kernel with agent scheduler"
                    })
                );
            } else {
                println!("Orchestrator: Not yet implemented");
            }
        }
        AgentAction::Pipeline {
            action: PipelineAction::Run {
                pipeline_id,
                json,
            },
        } => {
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "status": "not_implemented",
                        "pipeline_id": pipeline_id,
                        "message": "Pipeline execution requires an initialized kernel"
                    })
                );
            } else {
                println!("Pipeline run '{}': Not yet implemented", pipeline_id);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Note handlers — implemented via Kernel facade
// ---------------------------------------------------------------------------

fn handle_note(action: NoteAction, vault: Option<String>) {
    let vault_path = match resolve_vault(vault) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    use std::path::PathBuf;

    let kernel = match feroha_lib::kernel::Kernel::open(&PathBuf::from(&vault_path)) {
        Ok(k) => k,
        Err(e) => {
            eprintln!("Error opening vault '{}': {}", vault_path, e);
            std::process::exit(1);
        }
    };

    match action {
        NoteAction::Create { title } => {
            match kernel.create_note(&title) {
                Ok(meta) => {
                    println!("Created note: {} (path: {})", meta.title, meta.path);
                }
                Err(e) => {
                    eprintln!("Error creating note: {}", e);
                    std::process::exit(1);
                }
            }
        }
        NoteAction::List => {
            match kernel.list_notes() {
                Ok(notes) => {
                    if notes.is_empty() {
                        println!("No notes found in vault.");
                    } else {
                        println!("Notes in vault ({}):", notes.len());
                        for note in &notes {
                            println!(
                                "  {} — {} ({} bytes, tags: {:?})",
                                note.path, note.title, note.size, note.tags
                            );
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error listing notes: {}", e);
                    std::process::exit(1);
                }
            }
        }
        NoteAction::Search { keyword } => {
            match kernel.search_notes(&keyword) {
                Ok(notes) => {
                    if notes.is_empty() {
                        println!("No notes matching '{}' found.", keyword);
                    } else {
                        println!("Notes matching '{}' ({}):", keyword, notes.len());
                        for note in &notes {
                            println!("  {} — {}", note.path, note.title);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error searching notes: {}", e);
                    std::process::exit(1);
                }
            }
        }
        NoteAction::Read { path } => {
            match kernel.read_note(&path) {
                Ok(content) => {
                    print!("{}", content);
                }
                Err(e) => {
                    eprintln!("Error reading note '{}': {}", path, e);
                    std::process::exit(1);
                }
            }
        }
        NoteAction::Delete { path } => {
            match kernel.delete_note(&path) {
                Ok(()) => {
                    println!("Deleted note: {}", path);
                }
                Err(e) => {
                    eprintln!("Error deleting note '{}': {}", path, e);
                    std::process::exit(1);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Graph handlers — implemented via Kernel facade
// ---------------------------------------------------------------------------

fn handle_graph(action: GraphAction, vault: Option<String>) {
    let vault_path = match resolve_vault(vault) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    use std::path::PathBuf;

    let kernel = match feroha_lib::kernel::Kernel::open(&PathBuf::from(&vault_path)) {
        Ok(k) => k,
        Err(e) => {
            eprintln!("Error opening vault '{}': {}", vault_path, e);
            std::process::exit(1);
        }
    };

    match action {
        GraphAction::Export { format } => {
            match kernel.get_graph() {
                Ok(graph_data) => {
                    match format.as_str() {
                        "json" => {
                            match serde_json::to_string_pretty(&graph_data) {
                                Ok(json) => println!("{}", json),
                                Err(e) => eprintln!("Error serializing graph: {}", e),
                            }
                        }
                        _ => {
                            eprintln!(
                                "Unsupported format: '{}'. Use 'json'.",
                                format
                            );
                            std::process::exit(1);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error exporting graph: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }
}
