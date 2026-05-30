use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NetworkPolicy {
    Disabled,
    Allowed,
    AcademicOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SandboxPolicy {
    pub tool_allowlist: Vec<String>,
    pub read_roots: Vec<PathBuf>,
    pub write_roots: Vec<PathBuf>,
    pub network_policy: NetworkPolicy,
    pub max_runtime_secs: u64,
    pub requires_bridge: bool,
}

impl SandboxPolicy {
    pub fn read_only_research() -> Self {
        Self {
            tool_allowlist: strings(&[
                "search",
                "analyze",
                "deep_research",
                "vector_search",
                "fulltext_search",
                "web_search",
                "arxiv_search",
                "semantic_scholar_search",
                "llm_complete",
            ]),
            read_roots: vec![PathBuf::from(".")],
            write_roots: Vec::new(),
            network_policy: NetworkPolicy::Allowed,
            max_runtime_secs: 900,
            requires_bridge: true,
        }
    }

    pub fn dream() -> Self {
        Self {
            tool_allowlist: strings(&["dream_cycle", "graph_index", "bridge_proposal"]),
            read_roots: vec![PathBuf::from(".")],
            write_roots: vec![PathBuf::from(".dualtrack/dream")],
            network_policy: NetworkPolicy::Disabled,
            max_runtime_secs: 600,
            requires_bridge: true,
        }
    }

    pub fn mdt_index() -> Self {
        Self {
            tool_allowlist: strings(&["mdt_validate", "mdt_index"]),
            read_roots: vec![PathBuf::from(".")],
            write_roots: vec![PathBuf::from(".dualtrack/mdt")],
            network_policy: NetworkPolicy::Disabled,
            max_runtime_secs: 300,
            requires_bridge: true,
        }
    }

    pub fn read_only(tools: &[&str]) -> Self {
        Self {
            tool_allowlist: strings(tools),
            read_roots: vec![PathBuf::from(".")],
            write_roots: Vec::new(),
            network_policy: NetworkPolicy::Disabled,
            max_runtime_secs: 300,
            requires_bridge: false,
        }
    }

    pub fn with_write_roots(write_roots: Vec<PathBuf>) -> Self {
        Self {
            tool_allowlist: Vec::new(),
            read_roots: Vec::new(),
            write_roots,
            network_policy: NetworkPolicy::Disabled,
            max_runtime_secs: 300,
            requires_bridge: true,
        }
    }

    pub fn allows_tool(&self, tool_name: &str) -> bool {
        self.tool_allowlist.iter().any(|tool| tool == tool_name)
    }

    pub fn allows_write(&self, path: &Path) -> bool {
        let candidate = normalize_logical_path(path);
        self.write_roots
            .iter()
            .map(|root| normalize_logical_path(root))
            .any(|root| candidate.starts_with(root))
    }

    pub fn summary(&self) -> String {
        let tools = if self.tool_allowlist.is_empty() {
            "none".to_string()
        } else {
            self.tool_allowlist.join(",")
        };
        let writes = if self.write_roots.is_empty() {
            "read-only".to_string()
        } else {
            self.write_roots
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect::<Vec<_>>()
                .join(",")
        };
        format!(
            "tools=[{}]; writes={}; network={:?}; bridge={}",
            tools, writes, self.network_policy, self.requires_bridge
        )
    }
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

fn normalize_logical_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    normalized.push("..");
                }
            }
            Component::Normal(part) => normalized.push(part),
            Component::RootDir | Component::Prefix(_) => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::SandboxPolicy;

    #[test]
    fn test_sandbox_blocks_unlisted_tool() {
        let policy = SandboxPolicy::read_only_research();
        assert!(!policy.allows_tool("write_note"));
        assert!(policy.allows_tool("vector_search"));
    }

    #[test]
    fn test_sandbox_blocks_write_outside_root() {
        let policy =
            SandboxPolicy::with_write_roots(vec![PathBuf::from("vault/.dualtrack/ghosts")]);
        assert!(!policy.allows_write(Path::new("vault/source.md")));
        assert!(policy.allows_write(Path::new("vault/.dualtrack/ghosts/new.md")));
    }

    #[test]
    fn test_sandbox_blocks_parent_directory_escape() {
        let policy =
            SandboxPolicy::with_write_roots(vec![PathBuf::from("vault/.dualtrack/ghosts")]);
        assert!(!policy.allows_write(Path::new("vault/.dualtrack/ghosts/../outside.md")));
    }
}
