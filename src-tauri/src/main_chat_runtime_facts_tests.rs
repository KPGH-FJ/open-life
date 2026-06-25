use crate::main_chat_runtime_facts::{
    classify_agent_self_state_query, classify_provider_route_query, classify_runtime_clock_query,
    classify_tool_availability_query, run_main_chat_runtime_facts_slice_a_backend_report,
    run_main_chat_runtime_facts_slice_b_provider_route_report,
    run_main_chat_runtime_facts_slice_c_tool_availability_report,
    run_main_chat_runtime_facts_slice_d_agent_self_state_report, MainChatAgentSelfStateIntent,
    MainChatProviderRouteIntent, MainChatRuntimeClockIntent, MainChatToolAvailabilityIntent,
    RUNTIME_FACT_AGENT_SELF_STATE_GENERATION_PATH, RUNTIME_FACT_KEY_AGENT_DURABLE_CHANGE_STATUS,
    RUNTIME_FACT_KEY_AGENT_LAST_ACTION_SUMMARY, RUNTIME_FACT_KEY_AGENT_TASK_STATUS,
    RUNTIME_FACT_KEY_AGENT_TRACE_GAP, RUNTIME_FACT_KEY_DATE,
    RUNTIME_FACT_KEY_PROVIDER_CONFIGURED_DEFAULT_PROVIDER,
    RUNTIME_FACT_KEY_PROVIDER_CURRENT_MODEL_GENERATED, RUNTIME_FACT_KEY_PROVIDER_PLANNED_PROVIDER,
    RUNTIME_FACT_KEY_TIME, RUNTIME_FACT_KEY_TIMEZONE,
    RUNTIME_FACT_KEY_TOOL_MCP_SAFE_READ_CANDIDATE_COUNT, RUNTIME_FACT_KEY_TOOL_WEB_AVAILABLE,
    RUNTIME_FACT_KEY_TOOL_WRITE_AVAILABLE, RUNTIME_FACT_KEY_TRACE_GAP, RUNTIME_FACT_KEY_WEEKDAY,
    RUNTIME_FACT_PROVIDER_GENERATION_PATH, RUNTIME_FACT_SOURCE_TYPE,
    RUNTIME_FACT_TOOL_AVAILABILITY_GENERATION_PATH,
};

#[tokio::test]
async fn main_chat_runtime_facts_runtime_clock_slice_a_backend_report_covers_rf_01_to_rf_06() {
    let report = run_main_chat_runtime_facts_slice_a_backend_report().await;

    assert_eq!(report.report_kind, "main_chat_runtime_facts_slice");
    assert_eq!(report.slice_id, "slice_a_backend");
    assert!(report.runtime_facts_slice_ready, "{report:#?}");
    assert!(
        !report.runtime_facts_ready,
        "Slice A backend must not claim full Runtime Facts readiness"
    );
    assert!(!report.ui_included);
    assert!(report
        .out_of_scope_scenario_ids
        .iter()
        .any(|id| id == "RF-22"));
    assert_eq!(report.scenario_count, 6);
    assert_eq!(report.passed_scenario_count, 6);
    assert!(report.blockers.is_empty(), "{:?}", report.blockers);
    assert!(report.command_surface_proof.send_runtime_clock_path);
    assert!(report.command_surface_proof.stream_runtime_clock_path);
    assert!(report.no_silent_write_proof);

    for scenario_id in ["RF-01", "RF-02", "RF-03", "RF-04", "RF-05", "RF-06"] {
        let row = report
            .scenario_evidence
            .iter()
            .find(|row| row.scenario_id == scenario_id)
            .unwrap_or_else(|| panic!("missing scenario evidence {scenario_id}"));
        assert!(row.passed, "{row:#?}");
        assert_eq!(row.source_type.as_deref(), Some(RUNTIME_FACT_SOURCE_TYPE));
        assert_eq!(
            row.provider_generation_path.as_deref(),
            Some(RUNTIME_FACT_PROVIDER_GENERATION_PATH)
        );
        assert_eq!(row.model_generated, Some(false));
        assert_eq!(row.scheduler_generation_called, Some(false));
        assert_eq!(row.tool_called, Some(false));
        assert_eq!(row.direct_writes_executed, Some(false));
        assert!(!row.legacy_fallback_used);
        assert!(row
            .runtime_fact_source
            .iter()
            .any(|source| source == "local_clock"));
        assert!(row.runtime_fact_binding_count > 0);
        assert_eq!(row.runtime_fact_authority.as_deref(), Some("runtime"));
    }

    let weekday = report
        .scenario_evidence
        .iter()
        .find(|row| row.scenario_id == "RF-01")
        .expect("RF-01 evidence");
    assert!(weekday
        .runtime_fact_keys
        .iter()
        .any(|key| key == RUNTIME_FACT_KEY_DATE));
    assert!(weekday
        .runtime_fact_keys
        .iter()
        .any(|key| key == RUNTIME_FACT_KEY_WEEKDAY));
    assert!(weekday
        .runtime_fact_keys
        .iter()
        .any(|key| key == RUNTIME_FACT_KEY_TIMEZONE));

    let time = report
        .scenario_evidence
        .iter()
        .find(|row| row.scenario_id == "RF-03")
        .expect("RF-03 evidence");
    assert!(time
        .runtime_fact_keys
        .iter()
        .any(|key| key == RUNTIME_FACT_KEY_TIME));
    assert!(time.answer_preview.contains("09:15"));

    let context_conflict = report
        .scenario_evidence
        .iter()
        .find(|row| row.scenario_id == "RF-05")
        .expect("RF-05 evidence");
    assert!(context_conflict.context_conflict_ignored);
    assert!(context_conflict.answer_preview.contains("2026-06-23"));
    assert!(!context_conflict.answer_preview.contains("1999-01-01"));

    let unavailable = report
        .scenario_evidence
        .iter()
        .find(|row| row.scenario_id == "RF-06")
        .expect("RF-06 evidence");
    assert!(unavailable.trace_gap);
    assert_eq!(
        unavailable.runtime_fact_freshness.as_deref(),
        Some("unknown")
    );
    assert!(unavailable
        .runtime_fact_keys
        .iter()
        .any(|key| key == RUNTIME_FACT_KEY_TRACE_GAP));
    assert!(unavailable.answer_preview.contains("当前时间未知"));

    assert!(
        report
            .negative_assertion_summary
            .planning_question_not_captured
            == Some(true)
    );
    assert!(
        report
            .negative_assertion_summary
            .no_provider_call_for_runtime_facts
            == Some(true)
    );
    assert!(
        report
            .negative_assertion_summary
            .no_tool_call_for_runtime_facts
            == Some(true)
    );
    assert!(
        report
            .negative_assertion_summary
            .no_direct_write_for_runtime_facts
            == Some(true)
    );
    assert!(
        report
            .negative_assertion_summary
            .no_legacy_fallback_for_runtime_facts
            == Some(true)
    );
    assert!(
        report
            .negative_assertion_summary
            .context_cannot_override_runtime_clock
            == Some(true)
    );
    assert!(
        report
            .negative_assertion_summary
            .missing_clock_does_not_use_model
            == Some(true)
    );
    assert_eq!(
        report
            .negative_assertion_summary
            .current_route_requires_current_generation_evidence,
        None
    );
}

#[tokio::test]
async fn main_chat_runtime_facts_provider_route_slice_b_covers_rf_07_to_rf_10() {
    let report = run_main_chat_runtime_facts_slice_b_provider_route_report().await;

    assert_eq!(report.report_kind, "main_chat_runtime_facts_slice");
    assert_eq!(report.slice_id, "slice_b_provider_route_semantics");
    assert!(report.runtime_facts_slice_ready, "{report:#?}");
    assert!(
        !report.runtime_facts_ready,
        "Slice B must not claim full Runtime Facts readiness"
    );
    assert!(report.ui_included);
    assert_eq!(report.scenario_count, 4);
    assert_eq!(report.passed_scenario_count, 4);
    assert!(report.blockers.is_empty(), "{:?}", report.blockers);
    assert!(report.command_surface_proof.send_provider_route_path);
    assert!(
        report
            .command_surface_proof
            .send_provider_route_preflight_blocker_path
    );
    assert!(report.no_silent_write_proof);

    for scenario_id in ["RF-07", "RF-08", "RF-09", "RF-10"] {
        let row = report
            .scenario_evidence
            .iter()
            .find(|row| row.scenario_id == scenario_id)
            .unwrap_or_else(|| panic!("missing scenario evidence {scenario_id}"));
        assert!(row.passed, "{row:#?}");
        assert_eq!(row.source_type.as_deref(), Some(RUNTIME_FACT_SOURCE_TYPE));
        assert_eq!(row.tool_called, Some(false));
        assert_eq!(row.direct_writes_executed, Some(false));
        assert!(!row.legacy_fallback_used);
        assert!(row
            .runtime_fact_keys
            .iter()
            .any(|key| key == RUNTIME_FACT_KEY_PROVIDER_CURRENT_MODEL_GENERATED));
        assert!(row
            .runtime_fact_keys
            .iter()
            .any(|key| key == RUNTIME_FACT_KEY_PROVIDER_CONFIGURED_DEFAULT_PROVIDER));
        assert!(row
            .runtime_fact_keys
            .iter()
            .any(|key| key == RUNTIME_FACT_KEY_PROVIDER_PLANNED_PROVIDER));
        assert!(row
            .runtime_fact_source
            .iter()
            .any(|source| source == "provider_route"));
        assert!(row
            .route_labels
            .iter()
            .any(|label| label.starts_with("current_turn_generation:")));
        assert!(row
            .route_labels
            .iter()
            .any(|label| label.starts_with("configured_default_route:")));
        assert!(row
            .route_labels
            .iter()
            .any(|label| label.starts_with("planned_route_if_model_needed:")));
        assert_eq!(row.ui_primary_source_chip.as_deref(), Some("运行时路线"));
    }

    let current = report
        .scenario_evidence
        .iter()
        .find(|row| row.scenario_id == "RF-07")
        .expect("RF-07 evidence");
    assert_eq!(current.model_generated, Some(true));
    assert_eq!(current.scheduler_generation_called, Some(true));
    assert_eq!(
        current.current_turn_generation_provider.as_deref(),
        Some("openai")
    );
    assert_eq!(
        current.current_turn_generation_model.as_deref(),
        Some("gpt-slice-b-current")
    );
    assert_eq!(
        current.current_turn_generation_route_type.as_deref(),
        Some("cloud")
    );
    assert_eq!(
        current.configured_model.as_deref(),
        Some("gpt-configured-default")
    );

    let after_clock = report
        .scenario_evidence
        .iter()
        .find(|row| row.scenario_id == "RF-08")
        .expect("RF-08 evidence");
    assert_eq!(after_clock.model_generated, Some(false));
    assert_eq!(after_clock.scheduler_generation_called, Some(false));
    assert!(after_clock.current_turn_generation_provider.is_none());
    assert!(after_clock.current_turn_generation_model.is_none());
    assert_eq!(
        after_clock.current_turn_generation_route_type.as_deref(),
        Some("none")
    );
    assert!(after_clock.answer_preview.contains("没有调用模型"));

    let separated = report
        .scenario_evidence
        .iter()
        .find(|row| row.scenario_id == "RF-09")
        .expect("RF-09 evidence");
    assert_eq!(separated.configured_provider.as_deref(), Some("deepseek"));
    assert_eq!(
        separated.current_turn_generation_provider.as_deref(),
        Some("openai")
    );
    assert_eq!(
        separated.last_completed_generation_provider.as_deref(),
        Some("anthropic")
    );
    assert_eq!(
        separated.planned_route_if_model_needed_provider.as_deref(),
        Some("openai")
    );

    let blocked = report
        .scenario_evidence
        .iter()
        .find(|row| row.scenario_id == "RF-10")
        .expect("RF-10 evidence");
    assert_eq!(blocked.model_generated, Some(false));
    assert_eq!(
        blocked.provider_preflight_status.as_deref(),
        Some("blocked")
    );
    assert!(!blocked.provider_preflight_blockers.is_empty());
    assert_eq!(blocked.ui_status.as_deref(), Some("restricted"));
    assert!(blocked.current_turn_generation_provider.is_none());
    assert!(!blocked
        .answer_preview
        .contains("provider.preflight.status=ready"));

    assert!(
        report
            .negative_assertion_summary
            .current_route_requires_current_generation_evidence
            == Some(true)
    );
    assert!(
        report
            .negative_assertion_summary
            .no_current_route_for_model_generated_false
            == Some(true)
    );
    assert!(
        report
            .negative_assertion_summary
            .configured_route_not_invocation_proof
            == Some(true)
    );
    assert!(
        report
            .negative_assertion_summary
            .planned_route_not_invocation_proof
            == Some(true)
    );
    assert!(
        report
            .negative_assertion_summary
            .last_completed_route_not_current_turn
            == Some(true)
    );
    assert!(
        report
            .negative_assertion_summary
            .provider_preflight_blocker_not_fake_readiness
            == Some(true)
    );
    assert_eq!(
        report
            .negative_assertion_summary
            .context_cannot_override_runtime_clock,
        None
    );
}

#[tokio::test]
async fn main_chat_runtime_facts_tool_availability_slice_c_covers_rf_11_to_rf_15() {
    let report = run_main_chat_runtime_facts_slice_c_tool_availability_report().await;

    assert_eq!(report.report_kind, "main_chat_runtime_facts_slice");
    assert_eq!(report.slice_id, "slice_c_tool_mcp_availability");
    assert!(report.runtime_facts_slice_ready, "{report:#?}");
    assert!(
        !report.runtime_facts_ready,
        "Slice C must not claim full Runtime Facts readiness"
    );
    assert!(report.ui_included);
    assert_eq!(report.scenario_count, 5);
    assert_eq!(report.passed_scenario_count, 5);
    assert!(report.blockers.is_empty(), "{:?}", report.blockers);
    assert!(report.command_surface_proof.send_tool_availability_path);
    assert!(report.command_surface_proof.send_web_policy_blocked_path);
    assert!(
        report
            .command_surface_proof
            .send_mcp_no_safe_read_candidate_path
    );
    assert!(
        report
            .command_surface_proof
            .send_mcp_unknown_server_status_path
    );
    assert!(report.command_surface_proof.send_write_permission_path);
    assert!(report.no_silent_write_proof);

    for scenario_id in ["RF-11", "RF-12", "RF-13", "RF-14", "RF-15"] {
        let row = report
            .scenario_evidence
            .iter()
            .find(|row| row.scenario_id == scenario_id)
            .unwrap_or_else(|| panic!("missing scenario evidence {scenario_id}"));
        assert!(row.passed, "{row:#?}");
        assert_eq!(row.source_type.as_deref(), Some(RUNTIME_FACT_SOURCE_TYPE));
        assert_eq!(
            row.provider_generation_path.as_deref(),
            Some(RUNTIME_FACT_TOOL_AVAILABILITY_GENERATION_PATH)
        );
        assert_eq!(row.model_generated, Some(false));
        assert_eq!(row.scheduler_generation_called, Some(false));
        assert_eq!(row.tool_called, Some(false));
        assert_eq!(row.direct_writes_executed, Some(false));
        assert!(!row.legacy_fallback_used);
        assert_eq!(row.tool_web_active_reachability_probe, Some(false));
        assert_eq!(row.tool_mcp_raw_manifest_exposed, Some(false));
        assert_eq!(row.tool_write_silent_write_available, Some(false));
        assert_eq!(row.ui_primary_source_chip.as_deref(), Some("工具可用性"));
        assert!(row
            .runtime_fact_source
            .iter()
            .any(|source| source == "tool_policy"));
    }

    let web_unknown = report
        .scenario_evidence
        .iter()
        .find(|row| row.scenario_id == "RF-11")
        .expect("RF-11 evidence");
    assert!(web_unknown
        .runtime_fact_keys
        .iter()
        .any(|key| key == RUNTIME_FACT_KEY_TOOL_WEB_AVAILABLE));
    assert_eq!(web_unknown.tool_web_config_enabled, Some(true));
    assert_eq!(web_unknown.tool_web_policy_allowed, Some(true));
    assert_eq!(
        web_unknown.tool_web_credential_status.as_deref(),
        Some("not_required")
    );
    assert_eq!(
        web_unknown.tool_web_reachability_status.as_deref(),
        Some("unknown")
    );
    assert_eq!(
        web_unknown.tool_web_reachability_ttl_status.as_deref(),
        Some("not_observed")
    );
    assert_eq!(
        web_unknown.tool_web_cached_or_preflight_known_reachability,
        Some(false)
    );
    assert_eq!(web_unknown.tool_web_available.as_deref(), Some("unknown"));
    assert!(web_unknown.answer_preview.contains("不会主动探测网络"));

    let web_blocked = report
        .scenario_evidence
        .iter()
        .find(|row| row.scenario_id == "RF-12")
        .expect("RF-12 evidence");
    assert_eq!(web_blocked.tool_web_config_enabled, Some(true));
    assert_eq!(web_blocked.tool_web_policy_allowed, Some(false));
    assert!(web_blocked
        .tool_web_policy_blockers
        .contains(&"network_policy_disabled".to_string()));
    assert_eq!(web_blocked.tool_web_available.as_deref(), Some("blocked"));
    assert_eq!(web_blocked.ui_status.as_deref(), Some("restricted"));
    assert!(!web_blocked.answer_preview.contains("已联网"));

    let no_safe_mcp = report
        .scenario_evidence
        .iter()
        .find(|row| row.scenario_id == "RF-13")
        .expect("RF-13 evidence");
    assert!(no_safe_mcp
        .runtime_fact_keys
        .iter()
        .any(|key| key == RUNTIME_FACT_KEY_TOOL_MCP_SAFE_READ_CANDIDATE_COUNT));
    assert!(no_safe_mcp.tool_mcp_registered_count.unwrap_or_default() > 0);
    assert_eq!(no_safe_mcp.tool_mcp_safe_read_candidate_count, Some(0));
    assert_eq!(
        no_safe_mcp.tool_mcp_available.as_deref(),
        Some("no_safe_read_candidate")
    );
    assert!(!no_safe_mcp
        .answer_preview
        .contains("raw_rf13_hidden_write_manifest"));
    assert!(!no_safe_mcp
        .answer_preview
        .contains("RAW_MCP_DESCRIPTION_SHOULD_NOT_RENDER"));

    let unknown_mcp = report
        .scenario_evidence
        .iter()
        .find(|row| row.scenario_id == "RF-14")
        .expect("RF-14 evidence");
    assert!(
        unknown_mcp
            .tool_mcp_safe_read_candidate_count
            .unwrap_or_default()
            > 0
    );
    assert_eq!(
        unknown_mcp.tool_mcp_server_status.as_deref(),
        Some("unknown")
    );
    assert_eq!(
        unknown_mcp.tool_mcp_available.as_deref(),
        Some("unknown_server_status")
    );
    assert!(unknown_mcp.answer_preview.contains("不能标为 available"));
    assert!(!unknown_mcp
        .answer_preview
        .contains("safe_rf14_read_manifest"));
    assert!(!unknown_mcp
        .answer_preview
        .contains("SAFE_DESCRIPTION_SHOULD_NOT_RENDER"));

    let write = report
        .scenario_evidence
        .iter()
        .find(|row| row.scenario_id == "RF-15")
        .expect("RF-15 evidence");
    assert!(write
        .runtime_fact_keys
        .iter()
        .any(|key| key == RUNTIME_FACT_KEY_TOOL_WRITE_AVAILABLE));
    assert_eq!(
        write.tool_write_available.as_deref(),
        Some("proposal_permission_or_blocker")
    );
    assert_eq!(write.tool_write_requires_permission, Some(true));
    assert_eq!(write.ui_status.as_deref(), Some("waiting_for_user"));
    assert!(write
        .answer_preview
        .contains("proposal / permission / blocker"));

    assert_eq!(
        report
            .negative_assertion_summary
            .no_active_reachability_probe_for_tool_availability,
        Some(true)
    );
    assert_eq!(
        report
            .negative_assertion_summary
            .web_policy_blocker_not_fake_availability,
        Some(true)
    );
    assert_eq!(
        report
            .negative_assertion_summary
            .mcp_registry_not_availability_without_safe_read,
        Some(true)
    );
    assert_eq!(
        report
            .negative_assertion_summary
            .mcp_unknown_server_status_not_available,
        Some(true)
    );
    assert_eq!(
        report
            .negative_assertion_summary
            .write_capability_requires_permission,
        Some(true)
    );
    assert_eq!(
        report
            .negative_assertion_summary
            .no_raw_mcp_manifest_exposure,
        Some(true)
    );
}

#[tokio::test]
async fn main_chat_runtime_facts_agent_self_state_slice_d_covers_rf_16_to_rf_19() {
    let report = run_main_chat_runtime_facts_slice_d_agent_self_state_report().await;

    assert_eq!(report.report_kind, "main_chat_runtime_facts_slice");
    assert_eq!(report.slice_id, "slice_d_agent_self_state");
    assert!(report.runtime_facts_slice_ready, "{report:#?}");
    assert!(
        !report.runtime_facts_ready,
        "Slice D must not claim full Runtime Facts readiness"
    );
    assert!(report.ui_included);
    assert_eq!(report.scenario_count, 4);
    assert_eq!(report.passed_scenario_count, 4);
    assert!(report.blockers.is_empty(), "{:?}", report.blockers);
    assert!(report.command_surface_proof.send_self_state_completion_path);
    assert!(
        report
            .command_surface_proof
            .send_self_state_pending_proposal_path
    );
    assert!(
        report
            .command_surface_proof
            .send_self_state_observation_path
    );
    assert!(report.command_surface_proof.send_self_state_trace_gap_path);
    assert!(report
        .out_of_scope_scenario_ids
        .iter()
        .any(|id| id == "RF-20"));
    assert!(report
        .out_of_scope_scenario_ids
        .iter()
        .any(|id| id == "RF-21"));
    assert!(report.no_silent_write_proof);

    for scenario_id in ["RF-16", "RF-17", "RF-18", "RF-19"] {
        let row = report
            .scenario_evidence
            .iter()
            .find(|row| row.scenario_id == scenario_id)
            .unwrap_or_else(|| panic!("missing scenario evidence {scenario_id}"));
        assert!(row.passed, "{row:#?}");
        assert_eq!(row.source_type.as_deref(), Some(RUNTIME_FACT_SOURCE_TYPE));
        assert_eq!(
            row.provider_generation_path.as_deref(),
            Some(RUNTIME_FACT_AGENT_SELF_STATE_GENERATION_PATH)
        );
        assert_eq!(row.model_generated, Some(false));
        assert_eq!(row.scheduler_generation_called, Some(false));
        assert_eq!(row.tool_called, Some(false));
        assert_eq!(row.direct_writes_executed, Some(false));
        assert!(!row.legacy_fallback_used);
        assert_eq!(row.assistant_prose_used_for_task_status, Some(false));
        assert_eq!(row.memory_or_hs_override_allowed, Some(false));
        assert!(row
            .runtime_fact_source
            .iter()
            .any(|source| source == "task_session"));
        assert!(row.runtime_fact_binding_count > 0);
    }

    let completed = report
        .scenario_evidence
        .iter()
        .find(|row| row.scenario_id == "RF-16")
        .expect("RF-16 evidence");
    assert!(completed
        .runtime_fact_keys
        .iter()
        .any(|key| key == RUNTIME_FACT_KEY_AGENT_TASK_STATUS));
    assert!(completed.task_session_id.is_some());
    assert!(completed.run_id.is_some());
    assert_eq!(completed.task_status.as_deref(), Some("completed"));
    assert_eq!(completed.run_status.as_deref(), Some("completed"));
    assert_eq!(completed.delivery_status.as_deref(), Some("delivered"));
    assert_eq!(completed.completed_response, Some(true));
    assert_eq!(completed.final_delivery_evidence, Some(true));
    assert_eq!(completed.pending_proposal_count, Some(0));
    assert!(!completed
        .answer_preview
        .contains("DIRECT_PROSE_SHOULD_NOT_BE_STATUS"));

    let proposal = report
        .scenario_evidence
        .iter()
        .find(|row| row.scenario_id == "RF-17")
        .expect("RF-17 evidence");
    assert!(proposal
        .runtime_fact_keys
        .iter()
        .any(|key| key == RUNTIME_FACT_KEY_AGENT_DURABLE_CHANGE_STATUS));
    assert_eq!(proposal.task_status.as_deref(), Some("waiting_permission"));
    assert_eq!(proposal.run_status.as_deref(), Some("completed"));
    assert_eq!(
        proposal.delivery_status.as_deref(),
        Some("response_delivered_pending_review")
    );
    assert_eq!(proposal.completed_response, Some(true));
    assert!(proposal.pending_proposal_count.unwrap_or_default() > 0);
    assert_eq!(
        proposal.durable_change_status.as_deref(),
        Some("pending_review")
    );
    assert_eq!(proposal.durable_change_completed, Some(false));
    assert!(proposal
        .blocker_codes
        .iter()
        .any(|code| code == "proposal_pending"));
    assert_eq!(proposal.ui_primary_source_chip.as_deref(), Some("提案待审"));
    assert_eq!(proposal.ui_status.as_deref(), Some("waiting_for_user"));
    assert!(proposal.answer_preview.contains("待审变更"));

    let observation = report
        .scenario_evidence
        .iter()
        .find(|row| row.scenario_id == "RF-18")
        .expect("RF-18 evidence");
    assert!(observation
        .runtime_fact_keys
        .iter()
        .any(|key| key == RUNTIME_FACT_KEY_AGENT_LAST_ACTION_SUMMARY));
    assert_eq!(observation.task_status.as_deref(), Some("completed"));
    assert!(observation.action_count.unwrap_or_default() > 0);
    assert!(observation.observation_count.unwrap_or_default() > 0);
    assert!(observation.transcript_observation_count.unwrap_or_default() > 0);
    assert_eq!(observation.last_action_type.as_deref(), Some("file.read"));
    assert_eq!(observation.last_action_status.as_deref(), Some("completed"));
    assert!(observation
        .last_action_summary
        .as_deref()
        .is_some_and(|summary| summary.contains("action=file.read")));
    assert_eq!(
        observation.ui_primary_source_chip.as_deref(),
        Some("工具观察")
    );

    let trace_gap = report
        .scenario_evidence
        .iter()
        .find(|row| row.scenario_id == "RF-19")
        .expect("RF-19 evidence");
    assert!(trace_gap.trace_gap);
    assert!(trace_gap.task_session_id.is_none());
    assert!(trace_gap.run_id.is_none());
    assert!(trace_gap
        .runtime_fact_keys
        .iter()
        .any(|key| key == RUNTIME_FACT_KEY_AGENT_TRACE_GAP));
    assert_eq!(trace_gap.delivery_status.as_deref(), Some("unknown"));
    assert_eq!(trace_gap.ui_status.as_deref(), Some("unknown"));
    assert!(trace_gap
        .answer_preview
        .contains("trace_gap=task_session_missing"));

    assert_eq!(
        report
            .negative_assertion_summary
            .no_assistant_prose_used_for_task_status,
        Some(true)
    );
    assert_eq!(
        report
            .negative_assertion_summary
            .context_cannot_override_task_runtime_state,
        Some(true)
    );
    assert_eq!(
        report
            .negative_assertion_summary
            .proposal_pending_not_completed_durable_change,
        Some(true)
    );
    assert_eq!(
        report
            .negative_assertion_summary
            .no_history_invention_without_trace,
        Some(true)
    );
}

#[test]
fn main_chat_runtime_clock_classifier_is_bounded_and_keeps_planning_question_out() {
    assert_eq!(
        classify_runtime_clock_query("今天星期几"),
        Some(MainChatRuntimeClockIntent::AskCurrentWeekday)
    );
    assert_eq!(
        classify_runtime_clock_query("今天几号"),
        Some(MainChatRuntimeClockIntent::AskCurrentDate)
    );
    assert_eq!(
        classify_runtime_clock_query("现在几点"),
        Some(MainChatRuntimeClockIntent::AskCurrentTime)
    );
    assert_eq!(
        classify_runtime_clock_query("what time is it"),
        Some(MainChatRuntimeClockIntent::AskCurrentTime)
    );
    assert_eq!(
        classify_runtime_clock_query("What time should I leave tomorrow?"),
        None
    );
    assert_eq!(classify_runtime_clock_query("我今天完成了写周报"), None);
}

#[test]
fn main_chat_agent_self_state_classifier_is_bounded() {
    assert_eq!(
        classify_agent_self_state_query("这个任务完成了吗"),
        Some(MainChatAgentSelfStateIntent::AskTaskCompletion)
    );
    assert_eq!(
        classify_agent_self_state_query("你刚刚做了什么"),
        Some(MainChatAgentSelfStateIntent::AskLastActionSummary)
    );
    assert_eq!(
        classify_agent_self_state_query("what did you just do"),
        Some(MainChatAgentSelfStateIntent::AskLastActionSummary)
    );
    assert_eq!(classify_agent_self_state_query("请帮我完成这个任务"), None);
    assert_eq!(
        classify_agent_self_state_query("what did i ask before"),
        None
    );
}

#[test]
fn main_chat_provider_route_classifier_is_bounded_and_separates_previous_turn() {
    assert_eq!(
        classify_provider_route_query("你现在用什么模型"),
        Some(MainChatProviderRouteIntent::AskCurrentModelRoute)
    );
    assert_eq!(
        classify_provider_route_query("what model are you using now"),
        Some(MainChatProviderRouteIntent::AskCurrentModelRoute)
    );
    assert_eq!(
        classify_provider_route_query("刚才回答今天星期几时用了什么模型"),
        Some(MainChatProviderRouteIntent::AskPreviousTurnModelRoute)
    );
    assert_eq!(classify_provider_route_query("我想比较几个模型"), None);
}

#[test]
fn main_chat_tool_availability_classifier_is_bounded_and_separates_capability_from_execution() {
    assert_eq!(
        classify_tool_availability_query("你能联网吗"),
        Some(MainChatToolAvailabilityIntent::AskToolAvailability)
    );
    assert_eq!(
        classify_tool_availability_query("can you use mcp"),
        Some(MainChatToolAvailabilityIntent::AskToolAvailability)
    );
    assert_eq!(
        classify_tool_availability_query("你有写入能力吗"),
        Some(MainChatToolAvailabilityIntent::AskWriteCapability)
    );
    assert_eq!(
        classify_tool_availability_query("Please web.search OpenLife news"),
        None
    );
    assert_eq!(
        classify_tool_availability_query("请读取网页 https://example.com"),
        None
    );
}
