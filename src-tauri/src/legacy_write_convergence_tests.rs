use crate::legacy_write_convergence::{
    ensure_legacy_write_convergence_inventory_guard, ensure_lifemodel_materializer_caller_matrix,
    ensure_state_source_data_boundary, evaluate_legacy_write_convergence_inventory,
    evaluate_lifemodel_materializer_caller_matrix, evaluate_state_source_data_boundary,
    legacy_write_convergence_inventory, lifemodel_materializer_caller_matrix,
    LegacyWriteConvergenceStatus, LegacyWriteInventoryEntry, LegacyWritePathKind,
    LegacyWriteRiskClass, LegacyWriteSafeModeStatus, LifeModelMaterializerCallerGovernanceState,
    LifeModelMaterializerCallerKind, LifeModelMaterializerCallerMatrixEntry,
    LifeModelMaterializerCallerRisk,
};

fn entry<'a>(
    entries: &'a [LegacyWriteInventoryEntry],
    stable_id: &str,
) -> &'a LegacyWriteInventoryEntry {
    entries
        .iter()
        .find(|entry| entry.stable_id == stable_id)
        .unwrap_or_else(|| panic!("missing inventory entry {stable_id}"))
}

fn has_function(entry: &LegacyWriteInventoryEntry, function_name: &str) -> bool {
    entry
        .command_function_names
        .iter()
        .any(|name| name == function_name)
}

fn materializer_entry<'a>(
    entries: &'a [LifeModelMaterializerCallerMatrixEntry],
    stable_id: &str,
) -> &'a LifeModelMaterializerCallerMatrixEntry {
    entries
        .iter()
        .find(|entry| entry.stable_id == stable_id)
        .unwrap_or_else(|| panic!("missing materializer caller matrix entry {stable_id}"))
}

#[test]
fn legacy_write_convergence_inventory_covers_required_paths() {
    let entries = legacy_write_convergence_inventory();
    let required = [
        ("lifemodel_save_primitive", "LifeModelManager::save"),
        ("lifemodel_save_primitive", "persist_life_model"),
        ("manual_lifemodel_editor", "save_life_model"),
        ("builder_normal_proposal_flow", "builder_create_proposals"),
        ("builder_legacy_direct_apply", "builder_apply_signals"),
        (
            "builder_legacy_direct_apply",
            "builder_step no-signal completion branch",
        ),
        ("calibration_proposal_flow", "calibration_create_proposals"),
        ("calibration_direct_micro_evolution", "run_micro_evolution"),
        (
            "calibration_direct_micro_evolution",
            "apply_calibration(mode=direct)",
        ),
        (
            "feedback_evolution_direct_writes",
            "apply_feedback_evolution",
        ),
        (
            "feedback_evolution_direct_writes",
            "FeedbackEvolutionLegacyDirectApplyOverride",
        ),
        ("feedback_signals_source_data", "save_feedback"),
        ("feedback_signals_source_data", "log_analytics_event"),
        (
            "feedback_signals_source_data",
            "FeedbackStore::save_conversation_inference",
        ),
        (
            "feedback_signals_source_data",
            "FeedbackStore::fetch_evolution_signals",
        ),
        (
            "feedback_evolution_read_only_report",
            "generate_evolution_report",
        ),
        ("snapshot_restore", "restore_snapshot"),
        (
            "snapshot_restore",
            "SnapshotRestoreLegacyDirectApplyOverride",
        ),
        ("data_import", "import_all_data"),
        ("data_import", "apply_import_payload"),
        ("data_import", "DataImportLegacyDirectApplyOverride"),
        ("state_daily_goal_direct_writes", "add_daily_goal"),
        ("state_daily_goal_direct_writes", "persist_life_model"),
        (
            "state_daily_goal_direct_writes",
            "try_auto_checkin_daily_goals",
        ),
        ("raw_chat_memory_vector_source_writes", "save_message"),
        ("raw_chat_memory_vector_source_writes", "save_memory_record"),
        ("raw_chat_memory_vector_source_writes", "index_memory_chunk"),
        (
            "raw_chat_memory_vector_source_writes",
            "run_tier_maintenance",
        ),
        ("proposal_application_path", "accept_proposal"),
        ("proposal_application_path", "edit_proposal"),
        ("proposal_application_path", "apply_proposal_to_state"),
        ("external_write_proposal_path", "ExternalWriteAction"),
        ("external_write_proposal_path", "ScheduledTask"),
        ("external_write_proposal_path", "DataExport"),
        ("external_write_proposal_path", "calendar.propose_event"),
        ("external_write_proposal_path", "email.propose_draft"),
    ];

    for (stable_id, function_name) in required {
        assert!(
            has_function(entry(&entries, stable_id), function_name),
            "{stable_id} missing function {function_name}"
        );
    }

    assert_eq!(
        entry(&entries, "manual_lifemodel_editor").path_kind,
        LegacyWritePathKind::ManualLifeModelEditor
    );
    assert_eq!(
        entry(&entries, "proposal_application_path").path_kind,
        LegacyWritePathKind::ProposalApplicationPath
    );
}

#[test]
fn legacy_write_convergence_reports_high_risk_direct_writes_as_blockers_not_converged() {
    let entries = legacy_write_convergence_inventory();
    let report = evaluate_legacy_write_convergence_inventory(&entries);

    assert!(report.inventory_ready);
    assert!(!report.overall_converged);
    assert!(!report.all_direct_writes_converged);

    let save_primitive = entry(&entries, "lifemodel_save_primitive");
    assert_eq!(
        save_primitive.risk_class,
        LegacyWriteRiskClass::CompatibilityMaterializer
    );
    assert_eq!(
        save_primitive.current_status,
        LegacyWriteConvergenceStatus::CompatibilityPrimitive
    );
    assert!(save_primitive.currently_direct_write);
    assert!(save_primitive.high_risk_durable_truth_write);
    assert!(!save_primitive.normal_product_allowed);
    assert!(save_primitive
        .required_convergence_action
        .contains("accepted proposal application"));
    assert!(report
        .convergence_blockers
        .iter()
        .any(|blocker| blocker.contains("lifemodel_save_primitive")));

    for stable_id in [
        "manual_lifemodel_editor",
        "builder_legacy_direct_apply",
        "calibration_direct_micro_evolution",
        "feedback_evolution_direct_writes",
        "snapshot_restore",
        "data_import",
    ] {
        let entry = entry(&entries, stable_id);
        assert_eq!(
            entry.risk_class,
            LegacyWriteRiskClass::HighRiskLegacyDirectWrite
        );
        assert_eq!(
            entry.current_status,
            LegacyWriteConvergenceStatus::LegacyDirectWriteBlocker
        );
        assert!(entry.currently_direct_write);
        assert!(entry.high_risk_durable_truth_write);
        assert!(!entry.normal_product_allowed);
        assert!(
            report
                .convergence_blockers
                .iter()
                .any(|blocker| blocker.contains(stable_id)),
            "{stable_id} should remain a convergence blocker"
        );
    }

    ensure_legacy_write_convergence_inventory_guard()
        .expect("known convergence blockers should be reported without failing inventory guard");
}

#[test]
fn legacy_write_convergence_feedback_w83_guard_present_but_still_blocker() {
    let entries = legacy_write_convergence_inventory();
    let report = evaluate_legacy_write_convergence_inventory(&entries);
    let feedback_legacy = entry(&entries, "feedback_evolution_direct_writes");
    let feedback_report = entry(&entries, "feedback_evolution_read_only_report");

    assert_eq!(
        feedback_legacy.risk_class,
        LegacyWriteRiskClass::HighRiskLegacyDirectWrite
    );
    assert_eq!(
        feedback_legacy.current_status,
        LegacyWriteConvergenceStatus::LegacyDirectWriteBlocker
    );
    assert_eq!(
        feedback_legacy.safe_mode_status,
        LegacyWriteSafeModeStatus::GuardPresent
    );
    assert!(!feedback_legacy.normal_product_allowed);
    assert!(feedback_legacy.requires_proposal_first);
    assert!(feedback_legacy.currently_direct_write);
    assert!(feedback_legacy.high_risk_durable_truth_write);
    assert!(feedback_legacy.current_guard_summary.contains("W83"));
    assert!(feedback_legacy
        .current_guard_summary
        .contains("dev/migration"));
    assert!(feedback_legacy
        .current_guard_summary
        .contains("metadata-safe"));
    assert!(feedback_legacy
        .required_convergence_action
        .contains("Proposal"));
    assert!(feedback_legacy
        .blocking_reasons
        .iter()
        .any(|reason| reason.contains("dev_migration_override")));
    assert!(has_function(
        feedback_legacy,
        "FeedbackEvolutionLegacyDirectApplyOverride"
    ));
    assert!(!has_function(feedback_legacy, "generate_evolution_report"));
    assert!(report
        .convergence_blockers
        .iter()
        .any(|blocker| blocker.contains("feedback_evolution_direct_writes")));

    assert_eq!(
        feedback_report.current_status,
        LegacyWriteConvergenceStatus::LowRiskTransientSourceData
    );
    assert!(!feedback_report.currently_direct_write);
    assert!(!feedback_report.high_risk_durable_truth_write);
    assert!(feedback_report.normal_product_allowed);
    assert!(feedback_report.current_guard_summary.contains("read-only"));
    assert!(feedback_report
        .current_guard_summary
        .contains("does not write LifeModel"));
    assert!(
        !report
            .convergence_blockers
            .iter()
            .any(|blocker| blocker.contains("feedback_evolution_read_only_report")),
        "read-only feedback report must not be a legacy direct-write blocker"
    );

    assert!(!report.overall_converged);
    assert!(!report.all_direct_writes_converged);
}

#[test]
fn legacy_write_convergence_snapshot_restore_and_data_import_w84_guards_present_but_still_blockers()
{
    let entries = legacy_write_convergence_inventory();
    let report = evaluate_legacy_write_convergence_inventory(&entries);

    for (stable_id, override_name) in [
        (
            "snapshot_restore",
            "SnapshotRestoreLegacyDirectApplyOverride",
        ),
        ("data_import", "DataImportLegacyDirectApplyOverride"),
    ] {
        let entry = entry(&entries, stable_id);
        assert_eq!(
            entry.risk_class,
            LegacyWriteRiskClass::HighRiskLegacyDirectWrite
        );
        assert_eq!(
            entry.current_status,
            LegacyWriteConvergenceStatus::LegacyDirectWriteBlocker
        );
        assert_eq!(
            entry.safe_mode_status,
            LegacyWriteSafeModeStatus::GuardPresent
        );
        assert!(!entry.normal_product_allowed);
        assert!(entry.requires_proposal_first);
        assert!(entry.currently_direct_write);
        assert!(entry.high_risk_durable_truth_write);
        assert!(entry.current_guard_summary.contains("W84"));
        assert!(entry.current_guard_summary.contains("dev/migration/manual"));
        assert!(entry.current_guard_summary.contains("metadata-safe"));
        assert!(has_function(entry, override_name));
        assert!(entry
            .blocking_reasons
            .iter()
            .any(|reason| reason.contains("manual_restore_override")
                || reason.contains("dev_migration_override")));
        assert!(
            report
                .convergence_blockers
                .iter()
                .any(|blocker| blocker.contains(stable_id)),
            "{stable_id} should remain a convergence blocker"
        );
    }
}

#[test]
fn legacy_write_convergence_manual_editor_w80_audit_guard_remains_blocker_not_converged() {
    let entries = legacy_write_convergence_inventory();
    let report = evaluate_legacy_write_convergence_inventory(&entries);
    let manual = entry(&entries, "manual_lifemodel_editor");

    assert_eq!(
        manual.risk_class,
        LegacyWriteRiskClass::HighRiskLegacyDirectWrite
    );
    assert_eq!(
        manual.current_status,
        LegacyWriteConvergenceStatus::LegacyDirectWriteBlocker
    );
    assert_eq!(
        manual.safe_mode_status,
        LegacyWriteSafeModeStatus::GuardPresent
    );
    assert!(manual.requires_proposal_first);
    assert!(manual.currently_direct_write);
    assert!(manual.high_risk_durable_truth_write);
    assert!(!manual.normal_product_allowed);
    assert!(manual
        .current_guard_summary
        .contains("W80 metadata-safe manual override audit"));
    assert!(manual
        .required_convergence_action
        .contains("proposal patch review"));
    assert!(manual
        .required_convergence_action
        .contains("stronger manual override UX"));
    assert!(report
        .convergence_blockers
        .iter()
        .any(|blocker| blocker.contains("manual_lifemodel_editor")));
    assert!(!report.overall_converged);
    assert!(!report.all_direct_writes_converged);
}

#[test]
fn legacy_write_convergence_builder_w81_dev_gate_guard_present_but_still_blocker() {
    let entries = legacy_write_convergence_inventory();
    let report = evaluate_legacy_write_convergence_inventory(&entries);
    let builder_legacy = entry(&entries, "builder_legacy_direct_apply");
    let builder_normal = entry(&entries, "builder_normal_proposal_flow");

    assert_eq!(
        builder_normal.current_status,
        LegacyWriteConvergenceStatus::AlreadyProposalFirst
    );
    assert_eq!(
        builder_normal.safe_mode_status,
        LegacyWriteSafeModeStatus::ProposalFirstGuardPresent
    );
    assert!(builder_normal.normal_product_allowed);
    assert!(!builder_normal.currently_direct_write);
    assert!(builder_normal
        .current_guard_summary
        .contains("builder_create_proposals"));

    assert_eq!(
        builder_legacy.risk_class,
        LegacyWriteRiskClass::HighRiskLegacyDirectWrite
    );
    assert_eq!(
        builder_legacy.current_status,
        LegacyWriteConvergenceStatus::LegacyDirectWriteBlocker
    );
    assert_eq!(
        builder_legacy.safe_mode_status,
        LegacyWriteSafeModeStatus::GuardPresent
    );
    assert!(!builder_legacy.normal_product_allowed);
    assert!(builder_legacy.currently_direct_write);
    assert!(builder_legacy.high_risk_durable_truth_write);
    assert!(builder_legacy.current_guard_summary.contains("W81"));
    assert!(builder_legacy
        .current_guard_summary
        .contains("dev/migration"));
    assert!(builder_legacy
        .current_guard_summary
        .contains("no-signal completion"));
    assert!(builder_legacy
        .required_convergence_action
        .contains("builder_create_proposals"));
    assert!(report
        .convergence_blockers
        .iter()
        .any(|blocker| blocker.contains("builder_legacy_direct_apply")));
}

#[test]
fn legacy_write_convergence_calibration_w82_dev_gate_guard_present_but_still_blocker() {
    let entries = legacy_write_convergence_inventory();
    let report = evaluate_legacy_write_convergence_inventory(&entries);
    let calibration_normal = entry(&entries, "calibration_proposal_flow");
    let calibration_legacy = entry(&entries, "calibration_direct_micro_evolution");

    assert_eq!(
        calibration_normal.current_status,
        LegacyWriteConvergenceStatus::AlreadyProposalFirst
    );
    assert_eq!(
        calibration_normal.safe_mode_status,
        LegacyWriteSafeModeStatus::ProposalFirstGuardPresent
    );
    assert!(calibration_normal.normal_product_allowed);
    assert!(calibration_normal.requires_proposal_first);
    assert!(!calibration_normal.currently_direct_write);
    assert!(calibration_normal
        .current_guard_summary
        .contains("calibration_create_proposals"));

    assert_eq!(
        calibration_legacy.risk_class,
        LegacyWriteRiskClass::HighRiskLegacyDirectWrite
    );
    assert_eq!(
        calibration_legacy.current_status,
        LegacyWriteConvergenceStatus::LegacyDirectWriteBlocker
    );
    assert_eq!(
        calibration_legacy.safe_mode_status,
        LegacyWriteSafeModeStatus::GuardPresent
    );
    assert!(!calibration_legacy.normal_product_allowed);
    assert!(calibration_legacy.requires_proposal_first);
    assert!(calibration_legacy.currently_direct_write);
    assert!(calibration_legacy.high_risk_durable_truth_write);
    assert!(calibration_legacy.current_guard_summary.contains("W82"));
    assert!(calibration_legacy
        .current_guard_summary
        .contains("dev/migration"));
    assert!(calibration_legacy
        .current_guard_summary
        .contains("metadata-safe"));
    assert!(calibration_legacy
        .required_convergence_action
        .contains("calibration_create_proposals"));
    assert!(calibration_legacy
        .blocking_reasons
        .iter()
        .any(|reason| reason.contains("dev_migration_override")));
    assert!(report
        .convergence_blockers
        .iter()
        .any(|blocker| blocker.contains("calibration_direct_micro_evolution")));
    assert!(!report.overall_converged);
    assert!(!report.all_direct_writes_converged);
}

#[test]
fn legacy_write_convergence_identifies_proposal_first_targets_without_direct_unsafe_blocker() {
    let entries = legacy_write_convergence_inventory();
    let report = evaluate_legacy_write_convergence_inventory(&entries);

    for stable_id in [
        "builder_normal_proposal_flow",
        "calibration_proposal_flow",
        "proposal_application_path",
        "external_write_proposal_path",
    ] {
        let entry = entry(&entries, stable_id);
        assert!(matches!(
            entry.risk_class,
            LegacyWriteRiskClass::ProposalFirstConvergenceTarget
                | LegacyWriteRiskClass::ProposalOnlyDeclarative
        ));
        assert!(matches!(
            entry.current_status,
            LegacyWriteConvergenceStatus::AlreadyProposalFirst
                | LegacyWriteConvergenceStatus::ProposalFirstConvergenceTarget
                | LegacyWriteConvergenceStatus::ProposalOnlyDeclarative
        ));
        assert!(entry.requires_proposal_first);
        assert!(!entry.currently_direct_write);
        assert!(
            !report
                .convergence_blockers
                .iter()
                .any(|blocker| blocker.contains(stable_id)),
            "{stable_id} must not be reported as a direct unsafe blocker"
        );
    }
}

#[test]
fn legacy_write_convergence_low_risk_source_data_is_not_durable_lifemodel_truth() {
    let entries = legacy_write_convergence_inventory();

    for stable_id in [
        "feedback_signals_source_data",
        "state_daily_goal_direct_writes",
        "raw_chat_memory_vector_source_writes",
    ] {
        let entry = entry(&entries, stable_id);
        assert!(matches!(
            entry.risk_class,
            LegacyWriteRiskClass::LowRiskTransientState | LegacyWriteRiskClass::LowRiskSourceData
        ));
        assert!(entry.currently_direct_write);
        assert!(!entry.high_risk_durable_truth_write);
        assert!(!entry.requires_proposal_first);
        assert!(
            entry
                .current_guard_summary
                .contains("must not automatically promote to durable LifeModel truth")
                || entry
                    .current_guard_summary
                    .contains("must not automatically promote to durable LifeModel-HS truth")
        );
        assert!(
            entry
                .required_convergence_action
                .contains("proposal-first before durable LifeModel truth")
                || entry
                    .required_convergence_action
                    .contains("proposal-first before durable LifeModel-HS truth")
        );
    }
}

#[test]
fn legacy_write_convergence_feedback_signals_are_low_risk_source_data_not_accepted_truth() {
    let entries = legacy_write_convergence_inventory();
    let report = evaluate_legacy_write_convergence_inventory(&entries);
    let feedback_signals = entry(&entries, "feedback_signals_source_data");

    assert_eq!(
        feedback_signals.risk_class,
        LegacyWriteRiskClass::LowRiskSourceData
    );
    assert_eq!(
        feedback_signals.current_status,
        LegacyWriteConvergenceStatus::LowRiskTransientSourceData
    );
    assert_eq!(
        feedback_signals.safe_mode_status,
        LegacyWriteSafeModeStatus::LowRiskSourceDataGuardNotRequired
    );
    assert!(feedback_signals.normal_product_allowed);
    assert!(feedback_signals.currently_direct_write);
    assert!(!feedback_signals.high_risk_durable_truth_write);
    assert!(!feedback_signals.requires_proposal_first);
    assert!(feedback_signals
        .current_guard_summary
        .contains("signals, not accepted LifeModel truth"));
    assert!(feedback_signals
        .required_convergence_action
        .contains("Proposal"));
    assert!(
        !report
            .convergence_blockers
            .iter()
            .any(|blocker| blocker.contains("feedback_signals_source_data")),
        "Feedback signal source data must not be reported as accepted truth blocker"
    );
}

#[test]
fn legacy_write_convergence_state_daily_goal_w85_boundary_is_low_risk_source_data() {
    let entries = legacy_write_convergence_inventory();
    let entry = entry(&entries, "state_daily_goal_direct_writes");
    let report = evaluate_state_source_data_boundary(&entries);

    assert_eq!(
        entry.risk_class,
        LegacyWriteRiskClass::LowRiskTransientState
    );
    assert_eq!(
        entry.current_status,
        LegacyWriteConvergenceStatus::LowRiskTransientSourceData
    );
    assert_eq!(
        entry.safe_mode_status,
        LegacyWriteSafeModeStatus::LowRiskSourceDataGuardNotRequired
    );
    assert!(entry.normal_product_allowed);
    assert!(entry.currently_direct_write);
    assert!(!entry.high_risk_durable_truth_write);
    assert!(!entry.requires_proposal_first);
    assert!(has_function(entry, "persist_life_model"));
    assert!(entry.current_guard_summary.contains("source data"));
    assert!(entry
        .current_guard_summary
        .contains("writes the current LifeModel compatibility view"));
    assert!(entry
        .current_guard_summary
        .contains("not accepted durable LifeModel-HS truth"));
    assert!(entry
        .current_guard_summary
        .contains("must not automatically promote to durable LifeModel-HS truth"));

    assert_eq!(
        report.state_daily_goal_path_ids,
        vec!["state_daily_goal_direct_writes".to_string()]
    );
    assert_eq!(
        report.source_data_classification,
        "state_daily_goal_source_data_not_accepted_lifemodel_hs_truth"
    );
    assert_eq!(
        report.low_risk_transient_classification,
        "low_risk_transient_source_data"
    );
    assert!(report.compatibility_lifemodel_materialized_write);
    assert!(report.writes_current_lifemodel_compatibility_view);
    assert!(!report.accepted_durable_hs_truth_write);
    assert!(!report.active_hs_lifemodel_patch);
    assert!(report.proposal_required_for_hs_truth_promotion);
    assert!(report.ordinary_chat_unchanged);
    assert!(report.default_chat_unchanged);
    assert!(report.blocking_reasons.is_empty());

    ensure_state_source_data_boundary().expect("known W85 state boundary should pass");
}

#[test]
fn legacy_write_convergence_state_daily_goal_w85_report_is_metadata_safe() {
    let entries = legacy_write_convergence_inventory();
    let report = evaluate_state_source_data_boundary(&entries);

    assert!(report.blocking_reasons.is_empty());
    assert!(report.compatibility_lifemodel_materialized_write);
    assert!(report.writes_current_lifemodel_compatibility_view);
    assert!(!report.accepted_durable_hs_truth_write);
    assert!(!report.active_hs_lifemodel_patch);
    assert!(report.proposal_required_for_hs_truth_promotion);

    let debug_dump = format!("{report:?}");
    for forbidden in [
        "W85_RAW_STATE_TEXT_SECRET",
        "W85_RAW_DAILY_GOAL_NAME_SECRET",
        "RAW_STATE_TEXT_SECRET",
        "RAW_DAILY_GOAL_NAME_SECRET",
        "RAW_USER_CONTENT_SECRET",
        "RAW_LIFEMODEL_PAYLOAD_SECRET",
        "RAW_MEMORY_PAYLOAD_SECRET",
        "drink more water",
        "my private mood note",
        "secret goal name",
    ] {
        assert!(
            !debug_dump.contains(forbidden),
            "state source-data boundary report leaked raw marker {forbidden}"
        );
    }
}

#[test]
fn legacy_write_convergence_state_daily_goal_w85_boundary_fails_closed_for_bad_inventory() {
    fn assert_boundary_reason(
        mutate: impl FnOnce(&mut Vec<LegacyWriteInventoryEntry>),
        expected_reason: &str,
    ) {
        let mut entries = legacy_write_convergence_inventory();
        mutate(&mut entries);
        let report = evaluate_state_source_data_boundary(&entries);
        assert!(
            report
                .blocking_reasons
                .iter()
                .any(|reason| reason.contains(expected_reason)),
            "missing W85 boundary reason {expected_reason}: {:?}",
            report.blocking_reasons
        );
    }

    assert_boundary_reason(
        |entries| entries.retain(|entry| entry.stable_id != "state_daily_goal_direct_writes"),
        "state_daily_goal_direct_writes_inventory_entry_missing",
    );
    assert_boundary_reason(
        |entries| {
            entry_mut(entries, "state_daily_goal_direct_writes")
                .command_function_names
                .retain(|name| name != "persist_life_model")
        },
        "state_daily_goal_direct_writes_missing_compatibility_materialized_writer",
    );
    assert_boundary_reason(
        |entries| {
            entry_mut(entries, "state_daily_goal_direct_writes").risk_class =
                LegacyWriteRiskClass::HighRiskLegacyDirectWrite
        },
        "state_daily_goal_direct_writes_marked_high_risk_legacy_direct_write",
    );
    assert_boundary_reason(
        |entries| {
            entry_mut(entries, "state_daily_goal_direct_writes").current_status =
                LegacyWriteConvergenceStatus::AlreadyProposalFirst
        },
        "state_daily_goal_direct_writes_marked_already_proposal_first_or_converged",
    );
    assert_boundary_reason(
        |entries| {
            entry_mut(entries, "state_daily_goal_direct_writes").current_status =
                LegacyWriteConvergenceStatus::ProposalFirstConvergenceTarget
        },
        "state_daily_goal_direct_writes_marked_already_proposal_first_or_converged",
    );
    assert_boundary_reason(
        |entries| {
            entry_mut(entries, "state_daily_goal_direct_writes").current_status =
                LegacyWriteConvergenceStatus::ProposalOnlyDeclarative
        },
        "state_daily_goal_direct_writes_marked_already_proposal_first_or_converged",
    );
    assert_boundary_reason(
        |entries| {
            entry_mut(entries, "state_daily_goal_direct_writes").high_risk_durable_truth_write =
                true
        },
        "state_daily_goal_direct_writes_marked_durable_lifemodel_truth_write",
    );
    assert_boundary_reason(
        |entries| {
            let entry = entry_mut(entries, "state_daily_goal_direct_writes");
            entry.requires_proposal_first = true;
            entry.normal_product_allowed = false;
        },
        "state_daily_goal_direct_writes_confuses_proposal_first_with_blocked_normal_product",
    );
    for durable_writer in ["LifeModelManager::save", "save_life_model"] {
        assert_boundary_reason(
            |entries| {
                entry_mut(entries, "state_daily_goal_direct_writes")
                    .command_function_names
                    .push(durable_writer.into())
            },
            "state_daily_goal_direct_writes_lists_durable_lifemodel_truth_writer",
        );
    }
    assert_boundary_reason(
        |entries| {
            entry_mut(entries, "state_daily_goal_direct_writes")
                .current_guard_summary
                .push_str(" accepted_hs_truth_write=true")
        },
        "state_daily_goal_direct_writes_claims_accepted_hs_truth_write",
    );
    assert_boundary_reason(
        |entries| {
            entry_mut(entries, "state_daily_goal_direct_writes")
                .current_guard_summary
                .push_str(" auto_promotion_allowed")
        },
        "state_daily_goal_direct_writes_implies_automatic_truth_promotion",
    );
    assert_boundary_reason(
        |entries| {
            entry_mut(entries, "state_daily_goal_direct_writes")
                .command_function_names
                .push("send_message".into())
        },
        "state_daily_goal_direct_writes_lists_default_or_ordinary_chat_entrypoint",
    );
    assert_boundary_reason(
        |entries| {
            entry_mut(entries, "state_daily_goal_direct_writes")
                .command_function_names
                .push("start_stream_message".into())
        },
        "state_daily_goal_direct_writes_lists_default_or_ordinary_chat_entrypoint",
    );
    assert_boundary_reason(
        |entries| entry_mut(entries, "state_daily_goal_direct_writes").default_chat_affected = true,
        "state_daily_goal_direct_writes_affects_default_chat",
    );
}

#[test]
fn legacy_write_convergence_w86_materializer_caller_matrix_covers_known_callers() {
    let entries = lifemodel_materializer_caller_matrix();
    let report = evaluate_lifemodel_materializer_caller_matrix(&entries);
    let required = [
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

    assert!(report.matrix_ready);
    assert!(report.metadata_safe);
    assert!(!report.contains_raw_lifemodel_payload);
    assert!(!report.contains_raw_memory_text);
    assert!(!report.contains_raw_chat_text);
    assert!(!report.contains_raw_daily_goal_text);
    assert!(report.materializer_root_identified);
    assert!(report.all_known_callers_classified);
    assert!(report.unclassified_callers.is_empty());
    assert_eq!(report.caller_count, entries.len());
    assert_eq!(report.caller_count, required.len());
    assert!(report.high_risk_legacy_blocker_count >= 7);
    assert_eq!(report.proposal_first_count, 1);
    assert_eq!(report.source_data_compatibility_count, 8);
    assert_eq!(report.manual_override_count, 1);
    assert_eq!(report.restore_import_override_count, 2);
    assert!(report.ordinary_chat_auto_checkin_present);
    assert_eq!(
        report.ordinary_chat_auto_checkin_classification,
        "source_data_compatibility_materialization_not_migration_permission"
    );
    assert!(report.default_chat_route_unchanged);
    assert!(!report.migration_permission);
    assert!(!report.runtime_authority_granted);
    assert!(!report.proposal_first_convergence_complete);
    assert!(report.blocking_reasons.is_empty());

    for stable_id in required {
        assert!(
            entries.iter().any(|entry| entry.stable_id == stable_id),
            "missing W86 materializer caller matrix entry {stable_id}"
        );
    }

    ensure_lifemodel_materializer_caller_matrix()
        .expect("known W86 materializer caller matrix should pass");
}

#[test]
fn legacy_write_convergence_w86_materializer_matrix_matches_current_production_callsite_count() {
    let entries = lifemodel_materializer_caller_matrix();
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let persist_call_files = [
        "src/lib.rs",
        "src/commands/builder.rs",
        "src/commands/settings.rs",
        "src/commands/life_model.rs",
        "src/commands/proposal.rs",
        "src/commands/feedback.rs",
        "src/commands/state.rs",
        "src/commands/calibration.rs",
    ];
    let actual_persist_call_count = persist_call_files
        .iter()
        .map(|path| {
            let source =
                std::fs::read_to_string(format!("{manifest_dir}/{path}")).expect("read source");
            source
                .lines()
                .filter(|line| line.contains("persist_life_model("))
                .filter(|line| !line.contains("async fn persist_life_model("))
                .count()
        })
        .sum::<usize>();
    let matrix_persist_call_count = entries
        .iter()
        .filter(|entry| entry.write_entrypoint == "persist_life_model")
        .count();
    assert_eq!(
        matrix_persist_call_count, actual_persist_call_count,
        "W86 matrix must classify every current production persist_life_model caller"
    );

    let lib_rs = std::fs::read_to_string(format!("{manifest_dir}/src/lib.rs")).expect("read lib");
    let version_rs = std::fs::read_to_string(format!("{manifest_dir}/src/commands/version.rs"))
        .expect("read version");
    let core_life_model_rs = std::fs::read_to_string(format!(
        "{}/../openlife-core/src/life_model.rs",
        manifest_dir
    ))
    .expect("read core life_model");
    let actual_direct_save_count = count_occurrences(&lib_rs, "manager.save(&life_model)")
        + count_occurrences(&version_rs, "manager.save(&restored_model)")
        + count_occurrences(&core_life_model_rs, "self.save(&model)");
    let matrix_direct_save_count = entries
        .iter()
        .filter(|entry| entry.write_entrypoint == "LifeModelManager::save")
        .count();
    assert_eq!(
        matrix_direct_save_count, actual_direct_save_count,
        "W86 matrix must classify every current production LifeModelManager::save caller"
    );
}

#[test]
fn legacy_write_convergence_w86_materializer_matrix_fails_closed_for_unclassified_caller() {
    let mut entries = lifemodel_materializer_caller_matrix();
    entries.push(LifeModelMaterializerCallerMatrixEntry {
        stable_id: "synthetic_unclassified_materializer_caller".into(),
        display_name: "Synthetic unclassified materializer caller".into(),
        source_file_path: "src-tauri/src/synthetic.rs".into(),
        caller_function_name: "synthetic_unclassified_caller".into(),
        write_entrypoint: "persist_life_model".into(),
        kind: LifeModelMaterializerCallerKind::Unclassified,
        risk: LifeModelMaterializerCallerRisk::Unclassified,
        governance_state: LifeModelMaterializerCallerGovernanceState::Unclassified,
        normal_product_allowed: false,
        proposal_first: false,
        source_data_compatibility: false,
        manual_override: false,
        restore_import_override: false,
        high_risk_legacy_blocker: false,
        metadata_safe: true,
        contains_raw_lifemodel_payload: false,
        contains_raw_memory_text: false,
        contains_raw_chat_text: false,
        contains_raw_daily_goal_text: false,
        default_chat_route_changed: false,
        migration_permission: false,
        runtime_authority_granted: false,
        accepted_durable_lifemodel_hs_truth: false,
        proposal_first_convergence_complete: false,
        required_follow_up: "classify this caller before W87 restriction".into(),
        blocking_reasons: vec![],
    });

    let report = evaluate_lifemodel_materializer_caller_matrix(&entries);
    assert!(!report.matrix_ready);
    assert!(!report.all_known_callers_classified);
    assert!(report
        .unclassified_callers
        .contains(&"synthetic_unclassified_materializer_caller".to_string()));
    assert!(report
        .blocking_reasons
        .iter()
        .any(|reason| reason.contains("unclassified_materializer_caller")));
}

#[test]
fn legacy_write_convergence_w86_ordinary_chat_auto_checkin_is_source_data_only() {
    let entries = lifemodel_materializer_caller_matrix();
    let report = evaluate_lifemodel_materializer_caller_matrix(&entries);

    for stable_id in [
        "ordinary_chat_auto_checkin_source_data",
        "ordinary_stream_agent_loop_auto_checkin_source_data",
        "ordinary_stream_legacy_auto_checkin_source_data",
    ] {
        let entry = materializer_entry(&entries, stable_id);
        assert_eq!(
            entry.kind,
            LifeModelMaterializerCallerKind::OrdinaryChatAutoCheckinSourceData
        );
        assert_eq!(
            entry.risk,
            LifeModelMaterializerCallerRisk::SourceDataCompatibilityWrite
        );
        assert_eq!(
            entry.governance_state,
            LifeModelMaterializerCallerGovernanceState::SourceDataCompatibilityNotAcceptedTruth
        );
        assert!(entry.source_data_compatibility);
        assert!(entry.normal_product_allowed);
        assert!(!entry.migration_permission);
        assert!(!entry.runtime_authority_granted);
        assert!(!entry.accepted_durable_lifemodel_hs_truth);
        assert!(!entry.proposal_first_convergence_complete);
        assert!(entry
            .required_follow_up
            .contains("not accepted durable LifeModel-HS truth"));
    }

    assert!(report.ordinary_chat_auto_checkin_present);
    assert!(!report.migration_permission);
    assert!(!report.runtime_authority_granted);
}

#[test]
fn legacy_write_convergence_w86_proposal_apply_is_accepted_apply_but_mapping_not_complete() {
    let entries = lifemodel_materializer_caller_matrix();
    let entry = materializer_entry(&entries, "proposal_apply_lifemodel_update");
    let report = evaluate_lifemodel_materializer_caller_matrix(&entries);

    assert_eq!(
        entry.kind,
        LifeModelMaterializerCallerKind::AcceptedProposalApply
    );
    assert_eq!(
        entry.risk,
        LifeModelMaterializerCallerRisk::AcceptedProposalApply
    );
    assert_eq!(
        entry.governance_state,
        LifeModelMaterializerCallerGovernanceState::AcceptedProposalApplyNeedsSourceSpecificPatchMapping
    );
    assert!(entry.proposal_first);
    assert!(!entry.high_risk_legacy_blocker);
    assert!(!entry.proposal_first_convergence_complete);
    assert!(entry
        .required_follow_up
        .contains("source-specific patch mapping"));
    assert_eq!(report.proposal_first_count, 1);
    assert!(!report.proposal_first_convergence_complete);
}

#[test]
fn legacy_write_convergence_w86_manual_editor_is_audited_override_still_blocker() {
    let entries = lifemodel_materializer_caller_matrix();
    let entry = materializer_entry(&entries, "manual_lifemodel_editor_save");

    assert_eq!(
        entry.kind,
        LifeModelMaterializerCallerKind::ManualOverrideAudited
    );
    assert_eq!(
        entry.risk,
        LifeModelMaterializerCallerRisk::HighRiskManualOverrideBlocker
    );
    assert_eq!(
        entry.governance_state,
        LifeModelMaterializerCallerGovernanceState::AuditedManualOverrideStillLegacyBlocker
    );
    assert!(entry.manual_override);
    assert!(entry.high_risk_legacy_blocker);
    assert!(!entry.proposal_first);
    assert!(!entry.proposal_first_convergence_complete);
    assert!(entry.required_follow_up.contains("proposal"));
}

#[test]
fn legacy_write_convergence_w86_restore_and_import_are_gated_overrides_still_blockers() {
    let entries = lifemodel_materializer_caller_matrix();
    let report = evaluate_lifemodel_materializer_caller_matrix(&entries);

    for stable_id in [
        "snapshot_restore_legacy_direct_apply",
        "data_import_legacy_direct_apply",
    ] {
        let entry = materializer_entry(&entries, stable_id);
        assert_eq!(
            entry.kind,
            LifeModelMaterializerCallerKind::MigrationRestoreGated
        );
        assert_eq!(
            entry.risk,
            LifeModelMaterializerCallerRisk::HighRiskRestoreImportBlocker
        );
        assert_eq!(
            entry.governance_state,
            LifeModelMaterializerCallerGovernanceState::RestoreImportGatedLegacyBlocker
        );
        assert!(entry.restore_import_override);
        assert!(entry.high_risk_legacy_blocker);
        assert!(!entry.proposal_first_convergence_complete);
        assert!(!entry.accepted_durable_lifemodel_hs_truth);
        assert!(entry.required_follow_up.contains("migration"));
    }

    assert_eq!(report.restore_import_override_count, 2);
    assert!(!report.proposal_first_convergence_complete);
}

#[test]
fn legacy_write_convergence_w86_report_is_metadata_safe_and_raw_content_free() {
    let entries = lifemodel_materializer_caller_matrix();
    let report = evaluate_lifemodel_materializer_caller_matrix(&entries);

    assert!(report.metadata_safe);
    assert!(!report.contains_raw_lifemodel_payload);
    assert!(!report.contains_raw_memory_text);
    assert!(!report.contains_raw_chat_text);
    assert!(!report.contains_raw_daily_goal_text);
    assert!(entries.iter().all(|entry| entry.metadata_safe));
    assert!(entries
        .iter()
        .all(|entry| !entry.contains_raw_lifemodel_payload));
    assert!(entries.iter().all(|entry| !entry.contains_raw_memory_text));
    assert!(entries.iter().all(|entry| !entry.contains_raw_chat_text));
    assert!(entries
        .iter()
        .all(|entry| !entry.contains_raw_daily_goal_text));

    let debug_dump = format!("{report:?} {entries:?}");
    for forbidden in [
        "W86_RAW_PROMPT_SECRET",
        "W86_RAW_ASSISTANT_OUTPUT_SECRET",
        "W86_RAW_TOOL_PAYLOAD_SECRET",
        "W86_RAW_LIFEMODEL_TEXT_SECRET",
        "W86_RAW_MEMORY_TEXT_SECRET",
        "W86_RAW_DAILY_GOAL_TEXT_SECRET",
        "RAW_PROMPT_SECRET",
        "RAW_ASSISTANT_OUTPUT_SECRET",
        "RAW_TOOL_PAYLOAD_SECRET",
        "RAW_LIFEMODEL_PAYLOAD_SECRET",
        "RAW_MEMORY_PAYLOAD_SECRET",
        "RAW_DAILY_GOAL_NAME_SECRET",
        "private prompt text",
        "assistant hidden answer",
        "tool payload body",
        "life model raw yaml",
        "memory raw text",
        "daily goal raw text",
    ] {
        assert!(
            !debug_dump.contains(forbidden),
            "W86 materializer caller matrix leaked raw marker {forbidden}"
        );
    }
}

#[test]
fn legacy_write_convergence_w86_default_chat_route_unchanged_and_no_runtime_hook() {
    let entries = lifemodel_materializer_caller_matrix();
    let report = evaluate_lifemodel_materializer_caller_matrix(&entries);

    assert!(report.default_chat_route_unchanged);
    assert!(!report.runtime_authority_granted);
    assert!(!report.migration_permission);

    let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(lib_rs_path).expect("read src/lib.rs");
    let send_body = extract_rust_function_body(&source, "async fn send_message(");
    let stream_body = extract_rust_function_body(&source, "async fn start_stream_message(");
    for forbidden in [
        "lifemodel_materializer_caller_matrix",
        "evaluate_lifemodel_materializer_caller_matrix",
        "ensure_lifemodel_materializer_caller_matrix",
        "LifeModelMaterializerCallerMatrixReport",
    ] {
        assert!(
            !send_body.contains(forbidden),
            "send_message must not call W86 materializer caller matrix API {forbidden}"
        );
        assert!(
            !stream_body.contains(forbidden),
            "start_stream_message must not call W86 materializer caller matrix API {forbidden}"
        );
    }
}

#[test]
fn legacy_write_convergence_report_is_metadata_safe_and_raw_content_free() {
    let entries = legacy_write_convergence_inventory();
    let report = evaluate_legacy_write_convergence_inventory(&entries);

    assert!(report.metadata_safe);
    assert!(!report.contains_raw_content);
    assert!(entries.iter().all(|entry| entry.metadata_safe));
    assert!(entries.iter().all(|entry| !entry.contains_raw_content));

    let debug_dump = format!("{report:?} {entries:?}");
    for forbidden in [
        "RAW_PROMPT_SECRET",
        "RAW_ASSISTANT_OUTPUT_SECRET",
        "RAW_TOOL_PAYLOAD_SECRET",
        "RAW_MEMORY_TEXT_SECRET",
        "RAW_LIFEMODEL_CONTENT_SECRET",
        "RAW_BUILDER_ANSWER_SECRET",
        "RAW_BUILDER_PROPOSAL_PAYLOAD_SECRET",
        "prompt-token",
        "assistant-output",
        "memory-raw-content",
        "raw builder answer",
        "raw calibration answer",
        "raw calibration payload",
        "raw evolution payload",
        "raw LifeModel content",
        "raw proposal payload",
        "W82_RAW_CALIBRATION_TARGET_SECRET",
        "W82_RAW_CALIBRATION_REASON_SECRET",
        "W82_RAW_EVOLUTION_TARGET_SECRET",
        "W82_RAW_EVOLUTION_REASON_SECRET",
        "W83_RAW_FEEDBACK_TEXT_SECRET",
        "W83_RAW_CONVERSATION_INFERENCE_SECRET",
        "W83_RAW_EXISTING_EVOLUTION_RULE_SECRET",
    ] {
        assert!(
            !debug_dump.contains(forbidden),
            "legacy convergence report leaked raw marker {forbidden}"
        );
    }
}

#[test]
fn legacy_write_convergence_guard_fails_closed_for_bad_inventory() {
    fn assert_guard_reason(
        mutate: impl FnOnce(&mut Vec<LegacyWriteInventoryEntry>),
        expected_reason: &str,
    ) {
        let mut entries = legacy_write_convergence_inventory();
        mutate(&mut entries);
        let report = evaluate_legacy_write_convergence_inventory(&entries);
        assert!(!report.inventory_ready);
        assert!(
            report
                .guard_blocking_reasons
                .iter()
                .any(|reason| reason.contains(expected_reason)),
            "missing guard reason {expected_reason}: {:?}",
            report.guard_blocking_reasons
        );
    }

    assert_guard_reason(
        |entries| entry_mut(entries, "manual_lifemodel_editor").normal_product_allowed = true,
        "high_risk_legacy_direct_write_marked_normal_product_allowed",
    );
    assert_guard_reason(
        |entries| {
            entry_mut(entries, "builder_legacy_direct_apply")
                .required_convergence_action
                .clear()
        },
        "legacy_direct_write_missing_required_convergence_action",
    );
    assert_guard_reason(
        |entries| entry_mut(entries, "data_import").source_file_paths.clear(),
        "direct_write_missing_source_file_or_function",
    );
    assert_guard_reason(
        |entries| entry_mut(entries, "builder_normal_proposal_flow").currently_direct_write = true,
        "proposal_first_target_marked_direct_unsafe_blocker",
    );
    assert_guard_reason(
        |entries| entry_mut(entries, "state_daily_goal_direct_writes").metadata_safe = false,
        "entry_metadata_not_safe",
    );
    assert_guard_reason(
        |entries| {
            entry_mut(entries, "raw_chat_memory_vector_source_writes").contains_raw_content = true
        },
        "entry_contains_raw_content",
    );
    assert_guard_reason(
        |entries| entry_mut(entries, "manual_lifemodel_editor").default_chat_affected = true,
        "default_chat_marked_affected",
    );
    assert_guard_reason(
        |entries| {
            entry_mut(entries, "raw_chat_memory_vector_source_writes")
                .command_function_names
                .push("evaluate_low_energy_rule_trace_visibility".into())
        },
        "w73_w78_maturation_helper_listed_as_ordinary_chat_write_path",
    );
    assert_guard_reason(
        |entries| {
            entry_mut(entries, "external_write_proposal_path").provider_execution_enabled = true
        },
        "proposal_tool_marked_real_provider_write_executor",
    );
}

#[test]
fn legacy_write_convergence_default_chat_and_ordinary_entrypoints_remain_unchanged() {
    let entries = legacy_write_convergence_inventory();
    let report = evaluate_legacy_write_convergence_inventory(&entries);
    assert!(report.default_chat_unchanged);
    assert_eq!(report.default_chat_route, "legacy_stream");
    assert!(!report.w79_guard_called_by_ordinary_chat);

    let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(lib_rs_path).expect("read src/lib.rs");
    let send_body = extract_rust_function_body(&source, "async fn send_message(");
    let stream_body = extract_rust_function_body(&source, "async fn start_stream_message(");
    for forbidden in [
        "legacy_write_convergence_inventory",
        "evaluate_legacy_write_convergence_inventory",
        "ensure_legacy_write_convergence_inventory_guard",
        "LegacyWriteConvergenceReport",
        "ManualLifeModelOverrideAuditReport",
        "evaluate_manual_lifemodel_override_audit",
        "record_manual_lifemodel_override_audit_with_state",
        "builder_apply_signals",
        "builder_apply_signals_with_state",
        "builder_apply_signals_with_state_for_dev_migration",
        "BuilderLegacyDirectApplyOverride",
        "apply_calibration_with_state_for_dev_migration",
        "run_micro_evolution_with_state_for_dev_migration",
        "CalibrationLegacyDirectApplyDevMigrationOverride",
        "apply_feedback_evolution_with_state_for_dev_migration",
        "FeedbackEvolutionLegacyDirectApplyOverride",
        "restore_snapshot_with_state_for_manual_restore",
        "SnapshotRestoreLegacyDirectApplyOverride",
        "import_all_data_with_state_for_dev_migration",
        "DataImportLegacyDirectApplyOverride",
        "StateSourceDataBoundaryReport",
        "evaluate_state_source_data_boundary",
        "ensure_state_source_data_boundary",
        "LifeModelMaterializerCallerMatrixReport",
        "lifemodel_materializer_caller_matrix",
        "evaluate_lifemodel_materializer_caller_matrix",
        "ensure_lifemodel_materializer_caller_matrix",
    ] {
        assert!(
            !send_body.contains(forbidden),
            "send_message must not call legacy convergence or W81 API {forbidden}"
        );
        assert!(
            !stream_body.contains(forbidden),
            "start_stream_message must not call legacy convergence or W81 API {forbidden}"
        );
    }
}

#[test]
fn legacy_write_convergence_external_propose_tools_are_proposal_only_not_provider_executors() {
    let entries = legacy_write_convergence_inventory();
    let report = evaluate_legacy_write_convergence_inventory(&entries);
    let external = entry(&entries, "external_write_proposal_path");

    assert_eq!(
        external.current_status,
        LegacyWriteConvergenceStatus::ProposalOnlyDeclarative
    );
    assert!(has_function(external, "calendar.propose_event"));
    assert!(has_function(external, "email.propose_draft"));
    assert!(!external.provider_execution_enabled);
    assert!(!report.calendar_propose_event_provider_executor_enabled);
    assert!(!report.email_propose_draft_provider_executor_enabled);
    assert!(!report.external_provider_execution_enabled);
}

fn entry_mut<'a>(
    entries: &'a mut [LegacyWriteInventoryEntry],
    stable_id: &str,
) -> &'a mut LegacyWriteInventoryEntry {
    entries
        .iter_mut()
        .find(|entry| entry.stable_id == stable_id)
        .unwrap_or_else(|| panic!("missing inventory entry {stable_id}"))
}

fn extract_rust_function_body(source: &str, signature: &str) -> String {
    let signature_start = source.find(signature).expect("function signature exists");
    let brace_start = source[signature_start..]
        .find('{')
        .map(|index| signature_start + index)
        .expect("function body starts");
    let mut depth = 0usize;

    for (offset, ch) in source[brace_start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    let end = brace_start + offset + ch.len_utf8();
                    return source[brace_start..end].to_string();
                }
            }
            _ => {}
        }
    }

    panic!("function body closes");
}

fn count_occurrences(source: &str, needle: &str) -> usize {
    source.match_indices(needle).count()
}
