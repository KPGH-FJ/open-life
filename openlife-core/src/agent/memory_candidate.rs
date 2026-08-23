use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryCandidateKind {
    EpisodicLifeEvent,
    SemanticUserFact,
    ProceduralRule,
    Preference,
    IdentityOrRole,
}

impl MemoryCandidateKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::EpisodicLifeEvent => "episodic_life_event",
            Self::SemanticUserFact => "semantic_user_fact",
            Self::ProceduralRule => "procedural_rule",
            Self::Preference => "preference",
            Self::IdentityOrRole => "identity_or_role",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryDestination {
    MemoryProposal,
    LifeModelProposal,
}

impl MemoryDestination {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MemoryProposal => "memory_proposal",
            Self::LifeModelProposal => "life_model_proposal",
        }
    }
}

/// A typed, source-bound candidate emitted by the model-driven personal
/// intelligence lane. This type carries no natural-language intent parser.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryCandidate {
    pub candidate_id: String,
    pub source_span_id: String,
    pub kind: MemoryCandidateKind,
    pub destination: MemoryDestination,
    pub evidence_text: String,
    pub source_preview: String,
    pub normalized_claim: String,
    pub sensitivity: String,
    pub stability: String,
    pub explicitness: String,
    pub future_actionability: String,
    pub confidence: f32,
    pub reason_codes: Vec<String>,
}
