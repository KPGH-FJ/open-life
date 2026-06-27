use std::sync::Arc;

use openlife_core::agent::main_chat_agent_v1::MainChatAgentStrategy;
use openlife_core::llm::ChatMessage;
use serde::{Deserialize, Serialize};

use crate::main_chat_kernel::{
    main_chat_kernel_support_disposition,
    main_chat_live_provider_eval_requires_provider_backed_react,
    main_chat_react_turn_requires_governed_agent_loop_candidate_selection,
    MainChatKernelSupportDisposition,
};
use crate::AppState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum MainChatExecutionPath {
    KernelDirect,
    KernelReadTool,
    KernelWriteOutcome,
    ToolLoop,
    PlanExecute,
    GovernedBlocker,
    LegacyCompatFallback,
}

impl MainChatExecutionPath {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::KernelDirect => "KernelDirect",
            Self::KernelReadTool => "KernelReadTool",
            Self::KernelWriteOutcome => "KernelWriteOutcome",
            Self::ToolLoop => "ToolLoop",
            Self::PlanExecute => "PlanExecute",
            Self::GovernedBlocker => "GovernedBlocker",
            Self::LegacyCompatFallback => "LegacyCompatFallback",
        }
    }

    pub(crate) fn is_kernel_dispatch(self) -> bool {
        matches!(
            self,
            Self::KernelDirect
                | Self::KernelReadTool
                | Self::KernelWriteOutcome
                | Self::PlanExecute
                | Self::GovernedBlocker
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MainChatTurnRouteDecision {
    pub(crate) path: MainChatExecutionPath,
    pub(crate) strategy_label: String,
    pub(crate) reason_code: String,
    pub(crate) kernel_supported: bool,
    pub(crate) kernel_support_disposition: String,
    pub(crate) fallback_allowed: bool,
    pub(crate) requires_provider: bool,
    pub(crate) requires_tool_loop: bool,
    pub(crate) live_provider_backed_react_required: bool,
    pub(crate) governed_agent_loop_candidate_selection_required: bool,
}

impl MainChatTurnRouteDecision {
    pub(crate) fn execution_path_label(&self) -> &'static str {
        self.path.as_str()
    }

    pub(crate) fn legacy_compat_fallback(&self) -> Self {
        Self {
            path: MainChatExecutionPath::LegacyCompatFallback,
            strategy_label: self.strategy_label.clone(),
            reason_code: "legacy_compat_after_strategy_no_result".into(),
            kernel_supported: self.kernel_supported,
            kernel_support_disposition: self.kernel_support_disposition.clone(),
            fallback_allowed: true,
            requires_provider: false,
            requires_tool_loop: false,
            live_provider_backed_react_required: self.live_provider_backed_react_required,
            governed_agent_loop_candidate_selection_required: self
                .governed_agent_loop_candidate_selection_required,
        }
    }
}

pub(crate) async fn decide_main_chat_turn_route(
    selected_strategy: &MainChatAgentStrategy,
    messages: &[ChatMessage],
    state: &Arc<AppState>,
) -> MainChatTurnRouteDecision {
    let kernel_support_disposition =
        main_chat_kernel_support_disposition(selected_strategy, messages);
    let live_provider_backed_react_required =
        main_chat_live_provider_eval_requires_provider_backed_react(selected_strategy, state).await;
    let governed_agent_loop_candidate_selection_required =
        main_chat_react_turn_requires_governed_agent_loop_candidate_selection(
            selected_strategy,
            messages,
            state,
        )
        .await;

    decide_main_chat_turn_route_from_disposition(
        *selected_strategy,
        kernel_support_disposition,
        live_provider_backed_react_required,
        governed_agent_loop_candidate_selection_required,
    )
}

pub(crate) fn decide_main_chat_turn_route_from_disposition(
    selected_strategy: MainChatAgentStrategy,
    kernel_support_disposition: MainChatKernelSupportDisposition,
    live_provider_backed_react_required: bool,
    governed_agent_loop_candidate_selection_required: bool,
) -> MainChatTurnRouteDecision {
    let kernel_supported = matches!(
        kernel_support_disposition,
        MainChatKernelSupportDisposition::KernelSupported
            | MainChatKernelSupportDisposition::GovernedBlocker
    );
    let requires_tool_loop =
        live_provider_backed_react_required || governed_agent_loop_candidate_selection_required;

    let (path, reason_code, fallback_allowed, requires_provider) = if requires_tool_loop {
        (
            MainChatExecutionPath::ToolLoop,
            if live_provider_backed_react_required {
                "provider_backed_react_required"
            } else {
                "governed_agent_loop_candidate_selection_required"
            },
            true,
            live_provider_backed_react_required,
        )
    } else if !kernel_supported {
        (
            MainChatExecutionPath::LegacyCompatFallback,
            "kernel_support_unavailable",
            true,
            false,
        )
    } else {
        match kernel_support_disposition {
            MainChatKernelSupportDisposition::GovernedBlocker => (
                MainChatExecutionPath::GovernedBlocker,
                "kernel_governed_blocker",
                false,
                false,
            ),
            MainChatKernelSupportDisposition::KernelSupported => match selected_strategy {
                MainChatAgentStrategy::DirectAnswer => (
                    MainChatExecutionPath::KernelDirect,
                    "kernel_supported_direct_answer",
                    false,
                    false,
                ),
                MainChatAgentStrategy::ReActToolExecution => (
                    MainChatExecutionPath::KernelReadTool,
                    "kernel_supported_read_tool",
                    false,
                    false,
                ),
                MainChatAgentStrategy::PlanExecute => (
                    MainChatExecutionPath::PlanExecute,
                    "kernel_supported_plan_execute",
                    false,
                    false,
                ),
                MainChatAgentStrategy::MemoryProposal
                | MainChatAgentStrategy::LifeModelProposal
                | MainChatAgentStrategy::BlockedConfirmation => (
                    MainChatExecutionPath::KernelWriteOutcome,
                    "kernel_supported_write_outcome",
                    false,
                    false,
                ),
                MainChatAgentStrategy::ReviewMaturation => (
                    MainChatExecutionPath::GovernedBlocker,
                    "kernel_governed_blocker",
                    false,
                    false,
                ),
            },
        }
    };

    MainChatTurnRouteDecision {
        path,
        strategy_label: selected_strategy.as_str().into(),
        reason_code: reason_code.into(),
        kernel_supported,
        kernel_support_disposition: kernel_support_disposition.as_str().into(),
        fallback_allowed,
        requires_provider,
        requires_tool_loop,
        live_provider_backed_react_required,
        governed_agent_loop_candidate_selection_required,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openlife_core::agent::main_chat_agent_v1::{AgentIngress, MainChatAgentStrategy};
    use openlife_core::agent::AgentTaskKind;

    fn user_message(content: &str) -> ChatMessage {
        ChatMessage {
            role: "user".into(),
            content: content.into(),
        }
    }

    fn decision_for_user_text(user_text: &str) -> MainChatTurnRouteDecision {
        let ingress = AgentIngress::default();
        let ingress_decision = ingress.decide(
            "route-decision-test-session",
            user_text,
            None,
            AgentTaskKind::Conversation,
        );
        let messages = vec![user_message(user_text)];
        let disposition =
            main_chat_kernel_support_disposition(&ingress_decision.selected_strategy, &messages);
        decide_main_chat_turn_route_from_disposition(
            ingress_decision.selected_strategy,
            disposition,
            false,
            false,
        )
    }

    #[test]
    fn main_chat_turn_route_decision_maps_current_kernel_paths() {
        let cases = [
            (
                "Explain focused work in one concise paragraph.",
                MainChatAgentStrategy::DirectAnswer,
                MainChatExecutionPath::KernelDirect,
                "kernel_supported_direct_answer",
            ),
            (
                "Read Cargo.toml as a governed workspace file observation.",
                MainChatAgentStrategy::ReActToolExecution,
                MainChatExecutionPath::KernelReadTool,
                "kernel_supported_read_tool",
            ),
            (
                "Draft a weekly plan and break this goal into steps.",
                MainChatAgentStrategy::PlanExecute,
                MainChatExecutionPath::PlanExecute,
                "kernel_supported_plan_execute",
            ),
            (
                "Please remember that I prefer morning writing blocks.",
                MainChatAgentStrategy::MemoryProposal,
                MainChatExecutionPath::KernelWriteOutcome,
                "kernel_supported_write_outcome",
            ),
            (
                "Send this private medical update to my coworker.",
                MainChatAgentStrategy::BlockedConfirmation,
                MainChatExecutionPath::KernelWriteOutcome,
                "kernel_supported_write_outcome",
            ),
        ];

        for (user_text, expected_strategy, expected_path, expected_reason) in cases {
            let decision = decision_for_user_text(user_text);
            assert_eq!(decision.strategy_label, expected_strategy.as_str());
            assert_eq!(decision.path, expected_path, "{user_text}");
            assert_eq!(decision.reason_code, expected_reason, "{user_text}");
            assert!(decision.kernel_supported, "{user_text}");
            assert!(!decision.fallback_allowed, "{user_text}");
            assert!(!decision.requires_provider, "{user_text}");
            assert!(!decision.requires_tool_loop, "{user_text}");
        }
    }

    #[test]
    fn main_chat_turn_route_decision_keeps_tool_loop_and_legacy_explicit() {
        let provider_backed = decide_main_chat_turn_route_from_disposition(
            MainChatAgentStrategy::ReActToolExecution,
            MainChatKernelSupportDisposition::KernelSupported,
            true,
            false,
        );
        assert_eq!(provider_backed.path, MainChatExecutionPath::ToolLoop);
        assert_eq!(
            provider_backed.reason_code,
            "provider_backed_react_required"
        );
        assert!(provider_backed.fallback_allowed);
        assert!(provider_backed.requires_provider);
        assert!(provider_backed.requires_tool_loop);

        let governed_selection = decide_main_chat_turn_route_from_disposition(
            MainChatAgentStrategy::ReActToolExecution,
            MainChatKernelSupportDisposition::KernelSupported,
            false,
            true,
        );
        assert_eq!(governed_selection.path, MainChatExecutionPath::ToolLoop);
        assert_eq!(
            governed_selection.reason_code,
            "governed_agent_loop_candidate_selection_required"
        );
        assert!(governed_selection.fallback_allowed);
        assert!(!governed_selection.requires_provider);
        assert!(governed_selection.requires_tool_loop);

        let legacy = governed_selection.legacy_compat_fallback();
        assert_eq!(legacy.path, MainChatExecutionPath::LegacyCompatFallback);
        assert_eq!(
            legacy.execution_path_label(),
            MainChatExecutionPath::LegacyCompatFallback.as_str()
        );
        assert_eq!(legacy.reason_code, "legacy_compat_after_strategy_no_result");
        assert!(legacy.fallback_allowed);
        assert!(!legacy.requires_tool_loop);
    }

    #[test]
    fn main_chat_send_stream_route_parity_table_uses_single_decision_object() {
        use crate::main_chat_command_surface_eval::{
            main_chat_command_surface_eval_user_text, MainChatCommandSurfaceEvalScenario,
        };

        let cases = [
            (
                "direct_answer",
                MainChatCommandSurfaceEvalScenario::DirectProviderTrace,
                MainChatExecutionPath::KernelDirect,
            ),
            (
                "read_tool_file",
                MainChatCommandSurfaceEvalScenario::FileReadSuccess,
                MainChatExecutionPath::KernelReadTool,
            ),
            (
                "plan_execute_draft",
                MainChatCommandSurfaceEvalScenario::PlanExecuteDraft,
                MainChatExecutionPath::PlanExecute,
            ),
            (
                "proposal_path",
                MainChatCommandSurfaceEvalScenario::ProposalPath,
                MainChatExecutionPath::KernelWriteOutcome,
            ),
            (
                "web_blocker",
                MainChatCommandSurfaceEvalScenario::WebPolicyBlocker,
                MainChatExecutionPath::KernelReadTool,
            ),
            (
                "registered_mcp_success",
                MainChatCommandSurfaceEvalScenario::RegisteredMcpReadSuccess,
                MainChatExecutionPath::KernelReadTool,
            ),
            (
                "tool_permission_proposal",
                MainChatCommandSurfaceEvalScenario::RegisteredMcpPermissionProposal,
                MainChatExecutionPath::KernelReadTool,
            ),
        ];

        for (label, scenario, expected_path) in cases {
            let user_text = main_chat_command_surface_eval_user_text(scenario);
            let send_decision = decision_for_user_text(user_text);
            let stream_decision = decision_for_user_text(user_text);
            assert_eq!(send_decision, stream_decision, "{label}");
            assert_eq!(send_decision.path, expected_path, "{label}");
            assert!(send_decision.path.is_kernel_dispatch(), "{label}");
            assert!(!send_decision.fallback_allowed, "{label}");
        }

        let legacy_eligible = decide_main_chat_turn_route_from_disposition(
            MainChatAgentStrategy::ReActToolExecution,
            MainChatKernelSupportDisposition::KernelSupported,
            false,
            true,
        );
        assert_eq!(legacy_eligible.path, MainChatExecutionPath::ToolLoop);
        assert!(legacy_eligible.fallback_allowed);
        assert_eq!(
            legacy_eligible.legacy_compat_fallback().path,
            MainChatExecutionPath::LegacyCompatFallback
        );
    }

    #[test]
    fn main_chat_send_stream_route_parity_source_guard_prevents_local_branch_reimplementation() {
        let send_source = include_str!("main_chat_send.rs");
        let stream_source = include_str!("main_chat_streaming.rs");
        let pipeline_source = include_str!("main_chat_turn_pipeline.rs");

        for (label, source) in [("send", send_source), ("stream", stream_source)] {
            assert!(
                source.contains("decide_main_chat_turn_route("),
                "{label} must consume the shared route decision helper"
            );
            assert_eq!(
                source.matches("main_chat_kernel_supports_turn(").count(),
                0,
                "{label} must not reimplement kernel support branching"
            );
            assert_eq!(
                source
                    .matches("main_chat_live_provider_eval_requires_provider_backed_react(")
                    .count(),
                0,
                "{label} must not reimplement live-provider ReAct branching"
            );
            assert_eq!(
                source
                    .matches(
                        "main_chat_react_turn_requires_governed_agent_loop_candidate_selection("
                    )
                    .count(),
                0,
                "{label} must not reimplement governed AgentLoop candidate branching"
            );
        }

        assert!(
            pipeline_source.contains("main_chat_kernel_support_disposition("),
            "the shared pipeline helper owns kernel support disposition"
        );
        assert!(
            pipeline_source
                .contains("main_chat_live_provider_eval_requires_provider_backed_react("),
            "the shared pipeline helper owns live-provider ReAct routing"
        );
        assert!(
            pipeline_source
                .contains("main_chat_react_turn_requires_governed_agent_loop_candidate_selection("),
            "the shared pipeline helper owns governed candidate-selection routing"
        );
    }
}
