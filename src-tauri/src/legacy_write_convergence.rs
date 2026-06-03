#![allow(dead_code)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LegacyWriteRiskClass {
    CompatibilityMaterializer,
    HighRiskLegacyDirectWrite,
    ProposalFirstConvergenceTarget,
    ProposalOnlyDeclarative,
    LowRiskTransientState,
    LowRiskSourceData,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LegacyWriteConvergenceStatus {
    CompatibilityPrimitive,
    LegacyDirectWriteBlocker,
    AlreadyProposalFirst,
    ProposalFirstConvergenceTarget,
    ProposalOnlyDeclarative,
    LowRiskTransientSourceData,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LegacyWritePathKind {
    LifeModelSavePrimitive,
    ManualLifeModelEditor,
    BuilderNormalProposalFlow,
    BuilderLegacyDirectApply,
    CalibrationProposalFlow,
    CalibrationDirectMicroEvolution,
    FeedbackSignalsSourceData,
    FeedbackEvolutionDirectWrites,
    FeedbackEvolutionReadOnlyReport,
    SnapshotRestore,
    DataImport,
    StateDailyGoalDirectWrites,
    RawChatMemoryVectorSourceWrites,
    ProposalApplicationPath,
    ExternalWriteProposalPath,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LegacyWriteSafeModeStatus {
    CompatibilityPrimitiveGuardRequired,
    GuardRequired,
    GuardPresent,
    ProposalFirstGuardPresent,
    LowRiskSourceDataGuardNotRequired,
    ProposalOnlyDeclarative,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LegacyWriteInventoryEntry {
    pub(crate) stable_id: String,
    pub(crate) display_name: String,
    pub(crate) path_kind: LegacyWritePathKind,
    pub(crate) source_file_paths: Vec<String>,
    pub(crate) command_function_names: Vec<String>,
    pub(crate) risk_class: LegacyWriteRiskClass,
    pub(crate) current_status: LegacyWriteConvergenceStatus,
    pub(crate) normal_product_allowed: bool,
    pub(crate) requires_proposal_first: bool,
    pub(crate) currently_direct_write: bool,
    pub(crate) high_risk_durable_truth_write: bool,
    pub(crate) safe_mode_status: LegacyWriteSafeModeStatus,
    pub(crate) requires_safe_mode_guard: bool,
    pub(crate) default_chat_affected: bool,
    pub(crate) provider_execution_enabled: bool,
    pub(crate) current_guard_summary: String,
    pub(crate) required_convergence_action: String,
    pub(crate) next_recommended_slice: String,
    pub(crate) blocking_reasons: Vec<String>,
    pub(crate) metadata_safe: bool,
    pub(crate) contains_raw_content: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LegacyWriteConvergenceReport {
    pub(crate) inventory_ready: bool,
    pub(crate) guard_ready: bool,
    pub(crate) overall_converged: bool,
    pub(crate) all_direct_writes_converged: bool,
    pub(crate) metadata_safe: bool,
    pub(crate) contains_raw_content: bool,
    pub(crate) default_chat_unchanged: bool,
    pub(crate) default_chat_route: String,
    pub(crate) w79_guard_called_by_ordinary_chat: bool,
    pub(crate) w73_w78_maturation_helper_listed_as_chat_write_path: bool,
    pub(crate) proposal_first_targets_clean: bool,
    pub(crate) external_provider_execution_enabled: bool,
    pub(crate) calendar_propose_event_provider_executor_enabled: bool,
    pub(crate) email_propose_draft_provider_executor_enabled: bool,
    pub(crate) inventory_entry_count: usize,
    pub(crate) high_risk_legacy_direct_write_count: usize,
    pub(crate) low_risk_transient_or_source_data_count: usize,
    pub(crate) proposal_first_target_count: usize,
    pub(crate) convergence_blockers: Vec<String>,
    pub(crate) guard_blocking_reasons: Vec<String>,
}

pub(crate) fn legacy_write_convergence_inventory() -> Vec<LegacyWriteInventoryEntry> {
    vec![
        entry(
            "lifemodel_save_primitive",
            "LifeModel save primitive",
            LegacyWritePathKind::LifeModelSavePrimitive,
            &[
                "openlife-core/src/life_model.rs",
                "src-tauri/src/lib.rs",
            ],
            &["LifeModelManager::save", "persist_life_model"],
            LegacyWriteRiskClass::CompatibilityMaterializer,
            LegacyWriteConvergenceStatus::CompatibilityPrimitive,
            false,
            false,
            true,
            true,
            LegacyWriteSafeModeStatus::CompatibilityPrimitiveGuardRequired,
            true,
            "Central compatibility materializer for accepted proposal application, migration, and explicit manual override only; not a normal governed write path.",
            "Restrict to accepted proposal application, migration/manual override primitive, and compatibility materialization only.",
            "Future LifeModel materializer caller restriction after manual editor audit.",
            &[
                "compatibility_materializer_still_reachable_by_legacy_callers",
                "must_not_be_marked_normal_governed_write_path",
            ],
        ),
        entry(
            "manual_lifemodel_editor",
            "Manual LifeModel editor",
            LegacyWritePathKind::ManualLifeModelEditor,
            &[
                "src-tauri/src/commands/life_model.rs",
                "frontend/src/pages/LifeModelEditor.tsx",
            ],
            &["save_life_model", "LifeModelEditor.tsx"],
            LegacyWriteRiskClass::HighRiskLegacyDirectWrite,
            LegacyWriteConvergenceStatus::LegacyDirectWriteBlocker,
            false,
            true,
            true,
            true,
            LegacyWriteSafeModeStatus::GuardPresent,
            true,
            "W80 metadata-safe manual override audit records source, before/after hashes, rough changed sections, risk class, timestamp, command, and legacy-direct-write flags; editor save remains a high-risk legacy direct write blocker.",
            "Convert editor saves to proposal patch review or stronger manual override UX with confirmation and richer governance.",
            "Post-W80 manual LifeModel editor proposal-first convergence or stronger manual override UX.",
            &[
                "manual_editor_can_write_durable_lifemodel_truth_directly",
                "proposal_review_not_required_today",
            ],
        ),
        entry(
            "builder_normal_proposal_flow",
            "Builder normal proposal flow",
            LegacyWritePathKind::BuilderNormalProposalFlow,
            &["src-tauri/src/commands/builder.rs"],
            &["builder_create_proposals"],
            LegacyWriteRiskClass::ProposalFirstConvergenceTarget,
            LegacyWriteConvergenceStatus::AlreadyProposalFirst,
            true,
            true,
            false,
            false,
            LegacyWriteSafeModeStatus::ProposalFirstGuardPresent,
            false,
            "builder_create_proposals creates ProposalStore entries for finished Builder sessions with pending signals and does not directly mutate LifeModel truth.",
            "Keep builder_create_proposals as the normal Builder product path and convergence target.",
            "Preserve Builder proposal-first flow while retiring legacy direct apply separately.",
            &[],
        ),
        entry(
            "builder_legacy_direct_apply",
            "Builder legacy direct apply",
            LegacyWritePathKind::BuilderLegacyDirectApply,
            &["src-tauri/src/commands/builder.rs"],
            &[
                "builder_apply_signals",
                "builder_step no-signal completion branch",
            ],
            LegacyWriteRiskClass::HighRiskLegacyDirectWrite,
            LegacyWriteConvergenceStatus::LegacyDirectWriteBlocker,
            false,
            true,
            true,
            true,
            LegacyWriteSafeModeStatus::GuardPresent,
            true,
            "W81 guard present: builder_apply_signals fails closed by default and requires explicit dev/migration override; no-signal completion performs session-only cleanup without writing durable LifeModel truth. This is separate from the normal builder_create_proposals proposal-first path.",
            "Keep builder_create_proposals as the normal proposal-first path; retire builder_apply_signals or convert the remaining legacy direct apply capability to proposal-first before treating Builder as fully converged.",
            "Future Builder legacy direct apply removal or proposal-first conversion.",
            &[
                "builder_apply_signals_can_still_write_durable_lifemodel_truth_with_dev_migration_override",
                "builder_legacy_direct_apply_not_fully_proposal_first",
            ],
        ),
        entry(
            "calibration_proposal_flow",
            "Calibration proposal flow",
            LegacyWritePathKind::CalibrationProposalFlow,
            &["src-tauri/src/commands/calibration.rs"],
            &[
                "calibration_create_proposals",
                "apply_calibration(mode!=direct)",
            ],
            LegacyWriteRiskClass::ProposalFirstConvergenceTarget,
            LegacyWriteConvergenceStatus::AlreadyProposalFirst,
            true,
            true,
            false,
            false,
            LegacyWriteSafeModeStatus::ProposalFirstGuardPresent,
            false,
            "calibration_create_proposals and apply_calibration(mode!=direct) assess risk and create Review Center proposals instead of durable direct writes.",
            "Keep calibration_create_proposals / proposal mode as the product default and convergence target.",
            "Preserve calibration proposal-first flow while gating direct/evolution separately.",
            &[],
        ),
        entry(
            "calibration_direct_micro_evolution",
            "Calibration direct and micro-evolution writes",
            LegacyWritePathKind::CalibrationDirectMicroEvolution,
            &[
                "src-tauri/src/commands/calibration.rs",
                "frontend/src/pages/CalibrationPage.tsx",
            ],
            &["run_micro_evolution", "apply_calibration(mode=direct)"],
            LegacyWriteRiskClass::HighRiskLegacyDirectWrite,
            LegacyWriteConvergenceStatus::LegacyDirectWriteBlocker,
            false,
            true,
            true,
            true,
            LegacyWriteSafeModeStatus::GuardPresent,
            true,
            "W82 guard present: apply_calibration(mode=direct) and run_micro_evolution fail closed by default and require an explicit Calibration legacy direct apply dev/migration override; legacy responses are metadata-safe and return only counts, snapshot ids, warnings, and bounded signal counts.",
            "Keep calibration_create_proposals / proposal mode as the normal proposal-first path; retire apply_calibration(mode=direct) and run_micro_evolution direct persistence or convert remaining legacy capability to proposal-first before treating Calibration as fully converged.",
            "Future Calibration legacy direct/evolution removal or proposal-first conversion.",
            &[
                "direct_calibration_mode_can_still_write_durable_lifemodel_truth_with_dev_migration_override",
                "micro_evolution_can_still_apply_generated_changes_directly_with_dev_migration_override",
                "calibration_legacy_direct_apply_not_fully_proposal_first",
            ],
        ),
        entry(
            "feedback_signals_source_data",
            "Feedback signals source data",
            LegacyWritePathKind::FeedbackSignalsSourceData,
            &[
                "openlife-core/src/feedback.rs",
                "src-tauri/src/commands/feedback.rs",
                "src-tauri/src/lib.rs",
            ],
            &[
                "save_feedback",
                "log_analytics_event",
                "FeedbackStore::save_feedback",
                "FeedbackStore::log_event",
                "FeedbackStore::save_conversation_inference",
                "FeedbackStore::fetch_evolution_signals",
                "capture_conversation_signals",
            ],
            LegacyWriteRiskClass::LowRiskSourceData,
            LegacyWriteConvergenceStatus::LowRiskTransientSourceData,
            true,
            false,
            true,
            false,
            LegacyWriteSafeModeStatus::LowRiskSourceDataGuardNotRequired,
            false,
            "Feedback thumbs, analytics events, and conversation inference rows are low-risk source data signals, not accepted LifeModel truth; they must not automatically promote to durable LifeModel truth, active LifeModel, or evolution_rules truth.",
            "Keep feedback as source data; require proposal-first before durable LifeModel truth by promoting useful signals only through reviewable Proposal/Evidence candidates.",
            "Future Feedback source-data retention and EvidenceStore bridge.",
            &[],
        ),
        entry(
            "feedback_evolution_direct_writes",
            "Feedback evolution direct writes",
            LegacyWritePathKind::FeedbackEvolutionDirectWrites,
            &["src-tauri/src/commands/feedback.rs"],
            &[
                "apply_feedback_evolution",
                "FeedbackEvolutionLegacyDirectApplyOverride",
            ],
            LegacyWriteRiskClass::HighRiskLegacyDirectWrite,
            LegacyWriteConvergenceStatus::LegacyDirectWriteBlocker,
            false,
            true,
            true,
            true,
            LegacyWriteSafeModeStatus::GuardPresent,
            true,
            "W83 guard present: apply_feedback_evolution fails closed by default and requires an explicit Feedback evolution legacy direct apply dev/migration override; legacy response is metadata-safe and returns only counts/status/warnings.",
            "Convert remaining feedback-driven model changes into EvidenceStore records plus heuristic or LifeModel Proposal candidates; remove the legacy direct apply override before treating Feedback evolution as fully converged.",
            "Future Feedback evolution override removal or full proposal/evidence conversion.",
            &[
                "feedback_evolution_can_still_write_durable_lifemodel_truth_with_dev_migration_override",
                "feedback_evolution_legacy_direct_apply_not_fully_proposal_first",
                "evolution_rules_need_proposal_or_hs_candidate_flow",
            ],
        ),
        entry(
            "feedback_evolution_read_only_report",
            "Feedback evolution read-only report",
            LegacyWritePathKind::FeedbackEvolutionReadOnlyReport,
            &[
                "src-tauri/src/commands/feedback.rs",
                "openlife-core/src/feedback.rs",
            ],
            &[
                "generate_evolution_report",
                "FeedbackStore::generate_evolution_report",
            ],
            LegacyWriteRiskClass::LowRiskSourceData,
            LegacyWriteConvergenceStatus::LowRiskTransientSourceData,
            true,
            false,
            false,
            false,
            LegacyWriteSafeModeStatus::LowRiskSourceDataGuardNotRequired,
            false,
            "W83 read-only report: generate_evolution_report returns metadata-safe counts/status only and does not write LifeModel or evolution_rules truth.",
            "Keep report generation read-only; future candidate creation may create reviewable Proposal/Evidence records only, never active rules or LifeModel truth.",
            "Future Feedback evolution Proposal/Evidence candidate command.",
            &[],
        ),
        entry(
            "snapshot_restore",
            "Snapshot restore",
            LegacyWritePathKind::SnapshotRestore,
            &[
                "openlife-core/src/versioning.rs",
                "src-tauri/src/commands/version.rs",
            ],
            &["restore_snapshot"],
            LegacyWriteRiskClass::HighRiskLegacyDirectWrite,
            LegacyWriteConvergenceStatus::LegacyDirectWriteBlocker,
            false,
            true,
            true,
            true,
            LegacyWriteSafeModeStatus::GuardRequired,
            true,
            "Restore creates a pre-restore snapshot but then replaces current compatibility YAML directly.",
            "Make restore an explicit rollback/governor operation with confirmation and audit.",
            "W83 Snapshot restore rollback governance.",
            &[
                "restore_snapshot_replaces_current_lifemodel_yaml",
                "rollback_governance_not_yet_machine_guarded",
            ],
        ),
        entry(
            "data_import",
            "Data import",
            LegacyWritePathKind::DataImport,
            &["src-tauri/src/commands/settings.rs"],
            &["import_all_data", "apply_import_payload"],
            LegacyWriteRiskClass::HighRiskLegacyDirectWrite,
            LegacyWriteConvergenceStatus::LegacyDirectWriteBlocker,
            false,
            true,
            true,
            true,
            LegacyWriteSafeModeStatus::GuardPresent,
            true,
            "Import validates payloads and attempts rollback, but it remains an explicit high-risk migration/restore direct write path.",
            "Keep import as migration/restore only with stronger audit; do not treat imported content as accepted HS truth without rematerialization.",
            "W84 Data import migration/restore governance guard.",
            &[
                "import_all_data_can_replace_lifemodel_messages_or_vectors",
                "import_payload_not_equivalent_to_accepted_lifemodel_truth",
            ],
        ),
        entry(
            "state_daily_goal_direct_writes",
            "State and daily goal direct writes",
            LegacyWritePathKind::StateDailyGoalDirectWrites,
            &[
                "src-tauri/src/commands/state.rs",
                "src-tauri/src/lib.rs",
                "openlife-core/src/memory.rs",
            ],
            &[
                "add_daily_goal",
                "update_daily_goal",
                "delete_daily_goal",
                "toggle_daily_goal",
                "try_auto_checkin_daily_goals",
                "record_state_entry",
            ],
            LegacyWriteRiskClass::LowRiskTransientState,
            LegacyWriteConvergenceStatus::LowRiskTransientSourceData,
            true,
            false,
            true,
            false,
            LegacyWriteSafeModeStatus::LowRiskSourceDataGuardNotRequired,
            false,
            "Short-lived task/state samples may write local state, but they must not automatically promote to durable LifeModel truth.",
            "Keep as StateStore/transient task data; require proposal-first before durable LifeModel truth promotion.",
            "W85 StateStore TTL/source/confidence split.",
            &[],
        ),
        entry(
            "raw_chat_memory_vector_source_writes",
            "Raw chat, memory, and vector local source writes",
            LegacyWritePathKind::RawChatMemoryVectorSourceWrites,
            &[
                "openlife-core/src/memory.rs",
                "openlife-core/src/vectors.rs",
                "src-tauri/src/lib.rs",
                "src-tauri/src/commands/memory.rs",
            ],
            &[
                "save_message",
                "persist_chat_message_if_needed",
                "save_memory_record",
                "index_memory_chunk",
                "run_tier_maintenance",
                "archive_low_access_memories",
                "restore_archived",
                "set_importance",
            ],
            LegacyWriteRiskClass::LowRiskSourceData,
            LegacyWriteConvergenceStatus::LowRiskTransientSourceData,
            true,
            false,
            true,
            false,
            LegacyWriteSafeModeStatus::LowRiskSourceDataGuardNotRequired,
            false,
            "Local raw/source records and retrieval metadata are allowed as source data, but they must not automatically promote to durable LifeModel truth.",
            "Preserve as raw source data with retention/deletion controls; generated durable claims require proposal-first before durable LifeModel truth.",
            "W86 Raw source data retention and durable-claim proposal bridge.",
            &[],
        ),
        entry(
            "proposal_application_path",
            "Proposal application path",
            LegacyWritePathKind::ProposalApplicationPath,
            &[
                "src-tauri/src/commands/proposal.rs",
                "openlife-core/src/life_model/patch_store.rs",
            ],
            &["accept_proposal", "edit_proposal", "apply_proposal_to_state"],
            LegacyWriteRiskClass::ProposalFirstConvergenceTarget,
            LegacyWriteConvergenceStatus::ProposalFirstConvergenceTarget,
            true,
            true,
            false,
            true,
            LegacyWriteSafeModeStatus::ProposalFirstGuardPresent,
            false,
            "Review Center apply/edit is the governed convergence target; high-risk durable writes occur only after accepted proposal review.",
            "Make this the convergence target for legacy LifeModel, memory, tool, and HS mutations; preserve source-specific PatchStore mapping.",
            "W87 Source-specific proposal application patch mapping.",
            &[],
        ),
        entry(
            "external_write_proposal_path",
            "External write proposal path",
            LegacyWritePathKind::ExternalWriteProposalPath,
            &[
                "openlife-core/src/agent/action_executor/tool_executor.rs",
                "openlife-core/src/agent/action_executor/execution_tools.rs",
                "openlife-core/src/agent/action_executor/declarative_stubs.rs",
                "openlife-core/src/mcp.rs",
                "src-tauri/src/commands/proposal.rs",
            ],
            &[
                "ExternalWriteAction",
                "ScheduledTask",
                "DataExport",
                "calendar.propose_event",
                "email.propose_draft",
            ],
            LegacyWriteRiskClass::ProposalOnlyDeclarative,
            LegacyWriteConvergenceStatus::ProposalOnlyDeclarative,
            true,
            true,
            false,
            false,
            LegacyWriteSafeModeStatus::ProposalOnlyDeclarative,
            false,
            "External writes are proposal-first/declarative; calendar.propose_event and email.propose_draft are proposal-only and not real provider write executors.",
            "Keep provider execution disabled unless separately governed, reviewed, and regression-tested.",
            "Future provider executor enablement must be a separate governed slice.",
            &[],
        ),
    ]
}

pub(crate) fn evaluate_legacy_write_convergence_inventory(
    entries: &[LegacyWriteInventoryEntry],
) -> LegacyWriteConvergenceReport {
    let mut guard_blocking_reasons = Vec::new();
    let mut convergence_blockers = Vec::new();
    let mut high_risk_legacy_direct_write_count = 0usize;
    let mut low_risk_transient_or_source_data_count = 0usize;
    let mut proposal_first_target_count = 0usize;
    let mut proposal_first_targets_clean = true;
    let mut external_provider_execution_enabled = false;
    let mut calendar_propose_event_provider_executor_enabled = false;
    let mut email_propose_draft_provider_executor_enabled = false;
    let mut w73_w78_maturation_helper_listed_as_chat_write_path = false;

    for required_id in REQUIRED_STABLE_IDS {
        if !entries.iter().any(|entry| entry.stable_id == *required_id) {
            push_unique(
                &mut guard_blocking_reasons,
                format!("inventory_missing_required_path:{required_id}"),
            );
        }
    }

    for entry in entries {
        let is_proposal_first_target = matches!(
            entry.risk_class,
            LegacyWriteRiskClass::ProposalFirstConvergenceTarget
                | LegacyWriteRiskClass::ProposalOnlyDeclarative
        );
        let is_high_risk_direct = entry.currently_direct_write
            && entry.high_risk_durable_truth_write
            && matches!(
                entry.risk_class,
                LegacyWriteRiskClass::HighRiskLegacyDirectWrite
                    | LegacyWriteRiskClass::CompatibilityMaterializer
            );
        let is_legacy_direct_write = entry.currently_direct_write
            && matches!(
                entry.risk_class,
                LegacyWriteRiskClass::HighRiskLegacyDirectWrite
                    | LegacyWriteRiskClass::CompatibilityMaterializer
                    | LegacyWriteRiskClass::LowRiskTransientState
                    | LegacyWriteRiskClass::LowRiskSourceData
            );

        if is_high_risk_direct {
            high_risk_legacy_direct_write_count += 1;
        }
        if matches!(
            entry.risk_class,
            LegacyWriteRiskClass::LowRiskTransientState | LegacyWriteRiskClass::LowRiskSourceData
        ) {
            low_risk_transient_or_source_data_count += 1;
        }
        if is_proposal_first_target {
            proposal_first_target_count += 1;
        }

        if is_high_risk_direct
            || entry.current_status == LegacyWriteConvergenceStatus::LegacyDirectWriteBlocker
        {
            push_unique(
                &mut convergence_blockers,
                format!(
                    "{}:{}",
                    entry.stable_id,
                    if entry.blocking_reasons.is_empty() {
                        "legacy_direct_write_requires_convergence".to_string()
                    } else {
                        entry.blocking_reasons.join("|")
                    }
                ),
            );
        }

        if is_high_risk_direct && entry.normal_product_allowed {
            push_unique(
                &mut guard_blocking_reasons,
                format!(
                    "high_risk_legacy_direct_write_marked_normal_product_allowed:{}",
                    entry.stable_id
                ),
            );
        }

        if is_legacy_direct_write && entry.required_convergence_action.trim().is_empty() {
            push_unique(
                &mut guard_blocking_reasons,
                format!(
                    "legacy_direct_write_missing_required_convergence_action:{}",
                    entry.stable_id
                ),
            );
        }

        if entry.currently_direct_write
            && (entry.source_file_paths.is_empty() || entry.command_function_names.is_empty())
        {
            push_unique(
                &mut guard_blocking_reasons,
                format!(
                    "direct_write_missing_source_file_or_function:{}",
                    entry.stable_id
                ),
            );
        }

        if is_proposal_first_target
            && (entry.currently_direct_write
                || entry.current_status == LegacyWriteConvergenceStatus::LegacyDirectWriteBlocker)
        {
            proposal_first_targets_clean = false;
            push_unique(
                &mut guard_blocking_reasons,
                format!(
                    "proposal_first_target_marked_direct_unsafe_blocker:{}",
                    entry.stable_id
                ),
            );
        }

        if !entry.metadata_safe {
            push_unique(
                &mut guard_blocking_reasons,
                format!("entry_metadata_not_safe:{}", entry.stable_id),
            );
        }

        if entry.contains_raw_content {
            push_unique(
                &mut guard_blocking_reasons,
                format!("entry_contains_raw_content:{}", entry.stable_id),
            );
        }

        if entry.default_chat_affected {
            push_unique(
                &mut guard_blocking_reasons,
                format!("default_chat_marked_affected:{}", entry.stable_id),
            );
        }

        if entry
            .command_function_names
            .iter()
            .any(|name| W73_W78_MATURATION_HELPERS.contains(&name.as_str()))
        {
            w73_w78_maturation_helper_listed_as_chat_write_path = true;
            push_unique(
                &mut guard_blocking_reasons,
                format!(
                    "w73_w78_maturation_helper_listed_as_ordinary_chat_write_path:{}",
                    entry.stable_id
                ),
            );
        }

        if entry.provider_execution_enabled {
            external_provider_execution_enabled = true;
            if entry
                .command_function_names
                .iter()
                .any(|name| name == "calendar.propose_event")
            {
                calendar_propose_event_provider_executor_enabled = true;
            }
            if entry
                .command_function_names
                .iter()
                .any(|name| name == "email.propose_draft")
            {
                email_propose_draft_provider_executor_enabled = true;
            }
            push_unique(
                &mut guard_blocking_reasons,
                format!(
                    "proposal_tool_marked_real_provider_write_executor:{}",
                    entry.stable_id
                ),
            );
        }
    }

    let metadata_safe = entries.iter().all(|entry| entry.metadata_safe);
    let contains_raw_content = entries.iter().any(|entry| entry.contains_raw_content);
    let default_chat_unchanged = entries.iter().all(|entry| !entry.default_chat_affected);
    let all_direct_writes_converged = high_risk_legacy_direct_write_count == 0
        && entries.iter().all(|entry| {
            !matches!(
                entry.current_status,
                LegacyWriteConvergenceStatus::LegacyDirectWriteBlocker
                    | LegacyWriteConvergenceStatus::CompatibilityPrimitive
            )
        });
    let guard_ready = guard_blocking_reasons.is_empty()
        && metadata_safe
        && !contains_raw_content
        && default_chat_unchanged
        && proposal_first_targets_clean
        && !external_provider_execution_enabled
        && !w73_w78_maturation_helper_listed_as_chat_write_path;

    LegacyWriteConvergenceReport {
        inventory_ready: guard_ready,
        guard_ready,
        overall_converged: guard_ready && all_direct_writes_converged,
        all_direct_writes_converged,
        metadata_safe,
        contains_raw_content,
        default_chat_unchanged,
        default_chat_route: "legacy_stream".into(),
        w79_guard_called_by_ordinary_chat: false,
        w73_w78_maturation_helper_listed_as_chat_write_path,
        proposal_first_targets_clean,
        external_provider_execution_enabled,
        calendar_propose_event_provider_executor_enabled,
        email_propose_draft_provider_executor_enabled,
        inventory_entry_count: entries.len(),
        high_risk_legacy_direct_write_count,
        low_risk_transient_or_source_data_count,
        proposal_first_target_count,
        convergence_blockers,
        guard_blocking_reasons,
    }
}

pub(crate) fn ensure_legacy_write_convergence_inventory_guard(
) -> Result<LegacyWriteConvergenceReport, String> {
    let inventory = legacy_write_convergence_inventory();
    let report = evaluate_legacy_write_convergence_inventory(&inventory);
    if report.inventory_ready {
        Ok(report)
    } else {
        Err(format!(
            "legacy write convergence inventory guard blocked: {}",
            report.guard_blocking_reasons.join(",")
        ))
    }
}

const REQUIRED_STABLE_IDS: &[&str] = &[
    "lifemodel_save_primitive",
    "manual_lifemodel_editor",
    "builder_normal_proposal_flow",
    "builder_legacy_direct_apply",
    "calibration_proposal_flow",
    "calibration_direct_micro_evolution",
    "feedback_signals_source_data",
    "feedback_evolution_direct_writes",
    "feedback_evolution_read_only_report",
    "snapshot_restore",
    "data_import",
    "state_daily_goal_direct_writes",
    "raw_chat_memory_vector_source_writes",
    "proposal_application_path",
    "external_write_proposal_path",
];

const W73_W78_MATURATION_HELPERS: &[&str] = &[
    "evaluate_lifemodel_maturation_readiness",
    "ensure_lifemodel_maturation_readiness",
    "run_lifemodel_maturation_non_default_invocation",
    "ensure_lifemodel_maturation_non_default_invocation",
    "record_maturation_proposal_outcome_evidence",
    "evaluate_maturation_proposal_outcome_evidence",
    "evaluate_low_energy_collaboration_rule_candidate",
    "propose_low_energy_collaboration_rule_candidate",
    "evaluate_accepted_low_energy_rule_selection",
    "ensure_accepted_low_energy_rule_selection",
    "evaluate_low_energy_rule_trace_visibility",
    "ensure_low_energy_rule_trace_visibility",
];

#[allow(clippy::too_many_arguments)]
fn entry(
    stable_id: &str,
    display_name: &str,
    path_kind: LegacyWritePathKind,
    source_file_paths: &[&str],
    command_function_names: &[&str],
    risk_class: LegacyWriteRiskClass,
    current_status: LegacyWriteConvergenceStatus,
    normal_product_allowed: bool,
    requires_proposal_first: bool,
    currently_direct_write: bool,
    high_risk_durable_truth_write: bool,
    safe_mode_status: LegacyWriteSafeModeStatus,
    requires_safe_mode_guard: bool,
    current_guard_summary: &str,
    required_convergence_action: &str,
    next_recommended_slice: &str,
    blocking_reasons: &[&str],
) -> LegacyWriteInventoryEntry {
    LegacyWriteInventoryEntry {
        stable_id: stable_id.into(),
        display_name: display_name.into(),
        path_kind,
        source_file_paths: source_file_paths
            .iter()
            .map(|path| (*path).into())
            .collect(),
        command_function_names: command_function_names
            .iter()
            .map(|name| (*name).into())
            .collect(),
        risk_class,
        current_status,
        normal_product_allowed,
        requires_proposal_first,
        currently_direct_write,
        high_risk_durable_truth_write,
        safe_mode_status,
        requires_safe_mode_guard,
        default_chat_affected: false,
        provider_execution_enabled: false,
        current_guard_summary: current_guard_summary.into(),
        required_convergence_action: required_convergence_action.into(),
        next_recommended_slice: next_recommended_slice.into(),
        blocking_reasons: blocking_reasons
            .iter()
            .map(|reason| (*reason).into())
            .collect(),
        metadata_safe: true,
        contains_raw_content: false,
    }
}

fn push_unique(reasons: &mut Vec<String>, reason: String) {
    if !reasons.iter().any(|existing| existing == &reason) {
        reasons.push(reason);
    }
}
