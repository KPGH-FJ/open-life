use crate::llm::{
    ProviderPolicyAuthorization, ProviderPolicyProvenanceKind, ProviderPolicyProvenanceRef,
};

/// Narrow, typed Policy result consumed at provider and governed-action
/// boundaries. It contains no prompt, heuristic, LifeModel, or execution
/// lifecycle state.
#[derive(Debug, Clone)]
pub struct RuntimePolicyContext {
    provider_authorization: ProviderPolicyAuthorization,
    policy_provenance_refs: Vec<ProviderPolicyProvenanceRef>,
    external_write_requires_proposal: bool,
}

impl RuntimePolicyContext {
    pub fn new(
        provider_authorization: ProviderPolicyAuthorization,
        mut policy_provenance_refs: Vec<ProviderPolicyProvenanceRef>,
        external_write_requires_proposal: bool,
    ) -> Self {
        policy_provenance_refs.sort();
        policy_provenance_refs.dedup();
        Self {
            provider_authorization,
            policy_provenance_refs,
            external_write_requires_proposal,
        }
    }

    pub fn fail_closed() -> Self {
        let authorization = ProviderPolicyAuthorization::local_only_fail_closed(
            crate::llm::ProviderLocalOnlyReason::MissingCanonicalPolicy,
        );
        let route_digest = crate::agent::metadata_safe::metadata_safe_text_digest(&format!(
            "{}:{}:{:?}",
            authorization.decision_id(),
            authorization.policy_version(),
            authorization.data_route(),
        ))
        .1;
        let provenance = vec![ProviderPolicyProvenanceRef::new(
            ProviderPolicyProvenanceKind::FailClosedRouteDecision,
            authorization.decision_id(),
            route_digest,
        )];
        Self::new(authorization, provenance, true)
    }

    pub fn from_scheduled_claim(claim: &crate::tasks::ScheduledTaskClaim) -> anyhow::Result<Self> {
        let authorization = ProviderPolicyAuthorization::from_scheduled_claim(claim)?;
        let provenance = vec![ProviderPolicyProvenanceRef::new(
            ProviderPolicyProvenanceKind::ScheduledRouteDecision,
            authorization.decision_id(),
            &claim.provider_grant().policy_decision_digest,
        )];
        Ok(Self::new(authorization, provenance, true))
    }

    pub fn provider_authorization(&self) -> &ProviderPolicyAuthorization {
        &self.provider_authorization
    }

    pub fn policy_provenance_refs(&self) -> &[ProviderPolicyProvenanceRef] {
        &self.policy_provenance_refs
    }

    pub fn external_write_requires_proposal(&self) -> bool {
        self.external_write_requires_proposal
    }
}
