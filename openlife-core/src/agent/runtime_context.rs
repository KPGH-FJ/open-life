//! Typed context candidates shared by the canonical Chat and Work runtimes.
//!
//! These records describe bounded context selected for one provider request.
//! They are not an intent classifier, policy decision, or lifecycle owner.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextSourceKind {
    StableCore,
    RuntimePolicy,
    PolicyDisposition,
    SessionState,
    SelectedPersonalContext,
    ToolManifest,
    MaterializedFile,
    WorkspaceInstruction,
    SkillMetadata,
    SkillInstruction,
    Observation,
    LifeModelContext,
    HsSummary,
    LifeModelYaml,
    RawMemorySnippet,
}

impl ContextSourceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::StableCore => "stable_core",
            Self::RuntimePolicy => "runtime_policy",
            Self::PolicyDisposition => "policy_disposition",
            Self::SessionState => "session_state",
            Self::SelectedPersonalContext => "selected_personal_context",
            Self::ToolManifest => "tool_manifest",
            Self::MaterializedFile => "materialized_file",
            Self::WorkspaceInstruction => "workspace_instruction",
            Self::SkillMetadata => "skill_metadata",
            Self::SkillInstruction => "skill_instruction",
            Self::Observation => "observation",
            Self::LifeModelContext => "life_model_context",
            Self::HsSummary => "hs_summary",
            Self::LifeModelYaml => "life_model_yaml",
            Self::RawMemorySnippet => "raw_memory_snippet",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextSourceCandidate {
    pub source_kind: ContextSourceKind,
    pub source_id: String,
    pub content: String,
    pub inclusion_reason: String,
    pub privacy_class: String,
    pub token_estimate: u32,
    #[serde(default)]
    pub selected_skill_id: Option<String>,
}

impl ContextSourceCandidate {
    pub fn new(
        source_kind: ContextSourceKind,
        source_id: impl Into<String>,
        content: impl Into<String>,
        inclusion_reason: impl Into<String>,
        privacy_class: impl Into<String>,
        token_estimate: u32,
    ) -> Self {
        Self {
            source_kind,
            source_id: source_id.into(),
            content: content.into(),
            inclusion_reason: inclusion_reason.into(),
            privacy_class: privacy_class.into(),
            token_estimate,
            selected_skill_id: None,
        }
    }

    pub fn for_skill(mut self, skill_id: impl Into<String>) -> Self {
        self.selected_skill_id = Some(skill_id.into());
        self
    }
}
