use serde::{Deserialize, Serialize};

use crate::ai::sandbox::SandboxPolicy;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskIntentType {
    Research,
    Summarize,
    Verify,
    Dream,
    JsonLdIndex,
    JsonLdRead,
    MdtIndex,
    MdtRead,
    MdtPack,
    WriteProposal,
    ExternalImport,
    CodeAssist,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BridgeRisk {
    Low,
    Medium,
    High,
}

impl TaskIntentType {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "research" | "deep_research" | "deepresearch" | "fetch_papers" => Some(Self::Research),
            "summarize" | "summary" => Some(Self::Summarize),
            "verify" | "verification" | "review" => Some(Self::Verify),
            "dream" | "dream_cycle" => Some(Self::Dream),
            "jsonld_index" | "json_ld_index" | "memory_index" | "build_graph" => {
                Some(Self::JsonLdIndex)
            }
            "jsonld_read" | "json_ld_read" | "memory_read" => Some(Self::JsonLdRead),
            "mdt_index" | "index_mdt" => Some(Self::JsonLdIndex),
            "mdt_read" | "read_mdt" => Some(Self::JsonLdRead),
            "mdt_pack" | "pack_mdt" => Some(Self::MdtPack),
            "write_proposal" | "ghost_write" | "rewrite" | "correct" | "expand" | "translate"
            | "simplify" | "format" | "extract" => Some(Self::WriteProposal),
            "external_import" | "import" => Some(Self::ExternalImport),
            "code_assist" | "code" => Some(Self::CodeAssist),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Research => "research",
            Self::Summarize => "summarize",
            Self::Verify => "verify",
            Self::Dream => "dream",
            Self::JsonLdIndex => "jsonld_index",
            Self::JsonLdRead => "jsonld_read",
            Self::MdtIndex => "mdt_index",
            Self::MdtRead => "mdt_read",
            Self::MdtPack => "mdt_pack",
            Self::WriteProposal => "write_proposal",
            Self::ExternalImport => "external_import",
            Self::CodeAssist => "code_assist",
        }
    }

    pub fn default_sandbox_policy(self) -> SandboxPolicy {
        match self {
            TaskIntentType::Research => SandboxPolicy::read_only_research(),
            TaskIntentType::Summarize => SandboxPolicy::read_only(&[
                "summarize",
                "search",
                "vector_search",
                "fulltext_search",
                "read_note",
                "llm_complete",
            ]),
            TaskIntentType::Verify => SandboxPolicy::read_only(&[
                "search",
                "read_note",
                "vector_search",
                "proposition_kernel",
            ]),
            TaskIntentType::Dream => SandboxPolicy::dream(),
            TaskIntentType::JsonLdIndex => SandboxPolicy::jsonld_index(),
            TaskIntentType::JsonLdRead => {
                SandboxPolicy::read_only(&["jsonld_read", "jsonld_validate"])
            }
            TaskIntentType::MdtIndex => SandboxPolicy::mdt_index(),
            TaskIntentType::MdtRead => SandboxPolicy::read_only(&["mdt_read", "mdt_validate"]),
            TaskIntentType::MdtPack => {
                let mut policy = SandboxPolicy::mdt_index();
                policy.tool_allowlist.push("mdt_pack".to_string());
                policy
                    .write_roots
                    .push(std::path::PathBuf::from(".dualtrack/mdt/snapshots"));
                policy
            }
            TaskIntentType::WriteProposal => SandboxPolicy {
                tool_allowlist: vec!["llm_complete".into(), "ghost_write".into()],
                read_roots: vec![std::path::PathBuf::from(".")],
                write_roots: vec![std::path::PathBuf::from(".dualtrack/ghosts")],
                network_policy: crate::ai::sandbox::NetworkPolicy::Disabled,
                max_runtime_secs: 300,
                requires_bridge: true,
            },
            TaskIntentType::ExternalImport => SandboxPolicy {
                tool_allowlist: vec![
                    "web_search".into(),
                    "fetch_url".into(),
                    "bridge_proposal".into(),
                ],
                read_roots: vec![std::path::PathBuf::from(".")],
                write_roots: vec![std::path::PathBuf::from(".dualtrack/imports")],
                network_policy: crate::ai::sandbox::NetworkPolicy::Allowed,
                max_runtime_secs: 900,
                requires_bridge: true,
            },
            TaskIntentType::CodeAssist => {
                SandboxPolicy::read_only(&["read_file", "output_hook", "bridge_proposal"])
            }
        }
    }

    pub fn default_bridge_risk(self) -> BridgeRisk {
        match self {
            TaskIntentType::Summarize
            | TaskIntentType::Verify
            | TaskIntentType::JsonLdRead
            | TaskIntentType::MdtRead => BridgeRisk::Low,
            TaskIntentType::Research
            | TaskIntentType::Dream
            | TaskIntentType::JsonLdIndex
            | TaskIntentType::MdtIndex
            | TaskIntentType::MdtPack
            | TaskIntentType::WriteProposal => BridgeRisk::Medium,
            TaskIntentType::ExternalImport | TaskIntentType::CodeAssist => BridgeRisk::High,
        }
    }

    pub fn expected_output(self) -> &'static str {
        match self {
            Self::Research => "research brief with sources",
            Self::Summarize => "summary proposal",
            Self::Verify => "verification report",
            Self::Dream => "dream insight proposal",
            Self::JsonLdIndex => "generated JSON-LD graph artifacts",
            Self::JsonLdRead => "JSON-LD context bundle",
            Self::MdtIndex => "generated MDT index files",
            Self::MdtRead => "MDT context bundle",
            Self::MdtPack => "MDT archive package",
            Self::WriteProposal => "ghost write proposal",
            Self::ExternalImport => "import review proposal",
            Self::CodeAssist => "code assistance report",
        }
    }

    pub fn risk_reason(self) -> &'static str {
        match self {
            Self::Research => "uses research tools and may use network sources",
            Self::Summarize => "read-only local note summarization",
            Self::Verify => "read-only consistency verification",
            Self::Dream => "updates Dream-derived memory artifacts through review",
            Self::JsonLdIndex => "writes generated JSON-LD graph artifacts",
            Self::JsonLdRead => "read-only JSON-LD context expansion",
            Self::MdtIndex => "writes generated MDT index artifacts",
            Self::MdtRead => "read-only MDT context expansion",
            Self::MdtPack => "creates archive artifacts",
            Self::WriteProposal => "creates ghost write output requiring review",
            Self::ExternalImport => "uses external network sources and import artifacts",
            Self::CodeAssist => "reads code context and may propose high-impact changes",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{BridgeRisk, TaskIntentType};

    #[test]
    fn test_task_intent_defaults_drive_sandbox_and_risk() {
        let research = TaskIntentType::Research;
        let dream = TaskIntentType::Dream;
        let jsonld_index = TaskIntentType::JsonLdIndex;
        let mdt_index = TaskIntentType::MdtIndex;

        assert!(research.default_sandbox_policy().allows_tool("web_search"));
        assert_eq!(research.default_bridge_risk(), BridgeRisk::Medium);

        assert!(dream.default_sandbox_policy().allows_tool("dream_cycle"));
        assert_eq!(dream.default_bridge_risk(), BridgeRisk::Medium);

        assert!(mdt_index.default_sandbox_policy().allows_tool("mdt_index"));
        assert!(mdt_index
            .default_sandbox_policy()
            .allows_write(std::path::Path::new(".dualtrack/mdt/indexes/nodes.json")));
        assert!(mdt_index
            .default_sandbox_policy()
            .allows_write(std::path::Path::new(
                ".dualtrack/jsonld/indexes/graph.jsonld"
            )));
        assert!(jsonld_index
            .default_sandbox_policy()
            .allows_tool("jsonld_index"));
        assert!(jsonld_index
            .default_sandbox_policy()
            .allows_write(std::path::Path::new(
                ".dualtrack/jsonld/indexes/graph.jsonld"
            )));
        assert!(!jsonld_index
            .default_sandbox_policy()
            .allows_write(std::path::Path::new(".dualtrack/mdt/indexes/nodes.json")));
    }

    #[test]
    fn write_card_aliases_map_to_reviewable_write_proposals() {
        assert_eq!(
            TaskIntentType::parse("rewrite"),
            Some(TaskIntentType::WriteProposal)
        );
        assert_eq!(
            TaskIntentType::parse("correct"),
            Some(TaskIntentType::WriteProposal)
        );
    }

    #[test]
    fn legacy_mdt_aliases_route_to_jsonld_memory_intents() {
        assert_eq!(
            TaskIntentType::parse("mdt_index"),
            Some(TaskIntentType::JsonLdIndex)
        );
        assert_eq!(
            TaskIntentType::parse("index_mdt"),
            Some(TaskIntentType::JsonLdIndex)
        );
        assert_eq!(
            TaskIntentType::parse("mdt_read"),
            Some(TaskIntentType::JsonLdRead)
        );
        assert_eq!(
            TaskIntentType::parse("read_mdt"),
            Some(TaskIntentType::JsonLdRead)
        );
    }
}
