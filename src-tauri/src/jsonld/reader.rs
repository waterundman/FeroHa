use std::fs;
use std::path::Path;

use crate::jsonld::indexer::index_vault;
use crate::jsonld::types::{
    JsonLdContextBundle, JsonLdContextItem, JsonLdNodeIndex, JsonLdReadLevel,
};
use crate::mdt::types::{MdtContextBundle, MdtContextItem, MdtReadLevel};
use crate::parser::frontmatter::parse_frontmatter;

pub struct JsonLdReader;

impl JsonLdReader {
    pub fn load_context(
        project_root: &Path,
        query: &str,
        token_budget: usize,
    ) -> Result<JsonLdContextBundle, String> {
        let index = index_vault(project_root)?;
        let query_norm = query.to_lowercase();
        let mut scored: Vec<(i32, JsonLdNodeIndex)> = index
            .nodes
            .into_iter()
            .map(|node| (score_node(&node, &query_norm), node))
            .filter(|(score, _)| *score > 0)
            .collect();
        scored.sort_by(|(a, left), (b, right)| {
            b.cmp(a)
                .then_with(|| right.importance.cmp(&left.importance))
                .then_with(|| left.title.cmp(&right.title))
        });

        let mut remaining_budget = token_budget;
        let mut items = Vec::new();

        for (score, node) in scored {
            let level = choose_level(&node, score, &query_norm);
            let content = load_content(project_root, &node, &level)?;
            let cost = estimate_token_cost(&content);
            if cost > remaining_budget && level != JsonLdReadLevel::L0 {
                let downgraded = JsonLdReadLevel::L0;
                let downgraded_content = load_content(project_root, &node, &downgraded)?;
                let downgraded_cost = estimate_token_cost(&downgraded_content);
                if downgraded_cost > remaining_budget {
                    continue;
                }
                remaining_budget -= downgraded_cost;
                items.push(JsonLdContextItem {
                    node_id: node.id,
                    level: downgraded,
                    reason: format!("downgraded JSON-LD frame to fit budget after score {score}"),
                    content: downgraded_content,
                });
            } else {
                remaining_budget = remaining_budget.saturating_sub(cost);
                items.push(JsonLdContextItem {
                    node_id: node.id,
                    level,
                    reason: format!("matched JSON-LD index with score {score}"),
                    content,
                });
            }

            if remaining_budget == 0 {
                break;
            }
        }

        Ok(JsonLdContextBundle {
            query: query.to_string(),
            remaining_budget,
            items,
        })
    }
}

pub fn to_legacy_mdt_bundle(bundle: JsonLdContextBundle) -> MdtContextBundle {
    MdtContextBundle {
        query: bundle.query,
        remaining_budget: bundle.remaining_budget,
        items: bundle
            .items
            .into_iter()
            .map(|item| MdtContextItem {
                node_id: item.node_id,
                level: match item.level {
                    JsonLdReadLevel::L0 => MdtReadLevel::L0,
                    JsonLdReadLevel::L1 => MdtReadLevel::L1,
                    JsonLdReadLevel::L2 => MdtReadLevel::L2,
                    JsonLdReadLevel::L3 => MdtReadLevel::L3,
                },
                reason: item.reason,
                content: item.content,
            })
            .collect(),
    }
}

fn score_node(node: &JsonLdNodeIndex, query: &str) -> i32 {
    if query.trim().is_empty() {
        return node.importance.unwrap_or(1) as i32;
    }
    let mut score = 0;
    let title = node.title.to_lowercase();
    if title.contains(query) {
        score += 100;
    }
    if node
        .summary
        .as_ref()
        .map(|summary| summary.to_lowercase().contains(query))
        .unwrap_or(false)
    {
        score += 40;
    }
    if node
        .area
        .as_ref()
        .map(|area| area.to_lowercase().contains(query))
        .unwrap_or(false)
    {
        score += 20;
    }
    if node
        .tags
        .iter()
        .any(|tag| tag.to_lowercase().contains(query))
    {
        score += 20;
    }
    if node.source_path.to_lowercase().contains(query) {
        score += 10;
    }
    score + node.importance.unwrap_or(1) as i32
}

fn choose_level(node: &JsonLdNodeIndex, score: i32, query: &str) -> JsonLdReadLevel {
    if node.title.to_lowercase().contains(query) && node.importance.unwrap_or(1) >= 4 {
        JsonLdReadLevel::L3
    } else if score >= 80 {
        JsonLdReadLevel::L2
    } else if score >= 40 {
        JsonLdReadLevel::L1
    } else {
        JsonLdReadLevel::L0
    }
}

fn load_content(
    project_root: &Path,
    node: &JsonLdNodeIndex,
    level: &JsonLdReadLevel,
) -> Result<String, String> {
    match level {
        JsonLdReadLevel::L0 => Ok(format!(
            "{}\njsonld_id={}\nsource={}\ntags={}",
            node.title,
            node.id,
            node.source_path,
            node.tags.join(",")
        )),
        JsonLdReadLevel::L1 => Ok(format!(
            "{}\nsummary={}\nsource={}\narea={}\ntags={}",
            node.title,
            node.summary.as_deref().unwrap_or(""),
            node.source_path,
            node.area.as_deref().unwrap_or(""),
            node.tags.join(",")
        )),
        JsonLdReadLevel::L2 | JsonLdReadLevel::L3 => {
            let full_path = project_root.join(&node.source_path);
            let content = fs::read_to_string(&full_path)
                .map_err(|err| format!("failed to read {}: {err}", full_path.display()))?;
            if let Some((_, body_offset)) = parse_frontmatter(&content) {
                Ok(content[body_offset..].to_string())
            } else {
                Ok(content)
            }
        }
    }
}

fn estimate_token_cost(content: &str) -> usize {
    content.split_whitespace().count().max(1)
}

#[cfg(test)]
mod tests {
    #[test]
    fn jsonld_reader_loads_context_from_migrated_markdown() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("reader.md"),
            "---\nid: reader\ntitle: JSON-LD Reader\nimportance: 4\nsummary: Reader expands JSON-LD context.\ntags: jsonld\n---\n# JSON-LD Reader\n\nFull body.\n",
        )
        .unwrap();

        let bundle =
            crate::jsonld::reader::JsonLdReader::load_context(temp.path(), "reader", 1000).unwrap();

        assert_eq!(bundle.items.len(), 1);
        assert_eq!(
            bundle.items[0].level,
            crate::jsonld::types::JsonLdReadLevel::L3
        );
        assert!(bundle.items[0].content.contains("Full body"));
    }
}
