use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct PropositionId {
    pub id: String,
    pub content_hash: String,
    pub human_readable: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Proposition {
    pub pid: PropositionId,
    pub content: String,
    pub source_agent_id: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RelationType {
    Implies,
    Contradicts,
    Supports,
    DependsOn,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PropositionRelation {
    pub from: PropositionId,
    pub to: PropositionId,
    pub relation_type: RelationType,
    pub strength: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PropositionGraph {
    pub propositions: Vec<Proposition>,
    pub relations: Vec<PropositionRelation>,
    pub source_agent_id: String,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Violation {
    CycleDetected(Vec<String>),
    DanglingReference(String),
    DirectConflict(String, String),
    UnsupportedChain(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ViolationSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ViolationDiagnostic {
    pub violation: Violation,
    pub message: String,
    pub severity: ViolationSeverity,
    pub repair_hint: Option<String>,
}

impl Violation {
    pub fn severity(&self) -> ViolationSeverity {
        match self {
            Violation::CycleDetected(_)
            | Violation::DanglingReference(_)
            | Violation::DirectConflict(_, _) => ViolationSeverity::Error,
            Violation::UnsupportedChain(_) => ViolationSeverity::Warning,
        }
    }

    pub fn repair_hint(&self) -> Option<String> {
        match self {
            Violation::CycleDetected(_) => Some(
                "Break the circular implication/support chain or mark one edge as non-structural."
                    .to_string(),
            ),
            Violation::DanglingReference(id) => {
                Some(format!("Add proposition `{}` or remove the relation that references it.", id))
            }
            Violation::DirectConflict(a, b) => Some(format!(
                "Review claims `{}` and `{}`; keep both only if the contradiction is intentional evidence.",
                a, b
            )),
            Violation::UnsupportedChain(id) => Some(format!(
                "Attach supporting evidence or a foundation proposition for `{}`.",
                id
            )),
        }
    }

    pub fn diagnostic(&self) -> ViolationDiagnostic {
        ViolationDiagnostic {
            violation: self.clone(),
            message: self.to_string(),
            severity: self.severity(),
            repair_hint: self.repair_hint(),
        }
    }
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Violation::CycleDetected(ids) => {
                write!(f, "Cycle detected: {}", ids.join(" -> "))
            }
            Violation::DanglingReference(id) => {
                write!(f, "Dangling reference: proposition {} not found", id)
            }
            Violation::DirectConflict(a, b) => {
                write!(f, "Direct conflict: {} contradicts {}", a, b)
            }
            Violation::UnsupportedChain(id) => {
                write!(
                    f,
                    "Unsupported chain: proposition {} depends on non-terminal foundation",
                    id
                )
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VerificationResult {
    pub passed: bool,
    pub violations: Vec<Violation>,
    #[serde(default)]
    pub diagnostics: Vec<ViolationDiagnostic>,
    pub warnings: Vec<String>,
    pub topology_valid: bool,
    pub reference_valid: bool,
    pub conflict_free: bool,
    #[serde(default = "default_kernel_name")]
    pub kernel_name: String,
    #[serde(default = "default_kernel_scope")]
    pub scope: String,
    #[serde(default)]
    pub is_truth_proof: bool,
}

pub struct HybridLeanKernel;

impl HybridLeanKernel {
    pub fn verify(graph: &PropositionGraph) -> VerificationResult {
        let (topology_valid, top_violations) = Self::check_dag_topology(graph);
        let (reference_valid, ref_violations) = Self::check_references(graph);
        let (conflict_free, conf_violations) = Self::check_conflicts(graph);
        let (_, chain_violations) = Self::check_unsupported_chains(graph);

        let mut violations = Vec::new();
        violations.extend(top_violations);
        violations.extend(ref_violations);
        violations.extend(conf_violations);
        violations.extend(chain_violations);

        let warnings = Vec::new();
        let passed = violations.is_empty();

        VerificationResult {
            passed,
            diagnostics: violations.iter().map(Violation::diagnostic).collect(),
            violations,
            warnings,
            topology_valid,
            reference_valid,
            conflict_free,
            kernel_name: default_kernel_name(),
            scope: default_kernel_scope(),
            is_truth_proof: false,
        }
    }

    fn check_dag_topology(graph: &PropositionGraph) -> (bool, Vec<Violation>) {
        let mut violations = Vec::new();

        // Build adjacency: Implies, Supports, DependsOn are directed edges
        let mut adjacency: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for rel in &graph.relations {
            match rel.relation_type {
                RelationType::Implies | RelationType::Supports | RelationType::DependsOn => {
                    adjacency.entry(&rel.from.id).or_default().push(&rel.to.id);
                }
                RelationType::Contradicts => {}
            }
        }

        let mut visited = BTreeSet::new();
        let mut in_stack = BTreeSet::new();
        let mut path: Vec<String> = Vec::new();

        // Collect all nodes: propositions + nodes only in relations
        let mut all_nodes: Vec<&str> = graph
            .propositions
            .iter()
            .map(|p| p.pid.id.as_str())
            .collect();
        for rel in &graph.relations {
            all_nodes.push(&rel.from.id);
            all_nodes.push(&rel.to.id);
        }
        all_nodes.sort();
        all_nodes.dedup();

        for node in &all_nodes {
            if !visited.contains(*node) {
                Self::dfs_cycle(
                    node,
                    &adjacency,
                    &mut visited,
                    &mut in_stack,
                    &mut path,
                    &mut violations,
                );
            }
        }

        (violations.is_empty(), violations)
    }

    fn dfs_cycle(
        node: &str,
        adjacency: &BTreeMap<&str, Vec<&str>>,
        visited: &mut BTreeSet<String>,
        in_stack: &mut BTreeSet<String>,
        path: &mut Vec<String>,
        violations: &mut Vec<Violation>,
    ) {
        visited.insert(node.to_string());
        in_stack.insert(node.to_string());
        path.push(node.to_string());

        if let Some(neighbors) = adjacency.get(node) {
            // Sort neighbors for deterministic traversal
            let mut sorted_neighbors: Vec<&&str> = neighbors.iter().collect();
            sorted_neighbors.sort();
            for neighbor in sorted_neighbors {
                if in_stack.contains(*neighbor) {
                    let cycle_start = path.iter().position(|n| n == neighbor).unwrap();
                    let mut cycle: Vec<String> = path[cycle_start..].to_vec();
                    cycle.push((*neighbor).to_string());
                    violations.push(Violation::CycleDetected(cycle));
                } else if !visited.contains(*neighbor) {
                    Self::dfs_cycle(neighbor, adjacency, visited, in_stack, path, violations);
                }
            }
        }

        path.pop();
        in_stack.remove(node);
    }

    fn check_references(graph: &PropositionGraph) -> (bool, Vec<Violation>) {
        let mut violations = Vec::new();

        let prop_ids: BTreeSet<&str> = graph
            .propositions
            .iter()
            .map(|p| p.pid.id.as_str())
            .collect();

        let mut seen_violations: BTreeSet<String> = BTreeSet::new();
        for rel in &graph.relations {
            if !prop_ids.contains(rel.from.id.as_str()) {
                let key = format!("from:{}", rel.from.id);
                if !seen_violations.contains(&key) {
                    seen_violations.insert(key);
                    violations.push(Violation::DanglingReference(rel.from.id.clone()));
                }
            }
            if !prop_ids.contains(rel.to.id.as_str()) {
                let key = format!("to:{}", rel.to.id);
                if !seen_violations.contains(&key) {
                    seen_violations.insert(key);
                    violations.push(Violation::DanglingReference(rel.to.id.clone()));
                }
            }
        }

        (violations.is_empty(), violations)
    }

    fn check_conflicts(graph: &PropositionGraph) -> (bool, Vec<Violation>) {
        let mut violations = Vec::new();

        for rel in &graph.relations {
            if rel.relation_type == RelationType::Contradicts {
                violations.push(Violation::DirectConflict(
                    rel.from.id.clone(),
                    rel.to.id.clone(),
                ));
            }
        }

        (violations.is_empty(), violations)
    }

    fn check_unsupported_chains(graph: &PropositionGraph) -> (bool, Vec<Violation>) {
        let mut violations = Vec::new();

        let has_implies_or_supports = |pid: &str| -> bool {
            graph.relations.iter().any(|r| {
                (r.relation_type == RelationType::Implies
                    || r.relation_type == RelationType::Supports)
                    && r.to.id == pid
            })
        };

        let has_depends_on = |pid: &str| -> bool {
            graph
                .relations
                .iter()
                .any(|r| r.relation_type == RelationType::DependsOn && r.from.id == pid)
        };

        for rel in &graph.relations {
            if rel.relation_type == RelationType::DependsOn {
                let target = &rel.to.id;
                if !has_depends_on(target) && !has_implies_or_supports(target) {
                    violations.push(Violation::UnsupportedChain(rel.from.id.clone()));
                }
            }
        }

        (violations.is_empty(), violations)
    }
}

fn default_kernel_name() -> String {
    "PropositionKernel".to_string()
}

fn default_kernel_scope() -> String {
    "proposition_graph_consistency".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_pid(id: &str) -> PropositionId {
        PropositionId {
            id: id.to_string(),
            content_hash: format!("hash_{}", id),
            human_readable: format!("Prop {}", id),
        }
    }

    fn make_prop(id: &str, content: &str, confidence: f32) -> Proposition {
        Proposition {
            pid: make_pid(id),
            content: content.to_string(),
            source_agent_id: "test_agent".to_string(),
            confidence,
        }
    }

    #[test]
    fn test_verify_empty_graph() {
        let graph = PropositionGraph {
            propositions: vec![],
            relations: vec![],
            source_agent_id: "agent".to_string(),
            timestamp: 0,
        };
        let result = HybridLeanKernel::verify(&graph);
        assert!(result.passed);
        assert!(result.violations.is_empty());
        assert!(result.topology_valid);
        assert!(result.reference_valid);
        assert!(result.conflict_free);
    }

    #[test]
    fn test_verify_valid_graph() {
        let a = make_prop("A", "All men are mortal", 0.9);
        let b = make_prop("B", "Socrates is a man", 0.95);
        let c = make_prop("C", "Socrates is mortal", 0.85);

        #[allow(deprecated)]
        let graph = PropositionGraph {
            propositions: vec![a, b, c],
            relations: vec![PropositionRelation {
                from: make_pid("A"),
                to: make_pid("C"),
                relation_type: RelationType::Implies,
                strength: 0.8,
            }],
            source_agent_id: "agent".to_string(),
            timestamp: 0,
        };

        let result = HybridLeanKernel::verify(&graph);
        assert!(result.passed);
        assert!(result.violations.is_empty());
        assert!(result.topology_valid);
        assert!(result.reference_valid);
        assert!(result.conflict_free);
    }

    #[test]
    fn test_cycle_detection_simple() {
        let a = make_prop("A", "Claim A", 0.9);
        let b = make_prop("B", "Claim B", 0.9);

        let graph = PropositionGraph {
            propositions: vec![a, b],
            relations: vec![
                PropositionRelation {
                    from: make_pid("A"),
                    to: make_pid("B"),
                    relation_type: RelationType::Implies,
                    strength: 0.8,
                },
                PropositionRelation {
                    from: make_pid("B"),
                    to: make_pid("A"),
                    relation_type: RelationType::Implies,
                    strength: 0.8,
                },
            ],
            source_agent_id: "agent".to_string(),
            timestamp: 0,
        };

        let result = HybridLeanKernel::verify(&graph);
        assert!(!result.passed);
        assert!(!result.topology_valid);
        assert_eq!(result.violations.len(), 1);
        match &result.violations[0] {
            Violation::CycleDetected(ids) => {
                let display = format!("{}", result.violations[0]);
                assert!(display.contains("Cycle detected"));
                assert!(ids.contains(&"A".to_string()));
                assert!(ids.contains(&"B".to_string()));
            }
            _ => panic!("Expected CycleDetected violation"),
        }
    }

    #[test]
    fn test_cycle_detection_chain() {
        let a = make_prop("A", "Claim A", 0.9);
        let b = make_prop("B", "Claim B", 0.9);
        let c = make_prop("C", "Claim C", 0.9);

        let graph = PropositionGraph {
            propositions: vec![a, b, c],
            relations: vec![
                PropositionRelation {
                    from: make_pid("A"),
                    to: make_pid("B"),
                    relation_type: RelationType::Supports,
                    strength: 0.8,
                },
                PropositionRelation {
                    from: make_pid("B"),
                    to: make_pid("C"),
                    relation_type: RelationType::DependsOn,
                    strength: 0.7,
                },
                PropositionRelation {
                    from: make_pid("C"),
                    to: make_pid("A"),
                    relation_type: RelationType::Implies,
                    strength: 0.9,
                },
            ],
            source_agent_id: "agent".to_string(),
            timestamp: 0,
        };

        let result = HybridLeanKernel::verify(&graph);
        assert!(!result.passed);
        assert!(!result.topology_valid);
        match &result.violations[0] {
            Violation::CycleDetected(ids) => {
                assert!(!ids.is_empty());
            }
            _ => panic!("Expected CycleDetected violation"),
        }
    }

    #[test]
    fn test_dangling_reference() {
        let a = make_prop("A", "Claim A", 0.9);

        let graph = PropositionGraph {
            propositions: vec![a],
            relations: vec![PropositionRelation {
                from: make_pid("A"),
                to: make_pid("NONEXISTENT"),
                relation_type: RelationType::Implies,
                strength: 0.8,
            }],
            source_agent_id: "agent".to_string(),
            timestamp: 0,
        };

        let result = HybridLeanKernel::verify(&graph);
        assert!(!result.passed);
        assert!(!result.reference_valid);
        match &result.violations[0] {
            Violation::DanglingReference(id) => {
                assert_eq!(id, "NONEXISTENT");
            }
            _ => panic!("Expected DanglingReference violation"),
        }
    }

    #[test]
    fn test_direct_conflict() {
        let a = make_prop("A", "The sky is blue", 0.9);
        let b = make_prop("B", "The sky is red", 0.5);

        let graph = PropositionGraph {
            propositions: vec![a, b],
            relations: vec![PropositionRelation {
                from: make_pid("A"),
                to: make_pid("B"),
                relation_type: RelationType::Contradicts,
                strength: 1.0,
            }],
            source_agent_id: "agent".to_string(),
            timestamp: 0,
        };

        let result = HybridLeanKernel::verify(&graph);
        assert!(!result.passed);
        assert!(!result.conflict_free);
        match &result.violations[0] {
            Violation::DirectConflict(a_id, b_id) => {
                assert_eq!(a_id, "A");
                assert_eq!(b_id, "B");
            }
            _ => panic!("Expected DirectConflict violation"),
        }
    }

    #[test]
    fn test_serde_roundtrip() {
        let a = make_prop("A", "Claim A", 0.9);
        let b = make_prop("B", "Claim B", 0.8);

        let graph = PropositionGraph {
            propositions: vec![a, b],
            relations: vec![PropositionRelation {
                from: make_pid("A"),
                to: make_pid("B"),
                relation_type: RelationType::Implies,
                strength: 0.7,
            }],
            source_agent_id: "agent".to_string(),
            timestamp: 1712345678,
        };

        let json = serde_json::to_string(&graph).unwrap();
        let decoded: PropositionGraph = serde_json::from_str(&json).unwrap();

        assert_eq!(graph.propositions.len(), decoded.propositions.len());
        assert_eq!(graph.propositions[0].pid.id, decoded.propositions[0].pid.id);
        assert_eq!(
            graph.propositions[0].content,
            decoded.propositions[0].content
        );
        assert_eq!(graph.relations.len(), decoded.relations.len());
        assert_eq!(
            graph.relations[0].relation_type,
            decoded.relations[0].relation_type
        );
        assert_eq!(graph.source_agent_id, decoded.source_agent_id);
        assert_eq!(graph.timestamp, decoded.timestamp);
    }

    #[test]
    fn test_valid_graph_with_complex_relations() {
        let a = make_prop("A", "Foundation axiom", 1.0);
        let b = make_prop("B", "Derived theorem", 0.9);
        let c = make_prop("C", "Corollary", 0.85);

        let graph = PropositionGraph {
            propositions: vec![a, b, c],
            relations: vec![
                PropositionRelation {
                    from: make_pid("A"),
                    to: make_pid("B"),
                    relation_type: RelationType::Implies,
                    strength: 0.9,
                },
                PropositionRelation {
                    from: make_pid("B"),
                    to: make_pid("C"),
                    relation_type: RelationType::Supports,
                    strength: 0.8,
                },
            ],
            source_agent_id: "agent".to_string(),
            timestamp: 0,
        };

        let result = HybridLeanKernel::verify(&graph);
        assert!(result.passed);
        assert!(result.topology_valid);
        assert!(result.reference_valid);
        assert!(result.conflict_free);
    }
}
