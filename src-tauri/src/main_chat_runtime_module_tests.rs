use openlife_core::layer_router::Layer;

use crate::main_chat_legacy_fallback::{
    ordinary_send_chat_execution_plan, ordinary_stream_chat_execution_plan, OrdinaryChatRouteKind,
};

#[test]
fn main_chat_runtime_module_tests_are_not_concentrated_in_lib_rs() {
    let lib_rs_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs");
    let source = std::fs::read_to_string(lib_rs_path).expect("read src/lib.rs");

    for moved_test in [
        "main_chat_runtime_support_helpers_are_extracted_from_lib_rs",
        "main_chat_generation_support_helpers_are_extracted_from_lib_rs",
        "main_chat_proposal_support_helpers_are_extracted_from_lib_rs",
        "main_chat_legacy_fallback_helpers_are_extracted_from_lib_rs",
        "main_chat_legacy_agent_loop_helpers_are_extracted_from_lib_rs",
        "main_chat_preprocess_helpers_are_extracted_from_lib_rs",
        "main_chat_conversation_update_helpers_are_extracted_from_lib_rs",
        "main_chat_final_gate_aggregation_is_not_hidden_in_test_module",
        "main_chat_send_command_has_non_tauri_state_executor",
        "main_chat_stream_command_has_non_tauri_state_executor",
        "legacy_send_fallback_plan_has_no_agent_loop_or_tool_side_effects",
        "retired_stream_fallback_plan_is_blocked_for_l2_l3",
        "ordinary_stream_legacy_plan_is_built_after_governed_strategy_attempt",
        "obsolete_ordinary_chat_legacy_only_guard_wording_is_retired",
        "ordinary_chat_entrypoints_avoid_deprecated_agent_loop_helpers_and_direct_executor_construction",
        "chat_page_does_not_call_default_adapter_migration_preview_or_review_commands",
        "default_chat_entrypoints_do_not_call_w19_w60_command_surfaces_or_w73_readiness_report_or_w74_invocation",
        "default_chat_entrypoints_do_not_call_w19_w60_command_surfaces",
    ] {
        assert!(
            !source.contains(&format!("fn {moved_test}(")),
            "{moved_test} should live in main_chat_runtime_module_tests.rs"
        );
    }
}

#[test]
fn main_window_visibility_uses_tauri_window_config_not_hardcoded_index_asset() {
    let lib_rs_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs");
    let source = std::fs::read_to_string(lib_rs_path).expect("read src/lib.rs");
    let helper = source
        .split("fn ensure_main_window_visible")
        .nth(1)
        .and_then(|rest| rest.split("#[cfg_attr").next())
        .expect("ensure_main_window_visible helper body");

    assert!(
        helper.contains("WebviewWindowBuilder::from_config"),
        "main fallback window must be recreated from tauri.conf.json so devUrl and frontendDist stay authoritative"
    );
    assert!(
        !helper.contains("WebviewUrl::App(\"index.html\""),
        "main fallback window must not hard-code index.html, which can reopen a stale bundled UI"
    );
}

#[test]
fn main_chat_conversation_update_helpers_are_extracted_from_lib_rs() {
    let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(lib_rs_path).expect("read src/lib.rs");
    let module_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/main_chat_conversation_updates.rs");
    assert!(
        module_path.exists(),
        "Main Chat conversation update helper module file must exist outside lib.rs"
    );
    let module_source =
        std::fs::read_to_string(&module_path).expect("read main_chat_conversation_updates.rs");

    for expected in [
        "pub(crate) fn try_auto_checkin_daily_goals(",
        "pub(crate) fn build_reasoning_trace_prompt(",
        "pub(crate) async fn capture_conversation_signals(",
    ] {
        assert!(
            module_source.contains(expected),
            "conversation update module must expose {expected}"
        );
    }
    for forbidden in [
        "\nfn try_auto_checkin_daily_goals(",
        "\nfn build_reasoning_trace_prompt(",
        "\nasync fn capture_conversation_signals(",
    ] {
        assert!(
            !source.contains(forbidden),
            "conversation update helper {forbidden} should not remain in lib.rs"
        );
    }
}

#[test]
fn main_chat_preprocess_helpers_are_extracted_from_lib_rs() {
    let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(lib_rs_path).expect("read src/lib.rs");
    let module_path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/main_chat_preprocess.rs");
    assert!(
        module_path.exists(),
        "Main Chat preprocess helper module file must exist outside lib.rs"
    );
    let module_source =
        std::fs::read_to_string(&module_path).expect("read main_chat_preprocess.rs");

    for expected in [
        "pub(crate) async fn preprocess_chat_input(",
        "pub(crate) async fn preprocess_chat_input_v2(",
        "pub(crate) fn merge_memory_hits(",
    ] {
        assert!(
            module_source.contains(expected),
            "preprocess module must expose {expected}"
        );
    }
    for forbidden in [
        "\nasync fn preprocess_chat_input(",
        "\nasync fn preprocess_chat_input_v2(",
        "\npub(crate) fn merge_memory_hits(",
    ] {
        assert!(
            !source.contains(forbidden),
            "preprocess helper {forbidden} should not remain in lib.rs"
        );
    }
}

#[test]
fn main_chat_legacy_agent_loop_helpers_are_extracted_from_lib_rs() {
    let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(lib_rs_path).expect("read src/lib.rs");
    let module_path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/main_chat_legacy_agent_loop.rs");
    assert!(
        module_path.exists(),
        "Main Chat legacy AgentLoop helper module file must exist outside lib.rs"
    );
    let module_source =
        std::fs::read_to_string(&module_path).expect("read main_chat_legacy_agent_loop.rs");

    for expected in [
        "pub(crate) async fn send_message_with_agent_loop(",
        "pub(crate) async fn start_stream_message_with_agent_loop(",
        "pub(crate) async fn handle_agent_loop_fallback(",
        "pub(crate) fn emit_agent_status_update(",
        "struct TauriStreamingCallback",
    ] {
        assert!(
            module_source.contains(expected),
            "legacy AgentLoop module must expose {expected}"
        );
    }
    for forbidden in [
        "\nasync fn send_message_with_agent_loop(",
        "\nasync fn start_stream_message_with_agent_loop(",
        "\nasync fn handle_agent_loop_fallback(",
        "\nfn emit_agent_status_update(",
        "\nstruct TauriStreamingCallback",
    ] {
        assert!(
            !source.contains(forbidden),
            "legacy AgentLoop helper {forbidden} should not remain in lib.rs"
        );
    }
}

#[test]
fn main_chat_legacy_fallback_helpers_are_extracted_from_lib_rs() {
    let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(lib_rs_path).expect("read src/lib.rs");
    let module_path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/main_chat_legacy_fallback.rs");
    assert!(
        module_path.exists(),
        "Main Chat legacy fallback helper module file must exist outside lib.rs"
    );
    let module_source =
        std::fs::read_to_string(&module_path).expect("read main_chat_legacy_fallback.rs");

    for expected in [
        "pub(crate) enum OrdinaryChatRouteKind",
        "pub(crate) struct OrdinaryChatExecutionPlan",
        "pub(crate) fn ordinary_send_chat_execution_plan(",
        "pub(crate) fn ordinary_stream_chat_execution_plan(",
        "pub(crate) async fn run_retired_buffered_fallback_delivery(",
    ] {
        assert!(
            module_source.contains(expected),
            "legacy fallback module must expose {expected}"
        );
    }
    for forbidden in [
        "\nenum OrdinaryChatRouteKind",
        "\nstruct OrdinaryChatExecutionPlan",
        "\nfn ordinary_send_chat_execution_plan(",
        "\nfn ordinary_stream_chat_execution_plan(",
        "\nasync fn send_message_with_legacy_generation(",
    ] {
        assert!(
            !source.contains(forbidden),
            "legacy fallback helper {forbidden} should not remain in lib.rs"
        );
    }
    assert!(
        !module_source.contains("send_message_with_legacy_generation"),
        "legacy fallback module must not expose the retired success-generation fallback"
    );
}

#[test]
fn legacy_send_fallback_plan_has_no_agent_loop_or_tool_side_effects() {
    let plan = ordinary_send_chat_execution_plan(Layer::L2);

    assert_eq!(plan.route_kind, OrdinaryChatRouteKind::LegacyNonStream);
    assert!(!plan.constructs_agent_loop);
    assert!(!plan.constructs_action_executor);
    assert!(!plan.tool_execution_allowed);
    assert!(!plan.agent_actions_allowed);
    assert!(!plan.agent_observations_allowed);
    assert!(!plan.mcp_audit_write_allowed);
    assert!(!plan.external_write_allowed);
    assert!(!plan.plan_execute_allowed);
    assert!(!plan.golden_path_allowed);
    assert!(!plan.final_gate_allowed);
    assert!(!plan.guidance_consumption_enabled);
}

#[test]
fn retired_stream_fallback_plan_is_blocked_for_l2_l3() {
    for layer in [Layer::L2, Layer::L3] {
        let plan = ordinary_stream_chat_execution_plan(layer);

        assert_eq!(plan.route_kind, OrdinaryChatRouteKind::LegacyStream);
        assert!(!plan.constructs_agent_loop);
        assert!(!plan.constructs_action_executor);
        assert!(!plan.tool_execution_allowed);
        assert!(!plan.agent_actions_allowed);
        assert!(!plan.agent_observations_allowed);
        assert!(!plan.mcp_audit_write_allowed);
        assert!(!plan.external_write_allowed);
        assert!(!plan.plan_execute_allowed);
        assert!(!plan.golden_path_allowed);
        assert!(!plan.final_gate_allowed);
        assert!(!plan.guidance_consumption_enabled);
    }
}

#[test]
fn ordinary_chat_entrypoints_try_kernel_before_legacy_strategy_paths() {
    let send_module_path = format!("{}/src/main_chat_send.rs", env!("CARGO_MANIFEST_DIR"));
    let send_source = std::fs::read_to_string(send_module_path).expect("read main_chat_send.rs");
    let send_body =
        extract_rust_function_body(&send_source, "pub(crate) async fn send_message_with_state(");
    assert!(send_body.contains("run_main_chat_turn_pipeline_buffered("));
    assert!(
        !send_body.contains("decide_main_chat_turn_route("),
        "send_message must not own route branching after the turn pipeline wrapper lands"
    );
    assert!(
        !send_body.contains("try_run_main_chat_agent_strategy("),
        "send_message must not own strategy fallback after the turn pipeline wrapper lands"
    );
    assert!(
        !send_body.contains("main_chat_kernel_supports_turn("),
        "send_message must not reimplement kernel-vs-strategy route branching"
    );

    let stream_module_path = format!("{}/src/main_chat_streaming.rs", env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(stream_module_path).expect("read main_chat_streaming.rs");
    let stream_body = extract_rust_function_body(
        &source,
        "pub(crate) async fn start_stream_message_with_state(",
    );
    assert!(stream_body.contains("run_main_chat_turn_pipeline_streaming("));
    assert!(
        !stream_body.contains("decide_main_chat_turn_route("),
        "start_stream_message must not own route branching after the turn pipeline wrapper lands"
    );
    assert!(
        !stream_body.contains("try_run_main_chat_agent_strategy("),
        "start_stream_message must not own strategy fallback after the turn pipeline wrapper lands"
    );
    assert!(
        !stream_body.contains("ordinary_stream_chat_execution_plan(layer)"),
        "start_stream_message must not own fallback selection after the turn pipeline wrapper lands"
    );
    assert!(
        !stream_body.contains("main_chat_kernel_supports_turn("),
        "start_stream_message must not reimplement kernel-vs-strategy route branching"
    );

    let pipeline_module_path = format!(
        "{}/src/main_chat_turn_pipeline.rs",
        env!("CARGO_MANIFEST_DIR")
    );
    let pipeline_source =
        std::fs::read_to_string(pipeline_module_path).expect("read main_chat_turn_pipeline.rs");
    let pipeline_body = extract_rust_function_body(
        &pipeline_source,
        "pub(crate) async fn run_main_chat_turn_pipeline_streaming(",
    );
    let route_decision = pipeline_body
        .find("decide_main_chat_turn_route(")
        .expect("pipeline should call the shared route decision helper first");
    let strategy_attempt = pipeline_body
        .find("try_run_main_chat_agent_strategy(")
        .expect("pipeline should keep the explicit legacy strategy fallback");
    let tool_loop_attempt = pipeline_body
        .find("run_main_chat_tool_loop_adapter(")
        .expect("pipeline should dispatch ToolLoop before legacy strategy fallback");
    let legacy_plan_after_strategy = pipeline_body[strategy_attempt..]
        .find("run_retired_streaming_fallback_delivery(")
        .map(|offset| strategy_attempt + offset)
        .expect("pipeline should keep the final non-ToolLoop legacy stream delivery");

    assert!(
        route_decision < tool_loop_attempt,
        "pipeline should make the shared route decision before ToolLoop dispatch"
    );
    assert!(
        tool_loop_attempt < strategy_attempt,
        "ToolLoop adapter should handle ToolLoop route decisions before old strategy fallback"
    );
    assert!(
        strategy_attempt < legacy_plan_after_strategy,
        "pipeline should attempt old strategy before building the final non-ToolLoop legacy stream delivery"
    );
}

#[test]
fn stream_legacy_fallback_never_constructs_agent_runtime() {
    let pipeline_module_path = format!(
        "{}/src/main_chat_turn_pipeline.rs",
        env!("CARGO_MANIFEST_DIR")
    );
    let pipeline_source =
        std::fs::read_to_string(pipeline_module_path).expect("read main_chat_turn_pipeline.rs");
    let retired_stream_body = extract_rust_function_body(
        &pipeline_source,
        "async fn run_retired_streaming_fallback_delivery(",
    );

    assert!(
        !retired_stream_body.contains("AgentRuntime::new"),
        "stream legacy fallback must not construct the old AgentRuntime"
    );
    assert!(
        !retired_stream_body.contains(".execute_task("),
        "stream legacy fallback must not execute old AgentRuntime tasks"
    );
    assert!(
        retired_stream_body.contains("retired_stream_runtime_fallback_blocked"),
        "stream legacy fallback should return an explicit blocker payload"
    );
}

#[test]
fn obsolete_ordinary_chat_legacy_only_guard_wording_is_retired() {
    let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(lib_rs_path).expect("read src/lib.rs");
    let obsolete_test_name = concat!(
        "ordinary_chat_entrypoints_",
        "do_not_dispatch_to_agent_loop_helpers"
    );
    let obsolete_assertion = concat!(
        "ordinary send_message must not construct ",
        "AgentLoop or ActionExecutor"
    );

    assert!(
        !source.contains(obsolete_test_name),
        "ordinary Chat tests should no longer describe Main Chat v1 as a legacy-only route"
    );
    assert!(
        !source.contains(obsolete_assertion),
        "ordinary Chat tests should describe deprecated helper isolation, not forbid the governed Main Chat v1 strategy path"
    );
}

#[test]
fn ordinary_chat_entrypoints_avoid_deprecated_agent_loop_helpers_and_direct_executor_construction()
{
    let ordinary_chat_bodies = ordinary_chat_entrypoint_bodies();

    for (body_name, body) in &ordinary_chat_bodies {
        assert!(
            !body.contains("send_message_with_agent_loop(")
                && !body.contains("start_stream_message_with_agent_loop("),
            "{body_name} must not dispatch to the deprecated legacy AgentLoop helper"
        );
        assert!(
            !body.contains("ActionExecutor::new(") && !body.contains("AgentLoop::new("),
            "{body_name} should delegate governed execution through Main Chat v1 instead of constructing executors inline"
        );
    }
}

fn ordinary_chat_entrypoint_bodies() -> Vec<(&'static str, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let send_source =
        std::fs::read_to_string(root.join("main_chat_send.rs")).expect("read main_chat_send.rs");
    let stream_source = std::fs::read_to_string(root.join("main_chat_streaming.rs"))
        .expect("read main_chat_streaming.rs");

    vec![
        (
            "send_message_with_state",
            extract_rust_function_body(
                &send_source,
                "pub(crate) async fn send_message_with_state(",
            ),
        ),
        (
            "start_stream_message_with_state",
            extract_rust_function_body(
                &stream_source,
                "pub(crate) async fn start_stream_message_with_state(",
            ),
        ),
    ]
}

fn retired_default_adapter_token() -> &'static str {
    concat!("default_chat_", "adapter")
}

fn retired_default_adapter_type_token() -> &'static str {
    concat!("Default", "Chat", "Adapter")
}

#[test]
fn chat_page_does_not_call_default_adapter_migration_preview_or_review_commands() {
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repo root");
    let chat_paths = [
        repo_root.join("frontend/src/pages/ChatPage.tsx"),
        repo_root.join("frontend/src/pages/chat/ChatInputArea.tsx"),
        repo_root.join("frontend/src/pages/chat/useChatStreaming.ts"),
        repo_root.join("frontend/src/pages/chat/useChatContext.ts"),
        repo_root.join("frontend/src/pages/chat/useChatSessions.ts"),
    ];
    let forbidden = [
        retired_default_adapter_token(),
        retired_default_adapter_type_token(),
        "checkRuntimeMigrationGate",
        "draftControlledChatMigrationPlan",
        "recordControlledChatMigrationReviewDecision",
        "checkControlledChatMigrationImplementationGate",
        "runControlledChatMigrationShadowRun",
        concat!("run", "Default", "Chat", "Adapter", "ControlledPreview"),
        concat!("draft", "Default", "Chat", "Adapter", "ActivationPlan"),
        concat!(
            "draft",
            "Default",
            "Chat",
            "Adapter",
            "CutoverImplementationPlan"
        ),
        concat!(
            "draft",
            "Default",
            "Chat",
            "Adapter",
            "NarrowImplementationPlan"
        ),
        concat!("record", "Default", "Chat", "Adapter"),
        concat!("check", "Default", "Chat", "Adapter"),
        concat!("get", "Default", "Chat", "Adapter"),
        "getRuntimeStrategyRegistryStatus",
    ];

    for path in chat_paths {
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        for forbidden in forbidden {
            assert!(
                !source.contains(forbidden),
                "{} must not call {}",
                path.display(),
                forbidden
            );
        }
    }
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

#[test]
fn main_chat_runtime_support_helpers_are_extracted_from_lib_rs() {
    let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(lib_rs_path).expect("read src/lib.rs");
    let module_path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/main_chat_runtime_support.rs");
    assert!(
        module_path.exists(),
        "Main Chat runtime support helper module file must exist outside lib.rs"
    );
    let module_source =
        std::fs::read_to_string(&module_path).expect("read src/main_chat_runtime_support.rs");

    for expected in [
        "pub(crate) struct MainChatAgentTurn",
        "pub(crate) async fn start_main_chat_agent_turn(",
        "pub(crate) async fn append_main_chat_agent_transcript(",
        "pub(crate) async fn append_main_chat_direct_answer_contract_transcript(",
        "pub(crate) async fn complete_main_chat_agent_turn_session(",
        "pub(crate) async fn enqueue_main_chat_agent_action(",
        "pub(crate) async fn transition_main_chat_action(",
        "pub(crate) async fn fail_main_chat_action(",
    ] {
        assert!(
            module_source.contains(expected),
            "runtime support module must expose {expected}"
        );
    }
    for forbidden in [
        "\nstruct MainChatAgentTurn",
        "\nasync fn start_main_chat_agent_turn(",
        "\npub(crate) async fn append_main_chat_agent_transcript(",
        "\nasync fn append_main_chat_direct_answer_contract_transcript(",
        "\nasync fn complete_main_chat_agent_turn_session(",
        "\nasync fn enqueue_main_chat_agent_action(",
        "\npub(crate) async fn transition_main_chat_action(",
        "\npub(crate) async fn fail_main_chat_action(",
    ] {
        assert!(
            !source.contains(forbidden),
            "runtime support helper {forbidden} should not remain in lib.rs"
        );
    }
}

#[test]
fn main_chat_generation_support_helpers_are_extracted_from_lib_rs() {
    let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(lib_rs_path).expect("read src/lib.rs");
    let module_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/main_chat_generation_support.rs");
    assert!(
        module_path.exists(),
        "Main Chat generation/finalization support module file must exist outside lib.rs"
    );
    let module_source =
        std::fs::read_to_string(&module_path).expect("read src/main_chat_generation_support.rs");

    for expected in [
        "pub(crate) async fn persist_chat_message_if_needed(",
        "pub(crate) async fn persist_vector_memory_for_message(",
        "pub(crate) async fn finalize_chat_agent_run(",
        "pub(crate) async fn generate_non_stream_fallback(",
        "pub(crate) fn main_chat_provider_endpoint_kind(",
        "pub(crate) fn preview_text(",
    ] {
        assert!(
            module_source.contains(expected),
            "generation support module must expose {expected}"
        );
    }
    for forbidden in [
        "\nasync fn persist_chat_message_if_needed(",
        "\nasync fn persist_vector_memory_for_message(",
        "\nasync fn generate_and_persist_chat_proposals(",
        "\nasync fn finalize_chat_agent_run(",
        "\npub(crate) async fn generate_non_stream_fallback(",
        "\npub(crate) fn main_chat_provider_endpoint_kind(",
        "\npub(crate) fn preview_text(",
    ] {
        assert!(
            !source.contains(forbidden),
            "generation support helper {forbidden} should not remain in lib.rs"
        );
    }
}

#[test]
fn main_chat_proposal_support_helpers_are_extracted_from_lib_rs() {
    let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(lib_rs_path).expect("read src/lib.rs");
    assert!(
        source.contains("pub(crate) mod main_chat_proposal_support;"),
        "Main Chat proposal support module must be declared from lib.rs"
    );
    let module_path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/main_chat_proposal_support.rs");
    assert!(
        module_path.exists(),
        "Main Chat proposal support module file must exist outside lib.rs"
    );
    let module_source =
        std::fs::read_to_string(&module_path).expect("read src/main_chat_proposal_support.rs");

    for expected in [
        "pub(crate) async fn create_main_chat_agent_proposal(",
        "pub(crate) async fn attach_main_chat_tool_permission_proposal_metadata(",
    ] {
        assert!(
            module_source.contains(expected),
            "proposal support module must expose {expected}"
        );
    }
    for forbidden in [
        "\nasync fn create_main_chat_agent_proposal(",
        "\nasync fn attach_main_chat_tool_permission_proposal_metadata(",
    ] {
        assert!(
            !source.contains(forbidden),
            "proposal support helper {forbidden} should not remain in lib.rs"
        );
    }
}

#[test]
fn main_chat_final_gate_aggregation_is_not_hidden_in_test_module() {
    let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(&lib_rs_path).expect("read src/lib.rs");
    let command_source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/commands/agent_runtime/mod.rs"),
    )
    .expect("read commands/agent_runtime/mod.rs");

    assert!(
        source.contains("pub(crate) mod main_chat_final_gate;"),
        "pure Main Chat final-gate aggregation must live in a non-test module"
    );
    assert!(
        command_source.contains(
            "crate::main_chat_final_gate::build_main_chat_agent_execution_v1_final_gate_report("
        ),
        "the final acceptance runner must use the reusable final-gate aggregation module"
    );
    assert!(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/main_chat_final_gate.rs")
            .is_file(),
        "final-gate aggregation module file must exist outside #[cfg(test)]"
    );
}

#[test]
fn main_chat_command_surface_eval_report_normalization_is_not_hidden_in_test_module() {
    let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(&lib_rs_path).expect("read src/lib.rs");
    let module_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/main_chat_command_surface_eval.rs");

    assert!(
        source.contains("pub(crate) mod main_chat_command_surface_eval;"),
        "command-surface eval case matrix and evidence normalization must live in a non-test module"
    );
    assert!(
        module_path.is_file(),
        "command-surface eval report module file must exist outside #[cfg(test)]"
    );
    let module_source =
        std::fs::read_to_string(&module_path).expect("read command-surface eval module");
    assert!(
        module_source.contains("MainChatCommandSurfaceEvalReport"),
        "command-surface eval report type must be reusable by production/test code"
    );
    assert!(
        module_source.contains("MAIN_CHAT_COMMAND_SURFACE_EVAL_CASES"),
        "the 38-case send/stream command-surface matrix must not be hidden in tests"
    );
    assert!(
        module_source.contains("acceptance_evidence("),
        "command-surface acceptance evidence normalization must be reusable outside tests"
    );
    assert!(
        module_source.contains("from_case_evidence("),
        "command-surface report aggregation must be reusable outside tests"
    );
    assert!(
        module_source.contains("MainChatCommandSurfaceEvalReport::from_case_evidence("),
        "the 38-case command-surface runner must call the reusable report aggregation"
    );
}

#[test]
fn main_chat_live_provider_blocked_report_builder_is_not_hidden_in_test_module() {
    let module_path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/main_chat_final_gate.rs");
    let module_source = std::fs::read_to_string(&module_path).expect("read final gate module");
    let harness_source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/main_chat_live_provider_harness.rs"),
    )
    .expect("read live-provider harness module");

    assert!(
        module_source
            .contains("pub(crate) fn blocked_main_chat_live_provider_eval_harness_report("),
        "preflight-blocked live-provider harness reports must be built by reusable production code"
    );
    assert!(
        harness_source
            .contains("main_chat_final_gate::blocked_main_chat_live_provider_eval_harness_report("),
        "the live-provider harness must use the reusable blocked-report builder"
    );
}

#[test]
fn main_chat_live_provider_completed_report_builder_is_not_hidden_in_test_module() {
    let module_path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/main_chat_final_gate.rs");
    let module_source = std::fs::read_to_string(&module_path).expect("read final gate module");
    let final_acceptance_test_source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/main_chat_final_acceptance_tests.rs"),
    )
    .expect("read final acceptance tests module");

    assert!(
        module_source
            .contains("pub(crate) fn completed_main_chat_live_provider_eval_harness_report("),
        "completed live-provider harness report shape must be reusable production code"
    );
    assert!(
        module_source.contains("pub(crate) fn main_chat_live_provider_required_evidence("),
        "live-provider required-evidence list must not be duplicated in test helpers"
    );
    assert!(
        final_acceptance_test_source.contains(
            "main_chat_final_gate::completed_main_chat_live_provider_eval_harness_report("
        ),
        "final-gate tests must build completed live-provider reports through the reusable helper"
    );
}

#[test]
fn main_chat_live_provider_harness_execution_is_not_concentrated_in_lib_rs() {
    let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(&lib_rs_path).expect("read src/lib.rs");
    let module_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/main_chat_live_provider_harness.rs");

    assert!(
        source.contains("pub(crate) mod main_chat_live_provider_harness;"),
        "live-provider harness execution must live in a focused non-test module"
    );
    assert!(
        module_path.is_file(),
        "live-provider harness execution module file must exist outside #[cfg(test)]"
    );
    let module_source =
        std::fs::read_to_string(&module_path).expect("read live-provider harness module");
    assert!(
        module_source.contains("run_main_chat_live_provider_eval_harness_suite_from_state"),
        "live-provider harness suite must be reusable by the real final acceptance runner"
    );
    assert!(
        module_source.contains("send_message_with_state("),
        "live-provider harness execution must use the ordinary Main Chat send path"
    );
    assert!(
        !source.contains(
            "\npub(crate) async fn run_main_chat_live_provider_eval_harness_suite_from_state("
        ),
        "live-provider harness suite must not remain concentrated in src/lib.rs"
    );
    assert!(
        !source.contains("\npub(crate) async fn run_main_chat_live_provider_eval_harness("),
        "live-provider harness execution must not remain concentrated in src/lib.rs"
    );
}

#[test]
fn isolated_main_chat_eval_state_factory_is_not_hidden_in_test_module() {
    let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(&lib_rs_path).expect("read src/lib.rs");
    let module_path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/main_chat_eval_state.rs");

    assert!(
        source.contains("pub(crate) mod main_chat_eval_state;"),
        "isolated Main Chat eval state factory must live in a non-test module"
    );
    assert!(
        module_path.is_file(),
        "isolated Main Chat eval state module file must exist outside #[cfg(test)]"
    );
    let module_source = std::fs::read_to_string(&module_path).expect("read eval state module");
    assert!(
        module_source.contains("build_isolated_main_chat_eval_state"),
        "production/test code must share an isolated state factory for command-surface evidence"
    );
    assert!(
        !module_source.contains("#[cfg(test)]"),
        "isolated eval state factory must be callable by the real non-default final gate"
    );
}

#[test]
fn main_chat_command_surface_eval_scenario_setup_is_not_hidden_in_test_module() {
    let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(&lib_rs_path).expect("read src/lib.rs");
    let module_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/main_chat_command_surface_eval.rs");
    let module_source =
        std::fs::read_to_string(&module_path).expect("read command-surface eval module");

    assert!(
        module_source
            .contains("pub(crate) async fn configure_main_chat_command_surface_eval_state("),
        "scenario-specific command-surface eval state setup must be reusable outside #[cfg(test)]"
    );
    assert!(
        module_source.contains("pub(crate) fn main_chat_command_surface_eval_user_text("),
        "scenario prompt mapping must be reusable outside #[cfg(test)]"
    );
    assert!(
        module_source.contains("pub(crate) fn main_chat_command_surface_eval_session_id("),
        "deterministic session-id mapping must be reusable outside #[cfg(test)]"
    );
    assert!(
        !source.contains("\n    async fn configure_main_chat_command_surface_eval_state("),
        "scenario setup must not remain as a test-only helper in src/lib.rs"
    );
    assert!(
        !source.contains("\n    fn main_chat_command_surface_eval_user_text("),
        "scenario prompt mapping must not remain as a test-only helper in src/lib.rs"
    );
}

#[test]
fn main_chat_command_surface_eval_assertions_are_not_hidden_in_test_module() {
    let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(&lib_rs_path).expect("read src/lib.rs");
    let module_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/main_chat_command_surface_eval.rs");
    let module_source =
        std::fs::read_to_string(&module_path).expect("read command-surface eval module");

    assert!(
        module_source.contains("pub(crate) async fn assert_main_chat_command_surface_eval_case("),
        "command-surface case interpretation must be reusable outside #[cfg(test)]"
    );
    assert!(
        !module_source.contains(
            "#[cfg(test)]\npub(crate) async fn assert_main_chat_command_surface_eval_case("
        ),
        "command-surface case interpretation must not be cfg(test)-gated"
    );
    assert!(
        module_source.contains("pub(crate) fn main_chat_command_surface_eval_has_silent_write("),
        "no-silent-write detection must be reusable outside #[cfg(test)]"
    );
    assert!(
        !module_source.contains(
            "#[cfg(test)]\npub(crate) fn main_chat_command_surface_eval_has_silent_write("
        ),
        "no-silent-write detection must not be cfg(test)-gated"
    );
    assert!(
        !source.contains("\n    async fn assert_main_chat_command_surface_eval_case("),
        "command-surface assertions must not remain as test-only helpers in src/lib.rs"
    );
    assert!(
        !source.contains("\n    fn main_chat_command_surface_eval_has_silent_write("),
        "no-silent-write detection must not remain as a test-only helper in src/lib.rs"
    );
}

#[test]
fn main_chat_command_surface_send_eval_runner_uses_case_assertions() {
    let module_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/main_chat_command_surface_eval.rs");
    let module_source =
        std::fs::read_to_string(&module_path).expect("read command-surface eval module");
    let send_case_body = extract_rust_function_body(
        &module_source,
        "async fn run_main_chat_command_surface_state_eval_case(",
    );

    assert!(
        send_case_body.contains("assert_main_chat_command_surface_eval_case("),
        "production command-surface eval must verify real scenario effects before awarding coverage"
    );
    assert!(
        send_case_body.contains("list_transcript_entries("),
        "production command-surface eval must inspect transcript evidence"
    );
    assert!(
        send_case_body.contains("list_pending_proposals(20)"),
        "production command-surface eval must inspect proposal evidence"
    );
    assert!(
        send_case_body.contains("start_stream_message_with_state("),
        "production command-surface eval must execute stream cases through the reusable stream state executor"
    );
}

#[test]
fn main_chat_send_command_has_non_tauri_state_executor() {
    let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(&lib_rs_path).expect("read src/lib.rs");
    let send_body = extract_rust_function_body(&source, "async fn send_message(");
    let compact_send_body = send_body.split_whitespace().collect::<String>();

    assert!(
        source.contains("pub(crate) mod main_chat_send;"),
        "Main Chat send state executor must live in a focused non-test module"
    );
    let module_path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/main_chat_send.rs");
    assert!(
        module_path.exists(),
        "Main Chat send state executor module file must exist outside lib.rs"
    );
    let module_source = std::fs::read_to_string(&module_path).expect("read main_chat_send.rs");
    assert!(
        module_source.contains("pub(crate) async fn send_message_with_state("),
        "send module must expose the Arc<AppState> executor that final gates can call without tauri::State or mock IPC"
    );
    assert!(
        !source.contains("\npub(crate) async fn send_message_with_state("),
        "send state executor should not remain concentrated in lib.rs"
    );
    assert!(
        compact_send_body.contains(
            "main_chat_send::send_message_with_state(session_id,messages,selected_skill_id,state.inner()).await"
        ),
        "the Tauri command wrapper must call the reusable send_message_with_state executor"
    );
}

#[test]
fn main_chat_stream_command_has_non_tauri_state_executor() {
    let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(&lib_rs_path).expect("read src/lib.rs");
    let stream_body =
        extract_rust_function_body(&source, "async fn start_stream_message<R: tauri::Runtime>(");

    assert!(
        source.contains("pub(crate) mod main_chat_streaming;"),
        "Main Chat stream state executor must live in a focused non-test module"
    );
    let module_path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/main_chat_streaming.rs");
    assert!(
        module_path.exists(),
        "Main Chat stream state executor module file must exist outside lib.rs"
    );
    let module_source = std::fs::read_to_string(&module_path).expect("read main_chat_streaming.rs");
    assert!(
        module_source.contains("pub(crate) async fn start_stream_message_with_state("),
        "streaming module must expose the Arc<AppState> executor that final gates can call without tauri::State or mock IPC"
    );
    assert!(
        module_source.contains("const STREAM_INIT_TIMEOUT_SECS: u64 = 45;"),
        "streaming module must own stream init timeout policy"
    );
    assert!(
        module_source.contains("const STREAM_CHUNK_TIMEOUT_SECS: u64 = 90;"),
        "streaming module must own stream chunk timeout policy"
    );
    assert!(
        !source.contains("\npub(crate) async fn start_stream_message_with_state("),
        "stream state executor should not remain concentrated in lib.rs"
    );
    assert!(
        stream_body.contains("main_chat_streaming::start_stream_message_with_state("),
        "the Tauri stream command wrapper must call the reusable start_stream_message_with_state executor"
    );
}

#[test]
fn focused_main_chat_modules_import_helpers_from_owning_modules_not_lib_rs_root() {
    for module_name in [
        "main_chat_legacy_fallback.rs",
        "main_chat_legacy_agent_loop.rs",
        "main_chat_react_runtime.rs",
        "main_chat_react_execution.rs",
        "main_chat_live_provider_harness.rs",
    ] {
        let module_path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("src/{module_name}"));
        let module_source = std::fs::read_to_string(&module_path)
            .unwrap_or_else(|error| panic!("read {}: {error}", module_path.display()));
        for import_block in root_crate_import_blocks(&module_source) {
            for helper in [
                "append_main_chat_agent_transcript",
                "build_chat_runtime_hs_packet",
                "finalize_chat_agent_run",
                "generate_non_stream_fallback",
                "main_chat_provider_endpoint_kind",
                "preprocess_chat_input_v2",
                "preview_text",
            ] {
                assert!(
                    !import_block.contains(helper),
                    "{module_name} should import {helper} from its owning Main Chat module, not through src/lib.rs root re-exports"
                );
            }
        }
    }
}

fn root_crate_import_blocks(source: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut current: Option<String> = None;

    for line in source.lines() {
        let trimmed = line.trim_start();
        if let Some(block) = current.as_mut() {
            block.push('\n');
            block.push_str(line);
            if trimmed == "};" {
                blocks.push(current.take().expect("root import block"));
            }
            continue;
        }

        if trimmed.starts_with("use crate::{") {
            let block = line.to_string();
            if trimmed.ends_with("};") {
                blocks.push(block);
            } else {
                current = Some(block);
            }
        } else if trimmed.starts_with("use crate::")
            && !trimmed.starts_with("use crate::main_chat_")
            && trimmed.ends_with(';')
        {
            blocks.push(line.to_string());
        }
    }

    blocks
}
