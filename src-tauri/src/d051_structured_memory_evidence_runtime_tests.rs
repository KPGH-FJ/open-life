use openlife_core::agent::main_chat_agent_v1::{
    CanonicalToolReplayAuthority, ExecutionTranscriptEntry, ExecutionTranscriptEntryKind,
    QueuedExecutionAction,
};
use openlife_core::agent::{AgentAction, AgentObservation, AgentProposal, AgentRun};
use openlife_core::llm::ChatMessage;
use openlife_core::tool_execution_receipt::ToolExecutionReceipt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const USER_PROMPT: &str = "Read file `src-tauri/test-fixtures/d051_structured_evidence_private.md` and create a memory proposal only if the observation contains a useful supported personal fact.";
const PROVIDER_CONTROL_PROMPT: &str = "Give one concise focus tip for the D051 provider control.";
const PROVIDER_CONTROL_REPLY: &str = "D051_PROVIDER_CONTROL_OK";
const OBSERVATION_BODY: &str = include_str!("../test-fixtures/d051_structured_evidence_private.md");
const RAW_SENTINEL: &str = "D051_RAW_OBSERVATION_SENTINEL";
const CANDIDATE_TEXT: &str = "The user works in UTC.";

fn sha256(value: &[u8]) -> String {
    let digest = ring::digest::digest(&ring::digest::SHA256, value);
    let hex = digest
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("sha256:{hex}")
}

fn observation_ref_placeholder() -> &'static str {
    // This token exists only in the local HTTP server's response template. The
    // server replaces it with the exact observation ref it captures from the
    // real final provider request before any bytes are returned to OpenLife.
    "$OPENLIFE_FINAL_CONTEXT_OBSERVATION_REF"
}

fn captured_observation_ref(request: &str) -> Option<&str> {
    let start = request.find("agent-run://")?;
    let tail = &request[start..];
    let end = tail
        .find(|ch: char| ch == '"' || ch == '\\' || ch.is_whitespace() || ch == ']')
        .unwrap_or(tail.len());
    (end > "agent-run://".len()).then_some(&tail[..end])
}

fn positive_final_response() -> String {
    let start = OBSERVATION_BODY
        .find(CANDIDATE_TEXT)
        .expect("D051 fixture candidate");
    let end = start + CANDIDATE_TEXT.len();
    serde_json::json!({
        "final": "The governed read completed.",
        "actions": [],
        "thought_summary": "The observation is sufficient.",
        "warnings": [],
        "memory_evidence_schema": "openlife.memory_evidence.v1",
        "memory_evidence": [{
            "candidate_text": CANDIDATE_TEXT,
            "subject": "current_user",
            "assertion": "asserted_fact",
            "modality": "asserted",
            "confidence": 0.93,
            "evidence": {
                "observation_ref": observation_ref_placeholder(),
                "start_byte": start,
                "end_byte": end,
                "sha256": sha256(&OBSERVATION_BODY.as_bytes()[start..end]),
            }
        }]
    })
    .to_string()
}

fn no_extractor_final_response() -> String {
    serde_json::json!({
        "final": "The governed read completed without structured Memory evidence.",
        "actions": [],
        "thought_summary": "The observation is sufficient for the answer only.",
        "warnings": []
    })
    .to_string()
}

fn read_http_request(stream: &mut std::net::TcpStream) -> Vec<u8> {
    let mut request_bytes = Vec::new();
    let mut buffer = [0u8; 8192];
    let mut expected_len = None;
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        match std::io::Read::read(stream, &mut buffer) {
            Ok(0) => break,
            Ok(read) => request_bytes.extend_from_slice(&buffer[..read]),
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock
                        | std::io::ErrorKind::TimedOut
                        | std::io::ErrorKind::Interrupted
                ) =>
            {
                if std::time::Instant::now() >= deadline {
                    break;
                }
                continue;
            }
            Err(_) => break,
        }
        let request = String::from_utf8_lossy(&request_bytes);
        if expected_len.is_none() {
            if let Some((headers, _)) = request.split_once("\r\n\r\n") {
                let content_length = headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().ok())
                            .flatten()
                    })
                    .unwrap_or(0);
                expected_len = Some(headers.len() + 4 + content_length);
            }
        }
        if expected_len.is_some_and(|expected| request_bytes.len() >= expected) {
            break;
        }
    }
    request_bytes
}

fn provider_http_response(request: &str, content: &str, index: usize) -> (String, String) {
    let streaming = request
        .split_once("\r\n\r\n")
        .and_then(|(_, body)| serde_json::from_str::<serde_json::Value>(body).ok())
        .and_then(|body| body.get("stream").and_then(serde_json::Value::as_bool))
        .unwrap_or(false);
    if streaming {
        let chunk = serde_json::json!({
            "id": format!("chatcmpl-d051-stream-{index}"),
            "object": "chat.completion.chunk",
            "choices": [{
                "index": 0,
                "delta": {"content": content},
                "finish_reason": null
            }]
        });
        let terminal = serde_json::json!({
            "id": format!("chatcmpl-d051-stream-{index}"),
            "object": "chat.completion.chunk",
            "choices": [{
                "index": 0,
                "delta": {},
                "finish_reason": "stop"
            }]
        });
        (
            "text/event-stream".into(),
            format!("data: {chunk}\n\ndata: {terminal}\n\ndata: [DONE]\n\n"),
        )
    } else {
        (
            "application/json".into(),
            serde_json::json!({
                "id": format!("chatcmpl-d051-{index}"),
                "object": "chat.completion",
                "choices": [{
                    "index": 0,
                    "message": {"role": "assistant", "content": content},
                    "finish_reason": "stop"
                }]
            })
            .to_string(),
        )
    }
}

struct CapturedProvider {
    requests: Arc<Mutex<Vec<String>>>,
    control_count: Arc<AtomicUsize>,
    generation_count: Arc<AtomicUsize>,
    ranking_count: Arc<AtomicUsize>,
    final_request_reached: Option<Arc<std::sync::atomic::AtomicBool>>,
    final_response_release: Option<Arc<std::sync::atomic::AtomicBool>>,
}

impl CapturedProvider {
    fn request_count(&self) -> usize {
        self.requests.lock().expect("D051 request capture").len()
    }

    fn generation_count(&self) -> usize {
        self.generation_count.load(Ordering::SeqCst)
    }

    fn control_count(&self) -> usize {
        self.control_count.load(Ordering::SeqCst)
    }

    fn ranking_count(&self) -> usize {
        self.ranking_count.load(Ordering::SeqCst)
    }

    fn captured(&self) -> Vec<String> {
        self.requests.lock().expect("D051 request capture").clone()
    }
}

async fn configure_captured_provider(
    state: &Arc<crate::AppState>,
    final_response: String,
    hold_final_response: bool,
) -> CapturedProvider {
    let listener =
        std::net::TcpListener::bind("127.0.0.1:0").expect("bind D051 captured local provider");
    let address = listener.local_addr().expect("D051 provider address");
    listener
        .set_nonblocking(true)
        .expect("D051 provider nonblocking listener");
    let requests = Arc::new(Mutex::new(Vec::new()));
    let requests_for_server = Arc::clone(&requests);
    let control_count = Arc::new(AtomicUsize::new(0));
    let control_for_server = Arc::clone(&control_count);
    let generation_count = Arc::new(AtomicUsize::new(0));
    let generation_for_server = Arc::clone(&generation_count);
    let ranking_count = Arc::new(AtomicUsize::new(0));
    let ranking_for_server = Arc::clone(&ranking_count);
    let final_request_reached =
        hold_final_response.then(|| Arc::new(std::sync::atomic::AtomicBool::new(false)));
    let final_response_release =
        hold_final_response.then(|| Arc::new(std::sync::atomic::AtomicBool::new(false)));
    let reached_for_server = final_request_reached.as_ref().map(Arc::clone);
    let release_for_server = final_response_release.as_ref().map(Arc::clone);

    std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        while std::time::Instant::now() < deadline {
            let (mut stream, _) = match listener.accept() {
                Ok(accepted) => accepted,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(5));
                    continue;
                }
                Err(_) => break,
            };
            stream
                .set_nonblocking(false)
                .expect("D051 provider request socket blocking");
            let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
            let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
            let request = String::from_utf8_lossy(&read_http_request(&mut stream)).to_string();
            requests_for_server
                .lock()
                .expect("record D051 provider request")
                .push(request.clone());
            let is_control = request.contains(PROVIDER_CONTROL_PROMPT);
            let is_ranking = request.contains("Return ranked_candidate_ids now")
                || request.contains("Metadata-safe candidate contract");
            let (reply, generation_index) = if is_control {
                control_for_server.fetch_add(1, Ordering::SeqCst);
                (PROVIDER_CONTROL_REPLY.to_string(), None)
            } else if is_ranking {
                ranking_for_server.fetch_add(1, Ordering::SeqCst);
                (
                    serde_json::json!({"ranked_candidate_ids":["file.read"]}).to_string(),
                    None,
                )
            } else {
                let index = generation_for_server.fetch_add(1, Ordering::SeqCst);
                let exact_observation_ref = captured_observation_ref(&request)
                    .expect("post-observation provider request carries exact observation ref");
                let reply =
                    final_response.replace(observation_ref_placeholder(), exact_observation_ref);
                (reply, Some(index))
            };
            if generation_index == Some(0) {
                if let Some(reached) = &reached_for_server {
                    reached.store(true, Ordering::SeqCst);
                }
                if let Some(release) = &release_for_server {
                    let release_deadline = std::time::Instant::now() + Duration::from_secs(30);
                    while !release.load(Ordering::SeqCst)
                        && std::time::Instant::now() < release_deadline
                    {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                }
            }
            let response_index = requests_for_server
                .lock()
                .expect("count D051 provider requests")
                .len();
            let (content_type, body) = provider_http_response(&request, &reply, response_index);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}",
                body.len()
            );
            let _ = std::io::Write::write_all(&mut stream, response.as_bytes());
        }
    });

    let mut config = state.config.lock().await.clone();
    config.llm.provider = "openai".into();
    config.llm.openai_base = format!("http://{address}/v1");
    config.llm.openai_key = "d051-test-key".into();
    config.llm.chat_model = "gpt-d051-captured-local".into();
    config.prefer_local_model = false;
    config.system.network_policy.enabled = true;
    config.system.network_policy.default_decision = "allow".into();
    state.replace_provider_runtime_config(config).await;

    let capture = CapturedProvider {
        requests,
        control_count,
        generation_count,
        ranking_count,
        final_request_reached,
        final_response_release,
    };

    let control = crate::main_chat_send::send_message_with_operation_state(
        uuid::Uuid::new_v4().to_string(),
        "d051-provider-control".into(),
        vec![ChatMessage {
            role: "user".into(),
            content: PROVIDER_CONTROL_PROMPT.into(),
        }],
        None,
        state,
    )
    .await
    .expect("D051 same-state provider control turn");
    assert_eq!(
        control.provider_invocation_status,
        crate::main_chat_turn_runtime::ProviderInvocationState::Completed
    );
    assert!(control.reply.contains(PROVIDER_CONTROL_REPLY));
    assert_eq!(capture.request_count(), 1);
    assert_eq!(capture.control_count(), 1);
    assert_eq!(capture.generation_count(), 0);
    assert_eq!(capture.ranking_count(), 0);
    capture
}

fn messages() -> Vec<ChatMessage> {
    vec![ChatMessage {
        role: "user".into(),
        content: USER_PROMPT.into(),
    }]
}

async fn canonical_state_digest(state: &Arc<crate::AppState>) -> String {
    let memory = state
        .memory_lifecycle_store
        .as_ref()
        .expect("D051 MemoryLifecycleStore")
        .lock()
        .await
        .list_records(None, None, 200, 0)
        .expect("list D051 canonical Memory records");
    let manager = state.life_model_manager.lock().await;
    let life_model = manager.load().expect("load D051 canonical LifeModel");
    let hs_registry_path = manager.hs_asset_authority_registry_path();
    drop(manager);
    let hs_registry = std::fs::read(&hs_registry_path).unwrap_or_default();
    openlife_core::agent::metadata_safe::metadata_safe_value_digest(&serde_json::json!({
        "memory": memory,
        "lifeModel": life_model,
        "hsRegistryDigest": sha256(&hs_registry),
    }))
    .1
}

struct RuntimeArtifacts {
    events: Vec<crate::main_chat_event_stream::MainChatAgentDurableEvent>,
    proposals: Vec<AgentProposal>,
    run: AgentRun,
    actions: Vec<QueuedExecutionAction>,
    transcript: Vec<ExecutionTranscriptEntry>,
    // `file.read` is a built-in ToolGateway execution and is not expected to
    // emit MCP audit rows. This global store is loaded only as an additional
    // sensitive-body leak counterexample, never as proof of this execution.
    mcp_audit_leak_scan: Vec<openlife_core::mcp_audit::McpLogEntry>,
}

fn exact_one<'a, T>(matches: Vec<&'a T>, label: &str) -> &'a T {
    assert_eq!(
        matches.len(),
        1,
        "expected exactly one {label}; duplicates and missing owners both fail closed"
    );
    matches[0]
}

impl RuntimeArtifacts {
    fn final_event(&self) -> &crate::main_chat_event_stream::MainChatAgentDurableEvent {
        let event = exact_one(
            self.events
                .iter()
                .filter(|event| event.event_type == "final_delivery.created")
                .collect(),
            "D051 durable final delivery receipt",
        );
        let delivery_id = format!("delivery:{}:{}", self.run.id, self.run.id);
        assert_eq!(event.task_session_id, self.run.id);
        assert_eq!(event.run_id, self.run.id);
        assert_eq!(event.object_type, "final_delivery");
        assert_eq!(event.object_id, delivery_id);
        assert_eq!(event.source, "openlife_turn_runtime.final_delivery_owner");
        assert_eq!(event.payload["deliveryId"], delivery_id);
        assert_eq!(event.payload["taskSessionId"], self.run.id);
        assert_eq!(event.payload["runId"], self.run.id);
        assert_eq!(event.payload["bodyStored"], false);
        event
    }

    fn serialized_persistence(&self) -> String {
        serde_json::json!({
            "events": self.events,
            "proposals": self.proposals,
            "run": self.run,
            "actions": self.actions,
            "transcript": self.transcript,
            "globalMcpAuditLeakScan": self.mcp_audit_leak_scan,
        })
        .to_string()
    }
}

async fn load_artifacts(state: &Arc<crate::AppState>, operation_id: &str) -> RuntimeArtifacts {
    let events = crate::main_chat_event_stream::list_main_chat_agent_events_with_state(
        state,
        operation_id.to_string(),
        Some(0),
        Some(250),
    )
    .await
    .expect("list D051 durable events");
    let proposals = state
        .proposal_store
        .as_ref()
        .expect("D051 ProposalStore")
        .lock()
        .await
        .list_all_proposals(200, 0)
        .expect("list D051 proposals");
    let run = state
        .agent_run_store
        .as_ref()
        .expect("D051 AgentRunStore")
        .lock()
        .await
        .get_run(operation_id)
        .expect("load D051 AgentRun")
        .expect("D051 AgentRun exists");
    let actions = state
        .main_chat_action_queue_store
        .as_ref()
        .expect("D051 ActionQueue")
        .lock()
        .await
        .list_for_session(operation_id)
        .expect("list D051 action queue");
    let transcript = state
        .main_chat_agent_session_store
        .as_ref()
        .expect("D051 TaskSessionStore")
        .lock()
        .await
        .list_transcript_entries(operation_id)
        .expect("list D051 transcript");
    let mcp_audit_leak_scan = state
        .mcp_audit_store
        .lock()
        .await
        .list_logs(200)
        .expect("list D051 audit records");
    RuntimeArtifacts {
        events,
        proposals,
        run,
        actions,
        transcript,
        mcp_audit_leak_scan,
    }
}

async fn run_buffered(
    state: &Arc<crate::AppState>,
    operation_id: &str,
    session_id: &str,
) -> Result<serde_json::Value, String> {
    crate::main_chat_send::send_message_with_operation_state(
        operation_id.to_string(),
        session_id.to_string(),
        messages(),
        None,
        state,
    )
    .await
    .and_then(|result| serde_json::to_value(result).map_err(|error| error.to_string()))
}

async fn run_streaming(
    state: &Arc<crate::AppState>,
    operation_id: &str,
    session_id: &str,
) -> Result<serde_json::Value, String> {
    crate::main_chat_streaming::start_stream_message_with_operation_state(
        operation_id.to_string(),
        session_id.to_string(),
        messages(),
        None,
        state,
        |_, _| {},
    )
    .await
}

fn assert_one_post_observation_provider_call(capture: &CapturedProvider) {
    assert_eq!(
        capture.request_count(),
        2,
        "one proven control call plus exactly one post-observation target call is required"
    );
    assert_eq!(capture.control_count(), 1);
    assert_eq!(capture.generation_count(), 1);
    assert_eq!(
        capture.ranking_count(),
        0,
        "one exact file.read candidate needs no ranking call"
    );
    let requests = capture.captured();
    assert!(requests[0].contains(PROVIDER_CONTROL_PROMPT));
    assert!(
        requests[1].contains(CANDIDATE_TEXT),
        "the sole target request must carry the exact post-read observation context"
    );
}

struct RuntimeReadOwnerGraph<'a> {
    action: &'a AgentAction,
    observation: &'a AgentObservation,
    output_receipt: &'a openlife_core::agent::BoundContentReceipt,
    canonical_replay_authority: &'a CanonicalToolReplayAuthority,
    queue_action: &'a QueuedExecutionAction,
    transcript_observation: &'a ExecutionTranscriptEntry,
}

fn assert_real_durable_file_read_owner_graph<'a>(
    artifacts: &'a RuntimeArtifacts,
    operation_id: &str,
) -> RuntimeReadOwnerGraph<'a> {
    let matching_actions = artifacts
        .run
        .actions
        .iter()
        .filter(|action| {
            action.status == "succeeded"
                && action
                    .react_trace
                    .as_ref()
                    .is_some_and(|trace| trace.tool_id == "file.read")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matching_actions.len(),
        1,
        "the canonical AgentRun must own exactly one succeeded file.read action"
    );
    let action = matching_actions[0];
    let action_trace = action
        .react_trace
        .as_ref()
        .expect("D051 file.read action trace");
    assert_eq!(action_trace.run_id.as_deref(), Some(operation_id));
    assert_eq!(action_trace.action_id, action.id);
    assert_eq!(action_trace.tool_id, "file.read");
    assert_eq!(action_trace.tool_name, "file.read");

    let observation_id = action_trace
        .observation_id
        .as_deref()
        .expect("D051 file.read observation identity");
    let matching_observations = artifacts
        .run
        .observations
        .iter()
        .filter(|observation| observation.id == observation_id)
        .collect::<Vec<_>>();
    assert_eq!(
        matching_observations.len(),
        1,
        "the AgentRun action must bind exactly one canonical observation"
    );
    let observation = matching_observations[0];
    assert_eq!(observation.action_id.as_deref(), Some(action.id.as_str()));

    let output_receipt = action_trace
        .output_receipt
        .as_ref()
        .expect("D051 runtime persisted the bound observation-body receipt");
    let output_receipt_value =
        serde_json::to_value(output_receipt).expect("serialize D051 BoundContentReceipt");
    assert_eq!(output_receipt.version(), 2);
    assert_eq!(
        output_receipt.provenance(),
        openlife_core::agent::ContentReceiptProvenance::ObservedToolAdapterBody
    );
    assert_eq!(output_receipt.byte_count(), OBSERVATION_BODY.len());
    assert_eq!(output_receipt_value["runId"], operation_id);
    assert_eq!(output_receipt_value["actionId"], action.id);
    assert_eq!(output_receipt_value["observationId"], observation.id);
    assert_eq!(
        action
            .output
            .as_ref()
            .and_then(|output| output.get("receiptId")),
        output_receipt_value.get("receiptId"),
        "AgentRun's minimized output ref must identify its exact BoundContentReceipt"
    );

    let matching_queue_actions = artifacts
        .actions
        .iter()
        .filter(|queued| {
            queued.session_id == operation_id
                && queued.action.action_type == "file.read"
                && queued
                    .observation_metadata
                    .as_ref()
                    .and_then(|metadata| metadata.get("executorActionId"))
                    .and_then(serde_json::Value::as_str)
                    == Some(action.id.as_str())
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matching_queue_actions.len(),
        1,
        "ActionQueue must project exactly one row for the canonical AgentRun action"
    );
    let queue_action = matching_queue_actions[0];
    assert_eq!(
        queue_action.status,
        openlife_core::agent::main_chat_agent_v1::ExecutionQueueStatus::Completed
    );
    let queue_metadata = queue_action
        .observation_metadata
        .as_ref()
        .expect("D051 completed queue observation metadata");
    assert_eq!(queue_metadata["actionId"], queue_action.id);
    assert_eq!(queue_metadata["executorActionId"], action.id);
    assert_eq!(queue_metadata["observationId"], observation.id);
    assert_eq!(
        queue_metadata["replayExecutionEnvelope"]["queueActionId"],
        queue_action.id
    );
    assert_eq!(
        queue_metadata["replayExecutionEnvelope"]["executorActionId"],
        action.id
    );
    assert_eq!(
        queue_metadata["replayExecutionEnvelope"]["runId"],
        operation_id
    );
    assert_eq!(
        queue_metadata["replayExecutionEnvelope"]["taskSessionId"],
        operation_id
    );
    assert_eq!(
        queue_metadata["replayExecutionEnvelope"]["manifestId"],
        "file.read"
    );
    let replay_authority = queue_action
        .replay_authority
        .as_ref()
        .expect("ActionQueueStore must reload authenticated canonical replay authority");
    let replay_envelope = &queue_metadata["replayExecutionEnvelope"];
    let governed_input = queue_metadata
        .get("governedInput")
        .expect("D051 exact governed input");
    let (input_length_bytes, input_hash) =
        openlife_core::agent::metadata_safe::metadata_safe_value_digest(governed_input);
    assert!(!replay_authority.store_id().trim().is_empty());
    assert_eq!(replay_authority.action_id(), queue_action.id);
    assert_eq!(replay_authority.task_session_id(), operation_id);
    assert_eq!(replay_authority.run_id(), operation_id);
    assert_eq!(
        replay_authority.queue_action_type(),
        queue_action.action.action_type
    );
    assert_eq!(replay_authority.executor_action_id(), action.id);
    assert_eq!(replay_authority.executor_action_type(), action.action_type);
    assert_eq!(replay_authority.requested_target(), "file.read");
    assert_eq!(replay_authority.resolved_target(), "file.read");
    assert_eq!(replay_authority.manifest_id(), "file.read");
    assert_eq!(replay_authority.manifest_name(), "file.read");
    assert_eq!(replay_authority.manifest_source(), "builtin");
    assert_eq!(
        replay_authority.manifest_contract_digest(),
        replay_envelope["manifestContractDigest"]
            .as_str()
            .expect("D051 manifest contract digest")
    );
    assert_eq!(replay_authority.input_hash(), input_hash);
    assert_eq!(
        replay_authority.input_length_bytes(),
        input_length_bytes as u64
    );
    assert_eq!(replay_envelope["inputHash"], input_hash);
    assert_eq!(replay_envelope["inputLengthBytes"], input_length_bytes);
    let tool_receipt_value = queue_metadata
        .get("toolExecutionReceipt")
        .cloned()
        .expect("ActionQueue must persist the descriptive ToolExecutionReceipt fact");
    let tool_receipt_fact: ToolExecutionReceipt =
        serde_json::from_value(tool_receipt_value.clone())
            .expect("decode the persisted D051 ToolExecutionReceipt fact");
    assert_eq!(
        tool_receipt_fact.source_run_id.as_deref(),
        Some(operation_id)
    );
    assert_eq!(tool_receipt_fact.manifest_id.as_deref(), Some("file.read"));
    assert_eq!(tool_receipt_value["manifestId"], "file.read");
    assert_eq!(replay_authority.receipt_id(), tool_receipt_fact.receipt_id);
    assert_eq!(
        replay_authority.receipt_request_digest(),
        tool_receipt_fact.request_digest
    );
    assert_eq!(
        replay_authority.action_effect(),
        tool_receipt_fact.action_effect
    );
    assert_eq!(
        replay_authority.idempotency_contract(),
        tool_receipt_fact.idempotency_contract
    );
    assert_eq!(
        replay_authority.dispatch_kind(),
        tool_receipt_fact.dispatch_kind
    );
    assert_eq!(
        replay_authority.dispatch_attempt_count(),
        tool_receipt_fact.dispatch_attempt_count
    );
    assert_eq!(
        replay_authority.transport_status(),
        tool_receipt_fact.transport_status
    );
    assert_eq!(
        replay_authority.effect_status(),
        tool_receipt_fact.effect_status
    );
    assert_eq!(
        replay_authority.execution_outcome(),
        tool_receipt_fact.execution_outcome
    );
    assert_eq!(
        queue_action.replay_effect_certainty,
        openlife_core::agent::main_chat_agent_v1::ActionReplayEffectCertainty::EffectNotAttempted
    );
    assert_ne!(
        output_receipt_value["receiptId"], tool_receipt_value["receiptId"],
        "BoundContentReceipt and ToolExecutionReceipt are separate receipt domains"
    );

    let mut lifecycle_events = Vec::new();
    for required_event_type in ["tool.dispatch_prepared", "tool.started", "tool.completed"] {
        let event = exact_one(
            artifacts
                .events
                .iter()
                .filter(|event| event.event_type == required_event_type)
                .collect(),
            required_event_type,
        );
        assert_eq!(event.task_session_id, operation_id);
        assert_eq!(event.run_id, operation_id);
        assert_eq!(event.object_type, "tool_execution_receipt");
        assert_eq!(event.object_id, replay_authority.receipt_id());
        assert_eq!(event.payload["receiptId"], replay_authority.receipt_id());
        assert_eq!(event.payload["sourceRunId"], operation_id);
        assert_eq!(
            event.payload["manifestId"], "file.read",
            "D051 credit is only for the exact file.read manifest"
        );
        lifecycle_events.push(event);
    }
    let receipt_event = lifecycle_events[2];
    assert_eq!(receipt_event.object_id, replay_authority.receipt_id());
    assert_eq!(
        receipt_event.payload["receiptId"],
        replay_authority.receipt_id()
    );
    assert_eq!(receipt_event.payload["sourceRunId"], operation_id);
    assert_eq!(receipt_event.payload["manifestId"], "file.read");
    assert_eq!(receipt_event.payload["dispatchKind"], "local");
    assert_eq!(
        receipt_event.payload["transportStatus"],
        "response_observed"
    );
    assert_eq!(receipt_event.payload["executionOutcome"], "succeeded");
    assert_eq!(receipt_event.payload["dispatchObserved"], true);

    for field in [
        "receiptId",
        "sourceRunId",
        "manifestId",
        "requestDigest",
        "actionEffect",
        "idempotencyContract",
        "dispatchKind",
        "dispatchAttemptCount",
        "dispatchObserved",
        "transportStatus",
        "effectStatus",
        "executionOutcome",
        "startedAt",
        "dispatchedAt",
        "responseObservedAt",
        "finishedAt",
    ] {
        assert_eq!(
            receipt_event.payload[field], tool_receipt_value[field],
            "durable terminal event drifted from the exact persisted ToolExecutionReceipt field {field}"
        );
    }

    let matching_transcript = artifacts
        .transcript
        .iter()
        .filter(|entry| {
            entry.kind == ExecutionTranscriptEntryKind::Observation
                && entry.session_id == operation_id
                && entry.metadata["actionId"] == queue_action.id
                && entry.metadata["executorActionId"] == action.id
                && entry.metadata["runId"] == operation_id
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matching_transcript.len(),
        1,
        "Task transcript must project the same queue and AgentRun owners"
    );
    let transcript_observation = matching_transcript[0];
    assert_eq!(transcript_observation.metadata["status"], "completed");
    let action_completed = artifacts
        .events
        .iter()
        .filter(|event| event.event_type == "action.completed")
        .collect::<Vec<_>>();
    let action_completed = exact_one(action_completed, "D051 action completion projection");
    assert_eq!(action_completed.task_session_id, operation_id);
    assert_eq!(action_completed.run_id, operation_id);
    assert_eq!(action_completed.object_type, "action");
    assert_eq!(action_completed.object_id, queue_action.id);
    assert_eq!(action_completed.payload["actionId"], queue_action.id);
    assert!(action_completed.payload["observationIds"]
        .as_array()
        .is_some_and(|ids| ids == &[serde_json::json!(transcript_observation.id)]));
    let observation_created = artifacts
        .events
        .iter()
        .filter(|event| event.event_type == "observation.created")
        .collect::<Vec<_>>();
    let observation_created = exact_one(
        observation_created,
        "D051 transcript observation projection",
    );
    assert_eq!(observation_created.task_session_id, operation_id);
    assert_eq!(observation_created.run_id, operation_id);
    assert_eq!(observation_created.object_type, "observation");
    assert_eq!(observation_created.object_id, transcript_observation.id);
    assert_eq!(observation_created.payload["actionId"], queue_action.id);
    assert_eq!(
        observation_created.payload["observationId"],
        transcript_observation.id
    );

    RuntimeReadOwnerGraph {
        action,
        observation,
        output_receipt,
        canonical_replay_authority: replay_authority,
        queue_action,
        transcript_observation,
    }
}

fn expected_structured_candidate_digest(graph: &RuntimeReadOwnerGraph<'_>) -> String {
    let start = OBSERVATION_BODY
        .find(CANDIDATE_TEXT)
        .expect("D051 exact candidate slice");
    let end = start + CANDIDATE_TEXT.len();
    let observation_ref = format!(
        "agent-run://{}/action/{}/observation/{}",
        graph
            .action
            .react_trace
            .as_ref()
            .and_then(|trace| trace.run_id.as_deref())
            .expect("D051 graph run id"),
        graph.action.id,
        graph.observation.id,
    );
    openlife_core::agent::metadata_safe::metadata_safe_value_digest(&serde_json::json!({
        "schema": "openlife.memory_evidence.candidate.v1",
        "candidateText": CANDIDATE_TEXT,
        "evidence": {
            "observationRef": observation_ref,
            "startByte": start,
            "endByte": end,
            "sha256": sha256(&OBSERVATION_BODY.as_bytes()[start..end]),
        },
        "owner": {
            "runId": graph
                .action
                .react_trace
                .as_ref()
                .and_then(|trace| trace.run_id.as_deref())
                .expect("D051 graph run id"),
            "actionId": graph.action.id.as_str(),
            "observationId": graph.observation.id.as_str(),
            "outputReceiptDigest": graph.output_receipt.public_digest(),
            "toolReceiptId": graph.canonical_replay_authority.receipt_id(),
        }
    }))
    .1
}

fn assert_positive_durable_truth(artifacts: &RuntimeArtifacts, graph: &RuntimeReadOwnerGraph<'_>) {
    let final_event = artifacts.final_event();
    assert_eq!(
        final_event.payload["status"],
        "completed_with_pending_items"
    );
    assert_eq!(
        final_event.payload["memoryEvidenceStatus"],
        "proposal_staged"
    );
    assert_eq!(
        final_event.payload["memoryEvidenceReason"],
        "same_final_provider_evidence_admitted"
    );
    assert_eq!(
        final_event.payload["memoryEvidenceCredit"],
        "captured_local_http_contract"
    );
    assert_eq!(final_event.payload["externalLiveProviderCredit"], false);
    assert_eq!(final_event.payload["proposalCount"], 1);
    assert_eq!(final_event.payload["bodyStored"], false);
    assert_eq!(artifacts.proposals.len(), 1);
    assert_eq!(artifacts.proposals[0].status.to_string(), "pending");
    assert_eq!(
        artifacts.proposals[0].proposal_type,
        openlife_core::agent::ProposalType::MemoryWrite
    );
    assert_eq!(
        artifacts.proposals[0].run_id.as_deref(),
        Some(artifacts.run.id.as_str())
    );
    assert_eq!(
        artifacts.run.generated_proposals,
        vec![artifacts.proposals[0].id.clone()]
    );
    assert_eq!(artifacts.proposals[0].after["content"], CANDIDATE_TEXT);
    assert_eq!(
        artifacts.proposals[0].after["sourceRunId"],
        artifacts.run.id
    );
    assert_eq!(
        artifacts.proposals[0].after["sourceActionId"],
        graph.action.id
    );
    assert_eq!(
        artifacts.proposals[0].after["sourceObservationId"],
        graph.observation.id
    );
    assert_eq!(
        artifacts.proposals[0].after["sourceOutputReceiptDigest"],
        graph.output_receipt.public_digest()
    );
    assert_eq!(
        artifacts.proposals[0].after["sourceToolReceiptId"],
        graph.canonical_replay_authority.receipt_id()
    );
    assert_eq!(
        artifacts.proposals[0].after["candidateDigest"],
        expected_structured_candidate_digest(graph),
        "candidateDigest must bind the exact structured slice and canonical owner graph"
    );
    assert_eq!(
        graph
            .queue_action
            .observation_metadata
            .as_ref()
            .and_then(|metadata| metadata.get("toolExecutionReceipt"))
            .and_then(|receipt| receipt.get("receiptId")),
        Some(&artifacts.proposals[0].after["sourceToolReceiptId"]),
    );
    assert_eq!(
        graph.transcript_observation.metadata["executorActionId"],
        artifacts.proposals[0].after["sourceActionId"]
    );
    assert!(
        !artifacts.serialized_persistence().contains(RAW_SENTINEL),
        "raw observation body leaked into AgentRun/event/final receipt/proposal persistence or the global MCP-audit counterexample scan"
    );
}

#[test]
fn d051_admission_seam_absence_guard_requires_one_definition_and_one_product_caller() {
    let core_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../openlife-core/src/agent/structured_memory_evidence.rs");
    let core = std::fs::read_to_string(&core_path).unwrap_or_else(|error| {
        panic!(
            "D051 production admission seam is missing at {}: {error}",
            core_path.display()
        )
    });
    let kernel = include_str!("main_chat_kernel.rs");
    let stage = kernel
        .split("async fn stage_conditional_observation_memory_review(")
        .nth(1)
        .and_then(|tail| tail.split("async fn create_kernel_write_proposal(").next())
        .expect("D051 conditional review stage");
    assert!(core.contains("pub fn admit_structured_memory_evidence"));
    assert_eq!(
        [core.as_str(), kernel]
            .into_iter()
            .map(|source| source.matches("admit_structured_memory_evidence(").count())
            .sum::<usize>(),
        2,
        "source counting is an absence guard only; runtime owner binding proves behavioral authority"
    );
    assert!(!stage.contains("extract_main_chat_memory_candidates"));
    assert!(!stage.contains(".get(\"preview\")"));
    assert!(!stage.contains("observed_body"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn d051_buffered_runtime_uses_real_http_event_proposal_and_canonical_stores() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let before = canonical_state_digest(&state).await;
    let capture = configure_captured_provider(&state, positive_final_response(), false).await;
    let operation_id = uuid::Uuid::new_v4().to_string();
    let result = run_buffered(&state, &operation_id, "d051-buffered-positive")
        .await
        .expect("D051 buffered runtime");
    let artifacts = load_artifacts(&state, &operation_id).await;
    let owner_graph = assert_real_durable_file_read_owner_graph(&artifacts, &operation_id);
    let _ = artifacts.final_event();
    assert_one_post_observation_provider_call(&capture);
    assert_eq!(result["status"], "completed_with_pending_items");

    assert_positive_durable_truth(&artifacts, &owner_graph);
    assert_eq!(canonical_state_digest(&state).await, before);

    let retry = run_buffered(&state, &operation_id, "d051-buffered-positive")
        .await
        .expect("D051 durable retry recovery");
    assert_eq!(retry["status"], "completed_with_pending_items");
    assert_one_post_observation_provider_call(&capture);
    let after_retry = load_artifacts(&state, &operation_id).await;
    assert_eq!(
        after_retry.proposals.len(),
        1,
        "retry must reuse one Proposal"
    );
    let _ = after_retry.final_event();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn d051_buffered_and_streaming_project_identical_durable_evidence_truth() {
    let buffered_state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let buffered_capture =
        configure_captured_provider(&buffered_state, positive_final_response(), false).await;
    let buffered_operation = uuid::Uuid::new_v4().to_string();
    run_buffered(&buffered_state, &buffered_operation, "d051-buffered-parity")
        .await
        .expect("D051 buffered parity run");

    let streaming_state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let streaming_capture =
        configure_captured_provider(&streaming_state, positive_final_response(), false).await;
    let streaming_operation = uuid::Uuid::new_v4().to_string();
    run_streaming(
        &streaming_state,
        &streaming_operation,
        "d051-streaming-parity",
    )
    .await
    .expect("D051 streaming parity run");

    let buffered = load_artifacts(&buffered_state, &buffered_operation).await;
    let streaming = load_artifacts(&streaming_state, &streaming_operation).await;
    let buffered_graph = assert_real_durable_file_read_owner_graph(&buffered, &buffered_operation);
    let streaming_graph =
        assert_real_durable_file_read_owner_graph(&streaming, &streaming_operation);
    let _ = buffered.final_event();
    let _ = streaming.final_event();
    assert_one_post_observation_provider_call(&buffered_capture);
    assert_one_post_observation_provider_call(&streaming_capture);
    for field in [
        "status",
        "memoryEvidenceStatus",
        "memoryEvidenceReason",
        "memoryEvidenceCredit",
        "externalLiveProviderCredit",
        "proposalCount",
    ] {
        assert_eq!(
            buffered.final_event().payload[field],
            streaming.final_event().payload[field],
            "buffered/streaming field drift: {field}"
        );
    }
    assert_positive_durable_truth(&buffered, &buffered_graph);
    assert_positive_durable_truth(&streaming, &streaming_graph);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn d051_missing_structured_envelope_cannot_fall_back_to_observation_heuristics() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let capture = configure_captured_provider(&state, no_extractor_final_response(), false).await;
    let operation_id = uuid::Uuid::new_v4().to_string();
    let result = run_buffered(&state, &operation_id, "d051-no-extractor")
        .await
        .expect("D051 no-extractor runtime");
    let artifacts = load_artifacts(&state, &operation_id).await;
    let _ = artifacts.final_event();
    assert_one_post_observation_provider_call(&capture);
    assert!(artifacts.proposals.is_empty());
    assert_eq!(result["status"], "completed_with_partial_evidence");
    assert_eq!(
        artifacts.final_event().payload["memoryEvidenceStatus"],
        "unavailable"
    );
    assert_eq!(
        artifacts.final_event().payload["memoryEvidenceReason"],
        "structured_extractor_unavailable"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn d051_concurrent_same_operation_has_one_provider_execution_and_one_proposal() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let before = canonical_state_digest(&state).await;
    let capture = configure_captured_provider(&state, positive_final_response(), false).await;
    let operation_id = uuid::Uuid::new_v4().to_string();
    let first = run_buffered(&state, &operation_id, "d051-concurrent");
    let second = run_buffered(&state, &operation_id, "d051-concurrent");
    let (first, second) = tokio::time::timeout(Duration::from_secs(10), async move {
        tokio::join!(first, second)
    })
    .await
    .expect("same-operation concurrency must settle without deadlock");
    assert!(
        first.is_ok() || second.is_ok(),
        "one runtime owner must complete: first={first:?} second={second:?}"
    );
    match (&first, &second) {
        (Ok(left), Ok(right)) => {
            assert_eq!(left["run_id"], right["run_id"]);
            assert_eq!(left["status"], right["status"]);
            assert_eq!(left["reply"], right["reply"]);
        }
        (Ok(_), Err(error)) | (Err(error), Ok(_)) => assert!(
            error.contains("execution owner")
                || error.contains("operation_in_progress")
                || error.contains("reconciliation_required"),
            "competing call must receive typed owner/in-progress disposition: {error}"
        ),
        (Err(_), Err(_)) => unreachable!("at least one owner completed"),
    }
    let artifacts = load_artifacts(&state, &operation_id).await;
    let _ = assert_real_durable_file_read_owner_graph(&artifacts, &operation_id);
    let _ = artifacts.final_event();
    assert_one_post_observation_provider_call(&capture);
    assert_eq!(artifacts.proposals.len(), 1);
    assert_eq!(canonical_state_digest(&state).await, before);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn d051_real_cancel_barrier_releases_late_provider_output_without_durable_commit() {
    let state = crate::main_chat_eval_state::build_isolated_main_chat_eval_state();
    let before = canonical_state_digest(&state).await;
    let capture = configure_captured_provider(&state, positive_final_response(), true).await;
    let operation_id = uuid::Uuid::new_v4().to_string();
    let state_for_turn = Arc::clone(&state);
    let operation_for_turn = operation_id.clone();
    let turn = tokio::spawn(async move {
        run_streaming(&state_for_turn, &operation_for_turn, "d051-cancel-barrier").await
    });

    let reached = capture
        .final_request_reached
        .as_ref()
        .expect("D051 final-request barrier")
        .clone();
    tokio::time::timeout(Duration::from_secs(10), async move {
        while !reached.load(Ordering::SeqCst) {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("final provider request reached barrier");

    let cancel = tokio::time::timeout(
        Duration::from_secs(1),
        crate::main_chat_task_controls::cancel_main_chat_agent_task_with_state(
            &operation_id,
            &state,
        ),
    )
    .await;
    let release = capture
        .final_response_release
        .as_ref()
        .expect("D051 final-response release barrier")
        .clone();
    release.store(true, Ordering::SeqCst);
    cancel
        .expect("local cancellation must settle within one second")
        .expect("D051 cancellation request");

    let _ = tokio::time::timeout(Duration::from_secs(10), turn)
        .await
        .expect("cancelled D051 runtime settles")
        .expect("join cancelled D051 runtime");
    assert_one_post_observation_provider_call(&capture);
    let artifacts = load_artifacts(&state, &operation_id).await;
    let final_event = artifacts.final_event();
    assert!(artifacts.proposals.is_empty());
    assert_eq!(canonical_state_digest(&state).await, before);
    assert!(matches!(
        final_event.payload["status"].as_str(),
        Some("cancelled" | "interrupted")
    ));
    assert!(matches!(
        final_event.payload["memoryEvidenceStatus"].as_str(),
        Some("cancelled" | "unavailable")
    ));
    assert!(
        !artifacts.serialized_persistence().contains(RAW_SENTINEL),
        "late provider output or observation body was durably copied after cancellation"
    );
}
