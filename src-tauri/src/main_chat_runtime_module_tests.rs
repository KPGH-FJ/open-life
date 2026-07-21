#[test]
fn main_chat_runtime_module_tests_are_not_concentrated_in_lib_rs() {
    let lib_rs_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs");
    let source = std::fs::read_to_string(lib_rs_path).expect("read src/lib.rs");

    for moved_test in [
        "main_chat_runtime_support_helpers_are_extracted_from_lib_rs",
        "main_chat_generation_support_helpers_are_extracted_from_lib_rs",
        "main_chat_legacy_fallback_helpers_are_extracted_from_lib_rs",
        "main_chat_retired_runtime_modules_are_not_registered_or_present",
        "main_chat_preprocess_helpers_are_extracted_from_lib_rs",
        "main_chat_conversation_update_helpers_are_extracted_from_lib_rs",
        "main_chat_final_gate_aggregation_is_not_hidden_in_test_module",
        "main_chat_send_command_has_non_tauri_state_executor",
        "main_chat_stream_command_has_non_tauri_state_executor",
        "legacy_send_fallback_plan_has_no_agent_loop_or_tool_side_effects",
        "retired_stream_fallback_plan_is_blocked_for_l2_l3",
        "ordinary_stream_legacy_plan_is_built_after_governed_strategy_attempt",
        "obsolete_ordinary_chat_legacy_only_guard_wording_is_retired",
        "ordinary_chat_entrypoints_avoid_retired_agent_loop_helpers_and_direct_executor_construction",
        "chat_page_does_not_call_default_adapter_migration_preview_or_review_commands",
        "transient_state_chat_authority_has_no_frontend_or_shipped_command_write_bypass",
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
    let config_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tauri.conf.json");
    let config: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(config_path).expect("read tauri.conf.json"))
            .expect("parse tauri.conf.json");
    let capability_path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("capabilities/default.json");
    let capability: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(capability_path).expect("read default capability"),
    )
    .expect("parse default capability");
    let dev_capability_path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("capabilities/dev-extensions.json");
    let dev_capability: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dev_capability_path).expect("read dev capability"),
    )
    .expect("parse dev capability");
    let main_window = config
        .get("app")
        .and_then(|app| app.get("windows"))
        .and_then(|windows| windows.as_array())
        .and_then(|windows| {
            windows
                .iter()
                .find(|window| window.get("label").and_then(|label| label.as_str()) == Some("main"))
        })
        .expect("main window config");
    let helper = source
        .split("fn ensure_main_window_visible")
        .nth(1)
        .and_then(|rest| rest.split("#[cfg_attr").next())
        .expect("ensure_main_window_visible helper body");

    assert!(
        source.contains("fn runtime_dev_url()") && source.contains("OPENLIFE_DEV_URL"),
        "main window setup must preserve the runtime dev-server override used by native UI dogfood"
    );
    assert!(
        source.contains("url.host_str()")
            && source.contains("127.0.0.1")
            && source.contains("localhost")
            && helper.contains("manager.config().build.dev_url"),
        "runtime dev URL override must stay constrained to the configured local dev origin"
    );
    assert!(
        helper.contains("runtime_dev_url()")
            && helper.contains("WebviewUrl::External(dev_url)")
            && helper.contains("WebviewWindowBuilder::from_config"),
        "main window setup must create the window with the runtime dev URL instead of starting on stale bundled assets"
    );
    assert_eq!(
        main_window.get("create").and_then(|create| create.as_bool()),
        Some(false),
        "main window must be created in setup so runtime dev URL overrides can be applied before first load"
    );
    assert!(
        capability.get("remote").is_none(),
        "release IPC capability must not authorize a remote web origin"
    );
    let remote_urls = dev_capability
        .get("remote")
        .and_then(|remote| remote.get("urls"))
        .and_then(|urls| urls.as_array())
        .expect("development capability remote urls");
    assert!(
        remote_urls.iter().all(|url| matches!(
            url.as_str(),
            Some("http://127.0.0.1:*" | "http://localhost:*")
        )),
        "development-only remote IPC capability must stay limited to loopback origins"
    );
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
fn dormant_main_chat_conversation_update_route_stays_deleted() {
    let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(lib_rs_path).expect("read src/lib.rs");
    let retired_module = ["main", "chat", "conversation", "updates"].join("_");
    let module_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join(format!("{retired_module}.rs"));
    assert!(
        !module_path.exists(),
        "the callerless conversation-update module must stay deleted instead of being protected by an extraction test"
    );
    assert!(
        !source.contains(&format!("mod {retired_module}")),
        "the deleted module must not remain compiled into the shipped crate"
    );
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
        "pub(crate) fn merge_memory_hits(",
    ] {
        assert!(
            module_source.contains(expected),
            "preprocess module must expose {expected}"
        );
    }
    for forbidden in [
        "\nasync fn preprocess_chat_input(",
        "\npub(crate) fn merge_memory_hits(",
    ] {
        assert!(
            !source.contains(forbidden),
            "preprocess helper {forbidden} should not remain in lib.rs"
        );
    }
    assert!(
        !module_source.contains("preprocess_chat_input_v2"),
        "the uncalled ContextAssembler preprocessing route must stay deleted"
    );
}

#[test]
fn main_chat_retired_runtime_modules_are_not_registered_or_present() {
    let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(lib_rs_path).expect("read src/lib.rs");
    let src_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");

    for (module_name, registration) in [
        (
            ["main_chat_", "strategy.rs"].concat(),
            ["pub(crate) mod main_chat_", "strategy;"].concat(),
        ),
        (
            ["main_chat_", "tool_loop.rs"].concat(),
            ["pub(crate) mod main_chat_", "tool_loop;"].concat(),
        ),
        (
            ["main_chat_", "legacy_agent_loop.rs"].concat(),
            ["pub(crate) mod main_chat_", "legacy_agent_loop;"].concat(),
        ),
    ] {
        assert!(
            !src_root.join(&module_name).exists(),
            "{module_name} must not remain as a product Main Chat runtime module"
        );
        assert!(
            !source.contains(&registration),
            "lib.rs must not register retired product Main Chat runtime module {module_name}"
        );
    }
}

fn retired_delivery_marker(prefix: &str) -> String {
    [prefix, "_fallback_delivery"].join("")
}

#[test]
fn main_chat_retired_fallback_delivery_is_absent_from_product_modules() {
    let legacy_module_path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/main_chat_legacy_fallback.rs");
    assert!(
        !legacy_module_path.exists(),
        "retired ordinary fallback delivery must not remain as a production module"
    );

    let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
    let lib_source = std::fs::read_to_string(lib_rs_path).expect("read src/lib.rs");
    assert!(
        !lib_source.contains("main_chat_legacy_fallback"),
        "lib.rs must not register a retired ordinary fallback module"
    );

    let pipeline_path = format!(
        "{}/src/main_chat_turn_pipeline.rs",
        env!("CARGO_MANIFEST_DIR")
    );
    let pipeline_source = std::fs::read_to_string(pipeline_path).expect("read pipeline");
    for forbidden in [
        ["Legacy", "CompatFallback"].join(""),
        ["legacy_compat", "_fallback"].join(""),
        retired_delivery_marker("run_retired_buffered"),
        retired_delivery_marker("run_retired_streaming"),
    ] {
        assert!(
            !pipeline_source.contains(&forbidden),
            "ordinary turn pipeline must not contain {forbidden}"
        );
    }
    let legacy_true_assignment = ["legacy_fallback_used = ", "true"].join("");
    assert!(
        !pipeline_source.contains(&legacy_true_assignment),
        "ordinary turn pipeline must not record blocked retired fallback as used"
    );
}

#[test]
fn ordinary_chat_entrypoints_and_pipeline_delegate_to_openlife_turn_runtime_only() {
    let send_module_path = format!("{}/src/main_chat_send.rs", env!("CARGO_MANIFEST_DIR"));
    let send_source = std::fs::read_to_string(send_module_path).expect("read main_chat_send.rs");
    let send_body = extract_rust_function_body(
        &send_source,
        "pub(crate) fn send_message_with_operation_state(",
    );
    assert!(send_body.contains("OpenLifeTurnRuntime::new("));
    assert!(
        !send_body.contains("decide_main_chat_turn_route("),
        "send_message must not own route branching after OpenLifeTurnRuntime lands"
    );
    assert!(
        !send_body.contains(&["try_run_main_chat_agent_", "strategy("].concat()),
        "send_message must not own retired strategy fallback after OpenLifeTurnRuntime lands"
    );
    assert!(
        !send_body.contains(&["run_main_chat_tool_loop_", "adapter("].concat()),
        "send_message must not dispatch to the retired ToolLoop adapter"
    );

    let stream_module_path = format!("{}/src/main_chat_streaming.rs", env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(stream_module_path).expect("read main_chat_streaming.rs");
    let stream_body = extract_rust_function_body(
        &source,
        "pub(crate) fn start_stream_message_with_operation_state<'a>(",
    );
    assert!(stream_body.contains("OpenLifeTurnRuntime::new("));
    assert!(
        !stream_body.contains("decide_main_chat_turn_route("),
        "start_stream_message must not own route branching after OpenLifeTurnRuntime lands"
    );
    assert!(
        !stream_body.contains(&["try_run_main_chat_agent_", "strategy("].concat()),
        "start_stream_message must not own retired strategy fallback after OpenLifeTurnRuntime lands"
    );
    assert!(
        !stream_body.contains(&["run_main_chat_tool_loop_", "adapter("].concat()),
        "start_stream_message must not dispatch to the retired ToolLoop adapter"
    );

    let pipeline_module_path = format!(
        "{}/src/main_chat_turn_pipeline.rs",
        env!("CARGO_MANIFEST_DIR")
    );
    let pipeline_source =
        std::fs::read_to_string(pipeline_module_path).expect("read main_chat_turn_pipeline.rs");
    assert!(
        pipeline_source.contains("OpenLifeTurnRuntime::new("),
        "pipeline compatibility wrapper must delegate to OpenLifeTurnRuntime"
    );
    for forbidden in [
        ["try_run_main_chat_agent_", "strategy("].join(""),
        ["run_main_chat_tool_loop_", "adapter("].join(""),
        ["handle_agent_loop_", "fallback("].join(""),
    ] {
        assert!(
            !pipeline_source.contains(&forbidden),
            "pipeline compatibility wrapper must not call retired runtime helper {forbidden}"
        );
    }
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
fn ordinary_chat_entrypoints_avoid_retired_agent_loop_helpers_and_direct_executor_construction() {
    let ordinary_chat_bodies = ordinary_chat_entrypoint_bodies();

    for (body_name, body) in &ordinary_chat_bodies {
        assert!(
            !body.contains(&["send_message_with_", "agent_loop("].concat())
                && !body.contains(&["start_stream_message_with_", "agent_loop("].concat()),
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
            "send_message_with_operation_state",
            extract_rust_function_body(
                &send_source,
                "pub(crate) fn send_message_with_operation_state(",
            ),
        ),
        (
            "start_stream_message_with_operation_state",
            extract_rust_function_body(
                &stream_source,
                "pub(crate) fn start_stream_message_with_operation_state<'a>(",
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
fn production_conversation_does_not_call_default_adapter_migration_preview_or_review_commands() {
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repo root");
    let conversation_paths = [
        repo_root.join("frontend/src/ui/journeys/governedAction/WorkspaceConversationPanel.tsx"),
        repo_root.join("frontend/src/ui/journeys/governedAction/useWorkspaceConversation.ts"),
        repo_root
            .join("frontend/src/ui/journeys/governedAction/workspaceConversationDataSource.ts"),
        repo_root.join("frontend/src/App.tsx"),
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

    for path in conversation_paths {
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

#[test]
fn transient_state_chat_authority_has_no_frontend_or_shipped_command_write_bypass() {
    let repo_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repo root");
    let conversation = std::fs::read_to_string(
        repo_root
            .join("frontend/src/ui/journeys/governedAction/workspaceConversationDataSource.ts"),
    )
    .expect("read production Workspace conversation data source");
    let frontend_bridge = std::fs::read_to_string(repo_root.join("frontend/src/tauri.ts"))
        .expect("read frontend Tauri bridge");
    let shipped_handlers = std::fs::read_to_string(repo_root.join("src-tauri/src/lib.rs"))
        .expect("read shipped Tauri handlers");
    let state_commands = std::fs::read_to_string(repo_root.join("src-tauri/src/commands/state.rs"))
        .expect("read state commands");
    let runtime = std::fs::read_to_string(repo_root.join("src-tauri/src/main_chat_kernel.rs"))
        .expect("read Main Chat kernel");
    let policy =
        std::fs::read_to_string(repo_root.join("openlife-core/src/agent/main_chat_agent_v1.rs"))
            .expect("read Main Chat policy");

    for forbidden in [
        "tryHandleQuickCommand",
        "addDailyGoal(",
        "toggleDailyGoal(",
        "updateDailyGoal(",
        "deleteDailyGoal(",
        "recordState(",
        "saveChatMessage(",
        "handleSaveAsDailyGoal",
    ] {
        assert!(
            !conversation.contains(forbidden),
            "production Workspace conversation must not bypass TurnRuntime through {forbidden}"
        );
    }

    for retired_bridge in [
        "export async function addDailyGoal",
        "export async function toggleDailyGoal",
        "export async function updateDailyGoal",
        "export async function deleteDailyGoal",
        "invoke(\"add_daily_goal\"",
        "invoke(\"toggle_daily_goal\"",
        "invoke(\"update_daily_goal\"",
        "invoke(\"delete_daily_goal\"",
        "export async function recordState",
        "invoke(\"record_state\"",
    ] {
        assert!(
            !frontend_bridge.contains(retired_bridge),
            "frontend bridge must not retain retired state mutation route {retired_bridge}"
        );
    }

    for retired_command in [
        "commands::state::add_daily_goal",
        "commands::state::toggle_daily_goal",
        "commands::state::update_daily_goal",
        "commands::state::delete_daily_goal",
        "record_state,",
    ] {
        assert!(
            !shipped_handlers.contains(retired_command),
            "shipped handler surface must not retain {retired_command}"
        );
    }

    for retired_implementation in [
        "pub async fn add_daily_goal",
        "pub async fn toggle_daily_goal",
        "pub async fn update_daily_goal",
        "pub async fn delete_daily_goal",
        "pub async fn record_state",
        "record_state_with_state",
    ] {
        assert!(
            !state_commands.contains(retired_implementation),
            "state command module must not retain {retired_implementation}"
        );
    }

    assert!(
        conversation.contains("sendMessageV2"),
        "production Workspace conversation must enter the shared buffered TurnRuntime command"
    );
    assert!(
        policy.contains("TransientStateCommand"),
        "PolicyRouter must own the typed transient-state route"
    );
    assert!(
        runtime.contains("StateGateway::new("),
        "Main Chat runtime must commit authorized transient-state effects through StateGateway"
    );
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
        "pub(crate) async fn finalize_chat_agent_run(",
        "pub(crate) fn main_chat_provider_endpoint_kind(",
        "pub(crate) fn preview_text(",
    ] {
        assert!(
            module_source.contains(expected),
            "generation support module must expose {expected}"
        );
    }
    assert!(
        !module_source.contains("persist_vector_memory_for_message"),
        "ordinary chat turns must not be copied into a second vector-content store"
    );
    assert!(
        !module_source.contains("generate_non_stream_fallback"),
        "the unused second provider/fail-soft completion route must stay deleted"
    );
    for forbidden in [
        "proposal_engine",
        "generate_from_run",
        "ChatProposalGeneratorAdapter",
        "generate_and_persist_chat_proposals",
    ] {
        assert!(
            !module_source.contains(forbidden),
            "ordinary Main Chat finalization must not restore post-hoc proposal authority {forbidden}"
        );
    }
    let bootstrap_source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/bootstrap.rs"),
    )
    .expect("read bootstrap.rs");
    let state_source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/state.rs"),
    )
    .expect("read state.rs");
    assert!(!bootstrap_source.contains("ChatProposalGeneratorAdapter"));
    assert!(!state_source.contains("proposal_engine"));
    for forbidden in [
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
fn retired_main_chat_proposal_support_route_is_absent() {
    let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(lib_rs_path).expect("read src/lib.rs");
    let module_path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/main_chat_proposal_support.rs");
    assert!(
        !module_path.exists(),
        "the dormant parallel Main Chat Proposal route must remain deleted"
    );
    for forbidden in [
        "mod main_chat_proposal_support;",
        "create_main_chat_agent_proposal(",
        "attach_main_chat_tool_permission_proposal_metadata(",
    ] {
        assert!(
            !source.contains(forbidden),
            "retired Proposal route symbol {forbidden} must not return to lib.rs"
        );
    }
}

#[test]
fn main_chat_final_gate_aggregation_is_not_hidden_in_test_module() {
    let lib_rs_path = format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(&lib_rs_path).expect("read src/lib.rs");
    let module_path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/main_chat_final_gate.rs");
    let module_source = std::fs::read_to_string(&module_path).expect("read final gate module");
    let command_source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/commands/agent_runtime/mod.rs"),
    )
    .expect("read commands/agent_runtime/mod.rs");

    assert!(
        source.contains("pub(crate) mod main_chat_final_gate;"),
        "pure Main Chat final-gate aggregation must live in a non-test module"
    );
    assert!(
        module_source
            .contains("pub(crate) fn build_main_chat_agent_execution_v1_final_gate_report("),
        "final-gate aggregation must be owned by the reusable final-gate module"
    );
    assert!(
        module_path.is_file(),
        "final-gate aggregation module file must exist outside #[cfg(test)]"
    );
    assert!(
        !command_source.contains("run_main_chat_agent_execution_v1_final_acceptance_gate")
            && !command_source.contains("build_main_chat_agent_execution_v1_final_gate_report("),
        "Phase7 command surface must not restore the retired final acceptance runner"
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
    let live_provider_test_source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/main_chat_live_provider_tests.rs"),
    )
    .expect("read live provider tests module");
    let retired_final_acceptance_test_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/main_chat_final_acceptance_tests.rs");

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
        live_provider_test_source.contains(
            "crate::main_chat_final_gate::completed_main_chat_live_provider_eval_harness_report("
        ),
        "live-provider tests must build completed report contract fixtures through the reusable helper"
    );
    assert!(
        !retired_final_acceptance_test_path.exists(),
        "retired final acceptance test owner must remain expected-absent under Phase7"
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
        module_source.contains("send_message_with_operation_state("),
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
        !module_source.contains("#[cfg(test)]\npub(crate) fn build_isolated_main_chat_eval_state("),
        "the isolated eval state factory itself must not be hidden behind #[cfg(test)]"
    );
    let harness_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/main_chat_live_provider_harness.rs");
    let harness_source =
        std::fs::read_to_string(&harness_path).expect("read live-provider harness module");
    assert!(
        harness_source.contains("main_chat_eval_state::build_isolated_main_chat_eval_state()"),
        "the real non-default live-provider gate must consume the shared eval state factory"
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
        send_case_body.contains("start_stream_message_with_operation_state("),
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
        module_source.contains("pub(crate) fn send_message_with_operation_state("),
        "send module must expose the boxed Arc<AppState> executor that final gates can call without tauri::State or mock IPC"
    );
    assert!(
        module_source.contains("Pin<Box<dyn Future<Output = Result<SendMessageResult, String>>")
            && module_source.contains("Box::pin(async move"),
        "send executor must own the boxed command-to-runtime future seam"
    );
    assert!(
        !source.contains("\npub(crate) fn send_message_with_operation_state("),
        "send state executor should not remain concentrated in lib.rs"
    );
    assert!(
        compact_send_body.contains(
            "main_chat_send::send_message_with_operation_state(operation_id,session_id,messages,selected_skill_id,state.inner(),).await"
        ),
        "the Tauri command wrapper must call the operation-aware reusable send executor"
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
        module_source.contains("pub(crate) fn start_stream_message_with_operation_state<'a>("),
        "streaming module must expose the boxed Arc<AppState> executor that final gates can call without tauri::State or mock IPC"
    );
    assert!(
        module_source.contains(
            "Pin<Box<dyn Future<Output = Result<serde_json::Value, String>> + Send + 'a>>"
        ) && module_source.contains("Box::pin(async move"),
        "stream executor must own the boxed command-to-runtime future seam"
    );
    assert!(
        !module_source.contains("STREAM_INIT_TIMEOUT_SECS")
            && !module_source.contains("STREAM_CHUNK_TIMEOUT_SECS")
            && !module_source.contains("tokio::time::timeout"),
        "the command wrapper must not impose a fixed whole-turn timeout; provider adapters own connect/request/idle timeout semantics"
    );
    let provider_source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../openlife-core/src/llm.rs"),
    )
    .expect("read provider adapter timeout policy");
    assert!(
        provider_source.contains("STREAM_CONNECT_TIMEOUT_SECS")
            && provider_source.contains("STREAM_IDLE_TIMEOUT_SECS"),
        "provider adapters must bound connection and stream-idle waits"
    );
    assert!(
        !source.contains("\npub(crate) fn start_stream_message_with_operation_state"),
        "stream state executor should not remain concentrated in lib.rs"
    );
    assert!(
        stream_body.contains("main_chat_streaming::start_stream_message_with_operation_state("),
        "the Tauri stream command wrapper must call the operation-aware reusable stream executor"
    );
}

#[test]
fn focused_main_chat_modules_import_helpers_from_owning_modules_not_lib_rs_root() {
    for module_name in [
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

#[test]
fn ordinary_main_chat_run_identity_has_no_late_constructor_or_random_terminal_fallback() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let kernel_source = std::fs::read_to_string(manifest_dir.join("src/main_chat_kernel.rs"))
        .expect("read Main Chat kernel");
    let runtime_source =
        std::fs::read_to_string(manifest_dir.join("src/main_chat_turn_runtime.rs"))
            .expect("read Main Chat turn runtime");
    let generation_source =
        std::fs::read_to_string(manifest_dir.join("src/main_chat_generation_support.rs"))
            .expect("read Main Chat generation support");

    let kernel_product_source = kernel_source
        .split_once("#[cfg(test)]\nmod tests")
        .map(|(product_source, _)| product_source)
        .unwrap_or(&kernel_source);
    let runtime_product_source = runtime_source
        .split_once("#[cfg(test)]\nmod product_receipt_ipc_tests")
        .map(|(product_source, _)| product_source)
        .unwrap_or(&runtime_source);
    assert!(
        !kernel_product_source.contains("AgentRun::new_chat_run("),
        "ordinary kernel builders must load the early canonical AgentRun instead of constructing a late compatibility run"
    );
    for forbidden_terminal_run_fallback in [
        "run_id = uuid::Uuid::new_v4",
        "run_id: uuid::Uuid::new_v4",
        "unwrap_or_else(|| uuid::Uuid::new_v4",
        "unwrap_or_else(uuid::Uuid::new_v4",
    ] {
        assert!(
            !runtime_product_source.contains(forbidden_terminal_run_fallback),
            "cancel and failure finalizers must retain the persisted early run id instead of inventing a terminal fallback id"
        );
    }
    assert!(
        !generation_source.contains("store.create_run(agent_run)"),
        "ordinary run finalization must update the persisted early AgentRun and fail closed when it is missing"
    );
}

#[test]
fn ordinary_final_delivery_has_one_durable_turn_runtime_owner() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let runtime = std::fs::read_to_string(root.join("main_chat_turn_runtime.rs"))
        .expect("read TurnRuntime final owner");
    let event_store = std::fs::read_to_string(root.join("main_chat_event_stream.rs"))
        .expect("read durable event projection");
    let core =
        std::fs::read_to_string(root.join("../../openlife-core/src/agent/main_chat_agent_v1.rs"))
            .expect("read canonical Task owner store");
    let runtime_product_source = runtime
        .split_once("#[cfg(test)]\nmod turn_admission_tests")
        .expect("TurnRuntime production/test boundary must exist")
        .0;
    let final_receipt_persistence = extract_rust_function_body(
        runtime_product_source,
        "async fn persist_openlife_turn_final_delivery_receipt(",
    );
    let final_receipt_recovery = extract_rust_function_body(
        runtime_product_source,
        "async fn recover_openlife_turn_from_durable_final(",
    );
    let replay_final_receipt_persistence = extract_rust_function_body(
        runtime_product_source,
        "async fn persist_openlife_replay_final_receipt(",
    );
    let final_append_owner = extract_rust_function_body(
        &event_store,
        "pub(crate) fn append_terminal_final_and_seal(",
    );
    const FINAL_OWNER_SOURCE: &str = "openlife_turn_runtime.final_delivery_owner";
    assert_eq!(
        runtime_product_source.matches(FINAL_OWNER_SOURCE).count(),
        1,
        "production TurnRuntime must contain exactly one final-owner authentication use"
    );
    assert_eq!(
        final_receipt_persistence
            .matches(".append_terminal_final_and_seal(")
            .count(),
        1,
        "ordinary FinalDelivery persistence must delegate exactly once to the atomic EventStore append owner"
    );
    assert_eq!(
        final_append_owner.matches(FINAL_OWNER_SOURCE).count(),
        1,
        "the atomic EventStore transaction must assign exactly one durable FinalDelivery owner"
    );
    assert_eq!(
        final_receipt_recovery.matches(FINAL_OWNER_SOURCE).count(),
        1,
        "durable FinalDelivery recovery must authenticate exactly one append owner"
    );
    const TASK_OWNER_RECEIPT_CALL: &str = ".canonical_owner_receipt(";
    assert_eq!(
        runtime_product_source
            .matches(TASK_OWNER_RECEIPT_CALL)
            .count(),
        3,
        "production TurnRuntime must obtain the versioned Task owner receipt once for ordinary persistence, once for ordinary recovery, and once for replay finalization"
    );
    assert_eq!(
        final_receipt_persistence
            .matches(TASK_OWNER_RECEIPT_CALL)
            .count(),
        1
    );
    assert_eq!(
        final_receipt_recovery
            .matches(TASK_OWNER_RECEIPT_CALL)
            .count(),
        1
    );
    assert_eq!(
        replay_final_receipt_persistence
            .matches(TASK_OWNER_RECEIPT_CALL)
            .count(),
        1,
        "replay finalization must bind its own terminal epoch to one versioned Task owner receipt"
    );
    assert_eq!(
        final_receipt_persistence
            .matches("\"taskOwnerDigestVersion\"")
            .count(),
        1,
        "the one final append owner must persist the Task owner digest version"
    );
    assert_eq!(
        final_receipt_recovery
            .matches("final_payload_task_owner_digest(")
            .count(),
        1,
        "final recovery must authenticate the Task owner digest version before comparing it"
    );
    assert_eq!(
        core.matches("pub fn canonical_owner_receipt(").count(),
        1,
        "the Task store must expose exactly one versioned canonical owner receipt API"
    );
    assert_eq!(
        core.matches("pub fn canonical_owner_digest(").count(),
        0,
        "the digest-only Task owner API must stay absent"
    );
    assert!(!runtime.contains("replay-final:"));
    assert!(!runtime.contains("openlife_turn_runtime.replay_aggregate"));
    let snapshot_materializer = event_store
        .split_once("fn event_drafts_from_snapshot(")
        .expect("snapshot event materializer must exist")
        .1
        .split_once("fn serialized_metadata_enum_label")
        .expect("snapshot event materializer boundary must exist")
        .0;
    assert!(
        !snapshot_materializer.contains("final_delivery.created"),
        "snapshot projection must not create a second durable FinalDelivery owner"
    );
}

#[test]
fn kernel_cannot_reclassify_policy_authority_or_upgrade_a_direct_route() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let kernel_source = std::fs::read_to_string(manifest_dir.join("src/main_chat_kernel.rs"))
        .expect("read Main Chat kernel");
    let intent_signal_source = std::fs::read_to_string(
        manifest_dir.join("../openlife-core/src/agent/main_chat_governance_intent.rs"),
    )
    .expect("read Main Chat intent signal extractor");
    assert!(
        !kernel_source.contains("classify_main_chat_governance_intent"),
        "PolicyRouter must be the only product intent authority"
    );
    assert!(!kernel_source.contains("pub selected_strategy: Option<MainChatAgentStrategy>"));
    assert!(kernel_source.contains("pub policy_decision: PolicyDecision"));
    assert!(kernel_source.contains("enforce_kernel_read_capability("));
    assert!(!kernel_source.contains("fn is_file_write_intent("));
    assert!(!kernel_source.contains("fn is_dangerous_shell_write_intent("));
    assert!(!intent_signal_source.contains("pub fn classify_main_chat_governance_intent("));
    assert!(intent_signal_source.contains("pub fn extract_main_chat_intent_signals("));
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
