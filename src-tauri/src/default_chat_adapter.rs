pub(crate) const LEGACY_STREAM_PATH: &str = "legacy_stream";
pub(crate) const CONTROLLED_ADAPTER_PATH: &str = "controlled_adapter";

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DefaultChatAdapterCallsite {
    SendMessage,
    StartStreamMessage,
}

impl DefaultChatAdapterCallsite {
    fn caller(self) -> &'static str {
        match self {
            Self::SendMessage => "send_message",
            Self::StartStreamMessage => "start_stream_message",
        }
    }

    fn contract_shape(self) -> &'static str {
        match self {
            Self::SendMessage => "send_message_compatible",
            Self::StartStreamMessage => "stream_message_compatible",
        }
    }

    fn route_path(self, route: &DefaultChatAdapterRoute) -> &str {
        match self {
            Self::SendMessage => &route.default_send_path,
            Self::StartStreamMessage => &route.start_stream_path,
        }
    }
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DefaultChatAdapterCallsiteContract {
    pub(crate) callsite: String,
    pub(crate) contract_ready: bool,
    pub(crate) boundary_ready: bool,
    pub(crate) contract_shape: String,
    pub(crate) selected_adapter_path: String,
    pub(crate) required_callsite_path: String,
    pub(crate) actual_callsite_path: String,
    pub(crate) controlled_adapter_executor_attached: bool,
    pub(crate) side_effect_free_before_legacy_entry: bool,
    pub(crate) blocking_reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DefaultChatAdapterOrdinaryEntryPreflight {
    pub(crate) callsite: String,
    pub(crate) preflight_ready: bool,
    pub(crate) contract_ready: bool,
    pub(crate) legacy_entry_allowed: bool,
    pub(crate) ordinary_entry_path: String,
    pub(crate) required_entry_path: String,
    pub(crate) contract_shape: String,
    pub(crate) side_effect_lock_engaged: bool,
    pub(crate) default_chat_migration_allowed: bool,
    pub(crate) controlled_adapter_executor_attached: bool,
    pub(crate) runtime_call_enabled: bool,
    pub(crate) model_call_enabled: bool,
    pub(crate) tool_call_enabled: bool,
    pub(crate) allow_writes: bool,
    pub(crate) max_tool_calls: u32,
    pub(crate) chat_message_saved: bool,
    pub(crate) agent_run_recorded: bool,
    pub(crate) evidence_recorded: bool,
    pub(crate) blocking_reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DefaultChatAdapterInvocationPlan {
    pub(crate) caller: String,
    pub(crate) plan_ready: bool,
    pub(crate) harness_ready: bool,
    pub(crate) selected_adapter_path: String,
    pub(crate) fallback_adapter_path: String,
    pub(crate) controlled_adapter_candidate_path: String,
    pub(crate) controlled_adapter_invocation_allowed: bool,
    pub(crate) controlled_adapter_executor_attached: bool,
    pub(crate) send_contract_shape: String,
    pub(crate) stream_contract_shape: String,
    pub(crate) runtime_call_enabled: bool,
    pub(crate) model_call_enabled: bool,
    pub(crate) tool_call_enabled: bool,
    pub(crate) allow_writes: bool,
    pub(crate) max_tool_calls: u32,
    pub(crate) chat_message_saved: bool,
    pub(crate) agent_run_recorded: bool,
    pub(crate) evidence_recorded: bool,
    pub(crate) default_chat_path_unchanged: bool,
    pub(crate) blocking_reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DefaultChatAdapterInvocationBoundary {
    pub(crate) caller: String,
    pub(crate) boundary_ready: bool,
    pub(crate) plan_ready: bool,
    pub(crate) selected_adapter_path: String,
    pub(crate) required_callsite_path: String,
    pub(crate) fallback_adapter_path: String,
    pub(crate) controlled_adapter_candidate_path: String,
    pub(crate) legacy_adapter_invocation_required: bool,
    pub(crate) controlled_adapter_invocation_allowed: bool,
    pub(crate) controlled_adapter_executor_attached: bool,
    pub(crate) side_effect_free_before_legacy_entry: bool,
    pub(crate) runtime_call_enabled: bool,
    pub(crate) model_call_enabled: bool,
    pub(crate) tool_call_enabled: bool,
    pub(crate) allow_writes: bool,
    pub(crate) max_tool_calls: u32,
    pub(crate) chat_message_saved: bool,
    pub(crate) agent_run_recorded: bool,
    pub(crate) evidence_recorded: bool,
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

pub(crate) fn evaluate_default_chat_adapter_callsite_contract(
    callsite: DefaultChatAdapterCallsite,
    route: &DefaultChatAdapterRoute,
) -> DefaultChatAdapterCallsiteContract {
    let boundary = evaluate_default_chat_adapter_invocation_boundary(callsite.caller(), route);
    let boundary_guard_passed =
        ensure_default_chat_adapter_invocation_boundary(callsite.caller(), route).is_ok();
    debug_assert_eq!(boundary_guard_passed, boundary.boundary_ready);
    let actual_callsite_path = callsite.route_path(route).to_string();
    let mut blocking_reasons = boundary.blocking_reasons.clone();

    if !boundary_guard_passed {
        blocking_reasons.insert(0, "invocation_boundary_not_ready".into());
    }
    if actual_callsite_path != boundary.required_callsite_path {
        blocking_reasons.push("callsite_path_not_legacy_stream".into());
    }

    let contract_ready = boundary_guard_passed
        && blocking_reasons.is_empty()
        && actual_callsite_path == LEGACY_STREAM_PATH;

    DefaultChatAdapterCallsiteContract {
        callsite: callsite.caller().into(),
        contract_ready,
        boundary_ready: boundary_guard_passed,
        contract_shape: callsite.contract_shape().into(),
        selected_adapter_path: if contract_ready {
            LEGACY_STREAM_PATH
        } else {
            "blocked"
        }
        .into(),
        required_callsite_path: boundary.required_callsite_path,
        actual_callsite_path,
        controlled_adapter_executor_attached: boundary.controlled_adapter_executor_attached,
        side_effect_free_before_legacy_entry: boundary.side_effect_free_before_legacy_entry,
        blocking_reasons,
    }
}

pub(crate) fn ensure_default_chat_adapter_callsite_contract(
    callsite: DefaultChatAdapterCallsite,
    route: &DefaultChatAdapterRoute,
) -> Result<DefaultChatAdapterCallsiteContract, String> {
    let contract = evaluate_default_chat_adapter_callsite_contract(callsite, route);

    if contract.contract_ready {
        Ok(contract)
    } else {
        Err(format!(
            "{} blocked by default Chat adapter callsite contract: {}",
            contract.callsite,
            contract.blocking_reasons.join(", ")
        ))
    }
}

pub(crate) fn evaluate_default_chat_adapter_ordinary_entry_preflight(
    callsite: DefaultChatAdapterCallsite,
    route: &DefaultChatAdapterRoute,
) -> DefaultChatAdapterOrdinaryEntryPreflight {
    let contract = evaluate_default_chat_adapter_callsite_contract(callsite, route);
    let contract_guard_passed =
        ensure_default_chat_adapter_callsite_contract(callsite, route).is_ok();
    debug_assert_eq!(contract_guard_passed, contract.contract_ready);
    let boundary = evaluate_default_chat_adapter_invocation_boundary(callsite.caller(), route);
    let mut blocking_reasons = contract.blocking_reasons.clone();

    if !contract_guard_passed {
        blocking_reasons.insert(0, "callsite_contract_not_ready".into());
    }

    let side_effect_lock_engaged = !boundary.runtime_call_enabled
        && !boundary.model_call_enabled
        && !boundary.tool_call_enabled
        && !boundary.allow_writes
        && boundary.max_tool_calls == 0
        && !boundary.chat_message_saved
        && !boundary.agent_run_recorded
        && !boundary.evidence_recorded;

    if contract_guard_passed && !side_effect_lock_engaged {
        blocking_reasons.push("ordinary_entry_side_effect_lock_not_engaged".into());
    }

    let legacy_entry_allowed = contract_guard_passed
        && contract.contract_ready
        && contract.selected_adapter_path == LEGACY_STREAM_PATH
        && contract.actual_callsite_path == LEGACY_STREAM_PATH
        && !contract.controlled_adapter_executor_attached
        && side_effect_lock_engaged;
    let default_chat_migration_allowed = false;
    let preflight_ready =
        legacy_entry_allowed && !default_chat_migration_allowed && blocking_reasons.is_empty();

    DefaultChatAdapterOrdinaryEntryPreflight {
        callsite: contract.callsite,
        preflight_ready,
        contract_ready: contract_guard_passed && contract.contract_ready,
        legacy_entry_allowed,
        ordinary_entry_path: if preflight_ready {
            LEGACY_STREAM_PATH
        } else {
            "blocked"
        }
        .into(),
        required_entry_path: contract.required_callsite_path,
        contract_shape: contract.contract_shape,
        side_effect_lock_engaged,
        default_chat_migration_allowed,
        controlled_adapter_executor_attached: contract.controlled_adapter_executor_attached,
        runtime_call_enabled: boundary.runtime_call_enabled,
        model_call_enabled: boundary.model_call_enabled,
        tool_call_enabled: boundary.tool_call_enabled,
        allow_writes: boundary.allow_writes,
        max_tool_calls: boundary.max_tool_calls,
        chat_message_saved: boundary.chat_message_saved,
        agent_run_recorded: boundary.agent_run_recorded,
        evidence_recorded: boundary.evidence_recorded,
        blocking_reasons,
    }
}

pub(crate) fn ensure_default_chat_adapter_ordinary_entry_preflight(
    callsite: DefaultChatAdapterCallsite,
    route: &DefaultChatAdapterRoute,
) -> Result<DefaultChatAdapterOrdinaryEntryPreflight, String> {
    let preflight = evaluate_default_chat_adapter_ordinary_entry_preflight(callsite, route);

    if preflight.preflight_ready {
        Ok(preflight)
    } else {
        Err(format!(
            "{} blocked by default Chat adapter ordinary entry preflight: {}",
            preflight.callsite,
            preflight.blocking_reasons.join(", ")
        ))
    }
}

pub(crate) fn evaluate_default_chat_adapter_invocation_boundary(
    caller: &str,
    route: &DefaultChatAdapterRoute,
) -> DefaultChatAdapterInvocationBoundary {
    let plan = plan_default_chat_adapter_invocation(caller, route);
    let plan_guard_passed = ensure_default_chat_adapter_invocation_plan(caller, route).is_ok();
    debug_assert_eq!(plan_guard_passed, plan.plan_ready);
    let mut blocking_reasons = plan.blocking_reasons.clone();

    if !plan_guard_passed {
        blocking_reasons.insert(0, "invocation_plan_not_ready".into());
    }
    if plan_guard_passed && plan.selected_adapter_path != LEGACY_STREAM_PATH {
        blocking_reasons.push("selected_adapter_path_not_legacy_stream".into());
    }

    let side_effect_free_before_legacy_entry = !plan.runtime_call_enabled
        && !plan.model_call_enabled
        && !plan.tool_call_enabled
        && !plan.allow_writes
        && plan.max_tool_calls == 0
        && !plan.chat_message_saved
        && !plan.agent_run_recorded
        && !plan.evidence_recorded;

    if plan_guard_passed && !side_effect_free_before_legacy_entry {
        blocking_reasons.push("invocation_boundary_not_side_effect_free".into());
    }

    let boundary_ready = plan_guard_passed
        && blocking_reasons.is_empty()
        && plan.selected_adapter_path == LEGACY_STREAM_PATH
        && !plan.controlled_adapter_invocation_allowed
        && !plan.controlled_adapter_executor_attached
        && side_effect_free_before_legacy_entry;

    DefaultChatAdapterInvocationBoundary {
        caller: caller.into(),
        boundary_ready,
        plan_ready: plan_guard_passed,
        selected_adapter_path: if boundary_ready {
            LEGACY_STREAM_PATH
        } else {
            "blocked"
        }
        .into(),
        required_callsite_path: LEGACY_STREAM_PATH.into(),
        fallback_adapter_path: plan.fallback_adapter_path,
        controlled_adapter_candidate_path: plan.controlled_adapter_candidate_path,
        legacy_adapter_invocation_required: boundary_ready,
        controlled_adapter_invocation_allowed: plan.controlled_adapter_invocation_allowed,
        controlled_adapter_executor_attached: plan.controlled_adapter_executor_attached,
        side_effect_free_before_legacy_entry,
        runtime_call_enabled: plan.runtime_call_enabled,
        model_call_enabled: plan.model_call_enabled,
        tool_call_enabled: plan.tool_call_enabled,
        allow_writes: plan.allow_writes,
        max_tool_calls: plan.max_tool_calls,
        chat_message_saved: plan.chat_message_saved,
        agent_run_recorded: plan.agent_run_recorded,
        evidence_recorded: plan.evidence_recorded,
        blocking_reasons,
    }
}

pub(crate) fn ensure_default_chat_adapter_invocation_boundary(
    caller: &str,
    route: &DefaultChatAdapterRoute,
) -> Result<DefaultChatAdapterInvocationBoundary, String> {
    let boundary = evaluate_default_chat_adapter_invocation_boundary(caller, route);

    if boundary.boundary_ready {
        Ok(boundary)
    } else {
        Err(format!(
            "{caller} blocked by default Chat adapter invocation boundary: {}",
            boundary.blocking_reasons.join(", ")
        ))
    }
}

pub(crate) fn plan_default_chat_adapter_invocation(
    caller: &str,
    route: &DefaultChatAdapterRoute,
) -> DefaultChatAdapterInvocationPlan {
    let harness = evaluate_default_chat_adapter_cutover_harness(caller, route);
    let harness_guard_passed = ensure_default_chat_cutover_harness(caller, route).is_ok();
    debug_assert_eq!(harness_guard_passed, harness.harness_ready);
    let mut blocking_reasons = harness.blocking_reasons.clone();

    if !harness_guard_passed {
        blocking_reasons.insert(0, "cutover_harness_not_ready".into());
    }

    let plan_ready = harness_guard_passed && blocking_reasons.is_empty();

    DefaultChatAdapterInvocationPlan {
        caller: caller.into(),
        plan_ready,
        harness_ready: harness_guard_passed,
        selected_adapter_path: if plan_ready {
            LEGACY_STREAM_PATH
        } else {
            "blocked"
        }
        .into(),
        fallback_adapter_path: LEGACY_STREAM_PATH.into(),
        controlled_adapter_candidate_path: CONTROLLED_ADAPTER_PATH.into(),
        controlled_adapter_invocation_allowed: false,
        controlled_adapter_executor_attached: false,
        send_contract_shape: "send_message_compatible".into(),
        stream_contract_shape: "stream_message_compatible".into(),
        runtime_call_enabled: false,
        model_call_enabled: false,
        tool_call_enabled: false,
        allow_writes: false,
        max_tool_calls: 0,
        chat_message_saved: false,
        agent_run_recorded: false,
        evidence_recorded: false,
        default_chat_path_unchanged: harness.default_chat_path_unchanged,
        blocking_reasons,
    }
}

pub(crate) fn ensure_default_chat_adapter_invocation_plan(
    caller: &str,
    route: &DefaultChatAdapterRoute,
) -> Result<(), String> {
    let plan = plan_default_chat_adapter_invocation(caller, route);

    if plan.plan_ready {
        Ok(())
    } else {
        Err(format!(
            "{caller} blocked by default Chat adapter invocation plan: {}",
            plan.blocking_reasons.join(", ")
        ))
    }
}
