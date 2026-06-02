use sha2::{Digest, Sha256};

pub(crate) const LEGACY_STREAM_PATH: &str = "legacy_stream";
pub(crate) const CONTROLLED_ADAPTER_PATH: &str = "controlled_adapter";
#[allow(dead_code)]
pub(crate) const CONTROLLED_ADAPTER_EXECUTOR_DISABLED_STATE: &str = "disabled_unattached";
#[allow(dead_code)]
const DEFAULT_CHAT_EXECUTOR_SKELETON_SEND_MESSAGE_RESULT_SHAPE: &str = "send_message_result";
#[allow(dead_code)]
const DEFAULT_CHAT_EXECUTOR_SKELETON_STREAM_BOUNDARY_SHAPE: &str = "stream_boundary";
#[allow(dead_code)]
const DEFAULT_CHAT_EXECUTOR_SKELETON_UNKNOWN_SHAPE: &str = "unknown";

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

#[allow(dead_code)]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct DefaultChatAdapterDescriptorSideEffectBudget {
    pub(crate) runtime_calls: u32,
    pub(crate) model_calls: u32,
    pub(crate) tool_calls: u32,
    pub(crate) store_writes: u32,
    pub(crate) chat_message_writes: u32,
    pub(crate) agent_run_writes: u32,
    pub(crate) evidence_writes: u32,
    pub(crate) proposal_writes: u32,
    pub(crate) memory_writes: u32,
    pub(crate) life_model_writes: u32,
    pub(crate) mcp_audit_writes: u32,
    pub(crate) external_writes: u32,
}

#[allow(dead_code)]
impl DefaultChatAdapterDescriptorSideEffectBudget {
    pub(crate) fn zero() -> Self {
        Self::default()
    }

    pub(crate) fn is_zero(&self) -> bool {
        self.runtime_calls == 0
            && self.model_calls == 0
            && self.tool_calls == 0
            && self.store_writes == 0
            && self.chat_message_writes == 0
            && self.agent_run_writes == 0
            && self.evidence_writes == 0
            && self.proposal_writes == 0
            && self.memory_writes == 0
            && self.life_model_writes == 0
            && self.mcp_audit_writes == 0
            && self.external_writes == 0
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DefaultChatControlledAdapterDescriptor {
    pub(crate) descriptor_kind: String,
    pub(crate) metadata_safe: bool,
    pub(crate) contains_raw_content: bool,
    pub(crate) mapper_side_effect_free: bool,
    pub(crate) callsite_kind: String,
    pub(crate) contract_shape: String,
    pub(crate) route_mode: String,
    pub(crate) selected_adapter_path: String,
    pub(crate) required_callsite_path: String,
    pub(crate) actual_callsite_path: String,
    pub(crate) default_send_path: String,
    pub(crate) start_stream_path: String,
    pub(crate) controlled_adapter_candidate_path: String,
    pub(crate) controlled_adapter_enabled: bool,
    pub(crate) automatic_migration_enabled: bool,
    pub(crate) controlled_adapter_invocation_allowed: bool,
    pub(crate) controlled_adapter_executor_enabled: bool,
    pub(crate) controlled_adapter_executor_attached: bool,
    pub(crate) controlled_adapter_executor_state: String,
    pub(crate) allow_writes: bool,
    pub(crate) max_tool_calls: u32,
    pub(crate) side_effect_budget: DefaultChatAdapterDescriptorSideEffectBudget,
    pub(crate) input_length_bytes: usize,
    pub(crate) input_length_chars: usize,
    pub(crate) input_sha256: String,
    pub(crate) route_guard_passed: bool,
    pub(crate) descriptor_ready: bool,
    pub(crate) fail_closed: bool,
    pub(crate) migration_permission: bool,
    pub(crate) blocking_reasons: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DefaultChatControlledAdapterContractReport {
    pub(crate) callsite_kind: String,
    pub(crate) contract_shape: String,
    pub(crate) descriptor_ready: bool,
    pub(crate) contract_ready: bool,
    pub(crate) metadata_safe: bool,
    pub(crate) contains_raw_content: bool,
    pub(crate) mapper_side_effect_free: bool,
    pub(crate) selected_adapter_path: String,
    pub(crate) required_callsite_path: String,
    pub(crate) actual_callsite_path: String,
    pub(crate) default_send_path: String,
    pub(crate) start_stream_path: String,
    pub(crate) controlled_adapter_candidate_path: String,
    pub(crate) controlled_adapter_enabled: bool,
    pub(crate) automatic_migration_enabled: bool,
    pub(crate) controlled_adapter_invocation_allowed: bool,
    pub(crate) controlled_adapter_executor_enabled: bool,
    pub(crate) controlled_adapter_executor_attached: bool,
    pub(crate) controlled_adapter_executor_state: String,
    pub(crate) allow_writes: bool,
    pub(crate) max_tool_calls: u32,
    pub(crate) side_effect_budget: DefaultChatAdapterDescriptorSideEffectBudget,
    pub(crate) migration_permission: bool,
    pub(crate) default_chat_unchanged: bool,
    pub(crate) input_length_bytes: usize,
    pub(crate) input_length_chars: usize,
    pub(crate) input_sha256: String,
    pub(crate) blocking_reasons: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DefaultChatControlledAdapterInvocationHarness {
    pub(crate) harness_kind: String,
    pub(crate) callsite_kind: String,
    pub(crate) contract_shape: String,
    pub(crate) contract_ready: bool,
    pub(crate) harness_ready: bool,
    pub(crate) metadata_safe: bool,
    pub(crate) contains_raw_content: bool,
    pub(crate) non_default: bool,
    pub(crate) ordinary_default_chat_path_unchanged: bool,
    pub(crate) selected_adapter_path: String,
    pub(crate) candidate_adapter_path: String,
    pub(crate) controlled_adapter_invocation_allowed: bool,
    pub(crate) controlled_adapter_executor_enabled: bool,
    pub(crate) controlled_adapter_executor_attached: bool,
    pub(crate) controlled_adapter_executor_state: String,
    pub(crate) allow_writes: bool,
    pub(crate) max_tool_calls: u32,
    pub(crate) side_effect_budget: DefaultChatAdapterDescriptorSideEffectBudget,
    pub(crate) side_effect_budget_zero: bool,
    pub(crate) runtime_call_enabled: bool,
    pub(crate) model_call_enabled: bool,
    pub(crate) tool_call_enabled: bool,
    pub(crate) business_write_disabled: bool,
    pub(crate) migration_permission: bool,
    pub(crate) input_length_bytes: usize,
    pub(crate) input_length_chars: usize,
    pub(crate) input_sha256: String,
    pub(crate) blocking_reasons: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DefaultChatControlledAdapterSendCompatibleProof {
    pub(crate) proof_kind: String,
    pub(crate) callsite_kind: String,
    pub(crate) contract_shape: String,
    pub(crate) send_message_result_compatible: bool,
    pub(crate) proof_ready: bool,
    pub(crate) descriptor_ready: bool,
    pub(crate) contract_ready: bool,
    pub(crate) harness_ready: bool,
    pub(crate) metadata_safe: bool,
    pub(crate) contains_raw_content: bool,
    pub(crate) selected_adapter_path: String,
    pub(crate) candidate_adapter_path: String,
    pub(crate) required_callsite_path: String,
    pub(crate) actual_callsite_path: String,
    pub(crate) default_send_path: String,
    pub(crate) start_stream_path: String,
    pub(crate) controlled_adapter_enabled: bool,
    pub(crate) automatic_migration_enabled: bool,
    pub(crate) controlled_adapter_invocation_allowed: bool,
    pub(crate) controlled_adapter_executor_enabled: bool,
    pub(crate) controlled_adapter_executor_attached: bool,
    pub(crate) controlled_adapter_executor_state: String,
    pub(crate) allow_writes: bool,
    pub(crate) max_tool_calls: u32,
    pub(crate) side_effect_budget: DefaultChatAdapterDescriptorSideEffectBudget,
    pub(crate) side_effect_budget_zero: bool,
    pub(crate) runtime_call_enabled: bool,
    pub(crate) model_call_enabled: bool,
    pub(crate) tool_call_enabled: bool,
    pub(crate) business_write_disabled: bool,
    pub(crate) migration_permission: bool,
    pub(crate) chat_message_saved: bool,
    pub(crate) agent_run_recorded: bool,
    pub(crate) evidence_recorded: bool,
    pub(crate) proposal_created: bool,
    pub(crate) memory_written: bool,
    pub(crate) life_model_written: bool,
    pub(crate) external_write_recorded: bool,
    pub(crate) default_chat_unchanged: bool,
    pub(crate) input_length_bytes: usize,
    pub(crate) input_length_chars: usize,
    pub(crate) input_sha256: String,
    pub(crate) blocking_reasons: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DefaultChatControlledAdapterStreamBoundaryProof {
    pub(crate) proof_kind: String,
    pub(crate) callsite_kind: String,
    pub(crate) contract_shape: String,
    pub(crate) stream_message_compatible: bool,
    pub(crate) proof_ready: bool,
    pub(crate) descriptor_ready: bool,
    pub(crate) contract_ready: bool,
    pub(crate) harness_ready: bool,
    pub(crate) metadata_safe: bool,
    pub(crate) contains_raw_content: bool,
    pub(crate) selected_adapter_path: String,
    pub(crate) candidate_adapter_path: String,
    pub(crate) required_callsite_path: String,
    pub(crate) actual_callsite_path: String,
    pub(crate) default_send_path: String,
    pub(crate) start_stream_path: String,
    pub(crate) controlled_adapter_enabled: bool,
    pub(crate) automatic_migration_enabled: bool,
    pub(crate) controlled_adapter_invocation_allowed: bool,
    pub(crate) stream_started: bool,
    pub(crate) stream_events_emitted: bool,
    pub(crate) event_channel_opened: bool,
    pub(crate) executor_enabled: bool,
    pub(crate) executor_attached: bool,
    pub(crate) executor_state: String,
    pub(crate) allow_writes: bool,
    pub(crate) max_tool_calls: u32,
    pub(crate) side_effect_budget: DefaultChatAdapterDescriptorSideEffectBudget,
    pub(crate) side_effect_budget_zero: bool,
    pub(crate) runtime_call_enabled: bool,
    pub(crate) model_call_enabled: bool,
    pub(crate) tool_call_enabled: bool,
    pub(crate) business_write_disabled: bool,
    pub(crate) migration_permission: bool,
    pub(crate) chat_message_saved: bool,
    pub(crate) agent_run_recorded: bool,
    pub(crate) evidence_recorded: bool,
    pub(crate) proposal_created: bool,
    pub(crate) memory_written: bool,
    pub(crate) life_model_written: bool,
    pub(crate) mcp_audit_written: bool,
    pub(crate) external_write_recorded: bool,
    pub(crate) default_chat_unchanged: bool,
    pub(crate) input_length_bytes: usize,
    pub(crate) input_length_chars: usize,
    pub(crate) input_sha256: String,
    pub(crate) blocking_reasons: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DefaultChatControlledAdapterExecutorAttachmentGateReport {
    pub(crate) report_kind: String,
    pub(crate) gate_report_metadata_ready: bool,
    pub(crate) executor_skeleton_discussion_ready: bool,
    pub(crate) executor_attachment_allowed: bool,
    pub(crate) executor_enabled: bool,
    pub(crate) executor_attached: bool,
    pub(crate) executor_implementation_present: bool,
    pub(crate) human_review_present: bool,
    pub(crate) route_cutover_permission: bool,
    pub(crate) migration_permission: bool,
    pub(crate) ordinary_default_chat_unchanged: bool,
    pub(crate) selected_adapter_path: String,
    pub(crate) candidate_adapter_path: String,
    pub(crate) default_send_path: String,
    pub(crate) start_stream_path: String,
    pub(crate) controlled_adapter_enabled: bool,
    pub(crate) automatic_migration_enabled: bool,
    pub(crate) controlled_adapter_invocation_allowed: bool,
    pub(crate) send_message_result_compatible: bool,
    pub(crate) stream_message_compatible: bool,
    pub(crate) send_proof_ready: bool,
    pub(crate) stream_boundary_proof_ready: bool,
    pub(crate) w68_send_compatible_proof_ready: bool,
    pub(crate) w69_stream_boundary_proof_ready: bool,
    pub(crate) send_descriptor_ready: bool,
    pub(crate) send_contract_ready: bool,
    pub(crate) send_harness_ready: bool,
    pub(crate) stream_descriptor_ready: bool,
    pub(crate) stream_contract_ready: bool,
    pub(crate) stream_harness_ready: bool,
    pub(crate) w65_w67_metadata_safe: bool,
    pub(crate) metadata_safe: bool,
    pub(crate) contains_raw_content: bool,
    pub(crate) stream_started: bool,
    pub(crate) stream_events_emitted: bool,
    pub(crate) event_channel_opened: bool,
    pub(crate) allow_writes: bool,
    pub(crate) max_tool_calls: u32,
    pub(crate) side_effect_budget: DefaultChatAdapterDescriptorSideEffectBudget,
    pub(crate) side_effect_budget_zero: bool,
    pub(crate) runtime_call_enabled: bool,
    pub(crate) model_call_enabled: bool,
    pub(crate) tool_call_enabled: bool,
    pub(crate) business_write_disabled: bool,
    pub(crate) chat_message_saved: bool,
    pub(crate) agent_run_recorded: bool,
    pub(crate) evidence_recorded: bool,
    pub(crate) proposal_created: bool,
    pub(crate) memory_written: bool,
    pub(crate) life_model_written: bool,
    pub(crate) mcp_audit_written: bool,
    pub(crate) external_write_recorded: bool,
    pub(crate) input_length_bytes: usize,
    pub(crate) input_length_chars: usize,
    pub(crate) input_sha256: String,
    pub(crate) blocking_reasons: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DefaultChatControlledAdapterExecutorSkeletonInput {
    pub(crate) input_kind: String,
    pub(crate) callsite_kind: String,
    pub(crate) route_current_mode: String,
    pub(crate) route_adapter_scaffold_present: bool,
    pub(crate) route_default_send_path: String,
    pub(crate) route_start_stream_path: String,
    pub(crate) route_controlled_adapter_enabled: bool,
    pub(crate) route_automatic_migration_enabled: bool,
    pub(crate) route_requires_separate_cutover_implementation: bool,
    pub(crate) input_length_bytes: usize,
    pub(crate) input_length_chars: usize,
    pub(crate) input_sha256: String,
    pub(crate) requested_shape: String,
}

#[allow(dead_code)]
impl DefaultChatControlledAdapterExecutorSkeletonInput {
    pub(crate) fn from_request_metadata(
        callsite: DefaultChatAdapterCallsite,
        route: &DefaultChatAdapterRoute,
        input: &str,
        requested_shape: &str,
    ) -> Self {
        Self {
            input_kind: "default_chat_controlled_adapter_executor_skeleton_input".into(),
            callsite_kind: callsite.caller().into(),
            route_current_mode: route.current_mode.clone(),
            route_adapter_scaffold_present: route.adapter_scaffold_present,
            route_default_send_path: route.default_send_path.clone(),
            route_start_stream_path: route.start_stream_path.clone(),
            route_controlled_adapter_enabled: route.controlled_adapter_enabled,
            route_automatic_migration_enabled: route.automatic_migration_enabled,
            route_requires_separate_cutover_implementation: route
                .requires_separate_cutover_implementation,
            input_length_bytes: input.len(),
            input_length_chars: input.chars().count(),
            input_sha256: default_chat_adapter_metadata_sha256(input),
            requested_shape: default_chat_executor_skeleton_requested_shape(requested_shape).into(),
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DefaultChatControlledAdapterExecutorSkeletonOutput {
    pub(crate) output_kind: String,
    pub(crate) compatible_shape: String,
    pub(crate) executor_state: String,
    pub(crate) no_user_visible_output: bool,
    pub(crate) raw_output_present: bool,
    pub(crate) blocking_reasons: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DefaultChatControlledAdapterDisabledExecutorSkeleton {
    pub(crate) skeleton_kind: String,
    pub(crate) skeleton_contract_ready: bool,
    pub(crate) metadata_safe: bool,
    pub(crate) contains_raw_content: bool,
    pub(crate) executor_skeleton_present: bool,
    pub(crate) gate_report_metadata_ready: bool,
    pub(crate) executor_attachment_allowed: bool,
    pub(crate) executor_enabled: bool,
    pub(crate) executor_attached: bool,
    pub(crate) executor_runnable: bool,
    pub(crate) invocation_allowed: bool,
    pub(crate) route_cutover_permission: bool,
    pub(crate) migration_permission: bool,
    pub(crate) ordinary_default_chat_unchanged: bool,
    pub(crate) selected_adapter_path: String,
    pub(crate) callsite_kind: String,
    pub(crate) requested_shape: String,
    pub(crate) controlled_adapter_enabled: bool,
    pub(crate) automatic_migration_enabled: bool,
    pub(crate) stream_started: bool,
    pub(crate) event_channel_opened: bool,
    pub(crate) stream_events_emitted: bool,
    pub(crate) runtime_call_enabled: bool,
    pub(crate) model_call_enabled: bool,
    pub(crate) tool_call_enabled: bool,
    pub(crate) allow_writes: bool,
    pub(crate) max_tool_calls: u32,
    pub(crate) side_effect_budget: DefaultChatAdapterDescriptorSideEffectBudget,
    pub(crate) side_effect_budget_zero: bool,
    pub(crate) business_write_disabled: bool,
    pub(crate) chat_message_saved: bool,
    pub(crate) agent_run_recorded: bool,
    pub(crate) evidence_recorded: bool,
    pub(crate) proposal_created: bool,
    pub(crate) memory_written: bool,
    pub(crate) life_model_written: bool,
    pub(crate) mcp_audit_written: bool,
    pub(crate) external_write_recorded: bool,
    pub(crate) input_length_bytes: usize,
    pub(crate) input_length_chars: usize,
    pub(crate) input_sha256: String,
    pub(crate) output: DefaultChatControlledAdapterExecutorSkeletonOutput,
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

#[allow(dead_code)]
pub(crate) fn describe_default_chat_controlled_adapter_candidate(
    callsite: DefaultChatAdapterCallsite,
    route: &DefaultChatAdapterRoute,
    input: &str,
) -> DefaultChatControlledAdapterDescriptor {
    let mut blocking_reasons = default_chat_legacy_route_blockers(route);
    let actual_callsite_path = callsite.route_path(route).to_string();

    if actual_callsite_path != LEGACY_STREAM_PATH {
        blocking_reasons.push("callsite_path_not_legacy_stream".into());
    }

    let route_guard_passed = blocking_reasons.is_empty();
    let descriptor_ready = route_guard_passed
        && route.current_mode == LEGACY_STREAM_PATH
        && actual_callsite_path == LEGACY_STREAM_PATH
        && !route.controlled_adapter_enabled
        && !route.automatic_migration_enabled;

    DefaultChatControlledAdapterDescriptor {
        descriptor_kind: "default_chat_controlled_adapter_candidate".into(),
        metadata_safe: true,
        contains_raw_content: false,
        mapper_side_effect_free: true,
        callsite_kind: callsite.caller().into(),
        contract_shape: callsite.contract_shape().into(),
        route_mode: route.current_mode.clone(),
        selected_adapter_path: if descriptor_ready {
            LEGACY_STREAM_PATH
        } else {
            "blocked"
        }
        .into(),
        required_callsite_path: LEGACY_STREAM_PATH.into(),
        actual_callsite_path,
        default_send_path: route.default_send_path.clone(),
        start_stream_path: route.start_stream_path.clone(),
        controlled_adapter_candidate_path: CONTROLLED_ADAPTER_PATH.into(),
        controlled_adapter_enabled: route.controlled_adapter_enabled,
        automatic_migration_enabled: route.automatic_migration_enabled,
        controlled_adapter_invocation_allowed: false,
        controlled_adapter_executor_enabled: false,
        controlled_adapter_executor_attached: false,
        controlled_adapter_executor_state: CONTROLLED_ADAPTER_EXECUTOR_DISABLED_STATE.into(),
        allow_writes: false,
        max_tool_calls: 0,
        side_effect_budget: DefaultChatAdapterDescriptorSideEffectBudget::zero(),
        input_length_bytes: input.len(),
        input_length_chars: input.chars().count(),
        input_sha256: default_chat_adapter_metadata_sha256(input),
        route_guard_passed,
        descriptor_ready,
        fail_closed: !descriptor_ready,
        migration_permission: false,
        blocking_reasons,
    }
}

#[allow(dead_code)]
fn default_chat_adapter_metadata_sha256(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

#[allow(dead_code)]
pub(crate) fn evaluate_default_chat_controlled_adapter_contract(
    callsite: DefaultChatAdapterCallsite,
    route: &DefaultChatAdapterRoute,
    input: &str,
) -> DefaultChatControlledAdapterContractReport {
    let descriptor = describe_default_chat_controlled_adapter_candidate(callsite, route, input);
    let default_chat_unchanged = descriptor.route_mode == LEGACY_STREAM_PATH
        && descriptor.default_send_path == LEGACY_STREAM_PATH
        && descriptor.start_stream_path == LEGACY_STREAM_PATH
        && !descriptor.controlled_adapter_enabled
        && !descriptor.automatic_migration_enabled;
    let mut blocking_reasons = descriptor.blocking_reasons.clone();

    if !descriptor.descriptor_ready {
        push_unique_blocker(&mut blocking_reasons, "descriptor_not_ready");
    }
    if !descriptor.metadata_safe {
        push_unique_blocker(&mut blocking_reasons, "descriptor_not_metadata_safe");
    }
    if descriptor.contains_raw_content {
        push_unique_blocker(&mut blocking_reasons, "descriptor_contains_raw_content");
    }
    if !descriptor.mapper_side_effect_free {
        push_unique_blocker(&mut blocking_reasons, "mapper_not_side_effect_free");
    }
    if descriptor.selected_adapter_path != LEGACY_STREAM_PATH {
        push_unique_blocker(
            &mut blocking_reasons,
            "selected_adapter_path_not_legacy_stream",
        );
    }
    if descriptor.required_callsite_path != LEGACY_STREAM_PATH {
        push_unique_blocker(
            &mut blocking_reasons,
            "required_callsite_path_not_legacy_stream",
        );
    }
    if descriptor.actual_callsite_path != LEGACY_STREAM_PATH {
        push_unique_blocker(
            &mut blocking_reasons,
            "actual_callsite_path_not_legacy_stream",
        );
    }
    if descriptor.controlled_adapter_invocation_allowed {
        push_unique_blocker(
            &mut blocking_reasons,
            "controlled_adapter_invocation_allowed",
        );
    }
    if descriptor.controlled_adapter_executor_enabled {
        push_unique_blocker(&mut blocking_reasons, "controlled_adapter_executor_enabled");
    }
    if descriptor.controlled_adapter_executor_attached {
        push_unique_blocker(
            &mut blocking_reasons,
            "controlled_adapter_executor_attached",
        );
    }
    if descriptor.controlled_adapter_executor_state != CONTROLLED_ADAPTER_EXECUTOR_DISABLED_STATE {
        push_unique_blocker(
            &mut blocking_reasons,
            "controlled_adapter_executor_not_disabled",
        );
    }
    if descriptor.allow_writes {
        push_unique_blocker(&mut blocking_reasons, "allow_writes_enabled");
    }
    if descriptor.max_tool_calls != 0 {
        push_unique_blocker(&mut blocking_reasons, "max_tool_calls_not_zero");
    }
    if !descriptor.side_effect_budget.is_zero() {
        push_unique_blocker(&mut blocking_reasons, "side_effect_budget_not_zero");
    }
    if descriptor.migration_permission {
        push_unique_blocker(&mut blocking_reasons, "migration_permission_enabled");
    }
    if !default_chat_unchanged {
        push_unique_blocker(&mut blocking_reasons, "default_chat_not_legacy_stream");
    }

    let contract_ready = blocking_reasons.is_empty();

    DefaultChatControlledAdapterContractReport {
        callsite_kind: descriptor.callsite_kind,
        contract_shape: descriptor.contract_shape,
        descriptor_ready: descriptor.descriptor_ready,
        contract_ready,
        metadata_safe: descriptor.metadata_safe,
        contains_raw_content: descriptor.contains_raw_content,
        mapper_side_effect_free: descriptor.mapper_side_effect_free,
        selected_adapter_path: descriptor.selected_adapter_path,
        required_callsite_path: descriptor.required_callsite_path,
        actual_callsite_path: descriptor.actual_callsite_path,
        default_send_path: descriptor.default_send_path,
        start_stream_path: descriptor.start_stream_path,
        controlled_adapter_candidate_path: descriptor.controlled_adapter_candidate_path,
        controlled_adapter_enabled: descriptor.controlled_adapter_enabled,
        automatic_migration_enabled: descriptor.automatic_migration_enabled,
        controlled_adapter_invocation_allowed: descriptor.controlled_adapter_invocation_allowed,
        controlled_adapter_executor_enabled: descriptor.controlled_adapter_executor_enabled,
        controlled_adapter_executor_attached: descriptor.controlled_adapter_executor_attached,
        controlled_adapter_executor_state: descriptor.controlled_adapter_executor_state,
        allow_writes: descriptor.allow_writes,
        max_tool_calls: descriptor.max_tool_calls,
        side_effect_budget: descriptor.side_effect_budget,
        migration_permission: descriptor.migration_permission,
        default_chat_unchanged,
        input_length_bytes: descriptor.input_length_bytes,
        input_length_chars: descriptor.input_length_chars,
        input_sha256: descriptor.input_sha256,
        blocking_reasons,
    }
}

#[allow(dead_code)]
pub(crate) fn ensure_default_chat_controlled_adapter_contract(
    callsite: DefaultChatAdapterCallsite,
    route: &DefaultChatAdapterRoute,
    input: &str,
) -> Result<DefaultChatControlledAdapterContractReport, String> {
    let report = evaluate_default_chat_controlled_adapter_contract(callsite, route, input);
    if report.contract_ready {
        Ok(report)
    } else {
        Err(format!(
            "{} controlled_adapter_contract_not_ready: {}",
            report.callsite_kind,
            report.blocking_reasons.join(",")
        ))
    }
}

#[allow(dead_code)]
pub(crate) fn evaluate_default_chat_controlled_adapter_invocation_harness(
    callsite: DefaultChatAdapterCallsite,
    route: &DefaultChatAdapterRoute,
    input: &str,
) -> DefaultChatControlledAdapterInvocationHarness {
    let contract = evaluate_default_chat_controlled_adapter_contract(callsite, route, input);
    let side_effect_budget_zero = contract.side_effect_budget.is_zero();
    let runtime_call_enabled = contract.side_effect_budget.runtime_calls != 0;
    let model_call_enabled = contract.side_effect_budget.model_calls != 0;
    let tool_call_enabled = contract.side_effect_budget.tool_calls != 0;
    let business_write_disabled = !contract.allow_writes
        && contract.side_effect_budget.store_writes == 0
        && contract.side_effect_budget.chat_message_writes == 0
        && contract.side_effect_budget.agent_run_writes == 0
        && contract.side_effect_budget.evidence_writes == 0
        && contract.side_effect_budget.proposal_writes == 0
        && contract.side_effect_budget.memory_writes == 0
        && contract.side_effect_budget.life_model_writes == 0
        && contract.side_effect_budget.mcp_audit_writes == 0
        && contract.side_effect_budget.external_writes == 0;
    let non_default = true;
    let mut blocking_reasons = contract.blocking_reasons.clone();

    if !contract.contract_ready {
        push_unique_blocker(&mut blocking_reasons, "contract_not_ready");
    }
    if !contract.metadata_safe {
        push_unique_blocker(&mut blocking_reasons, "metadata_not_safe");
    }
    if contract.contains_raw_content {
        push_unique_blocker(&mut blocking_reasons, "raw_content_present");
    }
    if !contract.default_chat_unchanged {
        push_unique_blocker(&mut blocking_reasons, "ordinary_default_chat_path_changed");
    }
    if contract.selected_adapter_path != LEGACY_STREAM_PATH {
        push_unique_blocker(
            &mut blocking_reasons,
            "selected_adapter_path_not_legacy_stream",
        );
    }
    if contract.controlled_adapter_candidate_path != CONTROLLED_ADAPTER_PATH {
        push_unique_blocker(
            &mut blocking_reasons,
            "candidate_adapter_path_not_controlled_adapter",
        );
    }
    if contract.controlled_adapter_enabled {
        push_unique_blocker(&mut blocking_reasons, "controlled_adapter_enabled");
    }
    if contract.automatic_migration_enabled {
        push_unique_blocker(&mut blocking_reasons, "automatic_migration_enabled");
    }
    if contract.controlled_adapter_invocation_allowed {
        push_unique_blocker(
            &mut blocking_reasons,
            "controlled_adapter_invocation_allowed",
        );
    }
    if contract.controlled_adapter_executor_enabled {
        push_unique_blocker(&mut blocking_reasons, "controlled_adapter_executor_enabled");
    }
    if contract.controlled_adapter_executor_attached {
        push_unique_blocker(
            &mut blocking_reasons,
            "controlled_adapter_executor_attached",
        );
    }
    if contract.controlled_adapter_executor_state != CONTROLLED_ADAPTER_EXECUTOR_DISABLED_STATE {
        push_unique_blocker(
            &mut blocking_reasons,
            "controlled_adapter_executor_not_disabled",
        );
    }
    if contract.allow_writes {
        push_unique_blocker(&mut blocking_reasons, "allow_writes_enabled");
    }
    if contract.max_tool_calls != 0 {
        push_unique_blocker(&mut blocking_reasons, "max_tool_calls_not_zero");
    }
    if !side_effect_budget_zero {
        push_unique_blocker(&mut blocking_reasons, "side_effect_budget_not_zero");
    }
    if runtime_call_enabled {
        push_unique_blocker(&mut blocking_reasons, "runtime_call_enabled");
    }
    if model_call_enabled {
        push_unique_blocker(&mut blocking_reasons, "model_call_enabled");
    }
    if tool_call_enabled {
        push_unique_blocker(&mut blocking_reasons, "tool_call_enabled");
    }
    if !business_write_disabled {
        push_unique_blocker(&mut blocking_reasons, "business_write_enabled");
    }
    if contract.migration_permission {
        push_unique_blocker(&mut blocking_reasons, "migration_permission_enabled");
    }
    if !non_default {
        push_unique_blocker(&mut blocking_reasons, "not_non_default");
    }

    let harness_ready = blocking_reasons.is_empty();

    DefaultChatControlledAdapterInvocationHarness {
        harness_kind: "default_chat_controlled_adapter_non_default_invocation_harness".into(),
        callsite_kind: contract.callsite_kind,
        contract_shape: contract.contract_shape,
        contract_ready: contract.contract_ready,
        harness_ready,
        metadata_safe: contract.metadata_safe,
        contains_raw_content: contract.contains_raw_content,
        non_default,
        ordinary_default_chat_path_unchanged: contract.default_chat_unchanged,
        selected_adapter_path: contract.selected_adapter_path,
        candidate_adapter_path: contract.controlled_adapter_candidate_path,
        controlled_adapter_invocation_allowed: contract.controlled_adapter_invocation_allowed,
        controlled_adapter_executor_enabled: contract.controlled_adapter_executor_enabled,
        controlled_adapter_executor_attached: contract.controlled_adapter_executor_attached,
        controlled_adapter_executor_state: contract.controlled_adapter_executor_state,
        allow_writes: contract.allow_writes,
        max_tool_calls: contract.max_tool_calls,
        side_effect_budget: contract.side_effect_budget,
        side_effect_budget_zero,
        runtime_call_enabled,
        model_call_enabled,
        tool_call_enabled,
        business_write_disabled,
        migration_permission: contract.migration_permission,
        input_length_bytes: contract.input_length_bytes,
        input_length_chars: contract.input_length_chars,
        input_sha256: contract.input_sha256,
        blocking_reasons,
    }
}

#[allow(dead_code)]
pub(crate) fn ensure_default_chat_controlled_adapter_invocation_harness(
    callsite: DefaultChatAdapterCallsite,
    route: &DefaultChatAdapterRoute,
    input: &str,
) -> Result<DefaultChatControlledAdapterInvocationHarness, String> {
    let harness =
        evaluate_default_chat_controlled_adapter_invocation_harness(callsite, route, input);
    if harness.harness_ready {
        Ok(harness)
    } else {
        Err(format!(
            "{} controlled_adapter_invocation_harness_not_ready: {}",
            harness.callsite_kind,
            harness.blocking_reasons.join(",")
        ))
    }
}

#[allow(dead_code)]
pub(crate) fn evaluate_default_chat_controlled_adapter_send_compatible_proof(
    callsite: DefaultChatAdapterCallsite,
    route: &DefaultChatAdapterRoute,
    input: &str,
) -> DefaultChatControlledAdapterSendCompatibleProof {
    let descriptor = describe_default_chat_controlled_adapter_candidate(callsite, route, input);
    let contract = evaluate_default_chat_controlled_adapter_contract(callsite, route, input);
    let harness =
        evaluate_default_chat_controlled_adapter_invocation_harness(callsite, route, input);
    let side_effect_budget_zero = harness.side_effect_budget.is_zero();
    let chat_message_saved = harness.side_effect_budget.chat_message_writes != 0;
    let agent_run_recorded = harness.side_effect_budget.agent_run_writes != 0;
    let evidence_recorded = harness.side_effect_budget.evidence_writes != 0;
    let proposal_created = harness.side_effect_budget.proposal_writes != 0;
    let memory_written = harness.side_effect_budget.memory_writes != 0;
    let life_model_written = harness.side_effect_budget.life_model_writes != 0;
    let external_write_recorded = harness.side_effect_budget.external_writes != 0;
    let mut blocking_reasons = harness.blocking_reasons.clone();

    if callsite != DefaultChatAdapterCallsite::SendMessage {
        push_unique_blocker(&mut blocking_reasons, "callsite_not_send_message");
    }
    if harness.contract_shape != "send_message_compatible" {
        push_unique_blocker(
            &mut blocking_reasons,
            "contract_shape_not_send_message_compatible",
        );
    }
    if !descriptor.descriptor_ready {
        push_unique_blocker(&mut blocking_reasons, "descriptor_not_ready");
    }
    if !contract.contract_ready {
        push_unique_blocker(&mut blocking_reasons, "contract_not_ready");
    }
    if !harness.harness_ready {
        push_unique_blocker(&mut blocking_reasons, "harness_not_ready");
    }
    if !descriptor.metadata_safe || !contract.metadata_safe || !harness.metadata_safe {
        push_unique_blocker(&mut blocking_reasons, "metadata_not_safe");
    }
    if descriptor.contains_raw_content
        || contract.contains_raw_content
        || harness.contains_raw_content
    {
        push_unique_blocker(&mut blocking_reasons, "raw_content_present");
    }
    if !descriptor.mapper_side_effect_free {
        push_unique_blocker(&mut blocking_reasons, "mapper_not_side_effect_free");
    }
    if descriptor.route_mode != LEGACY_STREAM_PATH {
        push_unique_blocker(&mut blocking_reasons, "current_mode_not_legacy_stream");
    }
    if contract.default_send_path != LEGACY_STREAM_PATH {
        push_unique_blocker(&mut blocking_reasons, "default_send_path_not_legacy_stream");
    }
    if contract.start_stream_path != LEGACY_STREAM_PATH {
        push_unique_blocker(&mut blocking_reasons, "start_stream_path_not_legacy_stream");
    }
    if harness.selected_adapter_path != LEGACY_STREAM_PATH {
        push_unique_blocker(
            &mut blocking_reasons,
            "selected_adapter_path_not_legacy_stream",
        );
    }
    if harness.candidate_adapter_path != CONTROLLED_ADAPTER_PATH {
        push_unique_blocker(
            &mut blocking_reasons,
            "candidate_adapter_path_not_controlled_adapter",
        );
    }
    if contract.required_callsite_path != LEGACY_STREAM_PATH {
        push_unique_blocker(
            &mut blocking_reasons,
            "required_callsite_path_not_legacy_stream",
        );
    }
    if contract.actual_callsite_path != LEGACY_STREAM_PATH {
        push_unique_blocker(
            &mut blocking_reasons,
            "actual_callsite_path_not_legacy_stream",
        );
    }
    if harness.controlled_adapter_invocation_allowed {
        push_unique_blocker(
            &mut blocking_reasons,
            "controlled_adapter_invocation_allowed",
        );
    }
    if harness.controlled_adapter_executor_enabled {
        push_unique_blocker(&mut blocking_reasons, "controlled_adapter_executor_enabled");
    }
    if harness.controlled_adapter_executor_attached {
        push_unique_blocker(
            &mut blocking_reasons,
            "controlled_adapter_executor_attached",
        );
    }
    if harness.controlled_adapter_executor_state != CONTROLLED_ADAPTER_EXECUTOR_DISABLED_STATE {
        push_unique_blocker(
            &mut blocking_reasons,
            "controlled_adapter_executor_not_disabled",
        );
    }
    if harness.allow_writes {
        push_unique_blocker(&mut blocking_reasons, "allow_writes_enabled");
    }
    if harness.max_tool_calls != 0 {
        push_unique_blocker(&mut blocking_reasons, "max_tool_calls_not_zero");
    }
    if !side_effect_budget_zero {
        push_unique_blocker(&mut blocking_reasons, "side_effect_budget_not_zero");
    }
    if harness.runtime_call_enabled {
        push_unique_blocker(&mut blocking_reasons, "runtime_call_enabled");
    }
    if harness.model_call_enabled {
        push_unique_blocker(&mut blocking_reasons, "model_call_enabled");
    }
    if harness.tool_call_enabled {
        push_unique_blocker(&mut blocking_reasons, "tool_call_enabled");
    }
    if !harness.business_write_disabled {
        push_unique_blocker(&mut blocking_reasons, "business_write_enabled");
    }
    if harness.migration_permission {
        push_unique_blocker(&mut blocking_reasons, "migration_permission_enabled");
    }
    if !harness.ordinary_default_chat_path_unchanged {
        push_unique_blocker(&mut blocking_reasons, "default_chat_not_legacy_stream");
    }
    if chat_message_saved {
        push_unique_blocker(&mut blocking_reasons, "chat_message_saved");
    }
    if agent_run_recorded {
        push_unique_blocker(&mut blocking_reasons, "agent_run_recorded");
    }
    if evidence_recorded {
        push_unique_blocker(&mut blocking_reasons, "evidence_recorded");
    }
    if proposal_created {
        push_unique_blocker(&mut blocking_reasons, "proposal_created");
    }
    if memory_written {
        push_unique_blocker(&mut blocking_reasons, "memory_written");
    }
    if life_model_written {
        push_unique_blocker(&mut blocking_reasons, "life_model_written");
    }
    if external_write_recorded {
        push_unique_blocker(&mut blocking_reasons, "external_write_recorded");
    }

    let send_message_result_compatible = callsite == DefaultChatAdapterCallsite::SendMessage
        && harness.contract_shape == "send_message_compatible"
        && descriptor.descriptor_ready
        && contract.contract_ready
        && harness.harness_ready
        && descriptor.metadata_safe
        && contract.metadata_safe
        && harness.metadata_safe
        && !descriptor.contains_raw_content
        && !contract.contains_raw_content
        && !harness.contains_raw_content
        && descriptor.mapper_side_effect_free
        && descriptor.route_mode == LEGACY_STREAM_PATH
        && contract.default_send_path == LEGACY_STREAM_PATH
        && contract.start_stream_path == LEGACY_STREAM_PATH
        && harness.selected_adapter_path == LEGACY_STREAM_PATH
        && harness.candidate_adapter_path == CONTROLLED_ADAPTER_PATH
        && contract.required_callsite_path == LEGACY_STREAM_PATH
        && contract.actual_callsite_path == LEGACY_STREAM_PATH
        && !contract.controlled_adapter_enabled
        && !contract.automatic_migration_enabled
        && !harness.controlled_adapter_invocation_allowed
        && !harness.controlled_adapter_executor_enabled
        && !harness.controlled_adapter_executor_attached
        && harness.controlled_adapter_executor_state == CONTROLLED_ADAPTER_EXECUTOR_DISABLED_STATE
        && !harness.allow_writes
        && harness.max_tool_calls == 0
        && side_effect_budget_zero
        && !harness.runtime_call_enabled
        && !harness.model_call_enabled
        && !harness.tool_call_enabled
        && harness.business_write_disabled
        && !harness.migration_permission
        && !chat_message_saved
        && !agent_run_recorded
        && !evidence_recorded
        && !proposal_created
        && !memory_written
        && !life_model_written
        && !external_write_recorded
        && harness.ordinary_default_chat_path_unchanged;
    let proof_ready = send_message_result_compatible && blocking_reasons.is_empty();

    DefaultChatControlledAdapterSendCompatibleProof {
        proof_kind: "default_chat_controlled_adapter_send_compatible_proof".into(),
        callsite_kind: harness.callsite_kind,
        contract_shape: harness.contract_shape,
        send_message_result_compatible,
        proof_ready,
        descriptor_ready: descriptor.descriptor_ready,
        contract_ready: contract.contract_ready,
        harness_ready: harness.harness_ready,
        metadata_safe: descriptor.metadata_safe && contract.metadata_safe && harness.metadata_safe,
        contains_raw_content: descriptor.contains_raw_content
            || contract.contains_raw_content
            || harness.contains_raw_content,
        selected_adapter_path: harness.selected_adapter_path,
        candidate_adapter_path: harness.candidate_adapter_path,
        required_callsite_path: contract.required_callsite_path,
        actual_callsite_path: contract.actual_callsite_path,
        default_send_path: contract.default_send_path,
        start_stream_path: contract.start_stream_path,
        controlled_adapter_enabled: contract.controlled_adapter_enabled,
        automatic_migration_enabled: contract.automatic_migration_enabled,
        controlled_adapter_invocation_allowed: harness.controlled_adapter_invocation_allowed,
        controlled_adapter_executor_enabled: harness.controlled_adapter_executor_enabled,
        controlled_adapter_executor_attached: harness.controlled_adapter_executor_attached,
        controlled_adapter_executor_state: harness.controlled_adapter_executor_state,
        allow_writes: harness.allow_writes,
        max_tool_calls: harness.max_tool_calls,
        side_effect_budget: harness.side_effect_budget,
        side_effect_budget_zero,
        runtime_call_enabled: harness.runtime_call_enabled,
        model_call_enabled: harness.model_call_enabled,
        tool_call_enabled: harness.tool_call_enabled,
        business_write_disabled: harness.business_write_disabled,
        migration_permission: harness.migration_permission,
        chat_message_saved,
        agent_run_recorded,
        evidence_recorded,
        proposal_created,
        memory_written,
        life_model_written,
        external_write_recorded,
        default_chat_unchanged: harness.ordinary_default_chat_path_unchanged,
        input_length_bytes: harness.input_length_bytes,
        input_length_chars: harness.input_length_chars,
        input_sha256: harness.input_sha256,
        blocking_reasons,
    }
}

#[allow(dead_code)]
pub(crate) fn ensure_default_chat_controlled_adapter_send_compatible_proof(
    callsite: DefaultChatAdapterCallsite,
    route: &DefaultChatAdapterRoute,
    input: &str,
) -> Result<DefaultChatControlledAdapterSendCompatibleProof, String> {
    let proof =
        evaluate_default_chat_controlled_adapter_send_compatible_proof(callsite, route, input);
    if proof.proof_ready {
        Ok(proof)
    } else {
        Err(format!(
            "{} send_compatible_proof_not_ready: {}",
            proof.callsite_kind,
            proof.blocking_reasons.join(",")
        ))
    }
}

#[allow(dead_code)]
pub(crate) fn evaluate_default_chat_controlled_adapter_stream_boundary_proof(
    callsite: DefaultChatAdapterCallsite,
    route: &DefaultChatAdapterRoute,
    input: &str,
) -> DefaultChatControlledAdapterStreamBoundaryProof {
    let descriptor = describe_default_chat_controlled_adapter_candidate(callsite, route, input);
    let contract = evaluate_default_chat_controlled_adapter_contract(callsite, route, input);
    let harness =
        evaluate_default_chat_controlled_adapter_invocation_harness(callsite, route, input);
    let side_effect_budget_zero = harness.side_effect_budget.is_zero();
    let chat_message_saved = harness.side_effect_budget.chat_message_writes != 0;
    let agent_run_recorded = harness.side_effect_budget.agent_run_writes != 0;
    let evidence_recorded = harness.side_effect_budget.evidence_writes != 0;
    let proposal_created = harness.side_effect_budget.proposal_writes != 0;
    let memory_written = harness.side_effect_budget.memory_writes != 0;
    let life_model_written = harness.side_effect_budget.life_model_writes != 0;
    let mcp_audit_written = harness.side_effect_budget.mcp_audit_writes != 0;
    let external_write_recorded = harness.side_effect_budget.external_writes != 0;
    let stream_started = false;
    let stream_events_emitted = false;
    let event_channel_opened = false;
    let executor_enabled = harness.controlled_adapter_executor_enabled;
    let executor_attached = harness.controlled_adapter_executor_attached;
    let mut blocking_reasons = harness.blocking_reasons.clone();

    if callsite != DefaultChatAdapterCallsite::StartStreamMessage {
        push_unique_blocker(&mut blocking_reasons, "callsite_not_start_stream_message");
    }
    if harness.contract_shape != "stream_message_compatible" {
        push_unique_blocker(
            &mut blocking_reasons,
            "contract_shape_not_stream_message_compatible",
        );
    }
    if !descriptor.descriptor_ready {
        push_unique_blocker(&mut blocking_reasons, "descriptor_not_ready");
    }
    if !contract.contract_ready {
        push_unique_blocker(&mut blocking_reasons, "contract_not_ready");
    }
    if !harness.harness_ready {
        push_unique_blocker(&mut blocking_reasons, "harness_not_ready");
    }
    if !descriptor.metadata_safe || !contract.metadata_safe || !harness.metadata_safe {
        push_unique_blocker(&mut blocking_reasons, "metadata_not_safe");
    }
    if descriptor.contains_raw_content
        || contract.contains_raw_content
        || harness.contains_raw_content
    {
        push_unique_blocker(&mut blocking_reasons, "raw_content_present");
    }
    if !descriptor.mapper_side_effect_free {
        push_unique_blocker(&mut blocking_reasons, "mapper_not_side_effect_free");
    }
    if descriptor.route_mode != LEGACY_STREAM_PATH {
        push_unique_blocker(&mut blocking_reasons, "current_mode_not_legacy_stream");
    }
    if contract.default_send_path != LEGACY_STREAM_PATH {
        push_unique_blocker(&mut blocking_reasons, "default_send_path_not_legacy_stream");
    }
    if contract.start_stream_path != LEGACY_STREAM_PATH {
        push_unique_blocker(&mut blocking_reasons, "start_stream_path_not_legacy_stream");
    }
    if harness.selected_adapter_path != LEGACY_STREAM_PATH {
        push_unique_blocker(
            &mut blocking_reasons,
            "selected_adapter_path_not_legacy_stream",
        );
    }
    if harness.candidate_adapter_path != CONTROLLED_ADAPTER_PATH {
        push_unique_blocker(
            &mut blocking_reasons,
            "candidate_adapter_path_not_controlled_adapter",
        );
    }
    if contract.required_callsite_path != LEGACY_STREAM_PATH {
        push_unique_blocker(
            &mut blocking_reasons,
            "required_callsite_path_not_legacy_stream",
        );
    }
    if contract.actual_callsite_path != LEGACY_STREAM_PATH {
        push_unique_blocker(
            &mut blocking_reasons,
            "actual_callsite_path_not_legacy_stream",
        );
    }
    if contract.controlled_adapter_enabled {
        push_unique_blocker(&mut blocking_reasons, "controlled_adapter_enabled");
    }
    if contract.automatic_migration_enabled {
        push_unique_blocker(&mut blocking_reasons, "automatic_migration_enabled");
    }
    if harness.controlled_adapter_invocation_allowed {
        push_unique_blocker(
            &mut blocking_reasons,
            "controlled_adapter_invocation_allowed",
        );
    }
    if stream_started {
        push_unique_blocker(&mut blocking_reasons, "stream_started");
    }
    if event_channel_opened {
        push_unique_blocker(&mut blocking_reasons, "event_channel_opened");
    }
    if stream_events_emitted {
        push_unique_blocker(&mut blocking_reasons, "stream_events_emitted");
    }
    if executor_enabled {
        push_unique_blocker(&mut blocking_reasons, "executor_enabled");
    }
    if executor_attached {
        push_unique_blocker(&mut blocking_reasons, "executor_attached");
    }
    if harness.controlled_adapter_executor_state != CONTROLLED_ADAPTER_EXECUTOR_DISABLED_STATE {
        push_unique_blocker(&mut blocking_reasons, "executor_not_disabled_unattached");
    }
    if harness.allow_writes {
        push_unique_blocker(&mut blocking_reasons, "allow_writes_enabled");
    }
    if harness.max_tool_calls != 0 {
        push_unique_blocker(&mut blocking_reasons, "max_tool_calls_not_zero");
    }
    if !side_effect_budget_zero {
        push_unique_blocker(&mut blocking_reasons, "side_effect_budget_not_zero");
    }
    if harness.runtime_call_enabled {
        push_unique_blocker(&mut blocking_reasons, "runtime_call_enabled");
    }
    if harness.model_call_enabled {
        push_unique_blocker(&mut blocking_reasons, "model_call_enabled");
    }
    if harness.tool_call_enabled {
        push_unique_blocker(&mut blocking_reasons, "tool_call_enabled");
    }
    if !harness.business_write_disabled {
        push_unique_blocker(&mut blocking_reasons, "business_write_enabled");
    }
    if harness.migration_permission {
        push_unique_blocker(&mut blocking_reasons, "migration_permission_enabled");
    }
    if !harness.ordinary_default_chat_path_unchanged {
        push_unique_blocker(&mut blocking_reasons, "default_chat_not_legacy_stream");
    }
    if chat_message_saved {
        push_unique_blocker(&mut blocking_reasons, "chat_message_saved");
    }
    if agent_run_recorded {
        push_unique_blocker(&mut blocking_reasons, "agent_run_recorded");
    }
    if evidence_recorded {
        push_unique_blocker(&mut blocking_reasons, "evidence_recorded");
    }
    if proposal_created {
        push_unique_blocker(&mut blocking_reasons, "proposal_created");
    }
    if memory_written {
        push_unique_blocker(&mut blocking_reasons, "memory_written");
    }
    if life_model_written {
        push_unique_blocker(&mut blocking_reasons, "life_model_written");
    }
    if mcp_audit_written {
        push_unique_blocker(&mut blocking_reasons, "mcp_audit_written");
    }
    if external_write_recorded {
        push_unique_blocker(&mut blocking_reasons, "external_write_recorded");
    }

    let stream_message_compatible = callsite == DefaultChatAdapterCallsite::StartStreamMessage
        && harness.contract_shape == "stream_message_compatible"
        && descriptor.descriptor_ready
        && contract.contract_ready
        && harness.harness_ready
        && descriptor.metadata_safe
        && contract.metadata_safe
        && harness.metadata_safe
        && !descriptor.contains_raw_content
        && !contract.contains_raw_content
        && !harness.contains_raw_content
        && descriptor.mapper_side_effect_free
        && descriptor.route_mode == LEGACY_STREAM_PATH
        && contract.default_send_path == LEGACY_STREAM_PATH
        && contract.start_stream_path == LEGACY_STREAM_PATH
        && harness.selected_adapter_path == LEGACY_STREAM_PATH
        && harness.candidate_adapter_path == CONTROLLED_ADAPTER_PATH
        && contract.required_callsite_path == LEGACY_STREAM_PATH
        && contract.actual_callsite_path == LEGACY_STREAM_PATH
        && !contract.controlled_adapter_enabled
        && !contract.automatic_migration_enabled
        && !harness.controlled_adapter_invocation_allowed
        && !stream_started
        && !event_channel_opened
        && !stream_events_emitted
        && !executor_enabled
        && !executor_attached
        && harness.controlled_adapter_executor_state == CONTROLLED_ADAPTER_EXECUTOR_DISABLED_STATE
        && !harness.allow_writes
        && harness.max_tool_calls == 0
        && side_effect_budget_zero
        && !harness.runtime_call_enabled
        && !harness.model_call_enabled
        && !harness.tool_call_enabled
        && harness.business_write_disabled
        && !harness.migration_permission
        && !chat_message_saved
        && !agent_run_recorded
        && !evidence_recorded
        && !proposal_created
        && !memory_written
        && !life_model_written
        && !mcp_audit_written
        && !external_write_recorded
        && harness.ordinary_default_chat_path_unchanged;
    let proof_ready = stream_message_compatible && blocking_reasons.is_empty();

    DefaultChatControlledAdapterStreamBoundaryProof {
        proof_kind: "default_chat_controlled_adapter_stream_boundary_proof".into(),
        callsite_kind: harness.callsite_kind,
        contract_shape: harness.contract_shape,
        stream_message_compatible,
        proof_ready,
        descriptor_ready: descriptor.descriptor_ready,
        contract_ready: contract.contract_ready,
        harness_ready: harness.harness_ready,
        metadata_safe: descriptor.metadata_safe && contract.metadata_safe && harness.metadata_safe,
        contains_raw_content: descriptor.contains_raw_content
            || contract.contains_raw_content
            || harness.contains_raw_content,
        selected_adapter_path: harness.selected_adapter_path,
        candidate_adapter_path: harness.candidate_adapter_path,
        required_callsite_path: contract.required_callsite_path,
        actual_callsite_path: contract.actual_callsite_path,
        default_send_path: contract.default_send_path,
        start_stream_path: contract.start_stream_path,
        controlled_adapter_enabled: contract.controlled_adapter_enabled,
        automatic_migration_enabled: contract.automatic_migration_enabled,
        controlled_adapter_invocation_allowed: harness.controlled_adapter_invocation_allowed,
        stream_started,
        stream_events_emitted,
        event_channel_opened,
        executor_enabled,
        executor_attached,
        executor_state: harness.controlled_adapter_executor_state,
        allow_writes: harness.allow_writes,
        max_tool_calls: harness.max_tool_calls,
        side_effect_budget: harness.side_effect_budget,
        side_effect_budget_zero,
        runtime_call_enabled: harness.runtime_call_enabled,
        model_call_enabled: harness.model_call_enabled,
        tool_call_enabled: harness.tool_call_enabled,
        business_write_disabled: harness.business_write_disabled,
        migration_permission: harness.migration_permission,
        chat_message_saved,
        agent_run_recorded,
        evidence_recorded,
        proposal_created,
        memory_written,
        life_model_written,
        mcp_audit_written,
        external_write_recorded,
        default_chat_unchanged: harness.ordinary_default_chat_path_unchanged,
        input_length_bytes: harness.input_length_bytes,
        input_length_chars: harness.input_length_chars,
        input_sha256: harness.input_sha256,
        blocking_reasons,
    }
}

#[allow(dead_code)]
pub(crate) fn ensure_default_chat_controlled_adapter_stream_boundary_proof(
    callsite: DefaultChatAdapterCallsite,
    route: &DefaultChatAdapterRoute,
    input: &str,
) -> Result<DefaultChatControlledAdapterStreamBoundaryProof, String> {
    let proof =
        evaluate_default_chat_controlled_adapter_stream_boundary_proof(callsite, route, input);
    if proof.proof_ready {
        Ok(proof)
    } else {
        Err(format!(
            "{} stream_boundary_proof_not_ready: {}",
            proof.callsite_kind,
            proof.blocking_reasons.join(",")
        ))
    }
}

#[allow(dead_code)]
pub(crate) fn evaluate_default_chat_controlled_adapter_executor_attachment_gate(
    route: &DefaultChatAdapterRoute,
    input: &str,
) -> DefaultChatControlledAdapterExecutorAttachmentGateReport {
    let send_descriptor = describe_default_chat_controlled_adapter_candidate(
        DefaultChatAdapterCallsite::SendMessage,
        route,
        input,
    );
    let send_contract = evaluate_default_chat_controlled_adapter_contract(
        DefaultChatAdapterCallsite::SendMessage,
        route,
        input,
    );
    let send_harness = evaluate_default_chat_controlled_adapter_invocation_harness(
        DefaultChatAdapterCallsite::SendMessage,
        route,
        input,
    );
    let send_proof = evaluate_default_chat_controlled_adapter_send_compatible_proof(
        DefaultChatAdapterCallsite::SendMessage,
        route,
        input,
    );
    let stream_descriptor = describe_default_chat_controlled_adapter_candidate(
        DefaultChatAdapterCallsite::StartStreamMessage,
        route,
        input,
    );
    let stream_contract = evaluate_default_chat_controlled_adapter_contract(
        DefaultChatAdapterCallsite::StartStreamMessage,
        route,
        input,
    );
    let stream_harness = evaluate_default_chat_controlled_adapter_invocation_harness(
        DefaultChatAdapterCallsite::StartStreamMessage,
        route,
        input,
    );
    let stream_proof = evaluate_default_chat_controlled_adapter_stream_boundary_proof(
        DefaultChatAdapterCallsite::StartStreamMessage,
        route,
        input,
    );
    let side_effect_budget = default_chat_adapter_sum_side_effect_budgets(&[
        &send_descriptor.side_effect_budget,
        &send_contract.side_effect_budget,
        &send_harness.side_effect_budget,
        &send_proof.side_effect_budget,
        &stream_descriptor.side_effect_budget,
        &stream_contract.side_effect_budget,
        &stream_harness.side_effect_budget,
        &stream_proof.side_effect_budget,
    ]);
    let side_effect_budget_zero = side_effect_budget.is_zero();
    let send_descriptor_ready = send_descriptor.descriptor_ready;
    let send_contract_ready = send_contract.contract_ready;
    let send_harness_ready = send_harness.harness_ready;
    let stream_descriptor_ready = stream_descriptor.descriptor_ready;
    let stream_contract_ready = stream_contract.contract_ready;
    let stream_harness_ready = stream_harness.harness_ready;
    let w65_w67_metadata_safe = send_descriptor.metadata_safe
        && send_contract.metadata_safe
        && send_harness.metadata_safe
        && stream_descriptor.metadata_safe
        && stream_contract.metadata_safe
        && stream_harness.metadata_safe;
    let metadata_safe =
        w65_w67_metadata_safe && send_proof.metadata_safe && stream_proof.metadata_safe;
    let contains_raw_content = send_descriptor.contains_raw_content
        || send_contract.contains_raw_content
        || send_harness.contains_raw_content
        || send_proof.contains_raw_content
        || stream_descriptor.contains_raw_content
        || stream_contract.contains_raw_content
        || stream_harness.contains_raw_content
        || stream_proof.contains_raw_content;
    let send_proof_ready = send_proof.proof_ready;
    let stream_boundary_proof_ready = stream_proof.proof_ready;
    let selected_adapter_path = if send_proof.selected_adapter_path == LEGACY_STREAM_PATH
        && stream_proof.selected_adapter_path == LEGACY_STREAM_PATH
    {
        LEGACY_STREAM_PATH
    } else {
        "blocked"
    }
    .into();
    let candidate_adapter_path = if send_proof.candidate_adapter_path == CONTROLLED_ADAPTER_PATH
        && stream_proof.candidate_adapter_path == CONTROLLED_ADAPTER_PATH
    {
        CONTROLLED_ADAPTER_PATH
    } else {
        "blocked"
    }
    .into();
    let controlled_adapter_enabled = route.controlled_adapter_enabled
        || send_proof.controlled_adapter_enabled
        || stream_proof.controlled_adapter_enabled;
    let automatic_migration_enabled = route.automatic_migration_enabled
        || send_proof.automatic_migration_enabled
        || stream_proof.automatic_migration_enabled;
    let controlled_adapter_invocation_allowed = send_proof.controlled_adapter_invocation_allowed
        || stream_proof.controlled_adapter_invocation_allowed
        || send_harness.controlled_adapter_invocation_allowed
        || stream_harness.controlled_adapter_invocation_allowed;
    let executor_enabled =
        send_proof.controlled_adapter_executor_enabled || stream_proof.executor_enabled;
    let executor_attached =
        send_proof.controlled_adapter_executor_attached || stream_proof.executor_attached;
    let executor_implementation_present = false;
    let human_review_present = false;
    let route_cutover_permission = false;
    let migration_permission = false;
    let ordinary_default_chat_unchanged =
        send_proof.default_chat_unchanged && stream_proof.default_chat_unchanged;
    let stream_started = stream_proof.stream_started;
    let stream_events_emitted = stream_proof.stream_events_emitted;
    let event_channel_opened = stream_proof.event_channel_opened;
    let allow_writes = send_proof.allow_writes
        || stream_proof.allow_writes
        || send_harness.allow_writes
        || stream_harness.allow_writes;
    let max_tool_calls = send_proof
        .max_tool_calls
        .max(stream_proof.max_tool_calls)
        .max(send_harness.max_tool_calls)
        .max(stream_harness.max_tool_calls);
    let runtime_call_enabled = side_effect_budget.runtime_calls != 0
        || send_proof.runtime_call_enabled
        || stream_proof.runtime_call_enabled
        || send_harness.runtime_call_enabled
        || stream_harness.runtime_call_enabled;
    let model_call_enabled = side_effect_budget.model_calls != 0
        || send_proof.model_call_enabled
        || stream_proof.model_call_enabled
        || send_harness.model_call_enabled
        || stream_harness.model_call_enabled;
    let tool_call_enabled = side_effect_budget.tool_calls != 0
        || send_proof.tool_call_enabled
        || stream_proof.tool_call_enabled
        || send_harness.tool_call_enabled
        || stream_harness.tool_call_enabled;
    let business_write_disabled = !allow_writes
        && side_effect_budget.store_writes == 0
        && side_effect_budget.chat_message_writes == 0
        && side_effect_budget.agent_run_writes == 0
        && side_effect_budget.evidence_writes == 0
        && side_effect_budget.proposal_writes == 0
        && side_effect_budget.memory_writes == 0
        && side_effect_budget.life_model_writes == 0
        && side_effect_budget.mcp_audit_writes == 0
        && side_effect_budget.external_writes == 0
        && send_proof.business_write_disabled
        && stream_proof.business_write_disabled
        && send_harness.business_write_disabled
        && stream_harness.business_write_disabled;
    let chat_message_saved = side_effect_budget.chat_message_writes != 0
        || send_proof.chat_message_saved
        || stream_proof.chat_message_saved;
    let agent_run_recorded = side_effect_budget.agent_run_writes != 0
        || send_proof.agent_run_recorded
        || stream_proof.agent_run_recorded;
    let evidence_recorded = side_effect_budget.evidence_writes != 0
        || send_proof.evidence_recorded
        || stream_proof.evidence_recorded;
    let proposal_created = side_effect_budget.proposal_writes != 0
        || send_proof.proposal_created
        || stream_proof.proposal_created;
    let memory_written = side_effect_budget.memory_writes != 0
        || send_proof.memory_written
        || stream_proof.memory_written;
    let life_model_written = side_effect_budget.life_model_writes != 0
        || send_proof.life_model_written
        || stream_proof.life_model_written;
    let mcp_audit_written =
        side_effect_budget.mcp_audit_writes != 0 || stream_proof.mcp_audit_written;
    let external_write_recorded = side_effect_budget.external_writes != 0
        || send_proof.external_write_recorded
        || stream_proof.external_write_recorded;
    let mut blocking_reasons = Vec::new();

    for reason in send_descriptor
        .blocking_reasons
        .iter()
        .chain(send_contract.blocking_reasons.iter())
        .chain(send_harness.blocking_reasons.iter())
        .chain(send_proof.blocking_reasons.iter())
        .chain(stream_descriptor.blocking_reasons.iter())
        .chain(stream_contract.blocking_reasons.iter())
        .chain(stream_harness.blocking_reasons.iter())
        .chain(stream_proof.blocking_reasons.iter())
    {
        push_unique_blocker(&mut blocking_reasons, reason);
    }
    if !send_proof_ready {
        push_unique_blocker(&mut blocking_reasons, "send_proof_not_ready");
    }
    if !stream_boundary_proof_ready {
        push_unique_blocker(&mut blocking_reasons, "stream_boundary_proof_not_ready");
    }
    if !(send_descriptor_ready
        && send_contract_ready
        && send_harness_ready
        && stream_descriptor_ready
        && stream_contract_ready
        && stream_harness_ready)
    {
        push_unique_blocker(&mut blocking_reasons, "metadata_safe_layers_not_ready");
    }
    if !metadata_safe {
        push_unique_blocker(&mut blocking_reasons, "metadata_not_safe");
    }
    if contains_raw_content {
        push_unique_blocker(&mut blocking_reasons, "raw_content_present");
    }
    if route.current_mode != LEGACY_STREAM_PATH
        || route.default_send_path != LEGACY_STREAM_PATH
        || route.start_stream_path != LEGACY_STREAM_PATH
    {
        push_unique_blocker(&mut blocking_reasons, "route_drift_from_legacy_stream");
    }
    if selected_adapter_path != LEGACY_STREAM_PATH {
        push_unique_blocker(
            &mut blocking_reasons,
            "selected_adapter_path_not_legacy_stream",
        );
    }
    if controlled_adapter_enabled {
        push_unique_blocker(&mut blocking_reasons, "controlled_adapter_enabled");
    }
    if automatic_migration_enabled {
        push_unique_blocker(&mut blocking_reasons, "automatic_migration_enabled");
    }
    if controlled_adapter_invocation_allowed {
        push_unique_blocker(
            &mut blocking_reasons,
            "controlled_adapter_invocation_allowed",
        );
    }
    if executor_enabled {
        push_unique_blocker(&mut blocking_reasons, "executor_enabled");
    }
    if executor_attached {
        push_unique_blocker(&mut blocking_reasons, "executor_attached");
    }
    if !executor_implementation_present {
        push_unique_blocker(&mut blocking_reasons, "executor_implementation_missing");
    }
    if !human_review_present {
        push_unique_blocker(&mut blocking_reasons, "human_review_missing");
    }
    if !route_cutover_permission {
        push_unique_blocker(&mut blocking_reasons, "route_cutover_not_authorized");
    }
    if migration_permission || send_proof.migration_permission || stream_proof.migration_permission
    {
        push_unique_blocker(&mut blocking_reasons, "migration_permission_enabled");
    }
    if stream_started {
        push_unique_blocker(&mut blocking_reasons, "stream_started");
    }
    if event_channel_opened {
        push_unique_blocker(&mut blocking_reasons, "event_channel_opened");
    }
    if stream_events_emitted {
        push_unique_blocker(&mut blocking_reasons, "stream_events_emitted");
    }
    if allow_writes {
        push_unique_blocker(&mut blocking_reasons, "allow_writes_enabled");
    }
    if max_tool_calls != 0 {
        push_unique_blocker(&mut blocking_reasons, "max_tool_calls_not_zero");
    }
    if !side_effect_budget_zero {
        push_unique_blocker(&mut blocking_reasons, "side_effect_budget_not_zero");
    }
    if runtime_call_enabled {
        push_unique_blocker(&mut blocking_reasons, "runtime_call_enabled");
    }
    if model_call_enabled {
        push_unique_blocker(&mut blocking_reasons, "model_call_enabled");
    }
    if tool_call_enabled {
        push_unique_blocker(&mut blocking_reasons, "tool_call_enabled");
    }
    if !business_write_disabled {
        push_unique_blocker(&mut blocking_reasons, "business_write_enabled");
    }
    if chat_message_saved {
        push_unique_blocker(&mut blocking_reasons, "chat_message_saved");
    }
    if agent_run_recorded {
        push_unique_blocker(&mut blocking_reasons, "agent_run_recorded");
    }
    if evidence_recorded {
        push_unique_blocker(&mut blocking_reasons, "evidence_recorded");
    }
    if proposal_created {
        push_unique_blocker(&mut blocking_reasons, "proposal_created");
    }
    if memory_written {
        push_unique_blocker(&mut blocking_reasons, "memory_written");
    }
    if life_model_written {
        push_unique_blocker(&mut blocking_reasons, "life_model_written");
    }
    if mcp_audit_written {
        push_unique_blocker(&mut blocking_reasons, "mcp_audit_written");
    }
    if external_write_recorded {
        push_unique_blocker(&mut blocking_reasons, "external_write_recorded");
    }
    if !ordinary_default_chat_unchanged {
        push_unique_blocker(&mut blocking_reasons, "default_chat_not_legacy_stream");
    }

    let gate_report_metadata_ready = send_proof_ready
        && stream_boundary_proof_ready
        && send_descriptor_ready
        && send_contract_ready
        && send_harness_ready
        && stream_descriptor_ready
        && stream_contract_ready
        && stream_harness_ready
        && w65_w67_metadata_safe
        && metadata_safe
        && !contains_raw_content
        && selected_adapter_path == LEGACY_STREAM_PATH
        && !controlled_adapter_enabled
        && !automatic_migration_enabled
        && !controlled_adapter_invocation_allowed
        && !executor_enabled
        && !executor_attached
        && !migration_permission
        && !stream_started
        && !event_channel_opened
        && !stream_events_emitted
        && !allow_writes
        && max_tool_calls == 0
        && side_effect_budget_zero
        && !runtime_call_enabled
        && !model_call_enabled
        && !tool_call_enabled
        && business_write_disabled
        && !chat_message_saved
        && !agent_run_recorded
        && !evidence_recorded
        && !proposal_created
        && !memory_written
        && !life_model_written
        && !mcp_audit_written
        && !external_write_recorded
        && ordinary_default_chat_unchanged;
    let executor_skeleton_discussion_ready = gate_report_metadata_ready;

    DefaultChatControlledAdapterExecutorAttachmentGateReport {
        report_kind: "default_chat_controlled_adapter_executor_attachment_gate_report".into(),
        gate_report_metadata_ready,
        executor_skeleton_discussion_ready,
        executor_attachment_allowed: false,
        executor_enabled,
        executor_attached,
        executor_implementation_present,
        human_review_present,
        route_cutover_permission,
        migration_permission,
        ordinary_default_chat_unchanged,
        selected_adapter_path,
        candidate_adapter_path,
        default_send_path: route.default_send_path.clone(),
        start_stream_path: route.start_stream_path.clone(),
        controlled_adapter_enabled,
        automatic_migration_enabled,
        controlled_adapter_invocation_allowed,
        send_message_result_compatible: send_proof.send_message_result_compatible,
        stream_message_compatible: stream_proof.stream_message_compatible,
        send_proof_ready,
        stream_boundary_proof_ready,
        w68_send_compatible_proof_ready: send_proof_ready,
        w69_stream_boundary_proof_ready: stream_boundary_proof_ready,
        send_descriptor_ready,
        send_contract_ready,
        send_harness_ready,
        stream_descriptor_ready,
        stream_contract_ready,
        stream_harness_ready,
        w65_w67_metadata_safe,
        metadata_safe,
        contains_raw_content,
        stream_started,
        stream_events_emitted,
        event_channel_opened,
        allow_writes,
        max_tool_calls,
        side_effect_budget,
        side_effect_budget_zero,
        runtime_call_enabled,
        model_call_enabled,
        tool_call_enabled,
        business_write_disabled,
        chat_message_saved,
        agent_run_recorded,
        evidence_recorded,
        proposal_created,
        memory_written,
        life_model_written,
        mcp_audit_written,
        external_write_recorded,
        input_length_bytes: send_proof.input_length_bytes,
        input_length_chars: send_proof.input_length_chars,
        input_sha256: send_proof.input_sha256,
        blocking_reasons,
    }
}

#[allow(dead_code)]
pub(crate) fn ensure_default_chat_controlled_adapter_executor_attachment_gate(
    route: &DefaultChatAdapterRoute,
    input: &str,
) -> Result<DefaultChatControlledAdapterExecutorAttachmentGateReport, String> {
    let report = evaluate_default_chat_controlled_adapter_executor_attachment_gate(route, input);
    if report.executor_attachment_allowed && report.blocking_reasons.is_empty() {
        Ok(report)
    } else {
        Err(format!(
            "default_chat_controlled_adapter_executor_attachment_gate_not_ready: {}",
            report.blocking_reasons.join(",")
        ))
    }
}

#[allow(dead_code)]
pub(crate) fn evaluate_default_chat_controlled_adapter_disabled_executor_skeleton(
    input: &DefaultChatControlledAdapterExecutorSkeletonInput,
    gate_report: &DefaultChatControlledAdapterExecutorAttachmentGateReport,
) -> DefaultChatControlledAdapterDisabledExecutorSkeleton {
    let executor_skeleton_present = true;
    let executor_enabled = false;
    let executor_attached = false;
    let executor_runnable = false;
    let invocation_allowed = false;
    let route_cutover_permission = false;
    let migration_permission = false;
    let selected_adapter_path = LEGACY_STREAM_PATH.to_string();
    let side_effect_budget = DefaultChatAdapterDescriptorSideEffectBudget::zero();
    let side_effect_budget_zero = side_effect_budget.is_zero();
    let compatible_shape =
        default_chat_executor_skeleton_compatible_shape(&input.requested_shape).to_string();
    let requested_shape_known = compatible_shape != "blocked";
    let route_clean = input.route_current_mode == LEGACY_STREAM_PATH
        && input.route_default_send_path == LEGACY_STREAM_PATH
        && input.route_start_stream_path == LEGACY_STREAM_PATH
        && !input.route_controlled_adapter_enabled
        && !input.route_automatic_migration_enabled
        && gate_report.default_send_path == LEGACY_STREAM_PATH
        && gate_report.start_stream_path == LEGACY_STREAM_PATH;
    let ordinary_default_chat_unchanged =
        route_clean && gate_report.ordinary_default_chat_unchanged;
    let controlled_adapter_enabled =
        input.route_controlled_adapter_enabled || gate_report.controlled_adapter_enabled;
    let automatic_migration_enabled =
        input.route_automatic_migration_enabled || gate_report.automatic_migration_enabled;
    let metadata_safe = gate_report.metadata_safe && !gate_report.contains_raw_content;
    let contains_raw_content = gate_report.contains_raw_content;
    let stream_started = false;
    let event_channel_opened = false;
    let stream_events_emitted = false;
    let runtime_call_enabled = false;
    let model_call_enabled = false;
    let tool_call_enabled = false;
    let allow_writes = false;
    let max_tool_calls = 0;
    let business_write_disabled = true;
    let chat_message_saved = false;
    let agent_run_recorded = false;
    let evidence_recorded = false;
    let proposal_created = false;
    let memory_written = false;
    let life_model_written = false;
    let mcp_audit_written = false;
    let external_write_recorded = false;
    let mut blocking_reasons = Vec::new();

    if !gate_report.gate_report_metadata_ready {
        push_unique_blocker(&mut blocking_reasons, "w70_gate_report_not_metadata_ready");
    }
    if gate_report.executor_attachment_allowed {
        push_unique_blocker(
            &mut blocking_reasons,
            "executor_attachment_allowed_unexpectedly",
        );
    }
    if gate_report.executor_enabled {
        push_unique_blocker(&mut blocking_reasons, "executor_enabled");
    }
    if gate_report.executor_attached {
        push_unique_blocker(&mut blocking_reasons, "executor_attached");
    }
    if executor_runnable {
        push_unique_blocker(&mut blocking_reasons, "executor_runnable");
    }
    if controlled_adapter_enabled {
        push_unique_blocker(&mut blocking_reasons, "controlled_adapter_enabled");
    }
    if automatic_migration_enabled {
        push_unique_blocker(&mut blocking_reasons, "automatic_migration_enabled");
    }
    if !route_clean {
        push_unique_blocker(&mut blocking_reasons, "route_drift_from_legacy_stream");
    }
    if gate_report.controlled_adapter_invocation_allowed {
        push_unique_blocker(&mut blocking_reasons, "invocation_allowed_unexpectedly");
    }
    if gate_report.runtime_call_enabled || gate_report.side_effect_budget.runtime_calls != 0 {
        push_unique_blocker(&mut blocking_reasons, "runtime_budget_not_zero");
    }
    if gate_report.model_call_enabled || gate_report.side_effect_budget.model_calls != 0 {
        push_unique_blocker(&mut blocking_reasons, "model_budget_not_zero");
    }
    if gate_report.tool_call_enabled
        || gate_report.side_effect_budget.tool_calls != 0
        || gate_report.max_tool_calls != 0
    {
        push_unique_blocker(&mut blocking_reasons, "tool_budget_not_zero");
    }
    if default_chat_adapter_write_budget_nonzero(gate_report) {
        push_unique_blocker(&mut blocking_reasons, "write_budget_not_zero");
    }
    if gate_report.stream_started {
        push_unique_blocker(&mut blocking_reasons, "stream_started");
    }
    if gate_report.event_channel_opened {
        push_unique_blocker(&mut blocking_reasons, "event_channel_opened");
    }
    if gate_report.stream_events_emitted {
        push_unique_blocker(&mut blocking_reasons, "stream_events_emitted");
    }
    if !gate_report.metadata_safe {
        push_unique_blocker(&mut blocking_reasons, "metadata_not_safe");
    }
    if gate_report.contains_raw_content {
        push_unique_blocker(&mut blocking_reasons, "raw_content_present");
    }
    if !requested_shape_known {
        push_unique_blocker(&mut blocking_reasons, "unknown_requested_shape");
    }

    let skeleton_contract_ready = executor_skeleton_present
        && gate_report.gate_report_metadata_ready
        && metadata_safe
        && !contains_raw_content
        && !gate_report.executor_attachment_allowed
        && !gate_report.executor_enabled
        && !gate_report.executor_attached
        && !executor_enabled
        && !executor_attached
        && !executor_runnable
        && !invocation_allowed
        && !route_cutover_permission
        && !migration_permission
        && selected_adapter_path == LEGACY_STREAM_PATH
        && ordinary_default_chat_unchanged
        && requested_shape_known
        && !controlled_adapter_enabled
        && !automatic_migration_enabled
        && !gate_report.controlled_adapter_invocation_allowed
        && !gate_report.runtime_call_enabled
        && !gate_report.model_call_enabled
        && !gate_report.tool_call_enabled
        && gate_report.max_tool_calls == 0
        && gate_report.side_effect_budget.is_zero()
        && side_effect_budget_zero
        && !gate_report.stream_started
        && !gate_report.event_channel_opened
        && !gate_report.stream_events_emitted
        && !gate_report.allow_writes
        && !default_chat_adapter_write_budget_nonzero(gate_report)
        && !stream_started
        && !event_channel_opened
        && !stream_events_emitted
        && !runtime_call_enabled
        && !model_call_enabled
        && !tool_call_enabled
        && !allow_writes
        && max_tool_calls == 0
        && business_write_disabled
        && !chat_message_saved
        && !agent_run_recorded
        && !evidence_recorded
        && !proposal_created
        && !memory_written
        && !life_model_written
        && !mcp_audit_written
        && !external_write_recorded
        && blocking_reasons.is_empty();
    let output = DefaultChatControlledAdapterExecutorSkeletonOutput {
        output_kind: "default_chat_controlled_adapter_disabled_executor_skeleton_output".into(),
        compatible_shape,
        executor_state: CONTROLLED_ADAPTER_EXECUTOR_DISABLED_STATE.into(),
        no_user_visible_output: true,
        raw_output_present: false,
        blocking_reasons: blocking_reasons.clone(),
    };

    DefaultChatControlledAdapterDisabledExecutorSkeleton {
        skeleton_kind: "default_chat_controlled_adapter_disabled_executor_skeleton".into(),
        skeleton_contract_ready,
        metadata_safe,
        contains_raw_content,
        executor_skeleton_present,
        gate_report_metadata_ready: gate_report.gate_report_metadata_ready,
        executor_attachment_allowed: gate_report.executor_attachment_allowed,
        executor_enabled,
        executor_attached,
        executor_runnable,
        invocation_allowed,
        route_cutover_permission,
        migration_permission,
        ordinary_default_chat_unchanged,
        selected_adapter_path,
        callsite_kind: input.callsite_kind.clone(),
        requested_shape: input.requested_shape.clone(),
        controlled_adapter_enabled,
        automatic_migration_enabled,
        stream_started,
        event_channel_opened,
        stream_events_emitted,
        runtime_call_enabled,
        model_call_enabled,
        tool_call_enabled,
        allow_writes,
        max_tool_calls,
        side_effect_budget,
        side_effect_budget_zero,
        business_write_disabled,
        chat_message_saved,
        agent_run_recorded,
        evidence_recorded,
        proposal_created,
        memory_written,
        life_model_written,
        mcp_audit_written,
        external_write_recorded,
        input_length_bytes: input.input_length_bytes,
        input_length_chars: input.input_length_chars,
        input_sha256: input.input_sha256.clone(),
        output,
        blocking_reasons,
    }
}

#[allow(dead_code)]
pub(crate) fn ensure_default_chat_controlled_adapter_disabled_executor_skeleton(
    input: &DefaultChatControlledAdapterExecutorSkeletonInput,
    gate_report: &DefaultChatControlledAdapterExecutorAttachmentGateReport,
) -> Result<DefaultChatControlledAdapterDisabledExecutorSkeleton, String> {
    let skeleton =
        evaluate_default_chat_controlled_adapter_disabled_executor_skeleton(input, gate_report);
    if skeleton.skeleton_contract_ready {
        Ok(skeleton)
    } else {
        Err(format!(
            "default_chat_controlled_adapter_disabled_executor_skeleton_not_ready: {}",
            skeleton.blocking_reasons.join(",")
        ))
    }
}

fn default_chat_executor_skeleton_requested_shape(requested_shape: &str) -> &'static str {
    match requested_shape {
        DEFAULT_CHAT_EXECUTOR_SKELETON_SEND_MESSAGE_RESULT_SHAPE => {
            DEFAULT_CHAT_EXECUTOR_SKELETON_SEND_MESSAGE_RESULT_SHAPE
        }
        DEFAULT_CHAT_EXECUTOR_SKELETON_STREAM_BOUNDARY_SHAPE => {
            DEFAULT_CHAT_EXECUTOR_SKELETON_STREAM_BOUNDARY_SHAPE
        }
        _ => DEFAULT_CHAT_EXECUTOR_SKELETON_UNKNOWN_SHAPE,
    }
}

fn default_chat_executor_skeleton_compatible_shape(requested_shape: &str) -> &'static str {
    match requested_shape {
        DEFAULT_CHAT_EXECUTOR_SKELETON_SEND_MESSAGE_RESULT_SHAPE => {
            DEFAULT_CHAT_EXECUTOR_SKELETON_SEND_MESSAGE_RESULT_SHAPE
        }
        DEFAULT_CHAT_EXECUTOR_SKELETON_STREAM_BOUNDARY_SHAPE => {
            DEFAULT_CHAT_EXECUTOR_SKELETON_STREAM_BOUNDARY_SHAPE
        }
        _ => "blocked",
    }
}

fn default_chat_adapter_write_budget_nonzero(
    gate_report: &DefaultChatControlledAdapterExecutorAttachmentGateReport,
) -> bool {
    gate_report.allow_writes
        || gate_report.side_effect_budget.store_writes != 0
        || gate_report.side_effect_budget.chat_message_writes != 0
        || gate_report.side_effect_budget.agent_run_writes != 0
        || gate_report.side_effect_budget.evidence_writes != 0
        || gate_report.side_effect_budget.proposal_writes != 0
        || gate_report.side_effect_budget.memory_writes != 0
        || gate_report.side_effect_budget.life_model_writes != 0
        || gate_report.side_effect_budget.mcp_audit_writes != 0
        || gate_report.side_effect_budget.external_writes != 0
        || gate_report.chat_message_saved
        || gate_report.agent_run_recorded
        || gate_report.evidence_recorded
        || gate_report.proposal_created
        || gate_report.memory_written
        || gate_report.life_model_written
        || gate_report.mcp_audit_written
        || gate_report.external_write_recorded
}

fn default_chat_adapter_sum_side_effect_budgets(
    budgets: &[&DefaultChatAdapterDescriptorSideEffectBudget],
) -> DefaultChatAdapterDescriptorSideEffectBudget {
    let mut total = DefaultChatAdapterDescriptorSideEffectBudget::zero();

    for budget in budgets {
        total.runtime_calls += budget.runtime_calls;
        total.model_calls += budget.model_calls;
        total.tool_calls += budget.tool_calls;
        total.store_writes += budget.store_writes;
        total.chat_message_writes += budget.chat_message_writes;
        total.agent_run_writes += budget.agent_run_writes;
        total.evidence_writes += budget.evidence_writes;
        total.proposal_writes += budget.proposal_writes;
        total.memory_writes += budget.memory_writes;
        total.life_model_writes += budget.life_model_writes;
        total.mcp_audit_writes += budget.mcp_audit_writes;
        total.external_writes += budget.external_writes;
    }

    total
}

fn push_unique_blocker(blocking_reasons: &mut Vec<String>, reason: &str) {
    if !blocking_reasons.iter().any(|existing| existing == reason) {
        blocking_reasons.push(reason.to_string());
    }
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
