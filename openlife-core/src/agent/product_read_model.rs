use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewModelStatus {
    Loading,
    Ready,
    Empty,
    Error,
    Stale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViewModelSource {
    #[serde(rename = "backend-readmodel")]
    BackendReadModel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EvidenceSource {
    #[serde(rename = "backend-readmodel")]
    BackendReadModel,
    #[serde(rename = "audit")]
    Audit,
    #[serde(rename = "task")]
    Task,
    #[serde(rename = "review")]
    Review,
    #[serde(rename = "memory")]
    Memory,
    #[serde(rename = "lifemodel")]
    LifeModel,
    #[serde(rename = "settings")]
    Settings,
    #[serde(rename = "provider")]
    Provider,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceSensitivity {
    Public,
    LocalPrivate,
    Sensitive,
    Redacted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceRef {
    pub id: String,
    pub label: String,
    pub source: EvidenceSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sensitivity: Option<EvidenceSensitivity>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewModelWarningSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewModelWarning {
    pub code: String,
    pub message: String,
    pub severity: ViewModelWarningSeverity,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_refs: Vec<EvidenceRef>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductActionKind {
    Open,
    Start,
    Continue,
    Retry,
    Cancel,
    Refresh,
    Inspect,
    Configure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductAction {
    pub id: String,
    pub label: String,
    pub kind: ProductActionKind,
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_ref: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewItemMaterializationStatus {
    NotApplicable,
    NotStarted,
    Applying,
    Applied,
    Failed,
    RolledBack,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewActionKind {
    Approve,
    Reject,
    Edit,
    Later,
    Revoke,
    Apply,
    ViewEvidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewActionEffect {
    DecisionOnly,
    MaterializationRequest,
    EvidenceOnly,
}

impl ReviewActionKind {
    pub fn expected_effect(self) -> ReviewActionEffect {
        match self {
            Self::Approve | Self::Reject | Self::Edit | Self::Later | Self::Revoke => {
                ReviewActionEffect::DecisionOnly
            }
            Self::Apply => ReviewActionEffect::MaterializationRequest,
            Self::ViewEvidence => ReviewActionEffect::EvidenceOnly,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewAction {
    pub id: String,
    pub label: String,
    pub kind: ReviewActionKind,
    pub effect: ReviewActionEffect,
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_reason: Option<String>,
    #[serde(default)]
    pub requires_confirmation: bool,
    pub target_review_item_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_materialization_status_after_dispatch: Option<ReviewItemMaterializationStatus>,
    #[serde(default)]
    pub completion_proof_after_dispatch: bool,
}

impl ReviewAction {
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        kind: ReviewActionKind,
        target_review_item_id: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            kind,
            effect: kind.expected_effect(),
            enabled: true,
            disabled_reason: None,
            requires_confirmation: false,
            target_review_item_id: target_review_item_id.into(),
            expected_materialization_status_after_dispatch: None,
            completion_proof_after_dispatch: false,
        }
    }

    pub fn validate(&self) -> Result<(), ProductReadModelContractError> {
        let expected = self.kind.expected_effect();
        if self.effect != expected {
            return Err(ProductReadModelContractError::ReviewActionEffectMismatch {
                kind: self.kind,
                expected,
                actual: self.effect,
            });
        }
        for (field, value) in [
            ("id", self.id.as_str()),
            ("label", self.label.as_str()),
            ("targetReviewItemId", self.target_review_item_id.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(
                    ProductReadModelContractError::ReviewActionRequiredFieldMissing { field },
                );
            }
        }
        if self.enabled && self.disabled_reason.is_some() {
            return Err(
                ProductReadModelContractError::EnabledReviewActionHasDisabledReason {
                    id: self.id.clone(),
                },
            );
        }
        if !self.enabled
            && self
                .disabled_reason
                .as_deref()
                .is_none_or(|reason| reason.trim().is_empty())
        {
            return Err(
                ProductReadModelContractError::DisabledReviewActionMissingReason {
                    id: self.id.clone(),
                },
            );
        }
        // Clicking an enabled Approve action on an already-rendered ReviewItem
        // is itself the user's decision. `requires_confirmation` means an
        // additional modal is necessary; it is not a universal prerequisite
        // for every approval. Apply remains a separate consequential action.
        if self.kind == ReviewActionKind::Apply && !self.requires_confirmation {
            return Err(
                ProductReadModelContractError::ReviewActionConfirmationRequired {
                    id: self.id.clone(),
                    kind: self.kind,
                },
            );
        }
        if self.completion_proof_after_dispatch {
            return Err(
                ProductReadModelContractError::ReviewActionClaimsCompletionProof {
                    id: self.id.clone(),
                },
            );
        }
        Ok(())
    }

    pub fn disabled(mut self, reason: impl Into<String>) -> Self {
        self.enabled = false;
        self.disabled_reason = Some(reason.into());
        self
    }

    pub fn requiring_confirmation(mut self) -> Self {
        self.requires_confirmation = true;
        self
    }

    pub fn with_expected_materialization_status(
        mut self,
        status: ReviewItemMaterializationStatus,
    ) -> Self {
        self.expected_materialization_status_after_dispatch = Some(status);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DebugActionKind {
    RawTrace,
    RawJson,
    Export,
    ProviderHealth,
    RouteEvidence,
    Transcript,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugAction {
    pub id: String,
    pub label: String,
    pub kind: DebugActionKind,
    pub enabled: bool,
    #[serde(default)]
    pub developer_only: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ViewModelActions {
    #[serde(default)]
    pub primary: Vec<ProductAction>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub review: Vec<ReviewAction>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub debug_only: Vec<DebugAction>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewModelEnvelope<T> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    pub status: ViewModelStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_updated_at: Option<String>,
    pub source: ViewModelSource,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_refs: Vec<EvidenceRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<ViewModelWarning>,
    pub actions: ViewModelActions,
}

impl<T> ViewModelEnvelope<T> {
    pub fn backend_read_model(status: ViewModelStatus, data: Option<T>) -> Self {
        Self {
            data,
            status,
            last_updated_at: None,
            source: ViewModelSource::BackendReadModel,
            evidence_refs: Vec::new(),
            warnings: Vec::new(),
            actions: ViewModelActions::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductRiskLevel {
    None,
    Low,
    Medium,
    High,
    Critical,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderRouteType {
    Local,
    Cloud,
    Hybrid,
    Auto,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalTransmissionStatus {
    NotSent,
    Sent,
    Possible,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderPrivacyBoundarySummary {
    pub route_type: ProviderRouteType,
    pub external_transmission: ExternalTransmissionStatus,
    pub provider_label: String,
    pub model_label: String,
    pub privacy_label: String,
    pub risk: ProductRiskLevel,
    pub local_only_required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked_reason: Option<String>,
    #[serde(default)]
    pub evidence_refs: Vec<EvidenceRef>,
}

impl ProviderPrivacyBoundarySummary {
    pub fn unknown() -> Self {
        Self {
            route_type: ProviderRouteType::Unknown,
            external_transmission: ExternalTransmissionStatus::Unknown,
            provider_label: "provider unknown".into(),
            model_label: "model unknown".into(),
            privacy_label: "privacy boundary unknown".into(),
            risk: ProductRiskLevel::Unknown,
            local_only_required: false,
            blocked_reason: None,
            evidence_refs: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackendEntityKind {
    #[serde(rename = "task")]
    Task,
    #[serde(rename = "run")]
    Run,
    #[serde(rename = "conversation")]
    Conversation,
    #[serde(rename = "review_item")]
    ReviewItem,
    #[serde(rename = "memory")]
    Memory,
    #[serde(rename = "lifemodel")]
    LifeModel,
    #[serde(rename = "proposal")]
    Proposal,
    #[serde(rename = "tool_permission")]
    ToolPermission,
    #[serde(rename = "evidence")]
    Evidence,
    #[serde(rename = "external_resource")]
    ExternalResource,
    #[serde(rename = "schedule")]
    Schedule,
    #[serde(rename = "policy")]
    Policy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendEntityRef {
    pub id: String,
    pub kind: BackendEntityKind,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProductReadModelContractError {
    ReviewActionEffectMismatch {
        kind: ReviewActionKind,
        expected: ReviewActionEffect,
        actual: ReviewActionEffect,
    },
    ReviewActionRequiredFieldMissing {
        field: &'static str,
    },
    EnabledReviewActionHasDisabledReason {
        id: String,
    },
    DisabledReviewActionMissingReason {
        id: String,
    },
    ReviewActionConfirmationRequired {
        id: String,
        kind: ReviewActionKind,
    },
    ReviewActionClaimsCompletionProof {
        id: String,
    },
}

impl std::fmt::Display for ProductReadModelContractError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReviewActionEffectMismatch {
                kind,
                expected,
                actual,
            } => write!(
                f,
                "review action {:?} must use effect {:?}, got {:?}",
                kind, expected, actual
            ),
            Self::ReviewActionRequiredFieldMissing { field } => {
                write!(f, "review action required field is missing: {field}")
            }
            Self::EnabledReviewActionHasDisabledReason { id } => {
                write!(f, "enabled review action {id} cannot carry disabledReason")
            }
            Self::DisabledReviewActionMissingReason { id } => {
                write!(f, "disabled review action {id} requires disabledReason")
            }
            Self::ReviewActionConfirmationRequired { id, kind } => write!(
                f,
                "review action {id} with kind {kind:?} requires confirmation"
            ),
            Self::ReviewActionClaimsCompletionProof { id } => write!(
                f,
                "review action {id} cannot claim completion proof after dispatch"
            ),
        }
    }
}

impl std::error::Error for ProductReadModelContractError {}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn product_read_model_envelope_serializes_backend_owned_contract() {
        let mut envelope = ViewModelEnvelope::backend_read_model(ViewModelStatus::Ready, Some(7));
        envelope.last_updated_at = Some("2026-07-09T00:00:00Z".into());
        envelope.evidence_refs.push(EvidenceRef {
            id: "evidence-1".into(),
            label: "Backend projection".into(),
            source: EvidenceSource::BackendReadModel,
            sensitivity: Some(EvidenceSensitivity::LocalPrivate),
        });
        envelope.actions.primary.push(ProductAction {
            id: "refresh".into(),
            label: "Refresh".into(),
            kind: ProductActionKind::Refresh,
            enabled: true,
            disabled_reason: None,
            target_ref: Some("surface:today".into()),
        });

        let value = serde_json::to_value(&envelope).expect("serialize envelope");
        assert_eq!(value["source"], json!("backend-readmodel"));
        assert_eq!(value["status"], json!("ready"));
        assert_eq!(value["lastUpdatedAt"], json!("2026-07-09T00:00:00Z"));
        assert_eq!(
            value["evidenceRefs"][0]["source"],
            json!("backend-readmodel")
        );
        assert_eq!(
            value["evidenceRefs"][0]["sensitivity"],
            json!("local_private")
        );
        assert_eq!(value["actions"]["primary"][0]["kind"], json!("refresh"));
        assert_eq!(
            value["actions"]["primary"][0]["targetRef"],
            json!("surface:today")
        );
    }

    #[test]
    fn product_read_model_review_action_effect_invariant_accepts_valid_actions() {
        for (kind, effect) in [
            (ReviewActionKind::Approve, ReviewActionEffect::DecisionOnly),
            (ReviewActionKind::Reject, ReviewActionEffect::DecisionOnly),
            (ReviewActionKind::Edit, ReviewActionEffect::DecisionOnly),
            (ReviewActionKind::Later, ReviewActionEffect::DecisionOnly),
            (ReviewActionKind::Revoke, ReviewActionEffect::DecisionOnly),
            (
                ReviewActionKind::Apply,
                ReviewActionEffect::MaterializationRequest,
            ),
            (
                ReviewActionKind::ViewEvidence,
                ReviewActionEffect::EvidenceOnly,
            ),
        ] {
            let mut action = ReviewAction::new("action", "Action", kind, "review:item");
            if matches!(kind, ReviewActionKind::Approve | ReviewActionKind::Apply) {
                action = action.requiring_confirmation();
            }
            assert_eq!(action.effect, effect);
            action.validate().expect("valid action invariant");
        }
    }

    #[test]
    fn product_read_model_review_action_effect_invariant_rejects_mismatches() {
        let mut action =
            ReviewAction::new("apply", "Apply", ReviewActionKind::Apply, "review:item")
                .requiring_confirmation();
        action.effect = ReviewActionEffect::DecisionOnly;

        let err = action
            .validate()
            .expect_err("apply must not be decision-only");
        assert_eq!(
            err,
            ProductReadModelContractError::ReviewActionEffectMismatch {
                kind: ReviewActionKind::Apply,
                expected: ReviewActionEffect::MaterializationRequest,
                actual: ReviewActionEffect::DecisionOnly,
            }
        );
    }

    #[test]
    fn product_read_model_review_action_rejects_completion_claims_and_fake_disabled_states() {
        let mut completion = ReviewAction::new(
            "approve",
            "Approve",
            ReviewActionKind::Approve,
            "review:item",
        )
        .requiring_confirmation();
        completion.completion_proof_after_dispatch = true;
        assert_eq!(
            completion.validate(),
            Err(
                ProductReadModelContractError::ReviewActionClaimsCompletionProof {
                    id: "approve".into()
                }
            )
        );

        let mut disabled =
            ReviewAction::new("reject", "Reject", ReviewActionKind::Reject, "review:item");
        disabled.enabled = false;
        assert_eq!(
            disabled.validate(),
            Err(
                ProductReadModelContractError::DisabledReviewActionMissingReason {
                    id: "reject".into()
                }
            )
        );
    }

    #[test]
    fn product_read_model_provider_privacy_unknown_fails_closed() {
        let summary = ProviderPrivacyBoundarySummary::unknown();
        let value = serde_json::to_value(&summary).expect("serialize provider boundary");

        assert_eq!(value["routeType"], json!("unknown"));
        assert_eq!(value["externalTransmission"], json!("unknown"));
        assert_eq!(value["risk"], json!("unknown"));
        assert_eq!(value["localOnlyRequired"], json!(false));
    }
}
