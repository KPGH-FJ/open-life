use crate::AppState;
use chrono::{Duration, Utc};
use openlife_core::agent::main_chat_agent_v1::PolicyDecision;
use openlife_core::agent::{
    LifeModelLearningCapture, LifeModelLearningCaptureReceipt, LifeModelLearningDecisionReceipt,
    LifeModelLearningExplicitness, LifeModelLearningSensitivity, MainChatMemoryCandidate,
    MemoryCandidateKind, MemoryDestination,
};
use openlife_core::life_model::v2::{LifeModelSectionV2, LifeModelUserValueV2};
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
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeleteLifeModelLearningCandidateReceipt {
    pub candidate_id: String,
    pub deleted: bool,
    pub proposal_deleted: bool,
    pub canonical_life_model_changed: bool,
}

pub(crate) async fn current_workspace_ref(state: &Arc<AppState>) -> String {
    let configured_root = state
        .config
        .lock()
        .await
        .system
        .workspace_memory_root
        .clone();
    match configured_root.filter(|root| !root.trim().is_empty()) {
        Some(root) => format!(
            "workspace:{}",
            openlife_core::agent::metadata_safe_text_digest(root.trim()).1
        ),
        None => "workspace:default".into(),
    }
}

pub(crate) async fn capture_explicit_main_chat_candidate(
    state: &Arc<AppState>,
    candidate: &MainChatMemoryCandidate,
    policy: &PolicyDecision,
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
    let source_digest = openlife_core::agent::metadata_safe_text_digest(source_user_message).1;
    if source_digest != policy.authorized_user_message_digest {
        return Err("lifemodel_learning_source_digest_mismatch".into());
    }
    if policy.authorized_user_message_id.trim().is_empty()
        || !policy.allows_memory_candidate(&candidate.candidate_id)
    {
        return Err("lifemodel_learning_policy_authority_missing".into());
    }
    let typed = typed_learning_candidate(candidate)
        .ok_or_else(|| "lifemodel_learning_typed_candidate_required".to_string())?;
    let store = state
        .life_model_learning_store
        .as_ref()
        .ok_or_else(|| "lifemodel_learning_store_unavailable".to_string())?;
    let now = Utc::now();
    let capture = LifeModelLearningCapture {
        workspace_ref: current_workspace_ref(state).await,
        source_ref: format!("message:{}", policy.authorized_user_message_id),
        source_digest,
        independence_ref: format!("message:{}", policy.authorized_user_message_id),
        summary: typed.statement.clone(),
        section: typed.section,
        value: LifeModelUserValueV2::Statement {
            statement: typed.statement,
        },
        target_key: typed.target_key,
        suggestion_class: typed.suggestion_class,
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

fn typed_learning_candidate(candidate: &MainChatMemoryCandidate) -> Option<TypedLearningCandidate> {
    let claim = candidate.normalized_claim.trim();
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
    let lower = claim.to_ascii_lowercase();
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
        }
    })
}

pub(crate) fn supports_explicit_user_text(user_text: &str) -> bool {
    if openlife_core::privacy::assess_sensitive_content(user_text).requires_memory_review() {
        return false;
    }
    let candidate = MainChatMemoryCandidate {
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

    fn candidate(claim: &str) -> MainChatMemoryCandidate {
        MainChatMemoryCandidate {
            candidate_id: "candidate-one".into(),
            source_span_id: "span-one".into(),
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
            })
        );
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
        let decision = openlife_core::agent::main_chat_agent_v1::AgentIngress::default().decide(
            "lifemodel-learning-bridge",
            user_text,
            None,
            openlife_core::agent::AgentTaskKind::Conversation,
        );
        let routing = openlife_core::agent::plan_main_chat_memory_routing(user_text);
        let normal_candidate = routing
            .candidates
            .iter()
            .find(|candidate| candidate.destination == MemoryDestination::LifeModelProposal)
            .expect("exact LifeModel learning candidate");
        assert!(decision
            .policy_decision
            .allows_memory_candidate(&normal_candidate.candidate_id));
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

        let receipt = capture_explicit_main_chat_candidate(
            &state,
            normal_candidate,
            &decision.policy_decision,
            user_text,
        )
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
        let mut sensitive_policy = decision.policy_decision.clone();
        sensitive_policy.authorized_user_message_digest =
            openlife_core::agent::metadata_safe_text_digest(sensitive_text).1;
        sensitive_policy.authorized_memory_candidate_ids =
            vec![sensitive_candidate.candidate_id.clone()];
        assert_eq!(sensitive_candidate.sensitivity, "internal");
        assert!(capture_explicit_main_chat_candidate(
            &state,
            &sensitive_candidate,
            &sensitive_policy,
            sensitive_text,
        )
        .await
        .is_err());
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
        let decision = openlife_core::agent::main_chat_agent_v1::AgentIngress::default().decide(
            "lifemodel-learning-rejection",
            user_text,
            None,
            openlife_core::agent::AgentTaskKind::Conversation,
        );
        let routing = openlife_core::agent::plan_main_chat_memory_routing(user_text);
        let candidate = routing
            .candidates
            .iter()
            .find(|candidate| candidate.destination == MemoryDestination::LifeModelProposal)
            .unwrap();
        let staged = capture_explicit_main_chat_candidate(
            &state,
            candidate,
            &decision.policy_decision,
            user_text,
        )
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
}
