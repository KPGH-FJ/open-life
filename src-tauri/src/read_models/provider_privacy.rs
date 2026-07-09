use crate::provider_validation::{
    cloud_api_configured, load_provider_validation_record_from_path, provider_validation_path,
    summarize_provider_validation,
};
use crate::state::AppState;
use openlife_core::agent::{
    build_provider_privacy_boundary_summary, EvidenceRef, EvidenceSensitivity, EvidenceSource,
    ProviderPrivacyBoundaryBuildInput, ProviderPrivacyBoundarySummary, ViewModelEnvelope,
    ViewModelStatus, ViewModelWarning, ViewModelWarningSeverity,
};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn get_provider_privacy_boundary_summary(
    state: State<'_, Arc<AppState>>,
) -> Result<ViewModelEnvelope<ProviderPrivacyBoundarySummary>, String> {
    get_provider_privacy_boundary_summary_with_state(state.inner()).await
}

pub(crate) async fn get_provider_privacy_boundary_summary_with_state(
    state: &Arc<AppState>,
) -> Result<ViewModelEnvelope<ProviderPrivacyBoundarySummary>, String> {
    let config = state.config.lock().await.clone();
    let record = load_provider_validation_record_from_path(&provider_validation_path());
    let validation = summarize_provider_validation(&config, record.as_ref(), chrono::Utc::now());
    let evidence_refs = vec![
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
    let mut warnings = Vec::new();
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

    let summary = build_provider_privacy_boundary_summary(ProviderPrivacyBoundaryBuildInput {
        prefer_local_model: config.prefer_local_model,
        local_model_label: Some(config.local_model.clone()),
        cloud_provider_label: Some(config.effective_provider_label()),
        cloud_model_label: Some(config.llm.chat_model.clone()),
        cloud_api_configured: cloud_api_configured(&config),
        provider_validation_status: Some(validation.status.into()),
        provider_validation_validated: validation.validated,
        network_policy_enabled: config.system.network_policy.enabled,
        network_default_decision: Some(config.system.network_policy.default_decision.clone()),
        local_only_required: false,
        latest_route_type: None,
        latest_external_transmission: None,
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
