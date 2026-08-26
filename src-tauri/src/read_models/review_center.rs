use crate::state::AppState;
use openlife_core::agent::{
    build_review_center_view_model, build_review_decision_context, AgentProposal,
    EvidenceSensitivity, MemoryLifecycleStatus, MemoryMaterializationStatus, ProposalStatus,
    ProposalType, ReviewCenterBuildInput, ReviewCenterViewModel, ReviewItemArtifactEvidence,
    ReviewItemMaterializationStatus, ReviewReadableValue, ReviewReadableValueKind,
    ViewModelEnvelope, ViewModelStatus, ViewModelWarning, ViewModelWarningSeverity,
};
use openlife_core::task_runtime::{
    CanonicalArtifactEffectState, CanonicalArtifactReviewSubject, CanonicalArtifactUndoOperation,
    CanonicalTaskRuntimeStore,
};
use similar::TextDiff;
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub async fn get_review_center_view_model(
    state: State<'_, Arc<AppState>>,
) -> Result<ViewModelEnvelope<ReviewCenterViewModel>, String> {
    get_review_center_view_model_with_state(state.inner()).await
}

pub(crate) async fn get_review_center_view_model_with_state(
    state: &Arc<AppState>,
) -> Result<ViewModelEnvelope<ReviewCenterViewModel>, String> {
    let Some(proposal_store) = state.proposal_store.as_ref() else {
        let mut envelope = ViewModelEnvelope::backend_read_model(ViewModelStatus::Error, None);
        envelope.last_updated_at = Some(chrono::Utc::now().to_rfc3339());
        envelope.warnings.push(warning(
            "proposal_store_unavailable",
            "Proposal store is unavailable; ReviewCenterViewModel cannot determine action eligibility.",
        ));
        return Ok(envelope);
    };

    let proposal_store = proposal_store.lock().await;
    let proposals = proposal_store
        .list_all_proposals(100, 0)
        .map_err(|err| format!("failed to load review proposals: {err}"))?;
    let (dispatch_materialization_overrides, mut dispatch_warnings) =
        dispatch_materialization_overrides(&proposal_store, &proposals);
    drop(proposal_store);
    let (canonical_artifact_evidence, mut canonical_artifact_warnings) =
        canonical_artifact_evidence_overrides(state, &proposals).await;
    let artifact_evidence = canonical_artifact_evidence;
    let (decision_context_overrides, mut artifact_diff_warnings) =
        canonical_artifact_decision_context_overrides(state, &proposals).await;
    let config = state.config.lock().await;
    let safe_paths = config
        .system
        .artifact_output_directory
        .clone()
        .into_iter()
        .collect();
    drop(config);

    let (safe_path_overrides, mut safe_path_warnings) =
        canonical_artifact_safe_path_overrides(state, &proposals).await;

    let (mut materialization_overrides, mut warnings) =
        memory_materialization_overrides(state, &proposals).await;
    materialization_overrides.extend(dispatch_materialization_overrides);
    warnings.append(&mut dispatch_warnings);
    warnings.append(&mut canonical_artifact_warnings);
    warnings.append(&mut artifact_diff_warnings);
    warnings.append(&mut safe_path_warnings);
    // Review availability is owned by the exact Proposal/Artifact capability
    // being reviewed. Unrelated startup warnings (including retired execution
    // stores) must not turn the whole Review Center into Safe Mode.
    let safe_mode_reason = None;
    let model = build_review_center_view_model(ReviewCenterBuildInput {
        proposals,
        safe_mode_active: false,
        safe_mode_reason,
        safe_paths,
        safe_path_overrides,
        materialization_overrides,
        artifact_evidence,
        decision_context_overrides,
    });

    let status = if model.items.is_empty() {
        ViewModelStatus::Empty
    } else {
        ViewModelStatus::Ready
    };
    let mut envelope = ViewModelEnvelope::backend_read_model(status, Some(model));
    envelope.last_updated_at = Some(chrono::Utc::now().to_rfc3339());
    envelope.warnings.append(&mut warnings);
    Ok(envelope)
}

const REVIEW_TEXT_DIFF_MAX_BYTES: u64 = 1024 * 1024;
const REVIEW_TEXT_DIFF_MAX_CHARS: usize = 12_000;

async fn canonical_artifact_decision_context_overrides(
    state: &Arc<AppState>,
    proposals: &[AgentProposal],
) -> (
    BTreeMap<String, openlife_core::agent::ReviewDecisionContext>,
    Vec<ViewModelWarning>,
) {
    let Some(store) = state.canonical_task_runtime_store.as_ref() else {
        return (BTreeMap::new(), Vec::new());
    };
    let store = store.lock().await;
    let database_path = store.db_path().map(Path::to_path_buf);
    let mut overrides = BTreeMap::new();
    let mut warnings = Vec::new();

    for proposal in proposals
        .iter()
        .filter(|proposal| proposal.proposal_type == ProposalType::ExternalWriteAction)
    {
        if proposal.after.get("undoOfArtifactId").is_some() {
            match canonical_artifact_undo_decision_context(
                &store,
                database_path.as_deref(),
                proposal,
            ) {
                Ok(context) => {
                    overrides.insert(proposal.id.clone(), context);
                }
                Err(error) => warnings.push(warning(
                    "canonical_artifact_undo_diff_unavailable",
                    format!(
                        "Artifact Undo preview is unavailable for proposal {}: {error}",
                        proposal.id
                    ),
                )),
            }
            continue;
        }
        let Ok(subject) =
            serde_json::from_value::<CanonicalArtifactReviewSubject>(proposal.after.clone())
        else {
            continue;
        };
        if subject.validate().is_err() {
            continue;
        }
        let projection = (|| -> Result<_, String> {
            let version = store
                .load_artifact_version(&subject.artifact_id, subject.artifact_version)
                .map_err(|error| error.to_string())?
                .ok_or_else(|| "canonical_artifact_version_missing".to_string())?;
            if version.content_digest != subject.content_digest
                || version.target_reference.as_deref() != Some(subject.path.as_str())
            {
                return Err("canonical_artifact_review_projection_identity_mismatch".into());
            }
            if subject.operation == "move" {
                let source = subject
                    .source_path
                    .as_deref()
                    .ok_or_else(|| "canonical_artifact_move_source_missing".to_string())?;
                let target = subject
                    .target_path
                    .as_deref()
                    .ok_or_else(|| "canonical_artifact_move_target_missing".to_string())?;
                let mut context = build_review_decision_context(proposal, &[]);
                context.title = "确认重命名文件".into();
                context.summary = format!(
                    "核对 {} 将重命名为 {}，文件内容保持不变。",
                    Path::new(source)
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or(source),
                    Path::new(target)
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or(target)
                );
                context.before = Some(ReviewReadableValue {
                    kind: ReviewReadableValueKind::Text,
                    summary: format!("当前名称 · {}", source),
                    detail: None,
                    sensitivity: EvidenceSensitivity::LocalPrivate,
                    truncated: false,
                });
                context.after = ReviewReadableValue {
                    kind: ReviewReadableValueKind::Text,
                    summary: format!("新名称 · {}", target),
                    detail: Some(format!("文件内容摘要保持为 {}", subject.content_digest)),
                    sensitivity: EvidenceSensitivity::LocalPrivate,
                    truncated: false,
                };
                return Ok(context);
            }
            if !is_text_review_media_type(&subject) {
                return Err("canonical_artifact_review_diff_unsupported_media_type".into());
            }
            let draft_reference = version
                .draft_reference
                .as_deref()
                .ok_or_else(|| "canonical_artifact_draft_reference_missing".to_string())?;
            let proposed = read_owned_review_text(
                database_path.as_deref(),
                "artifact-drafts",
                draft_reference,
                &version.content_digest,
            )?;
            let current = if subject.expected_target_absent {
                String::new()
            } else {
                let snapshot = store
                    .load_artifact_pre_change_snapshot(
                        &subject.artifact_id,
                        subject.artifact_version,
                    )
                    .map_err(|error| error.to_string())?
                    .ok_or_else(|| "canonical_artifact_pre_change_snapshot_missing".to_string())?;
                if subject.expected_target_digest.as_deref()
                    != Some(snapshot.content_digest.as_str())
                {
                    return Err("canonical_artifact_pre_change_digest_mismatch".into());
                }
                read_owned_review_text(
                    database_path.as_deref(),
                    "artifact-pre-change",
                    &snapshot.snapshot_reference,
                    &snapshot.content_digest,
                )?
            };
            let (diff, truncated) = bounded_unified_text_diff(&current, &proposed);
            let mut context = build_review_decision_context(proposal, &[]);
            context.title = if subject.expected_target_absent {
                "确认创建文件".into()
            } else {
                "确认文件修改".into()
            };
            context.summary = format!(
                "核对 {} 的精确文本变更后再决定是否写入。",
                Path::new(&subject.path)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or(subject.path.as_str())
            );
            context.before = Some(ReviewReadableValue {
                kind: ReviewReadableValueKind::Text,
                summary: if subject.expected_target_absent {
                    "文件当前不存在".into()
                } else {
                    format!(
                        "当前文件 · {}",
                        subject
                            .expected_target_digest
                            .as_deref()
                            .unwrap_or("摘要未知")
                    )
                },
                detail: None,
                sensitivity: EvidenceSensitivity::LocalPrivate,
                truncated: false,
            });
            context.after = ReviewReadableValue {
                kind: ReviewReadableValueKind::Text,
                summary: format!("建议版本 · {}", subject.content_digest),
                detail: Some(diff),
                sensitivity: EvidenceSensitivity::LocalPrivate,
                truncated,
            };
            Ok(context)
        })();
        match projection {
            Ok(context) => {
                overrides.insert(proposal.id.clone(), context);
            }
            Err(error) if error == "canonical_artifact_review_diff_unsupported_media_type" => {}
            Err(error) => warnings.push(warning(
                "canonical_artifact_review_diff_unavailable",
                format!(
                    "Artifact text diff is unavailable for proposal {}: {error}",
                    proposal.id
                ),
            )),
        }
    }
    (overrides, warnings)
}

fn canonical_artifact_undo_decision_context(
    store: &CanonicalTaskRuntimeStore,
    database_path: Option<&Path>,
    proposal: &AgentProposal,
) -> Result<openlife_core::agent::ReviewDecisionContext, String> {
    let artifact_id = proposal
        .after
        .get("undoOfArtifactId")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "canonical_artifact_undo_identity_missing".to_string())?;
    let version = proposal
        .after
        .get("artifactVersion")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| "canonical_artifact_undo_version_missing".to_string())?;
    let undo = store
        .load_artifact_undo_version(artifact_id, version)
        .map_err(|error| error.to_string())?
        .filter(|undo| undo.proposal_id == proposal.id)
        .ok_or_else(|| "canonical_artifact_undo_checkpoint_missing".to_string())?;
    let artifact = store
        .load_artifact(artifact_id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "canonical_artifact_missing".to_string())?;
    let artifact_version = store
        .load_artifact_version(artifact_id, version)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "canonical_artifact_version_missing".to_string())?;
    if artifact.current_version != version
        || proposal.run_id.as_deref()
            != proposal
                .after
                .get("sourceRunId")
                .and_then(serde_json::Value::as_str)
        || proposal.source_detail.as_deref()
            != proposal
                .after
                .get("canonicalTaskId")
                .and_then(serde_json::Value::as_str)
    {
        return Err("canonical_artifact_undo_origin_mismatch".into());
    }
    let mut context = build_review_decision_context(proposal, &[]);
    match undo.operation {
        CanonicalArtifactUndoOperation::RestoreReplaced => {
            let expected_target_digest = proposal
                .after
                .get("expected_target_digest")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| "canonical_artifact_undo_target_digest_missing".to_string())?;
            let restore_digest = proposal
                .after
                .get("restore_digest")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| "canonical_artifact_undo_restore_digest_missing".to_string())?;
            let target = proposal
                .after
                .get("path")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| "canonical_artifact_undo_target_missing".to_string())?;
            let snapshot = proposal
                .after
                .get("snapshot_path")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| "canonical_artifact_undo_snapshot_missing".to_string())?;
            if expected_target_digest != artifact.content_digest
                || restore_digest != undo.content_digest
                || proposal
                    .after
                    .get("contentDigest")
                    .and_then(serde_json::Value::as_str)
                    != Some(restore_digest)
                || target != undo.target_reference
                || snapshot != undo.source_reference
                || artifact_version.target_reference.as_deref() != Some(target)
            {
                return Err("canonical_artifact_undo_projection_identity_mismatch".into());
            }
            let (detail, truncated) = if is_text_review_media_type_name(&artifact.media_type) {
                let current = read_owned_review_text(
                    database_path,
                    "artifact-drafts",
                    artifact_version
                        .draft_reference
                        .as_deref()
                        .ok_or_else(|| "canonical_artifact_draft_reference_missing".to_string())?,
                    expected_target_digest,
                )?;
                let restored = read_owned_review_text(
                    database_path,
                    "artifact-pre-change",
                    snapshot,
                    restore_digest,
                )?;
                let (diff, truncated) = bounded_unified_text_diff(&current, &restored);
                (Some(diff), truncated)
            } else {
                (None, false)
            };
            context.title = "确认撤销文件修改".into();
            context.summary = format!(
                "核对 {} 将恢复到修改前版本后再决定。",
                Path::new(target)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or(target)
            );
            context.before = Some(ReviewReadableValue {
                kind: ReviewReadableValueKind::Text,
                summary: format!("当前版本 · {expected_target_digest}"),
                detail: None,
                sensitivity: EvidenceSensitivity::LocalPrivate,
                truncated: false,
            });
            context.after = ReviewReadableValue {
                kind: ReviewReadableValueKind::Text,
                summary: format!("恢复版本 · {restore_digest}"),
                detail,
                sensitivity: EvidenceSensitivity::LocalPrivate,
                truncated,
            };
        }
        CanonicalArtifactUndoOperation::TrashCreated => {
            let source = proposal
                .after
                .get("source_path")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| "canonical_artifact_undo_source_missing".to_string())?;
            let digest = proposal
                .after
                .get("source_digest")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| "canonical_artifact_undo_source_digest_missing".to_string())?;
            if source != undo.source_reference || digest != undo.content_digest {
                return Err("canonical_artifact_undo_projection_identity_mismatch".into());
            }
            context.title = "确认撤销已创建文件".into();
            context.summary = format!(
                "确认将 {} 移到 OpenLife 的可恢复位置。",
                Path::new(source)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or(source)
            );
            context.before = Some(ReviewReadableValue {
                kind: ReviewReadableValueKind::Text,
                summary: format!("当前文件 · {digest}"),
                detail: None,
                sensitivity: EvidenceSensitivity::LocalPrivate,
                truncated: false,
            });
            context.after = ReviewReadableValue {
                kind: ReviewReadableValueKind::Text,
                summary: "移到 OpenLife 可恢复位置".into(),
                detail: None,
                sensitivity: EvidenceSensitivity::LocalPrivate,
                truncated: false,
            };
        }
        CanonicalArtifactUndoOperation::RestoreMoved => {
            let source = proposal
                .after
                .get("source_path")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| "canonical_artifact_undo_source_missing".to_string())?;
            let target = proposal
                .after
                .get("target_path")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| "canonical_artifact_undo_target_missing".to_string())?;
            let digest = proposal
                .after
                .get("source_digest")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| "canonical_artifact_undo_source_digest_missing".to_string())?;
            if source != undo.source_reference
                || target != undo.target_reference
                || digest != undo.content_digest
                || artifact_version.materialized_reference.as_deref() != Some(source)
            {
                return Err("canonical_artifact_undo_projection_identity_mismatch".into());
            }
            context.title = "确认撤销文件重命名".into();
            context.summary = format!(
                "确认将 {} 恢复为原名称 {}，文件内容保持不变。",
                Path::new(source)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or(source),
                Path::new(target)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or(target)
            );
            context.before = Some(ReviewReadableValue {
                kind: ReviewReadableValueKind::Text,
                summary: format!("当前名称 · {source}"),
                detail: None,
                sensitivity: EvidenceSensitivity::LocalPrivate,
                truncated: false,
            });
            context.after = ReviewReadableValue {
                kind: ReviewReadableValueKind::Text,
                summary: format!("恢复名称 · {target}"),
                detail: Some(format!("文件内容摘要保持为 {digest}")),
                sensitivity: EvidenceSensitivity::LocalPrivate,
                truncated: false,
            };
        }
    }
    Ok(context)
}

fn is_text_review_media_type(subject: &CanonicalArtifactReviewSubject) -> bool {
    matches!(
        subject.artifact_kind.as_str(),
        "markdown" | "html" | "json" | "yaml" | "csv" | "text"
    )
}

fn is_text_review_media_type_name(media_type: &str) -> bool {
    media_type.starts_with("text/")
        || matches!(
            media_type.split(';').next().unwrap_or(media_type),
            "application/json" | "application/yaml" | "application/x-yaml"
        )
}

fn read_owned_review_text(
    database_path: Option<&Path>,
    storage_directory: &str,
    reference: &str,
    expected_digest: &str,
) -> Result<String, String> {
    let path = Path::new(reference);
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|_| "canonical_artifact_review_source_missing".to_string())?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("canonical_artifact_review_source_type_invalid".into());
    }
    if metadata.len() > REVIEW_TEXT_DIFF_MAX_BYTES {
        return Err("canonical_artifact_review_source_too_large".into());
    }
    if let Some(parent) = database_path.and_then(Path::parent) {
        let root = parent
            .join(storage_directory)
            .canonicalize()
            .map_err(|_| "canonical_artifact_review_source_root_unavailable".to_string())?;
        let canonical = path
            .canonicalize()
            .map_err(|_| "canonical_artifact_review_source_missing".to_string())?;
        if !canonical.starts_with(root) {
            return Err("canonical_artifact_review_source_outside_store".into());
        }
    } else if !cfg!(test) {
        return Err("canonical_artifact_review_source_root_unavailable".into());
    }
    let bytes = std::fs::read(path)
        .map_err(|_| "canonical_artifact_review_source_read_failed".to_string())?;
    if crate::artifact_materializer::artifact_content_digest(&bytes) != expected_digest {
        return Err("canonical_artifact_review_source_digest_mismatch".into());
    }
    String::from_utf8(bytes).map_err(|_| "canonical_artifact_review_source_not_utf8".to_string())
}

fn bounded_unified_text_diff(current: &str, proposed: &str) -> (String, bool) {
    let diff = TextDiff::from_lines(current, proposed)
        .unified_diff()
        .context_radius(3)
        .header("当前文件", "建议版本")
        .to_string();
    let count = diff.chars().count();
    if count <= REVIEW_TEXT_DIFF_MAX_CHARS {
        return (diff, false);
    }
    let mut bounded = diff
        .chars()
        .take(REVIEW_TEXT_DIFF_MAX_CHARS)
        .collect::<String>();
    bounded.push_str("\n… diff 已截断；批准仍绑定完整内容摘要。\n");
    (bounded, true)
}

async fn canonical_artifact_safe_path_overrides(
    state: &Arc<AppState>,
    proposals: &[AgentProposal],
) -> (BTreeMap<String, Vec<String>>, Vec<ViewModelWarning>) {
    let mut overrides = BTreeMap::new();
    let mut warnings = Vec::new();
    for proposal in proposals
        .iter()
        .filter(|proposal| proposal.proposal_type == ProposalType::ExternalWriteAction)
    {
        match crate::canonical_work_runtime::artifact_safe_paths_for_proposal(state, proposal).await
        {
            Ok(paths) => {
                overrides.insert(proposal.id.clone(), paths);
            }
            Err(error) => warnings.push(warning(
                "canonical_artifact_review_scope_unavailable",
                format!(
                    "Canonical artifact review scope could not be reconstructed for proposal {}: {error}",
                    proposal.id
                ),
            )),
        }
    }
    (overrides, warnings)
}

async fn canonical_artifact_evidence_overrides(
    state: &Arc<AppState>,
    proposals: &[AgentProposal],
) -> (
    BTreeMap<String, ReviewItemArtifactEvidence>,
    Vec<ViewModelWarning>,
) {
    let Some(store) = state.canonical_task_runtime_store.as_ref() else {
        return (BTreeMap::new(), Vec::new());
    };
    let store = store.lock().await;
    let mut evidence = BTreeMap::new();
    let mut warnings = Vec::new();
    for proposal in proposals {
        match store.load_artifact_effect(&proposal.id) {
            Ok(Some(record)) => {
                let state = match record.state {
                    CanonicalArtifactEffectState::Prepared => "prepared",
                    CanonicalArtifactEffectState::Staged => "staged",
                    CanonicalArtifactEffectState::Confirmed => "confirmed",
                    CanonicalArtifactEffectState::FailedBeforeEffect => "failed_before_effect",
                    CanonicalArtifactEffectState::EffectUnknown => "unknown",
                };
                evidence.insert(
                    proposal.id.clone(),
                    ReviewItemArtifactEvidence {
                        state: state.into(),
                        target_reference_digest: record.target_reference_digest,
                        content_digest: record.content_digest,
                        observed_content_digest: record.observed_content_digest,
                        byte_size: record.byte_size,
                        media_type: record.media_type,
                        error_code: record.error_code,
                    },
                );
            }
            Ok(None) => {}
            Err(error) => warnings.push(warning(
                "canonical_artifact_effect_lookup_failed",
                format!(
                    "Canonical artifact effect lookup failed for proposal {}: {error}",
                    proposal.id
                ),
            )),
        }
    }
    (evidence, warnings)
}

fn dispatch_materialization_overrides(
    proposal_store: &openlife_core::agent::ProposalStore,
    proposals: &[AgentProposal],
) -> (
    BTreeMap<String, ReviewItemMaterializationStatus>,
    Vec<ViewModelWarning>,
) {
    let mut overrides = BTreeMap::new();
    let mut warnings = Vec::new();
    for proposal in proposals
        .iter()
        .filter(|proposal| is_dispatch_backed_review_item(proposal))
    {
        match proposal_store.dispatch_state(&proposal.id) {
            Ok(Some(dispatch_state)) => {
                if let Some(status) =
                    action_materialization_status(proposal.status, dispatch_state.as_str())
                {
                    overrides.insert(proposal.id.clone(), status);
                }
            }
            Ok(None) => {
                overrides.insert(
                    proposal.id.clone(),
                    ReviewItemMaterializationStatus::Unknown,
                );
                warnings.push(warning(
                    "governed_action_dispatch_receipt_missing",
                    format!(
                        "Governed action {} has no dispatch receipt; its effect stays unknown.",
                        proposal.id
                    ),
                ));
            }
            Err(error) => {
                overrides.insert(
                    proposal.id.clone(),
                    ReviewItemMaterializationStatus::Unknown,
                );
                warnings.push(warning(
                    "governed_action_dispatch_receipt_unavailable",
                    format!(
                        "Governed action {} dispatch receipt could not be read: {error}",
                        proposal.id
                    ),
                ));
            }
        }
    }
    (overrides, warnings)
}

fn is_dispatch_backed_review_item(proposal: &AgentProposal) -> bool {
    match proposal.proposal_type {
        ProposalType::MemoryArchive => true,
        ProposalType::LifeModelUpdate => {
            matches!(
                proposal.affected_path.as_str(),
                openlife_core::life_model::v2::LIFE_MODEL_V2_TYPED_DIFF_PATH
                    | openlife_core::life_model::legacy_migration::LIFE_MODEL_V2_LEGACY_MIGRATION_PATH
            )
        }
        _ => false,
    }
}

fn action_materialization_status(
    proposal_status: ProposalStatus,
    dispatch_state: &str,
) -> Option<ReviewItemMaterializationStatus> {
    match dispatch_state {
        "unclaimed" => None,
        "claimed" | "confirmed_projection_pending" => {
            Some(ReviewItemMaterializationStatus::Applying)
        }
        "failed_before_effect" => Some(ReviewItemMaterializationStatus::Failed),
        "unknown" => Some(ReviewItemMaterializationStatus::Unknown),
        "confirmed" if proposal_status == ProposalStatus::Accepted => {
            Some(ReviewItemMaterializationStatus::Applied)
        }
        "confirmed" => Some(ReviewItemMaterializationStatus::Unknown),
        _ => Some(ReviewItemMaterializationStatus::Unknown),
    }
}

async fn memory_materialization_overrides(
    state: &Arc<AppState>,
    proposals: &[openlife_core::agent::AgentProposal],
) -> (
    BTreeMap<String, ReviewItemMaterializationStatus>,
    Vec<ViewModelWarning>,
) {
    let Some(lifecycle_store) = state.memory_lifecycle_store.as_ref() else {
        return (
            BTreeMap::new(),
            vec![warning(
                "memory_lifecycle_store_unavailable",
                "Memory lifecycle proof is unavailable; accepted memory review items stay fail-closed.",
            )],
        );
    };

    let lifecycle_store = lifecycle_store.lock().await;
    let mut overrides = BTreeMap::new();
    let mut warnings = Vec::new();
    for proposal in proposals {
        match lifecycle_store.get_record_by_proposal_id(&proposal.id) {
            Ok(Some(record)) => {
                overrides.insert(
                    proposal.id.clone(),
                    materialization_status_from_memory_lifecycle(
                        record.status,
                        record.materialization_status,
                    ),
                );
            }
            Ok(None) => {}
            Err(err) => warnings.push(warning(
                "memory_lifecycle_lookup_failed",
                format!(
                    "Memory lifecycle proof lookup failed for proposal {}: {err}",
                    proposal.id
                ),
            )),
        }
    }
    (overrides, warnings)
}

fn materialization_status_from_memory_lifecycle(
    status: MemoryLifecycleStatus,
    materialization_status: MemoryMaterializationStatus,
) -> ReviewItemMaterializationStatus {
    if status == MemoryLifecycleStatus::RolledBack {
        return ReviewItemMaterializationStatus::RolledBack;
    }
    if status == MemoryLifecycleStatus::MaterializationFailed {
        return ReviewItemMaterializationStatus::Failed;
    }
    match materialization_status {
        MemoryMaterializationStatus::NotRequired => ReviewItemMaterializationStatus::NotApplicable,
        MemoryMaterializationStatus::Pending => ReviewItemMaterializationStatus::Applying,
        MemoryMaterializationStatus::Materialized => ReviewItemMaterializationStatus::Applied,
        MemoryMaterializationStatus::Failed => ReviewItemMaterializationStatus::Failed,
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
    use super::{
        action_materialization_status, bounded_unified_text_diff,
        dispatch_materialization_overrides, REVIEW_TEXT_DIFF_MAX_CHARS,
    };
    use openlife_core::agent::{
        AgentProposal, ProposalSource, ProposalStatus, ProposalStore, ProposalType,
        ReviewItemMaterializationStatus, RiskLevel,
    };
    use serde_json::json;

    fn memory_stop_recall_proposal() -> AgentProposal {
        AgentProposal::new(
            ProposalType::MemoryArchive,
            "memory.lifecycle.memory:test",
            json!({
                "owner": {
                    "ownerKind": "memory_lifecycle",
                    "ownerId": "memory:test"
                },
                "recallDisposition": "paused"
            }),
            "Stop recalling one reviewed Memory.",
            1.0,
            RiskLevel::Medium,
            ProposalSource::Manual,
        )
    }

    #[test]
    fn review_text_diff_is_exact_and_bounded() {
        let (diff, truncated) = bounded_unified_text_diff(
            "# 标题\n\n保留这一段。\n\n旧结论。\n",
            "# 新标题\n\n保留这一段。\n\n短结论。\n",
        );
        assert!(!truncated);
        assert!(diff.contains("--- 当前文件"));
        assert!(diff.contains("+++ 建议版本"));
        assert!(diff.contains("-# 标题"));
        assert!(diff.contains("+# 新标题"));
        assert!(diff.contains(" 保留这一段。"));
        assert!(diff.contains("-旧结论。"));
        assert!(diff.contains("+短结论。"));

        let current = (0..(REVIEW_TEXT_DIFF_MAX_CHARS + 100))
            .map(|index| format!("旧行 {index}\n"))
            .collect::<String>();
        let proposed = (0..(REVIEW_TEXT_DIFF_MAX_CHARS + 100))
            .map(|index| format!("新行 {index}\n"))
            .collect::<String>();
        let (bounded, truncated) = bounded_unified_text_diff(&current, &proposed);
        assert!(truncated);
        assert!(bounded.chars().count() < REVIEW_TEXT_DIFF_MAX_CHARS + 100);
        assert!(bounded.contains("diff 已截断"));
    }

    #[test]
    fn confirmed_dispatch_is_applied_only_after_accepted_projection() {
        assert_eq!(
            action_materialization_status(ProposalStatus::Accepted, "confirmed"),
            Some(ReviewItemMaterializationStatus::Applied)
        );
        assert_eq!(
            action_materialization_status(ProposalStatus::Pending, "confirmed"),
            Some(ReviewItemMaterializationStatus::Unknown)
        );
        assert_eq!(
            action_materialization_status(ProposalStatus::Pending, "unknown"),
            Some(ReviewItemMaterializationStatus::Unknown)
        );
    }

    #[test]
    fn review_projection_reads_confirmed_lifemodel_v2_and_migration_dispatch_receipts() {
        let store = ProposalStore::new_in_memory().expect("proposal store");
        for affected_path in [
            openlife_core::life_model::v2::LIFE_MODEL_V2_TYPED_DIFF_PATH,
            openlife_core::life_model::legacy_migration::LIFE_MODEL_V2_LEGACY_MIGRATION_PATH,
        ] {
            let mut proposal = AgentProposal::new(
                ProposalType::LifeModelUpdate,
                affected_path,
                json!({"schemaVersion": "review-projection-test"}),
                "Apply one reviewed LifeModel v2 change.",
                1.0,
                RiskLevel::Medium,
                ProposalSource::Manual,
            );
            store.create_proposal(&proposal).expect("create proposal");
            let claim = store
                .claim_dispatch(&proposal.id)
                .expect("claim dispatch")
                .expect("claim id");
            assert!(store
                .mark_effect_confirmed_projection_pending(&proposal.id, &claim)
                .expect("persist confirmed effect"));
            proposal.accept();
            assert!(store
                .project_confirmed_effect(&proposal, &claim)
                .expect("project accepted proposal"));

            let (overrides, warnings) =
                dispatch_materialization_overrides(&store, std::slice::from_ref(&proposal));

            assert!(warnings.is_empty());
            assert_eq!(
                overrides.get(&proposal.id),
                Some(&ReviewItemMaterializationStatus::Applied),
                "confirmed dispatch receipt must close {affected_path}"
            );
        }
    }

    #[test]
    fn review_projection_reads_confirmed_memory_retrieval_dispatch_receipt() {
        let store = ProposalStore::new_in_memory().expect("proposal store");
        let mut proposal = memory_stop_recall_proposal();
        store.create_proposal(&proposal).expect("create proposal");
        let claim = store
            .claim_dispatch(&proposal.id)
            .expect("claim dispatch")
            .expect("claim id");
        assert!(store
            .mark_effect_confirmed_projection_pending(&proposal.id, &claim)
            .expect("persist confirmed effect"));
        proposal.accept();
        assert!(store
            .project_confirmed_effect(&proposal, &claim)
            .expect("project accepted proposal"));

        let (overrides, warnings) =
            dispatch_materialization_overrides(&store, std::slice::from_ref(&proposal));

        assert!(warnings.is_empty());
        assert_eq!(
            overrides.get(&proposal.id),
            Some(&ReviewItemMaterializationStatus::Applied)
        );
    }
}
