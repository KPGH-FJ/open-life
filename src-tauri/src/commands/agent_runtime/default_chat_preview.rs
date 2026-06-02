use super::*;

#[tauri::command]
pub async fn run_default_chat_adapter_controlled_preview(
    input: DefaultChatAdapterControlledPreviewInput,
    state: State<'_, Arc<AppState>>,
) -> Result<DefaultChatAdapterControlledPreviewReport, String> {
    run_default_chat_adapter_controlled_preview_with_state(input, &state.inner().clone()).await
}

pub(crate) async fn run_default_chat_adapter_controlled_preview_with_state(
    input: DefaultChatAdapterControlledPreviewInput,
    state: &Arc<AppState>,
) -> Result<DefaultChatAdapterControlledPreviewReport, String> {
    let source_session_id = safe_internal_id(&input.source_session_id, "sourceSessionId")?;
    let allow_writes = false;
    let max_tool_calls = 0;
    let default_chat_path_unchanged = true;
    let chat_message_saved = false;

    let readiness = check_default_chat_adapter_implementation_readiness_with_state(
        DefaultChatAdapterImplementationReadinessInput {
            source_session_id: source_session_id.clone(),
            message: input.message.clone(),
            required_approved_candidates: input.required_approved_candidates,
            required_promotions: input.required_promotions,
        },
        state,
    )
    .await?;

    if !readiness.implementation_ready {
        let mut blocking_reasons = readiness.blocking_reasons.clone();
        push_unique_string(
            &mut blocking_reasons,
            "implementation_readiness_not_ready".into(),
        );
        let metadata_safe_summary =
            default_chat_adapter_controlled_preview_blocked_summary(&readiness, &blocking_reasons);
        let reasoning_trace = ReasoningTrace {
            strategy_result: Some(metadata_safe_summary.clone()),
            output: Some("default_chat_adapter_controlled_preview_blocked".into()),
            stable_steps: vec![
                "implementation_readiness_check".into(),
                "blocked_before_runtime".into(),
            ],
            ..ReasoningTrace::default()
        };
        return Ok(DefaultChatAdapterControlledPreviewReport {
            preview_ready: false,
            blocked: true,
            contract_shape: "blocked".into(),
            source_session_id,
            adapter_path: "blocked".into(),
            reply: None,
            reasoning_trace,
            tool_calls: Vec::new(),
            run_id: None,
            allow_writes,
            max_tool_calls,
            default_chat_path_unchanged,
            chat_message_saved,
            agent_run_recorded: false,
            implementation_ready: false,
            warnings: Vec::new(),
            blocking_reasons,
            metadata_safe_summary,
        });
    }

    let input_message_hash = sha256_metadata_checksum(&input.message);
    let mut preview_run =
        new_default_chat_adapter_controlled_preview_run(&source_session_id, &input_message_hash);
    let preview_run_id = preview_run.id.clone();
    create_default_chat_adapter_controlled_preview_run(state, &preview_run).await?;

    let runtime_input = MultiStrategyAgentPreviewInput {
        session_id: source_session_id.clone(),
        user_text: input.message,
        tools_prompt:
            "No developer tools catalog supplied for this default Chat adapter controlled preview."
                .into(),
        allow_planning: false,
        local_model_available: true,
        layer: Some("L2".into()),
        execution_budget: Some(MultiStrategyAgentPreviewExecutionBudgetInput {
            max_steps: Some(2),
            max_tool_calls: Some(max_tool_calls),
            timeout_seconds: Some(30),
            allow_cloud: Some(false),
            allow_writes: Some(allow_writes),
        }),
    };

    let execution =
        execute_multi_strategy_agent_preview(runtime_input, state, &preview_run_id).await;
    let execution = match execution {
        Ok(execution) => execution,
        Err(error) => {
            let safe_error = metadata_safe_default_chat_adapter_controlled_preview_error(&error);
            fail_default_chat_adapter_controlled_preview_run(state, &mut preview_run, &safe_error)
                .await;
            let metadata_safe_summary =
                default_chat_adapter_controlled_preview_failed_summary(&readiness, &safe_error);
            let reasoning_trace = ReasoningTrace {
                strategy_result: Some(metadata_safe_summary.clone()),
                output: Some("default_chat_adapter_controlled_preview_failed".into()),
                ..ReasoningTrace::default()
            };
            return Ok(DefaultChatAdapterControlledPreviewReport {
                preview_ready: false,
                blocked: false,
                contract_shape: "failed".into(),
                source_session_id,
                adapter_path: "controlled_adapter_preview_failed".into(),
                reply: None,
                reasoning_trace,
                tool_calls: Vec::new(),
                run_id: Some(preview_run_id),
                allow_writes,
                max_tool_calls,
                default_chat_path_unchanged,
                chat_message_saved,
                agent_run_recorded: true,
                implementation_ready: true,
                warnings: vec!["controlled adapter preview runtime failed".into()],
                blocking_reasons: vec![safe_error],
                metadata_safe_summary,
            });
        }
    };

    let contract_shape =
        default_chat_adapter_controlled_preview_contract_shape(&execution.output).to_string();
    let reply = default_chat_adapter_controlled_preview_reply(&execution.output);
    let mut warnings = preview_output_warnings(&execution.output, &execution.warnings);
    push_unique_string(
        &mut warnings,
        "controlled adapter preview forced allowWrites=false".into(),
    );
    let blocking_reasons =
        default_chat_adapter_controlled_preview_contract_blockers(&execution.output);
    let preview_ready = contract_shape == "send_message_compatible" && blocking_reasons.is_empty();
    let output_digest = reply
        .as_deref()
        .filter(|value| !value.is_empty())
        .map(sha256_metadata_checksum);
    let metadata_safe_summary = default_chat_adapter_controlled_preview_metadata_safe_summary(
        &execution.output,
        &readiness,
        &contract_shape,
        preview_ready,
        output_digest.as_deref(),
    );
    let audit = default_chat_adapter_controlled_preview_audit_summary(
        &execution.output,
        &readiness,
        &contract_shape,
        preview_ready,
        output_digest.as_deref(),
        &warnings,
    );
    complete_default_chat_adapter_controlled_preview_run(
        state,
        &mut preview_run,
        DefaultChatAdapterControlledPreviewRunCompletion {
            audit: audit.clone(),
            warnings: warnings.clone(),
            context_summary: execution.context_summary,
            hs_selection_audit: execution.hs_selection_audit,
            behavior_checks: execution.behavior_checks,
        },
    )
    .await?;

    let reasoning_trace = ReasoningTrace {
        strategy_result: Some(audit),
        output: Some("default_chat_adapter_controlled_preview".into()),
        stable_steps: vec![
            "implementation_readiness_check".into(),
            "controlled_adapter_preview".into(),
            "send_message_contract_shape_validation".into(),
            "metadata_safe_audit".into(),
        ],
        ..ReasoningTrace::default()
    };

    Ok(DefaultChatAdapterControlledPreviewReport {
        preview_ready,
        blocked: !preview_ready,
        contract_shape,
        source_session_id,
        adapter_path: "controlled_adapter_preview".into(),
        reply,
        reasoning_trace,
        tool_calls: Vec::new(),
        run_id: Some(preview_run_id),
        allow_writes,
        max_tool_calls,
        default_chat_path_unchanged,
        chat_message_saved,
        agent_run_recorded: true,
        implementation_ready: readiness.implementation_ready,
        warnings,
        blocking_reasons,
        metadata_safe_summary,
    })
}

#[tauri::command]
pub async fn record_default_chat_adapter_controlled_preview_review_decision(
    input: DefaultChatAdapterControlledPreviewReviewDecisionInput,
    state: State<'_, Arc<AppState>>,
) -> Result<DefaultChatAdapterControlledPreviewReviewDecisionResult, String> {
    record_default_chat_adapter_controlled_preview_review_decision_with_state(
        input,
        &state.inner().clone(),
    )
    .await
}

pub(crate) async fn record_default_chat_adapter_controlled_preview_review_decision_with_state(
    input: DefaultChatAdapterControlledPreviewReviewDecisionInput,
    state: &Arc<AppState>,
) -> Result<DefaultChatAdapterControlledPreviewReviewDecisionResult, String> {
    let preview_run_id = safe_internal_id(&input.preview_run_id, "previewRunId")?;
    let decision_kind = safe_enum_value(
        &input.decision_kind,
        "decisionKind",
        &["approve", "reject", "request_rework"],
    )?;
    let created_at = chrono::Utc::now().to_rfc3339();
    let run =
        load_default_chat_adapter_controlled_preview_review_run(state, &preview_run_id).await?;
    let readiness = default_chat_adapter_controlled_preview_review_readiness(run.as_ref())?;
    let mut blocking_reasons = readiness.blocking_reasons.clone();

    if decision_kind == "approve" {
        if readiness.contract_shape != "send_message_compatible" {
            push_unique_string(
                &mut blocking_reasons,
                "preview_run_contract_shape_not_send_message_compatible".into(),
            );
        }
        if !readiness.preview_ready {
            push_unique_string(
                &mut blocking_reasons,
                "preview_run_not_ready_for_approval".into(),
            );
        }
    }

    if !blocking_reasons.is_empty() {
        return Ok(DefaultChatAdapterControlledPreviewReviewDecisionResult {
            recorded: false,
            evidence_id: None,
            preview_run_id,
            decision_kind,
            contract_shape: readiness.contract_shape,
            preview_summary_digest: readiness.digest,
            created_at,
            blocking_reasons,
        });
    }

    let reviewer_note_metadata =
        metadata_safe_reviewer_note_fields(input.optional_reviewer_note.as_deref());
    let mut evidence_draft = EvidenceDraft::new(
        EvidenceType::RuntimeBehavior,
        DEFAULT_CHAT_ADAPTER_CONTROLLED_PREVIEW_REVIEW_DECISION_EVIDENCE_PATH,
        1.0,
        RiskLevel::Low,
        EvidencePrivacyLevel::Internal,
    );
    evidence_draft.run_metadata = json!({
        "previewRunId": preview_run_id.clone(),
        "decisionKind": decision_kind.clone(),
        "contractShape": readiness.contract_shape.clone(),
        "previewSummaryDigest": readiness.digest.clone(),
        "reviewerNoteChecksum": reviewer_note_metadata.checksum,
        "reviewerNoteLength": reviewer_note_metadata.length,
        "reviewerNoteCategory": reviewer_note_metadata.category,
        "createdAt": created_at.clone(),
    });

    let record = {
        let store = state.evidence_store.lock().await;
        store.create_evidence(evidence_draft).map_err(|e| {
            format!("failed to record default Chat adapter controlled preview review evidence: {e}")
        })?
    };

    Ok(DefaultChatAdapterControlledPreviewReviewDecisionResult {
        recorded: true,
        evidence_id: Some(record.id),
        preview_run_id,
        decision_kind,
        contract_shape: readiness.contract_shape,
        preview_summary_digest: readiness.digest,
        created_at,
        blocking_reasons,
    })
}

#[tauri::command]
pub async fn get_default_chat_adapter_controlled_preview_review_summary(
    state: State<'_, Arc<AppState>>,
) -> Result<DefaultChatAdapterControlledPreviewReviewSummary, String> {
    get_default_chat_adapter_controlled_preview_review_summary_with_state(&state.inner().clone())
        .await
}

pub(crate) async fn get_default_chat_adapter_controlled_preview_review_summary_with_state(
    state: &Arc<AppState>,
) -> Result<DefaultChatAdapterControlledPreviewReviewSummary, String> {
    let records = {
        let store = state.evidence_store.lock().await;
        store
            .query(EvidenceQuery {
                affected_path: Some(
                    DEFAULT_CHAT_ADAPTER_CONTROLLED_PREVIEW_REVIEW_DECISION_EVIDENCE_PATH.into(),
                ),
                evidence_type: Some(EvidenceType::RuntimeBehavior),
                ..EvidenceQuery::default()
            })
            .map_err(|e| {
                format!(
                    "failed to read default Chat adapter controlled preview review evidence: {e}"
                )
            })?
    };
    let records = records
        .into_iter()
        .filter(default_chat_adapter_controlled_preview_review_decision_evidence_is_metadata_safe)
        .collect::<Vec<_>>();

    let approved_count = records
        .iter()
        .filter(|record| {
            default_chat_adapter_controlled_preview_review_decision_kind(record) == Some("approve")
        })
        .count();
    let reject_or_rework_count = records
        .iter()
        .filter(|record| {
            matches!(
                default_chat_adapter_controlled_preview_review_decision_kind(record),
                Some("reject" | "request_rework")
            )
        })
        .count();
    let latest_decision = records
        .first()
        .and_then(default_chat_adapter_controlled_preview_review_latest_decision);
    let latest_timestamp = latest_decision
        .as_ref()
        .map(|decision| decision.created_at.clone());
    let latest_decision_present = latest_decision.is_some();
    let blocking_reasons = if latest_decision_present {
        Vec::new()
    } else {
        vec!["controlled_preview_review_decision_missing".into()]
    };
    let blocking_reason_count = blocking_reasons.len();

    Ok(DefaultChatAdapterControlledPreviewReviewSummary {
        latest_decision,
        approved_count,
        reject_or_rework_count,
        latest_timestamp,
        blocking_reasons,
        metadata_safe_summary: json!({
            "controlledPreviewReview": "default_chat_adapter",
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
            "notAutomaticMigration": true,
        }),
    })
}

async fn default_chat_adapter_controlled_preview_review_records(
    state: &Arc<AppState>,
) -> Result<Vec<openlife_core::agent::EvidenceRecord>, String> {
    let records = {
        let store = state.evidence_store.lock().await;
        store
            .query(EvidenceQuery {
                affected_path: Some(
                    DEFAULT_CHAT_ADAPTER_CONTROLLED_PREVIEW_REVIEW_DECISION_EVIDENCE_PATH.into(),
                ),
                evidence_type: Some(EvidenceType::RuntimeBehavior),
                ..EvidenceQuery::default()
            })
            .map_err(|e| {
                format!(
                    "failed to read default Chat adapter controlled preview review evidence: {e}"
                )
            })?
    };
    Ok(records
        .into_iter()
        .filter(default_chat_adapter_controlled_preview_review_decision_evidence_is_metadata_safe)
        .collect())
}

#[tauri::command]
pub async fn check_default_chat_adapter_controlled_preview_approval_readiness(
    input: DefaultChatAdapterControlledPreviewApprovalReadinessInput,
    state: State<'_, Arc<AppState>>,
) -> Result<DefaultChatAdapterControlledPreviewApprovalReadinessReport, String> {
    check_default_chat_adapter_controlled_preview_approval_readiness_with_state(
        input,
        &state.inner().clone(),
    )
    .await
}

pub(crate) async fn check_default_chat_adapter_controlled_preview_approval_readiness_with_state(
    input: DefaultChatAdapterControlledPreviewApprovalReadinessInput,
    state: &Arc<AppState>,
) -> Result<DefaultChatAdapterControlledPreviewApprovalReadinessReport, String> {
    let source_session_id = safe_internal_id(&input.source_session_id, "sourceSessionId")?;
    let required_approved_previews = input.required_approved_previews.unwrap_or(1).max(1);
    let implementation_readiness = check_default_chat_adapter_implementation_readiness_with_state(
        DefaultChatAdapterImplementationReadinessInput {
            source_session_id,
            message: input.message,
            required_approved_candidates: input.required_approved_candidates,
            required_promotions: input.required_promotions,
        },
        state,
    )
    .await?;
    let review_summary =
        get_default_chat_adapter_controlled_preview_review_summary_with_state(state).await?;
    let latest_decision = review_summary.latest_decision.clone();
    let implementation_readiness_ready = implementation_readiness.implementation_ready;
    let approved_preview_count = review_summary.approved_count;
    let mut blocking_reasons = Vec::new();

    for reason in &implementation_readiness.blocking_reasons {
        push_unique_string(&mut blocking_reasons, reason.clone());
    }
    for reason in &review_summary.blocking_reasons {
        push_unique_string(&mut blocking_reasons, reason.clone());
    }
    if !implementation_readiness_ready {
        push_unique_string(
            &mut blocking_reasons,
            "implementation_readiness_not_ready".into(),
        );
    }
    if approved_preview_count < required_approved_previews {
        push_unique_string(
            &mut blocking_reasons,
            "controlled_preview_review_approved_count_below_required".into(),
        );
    }

    let mut preview_review_approved = false;
    let mut preview_digest_matched = false;
    match latest_decision.as_ref() {
        Some(decision) if decision.decision_kind == "approve" => {
            preview_review_approved = true;
        }
        Some(_) => {
            push_unique_string(
                &mut blocking_reasons,
                "latest_controlled_preview_review_not_approve".into(),
            );
        }
        None => {
            push_unique_string(
                &mut blocking_reasons,
                "controlled_preview_review_approval_missing".into(),
            );
        }
    }

    let review_records = default_chat_adapter_controlled_preview_review_records(state).await?;
    let approved_decisions = review_records
        .iter()
        .filter(|record| {
            default_chat_adapter_controlled_preview_review_decision_kind(record) == Some("approve")
        })
        .filter_map(default_chat_adapter_controlled_preview_review_latest_decision)
        .collect::<Vec<_>>();

    let mut verified_preview_run_ids = Vec::new();
    for decision in approved_decisions {
        if verified_preview_run_ids.len() >= required_approved_previews {
            break;
        }
        let preview_run_id = safe_internal_id(&decision.preview_run_id, "previewRunId")?;
        let run =
            load_default_chat_adapter_controlled_preview_review_run(state, &preview_run_id).await?;
        let readiness = default_chat_adapter_controlled_preview_review_readiness(run.as_ref())?;
        let digest_matched = readiness.digest == decision.preview_summary_digest;
        if latest_decision
            .as_ref()
            .is_some_and(|latest| latest.preview_run_id == decision.preview_run_id)
        {
            preview_digest_matched = digest_matched;
            if !digest_matched {
                push_unique_string(
                    &mut blocking_reasons,
                    "controlled_preview_review_digest_mismatch".into(),
                );
            }
        }
        for reason in &readiness.blocking_reasons {
            push_unique_string(&mut blocking_reasons, reason.clone());
        }
        if readiness.contract_shape != "send_message_compatible" {
            push_unique_string(
                &mut blocking_reasons,
                "preview_run_contract_shape_not_send_message_compatible".into(),
            );
        }
        if !readiness.preview_ready {
            push_unique_string(
                &mut blocking_reasons,
                "preview_run_not_ready_for_approval_readiness".into(),
            );
        }
        if digest_matched
            && readiness.blocking_reasons.is_empty()
            && readiness.contract_shape == "send_message_compatible"
            && readiness.preview_ready
        {
            verified_preview_run_ids.push(preview_run_id);
        }
    }

    if preview_review_approved && !preview_digest_matched {
        push_unique_string(
            &mut blocking_reasons,
            "controlled_preview_review_digest_mismatch".into(),
        );
    }
    if verified_preview_run_ids.len() < required_approved_previews {
        push_unique_string(
            &mut blocking_reasons,
            "controlled_preview_verified_approval_count_below_required".into(),
        );
    }
    if !implementation_readiness.default_chat_unchanged {
        push_unique_string(&mut blocking_reasons, "default_chat_changed".into());
    }
    if implementation_readiness.controlled_adapter_enabled {
        push_unique_string(&mut blocking_reasons, "controlled_adapter_enabled".into());
    }
    if implementation_readiness.automatic_migration_enabled {
        push_unique_string(&mut blocking_reasons, "automatic_migration_enabled".into());
    }

    let default_send_path = implementation_readiness.default_send_path.clone();
    let start_stream_path = implementation_readiness.start_stream_path.clone();
    let default_chat_unchanged = implementation_readiness.default_chat_unchanged;
    let controlled_adapter_enabled = implementation_readiness.controlled_adapter_enabled;
    let automatic_migration_enabled = implementation_readiness.automatic_migration_enabled;
    let ready = implementation_readiness_ready
        && preview_review_approved
        && preview_digest_matched
        && verified_preview_run_ids.len() >= required_approved_previews
        && default_chat_unchanged
        && !controlled_adapter_enabled
        && !automatic_migration_enabled
        && default_send_path == "legacy_stream"
        && start_stream_path == "legacy_stream"
        && blocking_reasons.is_empty();
    let latest_decision_kind = latest_decision
        .as_ref()
        .map(|decision| decision.decision_kind.clone())
        .unwrap_or_else(|| "none".into());
    let blocking_reason_count = blocking_reasons.len();

    Ok(DefaultChatAdapterControlledPreviewApprovalReadinessReport {
        ready,
        required_approved_previews,
        approved_preview_count,
        latest_decision,
        verified_preview_run_ids: verified_preview_run_ids.clone(),
        implementation_readiness_ready,
        preview_review_approved,
        preview_digest_matched,
        default_chat_unchanged,
        controlled_adapter_enabled,
        automatic_migration_enabled,
        default_send_path: default_send_path.clone(),
        start_stream_path: start_stream_path.clone(),
        blocking_reasons,
        metadata_safe_summary: json!({
            "controlledPreviewApprovalReadiness": "default_chat_adapter",
            "metadataSafe": true,
            "readOnly": true,
            "ready": ready,
            "requiredApprovedPreviews": required_approved_previews,
            "approvedPreviewCount": approved_preview_count,
            "verifiedPreviewRunCount": verified_preview_run_ids.len(),
            "implementationReadinessReady": implementation_readiness_ready,
            "previewReviewApproved": preview_review_approved,
            "previewDigestMatched": preview_digest_matched,
            "defaultChatUnchanged": default_chat_unchanged,
            "controlledAdapterEnabled": controlled_adapter_enabled,
            "automaticMigrationEnabled": automatic_migration_enabled,
            "defaultSendPath": default_send_path,
            "startStreamPath": start_stream_path,
            "latestDecisionKind": latest_decision_kind,
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
            "agentRunStorage": "read_only",
            "runtimeCallStorage": "none",
            "modelCallStorage": "none",
            "externalWriteStorage": "none",
            "transcriptStorage": "none",
            "notAutomaticMigration": true,
        }),
    })
}

#[tauri::command]
pub async fn draft_default_chat_adapter_cutover_implementation_plan(
    input: DefaultChatAdapterCutoverImplementationPlanInput,
    state: State<'_, Arc<AppState>>,
) -> Result<DefaultChatAdapterCutoverImplementationPlanDraft, String> {
    draft_default_chat_adapter_cutover_implementation_plan_with_state(input, &state.inner().clone())
        .await
}

pub(crate) async fn draft_default_chat_adapter_cutover_implementation_plan_with_state(
    input: DefaultChatAdapterCutoverImplementationPlanInput,
    state: &Arc<AppState>,
) -> Result<DefaultChatAdapterCutoverImplementationPlanDraft, String> {
    let source_session_id = safe_internal_id(&input.source_session_id, "sourceSessionId")?;
    let input_message_length = input.message.chars().count();
    let input_message_hash = sha256_metadata_checksum(&input.message);
    let approval_readiness =
        check_default_chat_adapter_controlled_preview_approval_readiness_with_state(
            DefaultChatAdapterControlledPreviewApprovalReadinessInput {
                source_session_id: source_session_id.clone(),
                message: input.message,
                required_approved_previews: input.required_approved_previews,
                required_approved_candidates: input.required_approved_candidates,
                required_promotions: input.required_promotions,
            },
            state,
        )
        .await?;

    default_chat_adapter_cutover_implementation_plan_from_readiness(
        source_session_id,
        input_message_length,
        input_message_hash,
        approval_readiness,
    )
}

fn default_chat_adapter_cutover_implementation_plan_from_readiness(
    source_session_id: String,
    input_message_length: usize,
    input_message_hash: String,
    approval_readiness: DefaultChatAdapterControlledPreviewApprovalReadinessReport,
) -> Result<DefaultChatAdapterCutoverImplementationPlanDraft, String> {
    let mut blocking_reasons = Vec::new();
    if !approval_readiness.ready {
        push_unique_string(
            &mut blocking_reasons,
            "controlled_preview_approval_readiness_not_ready".into(),
        );
    }
    for reason in &approval_readiness.blocking_reasons {
        push_unique_string(&mut blocking_reasons, reason.clone());
    }
    if approval_readiness.controlled_adapter_enabled {
        push_unique_string(&mut blocking_reasons, "controlled_adapter_enabled".into());
    }
    if approval_readiness.automatic_migration_enabled {
        push_unique_string(&mut blocking_reasons, "automatic_migration_enabled".into());
    }
    if !approval_readiness.default_chat_unchanged {
        push_unique_string(&mut blocking_reasons, "default_chat_changed".into());
    }
    if approval_readiness.default_send_path != "legacy_stream" {
        push_unique_string(
            &mut blocking_reasons,
            "default_send_path_not_legacy_stream".into(),
        );
    }
    if approval_readiness.start_stream_path != "legacy_stream" {
        push_unique_string(
            &mut blocking_reasons,
            "start_stream_path_not_legacy_stream".into(),
        );
    }

    let draft_ready = approval_readiness.ready
        && approval_readiness.default_chat_unchanged
        && !approval_readiness.controlled_adapter_enabled
        && !approval_readiness.automatic_migration_enabled
        && approval_readiness.default_send_path == "legacy_stream"
        && approval_readiness.start_stream_path == "legacy_stream"
        && blocking_reasons.is_empty();
    let plan_sections = if draft_ready {
        default_chat_adapter_cutover_implementation_plan_sections()
    } else {
        Vec::new()
    };
    let stable_plan_digest = if draft_ready {
        Some(default_chat_adapter_cutover_implementation_plan_digest(
            &source_session_id,
            input_message_length,
            &input_message_hash,
            &approval_readiness,
            &plan_sections,
        )?)
    } else {
        None
    };
    let latest_decision_kind = approval_readiness
        .latest_decision
        .as_ref()
        .map(|decision| decision.decision_kind.clone())
        .unwrap_or_else(|| "none".into());
    let latest_preview_run_id = approval_readiness
        .latest_decision
        .as_ref()
        .map(|decision| decision.preview_run_id.clone());
    let plan_section_count = plan_sections.len();
    let blocking_reason_count = blocking_reasons.len();
    let metadata_safe_summary = json!({
        "cutoverImplementationPlan": "default_chat_adapter",
        "metadataSafe": true,
        "readOnly": true,
        "humanReviewOnly": true,
        "draftReady": draft_ready,
        "manualReviewRequired": true,
        "notAutomaticMigration": true,
        "requiresSeparateImplementation": true,
        "requiresSeparateCutoverReview": true,
        "controlledPreviewApprovalReady": draft_ready,
        "requiredApprovedPreviews": approval_readiness.required_approved_previews,
        "approvedPreviewCount": approval_readiness.approved_preview_count,
        "verifiedPreviewRunCount": approval_readiness.verified_preview_run_ids.len(),
        "latestDecisionKind": latest_decision_kind,
        "latestPreviewRunId": latest_preview_run_id,
        "defaultChatUnchanged": approval_readiness.default_chat_unchanged,
        "controlledAdapterEnabled": approval_readiness.controlled_adapter_enabled,
        "automaticMigrationEnabled": approval_readiness.automatic_migration_enabled,
        "defaultSendPath": approval_readiness.default_send_path,
        "startStreamPath": approval_readiness.start_stream_path,
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
    });

    Ok(DefaultChatAdapterCutoverImplementationPlanDraft {
        draft_ready,
        controlled_preview_approval_readiness: approval_readiness,
        manual_review_required: true,
        not_automatic_migration: true,
        requires_separate_implementation: true,
        requires_separate_cutover_review: true,
        source_session_id,
        input_message_length,
        input_message_hash,
        stable_plan_digest,
        plan_sections,
        blocking_reasons,
        metadata_safe_summary,
    })
}

fn default_chat_adapter_cutover_implementation_plan_sections(
) -> Vec<DefaultChatAdapterCutoverImplementationPlanSection> {
    vec![
        DefaultChatAdapterCutoverImplementationPlanSection {
            section_key: "implementationScope".into(),
            title: "Implementation Scope".into(),
            items: vec![
                "Prepare a separately reviewed adapter cutover implementation that can be evaluated after W45 approval readiness remains valid.".into(),
                "Keep this draft as planning material only; it does not add, enable, or switch a default Chat adapter.".into(),
                "Limit future implementation work to the default Chat adapter boundary, fallback, observability, tests, and rollback.".into(),
            ],
        },
        DefaultChatAdapterCutoverImplementationPlanSection {
            section_key: "adapterContractRequirements".into(),
            title: "Adapter Contract Requirements".into(),
            items: vec![
                "Preserve send_message-compatible response shape, error shape, session semantics, and stream completion semantics.".into(),
                "Require approved W43 controlled preview evidence to remain completed, write-disabled, zero-tool, metadata-safe, and side-effect-free.".into(),
                "Reject any implementation that stores raw prompt, raw assistant output, tool payload, Proposal, Memory, LifeModel patch, Evidence, MCP audit, or Chat messages from planning checks.".into(),
            ],
        },
        DefaultChatAdapterCutoverImplementationPlanSection {
            section_key: "routingChangeBoundary".into(),
            title: "Routing Change Boundary".into(),
            items: vec![
                "Default Send and start_stream_message must remain on legacy_stream until a later implementation is separately reviewed.".into(),
                "Any future routing flag must default disabled and keep an explicit stable fallback to legacy_stream.".into(),
                "The W46 command itself must never be called by ordinary Chat send or streaming paths.".into(),
            ],
        },
        DefaultChatAdapterCutoverImplementationPlanSection {
            section_key: "safetyPreconditions".into(),
            title: "Safety Preconditions".into(),
            items: vec![
                "W42 implementation readiness and W45 controlled preview approval readiness must remain ready at implementation review time.".into(),
                "Controlled adapter enabled and automatic migration enabled must both remain false during planning.".into(),
                "Default send and stream paths must both remain legacy_stream while this plan is drafted.".into(),
            ],
        },
        DefaultChatAdapterCutoverImplementationPlanSection {
            section_key: "fallbackPlan".into(),
            title: "Fallback Plan".into(),
            items: vec![
                "Keep ordinary legacy_stream Chat as the fallback for blocked readiness, adapter errors, unsupported contract shapes, or missing review evidence.".into(),
                "Do not retry through controlled preview or promote preview output from this planning command.".into(),
                "Surface metadata-safe blockers in Settings and leave the user on the existing Chat path.".into(),
            ],
        },
        DefaultChatAdapterCutoverImplementationPlanSection {
            section_key: "rollbackPlan".into(),
            title: "Rollback Plan".into(),
            items: vec![
                "A future cutover implementation must be reversible without rewriting Chat history or synthesizing replacement evidence.".into(),
                "Rollback must restore legacy_stream routing and keep existing ordinary Chat messages untouched.".into(),
                "Rollback must not patch LifeModel, Memory, Proposal, Evidence, MCP audit, or external tools.".into(),
            ],
        },
        DefaultChatAdapterCutoverImplementationPlanSection {
            section_key: "observabilityPlan".into(),
            title: "Observability Plan".into(),
            items: vec![
                "Track only metadata-safe readiness, routing, fallback, error, latency, and blocker counters.".into(),
                "Expose approved preview run ids, digest match status, and section digest for human review without transcript content.".into(),
                "Keep observability separate from Chat persistence, Evidence writes, model calls, runtime calls, MCP audit, and tool payloads.".into(),
            ],
        },
        DefaultChatAdapterCutoverImplementationPlanSection {
            section_key: "testPlan".into(),
            title: "Test Plan".into(),
            items: vec![
                "Verify W45 blocked returns draftReady=false, no plan sections, and propagated blockers.".into(),
                "Verify W45 ready returns all plan sections, stable digest, and fixed human-review-only flags.".into(),
                "Verify side-effect counts remain unchanged for AgentRun, Evidence, Proposal, Memory, LifeModel patch, MCP audit, and Chat messages.".into(),
                "Verify serialized output contains no raw prompt, assistant output, tool payload, reviewer note, or private transcript.".into(),
                "Verify default Send, send_message, and start_stream_message do not call the W46 command.".into(),
            ],
        },
        DefaultChatAdapterCutoverImplementationPlanSection {
            section_key: "explicitNonGoals".into(),
            title: "Explicit Non Goals".into(),
            items: vec![
                "Do not migrate default Chat.".into(),
                "Do not enable an adapter flag or automatic migration.".into(),
                "Do not run controlled preview, runtime, tools, model calls, proposal apply, or external writes.".into(),
                "Do not persist raw message text, assistant output, tool payloads, reviewer notes, Chat messages, Evidence, AgentRuns, Proposals, Memory, or LifeModel patches.".into(),
            ],
        },
    ]
}

fn default_chat_adapter_cutover_implementation_plan_digest(
    source_session_id: &str,
    input_message_length: usize,
    input_message_hash: &str,
    approval_readiness: &DefaultChatAdapterControlledPreviewApprovalReadinessReport,
    plan_sections: &[DefaultChatAdapterCutoverImplementationPlanSection],
) -> Result<String, String> {
    metadata_hash_for_serializable(&json!({
        "sourceSessionId": source_session_id,
        "inputMessageLength": input_message_length,
        "inputMessageHash": input_message_hash,
        "manualReviewRequired": true,
        "notAutomaticMigration": true,
        "requiresSeparateImplementation": true,
        "requiresSeparateCutoverReview": true,
        "readiness": {
            "ready": approval_readiness.ready,
            "requiredApprovedPreviews": approval_readiness.required_approved_previews,
            "approvedPreviewCount": approval_readiness.approved_preview_count,
            "verifiedPreviewRunIds": approval_readiness.verified_preview_run_ids,
            "implementationReadinessReady": approval_readiness.implementation_readiness_ready,
            "previewReviewApproved": approval_readiness.preview_review_approved,
            "previewDigestMatched": approval_readiness.preview_digest_matched,
            "defaultChatUnchanged": approval_readiness.default_chat_unchanged,
            "controlledAdapterEnabled": approval_readiness.controlled_adapter_enabled,
            "automaticMigrationEnabled": approval_readiness.automatic_migration_enabled,
            "defaultSendPath": approval_readiness.default_send_path,
            "startStreamPath": approval_readiness.start_stream_path,
            "latestDecisionKind": approval_readiness
                .latest_decision
                .as_ref()
                .map(|decision| decision.decision_kind.as_str())
                .unwrap_or("none"),
            "latestPreviewRunId": approval_readiness
                .latest_decision
                .as_ref()
                .map(|decision| decision.preview_run_id.as_str())
                .unwrap_or("none"),
            "latestPreviewSummaryDigest": approval_readiness
                .latest_decision
                .as_ref()
                .map(|decision| decision.preview_summary_digest.as_str())
                .unwrap_or("none"),
        },
        "planSections": plan_sections,
    }))
}

#[tauri::command]
pub async fn record_default_chat_adapter_cutover_plan_review_decision(
    input: DefaultChatAdapterCutoverPlanReviewDecisionInput,
    state: State<'_, Arc<AppState>>,
) -> Result<DefaultChatAdapterCutoverPlanReviewDecisionResult, String> {
    record_default_chat_adapter_cutover_plan_review_decision_with_state(
        input,
        &state.inner().clone(),
    )
    .await
}

pub(crate) async fn record_default_chat_adapter_cutover_plan_review_decision_with_state(
    input: DefaultChatAdapterCutoverPlanReviewDecisionInput,
    state: &Arc<AppState>,
) -> Result<DefaultChatAdapterCutoverPlanReviewDecisionResult, String> {
    let decision_kind = safe_enum_value(
        &input.decision_kind,
        "decisionKind",
        &["approve", "reject", "request_rework"],
    )?;
    let source_session_id = safe_internal_id(&input.source_session_id, "sourceSessionId")?;
    let draft = draft_default_chat_adapter_cutover_implementation_plan_with_state(
        DefaultChatAdapterCutoverImplementationPlanInput {
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
            "cutover_implementation_plan_not_ready".into(),
        );
        return Ok(DefaultChatAdapterCutoverPlanReviewDecisionResult {
            recorded: false,
            evidence_id: None,
            decision_kind,
            source_session_id,
            draft_ready: false,
            cutover_plan_digest: draft.stable_plan_digest,
            plan_section_count: draft.plan_sections.len(),
            created_at,
            blocking_reasons,
        });
    }

    let reviewer_note_metadata =
        metadata_safe_reviewer_note_fields(input.optional_reviewer_note.as_deref());
    let mut evidence_draft = EvidenceDraft::new(
        EvidenceType::RuntimeBehavior,
        DEFAULT_CHAT_ADAPTER_CUTOVER_PLAN_REVIEW_DECISION_EVIDENCE_PATH,
        1.0,
        RiskLevel::Low,
        EvidencePrivacyLevel::Internal,
    );
    evidence_draft.run_metadata = json!({
        "evidenceKind": "default_chat_adapter_cutover_plan_review_decision",
        "decisionKind": decision_kind.clone(),
        "sourceSessionId": source_session_id.clone(),
        "draftReady": draft.draft_ready,
        "w45Ready": draft.controlled_preview_approval_readiness.ready,
        "cutoverPlanDigest": draft.stable_plan_digest.clone(),
        "planSectionCount": draft.plan_sections.len(),
        "reviewerNoteChecksum": reviewer_note_metadata.checksum,
        "reviewerNoteLength": reviewer_note_metadata.length,
        "reviewerNoteCategory": reviewer_note_metadata.category,
        "createdAt": created_at.clone(),
    });

    let record = {
        let store = state.evidence_store.lock().await;
        store.create_evidence(evidence_draft).map_err(|e| {
            format!("failed to record default Chat adapter cutover plan review evidence: {e}")
        })?
    };

    Ok(DefaultChatAdapterCutoverPlanReviewDecisionResult {
        recorded: true,
        evidence_id: Some(record.id),
        decision_kind,
        source_session_id,
        draft_ready: draft.draft_ready,
        cutover_plan_digest: draft.stable_plan_digest,
        plan_section_count: draft.plan_sections.len(),
        created_at,
        blocking_reasons,
    })
}

#[tauri::command]
pub async fn get_default_chat_adapter_cutover_plan_review_summary(
    state: State<'_, Arc<AppState>>,
) -> Result<DefaultChatAdapterCutoverPlanReviewSummary, String> {
    get_default_chat_adapter_cutover_plan_review_summary_with_state(&state.inner().clone()).await
}

pub(crate) async fn get_default_chat_adapter_cutover_plan_review_summary_with_state(
    state: &Arc<AppState>,
) -> Result<DefaultChatAdapterCutoverPlanReviewSummary, String> {
    let records = default_chat_adapter_cutover_plan_review_records(state).await?;
    let approved_count = records
        .iter()
        .filter(|record| {
            default_chat_adapter_cutover_plan_review_decision_kind(record) == Some("approve")
        })
        .count();
    let rejected_count = records
        .iter()
        .filter(|record| {
            default_chat_adapter_cutover_plan_review_decision_kind(record) == Some("reject")
        })
        .count();
    let request_rework_count = records
        .iter()
        .filter(|record| {
            default_chat_adapter_cutover_plan_review_decision_kind(record) == Some("request_rework")
        })
        .count();
    let latest_decision = records
        .first()
        .and_then(default_chat_adapter_cutover_plan_review_latest_decision);
    let latest_approved_plan_digest = records
        .iter()
        .filter(|record| {
            default_chat_adapter_cutover_plan_review_decision_kind(record) == Some("approve")
        })
        .find_map(|record| {
            record
                .run_metadata
                .get("cutoverPlanDigest")
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
        vec!["cutover_plan_review_decision_missing".into()]
    };
    let blocking_reason_count = blocking_reasons.len();

    Ok(DefaultChatAdapterCutoverPlanReviewSummary {
        latest_decision,
        approved_count,
        rejected_count,
        request_rework_count,
        latest_approved_plan_digest,
        latest_timestamp,
        blocking_reasons,
        metadata_safe_summary: json!({
            "cutoverPlanReview": "default_chat_adapter",
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
pub async fn check_default_chat_adapter_cutover_plan_approval_readiness(
    input: DefaultChatAdapterCutoverPlanApprovalReadinessInput,
    state: State<'_, Arc<AppState>>,
) -> Result<DefaultChatAdapterCutoverPlanApprovalReadinessReport, String> {
    check_default_chat_adapter_cutover_plan_approval_readiness_with_state(
        input,
        &state.inner().clone(),
    )
    .await
}

pub(crate) async fn check_default_chat_adapter_cutover_plan_approval_readiness_with_state(
    input: DefaultChatAdapterCutoverPlanApprovalReadinessInput,
    state: &Arc<AppState>,
) -> Result<DefaultChatAdapterCutoverPlanApprovalReadinessReport, String> {
    let source_session_id = safe_internal_id(&input.source_session_id, "sourceSessionId")?;
    let draft = draft_default_chat_adapter_cutover_implementation_plan_with_state(
        DefaultChatAdapterCutoverImplementationPlanInput {
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
        get_default_chat_adapter_cutover_plan_review_summary_with_state(state).await?;
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

    let w45_ready = draft.controlled_preview_approval_readiness.ready;
    let default_chat_unchanged = draft
        .controlled_preview_approval_readiness
        .default_chat_unchanged;
    let controlled_adapter_enabled = draft
        .controlled_preview_approval_readiness
        .controlled_adapter_enabled;
    let automatic_migration_enabled = draft
        .controlled_preview_approval_readiness
        .automatic_migration_enabled;
    let default_send_path = draft
        .controlled_preview_approval_readiness
        .default_send_path
        .clone();
    let start_stream_path = draft
        .controlled_preview_approval_readiness
        .start_stream_path
        .clone();

    if !draft.draft_ready {
        push_unique_string(
            &mut blocking_reasons,
            "cutover_implementation_plan_not_ready".into(),
        );
    }
    if !w45_ready {
        push_unique_string(
            &mut blocking_reasons,
            "controlled_preview_approval_readiness_not_ready".into(),
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

    let mut cutover_plan_review_approved = false;
    let mut cutover_plan_digest_matched = false;
    match latest_decision.as_ref() {
        Some(decision) if decision.decision_kind == "approve" => {
            cutover_plan_review_approved = true;
            cutover_plan_digest_matched = current_plan_digest.is_some()
                && decision.cutover_plan_digest == current_plan_digest;
            if !decision.draft_ready {
                push_unique_string(
                    &mut blocking_reasons,
                    "approved_cutover_plan_draft_not_ready".into(),
                );
            }
            if !decision.w45_ready {
                push_unique_string(
                    &mut blocking_reasons,
                    "approved_cutover_plan_w45_not_ready".into(),
                );
            }
            if decision.plan_section_count == 0 {
                push_unique_string(
                    &mut blocking_reasons,
                    "approved_cutover_plan_sections_missing".into(),
                );
            }
            if !cutover_plan_digest_matched {
                push_unique_string(
                    &mut blocking_reasons,
                    "cutover_plan_review_digest_mismatch".into(),
                );
            }
        }
        Some(_) => {
            push_unique_string(
                &mut blocking_reasons,
                "latest_cutover_plan_review_not_approve".into(),
            );
        }
        None => {
            push_unique_string(
                &mut blocking_reasons,
                "cutover_plan_review_approval_missing".into(),
            );
        }
    }

    let latest_decision_kind = latest_decision
        .as_ref()
        .map(|decision| decision.decision_kind.clone())
        .unwrap_or_else(|| "none".into());
    let blocking_reason_count = blocking_reasons.len();
    let ready = draft.draft_ready
        && w45_ready
        && cutover_plan_review_approved
        && cutover_plan_digest_matched
        && default_chat_unchanged
        && !controlled_adapter_enabled
        && !automatic_migration_enabled
        && default_send_path == "legacy_stream"
        && start_stream_path == "legacy_stream"
        && blocking_reasons.is_empty();

    Ok(DefaultChatAdapterCutoverPlanApprovalReadinessReport {
        ready,
        draft_ready: draft.draft_ready,
        w45_ready,
        cutover_plan_review_approved,
        cutover_plan_digest_matched,
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
            "cutoverPlanApprovalReadiness": "default_chat_adapter",
            "metadataSafe": true,
            "readOnly": true,
            "ready": ready,
            "draftReady": draft.draft_ready,
            "w45Ready": w45_ready,
            "cutoverPlanReviewApproved": cutover_plan_review_approved,
            "cutoverPlanDigestMatched": cutover_plan_digest_matched,
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
            "agentRunStorage": "read_only",
            "runtimeCallStorage": "none",
            "modelCallStorage": "none",
            "externalWriteStorage": "none",
            "transcriptStorage": "none",
            "notAutomaticMigration": true,
        }),
    })
}

async fn default_chat_adapter_cutover_plan_review_records(
    state: &Arc<AppState>,
) -> Result<Vec<openlife_core::agent::EvidenceRecord>, String> {
    let records = {
        let store = state.evidence_store.lock().await;
        store
            .query(EvidenceQuery {
                affected_path: Some(
                    DEFAULT_CHAT_ADAPTER_CUTOVER_PLAN_REVIEW_DECISION_EVIDENCE_PATH.into(),
                ),
                evidence_type: Some(EvidenceType::RuntimeBehavior),
                ..EvidenceQuery::default()
            })
            .map_err(|e| {
                format!("failed to read default Chat adapter cutover plan review evidence: {e}")
            })?
    };
    Ok(records
        .into_iter()
        .filter(default_chat_adapter_cutover_plan_review_decision_evidence_is_metadata_safe)
        .collect())
}

struct DefaultChatAdapterControlledPreviewReviewReadiness {
    digest: String,
    contract_shape: String,
    preview_ready: bool,
    blocking_reasons: Vec<String>,
}

async fn load_default_chat_adapter_controlled_preview_review_run(
    state: &Arc<AppState>,
    preview_run_id: &str,
) -> Result<Option<AgentRun>, String> {
    let Some(store_arc) = state.agent_run_store.as_ref() else {
        return Ok(None);
    };
    let store = store_arc.lock().await;
    store
        .get_run(preview_run_id)
        .map_err(|e| format!("failed to read default Chat adapter controlled preview run: {e}"))
}

fn default_chat_adapter_controlled_preview_review_readiness(
    run: Option<&AgentRun>,
) -> Result<DefaultChatAdapterControlledPreviewReviewReadiness, String> {
    let Some(run) = run else {
        let summary = json!({
            "runFound": false,
            "metadataSafe": true,
            "sideEffectAuditReady": false,
        });
        return Ok(DefaultChatAdapterControlledPreviewReviewReadiness {
            digest: metadata_hash_for_serializable(&summary)?,
            contract_shape: "missing".into(),
            preview_ready: false,
            blocking_reasons: vec!["preview_run_missing".into()],
        });
    };

    let audit = run
        .reasoning_trace
        .as_ref()
        .and_then(|trace| trace.strategy_result.as_ref());
    let metadata_safe = audit_bool(audit, "metadataSafe").unwrap_or(false);
    let contract_shape = audit_string(audit, "contractShape")
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| "missing".into());
    let contract_shape_allowed = matches!(
        contract_shape.as_str(),
        "send_message_compatible" | "blocked" | "failed"
    );
    let preview_ready = audit_bool(audit, "previewReady").unwrap_or(false);
    let allow_writes = default_chat_adapter_controlled_preview_review_allow_writes(audit);
    let max_tool_calls = default_chat_adapter_controlled_preview_review_max_tool_calls(audit);
    let chat_message_saved = audit_bool(audit, "chatMessageSaved").unwrap_or(false);
    let storage = |key: &str| audit_string(audit, key).unwrap_or("missing");
    let declared_write_step_count =
        audit_u64_at(audit, &["writeControl", "declaredWriteStepCount"]).unwrap_or_default();
    let proposal_required_step_count =
        audit_u64_at(audit, &["writeControl", "proposalRequiredStepCount"]).unwrap_or_default();
    let proposal_id_count = audit_u64(audit, "proposalIdCount").unwrap_or_default();

    let side_effects_absent = run.user_input.is_none()
        && run.generated_proposals.is_empty()
        && run.actions.is_empty()
        && run.observations.is_empty()
        && run.tool_call_count == 0
        && proposal_id_count == 0
        && declared_write_step_count == 0
        && proposal_required_step_count == 0
        && allow_writes == Some(false)
        && max_tool_calls == Some(0)
        && !chat_message_saved;

    let summary = json!({
        "runFound": true,
        "previewRunId": run.id,
        "reasoningStrategy": run.reasoning_strategy.as_deref().unwrap_or("missing"),
        "status": run.status.to_string(),
        "contractShape": contract_shape.clone(),
        "previewReady": preview_ready,
        "metadataSafe": metadata_safe,
        "allowWrites": allow_writes,
        "maxToolCalls": max_tool_calls,
        "chatMessageSaved": chat_message_saved,
        "contentStorage": storage("contentStorage"),
        "toolStorage": storage("toolStorage"),
        "chatHistoryStorage": storage("chatHistoryStorage"),
        "proposalStorage": storage("proposalStorage"),
        "lifeModelPatchStorage": storage("lifeModelPatchStorage"),
        "memoryStorage": storage("memoryStorage"),
        "evidenceStorage": storage("evidenceStorage"),
        "mcpAuditStorage": storage("mcpAuditStorage"),
        "externalWriteStorage": storage("externalWriteStorage"),
        "userInputStored": run.user_input.is_some(),
        "generatedProposalCount": run.generated_proposals.len(),
        "actionCount": run.actions.len(),
        "observationCount": run.observations.len(),
        "toolCallCount": run.tool_call_count,
        "proposalIdCount": proposal_id_count,
        "declaredWriteStepCount": declared_write_step_count,
        "proposalRequiredStepCount": proposal_required_step_count,
        "sideEffectsAbsent": side_effects_absent,
    });
    let mut blocking_reasons = Vec::new();

    if run.reasoning_strategy.as_deref() != Some("default_chat_adapter_controlled_preview") {
        push_unique_string(
            &mut blocking_reasons,
            "preview_run_strategy_mismatch".into(),
        );
    }
    if run.status != AgentRunStatus::Completed {
        push_unique_string(&mut blocking_reasons, "preview_run_not_completed".into());
    }
    if audit.is_none() {
        push_unique_string(&mut blocking_reasons, "preview_run_audit_missing".into());
    }
    if !contract_shape_allowed {
        push_unique_string(
            &mut blocking_reasons,
            "preview_run_contract_shape_invalid".into(),
        );
    }
    if !metadata_safe {
        push_unique_string(
            &mut blocking_reasons,
            "preview_run_metadata_not_safe".into(),
        );
    }
    if allow_writes != Some(false) {
        push_unique_string(
            &mut blocking_reasons,
            "preview_run_allow_writes_not_false".into(),
        );
    }
    if max_tool_calls != Some(0) {
        push_unique_string(
            &mut blocking_reasons,
            "preview_run_max_tool_calls_not_zero".into(),
        );
    }
    for (key, reason) in [
        ("contentStorage", "preview_run_content_storage_not_none"),
        ("toolStorage", "preview_run_tool_storage_not_none"),
        (
            "chatHistoryStorage",
            "preview_run_chat_history_storage_not_none",
        ),
        ("proposalStorage", "preview_run_proposal_storage_not_none"),
        (
            "lifeModelPatchStorage",
            "preview_run_life_model_patch_storage_not_none",
        ),
        ("memoryStorage", "preview_run_memory_storage_not_none"),
        ("evidenceStorage", "preview_run_evidence_storage_not_none"),
        ("mcpAuditStorage", "preview_run_mcp_audit_storage_not_none"),
        (
            "externalWriteStorage",
            "preview_run_external_write_storage_not_none",
        ),
    ] {
        if audit_string(audit, key) != Some("none") {
            push_unique_string(&mut blocking_reasons, reason.into());
        }
    }
    if chat_message_saved {
        push_unique_string(
            &mut blocking_reasons,
            "preview_run_chat_message_saved".into(),
        );
    }
    if run.user_input.is_some() {
        push_unique_string(
            &mut blocking_reasons,
            "preview_run_user_input_persisted".into(),
        );
    }
    if !run.generated_proposals.is_empty()
        || proposal_id_count > 0
        || proposal_required_step_count > 0
    {
        push_unique_string(
            &mut blocking_reasons,
            "preview_run_proposal_side_effects_present".into(),
        );
    }
    if !run.actions.is_empty()
        || !run.observations.is_empty()
        || run.tool_call_count > 0
        || declared_write_step_count > 0
    {
        push_unique_string(
            &mut blocking_reasons,
            "preview_run_external_write_side_effects_present".into(),
        );
    }

    Ok(DefaultChatAdapterControlledPreviewReviewReadiness {
        digest: metadata_hash_for_serializable(&summary)?,
        contract_shape,
        preview_ready,
        blocking_reasons,
    })
}

fn default_chat_adapter_controlled_preview_blocked_summary(
    readiness: &DefaultChatAdapterImplementationReadinessReport,
    blocking_reasons: &[String],
) -> Value {
    json!({
        "adapterPreview": "default_chat_adapter_controlled_preview",
        "metadataSafe": true,
        "nonDefault": true,
        "blockedBeforeRuntime": true,
        "implementationReady": readiness.implementation_ready,
        "contractShape": "blocked",
        "previewReady": false,
        "allowWrites": false,
        "maxToolCalls": 0,
        "defaultChatPathUnchanged": true,
        "chatMessageSaved": false,
        "agentRunRecorded": false,
        "defaultSendPath": readiness.default_send_path,
        "startStreamPath": readiness.start_stream_path,
        "notAutomaticMigration": true,
        "requiresHumanReview": true,
        "blockingReasonCount": blocking_reasons.len(),
        "contentStorage": "none",
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
    })
}

fn default_chat_adapter_controlled_preview_failed_summary(
    readiness: &DefaultChatAdapterImplementationReadinessReport,
    safe_error: &str,
) -> Value {
    json!({
        "adapterPreview": "default_chat_adapter_controlled_preview",
        "metadataSafe": true,
        "nonDefault": true,
        "blockedBeforeRuntime": false,
        "previewErrorCode": default_chat_adapter_controlled_preview_error_code(safe_error),
        "implementationReady": readiness.implementation_ready,
        "contractShape": "failed",
        "previewReady": false,
        "allowWrites": false,
        "maxToolCalls": 0,
        "defaultChatPathUnchanged": true,
        "chatMessageSaved": false,
        "agentRunRecorded": true,
        "defaultSendPath": readiness.default_send_path,
        "startStreamPath": readiness.start_stream_path,
        "notAutomaticMigration": true,
        "requiresHumanReview": true,
        "contentStorage": "none",
        "toolStorage": "none",
        "chatHistoryStorage": "none",
        "proposalStorage": "none",
        "lifeModelPatchStorage": "none",
        "memoryStorage": "none",
        "evidenceStorage": "none",
        "mcpAuditStorage": "none",
        "externalWriteStorage": "none",
        "transcriptStorage": "none",
    })
}

fn default_chat_adapter_controlled_preview_metadata_safe_summary(
    output: &MultiStrategyRuntimeOutput,
    readiness: &DefaultChatAdapterImplementationReadinessReport,
    contract_shape: &str,
    preview_ready: bool,
    output_digest: Option<&str>,
) -> Value {
    let metadata = &output.selection.metadata_safe_summary;
    let governance_decision_kind = output
        .selection
        .governance_decision
        .as_ref()
        .map(|decision| preview_governance_decision_kind(decision.kind))
        .unwrap_or("unknown");
    json!({
        "adapterPreview": "default_chat_adapter_controlled_preview",
        "metadataSafe": true,
        "nonDefault": true,
        "implementationReady": readiness.implementation_ready,
        "contractShape": contract_shape,
        "previewReady": preview_ready,
        "strategyKind": preview_strategy_kind(output.selection.kind),
        "payloadKind": preview_payload_kind(&output.payload),
        "governanceDecisionKind": governance_decision_kind,
        "taskKind": metadata.get("taskKind").and_then(Value::as_str).unwrap_or("unknown"),
        "reasonCode": metadata.get("reasonCode").and_then(Value::as_str).unwrap_or("unknown"),
        "riskLevel": metadata.get("riskLevel").and_then(Value::as_str).unwrap_or("unknown"),
        "hasHsPacket": metadata.get("hasHsPacket").and_then(Value::as_bool).unwrap_or(false),
        "planStepCount": preview_plan_step_count(&output.payload),
        "proposalIdCount": preview_proposal_ids(&output.payload).len(),
        "blocked": matches!(output.payload, MultiStrategyRuntimePayload::Blocked),
        "replyPresent": default_chat_adapter_controlled_preview_reply(output).is_some(),
        "outputDigestPresent": output_digest.is_some(),
        "allowWrites": false,
        "maxToolCalls": 0,
        "defaultChatPathUnchanged": true,
        "chatMessageSaved": false,
        "agentRunRecorded": true,
        "defaultSendPath": readiness.default_send_path,
        "startStreamPath": readiness.start_stream_path,
        "notAutomaticMigration": true,
        "requiresHumanReview": true,
        "contentStorage": "none",
        "toolStorage": "none",
        "chatHistoryStorage": "none",
        "proposalStorage": "none",
        "lifeModelPatchStorage": "none",
        "memoryStorage": "none",
        "evidenceStorage": "none",
        "mcpAuditStorage": "none",
        "externalWriteStorage": "none",
        "transcriptStorage": "none",
    })
}

fn default_chat_adapter_controlled_preview_audit_summary(
    output: &MultiStrategyRuntimeOutput,
    readiness: &DefaultChatAdapterImplementationReadinessReport,
    contract_shape: &str,
    preview_ready: bool,
    output_digest: Option<&str>,
    warnings: &[String],
) -> Value {
    let mut write_control = preview_write_control(&output.payload);
    if let Some(map) = write_control.as_object_mut() {
        map.insert("allowWrites".into(), Value::Bool(false));
    }
    let metadata = default_chat_adapter_controlled_preview_metadata_safe_summary(
        output,
        readiness,
        contract_shape,
        preview_ready,
        output_digest,
    );
    json!({
        "adapterPreview": "default_chat_adapter_controlled_preview",
        "strategyKind": metadata["strategyKind"],
        "payloadKind": metadata["payloadKind"],
        "contractShape": contract_shape,
        "previewReady": preview_ready,
        "governanceDecisionKind": metadata["governanceDecisionKind"],
        "taskKind": metadata["taskKind"],
        "reasonCode": metadata["reasonCode"],
        "riskLevel": metadata["riskLevel"],
        "hasHsPacket": metadata["hasHsPacket"],
        "planStepCount": metadata["planStepCount"],
        "planStepStatuses": preview_plan_step_statuses(&output.payload),
        "proposalIdCount": metadata["proposalIdCount"],
        "blocked": metadata["blocked"],
        "replyPresent": metadata["replyPresent"],
        "outputDigest": output_digest,
        "warnings": warnings,
        "metadataSafe": true,
        "nonDefault": true,
        "defaultChatPathUnchanged": true,
        "runtimeLimits": {
            "allowWrites": false,
            "maxToolCalls": 0
        },
        "contentStorage": "none",
        "toolStorage": "none",
        "chatHistoryStorage": "none",
        "proposalStorage": "none",
        "lifeModelPatchStorage": "none",
        "memoryStorage": "none",
        "evidenceStorage": "none",
        "mcpAuditStorage": "none",
        "externalWriteStorage": "none",
        "writeControl": write_control,
    })
}

fn default_chat_adapter_controlled_preview_reply(
    output: &MultiStrategyRuntimeOutput,
) -> Option<String> {
    match &output.payload {
        MultiStrategyRuntimePayload::ReAct(runtime_output)
            if !runtime_output.user_output.trim().is_empty() =>
        {
            Some(runtime_output.user_output.clone())
        }
        MultiStrategyRuntimePayload::ReAct(_)
        | MultiStrategyRuntimePayload::PlanExecute(_)
        | MultiStrategyRuntimePayload::Blocked => None,
    }
}

fn default_chat_adapter_controlled_preview_contract_shape(
    output: &MultiStrategyRuntimeOutput,
) -> &'static str {
    match &output.payload {
        MultiStrategyRuntimePayload::ReAct(runtime_output)
            if !runtime_output.user_output.trim().is_empty()
                && runtime_output.proposal_ids.is_empty() =>
        {
            "send_message_compatible"
        }
        MultiStrategyRuntimePayload::Blocked => "blocked",
        MultiStrategyRuntimePayload::ReAct(_) | MultiStrategyRuntimePayload::PlanExecute(_) => {
            "failed"
        }
    }
}

fn default_chat_adapter_controlled_preview_contract_blockers(
    output: &MultiStrategyRuntimeOutput,
) -> Vec<String> {
    let mut blocking_reasons = Vec::new();
    match &output.payload {
        MultiStrategyRuntimePayload::Blocked => {
            push_unique_string(&mut blocking_reasons, "preview_runtime_blocked".into());
        }
        MultiStrategyRuntimePayload::PlanExecute(_) => {
            push_unique_string(
                &mut blocking_reasons,
                "preview_runtime_returned_non_chat_payload".into(),
            );
        }
        MultiStrategyRuntimePayload::ReAct(runtime_output) => {
            if runtime_output.user_output.trim().is_empty() {
                push_unique_string(&mut blocking_reasons, "preview_reply_missing".into());
            }
            if !runtime_output.proposal_ids.is_empty() {
                push_unique_string(&mut blocking_reasons, "preview_proposal_ids_present".into());
            }
        }
    }

    let write_control = preview_write_control(&output.payload);
    let declared_write_step_count = write_control
        .get("declaredWriteStepCount")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    let proposal_required_step_count = write_control
        .get("proposalRequiredStepCount")
        .and_then(Value::as_u64)
        .unwrap_or_default();
    if declared_write_step_count > 0 || proposal_required_step_count > 0 {
        push_unique_string(
            &mut blocking_reasons,
            "preview_write_or_proposal_step_present".into(),
        );
    }

    blocking_reasons
}

fn default_chat_adapter_controlled_preview_review_decision_evidence_is_metadata_safe(
    record: &openlife_core::agent::EvidenceRecord,
) -> bool {
    if record.affected_path != DEFAULT_CHAT_ADAPTER_CONTROLLED_PREVIEW_REVIEW_DECISION_EVIDENCE_PATH
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
        "previewRunId",
        "decisionKind",
        "contractShape",
        "previewSummaryDigest",
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

    metadata_string_is_safe(&record.run_metadata, "previewRunId", safe_internal_id)
        && metadata_string_is_safe(&record.run_metadata, "decisionKind", |value, field| {
            safe_enum_value(value, field, &["approve", "reject", "request_rework"])
        })
        && metadata_string_is_safe(&record.run_metadata, "contractShape", |value, field| {
            safe_enum_value(
                value,
                field,
                &["send_message_compatible", "blocked", "failed"],
            )
        })
        && metadata_string_is_safe(&record.run_metadata, "previewSummaryDigest", |value, _| {
            safe_checksum(value)
        })
        && reviewer_note_flat_metadata_is_safe(&record.run_metadata)
        && record
            .run_metadata
            .get("createdAt")
            .and_then(Value::as_str)
            .is_some_and(|value| chrono::DateTime::parse_from_rfc3339(value).is_ok())
        && !contains_unsafe_promotion_metadata(&record.run_metadata)
}

fn default_chat_adapter_controlled_preview_review_decision_kind(
    record: &openlife_core::agent::EvidenceRecord,
) -> Option<&str> {
    record
        .run_metadata
        .get("decisionKind")
        .and_then(Value::as_str)
}

fn default_chat_adapter_controlled_preview_review_latest_decision(
    record: &openlife_core::agent::EvidenceRecord,
) -> Option<DefaultChatAdapterControlledPreviewReviewLatestDecision> {
    Some(DefaultChatAdapterControlledPreviewReviewLatestDecision {
        evidence_id: record.id.clone(),
        preview_run_id: record
            .run_metadata
            .get("previewRunId")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)?,
        decision_kind: default_chat_adapter_controlled_preview_review_decision_kind(record)?
            .to_string(),
        contract_shape: record
            .run_metadata
            .get("contractShape")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)?,
        preview_summary_digest: record
            .run_metadata
            .get("previewSummaryDigest")
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

fn default_chat_adapter_cutover_plan_review_decision_evidence_is_metadata_safe(
    record: &openlife_core::agent::EvidenceRecord,
) -> bool {
    if record.affected_path != DEFAULT_CHAT_ADAPTER_CUTOVER_PLAN_REVIEW_DECISION_EVIDENCE_PATH
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
        "w45Ready",
        "cutoverPlanDigest",
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

    let digest_is_safe = match record.run_metadata.get("cutoverPlanDigest") {
        Some(Value::Null) => true,
        Some(Value::String(value)) => safe_checksum_field(value, "cutoverPlanDigest").is_ok(),
        _ => false,
    };

    record
        .run_metadata
        .get("evidenceKind")
        .and_then(Value::as_str)
        .is_some_and(|value| value == "default_chat_adapter_cutover_plan_review_decision")
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
            .get("w45Ready")
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

fn default_chat_adapter_cutover_plan_review_decision_kind(
    record: &openlife_core::agent::EvidenceRecord,
) -> Option<&str> {
    record
        .run_metadata
        .get("decisionKind")
        .and_then(Value::as_str)
}

fn default_chat_adapter_cutover_plan_review_latest_decision(
    record: &openlife_core::agent::EvidenceRecord,
) -> Option<DefaultChatAdapterCutoverPlanReviewLatestDecision> {
    Some(DefaultChatAdapterCutoverPlanReviewLatestDecision {
        evidence_id: record.id.clone(),
        decision_kind: default_chat_adapter_cutover_plan_review_decision_kind(record)?.to_string(),
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
        cutover_plan_digest: record
            .run_metadata
            .get("cutoverPlanDigest")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        plan_section_count: record
            .run_metadata
            .get("planSectionCount")
            .and_then(Value::as_u64)
            .unwrap_or_default() as usize,
        w45_ready: record
            .run_metadata
            .get("w45Ready")
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
