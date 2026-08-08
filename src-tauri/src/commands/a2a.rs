use crate::errors::AppError;
use crate::AppState;
use openlife_core::a2a::{
    a2a_response_to_reasoning_result, reasoning_input_to_a2a_task, A2AClient, A2AEndpointTransport,
    A2AServerHandler, AgentCard, SendTaskRequest,
};
use openlife_core::agent::ReasoningInput;
use openlife_core::network_client::resolve_network_policy_decision;
use std::sync::Arc;
use tauri::State;

use crate::provider_network_consent::{
    authorize_external_network_dispatch, NetworkConsentSubject, NetworkConsentSubmissionScope,
    ProviderNetworkAuthorization,
};

const A2A_PUBLIC_CARD_CAPABILITY: &str = "a2a.card.public";
const A2A_PRIVATE_CARD_CAPABILITY: &str = "a2a.card.private";
#[cfg(any(test, feature = "dev-extensions"))]
const A2A_TASK_CAPABILITY: &str = "a2a.task";

struct AuthorizedA2AEdge {
    network_policy: openlife_core::config::NetworkPolicy,
    network_policy_decision: openlife_core::network_client::NetworkPolicyDecision,
    transport: A2AEndpointTransport,
}

async fn authorize_a2a_edge(
    state: &Arc<AppState>,
    base_url: &str,
    endpoint_url: &str,
    capability: &str,
    blocked_action_type: &str,
    originating_task_session_id: Option<&str>,
) -> Result<AuthorizedA2AEdge, AppError> {
    let transport = A2AEndpointTransport::for_base_url(base_url).map_err(AppError::from)?;
    let network_policy = state.config.lock().await.system.network_policy.clone();
    let decision = resolve_network_policy_decision(&network_policy, endpoint_url, capability)
        .map_err(AppError::from)?;
    let authorization = authorize_external_network_dispatch(
        state,
        &network_policy,
        &decision,
        endpoint_url,
        capability,
        NetworkConsentSubject {
            permission_source: "a2a",
            risk_level: "high",
            capabilities: &["network", "external_transmission"],
            target: &decision.host,
            affected_path_prefix: "tool_permission.a2a",
            blocked_action_type,
            proposal_summary:
                "Allow one endpoint-bound A2A network retry after explicit Review Center approval.",
        },
        originating_task_session_id,
        NetworkConsentSubmissionScope::ExplicitCommand,
    )
    .await?;
    match authorization {
        ProviderNetworkAuthorization::Authorized {
            network_policy,
            network_policy_decision,
            ..
        } => Ok(AuthorizedA2AEdge {
            network_policy: *network_policy,
            network_policy_decision,
            transport,
        }),
        ProviderNetworkAuthorization::ConsentRequired { proposal_id } => {
            Err(AppError::permission(format!(
                "A2A network consent is pending Review Center approval: proposal_id={proposal_id}"
            )))
        }
        ProviderNetworkAuthorization::Denied { reason_code } => Err(AppError::permission(format!(
            "A2A network dispatch denied before connection: {reason_code}"
        ))),
    }
}

fn resolve_pairing_token(
    transport: A2AEndpointTransport,
    provided: Option<String>,
) -> Result<String, AppError> {
    if let Some(token) = provided
        .map(|token| token.trim().to_string())
        .filter(|token| !token.is_empty())
    {
        if !(32..=4096).contains(&token.len()) || token.chars().any(char::is_control) {
            return Err(AppError::permission(
                "A2A pairing credential must contain 32..=4096 non-control characters",
            ));
        }
        return Ok(token);
    }
    if transport == A2AEndpointTransport::PairedLoopback {
        return crate::a2a_server::paired_token_for_local_client().map_err(AppError::permission);
    }
    Err(AppError::permission(
        "Remote A2A endpoint is not paired; a bearer credential is required",
    ))
}

#[cfg(any(test, feature = "dev-extensions"))]
fn minimize_outbound_task(req: &mut SendTaskRequest) -> Result<(), AppError> {
    let skill = req
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.get("skill"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|skill| !skill.is_empty())
        .map(str::to_string);
    if skill.as_ref().is_some_and(|skill| skill.len() > 128) {
        return Err(AppError::serialization("A2A skill identifier is too large"));
    }
    let request_id = uuid::Uuid::parse_str(&req.id)
        .map_err(|_| AppError::serialization("A2A request id must be a canonical UUIDv4"))?;
    if request_id.get_version_num() != 4 || request_id.hyphenated().to_string() != req.id {
        return Err(AppError::serialization(
            "A2A request id must be a canonical UUIDv4",
        ));
    }
    req.session_id = req.session_id.as_ref().map(|session_id| {
        let (_, digest) = openlife_core::agent::metadata_safe::metadata_safe_value_digest(
            &serde_json::Value::String(session_id.clone()),
        );
        format!("a2a-session:sha256:{digest}")
    });
    req.message.metadata = None;
    req.metadata = skill.map(|skill| {
        std::collections::HashMap::from([("skill".into(), serde_json::Value::String(skill))])
    });
    req.accepted_output_modes = Some(vec!["text".into()]);
    req.push_notification = None;
    req.history_length = None;
    openlife_core::a2a::validate_text_task_envelope(req).map_err(AppError::from)
}

#[tauri::command]
pub async fn a2a_discover_agent(
    url: String,
    private_card: Option<bool>,
    pairing_token: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<AgentCard, AppError> {
    let private_card = private_card.unwrap_or(false);
    let endpoint_url = if private_card {
        A2AClient::private_card_url(&url)
    } else {
        A2AClient::public_card_url(&url)
    }
    .map_err(AppError::from)?;
    let capability = if private_card {
        A2A_PRIVATE_CARD_CAPABILITY
    } else {
        A2A_PUBLIC_CARD_CAPABILITY
    };
    let prevalidated_transport =
        A2AEndpointTransport::for_base_url(&url).map_err(AppError::from)?;
    let token = if private_card {
        Some(resolve_pairing_token(
            prevalidated_transport,
            pairing_token,
        )?)
    } else {
        None
    };
    let edge = authorize_a2a_edge(
        state.inner(),
        &url,
        &endpoint_url,
        capability,
        "a2a_card_discovery",
        None,
    )
    .await?;
    debug_assert_eq!(edge.transport, prevalidated_transport);
    let client = A2AClient::with_authorized_edge(
        edge.network_policy,
        edge.network_policy_decision,
        token,
        edge.transport,
    )
    .map_err(AppError::from)?;
    if private_card {
        client
            .discover_private_agent_card(&url)
            .await
            .map_err(AppError::from)
    } else {
        client
            .discover_agent_card(&url)
            .await
            .map_err(AppError::from)
    }
}

#[tauri::command]
#[cfg(feature = "dev-extensions")]
pub async fn a2a_send_task(
    url: String,
    request_json: String,
    pairing_token: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<String, AppError> {
    a2a_send_task_with_state(&url, &request_json, pairing_token, state.inner()).await
}

#[cfg(any(test, feature = "dev-extensions"))]
async fn a2a_send_task_with_state(
    url: &str,
    request_json: &str,
    pairing_token: Option<String>,
    state: &Arc<AppState>,
) -> Result<String, AppError> {
    let mut req: SendTaskRequest = serde_json::from_str(request_json).map_err(AppError::from)?;
    minimize_outbound_task(&mut req)?;
    let endpoint_url = A2AClient::task_url(url).map_err(AppError::from)?;
    let prevalidated_transport = A2AEndpointTransport::for_base_url(url).map_err(AppError::from)?;
    let token = resolve_pairing_token(prevalidated_transport, pairing_token)?;
    let edge = authorize_a2a_edge(
        state,
        url,
        &endpoint_url,
        A2A_TASK_CAPABILITY,
        "a2a_task_dispatch",
        req.session_id.as_deref(),
    )
    .await?;
    debug_assert_eq!(edge.transport, prevalidated_transport);
    let authorization = openlife_core::agent::A2AOutboundAuthorization::new(
        url.to_string(),
        edge.network_policy,
        edge.network_policy_decision,
        token,
        edge.transport,
    )
    .map_err(AppError::from)?;
    let task_text = req
        .message
        .parts
        .iter()
        .filter_map(|part| match part {
            openlife_core::a2a::Part::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    let mut resources =
        crate::tool_gateway_resources::snapshot_tool_gateway_resources_for_dev_command(state)
            .await
            .map_err(AppError::internal)?;
    if !resources
        .shared
        .registry
        .list_manifests()
        .iter()
        .any(|manifest| manifest.name == "a2a.call_agent")
    {
        resources.shared.registry.register_dev_a2a_tool();
    }
    let permission_store = openlife_core::tool_permissions::ToolPermissionStore::new_in_memory()
        .map_err(AppError::from)?;
    permission_store
        .grant(
            "a2a.call_agent",
            "builtin",
            "medium",
            "external_side_effect",
            openlife_core::tool_permissions::ToolPermissionPolicy::AllowOnce,
            None,
        )
        .map_err(AppError::from)?;
    // The external A2A request id is already a validated UUIDv4. Reuse it as
    // the canonical execution owner so a retry of the same non-idempotent
    // request cannot silently create a second run or dispatch under a
    // non-canonical synthetic label. All fallible pre-dispatch setup above is
    // complete before this durable Running row is created.
    let mut execution_run =
        openlife_core::agent::AgentRun::new_tool_execution_run("a2a.call_agent");
    execution_run.id = req.id.clone();
    execution_run.session_id = req.session_id.clone();
    let durable_owner =
        openlife_core::agent::AgentRunA2AToolExecutionOwner::new_for_unpersisted_run(
            resources.agent_run_store.clone(),
            execution_run.clone(),
            url,
        )
        .map_err(AppError::from)?;
    let authorization = authorization.with_durable_tool_execution_owner(durable_owner);
    let action_ctx = openlife_core::agent::ActionExecutionContext::new(
        &resources.shared.registry,
        &permission_store,
        &resources.shared.audit_store,
        &resources.shared.privacy_engine,
        &resources.shared.safe_paths,
    )
    .with_tool_audit_persistence_observer(resources.shared.persistence_coordinator.as_ref())
    .with_durable_store_failure_observer(resources.shared.persistence_coordinator.as_ref())
    .with_agent_run_store(&resources.agent_run_store)
    .with_a2a_outbound_authorization(&authorization);
    let execution = openlife_core::agent::ToolGateway::from_executor_config(
        openlife_core::agent::ActionExecutorConfig {
            allow_writes: true,
            allow_cloud: true,
            ..Default::default()
        },
    )
    .execute(
        openlife_core::agent::AgentActionRequest {
            action_type: "builtin_tool".into(),
            target: "a2a.call_agent".into(),
            input: serde_json::json!({
                "url": url,
                "task": task_text,
                "session_id": req.session_id,
                "request_id": req.id,
            }),
            source_run_id: Some(execution_run.id.clone()),
            step_index: 0,
        },
        &action_ctx,
    )
    .await;
    let result = match execution {
        Ok(result) => result,
        Err(error) => {
            crate::terminal_owner_write_gateway::register_agent_run_store_error(state, &error);
            if let Err(recovery_error) = resources
                .agent_run_store
                .reconcile_agent_run_tool_execution_owner_now()
            {
                crate::terminal_owner_write_gateway::register_agent_run_store_error(
                    state,
                    &recovery_error,
                );
                return Err(AppError::internal(format!(
                    "a2a_tool_gateway_execution_and_owner_reconciliation_failed:{recovery_error}"
                )));
            }
            return Err(AppError::from(error));
        }
    };
    if result.status != openlife_core::agent::ActionExecutionStatus::Succeeded {
        return Err(AppError::permission(
            result
                .stop_reason
                .or(result.action.error)
                .unwrap_or_else(|| "a2a_tool_gateway_execution_failed".into()),
        ));
    }
    result
        .action
        .output
        .and_then(|value| {
            value
                .get("text")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })
        .ok_or_else(|| AppError::internal("a2a_tool_gateway_output_missing"))
}

#[tauri::command]
pub async fn a2a_local_agent_card(state: State<'_, Arc<AppState>>) -> Result<AgentCard, AppError> {
    let model = {
        let manager = state.life_model_manager.lock().await;
        manager
            .load_active_legacy_runtime_model()
            .map_err(AppError::from)?
            .unwrap_or_else(openlife_core::life_model::LifeModel::default_model)
    };
    Ok(A2AServerHandler::default_agent_card(
        crate::a2a_server::configured_a2a_port(),
        &model,
    ))
}

#[tauri::command]
pub async fn a2a_handle_task(
    request_json: String,
    state: State<'_, Arc<AppState>>,
) -> Result<String, AppError> {
    let req: SendTaskRequest = serde_json::from_str(&request_json).map_err(AppError::from)?;
    let life_model = {
        let manager = state.life_model_manager.lock().await;
        manager
            .load_active_legacy_runtime_model()
            .map_err(AppError::from)?
            .unwrap_or_else(openlife_core::life_model::LifeModel::default_model)
    };
    let privacy_engine = state.privacy_engine.lock().await.clone();
    let handler = A2AServerHandler {
        life_model,
        privacy_engine,
    };
    let resp = handler.handle_task(req).await;
    serde_json::to_string(&resp).map_err(AppError::from)
}

#[tauri::command]
pub async fn a2a_bridge_local(
    session_id: Option<String>,
    _method: String,
    text: String,
    skill: Option<String>,
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, AppError> {
    let req = ReasoningInput {
        task_kind: openlife_core::agent::AgentTaskKind::Conversation,
        user_text: text.clone(),
        session_id: session_id.clone().unwrap_or_default(),
    };
    let a2a_req = reasoning_input_to_a2a_task(&req, skill.as_deref(), None);
    let life_model = {
        let manager = state.life_model_manager.lock().await;
        manager
            .load_active_legacy_runtime_model()
            .map_err(AppError::from)?
            .unwrap_or_else(openlife_core::life_model::LifeModel::default_model)
    };
    let privacy_engine = state.privacy_engine.lock().await.clone();
    let handler = A2AServerHandler {
        life_model,
        privacy_engine,
    };
    let resp = handler.handle_task(a2a_req).await;
    let reasoning_result = a2a_response_to_reasoning_result(&resp).map_err(AppError::from)?;
    let bridge_preview = reasoning_input_to_a2a_task(
        &ReasoningInput {
            task_kind: openlife_core::agent::AgentTaskKind::Conversation,
            user_text: text,
            session_id: session_id.unwrap_or_default(),
        },
        None,
        None,
    );
    Ok(serde_json::json!({
        "request": {
            "task_kind": "conversation",
            "user_text": req.user_text,
            "session_id": req.session_id,
        },
        "a2a_request": bridge_preview,
        "response": resp,
        "reasoning_result": reasoning_result,
    }))
}

#[tauri::command]
pub async fn a2a_restart_sidecar(state: State<'_, Arc<AppState>>) -> Result<(), AppError> {
    let sidecar = state.a2a_sidecar.lock().await.clone();
    sidecar.stop()?;
    sidecar.start().await
}

#[tauri::command]
pub async fn a2a_stop_sidecar(state: State<'_, Arc<AppState>>) -> Result<(), AppError> {
    let sidecar = state.a2a_sidecar.lock().await.clone();
    sidecar.stop()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[test]
    fn outbound_task_command_has_no_direct_a2a_client_dispatch_bypass() {
        let source = include_str!("a2a.rs");
        let production_source = source
            .split("#[cfg(test)]\nmod tests")
            .next()
            .expect("production A2A command source");
        assert!(!production_source.contains("client.send_task("));
        assert!(production_source.contains("ToolGateway::from_executor_config"));
        assert!(production_source.contains("with_a2a_outbound_authorization"));
        assert!(production_source.contains("AgentRun::new_tool_execution_run"));
        assert_eq!(
            production_source
                .matches(".update_run(&execution_run)")
                .count(),
            0,
            "A2A terminalization must stay inside the atomic durable owner, not a second command-layer update"
        );
        assert!(production_source.contains("new_for_unpersisted_run"));
        assert!(production_source.contains("reconcile_agent_run_tool_execution_owner_now"));
    }

    fn task_json(id: &str) -> String {
        serde_json::to_string(&SendTaskRequest {
            id: id.into(),
            session_id: Some("a2a-consent-session".into()),
            message: openlife_core::a2a::Message {
                role: "user".into(),
                parts: vec![openlife_core::a2a::Part::Text {
                    text: "send after review".into(),
                }],
                metadata: None,
            },
            accepted_output_modes: Some(vec!["text".into()]),
            push_notification: None,
            history_length: None,
            metadata: None,
        })
        .unwrap()
    }

    async fn accept_next_a2a_consent(
        state: &Arc<AppState>,
        url: &str,
        request_json: &str,
        token: &str,
    ) {
        let pending = a2a_send_task_with_state(url, request_json, Some(token.to_string()), state)
            .await
            .unwrap_err();
        assert!(pending.message().contains("proposal_id="));
        let proposal = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_proposals_filtered(
                Some(openlife_core::agent::ProposalStatus::Pending),
                Some(openlife_core::agent::ProposalType::ToolPermission),
                None,
                20,
            )
            .unwrap()
            .into_iter()
            .find(|proposal| {
                proposal
                    .source_detail
                    .as_deref()
                    .is_some_and(|source| source.starts_with("a2a_network_consent:"))
            })
            .expect("A2A consent proposal");
        crate::commands::proposal::accept_proposal_with_state(proposal.id, state)
            .await
            .unwrap();
    }

    async fn read_one_http_request(socket: &mut tokio::net::TcpStream) -> String {
        let mut bytes = Vec::new();
        let mut buffer = [0_u8; 4096];
        loop {
            let read = socket.read(&mut buffer).await.unwrap();
            if read == 0 {
                break;
            }
            bytes.extend_from_slice(&buffer[..read]);
            if let Some(headers_end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
                let headers_end = headers_end + 4;
                let headers = String::from_utf8_lossy(&bytes[..headers_end]);
                let content_length = headers
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length: ")
                            .and_then(|value| value.trim().parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                if bytes.len() >= headers_end + content_length {
                    break;
                }
            }
        }
        String::from_utf8(bytes).unwrap()
    }

    fn spawn_a2a_server(
        listener: tokio::net::TcpListener,
        request_id: String,
        respond: bool,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let request = read_one_http_request(&mut socket).await;
            assert!(request.contains("authorization: Bearer paired-"));
            if !respond {
                return;
            }
            let body = serde_json::json!({
                "id": request_id,
                "status": {"state": "COMPLETED", "message": null},
                "artifacts": null,
                "history": null,
                "metadata": null
            })
            .to_string();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(), body
            );
            socket.write_all(response.as_bytes()).await.unwrap();
        })
    }

    #[tokio::test]
    async fn invalid_request_id_cannot_mint_consent_owner_or_dispatch_on_repeated_attempts() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let request_json = task_json("caller-controlled-non-uuid");
        let token = "paired-invalid-012345678901234567890123";

        for _ in 0..2 {
            let error = a2a_send_task_with_state(&url, &request_json, Some(token.into()), &state)
                .await
                .unwrap_err();
            assert!(error.message().contains("canonical UUIDv4"));
        }
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(100), listener.accept())
                .await
                .is_err()
        );
        assert!(state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_proposals_filtered(
                Some(openlife_core::agent::ProposalStatus::Pending),
                Some(openlife_core::agent::ProposalType::ToolPermission),
                None,
                20,
            )
            .unwrap()
            .into_iter()
            .all(
                |proposal| proposal.source != openlife_core::agent::ProposalSource::NetworkConsent
            ));
        assert!(state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .get_run("caller-controlled-non-uuid")
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn durable_a2a_fault_boundaries_never_confirm_failure_or_dispatch_twice() {
        use openlife_core::agent::{
            AgentRunToolExecutionFaultPoint as FaultPoint,
            AgentRunToolExecutionState as ExecutionState,
        };

        #[derive(Clone, Copy)]
        struct Scenario {
            name: &'static str,
            fault: Option<FaultPoint>,
            first_send_expected: bool,
            server_responds: bool,
            expected_state: Option<ExecutionState>,
            expected_parent_status: Option<openlife_core::agent::AgentRunStatus>,
        }

        let scenarios = [
            Scenario {
                name: "prepare_failure",
                fault: Some(FaultPoint::Prepare),
                first_send_expected: false,
                server_responds: false,
                expected_state: None,
                expected_parent_status: None,
            },
            Scenario {
                name: "dispatch_cas_failure_before_send",
                fault: Some(FaultPoint::DispatchAttempted),
                first_send_expected: false,
                server_responds: false,
                expected_state: Some(ExecutionState::TerminalNotAttempted),
                expected_parent_status: Some(openlife_core::agent::AgentRunStatus::Failed),
            },
            Scenario {
                name: "send_then_disconnect_before_response",
                fault: None,
                first_send_expected: true,
                server_responds: false,
                expected_state: Some(ExecutionState::TerminalRemoteUnknown),
                expected_parent_status: Some(openlife_core::agent::AgentRunStatus::RemoteUnknown),
            },
            Scenario {
                name: "response_transition_failure",
                fault: Some(FaultPoint::ResponseObserved),
                first_send_expected: true,
                server_responds: true,
                expected_state: Some(ExecutionState::TerminalRemoteUnknown),
                expected_parent_status: Some(openlife_core::agent::AgentRunStatus::RemoteUnknown),
            },
            Scenario {
                name: "terminal_failure_after_response",
                fault: Some(FaultPoint::Terminal),
                first_send_expected: true,
                server_responds: true,
                expected_state: Some(ExecutionState::ResponseObserved),
                expected_parent_status: Some(openlife_core::agent::AgentRunStatus::Running),
            },
            Scenario {
                name: "receipt_issuance_failure",
                fault: Some(FaultPoint::BoundContentReceiptIssuance),
                first_send_expected: true,
                server_responds: true,
                expected_state: Some(ExecutionState::TerminalRemoteUnknown),
                expected_parent_status: Some(openlife_core::agent::AgentRunStatus::RemoteUnknown),
            },
            Scenario {
                name: "agent_run_update_failure",
                fault: Some(FaultPoint::AgentRunUpdate),
                first_send_expected: true,
                server_responds: true,
                expected_state: Some(ExecutionState::ResponseObserved),
                expected_parent_status: Some(openlife_core::agent::AgentRunStatus::Running),
            },
        ];

        for scenario in scenarios {
            let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let listener_addr = listener.local_addr().unwrap();
            let url = format!("http://{listener_addr}");
            let request_id = uuid::Uuid::new_v4().to_string();
            let request_json = task_json(&request_id);
            let token = format!("paired-{}-012345678901234567890123", scenario.name);
            accept_next_a2a_consent(&state, &url, &request_json, &token).await;
            if let Some(fault) = scenario.fault {
                state
                    .agent_run_store
                    .as_ref()
                    .unwrap()
                    .lock()
                    .await
                    .install_tool_execution_fault_for_test(fault)
                    .unwrap();
            }

            let mut idle_listener = Some(listener);
            let first_server = scenario.first_send_expected.then(|| {
                spawn_a2a_server(
                    idle_listener.take().unwrap(),
                    request_id.clone(),
                    scenario.server_responds,
                )
            });
            let first =
                a2a_send_task_with_state(&url, &request_json, Some(token.clone()), &state).await;
            assert!(first.is_err(), "{} must fail closed", scenario.name);
            if let Some(server) = first_server {
                server.await.unwrap();
            } else {
                assert!(
                    tokio::time::timeout(
                        std::time::Duration::from_millis(100),
                        idle_listener.as_ref().unwrap().accept()
                    )
                    .await
                    .is_err(),
                    "{} crossed the network boundary before its CAS fence",
                    scenario.name
                );
            }
            drop(idle_listener);

            let durable = state
                .agent_run_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .list_agent_run_tool_executions(&request_id)
                .unwrap();
            match scenario.expected_state {
                None => assert!(
                    durable.is_empty(),
                    "{} minted a prepared row",
                    scenario.name
                ),
                Some(expected) => {
                    assert_eq!(durable.len(), 1, "{} durable row count", scenario.name);
                    assert_eq!(
                        durable[0].state, expected,
                        "{} durable state",
                        scenario.name
                    );
                    assert!(!durable[0].automatic_retry_safe());
                    if expected != ExecutionState::TerminalSucceeded {
                        assert_ne!(
                            durable[0].effect_status,
                            openlife_core::agent::ToolEffectStatus::Confirmed,
                            "{} must not turn a persistence failure into confirmed failure",
                            scenario.name
                        );
                    }
                }
            }
            let parent = state
                .agent_run_store
                .as_ref()
                .unwrap()
                .lock()
                .await
                .get_run(&request_id)
                .unwrap();
            match scenario.expected_parent_status {
                None => assert!(parent.is_none(), "{} left a parent row", scenario.name),
                Some(expected) => assert_eq!(
                    parent.expect("durable A2A parent").status,
                    expected,
                    "{} parent status",
                    scenario.name
                ),
            }

            accept_next_a2a_consent(&state, &url, &request_json, &token).await;
            let replay_listener = tokio::net::TcpListener::bind(listener_addr).await.unwrap();
            let replay =
                a2a_send_task_with_state(&url, &request_json, Some(token.clone()), &state).await;
            assert!(replay.is_err(), "{} replay must fail closed", scenario.name);
            assert!(
                tokio::time::timeout(
                    std::time::Duration::from_millis(100),
                    replay_listener.accept()
                )
                .await
                .is_err(),
                "{} dispatched the non-idempotent request twice",
                scenario.name
            );
        }
    }

    #[tokio::test]
    async fn network_ask_stages_review_and_allow_once_dispatches_exactly_one_a2a_task() {
        let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let listener_addr = listener.local_addr().unwrap();
        let url = format!("http://{listener_addr}");
        let request_id = "123e4567-e89b-42d3-a456-426614174000";
        let request_json = task_json(request_id);
        let token = "paired-consent-012345678901234567890123";

        let pending = a2a_send_task_with_state(&url, &request_json, Some(token.into()), &state)
            .await
            .unwrap_err();
        assert!(pending.message().contains("proposal_id="));
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(100), listener.accept())
                .await
                .is_err()
        );

        let duplicate_pending =
            a2a_send_task_with_state(&url, &request_json, Some(token.into()), &state)
                .await
                .unwrap_err();
        assert!(duplicate_pending.message().contains("proposal_id="));

        let pending_proposals = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_proposals_filtered(
                Some(openlife_core::agent::ProposalStatus::Pending),
                Some(openlife_core::agent::ProposalType::ToolPermission),
                None,
                20,
            )
            .unwrap()
            .into_iter()
            .filter(|proposal| {
                proposal
                    .source_detail
                    .as_deref()
                    .is_some_and(|source| source.starts_with("a2a_network_consent:"))
            })
            .collect::<Vec<_>>();
        assert_eq!(
            pending_proposals.len(),
            1,
            "repeat ask must reuse one pending review item"
        );
        let proposal = pending_proposals
            .into_iter()
            .next()
            .expect("A2A consent must use the authoritative ReviewWorkflow");
        assert_eq!(
            proposal.source,
            openlife_core::agent::ProposalSource::NetworkConsent,
            "an explicit A2A command must not claim Main Chat proposal authority"
        );
        assert_eq!(proposal.after["source"], "a2a");
        assert_eq!(
            proposal.after["blocked_action"]["action_type"],
            "a2a_task_dispatch"
        );
        crate::commands::proposal::accept_proposal_with_state(proposal.id, &state)
            .await
            .unwrap();

        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            let mut buffer = [0_u8; 4096];
            loop {
                let read = socket.read(&mut buffer).await.unwrap();
                if read == 0 {
                    break;
                }
                bytes.extend_from_slice(&buffer[..read]);
                if let Some(headers_end) = bytes.windows(4).position(|window| window == b"\r\n\r\n")
                {
                    let headers_end = headers_end + 4;
                    let headers = String::from_utf8_lossy(&bytes[..headers_end]);
                    let content_length = headers
                        .lines()
                        .find_map(|line| {
                            line.to_ascii_lowercase()
                                .strip_prefix("content-length: ")
                                .and_then(|value| value.trim().parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    if bytes.len() >= headers_end + content_length {
                        break;
                    }
                }
            }
            let request = String::from_utf8(bytes).unwrap();
            assert!(request.contains("authorization: Bearer paired-consent"));
            assert!(request.contains("\"contextManifest\""));
            let body = serde_json::json!({
                "id": request_id,
                "status": {"state": "COMPLETED", "message": null},
                "artifacts": null,
                "history": null,
                "metadata": null
            })
            .to_string();
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(), body
            );
            socket.write_all(response.as_bytes()).await.unwrap();
        });
        let response = a2a_send_task_with_state(&url, &request_json, Some(token.into()), &state)
            .await
            .unwrap();
        assert!(response.contains("remote_reported_completed"));
        server.await.unwrap();
        let persisted_run = state
            .agent_run_store
            .as_ref()
            .expect("canonical AgentRun owner")
            .lock()
            .await
            .get_run(request_id)
            .unwrap()
            .expect("accepted A2A execution is attached to its UUIDv4 owner");
        assert_eq!(persisted_run.id, request_id);
        assert_eq!(
            persisted_run.status,
            openlife_core::agent::AgentRunStatus::Completed
        );
        assert_eq!(persisted_run.actions.len(), 1);
        assert_eq!(persisted_run.observations.len(), 1);
        let trace = persisted_run.actions[0]
            .react_trace
            .as_ref()
            .expect("durable A2A action trace");
        assert_eq!(trace.run_id.as_deref(), Some(request_id));
        let bound_body = trace
            .output_receipt
            .as_ref()
            .expect("A2A adapter body receipt is CAS-attached to the run");
        assert_eq!(bound_body.version(), 2);
        assert!(persisted_run.observations[0].react_trace.is_none());
        let durable_attempts = state
            .agent_run_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_agent_run_tool_executions(request_id)
            .unwrap();
        assert_eq!(durable_attempts.len(), 1);
        let durable_attempt = &durable_attempts[0];
        assert_eq!(
            durable_attempt.state,
            openlife_core::agent::AgentRunToolExecutionState::TerminalSucceeded
        );
        assert_eq!(
            durable_attempt.idempotency_contract,
            openlife_core::tool_manifest::ToolIdempotencyContract::NonIdempotent
        );
        assert_eq!(durable_attempt.dispatch_attempt_count, 1);
        assert_eq!(
            durable_attempt.transport_status,
            openlife_core::agent::ToolTransportStatus::ResponseObserved
        );
        assert_eq!(
            durable_attempt.effect_status,
            openlife_core::agent::ToolEffectStatus::Confirmed
        );
        assert!(!durable_attempt.automatic_retry_safe());
        assert!(!durable_attempt.endpoint_digest.contains(&url));
        assert!(!durable_attempt.request_digest.contains("send after review"));

        let consumed = a2a_send_task_with_state(&url, &request_json, Some(token.into()), &state)
            .await
            .unwrap_err();
        assert!(consumed.message().contains("proposal_id="));

        let repeated_proposal = state
            .proposal_store
            .as_ref()
            .unwrap()
            .lock()
            .await
            .list_proposals_filtered(
                Some(openlife_core::agent::ProposalStatus::Pending),
                Some(openlife_core::agent::ProposalType::ToolPermission),
                None,
                20,
            )
            .unwrap()
            .into_iter()
            .find(|proposal| {
                proposal
                    .source_detail
                    .as_deref()
                    .is_some_and(|source| source.starts_with("a2a_network_consent:"))
            })
            .expect("consumed AllowOnce stages a fresh review item");
        crate::commands::proposal::accept_proposal_with_state(repeated_proposal.id, &state)
            .await
            .unwrap();
        let replay_listener = tokio::net::TcpListener::bind(listener_addr).await.unwrap();
        let duplicate_owner =
            a2a_send_task_with_state(&url, &request_json, Some(token.into()), &state)
                .await
                .unwrap_err();
        assert!(
            !duplicate_owner.message().contains("proposal_id="),
            "the second approval reached the canonical run-id replay barrier"
        );
        assert!(
            tokio::time::timeout(
                std::time::Duration::from_millis(100),
                replay_listener.accept()
            )
            .await
            .is_err(),
            "an existing non-idempotent A2A owner must block a second network dispatch"
        );
    }
}
