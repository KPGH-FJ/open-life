//! Owned ToolGateway resource snapshots for Tauri product execution paths.
//!
//! `AppState` mutexes protect replaceable handles; they must never become part
//! of provider, tool, or network I/O lifetimes. This module is the sole lock
//! acquisition authority for product `ActionExecutionContext` construction.
//! Each public function returns a purpose-specific type whose required stores
//! are non-optional, and every outer guard is dropped before the next lock.
//!
//! Product source map and lock-order contract:
//! 1. `config` (copy bounded typed execution fields; credentials remain only
//!    in non-serializable runtime objects),
//! 2. `tool_permission_store`,
//! 3. `mcp_registry`,
//! 4. `mcp_audit_store`,
//! 5. `privacy_engine`,
//! 6. `memory_store` for governed product execution,
//! 7. `memory_lifecycle_store` retrieval authority where available,
//! 8. required `proposal_store` or `agent_run_store` only where the
//!    purpose-specific return type declares them.
//!
//! The consumers are the dev ToolGateway command, Main Chat read execution,
//! Main Chat replay/AgentLoop execution and scheduled execution. MCP's
//! `McpSession` mutex is intentionally outside this table: it
//! is a single-owner transport/session lock on an owned registry snapshot, not
//! an `AppState` registry guard, and cancellation poisons/kills that transport.

use std::sync::Arc;

use crate::AppState;

#[derive(Clone)]
pub(crate) struct SharedToolGatewayResources {
    pub(crate) safe_paths: Vec<String>,
    pub(crate) permission_store: openlife_core::tool_permissions::ToolPermissionStore,
    pub(crate) registry: openlife_core::mcp::McpRegistry,
    pub(crate) audit_store: openlife_core::mcp_audit::McpAuditStore,
    pub(crate) privacy_engine: openlife_core::privacy::PrivacyEngine,
    pub(crate) persistence_coordinator: Arc<crate::persistence_coordinator::PersistenceCoordinator>,
}

#[derive(Clone)]
pub(crate) struct GovernedToolGatewayResources {
    pub(crate) shared: SharedToolGatewayResources,
    pub(crate) calendar_ics_paths: Vec<String>,
    pub(crate) network_policy: openlife_core::config::NetworkPolicy,
    pub(crate) search_provider:
        openlife_core::agent::action_executor::helpers::SearchProviderConfig,
    pub(crate) memory_store: openlife_core::memory::MemoryStore,
    pub(crate) memory_lifecycle_retrieval_reader:
        Option<openlife_core::agent::MemoryLifecycleRetrievalReader>,
    pub(crate) canonical_state: Option<openlife_core::agent::CanonicalStateSnapshot>,
}

#[cfg(any(test, feature = "dev-extensions"))]
pub(crate) struct DevToolGatewayResources {
    pub(crate) shared: SharedToolGatewayResources,
    pub(crate) agent_run_store: openlife_core::agent::AgentRunStore,
}

pub(crate) struct MainChatReadToolGatewayResources {
    pub(crate) governed: GovernedToolGatewayResources,
    pub(crate) agent_run_store: openlife_core::agent::AgentRunStore,
}

pub(crate) struct MainChatExecutionToolGatewayResources {
    pub(crate) governed: GovernedToolGatewayResources,
    pub(crate) agent_run_store: openlife_core::agent::AgentRunStore,
}

#[derive(Clone)]
pub(crate) struct MainChatAgentLoopLimitSnapshot {
    pub(crate) max_steps: u32,
    pub(crate) max_tool_calls: u32,
    pub(crate) timeout_seconds: u64,
}

pub(crate) struct MainChatAgentLoopToolGatewayResources {
    pub(crate) execution: MainChatExecutionToolGatewayResources,
    pub(crate) agent_runtime_config: openlife_core::agent::AgentRuntimeConfig,
    pub(crate) limits: MainChatAgentLoopLimitSnapshot,
}

pub(crate) struct ScheduledToolGatewayResources {
    pub(crate) governed: GovernedToolGatewayResources,
    pub(crate) proposal_store: openlife_core::agent::ProposalStore,
    pub(crate) agent_run_store: openlife_core::agent::AgentRunStore,
    pub(crate) agent_runtime_config: openlife_core::agent::AgentRuntimeConfig,
}

struct CapturedToolRuntimeConfig {
    safe_paths: Vec<String>,
    calendar_ics_paths: Vec<String>,
    network_policy: openlife_core::config::NetworkPolicy,
    search_provider: openlife_core::agent::action_executor::helpers::SearchProviderConfig,
    agent_runtime_config: openlife_core::agent::AgentRuntimeConfig,
    main_chat_agent_loop_limits: MainChatAgentLoopLimitSnapshot,
}

async fn capture_tool_runtime_config(state: &Arc<AppState>) -> CapturedToolRuntimeConfig {
    let config = state.config.lock().await;
    CapturedToolRuntimeConfig {
        safe_paths: config.system.safe_paths.clone(),
        calendar_ics_paths: config.system.calendar_ics_paths.clone(),
        network_policy: config.system.network_policy.clone(),
        search_provider:
            openlife_core::agent::action_executor::helpers::SearchProviderConfig::from_system_config(
                &config.system,
            ),
        agent_runtime_config: openlife_core::agent::AgentRuntimeConfig {
            default_strategy: config.reasoning.default_strategy.clone(),
            meaning_timeout_ms: config.reasoning.meaning_timeout_ms,
            strategy_timeout_ms: config.reasoning.strategy_timeout_ms,
            generation_timeout_ms: config.reasoning.generation_timeout_ms,
        },
        main_chat_agent_loop_limits: MainChatAgentLoopLimitSnapshot {
            max_steps: config.system.agent_loop_max_steps,
            max_tool_calls: config.system.agent_loop_max_tool_calls,
            timeout_seconds: config.system.agent_loop_timeout_seconds,
        },
    }
}

async fn capture_shared_after_config(
    state: &Arc<AppState>,
    safe_paths: Vec<String>,
) -> SharedToolGatewayResources {
    let permission_store = { state.tool_permission_store.lock().await.clone() };
    let registry = { state.mcp_registry.lock().await.clone() };
    let audit_store = { state.mcp_audit_store.lock().await.clone() };
    let privacy_engine = { state.privacy_engine.lock().await.clone() };
    SharedToolGatewayResources {
        safe_paths,
        permission_store,
        registry,
        audit_store,
        privacy_engine,
        persistence_coordinator: Arc::clone(&state.persistence_coordinator),
    }
}

async fn capture_governed(
    state: &Arc<AppState>,
) -> (GovernedToolGatewayResources, CapturedToolRuntimeConfig) {
    let config = capture_tool_runtime_config(state).await;
    let shared = capture_shared_after_config(state, config.safe_paths.clone()).await;
    let memory_store = { state.memory_store.lock().await.clone() };
    let memory_lifecycle_retrieval_reader =
        if let Some(store) = state.memory_lifecycle_store.as_ref() {
            Some(store.lock().await.retrieval_reader())
        } else {
            None
        };
    let canonical_state = state.state_store.as_ref().and_then(|store| {
        let daily_tasks = store.get_product_daily_tasks();
        let observations = store.list_state_observations(false);
        match (daily_tasks, observations) {
            (Ok(daily_tasks), Ok(observations)) => {
                Some(openlife_core::agent::CanonicalStateSnapshot {
                    daily_tasks,
                    observations,
                })
            }
            (daily_tasks, observations) => {
                log::warn!(
                    "[tool-gateway] canonical StateStore snapshot unavailable: daily_tasks={:?}, observations={:?}",
                    daily_tasks.as_ref().err().map(ToString::to_string),
                    observations.as_ref().err().map(ToString::to_string),
                );
                None
            }
        }
    });
    (
        GovernedToolGatewayResources {
            shared,
            calendar_ics_paths: config.calendar_ics_paths.clone(),
            network_policy: config.network_policy.clone(),
            search_provider: config.search_provider.clone(),
            memory_store,
            memory_lifecycle_retrieval_reader,
            canonical_state,
        },
        config,
    )
}

async fn require_proposal_store(
    state: &Arc<AppState>,
) -> Result<openlife_core::agent::ProposalStore, String> {
    let store = state
        .proposal_store
        .as_ref()
        .ok_or_else(|| "tool_gateway_proposal_store_unavailable".to_string())?;
    Ok(store.lock().await.clone())
}

async fn require_agent_run_store(
    state: &Arc<AppState>,
) -> Result<openlife_core::agent::AgentRunStore, String> {
    let store = state
        .agent_run_store
        .as_ref()
        .ok_or_else(|| "tool_gateway_agent_run_store_unavailable".to_string())?;
    let store = store.lock().await;
    // ToolGateway receives a cloned handle and performs synchronous canonical
    // reads below the Tauri boundary. Verify and classify that owner while the
    // shared guard is still held so a broken AgentRun database cannot reach a
    // tool dispatch and later be disguised as an ordinary blocker.
    crate::terminal_owner_write_gateway::register_agent_run_store_result(
        state,
        store.verify_readable().map_err(|error| error.to_string()),
    )?;
    Ok(store.clone())
}

#[cfg(any(test, feature = "dev-extensions"))]
pub(crate) async fn snapshot_tool_gateway_resources_for_dev_command(
    state: &Arc<AppState>,
) -> Result<DevToolGatewayResources, String> {
    let safe_paths = { state.config.lock().await.system.safe_paths.clone() };
    let shared = capture_shared_after_config(state, safe_paths).await;
    let agent_run_store = require_agent_run_store(state).await?;
    Ok(DevToolGatewayResources {
        shared,
        agent_run_store,
    })
}

pub(crate) async fn snapshot_tool_gateway_resources_for_main_chat_read(
    state: &Arc<AppState>,
) -> Result<MainChatReadToolGatewayResources, String> {
    let (governed, _) = capture_governed(state).await;
    let agent_run_store = require_agent_run_store(state).await?;
    Ok(MainChatReadToolGatewayResources {
        governed,
        agent_run_store,
    })
}

pub(crate) async fn snapshot_tool_gateway_resources_for_main_chat_execution(
    state: &Arc<AppState>,
) -> Result<MainChatExecutionToolGatewayResources, String> {
    let (governed, _) = capture_governed(state).await;
    let agent_run_store = require_agent_run_store(state).await?;
    Ok(MainChatExecutionToolGatewayResources {
        governed,
        agent_run_store,
    })
}

pub(crate) async fn snapshot_tool_gateway_resources_for_main_chat_agent_loop(
    state: &Arc<AppState>,
) -> Result<MainChatAgentLoopToolGatewayResources, String> {
    let (governed, config) = capture_governed(state).await;
    let agent_run_store = require_agent_run_store(state).await?;
    Ok(MainChatAgentLoopToolGatewayResources {
        execution: MainChatExecutionToolGatewayResources {
            governed,
            agent_run_store,
        },
        agent_runtime_config: config.agent_runtime_config,
        limits: config.main_chat_agent_loop_limits,
    })
}

pub(crate) async fn snapshot_tool_gateway_resources_for_scheduler(
    state: &Arc<AppState>,
) -> Result<ScheduledToolGatewayResources, String> {
    let (governed, config) = capture_governed(state).await;
    let proposal_store = require_proposal_store(state).await?;
    let agent_run_store = require_agent_run_store(state).await?;
    Ok(ScheduledToolGatewayResources {
        governed,
        proposal_store,
        agent_run_store,
        agent_runtime_config: config.agent_runtime_config,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct RecordingDurableFailureObserver {
        coordinator: Arc<crate::persistence_coordinator::PersistenceCoordinator>,
        failures: std::sync::Mutex<Vec<(String, String)>>,
    }

    impl openlife_core::agent::DurableStoreFailureObserver for RecordingDurableFailureObserver {
        fn durable_store_failed(&self, store_kind: &'static str, raw_error: &str) {
            self.failures
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push((store_kind.into(), raw_error.into()));
            openlife_core::agent::DurableStoreFailureObserver::durable_store_failed(
                self.coordinator.as_ref(),
                store_kind,
                raw_error,
            );
        }
    }

    #[tokio::test]
    async fn governed_snapshot_tracks_each_config_generation_without_global_search_state() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let mut brave = state.config.lock().await.clone();
        brave.system.search_provider = "brave".into();
        brave.system.search_provider_key = "brave-test-key".into();
        state.replace_provider_runtime_config(brave).await;

        let (first, _) = capture_governed(&state).await;
        assert_eq!(first.search_provider.provider, "brave");
        assert_eq!(first.search_provider.api_key, "brave-test-key");

        let mut searxng = state.config.lock().await.clone();
        searxng.system.search_provider = "searxng".into();
        searxng.system.search_provider_key.clear();
        searxng.system.searxng_url = "https://search.example.test".into();
        state.replace_provider_runtime_config(searxng).await;

        let (second, _) = capture_governed(&state).await;
        assert_eq!(second.search_provider.provider, "searxng");
        assert_eq!(
            second.search_provider.searxng_url,
            "https://search.example.test"
        );
        assert_eq!(
            first.search_provider.provider, "brave",
            "an in-flight ToolGateway snapshot must remain bound to its original config generation"
        );
    }

    fn install_release_like_persistence_coordinator(state: &mut Arc<AppState>) {
        let coordinator = Arc::new(
            crate::persistence_coordinator::PersistenceCoordinator::for_release_bootstrap(),
        );
        for store in crate::persistence_coordinator::EXPECTED_BOOTSTRAP_STORES {
            coordinator.register_read_write(*store);
        }
        coordinator.seal();
        Arc::get_mut(state)
            .expect("test state has one outer owner")
            .persistence_coordinator = coordinator;
    }

    #[tokio::test]
    async fn cloned_tool_gateway_store_preflight_classifies_durable_failure_before_dispatch() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tool-gateway-agent-run-preflight.db");
        let store = openlife_core::agent::AgentRunStore::new(&path).unwrap();
        let mut state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        Arc::get_mut(&mut state)
            .expect("test state has one outer owner")
            .agent_run_store = Some(Arc::new(tokio::sync::Mutex::new(store)));
        install_release_like_persistence_coordinator(&mut state);
        let fault = rusqlite::Connection::open(&path).unwrap();
        fault.execute_batch("DROP TABLE agent_runs;").unwrap();
        drop(fault);

        let error = match snapshot_tool_gateway_resources_for_main_chat_read(&state).await {
            Err(error) => error,
            Ok(_) => panic!("a broken cloned AgentRun owner must fail before ToolGateway dispatch"),
        };
        assert!(error.to_ascii_lowercase().contains("no such table"));
        assert_eq!(
            state.persistence_coordinator.snapshot().mode,
            crate::persistence_coordinator::PersistenceRuntimeMode::UnavailableDegraded
        );
        assert!(state
            .persistence_coordinator
            .require_effects_allowed()
            .is_err());
    }

    #[tokio::test]
    async fn cloned_tool_gateway_owner_read_toctou_degrades_before_future_effects() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory
            .path()
            .join("tool-gateway-agent-run-owner-toctou.db");
        let store = openlife_core::agent::AgentRunStore::new(&path).unwrap();
        let owner = openlife_core::agent::AgentRun::new_chat_run("owner-read-toctou", "");
        store.create_run(&owner).unwrap();

        let mut state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        Arc::get_mut(&mut state)
            .expect("test state has one outer owner")
            .agent_run_store = Some(Arc::new(tokio::sync::Mutex::new(store)));
        install_release_like_persistence_coordinator(&mut state);

        // The resource snapshot must be healthy first. The fault is injected
        // only after the cloned canonical handle crossed the Tauri boundary.
        let resources = snapshot_tool_gateway_resources_for_main_chat_read(&state)
            .await
            .expect("healthy AgentRun owner snapshot");
        let fault = rusqlite::Connection::open(&path).unwrap();
        fault.execute_batch("DROP TABLE agent_runs;").unwrap();
        drop(fault);

        let context = openlife_core::agent::ActionExecutionContext::new(
            &resources.governed.shared.registry,
            &resources.governed.shared.permission_store,
            &resources.governed.shared.audit_store,
            &resources.governed.shared.privacy_engine,
            &resources.governed.shared.safe_paths,
        )
        .with_tool_audit_persistence_observer(
            resources.governed.shared.persistence_coordinator.as_ref(),
        )
        .with_durable_store_failure_observer(
            resources.governed.shared.persistence_coordinator.as_ref(),
        )
        .with_memory_store(&resources.governed.memory_store)
        .with_agent_run_store(&resources.agent_run_store);
        let result = openlife_core::agent::ToolGateway::from_executor_config(Default::default())
            .execute(
                openlife_core::agent::AgentActionRequest {
                    action_type: "memory_search".into(),
                    target: "memory.search".into(),
                    input: serde_json::json!({"query": "toctou"}),
                    source_run_id: Some(owner.id),
                    step_index: 0,
                },
                &context,
            )
            .await
            .expect("owner read failure is represented as a blocked ToolGateway result");
        assert_eq!(
            result.stop_reason.as_deref(),
            Some("internal_read_canonical_run_owner_authority_unavailable")
        );
        assert_eq!(
            state.persistence_coordinator.snapshot().mode,
            crate::persistence_coordinator::PersistenceRuntimeMode::UnavailableDegraded,
            "the raw durable read failure must be observed before it is rewritten as a blocker"
        );
        assert!(state
            .persistence_coordinator
            .require_effects_allowed()
            .is_err());
    }

    #[tokio::test]
    async fn cloned_tool_gateway_receipt_ledger_toctou_degrades_before_future_effects() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory
            .path()
            .join("tool-gateway-agent-run-receipt-toctou.db");
        let store = openlife_core::agent::AgentRunStore::new(&path).unwrap();
        let owner = openlife_core::agent::AgentRun::new_tool_execution_run("web.search");
        store.create_run(&owner).unwrap();

        let mut state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        Arc::get_mut(&mut state)
            .expect("test state has one outer owner")
            .agent_run_store = Some(Arc::new(tokio::sync::Mutex::new(store)));
        install_release_like_persistence_coordinator(&mut state);
        let resources = snapshot_tool_gateway_resources_for_main_chat_read(&state)
            .await
            .expect("healthy AgentRun receipt-owner snapshot");

        let fault = rusqlite::Connection::open(&path).unwrap();
        fault.execute_batch("DROP TABLE agent_runs;").unwrap();
        drop(fault);
        let network_policy = openlife_core::config::NetworkPolicy {
            default_decision: "allow".into(),
            ..Default::default()
        };
        let context = openlife_core::agent::ActionExecutionContext::new(
            &resources.governed.shared.registry,
            &resources.governed.shared.permission_store,
            &resources.governed.shared.audit_store,
            &resources.governed.shared.privacy_engine,
            &resources.governed.shared.safe_paths,
        )
        .with_tool_audit_persistence_observer(
            resources.governed.shared.persistence_coordinator.as_ref(),
        )
        .with_durable_store_failure_observer(
            resources.governed.shared.persistence_coordinator.as_ref(),
        )
        .with_agent_run_store(&resources.agent_run_store)
        .with_network_policy(&network_policy)
        .with_web_search_fixture_output("bounded receipt TOCTOU fixture");
        let execution = openlife_core::agent::ToolGateway::from_executor_config(Default::default())
            .execute(
                openlife_core::agent::AgentActionRequest {
                    action_type: "mcp_tool".into(),
                    target: "web.search".into(),
                    input: serde_json::json!({"arguments": {"query": "OpenLife"}}),
                    source_run_id: Some(owner.id),
                    step_index: 0,
                },
                &context,
            )
            .await;
        assert!(
            execution.is_err(),
            "a receipt ledger whose canonical owner vanished must fail closed"
        );
        assert_eq!(
            state.persistence_coordinator.snapshot().mode,
            crate::persistence_coordinator::PersistenceRuntimeMode::UnavailableDegraded,
            "receipt issuance must expose its raw AgentRunStore failure before ToolGateway rewrites it"
        );
        assert!(state
            .persistence_coordinator
            .require_effects_allowed()
            .is_err());
    }

    #[tokio::test]
    async fn cloned_core_os_agent_run_lookup_toctou_degrades_before_future_effects() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory
            .path()
            .join("tool-gateway-core-os-run-lookup-toctou.db");
        let store = openlife_core::agent::AgentRunStore::new(&path).unwrap();
        let mut owner = openlife_core::agent::AgentRun::new_tool_execution_run("agent_run.lookup");
        // Keep this adapter-fault fixture outside privacy patterns so it reaches dispatch.
        owner.id = "agent-run-lookup-toctou-owner".into();
        store.create_run(&owner).unwrap();

        let mut state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        Arc::get_mut(&mut state)
            .expect("test state has one outer owner")
            .agent_run_store = Some(Arc::new(tokio::sync::Mutex::new(store)));
        install_release_like_persistence_coordinator(&mut state);
        let resources = snapshot_tool_gateway_resources_for_main_chat_read(&state)
            .await
            .expect("healthy AgentRun Core OS snapshot");
        let failure_observer = RecordingDurableFailureObserver {
            coordinator: Arc::clone(&resources.governed.shared.persistence_coordinator),
            failures: std::sync::Mutex::new(Vec::new()),
        };

        let fault = rusqlite::Connection::open(&path).unwrap();
        fault.execute_batch("DROP TABLE agent_runs;").unwrap();
        drop(fault);
        let context = openlife_core::agent::ActionExecutionContext::new(
            &resources.governed.shared.registry,
            &resources.governed.shared.permission_store,
            &resources.governed.shared.audit_store,
            &resources.governed.shared.privacy_engine,
            &resources.governed.shared.safe_paths,
        )
        .with_tool_audit_persistence_observer(
            resources.governed.shared.persistence_coordinator.as_ref(),
        )
        .with_durable_store_failure_observer(&failure_observer)
        .with_agent_run_store(&resources.agent_run_store);
        let result = openlife_core::agent::ToolGateway::from_executor_config(Default::default())
            .execute(
                openlife_core::agent::AgentActionRequest {
                    action_type: "builtin_tool".into(),
                    target: "agent_run.lookup".into(),
                    input: serde_json::json!({"arguments": {"run_id": owner.id}}),
                    source_run_id: None,
                    step_index: 0,
                },
                &context,
            )
            .await
            .expect("Core OS store failure is represented as a failed tool result");
        assert_ne!(
            result.status,
            openlife_core::agent::ActionExecutionStatus::Succeeded
        );
        let observed_failures = failure_observer
            .failures
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        assert_eq!(
            result.execution_receipt.dispatch_kind,
            openlife_core::tool_execution_receipt::ToolDispatchKind::Local,
            "the TOCTOU fault must be observed inside the admitted adapter; result={result:?}; observed_failures={observed_failures:?}"
        );
        assert_eq!(result.execution_receipt.dispatch_attempt_count, 1);
        assert!(
            observed_failures
                .iter()
                .any(|(store, _)| store == "AgentRunStore"),
            "AgentRunStore raw failure did not reach the durable observer: result={result:?}; observed_failures={observed_failures:?}"
        );
        assert_eq!(
            state.persistence_coordinator.snapshot().mode,
            crate::persistence_coordinator::PersistenceRuntimeMode::UnavailableDegraded,
            "Core OS must observe and classify the raw AgentRunStore failure before converting it to action output; observed_failures={observed_failures:?}"
        );
        assert!(state
            .persistence_coordinator
            .require_effects_allowed()
            .is_err());
    }

    #[test]
    fn cloned_tool_gateway_error_callback_degrades_only_for_durable_store_failures() {
        let mut state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        install_release_like_persistence_coordinator(&mut state);
        crate::terminal_owner_write_gateway::register_agent_run_store_error(
            &state,
            "ToolGateway blocked: permission denied by product policy",
        );
        assert_eq!(
            state.persistence_coordinator.snapshot().mode,
            crate::persistence_coordinator::PersistenceRuntimeMode::ReadWrite
        );
        crate::terminal_owner_write_gateway::register_agent_run_store_error(
            &state,
            "ToolGateway failed: Failed to lookup agent run: no such table: agent_runs",
        );
        assert_eq!(
            state.persistence_coordinator.snapshot().mode,
            crate::persistence_coordinator::PersistenceRuntimeMode::UnavailableDegraded
        );
    }
}
