use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EpochEndReason {
    NoveltyPlateau,
    EvidencePlateau,
    DreamCoverageReached,
    ContradictionRiskHigh,
    ToolLoop,
    BudgetExhausted,
    HumanInterrupted,
}

impl EpochEndReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NoveltyPlateau => "novelty_plateau",
            Self::EvidencePlateau => "evidence_plateau",
            Self::DreamCoverageReached => "dream_coverage_reached",
            Self::ContradictionRiskHigh => "contradiction_risk_high",
            Self::ToolLoop => "tool_loop",
            Self::BudgetExhausted => "budget_exhausted",
            Self::HumanInterrupted => "human_interrupted",
        }
    }
}

impl fmt::Display for EpochEndReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct DreamAuditSnapshot {
    pub community_coverage: f32,
    pub salience_shift: f32,
    pub contradiction_risk: f32,
}

impl Default for DreamAuditSnapshot {
    fn default() -> Self {
        Self {
            community_coverage: 0.0,
            salience_shift: 0.0,
            contradiction_risk: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_end_reason_serializes_as_snake_case() {
        let value = serde_json::to_value(EpochEndReason::DreamCoverageReached).unwrap();
        assert_eq!(value, serde_json::json!("dream_coverage_reached"));
    }
}
