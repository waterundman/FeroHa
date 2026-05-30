use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

use crate::ai::llm_router::LlmRouter;
use crate::ai::sandbox::{NetworkPolicy, SandboxPolicy};
use crate::ai::subagent::Subagent;
use crate::diff::ghost_store::{GhostBlock, GhostOp};
use crate::AiState;
use crate::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool_name: String,
    pub content: String,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub tool: String,
    pub params: serde_json::Value,
}

#[derive(Serialize)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[async_trait::async_trait(?Send)]
pub trait AgentTool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameter_schema(&self) -> serde_json::Value;
    async fn execute(
        &self,
        params: serde_json::Value,
        state: &Mutex<AppState>,
        ai_state: &Mutex<AiState>,
        router: &LlmRouter,
    ) -> Result<ToolResult, String>;
}

pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn AgentTool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        ToolRegistry {
            tools: HashMap::new(),
        }
    }

    pub fn register(&mut self, tool: Box<dyn AgentTool>) {
        let name = tool.name().to_string();
        self.tools.insert(name, tool);
    }

    pub fn get(&self, name: &str) -> Option<&dyn AgentTool> {
        self.tools.get(name).map(|t| t.as_ref())
    }

    pub fn get_allowed(
        &self,
        name: &str,
        policy: &SandboxPolicy,
    ) -> Result<&dyn AgentTool, String> {
        let tool = self
            .get(name)
            .ok_or_else(|| format!("Unknown tool: {}", name))?;
        if self.is_tool_allowed(name, policy) {
            Ok(tool)
        } else {
            Err(format!("Tool blocked by sandbox policy: {}", name))
        }
    }

    pub fn is_tool_allowed(&self, name: &str, policy: &SandboxPolicy) -> bool {
        let tool_allowed = match name {
            "search" => {
                policy.allows_tool("search")
                    || policy.allows_tool("vector_search")
                    || policy.allows_tool("fulltext_search")
            }
            _ => policy.allows_tool(name),
        };
        tool_allowed && tool_allowed_by_network_policy(name, &policy.network_policy)
    }

    pub fn build_tool_prompt(&self) -> String {
        let tool_jsons = self
            .tools
            .values()
            .map(|tool| tool_prompt_json(tool.as_ref()))
            .collect::<Vec<_>>();
        self.render_tool_prompt(tool_jsons)
    }

    pub fn build_tool_prompt_for_policy(&self, policy: &SandboxPolicy) -> String {
        let tool_jsons = self
            .tools
            .values()
            .filter(|tool| self.is_tool_allowed(tool.name(), policy))
            .map(|tool| tool_prompt_json(tool.as_ref()))
            .collect::<Vec<_>>();
        self.render_tool_prompt(tool_jsons)
    }

    fn render_tool_prompt(&self, tool_jsons: Vec<String>) -> String {
        let tool_descriptions = tool_jsons.join("\n");
        format!(
            "## Available Tools

When you need to use a tool, respond with a JSON object on its own line:
```json
{{\"tool\":\"<tool_name>\",\"params\":{{...}}}}
```

Available tools:
{tool_descriptions}

When you have a final answer without needing more tools, respond with normal text."
        )
    }

    pub fn list_tools(&self) -> Vec<ToolInfo> {
        self.tools
            .values()
            .map(|t| ToolInfo {
                name: t.name().to_string(),
                description: t.description().to_string(),
                parameters: t.parameter_schema(),
            })
            .collect()
    }
}

fn tool_prompt_json(tool: &dyn AgentTool) -> String {
    let info = serde_json::json!({
        "name": tool.name(),
        "description": tool.description(),
        "parameters": tool.parameter_schema(),
    });
    serde_json::to_string_pretty(&info).unwrap_or_default()
}

fn tool_allowed_by_network_policy(tool_name: &str, network_policy: &NetworkPolicy) -> bool {
    match tool_name {
        "deep_research" => matches!(network_policy, NetworkPolicy::Allowed),
        _ => true,
    }
}

pub fn parse_tool_call(text: &str) -> Option<ToolCall> {
    // Find the first occurrence of {"tool"
    let start_marker = r#"{"tool""#;
    let start = text.find(start_marker)?;

    let slice = &text[start..];

    // Track brace nesting to find the matching closing brace
    let mut depth = 0;
    let mut end = 0;
    let mut in_string = false;
    let mut escape_next = false;

    for (i, ch) in slice.char_indices() {
        if escape_next {
            escape_next = false;
            continue;
        }
        if ch == '\\' {
            escape_next = true;
            continue;
        }
        if ch == '"' {
            in_string = !in_string;
            continue;
        }
        if in_string {
            continue;
        }
        if ch == '{' {
            depth += 1;
        } else if ch == '}' {
            depth -= 1;
            if depth == 0 {
                end = i + 1;
                break;
            }
        }
    }

    if end == 0 {
        return None;
    }

    let json_str = &slice[..end];

    // First try parsing as ToolCall directly
    if let Ok(tool_call) = serde_json::from_str::<ToolCall>(json_str) {
        return Some(tool_call);
    }

    // Fallback: try to extract tool and params manually
    let parsed: serde_json::Value = serde_json::from_str(json_str).ok()?;
    let tool = parsed.get("tool")?.as_str()?.to_string();
    let params = parsed
        .get("params")
        .cloned()
        .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
    Some(ToolCall { tool, params })
}

// ── SearchTool ──

pub struct SearchTool;

#[async_trait::async_trait(?Send)]
impl AgentTool for SearchTool {
    fn name(&self) -> &str {
        "search"
    }

    fn description(&self) -> &str {
        "Search the knowledge base for relevant notes and information."
    }

    fn parameter_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query"
                },
                "top_k": {
                    "type": "integer",
                    "default": 5,
                    "description": "Number of results to return"
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(
        &self,
        params: serde_json::Value,
        state: &Mutex<AppState>,
        _ai_state: &Mutex<AiState>,
        _router: &LlmRouter,
    ) -> Result<ToolResult, String> {
        let query = params["query"].as_str().unwrap_or("").to_string();
        let top_k = params["top_k"].as_u64().unwrap_or(5) as usize;

        let app = state.lock().map_err(|e| e.to_string())?;
        let entries: Vec<String> = if let Some(ref sync_engine) = app.sync_engine {
            let store = sync_engine.store();
            store
                .search_text(&query, top_k)
                .into_iter()
                .map(|r| {
                    format!(
                        "### {}\nSource: {}\n{}\nRelevance: {:.2}",
                        if r.heading_context.is_empty() {
                            &r.source_file
                        } else {
                            &r.heading_context
                        },
                        r.source_file,
                        r.chunk_text.chars().take(300).collect::<String>(),
                        r.score
                    )
                })
                .collect()
        } else {
            vec![]
        };
        drop(app);

        if entries.is_empty() {
            Ok(ToolResult {
                tool_name: "search".to_string(),
                content: format!("No results found for query: \"{}\"", query),
                metadata: serde_json::json!({"query": query, "results_count": 0}),
            })
        } else {
            let count = entries.len();
            Ok(ToolResult {
                tool_name: "search".to_string(),
                content: entries.join("\n\n"),
                metadata: serde_json::json!({"query": query, "results_count": count}),
            })
        }
    }
}

// ── SummarizeTool ──

pub struct SummarizeTool;

#[async_trait::async_trait(?Send)]
impl AgentTool for SummarizeTool {
    fn name(&self) -> &str {
        "summarize"
    }

    fn description(&self) -> &str {
        "Summarize a note or topic from the knowledge base."
    }

    fn parameter_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "target": {
                    "type": "string",
                    "description": "Note path or topic to summarize"
                }
            },
            "required": ["target"]
        })
    }

    async fn execute(
        &self,
        params: serde_json::Value,
        state: &Mutex<AppState>,
        _ai_state: &Mutex<AiState>,
        router: &LlmRouter,
    ) -> Result<ToolResult, String> {
        let target = params["target"].as_str().unwrap_or("").to_string();

        let content = {
            let app = state.lock().map_err(|e| e.to_string())?;
            let vault = app.vault.as_ref().ok_or("No vault open")?;
            let content = vault.read_note(&target).unwrap_or_default();
            if content.is_empty() {
                // Try searching for the topic in vector store
                if let Some(ref sync_engine) = app.sync_engine {
                    let store = sync_engine.store();
                    let results = store.search_text(&target, 3);
                    if !results.is_empty() {
                        results
                            .into_iter()
                            .map(|r| format!("[{}] {}", r.source_file, r.chunk_text))
                            .collect::<Vec<_>>()
                            .join("\n\n")
                    } else {
                        return Ok(ToolResult {
                            tool_name: "summarize".to_string(),
                            content: format!("No content found for: \"{}\"", target),
                            metadata: serde_json::json!({"target": target, "found": false}),
                        });
                    }
                } else {
                    return Ok(ToolResult {
                        tool_name: "summarize".to_string(),
                        content: format!("No content found for: \"{}\"", target),
                        metadata: serde_json::json!({"target": target, "found": false}),
                    });
                }
            } else {
                content
            }
        };

        let mut router = router.clone();
        let system =
            "Summarize the following content in Markdown. Include key points, main arguments, and a TL;DR.";
        match router.complete(system, &content, None).await {
            Ok(response) => Ok(ToolResult {
                tool_name: "summarize".to_string(),
                content: response.text,
                metadata: serde_json::json!({"target": target, "tokens": response.tokens_in + response.tokens_out}),
            }),
            Err(e) => Ok(ToolResult {
                tool_name: "summarize".to_string(),
                content: format!("Summarization unavailable: {}", e),
                metadata: serde_json::json!({"target": target, "error": e}),
            }),
        }
    }
}

// ── AnalyzeTool ──

pub struct AnalyzeTool;

#[async_trait::async_trait(?Send)]
impl AgentTool for AnalyzeTool {
    fn name(&self) -> &str {
        "analyze"
    }

    fn description(&self) -> &str {
        "Analyze a concept or topic in depth using local notes and knowledge."
    }

    fn parameter_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "concept": {
                    "type": "string",
                    "description": "Concept or topic to analyze"
                },
                "depth": {
                    "type": "string",
                    "enum": ["basic", "deep"],
                    "default": "basic",
                    "description": "Analysis depth"
                }
            },
            "required": ["concept"]
        })
    }

    async fn execute(
        &self,
        params: serde_json::Value,
        state: &Mutex<AppState>,
        _ai_state: &Mutex<AiState>,
        router: &LlmRouter,
    ) -> Result<ToolResult, String> {
        let concept = params["concept"].as_str().unwrap_or("").to_string();
        let depth = params["depth"].as_str().unwrap_or("basic").to_string();

        let contexts: Vec<String> = {
            let app = state.lock().map_err(|e| e.to_string())?;
            if let Some(ref sync_engine) = app.sync_engine {
                let store = sync_engine.store();
                let top_k = if depth == "deep" { 10 } else { 5 };
                store
                    .search_text(&concept, top_k)
                    .into_iter()
                    .map(|r| {
                        format!(
                            "[{}] {}",
                            r.source_file,
                            r.chunk_text.chars().take(500).collect::<String>()
                        )
                    })
                    .collect()
            } else {
                vec![]
            }
        };

        let system = if depth == "deep" {
            "You are an expert analyst. Provide a comprehensive analysis of the concept using the provided context. Include: definitions, key relationships, implications, and connections to broader knowledge. Use Markdown with headings."
        } else {
            "You are a knowledge assistant. Analyze the concept based on the provided context. Include key points and a concise summary. Use Markdown."
        };

        let prompt = if contexts.is_empty() {
            format!("Analyze: {}\n\n(No local notes found. Provide a general analysis based on your knowledge.)", concept)
        } else {
            format!(
                "Analyze: {}\n\n## Context from user's notes:\n{}\n\nProvide analysis that connects these notes with broader knowledge.",
                concept,
                contexts.join("\n\n")
            )
        };

        let mut router = router.clone();
        match router.complete(system, &prompt, None).await {
            Ok(response) => Ok(ToolResult {
                tool_name: "analyze".to_string(),
                content: response.text,
                metadata: serde_json::json!({
                    "concept": concept,
                    "depth": depth,
                    "context_chunks": contexts.len(),
                    "tokens": response.tokens_in + response.tokens_out
                }),
            }),
            Err(e) => Ok(ToolResult {
                tool_name: "analyze".to_string(),
                content: format!("Analysis unavailable: {}", e),
                metadata: serde_json::json!({"concept": concept, "error": e}),
            }),
        }
    }
}

// ── DeepResearchTool ──

pub struct DeepResearchTool;

#[async_trait::async_trait(?Send)]
impl AgentTool for DeepResearchTool {
    fn name(&self) -> &str {
        "deep_research"
    }

    fn description(&self) -> &str {
        "Conduct multi-source deep research on a question, searching local notes and external sources."
    }

    fn parameter_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "question": {
                    "type": "string",
                    "description": "Research question"
                }
            },
            "required": ["question"]
        })
    }

    async fn execute(
        &self,
        params: serde_json::Value,
        state: &Mutex<AppState>,
        ai_state: &Mutex<AiState>,
        router: &LlmRouter,
    ) -> Result<ToolResult, String> {
        let question = params["question"].as_str().unwrap_or("").to_string();

        let (sub, dualtrack_dir) = {
            let ai = ai_state.lock().map_err(|e| e.to_string())?;
            let sub = ai.subagent.as_ref().cloned();
            let app = state.lock().map_err(|e| e.to_string())?;
            let dualtrack_dir = app.dualtrack_dir.clone();
            drop(app);
            (sub, dualtrack_dir)
        };

        let sub = sub.unwrap_or_else(|| Subagent::new(None));
        let task_id = format!(
            "tool_deep_research_{}",
            uuid::Uuid::new_v4().to_string().replace('-', "")
        );

        match sub
            .execute_deep_research(
                &question,
                None,
                Some(router.clone()),
                &task_id,
                &dualtrack_dir,
                None,
            )
            .await
        {
            Ok((report, _ghost_ids)) => Ok(ToolResult {
                tool_name: "deep_research".to_string(),
                content: report,
                metadata: serde_json::json!({"question": question, "task_id": task_id}),
            }),
            Err(e) => Ok(ToolResult {
                tool_name: "deep_research".to_string(),
                content: format!("Deep research failed: {}", e),
                metadata: serde_json::json!({"question": question, "error": e}),
            }),
        }
    }
}

// ── GhostWriteTool ──

pub struct GhostWriteTool;

#[async_trait::async_trait(?Send)]
impl AgentTool for GhostWriteTool {
    fn name(&self) -> &str {
        "ghost_write"
    }

    fn description(&self) -> &str {
        "Write AI-generated content as a ghost note suggestion. The user must approve before it becomes a real note."
    }

    fn parameter_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "description": "Title for the ghost note"
                },
                "content": {
                    "type": "string",
                    "description": "Content to write as a ghost suggestion"
                },
                "target_note": {
                    "type": "string",
                    "description": "Related note path for context"
                }
            },
            "required": ["title", "content"]
        })
    }

    async fn execute(
        &self,
        params: serde_json::Value,
        _state: &Mutex<AppState>,
        ai_state: &Mutex<AiState>,
        _router: &LlmRouter,
    ) -> Result<ToolResult, String> {
        let title = params["title"].as_str().unwrap_or("ghost_note").to_string();
        let content = params["content"].as_str().unwrap_or("").to_string();
        let target_note = params["target_note"]
            .as_str()
            .unwrap_or("untitled.md")
            .to_string();

        let blocks: Vec<GhostBlock> = content
            .split("\n\n")
            .filter(|p| !p.trim().is_empty())
            .enumerate()
            .map(|(i, para)| GhostBlock {
                block_id: format!("ghost-block-{}", i),
                content: para.to_string(),
                operation: GhostOp::Suggestion,
                after_block_id: None,
                heading_context: title.clone(),
                context: vec![],
                verified: None,
                verification_result: None,
            })
            .collect();

        let ai = ai_state.lock().map_err(|e| e.to_string())?;
        let task_id = format!(
            "ghost_{}",
            uuid::Uuid::new_v4().to_string().replace('-', "")
        );
        match ai.ghost_store.create(
            &target_note,
            &title,
            blocks,
            Some(task_id.clone()),
        ) {
            Ok(ghost_note) => Ok(ToolResult {
                tool_name: "ghost_write".to_string(),
                content: format!(
                    "Ghost note created: {}\nID: {}\nBlocks: {}\n\nPlease review and approve before it becomes a real note.",
                    title,
                    ghost_note.id,
                    ghost_note.suggested_blocks.len()
                ),
                metadata: serde_json::json!({
                    "ghost_id": ghost_note.id,
                    "task_id": task_id,
                    "target_note": target_note,
                    "blocks_count": ghost_note.suggested_blocks.len()
                }),
            }),
            Err(e) => Ok(ToolResult {
                tool_name: "ghost_write".to_string(),
                content: format!("Failed to create ghost note: {}", e),
                metadata: serde_json::json!({"target_note": target_note, "error": e}),
            }),
        }
    }
}

pub fn create_default_registry() -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(SearchTool));
    registry.register(Box::new(SummarizeTool));
    registry.register(Box::new(AnalyzeTool));
    registry.register(Box::new(DeepResearchTool));
    registry.register(Box::new(GhostWriteTool));
    registry
}

#[cfg(test)]
mod sandbox_tests {
    use super::create_default_registry;
    use crate::ai::sandbox::SandboxPolicy;

    #[test]
    fn registry_hides_tools_blocked_by_sandbox_policy() {
        let registry = create_default_registry();
        let policy = SandboxPolicy::read_only(&["vector_search"]);

        let prompt = registry.build_tool_prompt_for_policy(&policy);

        assert!(prompt.contains("\"name\": \"search\""));
        assert!(!prompt.contains("\"name\": \"ghost_write\""));
        assert!(registry.get_allowed("search", &policy).is_ok());
        assert!(registry.get_allowed("ghost_write", &policy).is_err());
    }

    #[test]
    fn registry_blocks_network_tools_when_network_is_disabled() {
        let registry = create_default_registry();
        let policy = SandboxPolicy::read_only(&["deep_research"]);

        assert!(registry.get_allowed("deep_research", &policy).is_err());
    }
}
