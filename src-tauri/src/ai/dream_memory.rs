use crate::harness::workflow::{
    safe_runtime_component, ArtifactRef, ArtifactType, RetentionPolicy,
};
use sha2::{Digest, Sha256};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DreamMemoryZone {
    Working,
    Semantic,
    LongTerm,
}

impl DreamMemoryZone {
    pub fn canonical_dir(self) -> &'static str {
        match self {
            DreamMemoryZone::Working => "memory/working",
            DreamMemoryZone::Semantic => "memory/semantic",
            DreamMemoryZone::LongTerm => "memory/long_term",
        }
    }
}

const WORKING_ROOTS: &[&str] = &[
    ".dualtrack/memory/working/",
    ".dualtrack/research/",
    ".dualtrack/snapshots/",
    ".dualtrack/output/",
    ".dualtrack/bridge/",
    ".dualtrack/ghosts/",
];

const SEMANTIC_ROOTS: &[&str] = &[
    ".dualtrack/memory/semantic/",
    ".dualtrack/jsonld/",
    ".dualtrack/mdt/",
    ".dualtrack/fts/",
    ".dualtrack/vectors/",
];

const LONG_TERM_ROOTS: &[&str] = &[
    ".dualtrack/memory/long_term/",
    ".dualtrack/dream/",
    ".dualtrack/archive/",
    ".dualtrack/imports/",
];

pub fn classify_ai_memory_path(path: &str) -> Option<DreamMemoryZone> {
    let normalized = normalize_vault_path(path);
    if WORKING_ROOTS
        .iter()
        .any(|root| normalized.starts_with(root))
    {
        return Some(DreamMemoryZone::Working);
    }
    if SEMANTIC_ROOTS
        .iter()
        .any(|root| normalized.starts_with(root))
    {
        return Some(DreamMemoryZone::Semantic);
    }
    if LONG_TERM_ROOTS
        .iter()
        .any(|root| normalized.starts_with(root))
    {
        return Some(DreamMemoryZone::LongTerm);
    }
    None
}

pub fn ensure_dream_memory_layout(dualtrack_dir: &Path) -> Result<(), String> {
    for zone in [
        DreamMemoryZone::Working,
        DreamMemoryZone::Semantic,
        DreamMemoryZone::LongTerm,
    ] {
        let canonical_relative_path = format!(".dualtrack/{}/", zone.canonical_dir());
        if classify_ai_memory_path(&canonical_relative_path) != Some(zone) {
            return Err(format!(
                "Dream memory layout mismatch for {}",
                canonical_relative_path
            ));
        }
        std::fs::create_dir_all(dualtrack_dir.join(zone.canonical_dir()))
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub fn working_result_artifact(
    vault_root: &Path,
    task_id: &str,
    step_id: &str,
    created_at: &str,
) -> Result<ArtifactRef, String> {
    let task_id = safe_runtime_component(task_id).map_err(|error| error.to_string())?;
    let step_id = safe_runtime_component(step_id).map_err(|error| error.to_string())?;
    let uri = format!(".dualtrack/research/results/{task_id}/result.md");
    if classify_ai_memory_path(&uri) != Some(DreamMemoryZone::Working) {
        return Err(format!("Working artifact path is outside Dream Working: {uri}"));
    }
    let bytes = std::fs::read(vault_root.join(&uri)).map_err(|error| error.to_string())?;

    Ok(ArtifactRef {
        artifact_id: format!("working_result_{task_id}"),
        artifact_type: ArtifactType::Other,
        uri,
        hash: sha256_prefixed(&bytes),
        mime_type: "text/markdown".to_string(),
        producer_step_id: step_id.to_string(),
        retention_policy: RetentionPolicy::Workflow,
        created_at: created_at.to_string(),
    })
}

pub fn write_semantic_workflow_memory(
    dualtrack_dir: &Path,
    workflow_id: &str,
    run_id: &str,
    step_id: &str,
    content: &str,
    created_at: &str,
) -> Result<ArtifactRef, String> {
    let workflow_id =
        safe_runtime_component(workflow_id).map_err(|error| error.to_string())?;
    let run_id = safe_runtime_component(run_id).map_err(|error| error.to_string())?;
    let step_id = safe_runtime_component(step_id).map_err(|error| error.to_string())?;
    ensure_dream_memory_layout(dualtrack_dir)?;

    let uri =
        format!(".dualtrack/memory/semantic/workflows/{workflow_id}/{run_id}.md");
    if classify_ai_memory_path(&uri) != Some(DreamMemoryZone::Semantic) {
        return Err(format!(
            "Semantic artifact path is outside Dream Semantic: {uri}"
        ));
    }
    let path = dualtrack_dir
        .join("memory")
        .join("semantic")
        .join("workflows")
        .join(workflow_id)
        .join(format!("{run_id}.md"));
    let parent = path
        .parent()
        .ok_or_else(|| format!("Semantic artifact has no parent: {}", path.display()))?;
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    std::fs::write(&path, content.as_bytes()).map_err(|error| error.to_string())?;

    Ok(ArtifactRef {
        artifact_id: format!("semantic_{workflow_id}_{run_id}"),
        artifact_type: ArtifactType::Other,
        uri,
        hash: sha256_prefixed(content.as_bytes()),
        mime_type: "text/markdown".to_string(),
        producer_step_id: step_id.to_string(),
        retention_policy: RetentionPolicy::Workflow,
        created_at: created_at.to_string(),
    })
}

fn sha256_prefixed(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

fn normalize_vault_path(path: &str) -> String {
    path.replace('\\', "/")
        .trim_start_matches("./")
        .trim_start_matches('/')
        .to_string()
}

#[cfg(test)]
mod tests {
    #[test]
    fn classifies_legacy_dualtrack_roots_into_the_three_dream_memory_zones() {
        assert_eq!(
            super::classify_ai_memory_path(".dualtrack/research/results/task/result.md"),
            Some(super::DreamMemoryZone::Working)
        );
        assert_eq!(
            super::classify_ai_memory_path(".dualtrack/jsonld/indexes/claims.json"),
            Some(super::DreamMemoryZone::Semantic)
        );
        assert_eq!(
            super::classify_ai_memory_path(".dualtrack/dream/insight.md"),
            Some(super::DreamMemoryZone::LongTerm)
        );
        assert_eq!(super::classify_ai_memory_path("Human/Dream.md"), None);
    }

    #[test]
    fn ensures_canonical_dream_memory_directories_exist() {
        let dir = tempfile::tempdir().unwrap();
        let dualtrack_dir = dir.path().join(".dualtrack");

        super::ensure_dream_memory_layout(&dualtrack_dir).unwrap();

        assert!(dualtrack_dir.join("memory").join("working").exists());
        assert!(dualtrack_dir.join("memory").join("semantic").exists());
        assert!(dualtrack_dir.join("memory").join("long_term").exists());
    }

    #[test]
    fn semantic_workflow_memory_writes_only_under_canonical_semantic_root() {
        let root = tempfile::tempdir().unwrap();
        let dualtrack = root.path().join(".dualtrack");
        super::ensure_dream_memory_layout(&dualtrack).unwrap();

        let artifact = super::write_semantic_workflow_memory(
            &dualtrack,
            "wf_100",
            "run_100",
            "S001",
            "# Verified knowledge\n",
            "100",
        )
        .unwrap();

        assert_eq!(
            artifact.uri,
            ".dualtrack/memory/semantic/workflows/wf_100/run_100.md"
        );
        assert_eq!(
            super::classify_ai_memory_path(&artifact.uri),
            Some(super::DreamMemoryZone::Semantic)
        );
        assert!(artifact.hash.starts_with("sha256:"));
        assert!(!dualtrack
            .join("memory/long_term/workflows/wf_100/run_100.md")
            .exists());
    }

    #[test]
    fn working_result_artifact_references_existing_research_output() {
        let root = tempfile::tempdir().unwrap();
        let result_dir = root
            .path()
            .join(".dualtrack/research/results/task_100");
        std::fs::create_dir_all(&result_dir).unwrap();
        std::fs::write(result_dir.join("result.md"), "# Working result\n").unwrap();

        let artifact =
            super::working_result_artifact(root.path(), "task_100", "S001", "100").unwrap();

        assert_eq!(
            artifact.uri,
            ".dualtrack/research/results/task_100/result.md"
        );
        assert_eq!(
            super::classify_ai_memory_path(&artifact.uri),
            Some(super::DreamMemoryZone::Working)
        );
        assert!(artifact.hash.starts_with("sha256:"));
    }
}
