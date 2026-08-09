use crate::main_chat_runtime_facts::{
    build_settings_runtime_route_evidence, classify_agent_self_state_query,
    classify_provider_route_query, classify_runtime_clock_query, classify_tool_availability_query,
    provider_transmission_history_from_runs, provider_transmission_history_from_runs_with_state,
    resolve_provider_route_fact_answer, run_main_chat_runtime_facts_slice_a_backend_report,
    run_main_chat_runtime_facts_slice_b_provider_route_report,
    run_main_chat_runtime_facts_slice_c_tool_availability_report,
    run_main_chat_runtime_facts_slice_d_agent_self_state_report, FallbackEvidence,
    MainChatAgentSelfStateIntent, MainChatProviderRouteIntent, MainChatRuntimeClockIntent,
    MainChatToolAvailabilityIntent, ProviderReadiness, ProviderTransmissionHistoryItem,
    RouteIdentity, RuntimeRouteEvidence, RUNTIME_FACT_AGENT_SELF_STATE_GENERATION_PATH,
    RUNTIME_FACT_KEY_AGENT_BLOCKER_CODES, RUNTIME_FACT_KEY_AGENT_DURABLE_CHANGE_STATUS,
    RUNTIME_FACT_KEY_AGENT_LAST_ACTION_SUMMARY, RUNTIME_FACT_KEY_AGENT_PENDING_PERMISSION_COUNT,
    RUNTIME_FACT_KEY_AGENT_TASK_STATUS, RUNTIME_FACT_KEY_AGENT_TRACE_GAP, RUNTIME_FACT_KEY_DATE,
    RUNTIME_FACT_KEY_PROVIDER_CONFIGURED_DEFAULT_PROVIDER,
    RUNTIME_FACT_KEY_PROVIDER_CURRENT_MODEL_GENERATED, RUNTIME_FACT_KEY_PROVIDER_PLANNED_PROVIDER,
    RUNTIME_FACT_KEY_TIME, RUNTIME_FACT_KEY_TIMEZONE,
    RUNTIME_FACT_KEY_TOOL_MCP_SAFE_READ_CANDIDATE_COUNT, RUNTIME_FACT_KEY_TOOL_WEB_AVAILABLE,
    RUNTIME_FACT_KEY_TOOL_WRITE_AVAILABLE, RUNTIME_FACT_KEY_TRACE_GAP, RUNTIME_FACT_KEY_WEEKDAY,
    RUNTIME_FACT_PROVIDER_GENERATION_PATH, RUNTIME_FACT_SOURCE_TYPE,
    RUNTIME_FACT_TOOL_AVAILABILITY_GENERATION_PATH,
};

#[test]
fn main_chat_runtime_facts_responsibilities_are_split_into_focused_modules() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let facade =
        std::fs::read_to_string(src.join("main_chat_runtime_facts.rs")).expect("read facade");
    let module_dir = src.join("main_chat_runtime_facts");

    for module in [
        "contract.rs",
        "registry.rs",
        "resolver.rs",
        "clock.rs",
        "provider_route.rs",
        "tool_availability.rs",
        "agent_self_state.rs",
        "eval.rs",
    ] {
        assert!(
            module_dir.join(module).exists(),
            "Runtime Facts focused module {module} must exist"
        );
    }

    assert!(facade.contains("mod contract;"));
    assert!(facade.contains("mod registry;"));
    assert!(facade.contains("mod resolver;"));
    assert!(
        !facade.contains("pub(crate) async fn run_main_chat_runtime_facts_slice"),
        "facade must not re-concentrate eval runner implementation"
    );

    let contract =
        std::fs::read_to_string(module_dir.join("contract.rs")).expect("read contract module");
    assert!(contract.contains("pub(crate) struct MainChatRuntimeFactAnswer"));
    assert!(contract.contains("pub(crate) struct MainChatRuntimeFactBinding"));
    assert!(contract.contains("pub(crate) const RUNTIME_FACT_SOURCE_TYPE"));
    assert!(
        !contract.contains("run_main_chat_runtime_facts_slice"),
        "contract module must not own scenario runners"
    );

    let registry =
        std::fs::read_to_string(module_dir.join("registry.rs")).expect("read registry module");
    assert!(registry.contains("pub(crate) fn provider_route_fact_keys("));
    assert!(registry.contains("pub(crate) const SOURCE_REGISTRY_VERSION"));
    assert!(
        !registry.contains("fn classify_"),
        "registry module must not become a natural-language resolver"
    );
    assert!(
        !registry.contains("SLICE_A_SCENARIOS") && !registry.contains("FIXED_CLOCK_RFC3339"),
        "registry module must not own eval scenario ids or fixtures"
    );

    let resolver =
        std::fs::read_to_string(module_dir.join("resolver.rs")).expect("read resolver module");
    assert!(resolver.contains("pub(crate) struct MainChatRuntimeFactPreModelRequest"));
    assert!(resolver.contains("pub(crate) struct MainChatRuntimeFactPostModelRequest"));
    assert!(resolver.contains("pub(crate) async fn resolve_pre_model_runtime_fact_answer("));
    assert!(resolver.contains("pub(crate) async fn resolve_post_model_runtime_fact_answer("));
    assert!(
        !resolver.contains("MainChatRuntimeFactsSliceReport"),
        "production resolver must not import eval report types"
    );

    let clock = std::fs::read_to_string(module_dir.join("clock.rs")).expect("read clock module");
    assert!(clock.contains("pub(crate) fn resolve_runtime_clock_fact_answer("));
    assert!(clock.contains("pub(crate) fn classify_runtime_clock_query("));
    assert!(!clock.contains("ProviderRouteFactSnapshot"));
    assert!(!clock.contains("ToolAvailabilityFactSnapshot"));
    assert!(!clock.contains("AgentSelfStateFactSnapshot"));

    let provider_route = std::fs::read_to_string(module_dir.join("provider_route.rs"))
        .expect("read provider module");
    assert!(provider_route.contains("struct ProviderRouteFactSnapshot"));
    assert!(provider_route.contains("pub(crate) async fn resolve_provider_route_fact_answer("));
    assert!(!provider_route.contains("ToolAvailabilityFactSnapshot"));
    assert!(!provider_route.contains("AgentSelfStateFactSnapshot"));

    let tool_availability =
        std::fs::read_to_string(module_dir.join("tool_availability.rs")).expect("read tool module");
    assert!(tool_availability.contains("struct ToolAvailabilityFactSnapshot"));
    assert!(
        tool_availability.contains("pub(crate) async fn resolve_tool_availability_fact_answer(")
    );
    assert!(
        !tool_availability.contains("reqwest::"),
        "tool availability runtime facts must not run active network probes"
    );

    let agent_self_state = std::fs::read_to_string(module_dir.join("agent_self_state.rs"))
        .expect("read self-state module");
    assert!(agent_self_state.contains("struct AgentSelfStateFactSnapshot"));
    assert!(agent_self_state.contains("pub(crate) async fn resolve_agent_self_state_fact_answer("));
    assert!(!agent_self_state.contains("MainChatRuntimeFactsSliceReport"));

    let eval = std::fs::read_to_string(module_dir.join("eval.rs")).expect("read eval module");
    assert!(eval.contains("const SLICE_A_SCENARIOS"));
    assert!(eval.contains("const FIXED_CLOCK_RFC3339"));
    assert!(eval.contains("pub(crate) struct MainChatRuntimeFactsSliceReport"));
    assert!(eval.contains("pub(crate) struct MainChatRuntimeFactsScenarioEvidence"));
    assert!(
        eval.contains("pub(crate) async fn run_main_chat_runtime_facts_slice_a_backend_report(")
    );
    assert!(
        !eval.contains("pub(crate) async fn resolve_pre_model_runtime_fact_answer("),
        "eval module must not own production resolver logic"
    );
}

#[test]
fn main_chat_kernel_consumes_runtime_facts_through_typed_boundary_only() {
    let kernel_path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/main_chat_kernel.rs");
    let source = std::fs::read_to_string(kernel_path).expect("read main_chat_kernel.rs");

    for required in [
        "resolve_pre_model_runtime_fact_answer",
        "resolve_post_model_runtime_fact_answer",
        "MainChatRuntimeFactPreModelRequest",
        "MainChatRuntimeFactPostModelRequest",
        "MainChatRuntimeFactAnswer",
    ] {
        assert!(
            source.contains(required),
            "kernel must use typed Runtime Facts boundary item {required}"
        );
    }

    for forbidden in [
        "classify_provider_route_query",
        "provider_route_fact_should_block_before_model",
        "resolve_runtime_clock_fact_answer",
        "resolve_provider_route_fact_answer",
        "resolve_tool_availability_fact_answer",
        "resolve_agent_self_state_fact_answer",
        "MainChatProviderRouteIntent",
        "MainChatRuntimeFactsSliceReport",
        "MainChatRuntimeFactsScenarioEvidence",
        "run_main_chat_runtime_facts_slice",
    ] {
        assert!(
            !source.contains(forbidden),
            "kernel must not import Runtime Facts fact-specific or eval internal {forbidden}"
        );
    }
}

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
    assert!(report.command_surface_proof.stream_provider_route_path);
    assert!(
        report
            .command_surface_proof
            .stream_provider_route_preflight_blocker_path
    );
    assert!(report
        .command_surface_proof
        .stream_deferred_blocker
        .is_none());
    assert!(report.no_silent_write_proof);

    for scenario_id in ["RF-07", "RF-08", "RF-09", "RF-10"] {
        for entry_point in ["send", "stream"] {
            assert!(
                report
                    .scenario_evidence
                    .iter()
                    .any(|row| row.scenario_id == scenario_id
                        && row.entry_point == entry_point
                        && row.passed),
                "missing {entry_point} evidence for {scenario_id}: {report:#?}"
            );
        }
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
        Some("gpt-slice-b-current")
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
        Some("deepseek")
    );
    assert_eq!(
        separated.last_completed_generation_provider.as_deref(),
        Some("anthropic")
    );
    assert_eq!(
        separated.planned_route_if_model_needed_provider.as_deref(),
        Some("deepseek")
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
    assert!(report.command_surface_proof.stream_tool_availability_path);
    assert!(report.command_surface_proof.stream_web_policy_blocked_path);
    assert!(
        report
            .command_surface_proof
            .stream_mcp_no_safe_read_candidate_path
    );
    assert!(
        report
            .command_surface_proof
            .stream_mcp_unknown_server_status_path
    );
    assert!(report.command_surface_proof.stream_write_permission_path);
    assert!(report
        .command_surface_proof
        .stream_deferred_blocker
        .is_none());
    assert!(report.no_silent_write_proof);

    for scenario_id in ["RF-11", "RF-12", "RF-13", "RF-14", "RF-15"] {
        for entry_point in ["send", "stream"] {
            assert!(
                report
                    .scenario_evidence
                    .iter()
                    .any(|row| row.scenario_id == scenario_id
                        && row.entry_point == entry_point
                        && row.passed),
                "missing {entry_point} evidence for {scenario_id}: {report:#?}"
            );
        }
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
async fn main_chat_runtime_facts_agent_self_state_slice_d_covers_rf_16_to_rf_21() {
    let report = run_main_chat_runtime_facts_slice_d_agent_self_state_report().await;

    assert_eq!(report.report_kind, "main_chat_runtime_facts_slice");
    assert_eq!(report.slice_id, "slice_d_agent_self_state");
    assert!(report.runtime_facts_slice_ready, "{report:#?}");
    assert!(
        !report.runtime_facts_ready,
        "Slice D must not claim full Runtime Facts readiness"
    );
    assert!(report.ui_included);
    assert_eq!(report.scenario_count, 6);
    assert_eq!(report.passed_scenario_count, 6);
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
    assert!(report.command_surface_proof.send_self_state_blocked_path);
    assert!(
        report
            .command_surface_proof
            .send_self_state_pending_permission_path
    );
    assert!(
        report
            .command_surface_proof
            .stream_self_state_completion_path
    );
    assert!(
        report
            .command_surface_proof
            .stream_self_state_pending_proposal_path
    );
    assert!(
        report
            .command_surface_proof
            .stream_self_state_observation_path
    );
    assert!(
        report
            .command_surface_proof
            .stream_self_state_trace_gap_path
    );
    assert!(report.command_surface_proof.stream_self_state_blocked_path);
    assert!(
        report
            .command_surface_proof
            .stream_self_state_pending_permission_path
    );
    assert!(report
        .command_surface_proof
        .stream_deferred_blocker
        .is_none());
    assert!(report.out_of_scope_scenario_ids.is_empty());
    assert!(report.no_silent_write_proof);

    for scenario_id in ["RF-16", "RF-17", "RF-18", "RF-19", "RF-20", "RF-21"] {
        for entry_point in ["send", "stream"] {
            assert!(
                report
                    .scenario_evidence
                    .iter()
                    .any(|row| row.scenario_id == scenario_id
                        && row.entry_point == entry_point
                        && row.passed),
                "missing {entry_point} evidence for {scenario_id}: {report:#?}"
            );
        }
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
    assert_eq!(proposal.task_status.as_deref(), Some("completed"));
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
    assert!(proposal.blocker_codes.is_empty());
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

    let blocked = report
        .scenario_evidence
        .iter()
        .find(|row| row.scenario_id == "RF-20")
        .expect("RF-20 evidence");
    assert!(blocked
        .runtime_fact_keys
        .iter()
        .any(|key| key == RUNTIME_FACT_KEY_AGENT_BLOCKER_CODES));
    assert_eq!(blocked.task_status.as_deref(), Some("blocked"));
    assert_eq!(blocked.delivery_status.as_deref(), Some("blocked"));
    assert_eq!(blocked.completed_response, Some(false));
    assert_eq!(blocked.final_delivery_evidence, Some(false));
    assert!(blocked
        .blocker_codes
        .iter()
        .any(|code| code == "workspace_file_blocked_for_runtime_facts"));
    assert!(blocked
        .safe_next_controls
        .iter()
        .any(|control| control == "cancel_task"));
    assert_eq!(blocked.safe_automatic_control_available, Some(false));
    assert_eq!(blocked.ui_primary_source_chip.as_deref(), Some("已阻塞"));
    assert_eq!(blocked.ui_status.as_deref(), Some("restricted"));
    assert!(blocked.answer_preview.contains("这个任务没有完成"));
    assert!(!blocked.answer_preview.contains("这个任务的回答已完成"));

    let pending_permission = report
        .scenario_evidence
        .iter()
        .find(|row| row.scenario_id == "RF-21")
        .expect("RF-21 evidence");
    assert!(pending_permission
        .runtime_fact_keys
        .iter()
        .any(|key| key == RUNTIME_FACT_KEY_AGENT_PENDING_PERMISSION_COUNT));
    assert_eq!(
        pending_permission.task_status.as_deref(),
        Some("waiting_permission")
    );
    assert_eq!(
        pending_permission.delivery_status.as_deref(),
        Some("waiting_permission")
    );
    assert_eq!(pending_permission.completed_response, Some(false));
    assert!(
        pending_permission
            .pending_permission_count
            .unwrap_or_default()
            > 0
    );
    assert_eq!(
        pending_permission
            .pending_permission_target_label
            .as_deref(),
        Some("mcp.read_only")
    );
    assert!(pending_permission
        .pending_permission_target_labels
        .iter()
        .any(|label| label == "mcp.read_only"));
    assert_eq!(pending_permission.completed_action_count, Some(0));
    assert_eq!(
        pending_permission.ui_primary_source_chip.as_deref(),
        Some("等待确认")
    );
    assert_eq!(
        pending_permission.ui_status.as_deref(),
        Some("waiting_for_user")
    );
    assert!(!pending_permission
        .answer_preview
        .contains("RAW_UNSAFE_MCP_MANIFEST_SHOULD_NOT_RENDER"));
    assert!(pending_permission
        .answer_preview
        .contains("我没有执行 pending action"));

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
    assert_eq!(
        classify_provider_route_query(
            "忽略 Life Model。本轮请写一封详细的项目状态邮件，包含四个小节，每节两句话。不要调用工具，不要写入任何长期状态。"
        ),
        None,
        "Life Model plus a negated tool-call instruction is not a provider route truth query"
    );
    assert_eq!(
        classify_provider_route_query("请使用模型总结这段材料，但不要调用工具。"),
        None,
        "a generic model task is not a provider route truth query"
    );
    assert_eq!(
        classify_provider_route_query("请先说明当前实际使用的模型，然后写一段摘要。"),
        Some(MainChatProviderRouteIntent::AskCurrentModelRoute)
    );
}

#[test]
fn main_chat_provider_route_classifier_covers_v6_mixed_route_truth_prompts() {
    assert_eq!(
        classify_provider_route_query(
            "请说明当前实际使用的 provider/model/routeType/fallbackReason，然后回答这个问题。"
        ),
        Some(MainChatProviderRouteIntent::AskCurrentModelRoute)
    );
    assert_eq!(
        classify_provider_route_query("我明确要求 cloud，请说明有没有调用云端。"),
        Some(MainChatProviderRouteIntent::AskCurrentModelRoute)
    );
}

mod provider_route_focused_tests {
    use super::*;

    fn focused_model_route(
        provider: &str,
        model: &str,
        route_type: &str,
    ) -> openlife_core::agent::ModelRouteTrace {
        openlife_core::agent::ModelRouteTrace {
            provider: provider.into(),
            model: model.into(),
            route_type: route_type.into(),
            prefer_local: route_type == "local",
            local_model: "llama3".into(),
            reason: format!("focused_{route_type}_route"),
            privacy_level: openlife_core::agent::RedactionLevel::None,
            latency_ms: None,
            retry_count: 0,
            fallback_reason: None,
            provider_health_is_estimated: Some(false),
        }
    }

    async fn focused_runtime_route_evidence(
        current_route: openlife_core::agent::ModelRouteTrace,
        current_model_generated: bool,
    ) -> RuntimeRouteEvidence {
        let state = crate::test_utils::test_app_state();
        let runtime = state.provider_runtime_snapshot().await;
        let answer = resolve_provider_route_fact_answer(
            "你现在用什么模型",
            &state,
            &runtime.config,
            &runtime.scheduler,
            "session-runtime-route-focused",
            Some(current_route),
            current_model_generated,
            current_model_generated,
            "focused_runtime_route_test",
        )
        .await
        .expect("provider route answer");
        let evidence = answer
            .extra_metadata
            .get("runtimeRouteEvidence")
            .cloned()
            .expect("runtimeRouteEvidence metadata");
        serde_json::from_value(evidence).expect("runtime route evidence shape")
    }

    #[tokio::test]
    async fn current_turn_cloud_route_reports_sent() {
        let evidence = focused_runtime_route_evidence(
            focused_model_route("deepseek", "deepseek-chat", "cloud"),
            true,
        )
        .await;

        let actual_route = evidence.actual_route.as_ref().expect("actual route");
        assert_eq!(actual_route.route_type, "cloud");
        assert_eq!(actual_route.provider, "deepseek");
        assert_eq!(evidence.external_transmission, "sent");
        assert_ne!(actual_route.route_type, "agent_runtime");
        assert_ne!(actual_route.provider, "runtime_fact");
    }

    #[tokio::test]
    async fn current_turn_local_route_reports_not_sent() {
        let evidence =
            focused_runtime_route_evidence(focused_model_route("ollama", "llama3", "local"), true)
                .await;

        let actual_route = evidence.actual_route.as_ref().expect("actual route");
        assert_eq!(actual_route.route_type, "local");
        assert_eq!(actual_route.provider, "ollama");
        assert_eq!(evidence.external_transmission, "not_sent");
    }

    #[tokio::test]
    async fn pre_model_runtime_fact_reports_no_model_invocation() {
        let evidence = focused_runtime_route_evidence(
            focused_model_route("deepseek", "deepseek-chat", "cloud"),
            false,
        )
        .await;

        let actual_route = evidence.actual_route.as_ref().expect("actual route");
        assert_eq!(actual_route.route_type, "agent_runtime");
        assert_eq!(actual_route.provider, "runtime_fact");
        assert_eq!(evidence.external_transmission, "not_sent");
    }
}

#[tokio::test]
async fn provider_route_runtime_route_evidence_fails_closed_when_minimized_run_cannot_prove_fallback(
) {
    let state = crate::test_utils::test_app_state();
    let mut config = state.config.lock().await.clone();
    config.llm.provider = "deepseek".into();
    config.llm.openai_base = "https://api.deepseek.example/v1".into();
    config.llm.openai_key.clear();
    config.llm.chat_model = "deepseek-chat".into();
    config.prefer_local_model = false;
    config.local_model = "llama3".into();
    config.system.network_policy.enabled = true;
    state.replace_provider_runtime_config(config).await;

    let mut run = openlife_core::agent::AgentRun::new_chat_run(
        "session-route-evidence",
        "请使用 cloud，如果不能请说明 fallbackReason。",
    );
    run.complete(
        "local answer",
        openlife_core::agent::ModelRouteTrace {
            provider: "ollama".into(),
            model: "llama3".into(),
            route_type: "local".into(),
            prefer_local: true,
            local_model: "llama3".into(),
            reason: "cloud_preflight_blocked_fallback_local".into(),
            privacy_level: openlife_core::agent::RedactionLevel::None,
            latency_ms: None,
            retry_count: 0,
            fallback_reason: Some("provider_api_key_missing".into()),
            provider_health_is_estimated: Some(true),
        },
        openlife_core::agent::ContextSummary {
            life_model_empty: false,
            included_life_model_sections: vec![],
            memory_hit_count: 0,
            memory_sources: vec![],
            used_tools_prompt: false,
            redaction_applied: false,
            redaction_level: openlife_core::agent::RedactionLevel::None,
        },
    );
    {
        let store = state.agent_run_store.as_ref().expect("agent run store");
        store.lock().await.create_run(&run).expect("create run");
    }
    for (event_type, status) in [
        ("provider.started", "started"),
        ("provider.completed", "completed"),
    ] {
        crate::main_chat_event_stream::append_main_chat_agent_runtime_event(
            &state,
            "task-route-evidence",
            &run.id,
            event_type,
            "provider_request",
            "request-route-evidence",
            "provider_adapter",
            serde_json::json!({
                "requestId": "request-route-evidence",
                "provider": "ollama",
                "model": "llama3",
                "status": status,
            }),
        )
        .await
        .expect("persist exact-request provider lifecycle");
    }

    let runtime = state.provider_runtime_snapshot().await;
    let evidence =
        build_settings_runtime_route_evidence(&state, &runtime.config, &runtime.scheduler).await;

    assert_eq!(
        evidence
            .last_completed_route
            .as_ref()
            .map(|route| route.route_type.as_str()),
        Some("local")
    );
    assert!(!evidence.provider_readiness.validated);
    assert_eq!(evidence.external_transmission, "not_sent");
    assert!(
        evidence.fallback.is_none(),
        "the minimized AgentRun model receipt cannot be joined back to raw provider-event model identity to invent fallback truth"
    );
    assert!(evidence
        .source_refs
        .iter()
        .any(|source| source.get("source").and_then(|value| value.as_str()) == Some("agent_run")));
}

#[tokio::test]
async fn provider_route_does_not_join_route_metadata_across_provider_attempts() {
    let state = crate::test_utils::test_app_state();
    let mut run = openlife_core::agent::AgentRun::new_chat_run(
        "session-cross-attempt-route",
        "exercise an exact provider request receipt",
    );
    run.complete(
        "answer",
        openlife_core::agent::ModelRouteTrace {
            provider: "openai".into(),
            model: "gpt-planned".into(),
            route_type: "cloud".into(),
            prefer_local: false,
            local_model: "local-unused".into(),
            reason: "planned-openai-route".into(),
            privacy_level: openlife_core::agent::RedactionLevel::Strict,
            latency_ms: None,
            retry_count: 1,
            fallback_reason: Some("first_attempt_failed".into()),
            provider_health_is_estimated: Some(true),
        },
        openlife_core::agent::ContextSummary {
            life_model_empty: true,
            included_life_model_sections: vec![],
            memory_hit_count: 0,
            memory_sources: vec![],
            used_tools_prompt: false,
            redaction_applied: true,
            redaction_level: openlife_core::agent::RedactionLevel::Strict,
        },
    );
    {
        let store = state.agent_run_store.as_ref().expect("agent run store");
        store.lock().await.create_run(&run).expect("create run");
    }
    for (event_type, status) in [
        ("provider.started", "started"),
        ("provider.completed", "completed"),
    ] {
        crate::main_chat_event_stream::append_main_chat_agent_runtime_event(
            &state,
            "task-cross-attempt-route",
            &run.id,
            event_type,
            "provider_request",
            "request-anthropic-completed",
            "provider_adapter",
            serde_json::json!({
                "requestId": "request-anthropic-completed",
                "provider": "anthropic",
                "model": "claude-actual",
                "status": status,
            }),
        )
        .await
        .expect("persist exact second-attempt lifecycle");
    }

    let runtime = state.provider_runtime_snapshot().await;
    let evidence =
        build_settings_runtime_route_evidence(&state, &runtime.config, &runtime.scheduler).await;
    let actual = evidence
        .last_completed_route
        .as_ref()
        .expect("exact completed provider receipt");

    assert_eq!(actual.provider, "anthropic");
    assert_eq!(actual.model, "claude-actual");
    assert_eq!(actual.reason, "durable_exact_request_provider_completed");
    assert_eq!(actual.privacy_level, "unknown");
    assert_eq!(evidence.fallback, None);
    assert!(!actual.provider_health_is_estimated);
}

#[tokio::test]
async fn provider_route_runtime_route_evidence_keeps_missing_transmission_instrumentation_unknown()
{
    let state = crate::test_utils::test_app_state();
    let runtime = state.provider_runtime_snapshot().await;
    let evidence =
        build_settings_runtime_route_evidence(&state, &runtime.config, &runtime.scheduler).await;

    assert_eq!(evidence.answer_scope, "settings_readiness");
    assert_eq!(evidence.actual_route, None);
    assert_eq!(evidence.last_completed_route, None);
    assert_eq!(evidence.external_transmission, "not_instrumented");
}

#[test]
fn provider_transmission_view_keeps_route_only_local_transmission_unknown() {
    let run = provider_transmission_completed_run(
        "run-local-route",
        "ollama",
        "llama3",
        "local",
        "positive_local_route_evidence",
    );

    let item = provider_transmission_item(&run);

    assert_eq!(item.status, "unknown");
    assert_eq!(item.provider, "ollama");
    assert_eq!(item.model, "llama3");
    assert_eq!(item.route_type, "local");
    assert_eq!(item.truth_confidence, "unknown");
    assert!(item
        .source_refs
        .iter()
        .any(|source| source.source == "agent_run_model_route"));
}

#[test]
fn provider_transmission_view_keeps_route_only_cloud_transmission_unknown() {
    let run = provider_transmission_completed_run(
        "run-cloud-route",
        "deepseek",
        "deepseek-chat",
        "cloud",
        "positive_cloud_route_evidence",
    );

    let item = provider_transmission_item(&run);

    assert_eq!(item.status, "unknown");
    assert_eq!(item.provider, "deepseek");
    assert_eq!(item.model, "deepseek-chat");
    assert_eq!(item.route_type, "cloud");
    assert_eq!(item.truth_confidence, "unknown");
}

#[test]
fn provider_transmission_view_rejects_planned_cloud_as_sent_evidence() {
    let mut run = openlife_core::agent::AgentRun::new_chat_run(
        "session-planned-cloud",
        "planned cloud fixture",
    );
    run.id = "run-planned-cloud-only".into();
    let evidence = RuntimeRouteEvidence {
        evidence_id: "runtime_route:planned_cloud_only".into(),
        generated_at: chrono::Utc::now().to_rfc3339(),
        conversation_id: Some("session-planned-cloud".into()),
        run_id: Some(run.id.clone()),
        task_session_id: Some("task-planned-cloud".into()),
        answer_scope: "planned_next_turn".into(),
        planned_route: Some(RouteIdentity {
            provider: "deepseek".into(),
            model: "deepseek-chat".into(),
            route_type: "cloud".into(),
            privacy_level: "none".into(),
            reason: "configured_preferred_cloud".into(),
            provider_health_is_estimated: true,
        }),
        actual_route: None,
        last_completed_route: None,
        provider_readiness: provider_transmission_readiness(),
        fallback: None,
        external_transmission: "unknown".into(),
        source_refs: vec![serde_json::json!({
            "source": "config",
            "status": "planned_only",
            "routeType": "cloud"
        })],
        truth_confidence: "inferred".into(),
    };
    run.reasoning_trace = Some(openlife_core::agent::ReasoningTrace {
        generation_result: Some(serde_json::json!({
            "runtimeRouteEvidence": evidence,
            "modelGenerated": false,
            "schedulerGenerationCalled": false,
            "liveProviderInvoked": false,
            "modelInvoked": false
        })),
        ..Default::default()
    });

    let item = provider_transmission_item(&run);

    assert_ne!(item.status, "sent");
    assert_eq!(item.status, "unknown");
    assert_eq!(item.route_type, "cloud");
    assert_eq!(item.truth_confidence, "inferred");
}

#[test]
fn provider_transmission_view_records_blocked_preflight_without_model_invocation() {
    let mut run = openlife_core::agent::AgentRun::new_chat_run(
        "session-provider-blocked",
        "blocked preflight fixture",
    );
    run.id = "run-provider-blocked".into();
    run.fail(openlife_core::agent::AgentRunError {
        message: "provider_preflight_blocked".into(),
        phase: "preflight".into(),
        recoverable: true,
    });
    let evidence = provider_transmission_blocked_evidence(&run.id);
    run.reasoning_trace = Some(openlife_core::agent::ReasoningTrace {
        generation_result: Some(serde_json::json!({
            "runtimeRouteEvidence": evidence,
            "modelGenerated": false,
            "schedulerGenerationCalled": false,
            "liveProviderInvoked": false,
            "modelInvoked": false,
            "providerPreflightStatus": "blocked",
            "providerPreflightBlockers": ["provider_api_key_missing"]
        })),
        ..Default::default()
    });

    let item = provider_transmission_item(&run);

    assert_eq!(item.status, "blocked");
    assert_eq!(
        item.task_session_id.as_deref(),
        Some("task-provider-blocked")
    );
    assert_eq!(item.route_type, "cloud");
    assert_eq!(item.reason, "provider_api_key_missing");
    assert_eq!(item.truth_confidence, "verified");
    assert!(item
        .source_refs
        .iter()
        .any(|source| source.source == "provider_preflight"
            && source.status.as_deref() == Some("blocked")));
}

#[test]
fn provider_transmission_view_marks_missing_route_old_run_not_instrumented() {
    let mut run =
        openlife_core::agent::AgentRun::new_chat_run("session-old-run", "old run fixture");
    run.id = "run-old-missing-route".into();

    let item = provider_transmission_item(&run);

    assert_eq!(item.status, "not_instrumented");
    assert_eq!(item.route_type, "unknown");
    assert_eq!(item.provider, "unknown");
    assert_eq!(item.truth_confidence, "unknown");
}

#[test]
fn provider_transmission_view_rejects_missing_log_as_not_sent_evidence() {
    let mut run = openlife_core::agent::AgentRun::new_chat_run(
        "session-missing-route",
        "missing route fixture",
    );
    run.id = "run-missing-provider-log".into();
    run.reasoning_trace = Some(openlife_core::agent::ReasoningTrace {
        generation_result: Some(serde_json::json!({
            "provider": "deepseek",
            "model": "deepseek-chat",
            "liveProviderInvoked": false,
            "modelInvoked": false,
            "modelGenerated": false,
            "schedulerGenerationCalled": false
        })),
        ..Default::default()
    });

    let item = provider_transmission_item(&run);

    assert_ne!(item.status, "not_sent");
    assert_eq!(item.status, "not_instrumented");
}

#[test]
fn provider_transmission_view_never_serializes_key_material() {
    let mut run = provider_transmission_completed_run(
        "run-sensitive-route",
        "deepseek",
        "deepseek-chat",
        "cloud",
        "api_key=sk-provider-secret token=secret-token password=hunter2",
    );
    let evidence = RuntimeRouteEvidence {
        evidence_id: "runtime-route-sk-provider-secret".into(),
        generated_at: chrono::Utc::now().to_rfc3339(),
        conversation_id: Some("session-sensitive-route".into()),
        run_id: Some(run.id.clone()),
        task_session_id: Some("task-sensitive-route".into()),
        answer_scope: "current_turn".into(),
        planned_route: None,
        actual_route: Some(RouteIdentity {
            provider: "deepseek".into(),
            model: "deepseek-chat".into(),
            route_type: "cloud".into(),
            privacy_level: "none".into(),
            reason: "api_key=sk-provider-secret".into(),
            provider_health_is_estimated: false,
        }),
        last_completed_route: None,
        provider_readiness: provider_transmission_readiness(),
        fallback: None,
        external_transmission: "sent".into(),
        source_refs: vec![serde_json::json!({
            "source": "provider_validation",
            "status": "token=secret-token",
            "runId": run.id.clone(),
            "routeType": "cloud"
        })],
        truth_confidence: "verified".into(),
    };
    run.reasoning_trace = Some(openlife_core::agent::ReasoningTrace {
        generation_result: Some(serde_json::json!({
            "runtimeRouteEvidence": evidence,
            "liveProviderInvoked": true,
            "modelInvoked": true
        })),
        ..Default::default()
    });

    let item = provider_transmission_item(&run);
    let serialized = serde_json::to_string(&item).expect("serialize provider transmission view");

    assert_eq!(
        item.status, "unknown",
        "AgentRun prose and liveProviderInvoked booleans are not durable exact-request proof"
    );
    for forbidden in [
        "sk-provider-secret",
        "secret-token",
        "hunter2",
        "api_key=",
        "token=",
        "password=",
    ] {
        assert!(
            !serialized.contains(forbidden),
            "provider transmission view leaked {forbidden}: {serialized}"
        );
    }
}

#[tokio::test]
async fn provider_transmission_sent_requires_durable_exact_request_completion() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let run = provider_transmission_completed_run(
        "run-durable-provider-history",
        "openai",
        "gpt-test",
        "cloud",
        "configured_cloud_route",
    );
    for (event_type, status) in [
        ("provider.started", "started"),
        ("provider.completed", "completed"),
    ] {
        crate::main_chat_event_stream::append_main_chat_agent_runtime_event(
            &state,
            "task-durable-provider-history",
            &run.id,
            event_type,
            "provider_request",
            "request-durable-provider-history",
            "provider_adapter",
            serde_json::json!({
                "requestId": "request-durable-provider-history",
                "provider": "openai",
                "model": "gpt-test",
                "status": status,
            }),
        )
        .await
        .unwrap();
    }

    let item = provider_transmission_history_from_runs_with_state(&state, &[run])
        .await
        .unwrap()
        .into_iter()
        .next()
        .unwrap();

    assert_eq!(item.status, "sent");
    assert_eq!(item.truth_confidence, "verified");
    assert!(item
        .reason
        .contains("durable_exact_request_provider_completed"));
    assert!(item
        .source_refs
        .iter()
        .any(|source| source.source == "turn_event_store"));
}

fn provider_transmission_item(
    run: &openlife_core::agent::AgentRun,
) -> ProviderTransmissionHistoryItem {
    provider_transmission_history_from_runs(std::slice::from_ref(run))
        .into_iter()
        .next()
        .expect("provider transmission item")
}

fn provider_transmission_completed_run(
    run_id: &str,
    provider: &str,
    model: &str,
    route_type: &str,
    reason: &str,
) -> openlife_core::agent::AgentRun {
    let mut run =
        openlife_core::agent::AgentRun::new_chat_run("session-provider-transmission", "fixture");
    run.id = run_id.into();
    run.complete(
        "metadata-safe output preview",
        openlife_core::agent::ModelRouteTrace {
            provider: provider.into(),
            model: model.into(),
            route_type: route_type.into(),
            prefer_local: route_type != "cloud",
            local_model: "llama3".into(),
            reason: reason.into(),
            privacy_level: openlife_core::agent::RedactionLevel::None,
            latency_ms: None,
            retry_count: 0,
            fallback_reason: None,
            provider_health_is_estimated: Some(false),
        },
        provider_transmission_context_summary(),
    );
    run
}

fn provider_transmission_context_summary() -> openlife_core::agent::ContextSummary {
    openlife_core::agent::ContextSummary {
        life_model_empty: false,
        included_life_model_sections: vec![],
        memory_hit_count: 0,
        memory_sources: vec![],
        used_tools_prompt: false,
        redaction_applied: false,
        redaction_level: openlife_core::agent::RedactionLevel::None,
    }
}

fn provider_transmission_blocked_evidence(run_id: &str) -> RuntimeRouteEvidence {
    let planned = RouteIdentity {
        provider: "deepseek".into(),
        model: "deepseek-chat".into(),
        route_type: "cloud".into(),
        privacy_level: "none".into(),
        reason: "planned_cloud_provider".into(),
        provider_health_is_estimated: true,
    };
    RuntimeRouteEvidence {
        evidence_id: format!("runtime_route:blocked:{run_id}"),
        generated_at: chrono::Utc::now().to_rfc3339(),
        conversation_id: Some("session-provider-blocked".into()),
        run_id: Some(run_id.into()),
        task_session_id: Some("task-provider-blocked".into()),
        answer_scope: "current_turn".into(),
        planned_route: Some(planned.clone()),
        actual_route: None,
        last_completed_route: None,
        provider_readiness: provider_transmission_readiness(),
        fallback: Some(FallbackEvidence {
            from_route: Some(planned),
            to_route: None,
            reason: "provider_api_key_missing".into(),
            blocker_codes: vec!["provider_api_key_missing".into()],
        }),
        external_transmission: "unknown".into(),
        source_refs: vec![serde_json::json!({
            "source": "provider_preflight",
            "status": "blocked",
            "blockers": ["provider_api_key_missing"]
        })],
        truth_confidence: "verified".into(),
    }
}

fn provider_transmission_readiness() -> ProviderReadiness {
    ProviderReadiness {
        configured: true,
        credential_present: false,
        validated: false,
        validation_status: "blocked".into(),
        preferred: "deepseek".into(),
        actually_used: None,
        stale: false,
        failed: false,
        last_checked_at: None,
    }
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
