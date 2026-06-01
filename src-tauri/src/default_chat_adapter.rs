pub(crate) const LEGACY_STREAM_PATH: &str = "legacy_stream";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DefaultChatAdapterRoute {
    pub(crate) current_mode: String,
    pub(crate) adapter_scaffold_present: bool,
    pub(crate) controlled_adapter_enabled: bool,
    pub(crate) automatic_migration_enabled: bool,
    pub(crate) default_send_path: String,
    pub(crate) start_stream_path: String,
    pub(crate) requires_separate_cutover_implementation: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DefaultChatAdapterCutoverHarness {
    pub(crate) caller: String,
    pub(crate) harness_ready: bool,
    pub(crate) route_guard_passed: bool,
    pub(crate) invocation_mode: String,
    pub(crate) controlled_adapter_invocation_allowed: bool,
    pub(crate) runtime_call_enabled: bool,
    pub(crate) model_call_enabled: bool,
    pub(crate) tool_call_enabled: bool,
    pub(crate) allow_writes: bool,
    pub(crate) max_tool_calls: u32,
    pub(crate) chat_message_saved: bool,
    pub(crate) agent_run_recorded: bool,
    pub(crate) evidence_recorded: bool,
    pub(crate) default_chat_path_unchanged: bool,
    pub(crate) default_send_path: String,
    pub(crate) start_stream_path: String,
    pub(crate) requires_separate_cutover_implementation: bool,
    pub(crate) blocking_reasons: Vec<String>,
}

pub(crate) fn resolve_default_chat_adapter_route() -> DefaultChatAdapterRoute {
    DefaultChatAdapterRoute {
        current_mode: LEGACY_STREAM_PATH.into(),
        adapter_scaffold_present: true,
        controlled_adapter_enabled: false,
        automatic_migration_enabled: false,
        default_send_path: LEGACY_STREAM_PATH.into(),
        start_stream_path: LEGACY_STREAM_PATH.into(),
        requires_separate_cutover_implementation: true,
    }
}

fn default_chat_legacy_route_blockers(route: &DefaultChatAdapterRoute) -> Vec<String> {
    let mut blockers = Vec::new();

    if !route.adapter_scaffold_present {
        blockers.push("adapter_scaffold_missing".into());
    }
    if route.current_mode != LEGACY_STREAM_PATH {
        blockers.push("current_mode_not_legacy_stream".into());
    }
    if route.controlled_adapter_enabled {
        blockers.push("controlled_adapter_enabled".into());
    }
    if route.automatic_migration_enabled {
        blockers.push("automatic_migration_enabled".into());
    }
    if route.default_send_path != LEGACY_STREAM_PATH {
        blockers.push("default_send_path_not_legacy_stream".into());
    }
    if route.start_stream_path != LEGACY_STREAM_PATH {
        blockers.push("start_stream_path_not_legacy_stream".into());
    }
    if !route.requires_separate_cutover_implementation {
        blockers.push("separate_cutover_implementation_not_required".into());
    }

    blockers
}

pub(crate) fn evaluate_default_chat_adapter_cutover_harness(
    caller: &str,
    route: &DefaultChatAdapterRoute,
) -> DefaultChatAdapterCutoverHarness {
    let blocking_reasons = default_chat_legacy_route_blockers(route);
    let route_guard_passed = blocking_reasons.is_empty();
    let default_chat_path_unchanged = route.current_mode == LEGACY_STREAM_PATH
        && route.default_send_path == LEGACY_STREAM_PATH
        && route.start_stream_path == LEGACY_STREAM_PATH
        && !route.controlled_adapter_enabled
        && !route.automatic_migration_enabled;

    DefaultChatAdapterCutoverHarness {
        caller: caller.into(),
        harness_ready: route_guard_passed,
        route_guard_passed,
        invocation_mode: if route_guard_passed {
            "legacy_guarded"
        } else {
            "blocked"
        }
        .into(),
        controlled_adapter_invocation_allowed: false,
        runtime_call_enabled: false,
        model_call_enabled: false,
        tool_call_enabled: false,
        allow_writes: false,
        max_tool_calls: 0,
        chat_message_saved: false,
        agent_run_recorded: false,
        evidence_recorded: false,
        default_chat_path_unchanged,
        default_send_path: route.default_send_path.clone(),
        start_stream_path: route.start_stream_path.clone(),
        requires_separate_cutover_implementation: route.requires_separate_cutover_implementation,
        blocking_reasons,
    }
}

pub(crate) fn ensure_default_chat_cutover_harness(
    caller: &str,
    route: &DefaultChatAdapterRoute,
) -> Result<(), String> {
    let harness = evaluate_default_chat_adapter_cutover_harness(caller, route);

    if harness.harness_ready {
        Ok(())
    } else {
        Err(format!(
            "{caller} blocked by default Chat adapter cutover harness: {}",
            harness.blocking_reasons.join(", ")
        ))
    }
}
