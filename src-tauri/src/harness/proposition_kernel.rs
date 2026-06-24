#[allow(unused_imports)]
pub use crate::harness::lean_kernel::{
    Proposition, PropositionGraph, PropositionId, PropositionRelation, RelationType,
    VerificationResult, Violation, ViolationDiagnostic, ViolationSeverity,
};

use crate::harness::lean_kernel::HybridLeanKernel;

pub struct PropositionKernel;

impl PropositionKernel {
    pub const NAME: &'static str = "PropositionKernel";

    pub fn verify(graph: &PropositionGraph) -> VerificationResult {
        HybridLeanKernel::verify(graph)
    }
}

#[cfg(test)]
mod tests {
    use crate::harness::proposition_kernel::PropositionKernel;
    use crate::harness::proposition_kernel::{
        Proposition, PropositionGraph, PropositionId, PropositionRelation, RelationType,
        ViolationSeverity,
    };

    fn make_pid(id: &str) -> PropositionId {
        PropositionId {
            id: id.to_string(),
            content_hash: format!("hash_{}", id),
            human_readable: format!("Prop {}", id),
        }
    }

    fn make_prop(id: &str) -> Proposition {
        Proposition {
            pid: make_pid(id),
            content: format!("Claim {}", id),
            source_agent_id: "test_agent".to_string(),
            confidence: 0.9,
        }
    }

    #[test]
    fn proposition_kernel_reports_identity_and_scope() {
        let graph = PropositionGraph {
            propositions: vec![make_prop("A")],
            relations: vec![],
            source_agent_id: "agent".to_string(),
            timestamp: 0,
        };

        let result = PropositionKernel::verify(&graph);

        assert_eq!(result.kernel_name, "PropositionKernel");
        assert_eq!(result.scope, "proposition_graph_consistency");
        assert!(!result.is_truth_proof);
        assert!(result.passed);
    }

    #[test]
    fn proposition_kernel_emits_diagnostics_with_repair_hints() {
        let graph = PropositionGraph {
            propositions: vec![make_prop("A")],
            relations: vec![PropositionRelation {
                from: make_pid("A"),
                to: make_pid("MISSING"),
                relation_type: RelationType::Implies,
                strength: 0.8,
            }],
            source_agent_id: "agent".to_string(),
            timestamp: 0,
        };

        let result = PropositionKernel::verify(&graph);

        assert!(!result.passed);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].severity, ViolationSeverity::Error);
        assert!(result.diagnostics[0]
            .repair_hint
            .as_ref()
            .unwrap()
            .contains("Add proposition"));
    }
}
