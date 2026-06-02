use super::*;

#[tauri::command]
pub async fn get_default_chat_adapter_ordinary_entry_preflight_status(
) -> Result<DefaultChatAdapterOrdinaryEntryPreflightStatus, String> {
    get_default_chat_adapter_ordinary_entry_preflight_status_with_route(
        crate::default_chat_adapter::resolve_default_chat_adapter_route(),
    )
    .await
}

pub(crate) async fn get_default_chat_adapter_ordinary_entry_preflight_status_with_route(
    route: crate::default_chat_adapter::DefaultChatAdapterRoute,
) -> Result<DefaultChatAdapterOrdinaryEntryPreflightStatus, String> {
    let send_message_preflight =
        crate::default_chat_adapter::evaluate_default_chat_adapter_ordinary_entry_preflight(
            crate::default_chat_adapter::DefaultChatAdapterCallsite::SendMessage,
            &route,
        );
    let stream_message_preflight =
        crate::default_chat_adapter::evaluate_default_chat_adapter_ordinary_entry_preflight(
            crate::default_chat_adapter::DefaultChatAdapterCallsite::StartStreamMessage,
            &route,
        );
    let default_chat_unchanged = route.current_mode == "legacy_stream"
        && route.default_send_path == "legacy_stream"
        && route.start_stream_path == "legacy_stream"
        && !route.controlled_adapter_enabled
        && !route.automatic_migration_enabled;

    let send_message_preflight =
        default_chat_adapter_ordinary_entry_preflight_check(send_message_preflight);
    let stream_message_preflight =
        default_chat_adapter_ordinary_entry_preflight_check(stream_message_preflight);
    let mut blocking_reasons = Vec::new();

    if !send_message_preflight.preflight_ready {
        push_unique_string(
            &mut blocking_reasons,
            "send_message_preflight_not_ready".into(),
        );
        for reason in &send_message_preflight.blocking_reasons {
            push_unique_string(&mut blocking_reasons, reason.clone());
        }
    }
    if !stream_message_preflight.preflight_ready {
        push_unique_string(
            &mut blocking_reasons,
            "start_stream_message_preflight_not_ready".into(),
        );
        for reason in &stream_message_preflight.blocking_reasons {
            push_unique_string(&mut blocking_reasons, reason.clone());
        }
    }
    if !default_chat_unchanged {
        push_unique_string(&mut blocking_reasons, "default_chat_route_drifted".into());
    }
    if route.controlled_adapter_enabled {
        push_unique_string(&mut blocking_reasons, "controlled_adapter_enabled".into());
    }
    if route.automatic_migration_enabled {
        push_unique_string(&mut blocking_reasons, "automatic_migration_enabled".into());
    }

    let status_ready = default_chat_unchanged
        && send_message_preflight.preflight_ready
        && stream_message_preflight.preflight_ready
        && blocking_reasons.is_empty();
    let blocking_reason_count = blocking_reasons.len();
    let send_preflight_ready = send_message_preflight.preflight_ready;
    let stream_preflight_ready = stream_message_preflight.preflight_ready;
    let send_side_effect_lock_engaged = send_message_preflight.side_effect_lock_engaged;
    let stream_side_effect_lock_engaged = stream_message_preflight.side_effect_lock_engaged;

    Ok(DefaultChatAdapterOrdinaryEntryPreflightStatus {
        status_ready,
        default_chat_unchanged,
        current_mode: route.current_mode.clone(),
        controlled_adapter_enabled: route.controlled_adapter_enabled,
        automatic_migration_enabled: route.automatic_migration_enabled,
        default_send_path: route.default_send_path.clone(),
        start_stream_path: route.start_stream_path.clone(),
        send_message_preflight,
        stream_message_preflight,
        blocking_reasons,
        metadata_safe_summary: json!({
            "ordinaryEntryPreflight": "default_chat_adapter",
            "metadataSafe": true,
            "readOnly": true,
            "statusReady": status_ready,
            "defaultChatUnchanged": default_chat_unchanged,
            "currentMode": route.current_mode,
            "controlledAdapterEnabled": route.controlled_adapter_enabled,
            "automaticMigrationEnabled": route.automatic_migration_enabled,
            "defaultSendPath": route.default_send_path,
            "startStreamPath": route.start_stream_path,
            "sendPreflightReady": send_preflight_ready,
            "streamPreflightReady": stream_preflight_ready,
            "sendSideEffectLockEngaged": send_side_effect_lock_engaged,
            "streamSideEffectLockEngaged": stream_side_effect_lock_engaged,
            "notAutomaticMigration": true,
            "blockingReasonCount": blocking_reason_count,
            "contentStorage": "none",
            "toolStorage": "none",
            "chatHistoryStorage": "none",
            "proposalStorage": "none",
            "lifeModelPatchStorage": "none",
            "memoryStorage": "none",
            "evidenceStorage": "none",
            "mcpAuditStorage": "none",
            "transcriptStorage": "none",
            "agentRunStorage": "none",
            "modelCallStorage": "none",
        }),
    })
}

fn default_chat_adapter_ordinary_entry_preflight_check(
    preflight: crate::default_chat_adapter::DefaultChatAdapterOrdinaryEntryPreflight,
) -> DefaultChatAdapterOrdinaryEntryPreflightCheck {
    DefaultChatAdapterOrdinaryEntryPreflightCheck {
        callsite: preflight.callsite,
        preflight_ready: preflight.preflight_ready,
        contract_ready: preflight.contract_ready,
        legacy_entry_allowed: preflight.legacy_entry_allowed,
        ordinary_entry_path: preflight.ordinary_entry_path,
        required_entry_path: preflight.required_entry_path,
        contract_shape: preflight.contract_shape,
        side_effect_lock_engaged: preflight.side_effect_lock_engaged,
        default_chat_migration_allowed: preflight.default_chat_migration_allowed,
        controlled_adapter_executor_attached: preflight.controlled_adapter_executor_attached,
        runtime_call_enabled: preflight.runtime_call_enabled,
        model_call_enabled: preflight.model_call_enabled,
        tool_call_enabled: preflight.tool_call_enabled,
        allow_writes: preflight.allow_writes,
        max_tool_calls: preflight.max_tool_calls,
        chat_message_saved: preflight.chat_message_saved,
        agent_run_recorded: preflight.agent_run_recorded,
        evidence_recorded: preflight.evidence_recorded,
        blocking_reasons: preflight.blocking_reasons,
    }
}

#[tauri::command]
pub async fn check_default_chat_adapter_narrow_implementation_discussion_gate(
    input: DefaultChatAdapterNarrowImplementationDiscussionGateInput,
    state: State<'_, Arc<AppState>>,
) -> Result<DefaultChatAdapterNarrowImplementationDiscussionGateReport, String> {
    check_default_chat_adapter_narrow_implementation_discussion_gate_with_state(
        input,
        &state.inner().clone(),
    )
    .await
}

pub(crate) async fn check_default_chat_adapter_narrow_implementation_discussion_gate_with_state(
    input: DefaultChatAdapterNarrowImplementationDiscussionGateInput,
    state: &Arc<AppState>,
) -> Result<DefaultChatAdapterNarrowImplementationDiscussionGateReport, String> {
    check_default_chat_adapter_narrow_implementation_discussion_gate_with_state_and_route(
        input,
        state,
        crate::default_chat_adapter::resolve_default_chat_adapter_route(),
    )
    .await
}

pub(crate) async fn check_default_chat_adapter_narrow_implementation_discussion_gate_with_state_and_route(
    input: DefaultChatAdapterNarrowImplementationDiscussionGateInput,
    state: &Arc<AppState>,
    route: crate::default_chat_adapter::DefaultChatAdapterRoute,
) -> Result<DefaultChatAdapterNarrowImplementationDiscussionGateReport, String> {
    let cutover_plan_approval =
        check_default_chat_adapter_cutover_plan_approval_readiness_with_state(
            DefaultChatAdapterCutoverPlanApprovalReadinessInput {
                source_session_id: input.source_session_id,
                message: input.message,
                required_approved_previews: input.required_approved_previews,
                required_approved_candidates: input.required_approved_candidates,
                required_promotions: input.required_promotions,
            },
            state,
        )
        .await?;
    let ordinary_entry_preflight_status =
        get_default_chat_adapter_ordinary_entry_preflight_status_with_route(route).await?;

    let mut blocking_reasons = Vec::new();
    for reason in &cutover_plan_approval.blocking_reasons {
        push_unique_string(&mut blocking_reasons, reason.clone());
    }
    for reason in &ordinary_entry_preflight_status.blocking_reasons {
        push_unique_string(&mut blocking_reasons, reason.clone());
    }

    let cutover_plan_approval_ready = cutover_plan_approval.ready;
    let ordinary_entry_preflight_status_ready = ordinary_entry_preflight_status.status_ready;
    let send_preflight_ready = ordinary_entry_preflight_status
        .send_message_preflight
        .preflight_ready;
    let stream_preflight_ready = ordinary_entry_preflight_status
        .stream_message_preflight
        .preflight_ready;
    let default_chat_unchanged = cutover_plan_approval.default_chat_unchanged
        && ordinary_entry_preflight_status.default_chat_unchanged
        && ordinary_entry_preflight_status.current_mode == "legacy_stream"
        && ordinary_entry_preflight_status.default_send_path == "legacy_stream"
        && ordinary_entry_preflight_status.start_stream_path == "legacy_stream";
    let controlled_adapter_enabled = cutover_plan_approval.controlled_adapter_enabled
        || ordinary_entry_preflight_status.controlled_adapter_enabled;
    let automatic_migration_enabled = cutover_plan_approval.automatic_migration_enabled
        || ordinary_entry_preflight_status.automatic_migration_enabled;
    let default_send_path = ordinary_entry_preflight_status.default_send_path.clone();
    let start_stream_path = ordinary_entry_preflight_status.start_stream_path.clone();

    if !cutover_plan_approval_ready {
        push_unique_string(
            &mut blocking_reasons,
            "cutover_plan_approval_readiness_not_ready".into(),
        );
    }
    if !ordinary_entry_preflight_status_ready {
        push_unique_string(
            &mut blocking_reasons,
            "ordinary_entry_preflight_status_not_ready".into(),
        );
    }
    if !default_chat_unchanged {
        push_unique_string(&mut blocking_reasons, "default_chat_changed".into());
    }
    if controlled_adapter_enabled {
        push_unique_string(&mut blocking_reasons, "controlled_adapter_enabled".into());
    }
    if automatic_migration_enabled {
        push_unique_string(&mut blocking_reasons, "automatic_migration_enabled".into());
    }
    if default_send_path != "legacy_stream" {
        push_unique_string(
            &mut blocking_reasons,
            "default_send_path_not_legacy_stream".into(),
        );
    }
    if start_stream_path != "legacy_stream" {
        push_unique_string(
            &mut blocking_reasons,
            "start_stream_path_not_legacy_stream".into(),
        );
    }

    let blocking_reason_count = blocking_reasons.len();
    let eligible = cutover_plan_approval_ready
        && ordinary_entry_preflight_status_ready
        && send_preflight_ready
        && stream_preflight_ready
        && default_chat_unchanged
        && !controlled_adapter_enabled
        && !automatic_migration_enabled
        && default_send_path == "legacy_stream"
        && start_stream_path == "legacy_stream"
        && blocking_reasons.is_empty();
    let summary_default_send_path = default_send_path.clone();
    let summary_start_stream_path = start_stream_path.clone();

    Ok(DefaultChatAdapterNarrowImplementationDiscussionGateReport {
        eligible,
        default_chat_unchanged,
        cutover_plan_approval_ready,
        ordinary_entry_preflight_status_ready,
        send_preflight_ready,
        stream_preflight_ready,
        controlled_adapter_enabled,
        automatic_migration_enabled,
        default_send_path,
        start_stream_path,
        blocking_reasons,
        metadata_safe_summary: json!({
            "narrowImplementationDiscussionGate": "default_chat_adapter",
            "metadataSafe": true,
            "readOnly": true,
            "eligible": eligible,
            "defaultChatUnchanged": default_chat_unchanged,
            "cutoverPlanApprovalReady": cutover_plan_approval_ready,
            "ordinaryEntryPreflightStatusReady": ordinary_entry_preflight_status_ready,
            "sendPreflightReady": send_preflight_ready,
            "streamPreflightReady": stream_preflight_ready,
            "controlledAdapterEnabled": controlled_adapter_enabled,
            "automaticMigrationEnabled": automatic_migration_enabled,
            "defaultSendPath": summary_default_send_path,
            "startStreamPath": summary_start_stream_path,
            "notAutomaticMigration": true,
            "requiresSeparateImplementation": true,
            "blockingReasonCount": blocking_reason_count,
            "contentStorage": "none",
            "toolStorage": "none",
            "chatHistoryStorage": "none",
            "proposalStorage": "none",
            "lifeModelPatchStorage": "none",
            "memoryStorage": "none",
            "evidenceStorage": "read_only",
            "mcpAuditStorage": "none",
            "transcriptStorage": "none",
            "agentRunStorage": "none",
            "runtimeCallStorage": "none",
            "modelCallStorage": "none",
            "externalWriteStorage": "none",
        }),
    })
}

#[tauri::command]
pub async fn draft_default_chat_adapter_narrow_implementation_plan(
    input: DefaultChatAdapterNarrowImplementationPlanInput,
    state: State<'_, Arc<AppState>>,
) -> Result<DefaultChatAdapterNarrowImplementationPlanDraft, String> {
    draft_default_chat_adapter_narrow_implementation_plan_with_state(input, &state.inner().clone())
        .await
}

pub(crate) async fn draft_default_chat_adapter_narrow_implementation_plan_with_state(
    input: DefaultChatAdapterNarrowImplementationPlanInput,
    state: &Arc<AppState>,
) -> Result<DefaultChatAdapterNarrowImplementationPlanDraft, String> {
    let source_session_id = safe_internal_id(&input.source_session_id, "sourceSessionId")?;
    let input_message_length = input.message.chars().count();
    let input_message_hash = sha256_metadata_checksum(&input.message);
    let discussion_gate =
        check_default_chat_adapter_narrow_implementation_discussion_gate_with_state(
            DefaultChatAdapterNarrowImplementationDiscussionGateInput {
                source_session_id: source_session_id.clone(),
                message: input.message,
                required_approved_previews: input.required_approved_previews,
                required_approved_candidates: input.required_approved_candidates,
                required_promotions: input.required_promotions,
            },
            state,
        )
        .await?;

    default_chat_adapter_narrow_implementation_plan_from_gate(
        source_session_id,
        input_message_length,
        input_message_hash,
        discussion_gate,
    )
}

fn default_chat_adapter_narrow_implementation_plan_from_gate(
    source_session_id: String,
    input_message_length: usize,
    input_message_hash: String,
    discussion_gate: DefaultChatAdapterNarrowImplementationDiscussionGateReport,
) -> Result<DefaultChatAdapterNarrowImplementationPlanDraft, String> {
    let mut blocking_reasons = Vec::new();
    if !discussion_gate.eligible {
        push_unique_string(
            &mut blocking_reasons,
            "narrow_implementation_discussion_gate_not_ready".into(),
        );
    }
    for reason in &discussion_gate.blocking_reasons {
        push_unique_string(&mut blocking_reasons, reason.clone());
    }
    if !discussion_gate.default_chat_unchanged {
        push_unique_string(&mut blocking_reasons, "default_chat_changed".into());
    }
    if discussion_gate.controlled_adapter_enabled {
        push_unique_string(&mut blocking_reasons, "controlled_adapter_enabled".into());
    }
    if discussion_gate.automatic_migration_enabled {
        push_unique_string(&mut blocking_reasons, "automatic_migration_enabled".into());
    }
    if discussion_gate.default_send_path != "legacy_stream" {
        push_unique_string(
            &mut blocking_reasons,
            "default_send_path_not_legacy_stream".into(),
        );
    }
    if discussion_gate.start_stream_path != "legacy_stream" {
        push_unique_string(
            &mut blocking_reasons,
            "start_stream_path_not_legacy_stream".into(),
        );
    }

    let draft_ready = discussion_gate.eligible
        && discussion_gate.default_chat_unchanged
        && !discussion_gate.controlled_adapter_enabled
        && !discussion_gate.automatic_migration_enabled
        && discussion_gate.default_send_path == "legacy_stream"
        && discussion_gate.start_stream_path == "legacy_stream"
        && blocking_reasons.is_empty();
    let plan_sections = if draft_ready {
        default_chat_adapter_narrow_implementation_plan_sections()
    } else {
        Vec::new()
    };
    let stable_plan_digest = if draft_ready {
        Some(default_chat_adapter_narrow_implementation_plan_digest(
            &source_session_id,
            input_message_length,
            &input_message_hash,
            &discussion_gate,
            &plan_sections,
        )?)
    } else {
        None
    };
    let plan_section_count = plan_sections.len();
    let blocking_reason_count = blocking_reasons.len();
    let summary_discussion_gate_eligible = discussion_gate.eligible;
    let summary_default_chat_unchanged = discussion_gate.default_chat_unchanged;
    let summary_controlled_adapter_enabled = discussion_gate.controlled_adapter_enabled;
    let summary_automatic_migration_enabled = discussion_gate.automatic_migration_enabled;
    let summary_default_send_path = discussion_gate.default_send_path.clone();
    let summary_start_stream_path = discussion_gate.start_stream_path.clone();

    Ok(DefaultChatAdapterNarrowImplementationPlanDraft {
        draft_ready,
        discussion_gate,
        manual_review_required: true,
        not_automatic_migration: true,
        requires_separate_implementation: true,
        requires_separate_cutover_review: true,
        source_session_id,
        input_message_length,
        input_message_hash: input_message_hash.clone(),
        stable_plan_digest,
        plan_sections,
        blocking_reasons,
        metadata_safe_summary: json!({
            "narrowImplementationPlan": "default_chat_adapter",
            "metadataSafe": true,
            "readOnly": true,
            "humanReviewOnly": true,
            "draftReady": draft_ready,
            "manualReviewRequired": true,
            "notAutomaticMigration": true,
            "requiresSeparateImplementation": true,
            "requiresSeparateCutoverReview": true,
            "discussionGateEligible": summary_discussion_gate_eligible,
            "defaultChatUnchanged": summary_default_chat_unchanged,
            "controlledAdapterEnabled": summary_controlled_adapter_enabled,
            "automaticMigrationEnabled": summary_automatic_migration_enabled,
            "defaultSendPath": summary_default_send_path,
            "startStreamPath": summary_start_stream_path,
            "inputMessageLength": input_message_length,
            "inputMessageHash": input_message_hash,
            "planSectionCount": plan_section_count,
            "blockingReasonCount": blocking_reason_count,
            "contentStorage": "none",
            "toolStorage": "none",
            "chatHistoryStorage": "none",
            "proposalStorage": "none",
            "lifeModelPatchStorage": "none",
            "memoryStorage": "none",
            "evidenceStorage": "read_only",
            "mcpAuditStorage": "none",
            "agentRunStorage": "read_only",
            "runtimeCallStorage": "none",
            "modelCallStorage": "none",
            "externalWriteStorage": "none",
            "transcriptStorage": "none",
        }),
    })
}

fn default_chat_adapter_narrow_implementation_plan_sections(
) -> Vec<DefaultChatAdapterNarrowImplementationPlanSection> {
    vec![
        DefaultChatAdapterNarrowImplementationPlanSection {
            section_key: "implementationScope".into(),
            title: "Implementation Scope".into(),
            items: vec![
                "Draft only the narrow adapter implementation slice that can be reviewed after W57 eligibility remains valid.".into(),
                "Keep default Chat on legacy_stream while this plan is drafted.".into(),
                "Treat this command as planning material only; it does not attach an executor or switch routing.".into(),
            ],
        },
        DefaultChatAdapterNarrowImplementationPlanSection {
            section_key: "adapterCallsiteBoundary".into(),
            title: "Adapter Callsite Boundary".into(),
            items: vec![
                "Preserve send_message and start_stream_message contract shapes and current legacy route paths.".into(),
                "Require ordinary entries to keep using W55 ordinary-entry preflight before legacy entry.".into(),
                "Do not call W57 or W58 from ordinary Chat send or streaming paths.".into(),
            ],
        },
        DefaultChatAdapterNarrowImplementationPlanSection {
            section_key: "controlledExecutorBoundary".into(),
            title: "Controlled Executor Boundary".into(),
            items: vec![
                "Keep controlled adapter execution disabled and unattached during this planning stage.".into(),
                "Do not run controlled preview, MultiStrategy runtime, model calls, tool calls, proposal apply, or external writes.".into(),
            ],
        },
        DefaultChatAdapterNarrowImplementationPlanSection {
            section_key: "fallbackPlan".into(),
            title: "Fallback Plan".into(),
            items: vec![
                "Keep legacy_stream as the stable fallback for blocked gate checks, route drift, missing review evidence, or future adapter errors.".into(),
                "Surface metadata-safe blockers in Settings without changing Chat history.".into(),
            ],
        },
        DefaultChatAdapterNarrowImplementationPlanSection {
            section_key: "rollbackPlan".into(),
            title: "Rollback Plan".into(),
            items: vec![
                "A later implementation must be reversible to legacy_stream without rewriting Chat history or evidence.".into(),
                "Rollback must not patch LifeModel, Memory, Proposal, Evidence, MCP audit, or external tools.".into(),
            ],
        },
        DefaultChatAdapterNarrowImplementationPlanSection {
            section_key: "observabilityPlan".into(),
            title: "Observability Plan".into(),
            items: vec![
                "Track only metadata-safe readiness, route, blocker, fallback, and contract counters.".into(),
                "Expose stable plan digest and W57 gate status without transcript content.".into(),
            ],
        },
        DefaultChatAdapterNarrowImplementationPlanSection {
            section_key: "testPlan".into(),
            title: "Test Plan".into(),
            items: vec![
                "Verify W57 blocked returns draftReady=false, no plan sections, and propagated blockers.".into(),
                "Verify W57 eligible returns all plan sections, stable digest, and fixed human-review-only flags.".into(),
                "Verify side-effect counts remain unchanged for AgentRun, Evidence, Proposal, Memory, LifeModel patch, MCP audit, and Chat messages.".into(),
                "Verify serialized output contains no raw prompt, assistant output, tool payload, reviewer note, preview content, or private transcript.".into(),
                "Verify default Send, send_message, and start_stream_message do not call the W58 command.".into(),
            ],
        },
        DefaultChatAdapterNarrowImplementationPlanSection {
            section_key: "explicitNonGoals".into(),
            title: "Explicit Non Goals".into(),
            items: vec![
                "Do not migrate default Chat.".into(),
                "Do not enable controlled adapter or automatic migration.".into(),
                "Do not create Chat, AgentRun, Evidence, Proposal, Memory, LifeModel, MCP audit, or external write records.".into(),
                "Do not run controlled preview, runtime, tools, model calls, proposal apply, or external writes.".into(),
            ],
        },
    ]
}

fn default_chat_adapter_narrow_implementation_plan_digest(
    source_session_id: &str,
    input_message_length: usize,
    input_message_hash: &str,
    discussion_gate: &DefaultChatAdapterNarrowImplementationDiscussionGateReport,
    plan_sections: &[DefaultChatAdapterNarrowImplementationPlanSection],
) -> Result<String, String> {
    metadata_hash_for_serializable(&json!({
        "sourceSessionId": source_session_id,
        "inputMessageLength": input_message_length,
        "inputMessageHash": input_message_hash,
        "manualReviewRequired": true,
        "notAutomaticMigration": true,
        "requiresSeparateImplementation": true,
        "requiresSeparateCutoverReview": true,
        "discussionGate": {
            "eligible": discussion_gate.eligible,
            "defaultChatUnchanged": discussion_gate.default_chat_unchanged,
            "cutoverPlanApprovalReady": discussion_gate.cutover_plan_approval_ready,
            "ordinaryEntryPreflightStatusReady": discussion_gate
                .ordinary_entry_preflight_status_ready,
            "sendPreflightReady": discussion_gate.send_preflight_ready,
            "streamPreflightReady": discussion_gate.stream_preflight_ready,
            "controlledAdapterEnabled": discussion_gate.controlled_adapter_enabled,
            "automaticMigrationEnabled": discussion_gate.automatic_migration_enabled,
            "defaultSendPath": discussion_gate.default_send_path,
            "startStreamPath": discussion_gate.start_stream_path,
        },
        "planSections": plan_sections,
    }))
}

#[tauri::command]
pub async fn record_default_chat_adapter_narrow_implementation_plan_review_decision(
    input: DefaultChatAdapterNarrowImplementationPlanReviewDecisionInput,
    state: State<'_, Arc<AppState>>,
) -> Result<DefaultChatAdapterNarrowImplementationPlanReviewDecisionResult, String> {
    record_default_chat_adapter_narrow_implementation_plan_review_decision_with_state(
        input,
        &state.inner().clone(),
    )
    .await
}

pub(crate) async fn record_default_chat_adapter_narrow_implementation_plan_review_decision_with_state(
    input: DefaultChatAdapterNarrowImplementationPlanReviewDecisionInput,
    state: &Arc<AppState>,
) -> Result<DefaultChatAdapterNarrowImplementationPlanReviewDecisionResult, String> {
    let decision_kind = safe_enum_value(
        &input.decision_kind,
        "decisionKind",
        &["approve", "reject", "request_rework"],
    )?;
    let source_session_id = safe_internal_id(&input.source_session_id, "sourceSessionId")?;
    let draft = draft_default_chat_adapter_narrow_implementation_plan_with_state(
        DefaultChatAdapterNarrowImplementationPlanInput {
            source_session_id: source_session_id.clone(),
            message: input.message,
            required_approved_previews: input.required_approved_previews,
            required_approved_candidates: input.required_approved_candidates,
            required_promotions: input.required_promotions,
        },
        state,
    )
    .await?;
    let created_at = chrono::Utc::now().to_rfc3339();
    let mut blocking_reasons = draft.blocking_reasons.clone();

    if decision_kind == "approve" && !draft.draft_ready {
        push_unique_string(
            &mut blocking_reasons,
            "narrow_implementation_plan_not_ready".into(),
        );
        return Ok(
            DefaultChatAdapterNarrowImplementationPlanReviewDecisionResult {
                recorded: false,
                evidence_id: None,
                decision_kind,
                source_session_id,
                draft_ready: false,
                narrow_plan_digest: draft.stable_plan_digest,
                plan_section_count: draft.plan_sections.len(),
                created_at,
                blocking_reasons,
            },
        );
    }

    let reviewer_note_metadata =
        metadata_safe_reviewer_note_fields(input.optional_reviewer_note.as_deref());
    let mut evidence_draft = EvidenceDraft::new(
        EvidenceType::RuntimeBehavior,
        DEFAULT_CHAT_ADAPTER_NARROW_IMPLEMENTATION_PLAN_REVIEW_DECISION_EVIDENCE_PATH,
        1.0,
        RiskLevel::Low,
        EvidencePrivacyLevel::Internal,
    );
    evidence_draft.run_metadata = json!({
        "evidenceKind": "default_chat_adapter_narrow_implementation_plan_review_decision",
        "decisionKind": decision_kind.clone(),
        "sourceSessionId": source_session_id.clone(),
        "draftReady": draft.draft_ready,
        "w57Eligible": draft.discussion_gate.eligible,
        "narrowPlanDigest": draft.stable_plan_digest.clone(),
        "planSectionCount": draft.plan_sections.len(),
        "reviewerNoteChecksum": reviewer_note_metadata.checksum,
        "reviewerNoteLength": reviewer_note_metadata.length,
        "reviewerNoteCategory": reviewer_note_metadata.category,
        "createdAt": created_at.clone(),
    });

    let record = {
        let store = state.evidence_store.lock().await;
        store.create_evidence(evidence_draft).map_err(|e| {
            format!(
                "failed to record default Chat adapter narrow implementation plan review evidence: {e}"
            )
        })?
    };

    Ok(
        DefaultChatAdapterNarrowImplementationPlanReviewDecisionResult {
            recorded: true,
            evidence_id: Some(record.id),
            decision_kind,
            source_session_id,
            draft_ready: draft.draft_ready,
            narrow_plan_digest: draft.stable_plan_digest,
            plan_section_count: draft.plan_sections.len(),
            created_at,
            blocking_reasons,
        },
    )
}

#[tauri::command]
pub async fn get_default_chat_adapter_narrow_implementation_plan_review_summary(
    state: State<'_, Arc<AppState>>,
) -> Result<DefaultChatAdapterNarrowImplementationPlanReviewSummary, String> {
    get_default_chat_adapter_narrow_implementation_plan_review_summary_with_state(
        &state.inner().clone(),
    )
    .await
}

pub(crate) async fn get_default_chat_adapter_narrow_implementation_plan_review_summary_with_state(
    state: &Arc<AppState>,
) -> Result<DefaultChatAdapterNarrowImplementationPlanReviewSummary, String> {
    let records = default_chat_adapter_narrow_implementation_plan_review_records(state).await?;
    let approved_count = records
        .iter()
        .filter(|record| {
            default_chat_adapter_narrow_implementation_plan_review_decision_kind(record)
                == Some("approve")
        })
        .count();
    let rejected_count = records
        .iter()
        .filter(|record| {
            default_chat_adapter_narrow_implementation_plan_review_decision_kind(record)
                == Some("reject")
        })
        .count();
    let request_rework_count = records
        .iter()
        .filter(|record| {
            default_chat_adapter_narrow_implementation_plan_review_decision_kind(record)
                == Some("request_rework")
        })
        .count();
    let latest_decision = records
        .first()
        .and_then(default_chat_adapter_narrow_implementation_plan_review_latest_decision);
    let latest_approved_plan_digest = records
        .iter()
        .filter(|record| {
            default_chat_adapter_narrow_implementation_plan_review_decision_kind(record)
                == Some("approve")
        })
        .find_map(|record| {
            record
                .run_metadata
                .get("narrowPlanDigest")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
        });
    let latest_timestamp = latest_decision
        .as_ref()
        .map(|decision| decision.created_at.clone());
    let latest_decision_present = latest_decision.is_some();
    let blocking_reasons = if latest_decision_present {
        Vec::new()
    } else {
        vec!["narrow_implementation_plan_review_decision_missing".into()]
    };
    let blocking_reason_count = blocking_reasons.len();

    Ok(DefaultChatAdapterNarrowImplementationPlanReviewSummary {
        latest_decision,
        approved_count,
        rejected_count,
        request_rework_count,
        latest_approved_plan_digest,
        latest_timestamp,
        blocking_reasons,
        metadata_safe_summary: json!({
            "narrowImplementationPlanReview": "default_chat_adapter",
            "metadataSafe": true,
            "readOnly": true,
            "approvedCount": approved_count,
            "rejectedCount": rejected_count,
            "requestReworkCount": request_rework_count,
            "latestDecisionPresent": latest_decision_present,
            "blockingReasonCount": blocking_reason_count,
            "contentStorage": "none",
            "reviewerNoteStorage": "length_checksum_category_only",
            "toolStorage": "none",
            "chatHistoryStorage": "none",
            "proposalStorage": "none",
            "lifeModelPatchStorage": "none",
            "memoryStorage": "none",
            "evidenceStorage": "read_only",
            "mcpAuditStorage": "none",
            "agentRunStorage": "none",
            "runtimeCallStorage": "none",
            "modelCallStorage": "none",
            "externalWriteStorage": "none",
            "transcriptStorage": "none",
            "notAutomaticMigration": true,
        }),
    })
}

#[tauri::command]
pub async fn check_default_chat_adapter_narrow_implementation_plan_approval_readiness(
    input: DefaultChatAdapterNarrowImplementationPlanApprovalReadinessInput,
    state: State<'_, Arc<AppState>>,
) -> Result<DefaultChatAdapterNarrowImplementationPlanApprovalReadinessReport, String> {
    check_default_chat_adapter_narrow_implementation_plan_approval_readiness_with_state(
        input,
        &state.inner().clone(),
    )
    .await
}

pub(crate) async fn check_default_chat_adapter_narrow_implementation_plan_approval_readiness_with_state(
    input: DefaultChatAdapterNarrowImplementationPlanApprovalReadinessInput,
    state: &Arc<AppState>,
) -> Result<DefaultChatAdapterNarrowImplementationPlanApprovalReadinessReport, String> {
    let source_session_id = safe_internal_id(&input.source_session_id, "sourceSessionId")?;
    let draft = draft_default_chat_adapter_narrow_implementation_plan_with_state(
        DefaultChatAdapterNarrowImplementationPlanInput {
            source_session_id,
            message: input.message,
            required_approved_previews: input.required_approved_previews,
            required_approved_candidates: input.required_approved_candidates,
            required_promotions: input.required_promotions,
        },
        state,
    )
    .await?;
    let review_summary =
        get_default_chat_adapter_narrow_implementation_plan_review_summary_with_state(state)
            .await?;
    let latest_decision = review_summary.latest_decision.clone();
    let current_plan_digest = draft.stable_plan_digest.clone();
    let latest_approved_plan_digest = review_summary.latest_approved_plan_digest.clone();
    let mut blocking_reasons = Vec::new();

    for reason in &draft.blocking_reasons {
        push_unique_string(&mut blocking_reasons, reason.clone());
    }
    for reason in &review_summary.blocking_reasons {
        push_unique_string(&mut blocking_reasons, reason.clone());
    }

    let discussion_gate_eligible = draft.discussion_gate.eligible;
    let default_chat_unchanged = draft.discussion_gate.default_chat_unchanged;
    let controlled_adapter_enabled = draft.discussion_gate.controlled_adapter_enabled;
    let automatic_migration_enabled = draft.discussion_gate.automatic_migration_enabled;
    let default_send_path = draft.discussion_gate.default_send_path.clone();
    let start_stream_path = draft.discussion_gate.start_stream_path.clone();

    if !draft.draft_ready {
        push_unique_string(
            &mut blocking_reasons,
            "narrow_implementation_plan_not_ready".into(),
        );
    }
    if !discussion_gate_eligible {
        push_unique_string(
            &mut blocking_reasons,
            "narrow_implementation_discussion_gate_not_ready".into(),
        );
    }
    if !default_chat_unchanged {
        push_unique_string(&mut blocking_reasons, "default_chat_not_unchanged".into());
    }
    if controlled_adapter_enabled {
        push_unique_string(&mut blocking_reasons, "controlled_adapter_enabled".into());
    }
    if automatic_migration_enabled {
        push_unique_string(&mut blocking_reasons, "automatic_migration_enabled".into());
    }
    if default_send_path != "legacy_stream" {
        push_unique_string(
            &mut blocking_reasons,
            "default_send_path_not_legacy_stream".into(),
        );
    }
    if start_stream_path != "legacy_stream" {
        push_unique_string(
            &mut blocking_reasons,
            "start_stream_path_not_legacy_stream".into(),
        );
    }

    let mut narrow_plan_review_approved = false;
    let mut narrow_plan_digest_matched = false;
    match latest_decision.as_ref() {
        Some(decision) if decision.decision_kind == "approve" => {
            narrow_plan_review_approved = true;
            narrow_plan_digest_matched =
                current_plan_digest.is_some() && decision.narrow_plan_digest == current_plan_digest;
            if !decision.draft_ready {
                push_unique_string(
                    &mut blocking_reasons,
                    "approved_narrow_implementation_plan_draft_not_ready".into(),
                );
            }
            if !decision.w57_eligible {
                push_unique_string(
                    &mut blocking_reasons,
                    "approved_narrow_implementation_plan_w57_not_eligible".into(),
                );
            }
            if decision.plan_section_count == 0 {
                push_unique_string(
                    &mut blocking_reasons,
                    "approved_narrow_implementation_plan_sections_missing".into(),
                );
            }
            if !narrow_plan_digest_matched {
                push_unique_string(
                    &mut blocking_reasons,
                    "narrow_implementation_plan_digest_mismatch".into(),
                );
            }
        }
        Some(_) => {
            push_unique_string(
                &mut blocking_reasons,
                "latest_narrow_implementation_plan_review_not_approved".into(),
            );
        }
        None => {
            push_unique_string(
                &mut blocking_reasons,
                "narrow_implementation_plan_review_approval_missing".into(),
            );
        }
    }

    let latest_decision_kind = latest_decision
        .as_ref()
        .map(|decision| decision.decision_kind.clone())
        .unwrap_or_else(|| "none".into());
    let blocking_reason_count = blocking_reasons.len();
    let ready = draft.draft_ready
        && discussion_gate_eligible
        && narrow_plan_review_approved
        && narrow_plan_digest_matched
        && default_chat_unchanged
        && !controlled_adapter_enabled
        && !automatic_migration_enabled
        && default_send_path == "legacy_stream"
        && start_stream_path == "legacy_stream"
        && blocking_reasons.is_empty();

    Ok(
        DefaultChatAdapterNarrowImplementationPlanApprovalReadinessReport {
            ready,
            draft_ready: draft.draft_ready,
            discussion_gate_eligible,
            narrow_plan_review_approved,
            narrow_plan_digest_matched,
            current_plan_digest: current_plan_digest.clone(),
            latest_approved_plan_digest: latest_approved_plan_digest.clone(),
            latest_decision,
            default_chat_unchanged,
            controlled_adapter_enabled,
            automatic_migration_enabled,
            default_send_path: default_send_path.clone(),
            start_stream_path: start_stream_path.clone(),
            blocking_reasons,
            metadata_safe_summary: json!({
                "narrowImplementationPlanApprovalReadiness": "default_chat_adapter",
                "metadataSafe": true,
                "readOnly": true,
                "ready": ready,
                "draftReady": draft.draft_ready,
                "discussionGateEligible": discussion_gate_eligible,
                "narrowPlanReviewApproved": narrow_plan_review_approved,
                "narrowPlanDigestMatched": narrow_plan_digest_matched,
                "currentPlanDigestPresent": current_plan_digest.is_some(),
                "latestApprovedPlanDigestPresent": latest_approved_plan_digest.is_some(),
                "latestDecisionKind": latest_decision_kind,
                "defaultChatUnchanged": default_chat_unchanged,
                "controlledAdapterEnabled": controlled_adapter_enabled,
                "automaticMigrationEnabled": automatic_migration_enabled,
                "defaultSendPath": default_send_path,
                "startStreamPath": start_stream_path,
                "blockingReasonCount": blocking_reason_count,
                "contentStorage": "none",
                "reviewerNoteStorage": "length_checksum_category_only",
                "toolStorage": "none",
                "chatHistoryStorage": "none",
                "proposalStorage": "none",
                "lifeModelPatchStorage": "none",
                "memoryStorage": "none",
                "evidenceStorage": "read_only",
                "mcpAuditStorage": "none",
                "agentRunStorage": "none",
                "runtimeCallStorage": "none",
                "modelCallStorage": "none",
                "externalWriteStorage": "none",
                "transcriptStorage": "none",
                "notAutomaticMigration": true,
                "requiresSeparateImplementation": true,
            }),
        },
    )
}

pub(crate) async fn default_chat_adapter_narrow_implementation_plan_review_records(
    state: &Arc<AppState>,
) -> Result<Vec<openlife_core::agent::EvidenceRecord>, String> {
    let records = {
        let store = state.evidence_store.lock().await;
        store
            .query(EvidenceQuery {
                affected_path: Some(
                    DEFAULT_CHAT_ADAPTER_NARROW_IMPLEMENTATION_PLAN_REVIEW_DECISION_EVIDENCE_PATH
                        .into(),
                ),
                evidence_type: Some(EvidenceType::RuntimeBehavior),
                ..EvidenceQuery::default()
            })
            .map_err(|e| {
                format!(
                    "failed to read default Chat adapter narrow implementation plan review evidence: {e}"
                )
            })?
    };
    Ok(records
        .into_iter()
        .filter(
            default_chat_adapter_narrow_implementation_plan_review_decision_evidence_is_metadata_safe,
        )
        .collect())
}

fn default_chat_adapter_narrow_implementation_plan_review_decision_evidence_is_metadata_safe(
    record: &openlife_core::agent::EvidenceRecord,
) -> bool {
    if record.affected_path
        != DEFAULT_CHAT_ADAPTER_NARROW_IMPLEMENTATION_PLAN_REVIEW_DECISION_EVIDENCE_PATH
        || record.evidence_type != EvidenceType::RuntimeBehavior
        || record.summary.is_some()
        || !record.source_refs.is_empty()
        || !record.linked_agent_run_ids.is_empty()
        || !record.linked_proposal_ids.is_empty()
    {
        return false;
    }
    let Some(metadata) = record.run_metadata.as_object() else {
        return false;
    };
    let allowed = [
        "evidenceKind",
        "decisionKind",
        "sourceSessionId",
        "draftReady",
        "w57Eligible",
        "narrowPlanDigest",
        "planSectionCount",
        "reviewerNoteChecksum",
        "reviewerNoteLength",
        "reviewerNoteCategory",
        "createdAt",
    ];
    if metadata.len() != allowed.len()
        || !metadata.keys().all(|key| allowed.contains(&key.as_str()))
    {
        return false;
    }

    let digest_is_safe = match record.run_metadata.get("narrowPlanDigest") {
        Some(Value::Null) => true,
        Some(Value::String(value)) => safe_checksum_field(value, "narrowPlanDigest").is_ok(),
        _ => false,
    };

    record
        .run_metadata
        .get("evidenceKind")
        .and_then(Value::as_str)
        .is_some_and(|value| {
            value == "default_chat_adapter_narrow_implementation_plan_review_decision"
        })
        && metadata_string_is_safe(&record.run_metadata, "decisionKind", |value, field| {
            safe_enum_value(value, field, &["approve", "reject", "request_rework"])
        })
        && metadata_string_is_safe(&record.run_metadata, "sourceSessionId", safe_internal_id)
        && record
            .run_metadata
            .get("draftReady")
            .and_then(Value::as_bool)
            .is_some()
        && record
            .run_metadata
            .get("w57Eligible")
            .and_then(Value::as_bool)
            .is_some()
        && digest_is_safe
        && record
            .run_metadata
            .get("planSectionCount")
            .and_then(Value::as_u64)
            .is_some()
        && reviewer_note_flat_metadata_is_safe(&record.run_metadata)
        && record
            .run_metadata
            .get("createdAt")
            .and_then(Value::as_str)
            .is_some_and(|value| chrono::DateTime::parse_from_rfc3339(value).is_ok())
        && !contains_unsafe_promotion_metadata(&record.run_metadata)
}

fn default_chat_adapter_narrow_implementation_plan_review_decision_kind(
    record: &openlife_core::agent::EvidenceRecord,
) -> Option<&str> {
    record
        .run_metadata
        .get("decisionKind")
        .and_then(Value::as_str)
}

fn default_chat_adapter_narrow_implementation_plan_review_latest_decision(
    record: &openlife_core::agent::EvidenceRecord,
) -> Option<DefaultChatAdapterNarrowImplementationPlanReviewLatestDecision> {
    Some(
        DefaultChatAdapterNarrowImplementationPlanReviewLatestDecision {
            evidence_id: record.id.clone(),
            decision_kind: default_chat_adapter_narrow_implementation_plan_review_decision_kind(
                record,
            )?
            .to_string(),
            source_session_id: record
                .run_metadata
                .get("sourceSessionId")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)?,
            draft_ready: record
                .run_metadata
                .get("draftReady")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            narrow_plan_digest: record
                .run_metadata
                .get("narrowPlanDigest")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            plan_section_count: record
                .run_metadata
                .get("planSectionCount")
                .and_then(Value::as_u64)
                .unwrap_or_default() as usize,
            w57_eligible: record
                .run_metadata
                .get("w57Eligible")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            reviewer_note_checksum: record
                .run_metadata
                .get("reviewerNoteChecksum")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            reviewer_note_length: record
                .run_metadata
                .get("reviewerNoteLength")
                .and_then(Value::as_u64)
                .unwrap_or_default() as usize,
            reviewer_note_category: record
                .run_metadata
                .get("reviewerNoteCategory")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)?,
            created_at: record
                .run_metadata
                .get("createdAt")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| record.created_at.to_rfc3339()),
        },
    )
}
