use std::fs;
use std::path::{Path, PathBuf};

use crate::mdt::types::{
    MdtEdgeIndex, MdtMeta, MdtNodeIndex, MdtProjectIndex, MdtValidationReport,
};
use crate::parser::frontmatter::parse_frontmatter;

pub fn index_vault(root: &Path) -> Result<MdtProjectIndex, String> {
    let mut files = Vec::new();
    collect_markdown_files(root, root, &mut files)?;
    files.sort();

    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    for path in files {
        let content = fs::read_to_string(&path)
            .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
        let (frontmatter, _) = match parse_frontmatter(&content) {
            Some(parsed) => parsed,
            None => continue,
        };
        let meta = frontmatter.mdt.unwrap_or_default();
        let relative_path = normalize_relative_path(root, &path);
        let id = meta.id.clone().unwrap_or_else(|| relative_path.clone());
        let tree = meta.tree.clone().unwrap_or_default();

        for link in &meta.links {
            edges.push(MdtEdgeIndex {
                source: id.clone(),
                target: link.target.clone(),
                edge_type: link.edge_type.clone(),
                origin: "frontmatter".to_string(),
                confidence: link.confidence.unwrap_or(1.0),
                label: link.label.clone(),
            });
        }

        nodes.push(node_index_from_meta(
            relative_path,
            id,
            frontmatter.title,
            frontmatter.tags,
            meta,
            tree.parent,
            tree.order,
            tree.depth,
        ));
    }

    Ok(MdtProjectIndex { nodes, edges })
}

pub fn validate_vault(root: &Path) -> MdtValidationReport {
    match index_vault(root) {
        Ok(index) => {
            let mut errors = Vec::new();
            let mut seen = std::collections::HashSet::new();
            for node in &index.nodes {
                if !seen.insert(node.id.clone()) {
                    errors.push(format!("duplicate MDT node id: {}", node.id));
                }
            }
            MdtValidationReport {
                valid: errors.is_empty(),
                node_count: index.nodes.len(),
                edge_count: index.edges.len(),
                errors,
                warnings: Vec::new(),
            }
        }
        Err(error) => MdtValidationReport {
            valid: false,
            node_count: 0,
            edge_count: 0,
            errors: vec![error],
            warnings: Vec::new(),
        },
    }
}

fn collect_markdown_files(
    root: &Path,
    current: &Path,
    files: &mut Vec<PathBuf>,
) -> Result<(), String> {
    for entry in fs::read_dir(current)
        .map_err(|err| format!("failed to read directory {}: {err}", current.display()))?
    {
        let entry = entry.map_err(|err| err.to_string())?;
        let path = entry.path();
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if path.is_dir() {
            if file_name == ".dualtrack" {
                continue;
            }
            collect_markdown_files(root, &path, files)?;
        } else if is_markdown_node(&path) {
            files.push(path);
        }
    }
    let _ = root;
    Ok(())
}

fn is_markdown_node(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("md") | Some("mdt")
    )
}

fn normalize_relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn node_index_from_meta(
    path: String,
    id: String,
    title: Option<String>,
    tags: Vec<String>,
    meta: MdtMeta,
    parent: Option<String>,
    order: i64,
    depth: Option<u32>,
) -> MdtNodeIndex {
    let fallback_title = path
        .rsplit('/')
        .next()
        .unwrap_or(&path)
        .trim_end_matches(".mdt")
        .trim_end_matches(".md")
        .to_string();
    MdtNodeIndex {
        id,
        path,
        title: title.unwrap_or(fallback_title),
        slug: meta.slug,
        parent,
        order,
        depth,
        area: meta.area,
        tags,
        importance: meta.importance,
        storage_tier: meta.storage.and_then(|storage| storage.tier),
        summary: meta.summary,
        content_hash: meta.content_hash,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_indexer_emits_nodes_and_typed_edges() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("root.md"),
            "---\nmdt_version: \"0.1.0\"\nid: root\ntitle: Root\ntree:\n  parent: null\n  order: 0\nlinks:\n  - target: child\n    type: parent\n---\n# Root\n",
        )
        .unwrap();
        std::fs::write(
            temp.path().join("child.md"),
            "---\nmdt_version: \"0.1.0\"\nid: child\ntitle: Child\ntree:\n  parent: root\n  order: 1\narea: dream\n---\n# Child\n",
        )
        .unwrap();

        let index = crate::mdt::indexer::index_vault(temp.path()).unwrap();
        assert_eq!(index.nodes.len(), 2);
        assert_eq!(index.edges.len(), 1);
        assert_eq!(index.edges[0].edge_type, "parent");
    }
}
