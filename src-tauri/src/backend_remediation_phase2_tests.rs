use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use openlife_core::llm::ChatMessage;

use crate::main_chat_acceptance_test_support::configure_live_provider_eval_state_with_barriered_streaming_local_http_provider;
use crate::main_chat_acceptance_test_support::configure_live_provider_eval_state_with_captured_local_http_provider;
use crate::main_chat_acceptance_test_support::configure_live_provider_eval_state_with_hanging_local_http_provider;

#[test]
fn d009_scheduler_has_one_canonical_owner_and_legacy_json_cannot_execute() {
    let runner = include_str!("scheduler_runner.rs");
    let bootstrap = include_str!("bootstrap.rs");
    let tasks = include_str!("../../openlife-core/src/tasks.rs");

    assert!(!runner.contains("scheduled_tasks.json"));
    assert!(bootstrap.contains("migrate_legacy_json_if_present"));
    assert!(bootstrap.contains("stage_legacy_scheduled_task_review_proposals"));
    assert!(tasks.contains("legacy_due_pending_dispatch_state_unknown"));
    assert!(tasks.contains("legacy_future_pending_requires_fresh_review"));
    assert!(tasks.contains("legacy_scheduled_task_migration_records"));
}

#[test]
fn d009_scheduled_cloud_route_requires_review_policy_and_single_use_consumption() {
    let policy = include_str!("../../openlife-core/src/agent/main_chat_agent_v1.rs");
    let tasks = include_str!("../../openlife-core/src/tasks.rs");
    let proposal = include_str!("commands/proposal.rs");
    let runner = include_str!("scheduler_runner.rs");

    assert!(policy.contains("authorize_scheduled_provider_route"));
    assert!(proposal.contains("claimed_acceptance_snapshot"));
    assert!(proposal.contains("seal_reviewed_cloud_provider_grant"));
    assert!(tasks.contains("scheduler_provider_grant_consumptions"));
    assert!(tasks.contains("scheduled_cloud_grant_consumed_requires_review"));
    assert!(!runner.contains("effective_data_route != ProviderDataRoute::LocalOnly"));
    assert!(!tasks.contains("scheduled cloud provider grant issuance is not implemented"));
}

#[derive(Default)]
struct P003CountingDispatchObserver {
    count: std::sync::atomic::AtomicUsize,
}

#[async_trait::async_trait]
impl openlife_core::agent::ToolDispatchObserver for P003CountingDispatchObserver {
    async fn before_dispatch(
        &self,
        _attempt: &openlife_core::agent::ToolDispatchAttempt,
    ) -> anyhow::Result<()> {
        self.count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
}

#[test]
fn b1_deleted_agent_run_raw_read_callers_are_exactly_allowlisted() {
    fn visit_rust_sources(
        directory: &std::path::Path,
        repository_root: &std::path::Path,
        hits: &mut std::collections::BTreeMap<String, usize>,
    ) {
        for entry in std::fs::read_dir(directory).expect("read source directory") {
            let path = entry.expect("read source entry").path();
            if path.is_dir() {
                visit_rust_sources(&path, repository_root, hits);
            } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
                let relative = path
                    .strip_prefix(repository_root)
                    .expect("source must remain below repository root")
                    .to_string_lossy()
                    .to_string();
                if relative == "src-tauri/src/backend_remediation_phase2_tests.rs" {
                    continue;
                }
                let source = std::fs::read_to_string(&path).expect("read Rust source");
                let count = source.matches(".get_run_including_deleted(").count();
                if count > 0 {
                    hits.insert(relative, count);
                }
            }
        }
    }

    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repository_root = manifest.parent().expect("src-tauri has repository parent");
    let mut actual = std::collections::BTreeMap::new();
    visit_rust_sources(
        &repository_root.join("openlife-core/src"),
        repository_root,
        &mut actual,
    );
    visit_rust_sources(&manifest.join("src"), repository_root, &mut actual);
    let expected = std::collections::BTreeMap::from([
        // Store- and command-local hits are tombstone regression assertions.
        // Product code has only two authorized raw-read owners: early
        // canonical run admission prevents tombstone resurrection, and
        // projection recovery reads the deleted aggregate without exposing it
        // through a product list/detail surface.
        ("openlife-core/src/agent/store.rs".to_string(), 5usize),
        ("src-tauri/src/commands/agent.rs".to_string(), 2usize),
        (
            "src-tauri/src/main_chat_turn_runtime.rs".to_string(),
            1usize,
        ),
        ("src-tauri/src/memory_gateway.rs".to_string(), 1usize),
    ]);
    assert_eq!(
        actual, expected,
        "deleted AgentRun raw reads are limited to canonical run admission, projection recovery, and explicit regression assertions"
    );
}

#[test]
fn d044_shipped_agent_action_replay_bypass_is_absent_and_task_controls_remain_canonical() {
    let agent_commands = include_str!("commands/agent.rs");
    let lib = include_str!("lib.rs");
    let kernel = include_str!("main_chat_kernel.rs");
    let task_controls = include_str!("main_chat_task_controls.rs");
    let turn_runtime = include_str!("main_chat_turn_runtime.rs");
    let terminal_owner_gateway = include_str!("terminal_owner_write_gateway.rs");
    let tool_gateway_resources = include_str!("tool_gateway_resources.rs");
    let frontend_tauri = include_str!("../../frontend/src/tauri.ts");
    let agent_run_detail = include_str!("../../frontend/src/pages/AgentRunDetail.tsx");
    let chat_page = include_str!("../../frontend/src/pages/ChatPage.tsx");
    let tool_call_card = include_str!("../../frontend/src/components/ToolCallCard.tsx");
    let frontend_mock = include_str!("../../frontend/src/test/mocks/tauri.ts");
    let core_mcp = include_str!("../../openlife-core/src/mcp.rs");
    let core_os_tools =
        include_str!("../../openlife-core/src/agent/action_executor/core_os_tools.rs");

    for (surface, source, forbidden) in [
        (
            "backend command",
            agent_commands,
            "pub async fn replay_agent_action(",
        ),
        (
            "backend replay resource adapter",
            tool_gateway_resources,
            "snapshot_tool_gateway_resources_for_replay",
        ),
        ("shipped Tauri handler", lib, "replay_agent_action,"),
        (
            "frontend command bridge",
            frontend_tauri,
            "replayAgentAction",
        ),
        (
            "frontend command mock",
            frontend_mock,
            "replay_agent_action",
        ),
        (
            "AgentRun detail route",
            agent_run_detail,
            "replayAgentAction",
        ),
        ("Main Chat route", chat_page, "replayAgentAction"),
        ("tool-card execute authority", tool_call_card, "onExecute"),
        ("tool-card retry authority", tool_call_card, "onReplay"),
    ] {
        assert!(
            !source.contains(forbidden),
            "D044 retired direct replay surface remains in {surface}: {forbidden}"
        );
    }

    for required in [
        "pub(crate) async fn retry_main_chat_agent_action(",
        ".run_replay(",
    ] {
        assert!(
            task_controls.contains(required),
            "canonical TaskViewModel replay control lost required invariant: {required}"
        );
    }
    for required in [
        "pub(crate) async fn run_replay(",
        "claim_openlife_replay(",
        ".try_register(&task_session_id)",
    ] {
        assert!(
            turn_runtime.contains(required),
            "OpenLifeTurnRuntime replay authority lost required invariant: {required}"
        );
    }
    assert!(
        !turn_runtime.contains(".claim_replay_with_automatic_retry_proof("),
        "OpenLifeTurnRuntime must not bypass the terminal-owner write gateway for replay claims"
    );
    for required in [
        "pub(crate) async fn claim_action_replay(",
        "acquire_open_turn_write_fence(state, task_session_id)",
        ".claim_replay_with_automatic_retry_proof(",
    ] {
        assert!(
            terminal_owner_gateway.contains(required),
            "terminal-owner replay write authority lost required invariant: {required}"
        );
    }
    assert!(agent_run_detail.contains("retryMainChatAgentAction"));
    assert!(chat_page.contains("retryMainChatAgentAction"));
    assert!(agent_run_detail.contains("mailboxRoute"));
    assert!(chat_page.contains("mailboxLinkTarget"));
    assert!(
        !tool_call_card.contains("mailboxRoute"),
        "ToolCallCard stays presentation-only; page-level task and review projections own navigation"
    );
    let retired_generic_replay = ["permission", ".replay_action"].concat();
    assert!(
        !core_mcp.contains(&retired_generic_replay)
            && !core_os_tools.contains(&retired_generic_replay),
        "generic execute-by-name replay must stay deleted; canonical task-control claim plus ToolGateway is the only replay authority"
    );

    let exact_first_proposal = kernel_source_section(
        kernel,
        "async fn attach_kernel_tool_permission_proposal_identity(",
        "fn tool_call_status_from_kernel_status(",
    );
    assert!(exact_first_proposal.contains("submit_with_admission("));
    assert!(!exact_first_proposal.contains("update_proposal("));
    assert!(!exact_first_proposal.contains("create_proposal("));
    let main_chat_read_resources = kernel_source_section(
        tool_gateway_resources,
        "pub(crate) struct MainChatReadToolGatewayResources",
        "pub(crate) struct MainChatExecutionToolGatewayResources",
    );
    assert!(
        !main_chat_read_resources.contains("proposal_store:"),
        "Main Chat read ToolGateway snapshot must not create a generic Proposal before the exact queue identity exists"
    );
}

#[test]
fn d034_automatic_retry_has_no_bare_or_crate_forgeable_claim_authority() {
    let action_queue = include_str!("../../openlife-core/src/agent/main_chat_agent_v1.rs");
    let tool_gateway = include_str!("../../openlife-core/src/agent/tool_gateway.rs");
    let tool_receipt = include_str!("../../openlife-core/src/tool_execution_receipt.rs");
    let tauri_manifest = include_str!("../Cargo.toml");
    let core_manifest = include_str!("../../openlife-core/Cargo.toml");
    let tauri_features = tauri_manifest
        .split_once("[features]")
        .and_then(|(_, rest)| rest.split_once("[lib]"))
        .map(|(features, _)| features)
        .expect("Tauri shipped feature section remains parseable");
    let production_dependencies = tauri_manifest
        .split_once("[dependencies]")
        .and_then(|(_, rest)| rest.split_once("[target."))
        .map(|(dependencies, _)| dependencies)
        .expect("Tauri production dependency section remains parseable");
    let core_features = core_manifest
        .split_once("[features]")
        .and_then(|(_, rest)| rest.split_once("[dependencies]"))
        .map(|(features, _)| features)
        .expect("core feature section remains parseable");
    let proof_start = tool_gateway
        .find("pub struct ToolAutomaticRetryProof")
        .expect("automatic retry proof remains a typed ToolGateway capability");
    let proof_end = tool_gateway[proof_start..]
        .find("\n}\n")
        .map(|offset| proof_start + offset + 3)
        .expect("automatic retry proof declaration has a bounded body");
    let proof_declaration = &tool_gateway[proof_start..proof_end];

    assert!(
        !action_queue.contains("pub fn claim_replay_for_execution("),
        "release code must not expose a replay claim that omits ToolGateway proof"
    );
    assert!(
        !proof_declaration.contains("pub(crate)"),
        "automatic retry proof fields must be sealed inside ToolGateway"
    );
    assert!(
        !action_queue.contains("pub(crate) fn authority_digest("),
        "canonical replay authority must not expose its authenticator to sibling modules"
    );
    assert!(
        action_queue.contains("#[cfg(any(test, feature = \"test-utils\"))]\n    pub fn claim_replay_for_test_fixture("),
        "lower-level replay lifecycle tests may use only an explicitly test-gated semantic fixture"
    );
    assert!(
        tool_receipt.contains(
            "#[cfg(any(test, feature = \"test-utils\"))]\n    pub fn test_bind_to_action_metadata("
        ),
        "digest-only replay fixtures must stay behind the same test-only feature boundary"
    );
    assert!(production_dependencies.contains("openlife-core = { path = \"../openlife-core\" }"));
    assert!(
        !production_dependencies.contains("test-utils"),
        "the shipped Tauri dependency must not enable the replay test fixture feature"
    );
    assert!(core_features.contains("default = []"));
    assert!(core_features.contains("test-utils = []"));
    assert!(
        !tauri_features.contains("test-utils"),
        "no shipped Tauri feature may forward-enable the replay test fixture"
    );
}

fn owned_function<'a>(source: &'a str, start: &str, end: Option<&str>) -> &'a str {
    let (_, body) = source
        .split_once(start)
        .unwrap_or_else(|| panic!("missing product function start marker: {start}"));
    match end {
        Some(end) => body
            .split_once(end)
            .map(|(function, _)| function)
            .unwrap_or_else(|| panic!("missing product function end marker: {end}")),
        None => body,
    }
}

#[test]
fn d030_d031_replay_is_owned_only_by_openlife_turn_runtime() {
    let task_controls = include_str!("main_chat_task_controls.rs");
    let turn_runtime = include_str!("main_chat_turn_runtime.rs");
    let react_execution = include_str!("main_chat_react_execution.rs");

    assert!(
        turn_runtime.contains("pub(crate) async fn run_replay("),
        "resume/retry must enter the typed OpenLifeTurnRuntime replay entry"
    );
    for forbidden in [
        "MainChatTaskControlExecutionOwner",
        "register_main_chat_task_control_execution",
        "finalize_main_chat_replay_owner_exit",
        "execute_main_chat_react_action_with_tool_gateway(",
        "record_replay_dispatch_started(",
    ] {
        assert!(
            !task_controls.contains(forbidden),
            "task-control command/read-model layer retained replay runtime authority: {forbidden}"
        );
    }
    for command in [
        "pub(crate) async fn resume_main_chat_agent_task_with_state(",
        "pub(crate) async fn retry_main_chat_agent_action(",
    ] {
        let body = owned_function(task_controls, command, None);
        assert!(
            body.contains(".run_replay("),
            "{command} must delegate execution to OpenLifeTurnRuntime"
        );
    }
    assert!(
        turn_runtime.contains("impl openlife_core::agent::ToolDispatchObserver for MainChatReplayLifecycleObserver")
            && turn_runtime.contains("impl openlife_core::agent::ToolStartedTransitionObserver for MainChatReplayLifecycleObserver"),
        "replay must separate prepared validation from the actual adapter-start transition"
    );
    assert!(
        react_execution.contains("with_tool_started_transition_observer"),
        "the Main Chat ToolGateway adapter must expose its actual started transition to replay"
    );
}

fn kernel_source_section<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    source
        .split_once(start)
        .unwrap_or_else(|| panic!("missing source section start: {start}"))
        .1
        .split_once(end)
        .unwrap_or_else(|| panic!("missing source section end: {end}"))
        .0
}

#[test]
fn d045_memory_fact_admission_has_one_canonical_uniqueness_authority() {
    let gateway = include_str!("memory_gateway.rs");
    let lifecycle = include_str!("../../openlife-core/src/agent/memory_lifecycle.rs");
    let policy = include_str!("../../openlife-core/src/agent/main_chat_agent_v1.rs");
    let materializer = kernel_source_section(
        gateway,
        "pub(crate) async fn materialize_memory_proposal_with_state(",
        "pub(crate) async fn commit_explicit_user_memory_for_turn_with_state(",
    );

    assert!(
        !materializer.contains("search_text_memories("),
        "derived compatibility search must not authorize canonical Memory writes"
    );
    assert!(lifecycle.contains("idx_memory_lifecycle_active_fact_key"));
    assert!(lifecycle.contains("memory_lifecycle_proposal_links"));
    assert!(lifecycle.contains("transaction_with_behavior(TransactionBehavior::Immediate)"));
    assert!(gateway.contains("PolicyMemoryAdmissionProof"));
    assert!(gateway.contains("MainChatExecutionEpoch"));
    assert!(lifecycle.contains("admission_proof: PolicyMemoryAdmissionProof"));
    assert!(lifecycle.contains("admission_proof.consume_for_explicit_input(&input)?"));
    assert!(policy.contains("authority: PolicyDecisionAuthority"));
    assert!(policy.contains("pub struct PolicyMemoryAdmissionProof"));
    assert!(policy.contains("explicit Memory admission requires live PolicyRouter authority"));
    assert!(
        !gateway.contains("fn commit_explicit_user_memory_with_state("),
        "a no-proof product Memory admission entry must remain absent"
    );
}

#[test]
fn d046_knowledge_note_requires_one_operation_bound_canonical_transaction() {
    let memory = include_str!("../../openlife-core/src/memory.rs");
    let outbox = include_str!("../../openlife-core/src/persistence_outbox.rs");
    let command = include_str!("commands/memory.rs");
    let lib = include_str!("lib.rs");
    let frontend = include_str!("../../frontend/src/tauri.ts");
    let chat_page = include_str!("../../frontend/src/pages/ChatPage.tsx");

    for required in [
        "save_knowledge_note_idempotent_with_outbox(",
        "knowledge_note_operations",
        "transaction_with_behavior(TransactionBehavior::Immediate)",
        "ensure_knowledge_note_operation_id(operation_id)?",
    ] {
        assert!(
            memory.contains(required),
            "KnowledgeNote lost canonical idempotency invariant: {required}"
        );
    }
    assert!(outbox.contains("pub fn mutation_by_event_id("));
    assert!(command.contains("operation_id: String"));
    assert!(command.contains("pub async fn create_knowledge_note("));
    assert!(frontend.contains("operationId: string"));
    assert!(frontend.contains("operation_id: operationId"));
    assert!(frontend.contains("export async function createKnowledgeNote("));
    assert!(
        !memory.contains("pub fn save_memory_record_with_outbox("),
        "unkeyed manual Memory index write authority must stay absent"
    );
    assert!(
        !chat_page.contains("createKnowledgeNote") && !chat_page.contains("indexMemoryChunk"),
        "assistant-authored display content must not bypass ReviewWorkflow through manual indexing"
    );
    assert!(chat_page.contains("草拟记忆提案"));
    for (surface, source) in [
        ("memory command", command),
        ("shipped handler", lib),
        ("frontend bridge", frontend),
    ] {
        assert!(
            !source.contains("index_memory_chunk") && !source.contains("indexMemoryChunk"),
            "retired direct-Memory indexing route remains in {surface}"
        );
    }
}

#[test]
fn tool_gateway_product_paths_use_one_resource_snapshot_authority() {
    let lib = include_str!("lib.rs");
    let kernel = include_str!("main_chat_kernel.rs");
    let react_execution = include_str!("main_chat_react_execution.rs");
    let react_runtime = include_str!("main_chat_react_runtime.rs");
    let scheduler = include_str!("scheduler_runner.rs");
    let product_sources = [
        (
            "dev_tool_command",
            owned_function(
                lib,
                "async fn execute_tool_call(",
                Some("async fn inspect_mcp_call("),
            ),
        ),
        (
            "main_chat_kernel",
            owned_function(
                kernel,
                "impl MainChatKernelReadToolExecutor for AppStateMainChatReadToolExecutor",
                Some("struct KernelMcpReadCandidate"),
            ),
        ),
        (
            "main_chat_react_execution",
            owned_function(
                react_execution,
                "pub(crate) async fn execute_main_chat_react_action_with_tool_gateway(",
                None,
            ),
        ),
        (
            "main_chat_react_runtime",
            owned_function(
                react_runtime,
                "pub(crate) async fn try_run_main_chat_react_agent_loop(",
                Some("pub(crate) fn main_chat_permission_blocker_reason("),
            ),
        ),
        (
            "scheduled_execution",
            owned_function(
                scheduler,
                "async fn execute_scheduled_task(",
                Some("fn project_tool_terminal_receipts("),
            ),
        ),
    ];

    for (label, source) in product_sources {
        assert!(
            source.contains("snapshot_tool_gateway_resources"),
            "{label} must use the single Tauri ToolGateway resource snapshot authority"
        );
        assert!(
            source.contains("with_tool_audit_persistence_observer"),
            "{label} must report mandatory audit commit failure to PersistenceCoordinator"
        );
        for forbidden_duplicate in [
            "tool_permission_store.lock()",
            "mcp_registry.lock()",
            "mcp_audit_store.lock()",
            "privacy_engine.lock()",
            "memory_store.lock()",
        ] {
            assert!(
                !source.contains(forbidden_duplicate),
                "{label} still owns a duplicate AppState lock sequence: {forbidden_duplicate}"
            );
        }
    }

    let authority = include_str!("tool_gateway_resources.rs");
    assert!(
        !authority.contains("openlife_core::config::AppConfig"),
        "ToolGateway snapshots must not retain the full config or hydrated provider secrets"
    );
    for forbidden_external_execution in [
        "ToolGateway::",
        ".execute(",
        ".call_tool",
        "reqwest::",
        "NetworkClient",
        "ProviderAdapter",
    ] {
        assert!(
            !authority.contains(forbidden_external_execution),
            "snapshot authority must never perform external execution: {forbidden_external_execution}"
        );
    }

    fn collect_product_context_owners(
        root: &std::path::Path,
        current: &std::path::Path,
        owners: &mut std::collections::BTreeSet<String>,
    ) {
        fn before_inline_test_modules(source: &str) -> &str {
            let marker = "#[cfg(test)]";
            let mut offset = 0usize;
            while let Some(relative) = source[offset..].find(marker) {
                let index = offset + relative;
                let after_attribute = &source[index + marker.len()..];
                let trimmed = after_attribute.trim_start();
                let Some(after_mod) = trimmed.strip_prefix("mod ") else {
                    offset = index + marker.len();
                    continue;
                };
                let name_len = after_mod
                    .chars()
                    .take_while(|character| character.is_ascii_alphanumeric() || *character == '_')
                    .map(char::len_utf8)
                    .sum::<usize>();
                if after_mod[name_len..].trim_start().starts_with('{') {
                    return &source[..index];
                }
                offset = index + marker.len();
            }
            source
        }

        for entry in std::fs::read_dir(current).expect("read Tauri source directory") {
            let entry = entry.expect("read Tauri source entry");
            let path = entry.path();
            if path.is_dir() {
                collect_product_context_owners(root, &path, owners);
                continue;
            }
            if path.extension().and_then(std::ffi::OsStr::to_str) != Some("rs") {
                continue;
            }
            let file_name = path
                .file_name()
                .and_then(std::ffi::OsStr::to_str)
                .unwrap_or_default();
            if file_name.contains("test") {
                continue;
            }
            let source = std::fs::read_to_string(&path).expect("read Tauri Rust source");
            let production = before_inline_test_modules(&source);
            if production.contains("ActionExecutionContext::new(")
                || production.contains("ActionExecutionContext {")
            {
                assert!(
                    production.contains("snapshot_tool_gateway_resources"),
                    "product ActionExecutionContext owner bypasses the snapshot authority: {}",
                    path.display()
                );
                owners.insert(
                    path.strip_prefix(root)
                        .expect("Tauri source is below manifest root")
                        .to_string_lossy()
                        .replace('\\', "/"),
                );
            }
        }
    }

    let manifest_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut owners = std::collections::BTreeSet::new();
    collect_product_context_owners(manifest_root, &manifest_root.join("src"), &mut owners);
    let expected = [
        "src/commands/a2a.rs",
        "src/lib.rs",
        "src/main_chat_kernel.rs",
        "src/main_chat_react_execution.rs",
        "src/main_chat_react_runtime.rs",
        "src/scheduler_runner.rs",
    ]
    .into_iter()
    .map(str::to_string)
    .collect();
    assert_eq!(
        owners, expected,
        "every production ActionExecutionContext owner must be explicitly classified"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn real_app_state_snapshot_commit_cancel_paths_complete_10000_barrier_interleavings() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    const ITERATIONS: usize = 10_000;
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let scheduled_snapshot =
        crate::tool_gateway_resources::snapshot_tool_gateway_resources_for_scheduler(&state)
            .await
            .expect("Scheduler resources are complete");
    scheduled_snapshot
        .proposal_store
        .pending_count()
        .expect("Scheduler ProposalStore is typed and live");
    scheduled_snapshot
        .agent_run_store
        .run_count()
        .expect("Scheduler AgentRunStore is typed and live");
    assert!(!scheduled_snapshot
        .agent_runtime_config
        .default_strategy
        .is_empty());

    let agent_loop_snapshot =
        crate::tool_gateway_resources::snapshot_tool_gateway_resources_for_main_chat_agent_loop(
            &state,
        )
        .await
        .expect("Main Chat AgentLoop resources are complete");
    assert!(agent_loop_snapshot.limits.max_steps > 0);
    #[cfg(feature = "dev-extensions")]
    {
        let snapshot =
            crate::tool_gateway_resources::snapshot_tool_gateway_resources_for_dev_command(&state)
                .await
                .expect("dev resources are complete");
        snapshot
            .agent_run_store
            .run_count()
            .expect("dev AgentRunStore is typed and live");
    }
    let barrier = Arc::new(tokio::sync::Barrier::new(4));
    let read_progress = Arc::new(AtomicUsize::new(0));
    let execution_progress = Arc::new(AtomicUsize::new(0));
    let commit_cancel_progress = Arc::new(AtomicUsize::new(0));

    let read_worker = {
        let state = Arc::clone(&state);
        let barrier = Arc::clone(&barrier);
        let progress = Arc::clone(&read_progress);
        tokio::spawn(async move {
            for iteration in 0..ITERATIONS {
                barrier.wait().await;
                let snapshot = crate::tool_gateway_resources::snapshot_tool_gateway_resources_for_main_chat_read(&state)
                    .await
                    .expect("Main Chat read resources are complete");
                assert!(!snapshot
                    .governed
                    .shared
                    .registry
                    .list_manifests()
                    .is_empty());
                snapshot
                    .governed
                    .shared
                    .permission_store
                    .list()
                    .expect("read real ToolPermissionStore through owned snapshot");
                state
                    .proposal_store
                    .as_ref()
                    .expect("proposal store")
                    .lock()
                    .await
                    .pending_count()
                    .expect("read ProposalStore outside the read-tool snapshot");
                progress.store(iteration + 1, Ordering::Release);
                barrier.wait().await;
            }
        })
    };

    let execution_worker = {
        let state = Arc::clone(&state);
        let barrier = Arc::clone(&barrier);
        let progress = Arc::clone(&execution_progress);
        tokio::spawn(async move {
            for iteration in 0..ITERATIONS {
                barrier.wait().await;
                let snapshot = crate::tool_gateway_resources::snapshot_tool_gateway_resources_for_main_chat_execution(&state)
                    .await
                    .expect("Main Chat execution resources are complete");
                snapshot
                    .governed
                    .shared
                    .audit_store
                    .list_logs(1)
                    .expect("read real MCP audit store through owned snapshot");
                snapshot
                    .agent_run_store
                    .run_count()
                    .expect("read real AgentRunStore through owned snapshot");
                progress.store(iteration + 1, Ordering::Release);
                barrier.wait().await;
            }
        })
    };

    let commit_cancel_worker = {
        let state = Arc::clone(&state);
        let barrier = Arc::clone(&barrier);
        let progress = Arc::clone(&commit_cancel_progress);
        tokio::spawn(async move {
            for iteration in 0..ITERATIONS {
                barrier.wait().await;
                let cancellation_registry = {
                    state
                        .main_chat_runtime_state
                        .lock()
                        .await
                        .cancellation_registry
                        .clone()
                };
                let task_id = format!("p0-03-real-product-interleaving-{iteration}");
                let registration = cancellation_registry
                    .try_register(&task_id)
                    .expect("unique product execution owner");
                let epoch = registration.execution_epoch();
                if iteration % 2 == 0 {
                    epoch
                        .begin_canonical_commit("proposal", format!("proposal:p0-03-{iteration}"))
                        .expect("commit wins the even schedule")
                        .finish_not_modified();
                    cancellation_registry.cancel(&task_id);
                } else {
                    cancellation_registry.cancel(&task_id);
                    assert!(epoch
                        .begin_canonical_commit("proposal", format!("proposal:p0-03-{iteration}"),)
                        .is_err());
                }
                drop(registration);
                progress.store(iteration + 1, Ordering::Release);
                barrier.wait().await;
            }
        })
    };

    let workers = async {
        for _ in 0..ITERATIONS {
            barrier.wait().await;
            barrier.wait().await;
        }
        read_worker.await.expect("read snapshot worker joins");
        execution_worker
            .await
            .expect("execution snapshot worker joins");
        commit_cancel_worker
            .await
            .expect("commit/cancel worker joins");
    };
    if tokio::time::timeout(Duration::from_secs(45), workers)
        .await
        .is_err()
    {
        panic!(
            "P0-03 product interleaving stalled: read={}, execution={}, commit_cancel={}",
            read_progress.load(Ordering::Acquire),
            execution_progress.load(Ordering::Acquire),
            commit_cancel_progress.load(Ordering::Acquire),
        );
    }
    assert_eq!(read_progress.load(Ordering::Acquire), ITERATIONS);
    assert_eq!(execution_progress.load(Ordering::Acquire), ITERATIONS);
    assert_eq!(commit_cancel_progress.load(Ordering::Acquire), ITERATIONS);
}

#[tokio::test]
async fn missing_required_snapshot_stores_fail_before_tool_dispatch() {
    use std::sync::atomic::Ordering;

    let mut missing_agent = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    Arc::get_mut(&mut missing_agent)
        .expect("isolated state has one owner")
        .agent_run_store = None;
    for error in [
        crate::tool_gateway_resources::snapshot_tool_gateway_resources_for_main_chat_execution(
            &missing_agent,
        )
        .await
        .err()
        .expect("Main Chat execution requires AgentRunStore"),
        crate::tool_gateway_resources::snapshot_tool_gateway_resources_for_main_chat_agent_loop(
            &missing_agent,
        )
        .await
        .err()
        .expect("Main Chat AgentLoop requires AgentRunStore"),
        crate::tool_gateway_resources::snapshot_tool_gateway_resources_for_scheduler(
            &missing_agent,
        )
        .await
        .err()
        .expect("scheduler requires AgentRunStore"),
    ] {
        assert_eq!(error, "tool_gateway_agent_run_store_unavailable");
    }

    let observer = P003CountingDispatchObserver::default();
    let plan = crate::main_chat_react_tool_selection::MainChatReactActionPlan {
        queue_action_type: "mcp.read".into(),
        executor_action_type: "mcp_tool".into(),
        target: "builtin_echo".into(),
        arguments: serde_json::json!({"text":"must-not-dispatch"}),
        description: "Missing canonical AgentRun store counterexample.".into(),
        requires_network: false,
        uses_ephemeral_file_permission: false,
        uses_ephemeral_mcp_wrapper_permission: false,
        tool_candidates: Vec::new(),
    };
    let error = crate::main_chat_react_execution::execute_main_chat_react_action_with_tool_gateway(
        &missing_agent,
        &plan,
        false,
        Some(&observer),
        None,
        Some("run-missing-store-must-not-dispatch"),
        None,
        None,
        None,
    )
    .await
    .err()
    .expect("missing AgentRunStore fails before ToolGateway dispatch");
    assert_eq!(error, "tool_gateway_agent_run_store_unavailable");
    assert_eq!(observer.count.load(Ordering::SeqCst), 0);

    let mut missing_proposal = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    Arc::get_mut(&mut missing_proposal)
        .expect("isolated state has one owner")
        .proposal_store = None;
    crate::tool_gateway_resources::snapshot_tool_gateway_resources_for_main_chat_read(
        &missing_proposal,
    )
    .await
    .expect("Main Chat read must not acquire ProposalStore");
    crate::tool_gateway_resources::snapshot_tool_gateway_resources_for_main_chat_execution(
        &missing_proposal,
    )
    .await
    .expect("Main Chat execution must not acquire ProposalStore");
    crate::tool_gateway_resources::snapshot_tool_gateway_resources_for_main_chat_agent_loop(
        &missing_proposal,
    )
    .await
    .expect("Main Chat AgentLoop must not acquire ProposalStore");
    let scheduler_error =
        crate::tool_gateway_resources::snapshot_tool_gateway_resources_for_scheduler(
            &missing_proposal,
        )
        .await
        .err()
        .expect("scheduler requires ProposalStore");
    assert_eq!(scheduler_error, "tool_gateway_proposal_store_unavailable");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn hanging_provider_keeps_tool_registries_and_canonical_stores_live_until_cancel() {
    use std::sync::atomic::Ordering;

    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let (request_observed, _client_closed, release_late_response, _late_response_attempted) =
        configure_live_provider_eval_state_with_hanging_local_http_provider(&state).await;
    let turn_state = Arc::clone(&state);
    let turn = tokio::spawn(async move {
        crate::main_chat_streaming::start_stream_message_with_state(
            "p0-03-hanging-provider-lock-liveness".into(),
            vec![ChatMessage {
                role: "user".into(),
                content: "Contact the provider and wait until I cancel.".into(),
            }],
            None,
            &turn_state,
            |_name, _payload| {},
        )
        .await
    });

    tokio::time::timeout(Duration::from_secs(2), async {
        while !request_observed.load(Ordering::SeqCst) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("real local HTTP provider observes the request before lock probing");

    let task_session_id = tokio::time::timeout(Duration::from_secs(1), async {
        let session_store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("task session store")
            .lock()
            .await;
        session_store
            .list_sessions(None, 10, 0)
            .expect("list sessions while provider is hanging")
            .into_iter()
            .find(|session| session.chat_session_id == "p0-03-hanging-provider-lock-liveness")
            .expect("active provider-backed task session")
            .id
    })
    .await
    .expect("canonical task store remains available during provider await");

    tokio::time::timeout(Duration::from_secs(1), async {
        let read_snapshot =
            crate::tool_gateway_resources::snapshot_tool_gateway_resources_for_main_chat_read(
                &state,
            )
            .await
            .expect("Main Chat read resources remain complete");
        assert!(!read_snapshot
            .governed
            .shared
            .registry
            .list_manifests()
            .is_empty());
        read_snapshot
            .governed
            .shared
            .permission_store
            .list()
            .expect("permission store read during provider await");
        read_snapshot
            .governed
            .shared
            .audit_store
            .list_logs(1)
            .expect("audit store read during provider await");
        read_snapshot
            .governed
            .memory_store
            .list_sessions(1)
            .expect("memory store read during provider await");
        state
            .proposal_store
            .as_ref()
            .expect("proposal store")
            .lock()
            .await
            .pending_count()
            .expect("proposal store read during provider await");
    })
    .await
    .expect("provider I/O cannot retain any ToolGateway AppState guard");

    crate::main_chat_task_controls::cancel_main_chat_agent_task_with_state(
        &task_session_id,
        &state,
    )
    .await
    .expect("cancel remains live while provider is hanging");
    tokio::time::timeout(Duration::from_secs(2), turn)
        .await
        .expect("turn terminates after local cancel")
        .expect("provider-backed turn task joins")
        .expect("provider-backed turn returns typed cancellation");
    release_late_response.store(true, Ordering::SeqCst);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn hanging_mcp_transport_keeps_app_state_locks_live_and_cancel_terminates_it() {
    use openlife_core::tool_manifest::{ToolIdempotencyContract, ToolManifest, ToolSource};
    use openlife_core::tool_permissions::ToolPermissionPolicy;
    use std::collections::HashMap;

    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let marker_dir = tempfile::tempdir().expect("create hanging MCP marker directory");
    let marker = marker_dir.path().join("call-dispatched.marker");
    let script = r#"
import json, os, sys, time
for line in sys.stdin:
    message = json.loads(line)
    method = message.get('method')
    if method == 'initialize':
        print(json.dumps({'jsonrpc':'2.0','id':message['id'],'result':{'protocolVersion':'2024-11-05','capabilities':{}}}), flush=True)
    elif method == 'tools/list':
        print(json.dumps({'jsonrpc':'2.0','id':message['id'],'result':{'tools':[{'name':'p0_03_hang','description':'hang for lock liveness proof','parameters':{'type':'object','properties':{}}}]}}), flush=True)
    elif method == 'tools/call':
        with open(os.environ['OPENLIFE_P0_03_MCP_MARKER'], 'w', encoding='utf-8') as output:
            output.write('dispatched')
            output.flush()
            os.fsync(output.fileno())
        time.sleep(30)
"#;
    let manifest = ToolManifest {
        id: "mcp:p0-03:p0_03_hang".into(),
        name: "p0_03_hang".into(),
        description: "Real hanging MCP read for AppState lock liveness proof.".into(),
        parameters: serde_json::json!({"type":"object","properties":{}}),
        permission_level: "low".into(),
        risk_level: "low".into(),
        version: "1.0.0".into(),
        source: ToolSource::Mcp {
            server_name: "p0-03".into(),
        },
        capabilities: vec!["read".into()],
        requires_confirmation: false,
        enabled: true,
        declarative_only: false,
        action_type: "read".into(),
        idempotency_contract: ToolIdempotencyContract::Idempotent,
        tags: vec!["typed_contract".into(), "test".into()],
    };
    let mut env = HashMap::new();
    env.insert(
        "OPENLIFE_P0_03_MCP_MARKER".to_string(),
        marker.to_string_lossy().into_owned(),
    );
    let prepared = openlife_core::mcp::McpRegistry::prepare_registration(
        "p0-03",
        "python3",
        &["-u", "-c", script],
        &env,
        vec![manifest.clone()],
    )
    .await
    .expect("prepare typed hanging MCP server outside AppState registry guard");
    state
        .mcp_registry
        .lock()
        .await
        .commit_prepared_registration(prepared)
        .expect("synchronously commit prepared MCP server");
    state
        .tool_permission_store
        .lock()
        .await
        .grant(
            &manifest.name,
            &openlife_core::agent::action_executor::helpers::canonical_tool_source(&manifest),
            &manifest.risk_level,
            &manifest.action_type,
            ToolPermissionPolicy::AllowUntilRevoked,
            None,
        )
        .expect("grant exact typed MCP read scope");

    let cancellation_registry = {
        state
            .main_chat_runtime_state
            .lock()
            .await
            .cancellation_registry
            .clone()
    };
    let task_id = "p0-03-hanging-mcp-lock-liveness";
    let registration = cancellation_registry
        .try_register(task_id)
        .expect("register hanging MCP execution owner");
    let call_state = Arc::clone(&state);
    let call = tokio::spawn(async move {
        let resources =
            crate::tool_gateway_resources::snapshot_tool_gateway_resources_for_main_chat_execution(
                &call_state,
            )
            .await
            .expect("Main Chat execution resources remain complete");
        let context = openlife_core::agent::ActionExecutionContext::new(
            &resources.governed.shared.registry,
            &resources.governed.shared.permission_store,
            &resources.governed.shared.audit_store,
            &resources.governed.shared.privacy_engine,
            &resources.governed.shared.safe_paths,
        )
        .with_memory_store(&resources.governed.memory_store)
        .with_network_policy(&resources.governed.network_policy)
        .with_calendar_ics_paths(&resources.governed.calendar_ics_paths)
        .with_agent_run_store(&resources.agent_run_store);
        let context = if let Some(retrieval_reader) = resources
            .governed
            .memory_lifecycle_retrieval_reader
            .as_ref()
        {
            context.with_memory_lifecycle_retrieval_reader(retrieval_reader)
        } else {
            context
        };
        let gateway = openlife_core::agent::ToolGateway::from_executor_config(Default::default());
        let execution = gateway.execute(
            openlife_core::agent::AgentActionRequest {
                action_type: "mcp_tool".into(),
                target: "p0_03_hang".into(),
                input: serde_json::json!({"arguments": {}}),
                source_run_id: Some("run-p0-03-hanging-mcp".into()),
                step_index: 0,
            },
            &context,
        );
        tokio::pin!(execution);
        tokio::select! {
            result = &mut execution => result.map(|_| "completed").map_err(|error| error.to_string()),
            _ = registration.token.cancelled() => Err("locally_cancelled".to_string()),
        }
    });

    tokio::time::timeout(Duration::from_secs(2), async {
        while !marker.exists() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("real MCP subprocess observes tools/call before lock probing");

    tokio::time::timeout(Duration::from_secs(1), async {
        let snapshot =
            crate::tool_gateway_resources::snapshot_tool_gateway_resources_for_main_chat_read(
                &state,
            )
            .await
            .expect("Main Chat read resources remain complete");
        snapshot
            .governed
            .shared
            .permission_store
            .list()
            .expect("permission store stays live during MCP read");
        assert_eq!(snapshot.governed.shared.registry.list_servers().len(), 1);
        snapshot
            .governed
            .shared
            .audit_store
            .list_logs(1)
            .expect("audit store stays live during MCP read");
        snapshot
            .governed
            .memory_store
            .list_sessions(1)
            .expect("memory store stays live during MCP read");
        state
            .proposal_store
            .as_ref()
            .expect("proposal store")
            .lock()
            .await
            .pending_count()
            .expect("proposal store stays live during MCP read");
    })
    .await
    .expect("MCP session lock must remain transport-owned, not an AppState registry guard");

    let cancel_started = Instant::now();
    let cancel = cancellation_registry.cancel(task_id);
    assert!(cancel.active_turn_found);
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(1), call)
            .await
            .expect("hanging MCP call terminates after cancellation")
            .expect("hanging MCP task joins"),
        Err("locally_cancelled".to_string())
    );
    assert!(cancel_started.elapsed() < Duration::from_secs(1));
}

#[tokio::test]
async fn streaming_command_exposes_first_provider_token_before_second_is_released() {
    use std::sync::atomic::Ordering;

    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let release_second_chunk =
        configure_live_provider_eval_state_with_barriered_streaming_local_http_provider(
            &state,
            vec![
                ("first", Duration::from_millis(10)),
                (" second", Duration::from_millis(10)),
            ],
        )
        .await;

    let started = Instant::now();
    let events = Arc::new(Mutex::new(
        Vec::<(String, serde_json::Value, Duration)>::new(),
    ));
    let captured = Arc::clone(&events);
    let state_for_turn = Arc::clone(&state);
    let turn = tokio::spawn(async move {
        crate::main_chat_streaming::start_stream_message_with_state(
            "phase2-real-token-stream".into(),
            vec![ChatMessage {
                role: "user".into(),
                content: "Reply with exactly: first second".into(),
            }],
            None,
            &state_for_turn,
            move |name, payload| {
                captured.lock().expect("capture streaming event").push((
                    name.to_string(),
                    payload,
                    started.elapsed(),
                ));
            },
        )
        .await
    });

    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let first_chunk_observed = events
                .lock()
                .expect("read streaming barrier events")
                .iter()
                .any(|event| {
                    event.0 == "stream-message-chunk"
                        && event.1.get("chunk").and_then(serde_json::Value::as_str) == Some("first")
                });
            if first_chunk_observed {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("the first token must reach the callback while the provider is still blocked");

    {
        let events_before_release = events.lock().expect("read pre-release streaming events");
        assert!(events_before_release.iter().all(|event| {
            event.0 != "stream-message-done"
                && !(event.0 == "stream-message-chunk"
                    && event.1.get("chunk").and_then(serde_json::Value::as_str) == Some(" second"))
        }));
    }
    assert!(
        !turn.is_finished(),
        "the turn cannot finish while the provider withholds its second token"
    );

    release_second_chunk.store(true, Ordering::SeqCst);
    tokio::time::timeout(Duration::from_secs(2), turn)
        .await
        .expect("streaming turn completes after the provider barrier is released")
        .expect("streaming task joins")
        .expect("real streaming command succeeds");

    let events = events.lock().expect("read streaming events");
    assert_eq!(
        events.first().map(|event| event.0.as_str()),
        Some("stream-message-start")
    );
    assert_eq!(
        events.last().map(|event| event.0.as_str()),
        Some("stream-message-done")
    );

    let chunks = events
        .iter()
        .filter(|event| event.0 == "stream-message-chunk")
        .map(|event| {
            (
                event.1.get("chunk").and_then(serde_json::Value::as_str),
                event.2,
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        chunks.iter().map(|(chunk, _)| *chunk).collect::<Vec<_>>(),
        vec![Some("first"), Some(" second")],
        "the command must forward provider token boundaries without a duplicate full reply"
    );
    assert!(
        chunks[0].1 < chunks[1].1,
        "the first provider token must be observable before the delayed second token"
    );

    let done_payload = events
        .iter()
        .find(|event| event.0 == "stream-message-done")
        .map(|event| &event.1)
        .expect("streaming done payload");
    let canonical_run_id = done_payload
        .get("run_id")
        .and_then(serde_json::Value::as_str)
        .expect("done payload canonical run id");
    let canonical_task_session_id = done_payload
        .get("task_session_id")
        .and_then(serde_json::Value::as_str)
        .expect("done payload canonical task session id");
    let start_payload = events
        .iter()
        .find(|event| event.0 == "stream-message-start")
        .map(|event| &event.1)
        .expect("stream start payload");
    assert_eq!(
        start_payload
            .get("task_session_id")
            .and_then(serde_json::Value::as_str),
        Some(canonical_task_session_id)
    );
    assert_eq!(
        start_payload
            .get("run_id")
            .and_then(serde_json::Value::as_str),
        Some(canonical_run_id)
    );
    let realtime_execution_events = events
        .iter()
        .filter(|event| {
            matches!(
                event.0.as_str(),
                "main-chat-kernel-event" | "stream-message-chunk"
            )
        })
        .collect::<Vec<_>>();
    assert!(!realtime_execution_events.is_empty());
    assert!(realtime_execution_events.iter().all(|event| {
        event
            .1
            .get("task_session_id")
            .and_then(serde_json::Value::as_str)
            == Some(canonical_task_session_id)
            && event
                .1
                .get("run_id")
                .and_then(serde_json::Value::as_str)
                == Some(canonical_run_id)
    }), "every realtime kernel and provider-token event must carry the same early canonical task/run identity");

    let last_durable = events
        .iter()
        .rposition(|event| event.0 == "main-chat-agent-event")
        .expect("durable events are emitted");
    let done = events
        .iter()
        .rposition(|event| event.0 == "stream-message-done")
        .expect("done event is emitted");
    assert!(
        last_durable < done,
        "durable facts must be emitted before final done"
    );
}

#[tokio::test]
async fn missing_agent_run_store_fails_before_provider_dispatch() {
    use std::sync::atomic::Ordering;

    let mut state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    Arc::get_mut(&mut state)
        .expect("isolated state has no other owner before provider setup")
        .agent_run_store = None;
    let (request_observed, _client_closed, _release_late_response, _late_response_attempted) =
        configure_live_provider_eval_state_with_hanging_local_http_provider(&state).await;

    let result = tokio::time::timeout(
        Duration::from_secs(1),
        crate::main_chat_streaming::start_stream_message_with_state(
            "phase2-missing-early-agent-run-store".into(),
            vec![ChatMessage {
                role: "user".into(),
                content: "Contact the provider and answer in one sentence.".into(),
            }],
            None,
            &state,
            |_name, _payload| {},
        ),
    )
    .await
    .expect("missing canonical run store fails promptly");
    let error = result.expect_err("provider dispatch requires a persisted canonical AgentRun");
    assert!(
        error.contains("agent_run_store_unavailable"),
        "unexpected early AgentRun failure: {error}"
    );
    assert!(
        !request_observed.load(Ordering::SeqCst),
        "provider HTTP dispatch must remain unreachable when AgentRun persistence is unavailable"
    );
}

#[tokio::test]
async fn pre_registration_cancel_never_first_polls_kernel_or_dispatches_provider_or_tool() {
    use std::sync::atomic::Ordering;

    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let (request_observed, _client_closed, _release_late_response, _late_response_attempted) =
        configure_live_provider_eval_state_with_hanging_local_http_provider(&state).await;
    let chat_session_id = "phase2-pre-registration-cancel";
    let (_barrier_guard, reached, release, kernel_first_poll_count) =
        crate::main_chat_turn_runtime::install_main_chat_pre_registration_barrier_for_test(
            chat_session_id,
        );
    let state_for_turn = Arc::clone(&state);
    let turn = tokio::spawn(async move {
        crate::main_chat_streaming::start_stream_message_with_state(
            chat_session_id.into(),
            vec![ChatMessage {
                role: "user".into(),
                content: "Contact the provider and answer in one sentence.".into(),
            }],
            None,
            &state_for_turn,
            |_name, _payload| {},
        )
        .await
    });

    tokio::time::timeout(Duration::from_secs(1), reached.wait())
        .await
        .expect("turn reaches the deterministic pre-registration barrier");
    let task_session_id = {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("task session store")
            .lock()
            .await;
        store
            .list_sessions(None, 10, 0)
            .expect("list pre-registration task")
            .into_iter()
            .find(|session| session.chat_session_id == chat_session_id)
            .expect("exact pre-registration task exists")
            .id
    };
    let pending = crate::main_chat_task_controls::cancel_main_chat_agent_task_with_state(
        &task_session_id,
        &state,
    )
    .await
    .expect("task-before-run cancellation must be accepted as pending backend truth");
    assert!(pending.cancellation_pending);
    assert!(!pending.can_cancel);
    assert_eq!(
        pending.session.as_ref().map(|session| session.status),
        Some(openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Running)
    );
    assert!(
        crate::main_chat_event_stream::list_main_chat_agent_events_with_state(
            &state,
            task_session_id.clone(),
            None,
            Some(100),
        )
        .await
        .expect("list pre-run cancellation facts")
        .is_empty(),
        "pending cancellation before exact AgentRun ownership must not fabricate a terminal event"
    );
    release.wait().await;

    let done = tokio::time::timeout(Duration::from_secs(2), turn)
        .await
        .expect("pre-registration cancellation completes promptly")
        .expect("turn task joins")
        .expect("pre-registration cancellation returns a structured terminal");
    assert_eq!(
        done.get("status").and_then(serde_json::Value::as_str),
        Some("cancelled")
    );
    assert_eq!(
        kernel_first_poll_count.load(Ordering::SeqCst),
        0,
        "a cancellation known at registration must stop before the kernel future is first-polled"
    );
    assert!(
        !request_observed.load(Ordering::SeqCst),
        "pre-registration cancellation must keep provider HTTP unreachable"
    );
    let durable = crate::main_chat_event_stream::list_main_chat_agent_events_with_state(
        &state,
        task_session_id.clone(),
        None,
        Some(100),
    )
    .await
    .expect("list pre-registration cancellation facts");
    assert!(durable
        .iter()
        .any(|event| event.event_type == "cancel_requested"));
    assert!(durable
        .iter()
        .any(|event| event.event_type == "local_aborted"));
    assert!(durable.iter().all(|event| !matches!(
        event.event_type.as_str(),
        "provider.started" | "tool.started" | "provider.completed" | "tool.completed"
    )));
    let exact_run = state
        .agent_run_store
        .as_ref()
        .expect("AgentRun store")
        .lock()
        .await
        .get_run_for_task_id(&task_session_id)
        .expect("load exact cancelled AgentRun")
        .expect("cancelled AgentRun exists");
    assert_eq!(
        exact_run.status,
        openlife_core::agent::AgentRunStatus::Cancelled
    );
    assert!(durable
        .iter()
        .all(|event| event.run_id == exact_run.id && event.task_session_id == task_session_id));
    assert_eq!(
        durable
            .iter()
            .filter(|event| event.event_type == "cancel_requested")
            .count(),
        1
    );
    assert_eq!(
        durable
            .iter()
            .filter(|event| event.event_type == "local_aborted")
            .count(),
        1
    );
    let registry = state
        .main_chat_runtime_state
        .lock()
        .await
        .cancellation_registry
        .clone();
    assert!(
        !registry.is_cancellation_requested(&task_session_id),
        "pending cancellation truth clears only after durable runtime terminalization"
    );
}

#[tokio::test]
async fn one_turn_keeps_the_entry_provider_generation_after_runtime_config_replacement() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let entry_provider_requests =
        configure_live_provider_eval_state_with_captured_local_http_provider(
            &state,
            "entry provider generation response",
        )
        .await;
    let entry_snapshot = state.provider_runtime_snapshot().await;
    assert!(entry_snapshot.coherent);
    let entry_generation = entry_snapshot
        .scheduler
        .provider_config_generation()
        .to_string();

    let chat_session_id = "phase2-provider-generation-entry-snapshot";
    let (_barrier_guard, reached, release, _kernel_first_poll_count) =
        crate::main_chat_turn_runtime::install_main_chat_pre_registration_barrier_for_test(
            chat_session_id,
        );
    let state_for_turn = Arc::clone(&state);
    let turn = tokio::spawn(async move {
        crate::main_chat_send::send_message_with_state(
            chat_session_id.into(),
            vec![ChatMessage {
                role: "user".into(),
                content: "Contact the configured provider and answer with its exact response."
                    .into(),
            }],
            None,
            &state_for_turn,
        )
        .await
    });

    tokio::time::timeout(Duration::from_secs(1), reached.wait())
        .await
        .expect("turn reaches the deterministic post-entry snapshot barrier");

    let replacement_provider_requests =
        configure_live_provider_eval_state_with_captured_local_http_provider(
            &state,
            "replacement provider generation response",
        )
        .await;
    let replacement_snapshot = state.provider_runtime_snapshot().await;
    assert!(replacement_snapshot.coherent);
    assert_ne!(
        replacement_snapshot.scheduler.provider_config_generation(),
        entry_generation,
        "the counterfactual must replace the executable provider generation"
    );
    release.wait().await;

    let response = tokio::time::timeout(Duration::from_secs(5), turn)
        .await
        .expect("turn completes after provider generation replacement")
        .expect("turn task joins")
        .expect("entry provider generation remains executable");
    assert_eq!(response.reply, "entry provider generation response");
    assert_eq!(
        entry_provider_requests
            .lock()
            .expect("entry request capture")
            .len(),
        1,
        "the turn must dispatch through the provider generation captured at entry"
    );
    assert!(
        replacement_provider_requests
            .lock()
            .expect("replacement request capture")
            .is_empty(),
        "a config replacement during the turn must not redirect that turn"
    );
    let generation_projection = response
        .reasoning_trace
        .generation_result
        .as_ref()
        .expect("final generation projection");
    assert_eq!(
        generation_projection
            .get("turnProviderRuntimeGeneration")
            .and_then(serde_json::Value::as_str),
        Some(entry_generation.as_str())
    );
    assert_eq!(
        generation_projection
            .get("providerReceiptConfigGeneration")
            .and_then(serde_json::Value::as_str),
        Some(entry_generation.as_str())
    );

    let task_session_id = response
        .agent_ingress
        .as_ref()
        .and_then(|decision| decision.agent_task_session_id.as_deref())
        .expect("turn owns one canonical task session");
    let provider_events = crate::main_chat_event_stream::list_main_chat_agent_events_with_state(
        &state,
        task_session_id.to_string(),
        None,
        Some(100),
    )
    .await
    .expect("list provider receipt events")
    .into_iter()
    .filter(|event| event.event_type.starts_with("provider."))
    .collect::<Vec<_>>();
    assert!(
        !provider_events.is_empty(),
        "real provider receipts are durable"
    );
    assert!(
        provider_events.iter().all(|event| {
            event
                .payload
                .get("providerConfigGeneration")
                .and_then(serde_json::Value::as_str)
                == Some(entry_generation.as_str())
        }),
        "every durable provider receipt must remain bound to the entry snapshot generation"
    );
}

#[test]
fn turn_runtime_is_the_only_main_chat_provider_snapshot_capture_owner() {
    let runtime = include_str!("main_chat_turn_runtime.rs");
    let kernel = include_str!("main_chat_kernel.rs");
    let react = include_str!("main_chat_react_runtime.rs");

    assert_eq!(
        runtime.matches("provider_runtime_snapshot().await").count(),
        1,
        "OpenLifeTurnRuntime must capture one immutable provider generation per turn"
    );
    assert!(
        kernel.matches("provider_runtime_snapshot().await").count() == 0
            && react.matches("provider_runtime_snapshot().await").count() == 0,
        "kernel and AgentLoop must consume the TurnRuntime snapshot, not recapture mutable state"
    );
}

#[tokio::test]
async fn agent_run_create_failure_fails_before_provider_dispatch() {
    use std::sync::atomic::Ordering;

    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    {
        let run_store = state
            .agent_run_store
            .as_ref()
            .expect("agent run store")
            .lock()
            .await;
        run_store
            .install_create_failure_for_test()
            .expect("install deterministic AgentRun create failure");
    }
    let (request_observed, _client_closed, _release_late_response, _late_response_attempted) =
        configure_live_provider_eval_state_with_hanging_local_http_provider(&state).await;

    let emitted = Arc::new(Mutex::new(Vec::<String>::new()));
    let captured = Arc::clone(&emitted);
    let result = tokio::time::timeout(
        Duration::from_secs(1),
        crate::main_chat_streaming::start_stream_message_with_state(
            "phase2-agent-run-create-failure".into(),
            vec![ChatMessage {
                role: "user".into(),
                content: "Contact the provider and answer in one sentence.".into(),
            }],
            None,
            &state,
            move |name, _payload| {
                captured.lock().unwrap().push(name.to_string());
            },
        ),
    )
    .await
    .expect("AgentRun create failure returns promptly");
    let error = result.expect_err("AgentRun create failure must fail closed");
    assert!(
        error.contains("injected agent run create failure"),
        "unexpected AgentRun create error: {error}"
    );
    assert!(!request_observed.load(Ordering::SeqCst));
    assert!(
        emitted.lock().unwrap().is_empty(),
        "stream start cannot be emitted before canonical AgentRun persistence"
    );
    let sessions = state
        .main_chat_agent_session_store
        .as_ref()
        .expect("task session store")
        .lock()
        .await
        .list_sessions(None, 50, 0)
        .expect("list task sessions after AgentRun create failure");
    assert!(sessions.iter().all(|session| {
        session.status != openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Running
    }));
    assert!(sessions.iter().any(|session| {
        session.status == openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Failed
    }));
}

#[tokio::test]
async fn cancelled_turn_rejects_a_released_late_provider_response_and_durable_commit() {
    use std::sync::atomic::Ordering;

    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let (request_observed, client_closed, release_late_response, late_response_attempted) =
        configure_live_provider_eval_state_with_hanging_local_http_provider(&state).await;
    let events = Arc::new(Mutex::new(Vec::<(String, serde_json::Value)>::new()));
    let captured = Arc::clone(&events);
    let state_for_turn = Arc::clone(&state);
    let turn = tokio::spawn(async move {
        crate::main_chat_streaming::start_stream_message_with_state(
            "phase2-cancel-hanging-provider".into(),
            vec![ChatMessage {
                role: "user".into(),
                content: "Reply with one sentence after contacting the provider.".into(),
            }],
            None,
            &state_for_turn,
            move |name, payload| {
                captured
                    .lock()
                    .expect("capture cancellation stream event")
                    .push((name.to_string(), payload));
            },
        )
        .await
    });

    tokio::time::timeout(Duration::from_secs(2), async {
        while !request_observed.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("provider request is observed before cancellation");

    let task_session_id = {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("task session store")
            .lock()
            .await;
        store
            .list_sessions(None, 10, 0)
            .expect("list task sessions")
            .into_iter()
            .find(|session| session.chat_session_id == "phase2-cancel-hanging-provider")
            .expect("active task session")
            .id
    };
    let early_run = {
        let run_store = state
            .agent_run_store
            .as_ref()
            .expect("agent run store")
            .lock()
            .await;
        let runs = run_store
            .list_runs_for_session("phase2-cancel-hanging-provider", 10)
            .expect("list early provider turn runs");
        assert_eq!(
            runs.len(),
            1,
            "one canonical AgentRun must exist before the provider request is observed"
        );
        runs.into_iter().next().unwrap()
    };
    assert_eq!(early_run.task_id, task_session_id);
    assert_eq!(
        early_run.status,
        openlife_core::agent::AgentRunStatus::Running
    );

    let cancel_started = Instant::now();
    crate::main_chat_task_controls::cancel_main_chat_agent_task_with_state(
        &task_session_id,
        &state,
    )
    .await
    .expect("cancel active turn");
    let done_payload = tokio::time::timeout(Duration::from_secs(1), turn)
        .await
        .expect("local cancellation completes within one second")
        .expect("turn task joins")
        .expect("cancelled turn returns structured terminal payload");
    assert!(cancel_started.elapsed() < Duration::from_secs(1));
    tokio::time::timeout(Duration::from_secs(1), async {
        while !client_closed.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("dropping the local provider future closes the local HTTP connection");
    assert_eq!(
        done_payload
            .get("status")
            .and_then(serde_json::Value::as_str),
        Some("cancelled")
    );
    assert_eq!(
        done_payload
            .get("reasoning_trace")
            .and_then(|value| value.get("generation_result"))
            .and_then(|value| value.get("providerStatus"))
            .and_then(serde_json::Value::as_str),
        Some("remote_unknown")
    );
    assert_eq!(
        done_payload
            .get("run_id")
            .and_then(serde_json::Value::as_str),
        Some(early_run.id.as_str())
    );

    let durable_before = crate::main_chat_event_stream::list_main_chat_agent_events_with_state(
        &state,
        task_session_id.clone(),
        None,
        Some(100),
    )
    .await
    .expect("list cancellation facts");
    let event_types = durable_before
        .iter()
        .map(|event| event.event_type.as_str())
        .collect::<Vec<_>>();
    assert!(event_types.contains(&"cancel_requested"));
    assert!(event_types.contains(&"local_aborted"));
    assert!(event_types.contains(&"provider.remote_unknown"));
    assert!(!event_types.contains(&"effect_committed"));
    assert!(!event_types.contains(&"provider.completed"));
    assert!(
        durable_before
            .iter()
            .all(|event| event.run_id == early_run.id),
        "every cancellation fact must retain the early canonical AgentRun id"
    );
    let cancellation_object_ids = durable_before
        .iter()
        .filter(|event| {
            matches!(
                event.event_type.as_str(),
                "cancel_requested" | "local_aborted"
            )
        })
        .map(|event| event.object_id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(cancellation_object_ids.len(), 2);
    assert!(
        cancellation_object_ids
            .windows(2)
            .all(|pair| pair[0] == pair[1]),
        "one cancellation must use one stable object reference for both turn facts"
    );
    let remote_unknown = durable_before
        .iter()
        .find(|event| event.event_type == "provider.remote_unknown")
        .expect("remote provider uncertainty fact");
    let provider_started = durable_before
        .iter()
        .find(|event| event.event_type == "provider.started")
        .expect("provider start fact");
    assert_eq!(
        remote_unknown.object_id, provider_started.object_id,
        "provider cancellation facts retain the real provider request id"
    );
    assert_ne!(
        remote_unknown.object_id, cancellation_object_ids[0],
        "turn cancellation id must not replace the provider request id"
    );
    assert_eq!(
        remote_unknown
            .payload
            .get("localWaitAborted")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        remote_unknown
            .payload
            .get("localKernelFutureDropped")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
    assert_eq!(
        remote_unknown
            .payload
            .get("remoteCancellationConfirmed")
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );
    assert!(
        remote_unknown
            .payload
            .get("localRequestHandleAborted")
            .is_none(),
        "the durable fact must not claim transport abort without mechanical evidence"
    );

    let session_before_late_response = {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("task session store")
            .lock()
            .await;
        store
            .load_session(&task_session_id)
            .expect("load cancelled session")
            .expect("cancelled session exists")
    };

    release_late_response.store(true, Ordering::SeqCst);
    tokio::time::timeout(Duration::from_secs(1), async {
        while !late_response_attempted.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("the provider attempts to release its response after local cancellation");
    tokio::task::yield_now().await;

    let durable_after = crate::main_chat_event_stream::list_main_chat_agent_events_with_state(
        &state,
        task_session_id.clone(),
        None,
        Some(100),
    )
    .await
    .expect("recheck cancellation facts");
    assert_eq!(
        durable_after.len(),
        durable_before.len(),
        "no late durable event may arrive after local cancellation"
    );
    let session_after_late_response = {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("task session store")
            .lock()
            .await;
        store
            .load_session(&task_session_id)
            .expect("reload cancelled session")
            .expect("cancelled session still exists")
    };
    assert_eq!(
        session_after_late_response.updated_at, session_before_late_response.updated_at,
        "a late provider response must not mutate the cancelled canonical task session"
    );
    assert_eq!(
        session_after_late_response.final_summary,
        session_before_late_response.final_summary
    );
    let cancelled_run = {
        let run_store = state
            .agent_run_store
            .as_ref()
            .expect("agent run store")
            .lock()
            .await;
        run_store
            .get_run(&early_run.id)
            .expect("load cancelled early run")
            .expect("cancelled early run exists")
    };
    assert_eq!(
        cancelled_run.status,
        openlife_core::agent::AgentRunStatus::Cancelled
    );

    let captured = events.lock().expect("read cancellation stream events");
    assert_eq!(
        captured.last().map(|event| event.0.as_str()),
        Some("stream-message-done")
    );
}

#[tokio::test]
async fn stream_done_is_not_emitted_when_cancellation_durable_batch_fails() {
    use std::sync::atomic::Ordering;

    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let (request_observed, _client_closed, release_late_response, _late_response_attempted) =
        configure_live_provider_eval_state_with_hanging_local_http_provider(&state).await;
    {
        let event_store = state
            .main_chat_agent_event_store
            .as_ref()
            .expect("durable event store")
            .lock()
            .await;
        event_store
            .install_local_aborted_insert_failure_for_test()
            .expect("install deterministic second-draft failure");
    }

    let events = Arc::new(Mutex::new(Vec::<(String, serde_json::Value)>::new()));
    let captured = Arc::clone(&events);
    let state_for_turn = Arc::clone(&state);
    let turn = tokio::spawn(async move {
        crate::main_chat_streaming::start_stream_message_with_state(
            "phase2-cancel-durable-batch-failure".into(),
            vec![ChatMessage {
                role: "user".into(),
                content: "Contact the provider, then wait for cancellation.".into(),
            }],
            None,
            &state_for_turn,
            move |name, payload| {
                captured
                    .lock()
                    .expect("capture failed cancellation stream event")
                    .push((name.to_string(), payload));
            },
        )
        .await
    });

    tokio::time::timeout(Duration::from_secs(2), async {
        while !request_observed.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("provider request is observed before cancellation");

    let task_session_id = {
        let session_store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("task session store")
            .lock()
            .await;
        session_store
            .list_sessions(None, 10, 0)
            .expect("list task sessions")
            .into_iter()
            .find(|session| session.chat_session_id == "phase2-cancel-durable-batch-failure")
            .expect("active task session")
            .id
    };

    crate::main_chat_task_controls::cancel_main_chat_agent_task_with_state(
        &task_session_id,
        &state,
    )
    .await
    .expect("request cancellation");
    let error = tokio::time::timeout(Duration::from_secs(1), turn)
        .await
        .expect("failed durable batch returns without emitting done")
        .expect("turn task joins")
        .expect_err("the injected durable batch failure must surface");
    assert!(
        error.contains("injected local_aborted event failure"),
        "unexpected durable batch error: {error}"
    );

    release_late_response.store(true, Ordering::SeqCst);

    let captured = events.lock().expect("read failed cancellation events");
    assert!(
        captured
            .iter()
            .all(|(event_name, _)| event_name != "stream-message-done"),
        "stream done must remain behind a successfully committed durable cancellation batch"
    );
    drop(captured);

    let durable_events = crate::main_chat_event_stream::list_main_chat_agent_events_with_state(
        &state,
        task_session_id,
        None,
        Some(100),
    )
    .await
    .expect("list durable facts after injected batch failure");
    assert!(
        durable_events.iter().all(|event| !matches!(
            event.event_type.as_str(),
            "cancel_requested"
                | "local_aborted"
                | "provider.started"
                | "provider.completed"
                | "provider.failed"
                | "provider.remote_unknown"
        )),
        "a failed cancellation batch must leave none of its durable facts committed"
    );
}

#[tokio::test]
async fn conflicting_provider_attempt_metadata_finalizes_failed_with_unknown_receipt_state() {
    use std::sync::atomic::Ordering;

    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let (request_observed, _client_closed, release_late_response, _late_response_attempted) =
        configure_live_provider_eval_state_with_hanging_local_http_provider(&state).await;
    let events = Arc::new(Mutex::new(Vec::<(String, serde_json::Value)>::new()));
    let captured = Arc::clone(&events);
    let state_for_turn = Arc::clone(&state);
    let turn = tokio::spawn(async move {
        crate::main_chat_streaming::start_stream_message_with_state(
            "phase2-provider-attempt-metadata-conflict".into(),
            vec![ChatMessage {
                role: "user".into(),
                content: "Contact the provider and wait.".into(),
            }],
            None,
            &state_for_turn,
            move |name, payload| {
                captured
                    .lock()
                    .expect("capture provider conflict stream events")
                    .push((name.to_string(), payload));
            },
        )
        .await
    });

    tokio::time::timeout(Duration::from_secs(2), async {
        while !request_observed.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("provider request is observed before conflict injection");

    let provider_request_id = events
        .lock()
        .expect("read provider start before conflict")
        .iter()
        .find(|(name, payload)| {
            name == "main-chat-kernel-event"
                && payload.get("type").and_then(serde_json::Value::as_str)
                    == Some("provider_started")
        })
        .and_then(|(_, payload)| payload.get("request_id"))
        .and_then(serde_json::Value::as_str)
        .expect("provider request id before conflict")
        .to_string();
    let task_session_id = {
        let session_store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("task session store")
            .lock()
            .await;
        session_store
            .list_sessions(None, 10, 0)
            .expect("list conflict task sessions")
            .into_iter()
            .find(|session| session.chat_session_id == "phase2-provider-attempt-metadata-conflict")
            .expect("active conflict task session")
            .id
    };
    let canonical_run_id = {
        let run_store = state
            .agent_run_store
            .as_ref()
            .expect("agent run store")
            .lock()
            .await;
        let runs = run_store
            .list_runs_for_session("phase2-provider-attempt-metadata-conflict", 10)
            .expect("list conflict AgentRuns");
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].task_id, task_session_id);
        assert_eq!(
            runs[0].status,
            openlife_core::agent::AgentRunStatus::Running
        );
        runs[0].id.clone()
    };
    let registry = {
        state
            .main_chat_runtime_state
            .lock()
            .await
            .cancellation_registry
            .clone()
    };
    assert_eq!(
        registry
            .record_provider_completed(
                &task_session_id,
                &provider_request_id,
                "openai",
                "conflicting-model",
                chrono::Utc::now(),
            )
            .expect_err("conflicting model for one request id must fail closed"),
        crate::main_chat_cancellation::MainChatProviderAttemptError::MetadataConflict
    );

    let done = tokio::time::timeout(Duration::from_secs(1), turn)
        .await
        .expect("metadata conflict terminalizes promptly")
        .expect("provider conflict turn joins")
        .expect("provider conflict returns structured failed delivery");
    assert_eq!(
        done.get("status").and_then(serde_json::Value::as_str),
        Some("failed")
    );
    assert_eq!(
        done.get("run_id").and_then(serde_json::Value::as_str),
        Some(canonical_run_id.as_str())
    );
    assert_eq!(
        done.get("model_invoked")
            .and_then(serde_json::Value::as_bool),
        Some(true),
        "a receipt-state failure must not erase the already observed provider dispatch"
    );
    assert_eq!(
        done.get("reasoning_trace")
            .and_then(|value| value.get("generation_result"))
            .and_then(|value| value.get("providerStatus"))
            .and_then(serde_json::Value::as_str),
        Some("unknown")
    );

    let stored_session = {
        let session_store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("task session store")
            .lock()
            .await;
        session_store
            .load_session(&task_session_id)
            .expect("load conflict session")
            .expect("conflict session exists")
    };
    assert_eq!(
        stored_session.status,
        openlife_core::agent::main_chat_agent_v1::AgentTaskSessionStatus::Failed
    );

    let durable_events = crate::main_chat_event_stream::list_main_chat_agent_events_with_state(
        &state,
        task_session_id,
        None,
        Some(100),
    )
    .await
    .expect("list conflict terminal facts");
    assert!(durable_events
        .iter()
        .any(|event| event.event_type == "provider.receipt_state_failed"));
    assert!(durable_events.iter().any(|event| {
        event.event_type == "provider.started"
            && event.object_type == "provider_request"
            && event.object_id == provider_request_id
    }));
    assert!(durable_events
        .iter()
        .any(|event| event.event_type == "failed"));
    assert!(durable_events
        .iter()
        .all(|event| event.event_type != "provider.completed"));
    assert!(durable_events
        .iter()
        .all(|event| event.run_id == canonical_run_id));

    let failed_run = state
        .agent_run_store
        .as_ref()
        .expect("agent run store")
        .lock()
        .await
        .get_run(&canonical_run_id)
        .expect("load conflict AgentRun")
        .expect("conflict AgentRun exists");
    assert_eq!(
        failed_run.status,
        openlife_core::agent::AgentRunStatus::Failed
    );

    release_late_response.store(true, Ordering::SeqCst);
}

#[tokio::test]
async fn cancelled_hanging_react_candidate_ranking_exposes_provider_start_before_abort() {
    use std::sync::atomic::Ordering;

    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let (request_observed, _client_closed, _release_late_response, _late_response_attempted) =
        configure_live_provider_eval_state_with_hanging_local_http_provider(&state).await;
    let events = Arc::new(Mutex::new(Vec::<(String, serde_json::Value)>::new()));
    let captured = Arc::clone(&events);
    let state_for_turn = Arc::clone(&state);
    let turn = tokio::spawn(async move {
        crate::main_chat_streaming::start_stream_message_with_state(
            "phase2-cancel-hanging-react-ranking".into(),
            vec![ChatMessage {
                role: "user".into(),
                content: "Use an mcp read-only utility tool now.".into(),
            }],
            None,
            &state_for_turn,
            move |name, payload| {
                captured
                    .lock()
                    .expect("capture ReAct cancellation events")
                    .push((name.to_string(), payload));
            },
        )
        .await
    });

    tokio::time::timeout(Duration::from_secs(2), async {
        while !request_observed.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("candidate-ranking provider request is observed before cancellation");

    let provider_started = events
        .lock()
        .expect("read pre-cancel ReAct events")
        .iter()
        .find(|(name, payload)| {
            name == "main-chat-kernel-event"
                && payload.get("type").and_then(serde_json::Value::as_str)
                    == Some("provider_started")
        })
        .map(|(_, payload)| payload.clone())
        .expect("provider start must be emitted while candidate ranking is still hanging");
    let provider_request_id = provider_started
        .get("request_id")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .expect("provider request id")
        .to_string();
    assert_eq!(
        provider_started
            .get("provider")
            .and_then(serde_json::Value::as_str),
        Some("openai")
    );
    assert_eq!(
        provider_started
            .get("model")
            .and_then(serde_json::Value::as_str),
        Some("gpt-local-provider-harness")
    );
    assert!(provider_started.get("started_at").is_some());

    let task_session_id = {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("task session store")
            .lock()
            .await;
        store
            .list_sessions(None, 10, 0)
            .expect("list ReAct task sessions")
            .into_iter()
            .find(|session| session.chat_session_id == "phase2-cancel-hanging-react-ranking")
            .expect("active ReAct task session")
            .id
    };
    crate::main_chat_task_controls::cancel_main_chat_agent_task_with_state(
        &task_session_id,
        &state,
    )
    .await
    .expect("cancel hanging ReAct candidate ranking");
    let done = tokio::time::timeout(Duration::from_secs(1), turn)
        .await
        .expect("ReAct candidate-ranking cancellation completes locally")
        .expect("ReAct turn task joins")
        .expect("ReAct turn returns a cancellation payload");
    assert_eq!(
        done.get("status").and_then(serde_json::Value::as_str),
        Some("cancelled")
    );

    let durable_events = crate::main_chat_event_stream::list_main_chat_agent_events_with_state(
        &state,
        task_session_id,
        None,
        Some(100),
    )
    .await
    .expect("list request-level hanging ranking facts");
    assert!(durable_events.iter().any(|event| {
        event.event_type == "provider.started" && event.object_id == provider_request_id
    }));
    assert!(durable_events.iter().any(|event| {
        event.event_type == "provider.remote_unknown" && event.object_id == provider_request_id
    }));
    assert!(durable_events.iter().all(|event| {
        !(event.event_type == "provider.completed" && event.object_id == provider_request_id)
    }));
    let sequence_for = |event_type: &str| {
        durable_events
            .iter()
            .find(|event| event.event_type == event_type && event.object_id == provider_request_id)
            .map(|event| event.sequence)
            .unwrap_or_else(|| panic!("missing {event_type} for hanging ranking request"))
    };
    let cancel_sequence = durable_events
        .iter()
        .find(|event| event.event_type == "cancel_requested")
        .expect("cancel requested fact")
        .sequence;
    let local_aborted_sequence = durable_events
        .iter()
        .find(|event| event.event_type == "local_aborted")
        .expect("local aborted fact")
        .sequence;
    assert!(sequence_for("provider.started") < cancel_sequence);
    assert!(cancel_sequence < sequence_for("provider.remote_unknown"));
    assert!(sequence_for("provider.remote_unknown") < local_aborted_sequence);
}

#[tokio::test]
async fn cancelled_hanging_agent_loop_generation_keeps_completed_ranking_fact() {
    use std::sync::atomic::{AtomicBool, Ordering};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind two-stage provider");
    let provider_base = format!("http://{}/v1", listener.local_addr().unwrap());
    let agent_loop_request_observed = Arc::new(AtomicBool::new(false));
    let observed_for_server = Arc::clone(&agent_loop_request_observed);
    let captured_request_ids = Arc::new(Mutex::new(Vec::<String>::new()));
    let captured_request_ids_for_server = Arc::clone(&captured_request_ids);
    let release_hanging_request = Arc::new(tokio::sync::Notify::new());
    let release_for_server = Arc::clone(&release_hanging_request);
    let provider_server = tokio::spawn(async move {
        for request_index in 0..2 {
            let (mut socket, _) = listener.accept().await.expect("accept provider request");
            let mut request = Vec::new();
            let mut buffer = [0_u8; 8_192];
            loop {
                let read = socket
                    .read(&mut buffer)
                    .await
                    .expect("read provider request");
                if read == 0 {
                    return;
                }
                request.extend_from_slice(&buffer[..read]);
                let text = String::from_utf8_lossy(&request);
                let complete = text.find("\r\n\r\n").is_some_and(|header_end| {
                    let content_length = text[..header_end]
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().ok())
                                .flatten()
                        })
                        .unwrap_or(0);
                    request.len() >= header_end + 4 + content_length
                });
                if complete {
                    break;
                }
            }
            let request_text = String::from_utf8_lossy(&request);
            let request_id = request_text
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("x-openlife-request-id")
                        .then(|| value.trim().to_string())
                })
                .filter(|value| !value.is_empty())
                .expect("provider request carries x-openlife-request-id");
            captured_request_ids_for_server
                .lock()
                .expect("capture provider request ids")
                .push(request_id);

            if request_index == 0 {
                let body = serde_json::json!({
                    "id": "chatcmpl-ranking-completed",
                    "object": "chat.completion",
                    "choices": [{
                        "index": 0,
                        "message": { "role": "assistant", "content": "{}" },
                        "finish_reason": "stop"
                    }]
                })
                .to_string();
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                );
                socket
                    .write_all(response.as_bytes())
                    .await
                    .expect("complete ranking response");
            } else {
                observed_for_server.store(true, Ordering::SeqCst);
                release_for_server.notified().await;
            }
        }
    });
    let mut provider_config = state.config.lock().await.clone();
    provider_config.llm.provider = "openai".into();
    provider_config.llm.openai_base = provider_base;
    provider_config.llm.chat_model = "gpt-two-stage-provider".into();
    provider_config.llm.openai_key = "test-key".into();
    provider_config.prefer_local_model = false;
    provider_config.system.network_policy.enabled = true;
    provider_config.system.network_policy.default_decision = "allow".into();
    state.replace_provider_runtime_config(provider_config).await;

    let events = Arc::new(Mutex::new(Vec::<(String, serde_json::Value)>::new()));
    let captured = Arc::clone(&events);
    let state_for_turn = Arc::clone(&state);
    let mut turn = tokio::spawn(async move {
        crate::main_chat_streaming::start_stream_message_with_state(
            "phase2-cancel-hanging-agent-loop".into(),
            vec![ChatMessage {
                role: "user".into(),
                content: "Use an mcp read-only utility tool now.".into(),
            }],
            None,
            &state_for_turn,
            move |name, payload| {
                captured
                    .lock()
                    .expect("capture two-stage provider events")
                    .push((name.to_string(), payload));
            },
        )
        .await
    });

    let agent_loop_start_wait = tokio::time::timeout(Duration::from_secs(2), async {
        while !agent_loop_request_observed.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;
    if agent_loop_start_wait.is_err() {
        let turn_finished = turn.is_finished();
        let finished_turn_result = if turn_finished {
            Some((&mut turn).await)
        } else {
            None
        };
        let observed_events = events
            .lock()
            .expect("read two-stage timeout events")
            .iter()
            .map(|(name, payload)| {
                (
                    name.clone(),
                    payload
                        .get("type")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("<none>")
                        .to_string(),
                )
            })
            .collect::<Vec<_>>();
        let observed_request_ids = captured_request_ids
            .lock()
            .expect("read two-stage timeout provider ids")
            .clone();
        panic!(
            "AgentLoop provider request was not dispatched after ranking completed: turn_finished={turn_finished}, turn_result={finished_turn_result:?}, provider_request_ids={observed_request_ids:?}, events={observed_events:?}"
        );
    }

    let (started_ids, completed_ids) = {
        let events = events.lock().expect("read two-stage events");
        let started_ids = events
            .iter()
            .filter(|(name, payload)| {
                name == "main-chat-kernel-event"
                    && payload.get("type").and_then(serde_json::Value::as_str)
                        == Some("provider_started")
            })
            .filter_map(|(_, payload)| payload.get("request_id")?.as_str().map(str::to_string))
            .collect::<Vec<_>>();
        let completed_ids = events
            .iter()
            .filter(|(name, payload)| {
                name == "main-chat-kernel-event"
                    && payload.get("type").and_then(serde_json::Value::as_str)
                        == Some("provider_completed")
            })
            .filter_map(|(_, payload)| payload.get("request_id")?.as_str().map(str::to_string))
            .collect::<Vec<_>>();
        (started_ids, completed_ids)
    };
    assert_eq!(
        started_ids.len(),
        2,
        "ranking and AgentLoop starts are distinct"
    );
    assert_eq!(completed_ids, vec![started_ids[0].clone()]);
    assert_ne!(started_ids[0], started_ids[1]);
    let captured_request_ids = captured_request_ids
        .lock()
        .expect("read provider request header ids")
        .clone();
    assert_eq!(
        captured_request_ids, started_ids,
        "kernel attempt ids must equal the x-openlife-request-id values observed at HTTP"
    );

    let task_session_id = {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("task session store")
            .lock()
            .await;
        store
            .list_sessions(None, 10, 0)
            .expect("list AgentLoop task sessions")
            .into_iter()
            .find(|session| session.chat_session_id == "phase2-cancel-hanging-agent-loop")
            .expect("active AgentLoop task session")
            .id
    };
    crate::main_chat_task_controls::cancel_main_chat_agent_task_with_state(
        &task_session_id,
        &state,
    )
    .await
    .expect("cancel hanging AgentLoop provider request");
    let done = tokio::time::timeout(Duration::from_secs(1), turn)
        .await
        .expect("AgentLoop cancellation completes locally")
        .expect("AgentLoop turn task joins")
        .expect("AgentLoop turn returns cancellation payload");
    assert_eq!(
        done.get("status").and_then(serde_json::Value::as_str),
        Some("cancelled")
    );

    let durable_events = crate::main_chat_event_stream::list_main_chat_agent_events_with_state(
        &state,
        task_session_id,
        None,
        Some(100),
    )
    .await
    .expect("list two-attempt cancellation facts");
    let durable_provider_facts = durable_events
        .iter()
        .filter(|event| event.event_type.starts_with("provider."))
        .map(|event| {
            (
                event.sequence,
                event.event_type.clone(),
                event.object_id.clone(),
                event.payload.clone(),
            )
        })
        .collect::<Vec<_>>();
    assert!(
        durable_events.iter().any(|event| {
            event.event_type == "provider.started" && event.object_id == started_ids[0]
        }),
        "completed ranking start is missing: {durable_provider_facts:?}"
    );
    assert!(durable_events.iter().any(|event| {
        event.event_type == "provider.completed" && event.object_id == started_ids[0]
    }));
    assert!(durable_events.iter().all(|event| {
        !(event.event_type == "provider.remote_unknown" && event.object_id == started_ids[0])
    }));
    assert!(durable_events.iter().any(|event| {
        event.event_type == "provider.started" && event.object_id == started_ids[1]
    }));
    assert!(durable_events.iter().any(|event| {
        event.event_type == "provider.remote_unknown" && event.object_id == started_ids[1]
    }));
    assert!(durable_events.iter().all(|event| {
        !(event.event_type == "provider.completed" && event.object_id == started_ids[1])
    }));
    let sequence_for = |event_type: &str, request_id: &str| {
        durable_events
            .iter()
            .find(|event| event.event_type == event_type && event.object_id == request_id)
            .map(|event| event.sequence)
            .unwrap_or_else(|| panic!("missing {event_type} for {request_id}"))
    };
    let cancel_sequence = durable_events
        .iter()
        .find(|event| event.event_type == "cancel_requested")
        .expect("cancel requested fact")
        .sequence;
    let local_aborted_sequence = durable_events
        .iter()
        .find(|event| event.event_type == "local_aborted")
        .expect("local aborted fact")
        .sequence;
    assert!(
        sequence_for("provider.started", &started_ids[0])
            < sequence_for("provider.completed", &started_ids[0])
    );
    assert!(
        sequence_for("provider.completed", &started_ids[0])
            < sequence_for("provider.started", &started_ids[1])
    );
    assert!(sequence_for("provider.started", &started_ids[1]) < cancel_sequence);
    assert!(cancel_sequence < sequence_for("provider.remote_unknown", &started_ids[1]));
    assert!(sequence_for("provider.remote_unknown", &started_ids[1]) < local_aborted_sequence);

    let provider_attempts = done
        .get("reasoning_trace")
        .and_then(|value| value.get("generation_result"))
        .and_then(|value| value.get("providerAttempts"))
        .and_then(serde_json::Value::as_array)
        .expect("request-level provider attempt summary");
    assert!(provider_attempts.iter().any(|attempt| {
        attempt.get("requestId").and_then(serde_json::Value::as_str)
            == Some(started_ids[0].as_str())
            && attempt.get("status").and_then(serde_json::Value::as_str) == Some("completed")
    }));
    assert!(provider_attempts.iter().any(|attempt| {
        attempt.get("requestId").and_then(serde_json::Value::as_str)
            == Some(started_ids[1].as_str())
            && attempt.get("status").and_then(serde_json::Value::as_str) == Some("remote_unknown")
    }));

    release_hanging_request.notify_waiters();
    let _ = tokio::time::timeout(Duration::from_secs(1), provider_server).await;

    let next_turn_provider_requests =
        configure_live_provider_eval_state_with_captured_local_http_provider(
            &state,
            "next turn uses a fresh provider proof scope",
        )
        .await;
    let next_turn = crate::main_chat_send::send_message_with_state(
        "phase2-provider-proof-scope-next-turn".into(),
        vec![ChatMessage {
            role: "user".into(),
            content: "Reply with the configured provider response.".into(),
        }],
        None,
        &state,
    )
    .await
    .expect("next turn completes with a fresh provider proof scope");
    assert_eq!(
        next_turn.reply,
        "next turn uses a fresh provider proof scope"
    );
    assert_eq!(
        next_turn_provider_requests
            .lock()
            .expect("read next-turn provider request capture")
            .len(),
        1
    );
    let next_task_session_id = next_turn
        .agent_ingress
        .as_ref()
        .and_then(|decision| decision.agent_task_session_id.as_deref())
        .expect("next turn owns one canonical task session");
    let next_turn_provider_events =
        crate::main_chat_event_stream::list_main_chat_agent_events_with_state(
            &state,
            next_task_session_id.to_string(),
            None,
            Some(100),
        )
        .await
        .expect("list next-turn provider facts")
        .into_iter()
        .filter(|event| event.event_type.starts_with("provider."))
        .collect::<Vec<_>>();
    assert!(
        !next_turn_provider_events.is_empty(),
        "the next turn must persist its own provider lifecycle"
    );
    assert!(
        next_turn_provider_events
            .iter()
            .all(|event| !started_ids.contains(&event.object_id)),
        "a fresh turn must not inherit provider receipts from the cancelled turn"
    );
}

#[tokio::test]
async fn kernel_error_drops_the_runtime_cancellation_registration_without_a_ghost() {
    let mut state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    Arc::get_mut(&mut state)
        .expect("isolated state has no other owner before provider configuration")
        .main_chat_agent_event_store = None;
    let captured_provider_requests =
        configure_live_provider_eval_state_with_captured_local_http_provider(
            &state,
            "This response cannot be projected because the durable event store is unavailable.",
        )
        .await;

    let result = crate::main_chat_send::send_message_with_state(
        "phase2-kernel-error-cancellation-cleanup".into(),
        vec![ChatMessage {
            role: "user".into(),
            content: "Reply with one short sentence.".into(),
        }],
        None,
        &state,
    )
    .await;
    let error = match result {
        Err(error) => error,
        Ok(_) => panic!("the fixture must fail closed before provider dispatch"),
    };
    assert!(
        error.contains("main_chat_agent_event_store_unavailable"),
        "unexpected kernel error: {error}"
    );

    assert!(
        captured_provider_requests
            .lock()
            .expect("captured provider requests")
            .is_empty(),
        "a durable event-store preflight failure must prevent provider dispatch"
    );
    let terminal_sessions = {
        let store = state
            .main_chat_agent_session_store
            .as_ref()
            .expect("task session store")
            .lock()
            .await;
        store
            .list_sessions(None, 10, 0)
            .expect("list task sessions after pre-dispatch failure")
            .into_iter()
            .filter(|session| session.chat_session_id == "phase2-kernel-error-cancellation-cleanup")
            .collect::<Vec<_>>()
    };
    assert!(
        terminal_sessions.is_empty(),
        "preflight must not create a task session that implies execution started"
    );
    let terminal_runs = {
        let store = state
            .agent_run_store
            .as_ref()
            .expect("agent run store")
            .lock()
            .await;
        store
            .list_runs_for_session("phase2-kernel-error-cancellation-cleanup", 10)
            .expect("list canonical runs after pre-dispatch failure")
    };
    assert!(
        terminal_runs.is_empty(),
        "preflight must not create an AgentRun that implies execution started"
    );
    let registry = {
        state
            .main_chat_runtime_state
            .lock()
            .await
            .cancellation_registry
            .clone()
    };
    assert!(
        registry.active_registration_count() == 0,
        "preflight failure must not leave a runtime registration"
    );
}

#[tokio::test]
async fn repeated_prompt_creates_independent_uuid_request_task_and_run_ids_across_both_transports()
{
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let captured_provider_requests =
        configure_live_provider_eval_state_with_captured_local_http_provider(
            &state,
            "Independent runtime response.",
        )
        .await;
    let message = ChatMessage {
        role: "user".into(),
        content: "Explain the same idea twice without sharing execution state.".into(),
    };
    let mut turn_identities = Vec::new();

    for turn_index in 0..2 {
        let response = crate::main_chat_send::send_message_with_state(
            format!("phase2-independent-buffered-{turn_index}"),
            vec![message.clone()],
            None,
            &state,
        )
        .await
        .expect("buffered turn succeeds");
        let ingress = response.agent_ingress.expect("buffered ingress decision");
        turn_identities.push((
            ingress.request_id,
            ingress
                .agent_task_session_id
                .expect("buffered task session id"),
            response.run_id.expect("buffered run id"),
        ));
    }

    for turn_index in 0..2 {
        let response = crate::main_chat_streaming::start_stream_message_with_state(
            format!("phase2-independent-stream-{turn_index}"),
            vec![message.clone()],
            None,
            &state,
            |_, _| {},
        )
        .await
        .expect("streaming turn succeeds");
        turn_identities.push((
            response["agent_ingress"]["requestId"]
                .as_str()
                .expect("streaming request id")
                .to_string(),
            response["agent_ingress"]["agentTaskSessionId"]
                .as_str()
                .expect("streaming task session id")
                .to_string(),
            response["run_id"]
                .as_str()
                .expect("streaming run id")
                .to_string(),
        ));
    }

    for (request_id, task_id, run_id) in &turn_identities {
        assert_eq!(
            request_id, task_id,
            "one logical turn must keep one operation/request/task owner"
        );
        assert_eq!(
            request_id, run_id,
            "one logical turn must keep one operation/request/run owner"
        );
    }
    let operation_ids = turn_identities
        .iter()
        .map(|(request_id, _, _)| request_id)
        .collect::<Vec<_>>();
    let unique_ids = operation_ids
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        unique_ids.len(),
        operation_ids.len(),
        "repeated buffered and streaming turns must not share execution state"
    );
    for id in operation_ids {
        let parsed = uuid::Uuid::parse_str(&id).expect("identity is a UUID");
        assert_eq!(parsed.get_version_num(), 4, "identity must be UUIDv4: {id}");
    }
    assert_eq!(
        captured_provider_requests
            .lock()
            .expect("read captured provider requests")
            .len(),
        4,
        "each buffered and streaming turn must own one independent provider request"
    );
}

#[tokio::test]
async fn every_ordinary_kernel_builder_reuses_exactly_one_early_canonical_agent_run() {
    let cases = [
        (
            "success",
            "hello",
            "direct_answer",
            openlife_core::agent::AgentRunStatus::Completed,
        ),
        (
            "write",
            "Please remember this private health fact: coffee causes heart palpitations.",
            "memory_proposal",
            openlife_core::agent::AgentRunStatus::Completed,
        ),
        (
            "blocker",
            "Review what changed in my working style this month.",
            "review_maturation",
            openlife_core::agent::AgentRunStatus::Failed,
        ),
        (
            "plan",
            "Plan the seeded policy-note publication task, but ask me before any risky external publish step.",
            "plan_execute",
            openlife_core::agent::AgentRunStatus::Completed,
        ),
    ];

    for (case_id, prompt, expected_strategy, expected_run_status) in cases {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let session_id = format!("phase2-one-early-run-{case_id}");
        let response = crate::main_chat_send::send_message_with_state(
            session_id.clone(),
            vec![ChatMessage {
                role: "user".into(),
                content: prompt.into(),
            }],
            None,
            &state,
        )
        .await
        .unwrap_or_else(|error| panic!("{case_id} builder failed: {error}"));
        let run_id = response
            .run_id
            .as_deref()
            .unwrap_or_else(|| panic!("{case_id} builder omitted run id"));
        let task_session_id = response
            .agent_ingress
            .as_ref()
            .and_then(|decision| decision.agent_task_session_id.as_deref())
            .unwrap_or_else(|| panic!("{case_id} builder omitted task session id"));
        assert_eq!(
            response
                .agent_ingress
                .as_ref()
                .map(|decision| decision.selected_strategy.as_str()),
            Some(expected_strategy)
        );

        let runs = {
            let run_store = state
                .agent_run_store
                .as_ref()
                .expect("agent run store")
                .lock()
                .await;
            run_store
                .list_runs_for_session(&session_id, 10)
                .expect("list builder runs")
        };
        assert_eq!(
            runs.len(),
            1,
            "{case_id} builder must update the early run instead of inserting a compatibility run"
        );
        assert_eq!(runs[0].id, run_id);
        assert_eq!(runs[0].task_id, task_session_id);
        assert_eq!(runs[0].status, expected_run_status);

        let durable_events = crate::main_chat_event_stream::list_main_chat_agent_events_with_state(
            &state,
            task_session_id.to_string(),
            None,
            Some(250),
        )
        .await
        .expect("list builder durable events");
        assert!(
            durable_events.iter().all(|event| event.run_id == run_id),
            "{case_id} builder emitted a durable fact under another run id"
        );
        assert!(response.tool_calls.iter().all(|tool_call| {
            tool_call
                .run_id
                .as_deref()
                .is_none_or(|source_run_id| source_run_id == run_id)
        }));

        if case_id == "write" {
            {
                let proposal_store = state
                    .proposal_store
                    .as_ref()
                    .expect("proposal store")
                    .lock()
                    .await;
                let proposals = proposal_store
                    .list_pending_proposals(100)
                    .expect("list write builder proposals");
                assert!(
                    !proposals.is_empty(),
                    "write builder must stage a governed review item"
                );
                for proposal in proposals {
                    let origin = proposal_store
                        .terminal_owner_origin_binding(&proposal.id)
                        .expect("load canonical proposal origin")
                        .expect("write builder proposal has a terminal-owner origin");
                    assert_eq!(origin.task_session_id(), task_session_id);
                    assert_eq!(origin.run_id(), run_id);
                }
            }
        }
        if case_id == "plan" {
            let plan_event = durable_events
                .iter()
                .find(|event| event.event_type == "plan.created")
                .expect("Main Chat PlanExecute emits a plan.created fact");
            assert_eq!(
                plan_event
                    .payload
                    .get("taskSessionId")
                    .and_then(serde_json::Value::as_str),
                Some(task_session_id),
                "the event payload task identity must match the source run binding"
            );
            assert_ne!(
                plan_event
                    .payload
                    .get("planSessionId")
                    .and_then(serde_json::Value::as_str),
                Some(task_session_id),
                "the PlanExecute child workflow id must remain explicit instead of masquerading as the source task"
            );
            assert_eq!(
                plan_event
                    .payload
                    .pointer("/childWorkflowProvenance/eventTaskBoundToSourceRun")
                    .and_then(serde_json::Value::as_bool),
                Some(true)
            );
        }
    }
}
