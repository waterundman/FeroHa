use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct JsonLdProjectIndex {
    pub nodes: Vec<JsonLdNodeIndex>,
    pub edges: Vec<JsonLdEdgeIndex>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct JsonLdNodeIndex {
    pub id: String,
    pub path: String,
    pub title: String,
    #[serde(default)]
    pub jsonld_path: String,
    #[serde(default)]
    pub source_path: String,
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
    pub summary: Option<String>,
    #[serde(default)]
    pub content_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct JsonLdEdgeIndex {
    pub id: String,
    pub source: String,
    pub target: String,
    pub edge_type: String,
    pub origin: String,
    pub confidence: f32,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub via: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct JsonLdValidationReport {
    pub valid: bool,
    pub node_count: usize,
    pub edge_count: usize,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct JsonLdMigrationReport {
    pub format: String,
    pub schema_version: String,
    pub node_count: usize,
    pub edge_count: usize,
    pub generated_paths: JsonLdGeneratedPaths,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct JsonLdGeneratedPaths {
    pub context: String,
    pub shapes: String,
    pub nodes_dir: String,
    pub graph: String,
    pub nodes_index: String,
    pub edges_index: String,
    pub project: String,
    pub migration_report: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsonLdNodeDocument {
    #[serde(rename = "@context")]
    pub context: String,
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "@type")]
    pub node_type: String,
    pub name: String,
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    pub version: u64,
    pub created: String,
    pub updated: String,
    #[serde(rename = "sourcePath")]
    pub source_path: String,
    #[serde(rename = "bodyFormat")]
    pub body_format: String,
    pub body: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keywords: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub area: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub importance: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tree: Option<JsonLdTree>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<JsonLdLinkEdge>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legacy: Option<JsonLdLegacyMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct JsonLdTree {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(default)]
    pub order: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub path: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsonLdLinkEdge {
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "@type")]
    pub edge_type: String,
    pub source: String,
    pub target: String,
    pub relation: String,
    pub predicate: String,
    pub origin: String,
    pub confidence: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct JsonLdLegacyMeta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mdt_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mdt_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_tier: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsonLdGraphDocument {
    #[serde(rename = "@context")]
    pub context: String,
    #[serde(rename = "@graph")]
    pub graph: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum JsonLdReadLevel {
    L0,
    L1,
    L2,
    L3,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct JsonLdContextBundle {
    pub query: String,
    pub remaining_budget: usize,
    pub items: Vec<JsonLdContextItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsonLdContextItem {
    pub node_id: String,
    pub level: JsonLdReadLevel,
    pub reason: String,
    pub content: String,
}
