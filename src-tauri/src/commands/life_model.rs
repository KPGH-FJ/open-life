use crate::commands::proposal::{
    canonical_lifemodel_path, is_communication_style_lifemodel_path,
    COMMUNICATION_STYLE_CANONICAL_PATH,
};
use crate::errors::AppError;
use crate::legacy_write_convergence::{
    LifeModelMaterializerCallerContext, LifeModelMaterializerCallerKind,
    LifeModelMaterializerCallerPurpose,
};
use crate::{persist_life_model, AppState};
use openlife_core::agent::{AgentProposal, ProposalStatus};
use openlife_core::life_model::patch::{LifeModelPatch, PatchStatus};
use openlife_core::life_model::LifeModel;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tauri::State;

const MANUAL_LIFEMODEL_OVERRIDE_AUDIT_EVENT: &str = "manual_lifemodel_override_audit";
const MANUAL_LIFEMODEL_OVERRIDE_SOURCE: &str = "manual_lifemodel_editor";
const MANUAL_LIFEMODEL_OVERRIDE_COMMAND: &str = "save_life_model";
const MANUAL_LIFEMODEL_OVERRIDE_RISK_CLASS: &str = "GovernedManualOverride";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LifeModelChangeView {
    pub path: String,
    pub proposal_id: String,
    pub proposal_status: String,
    pub proposal_source: String,
    pub proposal_source_detail: Option<String>,
    pub proposal_run_id: Option<String>,
    pub source_excerpt: Option<String>,
    pub source_unavailable_reason: Option<String>,
    pub confidence: f32,
    pub risk_level: String,
    pub before: Option<Value>,
    pub after: Value,
    pub patch_id: Option<String>,
    pub patch_status: Option<String>,
    pub patch_path: Option<String>,
    pub patch_unavailable_reason: Option<String>,
    pub snapshot_versions: Vec<String>,
    pub snapshot_unavailable_reason: Option<String>,
    pub current_matches_accepted_after: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LifeModelCurrentView {
    pub path: String,
    pub label: String,
    pub value: Option<String>,
    pub unavailable_reason: Option<String>,
    pub current_value_source: String,
    pub change: Option<LifeModelChangeView>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GovernedManualLifeModelOverrideRequest {
    pub purpose: String,
    pub explicit_user_intent: bool,
    pub risk_acknowledged: bool,
    pub create_pre_change_snapshot: bool,
}

impl GovernedManualLifeModelOverrideRequest {
    #[cfg(test)]
    fn editor_save() -> Self {
        Self {
            purpose: "manual_lifemodel_editor_save".into(),
            explicit_user_intent: true,
            risk_acknowledged: true,
            create_pre_change_snapshot: true,
        }
    }

    fn is_valid(&self) -> bool {
        self.purpose == "manual_lifemodel_editor_save"
            && self.explicit_user_intent
            && self.risk_acknowledged
            && self.create_pre_change_snapshot
    }
}

fn require_governed_manual_lifemodel_override_request(
    request: Option<&GovernedManualLifeModelOverrideRequest>,
) -> Result<&GovernedManualLifeModelOverrideRequest, AppError> {
    if let Some(request) = request.filter(|request| request.is_valid()) {
        Ok(request)
    } else {
        Err(AppError::permission(
            "save_life_model requires an explicit governed manual override request with purpose manual_lifemodel_editor_save, explicitUserIntent=true, riskAcknowledged=true, and createPreChangeSnapshot=true.",
        ))
    }
}

fn value_as_trimmed_string(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn compact_source_excerpt(value: &str) -> Option<String> {
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.is_empty() {
        None
    } else if compact.chars().count() > 160 {
        Some(format!(
            "{}...",
            compact.chars().take(157).collect::<String>()
        ))
    } else {
        Some(compact)
    }
}

fn proposal_source_excerpt(proposal: &AgentProposal) -> (Option<String>, Option<String>) {
    if let Some(excerpt) = compact_source_excerpt(&proposal.reason) {
        return (Some(excerpt), None);
    }
    if let Some(source_detail) = proposal
        .source_detail
        .as_deref()
        .and_then(compact_source_excerpt)
    {
        return (Some(source_detail), None);
    }
    (None, Some("source_excerpt_unavailable".into()))
}

fn latest_matching_communication_style_proposal(
    proposals: Vec<AgentProposal>,
    current_value: Option<&str>,
) -> Option<AgentProposal> {
    let mut candidates: Vec<AgentProposal> = proposals
        .into_iter()
        .filter(|proposal| {
            proposal.status == ProposalStatus::Accepted
                && is_communication_style_lifemodel_path(&proposal.affected_path)
        })
        .collect();
    candidates.sort_by(|left, right| {
        let left_time = left.resolved_at.unwrap_or(left.created_at);
        let right_time = right.resolved_at.unwrap_or(right.created_at);
        right_time.cmp(&left_time)
    });

    if let Some(current) = current_value {
        if let Some(index) = candidates.iter().position(|proposal| {
            value_as_trimmed_string(&proposal.after).as_deref() == Some(current)
        }) {
            return Some(candidates.remove(index));
        }
    }
    candidates.into_iter().next()
}

fn communication_style_patch_for_proposal(patches: Vec<LifeModelPatch>) -> Option<LifeModelPatch> {
    patches.into_iter().find(|patch| {
        canonical_lifemodel_path(&patch.path_pointer) == COMMUNICATION_STYLE_CANONICAL_PATH
            && patch.status == PatchStatus::Applied
    })
}

async fn communication_style_patch_view(
    state: &Arc<AppState>,
    proposal_id: &str,
) -> (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
) {
    let Some(patch_store) = state.patch_store.as_ref() else {
        return (None, None, None, Some("patch_store_unavailable".into()));
    };
    let patches = {
        let store = patch_store.lock().await;
        match store.list_patches_by_proposal(proposal_id) {
            Ok(patches) => patches,
            Err(_) => return (None, None, None, Some("patch_read_failed".into())),
        }
    };
    match communication_style_patch_for_proposal(patches) {
        Some(patch) => (
            Some(patch.id),
            Some(patch.status.to_string()),
            Some(COMMUNICATION_STYLE_CANONICAL_PATH.into()),
            None,
        ),
        None => (None, None, None, Some("patch_missing".into())),
    }
}

async fn communication_style_snapshot_view(
    state: &Arc<AppState>,
    proposal_id: &str,
) -> (Vec<String>, Option<String>) {
    let snapshots = {
        let version_manager = state.version_manager.lock().await;
        match version_manager.get_patch_snapshots(proposal_id) {
            Ok(snapshots) => snapshots,
            Err(_) => return (Vec::new(), Some("snapshot_read_failed".into())),
        }
    };
    if snapshots.is_empty() {
        return (Vec::new(), Some("snapshot_missing".into()));
    }
    let has_before = snapshots
        .iter()
        .any(|snapshot| snapshot.tag == format!("patch:{proposal_id}:before"));
    let has_after = snapshots
        .iter()
        .any(|snapshot| snapshot.tag == format!("patch:{proposal_id}:after"));
    let mut versions: Vec<String> = snapshots
        .into_iter()
        .map(|snapshot| snapshot.version)
        .collect();
    versions.sort();
    versions.dedup();
    let unavailable = if has_before && has_after {
        None
    } else {
        Some("snapshot_incomplete".into())
    };
    (versions, unavailable)
}

async fn accepted_communication_style_proposal(
    state: &Arc<AppState>,
    current_value: Option<&str>,
) -> (Option<AgentProposal>, Option<String>) {
    let Some(proposal_store) = state.proposal_store.as_ref() else {
        return (None, Some("proposal_store_unavailable".into()));
    };
    let proposals = {
        let store = proposal_store.lock().await;
        match store.list_proposals_filtered(Some(ProposalStatus::Accepted), None, None, 200) {
            Ok(proposals) => proposals,
            Err(_) => return (None, Some("proposal_read_failed".into())),
        }
    };
    match latest_matching_communication_style_proposal(proposals, current_value) {
        Some(proposal) => (Some(proposal), None),
        None => (None, Some("accepted_proposal_missing".into())),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManualLifeModelOverrideAuditReport {
    pub source: String,
    pub before_hash: String,
    pub after_hash: String,
    pub changed_section_names: Vec<String>,
    pub changed_section_count: usize,
    pub risk_class: String,
    pub timestamp: String,
    pub command_function_name: String,
    pub operation_purpose: String,
    pub governed_operation: bool,
    pub pre_change_snapshot_created: bool,
    pub pre_change_snapshot_version: Option<String>,
    pub manual_override: bool,
    pub proposal_first: bool,
    pub still_legacy_direct_write: bool,
    pub metadata_safe: bool,
    pub contains_raw_content: bool,
    pub audit_event_name: String,
    pub audit_detail_json: String,
}

pub(crate) async fn get_life_model_with_state(
    state: &Arc<AppState>,
) -> Result<LifeModel, AppError> {
    let manager = state.life_model_manager.lock().await;
    manager.load().map_err(AppError::from)
}

#[tauri::command]
pub async fn get_life_model(state: State<'_, Arc<AppState>>) -> Result<LifeModel, AppError> {
    get_life_model_with_state(&state.inner().clone()).await
}

pub(crate) async fn get_life_model_current_view_with_state(
    state: &Arc<AppState>,
) -> Result<LifeModelCurrentView, AppError> {
    let model = get_life_model_with_state(state).await?;
    let current_value = model.preferences.communication_style.trim().to_string();
    let current_value = if current_value.is_empty() {
        None
    } else {
        Some(current_value)
    };
    let (proposal, proposal_unavailable_reason) =
        accepted_communication_style_proposal(state, current_value.as_deref()).await;

    let change = if let Some(proposal) = proposal {
        let (source_excerpt, source_unavailable_reason) = proposal_source_excerpt(&proposal);
        let (patch_id, patch_status, patch_path, patch_unavailable_reason) =
            communication_style_patch_view(state, &proposal.id).await;
        let (snapshot_versions, snapshot_unavailable_reason) =
            communication_style_snapshot_view(state, &proposal.id).await;
        let current_matches_accepted_after =
            current_value.as_deref() == value_as_trimmed_string(&proposal.after).as_deref();
        Some(LifeModelChangeView {
            path: COMMUNICATION_STYLE_CANONICAL_PATH.into(),
            proposal_id: proposal.id,
            proposal_status: proposal.status.to_string(),
            proposal_source: proposal.source.to_string(),
            proposal_source_detail: proposal.source_detail,
            proposal_run_id: proposal.run_id,
            source_excerpt,
            source_unavailable_reason,
            confidence: proposal.confidence,
            risk_level: proposal.risk_level.to_string(),
            before: proposal.before,
            after: proposal.after,
            patch_id,
            patch_status,
            patch_path,
            patch_unavailable_reason,
            snapshot_versions,
            snapshot_unavailable_reason,
            current_matches_accepted_after,
        })
    } else {
        None
    };

    let unavailable_reason = if current_value.is_none() {
        Some("current_value_empty".into())
    } else {
        None
    };
    let current_value_source = if current_value.is_some()
        && change
            .as_ref()
            .is_some_and(|change| change.current_matches_accepted_after)
    {
        "accepted_proposal".into()
    } else if current_value.is_some() && change.is_some() {
        "accepted_proposal_mismatch".into()
    } else if current_value.is_some() {
        proposal_unavailable_reason.unwrap_or_else(|| "current_life_model_only".into())
    } else {
        "unavailable".into()
    };

    Ok(LifeModelCurrentView {
        path: COMMUNICATION_STYLE_CANONICAL_PATH.into(),
        label: "沟通偏好".into(),
        value: current_value,
        unavailable_reason,
        current_value_source,
        change,
    })
}

#[tauri::command]
pub async fn get_life_model_current_view(
    state: State<'_, Arc<AppState>>,
) -> Result<LifeModelCurrentView, AppError> {
    get_life_model_current_view_with_state(&state.inner().clone()).await
}

pub(crate) async fn save_life_model_with_state(
    life_model: LifeModel,
    state: &Arc<AppState>,
    request: Option<GovernedManualLifeModelOverrideRequest>,
) -> Result<ManualLifeModelOverrideAuditReport, AppError> {
    let request = require_governed_manual_lifemodel_override_request(request.as_ref())?;
    let before = {
        let manager = state.life_model_manager.lock().await;
        manager.load().map_err(AppError::from)?
    };
    let pre_change_snapshot_version = {
        let vm = state.version_manager.lock().await;
        vm.snapshot(
            &before,
            "auto:pre-manual-lifemodel-override",
            "Manual LifeModel editor save pre-change snapshot",
        )
        .ok()
        .map(|snapshot| snapshot.version)
    };
    let after = persist_life_model(
        &state.clone(),
        life_model,
        true,
        LifeModelMaterializerCallerContext::new(
            "manual_lifemodel_editor_save",
            LifeModelMaterializerCallerKind::GovernedManualOverride,
            LifeModelMaterializerCallerPurpose::GovernedManualOverride,
        ),
    )
    .await
    .map_err(AppError::from)?;
    record_manual_lifemodel_override_audit_with_state(
        state,
        &before,
        &after,
        request,
        pre_change_snapshot_version,
    )
    .await
}

#[tauri::command]
pub async fn save_life_model(
    life_model: LifeModel,
    manual_override_request: Option<GovernedManualLifeModelOverrideRequest>,
    state: State<'_, Arc<AppState>>,
) -> Result<ManualLifeModelOverrideAuditReport, AppError> {
    save_life_model_with_state(life_model, &state.inner().clone(), manual_override_request).await
}

pub(crate) async fn record_manual_lifemodel_override_audit_with_state(
    state: &Arc<AppState>,
    before: &LifeModel,
    after: &LifeModel,
    request: &GovernedManualLifeModelOverrideRequest,
    pre_change_snapshot_version: Option<String>,
) -> Result<ManualLifeModelOverrideAuditReport, AppError> {
    let report = evaluate_manual_lifemodel_override_audit(
        before,
        after,
        request,
        pre_change_snapshot_version,
    )?;
    let feedback = state.feedback_store.lock().await;
    feedback
        .log_event(
            &report.audit_event_name,
            None,
            Some(&report.audit_detail_json),
        )
        .map_err(AppError::from)?;
    Ok(report)
}

pub(crate) fn evaluate_manual_lifemodel_override_audit(
    before: &LifeModel,
    after: &LifeModel,
    request: &GovernedManualLifeModelOverrideRequest,
    pre_change_snapshot_version: Option<String>,
) -> Result<ManualLifeModelOverrideAuditReport, AppError> {
    let before_hash = hash_life_model(before)?;
    let after_hash = hash_life_model(after)?;
    let changed_section_names = changed_life_model_sections(before, after)?;
    let timestamp = chrono::Utc::now().to_rfc3339();
    let detail = serde_json::json!({
        "source": MANUAL_LIFEMODEL_OVERRIDE_SOURCE,
        "beforeHash": before_hash,
        "afterHash": after_hash,
        "changedSectionNames": changed_section_names,
        "changedSectionCount": changed_section_names.len(),
        "riskClass": MANUAL_LIFEMODEL_OVERRIDE_RISK_CLASS,
        "timestamp": timestamp,
        "commandFunctionName": MANUAL_LIFEMODEL_OVERRIDE_COMMAND,
        "operationPurpose": request.purpose,
        "governedOperation": true,
        "preChangeSnapshotCreated": pre_change_snapshot_version.is_some(),
        "preChangeSnapshotVersion": pre_change_snapshot_version.clone(),
        "manualOverride": true,
        "proposalFirst": false,
        "stillLegacyDirectWrite": false,
        "metadataSafe": true,
        "containsRawContent": false,
    });
    let audit_detail_json = serde_json::to_string(&detail)?;

    Ok(ManualLifeModelOverrideAuditReport {
        source: MANUAL_LIFEMODEL_OVERRIDE_SOURCE.into(),
        before_hash,
        after_hash,
        changed_section_count: changed_section_names.len(),
        changed_section_names,
        risk_class: MANUAL_LIFEMODEL_OVERRIDE_RISK_CLASS.into(),
        timestamp,
        command_function_name: MANUAL_LIFEMODEL_OVERRIDE_COMMAND.into(),
        operation_purpose: request.purpose.clone(),
        governed_operation: true,
        pre_change_snapshot_created: pre_change_snapshot_version.is_some(),
        pre_change_snapshot_version,
        manual_override: true,
        proposal_first: false,
        still_legacy_direct_write: false,
        metadata_safe: true,
        contains_raw_content: false,
        audit_event_name: MANUAL_LIFEMODEL_OVERRIDE_AUDIT_EVENT.into(),
        audit_detail_json,
    })
}

fn hash_life_model(model: &LifeModel) -> Result<String, AppError> {
    let bytes = serde_json::to_vec(model)?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("sha256:{:x}", hasher.finalize()))
}

fn changed_life_model_sections(
    before: &LifeModel,
    after: &LifeModel,
) -> Result<Vec<String>, AppError> {
    let before = serde_json::to_value(before)?;
    let after = serde_json::to_value(after)?;
    let mut changed = Vec::new();
    for section in [
        "metadata",
        "identity",
        "goals",
        "capabilities",
        "state",
        "relationships",
        "preferences",
        "evolution_rules",
    ] {
        if before.get(section) != after.get(section) {
            changed.push(section.to_string());
        }
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use openlife_core::agent::{
        AgentProposal, ProposalSource, ProposalStatus, ProposalType, RiskLevel,
    };
    use std::collections::HashMap;

    fn test_app_state(temp_dir: &tempfile::TempDir) -> Arc<AppState> {
        let config = openlife_core::config::AppConfig::default();
        let hot_cache: openlife_core::memory_cache::SharedHotCache = Arc::new(
            tokio::sync::RwLock::new(openlife_core::memory_cache::HotMemoryCache::default()),
        );
        Arc::new(AppState {
            config: Arc::new(tokio::sync::Mutex::new(config.clone())),
            life_model_manager: Arc::new(tokio::sync::Mutex::new(
                openlife_core::life_model::LifeModelManager::new(
                    temp_dir.path().join("life-model").join("current"),
                ),
            )),
            memory_store: Arc::new(tokio::sync::Mutex::new(
                openlife_core::memory::MemoryStore::new_in_memory().unwrap(),
            )),
            mcp_registry: Arc::new(tokio::sync::Mutex::new(
                openlife_core::mcp::McpRegistry::new(),
            )),
            intent_router: Arc::new(tokio::sync::Mutex::new(
                openlife_core::router::IntentRouter::new(),
            )),
            layer_router: Arc::new(tokio::sync::Mutex::new(
                openlife_core::layer_router::LayerRouter::new(),
            )),
            scheduler: Arc::new(tokio::sync::Mutex::new(
                openlife_core::scheduler::InferenceScheduler::new(
                    config.local_model.clone(),
                    config.prefer_local_model,
                    config.llm.provider.clone(),
                    config.llm.openai_base.clone(),
                    config.llm.openai_key.clone(),
                    config.llm.chat_model.clone(),
                    config.llm.embedding_model.clone(),
                    config.llm.embedding_enabled,
                ),
            )),
            privacy_engine: Arc::new(tokio::sync::Mutex::new(
                openlife_core::privacy::PrivacyEngine::new(),
            )),
            version_manager: Arc::new(tokio::sync::Mutex::new(
                openlife_core::versioning::VersionManager::new(
                    temp_dir.path().join("life-model").join("versions"),
                ),
            )),
            feedback_store: Arc::new(tokio::sync::Mutex::new(
                openlife_core::feedback::FeedbackStore::new_in_memory().unwrap(),
            )),
            vector_store: Arc::new(tokio::sync::Mutex::new(
                openlife_core::vectors::VectorStore::new_in_memory().unwrap(),
            )),
            vector_persistence_mode: crate::state::VectorPersistenceMode::Enabled,
            builder_sessions: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            builder_session_store: Arc::new(tokio::sync::Mutex::new(
                openlife_core::builder::BuilderSessionStore::new(
                    temp_dir.path().join("builder_sessions.json"),
                ),
            )),
            a2a_sidecar: Arc::new(tokio::sync::Mutex::new(
                crate::a2a_sidecar::A2ASidecar::new(crate::a2a_server::configured_a2a_port()),
            )),
            last_snapshot_date: Arc::new(tokio::sync::Mutex::new(None)),
            mcp_audit_store: Arc::new(tokio::sync::Mutex::new(
                openlife_core::mcp_audit::McpAuditStore::new(temp_dir.path().join("mcp_audit.db")),
            )),
            agent_run_store: Some(Arc::new(tokio::sync::Mutex::new(
                openlife_core::agent::AgentRunStore::new_in_memory().unwrap(),
            ))),
            evidence_store: Arc::new(tokio::sync::Mutex::new(
                openlife_core::agent::EvidenceStore::new_in_memory().unwrap(),
            )),
            life_event_store: Some(Arc::new(tokio::sync::Mutex::new(
                openlife_core::agent::LifeEventStore::new_in_memory().unwrap(),
            ))),
            heuristic_store: Arc::new(tokio::sync::Mutex::new({
                let store = openlife_core::agent::HeuristicStore::new_in_memory().unwrap();
                store.seed_mvp_heuristics().unwrap();
                store
            })),
            policy_store: Arc::new(openlife_core::agent::PolicyStore::mvp_builtin()),
            proposal_store: Some(Arc::new(tokio::sync::Mutex::new(
                openlife_core::agent::ProposalStore::new_in_memory().unwrap(),
            ))),
            memory_lifecycle_store: Some(Arc::new(tokio::sync::Mutex::new(
                openlife_core::agent::MemoryLifecycleStore::new_in_memory().unwrap(),
            ))),
            plan_execute_session_store: Some(Arc::new(tokio::sync::Mutex::new(
                openlife_core::agent::PlanExecuteSessionStore::new_in_memory().unwrap(),
            ))),
            main_chat_agent_session_store: Some(Arc::new(tokio::sync::Mutex::new(
                openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStore::new_in_memory()
                    .unwrap(),
            ))),
            main_chat_action_queue_store: Some(Arc::new(tokio::sync::Mutex::new(
                openlife_core::agent::main_chat_agent_v1::ActionQueueStore::new_in_memory()
                    .unwrap(),
            ))),
            main_chat_agent_event_store: None,
            main_chat_selected_skill_ids: Arc::new(tokio::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            main_chat_runtime_state: crate::state::MainChatRuntimeState::shared(),
            patch_store: Some(Arc::new(tokio::sync::Mutex::new(
                openlife_core::life_model::patch_store::PatchStore::new_in_memory().unwrap(),
            ))),
            rollout_metrics_store: None,
            tool_permission_store: Arc::new(tokio::sync::Mutex::new(
                openlife_core::tool_permissions::ToolPermissionStore::new_in_memory().unwrap(),
            )),
            skill_registry: Arc::new(tokio::sync::Mutex::new(
                openlife_core::skills::SkillRegistry::built_in(),
            )),
            plugin_registry: Arc::new(tokio::sync::Mutex::new(
                openlife_core::plugins::PluginRegistry::new(temp_dir.path().join("plugins")),
            )),
            hot_cache,
            proposal_engine: Arc::new(tokio::sync::Mutex::new(
                openlife_core::agent::ProposalEngine::new(),
            )),
            startup_warnings: vec![],
            provider_health_cache: Arc::new(tokio::sync::Mutex::new(None)),
            scheduled_task_mutex: Arc::new(tokio::sync::Mutex::new(())),
            runtime_clock_source: Arc::new(tokio::sync::Mutex::new(
                crate::main_chat_runtime_facts::MainChatRuntimeClockSource::default(),
            )),
            web_search_fixture_output: Arc::new(tokio::sync::Mutex::new(None)),
            shutdown_notify: Arc::new(tokio::sync::Notify::new()),
        })
    }

    #[tokio::test]
    async fn get_life_model_returns_default_when_empty() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);

        let result = get_life_model_with_state(&state).await;
        assert!(result.is_ok());
        let model = result.unwrap();
        assert!(model.is_effectively_empty());
    }

    #[tokio::test]
    async fn lifemodel_closed_loop_current_view_shows_accepted_communication_style_with_trace() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let mut proposal = AgentProposal::new(
            ProposalType::PreferenceUpdate,
            "/preferences/communication",
            serde_json::json!("先共情，再给结构化建议"),
            "用户接受低风险沟通偏好。",
            0.92,
            RiskLevel::Low,
            ProposalSource::FeedbackEvolution,
        );
        proposal.run_id = Some("run-communication-style-1".into());
        proposal.source_detail = Some("maturation:preference.communication".into());
        let proposal_id = proposal.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();

        crate::commands::proposal::accept_proposal_with_state(proposal_id.clone(), &state)
            .await
            .unwrap();

        let view = get_life_model_current_view_with_state(&state)
            .await
            .unwrap();
        assert_eq!(view.path, COMMUNICATION_STYLE_CANONICAL_PATH);
        assert_eq!(view.value.as_deref(), Some("先共情，再给结构化建议"));
        assert_eq!(view.unavailable_reason, None);
        assert_eq!(view.current_value_source, "accepted_proposal");
        let change = view.change.expect("accepted preference has trace");
        assert_eq!(change.path, COMMUNICATION_STYLE_CANONICAL_PATH);
        assert_eq!(change.proposal_id, proposal_id);
        assert_eq!(change.proposal_status, "accepted");
        assert_eq!(change.proposal_source, "feedback_evolution");
        assert_eq!(
            change.proposal_source_detail.as_deref(),
            Some("maturation:preference.communication")
        );
        assert_eq!(
            change.proposal_run_id.as_deref(),
            Some("run-communication-style-1")
        );
        assert_eq!(
            change.source_excerpt.as_deref(),
            Some("用户接受低风险沟通偏好。")
        );
        assert_eq!(change.source_unavailable_reason, None);
        assert_eq!(change.patch_unavailable_reason, None);
        assert!(change.patch_id.is_some());
        assert_eq!(change.patch_status.as_deref(), Some("applied"));
        assert_eq!(
            change.patch_path.as_deref(),
            Some(COMMUNICATION_STYLE_CANONICAL_PATH)
        );
        assert_eq!(change.snapshot_unavailable_reason, None);
        assert_eq!(change.snapshot_versions.len(), 2);
        assert!(change.current_matches_accepted_after);
    }

    #[tokio::test]
    async fn lifemodel_closed_loop_current_view_reports_missing_patch_and_snapshot_reasons() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let mut model = LifeModel::default();
        model.preferences.communication_style = "直接一点，先给结论".into();
        state.life_model_manager.lock().await.save(&model).unwrap();
        let mut proposal = AgentProposal::new(
            ProposalType::PreferenceUpdate,
            "preferences.communication_style",
            serde_json::json!("直接一点，先给结论"),
            "用户曾接受沟通偏好。",
            0.88,
            RiskLevel::Low,
            ProposalSource::Manual,
        );
        proposal.accept();
        let proposal_id = proposal.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();

        let view = get_life_model_current_view_with_state(&state)
            .await
            .unwrap();

        assert_eq!(view.value.as_deref(), Some("直接一点，先给结论"));
        let change = view.change.expect("accepted proposal still traced");
        assert_eq!(change.proposal_id, proposal_id);
        assert_eq!(
            change.patch_unavailable_reason.as_deref(),
            Some("patch_missing")
        );
        assert_eq!(
            change.snapshot_unavailable_reason.as_deref(),
            Some("snapshot_missing")
        );
    }

    #[tokio::test]
    async fn lifemodel_closed_loop_reject_and_postpone_do_not_write_current_preference() {
        for action in ["reject", "postpone"] {
            let temp_dir = tempfile::tempdir().unwrap();
            let state = test_app_state(&temp_dir);
            let proposal = AgentProposal::new(
                ProposalType::PreferenceUpdate,
                "preferences.communication",
                serde_json::json!(format!("{action} should not write")),
                "用户还没有接受。",
                0.8,
                RiskLevel::Low,
                ProposalSource::FeedbackEvolution,
            );
            let proposal_id = proposal.id.clone();
            state
                .proposal_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .create_proposal(&proposal)
                .unwrap();
            if action == "reject" {
                crate::commands::proposal::reject_proposal_with_state(proposal_id.clone(), &state)
                    .await
                    .unwrap();
            } else {
                crate::commands::proposal::postpone_proposal_with_state(
                    proposal_id.clone(),
                    &state,
                )
                .await
                .unwrap();
            }

            let model = state.life_model_manager.lock().await.load().unwrap();
            assert_ne!(
                model.preferences.communication_style,
                format!("{action} should not write")
            );
            let view = get_life_model_current_view_with_state(&state)
                .await
                .unwrap();
            assert_eq!(view.value, None);
            assert_eq!(
                view.unavailable_reason.as_deref(),
                Some("current_value_empty")
            );
            assert!(view.change.is_none());
        }
    }

    #[tokio::test]
    async fn lifemodel_closed_loop_failed_accept_does_not_become_visible_current_preference() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let proposal = AgentProposal::new(
            ProposalType::PreferenceUpdate,
            "preferences.communication_style",
            serde_json::json!({ "style": "object values are invalid for this field" }),
            "坏数据不应显示为已接受偏好。",
            0.8,
            RiskLevel::Low,
            ProposalSource::Manual,
        );
        let proposal_id = proposal.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&proposal)
            .unwrap();

        let err =
            crate::commands::proposal::accept_proposal_with_state(proposal_id.clone(), &state)
                .await
                .expect_err("invalid preference payload must fail accept");
        assert!(!err.trim().is_empty());

        let stored = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_proposal(&proposal_id)
            .unwrap()
            .unwrap();
        assert_eq!(stored.status, ProposalStatus::Pending);
        let view = get_life_model_current_view_with_state(&state)
            .await
            .unwrap();
        assert_eq!(view.value, None);
        assert_eq!(
            view.unavailable_reason.as_deref(),
            Some("current_value_empty")
        );
        assert!(view.change.is_none());
    }

    #[tokio::test]
    async fn lifemodel_closed_loop_current_view_deduplicates_old_slash_and_dot_paths() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let mut model = LifeModel::default();
        model.preferences.communication_style = "新的 canonical 沟通偏好".into();
        state.life_model_manager.lock().await.save(&model).unwrap();

        let mut old = AgentProposal::new(
            ProposalType::PreferenceUpdate,
            "/preferences/communication",
            serde_json::json!("旧 slash 沟通偏好"),
            "旧 alias proposal。",
            0.7,
            RiskLevel::Low,
            ProposalSource::Manual,
        );
        old.accept();
        let old_id = old.id.clone();
        let mut current = AgentProposal::new(
            ProposalType::PreferenceUpdate,
            "preferences.communication_style",
            serde_json::json!("新的 canonical 沟通偏好"),
            "当前 accepted proposal。",
            0.9,
            RiskLevel::Low,
            ProposalSource::Manual,
        );
        current.accept();
        let current_id = current.id.clone();
        {
            let store = state.proposal_store.as_ref().unwrap().lock().await;
            store.create_proposal(&old).unwrap();
            store.create_proposal(&current).unwrap();
        }

        let view = get_life_model_current_view_with_state(&state)
            .await
            .unwrap();
        let change = view.change.expect("current matching accepted proposal");
        assert_eq!(view.path, COMMUNICATION_STYLE_CANONICAL_PATH);
        assert_eq!(change.path, COMMUNICATION_STYLE_CANONICAL_PATH);
        assert_eq!(change.proposal_id, current_id);
        assert_ne!(change.proposal_id, old_id);
        assert!(change.current_matches_accepted_after);
    }

    #[tokio::test]
    async fn save_and_get_life_model_roundtrip() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);

        // Create a life model with some data
        let mut model = LifeModel::default();
        model.identity.name = "TestUser".to_string();
        model
            .identity
            .values
            .push(openlife_core::life_model::ValueItem {
                name: "Honesty".to_string(),
                weight: 9,
                description: "Being truthful".to_string(),
            });

        // Save
        let save_result = save_life_model_with_state(
            model.clone(),
            &state,
            Some(GovernedManualLifeModelOverrideRequest::editor_save()),
        )
        .await;
        assert!(save_result.is_ok());

        // Get back
        let result = get_life_model_with_state(&state).await;
        assert!(result.is_ok());
        let retrieved = result.unwrap();
        assert_eq!(retrieved.identity.name, "TestUser");
        assert_eq!(retrieved.identity.values.len(), 1);
        assert_eq!(retrieved.identity.values[0].name, "Honesty");
    }

    #[tokio::test]
    async fn manual_lifemodel_save_without_governed_request_fails_closed_and_writes_nothing() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let mut model = LifeModel::default();
        model.identity.name = "RAW_IDENTITY_SECRET".to_string();

        let err = save_life_model_with_state(model, &state, None)
            .await
            .expect_err("manual LifeModel save requires governed request");

        assert!(matches!(err, AppError::PermissionDenied { .. }));
        assert!(err.message().contains("save_life_model"));
        assert!(err.message().contains("governed manual override request"));
        assert!(err.message().contains("riskAcknowledged=true"));
        assert!(get_life_model_with_state(&state)
            .await
            .unwrap()
            .is_effectively_empty());
    }

    #[tokio::test]
    async fn manual_lifemodel_override_audit_records_after_successful_save_and_hashes_persisted_model(
    ) {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let before = get_life_model_with_state(&state).await.unwrap();
        let before_audit_count = manual_override_audit_count_today(&state).await;
        let heuristic_count_before = heuristic_record_count(&state).await;

        let model = raw_marker_life_model();
        let save_report = save_life_model_with_state(
            model,
            &state,
            Some(GovernedManualLifeModelOverrideRequest::editor_save()),
        )
        .await
        .unwrap();

        let after = get_life_model_with_state(&state).await.unwrap();
        assert_eq!(after.identity.name, "RAW_IDENTITY_SECRET");
        assert_eq!(
            manual_override_audit_count_today(&state).await,
            before_audit_count + 1
        );

        let report = evaluate_manual_lifemodel_override_audit(
            &before,
            &after,
            &GovernedManualLifeModelOverrideRequest::editor_save(),
            save_report.pre_change_snapshot_version.clone(),
        )
        .unwrap();
        assert_eq!(report.source, "manual_lifemodel_editor");
        assert_eq!(report.command_function_name, "save_life_model");
        assert_eq!(report.risk_class, "GovernedManualOverride");
        assert_eq!(report.operation_purpose, "manual_lifemodel_editor_save");
        assert!(report.governed_operation);
        assert!(report.pre_change_snapshot_created);
        assert!(report
            .pre_change_snapshot_version
            .as_ref()
            .is_some_and(|version| !version.is_empty()));
        assert!(report.manual_override);
        assert!(!report.proposal_first);
        assert!(!report.still_legacy_direct_write);
        assert!(report.before_hash.starts_with("sha256:"));
        assert!(report.after_hash.starts_with("sha256:"));
        assert_ne!(report.before_hash, report.after_hash);
        assert!(report
            .changed_section_names
            .contains(&"identity".to_string()));
        assert!(report.changed_section_names.contains(&"goals".to_string()));
        assert_eq!(
            report.changed_section_count,
            report.changed_section_names.len()
        );

        assert_eq!(agent_run_count(&state).await, 0);
        assert_eq!(proposal_count(&state).await, 0);
        assert_eq!(patch_count(&state).await, 0);
        assert_eq!(heuristic_record_count(&state).await, heuristic_count_before);
    }

    #[tokio::test]
    async fn manual_lifemodel_override_audit_report_and_detail_are_metadata_safe() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let before = get_life_model_with_state(&state).await.unwrap();
        let mut after = raw_marker_life_model();
        after.state.health_status.physical = "RAW_HEALTH_PRIVACY_SECRET".to_string();

        let report = record_manual_lifemodel_override_audit_with_state(
            &state,
            &before,
            &after,
            &GovernedManualLifeModelOverrideRequest::editor_save(),
            Some("snapshot-v1".into()),
        )
        .await
        .unwrap();
        assert!(report.metadata_safe);
        assert!(!report.contains_raw_content);
        assert_eq!(manual_override_audit_count_today(&state).await, 1);

        let detail: serde_json::Value =
            serde_json::from_str(&report.audit_detail_json).expect("audit detail is json");
        assert_eq!(detail["source"], "manual_lifemodel_editor");
        assert_eq!(detail["governedOperation"], true);
        assert_eq!(detail["operationPurpose"], "manual_lifemodel_editor_save");
        assert_eq!(detail["preChangeSnapshotCreated"], true);
        assert_eq!(detail["preChangeSnapshotVersion"], "snapshot-v1");
        assert_eq!(detail["manualOverride"], true);
        assert_eq!(detail["proposalFirst"], false);
        assert_eq!(detail["stillLegacyDirectWrite"], false);
        assert_eq!(detail["riskClass"], "GovernedManualOverride");
        assert!(detail["beforeHash"]
            .as_str()
            .unwrap()
            .starts_with("sha256:"));
        assert!(detail["afterHash"].as_str().unwrap().starts_with("sha256:"));

        let debug_dump = format!("{report:?}");
        for forbidden in raw_lifemodel_markers() {
            assert!(
                !debug_dump.contains(forbidden),
                "audit debug leaked raw marker {forbidden}"
            );
            assert!(
                !report.audit_detail_json.contains(forbidden),
                "audit detail leaked raw marker {forbidden}"
            );
        }
    }

    #[test]
    fn manual_lifemodel_override_save_path_does_not_call_runtime_model_or_tool_surfaces() {
        let source_path = format!("{}/src/commands/life_model.rs", env!("CARGO_MANIFEST_DIR"));
        let source = std::fs::read_to_string(source_path).expect("read life_model.rs");
        let combined_body = [
            extract_rust_function_body(&source, "pub(crate) async fn save_life_model_with_state"),
            extract_rust_function_body(
                &source,
                "pub(crate) async fn record_manual_lifemodel_override_audit_with_state",
            ),
            extract_rust_function_body(
                &source,
                "pub(crate) fn evaluate_manual_lifemodel_override_audit",
            ),
        ]
        .join("\n");

        for forbidden in [
            "AgentRuntime",
            "execute_multi_strategy_agent_preview",
            "execute_task",
            ".generate(",
            "scheduler",
            "mcp_registry",
            "tool_permission_store",
            "proposal_engine",
            "ProposalEngine",
            "McpRegistry",
            "ToolPermission",
            "llm",
        ] {
            assert!(
                !combined_body.contains(forbidden),
                "W80 manual override save/audit path must not call runtime/model/tool surface {forbidden}"
            );
        }
    }

    fn raw_marker_life_model() -> LifeModel {
        let mut model = LifeModel::default();
        model.identity.name = "RAW_IDENTITY_SECRET".to_string();
        model
            .identity
            .values
            .push(openlife_core::life_model::ValueItem {
                name: "RAW_VALUE_SECRET".to_string(),
                weight: 9,
                description: "RAW_VALUE_DESCRIPTION_SECRET".to_string(),
            });
        model
            .goals
            .short_term
            .push(openlife_core::life_model::GoalItem {
                name: "RAW_GOAL_SECRET".to_string(),
                description: "RAW_GOAL_DESCRIPTION_SECRET".to_string(),
                priority: 8,
                ..Default::default()
            });
        model
            .relationships
            .inner_circle
            .push(openlife_core::life_model::Relationship {
                name: "RAW_RELATIONSHIP_SECRET".to_string(),
                relationship_type: "RAW_RELATIONSHIP_TYPE_SECRET".to_string(),
                importance: 7,
                notes: "RAW_RELATIONSHIP_NOTES_SECRET".to_string(),
            });
        model.state.health_status.mental = "RAW_HEALTH_SECRET".to_string();
        model
    }

    fn raw_lifemodel_markers() -> [&'static str; 10] {
        [
            "RAW_IDENTITY_SECRET",
            "RAW_VALUE_SECRET",
            "RAW_VALUE_DESCRIPTION_SECRET",
            "RAW_GOAL_SECRET",
            "RAW_GOAL_DESCRIPTION_SECRET",
            "RAW_RELATIONSHIP_SECRET",
            "RAW_RELATIONSHIP_TYPE_SECRET",
            "RAW_RELATIONSHIP_NOTES_SECRET",
            "RAW_HEALTH_SECRET",
            "RAW_HEALTH_PRIVACY_SECRET",
        ]
    }

    async fn manual_override_audit_count_today(state: &Arc<AppState>) -> i64 {
        let feedback = state.feedback_store.lock().await;
        feedback
            .count_event_today("manual_lifemodel_override_audit")
            .unwrap()
    }

    async fn agent_run_count(state: &Arc<AppState>) -> usize {
        let store = state.agent_run_store.as_ref().unwrap().lock().await;
        store.list_runs(20, 0).unwrap().len()
    }

    async fn proposal_count(state: &Arc<AppState>) -> usize {
        let store = state.proposal_store.as_ref().unwrap().lock().await;
        store.list_all_proposals(20, 0).unwrap().len()
    }

    async fn patch_count(state: &Arc<AppState>) -> usize {
        let store = state.patch_store.as_ref().unwrap().lock().await;
        store.patch_count().unwrap()
    }

    async fn heuristic_record_count(state: &Arc<AppState>) -> usize {
        let store = state.heuristic_store.lock().await;
        store
            .query(openlife_core::agent::HeuristicQuery::default())
            .unwrap()
            .len()
    }

    fn extract_rust_function_body(source: &str, signature: &str) -> String {
        let signature_start = source.find(signature).expect("function signature exists");
        let brace_start = source[signature_start..]
            .find('{')
            .map(|index| signature_start + index)
            .expect("function body starts");
        let mut depth = 0usize;

        for (offset, ch) in source[brace_start..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        let end = brace_start + offset + ch.len_utf8();
                        return source[brace_start..end].to_string();
                    }
                }
                _ => {}
            }
        }

        panic!("function body closes");
    }
}
