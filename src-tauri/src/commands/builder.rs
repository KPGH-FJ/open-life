use crate::errors::AppError;
use crate::AppState;
use openlife_core::agent::{
    AgentProposal, DurableWriteDecisionKind, DurableWriteRequest, DurableWriteSource,
    DurableWriteSubject, ProposalSource, ProposalType, ReviewWorkflow,
    RiskLevel as ProposalRiskLevel,
};
use openlife_core::builder::{
    BuilderDimension, BuilderEngine, BuilderMode, BuilderSession, BuilderSessionRetentionStatus,
    BuilderSummary,
};
use serde::Serialize;
use std::sync::Arc;
use tauri::State;

const BUILDER_USER_REPLY_MAX_BYTES: usize = 64 * 1024;
const BUILDER_REVIEW_MAX_DECISIONS: usize =
    openlife_core::life_model::patch::MAX_LIFEMODEL_PATCH_BATCH_OPERATIONS;
const BUILDER_REVIEW_VALUE_MAX_BYTES: usize =
    openlife_core::life_model::patch::MAX_LIFEMODEL_PATCH_BATCH_BYTES;
const BUILDER_REVIEW_TOTAL_MAX_BYTES: usize =
    openlife_core::life_model::patch::MAX_LIFEMODEL_PATCH_BATCH_BYTES;

fn validate_builder_session_id(session_id: &str) -> Result<(), AppError> {
    if session_id.trim() != session_id
        || session_id.is_empty()
        || session_id.len() > 256
        || session_id.chars().any(char::is_control)
    {
        return Err(AppError::serialization("Builder session id is invalid."));
    }
    Ok(())
}

fn validate_builder_user_reply(user_reply: &str) -> Result<(), AppError> {
    if user_reply.len() > BUILDER_USER_REPLY_MAX_BYTES
        || user_reply.chars().any(|character| {
            character == '\0' || character == '\u{fffe}' || character == '\u{ffff}'
        })
    {
        return Err(AppError::serialization(
            "Builder answer is invalid or exceeds the bounded input limit.",
        ));
    }
    Ok(())
}

fn validate_builder_decisions(
    decisions: &[openlife_core::builder::BuilderSignalDecision],
) -> Result<(), AppError> {
    if decisions.len() > BUILDER_REVIEW_MAX_DECISIONS {
        return Err(AppError::serialization(
            "Builder review exceeds the bounded decision limit.",
        ));
    }
    let mut total_bytes = 0usize;
    for decision in decisions {
        let proposed_value_bytes = decision
            .proposed_value
            .as_ref()
            .map(serde_json::to_vec)
            .transpose()
            .map_err(|_| AppError::serialization("Builder review decision is not serializable."))?
            .map(|bytes| bytes.len())
            .unwrap_or(0);
        if decision.id.trim() != decision.id
            || decision.id.is_empty()
            || decision.id.len() > 256
            || decision.id.chars().any(char::is_control)
            || decision.status.len() > 32
            || decision.status.chars().any(char::is_control)
            || proposed_value_bytes > BUILDER_REVIEW_VALUE_MAX_BYTES
        {
            return Err(AppError::serialization(
                "Builder review contains an invalid or oversized decision.",
            ));
        }
        total_bytes = total_bytes
            .checked_add(decision.id.len())
            .and_then(|total| total.checked_add(decision.status.len()))
            .and_then(|total| total.checked_add(proposed_value_bytes))
            .filter(|total| *total <= BUILDER_REVIEW_TOTAL_MAX_BYTES)
            .ok_or_else(|| {
                AppError::serialization("Builder review exceeds the bounded payload limit.")
            })?;
    }
    Ok(())
}

fn normalize_legacy_builder_signal(
    mut signal: openlife_core::builder::BuilderSignal,
) -> Result<openlife_core::builder::BuilderSignal, AppError> {
    match signal.affected_path.as_str() {
        "state.alerts" => {
            let messages = signal
                .proposed_value
                .as_array()
                .ok_or_else(|| {
                    AppError::serialization(
                        "Legacy Builder alert candidate is malformed and cannot be migrated.",
                    )
                })?
                .iter()
                .map(|item| {
                    item.get("message")
                        .and_then(serde_json::Value::as_str)
                        .map(str::trim)
                        .filter(|message| !message.is_empty())
                        .map(ToOwned::to_owned)
                        .ok_or_else(|| {
                            AppError::serialization(
                                "Legacy Builder alert candidate is missing its blocker text.",
                            )
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;
            if messages.is_empty() {
                return Err(AppError::serialization(
                    "Legacy Builder alert candidate contains no reviewable blocker text.",
                ));
            }
            signal.affected_path = "state.open_questions".into();
            signal.proposed_value = serde_json::json!(messages);
            signal.reason = format!("{}; migrated_from_derived_state_alert", signal.reason);
            Ok(signal)
        }
        "goals.daily" => Err(AppError::permission(
            "Legacy Builder daily-task candidates cannot write LifeModel after the StateStore cutover. Reject this candidate and create the task through Main Chat so current-user authority, TTL, and the StateGateway receipt are preserved.",
        )),
        _ => Ok(signal),
    }
}

#[derive(Debug, Serialize)]
pub struct BuilderSessionSummary {
    session_id: String,
    mode: BuilderMode,
    step_index: usize,
    finished: bool,
    current_prompt: String,
    pending_signal_count: usize,
    waiting_for_review: bool,
    review_in_progress: bool,
    target_dimension: Option<BuilderDimension>,
    retention_status: Option<BuilderSessionRetentionStatus>,
    expires_at: Option<chrono::DateTime<chrono::Utc>>,
    purge_after: Option<chrono::DateTime<chrono::Utc>>,
}

impl From<BuilderSession> for BuilderSessionSummary {
    fn from(session: BuilderSession) -> Self {
        let pending_signal_count = session.pending_signals.len();
        let waiting_for_review = session.finished && pending_signal_count > 0;
        let review_in_progress = session.review_claim_id.is_some();
        // The stored prompt can contain answer-derived material in Socratic or
        // pairwise lanes. Resume-list IPC therefore exposes only a static,
        // domain-derived label while the full prompt remains behind explicit
        // session resume/get boundaries.
        let current_prompt = if review_in_progress {
            "审阅提交正在协调".to_string()
        } else if waiting_for_review {
            "待确认的 Builder 候选已准备好".to_string()
        } else {
            session.progress().current_step_label
        };
        Self {
            session_id: session.session_id,
            mode: session.mode,
            step_index: session.step_index,
            finished: session.finished,
            current_prompt,
            pending_signal_count,
            waiting_for_review,
            review_in_progress,
            target_dimension: session.target_dimension,
            retention_status: session.retention.status_at(chrono::Utc::now()),
            expires_at: session.retention.expires_at,
            purge_after: session.retention.purge_after,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct BuilderPendingSignalView {
    id: String,
    source_step: usize,
    source_question_id: String,
    dimension: BuilderDimension,
    affected_path: String,
    proposed_value: serde_json::Value,
    confidence: f32,
    reason: String,
    risk_level: String,
    user_status: String,
}

impl From<&openlife_core::builder::BuilderSignal> for BuilderPendingSignalView {
    fn from(signal: &openlife_core::builder::BuilderSignal) -> Self {
        Self {
            id: signal.id.clone(),
            source_step: signal.source_step,
            source_question_id: signal.source_question_id.clone(),
            dimension: signal.dimension,
            affected_path: signal.affected_path.clone(),
            proposed_value: signal.proposed_value.clone(),
            confidence: signal.confidence,
            reason: signal.reason.clone(),
            risk_level: signal.risk_level.to_string(),
            user_status: format!("{:?}", signal.user_status),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct BuilderPendingSignalsView {
    session_id: String,
    signals: Vec<BuilderPendingSignalView>,
    summary: BuilderSummary,
    finished: bool,
}

fn pending_signals_view(session: &BuilderSession) -> BuilderPendingSignalsView {
    let signals = session
        .pending_signals
        .iter()
        .map(BuilderPendingSignalView::from)
        .collect();
    let count_for = |dimension| {
        session
            .pending_signals
            .iter()
            .filter(|signal| signal.dimension == dimension)
            .count()
    };
    BuilderPendingSignalsView {
        session_id: session.session_id.clone(),
        signals,
        summary: BuilderSummary {
            identity_summary: format!("基于 {} 个信号", count_for(BuilderDimension::Identity)),
            goals_summary: format!("基于 {} 个信号", count_for(BuilderDimension::Goals)),
            capabilities_summary: format!(
                "基于 {} 个信号",
                count_for(BuilderDimension::Capabilities)
            ),
            state_summary: format!("基于 {} 个信号", count_for(BuilderDimension::State)),
            assumptions: vec![
                "候选由 Builder 从本轮用户回答中提取；尚未经用户确认，也尚未写入 LifeModel。"
                    .to_string(),
            ],
            unresolved_questions: vec![],
            recommended_next_steps: vec![
                "审阅并确认候选".to_string(),
                "需要更多上下文时返回 Builder 继续完善".to_string(),
            ],
        },
        finished: session.finished,
    }
}

async fn builder_start_with_state(
    mode: String,
    session_id: String,
    state: &Arc<AppState>,
    target_dimension: Option<String>,
) -> Result<serde_json::Value, AppError> {
    validate_builder_session_id(&session_id)?;
    if mode.len() > 32 || mode.chars().any(char::is_control) {
        return Err(AppError::serialization("Builder mode is invalid."));
    }
    if target_dimension
        .as_ref()
        .is_some_and(|dimension| dimension.len() > 32 || dimension.chars().any(char::is_control))
    {
        return Err(AppError::serialization(
            "Builder target dimension is invalid.",
        ));
    }
    state
        .persistence_coordinator
        .require_effects_allowed()
        .map_err(|error| AppError::db_with_hint(error.to_string(), "read_only_degraded"))?;
    let requested_mode = match mode.as_str() {
        "quick" => BuilderMode::Quick,
        "incremental" => BuilderMode::Incremental,
        "socratic" => BuilderMode::Socratic,
        _ => {
            return Err(AppError::serialization(
                "Unsupported Builder mode; no session was created.",
            ))
        }
    };
    let model = {
        let manager = state.life_model_manager.lock().await;
        manager.load().map_err(AppError::from)?
    };

    let existing = {
        let store = state.builder_session_store.lock().await;
        store.resume_session(&session_id).map_err(AppError::from)?
    };
    let session = if let Some(existing) = existing {
        existing
    } else {
        let mut candidate = BuilderSession::new(&session_id, requested_mode);
        if let Some(dimension) = target_dimension {
            candidate.target_dimension = Some(
                dimension
                    .parse::<openlife_core::builder::BuilderDimension>()
                    .map_err(AppError::from)?,
            );
        }
        let engine = BuilderEngine::new();
        let _ = engine.next_prompt(&mut candidate, "", &model).await;
        let store = state.builder_session_store.lock().await;
        store
            .create_session_if_absent(&candidate)
            .map_err(AppError::from)?
    };

    if session.review_claim_id.is_some() {
        return Err(AppError::permission(
            "Builder review staging is already in progress for this session.",
        ));
    }
    if session.finished && !session.pending_signals.is_empty() {
        let analysis = session
            .analysis
            .clone()
            .unwrap_or_else(|| BuilderEngine::build_analysis(&model));
        let review = pending_signals_view(&session);
        return Ok(serde_json::json!({
            "prompt": session.current_prompt,
            "progress": session.progress(),
            "analysis": analysis,
            "finished": true,
            "waiting_for_review": true,
            "durable_lifemodel_write": false,
            "review": review,
            "mode": format!("{:?}", session.mode),
            "target_dimension": session.target_dimension.as_ref().map(|d| format!("{:?}", d)),
        }));
    }
    let analysis = session
        .analysis
        .clone()
        .unwrap_or_else(|| BuilderEngine::build_analysis(&model));
    Ok(serde_json::json!({
        "prompt": session.current_prompt,
        "progress": session.progress(),
        "analysis": analysis,
        "finished": false,
        "waiting_for_review": false,
        "durable_lifemodel_write": false,
        "mode": format!("{:?}", session.mode),
        "target_dimension": session.target_dimension.as_ref().map(|d| format!("{:?}", d)),
    }))
}

#[tauri::command]
pub async fn builder_start(
    mode: String,
    session_id: String,
    state: State<'_, Arc<AppState>>,
    target_dimension: Option<String>,
) -> Result<serde_json::Value, AppError> {
    builder_start_with_state(mode, session_id, state.inner(), target_dimension).await
}

#[tauri::command]
pub async fn builder_step(
    session_id: String,
    user_reply: String,
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, AppError> {
    builder_step_with_state(session_id, user_reply, state.inner()).await
}

async fn builder_step_with_state(
    session_id: String,
    user_reply: String,
    state: &Arc<AppState>,
) -> Result<serde_json::Value, AppError> {
    validate_builder_session_id(&session_id)?;
    validate_builder_user_reply(&user_reply)?;
    state
        .persistence_coordinator
        .require_effects_allowed()
        .map_err(|error| AppError::db_with_hint(error.to_string(), "read_only_degraded"))?;
    let mut session = {
        let store = state.builder_session_store.lock().await;
        store
            .get_active_session(&session_id)
            .map_err(AppError::from)?
            .ok_or_else(|| {
                AppError::not_found(
                    "Session not found or expired; resume it with builder_start before editing.",
                )
            })?
    };
    if session.review_claim_id.is_some() {
        return Err(AppError::permission(
            "Builder review staging is already in progress for this session.",
        ));
    }
    if session.finished && !session.pending_signals.is_empty() {
        return Err(AppError::permission(
            "Builder candidates are waiting for review; answer submission is closed for this session.",
        ));
    }
    let expected_revision = session.revision;
    let model = {
        let manager = state.life_model_manager.lock().await;
        manager.load().map_err(AppError::from)?
    };
    let engine = BuilderEngine::new();
    let (prompt, _preview_model) = engine.next_prompt(&mut session, &user_reply, &model).await;
    let finished = session.finished;
    let progress = session.progress();
    let analysis = session
        .analysis
        .clone()
        .unwrap_or_else(|| BuilderEngine::build_analysis(&model));

    if finished && session.pending_signals.is_empty() {
        session.finished = false;
        session.current_prompt =
            "没有形成可审阅的候选，构建会话已保留；请补充更具体的回答后重试。".into();
        let saved = {
            let store = state.builder_session_store.lock().await;
            store
                .save_session_if_revision(&session, expected_revision)
                .map_err(AppError::from)?
        };
        if saved.is_none() {
            return Err(AppError::permission(
                "Builder session changed concurrently; this answer was not committed.",
            ));
        }
        return Err(AppError::internal(
            "Builder produced no reviewable candidates; the session was retained without claiming completion.",
        ));
    }

    let waiting_for_review = finished;
    let session = {
        let store = state.builder_session_store.lock().await;
        store
            .save_session_if_revision(&session, expected_revision)
            .map_err(AppError::from)?
            .ok_or_else(|| {
                AppError::permission(
                    "Builder session changed concurrently; this answer was not committed.",
                )
            })?
    };

    let review = finished.then(|| pending_signals_view(&session));
    let result = serde_json::json!({
        "prompt": prompt,
        "finished": finished,
        "waiting_for_review": waiting_for_review,
        "durable_lifemodel_write": false,
        "progress": progress,
        "analysis": analysis,
        "review": review,
        "mode": format!("{:?}", session.mode),
        "target_dimension": session.target_dimension.as_ref().map(|d| format!("{:?}", d)),
    });
    Ok(result)
}

#[tauri::command]
pub async fn builder_list_unfinished(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<BuilderSessionSummary>, AppError> {
    state
        .persistence_coordinator
        .require_trusted_read("BuilderSessionStore")
        .map_err(|error| AppError::db_with_hint(error.to_string(), "canonical_state_unknown"))?;
    let store = state.builder_session_store.lock().await;
    Ok(store
        .list_unfinished_sessions()
        .map_err(AppError::from)?
        .into_iter()
        .map(BuilderSessionSummary::from)
        .collect())
}

#[tauri::command]
pub async fn builder_delete_session(
    session_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<(), AppError> {
    validate_builder_session_id(&session_id)?;
    state
        .persistence_coordinator
        .require_effects_allowed()
        .map_err(|error| AppError::db_with_hint(error.to_string(), "read_only_degraded"))?;
    let store = state.builder_session_store.lock().await;
    if store
        .remove_session_if_unclaimed(&session_id)
        .map_err(AppError::from)?
    {
        Ok(())
    } else {
        Err(AppError::permission(
            "Builder review staging is in progress; the canonical session was not deleted.",
        ))
    }
}

/// Get pending signals for a Quick Build session
#[tauri::command]
pub async fn builder_get_pending_signals(
    session_id: String,
    state: State<'_, Arc<AppState>>,
) -> Result<BuilderPendingSignalsView, AppError> {
    validate_builder_session_id(&session_id)?;
    state
        .persistence_coordinator
        .require_trusted_read("BuilderSessionStore")
        .map_err(|error| AppError::db_with_hint(error.to_string(), "canonical_state_unknown"))?;
    let session = {
        let store = state.builder_session_store.lock().await;
        store
            .get_active_session(&session_id)
            .map_err(AppError::from)?
            .ok_or_else(|| {
                AppError::not_found(
                    "Session not found or expired; resume it with builder_start before review.",
                )
            })?
    };

    debug_assert_eq!(session.session_id, session_id);
    Ok(pending_signals_view(&session))
}

async fn builder_create_proposals_with_state(
    session_id: String,
    decisions: Vec<openlife_core::builder::BuilderSignalDecision>,
    state: &Arc<AppState>,
) -> Result<serde_json::Value, AppError> {
    validate_builder_session_id(&session_id)?;
    validate_builder_decisions(&decisions)?;
    state
        .persistence_coordinator
        .require_effects_allowed()
        .map_err(|error| AppError::db_with_hint(error.to_string(), "read_only_degraded"))?;
    let session = {
        let store = state.builder_session_store.lock().await;
        store
            .get_active_session(&session_id)
            .map_err(AppError::from)?
            .ok_or_else(|| {
                AppError::not_found(
                    "Session not found or expired; resume it with builder_start before review.",
                )
            })?
    };
    if !session.finished || session.pending_signals.is_empty() {
        return Err(AppError::not_found(
            "当前构建会话没有待确认信号，无法创建 Proposal。",
        ));
    }
    let mut decision_map =
        std::collections::HashMap::<String, &openlife_core::builder::BuilderSignalDecision>::new();
    for decision in &decisions {
        if !matches!(decision.status.as_str(), "accepted" | "rejected" | "edited") {
            return Err(AppError::internal(
                "Unsupported Builder review decision status.",
            ));
        }
        if decision_map.insert(decision.id.clone(), decision).is_some() {
            return Err(AppError::internal(
                "Builder review decisions contain a duplicate signal id.",
            ));
        }
    }
    if decision_map.keys().any(|id| {
        !session
            .pending_signals
            .iter()
            .any(|signal| &signal.id == id)
    }) {
        return Err(AppError::internal(
            "Builder review contains a decision for an unknown signal id.",
        ));
    }

    let mut selected_signals = Vec::new();
    let mut rejected_count = 0usize;
    for signal in &session.pending_signals {
        let Some(decision) = decision_map.get(&signal.id) else {
            rejected_count += 1;
            continue;
        };
        if decision.status == "rejected" {
            rejected_count += 1;
            continue;
        }
        let mut selected = signal.clone();
        if decision.status == "edited" {
            selected.proposed_value = decision.proposed_value.clone().ok_or_else(|| {
                AppError::internal("Edited Builder signal is missing its proposed value.")
            })?;
            selected.user_status = openlife_core::builder::SignalUserStatus::Edited;
        } else {
            selected.user_status = openlife_core::builder::SignalUserStatus::Accepted;
        }
        selected_signals.push(normalize_legacy_builder_signal(selected)?);
    }

    if selected_signals.is_empty() {
        return Err(AppError::not_found(
            "没有被接受或编辑的信号可转为 Proposal。",
        ));
    }

    // Complete every fallible typed-contract and payload check before the
    // canonical Builder session is claimed. In particular, the LifeModel
    // batch has tighter aggregate bounds than an individual IPC value.
    let preparation: Result<_, AppError> = async {
        let model = {
            let manager = state.life_model_manager.lock().await;
            manager.load().map_err(AppError::from)?
        };
        let mut preview_model = model.clone();
        let (applied_fields, skipped_fields) =
            BuilderEngine::apply_signals_to_model(&mut preview_model, &selected_signals);
        if !skipped_fields.is_empty() || applied_fields.is_empty() {
            return Err(AppError::internal(
                "Builder review contains an invalid typed candidate; no Proposal was staged.",
            ));
        }
        let operations = selected_signals
            .iter()
            .map(
                |signal| openlife_core::life_model::patch::LifeModelPatchBatchOperationV1 {
                    candidate_id: signal.id.clone(),
                    path: signal.affected_path.clone(),
                    candidate: signal.proposed_value.clone(),
                },
            )
            .collect::<Vec<_>>();
        let batch = openlife_core::life_model::patch::LifeModelPatchBatchV1::new(operations)
            .map_err(AppError::internal)?;
        let batch_risk = if selected_signals
            .iter()
            .any(|signal| signal.risk_level == openlife_core::builder::RiskLevel::High)
        {
            ProposalRiskLevel::High
        } else if selected_signals
            .iter()
            .any(|signal| signal.risk_level == openlife_core::builder::RiskLevel::Medium)
        {
            ProposalRiskLevel::Medium
        } else {
            ProposalRiskLevel::Low
        };
        let confidence = selected_signals
            .iter()
            .map(|signal| signal.confidence)
            .fold(1.0_f32, f32::min);
        let agent_run = openlife_core::agent::AgentRun::new_builder_run(&session_id);
        let run_id = agent_run.id.clone();
        let mut proposal = AgentProposal::new(
            ProposalType::LifeModelUpdate,
            openlife_core::life_model::patch::LIFEMODEL_PATCH_BATCH_PATH,
            serde_json::to_value(&batch).map_err(AppError::from)?,
            &format!(
                "Builder review staged {} typed candidates for one atomic LifeModel update.",
                selected_signals.len()
            ),
            confidence,
            batch_risk,
            ProposalSource::BuilderReview,
        );
        proposal.run_id = Some(run_id.clone());
        proposal.source_detail = Some(format!(
            "builder_candidate_batch:operation_count={}",
            selected_signals.len()
        ));
        crate::life_model_write_gateway::stamp_lifemodel_proposal_base_hash_with_state(
            state,
            &mut proposal,
        )
        .await
        .map_err(AppError::from)?;
        state.agent_run_store.as_ref().ok_or_else(|| {
            AppError::db("AgentRun store is unavailable; Builder review cannot be traced.")
        })?;
        let proposal_store = state
            .proposal_store
            .clone()
            .ok_or_else(|| AppError::db("Proposal store is unavailable."))?;
        Ok((agent_run, run_id, proposal, proposal_store))
    }
    .await;
    let (agent_run, run_id, proposal, proposal_store) = preparation?;

    let claimed_session = {
        let store = state.builder_session_store.lock().await;
        store
            .claim_review_session(&session_id, session.revision)
            .map_err(AppError::from)?
            .ok_or_else(|| {
                AppError::permission(
                    "Builder review was already claimed or the session changed concurrently.",
                )
            })?
    };
    let claim_id = claimed_session
        .review_claim_id
        .clone()
        .ok_or_else(|| AppError::internal("Builder review claim was not persisted."))?;
    let create_run_result =
        crate::terminal_owner_write_gateway::create_agent_run(state, &agent_run)
            .await
            .map_err(|error| AppError::db_with_hint(error, "read_only_degraded"));
    if let Err(error) = create_run_result {
        let store = state.builder_session_store.lock().await;
        if let Err(release_error) = store.release_review_claim(&session_id, &claim_id) {
            return Err(AppError::db(format!(
                "Builder AgentRun create failed and its review claim release also failed: {release_error}"
            )));
        }
        return Err(error);
    }

    let mut proposal_ids = Vec::new();
    let mut created_count = 0usize;
    let mut reused_count = 0usize;
    let mut updated_count = 0usize;
    let submit_result: Result<(), AppError> = async {
        let store = proposal_store.lock().await;
        let outcome = ReviewWorkflow::new(&store)
            .submit(
                DurableWriteRequest::from_agent_proposal(
                    DurableWriteSource::Builder,
                    DurableWriteSubject::LifeModel,
                    proposal.clone(),
                    "Builder proposal is pending Review Center approval.",
                )
                .with_evidence_refs(vec![proposal.source_detail.clone().unwrap_or_default()]),
            )
            .map_err(AppError::from)?;
        match outcome.decision.kind {
            DurableWriteDecisionKind::CreatePendingProposal => created_count += 1,
            DurableWriteDecisionKind::ReusePendingProposal => reused_count += 1,
            DurableWriteDecisionKind::UpdatePendingProposal => updated_count += 1,
        }
        proposal_ids.push(outcome.proposal_id().to_string());
        Ok(())
    }
    .await;
    if let Err(error) = submit_result {
        let update_result = crate::terminal_owner_write_gateway::fail_agent_run_from_owned_phase(
            state,
            &run_id,
            crate::terminal_owner_write_gateway::AgentRunOwnedFailure::BuilderProposalSubmission,
        )
        .await;
        let release_result = state
            .builder_session_store
            .lock()
            .await
            .release_review_claim(&session_id, &claim_id);
        match (update_result, release_result) {
            (Err(update_error), Err(release_error)) => {
                return Err(AppError::db(format!(
                    "Builder proposal submission failed; AgentRun finalization failed ({update_error}); review claim release failed ({release_error})."
                )));
            }
            (Err(update_error), Ok(_)) => {
                return Err(AppError::db(format!(
                    "Builder proposal submission and AgentRun finalization both failed: {update_error}"
                )));
            }
            (Ok(_), Err(release_error)) => {
                return Err(AppError::db(format!(
                    "Builder proposal submission failed and its review claim could not be released: {release_error}"
                )));
            }
            (Ok(_), Ok(_)) => {}
        }
        return Err(error);
    }

    crate::terminal_owner_write_gateway::project_agent_run_from_proposal_staging(
        state,
        &run_id,
        &proposal_ids,
        crate::terminal_owner_write_gateway::AgentRunProposalStagingReceipt {
            kind: crate::terminal_owner_write_gateway::AgentRunProposalStagingKind::Builder,
            requested_count: 1,
            failed_count: 0,
        },
    )
    .await
    .map_err(|error| {
        AppError::db(format!(
            "Builder Proposals were committed, but AgentRun projection is degraded: {error}"
        ))
    })?;
    let removed = {
        let store = state.builder_session_store.lock().await;
        store.remove_claimed_session(&session_id, &claim_id)
    };
    match removed {
        Ok(true) => {}
        Ok(false) => {
            return Err(AppError::db(
                "Builder Proposals and AgentRun projection committed, but the review claim changed; reconciliation is required.",
            ));
        }
        Err(error) => {
            return Err(AppError::db(format!(
                "Builder Proposals and AgentRun projection committed, but review claim cleanup failed: {error}"
            )));
        }
    }

    Ok(serde_json::json!({
        "success": true,
        "created_count": created_count,
        "reused_count": reused_count,
        "updated_count": updated_count,
        "rejected_count": rejected_count,
        "proposal_ids": proposal_ids,
        "run_id": run_id,
        "warnings": [],
    }))
}

#[tauri::command]
pub async fn builder_create_proposals(
    session_id: String,
    decisions: Vec<openlife_core::builder::BuilderSignalDecision>,
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, AppError> {
    builder_create_proposals_with_state(session_id, decisions, state.inner()).await
}

#[tauri::command]
pub async fn get_model_4d_completion(
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, AppError> {
    let model = {
        let manager = state.life_model_manager.lock().await;
        manager.load().map_err(AppError::from)?
    };
    let completion = model.calculate_4d_completion();
    serde_json::to_value(completion).map_err(AppError::from)
}

#[tauri::command]
pub async fn goal_capability_gap_analysis(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<String>, AppError> {
    let model = {
        let manager = state.life_model_manager.lock().await;
        manager.load().map_err(AppError::from)?
    };
    Ok(model.goal_capability_gap_analysis())
}

#[tauri::command]
pub async fn goal_capability_gap_report(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<openlife_core::life_model::CapabilityGap>, AppError> {
    let model = {
        let manager = state.life_model_manager.lock().await;
        manager.load().map_err(AppError::from)?
    };
    Ok(model.goal_capability_gap_report())
}

#[tauri::command]
pub async fn identity_goal_alignment_check(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<String>, AppError> {
    let model = {
        let manager = state.life_model_manager.lock().await;
        manager.load().map_err(AppError::from)?
    };
    Ok(model.identity_goal_alignment_check())
}

#[tauri::command]
pub async fn identity_goal_alignment_report(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<openlife_core::life_model::AlignmentIssue>, AppError> {
    let model = {
        let manager = state.life_model_manager.lock().await;
        manager.load().map_err(AppError::from)?
    };
    Ok(model.identity_goal_alignment_report())
}

#[cfg(test)]
mod tests {
    use super::*;
    use openlife_core::builder::{
        BuilderSignal, BuilderSignalDecision, RiskLevel, SignalUserStatus,
    };
    use openlife_core::config::AppConfig;
    use openlife_core::feedback::FeedbackStore;
    use openlife_core::life_model::LifeModelManager;
    use openlife_core::mcp::McpRegistry;
    use openlife_core::mcp_audit::McpAuditStore;
    use openlife_core::memory::MemoryStore;
    use openlife_core::memory_cache::{HotMemoryCache, SharedHotCache};
    use openlife_core::privacy::PrivacyEngine;
    use openlife_core::scheduler::InferenceScheduler;
    use openlife_core::vectors::VectorStore;
    use openlife_core::versioning::VersionManager;

    fn test_app_state(temp_dir: &tempfile::TempDir) -> Arc<AppState> {
        let config = AppConfig::default();
        let life_model_manager =
            LifeModelManager::new(temp_dir.path().join("life-model").join("current"));
        let hot_cache: SharedHotCache =
            Arc::new(tokio::sync::RwLock::new(HotMemoryCache::default()));

        Arc::new(AppState {
            persistence_coordinator: Arc::new(
                crate::persistence_coordinator::PersistenceCoordinator::isolated_evaluation(),
            ),
            governed_data_import_journal: None,
            config: Arc::new(tokio::sync::Mutex::new(config.clone())),
            life_model_manager: Arc::new(tokio::sync::Mutex::new(life_model_manager)),
            life_model_write_coordinator: Arc::new(tokio::sync::Mutex::new(())),
            memory_store: Arc::new(tokio::sync::Mutex::new(
                MemoryStore::new_in_memory().unwrap(),
            )),
            mcp_registry: Arc::new(tokio::sync::Mutex::new(McpRegistry::new())),
            scheduler: Arc::new(tokio::sync::Mutex::new(InferenceScheduler::new(
                config.local_model.clone(),
                config.prefer_local_model,
                config.llm.provider.clone(),
                config.llm.openai_base.clone(),
                config.llm.openai_key.clone(),
                config.llm.chat_model.clone(),
                config.llm.embedding_model.clone(),
                config.llm.embedding_enabled,
            ))),
            privacy_engine: Arc::new(tokio::sync::Mutex::new(PrivacyEngine::new())),
            version_manager: Arc::new(tokio::sync::Mutex::new(VersionManager::new(
                temp_dir.path().join("life-model").join("versions"),
            ))),
            feedback_store: Arc::new(tokio::sync::Mutex::new(
                FeedbackStore::new_in_memory().unwrap(),
            )),
            vector_store: Arc::new(tokio::sync::Mutex::new(
                VectorStore::new_in_memory().unwrap(),
            )),
            vector_persistence_mode: crate::state::VectorPersistenceMode::Enabled,
            builder_session_store: Arc::new(tokio::sync::Mutex::new(
                openlife_core::builder::BuilderSessionStore::new(
                    temp_dir.path().join("builder_sessions.json"),
                ),
            )),
            a2a_sidecar: Arc::new(tokio::sync::Mutex::new(
                crate::a2a_sidecar::A2ASidecar::new(crate::a2a_server::configured_a2a_port()),
            )),
            last_snapshot_date: Arc::new(tokio::sync::Mutex::new(None)),
            mcp_audit_store: Arc::new(tokio::sync::Mutex::new(McpAuditStore::new(
                temp_dir.path().join("mcp_audit.db"),
            ))),
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
            startup_warnings: vec![],
            provider_health_cache: Arc::new(tokio::sync::Mutex::new(None)),
            scheduled_task_store: Arc::new(
                openlife_core::tasks::TaskStore::new_in_memory().unwrap(),
            ),
            runtime_clock_source: Arc::new(tokio::sync::Mutex::new(
                crate::main_chat_runtime_facts::MainChatRuntimeClockSource::default(),
            )),
            web_search_fixture_output: Arc::new(tokio::sync::Mutex::new(None)),
            resource_runtime: None,
            state_store: None,
            shutdown_notify: Arc::new(tokio::sync::Notify::new()),
        })
    }

    #[test]
    fn unfinished_builder_summary_does_not_expose_private_session_content() {
        let mut session = BuilderSession::new("summary-private", BuilderMode::Socratic);
        session.draft_yaml = "PRIVATE_BUILDER_ANSWER_SENTINEL".into();
        session
            .extracted_values
            .push(openlife_core::life_model::ValueItem {
                name: "private value".into(),
                weight: 0,
                description: "private description".into(),
            });
        session.current_prompt = "PRIVATE_ANSWER_DERIVED_PROMPT_SENTINEL".into();
        session.pending_signals.push(BuilderSignal {
            id: "private-candidate".into(),
            source_step: 1,
            source_question_id: "private-question".into(),
            dimension: BuilderDimension::Identity,
            affected_path: "identity.name".into(),
            proposed_value: serde_json::json!("PRIVATE_CANDIDATE_BODY_SENTINEL"),
            confidence: 0.9,
            reason: "private reason".into(),
            risk_level: RiskLevel::Low,
            user_status: SignalUserStatus::Pending,
        });
        session.finished = true;
        let json = serde_json::to_value(BuilderSessionSummary::from(session)).unwrap();

        assert!(json.get("draft_yaml").is_none());
        assert!(json.get("extracted_values").is_none());
        assert!(json.get("pending_signals").is_none());
        let encoded = json.to_string();
        for private_body in [
            "PRIVATE_BUILDER_ANSWER_SENTINEL",
            "PRIVATE_ANSWER_DERIVED_PROMPT_SENTINEL",
            "PRIVATE_CANDIDATE_BODY_SENTINEL",
            "private value",
            "private description",
            "private reason",
        ] {
            assert!(
                !encoded.contains(private_body),
                "unfinished summary leaked private Builder body: {private_body}"
            );
        }
        assert_eq!(json["pending_signal_count"], 1);
        assert_eq!(json["current_prompt"], "待确认的 Builder 候选已准备好");
    }

    #[test]
    fn builder_pending_signal_read_model_is_typed_and_bounded() {
        let signal = BuilderSignal {
            id: "sig-name".into(),
            source_step: 1,
            source_question_id: "name".into(),
            dimension: BuilderDimension::Identity,
            affected_path: "identity.name".into(),
            proposed_value: serde_json::json!("Alex"),
            confidence: 0.95,
            reason: "explicit answer".into(),
            risk_level: RiskLevel::Low,
            user_status: SignalUserStatus::Pending,
        };
        let encoded = serde_json::to_value(BuilderPendingSignalView::from(&signal)).unwrap();
        let keys = encoded
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            keys,
            std::collections::BTreeSet::from([
                "affected_path",
                "confidence",
                "dimension",
                "id",
                "proposed_value",
                "reason",
                "risk_level",
                "source_question_id",
                "source_step",
                "user_status",
            ])
        );
        assert_eq!(encoded["affected_path"], "identity.name");
        assert_eq!(encoded["proposed_value"], "Alex");
    }

    #[test]
    fn shipped_builder_command_surface_cannot_self_authorize_a_provider_request() {
        let product_source = include_str!("builder.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("Builder product source prefix");
        for forbidden in [
            concat!("Policy", "Allowed"),
            concat!("Privacy", "Decision"),
            concat!("PreparedProvider", "Request"),
            concat!("prepare_", "chat_request"),
            concat!("generate_", "prepared"),
        ] {
            assert!(
                !product_source.contains(forbidden),
                "shipped Builder command surface must not self-authorize provider work: {forbidden}"
            );
        }
    }

    #[tokio::test]
    async fn builder_start_restores_finished_review_session_from_store() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let mut session = BuilderSession::new("review-session", BuilderMode::Quick);
        session.finished = true;
        session.current_prompt =
            "快速构建问题已完成！接下来请审阅根据你回答生成的待确认建议。".into();
        session.pending_signals.push(BuilderSignal {
            id: "sig_name".into(),
            source_step: 1,
            source_question_id: "name".into(),
            dimension: BuilderDimension::Identity,
            affected_path: "identity.name".into(),
            proposed_value: serde_json::Value::String("fujing".into()),
            confidence: 0.95,
            reason: "用户直接提供的称呼".into(),
            risk_level: RiskLevel::Low,
            user_status: SignalUserStatus::Pending,
        });
        state
            .builder_session_store
            .lock()
            .await
            .save_session(&session)
            .unwrap();

        let res = builder_start_with_state("quick".into(), "review-session".into(), &state, None)
            .await
            .unwrap();

        assert_eq!(res.get("finished").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(
            res.get("review")
                .and_then(|review| review.get("signals"))
                .and_then(|v| v.as_array())
                .map(|v| v.len()),
            Some(1)
        );
        assert!(res.get("pending_signals").is_none());
        assert_eq!(
            res.get("prompt").and_then(|v| v.as_str()),
            Some("快速构建问题已完成！接下来请审阅根据你回答生成的待确认建议。")
        );
    }

    #[tokio::test]
    async fn builder_no_signal_completion_is_retained_and_fails_closed() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let mut session = BuilderSession::new("no-signal-session", BuilderMode::Quick);
        session.step_index = 7;
        state
            .builder_session_store
            .lock()
            .await
            .save_session(&session)
            .unwrap();

        let error = builder_step_with_state("no-signal-session".into(), "".into(), &state)
            .await
            .expect_err("empty candidate completion must not be reported as success");

        assert!(error.message().contains("no reviewable candidates"));

        let model = state.life_model_manager.lock().await.load().unwrap();
        assert!(model.is_effectively_empty());
        let persisted = state
            .builder_session_store
            .lock()
            .await
            .get_session("no-signal-session")
            .unwrap();
        let persisted = persisted.expect("failed completion must retain the session");
        assert!(!persisted.finished);
        assert!(persisted.pending_signals.is_empty());
    }

    #[tokio::test]
    async fn quick_builder_never_invokes_provider_and_keeps_review_session() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let captured_requests = crate::main_chat_acceptance_test_support::configure_live_provider_eval_state_with_captured_local_http_provider(
            &state,
            "{}",
        )
        .await;
        let mut session = BuilderSession::new("review-step-session", BuilderMode::Quick);
        session.step_index = 7;
        session.draft_yaml = [
            "# step 1\nfujing",
            "# step 2\n事业 / 学业",
            "# step 3\n- 把 OpenLife 跑通",
            "# step 4\n成为能持续创造产品的人",
            "# step 5\n- 编程\n- 产品设计",
            "# step 6\n- 精力不足",
            "# step 7\n苏格拉底追问型",
        ]
        .join("\n");

        state
            .builder_session_store
            .lock()
            .await
            .save_session(&session)
            .unwrap();

        let res = builder_step_with_state(
            "review-step-session".into(),
            "最终确认前的回答".into(),
            &state,
        )
        .await
        .unwrap();

        assert_eq!(res.get("finished").and_then(|v| v.as_bool()), Some(true));
        assert!(res
            .get("review")
            .and_then(|review| review.get("signals"))
            .and_then(|v| v.as_array())
            .is_some_and(|items| !items.is_empty()));
        assert!(res.get("pending_signals").is_none());
        assert!(
            captured_requests.lock().unwrap().is_empty(),
            "Quick Builder is deterministic form-to-candidate extraction and must not start a provider request"
        );

        let persisted = state
            .builder_session_store
            .lock()
            .await
            .get_session("review-step-session")
            .unwrap();
        assert!(persisted.is_some());

        let model = state.life_model_manager.lock().await.load().unwrap();
        assert!(model.is_effectively_empty());
    }

    #[tokio::test]
    async fn concurrent_builder_step_commits_exactly_one_revision() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let mut session = BuilderSession::new("concurrent-step", BuilderMode::Quick);
        session.step_index = 7;
        session.current_prompt = "【第 7 步/7：陪伴风格】".into();
        session.draft_yaml = [
            "# step 1\nAlex",
            "# step 2\n后端重构",
            "# step 3\n- 收口单系统",
            "# step 4\n持续创造产品",
            "# step 5\n- Rust",
            "# step 6\n精力不足",
        ]
        .join("\n");
        state
            .builder_session_store
            .lock()
            .await
            .save_session(&session)
            .unwrap();

        let (left, right) = tokio::join!(
            builder_step_with_state("concurrent-step".into(), "直接高效型".into(), &state),
            builder_step_with_state("concurrent-step".into(), "温和陪伴型".into(), &state)
        );
        assert_eq!(usize::from(left.is_ok()) + usize::from(right.is_ok()), 1);
        let stored = state
            .builder_session_store
            .lock()
            .await
            .get_session("concurrent-step")
            .unwrap()
            .unwrap();
        assert_eq!(stored.revision, 1);
        assert!(stored.finished);
        assert!(!stored.pending_signals.is_empty());
    }

    #[tokio::test]
    async fn builder_static_step_does_not_create_a_phantom_completed_agent_run() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);

        builder_start_with_state("quick".into(), "static-step".into(), &state, None)
            .await
            .unwrap();
        let response = builder_step_with_state("static-step".into(), "请叫我 Alex".into(), &state)
            .await
            .unwrap();

        assert_eq!(response["finished"], false);
        let runs = state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_runs_for_session("static-step", 10)
            .unwrap();
        assert!(
            runs.is_empty(),
            "a deterministic prompt transition is not a provider or agent execution"
        );
    }

    #[tokio::test]
    async fn stage6d_builder_step_restores_persisted_step7_session_into_review_path() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let mut session = BuilderSession::new("persisted-step7-session", BuilderMode::Quick);
        session.step_index = 7;
        session.finished = false;
        session.current_prompt = "【第 7 步/7：陪伴风格】".into();
        session.draft_yaml = [
            "# step 1\nAlex",
            "# step 2\n事业 / 学业",
            "# step 3\n- 把 OpenLife 跑通",
            "# step 4\n成为能持续创造产品的人",
            "# step 5\n- 编程\n- 产品设计",
            "# step 6\n精力不足和方向不清晰",
        ]
        .join("\n");
        state
            .builder_session_store
            .lock()
            .await
            .save_session(&session)
            .unwrap();

        let res = builder_step_with_state(
            "persisted-step7-session".into(),
            "直接高效型：少废话，直接给建议".into(),
            &state,
        )
        .await
        .unwrap();

        assert_eq!(res.get("finished").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(
            res.get("prompt").and_then(|v| v.as_str()),
            Some("快速构建问题已完成！接下来请审阅根据你回答生成的待确认建议。")
        );
        let pending_signals = res
            .get("review")
            .and_then(|review| review.get("signals"))
            .and_then(|v| v.as_array())
            .expect("step 7 should enter review with pending signals");
        assert!(pending_signals.iter().any(|signal| {
            signal
                .get("id")
                .and_then(|v| v.as_str())
                .is_some_and(|id| id == "sig_comm_style")
                && signal
                    .get("source_step")
                    .and_then(|v| v.as_u64())
                    .is_some_and(|step| step == 7)
        }));

        let persisted = state
            .builder_session_store
            .lock()
            .await
            .get_session("persisted-step7-session")
            .unwrap()
            .expect("finished review session should remain persisted for proposal creation");
        assert!(persisted.finished);
        let step_seven_answer = persisted
            .draft_yaml
            .lines()
            .filter_map(|line| line.strip_prefix("# builder-answer-json "))
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .find(|record| record["lane"] == "quick" && record["step"] == 7)
            .expect("step 7 answer remains bound to the displayed Quick question");
        assert_eq!(
            step_seven_answer["answer"],
            "直接高效型：少废话，直接给建议"
        );
        assert!(!persisted.pending_signals.is_empty());

        let model = state.life_model_manager.lock().await.load().unwrap();
        assert!(model.is_effectively_empty());
        let proposal_count = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_all_proposals(100, 0)
            .unwrap()
            .len();
        assert_eq!(
            proposal_count, 0,
            "step 7 completion must stop at review and not create or accept proposals automatically"
        );
    }

    #[tokio::test]
    async fn builder_create_proposals_moves_review_signals_to_proposal_store() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let mut session = BuilderSession::new("proposal-session", BuilderMode::Quick);
        session.finished = true;
        session.pending_signals.push(BuilderSignal {
            id: "sig_name".into(),
            source_step: 1,
            source_question_id: "name".into(),
            dimension: BuilderDimension::Identity,
            affected_path: "identity.name".into(),
            proposed_value: serde_json::Value::String("fujing".into()),
            confidence: 0.95,
            reason: "用户直接提供的称呼".into(),
            risk_level: RiskLevel::Low,
            user_status: SignalUserStatus::Pending,
        });
        state
            .builder_session_store
            .lock()
            .await
            .save_session(&session)
            .unwrap();

        let decisions = vec![BuilderSignalDecision {
            id: "sig_name".into(),
            status: "accepted".into(),
            proposed_value: None,
        }];
        let res = builder_create_proposals_with_state("proposal-session".into(), decisions, &state)
            .await
            .unwrap();

        assert_eq!(res.get("success").and_then(|v| v.as_bool()), Some(true));
        assert_eq!(res.get("created_count").and_then(|v| v.as_u64()), Some(1));
        let proposals = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_pending_proposals(10)
            .unwrap();
        assert_eq!(proposals.len(), 1);
        assert_eq!(proposals[0].proposal_type, ProposalType::LifeModelUpdate);
        assert_eq!(
            proposals[0].affected_path,
            openlife_core::life_model::patch::LIFEMODEL_PATCH_BATCH_PATH
        );
        let batch: openlife_core::life_model::patch::LifeModelPatchBatchV1 =
            serde_json::from_value(proposals[0].after.clone()).unwrap();
        assert_eq!(batch.operations.len(), 1);
        assert_eq!(batch.operations[0].path, "identity.name");
        assert_eq!(batch.operations[0].candidate, serde_json::json!("fujing"));

        let run_id = res["run_id"].as_str().expect("builder review run id");
        let run = state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_run(run_id)
            .unwrap()
            .expect("builder review run");
        assert_eq!(
            run.status,
            openlife_core::agent::AgentRunStatus::WaitingPermission
        );
        assert!(
            run.finished_at.is_none(),
            "staging a Proposal is waiting for review, not completed execution"
        );

        let model = state.life_model_manager.lock().await.load().unwrap();
        assert!(model.is_effectively_empty());
        let persisted = state
            .builder_session_store
            .lock()
            .await
            .get_session("proposal-session")
            .unwrap();
        assert!(persisted.is_none());
    }

    #[tokio::test]
    async fn builder_review_cannot_stage_a_statestore_owned_lifemodel_candidate() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let mut session = BuilderSession::new("state-owner-review", BuilderMode::Quick);
        session.finished = true;
        session.pending_signals.push(BuilderSignal {
            id: "sig_daily".into(),
            source_step: 3,
            source_question_id: "daily_task".into(),
            dimension: BuilderDimension::Goals,
            affected_path: "goals.daily".into(),
            proposed_value: serde_json::json!([{"name": "wrong owner", "done": false}]),
            confidence: 0.9,
            reason: "must route through StateGateway".into(),
            risk_level: RiskLevel::Low,
            user_status: SignalUserStatus::Pending,
        });
        state
            .builder_session_store
            .lock()
            .await
            .save_session(&session)
            .unwrap();

        let error = builder_create_proposals_with_state(
            "state-owner-review".into(),
            vec![BuilderSignalDecision {
                id: "sig_daily".into(),
                status: "accepted".into(),
                proposed_value: None,
            }],
            &state,
        )
        .await
        .expect_err("Builder must reject a second transient-state write owner");

        assert!(error
            .message()
            .contains("create the task through Main Chat"));
        assert!(state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_all_proposals(10, 0)
            .unwrap()
            .is_empty());
        let persisted = state
            .builder_session_store
            .lock()
            .await
            .get_active_session("state-owner-review")
            .unwrap()
            .expect("failed preparation must not consume the review session");
        assert!(persisted.review_claim_id.is_none());
    }

    #[tokio::test]
    async fn legacy_builder_alert_candidate_migrates_to_reviewed_open_question() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let mut session = BuilderSession::new("legacy-alert-review", BuilderMode::Quick);
        session.finished = true;
        session.pending_signals.push(BuilderSignal {
            id: "sig_alert".into(),
            source_step: 6,
            source_question_id: "current_blockers".into(),
            dimension: BuilderDimension::State,
            affected_path: "state.alerts".into(),
            proposed_value: serde_json::json!([{
                "dimension_name": "general",
                "severity": "info",
                "message": "当前卡点: 需要理清技术路线"
            }]),
            confidence: 0.8,
            reason: "legacy explicit blocker".into(),
            risk_level: RiskLevel::Medium,
            user_status: SignalUserStatus::Pending,
        });
        state
            .builder_session_store
            .lock()
            .await
            .save_session(&session)
            .unwrap();

        let result = builder_create_proposals_with_state(
            "legacy-alert-review".into(),
            vec![BuilderSignalDecision {
                id: "sig_alert".into(),
                status: "accepted".into(),
                proposed_value: None,
            }],
            &state,
        )
        .await
        .expect("legacy blocker text has a lossless canonical migration");

        let proposal_id = result["proposal_ids"][0].as_str().unwrap();
        let proposal = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_proposal(proposal_id)
            .unwrap()
            .unwrap();
        let batch: openlife_core::life_model::patch::LifeModelPatchBatchV1 =
            serde_json::from_value(proposal.after).unwrap();
        assert_eq!(batch.operations.len(), 1);
        assert_eq!(batch.operations[0].path, "state.open_questions");
        assert_eq!(
            batch.operations[0].candidate,
            serde_json::json!(["当前卡点: 需要理清技术路线"])
        );
    }

    #[tokio::test]
    async fn concurrent_builder_review_submission_stages_one_batch_once() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let mut session = BuilderSession::new("double-submit", BuilderMode::Quick);
        session.finished = true;
        session.pending_signals.push(BuilderSignal {
            id: "sig_name".into(),
            source_step: 1,
            source_question_id: "name".into(),
            dimension: BuilderDimension::Identity,
            affected_path: "identity.name".into(),
            proposed_value: serde_json::json!("Alex"),
            confidence: 0.95,
            reason: "explicit".into(),
            risk_level: RiskLevel::Low,
            user_status: SignalUserStatus::Pending,
        });
        state
            .builder_session_store
            .lock()
            .await
            .save_session(&session)
            .unwrap();
        let decisions = vec![BuilderSignalDecision {
            id: "sig_name".into(),
            status: "accepted".into(),
            proposed_value: None,
        }];

        let (left, right) = tokio::join!(
            builder_create_proposals_with_state("double-submit".into(), decisions.clone(), &state),
            builder_create_proposals_with_state("double-submit".into(), decisions, &state)
        );
        assert_eq!(usize::from(left.is_ok()) + usize::from(right.is_ok()), 1);
        let proposals = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_pending_proposals(10)
            .unwrap();
        assert_eq!(proposals.len(), 1);
    }

    #[tokio::test]
    async fn builder_review_stages_and_applies_one_atomic_typed_candidate_batch() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let mut session = BuilderSession::new("atomic-builder-review", BuilderMode::Quick);
        session.finished = true;
        session.pending_signals = vec![
            BuilderSignal {
                id: "sig_name".into(),
                source_step: 1,
                source_question_id: "name".into(),
                dimension: BuilderDimension::Identity,
                affected_path: "identity.name".into(),
                proposed_value: serde_json::json!("Alex"),
                confidence: 0.95,
                reason: "用户直接提供的称呼".into(),
                risk_level: RiskLevel::Low,
                user_status: SignalUserStatus::Pending,
            },
            BuilderSignal {
                id: "sig_focus".into(),
                source_step: 2,
                source_question_id: "current_focus".into(),
                dimension: BuilderDimension::State,
                affected_path: "state.current_focus".into(),
                proposed_value: serde_json::json!("完成后端单系统收口"),
                confidence: 0.9,
                reason: "用户直接提供的当前关注".into(),
                risk_level: RiskLevel::Low,
                user_status: SignalUserStatus::Pending,
            },
        ];
        state
            .builder_session_store
            .lock()
            .await
            .save_session(&session)
            .unwrap();

        let result = builder_create_proposals_with_state(
            "atomic-builder-review".into(),
            vec![
                BuilderSignalDecision {
                    id: "sig_name".into(),
                    status: "accepted".into(),
                    proposed_value: None,
                },
                BuilderSignalDecision {
                    id: "sig_focus".into(),
                    status: "accepted".into(),
                    proposed_value: None,
                },
            ],
            &state,
        )
        .await
        .unwrap();

        assert_eq!(result["created_count"], 1);
        let proposal_id = result["proposal_ids"][0]
            .as_str()
            .expect("atomic Builder proposal id");
        let proposal = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_proposal(proposal_id)
            .unwrap()
            .expect("atomic Builder proposal");
        assert_eq!(proposal.proposal_type, ProposalType::LifeModelUpdate);
        assert!(
            proposal.before.is_none(),
            "Builder batch must not duplicate canonical before content"
        );
        assert_eq!(
            proposal.affected_path,
            openlife_core::life_model::patch::LIFEMODEL_PATCH_BATCH_PATH
        );
        let payload = proposal
            .after
            .as_object()
            .expect("typed Builder batch payload");
        assert_eq!(
            payload
                .keys()
                .map(String::as_str)
                .collect::<std::collections::BTreeSet<_>>(),
            std::collections::BTreeSet::from(["operations", "schemaVersion"]),
            "Proposal payload may contain only schema identity and typed candidate operations"
        );
        for operation in payload["operations"].as_array().unwrap() {
            assert_eq!(
                operation
                    .as_object()
                    .unwrap()
                    .keys()
                    .map(String::as_str)
                    .collect::<std::collections::BTreeSet<_>>(),
                std::collections::BTreeSet::from(["candidate", "candidateId", "path"]),
                "Builder operation must not copy canonical collection before/after bodies"
            );
        }
        let batch: openlife_core::life_model::patch::LifeModelPatchBatchV1 =
            serde_json::from_value(proposal.after.clone()).unwrap();
        assert_eq!(batch.operations.len(), 2);
        assert!(batch
            .operations
            .iter()
            .any(|operation| operation.path == "identity.name" && operation.candidate == "Alex"));
        assert!(batch.operations.iter().any(|operation| {
            operation.path == "state.current_focus" && operation.candidate == "完成后端单系统收口"
        }));

        crate::commands::proposal::accept_proposal_with_state(proposal_id.into(), &state)
            .await
            .unwrap();
        let model = state.life_model_manager.lock().await.load().unwrap();
        assert_eq!(model.identity.name, "Alex");
        assert_eq!(model.state.current_focus, "完成后端单系统收口");
    }

    #[tokio::test]
    async fn phase4_builder_records_reused_review_workflow_outcome_id() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);

        let build_session = || {
            let mut session = BuilderSession::new("phase4-builder-reuse", BuilderMode::Quick);
            session.finished = true;
            session.pending_signals.push(BuilderSignal {
                id: "sig_name".into(),
                source_step: 1,
                source_question_id: "name".into(),
                dimension: BuilderDimension::Identity,
                affected_path: "identity.name".into(),
                proposed_value: serde_json::Value::String("fujing".into()),
                confidence: 0.95,
                reason: "用户直接提供的称呼".into(),
                risk_level: RiskLevel::Low,
                user_status: SignalUserStatus::Pending,
            });
            session
        };
        let decisions = || {
            vec![BuilderSignalDecision {
                id: "sig_name".into(),
                status: "accepted".into(),
                proposed_value: None,
            }]
        };

        state
            .builder_session_store
            .lock()
            .await
            .save_session(&build_session())
            .unwrap();
        let first =
            builder_create_proposals_with_state("phase4-builder-reuse".into(), decisions(), &state)
                .await
                .unwrap();
        let reused_id = first["proposal_ids"][0]
            .as_str()
            .expect("first proposal id")
            .to_string();

        state
            .builder_session_store
            .lock()
            .await
            .save_session(&build_session())
            .unwrap();
        let second =
            builder_create_proposals_with_state("phase4-builder-reuse".into(), decisions(), &state)
                .await
                .unwrap();
        assert_eq!(second["created_count"], 0);
        assert_eq!(second["reused_count"], 1);
        assert_eq!(second["updated_count"], 0);
        assert_eq!(second["proposal_ids"][0].as_str(), Some(reused_id.as_str()));
        let run_id = second["run_id"].as_str().expect("second run id");
        let stored_run = state
            .agent_run_store
            .as_ref()
            .expect("agent run store")
            .lock()
            .await
            .get_run(run_id)
            .unwrap()
            .expect("builder run exists");
        assert_eq!(
            stored_run.generated_proposals,
            vec![reused_id.clone()],
            "Builder AgentRun must record the authoritative reused proposal id"
        );

        let proposals = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_pending_proposals(10)
            .unwrap();
        assert_eq!(proposals.len(), 1);
        assert_eq!(proposals[0].id, reused_id);
    }

    #[tokio::test]
    async fn oversized_builder_answer_is_rejected_before_session_mutation() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let session = BuilderSession::new("bounded-answer-session", BuilderMode::Socratic);
        state
            .builder_session_store
            .lock()
            .await
            .save_session(&session)
            .unwrap();
        let before = state
            .builder_session_store
            .lock()
            .await
            .get_session("bounded-answer-session")
            .unwrap()
            .unwrap();

        let error = builder_step_with_state(
            "bounded-answer-session".into(),
            "x".repeat(BUILDER_USER_REPLY_MAX_BYTES + 1),
            &state,
        )
        .await
        .expect_err("oversized answer must fail before Builder execution");
        assert!(error.message().contains("bounded input limit"));
        let after = state
            .builder_session_store
            .lock()
            .await
            .get_session("bounded-answer-session")
            .unwrap()
            .unwrap();
        assert_eq!(after.revision, before.revision);
        assert_eq!(after.draft_yaml, before.draft_yaml);
    }

    #[tokio::test]
    async fn invalid_builder_mode_does_not_silently_substitute_quick() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);

        let error = builder_start_with_state(
            "unsupported-mode".into(),
            "invalid-mode-session".into(),
            &state,
            None,
        )
        .await
        .expect_err("invalid mode must fail instead of creating a Quick session");
        assert!(error.message().contains("Unsupported Builder mode"));
        assert!(state
            .builder_session_store
            .lock()
            .await
            .get_session("invalid-mode-session")
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn oversized_builder_review_fails_before_claim_or_session_mutation() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let mut session = BuilderSession::new("oversized-review-session", BuilderMode::Quick);
        session.finished = true;
        session.pending_signals = (0..=BUILDER_REVIEW_MAX_DECISIONS)
            .map(|index| BuilderSignal {
                id: format!("candidate-{index}"),
                source_step: 1,
                source_question_id: format!("question-{index}"),
                dimension: BuilderDimension::Identity,
                affected_path: format!("preferences.builder_candidate_{index}"),
                proposed_value: serde_json::json!(format!("value-{index}")),
                confidence: 0.9,
                reason: "bounded review test".into(),
                risk_level: RiskLevel::Low,
                user_status: SignalUserStatus::Pending,
            })
            .collect();
        state
            .builder_session_store
            .lock()
            .await
            .save_session(&session)
            .unwrap();
        let before = state
            .builder_session_store
            .lock()
            .await
            .get_session("oversized-review-session")
            .unwrap()
            .unwrap();
        let decisions = session
            .pending_signals
            .iter()
            .map(|signal| BuilderSignalDecision {
                id: signal.id.clone(),
                status: "accepted".into(),
                proposed_value: None,
            })
            .collect();

        let error = builder_create_proposals_with_state(
            "oversized-review-session".into(),
            decisions,
            &state,
        )
        .await
        .expect_err("65 accepted decisions must fail before the review claim");
        assert!(error.message().contains("bounded decision limit"));
        let after = state
            .builder_session_store
            .lock()
            .await
            .get_session("oversized-review-session")
            .unwrap()
            .unwrap();
        assert_eq!(after.revision, before.revision);
        assert_eq!(after.review_claim_id, before.review_claim_id);
        assert!(state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_pending_proposals(10)
            .unwrap()
            .is_empty());
        assert!(state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_runs(10, 0)
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn oversized_builder_candidate_fails_before_all_canonical_mutations() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let mut session = BuilderSession::new("oversized-value-session", BuilderMode::Quick);
        session.finished = true;
        session.pending_signals.push(BuilderSignal {
            id: "candidate-oversized".into(),
            source_step: 1,
            source_question_id: "oversized".into(),
            dimension: BuilderDimension::Identity,
            affected_path: "identity.name".into(),
            proposed_value: serde_json::json!("original"),
            confidence: 0.9,
            reason: "oversized value test".into(),
            risk_level: RiskLevel::Low,
            user_status: SignalUserStatus::Pending,
        });
        state
            .builder_session_store
            .lock()
            .await
            .save_session(&session)
            .unwrap();
        let before = state
            .builder_session_store
            .lock()
            .await
            .get_session("oversized-value-session")
            .unwrap()
            .unwrap();

        let error = builder_create_proposals_with_state(
            "oversized-value-session".into(),
            vec![BuilderSignalDecision {
                id: "candidate-oversized".into(),
                status: "edited".into(),
                proposed_value: Some(serde_json::json!("x".repeat(100 * 1024))),
            }],
            &state,
        )
        .await
        .expect_err("100 KiB candidate must fail at the command boundary");
        assert!(error.message().contains("invalid or oversized"));
        let after = state
            .builder_session_store
            .lock()
            .await
            .get_session("oversized-value-session")
            .unwrap()
            .unwrap();
        assert_eq!(after.revision, before.revision);
        assert_eq!(after.review_claim_id, before.review_claim_id);
        assert!(state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_pending_proposals(10)
            .unwrap()
            .is_empty());
        assert!(state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_runs(10, 0)
            .unwrap()
            .is_empty());
    }

    #[tokio::test]
    async fn final_patch_batch_size_is_validated_before_builder_claim() {
        let temp_dir = tempfile::tempdir().unwrap();
        let state = test_app_state(&temp_dir);
        let candidate_bytes = (1
            ..openlife_core::life_model::patch::MAX_LIFEMODEL_PATCH_BATCH_BYTES)
            .rev()
            .find(|size| {
                let decision = BuilderSignalDecision {
                    id: "candidate-large".into(),
                    status: "edited".into(),
                    proposed_value: Some(serde_json::json!("x".repeat(*size))),
                };
                if validate_builder_decisions(std::slice::from_ref(&decision)).is_err() {
                    return false;
                }
                openlife_core::life_model::patch::LifeModelPatchBatchV1::new(vec![
                    openlife_core::life_model::patch::LifeModelPatchBatchOperationV1 {
                        candidate_id: decision.id,
                        path: "identity.name".into(),
                        candidate: decision.proposed_value.unwrap(),
                    },
                ])
                .is_err()
            })
            .expect("there is payload overhead between IPC and final batch contracts");
        let mut session = BuilderSession::new("batch-bound-session", BuilderMode::Quick);
        session.finished = true;
        session.pending_signals.push(BuilderSignal {
            id: "candidate-large".into(),
            source_step: 1,
            source_question_id: "large".into(),
            dimension: BuilderDimension::Identity,
            affected_path: "identity.name".into(),
            proposed_value: serde_json::json!("original"),
            confidence: 0.9,
            reason: "batch bound test".into(),
            risk_level: RiskLevel::Low,
            user_status: SignalUserStatus::Pending,
        });
        state
            .builder_session_store
            .lock()
            .await
            .save_session(&session)
            .unwrap();
        let before = state
            .builder_session_store
            .lock()
            .await
            .get_session("batch-bound-session")
            .unwrap()
            .unwrap();

        let error = builder_create_proposals_with_state(
            "batch-bound-session".into(),
            vec![BuilderSignalDecision {
                id: "candidate-large".into(),
                status: "edited".into(),
                proposed_value: Some(serde_json::json!("x".repeat(candidate_bytes))),
            }],
            &state,
        )
        .await
        .expect_err("final typed batch overflow must fail before claiming the session");
        assert!(error
            .message()
            .contains("lifemodel_patch_batch_payload_too_large"));
        let after = state
            .builder_session_store
            .lock()
            .await
            .get_session("batch-bound-session")
            .unwrap()
            .unwrap();
        assert_eq!(after.revision, before.revision);
        assert_eq!(after.review_claim_id, before.review_claim_id);
        assert!(state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_pending_proposals(10)
            .unwrap()
            .is_empty());
        assert!(state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_runs(10, 0)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn builder_input_bounds_preserve_normal_long_answers_and_reject_review_amplification() {
        validate_builder_user_reply(&"x".repeat(BUILDER_USER_REPLY_MAX_BYTES)).unwrap();
        let oversized_value = BuilderSignalDecision {
            id: "oversized-value".into(),
            status: "edited".into(),
            proposed_value: Some(serde_json::json!("x".repeat(100 * 1024))),
        };
        let error = validate_builder_decisions(&[oversized_value])
            .expect_err("a 100 KiB candidate must fail before session lookup or mutation");
        assert!(error.message().contains("invalid or oversized"));
        let decisions = (0..=BUILDER_REVIEW_MAX_DECISIONS)
            .map(|index| BuilderSignalDecision {
                id: format!("candidate-{index}"),
                status: "accepted".into(),
                proposed_value: None,
            })
            .collect::<Vec<_>>();
        let error = validate_builder_decisions(&decisions)
            .expect_err("unbounded decision batches must fail before review claim");
        assert!(error.message().contains("bounded decision limit"));
    }
}
