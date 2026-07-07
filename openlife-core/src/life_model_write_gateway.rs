use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifeModelWriteIntentKind {
    AcceptedProposalMaterialization,
    ManualOverride,
    RestoreImportOverride,
    SourceDataCompatibility,
    AutomaticLearning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifeModelWriteGatewayStatus {
    Allowed,
    StaleConflict,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifeModelWriteGatewayRequest {
    pub intent: LifeModelWriteIntentKind,
    pub proposal_id: Option<String>,
    pub run_id: Option<String>,
    pub evidence_id: Option<String>,
    pub base_hash: Option<String>,
    pub current_hash: Option<String>,
    pub before_hash: Option<String>,
    pub after_hash: Option<String>,
    pub explicit_manual_override: bool,
    pub risk_acknowledged: bool,
}

impl LifeModelWriteGatewayRequest {
    pub fn accepted_proposal(
        proposal_id: impl Into<String>,
        run_id: Option<String>,
        evidence_id: Option<String>,
        base_hash: Option<String>,
        current_hash: Option<String>,
        before_hash: impl Into<String>,
        after_hash: impl Into<String>,
    ) -> Self {
        Self {
            intent: LifeModelWriteIntentKind::AcceptedProposalMaterialization,
            proposal_id: Some(proposal_id.into()),
            run_id,
            evidence_id,
            base_hash,
            current_hash,
            before_hash: Some(before_hash.into()),
            after_hash: Some(after_hash.into()),
            explicit_manual_override: false,
            risk_acknowledged: false,
        }
    }

    pub fn manual_override(before_hash: impl Into<String>, after_hash: impl Into<String>) -> Self {
        Self {
            intent: LifeModelWriteIntentKind::ManualOverride,
            proposal_id: None,
            run_id: None,
            evidence_id: None,
            base_hash: None,
            current_hash: None,
            before_hash: Some(before_hash.into()),
            after_hash: Some(after_hash.into()),
            explicit_manual_override: true,
            risk_acknowledged: true,
        }
    }

    pub fn automatic_learning() -> Self {
        Self {
            intent: LifeModelWriteIntentKind::AutomaticLearning,
            proposal_id: None,
            run_id: None,
            evidence_id: None,
            base_hash: None,
            current_hash: None,
            before_hash: None,
            after_hash: None,
            explicit_manual_override: false,
            risk_acknowledged: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LifeModelWriteGatewayDecision {
    pub status: LifeModelWriteGatewayStatus,
    pub allowed: bool,
    pub conflict_status: Option<String>,
    pub lane: String,
    pub proposal_id: Option<String>,
    pub run_id: Option<String>,
    pub evidence_id: Option<String>,
    pub base_hash: Option<String>,
    pub current_hash: Option<String>,
    pub before_hash: Option<String>,
    pub after_hash: Option<String>,
    pub reason_code: String,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct LifeModelWriteGateway;

impl LifeModelWriteGateway {
    pub fn decide(request: LifeModelWriteGatewayRequest) -> LifeModelWriteGatewayDecision {
        match request.intent {
            LifeModelWriteIntentKind::AcceptedProposalMaterialization => {
                if request
                    .proposal_id
                    .as_deref()
                    .unwrap_or("")
                    .trim()
                    .is_empty()
                {
                    return Self::blocked(request, "accepted_proposal_missing_proposal_id");
                }
                let base = request.base_hash.as_deref().unwrap_or("").trim();
                let current = request.current_hash.as_deref().unwrap_or("").trim();
                if base.is_empty() || current.is_empty() {
                    return Self::blocked(
                        request,
                        "accepted_proposal_missing_base_or_current_hash",
                    );
                }
                if base != current {
                    return Self::stale(request, "accepted_proposal_base_hash_stale");
                }
                if request
                    .before_hash
                    .as_deref()
                    .unwrap_or("")
                    .trim()
                    .is_empty()
                    || request
                        .after_hash
                        .as_deref()
                        .unwrap_or("")
                        .trim()
                        .is_empty()
                {
                    return Self::blocked(request, "accepted_proposal_missing_before_after_hash");
                }
                Self::allowed(request, "accepted_proposal_materialization_allowed")
            }
            LifeModelWriteIntentKind::ManualOverride
            | LifeModelWriteIntentKind::RestoreImportOverride => {
                if request.explicit_manual_override && request.risk_acknowledged {
                    Self::allowed(request, "governed_manual_override_allowed")
                } else {
                    Self::blocked(request, "manual_override_missing_governance")
                }
            }
            LifeModelWriteIntentKind::SourceDataCompatibility => {
                Self::allowed(request, "source_data_compatibility_allowed_not_truth")
            }
            LifeModelWriteIntentKind::AutomaticLearning => Self::blocked(
                request,
                "automatic_learning_cannot_write_canonical_lifemodel",
            ),
        }
    }

    fn allowed(
        request: LifeModelWriteGatewayRequest,
        reason_code: impl Into<String>,
    ) -> LifeModelWriteGatewayDecision {
        LifeModelWriteGatewayDecision {
            status: LifeModelWriteGatewayStatus::Allowed,
            allowed: true,
            conflict_status: None,
            lane: "canonical_lifemodel_truth".into(),
            proposal_id: request.proposal_id,
            run_id: request.run_id,
            evidence_id: request.evidence_id,
            base_hash: request.base_hash,
            current_hash: request.current_hash,
            before_hash: request.before_hash,
            after_hash: request.after_hash,
            reason_code: reason_code.into(),
        }
    }

    fn stale(
        request: LifeModelWriteGatewayRequest,
        reason_code: impl Into<String>,
    ) -> LifeModelWriteGatewayDecision {
        LifeModelWriteGatewayDecision {
            status: LifeModelWriteGatewayStatus::StaleConflict,
            allowed: false,
            conflict_status: Some("stale_base".into()),
            lane: "canonical_lifemodel_truth".into(),
            proposal_id: request.proposal_id,
            run_id: request.run_id,
            evidence_id: request.evidence_id,
            base_hash: request.base_hash,
            current_hash: request.current_hash,
            before_hash: request.before_hash,
            after_hash: request.after_hash,
            reason_code: reason_code.into(),
        }
    }

    fn blocked(
        request: LifeModelWriteGatewayRequest,
        reason_code: impl Into<String>,
    ) -> LifeModelWriteGatewayDecision {
        LifeModelWriteGatewayDecision {
            status: LifeModelWriteGatewayStatus::Blocked,
            allowed: false,
            conflict_status: None,
            lane: "canonical_lifemodel_truth".into(),
            proposal_id: request.proposal_id,
            run_id: request.run_id,
            evidence_id: request.evidence_id,
            base_hash: request.base_hash,
            current_hash: request.current_hash,
            before_hash: request.before_hash,
            after_hash: request.after_hash,
            reason_code: reason_code.into(),
        }
    }
}

#[cfg(test)]
mod life_model_write_gateway_tests {
    use super::*;

    #[test]
    fn life_model_write_gateway_accepts_proposal_materialization() {
        let request = LifeModelWriteGatewayRequest::accepted_proposal(
            "proposal-1",
            Some("run-1".into()),
            Some("evidence-1".into()),
            Some("hash:before".into()),
            Some("hash:before".into()),
            "hash:before",
            "hash:after",
        );

        let decision = LifeModelWriteGateway::decide(request);

        assert_eq!(decision.status, LifeModelWriteGatewayStatus::Allowed);
        assert!(decision.allowed);
        assert_eq!(decision.lane, "canonical_lifemodel_truth");
        assert_eq!(decision.proposal_id.as_deref(), Some("proposal-1"));
        assert_eq!(decision.run_id.as_deref(), Some("run-1"));
        assert_eq!(decision.evidence_id.as_deref(), Some("evidence-1"));
        assert_eq!(decision.base_hash.as_deref(), Some("hash:before"));
        assert_eq!(decision.current_hash.as_deref(), Some("hash:before"));
    }

    #[test]
    fn life_model_write_gateway_rejects_stale_proposal_base() {
        let request = LifeModelWriteGatewayRequest::accepted_proposal(
            "proposal-1",
            Some("run-1".into()),
            Some("evidence-1".into()),
            Some("hash:old".into()),
            Some("hash:current".into()),
            "hash:old",
            "hash:after",
        );

        let decision = LifeModelWriteGateway::decide(request);

        assert_eq!(decision.status, LifeModelWriteGatewayStatus::StaleConflict);
        assert!(!decision.allowed);
        assert_eq!(decision.conflict_status.as_deref(), Some("stale_base"));
        assert_eq!(decision.base_hash.as_deref(), Some("hash:old"));
        assert_eq!(decision.current_hash.as_deref(), Some("hash:current"));
    }

    #[test]
    fn life_model_write_gateway_requires_real_proposal_base_hash() {
        let request = LifeModelWriteGatewayRequest::accepted_proposal(
            "proposal-1",
            Some("run-1".into()),
            Some("evidence-1".into()),
            None,
            Some("hash:current".into()),
            "hash:current",
            "hash:after",
        );

        let decision = LifeModelWriteGateway::decide(request);

        assert_eq!(decision.status, LifeModelWriteGatewayStatus::Blocked);
        assert!(!decision.allowed);
        assert_eq!(
            decision.reason_code,
            "accepted_proposal_missing_base_or_current_hash"
        );
    }

    #[test]
    fn life_model_write_gateway_allows_governed_manual_override() {
        let decision = LifeModelWriteGateway::decide(
            LifeModelWriteGatewayRequest::manual_override("hash:before", "hash:after"),
        );

        assert_eq!(decision.status, LifeModelWriteGatewayStatus::Allowed);
        assert!(decision.allowed);
    }

    #[test]
    fn life_model_write_gateway_blocks_automatic_learning_to_canonical_truth() {
        let decision =
            LifeModelWriteGateway::decide(LifeModelWriteGatewayRequest::automatic_learning());

        assert_eq!(decision.status, LifeModelWriteGatewayStatus::Blocked);
        assert!(!decision.allowed);
    }
}
