use crate::bridge::proposal::{BridgeProposal, BridgeProposalStatus, SourceRefKind};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct BridgeProposalStore {
    root: PathBuf,
    lock: Arc<Mutex<()>>,
}

impl BridgeProposalStore {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            lock: Arc::new(Mutex::new(())),
        }
    }

    fn file_path(&self) -> PathBuf {
        self.root.join("proposals.json")
    }

    fn read_all_unlocked(&self) -> Result<Vec<BridgeProposal>, String> {
        let path = self.file_path();
        if !path.exists() {
            return Ok(Vec::new());
        }

        let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        if content.trim().is_empty() {
            return Ok(Vec::new());
        }

        serde_json::from_str(&content).map_err(|e| e.to_string())
    }

    fn read_all_lossy_unlocked(&self) -> Vec<BridgeProposal> {
        match self.read_all_unlocked() {
            Ok(proposals) => proposals,
            Err(error) => {
                tracing::warn!("Failed to read bridge proposals: {}", error);
                Vec::new()
            }
        }
    }

    fn write_all_unlocked(&self, proposals: &[BridgeProposal]) -> Result<(), String> {
        std::fs::create_dir_all(&self.root).map_err(|e| e.to_string())?;
        let content = serde_json::to_string_pretty(proposals).map_err(|e| e.to_string())?;
        let mut temp_file =
            tempfile::NamedTempFile::new_in(&self.root).map_err(|e| e.to_string())?;
        temp_file
            .write_all(content.as_bytes())
            .map_err(|e| e.to_string())?;
        temp_file.flush().map_err(|e| e.to_string())?;
        temp_file.as_file().sync_all().map_err(|e| e.to_string())?;
        temp_file
            .persist(self.file_path())
            .map(|_| ())
            .map_err(|e| e.error.to_string())
    }

    pub fn list(&self, status_filter: Option<&str>) -> Result<Vec<BridgeProposal>, String> {
        let _guard = self.lock.lock().map_err(|e| e.to_string())?;
        let requested_status = status_filter.map(BridgeProposalStatus::parse).transpose()?;
        let mut proposals = self.read_all_lossy_unlocked();
        proposals.retain(|proposal| {
            if let Some(filter) = &requested_status {
                proposal.status == *filter
            } else {
                proposal.status != BridgeProposalStatus::Archived
            }
        });
        proposals.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(proposals)
    }

    pub fn get(&self, id: &str) -> Result<Option<BridgeProposal>, String> {
        let _guard = self.lock.lock().map_err(|e| e.to_string())?;
        Ok(self
            .read_all_unlocked()?
            .into_iter()
            .find(|proposal| proposal.id == id))
    }

    pub fn upsert(&self, proposal: BridgeProposal) -> Result<BridgeProposal, String> {
        let _guard = self.lock.lock().map_err(|e| e.to_string())?;
        let mut proposals = self.read_all_unlocked()?;
        if let Some(existing) = proposals.iter_mut().find(|existing| {
            existing.source_ref.kind == proposal.source_ref.kind
                && existing.source_ref.id == proposal.source_ref.id
        }) {
            *existing = proposal.clone();
        } else {
            proposals.push(proposal.clone());
        }
        self.write_all_unlocked(&proposals)?;
        Ok(proposal)
    }

    pub fn update_status(
        &self,
        id: &str,
        status: BridgeProposalStatus,
        updated_at: u64,
    ) -> Result<BridgeProposal, String> {
        let _guard = self.lock.lock().map_err(|e| e.to_string())?;
        let mut proposals = self.read_all_unlocked()?;
        let proposal = proposals
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| format!("Bridge proposal not found: {id}"))?;

        proposal.status = status;
        proposal.updated_at = updated_at;
        let updated = proposal.clone();
        self.write_all_unlocked(&proposals)?;
        Ok(updated)
    }

    pub fn update_status_by_source_ref(
        &self,
        kind: &SourceRefKind,
        source_id: &str,
        status: BridgeProposalStatus,
        updated_at: u64,
    ) -> Result<Option<BridgeProposal>, String> {
        let _guard = self.lock.lock().map_err(|e| e.to_string())?;
        let mut proposals = self.read_all_unlocked()?;
        let Some(proposal) = proposals
            .iter_mut()
            .find(|p| &p.source_ref.kind == kind && p.source_ref.id == source_id)
        else {
            return Ok(None);
        };

        proposal.status = status;
        proposal.updated_at = updated_at;
        let updated = proposal.clone();
        self.write_all_unlocked(&proposals)?;
        Ok(Some(updated))
    }
}

pub fn store_for_dualtrack_dir(dualtrack_dir: &Path) -> BridgeProposalStore {
    BridgeProposalStore::new(dualtrack_dir.join("bridge"))
}

#[cfg(test)]
mod tests {
    use super::BridgeProposalStore;
    use crate::bridge::proposal::{
        BridgeProposal, BridgeProposalSource, BridgeProposalStatus, EvidenceKind, EvidenceRef,
        ImpactScope, ProposalAction, ProposalActionKind, ProposalRisk, SourceRef, SourceRefKind,
        TrustSnapshot,
    };

    fn proposal(id: &str, source_ref: SourceRef) -> BridgeProposal {
        BridgeProposal {
            id: id.to_string(),
            intent: format!("Proposal {id}"),
            summary: "Summary".to_string(),
            task_type: None,
            sandbox_summary: None,
            expected_output: None,
            risk_reason: None,
            source: BridgeProposalSource::Tool,
            source_ref,
            status: BridgeProposalStatus::Pending,
            evidence: vec![EvidenceRef {
                label: "Trace".to_string(),
                kind: EvidenceKind::Trace,
                reference: "trace-1".to_string(),
                confidence: Some(0.8),
                excerpt: Some("Relevant excerpt".to_string()),
            }],
            impact: ImpactScope::default(),
            risk: ProposalRisk::Low,
            actions: vec![ProposalAction {
                id: "approve".to_string(),
                label: "Approve".to_string(),
                kind: ProposalActionKind::ApproveTask,
                payload: serde_json::json!({ "id": id }),
            }],
            trust_snapshot: TrustSnapshot::default(),
            created_at: 100,
            updated_at: 100,
        }
    }

    fn source_ref(id: &str) -> SourceRef {
        SourceRef {
            kind: SourceRefKind::Task,
            id: id.to_string(),
            path: None,
        }
    }

    #[test]
    fn store_persists_and_lists_proposals() {
        let dir = tempfile::tempdir().unwrap();
        let store = BridgeProposalStore::new(dir.path().join("bridge"));

        store
            .upsert(proposal("proposal-1", source_ref("task-1")))
            .unwrap();

        let proposals = store.list(None).unwrap();
        assert_eq!(proposals.len(), 1);
        assert_eq!(proposals[0].id, "proposal-1");

        let reopened = BridgeProposalStore::new(dir.path().join("bridge"));
        let proposals = reopened.list(None).unwrap();
        assert_eq!(proposals.len(), 1);
        assert_eq!(proposals[0].source_ref.id, "task-1");
    }

    #[test]
    fn upsert_replaces_matching_source_ref() {
        let dir = tempfile::tempdir().unwrap();
        let store = BridgeProposalStore::new(dir.path().join("bridge"));

        store.upsert(proposal("old", source_ref("task-1"))).unwrap();
        let mut replacement = proposal("new", source_ref("task-1"));
        replacement.updated_at = 200;
        store.upsert(replacement).unwrap();

        let proposals = store.list(None).unwrap();
        assert_eq!(proposals.len(), 1);
        assert_eq!(proposals[0].id, "new");
    }

    #[test]
    fn status_filter_excludes_archived_by_default() {
        let dir = tempfile::tempdir().unwrap();
        let store = BridgeProposalStore::new(dir.path().join("bridge"));

        store
            .upsert(proposal("pending", source_ref("task-1")))
            .unwrap();
        let mut archived = proposal("archived", source_ref("task-2"));
        archived.status = BridgeProposalStatus::Archived;
        archived.updated_at = 200;
        store.upsert(archived).unwrap();

        let visible = store.list(None).unwrap();
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].id, "pending");

        let archived = store.list(Some("archived")).unwrap();
        assert_eq!(archived.len(), 1);
        assert_eq!(archived[0].id, "archived");
    }

    #[test]
    fn list_rejects_unknown_status_filter() {
        let dir = tempfile::tempdir().unwrap();
        let store = BridgeProposalStore::new(dir.path().join("bridge"));

        let error = store.list(Some("unknown")).unwrap_err();

        assert!(error.contains("Unknown bridge proposal status"));
    }

    #[test]
    fn update_status_errors_when_proposal_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        let store = BridgeProposalStore::new(dir.path().join("bridge"));

        let error = store
            .update_status("missing", BridgeProposalStatus::Rejected, 200)
            .unwrap_err();

        assert!(error.contains("Bridge proposal not found"));
    }

    #[test]
    fn update_status_by_source_ref_updates_matching_proposal() {
        let dir = tempfile::tempdir().unwrap();
        let store = BridgeProposalStore::new(dir.path().join("bridge"));
        let source = SourceRef {
            kind: SourceRefKind::Ghost,
            id: "ghost_1".to_string(),
            path: Some("Target.md".to_string()),
        };
        store.upsert(proposal("pending", source)).unwrap();

        let updated = store
            .update_status_by_source_ref(
                &SourceRefKind::Ghost,
                "ghost_1",
                BridgeProposalStatus::Applied,
                300,
            )
            .unwrap()
            .expect("matching source ref should update");

        assert_eq!(updated.status, BridgeProposalStatus::Applied);
        assert_eq!(updated.updated_at, 300);
    }

    #[test]
    fn update_status_by_source_ref_returns_none_for_missing_source() {
        let dir = tempfile::tempdir().unwrap();
        let store = BridgeProposalStore::new(dir.path().join("bridge"));

        let updated = store
            .update_status_by_source_ref(
                &SourceRefKind::Ghost,
                "ghost_missing",
                BridgeProposalStatus::Applied,
                300,
            )
            .unwrap();

        assert!(updated.is_none());
    }

    #[test]
    fn list_returns_empty_when_storage_json_is_corrupt() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("bridge");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("proposals.json"), "{not valid json").unwrap();
        let store = BridgeProposalStore::new(root);

        let proposals = store.list(None).unwrap();

        assert!(proposals.is_empty());
    }

    #[test]
    fn get_returns_error_when_storage_json_is_corrupt() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("bridge");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("proposals.json"), "{not valid json").unwrap();
        let store = BridgeProposalStore::new(root);

        let error = store.get("p1").unwrap_err();

        assert!(error.contains("invalid"));
    }

    #[test]
    fn risk_classifier_is_conservative() {
        assert_eq!(
            BridgeProposal::classify_risk(
                &ImpactScope {
                    modifies_notes: true,
                    ..ImpactScope::default()
                },
                false,
            ),
            ProposalRisk::High
        );
        assert_eq!(
            BridgeProposal::classify_risk(
                &ImpactScope {
                    exports_data: true,
                    ..ImpactScope::default()
                },
                false,
            ),
            ProposalRisk::High
        );
        assert_eq!(
            BridgeProposal::classify_risk(
                &ImpactScope {
                    external_side_effect: true,
                    ..ImpactScope::default()
                },
                false,
            ),
            ProposalRisk::High
        );
        assert_eq!(
            BridgeProposal::classify_risk(&ImpactScope::default(), true),
            ProposalRisk::High
        );
        assert_eq!(
            BridgeProposal::classify_risk(
                &ImpactScope {
                    notes: vec!["a.md".to_string(), "b.md".to_string()],
                    ..ImpactScope::default()
                },
                false,
            ),
            ProposalRisk::Medium
        );
        assert_eq!(
            BridgeProposal::classify_risk(
                &ImpactScope {
                    creates_files: true,
                    ..ImpactScope::default()
                },
                false,
            ),
            ProposalRisk::Medium
        );
        assert_eq!(
            BridgeProposal::classify_risk(&ImpactScope::default(), false),
            ProposalRisk::Low
        );
    }
}
