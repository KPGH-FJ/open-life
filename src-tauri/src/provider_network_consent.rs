use crate::errors::AppError;
use crate::AppState;
use openlife_core::network_client::{
    resolve_network_policy_decision, NetworkPolicyDecision, NetworkPolicyDisposition,
};
use std::sync::Arc;

pub(crate) enum ProviderNetworkAuthorization {
    Authorized {
        network_policy: Box<openlife_core::config::NetworkPolicy>,
        network_policy_decision: NetworkPolicyDecision,
        permission_id: Option<String>,
        reviewed_permission:
            Option<Box<openlife_core::tool_permissions::ConsumedReviewedNetworkPermission>>,
    },
    ConsentRequired {
        proposal_id: String,
    },
    Denied {
        reason_code: String,
    },
}

pub(crate) enum ExplicitProviderProbeAuthorization {
    Authorized {
        grant: Box<openlife_core::network_client::ExplicitProviderProbeGrant>,
        effective_network_policy_decision_id: String,
        permission_id: Option<String>,
    },
    ConsentRequired {
        proposal_id: String,
    },
    Denied {
        reason_code: String,
    },
}

#[derive(Clone, Copy)]
pub(crate) struct NetworkConsentSubject<'a> {
    pub permission_source: &'a str,
    pub risk_level: &'a str,
    pub capabilities: &'a [&'a str],
    pub target: &'a str,
    pub affected_path_prefix: &'a str,
    pub blocked_action_type: &'a str,
    pub proposal_summary: &'a str,
}

impl<'a> NetworkConsentSubject<'a> {
    fn validate(self) -> Result<Self, AppError> {
        if [
            self.permission_source,
            self.risk_level,
            self.target,
            self.affected_path_prefix,
            self.blocked_action_type,
            self.proposal_summary,
        ]
        .iter()
        .any(|value| value.trim().is_empty())
            || !matches!(self.risk_level, "low" | "medium" | "high" | "critical")
            || self.capabilities.is_empty()
            || self
                .capabilities
                .iter()
                .any(|capability| capability.trim().is_empty())
        {
            return Err(AppError::internal(
                "network consent subject contains an empty authority field",
            ));
        }
        Ok(self)
    }
}

#[derive(Clone, Copy)]
pub(crate) enum NetworkConsentSubmissionScope {
    ExplicitCommand,
}

impl NetworkConsentSubmissionScope {
    fn required_proposal_id(&self) -> Option<&str> {
        None
    }
}

fn provider_network_endpoint_fingerprint(url: &str) -> (usize, String) {
    openlife_core::agent::metadata_safe::metadata_safe_value_digest(
        &serde_json::json!({ "endpoint": url }),
    )
}

fn provider_network_permission_scope(
    capability: &str,
    decision: &NetworkPolicyDecision,
    endpoint_digest: &str,
) -> String {
    format!(
        "{capability}@{}#endpoint:{endpoint_digest}",
        decision.decision_id
    )
}

fn bind_reviewed_host_to_ephemeral_policy(
    policy: &mut openlife_core::config::NetworkPolicy,
    decision: &NetworkPolicyDecision,
) -> Result<(), AppError> {
    let host = decision.host.trim().trim_end_matches('.');
    if host.is_empty() {
        return Err(AppError::permission(
            "accepted network consent did not retain an exact endpoint host",
        ));
    }
    if !policy
        .domain_allowlist
        .iter()
        .any(|rule| rule.trim().trim_end_matches('.').eq_ignore_ascii_case(host))
    {
        policy.domain_allowlist.push(host.to_ascii_lowercase());
    }
    Ok(())
}

// Consent staging binds endpoint, capability, subject, origin, and exact
// review scope independently so a proposal cannot authorize a broader edge.
async fn stage_network_consent(
    state: &Arc<AppState>,
    decision: &NetworkPolicyDecision,
    capability: &str,
    subject: NetworkConsentSubject<'_>,
    endpoint_length_bytes: usize,
    endpoint_digest: &str,
    submission_scope: NetworkConsentSubmissionScope,
) -> Result<String, AppError> {
    use openlife_core::agent::{
        AgentProposal, DurableWriteRequest, DurableWriteSource, DurableWriteSubject,
        ProposalSource, ProposalType, ReviewWorkflow, RiskLevel,
    };

    let subject = subject.validate()?;
    let proposal_risk_level = match subject.risk_level {
        "low" => RiskLevel::Low,
        "medium" => RiskLevel::Medium,
        "high" => RiskLevel::High,
        "critical" => RiskLevel::Critical,
        _ => unreachable!("validated network consent risk level"),
    };
    let permission_scope = provider_network_permission_scope(capability, decision, endpoint_digest);
    let after = serde_json::json!({
        "permission_action": "grant",
        "permission_scope_kind": "network_policy",
        "permission": "allow_once",
        "tool_name": permission_scope,
        "source": subject.permission_source,
        "risk_level": subject.risk_level,
        "action_type": "network",
        "capabilities": subject.capabilities,
        "canonical_scope": {
            "tool_name": permission_scope,
            "source": subject.permission_source,
            "risk_level": subject.risk_level,
            "action_type": "network",
            "network_capability": capability,
            "network_policy_decision_id": decision.decision_id,
            "endpoint_digest": endpoint_digest,
            "endpoint_length_bytes": endpoint_length_bytes,
        },
        "blocked_action": {
            "action_type": subject.blocked_action_type,
            "target": subject.target,
            "resolved_target": capability,
            "network_policy_decision_id": decision.decision_id,
            "endpoint_digest": endpoint_digest,
            "endpoint_length_bytes": endpoint_length_bytes,
        },
        "reason": decision.reason_code,
        "auto_generated": true,
        "directWritesExecuted": false,
    });
    let affected_path = format!("{}.{}", subject.affected_path_prefix, subject.target);
    let proposal_source = ProposalSource::NetworkConsent;
    let durable_source = DurableWriteSource::NetworkConsent;
    let mut proposal = AgentProposal::new(
        ProposalType::ToolPermission,
        &affected_path,
        after,
        subject.proposal_summary,
        1.0,
        proposal_risk_level,
        proposal_source,
    );
    proposal.source_detail = Some(format!(
        "{}_network_consent:{}",
        subject.permission_source, decision.decision_id
    ));
    let request = DurableWriteRequest::from_agent_proposal(
        durable_source,
        DurableWriteSubject::ToolPermission,
        proposal,
        "External network consent is pending Review Center approval.",
    )
    .with_evidence_refs(vec![
        format!("network_policy_decision:{}", decision.decision_id),
        format!("network_endpoint:{endpoint_digest}"),
    ]);
    let _ = submission_scope;
    let proposal_store = state
        .proposal_store
        .as_ref()
        .ok_or_else(|| AppError::internal("Proposal store not available"))?;
    let store = proposal_store.lock().await;
    ReviewWorkflow::new(&store)
        .submit(request)
        .map(|outcome| outcome.proposal_id().to_string())
        .map_err(|error| {
            AppError::internal(format!("stage external network consent failed: {error}"))
        })
}

// Provider authorization keeps the enforced policy decision and review scope
// explicit at the network dispatch boundary.
pub(crate) async fn authorize_provider_network_dispatch(
    state: &Arc<AppState>,
    network_policy: &openlife_core::config::NetworkPolicy,
    decision: &NetworkPolicyDecision,
    url: &str,
    capability: &str,
    provider: &str,
    submission_scope: NetworkConsentSubmissionScope,
) -> Result<ProviderNetworkAuthorization, AppError> {
    authorize_external_network_dispatch(
        state,
        network_policy,
        decision,
        url,
        capability,
        NetworkConsentSubject {
            permission_source: "provider",
            risk_level: "high",
            capabilities: &["network", "external_transmission"],
            target: provider,
            affected_path_prefix: "tool_permission.provider",
            blocked_action_type: "provider_dispatch",
            proposal_summary:
                "Allow one provider network retry after explicit Review Center approval.",
        },
        submission_scope,
    )
    .await
}

/// Govern and issue one opaque explicit-provider-probe grant.
///
/// The scheduler cannot create this grant from policy/decision strings.  This
/// function first applies ReviewWorkflow/AllowOnce governance, then the core
/// network layer re-resolves the exact effective decision and final URL before
/// issuing the consumed in-process capability.
pub(crate) async fn authorize_explicit_provider_probe(
    state: &Arc<AppState>,
    scheduler: &openlife_core::scheduler::InferenceScheduler,
    network_policy: &openlife_core::config::NetworkPolicy,
    decision: &NetworkPolicyDecision,
    url: &str,
    capability: &str,
    provider: &str,
) -> Result<ExplicitProviderProbeAuthorization, AppError> {
    match authorize_provider_network_dispatch(
        state,
        network_policy,
        decision,
        url,
        capability,
        provider,
        NetworkConsentSubmissionScope::ExplicitCommand,
    )
    .await?
    {
        ProviderNetworkAuthorization::Authorized {
            network_policy,
            network_policy_decision,
            permission_id,
            reviewed_permission,
        } => {
            let effective_network_policy_decision_id = network_policy_decision.decision_id.clone();
            let challenge = scheduler
                .explicit_provider_probe_challenge()
                .map_err(|error| {
                    AppError::permission(format!(
                        "explicit provider probe runtime identity rejected: {error}"
                    ))
                })?;
            let grant = {
                let permission_store = state.tool_permission_store.lock().await;
                permission_store
                    .issue_explicit_provider_probe_grant(
                        challenge,
                        *network_policy,
                        decision,
                        network_policy_decision,
                        reviewed_permission.map(|permission| *permission),
                    )
                    .map_err(|error| {
                        AppError::permission(format!(
                            "explicit provider probe governance rejected issuance: {error}"
                        ))
                    })?
            };
            Ok(ExplicitProviderProbeAuthorization::Authorized {
                grant: Box::new(grant),
                effective_network_policy_decision_id,
                permission_id,
            })
        }
        ProviderNetworkAuthorization::ConsentRequired { proposal_id } => {
            Ok(ExplicitProviderProbeAuthorization::ConsentRequired { proposal_id })
        }
        ProviderNetworkAuthorization::Denied { reason_code } => {
            Ok(ExplicitProviderProbeAuthorization::Denied { reason_code })
        }
    }
}

// External authorization uses the same explicit endpoint-bound contract as
// provider dispatch and must not accept a caller-shaped aggregate grant.
pub(crate) async fn authorize_external_network_dispatch(
    state: &Arc<AppState>,
    network_policy: &openlife_core::config::NetworkPolicy,
    decision: &NetworkPolicyDecision,
    url: &str,
    capability: &str,
    subject: NetworkConsentSubject<'_>,
    submission_scope: NetworkConsentSubmissionScope,
) -> Result<ProviderNetworkAuthorization, AppError> {
    let subject = subject.validate()?;
    match decision.disposition {
        NetworkPolicyDisposition::Allow => Ok(ProviderNetworkAuthorization::Authorized {
            network_policy: Box::new(network_policy.clone()),
            network_policy_decision: decision.clone(),
            permission_id: None,
            reviewed_permission: None,
        }),
        NetworkPolicyDisposition::Deny => Ok(ProviderNetworkAuthorization::Denied {
            reason_code: decision.reason_code.clone(),
        }),
        NetworkPolicyDisposition::Ask => {
            let (endpoint_length_bytes, endpoint_digest) =
                provider_network_endpoint_fingerprint(url);
            let permission_scope =
                provider_network_permission_scope(capability, decision, &endpoint_digest);
            let required_proposal_id = submission_scope.required_proposal_id();
            let reviewed_permission = {
                let store = state.tool_permission_store.lock().await;
                let consumed = if let Some(proposal_id) = required_proposal_id {
                    store.consume_reviewed_network_once_for_proposal(
                        proposal_id,
                        &permission_scope,
                        subject.permission_source,
                        subject.risk_level,
                        "network",
                    )
                } else {
                    store.consume_reviewed_network_once(
                        &permission_scope,
                        subject.permission_source,
                        subject.risk_level,
                        "network",
                    )
                };
                consumed.map_err(|error| {
                    AppError::internal(format!(
                        "consume reviewed external network permission failed: {error}"
                    ))
                })?
            };
            let Some(reviewed_permission) = reviewed_permission else {
                if required_proposal_id.is_some() {
                    return Err(AppError::permission(
                        "provider network continuation grant missing, mismatched, or already consumed",
                    ));
                }
                let proposal_id = stage_network_consent(
                    state,
                    decision,
                    capability,
                    subject,
                    endpoint_length_bytes,
                    &endpoint_digest,
                    submission_scope,
                )
                .await?;
                return Ok(ProviderNetworkAuthorization::ConsentRequired { proposal_id });
            };
            let permission_id = Some(reviewed_permission.permission_id().to_string());

            let mut effective_policy = network_policy.clone();
            effective_policy
                .tool_overrides
                .insert(capability.to_string(), "allow".into());
            // This policy exists only for the exact, proposal-bound retry. Bind
            // its reviewed hostname so macOS RFC 2544 fake-IP DNS can use the
            // already-configured loopback system proxy without weakening the
            // default private/reserved-address block or persisting a global
            // domain allowlist entry.
            bind_reviewed_host_to_ephemeral_policy(&mut effective_policy, decision)?;
            let effective_decision =
                resolve_network_policy_decision(&effective_policy, url, capability).map_err(
                    |error| {
                        AppError::internal(format!(
                            "resolve granted external network policy failed: {error}"
                        ))
                    },
                )?;
            if effective_decision.disposition != NetworkPolicyDisposition::Allow {
                return Err(AppError::permission(
                    "accepted network consent did not produce an allowed edge decision",
                ));
            }
            Ok(ProviderNetworkAuthorization::Authorized {
                network_policy: Box::new(effective_policy),
                network_policy_decision: effective_decision,
                permission_id,
                reviewed_permission: Some(Box::new(reviewed_permission)),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_network_permission_scope_is_endpoint_bound_and_metadata_safe() {
        let decision = NetworkPolicyDecision {
            decision_id: "network-policy:test".into(),
            disposition: NetworkPolicyDisposition::Ask,
            reason_code: "network_policy_consent_required".into(),
            capability: "provider.openai".into(),
            host: "api.example.test".into(),
            endpoint_digest: format!("sha256:{}", "0".repeat(64)),
        };
        let first_url = "https://api.example.test/v1/chat/completions";
        let second_url = "https://api.example.test/v2/responses";
        let (_, first_digest) = provider_network_endpoint_fingerprint(first_url);
        let (_, second_digest) = provider_network_endpoint_fingerprint(second_url);
        let first_scope =
            provider_network_permission_scope("provider.openai", &decision, &first_digest);
        let second_scope =
            provider_network_permission_scope("provider.openai", &decision, &second_digest);

        assert_ne!(first_scope, second_scope);
        assert!(!first_scope.contains(first_url));
        assert!(!second_scope.contains(second_url));
        assert!(first_scope.contains(&first_digest));
        assert!(second_scope.contains(&second_digest));
    }

    #[test]
    fn reviewed_host_is_bound_only_to_the_ephemeral_policy() {
        let original = openlife_core::config::NetworkPolicy {
            enabled: true,
            default_decision: "ask".into(),
            ..Default::default()
        };
        let decision = NetworkPolicyDecision {
            decision_id: "network-policy:test".into(),
            disposition: NetworkPolicyDisposition::Ask,
            reason_code: "network_policy_consent_required".into(),
            capability: "web.fetch".into(),
            host: "Example.COM.".into(),
            endpoint_digest: format!("sha256:{}", "0".repeat(64)),
        };
        let mut effective = original.clone();

        bind_reviewed_host_to_ephemeral_policy(&mut effective, &decision).unwrap();
        bind_reviewed_host_to_ephemeral_policy(&mut effective, &decision).unwrap();

        assert!(original.domain_allowlist.is_empty());
        assert_eq!(effective.domain_allowlist, ["example.com"]);
    }

    #[test]
    fn reviewed_host_binding_fails_closed_when_decision_lost_its_host() {
        let mut policy = openlife_core::config::NetworkPolicy::default();
        let decision = NetworkPolicyDecision {
            decision_id: "network-policy:test".into(),
            disposition: NetworkPolicyDisposition::Ask,
            reason_code: "network_policy_consent_required".into(),
            capability: "web.fetch".into(),
            host: "  ".into(),
            endpoint_digest: format!("sha256:{}", "0".repeat(64)),
        };

        let error = bind_reviewed_host_to_ephemeral_policy(&mut policy, &decision).unwrap_err();

        assert!(error
            .to_string()
            .contains("did not retain an exact endpoint host"));
        assert!(policy.domain_allowlist.is_empty());
    }
}
