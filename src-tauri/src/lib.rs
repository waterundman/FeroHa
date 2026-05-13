// Dual-Track Note IDE — Library root
// Submodules are organized by architectural layer

pub mod cli;
pub mod diff;
pub mod fs;
pub mod graph;
pub mod parser;
pub mod plugin;
pub mod ai;
pub mod ipc;

// Re-export core types
pub use fs::vault::VaultManager;
pub use parser::ast::MarkdownAst;
pub use graph::link_graph::LinkGraph;
pub use ai::vectordb::VectorStore;
