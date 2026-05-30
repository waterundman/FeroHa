use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct MdtMeta {
    #[serde(default)]
    pub mdt_version: Option<String>,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub slug: Option<String>,
    #[serde(default)]
    pub tree: Option<MdtTree>,
    #[serde(default)]
    pub area: Option<String>,
    #[serde(default)]
    pub importance: Option<u8>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub links: Vec<MdtLink>,
    #[serde(default)]
    pub storage: Option<MdtStorage>,
    #[serde(default)]
    pub content_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct MdtTree {
    #[serde(default)]
    pub parent: Option<String>,
    #[serde(default)]
    pub order: i64,
    #[serde(default)]
    pub path: Vec<String>,
    #[serde(default)]
    pub depth: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MdtLink {
    pub target: String,
    #[serde(rename = "type")]
    pub edge_type: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub confidence: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct MdtStorage {
    #[serde(default)]
    pub tier: Option<String>,
    #[serde(default)]
    pub pinned: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct MdtProjectIndex {
    pub nodes: Vec<MdtNodeIndex>,
    pub edges: Vec<MdtEdgeIndex>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct MdtNodeIndex {
    pub id: String,
    pub path: String,
    pub title: String,
    #[serde(default)]
    pub slug: Option<String>,
    #[serde(default)]
    pub parent: Option<String>,
    #[serde(default)]
    pub order: i64,
    #[serde(default)]
    pub depth: Option<u32>,
    #[serde(default)]
    pub area: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub importance: Option<u8>,
    #[serde(default)]
    pub storage_tier: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub content_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct MdtEdgeIndex {
    pub source: String,
    pub target: String,
    pub edge_type: String,
    pub origin: String,
    pub confidence: f32,
    #[serde(default)]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MdtReadLevel {
    L0,
    L1,
    L2,
    L3,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct MdtContextBundle {
    pub query: String,
    pub remaining_budget: usize,
    pub items: Vec<MdtContextItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MdtContextItem {
    pub node_id: String,
    pub level: MdtReadLevel,
    pub reason: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct MdtValidationReport {
    pub valid: bool,
    pub node_count: usize,
    pub edge_count: usize,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}
