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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StateSourceDataBoundaryReport {
    pub(crate) state_daily_goal_path_ids: Vec<String>,
    pub(crate) source_data_classification: String,
    pub(crate) low_risk_transient_classification: String,
    pub(crate) compatibility_lifemodel_materialized_write: bool,
    pub(crate) accepted_durable_hs_truth_write: bool,
    pub(crate) active_hs_lifemodel_patch: bool,
    pub(crate) writes_current_lifemodel_compatibility_view: bool,
    pub(crate) proposal_required_for_hs_truth_promotion: bool,
    pub(crate) ordinary_chat_unchanged: bool,
    pub(crate) default_chat_unchanged: bool,
    pub(crate) blocking_reasons: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LifeModelMaterializerCallerKind {
    CompatibilityPrimitiveMaterializerRoot,
    CompatibilityPrimitiveDefaultInitialization,
    OrdinaryChatAutoCheckinSourceData,
    ManualOverrideAudited,
    SourceDataCompatibilityMaterialization,
    AcceptedProposalApply,
    LegacyDevMigrationOverride,
    MigrationRestoreGated,
    Unclassified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LifeModelMaterializerCallerRisk {
    CompatibilityMaterializerRoot,
    SourceDataCompatibilityWrite,
    AcceptedProposalApply,
    HighRiskLegacyBlocker,
    HighRiskManualOverrideBlocker,
    HighRiskRestoreImportBlocker,
    Unclassified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LifeModelMaterializerCallerGovernanceState {
    CompatibilityPrimitiveInternal,
    SourceDataCompatibilityNotAcceptedTruth,
    AuditedManualOverrideStillLegacyBlocker,
    AcceptedProposalApplyNeedsSourceSpecificPatchMapping,
    DevMigrationOverrideGuardedLegacyBlocker,
    RestoreImportGatedLegacyBlocker,
    Unclassified,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LifeModelMaterializerCallerMatrixEntry {
    pub(crate) stable_id: String,
    pub(crate) display_name: String,
    pub(crate) source_file_path: String,
    pub(crate) caller_function_name: String,
    pub(crate) write_entrypoint: String,
    pub(crate) kind: LifeModelMaterializerCallerKind,
    pub(crate) risk: LifeModelMaterializerCallerRisk,
    pub(crate) governance_state: LifeModelMaterializerCallerGovernanceState,
    pub(crate) normal_product_allowed: bool,
    pub(crate) proposal_first: bool,
    pub(crate) source_data_compatibility: bool,
    pub(crate) manual_override: bool,
    pub(crate) restore_import_override: bool,
    pub(crate) high_risk_legacy_blocker: bool,
    pub(crate) metadata_safe: bool,
    pub(crate) contains_raw_lifemodel_payload: bool,
    pub(crate) contains_raw_memory_text: bool,
    pub(crate) contains_raw_chat_text: bool,
    pub(crate) contains_raw_daily_goal_text: bool,
    pub(crate) default_chat_route_changed: bool,
    pub(crate) migration_permission: bool,
    pub(crate) runtime_authority_granted: bool,
    pub(crate) accepted_durable_lifemodel_hs_truth: bool,
    pub(crate) proposal_first_convergence_complete: bool,
    pub(crate) required_follow_up: String,
    pub(crate) blocking_reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LifeModelMaterializerCallerMatrixReport {
    pub(crate) matrix_ready: bool,
    pub(crate) metadata_safe: bool,
    pub(crate) contains_raw_lifemodel_payload: bool,
    pub(crate) contains_raw_memory_text: bool,
    pub(crate) contains_raw_chat_text: bool,
    pub(crate) contains_raw_daily_goal_text: bool,
    pub(crate) materializer_root_identified: bool,
    pub(crate) all_known_callers_classified: bool,
    pub(crate) unclassified_callers: Vec<String>,
    pub(crate) caller_count: usize,
    pub(crate) high_risk_legacy_blocker_count: usize,
    pub(crate) proposal_first_count: usize,
    pub(crate) source_data_compatibility_count: usize,
    pub(crate) manual_override_count: usize,
    pub(crate) restore_import_override_count: usize,
    pub(crate) ordinary_chat_auto_checkin_present: bool,
    pub(crate) ordinary_chat_auto_checkin_classification: String,
    pub(crate) default_chat_route_unchanged: bool,
    pub(crate) migration_permission: bool,
    pub(crate) runtime_authority_granted: bool,
    pub(crate) proposal_first_convergence_complete: bool,
    pub(crate) blocking_reasons: Vec<String>,
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
            &[
                "restore_snapshot",
                "SnapshotRestoreLegacyDirectApplyOverride",
            ],
            LegacyWriteRiskClass::HighRiskLegacyDirectWrite,
            LegacyWriteConvergenceStatus::LegacyDirectWriteBlocker,
            false,
            true,
            true,
            true,
            LegacyWriteSafeModeStatus::GuardPresent,
            true,
            "W84 guard present: restore_snapshot fails closed by default and requires an explicit dev/migration/manual restore override; legacy response is metadata-safe and returns only snapshot ids/status, not restored LifeModel or snapshot YAML.",
            "Convert restore into a stronger governed rollback operation with explicit audit, or keep it migration/manual-restore only.",
            "Future Snapshot restore override removal or governed rollback audit flow.",
            &[
                "restore_snapshot_can_still_replace_current_lifemodel_yaml_with_manual_restore_override",
                "snapshot_restore_legacy_direct_apply_not_fully_governed",
                "manual_restore_override_required_for_legacy_restore",
            ],
        ),
        entry(
            "data_import",
            "Data import",
            LegacyWritePathKind::DataImport,
            &["src-tauri/src/commands/settings.rs"],
            &[
                "import_all_data",
                "apply_import_payload",
                "DataImportLegacyDirectApplyOverride",
            ],
            LegacyWriteRiskClass::HighRiskLegacyDirectWrite,
            LegacyWriteConvergenceStatus::LegacyDirectWriteBlocker,
            false,
            true,
            true,
            true,
            LegacyWriteSafeModeStatus::GuardPresent,
            true,
            "W84 guard present: import_all_data fails closed by default and requires an explicit dev/migration/manual restore override; legacy response is metadata-safe and returns only counts/status, not raw LifeModel, messages, vectors, or imported payload.",
            "Keep import as migration/restore only with stronger audit; do not treat imported content as accepted HS truth without rematerialization.",
            "Future Data import override removal or governed migration audit flow.",
            &[
                "import_all_data_can_still_replace_lifemodel_messages_or_vectors_with_dev_migration_override",
                "import_payload_not_equivalent_to_accepted_lifemodel_truth",
                "data_import_dev_migration_override_required_for_legacy_import",
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
                "persist_life_model",
            ],
            LegacyWriteRiskClass::LowRiskTransientState,
            LegacyWriteConvergenceStatus::LowRiskTransientSourceData,
            true,
            false,
            true,
            false,
            LegacyWriteSafeModeStatus::LowRiskSourceDataGuardNotRequired,
            false,
            "W85 boundary proof: short-lived daily goals and state samples are source data / low-risk transient compatibility writes; State/Daily Goal writes the current LifeModel compatibility view through persist_life_model, but is not accepted durable LifeModel-HS truth and must not automatically promote to durable LifeModel-HS truth.",
            "Keep State/Daily Goal as source data and compatibility materialized state; future StateStore TTL/source/confidence split or HS truth promotion bridge must require proposal-first before durable LifeModel-HS truth promotion in a separate slice.",
            "Future StateStore TTL/source/confidence split and proposal bridge.",
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

pub(crate) fn evaluate_state_source_data_boundary(
    entries: &[LegacyWriteInventoryEntry],
) -> StateSourceDataBoundaryReport {
    let mut blocking_reasons = Vec::new();
    let matching_entries = entries
        .iter()
        .filter(|entry| entry.stable_id == STATE_DAILY_GOAL_DIRECT_WRITES_STABLE_ID)
        .collect::<Vec<_>>();
    let state_daily_goal_path_ids = matching_entries
        .iter()
        .map(|entry| entry.stable_id.clone())
        .collect::<Vec<_>>();
    let mut compatibility_lifemodel_materialized_write = false;
    let mut writes_current_lifemodel_compatibility_view = false;

    if matching_entries.is_empty() {
        push_unique(
            &mut blocking_reasons,
            "state_daily_goal_direct_writes_inventory_entry_missing".into(),
        );
    }
    if matching_entries.len() > 1 {
        push_unique(
            &mut blocking_reasons,
            "state_daily_goal_direct_writes_inventory_entry_duplicated".into(),
        );
    }

    for entry in matching_entries {
        if entry.path_kind != LegacyWritePathKind::StateDailyGoalDirectWrites {
            push_unique(
                &mut blocking_reasons,
                "state_daily_goal_direct_writes_wrong_path_kind".into(),
            );
        }

        if entry.risk_class == LegacyWriteRiskClass::HighRiskLegacyDirectWrite {
            push_unique(
                &mut blocking_reasons,
                "state_daily_goal_direct_writes_marked_high_risk_legacy_direct_write".into(),
            );
        }

        if entry.risk_class != LegacyWriteRiskClass::LowRiskTransientState {
            push_unique(
                &mut blocking_reasons,
                "state_daily_goal_direct_writes_not_low_risk_transient_state".into(),
            );
        }

        if matches!(
            entry.current_status,
            LegacyWriteConvergenceStatus::AlreadyProposalFirst
                | LegacyWriteConvergenceStatus::ProposalFirstConvergenceTarget
                | LegacyWriteConvergenceStatus::ProposalOnlyDeclarative
        ) {
            push_unique(
                &mut blocking_reasons,
                "state_daily_goal_direct_writes_marked_already_proposal_first_or_converged".into(),
            );
        }

        if entry.current_status != LegacyWriteConvergenceStatus::LowRiskTransientSourceData {
            push_unique(
                &mut blocking_reasons,
                "state_daily_goal_direct_writes_not_low_risk_transient_source_data".into(),
            );
        }

        if entry.high_risk_durable_truth_write {
            push_unique(
                &mut blocking_reasons,
                "state_daily_goal_direct_writes_marked_durable_lifemodel_truth_write".into(),
            );
        }

        if !entry
            .command_function_names
            .iter()
            .any(|name| name == STATE_DAILY_GOAL_COMPATIBILITY_WRITER)
        {
            push_unique(
                &mut blocking_reasons,
                "state_daily_goal_direct_writes_missing_compatibility_materialized_writer".into(),
            );
        } else {
            compatibility_lifemodel_materialized_write = true;
        }

        if entry.requires_proposal_first && !entry.normal_product_allowed {
            push_unique(
                &mut blocking_reasons,
                "state_daily_goal_direct_writes_confuses_proposal_first_with_blocked_normal_product"
                    .into(),
            );
        }

        if !entry.normal_product_allowed {
            push_unique(
                &mut blocking_reasons,
                "state_daily_goal_direct_writes_not_normal_product_source_data".into(),
            );
        }

        if !entry.metadata_safe {
            push_unique(
                &mut blocking_reasons,
                "state_daily_goal_direct_writes_metadata_not_safe".into(),
            );
        }

        if entry.contains_raw_content {
            push_unique(
                &mut blocking_reasons,
                "state_daily_goal_direct_writes_contains_raw_content".into(),
            );
        }

        if entry.default_chat_affected {
            push_unique(
                &mut blocking_reasons,
                "state_daily_goal_direct_writes_affects_default_chat".into(),
            );
        }

        if entry.command_function_names.iter().any(|name| {
            FORBIDDEN_STATE_DAILY_GOAL_HS_TRUTH_WRITERS
                .iter()
                .any(|writer| name.contains(writer))
        }) {
            push_unique(
                &mut blocking_reasons,
                "state_daily_goal_direct_writes_lists_durable_lifemodel_truth_writer".into(),
            );
        }

        if entry
            .command_function_names
            .iter()
            .any(|name| ORDINARY_CHAT_ENTRYPOINTS.contains(&name.as_str()))
        {
            push_unique(
                &mut blocking_reasons,
                "state_daily_goal_direct_writes_lists_default_or_ordinary_chat_entrypoint".into(),
            );
        }

        if !entry
            .current_guard_summary
            .contains("must not automatically promote to durable LifeModel-HS truth")
            || !entry.current_guard_summary.contains("source data")
            || !entry
                .current_guard_summary
                .contains("writes the current LifeModel compatibility view")
            || !entry
                .current_guard_summary
                .contains("not accepted durable LifeModel-HS truth")
        {
            push_unique(
                &mut blocking_reasons,
                "state_daily_goal_direct_writes_missing_source_data_no_auto_promotion_boundary"
                    .into(),
            );
        } else {
            writes_current_lifemodel_compatibility_view = true;
        }

        if !entry
            .required_convergence_action
            .contains("proposal-first before durable LifeModel-HS truth promotion")
        {
            push_unique(
                &mut blocking_reasons,
                "state_daily_goal_direct_writes_missing_proposal_first_truth_promotion_boundary"
                    .into(),
            );
        }

        let boundary_text = [
            entry.current_guard_summary.as_str(),
            entry.required_convergence_action.as_str(),
            entry.next_recommended_slice.as_str(),
        ]
        .join(" ")
        .to_ascii_lowercase();
        if AUTOMATIC_TRUTH_PROMOTION_ALLOWED_MARKERS
            .iter()
            .any(|marker| boundary_text.contains(marker))
        {
            push_unique(
                &mut blocking_reasons,
                "state_daily_goal_direct_writes_implies_automatic_truth_promotion".into(),
            );
        }

        if ACCEPTED_HS_TRUTH_WRITE_MARKERS
            .iter()
            .any(|marker| boundary_text.contains(marker))
        {
            push_unique(
                &mut blocking_reasons,
                "state_daily_goal_direct_writes_claims_accepted_hs_truth_write".into(),
            );
        }
    }

    let ordinary_chat_unchanged = entries
        .iter()
        .filter(|entry| entry.stable_id == STATE_DAILY_GOAL_DIRECT_WRITES_STABLE_ID)
        .all(|entry| {
            !entry
                .command_function_names
                .iter()
                .any(|name| ORDINARY_CHAT_ENTRYPOINTS.contains(&name.as_str()))
        });
    let default_chat_unchanged = entries
        .iter()
        .filter(|entry| entry.stable_id == STATE_DAILY_GOAL_DIRECT_WRITES_STABLE_ID)
        .all(|entry| !entry.default_chat_affected);

    StateSourceDataBoundaryReport {
        state_daily_goal_path_ids,
        source_data_classification: "state_daily_goal_source_data_not_accepted_lifemodel_hs_truth"
            .into(),
        low_risk_transient_classification: "low_risk_transient_source_data".into(),
        compatibility_lifemodel_materialized_write,
        accepted_durable_hs_truth_write: false,
        active_hs_lifemodel_patch: false,
        writes_current_lifemodel_compatibility_view,
        proposal_required_for_hs_truth_promotion: true,
        ordinary_chat_unchanged,
        default_chat_unchanged,
        blocking_reasons,
    }
}

pub(crate) fn ensure_state_source_data_boundary() -> Result<StateSourceDataBoundaryReport, String> {
    let inventory = legacy_write_convergence_inventory();
    let report = evaluate_state_source_data_boundary(&inventory);
    if report.blocking_reasons.is_empty() {
        Ok(report)
    } else {
        Err(format!(
            "state source-data boundary blocked: {}",
            report.blocking_reasons.join(",")
        ))
    }
}

pub(crate) fn lifemodel_materializer_caller_matrix() -> Vec<LifeModelMaterializerCallerMatrixEntry>
{
    vec![
        caller_matrix_entry(
            "lifemodel_materializer_root",
            "LifeModel compatibility materializer root",
            "src-tauri/src/lib.rs",
            "persist_life_model",
            "LifeModelManager::save",
            LifeModelMaterializerCallerKind::CompatibilityPrimitiveMaterializerRoot,
            LifeModelMaterializerCallerRisk::CompatibilityMaterializerRoot,
            LifeModelMaterializerCallerGovernanceState::CompatibilityPrimitiveInternal,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            "Restrict W87 callers to accepted proposal apply, source-data compatibility materialization, audited manual override, and gated restore/import/dev migration paths.",
            &[],
        ),
        caller_matrix_entry(
            "lifemodel_manager_default_initialization",
            "LifeModelManager default model initialization",
            "openlife-core/src/life_model.rs",
            "LifeModelManager::load(default_model_initialization)",
            "LifeModelManager::save",
            LifeModelMaterializerCallerKind::CompatibilityPrimitiveDefaultInitialization,
            LifeModelMaterializerCallerRisk::CompatibilityMaterializerRoot,
            LifeModelMaterializerCallerGovernanceState::CompatibilityPrimitiveInternal,
            true,
            false,
            false,
            false,
            false,
            false,
            false,
            false,
            "Internal first-load compatibility initialization only; not a migration permission, runtime authority grant, or proposal-first convergence completion signal.",
            &[],
        ),
        caller_matrix_entry(
            "ordinary_chat_auto_checkin_source_data",
            "Ordinary Chat daily-goal auto-checkin compatibility materialization",
            "src-tauri/src/lib.rs",
            "send_message",
            "persist_life_model",
            LifeModelMaterializerCallerKind::OrdinaryChatAutoCheckinSourceData,
            LifeModelMaterializerCallerRisk::SourceDataCompatibilityWrite,
            LifeModelMaterializerCallerGovernanceState::SourceDataCompatibilityNotAcceptedTruth,
            true,
            false,
            true,
            false,
            false,
            false,
            false,
            false,
            "Daily goal auto-checkin writes the current compatibility view from source data; it is not accepted durable LifeModel-HS truth and grants no migration permission.",
            &[],
        ),
        caller_matrix_entry(
            "ordinary_stream_agent_loop_auto_checkin_source_data",
            "Stream AgentLoop daily-goal auto-checkin compatibility materialization",
            "src-tauri/src/lib.rs",
            "start_stream_message_with_agent_loop",
            "persist_life_model",
            LifeModelMaterializerCallerKind::OrdinaryChatAutoCheckinSourceData,
            LifeModelMaterializerCallerRisk::SourceDataCompatibilityWrite,
            LifeModelMaterializerCallerGovernanceState::SourceDataCompatibilityNotAcceptedTruth,
            true,
            false,
            true,
            false,
            false,
            false,
            false,
            false,
            "Stream daily goal auto-checkin writes the current compatibility view from source data; it is not accepted durable LifeModel-HS truth and grants no migration permission.",
            &[],
        ),
        caller_matrix_entry(
            "ordinary_stream_legacy_auto_checkin_source_data",
            "Legacy stream daily-goal auto-checkin compatibility materialization",
            "src-tauri/src/lib.rs",
            "start_stream_message",
            "persist_life_model",
            LifeModelMaterializerCallerKind::OrdinaryChatAutoCheckinSourceData,
            LifeModelMaterializerCallerRisk::SourceDataCompatibilityWrite,
            LifeModelMaterializerCallerGovernanceState::SourceDataCompatibilityNotAcceptedTruth,
            true,
            false,
            true,
            false,
            false,
            false,
            false,
            false,
            "Legacy stream daily goal auto-checkin writes the current compatibility view from source data; it is not accepted durable LifeModel-HS truth and grants no migration permission.",
            &[],
        ),
        caller_matrix_entry(
            "manual_lifemodel_editor_save",
            "Manual LifeModel editor save",
            "src-tauri/src/commands/life_model.rs",
            "save_life_model_with_state",
            "persist_life_model",
            LifeModelMaterializerCallerKind::ManualOverrideAudited,
            LifeModelMaterializerCallerRisk::HighRiskManualOverrideBlocker,
            LifeModelMaterializerCallerGovernanceState::AuditedManualOverrideStillLegacyBlocker,
            false,
            false,
            false,
            true,
            false,
            true,
            false,
            false,
            "W80 audited manual override remains a high-risk legacy blocker until converted to proposal patch review or stronger governed override UX.",
            &[
                "manual_editor_can_write_durable_lifemodel_truth_directly",
                "manual_editor_not_proposal_first_converged",
            ],
        ),
        caller_matrix_entry(
            "state_record_state_source_data",
            "State record compatibility materialization",
            "src-tauri/src/commands/state.rs",
            "record_state_with_state",
            "persist_life_model",
            LifeModelMaterializerCallerKind::SourceDataCompatibilityMaterialization,
            LifeModelMaterializerCallerRisk::SourceDataCompatibilityWrite,
            LifeModelMaterializerCallerGovernanceState::SourceDataCompatibilityNotAcceptedTruth,
            true,
            false,
            true,
            false,
            false,
            false,
            false,
            false,
            "State samples are source data compatibility materialization only; proposal-first is required before durable LifeModel-HS truth promotion.",
            &[],
        ),
        caller_matrix_entry(
            "state_add_daily_goal_source_data",
            "Add daily goal compatibility materialization",
            "src-tauri/src/commands/state.rs",
            "add_daily_goal",
            "persist_life_model",
            LifeModelMaterializerCallerKind::SourceDataCompatibilityMaterialization,
            LifeModelMaterializerCallerRisk::SourceDataCompatibilityWrite,
            LifeModelMaterializerCallerGovernanceState::SourceDataCompatibilityNotAcceptedTruth,
            true,
            false,
            true,
            false,
            false,
            false,
            false,
            false,
            "Daily goal source data writes the current compatibility view only; it is not accepted durable LifeModel-HS truth.",
            &[],
        ),
        caller_matrix_entry(
            "state_update_daily_goal_source_data",
            "Update daily goal compatibility materialization",
            "src-tauri/src/commands/state.rs",
            "update_daily_goal",
            "persist_life_model",
            LifeModelMaterializerCallerKind::SourceDataCompatibilityMaterialization,
            LifeModelMaterializerCallerRisk::SourceDataCompatibilityWrite,
            LifeModelMaterializerCallerGovernanceState::SourceDataCompatibilityNotAcceptedTruth,
            true,
            false,
            true,
            false,
            false,
            false,
            false,
            false,
            "Daily goal source data writes the current compatibility view only; it is not accepted durable LifeModel-HS truth.",
            &[],
        ),
        caller_matrix_entry(
            "state_delete_daily_goal_source_data",
            "Delete daily goal compatibility materialization",
            "src-tauri/src/commands/state.rs",
            "delete_daily_goal",
            "persist_life_model",
            LifeModelMaterializerCallerKind::SourceDataCompatibilityMaterialization,
            LifeModelMaterializerCallerRisk::SourceDataCompatibilityWrite,
            LifeModelMaterializerCallerGovernanceState::SourceDataCompatibilityNotAcceptedTruth,
            true,
            false,
            true,
            false,
            false,
            false,
            false,
            false,
            "Daily goal source data writes the current compatibility view only; it is not accepted durable LifeModel-HS truth.",
            &[],
        ),
        caller_matrix_entry(
            "state_toggle_daily_goal_source_data",
            "Toggle daily goal compatibility materialization",
            "src-tauri/src/commands/state.rs",
            "toggle_daily_goal_with_state",
            "persist_life_model",
            LifeModelMaterializerCallerKind::SourceDataCompatibilityMaterialization,
            LifeModelMaterializerCallerRisk::SourceDataCompatibilityWrite,
            LifeModelMaterializerCallerGovernanceState::SourceDataCompatibilityNotAcceptedTruth,
            true,
            false,
            true,
            false,
            false,
            false,
            false,
            false,
            "Daily goal source data writes the current compatibility view only; it is not accepted durable LifeModel-HS truth.",
            &[],
        ),
        caller_matrix_entry(
            "proposal_apply_lifemodel_update",
            "Accepted proposal LifeModel apply",
            "src-tauri/src/commands/proposal.rs",
            "apply_proposal_to_state",
            "persist_life_model",
            LifeModelMaterializerCallerKind::AcceptedProposalApply,
            LifeModelMaterializerCallerRisk::AcceptedProposalApply,
            LifeModelMaterializerCallerGovernanceState::AcceptedProposalApplyNeedsSourceSpecificPatchMapping,
            true,
            true,
            false,
            false,
            false,
            false,
            false,
            true,
            "Accepted proposal apply is the convergence target, but W88-W89 still need source-specific patch mapping before proposal-first convergence can be marked complete.",
            &[],
        ),
        caller_matrix_entry(
            "builder_step_legacy_direct_apply",
            "Builder step legacy direct apply compatibility write",
            "src-tauri/src/commands/builder.rs",
            "builder_step_with_state",
            "persist_life_model",
            LifeModelMaterializerCallerKind::LegacyDevMigrationOverride,
            LifeModelMaterializerCallerRisk::HighRiskLegacyBlocker,
            LifeModelMaterializerCallerGovernanceState::DevMigrationOverrideGuardedLegacyBlocker,
            false,
            false,
            false,
            false,
            false,
            true,
            false,
            false,
            "W81 Builder legacy direct apply remains guarded for dev/migration only and must be removed or converted to proposal-first.",
            &[
                "builder_step_can_still_reach_legacy_direct_lifemodel_write",
                "builder_legacy_direct_apply_not_fully_proposal_first",
            ],
        ),
        caller_matrix_entry(
            "builder_apply_signals_legacy_direct_apply",
            "Builder apply signals legacy direct apply compatibility write",
            "src-tauri/src/commands/builder.rs",
            "builder_apply_signals_direct_apply_after_gate",
            "persist_life_model",
            LifeModelMaterializerCallerKind::LegacyDevMigrationOverride,
            LifeModelMaterializerCallerRisk::HighRiskLegacyBlocker,
            LifeModelMaterializerCallerGovernanceState::DevMigrationOverrideGuardedLegacyBlocker,
            false,
            false,
            false,
            false,
            false,
            true,
            false,
            false,
            "W81 Builder legacy direct apply remains guarded for dev/migration only and must be removed or converted to proposal-first.",
            &[
                "builder_apply_signals_can_still_write_with_dev_migration_override",
                "builder_legacy_direct_apply_not_fully_proposal_first",
            ],
        ),
        caller_matrix_entry(
            "calibration_micro_evolution_legacy_direct_apply",
            "Calibration micro-evolution legacy direct apply compatibility write",
            "src-tauri/src/commands/calibration.rs",
            "run_micro_evolution_direct_apply_after_gate",
            "persist_life_model",
            LifeModelMaterializerCallerKind::LegacyDevMigrationOverride,
            LifeModelMaterializerCallerRisk::HighRiskLegacyBlocker,
            LifeModelMaterializerCallerGovernanceState::DevMigrationOverrideGuardedLegacyBlocker,
            false,
            false,
            false,
            false,
            false,
            true,
            false,
            false,
            "W82 Calibration micro-evolution direct apply remains guarded for dev/migration only and must be converted to proposal-first.",
            &[
                "micro_evolution_can_still_apply_generated_changes_directly_with_dev_migration_override",
                "calibration_legacy_direct_apply_not_fully_proposal_first",
            ],
        ),
        caller_matrix_entry(
            "calibration_direct_apply_legacy_direct_apply",
            "Calibration direct apply compatibility write",
            "src-tauri/src/commands/calibration.rs",
            "apply_calibration_direct_apply_after_gate",
            "persist_life_model",
            LifeModelMaterializerCallerKind::LegacyDevMigrationOverride,
            LifeModelMaterializerCallerRisk::HighRiskLegacyBlocker,
            LifeModelMaterializerCallerGovernanceState::DevMigrationOverrideGuardedLegacyBlocker,
            false,
            false,
            false,
            false,
            false,
            true,
            false,
            false,
            "W82 Calibration direct apply remains guarded for dev/migration only and must be converted to proposal-first.",
            &[
                "direct_calibration_mode_can_still_write_with_dev_migration_override",
                "calibration_legacy_direct_apply_not_fully_proposal_first",
            ],
        ),
        caller_matrix_entry(
            "feedback_evolution_legacy_direct_apply",
            "Feedback evolution legacy direct apply compatibility write",
            "src-tauri/src/commands/feedback.rs",
            "apply_feedback_evolution_direct_apply_after_gate",
            "persist_life_model",
            LifeModelMaterializerCallerKind::LegacyDevMigrationOverride,
            LifeModelMaterializerCallerRisk::HighRiskLegacyBlocker,
            LifeModelMaterializerCallerGovernanceState::DevMigrationOverrideGuardedLegacyBlocker,
            false,
            false,
            false,
            false,
            false,
            true,
            false,
            false,
            "W83 Feedback evolution direct apply remains guarded for dev/migration only and must become reviewable Proposal/Evidence candidates.",
            &[
                "feedback_evolution_can_still_write_with_dev_migration_override",
                "feedback_evolution_legacy_direct_apply_not_fully_proposal_first",
            ],
        ),
        caller_matrix_entry(
            "snapshot_restore_legacy_direct_apply",
            "Snapshot restore direct LifeModel save",
            "src-tauri/src/commands/version.rs",
            "restore_snapshot_direct_apply_after_gate",
            "LifeModelManager::save",
            LifeModelMaterializerCallerKind::MigrationRestoreGated,
            LifeModelMaterializerCallerRisk::HighRiskRestoreImportBlocker,
            LifeModelMaterializerCallerGovernanceState::RestoreImportGatedLegacyBlocker,
            false,
            false,
            false,
            false,
            true,
            true,
            false,
            false,
            "W84 snapshot restore remains a gated migration/manual restore override and must not be marked fully converged.",
            &[
                "restore_snapshot_can_replace_current_lifemodel_yaml_with_manual_restore_override",
                "snapshot_restore_legacy_direct_apply_not_fully_governed",
            ],
        ),
        caller_matrix_entry(
            "data_import_legacy_direct_apply",
            "Data import compatibility materializer write",
            "src-tauri/src/commands/settings.rs",
            "apply_import_payload",
            "persist_life_model",
            LifeModelMaterializerCallerKind::MigrationRestoreGated,
            LifeModelMaterializerCallerRisk::HighRiskRestoreImportBlocker,
            LifeModelMaterializerCallerGovernanceState::RestoreImportGatedLegacyBlocker,
            false,
            false,
            false,
            false,
            true,
            true,
            false,
            false,
            "W84 data import remains a gated migration/manual restore override and imported content is not accepted LifeModel-HS truth without rematerialization.",
            &[
                "data_import_can_replace_lifemodel_messages_or_vectors_with_dev_migration_override",
                "import_payload_not_equivalent_to_accepted_lifemodel_truth",
            ],
        ),
    ]
}

pub(crate) fn evaluate_lifemodel_materializer_caller_matrix(
    entries: &[LifeModelMaterializerCallerMatrixEntry],
) -> LifeModelMaterializerCallerMatrixReport {
    let mut blocking_reasons = Vec::new();
    let mut unclassified_callers = Vec::new();
    let mut missing_required_caller = false;

    for required_id in REQUIRED_LIFEMODEL_MATERIALIZER_CALLER_IDS {
        let count = entries
            .iter()
            .filter(|entry| entry.stable_id == *required_id)
            .count();
        if count == 0 {
            missing_required_caller = true;
            push_unique(
                &mut blocking_reasons,
                format!("materializer_caller_missing_required:{required_id}"),
            );
        }
        if count > 1 {
            push_unique(
                &mut blocking_reasons,
                format!("materializer_caller_duplicated:{required_id}"),
            );
        }
    }

    for entry in entries {
        let unclassified = entry.kind == LifeModelMaterializerCallerKind::Unclassified
            || entry.risk == LifeModelMaterializerCallerRisk::Unclassified
            || entry.governance_state == LifeModelMaterializerCallerGovernanceState::Unclassified;
        if unclassified {
            push_unique(&mut unclassified_callers, entry.stable_id.clone());
            push_unique(
                &mut blocking_reasons,
                format!("unclassified_materializer_caller:{}", entry.stable_id),
            );
        }

        if entry.write_entrypoint != "persist_life_model"
            && entry.write_entrypoint != "LifeModelManager::save"
        {
            push_unique(
                &mut blocking_reasons,
                format!(
                    "materializer_caller_unknown_write_entrypoint:{}",
                    entry.stable_id
                ),
            );
        }

        if !entry.metadata_safe {
            push_unique(
                &mut blocking_reasons,
                format!("materializer_caller_metadata_not_safe:{}", entry.stable_id),
            );
        }

        if entry.contains_raw_lifemodel_payload
            || entry.contains_raw_memory_text
            || entry.contains_raw_chat_text
            || entry.contains_raw_daily_goal_text
        {
            push_unique(
                &mut blocking_reasons,
                format!(
                    "materializer_caller_contains_raw_content:{}",
                    entry.stable_id
                ),
            );
        }

        if entry.default_chat_route_changed {
            push_unique(
                &mut blocking_reasons,
                format!(
                    "materializer_caller_changes_default_chat_route:{}",
                    entry.stable_id
                ),
            );
        }

        if entry.migration_permission {
            push_unique(
                &mut blocking_reasons,
                format!(
                    "materializer_matrix_grants_migration_permission:{}",
                    entry.stable_id
                ),
            );
        }

        if entry.runtime_authority_granted {
            push_unique(
                &mut blocking_reasons,
                format!(
                    "materializer_matrix_grants_runtime_authority:{}",
                    entry.stable_id
                ),
            );
        }

        if entry.kind == LifeModelMaterializerCallerKind::OrdinaryChatAutoCheckinSourceData
            && (!entry.source_data_compatibility
                || entry.migration_permission
                || entry.runtime_authority_granted
                || entry.accepted_durable_lifemodel_hs_truth)
        {
            push_unique(
                &mut blocking_reasons,
                format!(
                    "ordinary_chat_auto_checkin_misclassified_as_authority_or_truth:{}",
                    entry.stable_id
                ),
            );
        }

        if entry.kind == LifeModelMaterializerCallerKind::ManualOverrideAudited
            && (entry.proposal_first || entry.proposal_first_convergence_complete)
        {
            push_unique(
                &mut blocking_reasons,
                format!(
                    "manual_editor_misclassified_as_proposal_first_or_converged:{}",
                    entry.stable_id
                ),
            );
        }

        if entry.kind == LifeModelMaterializerCallerKind::MigrationRestoreGated
            && entry.proposal_first_convergence_complete
        {
            push_unique(
                &mut blocking_reasons,
                format!(
                    "restore_import_misclassified_as_converged:{}",
                    entry.stable_id
                ),
            );
        }

        if entry.source_data_compatibility && entry.accepted_durable_lifemodel_hs_truth {
            push_unique(
                &mut blocking_reasons,
                format!(
                    "source_data_compatibility_marked_accepted_lifemodel_hs_truth:{}",
                    entry.stable_id
                ),
            );
        }
    }

    let metadata_safe = entries.iter().all(|entry| entry.metadata_safe);
    let contains_raw_lifemodel_payload = entries
        .iter()
        .any(|entry| entry.contains_raw_lifemodel_payload);
    let contains_raw_memory_text = entries.iter().any(|entry| entry.contains_raw_memory_text);
    let contains_raw_chat_text = entries.iter().any(|entry| entry.contains_raw_chat_text);
    let contains_raw_daily_goal_text = entries
        .iter()
        .any(|entry| entry.contains_raw_daily_goal_text);
    let materializer_root_identified = entries.iter().any(|entry| {
        entry.stable_id == LIFEMODEL_MATERIALIZER_ROOT_CALLER_ID
            && entry.kind == LifeModelMaterializerCallerKind::CompatibilityPrimitiveMaterializerRoot
            && entry.write_entrypoint == "LifeModelManager::save"
    });
    if !materializer_root_identified {
        push_unique(
            &mut blocking_reasons,
            "lifemodel_materializer_root_not_identified".into(),
        );
    }

    let high_risk_legacy_blocker_count = entries
        .iter()
        .filter(|entry| entry.high_risk_legacy_blocker)
        .count();
    let proposal_first_count = entries.iter().filter(|entry| entry.proposal_first).count();
    let source_data_compatibility_count = entries
        .iter()
        .filter(|entry| entry.source_data_compatibility)
        .count();
    let manual_override_count = entries.iter().filter(|entry| entry.manual_override).count();
    let restore_import_override_count = entries
        .iter()
        .filter(|entry| entry.restore_import_override)
        .count();
    let ordinary_chat_auto_checkin_present = entries.iter().any(|entry| {
        entry.kind == LifeModelMaterializerCallerKind::OrdinaryChatAutoCheckinSourceData
    });
    let default_chat_route_unchanged = entries
        .iter()
        .all(|entry| !entry.default_chat_route_changed);
    let migration_permission = entries.iter().any(|entry| entry.migration_permission);
    let runtime_authority_granted = entries.iter().any(|entry| entry.runtime_authority_granted);
    let proposal_first_convergence_complete = entries
        .iter()
        .all(|entry| entry.proposal_first_convergence_complete)
        && !entries.is_empty();
    let all_known_callers_classified = unclassified_callers.is_empty() && !missing_required_caller;
    let ordinary_chat_auto_checkin_classification = if ordinary_chat_auto_checkin_present {
        "source_data_compatibility_materialization_not_migration_permission".into()
    } else {
        "missing".into()
    };

    let matrix_ready = blocking_reasons.is_empty()
        && metadata_safe
        && !contains_raw_lifemodel_payload
        && !contains_raw_memory_text
        && !contains_raw_chat_text
        && !contains_raw_daily_goal_text
        && materializer_root_identified
        && all_known_callers_classified
        && default_chat_route_unchanged
        && !migration_permission
        && !runtime_authority_granted;

    LifeModelMaterializerCallerMatrixReport {
        matrix_ready,
        metadata_safe,
        contains_raw_lifemodel_payload,
        contains_raw_memory_text,
        contains_raw_chat_text,
        contains_raw_daily_goal_text,
        materializer_root_identified,
        all_known_callers_classified,
        unclassified_callers,
        caller_count: entries.len(),
        high_risk_legacy_blocker_count,
        proposal_first_count,
        source_data_compatibility_count,
        manual_override_count,
        restore_import_override_count,
        ordinary_chat_auto_checkin_present,
        ordinary_chat_auto_checkin_classification,
        default_chat_route_unchanged,
        migration_permission,
        runtime_authority_granted,
        proposal_first_convergence_complete,
        blocking_reasons,
    }
}

pub(crate) fn ensure_lifemodel_materializer_caller_matrix(
) -> Result<LifeModelMaterializerCallerMatrixReport, String> {
    let entries = lifemodel_materializer_caller_matrix();
    let report = evaluate_lifemodel_materializer_caller_matrix(&entries);
    if report.matrix_ready {
        Ok(report)
    } else {
        Err(format!(
            "LifeModel materializer caller matrix blocked: {}",
            report.blocking_reasons.join(",")
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

const STATE_DAILY_GOAL_DIRECT_WRITES_STABLE_ID: &str = "state_daily_goal_direct_writes";

const STATE_DAILY_GOAL_COMPATIBILITY_WRITER: &str = "persist_life_model";

const FORBIDDEN_STATE_DAILY_GOAL_HS_TRUTH_WRITERS: &[&str] =
    &["LifeModelManager::save", "save_life_model"];

const ORDINARY_CHAT_ENTRYPOINTS: &[&str] = &[
    "send_message",
    "start_stream_message",
    "default_chat",
    "default Chat",
];

const AUTOMATIC_TRUTH_PROMOTION_ALLOWED_MARKERS: &[&str] = &[
    "auto_promotion_allowed",
    "automatic_promotion_allowed",
    "automatically_promote_to_lifemodel_truth",
    "can automatically promote",
    "may automatically promote",
    "promote_without_proposal",
    "proposal not required",
];

const ACCEPTED_HS_TRUTH_WRITE_MARKERS: &[&str] = &[
    "accepted_hs_truth_write=true",
    "accepted durable hs truth write",
    "accepted durable lifemodel-hs truth write",
    "writes accepted durable lifemodel-hs truth",
];

const LIFEMODEL_MATERIALIZER_ROOT_CALLER_ID: &str = "lifemodel_materializer_root";

const REQUIRED_LIFEMODEL_MATERIALIZER_CALLER_IDS: &[&str] = &[
    "lifemodel_materializer_root",
    "lifemodel_manager_default_initialization",
    "ordinary_chat_auto_checkin_source_data",
    "ordinary_stream_agent_loop_auto_checkin_source_data",
    "ordinary_stream_legacy_auto_checkin_source_data",
    "manual_lifemodel_editor_save",
    "state_record_state_source_data",
    "state_add_daily_goal_source_data",
    "state_update_daily_goal_source_data",
    "state_delete_daily_goal_source_data",
    "state_toggle_daily_goal_source_data",
    "proposal_apply_lifemodel_update",
    "builder_step_legacy_direct_apply",
    "builder_apply_signals_legacy_direct_apply",
    "calibration_micro_evolution_legacy_direct_apply",
    "calibration_direct_apply_legacy_direct_apply",
    "feedback_evolution_legacy_direct_apply",
    "snapshot_restore_legacy_direct_apply",
    "data_import_legacy_direct_apply",
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

#[allow(clippy::too_many_arguments)]
fn caller_matrix_entry(
    stable_id: &str,
    display_name: &str,
    source_file_path: &str,
    caller_function_name: &str,
    write_entrypoint: &str,
    kind: LifeModelMaterializerCallerKind,
    risk: LifeModelMaterializerCallerRisk,
    governance_state: LifeModelMaterializerCallerGovernanceState,
    normal_product_allowed: bool,
    proposal_first: bool,
    source_data_compatibility: bool,
    manual_override: bool,
    restore_import_override: bool,
    high_risk_legacy_blocker: bool,
    proposal_first_convergence_complete: bool,
    accepted_durable_lifemodel_hs_truth: bool,
    required_follow_up: &str,
    blocking_reasons: &[&str],
) -> LifeModelMaterializerCallerMatrixEntry {
    LifeModelMaterializerCallerMatrixEntry {
        stable_id: stable_id.into(),
        display_name: display_name.into(),
        source_file_path: source_file_path.into(),
        caller_function_name: caller_function_name.into(),
        write_entrypoint: write_entrypoint.into(),
        kind,
        risk,
        governance_state,
        normal_product_allowed,
        proposal_first,
        source_data_compatibility,
        manual_override,
        restore_import_override,
        high_risk_legacy_blocker,
        metadata_safe: true,
        contains_raw_lifemodel_payload: false,
        contains_raw_memory_text: false,
        contains_raw_chat_text: false,
        contains_raw_daily_goal_text: false,
        default_chat_route_changed: false,
        migration_permission: false,
        runtime_authority_granted: false,
        accepted_durable_lifemodel_hs_truth,
        proposal_first_convergence_complete,
        required_follow_up: required_follow_up.into(),
        blocking_reasons: blocking_reasons
            .iter()
            .map(|reason| (*reason).into())
            .collect(),
    }
}

fn push_unique(reasons: &mut Vec<String>, reason: String) {
    if !reasons.iter().any(|existing| existing == &reason) {
        reasons.push(reason);
    }
}
