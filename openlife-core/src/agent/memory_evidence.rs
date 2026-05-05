use crate::agent::types::{AgentProposal, ProposalSource, ProposalType};
use crate::life_model::risk;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Type of evidence derived from memory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceType {
    RepeatedPreference,
    RecurringGoal,
    CapabilitySignal,
    StateTrend,
    Contradiction,
    RelationshipUpdate,
    ValueSignal,
    Custom(String),
}

impl std::fmt::Display for EvidenceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvidenceType::RepeatedPreference => write!(f, "repeated_preference"),
            EvidenceType::RecurringGoal => write!(f, "recurring_goal"),
            EvidenceType::CapabilitySignal => write!(f, "capability_signal"),
            EvidenceType::StateTrend => write!(f, "state_trend"),
            EvidenceType::Contradiction => write!(f, "contradiction"),
            EvidenceType::RelationshipUpdate => write!(f, "relationship_update"),
            EvidenceType::ValueSignal => write!(f, "value_signal"),
            EvidenceType::Custom(s) => write!(f, "custom:{}", s),
        }
    }
}

/// A piece of evidence derived from one or more memory records.
/// Used as input to LifeModel evolution proposal generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEvidence {
    pub id: String,
    /// IDs of the memory records that support this evidence.
    pub memory_ids: Vec<String>,
    /// What kind of evidence this is.
    pub evidence_type: EvidenceType,
    /// The claim derived from the memories.
    pub claim: String,
    /// The LifeModel field path this evidence affects (dot-notation).
    pub affected_life_model_path: String,
    /// Confidence in the claim (0.0 - 1.0).
    pub confidence: f32,
    /// Recency score (higher = more recent/relevant, 0.0 - 1.0).
    pub recency_score: f32,
    /// IDs of other evidence that contradicts this claim.
    pub contradiction_ids: Vec<String>,
    /// Human-readable summary of the source memories.
    pub source_summary: String,
    /// When this evidence was created.
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl MemoryEvidence {
    /// Create new evidence from memory IDs and a claim.
    pub fn new(
        memory_ids: Vec<String>,
        evidence_type: EvidenceType,
        claim: impl Into<String>,
        affected_path: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            memory_ids,
            evidence_type,
            claim: claim.into(),
            affected_life_model_path: affected_path.into(),
            confidence: 0.5,
            recency_score: 0.5,
            contradiction_ids: Vec::new(),
            source_summary: String::new(),
            created_at: chrono::Utc::now(),
        }
    }

    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    pub fn with_recency(mut self, recency: f32) -> Self {
        self.recency_score = recency.clamp(0.0, 1.0);
        self
    }

    pub fn with_contradiction(mut self, contradiction_id: impl Into<String>) -> Self {
        self.contradiction_ids.push(contradiction_id.into());
        self
    }

    pub fn with_source_summary(mut self, summary: impl Into<String>) -> Self {
        self.source_summary = summary.into();
        self
    }

    /// Whether this evidence meets the minimum confidence threshold for proposal generation.
    pub fn is_confident(&self, threshold: f32) -> bool {
        self.confidence >= threshold && self.recency_score > 0.0
    }

    /// Whether this evidence contradicts other known evidence.
    pub fn has_contradictions(&self) -> bool {
        !self.contradiction_ids.is_empty()
    }

    /// Generate a LifeModel evolution proposal from this evidence.
    ///
    /// Returns `None` if the evidence is too weak (confidence < `min_confidence`)
    /// or if contradictions exist without sufficient resolution.
    ///
    /// High-risk fields (per LifeModel risk classifier) are always marked
    /// with a review-required note in the proposal reason.
    pub fn to_proposal(&self, min_confidence: f32) -> Option<AgentProposal> {
        if !self.is_confident(min_confidence) {
            return None;
        }

        let field_risk = risk::classify_field_risk(&self.affected_life_model_path);
        let proposal_type = self.infer_proposal_type();

        // Contradictions produce clarification/low-confidence path
        let (confidence, reason_prefix) = if self.has_contradictions() {
            (
                (self.confidence * 0.5).clamp(0.0, 0.5),
                format!(
                    "[需澄清] 存在矛盾证据 ({} 条)，请人工审核: ",
                    self.contradiction_ids.len()
                ),
            )
        } else {
            (self.confidence, String::new())
        };

        let reason = format!(
            "{}{} (证据类型: {}, 来源: {})",
            reason_prefix, self.claim, self.evidence_type, self.source_summary
        );

        let high_risk_note = if risk::requires_explicit_review(&self.affected_life_model_path) {
            "\n⚠️ 高风险字段，需要显式用户确认。"
        } else {
            ""
        };

        let proposal = AgentProposal::new(
            proposal_type,
            &self.affected_life_model_path,
            serde_json::json!({
                "evidence_id": self.id,
                "evidence_type": self.evidence_type.to_string(),
                "confidence": confidence,
                "recency_score": self.recency_score,
                "memory_ids": self.memory_ids,
                "contradiction_ids": self.contradiction_ids,
                "proposed_change": self.claim,
            }),
            &format!("{}{}", reason, high_risk_note),
            confidence,
            field_risk,
            ProposalSource::MemoryGovernance,
        );

        Some(proposal)
    }

    /// Infer the appropriate ProposalType from evidence type.
    fn infer_proposal_type(&self) -> ProposalType {
        match self.evidence_type {
            EvidenceType::RepeatedPreference => ProposalType::PreferenceUpdate,
            EvidenceType::RecurringGoal => ProposalType::GoalUpdate,
            EvidenceType::CapabilitySignal => ProposalType::CapabilityUpdate,
            EvidenceType::StateTrend => ProposalType::StateUpdate,
            EvidenceType::Contradiction => ProposalType::PreferenceUpdate,
            EvidenceType::RelationshipUpdate => ProposalType::PreferenceUpdate,
            EvidenceType::ValueSignal => ProposalType::LifeModelUpdate,
            EvidenceType::Custom(_) => ProposalType::LifeModelUpdate,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::types::RiskLevel;

    #[test]
    fn test_create_basic_evidence() {
        let evidence = MemoryEvidence::new(
            vec!["mem-001".into(), "mem-002".into()],
            EvidenceType::RepeatedPreference,
            "User consistently prefers morning deep work sessions",
            "preferences.work_style",
        );

        assert!(!evidence.id.is_empty());
        assert_eq!(evidence.memory_ids.len(), 2);
        assert_eq!(evidence.evidence_type, EvidenceType::RepeatedPreference);
        assert!(evidence.claim.contains("morning"));
        assert_eq!(evidence.affected_life_model_path, "preferences.work_style");
        assert_eq!(evidence.confidence, 0.5);
        assert_eq!(evidence.recency_score, 0.5);
    }

    #[test]
    fn test_confidence_clamped() {
        let evidence = MemoryEvidence::new(
            vec!["mem-001".into()],
            EvidenceType::StateTrend,
            "Energy levels improving",
            "state.energy",
        )
        .with_confidence(1.5)
        .with_recency(-0.5);

        assert_eq!(evidence.confidence, 1.0);
        assert_eq!(evidence.recency_score, 0.0);
    }

    #[test]
    fn test_contradiction_handling() {
        let e1 = MemoryEvidence::new(
            vec!["mem-001".into()],
            EvidenceType::ValueSignal,
            "User values creativity",
            "identity.values",
        );

        let e2 = MemoryEvidence::new(
            vec!["mem-002".into()],
            EvidenceType::ValueSignal,
            "User values structure over creativity",
            "identity.values",
        )
        .with_contradiction(&e1.id);

        assert!(e2.has_contradictions());
        assert_eq!(e2.contradiction_ids[0], e1.id);
        assert!(!e1.has_contradictions());
    }

    #[test]
    fn test_is_confident_threshold() {
        let evidence = MemoryEvidence::new(
            vec!["mem-001".into()],
            EvidenceType::RecurringGoal,
            "User wants to learn Rust",
            "goals.short_term",
        )
        .with_confidence(0.8)
        .with_recency(0.9);

        assert!(evidence.is_confident(0.7));
        assert!(!evidence.is_confident(0.9));

        let weak = MemoryEvidence::new(
            vec!["mem-002".into()],
            EvidenceType::RecurringGoal,
            "Maybe user wants to exercise",
            "goals.short_term",
        )
        .with_confidence(0.2)
        .with_recency(0.0);

        assert!(!weak.is_confident(0.5));
    }

    #[test]
    fn test_source_summary() {
        let evidence = MemoryEvidence::new(
            vec!["mem-a".into(), "mem-b".into()],
            EvidenceType::RelationshipUpdate,
            "User often mentions colleague Alice positively",
            "relationships.key_people",
        )
        .with_source_summary("From 5 chat conversations spanning 2 weeks");

        assert!(evidence.source_summary.contains("5 chat"));
    }

    #[test]
    fn test_all_evidence_types_display() {
        let types = vec![
            (EvidenceType::RepeatedPreference, "repeated_preference"),
            (EvidenceType::RecurringGoal, "recurring_goal"),
            (EvidenceType::CapabilitySignal, "capability_signal"),
            (EvidenceType::StateTrend, "state_trend"),
            (EvidenceType::Contradiction, "contradiction"),
            (EvidenceType::RelationshipUpdate, "relationship_update"),
            (EvidenceType::ValueSignal, "value_signal"),
            (
                EvidenceType::Custom("mood_pattern".into()),
                "custom:mood_pattern",
            ),
        ];
        for (etype, expected) in types {
            assert_eq!(etype.to_string(), expected);
        }
    }

    // ── P1-6: MemoryEvidence to Proposal tests ─────────────────────────

    #[test]
    fn test_repeated_preference_generates_proposal() {
        let evidence = MemoryEvidence::new(
            vec!["mem-001".into()],
            EvidenceType::RepeatedPreference,
            "User prefers visual over text-based learning",
            "preferences.learning_style",
        )
        .with_confidence(0.85)
        .with_recency(0.9)
        .with_source_summary("Mentioned in 4 recent chat sessions");

        let proposal = evidence.to_proposal(0.6).unwrap();
        assert_eq!(proposal.proposal_type, ProposalType::PreferenceUpdate);
        assert_eq!(proposal.affected_path, "preferences.learning_style");
        assert_eq!(proposal.source, ProposalSource::MemoryGovernance);
        assert!(proposal.reason.contains("visual"));
        // Evidence link in after payload
        assert!(proposal.after["evidence_id"].as_str().unwrap() == evidence.id);
    }

    #[test]
    fn test_low_confidence_evidence_returns_none() {
        let evidence = MemoryEvidence::new(
            vec!["mem-001".into()],
            EvidenceType::RecurringGoal,
            "User wants to exercise more",
            "goals.short_term",
        )
        .with_confidence(0.3)
        .with_recency(0.8);

        assert!(evidence.to_proposal(0.6).is_none());
    }

    #[test]
    fn test_high_risk_field_requires_explicit_review() {
        let evidence = MemoryEvidence::new(
            vec!["mem-001".into(), "mem-002".into()],
            EvidenceType::ValueSignal,
            "User shifted value from security to adventure",
            "identity.values",
        )
        .with_confidence(0.8)
        .with_recency(0.7)
        .with_source_summary("Repeated across 3 weeks of conversations");

        let proposal = evidence.to_proposal(0.6).unwrap();
        // High-risk fields are ProposalType::LifeModelUpdate with RiskLevel::High
        assert!(matches!(proposal.risk_level, RiskLevel::High));
        assert!(proposal.reason.contains("高风险字段"));
    }

    #[test]
    fn test_contradiction_produces_low_confidence() {
        let e1 = MemoryEvidence::new(
            vec!["mem-a".into()],
            EvidenceType::StateTrend,
            "Energy consistently high in mornings",
            "state.energy",
        );

        let e2 = MemoryEvidence::new(
            vec!["mem-b".into()],
            EvidenceType::StateTrend,
            "Energy crashes in mornings after late nights",
            "state.energy",
        )
        .with_confidence(0.8)
        .with_recency(0.9)
        .with_contradiction(&e1.id)
        .with_source_summary("One observation conflicts with history");

        let proposal = e2.to_proposal(0.6).unwrap();
        // Contradiction halves confidence
        assert!(proposal.confidence <= 0.5);
        assert!(proposal.reason.contains("需澄清"));
        assert!(proposal.reason.contains("矛盾"));
    }

    #[test]
    fn test_recurring_goal_evidence() {
        let evidence = MemoryEvidence::new(
            vec!["mem-001".into(), "mem-002".into(), "mem-003".into()],
            EvidenceType::RecurringGoal,
            "User repeatedly mentions wanting to learn piano",
            "goals.short_term",
        )
        .with_confidence(0.9)
        .with_recency(0.95)
        .with_source_summary("5 chat sessions over 2 months");

        let proposal = evidence.to_proposal(0.6).unwrap();
        assert_eq!(proposal.proposal_type, ProposalType::GoalUpdate);
        assert_eq!(proposal.affected_path, "goals.short_term");
        assert_eq!(proposal.source, ProposalSource::MemoryGovernance);
    }

    #[test]
    fn test_capability_signal_risk_classification() {
        let evidence = MemoryEvidence::new(
            vec!["mem-001".into()],
            EvidenceType::CapabilitySignal,
            "User completed a Rust project successfully",
            "capabilities.skills",
        )
        .with_confidence(0.75)
        .with_recency(0.8);

        let proposal = evidence.to_proposal(0.6).unwrap();
        // capabilities are medium risk
        assert!(matches!(proposal.risk_level, RiskLevel::Medium));
    }
}
