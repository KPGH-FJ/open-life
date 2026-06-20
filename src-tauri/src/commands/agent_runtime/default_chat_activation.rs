use super::*;

#[tauri::command]
pub async fn get_default_chat_runtime_boundary_status(
    state: State<'_, Arc<AppState>>,
) -> Result<DefaultChatRuntimeBoundaryStatus, String> {
    get_default_chat_runtime_boundary_status_with_state(&state.inner().clone()).await
}

pub(crate) async fn get_default_chat_runtime_boundary_status_with_state(
    _state: &Arc<AppState>,
) -> Result<DefaultChatRuntimeBoundaryStatus, String> {
    Ok(DefaultChatRuntimeBoundaryStatus {
        current_mode: "legacy_stream".into(),
        controlled_candidate_available: false,
        default_chat_unchanged: true,
        candidate_promotion_readiness_required: true,
        automatic_migration_enabled: false,
        blocking_reasons: Vec::new(),
        metadata_safe_summary: json!({
            "runtimeBoundary": "default_chat",
            "metadataSafe": true,
            "readOnly": true,
            "currentMode": "legacy_stream",
            "controlledCandidateAvailable": false,
            "defaultChatUnchanged": true,
            "candidatePromotionReadinessRequired": true,
            "automaticMigrationEnabled": false,
            "contentStorage": "none",
            "toolStorage": "none",
            "chatHistoryStorage": "none",
            "proposalStorage": "none",
            "lifeModelPatchStorage": "none",
            "memoryStorage": "none",
            "evidenceStorage": "none",
            "mcpAuditStorage": "none",
        }),
    })
}

#[tauri::command]
pub async fn draft_default_chat_adapter_activation_plan(
    input: DefaultChatAdapterActivationPlanDraftInput,
    state: State<'_, Arc<AppState>>,
) -> Result<DefaultChatAdapterActivationPlanDraft, String> {
    draft_default_chat_adapter_activation_plan_with_state(input, &state.inner().clone()).await
}

pub(crate) async fn draft_default_chat_adapter_activation_plan_with_state(
    input: DefaultChatAdapterActivationPlanDraftInput,
    state: &Arc<AppState>,
) -> Result<DefaultChatAdapterActivationPlanDraft, String> {
    let candidate_promotion_readiness_report =
        check_controlled_chat_cutover_candidate_promotion_readiness_with_state(
            ControlledChatCutoverCandidatePromotionReadinessInput {
                required_approved_candidates: input.required_approved_candidates,
                required_promotions: input.required_promotions,
                session_id: input.session_id,
            },
            state,
        )
        .await?;
    let runtime_boundary_status =
        get_default_chat_runtime_boundary_status_with_state(state).await?;

    Ok(draft_default_chat_adapter_activation_plan_from_reports(
        candidate_promotion_readiness_report,
        runtime_boundary_status,
    ))
}

pub(crate) fn draft_default_chat_adapter_activation_plan_from_reports(
    candidate_promotion_readiness_report: ControlledChatCutoverCandidatePromotionReadinessReport,
    runtime_boundary_status: DefaultChatRuntimeBoundaryStatus,
) -> DefaultChatAdapterActivationPlanDraft {
    let mut blocking_reasons = Vec::new();
    if !candidate_promotion_readiness_report.ready {
        push_unique_string(
            &mut blocking_reasons,
            "candidate_promotion_readiness_not_ready".into(),
        );
        for reason in &candidate_promotion_readiness_report.blocking_reasons {
            push_unique_string(&mut blocking_reasons, reason.clone());
        }
    }
    if runtime_boundary_status.current_mode != "legacy_stream" {
        push_unique_string(
            &mut blocking_reasons,
            "default_chat_runtime_boundary_not_legacy_stream".into(),
        );
    }
    if runtime_boundary_status.automatic_migration_enabled {
        push_unique_string(&mut blocking_reasons, "automatic_migration_enabled".into());
    }
    if !runtime_boundary_status.default_chat_unchanged {
        push_unique_string(&mut blocking_reasons, "default_chat_changed".into());
    }
    if runtime_boundary_status.controlled_candidate_available {
        push_unique_string(
            &mut blocking_reasons,
            "controlled_candidate_available_on_default_path".into(),
        );
    }
    if !runtime_boundary_status.candidate_promotion_readiness_required {
        push_unique_string(
            &mut blocking_reasons,
            "candidate_promotion_readiness_not_required_by_boundary_status".into(),
        );
    }
    for reason in &runtime_boundary_status.blocking_reasons {
        push_unique_string(&mut blocking_reasons, reason.clone());
    }

    let draft_ready = candidate_promotion_readiness_report.ready
        && runtime_boundary_status.current_mode == "legacy_stream"
        && !runtime_boundary_status.automatic_migration_enabled
        && runtime_boundary_status.default_chat_unchanged
        && !runtime_boundary_status.controlled_candidate_available
        && runtime_boundary_status.candidate_promotion_readiness_required
        && blocking_reasons.is_empty();

    let (
        activation_scope,
        required_preconditions,
        adapter_contract_checks,
        fallback_plan,
        rollback_plan,
        observability_plan,
        test_plan,
    ) = if draft_ready {
        (
            vec![
                "human-review-only draft for a future default Chat controlled adapter activation; default Chat remains on legacy_stream.".into(),
                "Scope is limited to activation boundaries, adapter contract checks, fallback, rollback, observability, and tests.".into(),
                "This draft does not replace default Chat, add an activation flag, run runtime, or create AgentRun/Evidence/Proposal/Memory/LifeModel/MCP audit/chat records.".into(),
            ],
            vec![
                "W33 candidate promotion readiness must remain ready at implementation review time.".into(),
                "W34 default Chat runtime boundary must remain currentMode=legacy_stream with automaticMigrationEnabled=false.".into(),
                "A separate reviewed implementation must explicitly approve any adapter routing work before send_message or start_stream_message changes.".into(),
                "Settings may display this draft only as read-only review material without switch, migrate, or enable controls.".into(),
            ],
            vec![
                "Keep adapter output constrained to the W31/W33 send_message-compatible contract shape before any default path integration.".into(),
                "Preserve send_message and start_stream_message request/response semantics, streaming completion behavior, and error fallback shape.".into(),
                "Require write-disabled, zero-tool, metadata-safe candidate evidence to remain valid before implementation discussion continues.".into(),
                "Reject any adapter path that persists private transcript text, assistant content, full tool data, Proposal, Memory, LifeModel patch, Evidence, MCP audit, or Chat message during draft evaluation.".into(),
            ],
            vec![
                "Keep the existing legacy stream default path as the fallback whenever a future adapter is unavailable, blocked, or fails contract checks.".into(),
                "Do not automatically retry through controlled runtime or promote candidate output into Chat history from this draft.".into(),
                "Surface blockers and keep the user on ordinary Chat until a separate implementation is reviewed.".into(),
            ],
            vec![
                "Rollback must revert only a separate adapter implementation and leave current Chat history as ordinary messages.".into(),
                "Remove any future adapter routing from the default path and return currentMode to legacy_stream.".into(),
                "Do not synthesize replacement evidence, replay candidate output, or patch LifeModel/Memory during rollback.".into(),
            ],
            vec![
                "Track metadata-safe readiness, boundary, fallback, rollback, error, and latency counters without private transcript text or full tool data.".into(),
                "Expose blocking reason counts and latest metadata-safe readiness digests for human review.".into(),
                "Keep observability separate from Chat message persistence, Evidence writes, MCP audit logs, and model/tool runtime payloads.".into(),
            ],
            vec![
                "Verify W33 blocked returns draftReady=false with no activation plan sections.".into(),
                "Verify W34 non-legacy or automatic migration enabled returns draftReady=false with no activation plan sections.".into(),
                "Verify W33 ready plus W34 legacy returns the complete human-review-only activation plan.".into(),
                "Verify command side-effect counts remain unchanged for AgentRun, Proposal, Evidence, LifeModel patch, MCP audit, Memory, and Chat messages.".into(),
                "Verify serialized output is metadata-safe and contains no private transcript text, assistant text, or full tool data.".into(),
                "Verify send_message and start_stream_message do not call this draft command.".into(),
            ],
        )
    } else {
        (
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
    };

    let metadata_safe_summary = json!({
        "activationPlan": "default_chat_adapter_activation",
        "metadataSafe": true,
        "readOnly": true,
        "humanReviewOnly": true,
        "draftReady": draft_ready,
        "manualReviewRequired": true,
        "notAutomaticMigration": true,
        "requiresSeparateImplementation": true,
        "candidatePromotionReady": candidate_promotion_readiness_report.ready,
        "currentMode": runtime_boundary_status.current_mode,
        "automaticMigrationEnabled": runtime_boundary_status.automatic_migration_enabled,
        "defaultChatUnchanged": runtime_boundary_status.default_chat_unchanged,
        "blockingReasonCount": blocking_reasons.len(),
        "activationSectionCount": activation_scope.len(),
        "preconditionSectionCount": required_preconditions.len(),
        "adapterContractCheckCount": adapter_contract_checks.len(),
        "fallbackPlanCount": fallback_plan.len(),
        "rollbackPlanCount": rollback_plan.len(),
        "observabilityPlanCount": observability_plan.len(),
        "testPlanCount": test_plan.len(),
        "contentStorage": "none",
        "toolStorage": "none",
        "chatHistoryStorage": "none",
        "proposalStorage": "none",
        "lifeModelPatchStorage": "none",
        "memoryStorage": "none",
        "evidenceStorage": "read_only",
        "mcpAuditStorage": "none",
        "transcriptStorage": "none",
    });

    DefaultChatAdapterActivationPlanDraft {
        draft_ready,
        candidate_promotion_readiness_report,
        runtime_boundary_status,
        activation_scope,
        required_preconditions,
        adapter_contract_checks,
        fallback_plan,
        rollback_plan,
        observability_plan,
        test_plan,
        manual_review_required: true,
        not_automatic_migration: true,
        requires_separate_implementation: true,
        blocking_reasons,
        metadata_safe_summary,
    }
}

#[tauri::command]
pub async fn record_default_chat_adapter_activation_review_decision(
    input: DefaultChatAdapterActivationReviewDecisionInput,
    state: State<'_, Arc<AppState>>,
) -> Result<DefaultChatAdapterActivationReviewDecisionResult, String> {
    record_default_chat_adapter_activation_review_decision_with_state(input, &state.inner().clone())
        .await
}

pub(crate) async fn record_default_chat_adapter_activation_review_decision_with_state(
    input: DefaultChatAdapterActivationReviewDecisionInput,
    state: &Arc<AppState>,
) -> Result<DefaultChatAdapterActivationReviewDecisionResult, String> {
    let decision_kind = safe_enum_value(
        &input.decision_kind,
        "decisionKind",
        &["approve", "reject", "request_rework"],
    )?;
    let session_id = normalize_optional_internal_id(input.session_id.as_deref(), "sessionId")?;
    let draft = draft_default_chat_adapter_activation_plan_with_state(
        DefaultChatAdapterActivationPlanDraftInput {
            required_approved_candidates: input.required_approved_candidates,
            required_promotions: input.required_promotions,
            session_id,
        },
        state,
    )
    .await?;
    let activation_plan_digest = default_chat_adapter_activation_plan_digest(&draft)?;
    let created_at = chrono::Utc::now().to_rfc3339();
    let mut blocking_reasons = draft.blocking_reasons.clone();

    if decision_kind == "approve" && !draft.draft_ready {
        push_unique_string(
            &mut blocking_reasons,
            "activation_plan_draft_not_ready_for_approval".into(),
        );
        return Ok(DefaultChatAdapterActivationReviewDecisionResult {
            recorded: false,
            evidence_id: None,
            decision_kind,
            draft_ready: false,
            activation_plan_digest,
            created_at,
            blocking_reasons,
        });
    }

    let reviewer_note_metadata =
        metadata_safe_reviewer_note_fields(input.optional_reviewer_note.as_deref());
    let mut evidence_draft = EvidenceDraft::new(
        EvidenceType::RuntimeBehavior,
        DEFAULT_CHAT_ADAPTER_ACTIVATION_REVIEW_DECISION_EVIDENCE_PATH,
        1.0,
        RiskLevel::Low,
        EvidencePrivacyLevel::Internal,
    );
    evidence_draft.run_metadata = json!({
        "evidenceKind": "default_chat_adapter_activation_review_decision",
        "decisionKind": decision_kind.clone(),
        "draftReady": draft.draft_ready,
        "activationPlanDigest": activation_plan_digest.clone(),
        "candidatePromotionReady": draft.candidate_promotion_readiness_report.ready,
        "currentMode": draft.runtime_boundary_status.current_mode,
        "automaticMigrationEnabled": draft.runtime_boundary_status.automatic_migration_enabled,
        "reviewerNoteChecksum": reviewer_note_metadata.checksum,
        "reviewerNoteLength": reviewer_note_metadata.length,
        "reviewerNoteCategory": reviewer_note_metadata.category,
        "createdAt": created_at.clone(),
    });

    let record = {
        let store = state.evidence_store.lock().await;
        store.create_evidence(evidence_draft).map_err(|e| {
            format!(
                "failed to record default Chat adapter activation review decision evidence: {e}"
            )
        })?
    };

    Ok(DefaultChatAdapterActivationReviewDecisionResult {
        recorded: true,
        evidence_id: Some(record.id),
        decision_kind,
        draft_ready: draft.draft_ready,
        activation_plan_digest,
        created_at,
        blocking_reasons,
    })
}

#[tauri::command]
pub async fn get_default_chat_adapter_activation_review_summary(
    state: State<'_, Arc<AppState>>,
) -> Result<DefaultChatAdapterActivationReviewSummary, String> {
    get_default_chat_adapter_activation_review_summary_with_state(&state.inner().clone()).await
}

pub(crate) async fn get_default_chat_adapter_activation_review_summary_with_state(
    state: &Arc<AppState>,
) -> Result<DefaultChatAdapterActivationReviewSummary, String> {
    let records = {
        let store = state.evidence_store.lock().await;
        store
            .query(EvidenceQuery {
                affected_path: Some(
                    DEFAULT_CHAT_ADAPTER_ACTIVATION_REVIEW_DECISION_EVIDENCE_PATH.into(),
                ),
                evidence_type: Some(EvidenceType::RuntimeBehavior),
                ..EvidenceQuery::default()
            })
            .map_err(|e| {
                format!("failed to read default Chat adapter activation review evidence: {e}")
            })?
    };
    let records = records
        .into_iter()
        .filter(default_chat_adapter_activation_review_decision_evidence_is_metadata_safe)
        .collect::<Vec<_>>();

    let approved_count = records
        .iter()
        .filter(|record| {
            default_chat_adapter_activation_review_decision_kind(record) == Some("approve")
        })
        .count();
    let reject_or_rework_count = records
        .iter()
        .filter(|record| {
            matches!(
                default_chat_adapter_activation_review_decision_kind(record),
                Some("reject" | "request_rework")
            )
        })
        .count();
    let latest_decision = records
        .first()
        .and_then(default_chat_adapter_activation_review_latest_decision);
    let latest_timestamp = latest_decision
        .as_ref()
        .map(|decision| decision.created_at.clone());
    let latest_decision_present = latest_decision.is_some();
    let blocking_reasons = if latest_decision_present {
        Vec::new()
    } else {
        vec!["activation_review_decision_missing".into()]
    };
    let blocking_reason_count = blocking_reasons.len();

    Ok(DefaultChatAdapterActivationReviewSummary {
        latest_decision,
        approved_count,
        reject_or_rework_count,
        latest_timestamp,
        blocking_reasons,
        metadata_safe_summary: json!({
            "activationReview": "default_chat_adapter_activation",
            "metadataSafe": true,
            "readOnly": true,
            "approvedCount": approved_count,
            "rejectOrReworkCount": reject_or_rework_count,
            "latestDecisionPresent": latest_decision_present,
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
        }),
    })
}

#[tauri::command]
pub async fn check_default_chat_adapter_activation_implementation_gate(
    input: DefaultChatAdapterActivationImplementationGateInput,
    state: State<'_, Arc<AppState>>,
) -> Result<DefaultChatAdapterActivationImplementationGateReport, String> {
    check_default_chat_adapter_activation_implementation_gate_with_state(
        input,
        &state.inner().clone(),
    )
    .await
}

pub(crate) async fn check_default_chat_adapter_activation_implementation_gate_with_state(
    input: DefaultChatAdapterActivationImplementationGateInput,
    state: &Arc<AppState>,
) -> Result<DefaultChatAdapterActivationImplementationGateReport, String> {
    let draft = draft_default_chat_adapter_activation_plan_with_state(
        DefaultChatAdapterActivationPlanDraftInput {
            required_approved_candidates: input.required_approved_candidates,
            required_promotions: input.required_promotions,
            session_id: input.session_id,
        },
        state,
    )
    .await?;
    let current_activation_plan_digest = default_chat_adapter_activation_plan_digest(&draft)?;
    let review_summary =
        get_default_chat_adapter_activation_review_summary_with_state(state).await?;
    let latest_decision = review_summary.latest_decision.clone();
    let mut blocking_reasons = Vec::new();

    if !draft.draft_ready {
        push_unique_string(
            &mut blocking_reasons,
            "activation_plan_draft_not_ready".into(),
        );
        for reason in &draft.blocking_reasons {
            push_unique_string(&mut blocking_reasons, reason.clone());
        }
    }

    let activation_plan_digest_matched = latest_decision
        .as_ref()
        .is_some_and(|decision| decision.activation_plan_digest == current_activation_plan_digest);

    match latest_decision.as_ref() {
        Some(decision) => {
            if decision.decision_kind != "approve" {
                push_unique_string(
                    &mut blocking_reasons,
                    format!(
                        "latest_activation_review_decision_is_{}",
                        decision.decision_kind
                    ),
                );
            }
            if !decision.draft_ready {
                push_unique_string(
                    &mut blocking_reasons,
                    "activation_review_draft_not_ready".into(),
                );
            }
            if !activation_plan_digest_matched {
                push_unique_string(
                    &mut blocking_reasons,
                    "activation_plan_digest_mismatch".into(),
                );
            }
            if !decision.candidate_promotion_ready {
                push_unique_string(
                    &mut blocking_reasons,
                    "activation_review_candidate_promotion_not_ready".into(),
                );
            }
            if decision.current_mode != "legacy_stream" {
                push_unique_string(
                    &mut blocking_reasons,
                    "activation_review_current_mode_not_legacy_stream".into(),
                );
            }
            if decision.automatic_migration_enabled {
                push_unique_string(
                    &mut blocking_reasons,
                    "activation_review_automatic_migration_enabled".into(),
                );
            }
        }
        None => {
            push_unique_string(
                &mut blocking_reasons,
                "activation_review_decision_missing".into(),
            );
        }
    }

    if draft.runtime_boundary_status.current_mode != "legacy_stream" {
        push_unique_string(
            &mut blocking_reasons,
            "default_chat_runtime_boundary_not_legacy_stream".into(),
        );
    }
    if draft.runtime_boundary_status.automatic_migration_enabled {
        push_unique_string(&mut blocking_reasons, "automatic_migration_enabled".into());
    }
    if !draft.runtime_boundary_status.default_chat_unchanged {
        push_unique_string(&mut blocking_reasons, "default_chat_changed".into());
    }

    let implementation_gate_eligible = draft.draft_ready
        && latest_decision.as_ref().is_some_and(|decision| {
            decision.decision_kind == "approve"
                && decision.draft_ready
                && decision.candidate_promotion_ready
                && decision.current_mode == "legacy_stream"
                && !decision.automatic_migration_enabled
        })
        && activation_plan_digest_matched
        && draft.runtime_boundary_status.default_chat_unchanged
        && draft.runtime_boundary_status.current_mode == "legacy_stream"
        && !draft.runtime_boundary_status.automatic_migration_enabled
        && blocking_reasons.is_empty();
    let latest_decision_kind = latest_decision
        .as_ref()
        .map(|decision| decision.decision_kind.clone())
        .unwrap_or_else(|| "none".into());
    let blocking_reason_count = blocking_reasons.len();

    Ok(DefaultChatAdapterActivationImplementationGateReport {
        implementation_gate_eligible,
        draft_ready: draft.draft_ready,
        latest_decision,
        current_activation_plan_digest,
        activation_plan_digest_matched,
        default_chat_unchanged: draft.runtime_boundary_status.default_chat_unchanged,
        automatic_migration_enabled: draft.runtime_boundary_status.automatic_migration_enabled,
        current_mode: draft.runtime_boundary_status.current_mode.clone(),
        blocking_reasons,
        metadata_safe_summary: json!({
            "activationImplementationGate": "default_chat_adapter_activation",
            "metadataSafe": true,
            "readOnly": true,
            "notAutomaticMigration": true,
            "requiresSeparateImplementation": true,
            "implementationGateEligible": implementation_gate_eligible,
            "draftReady": draft.draft_ready,
            "latestDecisionKind": latest_decision_kind,
            "activationPlanDigestMatched": activation_plan_digest_matched,
            "candidatePromotionReady": draft.candidate_promotion_readiness_report.ready,
            "currentMode": draft.runtime_boundary_status.current_mode,
            "automaticMigrationEnabled": draft.runtime_boundary_status.automatic_migration_enabled,
            "defaultChatUnchanged": draft.runtime_boundary_status.default_chat_unchanged,
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
            "modelCallStorage": "none",
        }),
    })
}

#[tauri::command]
pub async fn get_default_chat_adapter_routing_status(
    input: DefaultChatAdapterRoutingStatusInput,
    state: State<'_, Arc<AppState>>,
) -> Result<DefaultChatAdapterRoutingStatus, String> {
    get_default_chat_adapter_routing_status_with_state(input, &state.inner().clone()).await
}

pub(crate) async fn get_default_chat_adapter_routing_status_with_state(
    input: DefaultChatAdapterRoutingStatusInput,
    state: &Arc<AppState>,
) -> Result<DefaultChatAdapterRoutingStatus, String> {
    let activation_gate = check_default_chat_adapter_activation_implementation_gate_with_state(
        DefaultChatAdapterActivationImplementationGateInput {
            required_approved_candidates: input.required_approved_candidates,
            required_promotions: input.required_promotions,
            session_id: input.session_id,
        },
        state,
    )
    .await?;
    let route = crate::default_chat_adapter::resolve_default_chat_adapter_route();
    let current_mode = route.current_mode;
    let adapter_scaffold_present = route.adapter_scaffold_present;
    let controlled_adapter_enabled = route.controlled_adapter_enabled;
    let default_send_path = route.default_send_path;
    let start_stream_path = route.start_stream_path;
    let requires_separate_cutover_implementation = route.requires_separate_cutover_implementation;
    let mut blocking_reasons = Vec::new();

    if !activation_gate.implementation_gate_eligible {
        push_unique_string(
            &mut blocking_reasons,
            "activation_implementation_gate_not_eligible".into(),
        );
        for reason in &activation_gate.blocking_reasons {
            push_unique_string(&mut blocking_reasons, reason.clone());
        }
    }
    let blocking_reason_count = blocking_reasons.len();

    Ok(DefaultChatAdapterRoutingStatus {
        current_mode: current_mode.clone(),
        adapter_scaffold_present,
        controlled_adapter_enabled,
        default_send_path: default_send_path.clone(),
        start_stream_path: start_stream_path.clone(),
        activation_implementation_gate_eligible: activation_gate.implementation_gate_eligible,
        requires_separate_cutover_implementation,
        blocking_reasons,
        metadata_safe_summary: json!({
            "defaultChatAdapterRouting": "disabled_scaffold",
            "metadataSafe": true,
            "readOnly": true,
            "routingMode": current_mode,
            "adapterScaffoldPresent": adapter_scaffold_present,
            "controlledAdapterEnabled": controlled_adapter_enabled,
            "defaultSendPath": default_send_path,
            "startStreamPath": start_stream_path,
            "activationImplementationGateEligible": activation_gate.implementation_gate_eligible,
            "notAutomaticMigration": true,
            "requiresSeparateCutoverImplementation": requires_separate_cutover_implementation,
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
            "modelCallStorage": "none",
        }),
    })
}

#[tauri::command]
pub async fn check_default_chat_adapter_contract_harness(
    input: DefaultChatAdapterContractHarnessInput,
    state: State<'_, Arc<AppState>>,
) -> Result<DefaultChatAdapterContractHarnessReport, String> {
    check_default_chat_adapter_contract_harness_with_state(input, &state.inner().clone()).await
}

pub(crate) async fn check_default_chat_adapter_contract_harness_with_state(
    input: DefaultChatAdapterContractHarnessInput,
    state: &Arc<AppState>,
) -> Result<DefaultChatAdapterContractHarnessReport, String> {
    let routing_status = get_default_chat_adapter_routing_status_with_state(
        DefaultChatAdapterRoutingStatusInput {
            required_approved_candidates: input.required_approved_candidates,
            required_promotions: input.required_promotions,
            session_id: input.session_id,
        },
        state,
    )
    .await?;

    let expected_path = "legacy_stream".to_string();
    let mut send_blocking_reasons = Vec::new();
    if routing_status.default_send_path != expected_path {
        push_unique_string(
            &mut send_blocking_reasons,
            "default_send_path_drifted".into(),
        );
    }
    let send_message_contract = DefaultChatAdapterContractCheck {
        name: "send_message".into(),
        ready: send_blocking_reasons.is_empty(),
        expected_path: expected_path.clone(),
        actual_path: routing_status.default_send_path.clone(),
        blocking_reasons: send_blocking_reasons,
    };

    let mut stream_blocking_reasons = Vec::new();
    if routing_status.start_stream_path != expected_path {
        push_unique_string(
            &mut stream_blocking_reasons,
            "start_stream_path_drifted".into(),
        );
    }
    let stream_message_contract = DefaultChatAdapterContractCheck {
        name: "start_stream_message".into(),
        ready: stream_blocking_reasons.is_empty(),
        expected_path,
        actual_path: routing_status.start_stream_path.clone(),
        blocking_reasons: stream_blocking_reasons,
    };

    let mut blocking_reasons = routing_status.blocking_reasons.clone();
    if !routing_status.adapter_scaffold_present {
        push_unique_string(&mut blocking_reasons, "adapter_scaffold_missing".into());
    }
    if routing_status.controlled_adapter_enabled {
        push_unique_string(&mut blocking_reasons, "controlled_adapter_enabled".into());
    }
    if routing_status.current_mode != "legacy_stream" {
        push_unique_string(&mut blocking_reasons, "default_chat_mode_drifted".into());
    }
    for reason in &send_message_contract.blocking_reasons {
        push_unique_string(&mut blocking_reasons, reason.clone());
    }
    for reason in &stream_message_contract.blocking_reasons {
        push_unique_string(&mut blocking_reasons, reason.clone());
    }

    let adapter_disabled = routing_status.adapter_scaffold_present
        && !routing_status.controlled_adapter_enabled
        && routing_status.current_mode == "legacy_stream"
        && routing_status.default_send_path == "legacy_stream"
        && routing_status.start_stream_path == "legacy_stream";
    let contract_shape = "disabled_adapter_legacy_stream_contract".to_string();
    let activation_implementation_gate_eligible =
        routing_status.activation_implementation_gate_eligible;
    let contract_harness_ready = adapter_disabled
        && activation_implementation_gate_eligible
        && send_message_contract.ready
        && stream_message_contract.ready
        && blocking_reasons.is_empty();
    let blocking_reason_count = blocking_reasons.len();

    Ok(DefaultChatAdapterContractHarnessReport {
        contract_harness_ready,
        contract_shape: contract_shape.clone(),
        adapter_disabled,
        activation_implementation_gate_eligible,
        routing_status,
        send_message_contract,
        stream_message_contract,
        blocking_reasons,
        metadata_safe_summary: json!({
            "contractHarness": "default_chat_adapter",
            "metadataSafe": true,
            "readOnly": true,
            "contractHarnessReady": contract_harness_ready,
            "contractShape": contract_shape,
            "adapterDisabled": adapter_disabled,
            "activationImplementationGateEligible": activation_implementation_gate_eligible,
            "currentMode": "legacy_stream",
            "defaultSendPath": "legacy_stream",
            "startStreamPath": "legacy_stream",
            "controlledAdapterEnabled": false,
            "notAutomaticMigration": true,
            "requiresSeparateCutoverImplementation": true,
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
            "modelCallStorage": "none",
        }),
    })
}

#[tauri::command]
pub async fn run_default_chat_adapter_dry_run(
    input: DefaultChatAdapterDryRunInput,
    state: State<'_, Arc<AppState>>,
) -> Result<DefaultChatAdapterDryRunReport, String> {
    run_default_chat_adapter_dry_run_with_state(input, &state.inner().clone()).await
}

pub(crate) async fn run_default_chat_adapter_dry_run_with_state(
    input: DefaultChatAdapterDryRunInput,
    state: &Arc<AppState>,
) -> Result<DefaultChatAdapterDryRunReport, String> {
    let source_session_id = safe_internal_id(&input.session_id, "sessionId")?;
    let input_message_length = input.message.chars().count();
    let input_message_hash = sha256_hex(&input.message);
    let contract_shape = "default_chat_adapter_dry_run_contract".to_string();
    let allow_writes = false;
    let max_tool_calls = 0;
    let default_chat_path_unchanged = true;
    let chat_message_saved = false;
    let agent_run_recorded = false;

    let contract_harness = check_default_chat_adapter_contract_harness_with_state(
        DefaultChatAdapterContractHarnessInput {
            required_approved_candidates: input.required_approved_candidates,
            required_promotions: input.required_promotions,
            session_id: Some(source_session_id.clone()),
        },
        state,
    )
    .await?;

    let mut blocking_reasons = contract_harness.blocking_reasons.clone();
    if !contract_harness.contract_harness_ready {
        push_unique_string(&mut blocking_reasons, "contract_harness_not_ready".into());
    }

    let dry_run_ready = contract_harness.contract_harness_ready && blocking_reasons.is_empty();
    let blocked = !dry_run_ready;
    let adapter_path = if dry_run_ready {
        "controlled_adapter_dry_run"
    } else {
        "blocked"
    }
    .to_string();
    let blocking_reason_count = blocking_reasons.len();

    Ok(DefaultChatAdapterDryRunReport {
        dry_run_ready,
        blocked,
        contract_shape: contract_shape.clone(),
        source_session_id,
        adapter_path: adapter_path.clone(),
        allow_writes,
        max_tool_calls,
        default_chat_path_unchanged,
        chat_message_saved,
        agent_run_recorded,
        contract_harness_ready: contract_harness.contract_harness_ready,
        input_message_length,
        input_message_hash,
        user_output_preview: None,
        blocking_reasons,
        metadata_safe_summary: json!({
            "adapterDryRun": "default_chat_adapter",
            "metadataSafe": true,
            "readOnly": true,
            "dryRunReady": dry_run_ready,
            "blocked": blocked,
            "contractShape": contract_shape,
            "adapterPath": adapter_path,
            "contractHarnessReady": contract_harness.contract_harness_ready,
            "allowWrites": allow_writes,
            "maxToolCalls": max_tool_calls,
            "defaultChatPathUnchanged": default_chat_path_unchanged,
            "chatMessageSaved": chat_message_saved,
            "agentRunRecorded": agent_run_recorded,
            "runtimeCallStorage": "none",
            "modelCallStorage": "none",
            "contentStorage": "length_checksum_only",
            "toolStorage": "none",
            "chatHistoryStorage": "none",
            "proposalStorage": "none",
            "lifeModelPatchStorage": "none",
            "memoryStorage": "none",
            "evidenceStorage": "read_only",
            "mcpAuditStorage": "none",
            "externalWriteStorage": "none",
            "transcriptStorage": "none",
            "notAutomaticMigration": true,
            "defaultSendPath": "legacy_stream",
            "startStreamPath": "legacy_stream",
            "blockingReasonCount": blocking_reason_count,
        }),
    })
}

#[tauri::command]
pub async fn record_default_chat_adapter_dry_run_review_decision(
    input: DefaultChatAdapterDryRunReviewDecisionInput,
    state: State<'_, Arc<AppState>>,
) -> Result<DefaultChatAdapterDryRunReviewDecisionResult, String> {
    record_default_chat_adapter_dry_run_review_decision_with_state(input, &state.inner().clone())
        .await
}

pub(crate) async fn record_default_chat_adapter_dry_run_review_decision_with_state(
    input: DefaultChatAdapterDryRunReviewDecisionInput,
    state: &Arc<AppState>,
) -> Result<DefaultChatAdapterDryRunReviewDecisionResult, String> {
    let decision_kind = safe_enum_value(
        &input.decision_kind,
        "decisionKind",
        &["approve", "reject", "request_rework"],
    )?;
    let source_session_id = safe_internal_id(&input.source_session_id, "sourceSessionId")?;
    let expected_digest = input
        .dry_run_summary_digest
        .as_deref()
        .map(|value| safe_checksum_field(value, "dryRunSummaryDigest"))
        .transpose()?;

    let dry_run = run_default_chat_adapter_dry_run_with_state(
        DefaultChatAdapterDryRunInput {
            session_id: source_session_id.clone(),
            message: input.message,
            required_approved_candidates: input.required_approved_candidates,
            required_promotions: input.required_promotions,
        },
        state,
    )
    .await?;
    let dry_run_summary_digest = metadata_hash_for_serializable(&dry_run)?;
    let created_at = chrono::Utc::now().to_rfc3339();
    let mut blocking_reasons = dry_run.blocking_reasons.clone();

    if expected_digest
        .as_ref()
        .is_some_and(|expected| expected != &dry_run_summary_digest)
    {
        push_unique_string(
            &mut blocking_reasons,
            "dry_run_summary_digest_mismatch".into(),
        );
        return Ok(DefaultChatAdapterDryRunReviewDecisionResult {
            recorded: false,
            evidence_id: None,
            decision_kind,
            source_session_id,
            contract_shape: dry_run.contract_shape,
            dry_run_ready: dry_run.dry_run_ready,
            dry_run_summary_digest,
            created_at,
            blocking_reasons,
        });
    }

    if decision_kind == "approve" && !dry_run.dry_run_ready {
        push_unique_string(
            &mut blocking_reasons,
            "dry_run_not_ready_for_approval".into(),
        );
        return Ok(DefaultChatAdapterDryRunReviewDecisionResult {
            recorded: false,
            evidence_id: None,
            decision_kind,
            source_session_id,
            contract_shape: dry_run.contract_shape,
            dry_run_ready: false,
            dry_run_summary_digest,
            created_at,
            blocking_reasons,
        });
    }

    let reviewer_note_metadata =
        metadata_safe_reviewer_note_fields(input.optional_reviewer_note.as_deref());
    let mut evidence_draft = EvidenceDraft::new(
        EvidenceType::RuntimeBehavior,
        DEFAULT_CHAT_ADAPTER_DRY_RUN_REVIEW_DECISION_EVIDENCE_PATH,
        1.0,
        RiskLevel::Low,
        EvidencePrivacyLevel::Internal,
    );
    evidence_draft.run_metadata = json!({
        "evidenceKind": "default_chat_adapter_dry_run_review_decision",
        "decisionKind": decision_kind.clone(),
        "sourceSessionId": source_session_id.clone(),
        "contractShape": dry_run.contract_shape.clone(),
        "dryRunReady": dry_run.dry_run_ready,
        "dryRunSummaryDigest": dry_run_summary_digest.clone(),
        "reviewerNoteChecksum": reviewer_note_metadata.checksum,
        "reviewerNoteLength": reviewer_note_metadata.length,
        "reviewerNoteCategory": reviewer_note_metadata.category,
        "createdAt": created_at.clone(),
    });

    let record = {
        let store = state.evidence_store.lock().await;
        store.create_evidence(evidence_draft).map_err(|e| {
            format!("failed to record default Chat adapter dry-run review decision evidence: {e}")
        })?
    };

    Ok(DefaultChatAdapterDryRunReviewDecisionResult {
        recorded: true,
        evidence_id: Some(record.id),
        decision_kind,
        source_session_id,
        contract_shape: dry_run.contract_shape,
        dry_run_ready: dry_run.dry_run_ready,
        dry_run_summary_digest,
        created_at,
        blocking_reasons,
    })
}

#[tauri::command]
pub async fn get_default_chat_adapter_dry_run_review_summary(
    state: State<'_, Arc<AppState>>,
) -> Result<DefaultChatAdapterDryRunReviewSummary, String> {
    get_default_chat_adapter_dry_run_review_summary_with_state(&state.inner().clone()).await
}

pub(crate) async fn get_default_chat_adapter_dry_run_review_summary_with_state(
    state: &Arc<AppState>,
) -> Result<DefaultChatAdapterDryRunReviewSummary, String> {
    let records = {
        let store = state.evidence_store.lock().await;
        store
            .query(EvidenceQuery {
                affected_path: Some(
                    DEFAULT_CHAT_ADAPTER_DRY_RUN_REVIEW_DECISION_EVIDENCE_PATH.into(),
                ),
                evidence_type: Some(EvidenceType::RuntimeBehavior),
                ..EvidenceQuery::default()
            })
            .map_err(|e| {
                format!("failed to read default Chat adapter dry-run review evidence: {e}")
            })?
    };
    let records = records
        .into_iter()
        .filter(default_chat_adapter_dry_run_review_decision_evidence_is_metadata_safe)
        .collect::<Vec<_>>();

    let approved_count = records
        .iter()
        .filter(|record| {
            default_chat_adapter_dry_run_review_decision_kind(record) == Some("approve")
        })
        .count();
    let reject_or_rework_count = records
        .iter()
        .filter(|record| {
            matches!(
                default_chat_adapter_dry_run_review_decision_kind(record),
                Some("reject" | "request_rework")
            )
        })
        .count();
    let latest_decision = records
        .first()
        .and_then(default_chat_adapter_dry_run_review_latest_decision);
    let latest_timestamp = latest_decision
        .as_ref()
        .map(|decision| decision.created_at.clone());
    let latest_decision_present = latest_decision.is_some();
    let blocking_reasons = if latest_decision_present {
        Vec::new()
    } else {
        vec!["dry_run_review_decision_missing".into()]
    };
    let blocking_reason_count = blocking_reasons.len();

    Ok(DefaultChatAdapterDryRunReviewSummary {
        latest_decision,
        approved_count,
        reject_or_rework_count,
        latest_timestamp,
        blocking_reasons,
        metadata_safe_summary: json!({
            "dryRunReview": "default_chat_adapter",
            "metadataSafe": true,
            "readOnly": true,
            "approvedCount": approved_count,
            "rejectOrReworkCount": reject_or_rework_count,
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
            "modelCallStorage": "none",
            "externalWriteStorage": "none",
            "transcriptStorage": "none",
        }),
    })
}

#[tauri::command]
pub async fn check_default_chat_adapter_implementation_readiness(
    input: DefaultChatAdapterImplementationReadinessInput,
    state: State<'_, Arc<AppState>>,
) -> Result<DefaultChatAdapterImplementationReadinessReport, String> {
    check_default_chat_adapter_implementation_readiness_with_state(input, &state.inner().clone())
        .await
}

pub(crate) async fn check_default_chat_adapter_implementation_readiness_with_state(
    input: DefaultChatAdapterImplementationReadinessInput,
    state: &Arc<AppState>,
) -> Result<DefaultChatAdapterImplementationReadinessReport, String> {
    let source_session_id = safe_internal_id(&input.source_session_id, "sourceSessionId")?;
    let activation_gate = check_default_chat_adapter_activation_implementation_gate_with_state(
        DefaultChatAdapterActivationImplementationGateInput {
            required_approved_candidates: input.required_approved_candidates,
            required_promotions: input.required_promotions,
            session_id: Some(source_session_id.clone()),
        },
        state,
    )
    .await?;
    let contract_harness = check_default_chat_adapter_contract_harness_with_state(
        DefaultChatAdapterContractHarnessInput {
            required_approved_candidates: input.required_approved_candidates,
            required_promotions: input.required_promotions,
            session_id: Some(source_session_id.clone()),
        },
        state,
    )
    .await?;
    let dry_run = run_default_chat_adapter_dry_run_with_state(
        DefaultChatAdapterDryRunInput {
            session_id: source_session_id,
            message: input.message,
            required_approved_candidates: input.required_approved_candidates,
            required_promotions: input.required_promotions,
        },
        state,
    )
    .await?;
    let dry_run_review_summary =
        get_default_chat_adapter_dry_run_review_summary_with_state(state).await?;
    let current_dry_run_digest = metadata_hash_for_serializable(&dry_run)?;
    let latest_dry_run_review_decision = dry_run_review_summary.latest_decision.clone();

    let activation_implementation_gate_eligible = activation_gate.implementation_gate_eligible;
    let contract_harness_ready = contract_harness.contract_harness_ready;
    let dry_run_ready = dry_run.dry_run_ready;
    let dry_run_review_approved = latest_dry_run_review_decision
        .as_ref()
        .is_some_and(|decision| decision.decision_kind == "approve" && decision.dry_run_ready);
    let dry_run_digest_matched = latest_dry_run_review_decision
        .as_ref()
        .is_some_and(|decision| decision.dry_run_summary_digest == current_dry_run_digest);
    let default_send_path = contract_harness.routing_status.default_send_path.clone();
    let start_stream_path = contract_harness.routing_status.start_stream_path.clone();
    let controlled_adapter_enabled = contract_harness.routing_status.controlled_adapter_enabled;
    let automatic_migration_enabled = activation_gate.automatic_migration_enabled;
    let default_chat_unchanged = activation_gate.default_chat_unchanged
        && contract_harness.routing_status.current_mode == "legacy_stream"
        && default_send_path == "legacy_stream"
        && start_stream_path == "legacy_stream";

    let mut blocking_reasons = Vec::new();
    for reason in &activation_gate.blocking_reasons {
        push_unique_string(&mut blocking_reasons, reason.clone());
    }
    for reason in &contract_harness.blocking_reasons {
        push_unique_string(&mut blocking_reasons, reason.clone());
    }
    for reason in &dry_run.blocking_reasons {
        push_unique_string(&mut blocking_reasons, reason.clone());
    }
    for reason in &dry_run_review_summary.blocking_reasons {
        push_unique_string(&mut blocking_reasons, reason.clone());
    }

    if !activation_implementation_gate_eligible {
        push_unique_string(
            &mut blocking_reasons,
            "activation_implementation_gate_not_eligible".into(),
        );
    }
    if !contract_harness_ready {
        push_unique_string(&mut blocking_reasons, "contract_harness_not_ready".into());
    }
    if !dry_run_ready {
        push_unique_string(&mut blocking_reasons, "dry_run_not_ready".into());
    }
    match latest_dry_run_review_decision.as_ref() {
        Some(decision) => {
            if decision.decision_kind != "approve" {
                push_unique_string(
                    &mut blocking_reasons,
                    "latest_dry_run_review_not_approve".into(),
                );
            }
            if !decision.dry_run_ready {
                push_unique_string(
                    &mut blocking_reasons,
                    "approved_dry_run_review_not_ready".into(),
                );
            }
            if decision.decision_kind == "approve" && !dry_run_digest_matched {
                push_unique_string(
                    &mut blocking_reasons,
                    "dry_run_review_digest_mismatch".into(),
                );
            }
        }
        None => {
            push_unique_string(
                &mut blocking_reasons,
                "dry_run_review_approval_missing".into(),
            );
        }
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

    let implementation_ready = activation_implementation_gate_eligible
        && contract_harness_ready
        && dry_run_ready
        && dry_run_review_approved
        && dry_run_digest_matched
        && default_chat_unchanged
        && !controlled_adapter_enabled
        && !automatic_migration_enabled
        && default_send_path == "legacy_stream"
        && start_stream_path == "legacy_stream"
        && blocking_reasons.is_empty();
    let blocking_reason_count = blocking_reasons.len();

    Ok(DefaultChatAdapterImplementationReadinessReport {
        implementation_ready,
        latest_dry_run_review_decision,
        activation_implementation_gate_eligible,
        contract_harness_ready,
        dry_run_ready,
        dry_run_review_approved,
        dry_run_digest_matched,
        default_chat_unchanged,
        controlled_adapter_enabled,
        automatic_migration_enabled,
        default_send_path: default_send_path.clone(),
        start_stream_path: start_stream_path.clone(),
        blocking_reasons,
        metadata_safe_summary: json!({
            "implementationReadiness": "default_chat_adapter",
            "metadataSafe": true,
            "readOnly": true,
            "implementationReady": implementation_ready,
            "activationImplementationGateEligible": activation_implementation_gate_eligible,
            "contractHarnessReady": contract_harness_ready,
            "dryRunReady": dry_run_ready,
            "dryRunReviewApproved": dry_run_review_approved,
            "dryRunDigestMatched": dry_run_digest_matched,
            "defaultChatUnchanged": default_chat_unchanged,
            "controlledAdapterEnabled": controlled_adapter_enabled,
            "automaticMigrationEnabled": automatic_migration_enabled,
            "defaultSendPath": default_send_path,
            "startStreamPath": start_stream_path,
            "blockingReasonCount": blocking_reason_count,
            "contentStorage": "length_checksum_only",
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

fn default_chat_adapter_activation_plan_digest(
    draft: &DefaultChatAdapterActivationPlanDraft,
) -> Result<String, String> {
    let mut value = serde_json::to_value(draft)
        .map_err(|e| format!("failed to serialize activation plan draft for hashing: {e}"))?;
    if let Some(report) = value
        .get_mut("candidatePromotionReadinessReport")
        .and_then(Value::as_object_mut)
    {
        report.remove("checkedAt");
    }
    metadata_hash_for_serializable(&value)
}

pub(crate) fn default_chat_adapter_activation_review_decision_evidence_is_metadata_safe(
    record: &openlife_core::agent::EvidenceRecord,
) -> bool {
    if record.affected_path != DEFAULT_CHAT_ADAPTER_ACTIVATION_REVIEW_DECISION_EVIDENCE_PATH
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
        "draftReady",
        "activationPlanDigest",
        "candidatePromotionReady",
        "currentMode",
        "automaticMigrationEnabled",
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

    record
        .run_metadata
        .get("evidenceKind")
        .and_then(Value::as_str)
        .is_some_and(|value| value == "default_chat_adapter_activation_review_decision")
        && metadata_string_is_safe(&record.run_metadata, "decisionKind", |value, field| {
            safe_enum_value(value, field, &["approve", "reject", "request_rework"])
        })
        && record
            .run_metadata
            .get("draftReady")
            .and_then(Value::as_bool)
            .is_some()
        && metadata_string_is_safe(&record.run_metadata, "activationPlanDigest", |value, _| {
            safe_checksum(value)
        })
        && record
            .run_metadata
            .get("candidatePromotionReady")
            .and_then(Value::as_bool)
            .is_some()
        && metadata_string_is_safe(&record.run_metadata, "currentMode", safe_internal_id)
        && record
            .run_metadata
            .get("automaticMigrationEnabled")
            .and_then(Value::as_bool)
            .is_some()
        && reviewer_note_flat_metadata_is_safe(&record.run_metadata)
        && record
            .run_metadata
            .get("createdAt")
            .and_then(Value::as_str)
            .is_some_and(|value| chrono::DateTime::parse_from_rfc3339(value).is_ok())
        && !contains_unsafe_promotion_metadata(&record.run_metadata)
}

fn default_chat_adapter_activation_review_decision_kind(
    record: &openlife_core::agent::EvidenceRecord,
) -> Option<&str> {
    record
        .run_metadata
        .get("decisionKind")
        .and_then(Value::as_str)
}

fn default_chat_adapter_activation_review_latest_decision(
    record: &openlife_core::agent::EvidenceRecord,
) -> Option<DefaultChatAdapterActivationReviewLatestDecision> {
    Some(DefaultChatAdapterActivationReviewLatestDecision {
        evidence_id: record.id.clone(),
        decision_kind: default_chat_adapter_activation_review_decision_kind(record)?.to_string(),
        draft_ready: record
            .run_metadata
            .get("draftReady")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        activation_plan_digest: record
            .run_metadata
            .get("activationPlanDigest")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)?,
        candidate_promotion_ready: record
            .run_metadata
            .get("candidatePromotionReady")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        current_mode: record
            .run_metadata
            .get("currentMode")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)?,
        automatic_migration_enabled: record
            .run_metadata
            .get("automaticMigrationEnabled")
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
    })
}

fn default_chat_adapter_dry_run_review_decision_evidence_is_metadata_safe(
    record: &openlife_core::agent::EvidenceRecord,
) -> bool {
    if record.affected_path != DEFAULT_CHAT_ADAPTER_DRY_RUN_REVIEW_DECISION_EVIDENCE_PATH
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
        "contractShape",
        "dryRunReady",
        "dryRunSummaryDigest",
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

    record
        .run_metadata
        .get("evidenceKind")
        .and_then(Value::as_str)
        .is_some_and(|value| value == "default_chat_adapter_dry_run_review_decision")
        && metadata_string_is_safe(&record.run_metadata, "decisionKind", |value, field| {
            safe_enum_value(value, field, &["approve", "reject", "request_rework"])
        })
        && metadata_string_is_safe(&record.run_metadata, "sourceSessionId", safe_internal_id)
        && metadata_string_is_safe(&record.run_metadata, "contractShape", safe_internal_id)
        && record
            .run_metadata
            .get("dryRunReady")
            .and_then(Value::as_bool)
            .is_some()
        && metadata_string_is_safe(&record.run_metadata, "dryRunSummaryDigest", |value, _| {
            safe_checksum_field(value, "dryRunSummaryDigest")
        })
        && reviewer_note_flat_metadata_is_safe(&record.run_metadata)
        && record
            .run_metadata
            .get("createdAt")
            .and_then(Value::as_str)
            .is_some_and(|value| chrono::DateTime::parse_from_rfc3339(value).is_ok())
        && !contains_unsafe_promotion_metadata(&record.run_metadata)
}

fn default_chat_adapter_dry_run_review_decision_kind(
    record: &openlife_core::agent::EvidenceRecord,
) -> Option<&str> {
    record
        .run_metadata
        .get("decisionKind")
        .and_then(Value::as_str)
}

fn default_chat_adapter_dry_run_review_latest_decision(
    record: &openlife_core::agent::EvidenceRecord,
) -> Option<DefaultChatAdapterDryRunReviewLatestDecision> {
    Some(DefaultChatAdapterDryRunReviewLatestDecision {
        evidence_id: record.id.clone(),
        decision_kind: default_chat_adapter_dry_run_review_decision_kind(record)?.to_string(),
        source_session_id: record
            .run_metadata
            .get("sourceSessionId")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)?,
        contract_shape: record
            .run_metadata
            .get("contractShape")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)?,
        dry_run_ready: record
            .run_metadata
            .get("dryRunReady")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        dry_run_summary_digest: record
            .run_metadata
            .get("dryRunSummaryDigest")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)?,
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
    })
}
