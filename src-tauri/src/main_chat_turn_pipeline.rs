use std::sync::Arc;

use crate::main_chat_turn_runtime::OpenLifeTurnRuntime;
#[allow(unused_imports)]
pub(crate) use crate::main_chat_turn_runtime::{
    decide_main_chat_turn_route, decide_main_chat_turn_route_from_disposition,
    MainChatExecutionPath, MainChatTurnDelivery, MainChatTurnRouteDecision, MainChatTurnStreamMode,
    OpenLifeTurnInput as MainChatTurnPipelineInput,
    OpenLifeTurnOutput as MainChatTurnPipelineOutput,
};
use crate::AppState;

pub(crate) async fn run_main_chat_turn_pipeline_buffered(
    input: MainChatTurnPipelineInput,
    state: &Arc<AppState>,
) -> Result<MainChatTurnPipelineOutput, String> {
    OpenLifeTurnRuntime::new(state).run_buffered(input).await
}

pub(crate) async fn run_main_chat_turn_pipeline_streaming(
    input: MainChatTurnPipelineInput,
    state: &Arc<AppState>,
    emit_stream_event: &mut (impl FnMut(&str, serde_json::Value) + Send),
) -> Result<MainChatTurnPipelineOutput, String> {
    OpenLifeTurnRuntime::new(state)
        .run_streaming(input, emit_stream_event)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use openlife_core::agent::main_chat_agent_v1::{AgentIngress, MainChatAgentStrategy};
    use openlife_core::agent::AgentTaskKind;
    use openlife_core::llm::ChatMessage;

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
        let disposition = crate::main_chat_kernel::main_chat_kernel_support_disposition(
            &ingress_decision.selected_strategy,
            &messages,
        );
        decide_main_chat_turn_route_from_disposition(
            ingress_decision.policy_route,
            ingress_decision.selected_strategy,
            disposition,
            false,
            false,
        )
    }

    #[test]
    fn openlife_turn_route_decision_maps_runtime_states_without_parallel_paths() {
        let cases = [
            (
                "Explain focused work in one concise paragraph.",
                MainChatAgentStrategy::DirectAnswer,
                MainChatExecutionPath::DirectAnswer,
                "openlife_runtime_direct_answer",
            ),
            (
                "Read Cargo.toml as a governed workspace file observation.",
                MainChatAgentStrategy::ReActToolExecution,
                MainChatExecutionPath::ReadOnlyTool,
                "openlife_runtime_read_only_tool",
            ),
            (
                "Draft a weekly plan and break this goal into steps.",
                MainChatAgentStrategy::PlanExecute,
                MainChatExecutionPath::PlanExecute,
                "openlife_runtime_plan_execute",
            ),
            (
                "Please remember that I prefer morning writing blocks.",
                MainChatAgentStrategy::MemoryProposal,
                MainChatExecutionPath::WriteOutcome,
                "openlife_runtime_proposal_only_write",
            ),
            (
                "Send this private medical update to my coworker.",
                MainChatAgentStrategy::BlockedConfirmation,
                MainChatExecutionPath::WriteOutcome,
                "openlife_runtime_confirmation_request",
            ),
        ];

        for (user_text, expected_strategy, expected_path, expected_reason) in cases {
            let decision = decision_for_user_text(user_text);
            assert_eq!(decision.strategy_label, expected_strategy.as_str());
            assert_eq!(decision.path, expected_path, "{user_text}");
            assert_eq!(decision.reason_code, expected_reason, "{user_text}");
            assert!(decision.kernel_supported, "{user_text}");
            assert!(!decision.fallback_allowed, "{user_text}");
            assert!(!decision.requires_tool_loop, "{user_text}");
        }
    }

    #[test]
    fn send_stream_wrappers_and_pipeline_delegate_to_openlife_turn_runtime() {
        let send_source = include_str!("main_chat_send.rs");
        let stream_source = include_str!("main_chat_streaming.rs");
        let pipeline_source = include_str!("main_chat_turn_pipeline.rs");
        let runtime_source = include_str!("main_chat_turn_runtime.rs");

        for (label, source) in [("send", send_source), ("stream", stream_source)] {
            assert!(
                source.contains("OpenLifeTurnRuntime::new("),
                "{label} must delegate to OpenLifeTurnRuntime"
            );
            for forbidden in retired_runtime_markers() {
                assert!(
                    !source.contains(&forbidden),
                    "{label} transport wrapper must not call retired runtime helper {forbidden}"
                );
            }
        }

        assert!(pipeline_source.contains("OpenLifeTurnRuntime::new("));
        let retired_markers = retired_runtime_markers()
            .into_iter()
            .filter(|marker| marker != "decide_main_chat_turn_route(")
            .collect::<Vec<_>>();
        assert!(
            retired_markers
                .iter()
                .all(|forbidden| !pipeline_source.contains(forbidden)),
            "pipeline compatibility wrapper must not own retired runtime branches"
        );
        for forbidden in [
            ["crate::main_chat_", "strategy"].join(""),
            ["crate::main_chat_", "tool_loop"].join(""),
            ["crate::main_chat_", "legacy_agent_loop"].join(""),
            "singleStepFallbackUsed\": true".to_string(),
            "legacyFallbackUsed\": true".to_string(),
        ] {
            assert!(
                !runtime_source.contains(&forbidden),
                "OpenLifeTurnRuntime must not depend on retired product path marker {forbidden}"
            );
        }
    }

    fn retired_runtime_markers() -> Vec<String> {
        vec![
            ["decide_main_chat_turn_", "route("].join(""),
            ["try_run_main_chat_agent_", "strategy("].join(""),
            ["run_main_chat_tool_loop_", "adapter("].join(""),
            ["send_message_with_", "agent_loop("].join(""),
            ["start_stream_message_with_", "agent_loop("].join(""),
            ["handle_agent_loop_", "fallback("].join(""),
        ]
    }
}
