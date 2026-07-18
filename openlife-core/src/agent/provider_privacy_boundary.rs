use crate::agent::product_read_model::{
    EvidenceRef, ExternalTransmissionStatus, ProductRiskLevel, ProviderPrivacyBoundarySummary,
    ProviderRouteType,
};
use crate::network_client::{NetworkPolicyDecision, NetworkPolicyDisposition};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderPrivacyBoundaryBuildInput {
    pub prefer_local_model: bool,
    pub local_model_label: Option<String>,
    pub cloud_provider_label: Option<String>,
    pub cloud_model_label: Option<String>,
    pub cloud_api_configured: bool,
    pub provider_validation_status: Option<String>,
    pub provider_validation_validated: bool,
    pub network_policy_enabled: bool,
    pub network_default_decision: Option<String>,
    pub network_policy_decision: Option<NetworkPolicyDecision>,
    pub local_only_required: bool,
    pub latest_route_type: Option<ProviderRouteType>,
    pub latest_external_transmission: Option<ExternalTransmissionStatus>,
    #[serde(default)]
    pub evidence_refs: Vec<EvidenceRef>,
}

pub fn build_provider_privacy_boundary_summary(
    input: ProviderPrivacyBoundaryBuildInput,
) -> ProviderPrivacyBoundarySummary {
    if input.latest_external_transmission == Some(ExternalTransmissionStatus::Sent) {
        return ProviderPrivacyBoundarySummary {
            route_type: input.latest_route_type.unwrap_or(ProviderRouteType::Cloud),
            external_transmission: ExternalTransmissionStatus::Sent,
            provider_label: cloud_provider_label(&input),
            model_label: cloud_model_label(&input),
            privacy_label: "external transmission observed".into(),
            risk: if input.local_only_required {
                ProductRiskLevel::High
            } else {
                ProductRiskLevel::Medium
            },
            local_only_required: input.local_only_required,
            blocked_reason: input.local_only_required.then(|| {
                "LocalOnly requirement conflicts with observed external transmission.".into()
            }),
            evidence_refs: input.evidence_refs,
        };
    }

    if input.local_only_required || input.prefer_local_model {
        let observed_not_sent =
            input.latest_external_transmission == Some(ExternalTransmissionStatus::NotSent);
        let route_type = input.latest_route_type.unwrap_or({
            if observed_not_sent {
                ProviderRouteType::Local
            } else {
                ProviderRouteType::Unknown
            }
        });
        let external_transmission = input
            .latest_external_transmission
            .unwrap_or(ExternalTransmissionStatus::Unknown);
        let evidence_missing = input.latest_external_transmission.is_none();
        let blocked_reason = network_policy_blocked_reason(&input).or_else(|| {
            evidence_missing.then(|| {
                if input.local_only_required {
                    "LocalOnly is required, but no runtime route evidence proves external transmission was not sent."
                        .into()
                } else {
                    "Local model preference is configured, but no runtime route evidence proves external transmission was not sent."
                        .into()
                }
            })
        });
        return ProviderPrivacyBoundarySummary {
            route_type,
            external_transmission,
            provider_label: "local model".into(),
            model_label: input
                .local_model_label
                .filter(|label| !label.trim().is_empty())
                .unwrap_or_else(|| "local model unknown".into()),
            privacy_label: if observed_not_sent {
                "runtime route evidence observed no external transmission".into()
            } else if input.local_only_required {
                "LocalOnly requirement is configured; actual transmission status remains unknown until route evidence is observed".into()
            } else {
                "local preference is configured; actual transmission status remains unknown until route evidence is observed".into()
            },
            risk: if observed_not_sent {
                ProductRiskLevel::Low
            } else {
                ProductRiskLevel::Unknown
            },
            local_only_required: input.local_only_required,
            blocked_reason,
            evidence_refs: input.evidence_refs,
        };
    }

    if input.cloud_api_configured {
        let blocked_reason = cloud_blocked_reason(&input);
        let validation_ready = input.provider_validation_validated && blocked_reason.is_none();
        return ProviderPrivacyBoundarySummary {
            route_type: input.latest_route_type.unwrap_or(ProviderRouteType::Auto),
            external_transmission: input
                .latest_external_transmission
                .unwrap_or(ExternalTransmissionStatus::Possible),
            provider_label: cloud_provider_label(&input),
            model_label: cloud_model_label(&input),
            privacy_label: if validation_ready {
                "cloud route configured; external transmission is possible, not proven by this summary"
                    .into()
            } else {
                "cloud route configured but validation is not ready; transmission status remains possible"
                    .into()
            },
            risk: ProductRiskLevel::Medium,
            local_only_required: false,
            blocked_reason,
            evidence_refs: input.evidence_refs,
        };
    }

    ProviderPrivacyBoundarySummary {
        route_type: input
            .latest_route_type
            .unwrap_or(ProviderRouteType::Unknown),
        external_transmission: input
            .latest_external_transmission
            .unwrap_or(ExternalTransmissionStatus::Unknown),
        provider_label: "cloud provider unconfigured".into(),
        model_label: input
            .cloud_model_label
            .filter(|label| !label.trim().is_empty())
            .unwrap_or_else(|| "model unknown".into()),
        privacy_label: "provider/privacy boundary unknown".into(),
        risk: ProductRiskLevel::Unknown,
        local_only_required: false,
        blocked_reason: Some(
            "No configured provider route proves external transmission safety.".into(),
        ),
        evidence_refs: input.evidence_refs,
    }
}

fn cloud_provider_label(input: &ProviderPrivacyBoundaryBuildInput) -> String {
    input
        .cloud_provider_label
        .as_ref()
        .filter(|label| !label.trim().is_empty())
        .cloned()
        .unwrap_or_else(|| "cloud provider unknown".into())
}

fn cloud_model_label(input: &ProviderPrivacyBoundaryBuildInput) -> String {
    input
        .cloud_model_label
        .as_ref()
        .filter(|label| !label.trim().is_empty())
        .cloned()
        .unwrap_or_else(|| "cloud model unknown".into())
}

fn cloud_blocked_reason(input: &ProviderPrivacyBoundaryBuildInput) -> Option<String> {
    if let Some(reason) = network_policy_blocked_reason(input) {
        return Some(reason);
    }
    if !input.network_policy_enabled {
        return Some("Network policy is disabled; cloud route cannot be treated as ready.".into());
    }
    if !input.provider_validation_validated {
        return Some(format!(
            "Provider validation is {}; cloud route is not proven ready.",
            input
                .provider_validation_status
                .as_deref()
                .unwrap_or("unknown")
        ));
    }
    if input
        .network_default_decision
        .as_deref()
        .is_some_and(|decision| decision == "deny")
    {
        return Some("Network policy default decision denies external network access.".into());
    }
    None
}

fn network_policy_blocked_reason(input: &ProviderPrivacyBoundaryBuildInput) -> Option<String> {
    let decision = input.network_policy_decision.as_ref()?;
    match decision.disposition {
        NetworkPolicyDisposition::Allow => None,
        NetworkPolicyDisposition::Ask => Some(format!(
            "Network consent is required before provider dispatch (decision_id={}).",
            decision.decision_id
        )),
        NetworkPolicyDisposition::Deny => Some(format!(
            "Cloud provider network route is blocked by {} (decision_id={}).",
            decision.reason_code, decision.decision_id
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> ProviderPrivacyBoundaryBuildInput {
        ProviderPrivacyBoundaryBuildInput {
            prefer_local_model: false,
            local_model_label: Some("qwen2.5".into()),
            cloud_provider_label: Some("OpenAI".into()),
            cloud_model_label: Some("gpt-4o-mini".into()),
            cloud_api_configured: true,
            provider_validation_status: Some("validated".into()),
            provider_validation_validated: true,
            network_policy_enabled: true,
            network_default_decision: Some("ask".into()),
            network_policy_decision: None,
            local_only_required: false,
            latest_route_type: None,
            latest_external_transmission: None,
            evidence_refs: Vec::new(),
        }
    }

    #[test]
    fn prefer_local_model_without_route_evidence_keeps_transmission_unknown() {
        let summary = build_provider_privacy_boundary_summary(ProviderPrivacyBoundaryBuildInput {
            prefer_local_model: true,
            ..input()
        });

        assert_eq!(summary.route_type, ProviderRouteType::Unknown);
        assert_eq!(
            summary.external_transmission,
            ExternalTransmissionStatus::Unknown
        );
        assert_eq!(summary.risk, ProductRiskLevel::Unknown);
        assert!(summary
            .blocked_reason
            .as_deref()
            .is_some_and(|reason| reason.contains("no runtime route evidence")));
        assert_eq!(summary.model_label, "qwen2.5");
    }

    #[test]
    fn local_only_required_without_route_evidence_does_not_claim_not_sent() {
        let summary = build_provider_privacy_boundary_summary(ProviderPrivacyBoundaryBuildInput {
            prefer_local_model: false,
            local_only_required: true,
            ..input()
        });

        assert_eq!(summary.route_type, ProviderRouteType::Unknown);
        assert_eq!(
            summary.external_transmission,
            ExternalTransmissionStatus::Unknown
        );
        assert_eq!(summary.risk, ProductRiskLevel::Unknown);
        assert!(summary
            .blocked_reason
            .as_deref()
            .is_some_and(|reason| reason.contains("LocalOnly is required")));
    }

    #[test]
    fn observed_not_sent_route_can_claim_not_sent() {
        let summary = build_provider_privacy_boundary_summary(ProviderPrivacyBoundaryBuildInput {
            prefer_local_model: true,
            latest_route_type: Some(ProviderRouteType::Local),
            latest_external_transmission: Some(ExternalTransmissionStatus::NotSent),
            ..input()
        });

        assert_eq!(summary.route_type, ProviderRouteType::Local);
        assert_eq!(
            summary.external_transmission,
            ExternalTransmissionStatus::NotSent
        );
        assert_eq!(summary.risk, ProductRiskLevel::Low);
        assert!(summary.blocked_reason.is_none());
    }

    #[test]
    fn validated_cloud_route_is_possible_not_sent_proof() {
        let summary = build_provider_privacy_boundary_summary(input());

        assert_eq!(summary.route_type, ProviderRouteType::Auto);
        assert_eq!(
            summary.external_transmission,
            ExternalTransmissionStatus::Possible
        );
        assert_eq!(summary.provider_label, "OpenAI");
        assert!(summary.blocked_reason.is_none());
    }

    #[test]
    fn stale_provider_validation_blocks_cloud_readiness() {
        let summary = build_provider_privacy_boundary_summary(ProviderPrivacyBoundaryBuildInput {
            provider_validation_status: Some("stale".into()),
            provider_validation_validated: false,
            ..input()
        });

        assert_eq!(
            summary.external_transmission,
            ExternalTransmissionStatus::Possible
        );
        assert!(summary
            .blocked_reason
            .as_deref()
            .is_some_and(|reason| reason.contains("stale")));
    }

    #[test]
    fn provider_privacy_uses_the_enforced_typed_network_decision() {
        let policy = crate::config::NetworkPolicy::default();
        let decision = crate::network_client::resolve_network_policy_decision(
            &policy,
            "https://api.openai.com/v1/models",
            "provider.openai",
        )
        .unwrap();
        let decision_id = decision.decision_id.clone();
        let summary = build_provider_privacy_boundary_summary(ProviderPrivacyBoundaryBuildInput {
            network_policy_decision: Some(decision),
            ..input()
        });

        assert!(summary
            .blocked_reason
            .as_deref()
            .is_some_and(|reason| { reason.contains("consent") && reason.contains(&decision_id) }));
    }

    #[test]
    fn observed_external_transmission_still_overrides_config() {
        let summary = build_provider_privacy_boundary_summary(ProviderPrivacyBoundaryBuildInput {
            prefer_local_model: true,
            local_only_required: true,
            latest_external_transmission: Some(ExternalTransmissionStatus::Sent),
            ..input()
        });

        assert_eq!(summary.route_type, ProviderRouteType::Cloud);
        assert_eq!(
            summary.external_transmission,
            ExternalTransmissionStatus::Sent
        );
        assert_eq!(summary.risk, ProductRiskLevel::High);
        assert!(summary.blocked_reason.is_some());
    }
}
