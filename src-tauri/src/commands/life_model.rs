use crate::artifact_materializer::{
    capture_artifact_target_precondition, ArtifactTargetPrecondition,
};
use crate::errors::AppError;
use crate::AppState;
use openlife_core::agent::{AgentProposal, ProposalSource, ProposalType, RiskLevel};
use openlife_core::life_model::legacy_migration::{
    LegacyLifeModelMigrationPreviewV2, LegacyLifeModelMigrationSelectionV2,
    LIFE_MODEL_V2_LEGACY_MIGRATION_PATH,
};
use openlife_core::life_model::v2::{
    life_model_item_digest_v2, LifeModelSectionV2, LifeModelTypedDiffV2, LifeModelTypedOperationV2,
    LifeModelUserValueV2, DEFAULT_LIFE_MODEL_V2_MODEL_ID, LIFE_MODEL_V2_TYPED_DIFF_PATH,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn confirm_lifemodel_learning_candidate(
    candidate_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<crate::life_model_learning::ConfirmLifeModelLearningCandidateReceipt, String> {
    crate::life_model_learning::confirm_candidate_with_state(state.inner(), &candidate_id).await
}

#[tauri::command]
pub async fn delete_lifemodel_learning_candidate(
    candidate_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<crate::life_model_learning::DeleteLifeModelLearningCandidateReceipt, String> {
    crate::life_model_learning::delete_candidate_with_state(state.inner(), &candidate_id).await
}

#[tauri::command]
pub async fn reject_lifemodel_learning_candidate(
    candidate_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<openlife_core::agent::LifeModelLearningDecisionReceipt, String> {
    crate::life_model_learning::reject_candidate_with_state(state.inner(), &candidate_id).await
}

#[tauri::command]
pub async fn pause_lifemodel_learning_suggestion_class(
    candidate_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<openlife_core::agent::LifeModelLearningDecisionReceipt, String> {
    crate::life_model_learning::pause_candidate_class_with_state(state.inner(), &candidate_id).await
}

#[tauri::command]
pub async fn stage_lifemodel_learning_candidate(
    candidate_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<crate::life_model_learning::StageLifeModelLearningCandidateReceipt, String> {
    crate::life_model_learning::stage_candidate_for_review_with_state(state.inner(), &candidate_id)
        .await
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EditLifeModelLearningProposalRequest {
    pub proposal_id: String,
    pub statement: String,
}

#[tauri::command]
pub async fn edit_lifemodel_learning_proposal(
    request: EditLifeModelLearningProposalRequest,
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    crate::commands::proposal::edit_lifemodel_learning_proposal_with_state(
        request.proposal_id,
        request.statement,
        state.inner(),
    )
    .await
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum LifeModelV2UserChange {
    Add {
        section: LifeModelSectionV2,
        value: LifeModelUserValueV2,
    },
    Replace {
        section: LifeModelSectionV2,
        item_id: String,
        value: LifeModelUserValueV2,
    },
    Remove {
        section: LifeModelSectionV2,
        item_id: String,
    },
    Clear,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DraftLifeModelV2ChangeRequest {
    pub base_version: Option<u64>,
    pub base_document_digest: Option<String>,
    pub change: LifeModelV2UserChange,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DraftLifeModelV2RollbackRequest {
    pub base_version: u64,
    pub base_document_digest: String,
    pub target_version: u64,
    pub target_document_digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LifeModelV2ExportFormat {
    Yaml,
    Json,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DraftLifeModelV2ExportRequest {
    pub model_version: u64,
    pub document_digest: String,
    pub projection_digest: Option<String>,
    pub format: LifeModelV2ExportFormat,
    pub target_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LifeModelV2ProposalReceipt {
    pub proposal_id: String,
    pub status: String,
    pub base_version: Option<u64>,
    pub base_document_digest: Option<String>,
    pub result_document_digest: Option<String>,
    pub operation_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DraftLegacyLifeModelMigrationRequest {
    pub source_digest: String,
    pub selections: Vec<LegacyLifeModelMigrationSelectionV2>,
    pub non_lifemodel_items_acknowledged: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DraftLegacyLifeModelMigrationReceipt {
    pub proposal_id: String,
    pub status: String,
    pub source_digest: String,
    pub result_document_digest: String,
    pub included_candidate_count: usize,
    pub excluded_candidate_count: usize,
    pub durable_write_executed: bool,
}

pub(crate) async fn draft_legacy_lifemodel_migration_with_state(
    request: DraftLegacyLifeModelMigrationRequest,
    state: &Arc<AppState>,
) -> Result<DraftLegacyLifeModelMigrationReceipt, AppError> {
    let manager = state.life_model_manager.lock().await;
    if manager
        .load_v2_current(DEFAULT_LIFE_MODEL_V2_MODEL_ID)?
        .is_some()
        || manager
            .load_v2_cutover(DEFAULT_LIFE_MODEL_V2_MODEL_ID)?
            .is_some()
    {
        return Err(AppError::internal(
            "lifemodel_v2_migration_existing_canonical_owner",
        ));
    }
    let source = manager
        .load_legacy_source_for_migration()?
        .ok_or_else(|| AppError::not_found("legacy_lifemodel_source_missing"))?;
    let preview = LegacyLifeModelMigrationPreviewV2::from_legacy_yaml(&source)?;
    if preview.source_digest != request.source_digest {
        return Err(AppError::internal("lifemodel_v2_migration_preview_stale"));
    }
    let mut proposal = AgentProposal::new(
        ProposalType::LifeModelUpdate,
        LIFE_MODEL_V2_LEGACY_MIGRATION_PATH,
        Value::Null,
        "Review the exact legacy LifeModel fields before establishing the canonical v2 owner.",
        1.0,
        RiskLevel::High,
        ProposalSource::Manual,
    );
    proposal.source_detail = Some("legacy_lifemodel_migration".into());
    let plan = preview.build_migration_plan(
        &request.selections,
        request.non_lifemodel_items_acknowledged,
        &proposal.created_at.to_rfc3339(),
    )?;
    proposal.before = Some(serde_json::json!({
        "legacySourceDigest": preview.source_digest,
        "sourceItemCount": preview.items.len(),
        "historyPreserved": true,
        "canonicalV2Present": false,
    }));
    proposal.after = serde_json::to_value(&plan)?;
    drop(manager);
    let proposal_store = state
        .proposal_store
        .as_ref()
        .ok_or_else(|| AppError::db("proposal_store_unavailable"))?;
    proposal_store.lock().await.create_proposal(&proposal)?;
    Ok(DraftLegacyLifeModelMigrationReceipt {
        proposal_id: proposal.id,
        status: "review_required".into(),
        source_digest: plan.legacy_source_digest,
        result_document_digest: plan.result_document_digest,
        included_candidate_count: plan.included_candidate_ids.len(),
        excluded_candidate_count: plan.excluded_candidate_ids.len(),
        durable_write_executed: false,
    })
}

#[tauri::command]
pub async fn draft_legacy_lifemodel_migration(
    request: DraftLegacyLifeModelMigrationRequest,
    state: State<'_, Arc<AppState>>,
) -> Result<DraftLegacyLifeModelMigrationReceipt, AppError> {
    draft_legacy_lifemodel_migration_with_state(request, state.inner()).await
}

fn ensure_exact_lifemodel_v2_base(
    current: Option<&openlife_core::life_model::v2::LifeModelVersionV2>,
    base_version: Option<u64>,
    base_document_digest: Option<&str>,
) -> Result<(), AppError> {
    match current {
        Some(current)
            if base_version == Some(current.model_version)
                && base_document_digest == Some(current.document_digest.as_str()) =>
        {
            Ok(())
        }
        None if base_version.is_none() && base_document_digest.is_none() => Ok(()),
        _ => Err(AppError::internal("lifemodel_v2_user_change_stale_base")),
    }
}

fn reviewed_item_source_refs(
    proposal_id: &str,
    current_version: Option<u64>,
    item_id: &str,
) -> Vec<String> {
    let mut refs = vec![format!("proposal:{proposal_id}")];
    if let Some(version) = current_version {
        refs.push(format!(
            "lifemodel-version:primary:{version}:item:{item_id}"
        ));
    }
    refs
}

pub(crate) async fn draft_lifemodel_v2_change_with_state(
    request: DraftLifeModelV2ChangeRequest,
    state: &Arc<AppState>,
) -> Result<LifeModelV2ProposalReceipt, AppError> {
    let manager = state.life_model_manager.lock().await;
    let current = manager.load_v2_current(DEFAULT_LIFE_MODEL_V2_MODEL_ID)?;
    if current.is_none() && manager.inspect_legacy_inventory()?.is_some() {
        return Err(AppError::internal(
            "lifemodel_v2_user_change_requires_legacy_migration",
        ));
    }
    ensure_exact_lifemodel_v2_base(
        current.as_ref(),
        request.base_version,
        request.base_document_digest.as_deref(),
    )?;
    let allow_empty_result = current.is_some();

    let mut proposal = AgentProposal::new(
        ProposalType::LifeModelUpdate,
        LIFE_MODEL_V2_TYPED_DIFF_PATH,
        Value::Null,
        "Apply one explicit user-reviewed LifeModel v2 change.",
        1.0,
        RiskLevel::Medium,
        ProposalSource::Manual,
    );
    proposal.source_detail = Some("lifemodel_v2_user_edit".into());
    proposal.base_hash = request.base_document_digest.clone();
    let confirmed_at = proposal.created_at.to_rfc3339();
    let current_version = current.as_ref().map(|version| version.model_version);
    let mut destructive = false;
    let mut sensitive = false;
    let operations = match request.change {
        LifeModelV2UserChange::Add { section, value } => {
            sensitive = section == LifeModelSectionV2::ImportantRelationships;
            let item_id = format!("user:{}", proposal.id);
            let item = value.into_item(
                item_id.clone(),
                reviewed_item_source_refs(&proposal.id, current_version, &item_id),
                confirmed_at,
            );
            vec![LifeModelTypedOperationV2::Add { section, item }]
        }
        LifeModelV2UserChange::Replace {
            section,
            item_id,
            value,
        } => {
            sensitive = section == LifeModelSectionV2::ImportantRelationships;
            let before = current
                .as_ref()
                .and_then(|version| version.document.item(section, &item_id))
                .ok_or_else(|| AppError::not_found("lifemodel_v2_edit_item_missing"))?;
            let item = value.into_item(
                item_id.clone(),
                reviewed_item_source_refs(&proposal.id, current_version, &item_id),
                confirmed_at,
            );
            vec![LifeModelTypedOperationV2::Replace {
                section,
                item_id,
                before_item_digest: life_model_item_digest_v2(&before)?,
                item,
            }]
        }
        LifeModelV2UserChange::Remove { section, item_id } => {
            destructive = true;
            sensitive = section == LifeModelSectionV2::ImportantRelationships;
            let before = current
                .as_ref()
                .and_then(|version| version.document.item(section, &item_id))
                .ok_or_else(|| AppError::not_found("lifemodel_v2_remove_item_missing"))?;
            vec![LifeModelTypedOperationV2::Remove {
                section,
                item_id,
                before_item_digest: life_model_item_digest_v2(&before)?,
            }]
        }
        LifeModelV2UserChange::Clear => {
            destructive = true;
            let current = current
                .as_ref()
                .ok_or_else(|| AppError::not_found("lifemodel_v2_clear_current_missing"))?;
            if current.document.is_empty() {
                return Err(AppError::internal("lifemodel_v2_clear_already_empty"));
            }
            current
                .document
                .items()
                .into_iter()
                .map(|(section, item)| {
                    Ok(LifeModelTypedOperationV2::Remove {
                        section,
                        item_id: item.id().to_string(),
                        before_item_digest: life_model_item_digest_v2(&item)?,
                    })
                })
                .collect::<anyhow::Result<Vec<_>>>()?
        }
    };
    if destructive || sensitive {
        proposal.risk_level = RiskLevel::High;
    }
    let diff = LifeModelTypedDiffV2::from_operations_for_review(
        DEFAULT_LIFE_MODEL_V2_MODEL_ID,
        current.as_ref(),
        operations,
        allow_empty_result,
    )?;
    if diff.base_version != request.base_version
        || diff.base_document_digest != request.base_document_digest
    {
        return Err(AppError::internal("lifemodel_v2_user_change_stale_base"));
    }
    proposal.before = Some(serde_json::json!({
        "baseVersion": request.base_version,
        "baseDocumentDigest": request.base_document_digest,
        "itemCount": current.as_ref().map(|version| version.document.total_item_count()).unwrap_or(0),
    }));
    proposal.after = serde_json::to_value(&diff)?;
    drop(manager);
    let proposal_store = state
        .proposal_store
        .as_ref()
        .ok_or_else(|| AppError::db("proposal_store_unavailable"))?;
    proposal_store.lock().await.create_proposal(&proposal)?;
    Ok(LifeModelV2ProposalReceipt {
        proposal_id: proposal.id,
        status: "review_required".into(),
        base_version: diff.base_version,
        base_document_digest: diff.base_document_digest,
        result_document_digest: Some(diff.result_document_digest),
        operation_count: diff.operations.len(),
    })
}

#[tauri::command]
pub async fn draft_lifemodel_v2_change(
    request: DraftLifeModelV2ChangeRequest,
    state: State<'_, Arc<AppState>>,
) -> Result<LifeModelV2ProposalReceipt, AppError> {
    draft_lifemodel_v2_change_with_state(request, state.inner()).await
}

pub(crate) async fn draft_lifemodel_v2_rollback_with_state(
    request: DraftLifeModelV2RollbackRequest,
    state: &Arc<AppState>,
) -> Result<LifeModelV2ProposalReceipt, AppError> {
    let manager = state.life_model_manager.lock().await;
    let current = manager
        .load_v2_current(DEFAULT_LIFE_MODEL_V2_MODEL_ID)?
        .ok_or_else(|| AppError::not_found("lifemodel_v2_rollback_current_missing"))?;
    ensure_exact_lifemodel_v2_base(
        Some(&current),
        Some(request.base_version),
        Some(&request.base_document_digest),
    )?;
    if request.target_version >= current.model_version {
        return Err(AppError::internal(
            "lifemodel_v2_rollback_target_not_historical",
        ));
    }
    let target = manager
        .load_v2_version(DEFAULT_LIFE_MODEL_V2_MODEL_ID, request.target_version)?
        .ok_or_else(|| AppError::not_found("lifemodel_v2_rollback_target_missing"))?;
    if target.document_digest != request.target_document_digest {
        return Err(AppError::internal("lifemodel_v2_rollback_target_drift"));
    }
    let diff = LifeModelTypedDiffV2::between_versions(&current, &target)?;
    let mut proposal = AgentProposal::new(
        ProposalType::LifeModelUpdate,
        LIFE_MODEL_V2_TYPED_DIFF_PATH,
        serde_json::to_value(&diff)?,
        "Append the selected historical LifeModel content as a new reviewed canonical version.",
        1.0,
        RiskLevel::High,
        ProposalSource::Manual,
    );
    proposal.base_hash = Some(current.document_digest.clone());
    proposal.source_detail = Some(format!(
        "lifemodel_v2_rollback:{}:{}",
        target.model_version, target.document_digest
    ));
    proposal.before = Some(serde_json::json!({
        "baseVersion": current.model_version,
        "baseDocumentDigest": current.document_digest,
        "targetVersion": target.model_version,
        "targetDocumentDigest": target.document_digest,
    }));
    drop(manager);
    let proposal_store = state
        .proposal_store
        .as_ref()
        .ok_or_else(|| AppError::db("proposal_store_unavailable"))?;
    proposal_store.lock().await.create_proposal(&proposal)?;
    Ok(LifeModelV2ProposalReceipt {
        proposal_id: proposal.id,
        status: "review_required".into(),
        base_version: diff.base_version,
        base_document_digest: diff.base_document_digest,
        result_document_digest: Some(diff.result_document_digest),
        operation_count: diff.operations.len(),
    })
}

#[tauri::command]
pub async fn draft_lifemodel_v2_rollback(
    request: DraftLifeModelV2RollbackRequest,
    state: State<'_, Arc<AppState>>,
) -> Result<LifeModelV2ProposalReceipt, AppError> {
    draft_lifemodel_v2_rollback_with_state(request, state.inner()).await
}

pub(crate) async fn draft_lifemodel_v2_export_with_state(
    request: DraftLifeModelV2ExportRequest,
    state: &Arc<AppState>,
) -> Result<LifeModelV2ProposalReceipt, AppError> {
    let extension_valid = match request.format {
        LifeModelV2ExportFormat::Yaml => {
            request.target_path.ends_with(".yaml") || request.target_path.ends_with(".yml")
        }
        LifeModelV2ExportFormat::Json => request.target_path.ends_with(".json"),
    };
    if !extension_valid {
        return Err(AppError::internal(
            "lifemodel_v2_export_target_extension_mismatch",
        ));
    }
    let manager = state.life_model_manager.lock().await;
    let version = manager
        .load_v2_version(DEFAULT_LIFE_MODEL_V2_MODEL_ID, request.model_version)?
        .ok_or_else(|| AppError::not_found("lifemodel_v2_export_version_missing"))?;
    if version.document_digest != request.document_digest {
        return Err(AppError::internal("lifemodel_v2_export_version_drift"));
    }
    let (content, projection_digest, format_label) = match request.format {
        LifeModelV2ExportFormat::Yaml => {
            let projection = version.human_yaml_projection()?;
            if request.projection_digest.as_deref() != Some(projection.projection_digest.as_str()) {
                return Err(AppError::internal("lifemodel_v2_export_projection_drift"));
            }
            (projection.yaml, Some(projection.projection_digest), "yaml")
        }
        LifeModelV2ExportFormat::Json => {
            if request.projection_digest.is_some() {
                return Err(AppError::internal(
                    "lifemodel_v2_json_export_projection_digest_unexpected",
                ));
            }
            let content = serde_json::to_string_pretty(&serde_json::json!({
                "schemaVersion": "openlife.lifemodel.v2.export.v1",
                "modelId": version.model_id,
                "modelVersion": version.model_version,
                "documentDigest": version.document_digest,
                "document": version.document,
            }))?;
            (format!("{content}\n"), None, "json")
        }
    };
    openlife_core::agent::action_executor::helpers::ensure_external_write_content_size(&content)?;
    drop(manager);
    let safe_paths = state
        .config
        .lock()
        .await
        .system
        .artifact_output_directory
        .clone()
        .into_iter()
        .collect::<Vec<_>>();
    let precondition = capture_artifact_target_precondition(&request.target_path, &safe_paths)
        .map_err(AppError::permission)?;
    let (expected_target_absent, expected_target_digest, operation) = match precondition {
        ArtifactTargetPrecondition::Absent => (true, None, "create"),
        ArtifactTargetPrecondition::ContentDigest(digest) => (false, Some(digest), "overwrite"),
    };
    let (size_bytes, content_digest) = openlife_core::agent::metadata_safe_text_digest(&content);
    let mut proposal = AgentProposal::new(
        ProposalType::ExternalWriteAction,
        &format!("filesystem.{}", request.target_path),
        serde_json::json!({
            "path": request.target_path,
            "content": content,
            "content_hash": content_digest,
            "size_bytes": size_bytes,
            "encoding": "utf-8",
            "operation": operation,
            "expected_target_absent": expected_target_absent,
            "expected_target_digest": expected_target_digest,
            "source": "lifemodel_v2_export",
            "format": format_label,
            "modelVersion": version.model_version,
            "documentDigest": version.document_digest,
            "projectionDigest": projection_digest,
        }),
        "Export one exact reviewed LifeModel version to a local file.",
        1.0,
        RiskLevel::High,
        ProposalSource::Manual,
    );
    proposal.source_detail = Some(format!("lifemodel_v2_{format_label}_export"));
    let proposal_store = state
        .proposal_store
        .as_ref()
        .ok_or_else(|| AppError::db("proposal_store_unavailable"))?;
    proposal_store.lock().await.create_proposal(&proposal)?;
    Ok(LifeModelV2ProposalReceipt {
        proposal_id: proposal.id,
        status: "review_required".into(),
        base_version: Some(version.model_version),
        base_document_digest: Some(version.document_digest),
        result_document_digest: None,
        operation_count: 0,
    })
}

#[tauri::command]
pub async fn draft_lifemodel_v2_export(
    request: DraftLifeModelV2ExportRequest,
    state: State<'_, Arc<AppState>>,
) -> Result<LifeModelV2ProposalReceipt, AppError> {
    draft_lifemodel_v2_export_with_state(request, state.inner()).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_app_state(temp_dir: &tempfile::TempDir) -> Arc<AppState> {
        let config = openlife_core::config::AppConfig::default();
        Arc::new(AppState {
            persistence_coordinator: Arc::new(
                crate::persistence_coordinator::PersistenceCoordinator::isolated_evaluation(),
            ),
            config: Arc::new(tokio::sync::Mutex::new(config.clone())),
            life_model_manager: Arc::new(tokio::sync::Mutex::new(
                openlife_core::life_model::LifeModelManager::new(
                    temp_dir.path().join("life-model").join("current"),
                ),
            )),
            life_model_write_coordinator: Arc::new(tokio::sync::Mutex::new(())),
            memory_store: Arc::new(tokio::sync::Mutex::new(
                openlife_core::memory::KnowledgeNoteProjectionStore::new_in_memory().unwrap(),
            )),
            conversation_store: Some(Arc::new(tokio::sync::Mutex::new(
                openlife_core::conversation::ConversationStore::new_in_memory().unwrap(),
            ))),
            mcp_registry: Arc::new(tokio::sync::Mutex::new(
                openlife_core::mcp::McpRegistry::new(),
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
            last_snapshot_date: Arc::new(tokio::sync::Mutex::new(None)),
            mcp_audit_store: Arc::new(tokio::sync::Mutex::new(
                openlife_core::mcp_audit::McpAuditStore::new(temp_dir.path().join("mcp_audit.db")),
            )),
            canonical_task_runtime_store: None,
            proposal_store: Some(Arc::new(tokio::sync::Mutex::new(
                openlife_core::agent::ProposalStore::new_in_memory().unwrap(),
            ))),
            memory_lifecycle_store: Some(Arc::new(tokio::sync::Mutex::new(
                openlife_core::agent::MemoryLifecycleStore::new_in_memory().unwrap(),
            ))),
            life_model_learning_store: Some(Arc::new(tokio::sync::Mutex::new(
                openlife_core::agent::LifeModelLearningStore::new_in_memory().unwrap(),
            ))),
            main_chat_runtime_state: crate::state::MainChatRuntimeState::shared(),
            tool_permission_store: Arc::new(tokio::sync::Mutex::new(
                openlife_core::tool_permissions::ToolPermissionStore::new_in_memory().unwrap(),
            )),
            skill_registry: Arc::new(tokio::sync::Mutex::new(
                openlife_core::skills::SkillRegistry::built_in(),
            )),
            startup_warnings: vec![],
            credential_bootstrap_snapshot: Default::default(),
            web_search_fixture_output: Arc::new(tokio::sync::Mutex::new(None)),
            work_initial_decision_fixture_output: Arc::new(tokio::sync::Mutex::new(None)),
            work_goal_contract_fixture_output: Arc::new(tokio::sync::Mutex::new(None)),
            work_steering_replan_fixture_output: Arc::new(tokio::sync::Mutex::new(None)),
            work_agent_step_fixture_outputs: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            work_semantic_verification_fixture_outputs: Arc::new(tokio::sync::Mutex::new(
                Vec::new(),
            )),
            resource_runtime: None,
        })
    }

    #[tokio::test]
    async fn legacy_owner_blocks_a_parallel_initial_v2_change() {
        let temp_dir = tempfile::tempdir().unwrap();
        let current = temp_dir.path().join("life-model/current");
        std::fs::create_dir_all(&current).unwrap();
        std::fs::write(
            current.join("life_model.yaml"),
            "identity:\n  name: Alice\n",
        )
        .unwrap();
        let state = test_app_state(&temp_dir);

        let error = draft_lifemodel_v2_change_with_state(
            DraftLifeModelV2ChangeRequest {
                base_version: None,
                base_document_digest: None,
                change: LifeModelV2UserChange::Add {
                    section: LifeModelSectionV2::Values,
                    value: LifeModelUserValueV2::Statement {
                        statement: "Autonomy matters.".into(),
                    },
                },
            },
            &state,
        )
        .await
        .unwrap_err();

        assert!(error.message().contains("requires_legacy_migration"));
        assert!(state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_all_proposals(100, 0)
            .unwrap()
            .is_empty());
        assert!(!current.join("life_model_v2.db").exists());
    }

    #[tokio::test]
    async fn reviewed_legacy_migration_preserves_source_and_survives_read_model_restart() {
        let temp_dir = tempfile::tempdir().unwrap();
        let current = temp_dir.path().join("life-model/current");
        let versions = temp_dir.path().join("life-model/versions");
        std::fs::create_dir_all(&current).unwrap();
        std::fs::create_dir_all(&versions).unwrap();
        let source = "identity:\n  name: Alice\ngoals:\n  short_term:\n    - name: Ship now\n";
        std::fs::write(current.join("life_model.yaml"), source).unwrap();
        std::fs::write(versions.join("one.yaml"), source).unwrap();
        std::fs::write(
            versions.join("index.json"),
            r#"{"versions":[{"version":"one"}]}"#,
        )
        .unwrap();
        let state = test_app_state(&temp_dir);
        let inventory = state
            .life_model_manager
            .lock()
            .await
            .inspect_legacy_inventory()
            .unwrap()
            .unwrap();
        let preview = inventory.preview.unwrap();
        let selections = preview
            .candidates
            .iter()
            .map(|candidate| {
                openlife_core::life_model::legacy_migration::LegacyLifeModelMigrationSelectionV2 {
                    candidate_id: candidate.candidate_id.clone(),
                    decision: openlife_core::life_model::legacy_migration::LegacyLifeModelMigrationDecisionV2::Include,
                    edited_value: None,
                }
            })
            .collect();
        let draft = draft_legacy_lifemodel_migration_with_state(
            DraftLegacyLifeModelMigrationRequest {
                source_digest: preview.source_digest.clone(),
                selections,
                non_lifemodel_items_acknowledged: true,
            },
            &state,
        )
        .await
        .unwrap();
        assert!(!draft.durable_write_executed);
        assert!(!current.join("life_model_v2.db").exists());

        crate::commands::proposal::accept_proposal_with_state(draft.proposal_id, &state)
            .await
            .unwrap();
        let manager = state.life_model_manager.lock().await;
        let canonical = manager
            .load_v2_current(DEFAULT_LIFE_MODEL_V2_MODEL_ID)
            .unwrap()
            .unwrap();
        let cutover = manager
            .load_v2_cutover(DEFAULT_LIFE_MODEL_V2_MODEL_ID)
            .unwrap()
            .unwrap();
        assert_eq!(canonical.model_version, 1);
        assert_eq!(canonical.document.identity.len(), 1);
        assert_eq!(cutover.document_digest, canonical.document_digest);
        assert_eq!(cutover.legacy_source_digest, preview.source_digest);
        drop(manager);
        assert_eq!(
            std::fs::read_to_string(current.join("life_model.yaml")).unwrap(),
            source
        );
        let backups = std::fs::read_dir(temp_dir.path().join("life-model/legacy-backups"))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(backups.len(), 1);

        let restarted = test_app_state(&temp_dir);
        let envelope =
            crate::read_models::life_model::get_life_model_view_model_with_state(&restarted)
                .await
                .unwrap();
        let data = envelope.data.unwrap();
        assert_eq!(
            data.truth_mode,
            openlife_core::agent::LifeModelTruthMode::Canonical
        );
        assert!(data.legacy_migration_inventory.is_none());
        assert_eq!(
            data.canonical_summary.unwrap().document_digest,
            canonical.document_digest
        );
    }

    #[tokio::test]
    async fn legacy_source_drift_after_review_fails_before_backup_or_cutover() {
        let temp_dir = tempfile::tempdir().unwrap();
        let current = temp_dir.path().join("life-model/current");
        std::fs::create_dir_all(&current).unwrap();
        std::fs::write(
            current.join("life_model.yaml"),
            "identity:\n  name: Alice\n",
        )
        .unwrap();
        let state = test_app_state(&temp_dir);
        let preview = state
            .life_model_manager
            .lock()
            .await
            .inspect_legacy_inventory()
            .unwrap()
            .unwrap()
            .preview
            .unwrap();
        let selections = preview
            .candidates
            .iter()
            .map(|candidate| {
                openlife_core::life_model::legacy_migration::LegacyLifeModelMigrationSelectionV2 {
                    candidate_id: candidate.candidate_id.clone(),
                    decision: openlife_core::life_model::legacy_migration::LegacyLifeModelMigrationDecisionV2::Include,
                    edited_value: None,
                }
            })
            .collect();
        let draft = draft_legacy_lifemodel_migration_with_state(
            DraftLegacyLifeModelMigrationRequest {
                source_digest: preview.source_digest,
                selections,
                non_lifemodel_items_acknowledged: true,
            },
            &state,
        )
        .await
        .unwrap();
        std::fs::write(current.join("life_model.yaml"), "identity:\n  name: Bob\n").unwrap();

        let error = crate::commands::proposal::accept_proposal_with_state(
            draft.proposal_id.clone(),
            &state,
        )
        .await
        .unwrap_err();
        assert!(error.contains("应用前校验失败"));
        let manager = state.life_model_manager.lock().await;
        assert!(manager
            .load_v2_current(DEFAULT_LIFE_MODEL_V2_MODEL_ID)
            .unwrap()
            .is_none());
        assert!(manager
            .load_v2_cutover(DEFAULT_LIFE_MODEL_V2_MODEL_ID)
            .unwrap()
            .is_none());
        drop(manager);
        assert!(!temp_dir.path().join("life-model/legacy-backups").exists());
        let proposal = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_proposal(&draft.proposal_id)
            .unwrap()
            .unwrap();
        assert_eq!(
            proposal.status,
            openlife_core::agent::ProposalStatus::Pending
        );
    }

    #[tokio::test]
    async fn lifemodel_v2_user_change_clear_and_rollback_are_proposal_first_and_versioned() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let add = draft_lifemodel_v2_change_with_state(
            DraftLifeModelV2ChangeRequest {
                base_version: None,
                base_document_digest: None,
                change: LifeModelV2UserChange::Add {
                    section: LifeModelSectionV2::Values,
                    value: LifeModelUserValueV2::Statement {
                        statement: "Autonomy matters.".into(),
                    },
                },
            },
            &state,
        )
        .await
        .unwrap();
        assert_eq!(add.status, "review_required");
        assert!(state
            .life_model_manager
            .lock()
            .await
            .load_v2_current(DEFAULT_LIFE_MODEL_V2_MODEL_ID)
            .unwrap()
            .is_none());

        crate::commands::proposal::accept_proposal_with_state(add.proposal_id, &state)
            .await
            .unwrap();
        let first = state
            .life_model_manager
            .lock()
            .await
            .load_v2_current(DEFAULT_LIFE_MODEL_V2_MODEL_ID)
            .unwrap()
            .unwrap();
        assert_eq!(first.model_version, 1);
        assert_eq!(first.document.values.len(), 1);

        let count_before_stale = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_all_proposals(100, 0)
            .unwrap()
            .len();
        let stale = draft_lifemodel_v2_change_with_state(
            DraftLifeModelV2ChangeRequest {
                base_version: None,
                base_document_digest: None,
                change: LifeModelV2UserChange::Clear,
            },
            &state,
        )
        .await
        .unwrap_err();
        assert!(stale.message().contains("stale_base"));
        assert_eq!(
            state
                .proposal_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .list_all_proposals(100, 0)
                .unwrap()
                .len(),
            count_before_stale
        );

        let clear = draft_lifemodel_v2_change_with_state(
            DraftLifeModelV2ChangeRequest {
                base_version: Some(first.model_version),
                base_document_digest: Some(first.document_digest.clone()),
                change: LifeModelV2UserChange::Clear,
            },
            &state,
        )
        .await
        .unwrap();
        crate::commands::proposal::accept_proposal_with_state(clear.proposal_id, &state)
            .await
            .unwrap();
        let second = state
            .life_model_manager
            .lock()
            .await
            .load_v2_current(DEFAULT_LIFE_MODEL_V2_MODEL_ID)
            .unwrap()
            .unwrap();
        assert_eq!(second.model_version, 2);
        assert!(second.document.is_empty());

        let rollback = draft_lifemodel_v2_rollback_with_state(
            DraftLifeModelV2RollbackRequest {
                base_version: second.model_version,
                base_document_digest: second.document_digest.clone(),
                target_version: first.model_version,
                target_document_digest: first.document_digest.clone(),
            },
            &state,
        )
        .await
        .unwrap();
        crate::commands::proposal::accept_proposal_with_state(rollback.proposal_id, &state)
            .await
            .unwrap();
        let manager = state.life_model_manager.lock().await;
        let third = manager
            .load_v2_current(DEFAULT_LIFE_MODEL_V2_MODEL_ID)
            .unwrap()
            .unwrap();
        assert_eq!(third.model_version, 3);
        assert_eq!(third.parent_version, Some(2));
        assert_eq!(third.document, first.document);
        assert_eq!(
            manager
                .load_v2_version(DEFAULT_LIFE_MODEL_V2_MODEL_ID, 1)
                .unwrap()
                .unwrap()
                .document,
            first.document
        );
        assert!(third
            .source_refs
            .iter()
            .any(|source| source.starts_with("lifemodel-version:primary:1:")));
    }

    #[tokio::test]
    async fn lifemodel_v2_export_binds_exact_version_format_and_target_precondition() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let add = draft_lifemodel_v2_change_with_state(
            DraftLifeModelV2ChangeRequest {
                base_version: None,
                base_document_digest: None,
                change: LifeModelV2UserChange::Add {
                    section: LifeModelSectionV2::Identity,
                    value: LifeModelUserValueV2::Statement {
                        statement: "I build personal software.".into(),
                    },
                },
            },
            &state,
        )
        .await
        .unwrap();
        crate::commands::proposal::accept_proposal_with_state(add.proposal_id, &state)
            .await
            .unwrap();
        let version = state
            .life_model_manager
            .lock()
            .await
            .load_v2_current(DEFAULT_LIFE_MODEL_V2_MODEL_ID)
            .unwrap()
            .unwrap();
        let safe_root = temp_dir.path().canonicalize().unwrap().join("exports");
        std::fs::create_dir_all(&safe_root).unwrap();
        state.config.lock().await.system.artifact_output_directory =
            Some(safe_root.to_string_lossy().into_owned());
        let target = safe_root.join("lifemodel.json");

        let export = draft_lifemodel_v2_export_with_state(
            DraftLifeModelV2ExportRequest {
                model_version: version.model_version,
                document_digest: version.document_digest.clone(),
                projection_digest: None,
                format: LifeModelV2ExportFormat::Json,
                target_path: target.to_string_lossy().into_owned(),
            },
            &state,
        )
        .await
        .unwrap();
        assert_eq!(export.status, "review_required");
        assert!(!target.exists());
        let proposal = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_proposal(&export.proposal_id)
            .unwrap()
            .unwrap();
        assert_eq!(proposal.after["format"], "json");
        assert_eq!(proposal.after["modelVersion"], version.model_version);
        assert_eq!(proposal.after["documentDigest"], version.document_digest);
        assert_eq!(proposal.after["expected_target_absent"], true);
        assert!(proposal.after["content"]
            .as_str()
            .unwrap()
            .contains("openlife.lifemodel.v2.export.v1"));

        let mismatch = draft_lifemodel_v2_export_with_state(
            DraftLifeModelV2ExportRequest {
                model_version: version.model_version,
                document_digest: version.document_digest,
                projection_digest: None,
                format: LifeModelV2ExportFormat::Yaml,
                target_path: target.to_string_lossy().into_owned(),
            },
            &state,
        )
        .await
        .unwrap_err();
        assert!(mismatch.message().contains("extension_mismatch"));
    }
}
