use crate::provider_validation::{
    cloud_api_configured, load_provider_validation_record_from_path,
    summarize_loaded_provider_validation, ProviderValidationLoad,
};
use crate::secret_store::{hydrate_bound_provider_secret, ProfileSecretStore, SecretStore};
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

struct SelectedProviderRoute {
    config: openlife_core::config::AppConfig,
    connection_id: String,
    provider_label: String,
    model_label: String,
}

async fn selected_provider_route(
    state: &Arc<AppState>,
    base_config: &openlife_core::config::AppConfig,
    conversation_id: Option<&str>,
) -> Option<SelectedProviderRoute> {
    let store = state.conversation_store.as_ref()?.lock().await;
    let selected_profile_id = store
        .selected_provider_profile_id(conversation_id)
        .ok()
        .flatten()?;
    let profile = store
        .list_provider_model_profiles()
        .ok()?
        .into_iter()
        .find(|profile| profile.profile_id == selected_profile_id)?;
    let connection = store
        .get_provider_connection(&profile.connection_id)
        .ok()??;
    drop(store);

    let mut config = crate::commands::settings::provider_connection_config(
        base_config,
        &connection,
        &profile.model_id,
        String::new(),
    );
    if connection.endpoint_class == "local" {
        config.prefer_local_model = true;
        config.local_model = profile.model_id.clone();
    } else if let Some(reference) = connection.credential_reference.as_deref() {
        if let Ok(Some(encoded)) = ProfileSecretStore.get(reference) {
            if let Ok(secret) = hydrate_bound_provider_secret(
                &connection.provider_id,
                &connection.endpoint,
                connection.credential_version,
                &encoded,
            ) {
                config.llm.openai_key = secret;
            }
        }
    }
    Some(SelectedProviderRoute {
        config,
        connection_id: connection.id,
        provider_label: connection.display_name,
        model_label: profile.display_name,
    })
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
    let base_config = runtime.config;
    let selected_route = selected_provider_route(state, &base_config, conversation_id).await;
    let (config, provider_label, model_label, validation_load, route_evidence_id) =
        if let Some(route) = selected_route {
            let validation_path = crate::commands::settings::provider_connection_validation_path(
                &route.connection_id,
            );
            (
                route.config,
                route.provider_label,
                route.model_label,
                load_provider_validation_record_from_path(&validation_path),
                format!("provider_connection:{}", route.connection_id),
            )
        } else {
            let mut unselected = base_config;
            unselected.prefer_local_model = false;
            unselected.llm.provider = "unselected".into();
            unselected.llm.openai_base.clear();
            unselected.llm.chat_model.clear();
            unselected.llm.openai_key.clear();
            (
                unselected,
                "未选择 Provider".into(),
                "未选择模型".into(),
                ProviderValidationLoad::Missing,
                "provider_profile:unselected".into(),
            )
        };
    let mut validation =
        summarize_loaded_provider_validation(&config, &validation_load, chrono::Utc::now());
    if !runtime_coherent {
        validation.validated = false;
        validation.status = "runtime_generation_incoherent";
        validation.last_error = Some("provider_runtime_generation_incoherent".into());
    }
    let durable_provider_truth = durable_provider_truth(state, conversation_id, turn_id).await;
    // ProviderPrivacy reports the same persistent Connection/Profile route
    // that Main Chat resolves. AppConfig remains only the independent policy
    // owner for local preference, search and network settings.
    let provider_network_url =
        openlife_core::llm::chat_completions_url(&config.llm.provider, &config.llm.openai_base);
    let network_policy_decision = runtime_coherent
        .then(|| {
            resolve_network_policy_decision(
                &config.system.network_policy,
                &provider_network_url,
                &format!("provider.{}", config.llm.provider),
            )
            .ok()
        })
        .flatten();
    let mut evidence_refs = vec![
        source_ref(
            route_evidence_id,
            "Persistent Provider Connection and Model Profile",
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
        cloud_provider_label: Some(provider_label),
        cloud_model_label: Some(model_label),
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
    use openlife_core::conversation::{
        BeginChatTurn, ProviderBinding, ProviderConnectionRecord, ProviderModelProfileRecord,
    };

    #[tokio::test]
    async fn selected_persistent_profile_labels_replace_legacy_config_labels() {
        let state = crate::test_utils::test_app_state();
        {
            let mut config = state.config.lock().await;
            config.llm.provider = "openai".into();
            config.llm.chat_model = "legacy-config-model".into();
        }
        let now = chrono::Utc::now();
        let connection = ProviderConnectionRecord {
            id: "persistent-connection".into(),
            provider_id: "deepseek".into(),
            display_name: "Persistent DeepSeek".into(),
            endpoint: "https://api.deepseek.com".into(),
            endpoint_class: "cloud".into(),
            credential_reference: None,
            credential_version: 3,
            protocol: "openai_compatible_chat_completions".into(),
            privacy_boundary: "provider_hosted".into(),
            validation_state: "unverified".into(),
            created_at: now,
            updated_at: now,
        };
        let profile = ProviderModelProfileRecord {
            profile_id: "persistent-profile".into(),
            connection_id: connection.id.clone(),
            model_id: "deepseek-persistent-model".into(),
            display_name: "Persistent Model".into(),
            capability_snapshot_json: "{}".into(),
            capability_source: "provider_discovery".into(),
            validation_state: "unverified".into(),
            created_at: now,
            updated_at: now,
        };
        {
            let store = state.conversation_store.as_ref().unwrap().lock().await;
            store
                .upsert_provider_model_profile(&connection, &profile)
                .unwrap();
            store
                .set_selected_provider_profile(None, &profile.profile_id)
                .unwrap();
        }

        let envelope = get_provider_privacy_boundary_summary_with_state(&state, None, None)
            .await
            .unwrap();
        let summary = envelope.data.unwrap();
        assert_eq!(summary.provider_label, "cloud provider unconfigured");
        assert_eq!(summary.model_label, "Persistent Model");
        assert_ne!(summary.provider_label, "OpenAI");
        assert_ne!(summary.model_label, "legacy-config-model");
        assert!(summary
            .evidence_refs
            .iter()
            .any(|reference| reference.id == "provider_connection:persistent-connection"));
    }

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
