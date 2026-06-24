use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::jsonld::types::{
    JsonLdEdgeIndex, JsonLdGeneratedPaths, JsonLdGraphDocument, JsonLdLegacyMeta, JsonLdLinkEdge,
    JsonLdMigrationReport, JsonLdNodeDocument, JsonLdNodeIndex, JsonLdProjectIndex, JsonLdTree,
    JsonLdValidationReport,
};
use crate::parser::frontmatter::{parse_frontmatter, Frontmatter};

const SCHEMA_VERSION: &str = "feroha-jsonld-blocks-v1";
const CONTEXT_FILE: &str = "contexts/feroha-v1.jsonld";
const SHAPES_FILE: &str = "shapes/feroha-v1.ttl";

const CORE_CONTEXT: &str = r#"{
  "@context": {
    "@version": 1.1,
    "@protected": true,
    "fh": "https://feroha.local/ns#",
    "schema": "https://schema.org/",
    "prov": "http://www.w3.org/ns/prov#",
    "NoteNode": "fh:NoteNode",
    "LinkEdge": "fh:LinkEdge",
    "name": "schema:name",
    "keywords": "schema:keywords",
    "summary": "schema:abstract",
    "body": "schema:text",
    "bodyFormat": "schema:encodingFormat",
    "schemaVersion": "fh:schemaVersion",
    "sourcePath": "fh:sourcePath",
    "area": "fh:area",
    "importance": "fh:importance",
    "tree": "fh:tree",
    "links": {
      "@id": "fh:links",
      "@container": "@list"
    },
    "source": {
      "@id": "fh:source",
      "@type": "@id"
    },
    "target": {
      "@id": "fh:target",
      "@type": "@id"
    },
    "predicate": {
      "@id": "fh:predicate",
      "@type": "@id"
    },
    "origin": "fh:origin",
    "confidence": "fh:confidence",
    "legacy": "fh:legacy"
  }
}"#;

const CORE_SHAPES: &str = r#"@prefix fh: <https://feroha.local/ns#> .
@prefix sh: <http://www.w3.org/ns/shacl#> .
@prefix schema: <https://schema.org/> .

fh:NoteNodeShape
  a sh:NodeShape ;
  sh:targetClass fh:NoteNode ;
  sh:property [ sh:path schema:name ; sh:minCount 1 ; sh:maxCount 1 ] ;
  sh:property [ sh:path fh:schemaVersion ; sh:minCount 1 ; sh:maxCount 1 ] ;
  sh:property [ sh:path fh:sourcePath ; sh:minCount 1 ; sh:maxCount 1 ] .

fh:LinkEdgeShape
  a sh:NodeShape ;
  sh:targetClass fh:LinkEdge ;
  sh:property [ sh:path fh:source ; sh:minCount 1 ; sh:maxCount 1 ] ;
  sh:property [ sh:path fh:target ; sh:minCount 1 ; sh:maxCount 1 ] .
"#;

struct SourceNote {
    path: PathBuf,
    relative_path: String,
    content: String,
    frontmatter: Option<Frontmatter>,
    body_offset: usize,
    node_id: String,
    jsonld_file_name: String,
}

pub fn index_vault(root: &Path) -> Result<JsonLdProjectIndex, String> {
    let migration = build_migration(root)?;
    Ok(migration.index)
}

pub fn index_vault_with_artifacts(root: &Path) -> Result<JsonLdProjectIndex, String> {
    let migration = build_migration(root)?;
    write_artifacts(root, &migration)?;
    Ok(migration.index)
}

pub fn migrate_vault_with_artifacts(root: &Path) -> Result<JsonLdMigrationReport, String> {
    let migration = build_migration(root)?;
    write_artifacts(root, &migration)?;
    Ok(migration.report)
}

pub fn validate_vault(root: &Path) -> JsonLdValidationReport {
    match build_migration(root) {
        Ok(migration) => {
            let errors = validate_index(&migration.index);
            JsonLdValidationReport {
                valid: errors.is_empty(),
                node_count: migration.index.nodes.len(),
                edge_count: migration.index.edges.len(),
                errors,
                warnings: migration.report.warnings,
            }
        }
        Err(error) => JsonLdValidationReport {
            valid: false,
            node_count: 0,
            edge_count: 0,
            errors: vec![error],
            warnings: Vec::new(),
        },
    }
}

struct MigrationBuild {
    documents: Vec<JsonLdNodeDocument>,
    graph: JsonLdGraphDocument,
    index: JsonLdProjectIndex,
    report: JsonLdMigrationReport,
}

fn build_migration(root: &Path) -> Result<MigrationBuild, String> {
    if !root.is_dir() {
        return Err(format!(
            "project root is not a directory: {}",
            root.display()
        ));
    }

    let mut paths = Vec::new();
    collect_source_files(root, root, &mut paths)?;
    paths.sort();

    let mut warnings = Vec::new();
    let mut notes = Vec::new();
    let mut raw_id_to_jsonld = HashMap::new();
    let mut path_to_jsonld = HashMap::new();
    let mut stem_to_path = HashMap::new();
    let mut seen_ids = HashSet::new();

    for path in paths {
        let relative_path = normalize_relative_path(root, &path);
        let content = fs::read_to_string(&path)
            .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
        let (frontmatter, body_offset) = parse_frontmatter(&content)
            .map(|(fm, offset)| (Some(fm), offset))
            .unwrap_or((None, 0));
        let legacy_id = frontmatter
            .as_ref()
            .and_then(|fm| fm.mdt.as_ref())
            .and_then(|mdt| mdt.id.clone());
        let mut node_id = legacy_id
            .as_ref()
            .map(|id| format!("urn:feroha:node:{}", safe_token(id)))
            .unwrap_or_else(|| {
                warnings.push(format!(
                    "{} has no legacy id; assigned a path-derived JSON-LD @id",
                    relative_path
                ));
                format!("urn:feroha:node:{}", short_hash(relative_path.as_bytes()))
            });
        if !seen_ids.insert(node_id.clone()) {
            node_id = format!("{}-{}", node_id, short_hash(relative_path.as_bytes()));
            warnings.push(format!("duplicate node id resolved for {}", relative_path));
            seen_ids.insert(node_id.clone());
        }
        let jsonld_file_name = format!("{}.jsonld", node_id.rsplit(':').next().unwrap_or("node"));

        if let Some(id) = legacy_id {
            raw_id_to_jsonld.insert(id, node_id.clone());
        }
        path_to_jsonld.insert(relative_path.clone(), node_id.clone());
        if let Some(stem) = Path::new(&relative_path)
            .file_stem()
            .and_then(|s| s.to_str())
        {
            stem_to_path.insert(stem.to_lowercase(), relative_path.clone());
        }

        notes.push(SourceNote {
            path,
            relative_path,
            content,
            frontmatter,
            body_offset,
            node_id,
            jsonld_file_name,
        });
    }

    let known_paths = path_to_jsonld.keys().cloned().collect::<HashSet<_>>();
    let mut documents = Vec::new();
    let mut node_indexes = Vec::new();
    let mut edge_indexes = Vec::new();
    let mut graph_items = Vec::new();

    for note in &notes {
        let fm = note.frontmatter.as_ref();
        let meta = fm.and_then(|frontmatter| frontmatter.mdt.clone());
        let body = note.content[note.body_offset..].to_string();
        let title = fm
            .and_then(|frontmatter| frontmatter.title.clone())
            .unwrap_or_else(|| fallback_title(&note.relative_path));
        let tags = fm
            .map(|frontmatter| frontmatter.tags.clone())
            .unwrap_or_default();
        let tree = meta.as_ref().and_then(|mdt| {
            mdt.tree.as_ref().map(|tree| JsonLdTree {
                parent: tree.parent.as_ref().map(|parent| {
                    resolve_target(
                        parent,
                        &raw_id_to_jsonld,
                        &path_to_jsonld,
                        &stem_to_path,
                        &known_paths,
                    )
                }),
                order: tree.order,
                depth: tree.depth,
                path: tree.path.clone(),
            })
        });
        let timestamps = file_timestamps(&note.path);
        let mut links = Vec::new();

        if let Some(parent) = tree.as_ref().and_then(|tree| tree.parent.clone()) {
            links.push(link_edge(
                &note.node_id,
                &parent,
                "parent",
                "schema:isPartOf",
                "tree",
                1.0,
                None,
            ));
        }

        if let Some(mdt) = meta.as_ref() {
            for link in &mdt.links {
                let target = resolve_target(
                    &link.target,
                    &raw_id_to_jsonld,
                    &path_to_jsonld,
                    &stem_to_path,
                    &known_paths,
                );
                links.push(link_edge(
                    &note.node_id,
                    &target,
                    &link.edge_type,
                    predicate_for_edge_type(&link.edge_type),
                    "legacy_frontmatter",
                    link.confidence.unwrap_or(1.0),
                    link.label.clone(),
                ));
            }
        }

        for wikilink in crate::parser::ast::extract_wikilinks(&body, &note.relative_path) {
            let target = resolve_target(
                &wikilink.target,
                &raw_id_to_jsonld,
                &path_to_jsonld,
                &stem_to_path,
                &known_paths,
            );
            links.push(link_edge(
                &note.node_id,
                &target,
                "reference",
                "schema:citation",
                "wikilink",
                1.0,
                wikilink.display,
            ));
        }

        dedupe_links(&mut links);

        let legacy = meta.as_ref().map(|mdt| JsonLdLegacyMeta {
            format: Some("mdt-frontmatter".to_string()),
            mdt_version: mdt.mdt_version.clone(),
            mdt_id: mdt.id.clone(),
            slug: mdt.slug.clone(),
            storage_tier: mdt
                .storage
                .as_ref()
                .and_then(|storage| storage.tier.clone()),
            content_hash: mdt.content_hash.clone(),
        });

        let document = JsonLdNodeDocument {
            context: format!("../{}", CONTEXT_FILE),
            id: note.node_id.clone(),
            node_type: "NoteNode".to_string(),
            name: title.clone(),
            schema_version: SCHEMA_VERSION.to_string(),
            version: 1,
            created: timestamps.0,
            updated: timestamps.1,
            source_path: note.relative_path.clone(),
            body_format: "text/markdown; profile=CommonMark".to_string(),
            body,
            keywords: tags.clone(),
            summary: meta.as_ref().and_then(|mdt| mdt.summary.clone()),
            area: meta.as_ref().and_then(|mdt| mdt.area.clone()),
            importance: meta.as_ref().and_then(|mdt| mdt.importance),
            tree: tree.clone(),
            links: links.clone(),
            legacy,
        };

        graph_items.push(serde_json::to_value(&document).map_err(|err| err.to_string())?);
        for link in &links {
            graph_items.push(serde_json::to_value(link).map_err(|err| err.to_string())?);
            edge_indexes.push(JsonLdEdgeIndex {
                id: link.id.clone(),
                source: link.source.clone(),
                target: link.target.clone(),
                edge_type: link.relation.clone(),
                origin: link.origin.clone(),
                confidence: link.confidence,
                label: link.label.clone(),
                via: Some(note.relative_path.clone()),
            });
        }

        node_indexes.push(JsonLdNodeIndex {
            id: note.node_id.clone(),
            path: note.jsonld_file_name.clone(),
            title,
            jsonld_path: format!("nodes/{}", note.jsonld_file_name),
            source_path: note.relative_path.clone(),
            parent: tree.as_ref().and_then(|tree| tree.parent.clone()),
            order: tree.as_ref().map(|tree| tree.order).unwrap_or_default(),
            depth: tree.as_ref().and_then(|tree| tree.depth),
            area: meta.as_ref().and_then(|mdt| mdt.area.clone()),
            tags,
            importance: meta.as_ref().and_then(|mdt| mdt.importance),
            summary: meta.as_ref().and_then(|mdt| mdt.summary.clone()),
            content_hash: meta.as_ref().and_then(|mdt| mdt.content_hash.clone()),
        });

        documents.push(document);
    }

    let index = JsonLdProjectIndex {
        nodes: node_indexes,
        edges: edge_indexes,
    };
    let graph = JsonLdGraphDocument {
        context: format!("../{}", CONTEXT_FILE),
        graph: graph_items,
    };
    let report = JsonLdMigrationReport {
        format: "feroha-jsonld-blocks".to_string(),
        schema_version: SCHEMA_VERSION.to_string(),
        node_count: index.nodes.len(),
        edge_count: index.edges.len(),
        generated_paths: generated_paths(),
        warnings,
    };

    Ok(MigrationBuild {
        documents,
        graph,
        index,
        report,
    })
}

fn write_artifacts(root: &Path, migration: &MigrationBuild) -> Result<(), String> {
    let base = root.join(".dualtrack").join("jsonld");
    let contexts_dir = base.join("contexts");
    let shapes_dir = base.join("shapes");
    let nodes_dir = base.join("nodes");
    let indexes_dir = base.join("indexes");

    fs::create_dir_all(&contexts_dir)
        .map_err(|err| format!("failed to create {}: {err}", contexts_dir.display()))?;
    fs::create_dir_all(&shapes_dir)
        .map_err(|err| format!("failed to create {}: {err}", shapes_dir.display()))?;
    fs::create_dir_all(&nodes_dir)
        .map_err(|err| format!("failed to create {}: {err}", nodes_dir.display()))?;
    fs::create_dir_all(&indexes_dir)
        .map_err(|err| format!("failed to create {}: {err}", indexes_dir.display()))?;

    fs::write(base.join(CONTEXT_FILE), CORE_CONTEXT)
        .map_err(|err| format!("failed to write context: {err}"))?;
    fs::write(base.join(SHAPES_FILE), CORE_SHAPES)
        .map_err(|err| format!("failed to write SHACL shapes: {err}"))?;

    for document in &migration.documents {
        let file_name = document
            .id
            .rsplit(':')
            .next()
            .map(|id| format!("{}.jsonld", id))
            .unwrap_or_else(|| format!("{}.jsonld", short_hash(document.id.as_bytes())));
        write_json(&nodes_dir.join(file_name), document)?;
    }

    write_json(&indexes_dir.join("graph.jsonld"), &migration.graph)?;
    write_json(&indexes_dir.join("nodes.json"), &migration.index.nodes)?;
    write_json(&indexes_dir.join("edges.json"), &migration.index.edges)?;
    write_json(
        &indexes_dir.join("project.json"),
        &serde_json::json!({
            "format": "feroha-jsonld-blocks",
            "schema_version": SCHEMA_VERSION,
            "node_count": migration.index.nodes.len(),
            "edge_count": migration.index.edges.len(),
            "graph_path": "graph.jsonld",
            "nodes_path": "nodes.json",
            "edges_path": "edges.json"
        }),
    )?;
    write_json(&base.join("migration-report.json"), &migration.report)?;
    Ok(())
}

fn validate_index(index: &JsonLdProjectIndex) -> Vec<String> {
    let mut errors = Vec::new();
    let mut seen = HashSet::new();
    for node in &index.nodes {
        if !node.id.starts_with("urn:feroha:node:") {
            errors.push(format!("invalid JSON-LD node @id: {}", node.id));
        }
        if node.title.trim().is_empty() {
            errors.push(format!("node has empty name: {}", node.id));
        }
        if !seen.insert(node.id.clone()) {
            errors.push(format!("duplicate JSON-LD node @id: {}", node.id));
        }
    }
    let ids = index
        .nodes
        .iter()
        .map(|node| node.id.clone())
        .collect::<HashSet<_>>();
    for edge in &index.edges {
        if !ids.contains(&edge.source) {
            errors.push(format!(
                "edge source is missing from graph: {}",
                edge.source
            ));
        }
    }
    errors
}

fn collect_source_files(
    root: &Path,
    current: &Path,
    files: &mut Vec<PathBuf>,
) -> Result<(), String> {
    for entry in fs::read_dir(current)
        .map_err(|err| format!("failed to read directory {}: {err}", current.display()))?
    {
        let entry = entry.map_err(|err| err.to_string())?;
        let path = entry.path();
        let name = path
            .file_name()
            .and_then(|part| part.to_str())
            .unwrap_or("");
        if path.is_dir() {
            if should_skip_dir(name) {
                continue;
            }
            collect_source_files(root, &path, files)?;
        } else if is_markdown_or_mdt(&path) {
            files.push(path);
        }
    }
    let _ = root;
    Ok(())
}

fn should_skip_dir(name: &str) -> bool {
    name.starts_with('.')
        || name.starts_with('_')
        || matches!(name, "node_modules" | "target" | "dist" | "build")
}

fn is_markdown_or_mdt(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("md") | Some("mdt")
    )
}

fn generated_paths() -> JsonLdGeneratedPaths {
    JsonLdGeneratedPaths {
        context: ".dualtrack/jsonld/contexts/feroha-v1.jsonld".to_string(),
        shapes: ".dualtrack/jsonld/shapes/feroha-v1.ttl".to_string(),
        nodes_dir: ".dualtrack/jsonld/nodes".to_string(),
        graph: ".dualtrack/jsonld/indexes/graph.jsonld".to_string(),
        nodes_index: ".dualtrack/jsonld/indexes/nodes.json".to_string(),
        edges_index: ".dualtrack/jsonld/indexes/edges.json".to_string(),
        project: ".dualtrack/jsonld/indexes/project.json".to_string(),
        migration_report: ".dualtrack/jsonld/migration-report.json".to_string(),
    }
}

fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let json = serde_json::to_string_pretty(value)
        .map_err(|err| format!("failed to serialize {}: {err}", path.display()))?;
    fs::write(path, json).map_err(|err| format!("failed to write {}: {err}", path.display()))
}

fn link_edge(
    source: &str,
    target: &str,
    edge_type: &str,
    predicate: &str,
    origin: &str,
    confidence: f32,
    label: Option<String>,
) -> JsonLdLinkEdge {
    let edge_name = edge_type.trim().to_lowercase().replace('-', "_");
    JsonLdLinkEdge {
        id: format!(
            "urn:feroha:edge:{}",
            short_hash(format!("{source}|{target}|{edge_name}|{origin}").as_bytes())
        ),
        edge_type: "LinkEdge".to_string(),
        source: source.to_string(),
        target: target.to_string(),
        relation: edge_name,
        predicate: predicate.to_string(),
        origin: origin.to_string(),
        confidence,
        label,
    }
}

fn dedupe_links(links: &mut Vec<JsonLdLinkEdge>) {
    let mut seen = HashSet::new();
    links.retain(|link| {
        seen.insert(format!(
            "{}|{}|{}|{}",
            link.source, link.target, link.predicate, link.origin
        ))
    });
}

fn predicate_for_edge_type(edge_type: &str) -> &'static str {
    match edge_type.trim().to_lowercase().as_str() {
        "parent" => "schema:isPartOf",
        "reference" => "schema:citation",
        "related" => "schema:about",
        "source" => "prov:wasDerivedFrom",
        "sequence" => "fh:precedes",
        "semantic" => "schema:about",
        "temporal" => "fh:temporalNeighbor",
        "bridge" => "fh:bridges",
        _ => "schema:citation",
    }
}

fn resolve_target(
    target: &str,
    raw_id_to_jsonld: &HashMap<String, String>,
    path_to_jsonld: &HashMap<String, String>,
    stem_to_path: &HashMap<String, String>,
    known_paths: &HashSet<String>,
) -> String {
    let normalized = target.trim().replace('\\', "/");
    if let Some(id) = raw_id_to_jsonld.get(&normalized) {
        return id.clone();
    }
    if let Some(id) = path_to_jsonld.get(&normalized) {
        return id.clone();
    }
    let with_ext = if normalized.ends_with(".md") || normalized.ends_with(".mdt") {
        normalized.clone()
    } else {
        format!("{}.md", normalized)
    };
    if known_paths.contains(&with_ext) {
        if let Some(id) = path_to_jsonld.get(&with_ext) {
            return id.clone();
        }
    }
    let stem = Path::new(&normalized)
        .file_stem()
        .and_then(|part| part.to_str())
        .unwrap_or(&normalized)
        .to_lowercase();
    if let Some(path) = stem_to_path
        .get(&stem)
        .and_then(|path| path_to_jsonld.get(path))
    {
        return path.clone();
    }
    format!("urn:feroha:external:{}", short_hash(normalized.as_bytes()))
}

fn fallback_title(relative_path: &str) -> String {
    relative_path
        .rsplit('/')
        .next()
        .unwrap_or(relative_path)
        .trim_end_matches(".mdt")
        .trim_end_matches(".md")
        .to_string()
}

fn normalize_relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn safe_token(value: &str) -> String {
    let mut token = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_ascii_lowercase();
    if token.is_empty() {
        token = short_hash(value.as_bytes());
    }
    token
}

fn short_hash(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())[..16].to_string()
}

fn file_timestamps(path: &Path) -> (String, String) {
    let metadata = fs::metadata(path).ok();
    let modified = metadata
        .as_ref()
        .and_then(|metadata| metadata.modified().ok())
        .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
    let created = metadata
        .and_then(|metadata| metadata.created().ok())
        .unwrap_or(modified);
    (timestamp_string(created), timestamp_string(modified))
}

fn timestamp_string(time: std::time::SystemTime) -> String {
    let millis = time
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("unix-ms:{millis}")
}

#[cfg(test)]
mod tests {
    #[test]
    fn jsonld_indexer_migrates_markdown_frontmatter_and_wikilinks() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("root.md"),
            "---\nmdt_version: \"0.1.0\"\nid: root\ntitle: Root\ntags: graph, jsonld\nlinks:\n  - target: child\n    type: related\n---\n# Root\n\nSee [[child]].\n",
        )
        .unwrap();
        std::fs::write(
            temp.path().join("child.md"),
            "---\nid: child\ntitle: Child\n---\n# Child\n",
        )
        .unwrap();

        let index = crate::jsonld::indexer::index_vault_with_artifacts(temp.path()).unwrap();

        assert_eq!(index.nodes.len(), 2);
        assert!(index
            .edges
            .iter()
            .any(|edge| edge.origin == "legacy_frontmatter"));
        assert!(index.edges.iter().any(|edge| edge.origin == "wikilink"));
        assert!(temp
            .path()
            .join(".dualtrack")
            .join("jsonld")
            .join("indexes")
            .join("graph.jsonld")
            .exists());
    }
}
