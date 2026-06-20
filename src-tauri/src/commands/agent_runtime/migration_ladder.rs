use super::*;

#[tauri::command]
pub async fn check_runtime_migration_gate(
    input: RuntimeMigrationGateCheckInput,
    state: State<'_, Arc<AppState>>,
) -> Result<RuntimeMigrationGateReport, String> {
    check_runtime_migration_gate_with_state(input, &state.inner().clone()).await
}

pub(crate) async fn check_runtime_migration_gate_with_state(
    input: RuntimeMigrationGateCheckInput,
    state: &Arc<AppState>,
) -> Result<RuntimeMigrationGateReport, String> {
    let preview_run = find_preview_run_for_gate(input, state).await?;
    Ok(openlife_core::agent::evaluate_runtime_migration_gate(
        openlife_core::agent::RuntimeMigrationGateInput {
            default_chat_uses_multi_strategy: false,
            preview_run: preview_run.as_ref(),
            fallback_available: true,
        },
    ))
}

#[tauri::command]
pub async fn check_controlled_chat_pilot_eligibility(
    input: ControlledChatPilotEligibilityCheckInput,
    state: State<'_, Arc<AppState>>,
) -> Result<ControlledChatPilotEligibilityReport, String> {
    check_controlled_chat_pilot_eligibility_with_state(input, &state.inner().clone()).await
}

pub(crate) async fn check_controlled_chat_pilot_eligibility_with_state(
    input: ControlledChatPilotEligibilityCheckInput,
    state: &Arc<AppState>,
) -> Result<ControlledChatPilotEligibilityReport, String> {
    let required_clean_runs = input
        .required_clean_runs
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_CONTROLLED_CHAT_PILOT_REQUIRED_CLEAN_RUNS);
    let preview_runs =
        find_preview_runs_for_pilot_eligibility(&input, required_clean_runs, state).await?;

    Ok(
        openlife_core::agent::evaluate_controlled_chat_pilot_eligibility(
            openlife_core::agent::ControlledChatPilotEligibilityInput {
                default_chat_uses_multi_strategy: false,
                preview_runs: &preview_runs,
                required_clean_runs,
                fallback_available: true,
            },
        ),
    )
}

#[tauri::command]
pub async fn record_controlled_pilot_promotion_evidence(
    input: ControlledPilotPromotionEvidenceInput,
    state: State<'_, Arc<AppState>>,
) -> Result<ControlledPilotPromotionEvidenceResult, String> {
    record_controlled_pilot_promotion_evidence_with_state(input, &state.inner().clone()).await
}

pub(crate) async fn record_controlled_pilot_promotion_evidence_with_state(
    input: ControlledPilotPromotionEvidenceInput,
    state: &Arc<AppState>,
) -> Result<ControlledPilotPromotionEvidenceResult, String> {
    let evidence = normalize_promotion_evidence_input(input)?;
    let store = state.evidence_store.lock().await;
    let existing = store
        .query(EvidenceQuery {
            affected_path: Some(CONTROLLED_PILOT_PROMOTION_EVIDENCE_PATH.into()),
            evidence_type: Some(EvidenceType::RuntimeBehavior),
            linked_agent_run_id: Some(evidence.pilot_run_id.clone()),
            ..EvidenceQuery::default()
        })
        .map_err(|e| format!("failed to query controlled pilot promotion evidence: {e}"))?;

    if let Some(record) = existing.first() {
        let existing_hash = record
            .run_metadata
            .get("promotedMessageHash")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        if existing_hash != evidence.promoted_message_hash {
            return Err(
                "promotion evidence already exists for pilotRunId with a different checksum".into(),
            );
        }
        let promoted_at = record
            .run_metadata
            .get("promotedAt")
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| record.created_at.to_rfc3339());
        return Ok(ControlledPilotPromotionEvidenceResult {
            evidence_id: record.id.clone(),
            created: false,
            pilot_run_id: evidence.pilot_run_id,
            promoted_at,
        });
    }

    let metadata = json!({
        "evidenceKind": "controlled_pilot_promotion",
        "pilotRunId": evidence.pilot_run_id.clone(),
        "sourceSessionId": evidence.source_session_id.clone(),
        "targetSessionId": evidence.target_session_id.clone(),
        "strategyKind": evidence.strategy_kind.clone(),
        "payloadKind": evidence.payload_kind.clone(),
        "governanceDecisionKind": evidence.governance_decision_kind.clone(),
        "promotedMessageLength": evidence.promoted_message_length,
        "promotedMessageHash": evidence.promoted_message_hash.clone(),
        "promotedAt": evidence.promoted_at.clone(),
        "metadataSafe": true,
        "contentStorage": "checksum_only",
        "toolStorage": "none"
    });
    let draft = EvidenceDraft::new(
        EvidenceType::RuntimeBehavior,
        CONTROLLED_PILOT_PROMOTION_EVIDENCE_PATH,
        1.0,
        RiskLevel::Low,
        EvidencePrivacyLevel::Internal,
    )
    .with_summary("Controlled pilot response promoted to chat history")
    .with_source_ref(EvidenceSourceRef::from_digest(
        EvidenceSourceType::AgentRun,
        &evidence.pilot_run_id,
        Some("controlled_pilot_promotion"),
        &evidence.promoted_message_hash,
    ))
    .with_linked_agent_run(evidence.pilot_run_id.clone());
    let mut draft = draft;
    draft.run_metadata = metadata;

    let record = store
        .create_evidence(draft)
        .map_err(|e| format!("failed to record controlled pilot promotion evidence: {e}"))?;

    Ok(ControlledPilotPromotionEvidenceResult {
        evidence_id: record.id,
        created: true,
        pilot_run_id: evidence.pilot_run_id,
        promoted_at: evidence.promoted_at,
    })
}

#[tauri::command]
pub async fn get_controlled_pilot_promotion_evidence_summary(
    state: State<'_, Arc<AppState>>,
) -> Result<ControlledPilotPromotionEvidenceSummary, String> {
    get_controlled_pilot_promotion_evidence_summary_with_state(&state.inner().clone()).await
}

pub(crate) async fn get_controlled_pilot_promotion_evidence_summary_with_state(
    state: &Arc<AppState>,
) -> Result<ControlledPilotPromotionEvidenceSummary, String> {
    let store = state.evidence_store.lock().await;
    let promotions = store
        .query(EvidenceQuery {
            affected_path: Some(CONTROLLED_PILOT_PROMOTION_EVIDENCE_PATH.into()),
            evidence_type: Some(EvidenceType::RuntimeBehavior),
            ..EvidenceQuery::default()
        })
        .map_err(|e| format!("failed to read controlled pilot promotion evidence: {e}"))?;
    let mismatch_blocks = store
        .query(EvidenceQuery {
            affected_path: Some(CONTROLLED_PILOT_PROMOTION_BLOCK_PATH.into()),
            evidence_type: Some(EvidenceType::RuntimeBehavior),
            ..EvidenceQuery::default()
        })
        .map_err(|e| format!("failed to read controlled pilot promotion block evidence: {e}"))?;

    let recent_promoted_pilot_run_ids = promotions
        .iter()
        .filter_map(promotion_evidence_pilot_run_id)
        .take(RECENT_PROMOTION_EVIDENCE_LIMIT)
        .collect();
    let latest_promotion_timestamp = promotions.first().map(promotion_evidence_timestamp);

    Ok(ControlledPilotPromotionEvidenceSummary {
        promoted_count: promotions.len(),
        recent_promoted_pilot_run_ids,
        latest_promotion_timestamp,
        source_target_mismatch_block_count: mismatch_blocks.len(),
    })
}

#[tauri::command]
pub async fn check_controlled_pilot_promotion_readiness(
    input: ControlledPilotPromotionReadinessCheckInput,
    state: State<'_, Arc<AppState>>,
) -> Result<ControlledPilotPromotionReadinessReport, String> {
    check_controlled_pilot_promotion_readiness_with_state(input, &state.inner().clone()).await
}

pub(crate) async fn check_controlled_pilot_promotion_readiness_with_state(
    input: ControlledPilotPromotionReadinessCheckInput,
    state: &Arc<AppState>,
) -> Result<ControlledPilotPromotionReadinessReport, String> {
    let required_promotions = input
        .required_promotions
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_CONTROLLED_CHAT_PILOT_REQUIRED_CLEAN_RUNS);
    let _session_scope_is_global_for_now = input
        .session_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    let store = state.evidence_store.lock().await;
    let promotions = store
        .query(EvidenceQuery {
            affected_path: Some(CONTROLLED_PILOT_PROMOTION_EVIDENCE_PATH.into()),
            evidence_type: Some(EvidenceType::RuntimeBehavior),
            ..EvidenceQuery::default()
        })
        .map_err(|e| format!("failed to read controlled pilot promotion evidence: {e}"))?;
    let mismatch_blocks = store
        .query(EvidenceQuery {
            affected_path: Some(CONTROLLED_PILOT_PROMOTION_BLOCK_PATH.into()),
            evidence_type: Some(EvidenceType::RuntimeBehavior),
            ..EvidenceQuery::default()
        })
        .map_err(|e| format!("failed to read controlled pilot promotion block evidence: {e}"))?;

    let promoted_count = promotions.len();
    let recent_promoted_pilot_run_ids = promotions
        .iter()
        .filter_map(promotion_evidence_pilot_run_id)
        .take(RECENT_PROMOTION_EVIDENCE_LIMIT)
        .collect();
    let latest_promotion_timestamp = promotions.first().map(promotion_evidence_timestamp);
    let metadata_safe_evidence_ready =
        !promotions.is_empty() && promotions.iter().all(promotion_evidence_is_metadata_safe);
    let default_chat_unchanged = true;

    let mut blocking_reasons = Vec::new();
    if promoted_count < required_promotions {
        push_unique_string(
            &mut blocking_reasons,
            format!(
                "insufficient_promotion_evidence: required {required_promotions} promotions, found {promoted_count}"
            ),
        );
    }
    if !metadata_safe_evidence_ready {
        push_unique_string(
            &mut blocking_reasons,
            "promotion_evidence_not_metadata_safe".to_string(),
        );
    }
    if !mismatch_blocks.is_empty() {
        push_unique_string(
            &mut blocking_reasons,
            "source_target_mismatch_blocks_present".to_string(),
        );
    }

    let ready = default_chat_unchanged
        && promoted_count >= required_promotions
        && metadata_safe_evidence_ready
        && mismatch_blocks.is_empty()
        && blocking_reasons.is_empty();

    Ok(ControlledPilotPromotionReadinessReport {
        ready,
        required_promotions,
        promoted_count,
        recent_promoted_pilot_run_ids,
        latest_promotion_timestamp,
        source_target_mismatch_block_count: mismatch_blocks.len(),
        metadata_safe_evidence_ready,
        default_chat_unchanged,
        blocking_reasons,
    })
}

#[tauri::command]
pub async fn draft_controlled_chat_migration_plan(
    input: ControlledChatMigrationPlanDraftInput,
    state: State<'_, Arc<AppState>>,
) -> Result<ControlledChatMigrationPlanDraft, String> {
    draft_controlled_chat_migration_plan_with_state(input, &state.inner().clone()).await
}

pub(crate) async fn draft_controlled_chat_migration_plan_with_state(
    input: ControlledChatMigrationPlanDraftInput,
    state: &Arc<AppState>,
) -> Result<ControlledChatMigrationPlanDraft, String> {
    let readiness_report = check_controlled_pilot_promotion_readiness_with_state(
        ControlledPilotPromotionReadinessCheckInput {
            required_promotions: input.required_promotions,
            session_id: input.session_id,
        },
        state,
    )
    .await?;

    let blocking_reasons = readiness_report.blocking_reasons.clone();
    if !readiness_report.ready {
        return Ok(ControlledChatMigrationPlanDraft {
            draft_ready: false,
            readiness_report,
            migration_scope: Vec::new(),
            required_preconditions: Vec::new(),
            rollback_plan: Vec::new(),
            fallback_plan: Vec::new(),
            test_plan: Vec::new(),
            manual_review_required: true,
            not_automatic_migration: true,
            blocking_reasons,
        });
    }

    Ok(ControlledChatMigrationPlanDraft {
        draft_ready: true,
        readiness_report,
        migration_scope: vec![
            "Draft scope is limited to a human-reviewed controlled pilot discussion; default Chat remains unchanged.".into(),
            "No default runtime feature flag is enabled or modified by this draft.".into(),
            "No LifeModel, Memory, Proposal, AgentRun, full tool call data, or promotion evidence write is part of this draft.".into(),
        ],
        required_preconditions: vec![
            "separate human approval is required before any migration implementation work begins.".into(),
            "Readiness pass must be treated only as permission to discuss the next step, not migration permission.".into(),
            "Default Chat send_message and start_stream_message paths must remain on the existing runtime until a later approved change.".into(),
            "Controlled pilot UI must remain explicit, reversible, and write-disabled unless a later review approves otherwise.".into(),
        ],
        rollback_plan: vec![
            "disable the controlled pilot entry and keep default Chat on the existing send path.".into(),
            "Keep existing Chat history and promoted assistant messages as ordinary messages; do not replay pilot output.".into(),
            "Use promotion evidence summaries only for audit review; do not synthesize replacement evidence.".into(),
        ],
        fallback_plan: vec![
            "Use the existing default Chat send path whenever the controlled pilot is unavailable, blocked, or fails.".into(),
            "If migration discussion is rejected, continue collecting reviewed pilot promotion evidence without changing default Chat.".into(),
            "If a future pilot degrades, show blockers and route users back to ordinary Chat without automatic retry or promotion.".into(),
        ],
        test_plan: vec![
            "Verify send_message and start_stream_message do not call the migration draft command.".into(),
            "Verify readiness blocked returns draftReady=false and no executable plan sections.".into(),
            "Verify readiness passed returns scope, preconditions, rollback, fallback, and test plan sections.".into(),
            "Verify the command creates no AgentRun, Proposal, Memory, LifeModel patch, or promotion evidence.".into(),
            "Verify serialized output contains no private transcript text, assistant transcript text, or full tool call data.".into(),
        ],
        manual_review_required: true,
        not_automatic_migration: true,
        blocking_reasons,
    })
}

#[tauri::command]
pub async fn record_controlled_chat_migration_review_decision(
    input: ControlledChatMigrationReviewDecisionInput,
    state: State<'_, Arc<AppState>>,
) -> Result<ControlledChatMigrationReviewDecisionResult, String> {
    record_controlled_chat_migration_review_decision_with_state(input, &state.inner().clone()).await
}

pub(crate) async fn record_controlled_chat_migration_review_decision_with_state(
    input: ControlledChatMigrationReviewDecisionInput,
    state: &Arc<AppState>,
) -> Result<ControlledChatMigrationReviewDecisionResult, String> {
    let decision_kind = safe_enum_value(
        &input.decision_kind,
        "decisionKind",
        &["approve", "reject", "request_rework"],
    )?;
    let session_id = normalize_optional_internal_id(input.session_id.as_deref(), "sessionId")?;
    let draft = draft_controlled_chat_migration_plan_with_state(
        ControlledChatMigrationPlanDraftInput {
            required_promotions: input.required_promotions,
            session_id: session_id.clone(),
        },
        state,
    )
    .await?;
    let draft_hash = metadata_hash_for_serializable(&draft)?;
    let created_at = chrono::Utc::now().to_rfc3339();
    let mut blocking_reasons = draft.blocking_reasons.clone();

    if decision_kind == "approve" && !draft.draft_ready {
        push_unique_string(
            &mut blocking_reasons,
            "draft_not_ready_for_approval".to_string(),
        );
        return Ok(ControlledChatMigrationReviewDecisionResult {
            recorded: false,
            evidence_id: None,
            decision_kind,
            draft_ready: false,
            draft_hash,
            created_at,
            blocking_reasons,
        });
    }

    let reviewer_note_metadata =
        metadata_safe_reviewer_note(input.optional_reviewer_note.as_deref());
    let metadata = json!({
        "evidenceKind": "migration_review_decision",
        "metadataSafe": true,
        "draftReady": draft.draft_ready,
        "decisionKind": decision_kind.clone(),
        "readinessCounts": {
            "requiredPromotions": draft.readiness_report.required_promotions,
            "promotedCount": draft.readiness_report.promoted_count,
            "recentPromotedPilotRunCount": draft.readiness_report.recent_promoted_pilot_run_ids.len(),
            "sourceTargetMismatchBlockCount": draft.readiness_report.source_target_mismatch_block_count,
            "blockingReasonCount": draft.blocking_reasons.len()
        },
        "draftHash": draft_hash.clone(),
        "createdAt": created_at.clone(),
        "sessionId": session_id.as_deref().unwrap_or("global"),
        "reviewerNote": reviewer_note_metadata,
        "blockingReasons": draft.blocking_reasons.clone(),
        "metadataSafeEvidenceReady": draft.readiness_report.metadata_safe_evidence_ready,
        "defaultChatUnchanged": draft.readiness_report.default_chat_unchanged,
        "manualReviewRequired": draft.manual_review_required,
        "notAutomaticMigration": draft.not_automatic_migration,
        "contentStorage": "checksum_only",
        "reviewerNoteStorage": "length_checksum_category_only",
        "toolStorage": "none",
        "transcriptStorage": "none"
    });

    let mut evidence_draft = EvidenceDraft::new(
        EvidenceType::RuntimeBehavior,
        CONTROLLED_CHAT_MIGRATION_REVIEW_DECISION_EVIDENCE_PATH,
        1.0,
        RiskLevel::Low,
        EvidencePrivacyLevel::Internal,
    )
    .with_summary("Controlled chat migration review decision recorded")
    .with_source_ref(EvidenceSourceRef::from_digest(
        EvidenceSourceType::RunMetadata,
        "controlled_chat_migration_plan_draft",
        Some("migration_review_decision"),
        &draft_hash,
    ));
    evidence_draft.run_metadata = metadata;

    let record = {
        let store = state.evidence_store.lock().await;
        store
            .create_evidence(evidence_draft)
            .map_err(|e| format!("failed to record migration review decision evidence: {e}"))?
    };

    Ok(ControlledChatMigrationReviewDecisionResult {
        recorded: true,
        evidence_id: Some(record.id),
        decision_kind,
        draft_ready: draft.draft_ready,
        draft_hash,
        created_at,
        blocking_reasons,
    })
}

#[tauri::command]
pub async fn get_controlled_chat_migration_review_decision_summary(
    state: State<'_, Arc<AppState>>,
) -> Result<ControlledChatMigrationReviewDecisionSummary, String> {
    get_controlled_chat_migration_review_decision_summary_with_state(&state.inner().clone()).await
}

pub(crate) async fn get_controlled_chat_migration_review_decision_summary_with_state(
    state: &Arc<AppState>,
) -> Result<ControlledChatMigrationReviewDecisionSummary, String> {
    let records = {
        let store = state.evidence_store.lock().await;
        store
            .query(EvidenceQuery {
                affected_path: Some(CONTROLLED_CHAT_MIGRATION_REVIEW_DECISION_EVIDENCE_PATH.into()),
                evidence_type: Some(EvidenceType::RuntimeBehavior),
                ..EvidenceQuery::default()
            })
            .map_err(|e| format!("failed to read migration review decision evidence: {e}"))?
    };
    let records = records
        .into_iter()
        .filter(migration_review_decision_evidence_is_metadata_safe)
        .collect::<Vec<_>>();

    let approved_count = records
        .iter()
        .filter(|record| migration_review_decision_kind(record) == Some("approve"))
        .count();
    let rework_reject_count = records
        .iter()
        .filter(|record| {
            matches!(
                migration_review_decision_kind(record),
                Some("reject" | "request_rework")
            )
        })
        .count();
    let latest_decision = records.first().and_then(migration_review_latest_decision);
    let latest_timestamp = latest_decision
        .as_ref()
        .map(|decision| decision.created_at.clone());
    let blocking_reasons = records
        .first()
        .map(migration_review_decision_blocking_reasons)
        .unwrap_or_default();

    Ok(ControlledChatMigrationReviewDecisionSummary {
        latest_decision,
        approved_count,
        rework_reject_count,
        latest_timestamp,
        blocking_reasons,
    })
}

#[tauri::command]
pub async fn check_controlled_chat_migration_implementation_gate(
    input: ControlledChatMigrationImplementationGateInput,
    state: State<'_, Arc<AppState>>,
) -> Result<ControlledChatMigrationImplementationGateReport, String> {
    check_controlled_chat_migration_implementation_gate_with_state(input, &state.inner().clone())
        .await
}

pub(crate) async fn check_controlled_chat_migration_implementation_gate_with_state(
    input: ControlledChatMigrationImplementationGateInput,
    state: &Arc<AppState>,
) -> Result<ControlledChatMigrationImplementationGateReport, String> {
    let session_id = normalize_optional_internal_id(input.session_id.as_deref(), "sessionId")?;
    let current_draft = draft_controlled_chat_migration_plan_with_state(
        ControlledChatMigrationPlanDraftInput {
            required_promotions: input.required_promotions,
            session_id,
        },
        state,
    )
    .await?;
    let current_draft_hash = metadata_hash_for_serializable(&current_draft)?;
    let readiness_report = current_draft.readiness_report.clone();
    let decision_summary =
        get_controlled_chat_migration_review_decision_summary_with_state(state).await?;
    let latest_decision = decision_summary.latest_decision;
    let draft_hash_matched = latest_decision
        .as_ref()
        .is_some_and(|decision| decision.draft_hash == current_draft_hash);
    let latest_is_approve = latest_decision
        .as_ref()
        .is_some_and(|decision| decision.decision_kind == "approve");
    let approved_after_latest_draft = latest_is_approve && draft_hash_matched;

    let mut blocking_reasons = Vec::new();
    if !readiness_report.ready {
        push_unique_string(
            &mut blocking_reasons,
            "promotion_readiness_currently_blocked".to_string(),
        );
        for reason in &readiness_report.blocking_reasons {
            push_unique_string(&mut blocking_reasons, reason.clone());
        }
    }
    if !current_draft.draft_ready {
        push_unique_string(
            &mut blocking_reasons,
            "migration_plan_draft_not_ready".to_string(),
        );
    }

    match latest_decision.as_ref() {
        Some(decision) if decision.decision_kind == "approve" => {
            if !decision.draft_ready {
                push_unique_string(
                    &mut blocking_reasons,
                    "latest_approval_draft_not_ready".to_string(),
                );
            }
            if !draft_hash_matched {
                push_unique_string(
                    &mut blocking_reasons,
                    "approved_draft_hash_mismatch".to_string(),
                );
            }
        }
        Some(decision) => {
            push_unique_string(
                &mut blocking_reasons,
                format!("latest_review_decision_is_{}", decision.decision_kind),
            );
        }
        None => {
            push_unique_string(
                &mut blocking_reasons,
                "metadata_safe_approve_decision_missing".to_string(),
            );
        }
    }

    let implementation_eligible = readiness_report.ready
        && current_draft.draft_ready
        && latest_is_approve
        && latest_decision
            .as_ref()
            .is_some_and(|decision| decision.draft_ready)
        && draft_hash_matched
        && blocking_reasons.is_empty();

    Ok(ControlledChatMigrationImplementationGateReport {
        implementation_eligible,
        latest_decision,
        readiness_report,
        draft_hash_matched,
        approved_after_latest_draft,
        blocking_reasons,
    })
}

#[tauri::command]
pub async fn run_controlled_chat_migration_shadow_run(
    input: ControlledChatMigrationShadowRunInput,
    state: State<'_, Arc<AppState>>,
) -> Result<ControlledChatMigrationShadowRunOutput, String> {
    run_controlled_chat_migration_shadow_run_with_state(input, &state.inner().clone()).await
}

pub(crate) async fn run_controlled_chat_migration_shadow_run_with_state(
    input: ControlledChatMigrationShadowRunInput,
    state: &Arc<AppState>,
) -> Result<ControlledChatMigrationShadowRunOutput, String> {
    let normalized = normalize_shadow_run_input(input)?;
    let implementation_gate_report =
        check_controlled_chat_migration_implementation_gate_with_state(
            ControlledChatMigrationImplementationGateInput {
                required_promotions: normalized.required_promotions,
                session_id: Some(normalized.session_id.clone()),
            },
            state,
        )
        .await?;

    if !implementation_gate_report.implementation_eligible {
        let mut blocking_reasons = vec!["implementation_gate_blocked".to_string()];
        for reason in &implementation_gate_report.blocking_reasons {
            push_unique_string(&mut blocking_reasons, reason.clone());
        }
        return Ok(ControlledChatMigrationShadowRunOutput {
            shadow_run_ready: false,
            shadow_run_id: None,
            metadata_safe_summary: shadow_blocked_summary(
                &normalized.descriptor_kind,
                normalized.user_input_checksum.as_deref(),
            ),
            implementation_gate_report,
            strategy_kind: "notRun".into(),
            payload_kind: "notRun".into(),
            warnings: Vec::new(),
            blocking_reasons,
        });
    }

    let mut shadow_run = new_shadow_agent_run(
        &normalized.session_id,
        &normalized.descriptor_kind,
        normalized.user_input_checksum.as_deref(),
    );
    let shadow_run_id = shadow_run.id.clone();
    create_shadow_run(state, &shadow_run).await?;

    let runtime_input = MultiStrategyAgentPreviewInput {
        session_id: normalized.session_id.clone(),
        user_text: shadow_prompt_for_descriptor(&normalized.descriptor_kind).into(),
        tools_prompt: "No developer tools catalog supplied for this shadow run.".into(),
        allow_planning: normalized.descriptor_kind == "planning_readiness_probe",
        local_model_available: normalized.descriptor_kind != "sensitive_local_only_probe",
        layer: Some("L2".into()),
        execution_budget: Some(MultiStrategyAgentPreviewExecutionBudgetInput {
            max_steps: Some(3),
            max_tool_calls: Some(0),
            timeout_seconds: Some(30),
            allow_cloud: Some(false),
            allow_writes: Some(false),
        }),
    };

    let execution =
        execute_multi_strategy_agent_preview(runtime_input, state, &shadow_run_id).await;
    let execution = match execution {
        Ok(execution) => execution,
        Err(error) => {
            let safe_error = metadata_safe_shadow_error(&error);
            fail_shadow_run(state, &mut shadow_run, &safe_error).await;
            return Ok(ControlledChatMigrationShadowRunOutput {
                shadow_run_ready: false,
                shadow_run_id: Some(shadow_run_id),
                implementation_gate_report,
                strategy_kind: "notRun".into(),
                payload_kind: "notRun".into(),
                metadata_safe_summary: shadow_failed_summary(
                    &normalized.descriptor_kind,
                    normalized.user_input_checksum.as_deref(),
                    &safe_error,
                ),
                warnings: vec!["shadow runtime failed before readiness comparison".into()],
                blocking_reasons: vec![safe_error],
            });
        }
    };

    let strategy_kind = preview_strategy_kind(execution.output.selection.kind).to_string();
    let payload_kind = preview_payload_kind(&execution.output.payload).to_string();
    let mut warnings = preview_output_warnings(&execution.output, &execution.warnings);
    push_unique_string(
        &mut warnings,
        "shadow runtime forced allowWrites=false".to_string(),
    );
    let metadata_safe_summary = shadow_metadata_safe_summary(
        &execution.output,
        &normalized.descriptor_kind,
        normalized.user_input_checksum.as_deref(),
    );
    let audit = shadow_audit_summary(
        &execution.output,
        &warnings,
        &normalized.descriptor_kind,
        normalized.user_input_checksum.as_deref(),
    );

    complete_shadow_run(
        state,
        &mut shadow_run,
        ShadowRunCompletion {
            audit,
            warnings: warnings.clone(),
            context_summary: execution.context_summary,
            hs_selection_audit: execution.hs_selection_audit,
            behavior_checks: execution.behavior_checks,
        },
    )
    .await?;

    Ok(ControlledChatMigrationShadowRunOutput {
        shadow_run_ready: true,
        shadow_run_id: Some(shadow_run_id),
        implementation_gate_report,
        strategy_kind,
        payload_kind,
        metadata_safe_summary,
        warnings,
        blocking_reasons: Vec::new(),
    })
}

#[tauri::command]
pub async fn record_controlled_chat_migration_shadow_review_decision(
    input: ControlledChatMigrationShadowReviewDecisionInput,
    state: State<'_, Arc<AppState>>,
) -> Result<ControlledChatMigrationShadowReviewDecisionResult, String> {
    record_controlled_chat_migration_shadow_review_decision_with_state(
        input,
        &state.inner().clone(),
    )
    .await
}

pub(crate) async fn record_controlled_chat_migration_shadow_review_decision_with_state(
    input: ControlledChatMigrationShadowReviewDecisionInput,
    state: &Arc<AppState>,
) -> Result<ControlledChatMigrationShadowReviewDecisionResult, String> {
    let shadow_run_id = safe_internal_id(&input.shadow_run_id, "shadowRunId")?;
    let decision_kind = safe_enum_value(
        &input.decision_kind,
        "decisionKind",
        &["approve", "reject", "request_rework"],
    )?;
    let created_at = chrono::Utc::now().to_rfc3339();
    let run = load_shadow_review_run(state, &shadow_run_id).await?;
    let readiness = shadow_review_readiness(run.as_ref())?;
    let blocking_reasons = readiness.blocking_reasons.clone();

    if !blocking_reasons.is_empty() {
        return Ok(ControlledChatMigrationShadowReviewDecisionResult {
            recorded: false,
            evidence_id: None,
            shadow_run_id,
            decision_kind,
            readiness_summary_digest: readiness.digest,
            created_at,
            blocking_reasons,
        });
    }

    let reviewer_note_metadata =
        metadata_safe_reviewer_note_fields(input.optional_reviewer_note.as_deref());
    let mut evidence_draft = EvidenceDraft::new(
        EvidenceType::RuntimeBehavior,
        CONTROLLED_CHAT_MIGRATION_SHADOW_REVIEW_DECISION_EVIDENCE_PATH,
        1.0,
        RiskLevel::Low,
        EvidencePrivacyLevel::Internal,
    );
    evidence_draft.run_metadata = json!({
        "shadowRunId": shadow_run_id.clone(),
        "decisionKind": decision_kind.clone(),
        "reviewerNoteChecksum": reviewer_note_metadata.checksum,
        "reviewerNoteLength": reviewer_note_metadata.length,
        "reviewerNoteCategory": reviewer_note_metadata.category,
        "readinessSummaryDigest": readiness.digest.clone(),
        "createdAt": created_at.clone(),
    });

    let record = {
        let store = state.evidence_store.lock().await;
        store.create_evidence(evidence_draft).map_err(|e| {
            format!("failed to record migration shadow review decision evidence: {e}")
        })?
    };

    Ok(ControlledChatMigrationShadowReviewDecisionResult {
        recorded: true,
        evidence_id: Some(record.id),
        shadow_run_id,
        decision_kind,
        readiness_summary_digest: readiness.digest,
        created_at,
        blocking_reasons,
    })
}

#[tauri::command]
pub async fn get_controlled_chat_migration_shadow_review_summary(
    state: State<'_, Arc<AppState>>,
) -> Result<ControlledChatMigrationShadowReviewSummary, String> {
    get_controlled_chat_migration_shadow_review_summary_with_state(&state.inner().clone()).await
}

pub(crate) async fn get_controlled_chat_migration_shadow_review_summary_with_state(
    state: &Arc<AppState>,
) -> Result<ControlledChatMigrationShadowReviewSummary, String> {
    let records = {
        let store = state.evidence_store.lock().await;
        store
            .query(EvidenceQuery {
                affected_path: Some(
                    CONTROLLED_CHAT_MIGRATION_SHADOW_REVIEW_DECISION_EVIDENCE_PATH.into(),
                ),
                evidence_type: Some(EvidenceType::RuntimeBehavior),
                ..EvidenceQuery::default()
            })
            .map_err(|e| format!("failed to read migration shadow review evidence: {e}"))?
    };
    let records = records
        .into_iter()
        .filter(shadow_review_decision_evidence_is_metadata_safe)
        .collect::<Vec<_>>();

    let approved_count = records
        .iter()
        .filter(|record| shadow_review_decision_kind(record) == Some("approve"))
        .count();
    let rework_reject_count = records
        .iter()
        .filter(|record| {
            matches!(
                shadow_review_decision_kind(record),
                Some("reject" | "request_rework")
            )
        })
        .count();
    let latest_decision = records.first().and_then(shadow_review_latest_decision);
    let latest_timestamp = latest_decision
        .as_ref()
        .map(|decision| decision.created_at.clone());

    Ok(ControlledChatMigrationShadowReviewSummary {
        latest_decision,
        approved_count,
        rework_reject_count,
        latest_timestamp,
        blocking_reasons: Vec::new(),
    })
}

#[tauri::command]
pub async fn check_controlled_chat_cutover_readiness(
    input: ControlledChatCutoverReadinessInput,
    state: State<'_, Arc<AppState>>,
) -> Result<ControlledChatCutoverReadinessReport, String> {
    check_controlled_chat_cutover_readiness_with_state(input, &state.inner().clone()).await
}

pub(crate) async fn check_controlled_chat_cutover_readiness_with_state(
    input: ControlledChatCutoverReadinessInput,
    state: &Arc<AppState>,
) -> Result<ControlledChatCutoverReadinessReport, String> {
    let implementation_gate_report =
        check_controlled_chat_migration_implementation_gate_with_state(
            ControlledChatMigrationImplementationGateInput {
                required_promotions: input.required_promotions,
                session_id: input.session_id,
            },
            state,
        )
        .await?;
    let shadow_review_summary =
        get_controlled_chat_migration_shadow_review_summary_with_state(state).await?;
    let latest_shadow_review_decision = shadow_review_summary.latest_decision.clone();
    let default_chat_unchanged = implementation_gate_report
        .readiness_report
        .default_chat_unchanged;

    let mut blocking_reasons = Vec::new();
    if !implementation_gate_report.implementation_eligible {
        push_unique_string(
            &mut blocking_reasons,
            "implementation_gate_not_eligible".into(),
        );
        for reason in &implementation_gate_report.blocking_reasons {
            push_unique_string(&mut blocking_reasons, reason.clone());
        }
    }
    if !default_chat_unchanged {
        push_unique_string(&mut blocking_reasons, "default_chat_changed".into());
    }

    let mut readiness_summary_digest = None;
    let mut verified_shadow_run_id = None;
    let mut shadow_run_ready = false;
    let latest_shadow_decision_kind = latest_shadow_review_decision
        .as_ref()
        .map(|decision| decision.decision_kind.clone())
        .unwrap_or_else(|| "none".into());

    match latest_shadow_review_decision.as_ref() {
        Some(decision) if decision.decision_kind == "approve" => {
            let run = load_shadow_review_run(state, &decision.shadow_run_id).await?;
            let readiness = shadow_review_readiness(run.as_ref())?;
            readiness_summary_digest = Some(readiness.digest.clone());
            for reason in &readiness.blocking_reasons {
                push_unique_string(&mut blocking_reasons, reason.clone());
            }
            if readiness.blocking_reasons.is_empty()
                && readiness.digest != decision.readiness_summary_digest
            {
                push_unique_string(
                    &mut blocking_reasons,
                    "shadow_run_readiness_digest_mismatch".into(),
                );
            }
            shadow_run_ready = readiness.blocking_reasons.is_empty()
                && readiness.digest == decision.readiness_summary_digest;
            if shadow_run_ready {
                verified_shadow_run_id = Some(decision.shadow_run_id.clone());
            }
        }
        Some(decision) => {
            readiness_summary_digest = Some(decision.readiness_summary_digest.clone());
            push_unique_string(
                &mut blocking_reasons,
                format!(
                    "latest_shadow_review_decision_is_{}",
                    decision.decision_kind
                ),
            );
        }
        None => {
            push_unique_string(
                &mut blocking_reasons,
                "shadow_review_approve_missing".into(),
            );
        }
    }

    let required_evidence_ready = implementation_gate_report.implementation_eligible
        && default_chat_unchanged
        && latest_shadow_review_decision
            .as_ref()
            .is_some_and(|decision| decision.decision_kind == "approve")
        && shadow_run_ready;
    let cutover_planning_eligible = required_evidence_ready && blocking_reasons.is_empty();
    let metadata_safe_summary =
        cutover_readiness_metadata_safe_summary(CutoverReadinessMetadataSafeSummaryInput {
            cutover_planning_eligible,
            required_evidence_ready,
            default_chat_unchanged,
            implementation_eligible: implementation_gate_report.implementation_eligible,
            latest_shadow_decision_kind: &latest_shadow_decision_kind,
            shadow_run_ready,
            verified_shadow_run_id: verified_shadow_run_id.as_deref(),
            readiness_summary_digest: readiness_summary_digest.as_deref(),
            shadow_review_summary: &shadow_review_summary,
        });

    Ok(ControlledChatCutoverReadinessReport {
        cutover_planning_eligible,
        implementation_gate_report,
        latest_shadow_review_decision,
        verified_shadow_run_id,
        readiness_summary_digest,
        default_chat_unchanged,
        required_evidence_ready,
        blocking_reasons,
        metadata_safe_summary,
    })
}

#[tauri::command]
pub async fn run_controlled_chat_cutover_candidate(
    input: ControlledChatCutoverCandidateInput,
    state: State<'_, Arc<AppState>>,
) -> Result<ControlledChatCutoverCandidateOutput, String> {
    run_controlled_chat_cutover_candidate_with_state(input, &state.inner().clone()).await
}

pub(crate) async fn run_controlled_chat_cutover_candidate_with_state(
    input: ControlledChatCutoverCandidateInput,
    state: &Arc<AppState>,
) -> Result<ControlledChatCutoverCandidateOutput, String> {
    let normalized = normalize_cutover_candidate_input(input)?;
    let readiness = check_controlled_chat_cutover_readiness_with_state(
        ControlledChatCutoverReadinessInput {
            required_promotions: normalized.required_promotions,
            session_id: Some(normalized.session_id.clone()),
        },
        state,
    )
    .await?;

    if !readiness.cutover_planning_eligible {
        let mut blocking_reasons = vec!["cutover_readiness_not_eligible".to_string()];
        for reason in &readiness.blocking_reasons {
            push_unique_string(&mut blocking_reasons, reason.clone());
        }
        return Ok(ControlledChatCutoverCandidateOutput {
            candidate_ready: false,
            candidate_run_id: None,
            output_preview: Some("Candidate blocked before runtime".into()),
            user_output: None,
            contract_shape: "blocked".into(),
            metadata_safe_summary: cutover_candidate_blocked_summary(
                &normalized.descriptor_kind,
                normalized.user_input_checksum.as_deref(),
            ),
            warnings: Vec::new(),
            blocking_reasons,
        });
    }

    let mut candidate_run = new_cutover_candidate_agent_run(
        &normalized.session_id,
        &normalized.descriptor_kind,
        normalized.user_input_checksum.as_deref(),
    );
    let candidate_run_id = candidate_run.id.clone();
    create_cutover_candidate_run(state, &candidate_run).await?;

    let runtime_input = MultiStrategyAgentPreviewInput {
        session_id: normalized.session_id.clone(),
        user_text: cutover_candidate_prompt_for_descriptor(&normalized.descriptor_kind).into(),
        tools_prompt: "No developer tools catalog supplied for this cutover candidate.".into(),
        allow_planning: false,
        local_model_available: true,
        layer: Some("L2".into()),
        execution_budget: Some(MultiStrategyAgentPreviewExecutionBudgetInput {
            max_steps: Some(2),
            max_tool_calls: Some(0),
            timeout_seconds: Some(30),
            allow_cloud: Some(false),
            allow_writes: Some(false),
        }),
    };

    let execution =
        execute_multi_strategy_agent_preview(runtime_input, state, &candidate_run_id).await;
    let execution = match execution {
        Ok(execution) => execution,
        Err(error) => {
            let safe_error = metadata_safe_cutover_candidate_error(&error);
            fail_cutover_candidate_run(state, &mut candidate_run, &safe_error).await;
            return Ok(ControlledChatCutoverCandidateOutput {
                candidate_ready: false,
                candidate_run_id: Some(candidate_run_id),
                output_preview: Some("Candidate failed before contract validation".into()),
                user_output: None,
                contract_shape: "failed".into(),
                metadata_safe_summary: cutover_candidate_failed_summary(
                    &normalized.descriptor_kind,
                    normalized.user_input_checksum.as_deref(),
                    &safe_error,
                ),
                warnings: vec!["candidate runtime failed before contract validation".into()],
                blocking_reasons: vec![safe_error],
            });
        }
    };

    let contract_shape = cutover_candidate_contract_shape(&execution.output).to_string();
    let candidate_ready = contract_shape == "send_message_compatible";
    let user_output = cutover_candidate_user_output(&execution.output);
    let output_preview = cutover_candidate_output_label(&execution.output);
    let mut warnings = preview_output_warnings(&execution.output, &execution.warnings);
    push_unique_string(
        &mut warnings,
        "candidate runtime forced allowWrites=false".to_string(),
    );
    let blocking_reasons = cutover_candidate_contract_blockers(&execution.output, &contract_shape);
    let output_digest = user_output
        .as_deref()
        .filter(|value| !value.is_empty())
        .map(sha256_metadata_checksum);
    let metadata_safe_summary = cutover_candidate_metadata_safe_summary(
        &execution.output,
        &normalized.descriptor_kind,
        normalized.user_input_checksum.as_deref(),
        &contract_shape,
        candidate_ready,
        output_digest.as_deref(),
    );
    let audit = cutover_candidate_audit_summary(
        &execution.output,
        &warnings,
        &normalized.descriptor_kind,
        normalized.user_input_checksum.as_deref(),
        &contract_shape,
        candidate_ready,
        output_digest.as_deref(),
    );

    complete_cutover_candidate_run(
        state,
        &mut candidate_run,
        CutoverCandidateRunCompletion {
            audit,
            warnings: warnings.clone(),
            context_summary: execution.context_summary,
            hs_selection_audit: execution.hs_selection_audit,
            behavior_checks: execution.behavior_checks,
        },
    )
    .await?;

    Ok(ControlledChatCutoverCandidateOutput {
        candidate_ready,
        candidate_run_id: Some(candidate_run_id),
        output_preview: Some(output_preview),
        user_output,
        contract_shape,
        metadata_safe_summary,
        warnings,
        blocking_reasons,
    })
}

#[tauri::command]
pub async fn record_controlled_chat_cutover_candidate_review_decision(
    input: ControlledChatCutoverCandidateReviewDecisionInput,
    state: State<'_, Arc<AppState>>,
) -> Result<ControlledChatCutoverCandidateReviewDecisionResult, String> {
    record_controlled_chat_cutover_candidate_review_decision_with_state(
        input,
        &state.inner().clone(),
    )
    .await
}

pub(crate) async fn record_controlled_chat_cutover_candidate_review_decision_with_state(
    input: ControlledChatCutoverCandidateReviewDecisionInput,
    state: &Arc<AppState>,
) -> Result<ControlledChatCutoverCandidateReviewDecisionResult, String> {
    let candidate_run_id = safe_internal_id(&input.candidate_run_id, "candidateRunId")?;
    let decision_kind = safe_enum_value(
        &input.decision_kind,
        "decisionKind",
        &["approve", "reject", "request_rework"],
    )?;
    let created_at = chrono::Utc::now().to_rfc3339();
    let run = load_cutover_candidate_review_run(state, &candidate_run_id).await?;
    let readiness = cutover_candidate_review_readiness(run.as_ref())?;
    let mut blocking_reasons = readiness.blocking_reasons.clone();

    if decision_kind == "approve" {
        if readiness.contract_shape != "send_message_compatible" {
            push_unique_string(
                &mut blocking_reasons,
                "candidate_run_contract_shape_not_send_message_compatible".into(),
            );
        }
        if !readiness.candidate_ready {
            push_unique_string(
                &mut blocking_reasons,
                "candidate_run_not_ready_for_approval".into(),
            );
        }
    }

    if !blocking_reasons.is_empty() {
        return Ok(ControlledChatCutoverCandidateReviewDecisionResult {
            recorded: false,
            evidence_id: None,
            candidate_run_id,
            decision_kind,
            contract_shape: readiness.contract_shape,
            candidate_summary_digest: readiness.digest,
            created_at,
            blocking_reasons,
        });
    }

    let reviewer_note_metadata =
        metadata_safe_reviewer_note_fields(input.optional_reviewer_note.as_deref());
    let mut evidence_draft = EvidenceDraft::new(
        EvidenceType::RuntimeBehavior,
        CONTROLLED_CHAT_CUTOVER_CANDIDATE_REVIEW_DECISION_EVIDENCE_PATH,
        1.0,
        RiskLevel::Low,
        EvidencePrivacyLevel::Internal,
    );
    evidence_draft.run_metadata = json!({
        "candidateRunId": candidate_run_id.clone(),
        "decisionKind": decision_kind.clone(),
        "contractShape": readiness.contract_shape.clone(),
        "candidateSummaryDigest": readiness.digest.clone(),
        "reviewerNoteChecksum": reviewer_note_metadata.checksum,
        "reviewerNoteLength": reviewer_note_metadata.length,
        "reviewerNoteCategory": reviewer_note_metadata.category,
        "createdAt": created_at.clone(),
    });

    let record = {
        let store = state.evidence_store.lock().await;
        store.create_evidence(evidence_draft).map_err(|e| {
            format!("failed to record cutover candidate review decision evidence: {e}")
        })?
    };

    Ok(ControlledChatCutoverCandidateReviewDecisionResult {
        recorded: true,
        evidence_id: Some(record.id),
        candidate_run_id,
        decision_kind,
        contract_shape: readiness.contract_shape,
        candidate_summary_digest: readiness.digest,
        created_at,
        blocking_reasons,
    })
}

#[tauri::command]
pub async fn get_controlled_chat_cutover_candidate_review_summary(
    state: State<'_, Arc<AppState>>,
) -> Result<ControlledChatCutoverCandidateReviewSummary, String> {
    get_controlled_chat_cutover_candidate_review_summary_with_state(&state.inner().clone()).await
}

pub(crate) async fn get_controlled_chat_cutover_candidate_review_summary_with_state(
    state: &Arc<AppState>,
) -> Result<ControlledChatCutoverCandidateReviewSummary, String> {
    let records = {
        let store = state.evidence_store.lock().await;
        store
            .query(EvidenceQuery {
                affected_path: Some(
                    CONTROLLED_CHAT_CUTOVER_CANDIDATE_REVIEW_DECISION_EVIDENCE_PATH.into(),
                ),
                evidence_type: Some(EvidenceType::RuntimeBehavior),
                ..EvidenceQuery::default()
            })
            .map_err(|e| format!("failed to read cutover candidate review evidence: {e}"))?
    };
    let records = records
        .into_iter()
        .filter(cutover_candidate_review_decision_evidence_is_metadata_safe)
        .collect::<Vec<_>>();

    let approved_count = records
        .iter()
        .filter(|record| cutover_candidate_review_decision_kind(record) == Some("approve"))
        .count();
    let rework_reject_count = records
        .iter()
        .filter(|record| {
            matches!(
                cutover_candidate_review_decision_kind(record),
                Some("reject" | "request_rework")
            )
        })
        .count();
    let latest_decision = records
        .first()
        .and_then(cutover_candidate_review_latest_decision);
    let latest_timestamp = latest_decision
        .as_ref()
        .map(|decision| decision.created_at.clone());

    Ok(ControlledChatCutoverCandidateReviewSummary {
        latest_decision,
        approved_count,
        rework_reject_count,
        latest_timestamp,
        blocking_reasons: Vec::new(),
    })
}

#[tauri::command]
pub async fn check_controlled_chat_cutover_candidate_promotion_readiness(
    input: ControlledChatCutoverCandidatePromotionReadinessInput,
    state: State<'_, Arc<AppState>>,
) -> Result<ControlledChatCutoverCandidatePromotionReadinessReport, String> {
    check_controlled_chat_cutover_candidate_promotion_readiness_with_state(
        input,
        &state.inner().clone(),
    )
    .await
}

pub(crate) async fn check_controlled_chat_cutover_candidate_promotion_readiness_with_state(
    input: ControlledChatCutoverCandidatePromotionReadinessInput,
    state: &Arc<AppState>,
) -> Result<ControlledChatCutoverCandidatePromotionReadinessReport, String> {
    let required_approved_candidates = input
        .required_approved_candidates
        .filter(|value| *value > 0)
        .unwrap_or(1);
    let checked_at = chrono::Utc::now().to_rfc3339();
    let cutover_readiness = check_controlled_chat_cutover_readiness_with_state(
        ControlledChatCutoverReadinessInput {
            required_promotions: input.required_promotions,
            session_id: input.session_id,
        },
        state,
    )
    .await?;
    let cutover_readiness_eligible = cutover_readiness.cutover_planning_eligible;
    let default_chat_unchanged = cutover_readiness.default_chat_unchanged;

    let records = {
        let store = state.evidence_store.lock().await;
        store
            .query(EvidenceQuery {
                affected_path: Some(
                    CONTROLLED_CHAT_CUTOVER_CANDIDATE_REVIEW_DECISION_EVIDENCE_PATH.into(),
                ),
                evidence_type: Some(EvidenceType::RuntimeBehavior),
                ..EvidenceQuery::default()
            })
            .map_err(|e| {
                format!("failed to read cutover candidate promotion readiness evidence: {e}")
            })?
    };
    let records = records
        .into_iter()
        .filter(cutover_candidate_review_decision_evidence_is_metadata_safe)
        .collect::<Vec<_>>();
    let latest_decision = records
        .first()
        .and_then(cutover_candidate_review_latest_decision);

    let mut approved_decisions = Vec::new();
    let mut approved_candidate_run_ids = Vec::<String>::new();
    for record in records
        .iter()
        .filter(|record| cutover_candidate_review_decision_kind(record) == Some("approve"))
    {
        let Some(decision) = cutover_candidate_review_latest_decision(record) else {
            continue;
        };
        if approved_candidate_run_ids
            .iter()
            .any(|run_id| run_id == &decision.candidate_run_id)
        {
            continue;
        }
        approved_candidate_run_ids.push(decision.candidate_run_id.clone());
        approved_decisions.push(decision);
    }

    let mut blocking_reasons = Vec::new();
    if !cutover_readiness_eligible {
        push_unique_string(
            &mut blocking_reasons,
            "cutover_readiness_not_eligible".into(),
        );
        for reason in &cutover_readiness.blocking_reasons {
            push_unique_string(&mut blocking_reasons, reason.clone());
        }
    }
    if !default_chat_unchanged {
        push_unique_string(&mut blocking_reasons, "default_chat_changed".into());
    }

    match latest_decision
        .as_ref()
        .map(|decision| decision.decision_kind.as_str())
    {
        Some("reject" | "request_rework") => {
            let decision_kind = latest_decision
                .as_ref()
                .map(|decision| decision.decision_kind.as_str())
                .unwrap_or("unknown");
            push_unique_string(
                &mut blocking_reasons,
                format!("latest_candidate_review_decision_is_{decision_kind}"),
            );
        }
        Some("approve") => {}
        Some(other) => {
            push_unique_string(
                &mut blocking_reasons,
                format!("latest_candidate_review_decision_is_{other}"),
            );
        }
        None => {
            push_unique_string(
                &mut blocking_reasons,
                "candidate_review_decision_missing".into(),
            );
        }
    }

    let approved_candidate_count = approved_decisions.len();
    if approved_candidate_count == 0 {
        push_unique_string(
            &mut blocking_reasons,
            "metadata_safe_candidate_approve_evidence_missing".into(),
        );
    }
    if approved_candidate_count < required_approved_candidates {
        push_unique_string(
            &mut blocking_reasons,
            format!(
                "insufficient_approved_candidate_evidence: required {required_approved_candidates}, found {approved_candidate_count}"
            ),
        );
    }

    let mut approved_candidates = Vec::new();
    for decision in approved_decisions {
        let run = load_cutover_candidate_review_run(state, &decision.candidate_run_id).await?;
        let readiness = cutover_candidate_review_readiness(run.as_ref())?;
        let mut candidate_blocking_reasons = readiness.blocking_reasons.clone();
        if readiness.contract_shape != "send_message_compatible" {
            push_unique_string(
                &mut candidate_blocking_reasons,
                "candidate_run_contract_shape_not_send_message_compatible".into(),
            );
        }
        if !readiness.candidate_ready {
            push_unique_string(
                &mut candidate_blocking_reasons,
                "candidate_run_not_ready_for_approval".into(),
            );
        }
        if run.is_some()
            && candidate_blocking_reasons.is_empty()
            && readiness.digest != decision.candidate_summary_digest
        {
            push_unique_string(
                &mut candidate_blocking_reasons,
                "candidate_run_summary_digest_mismatch".into(),
            );
        }

        for reason in &candidate_blocking_reasons {
            push_unique_string(&mut blocking_reasons, reason.clone());
        }
        let ready = candidate_blocking_reasons.is_empty();
        approved_candidates.push(ControlledChatCutoverCandidatePromotionApprovedCandidate {
            evidence_id: decision.evidence_id,
            candidate_run_id: decision.candidate_run_id,
            contract_shape: readiness.contract_shape,
            candidate_summary_digest: decision.candidate_summary_digest,
            run_readiness_digest: readiness.digest,
            decision_created_at: decision.created_at,
            ready,
            blocking_reasons: candidate_blocking_reasons,
        });
    }

    let ready = cutover_readiness_eligible
        && default_chat_unchanged
        && approved_candidate_count >= required_approved_candidates
        && latest_decision
            .as_ref()
            .is_some_and(|decision| decision.decision_kind == "approve")
        && approved_candidates.iter().all(|candidate| candidate.ready)
        && blocking_reasons.is_empty();
    let metadata_safe_summary = cutover_candidate_promotion_readiness_metadata_safe_summary(
        CutoverCandidatePromotionReadinessMetadataSafeSummaryInput {
            ready,
            cutover_readiness_eligible,
            required_approved_candidates,
            approved_candidate_count,
            latest_decision_kind: latest_decision
                .as_ref()
                .map(|decision| decision.decision_kind.as_str())
                .unwrap_or("none"),
            default_chat_unchanged,
            verified_candidate_count: approved_candidates.len(),
            blocking_reason_count: blocking_reasons.len(),
        },
    );

    Ok(ControlledChatCutoverCandidatePromotionReadinessReport {
        ready,
        cutover_readiness_eligible,
        required_approved_candidates,
        approved_candidate_count,
        latest_decision,
        approved_candidates,
        default_chat_unchanged,
        blocking_reasons,
        metadata_safe_summary,
        checked_at,
    })
}

struct NormalizedPromotionEvidenceInput {
    pilot_run_id: String,
    source_session_id: String,
    target_session_id: String,
    strategy_kind: String,
    payload_kind: String,
    governance_decision_kind: String,
    promoted_message_length: usize,
    promoted_message_hash: String,
    promoted_at: String,
}

struct NormalizedShadowRunInput {
    session_id: String,
    user_input_checksum: Option<String>,
    descriptor_kind: String,
    required_promotions: Option<usize>,
}

struct NormalizedCutoverCandidateInput {
    session_id: String,
    user_input_checksum: Option<String>,
    descriptor_kind: String,
    required_promotions: Option<usize>,
}

fn normalize_promotion_evidence_input(
    input: ControlledPilotPromotionEvidenceInput,
) -> Result<NormalizedPromotionEvidenceInput, String> {
    let pilot_run_id = safe_internal_id(&input.pilot_run_id, "pilotRunId")?;
    let source_session_id = safe_internal_id(&input.source_session_id, "sourceSessionId")?;
    let target_session_id = safe_internal_id(&input.target_session_id, "targetSessionId")?;
    if source_session_id != target_session_id {
        return Err("sourceSessionId must match targetSessionId for promotion evidence".into());
    }
    let strategy_kind = safe_enum_value(
        &input.strategy_kind,
        "strategyKind",
        &["react", "planExecute"],
    )?;
    let payload_kind = safe_enum_value(
        &input.payload_kind,
        "payloadKind",
        &["react", "planExecute", "blocked"],
    )?;
    let governance_decision_kind = safe_enum_value(
        input
            .governance_decision_kind
            .as_deref()
            .unwrap_or("unknown"),
        "governanceDecisionKind",
        &["allow", "warn", "block", "unknown"],
    )?;
    if input.promoted_message_length == 0 {
        return Err("promotedMessageLength must be greater than zero".into());
    }
    let promoted_message_hash = safe_checksum(&input.promoted_message_hash)?;
    let promoted_at = match input.promoted_at.as_deref().map(str::trim) {
        Some(value) if !value.is_empty() => {
            chrono::DateTime::parse_from_rfc3339(value)
                .map_err(|_| "promotedAt must be an RFC3339 timestamp".to_string())?;
            value.to_string()
        }
        _ => chrono::Utc::now().to_rfc3339(),
    };

    Ok(NormalizedPromotionEvidenceInput {
        pilot_run_id,
        source_session_id,
        target_session_id,
        strategy_kind,
        payload_kind,
        governance_decision_kind,
        promoted_message_length: input.promoted_message_length,
        promoted_message_hash,
        promoted_at,
    })
}

fn normalize_shadow_run_input(
    input: ControlledChatMigrationShadowRunInput,
) -> Result<NormalizedShadowRunInput, String> {
    let session_id = safe_internal_id(&input.session_id, "sessionId")?;
    let user_input_checksum = input
        .user_input_checksum
        .as_deref()
        .map(|value| safe_checksum_field(value, "userInputChecksum"))
        .transpose()?;
    let descriptor_kind = match input
        .bounded_test_prompt_descriptor
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(value) => safe_enum_value(
            value,
            "boundedTestPromptDescriptor",
            &[
                "default_readiness_probe",
                "planning_readiness_probe",
                "sensitive_local_only_probe",
            ],
        )?,
        None if user_input_checksum.is_some() => "default_readiness_probe".into(),
        None => {
            return Err(
                "userInputChecksum or boundedTestPromptDescriptor is required for shadow run"
                    .into(),
            )
        }
    };

    Ok(NormalizedShadowRunInput {
        session_id,
        user_input_checksum,
        descriptor_kind,
        required_promotions: input.required_promotions,
    })
}

fn normalize_cutover_candidate_input(
    input: ControlledChatCutoverCandidateInput,
) -> Result<NormalizedCutoverCandidateInput, String> {
    let session_id = safe_internal_id(&input.session_id, "sessionId")?;
    let user_input_checksum = input
        .user_input_checksum
        .as_deref()
        .map(|value| safe_checksum_field(value, "userInputChecksum"))
        .transpose()?;
    let descriptor_kind = match input
        .bounded_test_prompt_descriptor
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(value) => safe_enum_value(
            value,
            "boundedTestPromptDescriptor",
            &["default_contract_probe", "concise_response_probe"],
        )?,
        None => "default_contract_probe".into(),
    };

    Ok(NormalizedCutoverCandidateInput {
        session_id,
        user_input_checksum,
        descriptor_kind,
        required_promotions: input.required_promotions,
    })
}

fn promotion_evidence_pilot_run_id(
    record: &openlife_core::agent::EvidenceRecord,
) -> Option<String> {
    record
        .run_metadata
        .get("pilotRunId")
        .and_then(|value| value.as_str())
        .map(ToOwned::to_owned)
        .or_else(|| record.linked_agent_run_ids.first().cloned())
}

fn promotion_evidence_timestamp(record: &openlife_core::agent::EvidenceRecord) -> String {
    record
        .run_metadata
        .get("promotedAt")
        .and_then(|value| value.as_str())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| record.created_at.to_rfc3339())
}

fn promotion_evidence_is_metadata_safe(record: &openlife_core::agent::EvidenceRecord) -> bool {
    if record.affected_path != CONTROLLED_PILOT_PROMOTION_EVIDENCE_PATH
        || record.evidence_type != EvidenceType::RuntimeBehavior
    {
        return false;
    }
    let metadata = &record.run_metadata;
    let expected_flags = metadata
        .get("evidenceKind")
        .and_then(Value::as_str)
        .is_some_and(|value| value == "controlled_pilot_promotion")
        && metadata
            .get("metadataSafe")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        && metadata
            .get("contentStorage")
            .and_then(Value::as_str)
            .is_some_and(|value| value == "checksum_only")
        && metadata
            .get("toolStorage")
            .and_then(Value::as_str)
            .is_some_and(|value| value == "none");

    expected_flags
        && promotion_evidence_pilot_run_id(record).is_some()
        && metadata_string_is_safe(metadata, "pilotRunId", safe_internal_id)
        && metadata_string_is_safe(metadata, "sourceSessionId", safe_internal_id)
        && metadata_string_is_safe(metadata, "targetSessionId", safe_internal_id)
        && metadata_string_is_safe(metadata, "strategyKind", |value, field| {
            safe_enum_value(value, field, &["react", "planExecute"])
        })
        && metadata_string_is_safe(metadata, "payloadKind", |value, field| {
            safe_enum_value(value, field, &["react", "planExecute", "blocked"])
        })
        && metadata_string_is_safe(metadata, "promotedMessageHash", |value, _field| {
            safe_checksum(value)
        })
        && !contains_unsafe_promotion_metadata(metadata)
}

fn migration_review_decision_evidence_is_metadata_safe(
    record: &openlife_core::agent::EvidenceRecord,
) -> bool {
    if record.affected_path != CONTROLLED_CHAT_MIGRATION_REVIEW_DECISION_EVIDENCE_PATH
        || record.evidence_type != EvidenceType::RuntimeBehavior
        || !record.linked_agent_run_ids.is_empty()
        || !record.linked_proposal_ids.is_empty()
    {
        return false;
    }
    let metadata = &record.run_metadata;
    metadata
        .get("evidenceKind")
        .and_then(Value::as_str)
        .is_some_and(|value| value == "migration_review_decision")
        && metadata
            .get("metadataSafe")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        && metadata
            .get("reviewerNoteStorage")
            .and_then(Value::as_str)
            .is_some_and(|value| value == "length_checksum_category_only")
        && metadata
            .get("toolStorage")
            .and_then(Value::as_str)
            .is_some_and(|value| value == "none")
        && metadata
            .get("transcriptStorage")
            .and_then(Value::as_str)
            .is_some_and(|value| value == "none")
        && metadata_bool_is_present(metadata, "draftReady")
        && metadata_string_is_safe(metadata, "decisionKind", |value, field| {
            safe_enum_value(value, field, &["approve", "reject", "request_rework"])
        })
        && metadata_string_is_safe(metadata, "draftHash", |value, _field| safe_checksum(value))
        && metadata.get("createdAt").and_then(Value::as_str).is_some()
        && metadata
            .get("readinessCounts")
            .and_then(Value::as_object)
            .is_some()
        && reviewer_note_metadata_is_safe(metadata.get("reviewerNote"))
        && !contains_unsafe_promotion_metadata(metadata)
}

fn metadata_bool_is_present(metadata: &Value, key: &str) -> bool {
    metadata.get(key).and_then(Value::as_bool).is_some()
}

fn reviewer_note_metadata_is_safe(value: Option<&Value>) -> bool {
    let Some(Value::Object(note)) = value else {
        return false;
    };
    let category_is_bounded = note
        .get("category")
        .and_then(Value::as_str)
        .is_some_and(|value| matches!(value, "none" | "brief" | "standard" | "extended"));
    let checksum_is_safe = match note.get("checksum") {
        Some(Value::Null) => true,
        Some(Value::String(value)) => safe_checksum(value).is_ok(),
        _ => false,
    };

    note.get("length").and_then(Value::as_u64).is_some()
        && note.get("present").and_then(Value::as_bool).is_some()
        && category_is_bounded
        && checksum_is_safe
}

fn migration_review_decision_kind(record: &openlife_core::agent::EvidenceRecord) -> Option<&str> {
    record
        .run_metadata
        .get("decisionKind")
        .and_then(Value::as_str)
}

fn migration_review_latest_decision(
    record: &openlife_core::agent::EvidenceRecord,
) -> Option<ControlledChatMigrationReviewLatestDecision> {
    Some(ControlledChatMigrationReviewLatestDecision {
        evidence_id: record.id.clone(),
        decision_kind: migration_review_decision_kind(record)?.to_string(),
        draft_ready: record
            .run_metadata
            .get("draftReady")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        draft_hash: record
            .run_metadata
            .get("draftHash")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)?,
        created_at: migration_review_decision_timestamp(record),
    })
}

fn migration_review_decision_timestamp(record: &openlife_core::agent::EvidenceRecord) -> String {
    record
        .run_metadata
        .get("createdAt")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| record.created_at.to_rfc3339())
}

fn migration_review_decision_blocking_reasons(
    record: &openlife_core::agent::EvidenceRecord,
) -> Vec<String> {
    record
        .run_metadata
        .get("blockingReasons")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

struct ShadowReviewReadiness {
    digest: String,
    blocking_reasons: Vec<String>,
}

struct CutoverCandidateReviewReadiness {
    digest: String,
    contract_shape: String,
    candidate_ready: bool,
    blocking_reasons: Vec<String>,
}

async fn load_shadow_review_run(
    state: &Arc<AppState>,
    shadow_run_id: &str,
) -> Result<Option<AgentRun>, String> {
    let Some(store_arc) = state.agent_run_store.as_ref() else {
        return Ok(None);
    };
    let store = store_arc.lock().await;
    store
        .get_run(shadow_run_id)
        .map_err(|e| format!("failed to read shadow AgentRun for review: {e}"))
}

fn shadow_review_readiness(run: Option<&AgentRun>) -> Result<ShadowReviewReadiness, String> {
    let Some(run) = run else {
        let summary = json!({
            "runFound": false,
            "metadataSafe": true,
            "sideEffectAuditReady": false,
        });
        return Ok(ShadowReviewReadiness {
            digest: metadata_hash_for_serializable(&summary)?,
            blocking_reasons: vec!["shadow_run_missing".into()],
        });
    };

    let audit = run
        .reasoning_trace
        .as_ref()
        .and_then(|trace| trace.strategy_result.as_ref());
    let metadata_safe = audit_bool(audit, "metadataSafe").unwrap_or(false);
    let allow_writes = shadow_review_allow_writes(audit);
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
        && allow_writes == Some(false);

    let summary = json!({
        "runFound": true,
        "shadowRunId": run.id,
        "reasoningStrategy": run.reasoning_strategy.as_deref().unwrap_or("missing"),
        "status": run.status.to_string(),
        "metadataSafe": metadata_safe,
        "allowWrites": allow_writes,
        "contentStorage": storage("contentStorage"),
        "toolStorage": storage("toolStorage"),
        "chatHistoryStorage": storage("chatHistoryStorage"),
        "proposalStorage": storage("proposalStorage"),
        "lifeModelPatchStorage": storage("lifeModelPatchStorage"),
        "memoryStorage": storage("memoryStorage"),
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

    if run.reasoning_strategy.as_deref() != Some("controlled_migration_shadow_run") {
        push_unique_string(&mut blocking_reasons, "shadow_run_strategy_mismatch".into());
    }
    if run.status != AgentRunStatus::Completed {
        push_unique_string(&mut blocking_reasons, "shadow_run_not_completed".into());
    }
    if audit.is_none() {
        push_unique_string(&mut blocking_reasons, "shadow_run_audit_missing".into());
    }
    if !metadata_safe {
        push_unique_string(&mut blocking_reasons, "shadow_run_metadata_not_safe".into());
    }
    if allow_writes != Some(false) {
        push_unique_string(
            &mut blocking_reasons,
            "shadow_run_allow_writes_not_false".into(),
        );
    }
    for (key, reason) in [
        ("contentStorage", "shadow_run_content_storage_not_none"),
        ("toolStorage", "shadow_run_tool_storage_not_none"),
        (
            "chatHistoryStorage",
            "shadow_run_chat_history_storage_not_none",
        ),
        ("proposalStorage", "shadow_run_proposal_storage_not_none"),
        (
            "lifeModelPatchStorage",
            "shadow_run_life_model_patch_storage_not_none",
        ),
        ("memoryStorage", "shadow_run_memory_storage_not_none"),
    ] {
        if audit_string(audit, key) != Some("none") {
            push_unique_string(&mut blocking_reasons, reason.into());
        }
    }
    if run.user_input.is_some() {
        push_unique_string(
            &mut blocking_reasons,
            "shadow_run_user_input_persisted".into(),
        );
    }
    if !run.generated_proposals.is_empty()
        || proposal_id_count > 0
        || proposal_required_step_count > 0
    {
        push_unique_string(
            &mut blocking_reasons,
            "shadow_run_proposal_side_effects_present".into(),
        );
    }
    if !run.actions.is_empty()
        || !run.observations.is_empty()
        || run.tool_call_count > 0
        || declared_write_step_count > 0
    {
        push_unique_string(
            &mut blocking_reasons,
            "shadow_run_external_write_side_effects_present".into(),
        );
    }

    Ok(ShadowReviewReadiness {
        digest: metadata_hash_for_serializable(&summary)?,
        blocking_reasons,
    })
}

async fn load_cutover_candidate_review_run(
    state: &Arc<AppState>,
    candidate_run_id: &str,
) -> Result<Option<AgentRun>, String> {
    let Some(store_arc) = state.agent_run_store.as_ref() else {
        return Ok(None);
    };
    let store = store_arc.lock().await;
    store
        .get_run(candidate_run_id)
        .map_err(|e| format!("failed to read cutover candidate AgentRun for review: {e}"))
}

fn cutover_candidate_review_readiness(
    run: Option<&AgentRun>,
) -> Result<CutoverCandidateReviewReadiness, String> {
    let Some(run) = run else {
        let summary = json!({
            "runFound": false,
            "metadataSafe": true,
            "sideEffectAuditReady": false,
        });
        return Ok(CutoverCandidateReviewReadiness {
            digest: metadata_hash_for_serializable(&summary)?,
            contract_shape: "missing".into(),
            candidate_ready: false,
            blocking_reasons: vec!["candidate_run_missing".into()],
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
    let candidate_ready = audit_bool(audit, "candidateReady").unwrap_or(false);
    let allow_writes = cutover_candidate_review_allow_writes(audit);
    let max_tool_calls = cutover_candidate_review_max_tool_calls(audit);
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
        && max_tool_calls == Some(0);

    let summary = json!({
        "runFound": true,
        "candidateRunId": run.id,
        "reasoningStrategy": run.reasoning_strategy.as_deref().unwrap_or("missing"),
        "status": run.status.to_string(),
        "contractShape": contract_shape.clone(),
        "candidateReady": candidate_ready,
        "metadataSafe": metadata_safe,
        "allowWrites": allow_writes,
        "maxToolCalls": max_tool_calls,
        "contentStorage": storage("contentStorage"),
        "toolStorage": storage("toolStorage"),
        "chatHistoryStorage": storage("chatHistoryStorage"),
        "proposalStorage": storage("proposalStorage"),
        "lifeModelPatchStorage": storage("lifeModelPatchStorage"),
        "memoryStorage": storage("memoryStorage"),
        "evidenceStorage": storage("evidenceStorage"),
        "mcpAuditStorage": storage("mcpAuditStorage"),
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

    if run.reasoning_strategy.as_deref() != Some("controlled_chat_cutover_candidate") {
        push_unique_string(
            &mut blocking_reasons,
            "candidate_run_strategy_mismatch".into(),
        );
    }
    if run.status != AgentRunStatus::Completed {
        push_unique_string(&mut blocking_reasons, "candidate_run_not_completed".into());
    }
    if audit.is_none() {
        push_unique_string(&mut blocking_reasons, "candidate_run_audit_missing".into());
    }
    if !contract_shape_allowed {
        push_unique_string(
            &mut blocking_reasons,
            "candidate_run_contract_shape_invalid".into(),
        );
    }
    if !metadata_safe {
        push_unique_string(
            &mut blocking_reasons,
            "candidate_run_metadata_not_safe".into(),
        );
    }
    if allow_writes != Some(false) {
        push_unique_string(
            &mut blocking_reasons,
            "candidate_run_allow_writes_not_false".into(),
        );
    }
    if max_tool_calls != Some(0) {
        push_unique_string(
            &mut blocking_reasons,
            "candidate_run_max_tool_calls_not_zero".into(),
        );
    }
    for (key, reason) in [
        ("contentStorage", "candidate_run_content_storage_not_none"),
        ("toolStorage", "candidate_run_tool_storage_not_none"),
        (
            "chatHistoryStorage",
            "candidate_run_chat_history_storage_not_none",
        ),
        ("proposalStorage", "candidate_run_proposal_storage_not_none"),
        (
            "lifeModelPatchStorage",
            "candidate_run_life_model_patch_storage_not_none",
        ),
        ("memoryStorage", "candidate_run_memory_storage_not_none"),
        ("evidenceStorage", "candidate_run_evidence_storage_not_none"),
        (
            "mcpAuditStorage",
            "candidate_run_mcp_audit_storage_not_none",
        ),
    ] {
        if audit_string(audit, key) != Some("none") {
            push_unique_string(&mut blocking_reasons, reason.into());
        }
    }
    if run.user_input.is_some() {
        push_unique_string(
            &mut blocking_reasons,
            "candidate_run_user_input_persisted".into(),
        );
    }
    if !run.generated_proposals.is_empty()
        || proposal_id_count > 0
        || proposal_required_step_count > 0
    {
        push_unique_string(
            &mut blocking_reasons,
            "candidate_run_proposal_side_effects_present".into(),
        );
    }
    if !run.actions.is_empty()
        || !run.observations.is_empty()
        || run.tool_call_count > 0
        || declared_write_step_count > 0
    {
        push_unique_string(
            &mut blocking_reasons,
            "candidate_run_external_write_side_effects_present".into(),
        );
    }

    Ok(CutoverCandidateReviewReadiness {
        digest: metadata_hash_for_serializable(&summary)?,
        contract_shape,
        candidate_ready,
        blocking_reasons,
    })
}

struct CutoverReadinessMetadataSafeSummaryInput<'a> {
    cutover_planning_eligible: bool,
    required_evidence_ready: bool,
    default_chat_unchanged: bool,
    implementation_eligible: bool,
    latest_shadow_decision_kind: &'a str,
    shadow_run_ready: bool,
    verified_shadow_run_id: Option<&'a str>,
    readiness_summary_digest: Option<&'a str>,
    shadow_review_summary: &'a ControlledChatMigrationShadowReviewSummary,
}

struct CutoverCandidatePromotionReadinessMetadataSafeSummaryInput<'a> {
    ready: bool,
    cutover_readiness_eligible: bool,
    required_approved_candidates: usize,
    approved_candidate_count: usize,
    latest_decision_kind: &'a str,
    default_chat_unchanged: bool,
    verified_candidate_count: usize,
    blocking_reason_count: usize,
}

fn cutover_readiness_metadata_safe_summary(
    input: CutoverReadinessMetadataSafeSummaryInput<'_>,
) -> Value {
    json!({
        "cutoverReadinessGate": "controlled_chat_cutover_planning",
        "metadataSafe": true,
        "planningOnly": true,
        "notAutomaticMigration": true,
        "cutoverPlanningEligible": input.cutover_planning_eligible,
        "requiredEvidenceReady": input.required_evidence_ready,
        "defaultChatUnchanged": input.default_chat_unchanged,
        "implementationEligible": input.implementation_eligible,
        "latestShadowReviewDecisionKind": input.latest_shadow_decision_kind,
        "shadowRunReady": input.shadow_run_ready,
        "verifiedShadowRunId": input.verified_shadow_run_id.unwrap_or("none"),
        "readinessSummaryDigest": input.readiness_summary_digest.unwrap_or("none"),
        "approvedShadowReviewCount": input.shadow_review_summary.approved_count,
        "shadowReviewReworkRejectCount": input.shadow_review_summary.rework_reject_count,
        "contentStorage": "none",
        "toolStorage": "none",
        "chatHistoryStorage": "none",
        "proposalStorage": "none",
        "lifeModelPatchStorage": "none",
        "memoryStorage": "none",
        "reviewerNoteStorage": "length_checksum_category_only",
        "transcriptStorage": "none",
    })
}

fn cutover_candidate_promotion_readiness_metadata_safe_summary(
    input: CutoverCandidatePromotionReadinessMetadataSafeSummaryInput<'_>,
) -> Value {
    json!({
        "promotionReadinessGate": "controlled_chat_cutover_candidate",
        "metadataSafe": true,
        "readOnly": true,
        "notAutomaticMigration": true,
        "ready": input.ready,
        "cutoverReadinessEligible": input.cutover_readiness_eligible,
        "requiredApprovedCandidates": input.required_approved_candidates,
        "approvedCandidateCount": input.approved_candidate_count,
        "verifiedCandidateCount": input.verified_candidate_count,
        "latestDecisionKind": input.latest_decision_kind,
        "defaultChatUnchanged": input.default_chat_unchanged,
        "blockingReasonCount": input.blocking_reason_count,
        "contentStorage": "none",
        "toolStorage": "none",
        "chatHistoryStorage": "none",
        "proposalStorage": "none",
        "lifeModelPatchStorage": "none",
        "memoryStorage": "none",
        "evidenceStorage": "read_only",
        "mcpAuditStorage": "none",
        "reviewerNoteStorage": "length_checksum_category_only",
        "transcriptStorage": "none",
    })
}

fn cutover_candidate_blocked_summary(
    descriptor_kind: &str,
    user_input_checksum: Option<&str>,
) -> Value {
    json!({
        "candidateAdapter": "controlled_chat_cutover_candidate",
        "descriptorKind": descriptor_kind,
        "userInputChecksumPresent": user_input_checksum.is_some(),
        "candidateReady": false,
        "contractShape": "blocked",
        "blockedBeforeRuntime": true,
        "allowWrites": false,
        "maxToolCalls": 0,
        "metadataSafe": true,
        "nonDefault": true,
        "defaultChatUnchanged": true,
        "contentStorage": "none",
        "toolStorage": "none",
        "chatHistoryStorage": "none",
        "proposalStorage": "none",
        "lifeModelPatchStorage": "none",
        "memoryStorage": "none",
        "evidenceStorage": "none",
        "mcpAuditStorage": "none",
    })
}

fn cutover_candidate_failed_summary(
    descriptor_kind: &str,
    user_input_checksum: Option<&str>,
    safe_error: &str,
) -> Value {
    json!({
        "candidateAdapter": "controlled_chat_cutover_candidate",
        "descriptorKind": descriptor_kind,
        "userInputChecksumPresent": user_input_checksum.is_some(),
        "candidateReady": false,
        "contractShape": "failed",
        "candidateErrorCode": cutover_candidate_error_code(safe_error),
        "allowWrites": false,
        "maxToolCalls": 0,
        "metadataSafe": true,
        "nonDefault": true,
        "defaultChatUnchanged": true,
        "contentStorage": "none",
        "toolStorage": "none",
        "chatHistoryStorage": "none",
        "proposalStorage": "none",
        "lifeModelPatchStorage": "none",
        "memoryStorage": "none",
        "evidenceStorage": "none",
        "mcpAuditStorage": "none",
    })
}

fn cutover_candidate_metadata_safe_summary(
    output: &MultiStrategyRuntimeOutput,
    descriptor_kind: &str,
    user_input_checksum: Option<&str>,
    contract_shape: &str,
    candidate_ready: bool,
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
        "candidateAdapter": "controlled_chat_cutover_candidate",
        "descriptorKind": descriptor_kind,
        "userInputChecksumPresent": user_input_checksum.is_some(),
        "contractShape": contract_shape,
        "candidateReady": candidate_ready,
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
        "userOutputPresent": cutover_candidate_user_output(output).is_some(),
        "outputDigestPresent": output_digest.is_some(),
        "allowWrites": false,
        "maxToolCalls": 0,
        "metadataSafe": true,
        "nonDefault": true,
        "defaultChatUnchanged": true,
        "proposalApply": false,
        "contentStorage": "none",
        "toolStorage": "none",
        "chatHistoryStorage": "none",
        "proposalStorage": "none",
        "lifeModelPatchStorage": "none",
        "memoryStorage": "none",
        "evidenceStorage": "none",
        "mcpAuditStorage": "none",
    })
}

fn cutover_candidate_audit_summary(
    output: &MultiStrategyRuntimeOutput,
    warnings: &[String],
    descriptor_kind: &str,
    user_input_checksum: Option<&str>,
    contract_shape: &str,
    candidate_ready: bool,
    output_digest: Option<&str>,
) -> Value {
    let mut write_control = preview_write_control(&output.payload);
    if let Some(map) = write_control.as_object_mut() {
        map.insert("allowWrites".into(), Value::Bool(false));
    }
    let metadata = cutover_candidate_metadata_safe_summary(
        output,
        descriptor_kind,
        user_input_checksum,
        contract_shape,
        candidate_ready,
        output_digest,
    );
    json!({
        "candidateAdapter": "controlled_chat_cutover_candidate",
        "strategyKind": metadata["strategyKind"],
        "payloadKind": metadata["payloadKind"],
        "contractShape": contract_shape,
        "candidateReady": candidate_ready,
        "governanceDecisionKind": metadata["governanceDecisionKind"],
        "taskKind": metadata["taskKind"],
        "reasonCode": metadata["reasonCode"],
        "riskLevel": metadata["riskLevel"],
        "hasHsPacket": metadata["hasHsPacket"],
        "descriptorKind": descriptor_kind,
        "userInputChecksumPresent": user_input_checksum.is_some(),
        "planStepCount": metadata["planStepCount"],
        "planStepStatuses": preview_plan_step_statuses(&output.payload),
        "proposalIdCount": metadata["proposalIdCount"],
        "blocked": metadata["blocked"],
        "userOutputPresent": metadata["userOutputPresent"],
        "outputDigest": output_digest,
        "warnings": warnings,
        "metadataSafe": true,
        "nonDefault": true,
        "defaultChatUnchanged": true,
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
        "writeControl": write_control,
    })
}

fn shadow_review_decision_evidence_is_metadata_safe(
    record: &openlife_core::agent::EvidenceRecord,
) -> bool {
    if record.affected_path != CONTROLLED_CHAT_MIGRATION_SHADOW_REVIEW_DECISION_EVIDENCE_PATH
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
        "shadowRunId",
        "decisionKind",
        "reviewerNoteChecksum",
        "reviewerNoteLength",
        "reviewerNoteCategory",
        "readinessSummaryDigest",
        "createdAt",
    ];
    if metadata.len() != allowed.len()
        || !metadata.keys().all(|key| allowed.contains(&key.as_str()))
    {
        return false;
    }

    metadata_string_is_safe(&record.run_metadata, "shadowRunId", safe_internal_id)
        && metadata_string_is_safe(&record.run_metadata, "decisionKind", |value, field| {
            safe_enum_value(value, field, &["approve", "reject", "request_rework"])
        })
        && reviewer_note_flat_metadata_is_safe(&record.run_metadata)
        && metadata_string_is_safe(
            &record.run_metadata,
            "readinessSummaryDigest",
            |value, _| safe_checksum(value),
        )
        && record
            .run_metadata
            .get("createdAt")
            .and_then(Value::as_str)
            .is_some_and(|value| chrono::DateTime::parse_from_rfc3339(value).is_ok())
        && !contains_unsafe_promotion_metadata(&record.run_metadata)
}

fn shadow_review_decision_kind(record: &openlife_core::agent::EvidenceRecord) -> Option<&str> {
    record
        .run_metadata
        .get("decisionKind")
        .and_then(Value::as_str)
}

fn shadow_review_latest_decision(
    record: &openlife_core::agent::EvidenceRecord,
) -> Option<ControlledChatMigrationShadowReviewLatestDecision> {
    Some(ControlledChatMigrationShadowReviewLatestDecision {
        evidence_id: record.id.clone(),
        shadow_run_id: record
            .run_metadata
            .get("shadowRunId")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)?,
        decision_kind: shadow_review_decision_kind(record)?.to_string(),
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
        readiness_summary_digest: record
            .run_metadata
            .get("readinessSummaryDigest")
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

fn cutover_candidate_review_decision_evidence_is_metadata_safe(
    record: &openlife_core::agent::EvidenceRecord,
) -> bool {
    if record.affected_path != CONTROLLED_CHAT_CUTOVER_CANDIDATE_REVIEW_DECISION_EVIDENCE_PATH
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
        "candidateRunId",
        "decisionKind",
        "contractShape",
        "candidateSummaryDigest",
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

    metadata_string_is_safe(&record.run_metadata, "candidateRunId", safe_internal_id)
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
        && metadata_string_is_safe(
            &record.run_metadata,
            "candidateSummaryDigest",
            |value, _| safe_checksum(value),
        )
        && reviewer_note_flat_metadata_is_safe(&record.run_metadata)
        && record
            .run_metadata
            .get("createdAt")
            .and_then(Value::as_str)
            .is_some_and(|value| chrono::DateTime::parse_from_rfc3339(value).is_ok())
        && !contains_unsafe_promotion_metadata(&record.run_metadata)
}

fn cutover_candidate_review_decision_kind(
    record: &openlife_core::agent::EvidenceRecord,
) -> Option<&str> {
    record
        .run_metadata
        .get("decisionKind")
        .and_then(Value::as_str)
}

fn cutover_candidate_review_latest_decision(
    record: &openlife_core::agent::EvidenceRecord,
) -> Option<ControlledChatCutoverCandidateReviewLatestDecision> {
    Some(ControlledChatCutoverCandidateReviewLatestDecision {
        evidence_id: record.id.clone(),
        candidate_run_id: record
            .run_metadata
            .get("candidateRunId")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)?,
        decision_kind: cutover_candidate_review_decision_kind(record)?.to_string(),
        contract_shape: record
            .run_metadata
            .get("contractShape")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)?,
        candidate_summary_digest: record
            .run_metadata
            .get("candidateSummaryDigest")
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

fn shadow_review_allow_writes(audit: Option<&Value>) -> Option<bool> {
    audit_bool_at(audit, &["writeControl", "allowWrites"])
        .or_else(|| audit_bool(audit, "allowWrites"))
}

fn cutover_candidate_review_allow_writes(audit: Option<&Value>) -> Option<bool> {
    audit_bool_at(audit, &["runtimeLimits", "allowWrites"])
        .or_else(|| audit_bool_at(audit, &["writeControl", "allowWrites"]))
        .or_else(|| audit_bool(audit, "allowWrites"))
}

fn cutover_candidate_review_max_tool_calls(audit: Option<&Value>) -> Option<u64> {
    audit_u64_at(audit, &["runtimeLimits", "maxToolCalls"])
        .or_else(|| audit_u64(audit, "maxToolCalls"))
}

fn shadow_blocked_summary(descriptor_kind: &str, user_input_checksum: Option<&str>) -> Value {
    json!({
        "shadowRunRuntime": "controlled_chat_migration",
        "descriptorKind": descriptor_kind,
        "userInputChecksumPresent": user_input_checksum.is_some(),
        "blockedBeforeRuntime": true,
        "allowWrites": false,
        "metadataSafe": true,
        "contentStorage": "none",
        "toolStorage": "none",
        "chatHistoryStorage": "none",
    })
}

fn shadow_failed_summary(
    descriptor_kind: &str,
    user_input_checksum: Option<&str>,
    safe_error: &str,
) -> Value {
    json!({
        "shadowRunRuntime": "controlled_chat_migration",
        "descriptorKind": descriptor_kind,
        "userInputChecksumPresent": user_input_checksum.is_some(),
        "shadowErrorCode": shadow_error_code(safe_error),
        "allowWrites": false,
        "metadataSafe": true,
        "contentStorage": "none",
        "toolStorage": "none",
        "chatHistoryStorage": "none",
    })
}

fn shadow_metadata_safe_summary(
    output: &MultiStrategyRuntimeOutput,
    descriptor_kind: &str,
    user_input_checksum: Option<&str>,
) -> Value {
    let metadata = &output.selection.metadata_safe_summary;
    let governance_decision_kind = output
        .selection
        .governance_decision
        .as_ref()
        .map(|decision| preview_governance_decision_kind(decision.kind))
        .unwrap_or("unknown");
    json!({
        "shadowRunRuntime": "controlled_chat_migration",
        "descriptorKind": descriptor_kind,
        "userInputChecksumPresent": user_input_checksum.is_some(),
        "strategyKind": preview_strategy_kind(output.selection.kind),
        "payloadKind": preview_payload_kind(&output.payload),
        "governanceDecisionKind": governance_decision_kind,
        "taskKind": metadata.get("taskKind").and_then(Value::as_str).unwrap_or("unknown"),
        "reasonCode": metadata.get("reasonCode").and_then(Value::as_str).unwrap_or("unknown"),
        "riskLevel": metadata.get("riskLevel").and_then(Value::as_str).unwrap_or("unknown"),
        "hasHsPacket": metadata.get("hasHsPacket").and_then(Value::as_bool).unwrap_or(false),
        "planStepCount": preview_plan_step_count(&output.payload),
        "blocked": matches!(output.payload, MultiStrategyRuntimePayload::Blocked),
        "allowWrites": false,
        "metadataSafe": true,
        "contentStorage": "none",
        "toolStorage": "none",
        "chatHistoryStorage": "none",
        "proposalStorage": "none",
        "lifeModelPatchStorage": "none",
        "memoryStorage": "none",
    })
}

fn shadow_audit_summary(
    output: &MultiStrategyRuntimeOutput,
    warnings: &[String],
    descriptor_kind: &str,
    user_input_checksum: Option<&str>,
) -> Value {
    let mut write_control = preview_write_control(&output.payload);
    if let Some(map) = write_control.as_object_mut() {
        map.insert("allowWrites".into(), Value::Bool(false));
    }
    let metadata = shadow_metadata_safe_summary(output, descriptor_kind, user_input_checksum);
    json!({
        "shadowRunRuntime": "controlled_chat_migration",
        "strategyKind": metadata["strategyKind"],
        "payloadKind": metadata["payloadKind"],
        "governanceDecisionKind": metadata["governanceDecisionKind"],
        "taskKind": metadata["taskKind"],
        "reasonCode": metadata["reasonCode"],
        "riskLevel": metadata["riskLevel"],
        "hasHsPacket": metadata["hasHsPacket"],
        "descriptorKind": descriptor_kind,
        "userInputChecksumPresent": user_input_checksum.is_some(),
        "planStepCount": metadata["planStepCount"],
        "planStepStatuses": preview_plan_step_statuses(&output.payload),
        "proposalIdCount": preview_proposal_ids(&output.payload).len(),
        "blocked": metadata["blocked"],
        "warnings": warnings,
        "metadataSafe": true,
        "contentStorage": "none",
        "toolStorage": "none",
        "chatHistoryStorage": "none",
        "proposalStorage": "none",
        "lifeModelPatchStorage": "none",
        "memoryStorage": "none",
        "writeControl": write_control,
    })
}

fn shadow_prompt_for_descriptor(descriptor_kind: &str) -> &'static str {
    match descriptor_kind {
        "planning_readiness_probe" => "Plan a controlled migration comparison.",
        "sensitive_local_only_probe" => "Discuss a sensitive local-only readiness check.",
        "default_readiness_probe" => {
            "Compare default chat contract with controlled runtime readiness."
        }
        _ => "Compare default chat contract with controlled runtime readiness.",
    }
}

fn cutover_candidate_prompt_for_descriptor(descriptor_kind: &str) -> &'static str {
    match descriptor_kind {
        "concise_response_probe" => "Provide a concise default Chat compatible response.",
        "default_contract_probe" => {
            "Provide a concise default Chat compatible response for a controlled runtime probe."
        }
        _ => "Provide a concise default Chat compatible response for a controlled runtime probe.",
    }
}

fn cutover_candidate_user_output(output: &MultiStrategyRuntimeOutput) -> Option<String> {
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

fn cutover_candidate_contract_shape(output: &MultiStrategyRuntimeOutput) -> &'static str {
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

fn cutover_candidate_contract_blockers(
    output: &MultiStrategyRuntimeOutput,
    contract_shape: &str,
) -> Vec<String> {
    let mut blocking_reasons = Vec::new();
    match &output.payload {
        MultiStrategyRuntimePayload::Blocked => {
            push_unique_string(&mut blocking_reasons, "candidate_runtime_blocked".into());
        }
        MultiStrategyRuntimePayload::PlanExecute(_) => {
            push_unique_string(
                &mut blocking_reasons,
                "candidate_runtime_returned_non_chat_payload".into(),
            );
        }
        MultiStrategyRuntimePayload::ReAct(runtime_output) => {
            if runtime_output.user_output.trim().is_empty() {
                push_unique_string(
                    &mut blocking_reasons,
                    "candidate_user_output_missing".into(),
                );
            }
            if !runtime_output.proposal_ids.is_empty() {
                push_unique_string(
                    &mut blocking_reasons,
                    "candidate_proposal_ids_present".into(),
                );
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
            "candidate_write_or_proposal_step_present".into(),
        );
    }
    if contract_shape == "failed" && blocking_reasons.is_empty() {
        push_unique_string(
            &mut blocking_reasons,
            "candidate_contract_shape_failed".into(),
        );
    }

    blocking_reasons
}

fn cutover_candidate_output_label(output: &MultiStrategyRuntimeOutput) -> String {
    format!(
        "Cutover candidate: {} / {}",
        preview_strategy_kind(output.selection.kind),
        preview_payload_kind(&output.payload)
    )
}
