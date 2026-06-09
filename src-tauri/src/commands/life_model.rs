use crate::errors::AppError;
use crate::legacy_write_convergence::{
    LifeModelMaterializerCallerContext, LifeModelMaterializerCallerKind,
    LifeModelMaterializerCallerPurpose,
};
use crate::{persist_life_model, AppState};
use openlife_core::life_model::LifeModel;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tauri::State;

const MANUAL_LIFEMODEL_OVERRIDE_AUDIT_EVENT: &str = "manual_lifemodel_override_audit";
const MANUAL_LIFEMODEL_OVERRIDE_SOURCE: &str = "manual_lifemodel_editor";
const MANUAL_LIFEMODEL_OVERRIDE_COMMAND: &str = "save_life_model";
const MANUAL_LIFEMODEL_OVERRIDE_RISK_CLASS: &str = "GovernedManualOverride";

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
            builder_sessions: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            builder_session_store: Arc::new(tokio::sync::Mutex::new(
                openlife_core::builder::BuilderSessionStore::new(
                    temp_dir.path().join("builder_sessions.json"),
                ),
            )),
            a2a_sidecar: Arc::new(tokio::sync::Mutex::new(
                crate::a2a_sidecar::A2ASidecar::new(8765),
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
            heuristic_store: Arc::new(tokio::sync::Mutex::new({
                let store = openlife_core::agent::HeuristicStore::new_in_memory().unwrap();
                store.seed_mvp_heuristics().unwrap();
                store
            })),
            policy_store: Arc::new(openlife_core::agent::PolicyStore::mvp_builtin()),
            proposal_store: Some(Arc::new(tokio::sync::Mutex::new(
                openlife_core::agent::ProposalStore::new_in_memory().unwrap(),
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
