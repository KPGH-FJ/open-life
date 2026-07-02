use crate::legacy_write_convergence::{
    ensure_legacy_write_convergence_inventory_guard, ensure_lifemodel_materializer_caller_matrix,
    ensure_lifemodel_materializer_caller_restriction, ensure_state_source_data_boundary,
    evaluate_legacy_write_convergence_inventory, evaluate_lifemodel_materializer_caller_matrix,
    evaluate_lifemodel_materializer_caller_restriction, evaluate_state_source_data_boundary,
    legacy_write_convergence_inventory, lifemodel_materializer_caller_matrix,
    LegacyWriteConvergenceStatus, LegacyWriteInventoryEntry, LegacyWriteRiskClass,
    LegacyWriteSafeModeStatus, LifeModelMaterializerCallerContext,
    LifeModelMaterializerCallerGovernanceState, LifeModelMaterializerCallerKind,
    LifeModelMaterializerCallerMatrixEntry, LifeModelMaterializerCallerPurpose,
    LifeModelMaterializerCallerRestrictionReport, LifeModelMaterializerCallerRisk,
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

fn entry_mut<'a>(
    entries: &'a mut [LegacyWriteInventoryEntry],
    stable_id: &str,
) -> &'a mut LegacyWriteInventoryEntry {
    entries
        .iter_mut()
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

fn context_for_entry(
    entry: &LifeModelMaterializerCallerMatrixEntry,
) -> LifeModelMaterializerCallerContext {
    LifeModelMaterializerCallerContext::new(
        &entry.stable_id,
        entry.kind,
        LifeModelMaterializerCallerPurpose::from_governance_state(entry.governance_state)
            .expect("governance state maps to caller purpose"),
    )
}

fn extract_rust_function_body(source: &str, signature: &str) -> String {
    let start = source
        .find(signature)
        .or_else(|| {
            signature
                .strip_suffix('(')
                .and_then(|prefix| source.find(&format!("{prefix}<")))
        })
        .unwrap_or_else(|| panic!("missing function signature {signature}"));
    let body_start = source[start..]
        .find('{')
        .map(|offset| start + offset)
        .expect("missing body start");
    let mut depth = 0isize;
    let mut end = body_start;
    for (offset, ch) in source[body_start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end = body_start + offset + 1;
                    break;
                }
            }
            _ => {}
        }
    }
    source[body_start..end].to_string()
}

fn count_occurrences(haystack: &str, needle: &str) -> usize {
    haystack.match_indices(needle).count()
}

#[test]
fn legacy_write_convergence_w97_inventory_reports_final_convergence() {
    let entries = legacy_write_convergence_inventory();
    let report = evaluate_legacy_write_convergence_inventory(&entries);

    assert!(report.inventory_ready);
    assert!(report.guard_ready);
    assert!(report.overall_converged);
    assert!(report.all_direct_writes_converged);
    assert!(report.metadata_safe);
    assert!(!report.contains_raw_content);
    assert!(report.default_chat_unchanged);
    assert_eq!(report.default_chat_route, "main_chat_kernel");
    assert_eq!(report.high_risk_legacy_direct_write_count, 0);
    assert!(report.convergence_blockers.is_empty());
    assert!(report.guard_blocking_reasons.is_empty());

    ensure_legacy_write_convergence_inventory_guard()
        .expect("W97 final convergence inventory should pass");
}

#[test]
fn legacy_write_convergence_w97_inventory_classifies_all_final_write_categories() {
    let entries = legacy_write_convergence_inventory();

    let materializer = entry(&entries, "lifemodel_save_primitive");
    assert_eq!(
        materializer.current_status,
        LegacyWriteConvergenceStatus::GovernedCompatibilityMaterializer
    );
    assert_eq!(
        materializer.safe_mode_status,
        LegacyWriteSafeModeStatus::GovernedOperationRequired
    );
    assert!(materializer.currently_direct_write);
    assert!(!materializer.high_risk_durable_truth_write);

    let manual = entry(&entries, "manual_lifemodel_editor");
    assert_eq!(
        manual.risk_class,
        LegacyWriteRiskClass::GovernedManualOverride
    );
    assert_eq!(
        manual.current_status,
        LegacyWriteConvergenceStatus::GovernedManualOverride
    );
    assert!(manual.normal_product_allowed);
    assert!(!manual.requires_proposal_first);
    assert!(manual.currently_direct_write);
    assert!(!manual.high_risk_durable_truth_write);
    assert!(manual.current_guard_summary.contains("pre-change snapshot"));

    for stable_id in [
        "builder_legacy_direct_apply",
        "calibration_direct_micro_evolution",
        "feedback_evolution_direct_writes",
    ] {
        let retired = entry(&entries, stable_id);
        assert_eq!(
            retired.risk_class,
            LegacyWriteRiskClass::RetiredLegacyCompatibility
        );
        assert_eq!(
            retired.current_status,
            LegacyWriteConvergenceStatus::RetiredNoWriteCompatibility
        );
        assert_eq!(
            retired.safe_mode_status,
            LegacyWriteSafeModeStatus::RetiredNoWrite
        );
        assert!(!retired.currently_direct_write);
        assert!(!retired.high_risk_durable_truth_write);
        assert!(retired.blocking_reasons.is_empty());
    }

    for stable_id in ["snapshot_restore", "data_import"] {
        let governed = entry(&entries, stable_id);
        assert_eq!(
            governed.risk_class,
            LegacyWriteRiskClass::GovernedRestoreImportOperation
        );
        assert_eq!(
            governed.current_status,
            LegacyWriteConvergenceStatus::GovernedRestoreImportOperation
        );
        assert_eq!(
            governed.safe_mode_status,
            LegacyWriteSafeModeStatus::GovernedOperationRequired
        );
        assert!(governed.normal_product_allowed);
        assert!(!governed.requires_proposal_first);
        assert!(governed.currently_direct_write);
        assert!(!governed.high_risk_durable_truth_write);
        assert!(governed.current_guard_summary.contains("metadata-safe"));
        assert!(governed.blocking_reasons.is_empty());
    }

    let proposal = entry(&entries, "proposal_application_path");
    assert!(proposal.requires_proposal_first);
    assert!(proposal.high_risk_durable_truth_write);
    assert!(!proposal.currently_direct_write);
    assert!(proposal.current_guard_summary.contains("W95"));
    assert!(proposal
        .current_guard_summary
        .contains("no misleading fallback"));

    let state = entry(&entries, "state_daily_goal_direct_writes");
    assert_eq!(
        state.risk_class,
        LegacyWriteRiskClass::LowRiskTransientState
    );
    assert_eq!(
        state.current_status,
        LegacyWriteConvergenceStatus::LowRiskTransientSourceData
    );
    assert!(state.currently_direct_write);
    assert!(!state.high_risk_durable_truth_write);
    assert!(has_function(state, "persist_life_model"));
}

#[test]
fn legacy_write_convergence_w96_state_daily_goal_source_data_boundary_passes() {
    let entries = legacy_write_convergence_inventory();
    let report = evaluate_state_source_data_boundary(&entries);

    assert_eq!(
        report.state_daily_goal_path_ids,
        vec!["state_daily_goal_direct_writes".to_string()]
    );
    assert!(report.compatibility_lifemodel_materialized_write);
    assert!(report.writes_current_lifemodel_compatibility_view);
    assert!(!report.accepted_durable_hs_truth_write);
    assert!(!report.active_hs_lifemodel_patch);
    assert!(report.proposal_required_for_hs_truth_promotion);
    assert!(report.ordinary_chat_unchanged);
    assert!(report.default_chat_unchanged);
    assert!(report.blocking_reasons.is_empty());

    ensure_state_source_data_boundary().expect("W96 state/daily-goal boundary should pass");
}

#[test]
fn legacy_write_convergence_w97_materializer_matrix_has_no_legacy_blockers() {
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
        "snapshot_restore_governed_operation",
        "data_import_governed_operation",
    ];

    assert!(report.matrix_ready);
    assert!(report.metadata_safe);
    assert!(report.all_known_callers_classified);
    assert!(report.unclassified_callers.is_empty());
    assert_eq!(report.caller_count, required.len());
    assert_eq!(report.high_risk_legacy_blocker_count, 0);
    assert_eq!(report.proposal_first_count, 1);
    assert_eq!(report.manual_override_count, 1);
    assert_eq!(report.restore_import_override_count, 2);
    assert!(report.proposal_first_convergence_complete);
    assert!(report.blocking_reasons.is_empty());
    assert!(!report.migration_permission);
    assert!(!report.runtime_authority_granted);

    for stable_id in required {
        assert!(
            entries.iter().any(|entry| entry.stable_id == stable_id),
            "missing W97 materializer caller {stable_id}"
        );
    }

    for retired_id in [
        "builder_step_legacy_direct_apply",
        "builder_apply_signals_legacy_direct_apply",
        "calibration_micro_evolution_legacy_direct_apply",
        "calibration_direct_apply_legacy_direct_apply",
        "feedback_evolution_legacy_direct_apply",
        "snapshot_restore_legacy_direct_apply",
        "data_import_legacy_direct_apply",
    ] {
        assert!(
            entries.iter().all(|entry| entry.stable_id != retired_id),
            "{retired_id} must not remain a production materializer caller"
        );
    }

    ensure_lifemodel_materializer_caller_matrix()
        .expect("W97 materializer caller matrix should pass");
}

#[test]
fn legacy_write_convergence_w97_materializer_matrix_matches_current_production_callsite_count() {
    let entries = lifemodel_materializer_caller_matrix();
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let persist_call_files = [
        "src/lib.rs",
        "src/main_chat_turn_pipeline.rs",
        "src/main_chat_send.rs",
        "src/main_chat_streaming.rs",
        "src/main_chat_legacy_agent_loop.rs",
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
        "matrix must classify every current production persist_life_model caller"
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
        "matrix must classify every current production LifeModelManager::save caller"
    );
}

#[test]
fn legacy_write_convergence_w97_materializer_matrix_tracks_extracted_main_chat_callers() {
    let entries = lifemodel_materializer_caller_matrix();
    for (stable_id, source_file_path, caller_function_name) in [
        (
            "ordinary_chat_auto_checkin_source_data",
            "src-tauri/src/main_chat_turn_pipeline.rs",
            "run_main_chat_turn_pipeline_buffered",
        ),
        (
            "ordinary_stream_agent_loop_auto_checkin_source_data",
            "src-tauri/src/main_chat_legacy_agent_loop.rs",
            "start_stream_message_with_agent_loop",
        ),
        (
            "ordinary_stream_legacy_auto_checkin_source_data",
            "src-tauri/src/main_chat_turn_pipeline.rs",
            "run_main_chat_turn_pipeline_streaming",
        ),
    ] {
        let entry = materializer_entry(&entries, stable_id);
        assert_eq!(entry.source_file_path, source_file_path);
        assert_eq!(entry.caller_function_name, caller_function_name);
        assert_eq!(entry.write_entrypoint, "persist_life_model");
    }
}

#[test]
fn legacy_write_convergence_w97_materializer_contexts_allow_only_classified_callers() {
    let entries = lifemodel_materializer_caller_matrix();
    for stable_id in [
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
        "data_import_governed_operation",
    ] {
        let matrix_entry = materializer_entry(&entries, stable_id);
        assert_eq!(matrix_entry.write_entrypoint, "persist_life_model");
        let context = context_for_entry(matrix_entry);
        let report =
            ensure_lifemodel_materializer_caller_restriction(&context, "persist_life_model")
                .unwrap_or_else(|message| panic!("restriction blocked {stable_id}: {message}"));
        assert!(report.allowed);
        assert_eq!(report.stable_id, stable_id);
        assert!(report.matrix_entry_found);
        assert!(report.kind_matches_matrix);
        assert!(report.purpose_matches_matrix);
        assert!(report.metadata_safe);
        assert!(!report.high_risk_legacy_blocker);
        assert!(!report.migration_permission);
        assert!(!report.runtime_authority_granted);
        assert!(report.blocking_reasons.is_empty());
    }

    let restore_entry = materializer_entry(&entries, "snapshot_restore_governed_operation");
    let restore_context = context_for_entry(restore_entry);
    let restore_report = ensure_lifemodel_materializer_caller_restriction(
        &restore_context,
        "LifeModelManager::save",
    )
    .expect("governed restore direct save should be allowed");
    assert!(restore_report.allowed);
    assert!(restore_report.restore_import_override);

    let unclassified = LifeModelMaterializerCallerContext::new(
        "synthetic_unclassified_materializer_caller",
        LifeModelMaterializerCallerKind::Unclassified,
        LifeModelMaterializerCallerPurpose::Unclassified,
    );
    let report: LifeModelMaterializerCallerRestrictionReport =
        evaluate_lifemodel_materializer_caller_restriction(&unclassified, "persist_life_model");
    assert!(!report.allowed);
    assert!(!report.matrix_entry_found);
}

#[test]
fn legacy_write_convergence_w97_materializer_final_categories_are_explicit() {
    let entries = lifemodel_materializer_caller_matrix();

    let manual = materializer_entry(&entries, "manual_lifemodel_editor_save");
    assert_eq!(
        manual.kind,
        LifeModelMaterializerCallerKind::GovernedManualOverride
    );
    assert_eq!(
        manual.risk,
        LifeModelMaterializerCallerRisk::GovernedManualOverride
    );
    assert_eq!(
        manual.governance_state,
        LifeModelMaterializerCallerGovernanceState::GovernedManualOverride
    );
    assert!(manual.manual_override);
    assert!(!manual.high_risk_legacy_blocker);

    let proposal = materializer_entry(&entries, "proposal_apply_lifemodel_update");
    assert_eq!(
        proposal.kind,
        LifeModelMaterializerCallerKind::AcceptedProposalApply
    );
    assert_eq!(
        proposal.governance_state,
        LifeModelMaterializerCallerGovernanceState::AcceptedProposalApplySourceSpecificPatchMappingComplete
    );
    assert!(proposal.proposal_first);
    assert!(proposal.proposal_first_convergence_complete);
    assert!(proposal.accepted_durable_lifemodel_hs_truth);

    for stable_id in [
        "snapshot_restore_governed_operation",
        "data_import_governed_operation",
    ] {
        let governed = materializer_entry(&entries, stable_id);
        assert_eq!(
            governed.kind,
            LifeModelMaterializerCallerKind::GovernedRestoreImportOperation
        );
        assert_eq!(
            governed.risk,
            LifeModelMaterializerCallerRisk::GovernedRestoreImportOperation
        );
        assert_eq!(
            governed.governance_state,
            LifeModelMaterializerCallerGovernanceState::GovernedRestoreImportOperation
        );
        assert!(governed.restore_import_override);
        assert!(!governed.high_risk_legacy_blocker);
        assert!(!governed.proposal_first_convergence_complete);
        assert!(!governed.accepted_durable_lifemodel_hs_truth);
    }
}

#[test]
fn legacy_write_convergence_w97_reports_are_metadata_safe_and_raw_content_free() {
    let entries = legacy_write_convergence_inventory();
    let report = evaluate_legacy_write_convergence_inventory(&entries);
    let matrix_entries = lifemodel_materializer_caller_matrix();
    let matrix_report = evaluate_lifemodel_materializer_caller_matrix(&matrix_entries);

    assert!(report.metadata_safe);
    assert!(!report.contains_raw_content);
    assert!(matrix_report.metadata_safe);
    assert!(!matrix_report.contains_raw_lifemodel_payload);
    assert!(!matrix_report.contains_raw_memory_text);
    assert!(!matrix_report.contains_raw_chat_text);
    assert!(!matrix_report.contains_raw_daily_goal_text);

    let debug_dump = format!("{report:?} {entries:?} {matrix_report:?} {matrix_entries:?}");
    for forbidden in [
        "RAW_PROMPT_SECRET",
        "RAW_ASSISTANT_OUTPUT_SECRET",
        "RAW_TOOL_PAYLOAD_SECRET",
        "RAW_MEMORY_TEXT_SECRET",
        "RAW_LIFEMODEL_CONTENT_SECRET",
        "RAW_BUILDER_ANSWER_SECRET",
        "RAW_CALIBRATION_REASON_SECRET",
        "RAW_FEEDBACK_TEXT_SECRET",
        "RAW_IMPORT_PAYLOAD_SECRET",
        "raw LifeModel content",
        "raw proposal payload",
        "life model raw yaml",
    ] {
        assert!(
            !debug_dump.contains(forbidden),
            "W97 convergence reports leaked raw marker {forbidden}"
        );
    }
}

#[test]
fn legacy_write_convergence_w97_guard_fails_closed_for_bad_inventory() {
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
        |entries| entry_mut(entries, "manual_lifemodel_editor").metadata_safe = false,
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
        |entries| entry_mut(entries, "builder_normal_proposal_flow").currently_direct_write = true,
        "proposal_first_target_marked_direct_unsafe_blocker",
    );
    assert_guard_reason(
        |entries| {
            entry_mut(entries, "external_write_proposal_path").provider_execution_enabled = true
        },
        "proposal_tool_marked_real_provider_write_executor",
    );
}

#[test]
fn legacy_write_convergence_w97_default_chat_and_ordinary_entrypoints_remain_isolated() {
    let entries = legacy_write_convergence_inventory();
    let report = evaluate_legacy_write_convergence_inventory(&entries);
    assert!(report.default_chat_unchanged);
    assert_eq!(report.default_chat_route, "main_chat_kernel");
    assert!(!report.w79_guard_called_by_ordinary_chat);

    let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(lib_rs_path).expect("read src/lib.rs");
    let send_body = extract_rust_function_body(&source, "async fn send_message(");
    let stream_body = extract_rust_function_body(&source, "async fn start_stream_message(");
    for forbidden in [
        "legacy_write_convergence_inventory",
        "evaluate_legacy_write_convergence_inventory",
        "ensure_legacy_write_convergence_inventory_guard",
        "evaluate_manual_lifemodel_override_audit",
        "record_manual_lifemodel_override_audit_with_state",
        "builder_apply_signals",
        "run_micro_evolution",
        "apply_feedback_evolution",
        "restore_snapshot",
        "import_all_data",
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
            "send_message must not call W79-W97 surface {forbidden}"
        );
        assert!(
            !stream_body.contains(forbidden),
            "start_stream_message must not call W79-W97 surface {forbidden}"
        );
    }
}

#[test]
fn legacy_write_convergence_w97_retired_override_symbols_are_absent_from_production_code() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let production_files = [
        "src/commands/builder.rs",
        "src/commands/calibration.rs",
        "src/commands/feedback.rs",
        "src/commands/version.rs",
        "src/commands/settings.rs",
    ];
    let forbidden = [
        concat!("with_state_for_", "dev_migration"),
        concat!("direct_apply_after_", "gate"),
        concat!("BuilderLegacy", "DirectApplyOverride"),
        concat!("CalibrationLegacy", "DirectApplyDevMigrationOverride"),
        concat!("FeedbackEvolutionLegacy", "DirectApplyOverride"),
        concat!("SnapshotRestoreLegacy", "DirectApplyOverride"),
        concat!("DataImportLegacy", "DirectApplyOverride"),
        concat!("manual_restore_", "override"),
        concat!("dev_migration_", "override"),
    ];

    for path in production_files {
        let source = std::fs::read_to_string(format!("{manifest_dir}/{path}")).expect("read file");
        for marker in forbidden {
            assert!(
                !source.contains(marker),
                "{path} still contains retired legacy override marker {marker}"
            );
        }
    }
}
