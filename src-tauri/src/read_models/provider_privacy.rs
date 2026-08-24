use crate::provider_validation::{
    cloud_api_configured, load_provider_validation_record_from_path, provider_validation_path,
    summarize_loaded_provider_validation,
};
use crate::state::AppState;
use openlife_core::agent::{
    build_provider_privacy_boundary_summary, EvidenceRef, EvidenceSensitivity, EvidenceSource,
    ExternalTransmissionStatus, ProviderPrivacyBoundaryBuildInput, ProviderPrivacyBoundarySummary,
    ProviderRouteType, ViewModelEnvelope, ViewModelStatus, ViewModelWarning,
    ViewModelWarningSeverity,
};
use openlife_core::network_client::{resolve_network_policy_decision, NetworkPolicyDisposition};
use std::sync::Arc;
use tauri::State;

struct DurableProviderTruth {
    route_type: ProviderRouteType,
    external_transmission: ExternalTransmissionStatus,
    turn_id: String,
    turn_status: String,
}

async fn durable_provider_truth(
    state: &Arc<AppState>,
    conversation_id: Option<&str>,
    turn_id: Option<&str>,
) -> Option<DurableProviderTruth> {
    let store = state.conversation_store.as_ref()?.lock().await;
    let turn = match turn_id {
        Some(turn_id) => {
            let turn = store.get_turn(turn_id).ok()??.turn;
            if conversation_id
                .is_some_and(|conversation_id| turn.conversation_id != conversation_id)
            {
                return None;
            }
            turn
        }
        None => {
            let conversation_id = match conversation_id {
                Some(conversation_id) => {
                    store.get_conversation(conversation_id).ok()??;
                    conversation_id.to_string()
                }
                None => {
                    store
                        .list_conversations(true, 1)
                        .ok()?
                        .into_iter()
                        .next()?
                        .id
                }
            };
            store.latest_turn(&conversation_id).ok()??
        }
    };
    let local = turn.provider.provider_id.eq_ignore_ascii_case("ollama")
        || turn.provider.endpoint_class.eq_ignore_ascii_case("local");
    let route_type = if local {
        ProviderRouteType::Local
    } else {
        ProviderRouteType::Cloud
    };
    let external_transmission = match (turn.status, local) {
        (_, true) => ExternalTransmissionStatus::NotSent,
        (
            openlife_core::conversation::TurnStatus::Completed
            | openlife_core::conversation::TurnStatus::Failed,
            false,
        ) => ExternalTransmissionStatus::Sent,
        (
            openlife_core::conversation::TurnStatus::Running
            | openlife_core::conversation::TurnStatus::Cancelled
            | openlife_core::conversation::TurnStatus::Interrupted,
            false,
        ) => ExternalTransmissionStatus::Unknown,
    };
    Some(DurableProviderTruth {
        route_type,
        external_transmission,
        turn_id: turn.id,
        turn_status: turn.status.as_str().into(),
    })
}

#[tauri::command]
pub async fn get_provider_privacy_boundary_summary(
    state: State<'_, Arc<AppState>>,
    conversation_id: Option<String>,
    turn_id: Option<String>,
) -> Result<ViewModelEnvelope<ProviderPrivacyBoundarySummary>, String> {
    get_provider_privacy_boundary_summary_with_state(
        state.inner(),
        conversation_id.as_deref().filter(|value| !value.is_empty()),
        turn_id.as_deref().filter(|value| !value.is_empty()),
    )
    .await
}

pub(crate) async fn get_provider_privacy_boundary_summary_with_state(
    state: &Arc<AppState>,
    conversation_id: Option<&str>,
    turn_id: Option<&str>,
) -> Result<ViewModelEnvelope<ProviderPrivacyBoundarySummary>, String> {
    let runtime = state.provider_runtime_snapshot().await;
    let runtime_coherent = runtime.coherent;
    let config = runtime.config;
    let scheduler = runtime.scheduler;
    let validation_load = load_provider_validation_record_from_path(&provider_validation_path());
    let mut validation =
        summarize_loaded_provider_validation(&config, &validation_load, chrono::Utc::now());
    if !runtime_coherent {
        validation.validated = false;
        validation.status = "runtime_generation_incoherent";
        validation.last_error = Some("provider_runtime_generation_incoherent".into());
    }
    let durable_provider_truth = durable_provider_truth(state, conversation_id, turn_id).await;
    // ProviderPrivacy reports the same concrete route that Main Chat and the
    // shipped status probe will enforce. AppConfig remains the policy owner;
    // the scheduler snapshot remains the provider/base route owner.
    let provider_network_url =
        openlife_core::llm::chat_completions_url(&scheduler.provider, &scheduler.openai_base);
    let network_policy_decision = runtime_coherent
        .then(|| {
            resolve_network_policy_decision(
                &config.system.network_policy,
                &provider_network_url,
                &format!("provider.{}", scheduler.provider),
            )
            .ok()
        })
        .flatten();
    let mut evidence_refs = vec![
        source_ref(
            "app_config:model_route",
            "Model route configuration",
            EvidenceSource::Settings,
        ),
        source_ref(
            "provider_validation:summary",
            "Provider validation summary",
            EvidenceSource::Provider,
        ),
        source_ref(
            "privacy_policy:network",
            "Network and privacy policy",
            EvidenceSource::Provider,
        ),
    ];
    if let Some(decision) = network_policy_decision.as_ref() {
        evidence_refs.push(source_ref(
            format!("network_policy_decision:{}", decision.decision_id),
            format!(
                "Provider network decision: {} ({})",
                decision.disposition.as_str(),
                decision.reason_code
            ),
            EvidenceSource::Provider,
        ));
    }
    if let Some(truth) = durable_provider_truth.as_ref() {
        evidence_refs.push(source_ref(
            format!("conversation_turn:{}", truth.turn_id),
            format!("Canonical provider-bound Turn: {}", truth.turn_status),
            EvidenceSource::Provider,
        ));
    }
    let mut warnings = Vec::new();
    if !runtime_coherent {
        warnings.push(warning(
            "provider_runtime_generation_incoherent",
            "Provider configuration and executable adapter do not belong to one runtime generation; cloud readiness remains fail-closed.",
        ));
    }
    if !config.prefer_local_model && !validation.validated {
        warnings.push(warning(
            "provider_validation_not_ready",
            format!(
                "Provider validation status is {}; cloud readiness and external transmission remain fail-closed.",
                validation.status
            ),
        ));
    }
    if !config.system.network_policy.enabled {
        warnings.push(warning(
            "network_policy_disabled",
            "Network policy is disabled; cloud provider route cannot be treated as ready.",
        ));
    }
    if let Some(decision) = network_policy_decision.as_ref() {
        if decision.disposition != NetworkPolicyDisposition::Allow {
            warnings.push(warning(
                decision.reason_code.clone(),
                format!(
                    "Provider dispatch is {} before HTTP (decision_id={}).",
                    decision.disposition.as_str(),
                    decision.decision_id
                ),
            ));
        }
    }

    let summary = build_provider_privacy_boundary_summary(ProviderPrivacyBoundaryBuildInput {
        prefer_local_model: config.prefer_local_model,
        local_model_label: Some(config.local_model.clone()),
        cloud_provider_label: Some(config.effective_provider_label()),
        cloud_model_label: Some(config.llm.chat_model.clone()),
        cloud_api_configured: runtime_coherent && cloud_api_configured(&config),
        provider_validation_status: Some(validation.status.into()),
        provider_validation_validated: validation.validated,
        network_policy_enabled: config.system.network_policy.enabled,
        network_default_decision: Some(config.system.network_policy.default_decision.clone()),
        network_policy_decision,
        local_only_required: false,
        latest_route_type: durable_provider_truth
            .as_ref()
            .map(|truth| truth.route_type),
        latest_external_transmission: durable_provider_truth
            .as_ref()
            .map(|truth| truth.external_transmission),
        evidence_refs: evidence_refs.clone(),
    });
    if summary.external_transmission == openlife_core::agent::ExternalTransmissionStatus::Unknown {
        warnings.push(warning(
            "provider_route_evidence_missing",
            summary.blocked_reason.clone().unwrap_or_else(|| {
                "Runtime route evidence is missing; external transmission remains unknown.".into()
            }),
        ));
    }

    let mut envelope = ViewModelEnvelope::backend_read_model(ViewModelStatus::Ready, Some(summary));
    envelope.last_updated_at = Some(chrono::Utc::now().to_rfc3339());
    envelope.evidence_refs = evidence_refs;
    envelope.warnings = warnings;
    Ok(envelope)
}

fn source_ref(
    id: impl Into<String>,
    label: impl Into<String>,
    source: EvidenceSource,
) -> EvidenceRef {
    EvidenceRef {
        id: id.into(),
        label: label.into(),
        source,
        sensitivity: Some(EvidenceSensitivity::LocalPrivate),
    }
}

fn warning(code: impl Into<String>, message: impl Into<String>) -> ViewModelWarning {
    ViewModelWarning {
        code: code.into(),
        message: message.into(),
        severity: ViewModelWarningSeverity::Warning,
        evidence_refs: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openlife_core::conversation::{BeginChatTurn, ProviderBinding};

    #[tokio::test]
    async fn durable_provider_truth_is_scoped_to_the_requested_conversation() {
        let state = crate::test_utils::test_app_state();
        let local_conversation_id = uuid::Uuid::new_v4().to_string();
        let cloud_conversation_id = uuid::Uuid::new_v4().to_string();
        let local_turn_id = uuid::Uuid::new_v4().to_string();
        let cloud_turn_id = uuid::Uuid::new_v4().to_string();
        let store = state.conversation_store.as_ref().unwrap().lock().await;
        store
            .create_conversation(&local_conversation_id, "Local Conversation")
            .unwrap();
        store
            .begin_chat_turn(BeginChatTurn {
                turn_id: &local_turn_id,
                conversation_id: &local_conversation_id,
                user_message: "local",
                provider: &ProviderBinding {
                    profile_id: "profile:local".into(),
                    provider_id: "ollama".into(),
                    model_id: "llama3:latest".into(),
                    endpoint_class: "local".into(),
                    config_generation: "generation:local".into(),
                    reasoning_effort: None,
                },
            })
            .unwrap();
        store.complete_chat_turn(&local_turn_id, "done").unwrap();
        store
            .create_conversation(&cloud_conversation_id, "Cloud Conversation")
            .unwrap();
        store
            .begin_chat_turn(BeginChatTurn {
                turn_id: &cloud_turn_id,
                conversation_id: &cloud_conversation_id,
                user_message: "cloud",
                provider: &ProviderBinding {
                    profile_id: "profile:cloud".into(),
                    provider_id: "openai".into(),
                    model_id: "gpt".into(),
                    endpoint_class: "cloud".into(),
                    config_generation: "generation:cloud".into(),
                    reasoning_effort: None,
                },
            })
            .unwrap();
        store.complete_chat_turn(&cloud_turn_id, "done").unwrap();
        drop(store);

        let local = durable_provider_truth(&state, Some(&local_conversation_id), None)
            .await
            .unwrap();
        assert_eq!(local.route_type, ProviderRouteType::Local);
        assert_eq!(
            local.external_transmission,
            ExternalTransmissionStatus::NotSent
        );
        assert_eq!(local.turn_id, local_turn_id);

        let cloud = durable_provider_truth(&state, Some(&cloud_conversation_id), None)
            .await
            .unwrap();
        assert_eq!(cloud.route_type, ProviderRouteType::Cloud);
        assert_eq!(
            cloud.external_transmission,
            ExternalTransmissionStatus::Sent
        );
        assert_eq!(cloud.turn_id, cloud_turn_id);

        assert!(
            durable_provider_truth(&state, Some(&uuid::Uuid::new_v4().to_string()), None)
                .await
                .is_none()
        );
        let exact_local_run =
            durable_provider_truth(&state, Some(&local_conversation_id), Some(&local_turn_id))
                .await
                .unwrap();
        assert_eq!(exact_local_run.turn_id, local_turn_id);
        assert!(
            durable_provider_truth(&state, Some(&cloud_conversation_id), Some(&local_turn_id),)
                .await
                .is_none()
        );
    }
}
