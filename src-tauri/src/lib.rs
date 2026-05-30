// Dual-Track Note IDE — Library root
// Submodules are organized by architectural layer

pub mod ai;
pub mod bridge;
pub mod cli;
pub mod diff;
pub mod fs;
pub mod graph;
pub mod harness;
pub mod ipc;
pub mod kernel;
pub mod mdt;
pub mod parser;
pub mod plugin;
pub mod snapshot;

mod state;
pub use state::{AiState, AppConfig, AppState};

// Re-export core types
pub use ai::vectordb::VectorStore;
pub use fs::vault::VaultManager;
pub use graph::link_graph::LinkGraph;
pub use parser::ast::MarkdownAst;
