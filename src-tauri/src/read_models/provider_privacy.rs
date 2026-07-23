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
    event_id: String,
    event_type: String,
}

fn durable_provider_truth(
    event: &crate::main_chat_event_stream::MainChatAgentDurableEvent,
) -> Option<DurableProviderTruth> {
    if !matches!(
        event.source.as_str(),
        "provider_adapter" | "openlife_turn_runtime"
    ) || event.object_type != "provider_request"
    {
        return None;
    }
    let provider = event.payload.get("provider")?.as_str()?.trim();
    if provider.is_empty() {
        return None;
    }
    let local = provider.eq_ignore_ascii_case("ollama");
    let route_type = if local {
        ProviderRouteType::Local
    } else {
        ProviderRouteType::Cloud
    };
    let external_transmission = match (event.event_type.as_str(), local) {
        ("provider.completed" | "provider.failed", false) => ExternalTransmissionStatus::Sent,
        (
            "provider.completed"
            | "provider.failed"
            | "provider.started"
            | "provider.remote_unknown",
            true,
        ) => ExternalTransmissionStatus::NotSent,
        // Dispatch started, but no remote terminal was observed. The local
        // boundary cannot claim whether the remote side received the payload.
        ("provider.started" | "provider.remote_unknown", false) => {
            ExternalTransmissionStatus::Unknown
        }
        _ => return None,
    };
    Some(DurableProviderTruth {
        route_type,
        external_transmission,
        event_id: event.event_id.clone(),
        event_type: event.event_type.clone(),
    })
}

#[tauri::command]
pub async fn get_provider_privacy_boundary_summary(
    state: State<'_, Arc<AppState>>,
) -> Result<ViewModelEnvelope<ProviderPrivacyBoundarySummary>, String> {
    get_provider_privacy_boundary_summary_with_state(state.inner()).await
}

pub(crate) async fn get_provider_privacy_boundary_summary_with_state(
    state: &Arc<AppState>,
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
    let durable_provider_truth =
        crate::main_chat_event_stream::latest_main_chat_provider_event_with_state(state)
            .await
            .map_err(|error| format!("provider receipt read failed: {error}"))?
            .as_ref()
            .and_then(durable_provider_truth);
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
            format!("provider_receipt_event:{}", truth.event_id),
            format!("Durable provider adapter event: {}", truth.event_type),
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

    async fn append_provider_event(
        state: &Arc<AppState>,
        event_type: &str,
        status: &str,
    ) -> crate::main_chat_event_stream::MainChatAgentDurableEvent {
        let started = crate::main_chat_event_stream::append_main_chat_agent_runtime_event(
            state,
            "provider-privacy-task",
            "provider-privacy-run",
            "provider.started",
            "provider_request",
            "provider-request-1",
            "provider_adapter",
            serde_json::json!({
                "requestId": "provider-request-1",
                "provider": "openai",
                "model": "gpt-test",
                "status": "started",
            }),
        )
        .await
        .unwrap();
        if event_type == "provider.started" {
            return started;
        }
        crate::main_chat_event_stream::append_main_chat_agent_runtime_event(
            state,
            "provider-privacy-task",
            "provider-privacy-run",
            event_type,
            "provider_request",
            "provider-request-1",
            "provider_adapter",
            serde_json::json!({
                "requestId": "provider-request-1",
                "provider": "openai",
                "model": "gpt-test",
                "status": status,
            }),
        )
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn provider_privacy_consumes_durable_completed_provider_receipt() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let event = append_provider_event(&state, "provider.completed", "completed").await;

        let envelope = get_provider_privacy_boundary_summary_with_state(&state)
            .await
            .unwrap();
        let summary = envelope.data.unwrap();

        assert_eq!(
            summary.external_transmission,
            openlife_core::agent::ExternalTransmissionStatus::Sent
        );
        assert_eq!(
            summary.route_type,
            openlife_core::agent::ProviderRouteType::Cloud
        );
        assert!(envelope
            .evidence_refs
            .iter()
            .any(|evidence| evidence.id.contains(&event.event_id)));
    }

    #[tokio::test]
    async fn confirmed_failed_cloud_receipt_proves_external_transmission() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        append_provider_event(&state, "provider.failed", "failed").await;

        let envelope = get_provider_privacy_boundary_summary_with_state(&state)
            .await
            .unwrap();
        let summary = envelope.data.unwrap();

        assert_eq!(
            summary.external_transmission,
            openlife_core::agent::ExternalTransmissionStatus::Sent
        );
        assert_eq!(
            summary.route_type,
            openlife_core::agent::ProviderRouteType::Cloud
        );
    }

    #[tokio::test]
    async fn started_cloud_request_without_terminal_keeps_transmission_unknown() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        append_provider_event(&state, "provider.started", "started").await;

        let envelope = get_provider_privacy_boundary_summary_with_state(&state)
            .await
            .unwrap();
        assert_eq!(
            envelope.data.unwrap().external_transmission,
            openlife_core::agent::ExternalTransmissionStatus::Unknown
        );
    }

    #[tokio::test]
    async fn non_adapter_event_cannot_shadow_the_latest_durable_provider_receipt() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let trusted = append_provider_event(&state, "provider.completed", "completed").await;
        let rejected = crate::main_chat_event_stream::append_main_chat_agent_runtime_event(
            &state,
            "provider-privacy-task",
            "provider-privacy-run",
            "provider.started",
            "provider_request",
            "non-adapter-provider-request",
            "test_fixture",
            serde_json::json!({
                "requestId": "non-adapter-provider-request",
                "provider": "openai",
                "model": "gpt-test",
                "status": "started",
            }),
        )
        .await
        .unwrap_err();
        assert!(
            rejected.contains("provider_lifecycle_start_proof_mismatch"),
            "a schema-valid non-adapter lifecycle event must fail before it can obtain provider durability authority: {rejected}"
        );

        let envelope = get_provider_privacy_boundary_summary_with_state(&state)
            .await
            .unwrap();
        let summary = envelope.data.unwrap();

        assert_eq!(
            summary.external_transmission,
            openlife_core::agent::ExternalTransmissionStatus::Sent
        );
        assert!(envelope
            .evidence_refs
            .iter()
            .any(|evidence| evidence.id.contains(&trusted.event_id)));
    }
}
