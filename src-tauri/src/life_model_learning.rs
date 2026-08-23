use crate::AppState;
use chrono::{Duration, Utc};
use openlife_core::agent::{AgentProposal, ProposalSource, ProposalType, RiskLevel};
use openlife_core::agent::{
    DurableWriteRequest, DurableWriteSource, DurableWriteSubject, FinalDeliveryWordingContract,
    LifeModelLearningCapture, LifeModelLearningCaptureReceipt, LifeModelLearningDecisionReceipt,
    LifeModelLearningEvidencePolarity, LifeModelLearningExplicitness,
    LifeModelLearningMaterializationEvidence, LifeModelLearningReviewDecisionReceipt,
    LifeModelLearningSensitivity, LifeModelLearningSourceKind, ProposalStatus, ReviewWorkflow,
};
#[cfg(test)]
use openlife_core::agent::{MemoryCandidate, MemoryCandidateKind, MemoryDestination};
use openlife_core::life_model::v2::{
    LifeModelItemV2, LifeModelSectionV2, LifeModelTypedDiffV2, LifeModelTypedOperationV2,
    LifeModelUserValueV2, DEFAULT_LIFE_MODEL_V2_MODEL_ID, LIFE_MODEL_V2_TYPED_DIFF_PATH,
};
use openlife_core::work_orchestration::AgentLifeModelStatementSection;
use serde::Serialize;
use std::sync::Arc;

const OBSERVATION_RETENTION_DAYS: i64 = 30;
const CANDIDATE_RETENTION_DAYS: i64 = 90;

#[derive(Debug, Clone, PartialEq, Eq)]
struct TypedLearningCandidate {
    section: LifeModelSectionV2,
    statement: String,
    target_key: String,
    suggestion_class: String,
    replaces_target: bool,
}
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeleteLifeModelLearningCandidateReceipt {
    pub candidate_id: String,
    pub deleted: bool,
    pub proposal_deleted: bool,
    pub canonical_life_model_changed: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmLifeModelLearningCandidateReceipt {
    pub candidate_id: String,
    pub status: openlife_core::agent::LifeModelLearningCandidateStatus,
    pub source_kind: LifeModelLearningSourceKind,
    pub proposal_created: bool,
    pub canonical_life_model_changed: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StageLifeModelLearningCandidateReceipt {
    pub candidate_id: String,
    pub proposal_id: String,
    pub status: &'static str,
    pub base_version: Option<u64>,
    pub base_document_digest: Option<String>,
    pub result_document_digest: String,
    pub canonical_life_model_changed: bool,
}

pub(crate) async fn stage_candidate_for_review_with_state(
    state: &Arc<AppState>,
    candidate_id: &str,
) -> Result<StageLifeModelLearningCandidateReceipt, String> {
    let learning_store = state
        .life_model_learning_store
        .as_ref()
        .ok_or_else(|| "lifemodel_learning_store_unavailable".to_string())?;
    let workspace_ref = current_workspace_ref(state).await;
    let learning_store = learning_store.lock().await;
    let candidate = learning_store
        .get_candidate_for_workspace(&workspace_ref, candidate_id)
        .map_err(|error| format!("read_lifemodel_learning_candidate_failed:{error}"))?
        .ok_or_else(|| "lifemodel_learning_candidate_not_found".to_string())?;
    if candidate.status == openlife_core::agent::LifeModelLearningCandidateStatus::Proposed {
        let proposal_id = candidate
            .proposal_id
            .clone()
            .ok_or_else(|| "lifemodel_learning_proposed_candidate_missing_proposal".to_string())?;
        let proposal_store = state
            .proposal_store
            .as_ref()
            .ok_or_else(|| "proposal_store_unavailable".to_string())?;
        let proposal = proposal_store
            .lock()
            .await
            .get_proposal(&proposal_id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "lifemodel_learning_linked_proposal_missing".to_string())?;
        if !matches!(
            proposal.status,
            ProposalStatus::Pending | ProposalStatus::Edited | ProposalStatus::Postponed
        ) {
            return Err("lifemodel_learning_linked_proposal_not_active".into());
        }
        let diff: LifeModelTypedDiffV2 = serde_json::from_value(proposal.after)
            .map_err(|_| "lifemodel_learning_linked_proposal_invalid".to_string())?;
        return Ok(StageLifeModelLearningCandidateReceipt {
            candidate_id: candidate.id,
            proposal_id,
            status: "review_required",
            base_version: diff.base_version,
            base_document_digest: diff.base_document_digest,
            result_document_digest: diff.result_document_digest,
            canonical_life_model_changed: false,
        });
    }
    if candidate.status != openlife_core::agent::LifeModelLearningCandidateStatus::Reviewable
        || candidate.opposition_count > 0
        || candidate.confirmed_at.is_none()
    {
        return Err("lifemodel_learning_candidate_not_review_ready".into());
    }
    let candidate_snapshot_digest =
        openlife_core::agent::life_model_learning_candidate_snapshot_digest(&candidate)
            .map_err(|error| error.to_string())?;

    let manager = state.life_model_manager.lock().await;
    let current = manager
        .load_v2_current(DEFAULT_LIFE_MODEL_V2_MODEL_ID)
        .map_err(|error| error.to_string())?;
    let allow_empty_result = current.is_some();
    let mut proposal = AgentProposal::new(
        ProposalType::LifeModelUpdate,
        LIFE_MODEL_V2_TYPED_DIFF_PATH,
        serde_json::Value::Null,
        "Review one user-confirmed long-term fact before adding it to LifeModel v2.",
        1.0,
        RiskLevel::Medium,
        ProposalSource::ChatConversation,
    );
    proposal.source_detail = Some(format!(
        "lifemodel_learning:{}:{}",
        candidate.id, candidate_snapshot_digest
    ));
    proposal.base_hash = current
        .as_ref()
        .map(|version| version.document_digest.clone());
    let confirmed_at = candidate
        .confirmed_at
        .clone()
        .ok_or_else(|| "lifemodel_learning_candidate_confirmation_missing".to_string())?;
    let item_id = format!("learning:{}", candidate.id);
    let mut source_refs = vec![format!("lifemodel-learning-candidate:{}", candidate.id)];
    for source_ref in candidate.source_refs.iter().take(8) {
        if !source_refs.contains(source_ref) {
            source_refs.push(source_ref.clone());
        }
    }
    let item = candidate
        .value
        .clone()
        .into_item(item_id, source_refs, confirmed_at.clone());
    let diff = LifeModelTypedDiffV2::from_operations_for_review(
        DEFAULT_LIFE_MODEL_V2_MODEL_ID,
        current.as_ref(),
        vec![LifeModelTypedOperationV2::Add {
            section: candidate.section,
            item,
        }],
        allow_empty_result,
    )
    .map_err(|error| error.to_string())?;
    proposal.before = Some(serde_json::json!({
        "schema": "openlife.lifemodel.learning.review.v1",
        "candidateId": candidate.id,
        "candidateSnapshotDigest": candidate_snapshot_digest,
        "section": candidate.section,
        "proposedValue": candidate.value,
        "explicitness": candidate.explicitness,
        "stability": if candidate.independent_support_count >= 2 { "repeated" } else { "user_confirmed" },
        "sensitivity": candidate.sensitivity,
        "conflictStatus": "none",
        "supportCount": candidate.support_count,
        "independentSupportCount": candidate.independent_support_count,
        "sourceRefs": candidate.source_refs.iter().take(8).cloned().collect::<Vec<_>>(),
        "sourceRefsOmitted": candidate.source_refs.len().saturating_sub(8),
        "observationIds": candidate.observation_ids.iter().take(8).cloned().collect::<Vec<_>>(),
        "observationIdsOmitted": candidate.observation_ids.len().saturating_sub(8),
        "sourceKinds": candidate.source_kinds,
        "confirmedAt": confirmed_at,
    }));
    proposal.after = serde_json::to_value(&diff).map_err(|error| error.to_string())?;
    drop(manager);

    let proposal_store = state
        .proposal_store
        .as_ref()
        .ok_or_else(|| "proposal_store_unavailable".to_string())?;
    let proposal_store = proposal_store.lock().await;
    let competing = proposal_store
        .list_all_proposals(200, 0)
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|existing| {
            existing.affected_path == LIFE_MODEL_V2_TYPED_DIFF_PATH
                && matches!(
                    existing.status,
                    ProposalStatus::Pending | ProposalStatus::Edited | ProposalStatus::Postponed
                )
                && existing.source_detail.as_deref() != proposal.source_detail.as_deref()
        });
    if competing.is_some() {
        return Err("lifemodel_learning_another_v2_review_is_active".into());
    }
    let idempotency_key = format!(
        "lifemodel_learning_review:{}:{}:{}",
        candidate.id,
        candidate_snapshot_digest,
        diff.base_document_digest.as_deref().unwrap_or("empty")
    );
    let outcome = ReviewWorkflow::new(&proposal_store)
        .submit(
            DurableWriteRequest::from_agent_proposal(
                DurableWriteSource::MainChat,
                DurableWriteSubject::LifeModel,
                proposal,
                "One LifeModel learning suggestion is pending individual review.",
            )
            .with_idempotency_key(idempotency_key)
            .with_evidence_refs(candidate.source_refs.iter().take(8).cloned().collect())
            .with_final_delivery_wording_contract(
                FinalDeliveryWordingContract::ApprovalRequiredBeforeDurableWrite,
            ),
        )
        .map_err(|error| format!("stage_lifemodel_learning_review_failed:{error}"))?;
    let proposal_id = outcome.proposal_id().to_string();
    drop(proposal_store);
    learning_store
        .link_review_proposal(
            &workspace_ref,
            candidate_id,
            &candidate_snapshot_digest,
            &proposal_id,
            &Utc::now().to_rfc3339(),
        )
        .map_err(|error| format!("link_lifemodel_learning_review_failed:{error}"))?;
    Ok(StageLifeModelLearningCandidateReceipt {
        candidate_id: candidate.id,
        proposal_id,
        status: "review_required",
        base_version: diff.base_version,
        base_document_digest: diff.base_document_digest,
        result_document_digest: diff.result_document_digest,
        canonical_life_model_changed: false,
    })
}

pub(crate) async fn record_lifemodel_learning_review_edit_with_state(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
    statement: &str,
) -> Result<Option<LifeModelLearningReviewDecisionReceipt>, String> {
    let context =
        openlife_core::agent::review_decision_context::build_review_decision_context(proposal, &[])
            .life_model_learning;
    let Some(context) = context else {
        return Ok(None);
    };
    let store = state
        .life_model_learning_store
        .as_ref()
        .ok_or_else(|| "lifemodel_learning_store_unavailable".to_string())?;
    store
        .lock()
        .await
        .record_review_edit(
            &proposal.id,
            &context.candidate_id,
            statement,
            &Utc::now().to_rfc3339(),
        )
        .map(Some)
        .map_err(|error| format!("record_lifemodel_learning_review_edit_failed:{error}"))
}

pub(crate) async fn reconcile_lifemodel_learning_review_edit_with_state(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
) -> Result<Option<LifeModelLearningReviewDecisionReceipt>, String> {
    if proposal.status != ProposalStatus::Edited {
        return Ok(None);
    }
    let context =
        openlife_core::agent::review_decision_context::build_review_decision_context(proposal, &[])
            .life_model_learning;
    let Some(context) = context else {
        return Ok(None);
    };
    let diff: LifeModelTypedDiffV2 = serde_json::from_value(proposal.after.clone())
        .map_err(|_| "lifemodel_learning_edited_diff_invalid".to_string())?;
    let statement = match diff.operations.as_slice() {
        [LifeModelTypedOperationV2::Add {
            item: LifeModelItemV2::Statement(item),
            ..
        }] => item.statement.trim(),
        _ => return Err("lifemodel_learning_edited_operation_invalid".into()),
    };
    if statement.is_empty() || statement != context.proposed_statement.trim() {
        return Err("lifemodel_learning_edited_statement_binding_mismatch".into());
    }
    record_lifemodel_learning_review_edit_with_state(state, proposal, statement).await
}

pub(crate) async fn record_lifemodel_learning_review_rejected_with_state(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
) -> Result<Option<LifeModelLearningReviewDecisionReceipt>, String> {
    let context =
        openlife_core::agent::review_decision_context::build_review_decision_context(proposal, &[])
            .life_model_learning;
    let Some(context) = context else {
        return Ok(None);
    };
    let store = state
        .life_model_learning_store
        .as_ref()
        .ok_or_else(|| "lifemodel_learning_store_unavailable".to_string())?;
    store
        .lock()
        .await
        .record_review_rejected(
            &proposal.id,
            &context.candidate_id,
            &Utc::now().to_rfc3339(),
        )
        .map(Some)
        .map_err(|error| format!("record_lifemodel_learning_review_rejection_failed:{error}"))
}

pub(crate) async fn reconcile_lifemodel_learning_materialization_with_state(
    state: &Arc<AppState>,
    proposal: &AgentProposal,
) -> Result<Option<LifeModelLearningReviewDecisionReceipt>, String> {
    let context =
        openlife_core::agent::review_decision_context::build_review_decision_context(proposal, &[])
            .life_model_learning;
    let Some(context) = context else {
        return Ok(None);
    };
    let diff: LifeModelTypedDiffV2 = serde_json::from_value(proposal.after.clone())
        .map_err(|_| "lifemodel_learning_materialized_diff_invalid".to_string())?;
    let (section, value) = match diff.operations.as_slice() {
        [LifeModelTypedOperationV2::Add {
            section,
            item: LifeModelItemV2::Statement(item),
        }] => (
            *section,
            LifeModelUserValueV2::Statement {
                statement: item.statement.clone(),
            },
        ),
        _ => return Err("lifemodel_learning_materialized_operation_invalid".into()),
    };
    let expected_version = diff
        .base_version
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| "lifemodel_learning_materialized_version_overflow".to_string())?;
    let store = state
        .life_model_learning_store
        .as_ref()
        .ok_or_else(|| "lifemodel_learning_store_unavailable".to_string())?;
    let candidate = store
        .lock()
        .await
        .get_candidate_by_proposal_id(&proposal.id)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "lifemodel_learning_materialized_candidate_missing".to_string())?;
    if candidate.id != context.candidate_id {
        return Err("lifemodel_learning_materialized_candidate_mismatch".into());
    }
    let version = state
        .life_model_manager
        .lock()
        .await
        .load_v2_version(&diff.model_id, expected_version)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "lifemodel_learning_materialized_version_missing".to_string())?;
    let proposal_ref = format!("proposal:{}", proposal.id);
    let candidate_ref = format!("lifemodel-learning-candidate:{}", context.candidate_id);
    let mut reviewed_observation_ids = context.observation_ids.clone();
    if let Some(latest) = candidate.observation_ids.last() {
        if !reviewed_observation_ids.contains(latest) {
            reviewed_observation_ids.push(latest.clone());
        }
    }
    let observation_refs = reviewed_observation_ids
        .iter()
        .map(|id| format!("lifemodel-learning-observation:{id}"))
        .collect::<Vec<_>>();
    if version.document_digest != diff.result_document_digest
        || version.materialization_id != proposal_ref
        || !version.source_refs.contains(&proposal_ref)
        || !version.source_refs.contains(&candidate_ref)
        || observation_refs
            .iter()
            .any(|reference| !version.source_refs.contains(reference))
    {
        return Err("lifemodel_learning_materialized_version_binding_mismatch".into());
    }
    store
        .lock()
        .await
        .record_review_materialized(
            &proposal.id,
            &context.candidate_id,
            section,
            &value,
            LifeModelLearningMaterializationEvidence {
                model_version: version.model_version,
                document_digest: &version.document_digest,
            },
            &Utc::now().to_rfc3339(),
        )
        .map(Some)
        .map_err(|error| format!("record_lifemodel_learning_materialization_failed:{error}"))
}

pub(crate) async fn current_workspace_ref(state: &Arc<AppState>) -> String {
    let _ = state;
    "workspace:default".into()
}

/// Captures a model-proposed, typed LifeModel statement after binding its
/// evidence span to the authenticated user message. The model supplies
/// semantics; this boundary supplies provenance, sensitivity and lifecycle.
pub(crate) async fn capture_typed_explicit_conversation_candidate(
    state: &Arc<AppState>,
    section: AgentLifeModelStatementSection,
    statement: &str,
    source_span: &str,
    user_message_proof: &openlife_core::conversation::ConversationUserMessageProof,
    source_user_message: &str,
) -> Result<LifeModelLearningCaptureReceipt, String> {
    let source_digest = openlife_core::agent::metadata_safe_text_digest(source_user_message).1;
    if !user_message_proof.is_live()
        || user_message_proof.content_digest() != source_digest
        || user_message_proof.content_length_bytes() != source_user_message.len()
        || !source_user_message.contains(source_span)
    {
        return Err("lifemodel_learning_canonical_user_item_mismatch".into());
    }
    if openlife_core::privacy::assess_sensitive_content(source_span).requires_memory_review() {
        return Err("lifemodel_learning_sensitive_content_requires_editor".into());
    }
    let statement = statement.trim();
    if statement.is_empty() || statement.chars().count() > 320 {
        return Err("lifemodel_learning_statement_invalid".into());
    }
    let (section, suggestion_class) = match section {
        AgentLifeModelStatementSection::Identity => (LifeModelSectionV2::Identity, "identity"),
        AgentLifeModelStatementSection::Values => (LifeModelSectionV2::Values, "values"),
        AgentLifeModelStatementSection::StablePreferences => {
            (LifeModelSectionV2::StablePreferences, "stable_preferences")
        }
        AgentLifeModelStatementSection::PersonalBoundaries => (
            LifeModelSectionV2::PersonalBoundaries,
            "personal_boundaries",
        ),
        AgentLifeModelStatementSection::DecisionPrinciples => (
            LifeModelSectionV2::DecisionPrinciples,
            "decision_principles",
        ),
        AgentLifeModelStatementSection::CollaborationPreferences => (
            LifeModelSectionV2::CollaborationPreferences,
            "collaboration_preferences",
        ),
    };
    let digest = openlife_core::agent::metadata_safe_text_digest(statement).1;
    capture_typed_candidate_with_source(
        state,
        TypedLearningCandidate {
            section,
            statement: statement.to_string(),
            target_key: format!("{suggestion_class}.claim:{}", &digest[7..23]),
            suggestion_class: suggestion_class.to_string(),
            replaces_target: false,
        },
        &user_message_proof.item_ref(),
        &source_digest,
    )
    .await
}

#[cfg(test)]
pub(crate) async fn capture_explicit_main_chat_candidate(
    state: &Arc<AppState>,
    candidate: &MemoryCandidate,
    source_user_message: &str,
) -> Result<LifeModelLearningCaptureReceipt, String> {
    let source_digest = openlife_core::agent::metadata_safe_text_digest(source_user_message).1;
    capture_explicit_candidate_with_source(
        state,
        candidate,
        &format!("message:test:{}", &source_digest[7..23]),
        &source_digest,
        source_user_message,
    )
    .await
}

#[cfg(test)]
async fn capture_explicit_candidate_with_source(
    state: &Arc<AppState>,
    candidate: &MemoryCandidate,
    source_ref: &str,
    source_digest: &str,
    source_user_message: &str,
) -> Result<LifeModelLearningCaptureReceipt, String> {
    if openlife_core::privacy::assess_sensitive_content(source_user_message)
        .requires_memory_review()
    {
        return Err("lifemodel_learning_sensitive_content_not_supported_5_3a".into());
    }
    if candidate.destination != MemoryDestination::LifeModelProposal
        || candidate.kind != MemoryCandidateKind::Preference
        || candidate.explicitness != "explicit"
        || candidate.stability != "stable"
        || candidate.sensitivity != "internal"
    {
        return Err("lifemodel_learning_candidate_not_supported_5_3a".into());
    }
    let typed = typed_learning_candidate(candidate)
        .ok_or_else(|| "lifemodel_learning_typed_candidate_required".to_string())?;
    capture_typed_candidate_with_source(state, typed, source_ref, source_digest).await
}

async fn capture_typed_candidate_with_source(
    state: &Arc<AppState>,
    typed: TypedLearningCandidate,
    source_ref: &str,
    source_digest: &str,
) -> Result<LifeModelLearningCaptureReceipt, String> {
    let store = state
        .life_model_learning_store
        .as_ref()
        .ok_or_else(|| "lifemodel_learning_store_unavailable".to_string())?;
    let now = Utc::now();
    let capture = LifeModelLearningCapture {
        workspace_ref: current_workspace_ref(state).await,
        source_ref: source_ref.to_string(),
        source_digest: source_digest.to_string(),
        independence_ref: source_ref.to_string(),
        summary: typed.statement.clone(),
        section: typed.section,
        value: LifeModelUserValueV2::Statement {
            statement: typed.statement,
        },
        target_key: typed.target_key,
        suggestion_class: typed.suggestion_class,
        source_kind: if typed.replaces_target {
            LifeModelLearningSourceKind::UserCorrection
        } else {
            LifeModelLearningSourceKind::ExplicitUserMessage
        },
        polarity: if typed.replaces_target {
            LifeModelLearningEvidencePolarity::Corrects
        } else {
            LifeModelLearningEvidencePolarity::Supports
        },
        replaces_target: typed.replaces_target,
        attach_to_candidate_id: None,
        explicitness: LifeModelLearningExplicitness::ExplicitUserRequest,
        sensitivity: LifeModelLearningSensitivity::Internal,
        observed_at: now.to_rfc3339(),
        observation_expires_at: (now + Duration::days(OBSERVATION_RETENTION_DAYS)).to_rfc3339(),
        candidate_expires_at: (now + Duration::days(CANDIDATE_RETENTION_DAYS)).to_rfc3339(),
    };
    store
        .lock()
        .await
        .capture_explicit_candidate(capture)
        .map_err(|error| format!("capture_lifemodel_learning_candidate_failed:{error}"))
}

pub(crate) async fn delete_candidate_with_state(
    state: &Arc<AppState>,
    candidate_id: &str,
) -> Result<DeleteLifeModelLearningCandidateReceipt, String> {
    let store = state
        .life_model_learning_store
        .as_ref()
        .ok_or_else(|| "lifemodel_learning_store_unavailable".to_string())?;
    let workspace_ref = current_workspace_ref(state).await;
    let deleted = store
        .lock()
        .await
        .delete_candidate(&workspace_ref, candidate_id)
        .map_err(|error| format!("delete_lifemodel_learning_candidate_failed:{error}"))?;
    Ok(DeleteLifeModelLearningCandidateReceipt {
        candidate_id: candidate_id.into(),
        deleted,
        proposal_deleted: false,
        canonical_life_model_changed: false,
    })
}

pub(crate) async fn confirm_candidate_with_state(
    state: &Arc<AppState>,
    candidate_id: &str,
) -> Result<ConfirmLifeModelLearningCandidateReceipt, String> {
    let store = state
        .life_model_learning_store
        .as_ref()
        .ok_or_else(|| "lifemodel_learning_store_unavailable".to_string())?;
    let workspace_ref = current_workspace_ref(state).await;
    let source_ref = format!(
        "feedback:{}",
        &openlife_core::agent::metadata_safe_text_digest(&format!(
            "{workspace_ref}\0{candidate_id}\0confirm"
        ))
        .1[7..31]
    );
    let now = Utc::now();
    let receipt = store
        .lock()
        .await
        .confirm_candidate_as_user_feedback(
            &workspace_ref,
            candidate_id,
            &source_ref,
            &now.to_rfc3339(),
        )
        .map_err(|error| format!("confirm_lifemodel_learning_candidate_failed:{error}"))?;
    Ok(ConfirmLifeModelLearningCandidateReceipt {
        candidate_id: receipt.candidate.id,
        status: receipt.candidate.status,
        source_kind: receipt.observation.source_kind,
        proposal_created: receipt.proposal_created,
        canonical_life_model_changed: receipt.canonical_life_model_changed,
    })
}

pub(crate) async fn reject_candidate_with_state(
    state: &Arc<AppState>,
    candidate_id: &str,
) -> Result<LifeModelLearningDecisionReceipt, String> {
    let store = state
        .life_model_learning_store
        .as_ref()
        .ok_or_else(|| "lifemodel_learning_store_unavailable".to_string())?;
    let workspace_ref = current_workspace_ref(state).await;
    store
        .lock()
        .await
        .reject_and_suppress_candidate(&workspace_ref, candidate_id, &Utc::now().to_rfc3339())
        .map_err(|error| format!("reject_lifemodel_learning_candidate_failed:{error}"))
}

pub(crate) async fn pause_candidate_class_with_state(
    state: &Arc<AppState>,
    candidate_id: &str,
) -> Result<LifeModelLearningDecisionReceipt, String> {
    let store = state
        .life_model_learning_store
        .as_ref()
        .ok_or_else(|| "lifemodel_learning_store_unavailable".to_string())?;
    let workspace_ref = current_workspace_ref(state).await;
    store
        .lock()
        .await
        .pause_suggestion_class(&workspace_ref, candidate_id, &Utc::now().to_rfc3339())
        .map_err(|error| format!("pause_lifemodel_learning_suggestion_class_failed:{error}"))
}

#[cfg(test)]
fn typed_learning_candidate(candidate: &MemoryCandidate) -> Option<TypedLearningCandidate> {
    let claim = candidate.normalized_claim.trim();
    let lower = claim.to_ascii_lowercase();
    let collaboration_correction = lower.contains("communication style to ")
        || claim.contains("沟通风格改为")
        || claim.contains("长期协作方式改为");
    let collaboration = value_after_marker(
        claim,
        &[
            "communication style to ",
            "communication style is ",
            "沟通风格改为",
            "沟通风格是",
            "长期协作方式是",
            "与我长期协作时",
        ],
    );
    if let Some(value) = collaboration {
        return Some(TypedLearningCandidate {
            section: LifeModelSectionV2::CollaborationPreferences,
            statement: value,
            target_key: "collaboration_preferences.communication_style".into(),
            suggestion_class: "collaboration_preferences".into(),
            replaces_target: collaboration_correction,
        });
    }

    let preference = value_after_marker(
        claim,
        &[
            "my long-term preference is ",
            "my long term preference is ",
            "i prefer ",
            "我的长期偏好是",
            "我长期偏好",
            "我偏好",
            "我更喜欢",
        ],
    )?;
    let explicitly_long_term = lower.contains("long-term")
        || lower.contains("long term")
        || lower.contains("always")
        || lower.contains("from now on")
        || claim.contains("长期")
        || claim.contains("以后");
    explicitly_long_term.then(|| {
        let digest = openlife_core::agent::metadata_safe_text_digest(&preference).1;
        TypedLearningCandidate {
            section: LifeModelSectionV2::StablePreferences,
            statement: preference,
            target_key: format!("stable_preferences.claim:{}", &digest[7..23]),
            suggestion_class: "stable_preferences".into(),
            replaces_target: false,
        }
    })
}

#[cfg(test)]
pub(crate) fn supports_explicit_user_text(user_text: &str) -> bool {
    if openlife_core::privacy::assess_sensitive_content(user_text).requires_memory_review() {
        return false;
    }
    let candidate = MemoryCandidate {
        candidate_id: "preflight".into(),
        source_span_id: "preflight".into(),
        kind: MemoryCandidateKind::Preference,
        destination: MemoryDestination::LifeModelProposal,
        evidence_text: String::new(),
        source_preview: String::new(),
        normalized_claim: user_text.into(),
        sensitivity: "internal".into(),
        stability: "stable".into(),
        explicitness: "explicit".into(),
        future_actionability: "future_actionable".into(),
        confidence: 1.0,
        reason_codes: Vec::new(),
    };
    typed_learning_candidate(&candidate).is_some()
}

#[cfg(test)]
fn value_after_marker(value: &str, markers: &[&str]) -> Option<String> {
    let lower = value.to_ascii_lowercase();
    markers.iter().find_map(|marker| {
        let marker_lower = marker.to_ascii_lowercase();
        let index = lower.find(&marker_lower)?;
        let candidate =
            value
                .get(index + marker.len()..)?
                .trim()
                .trim_matches(|character: char| {
                    character.is_whitespace()
                        || matches!(
                            character,
                            '.' | ','
                                | ';'
                                | ':'
                                | '!'
                                | '?'
                                | '。'
                                | '，'
                                | '；'
                                | '：'
                                | '！'
                                | '？'
                                | '"'
                                | '\''
                        )
                });
        (!candidate.is_empty() && candidate.chars().count() <= 240).then(|| candidate.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(claim: &str) -> MemoryCandidate {
        let (_, digest) = openlife_core::agent::metadata_safe_text_digest(claim);
        MemoryCandidate {
            candidate_id: format!("candidate:{digest}"),
            source_span_id: format!("span:{digest}"),
            kind: MemoryCandidateKind::Preference,
            destination: MemoryDestination::LifeModelProposal,
            evidence_text: claim.into(),
            source_preview: claim.into(),
            normalized_claim: claim.into(),
            sensitivity: "internal".into(),
            stability: "stable".into(),
            explicitness: "explicit".into(),
            future_actionability: "future_actionable".into(),
            confidence: 0.9,
            reason_codes: vec!["stable_identity_or_preference".into()],
        }
    }

    #[test]
    fn maps_only_precise_long_term_preference_or_collaboration_statement() {
        assert_eq!(
            typed_learning_candidate(&candidate(
                "Update my life model: communication style is concise"
            )),
            Some(TypedLearningCandidate {
                section: LifeModelSectionV2::CollaborationPreferences,
                statement: "concise".into(),
                target_key: "collaboration_preferences.communication_style".into(),
                suggestion_class: "collaboration_preferences".into(),
                replaces_target: false,
            })
        );
        assert!(typed_learning_candidate(&candidate(
            "Update my life model: communication style to detailed"
        ))
        .is_some_and(|candidate| candidate.replaces_target));
        assert_eq!(
            typed_learning_candidate(&candidate("我的长期偏好是先给结论")),
            Some(TypedLearningCandidate {
                section: LifeModelSectionV2::StablePreferences,
                statement: "先给结论".into(),
                target_key: format!(
                    "stable_preferences.claim:{}",
                    &openlife_core::agent::metadata_safe_text_digest("先给结论").1[7..23]
                ),
                suggestion_class: "stable_preferences".into(),
                replaces_target: false,
            })
        );
        assert!(
            typed_learning_candidate(&candidate("Update my life model from this chat")).is_none()
        );
        assert!(!supports_explicit_user_text(
            "My long-term preference is user_password=hunter2"
        ));
    }

    #[tokio::test]
    async fn production_bridge_stages_candidate_without_proposal_or_canonical_version() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let user_text = "Update my life model: communication style is concise and direct.";
        let normal_candidate = candidate(user_text);
        let proposals_before = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_all_proposals(100, 0)
            .unwrap()
            .into_iter()
            .map(|proposal| proposal.id)
            .collect::<Vec<_>>();
        let canonical_before = state
            .life_model_manager
            .lock()
            .await
            .load_v2_current(openlife_core::life_model::v2::DEFAULT_LIFE_MODEL_V2_MODEL_ID)
            .unwrap();

        let receipt = capture_explicit_main_chat_candidate(&state, &normal_candidate, user_text)
            .await
            .unwrap();

        assert!(!receipt.proposal_created);
        assert!(!receipt.canonical_life_model_changed);
        assert_eq!(
            state
                .proposal_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .list_all_proposals(100, 0)
                .unwrap()
                .into_iter()
                .map(|proposal| proposal.id)
                .collect::<Vec<_>>(),
            proposals_before
        );
        assert_eq!(
            state
                .life_model_manager
                .lock()
                .await
                .load_v2_current(openlife_core::life_model::v2::DEFAULT_LIFE_MODEL_V2_MODEL_ID)
                .unwrap(),
            canonical_before
        );
        let view = crate::read_models::life_model::get_life_model_view_model_with_state(&state)
            .await
            .unwrap();
        assert!(view.data.as_ref().is_some_and(|model| {
            model.learning.available
                && model.learning.candidates.len() == 1
                && model.learning.candidates[0].id == receipt.candidate.id
        }));

        let deletion = delete_candidate_with_state(&state, &receipt.candidate.id)
            .await
            .unwrap();
        assert!(deletion.deleted);
        assert!(!deletion.proposal_deleted);
        assert!(!deletion.canonical_life_model_changed);
        let refreshed =
            crate::read_models::life_model::get_life_model_view_model_with_state(&state)
                .await
                .unwrap();
        assert!(refreshed.data.as_ref().is_some_and(|model| {
            model.learning.available && model.learning.candidates.is_empty()
        }));

        let sensitive_text = "My long-term preference is user_password=hunter2";
        let sensitive_candidate = candidate(sensitive_text);
        assert_eq!(sensitive_candidate.sensitivity, "internal");
        assert!(
            capture_explicit_main_chat_candidate(&state, &sensitive_candidate, sensitive_text,)
                .await
                .is_err()
        );
        assert!(state
            .life_model_learning_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_active_candidates("workspace:default", 20)
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn product_rejection_scrubs_candidate_without_touching_proposal_or_canonical_owner() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let user_text = "Update my life model: communication style is concise and direct.";
        let candidate = candidate(user_text);
        let staged = capture_explicit_main_chat_candidate(&state, &candidate, user_text)
            .await
            .unwrap();
        let proposals_before = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_all_proposals(100, 0)
            .unwrap()
            .into_iter()
            .map(|proposal| proposal.id)
            .collect::<Vec<_>>();
        let canonical_before = state
            .life_model_manager
            .lock()
            .await
            .load_v2_current(openlife_core::life_model::v2::DEFAULT_LIFE_MODEL_V2_MODEL_ID)
            .unwrap();

        let receipt = reject_candidate_with_state(&state, &staged.candidate.id)
            .await
            .unwrap();

        assert!(receipt.changed);
        assert!(receipt.content_scrubbed);
        assert_eq!(
            receipt.suppression_kind,
            Some(openlife_core::agent::LifeModelLearningSuppressionKind::ExactCandidate)
        );
        assert!(!receipt.proposal_changed);
        assert!(!receipt.canonical_life_model_changed);
        assert!(
            crate::read_models::life_model::get_life_model_view_model_with_state(&state)
                .await
                .unwrap()
                .data
                .is_some_and(|model| model.learning.candidates.is_empty())
        );
        assert_eq!(
            state
                .proposal_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .list_all_proposals(100, 0)
                .unwrap()
                .into_iter()
                .map(|proposal| proposal.id)
                .collect::<Vec<_>>(),
            proposals_before
        );
        assert_eq!(
            state
                .life_model_manager
                .lock()
                .await
                .load_v2_current(openlife_core::life_model::v2::DEFAULT_LIFE_MODEL_V2_MODEL_ID)
                .unwrap(),
            canonical_before
        );
    }

    #[tokio::test]
    async fn confirmed_candidate_stages_one_exact_review_without_canonical_write() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let user_text = "Update my life model: communication style is concise and direct.";
        let candidate = candidate(user_text);
        let captured = capture_explicit_main_chat_candidate(&state, &candidate, user_text)
            .await
            .unwrap();
        assert!(captured.candidate.confirmed_at.is_some());
        let canonical_before = state
            .life_model_manager
            .lock()
            .await
            .load_v2_current(DEFAULT_LIFE_MODEL_V2_MODEL_ID)
            .unwrap();

        let receipt = stage_candidate_for_review_with_state(&state, &captured.candidate.id)
            .await
            .unwrap();
        let replay = stage_candidate_for_review_with_state(&state, &captured.candidate.id)
            .await
            .unwrap();

        assert_eq!(receipt, replay);
        assert_eq!(receipt.status, "review_required");
        assert!(!receipt.canonical_life_model_changed);
        let proposal = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_proposal(&receipt.proposal_id)
            .unwrap()
            .unwrap();
        let context = openlife_core::agent::review_decision_context::build_review_decision_context(
            &proposal,
            &[],
        );
        let learning = context.life_model_learning.unwrap();
        assert_eq!(learning.candidate_id, captured.candidate.id);
        assert_eq!(learning.proposed_statement, "concise and direct");
        assert_eq!(learning.conflict_status, "none");
        assert!(state
            .life_model_learning_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_active_candidates("workspace:default", 5)
            .unwrap()
            .is_empty());
        assert_eq!(
            state
                .life_model_manager
                .lock()
                .await
                .load_v2_current(DEFAULT_LIFE_MODEL_V2_MODEL_ID)
                .unwrap(),
            canonical_before
        );

        let proposal_store = state.proposal_store.as_ref().unwrap();
        let mut terminal_proposal = proposal_store
            .lock()
            .await
            .get_proposal(&receipt.proposal_id)
            .unwrap()
            .unwrap();
        terminal_proposal.reject();
        proposal_store
            .lock()
            .await
            .update_proposal(&terminal_proposal)
            .unwrap();
        assert_eq!(
            stage_candidate_for_review_with_state(&state, &captured.candidate.id)
                .await
                .unwrap_err(),
            "lifemodel_learning_linked_proposal_not_active"
        );
    }

    #[tokio::test]
    async fn learning_review_uses_schema_aware_edit_without_materializing() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let user_text = "Update my life model: communication style is concise and direct.";
        let candidate = candidate(user_text);
        let captured = capture_explicit_main_chat_candidate(&state, &candidate, user_text)
            .await
            .unwrap();
        let staged = stage_candidate_for_review_with_state(&state, &captured.candidate.id)
            .await
            .unwrap();
        let canonical_before = state
            .life_model_manager
            .lock()
            .await
            .load_v2_current(DEFAULT_LIFE_MODEL_V2_MODEL_ID)
            .unwrap();

        let edit = crate::commands::proposal::edit_lifemodel_learning_proposal_with_state(
            staged.proposal_id.clone(),
            "简洁、直接，并先给结论".into(),
            &state,
        )
        .await
        .unwrap();
        assert_eq!(edit["status"], "edited_pending_review");
        assert_eq!(edit["durableWriteExecuted"], false);
        assert_eq!(edit["learning"]["status"], "proposed");
        assert_eq!(edit["learning"]["canonicalLifeModelChanged"], false);
        let proposal = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_proposal(&staged.proposal_id)
            .unwrap()
            .unwrap();
        assert_eq!(proposal.status, ProposalStatus::Edited);
        let context = openlife_core::agent::review_decision_context::build_review_decision_context(
            &proposal,
            &[],
        );
        assert_eq!(
            context.life_model_learning.unwrap().proposed_statement,
            "简洁、直接，并先给结论"
        );
        assert_eq!(
            state
                .life_model_manager
                .lock()
                .await
                .load_v2_current(DEFAULT_LIFE_MODEL_V2_MODEL_ID)
                .unwrap(),
            canonical_before
        );

        let accepted = crate::commands::proposal::accept_proposal_with_state(
            staged.proposal_id.clone(),
            &state,
        )
        .await
        .unwrap();
        assert_eq!(accepted["lifeModelLearning"]["status"], "materialized");
        assert_eq!(
            accepted["lifeModelLearning"]["canonicalLifeModelChanged"],
            true
        );
        let version = state
            .life_model_manager
            .lock()
            .await
            .load_v2_current(DEFAULT_LIFE_MODEL_V2_MODEL_ID)
            .unwrap()
            .unwrap();
        assert_eq!(version.model_version, 1);
        assert_eq!(
            version.materialization_id,
            format!("proposal:{}", staged.proposal_id)
        );
        assert!(version.source_refs.contains(&format!(
            "lifemodel-learning-candidate:{}",
            captured.candidate.id
        )));
        let materialized = state
            .life_model_learning_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_candidate_by_proposal_id(&staged.proposal_id)
            .unwrap()
            .unwrap();
        assert_eq!(
            materialized.status,
            openlife_core::agent::LifeModelLearningCandidateStatus::Materialized
        );
        assert_eq!(materialized.materialized_version, Some(1));
        assert!(materialized
            .source_kinds
            .contains(&LifeModelLearningSourceKind::UserCorrection));
        for observation_id in materialized.observation_ids {
            assert!(version
                .source_refs
                .contains(&format!("lifemodel-learning-observation:{observation_id}")));
        }
    }

    #[tokio::test]
    async fn accepted_learning_review_reconciles_a_persisted_edit_after_candidate_write_failure() {
        let mut state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let user_text = "Update my life model: communication style is concise and direct.";
        let candidate = candidate(user_text);
        let captured = capture_explicit_main_chat_candidate(&state, &candidate, user_text)
            .await
            .unwrap();
        let staged = stage_candidate_for_review_with_state(&state, &captured.candidate.id)
            .await
            .unwrap();
        let healthy_learning_store = state.life_model_learning_store.clone().unwrap();
        Arc::get_mut(&mut state).unwrap().life_model_learning_store = None;

        let error = crate::commands::proposal::edit_lifemodel_learning_proposal_with_state(
            staged.proposal_id.clone(),
            "简洁、直接，并先给结论".into(),
            &state,
        )
        .await
        .unwrap_err();
        assert!(error.contains("lifemodel_learning_store_unavailable"));
        let persisted = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_proposal(&staged.proposal_id)
            .unwrap()
            .unwrap();
        assert_eq!(persisted.status, ProposalStatus::Edited);
        assert_eq!(
            openlife_core::agent::review_decision_context::build_review_decision_context(
                &persisted,
                &[],
            )
            .life_model_learning
            .unwrap()
            .proposed_statement,
            "简洁、直接，并先给结论"
        );

        Arc::get_mut(&mut state).unwrap().life_model_learning_store = Some(healthy_learning_store);
        let accepted = crate::commands::proposal::accept_proposal_with_state(
            staged.proposal_id.clone(),
            &state,
        )
        .await
        .unwrap();
        assert_eq!(accepted["lifeModelLearning"]["status"], "materialized");
        let materialized = state
            .life_model_learning_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_candidate_by_proposal_id(&staged.proposal_id)
            .unwrap()
            .unwrap();
        assert!(materialized
            .source_kinds
            .contains(&LifeModelLearningSourceKind::UserCorrection));
        let version = state
            .life_model_manager
            .lock()
            .await
            .load_v2_current(DEFAULT_LIFE_MODEL_V2_MODEL_ID)
            .unwrap()
            .unwrap();
        for observation_id in materialized.observation_ids {
            assert!(version
                .source_refs
                .contains(&format!("lifemodel-learning-observation:{observation_id}")));
        }
    }

    #[tokio::test]
    async fn postponed_learning_review_is_not_rejection_and_reject_enters_cooldown_without_write() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let user_text = "Update my life model: communication style is concise and direct.";
        let candidate = candidate(user_text);
        let captured = capture_explicit_main_chat_candidate(&state, &candidate, user_text)
            .await
            .unwrap();
        let staged = stage_candidate_for_review_with_state(&state, &captured.candidate.id)
            .await
            .unwrap();
        let canonical_before = state
            .life_model_manager
            .lock()
            .await
            .load_v2_current(DEFAULT_LIFE_MODEL_V2_MODEL_ID)
            .unwrap();

        crate::commands::proposal::postpone_proposal_with_state(staged.proposal_id.clone(), &state)
            .await
            .unwrap();
        let deferred_candidate = state
            .life_model_learning_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_candidate_by_proposal_id(&staged.proposal_id)
            .unwrap()
            .unwrap();
        assert_eq!(
            deferred_candidate.status,
            openlife_core::agent::LifeModelLearningCandidateStatus::Proposed
        );

        crate::commands::proposal::reject_proposal_with_state(staged.proposal_id.clone(), &state)
            .await
            .unwrap();
        let rejected_proposal = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_proposal(&staged.proposal_id)
            .unwrap()
            .unwrap();
        let replay =
            record_lifemodel_learning_review_rejected_with_state(&state, &rejected_proposal)
                .await
                .unwrap()
                .unwrap();
        assert!(!replay.changed);
        assert_eq!(
            replay.status,
            openlife_core::agent::LifeModelLearningCandidateStatus::Rejected
        );
        assert!(replay.content_scrubbed);
        assert!(replay.cooldown_until.is_some());
        assert_eq!(
            state
                .life_model_manager
                .lock()
                .await
                .load_v2_current(DEFAULT_LIFE_MODEL_V2_MODEL_ID)
                .unwrap(),
            canonical_before
        );
    }

    #[tokio::test]
    async fn stale_learning_review_never_marks_candidate_materialized_or_rebases() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let user_text = "Update my life model: communication style is concise and direct.";
        let candidate = candidate(user_text);
        let captured = capture_explicit_main_chat_candidate(&state, &candidate, user_text)
            .await
            .unwrap();
        let staged = stage_candidate_for_review_with_state(&state, &captured.candidate.id)
            .await
            .unwrap();

        let current = state
            .life_model_manager
            .lock()
            .await
            .load_v2_current(DEFAULT_LIFE_MODEL_V2_MODEL_ID)
            .unwrap();
        let manual_item = LifeModelUserValueV2::Statement {
            statement: "A separate reviewed fact".into(),
        }
        .into_item(
            "manual:separate-fact".into(),
            vec!["manual:separate-review".into()],
            "2026-08-09T10:00:00Z".into(),
        );
        let manual_diff = LifeModelTypedDiffV2::from_operations_for_review(
            DEFAULT_LIFE_MODEL_V2_MODEL_ID,
            current.as_ref(),
            vec![LifeModelTypedOperationV2::Add {
                section: LifeModelSectionV2::StablePreferences,
                item: manual_item,
            }],
            true,
        )
        .unwrap();
        let mut manual_proposal = AgentProposal::new(
            ProposalType::LifeModelUpdate,
            LIFE_MODEL_V2_TYPED_DIFF_PATH,
            serde_json::to_value(&manual_diff).unwrap(),
            "Separate reviewed LifeModel change.",
            1.0,
            RiskLevel::Medium,
            ProposalSource::Manual,
        );
        manual_proposal.base_hash = manual_diff.base_document_digest.clone();
        let manual_id = manual_proposal.id.clone();
        state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .create_proposal(&manual_proposal)
            .unwrap();
        crate::commands::proposal::accept_proposal_with_state(manual_id, &state)
            .await
            .unwrap();

        let error = crate::commands::proposal::accept_proposal_with_state(
            staged.proposal_id.clone(),
            &state,
        )
        .await
        .unwrap_err();
        assert!(
            error.contains("lifemodel_v2_typed_diff_stale_base"),
            "unexpected stale learning proposal error: {error}"
        );
        let candidate = state
            .life_model_learning_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_candidate_by_proposal_id(&staged.proposal_id)
            .unwrap()
            .unwrap();
        assert_eq!(
            candidate.status,
            openlife_core::agent::LifeModelLearningCandidateStatus::Proposed
        );
        assert_eq!(candidate.materialized_version, None);
        assert_eq!(
            state
                .proposal_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .get_proposal(&staged.proposal_id)
                .unwrap()
                .unwrap()
                .status,
            ProposalStatus::Pending
        );
    }

    #[tokio::test]
    async fn second_v2_learning_review_waits_for_the_active_item() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let mut candidate_ids = Vec::new();
        for user_text in [
            "Update my life model: my long-term preference is morning planning.",
            "Update my life model: my long-term preference is weekly reviews.",
        ] {
            let candidate = candidate(user_text);
            candidate_ids.push(
                capture_explicit_main_chat_candidate(&state, &candidate, user_text)
                    .await
                    .unwrap()
                    .candidate
                    .id,
            );
        }

        stage_candidate_for_review_with_state(&state, &candidate_ids[0])
            .await
            .unwrap();
        assert_eq!(
            stage_candidate_for_review_with_state(&state, &candidate_ids[1])
                .await
                .unwrap_err(),
            "lifemodel_learning_another_v2_review_is_active"
        );
    }
}
