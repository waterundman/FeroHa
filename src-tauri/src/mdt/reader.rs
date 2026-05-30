use std::fs;
use std::path::Path;

use crate::mdt::indexer::index_vault;
use crate::mdt::types::{MdtContextBundle, MdtContextItem, MdtNodeIndex, MdtReadLevel};
use crate::parser::frontmatter::parse_frontmatter;

pub struct MdtReader;

impl MdtReader {
    pub fn load_context(
        project_root: &Path,
        query: &str,
        token_budget: usize,
    ) -> Result<MdtContextBundle, String> {
        let index = index_vault(project_root)?;
        let query_norm = query.to_lowercase();
        let mut scored: Vec<(i32, MdtNodeIndex)> = index
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
            if cost > remaining_budget && level != MdtReadLevel::L0 {
                let downgraded = MdtReadLevel::L0;
                let downgraded_content = load_content(project_root, &node, &downgraded)?;
                let downgraded_cost = estimate_token_cost(&downgraded_content);
                if downgraded_cost > remaining_budget {
                    continue;
                }
                remaining_budget -= downgraded_cost;
                items.push(MdtContextItem {
                    node_id: node.id,
                    level: downgraded,
                    reason: format!("downgraded to fit budget after score {score}"),
                    content: downgraded_content,
                });
            } else {
                remaining_budget = remaining_budget.saturating_sub(cost);
                items.push(MdtContextItem {
                    node_id: node.id,
                    level,
                    reason: format!("matched query with score {score}"),
                    content,
                });
            }

            if remaining_budget == 0 {
                break;
            }
        }

        Ok(MdtContextBundle {
            query: query.to_string(),
            remaining_budget,
            items,
        })
    }
}

fn score_node(node: &MdtNodeIndex, query: &str) -> i32 {
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
    score + node.importance.unwrap_or(1) as i32
}

fn choose_level(node: &MdtNodeIndex, score: i32, query: &str) -> MdtReadLevel {
    if node.title.to_lowercase().contains(query) && node.importance.unwrap_or(1) >= 4 {
        MdtReadLevel::L3
    } else if score >= 80 {
        MdtReadLevel::L2
    } else if score >= 40 {
        MdtReadLevel::L1
    } else {
        MdtReadLevel::L0
    }
}

fn load_content(
    project_root: &Path,
    node: &MdtNodeIndex,
    level: &MdtReadLevel,
) -> Result<String, String> {
    match level {
        MdtReadLevel::L0 => Ok(format!(
            "{}\narea={}\ntags={}",
            node.title,
            node.area.as_deref().unwrap_or(""),
            node.tags.join(",")
        )),
        MdtReadLevel::L1 => Ok(format!(
            "{}\nsummary={}\narea={}\ntags={}",
            node.title,
            node.summary.as_deref().unwrap_or(""),
            node.area.as_deref().unwrap_or(""),
            node.tags.join(",")
        )),
        MdtReadLevel::L2 | MdtReadLevel::L3 => {
            let full_path = project_root.join(&node.path);
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
    fn test_reader_upgrades_strong_title_match_to_l3() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("reader.md"),
            "---\nmdt_version: \"0.1.0\"\nid: reader\ntitle: Reader Design\ntree:\n  parent: null\n  order: 0\narea: format\nimportance: 4\nsummary: \"Reader expands context by budget.\"\n---\n# Reader Design\n\nFull body about L0 L1 L2 L3 context expansion.\n",
        )
        .unwrap();
        std::fs::write(
            temp.path().join("archive.md"),
            "---\nmdt_version: \"0.1.0\"\nid: archive\ntitle: Archive Format\ntree:\n  parent: null\n  order: 1\narea: format\nimportance: 2\nsummary: \"Archive packaging.\"\n---\n# Archive Format\n\nCold storage details.\n",
        )
        .unwrap();

        let bundle =
            crate::mdt::reader::MdtReader::load_context(temp.path(), "reader", 1000).unwrap();

        assert_eq!(bundle.items[0].node_id, "reader");
        assert_eq!(bundle.items[0].level, crate::mdt::types::MdtReadLevel::L3);
        assert!(bundle.items[0].content.contains("Full body about L0"));
    }
}
