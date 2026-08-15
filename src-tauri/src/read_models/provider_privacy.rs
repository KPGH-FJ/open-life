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

async fn durable_provider_truth(state: &Arc<AppState>) -> Option<DurableProviderTruth> {
    let store = state.conversation_store.as_ref()?.lock().await;
    let conversation = store.list_conversations(true, 1).ok()?.into_iter().next()?;
    let turn = store.latest_turn(&conversation.id).ok()??;
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
    let durable_provider_truth = durable_provider_truth(state).await;
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
