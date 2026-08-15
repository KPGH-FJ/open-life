use crate::agent::ToolStartedTransitionObserver;
use crate::privacy::PrivacyEngine;
#[cfg(test)]
use crate::tool_execution_receipt::ToolActionEffect;
use crate::tool_execution_receipt::ToolExecutionReceiptTracker;
use crate::tool_manifest::{ToolIdempotencyContract, ToolManifest, ToolSource};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cmp::Reverse;
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::{Mutex, Semaphore};

const MCP_HANDSHAKE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const MCP_LIST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
const MCP_CALL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
const MCP_MAX_FRAME_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy)]
struct McpClientLimits {
    handshake_timeout: std::time::Duration,
    list_timeout: std::time::Duration,
    call_timeout: std::time::Duration,
    max_frame_bytes: usize,
}

impl Default for McpClientLimits {
    fn default() -> Self {
        Self {
            handshake_timeout: MCP_HANDSHAKE_TIMEOUT,
            list_timeout: MCP_LIST_TIMEOUT,
            call_timeout: MCP_CALL_TIMEOUT,
            max_frame_bytes: MCP_MAX_FRAME_BYTES,
        }
    }
}

/// MCP Tool definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub parameters: Value, // JSON Schema object
}

/// JSON-RPC request for MCP
#[derive(Debug, Clone, Serialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: u64,
    method: String,
    params: Option<Value>,
}

/// JSON-RPC response from MCP
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: u64,
    #[serde(default)]
    result: Option<Value>,
    #[serde(default)]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct JsonRpcError {
    code: i32,
    message: String,
    #[serde(default)]
    data: Option<Value>,
}

struct McpSession {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    transport_state: McpTransportState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum McpTransportState {
    Healthy,
    Poisoned,
}

impl McpSession {
    fn ensure_healthy(&self) -> Result<()> {
        if self.transport_state == McpTransportState::Poisoned {
            anyhow::bail!(
                "MCP transport is unavailable after an incomplete or invalid prior request"
            );
        }
        Ok(())
    }

    fn poison(&mut self) {
        self.transport_state = McpTransportState::Poisoned;
        // A line-oriented stdio transport cannot safely recover after a request
        // future is dropped: a late response would otherwise be consumed by the
        // next request. Killing the owned subprocess makes that uncertainty
        // explicit and prevents the session from being reused.
        let _ = self.child.start_kill();
    }
}

/// Cancellation-safety boundary for one stdio write/read exchange.
///
/// The guard is armed before any request bytes are written. If the enclosing
/// future is timed out or dropped, `Drop` poisons and terminates the transport;
/// only a completely parsed, id-matched JSON-RPC response disarms it.
struct McpInFlightRequest<'a> {
    session: &'a mut McpSession,
    completed: bool,
    receipt_tracker: Option<ToolExecutionReceiptTracker>,
}

impl<'a> McpInFlightRequest<'a> {
    fn begin(
        session: &'a mut McpSession,
        receipt_tracker: Option<ToolExecutionReceiptTracker>,
    ) -> Result<Self> {
        session.ensure_healthy()?;
        Ok(Self {
            session,
            completed: false,
            receipt_tracker,
        })
    }

    fn mark_response_observed(&self) {
        if let Some(tracker) = &self.receipt_tracker {
            tracker.mark_response_observed();
        }
    }

    fn complete(mut self) {
        self.completed = true;
    }
}

impl Drop for McpInFlightRequest<'_> {
    fn drop(&mut self) {
        if !self.completed {
            if let Some(tracker) = &self.receipt_tracker {
                // Cancellation before the line delimiter is accepted is
                // definitely pre-dispatch. Once the delimiter crosses the
                // pipe boundary, the tracker is already dispatched and this
                // drop must preserve remote uncertainty.
                if tracker.snapshot().transport_status
                    != crate::tool_execution_receipt::ToolTransportStatus::NotAttempted
                {
                    tracker.mark_local_aborted();
                }
                tracker.finish();
            }
            self.session.poison();
        }
    }
}

/// Write one line-delimited JSON-RPC request and record the first point at
/// which the peer can parse and execute it. Once the delimiter has been
/// accepted by the pipe, a later flush failure or cancellation cannot prove
/// that the remote process did not act, so the receipt must already be in the
/// dispatched state before awaiting `flush`.
async fn write_json_rpc_request_frame<W>(
    writer: &mut W,
    request: &JsonRpcRequest,
    max_frame_bytes: usize,
    receipt_tracker: Option<&ToolExecutionReceiptTracker>,
) -> Result<()>
where
    W: AsyncWrite + Unpin,
{
    let json = serde_json::to_string(request)?;
    if json.len() > max_frame_bytes {
        anyhow::bail!("MCP request frame exceeds {} bytes", max_frame_bytes);
    }
    writer.write_all(json.as_bytes()).await?;
    writer.write_all(b"\n").await?;
    if let Some(tracker) = receipt_tracker {
        tracker.mark_mcp_dispatched();
    }
    writer.flush().await?;
    Ok(())
}

/// MCP client using bounded asynchronous stdio transport.
#[derive(Clone)]
pub struct McpClient {
    session: Arc<Mutex<McpSession>>,
    request_id: Arc<AtomicU64>,
    concurrency: Arc<Semaphore>,
    limits: McpClientLimits,
    pub command: String,
    pub args: Vec<String>,
}

impl McpClient {
    /// Start an MCP server subprocess and create a client
    pub async fn new(command: &str, args: &[&str], env: &HashMap<String, String>) -> Result<Self> {
        Self::new_with_limits(command, args, env, McpClientLimits::default()).await
    }

    async fn new_with_limits(
        command: &str,
        args: &[&str],
        env: &HashMap<String, String>,
        limits: McpClientLimits,
    ) -> Result<Self> {
        let mut cmd = Command::new(command);
        cmd.args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true);
        for (k, v) in env {
            cmd.env(k, v);
        }
        let mut child = cmd
            .spawn()
            .with_context(|| format!("failed to spawn MCP server: {}", command))?;
        let stdin = child.stdin.take().context("failed to get MCP stdin")?;
        let stdout = child.stdout.take().context("failed to get MCP stdout")?;
        let client = Self {
            session: Arc::new(Mutex::new(McpSession {
                child,
                stdin,
                stdout: BufReader::new(stdout),
                transport_state: McpTransportState::Healthy,
            })),
            request_id: Arc::new(AtomicU64::new(1)),
            concurrency: Arc::new(Semaphore::new(1)),
            limits,
            command: command.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
        };
        client
            .request_with_timeout(
                0,
                "initialize",
                Some(serde_json::json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {},
                    "clientInfo": { "name": "openlife", "version": "0.1.0" }
                })),
                limits.handshake_timeout,
                None,
                None,
            )
            .await
            .context("MCP initialize handshake failed")?;
        client.send_initialized_notification().await?;
        Ok(client)
    }

    fn next_id(&self) -> u64 {
        self.request_id.fetch_add(1, Ordering::Relaxed)
    }

    async fn send_initialized_notification(&self) -> Result<()> {
        let _permit = self
            .concurrency
            .acquire()
            .await
            .context("MCP concurrency semaphore closed")?;
        let mut session = self.session.lock().await;
        let in_flight = McpInFlightRequest::begin(&mut session, None)?;
        let notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        });
        let frame = serde_json::to_vec(&notification)?;
        if frame.len() > self.limits.max_frame_bytes {
            anyhow::bail!("MCP notification frame exceeds limit");
        }
        in_flight.session.stdin.write_all(&frame).await?;
        in_flight.session.stdin.write_all(b"\n").await?;
        in_flight.session.stdin.flush().await?;
        in_flight.complete();
        Ok(())
    }

    async fn request_with_timeout(
        &self,
        id: u64,
        method: &str,
        params: Option<Value>,
        timeout: std::time::Duration,
        receipt_tracker: Option<ToolExecutionReceiptTracker>,
        started_observer: Option<&dyn ToolStartedTransitionObserver>,
    ) -> Result<JsonRpcResponse> {
        let tracker_after_timeout = receipt_tracker.clone();
        let response = tokio::time::timeout(timeout, async move {
            let _permit = self
                .concurrency
                .acquire()
                .await
                .context("MCP concurrency semaphore closed")?;
            let mut session = self.session.lock().await;
            let in_flight = McpInFlightRequest::begin(&mut session, receipt_tracker)?;
            let request = JsonRpcRequest {
                jsonrpc: "2.0".into(),
                id,
                method: method.into(),
                params,
            };
            let dispatch_tracker = in_flight.receipt_tracker.clone();
            write_json_rpc_request_frame(
                &mut in_flight.session.stdin,
                &request,
                self.limits.max_frame_bytes,
                dispatch_tracker.as_ref(),
            )
            .await?;
            if let (Some(observer), Some(tracker)) =
                (started_observer, in_flight.receipt_tracker.as_ref())
            {
                observer.after_dispatch(&tracker.snapshot()).await?;
            }
            let frame =
                read_frame_limited(&mut in_flight.session.stdout, self.limits.max_frame_bytes)
                    .await?;
            let response: JsonRpcResponse = serde_json::from_slice(&frame)
                .with_context(|| "invalid bounded MCP JSON response")?;
            if response.id != id {
                anyhow::bail!(
                    "MCP response id mismatch: expected {}, received {}",
                    id,
                    response.id
                );
            }
            if response.jsonrpc != "2.0" {
                anyhow::bail!("MCP response jsonrpc version mismatch");
            }
            // A response is observed only after bounded framing, JSON parsing,
            // protocol version validation, and exact request-id matching.
            in_flight.mark_response_observed();
            in_flight.complete();
            Ok(response)
        })
        .await;
        match response {
            Ok(response) => response,
            Err(_) => {
                if let Some(tracker) = tracker_after_timeout {
                    if tracker.snapshot().transport_status
                        != crate::tool_execution_receipt::ToolTransportStatus::NotAttempted
                    {
                        tracker.mark_local_aborted();
                    }
                    tracker.finish();
                }
                Err(anyhow::anyhow!("MCP request '{}' timed out", method))
            }
        }
    }

    /// List available tools from the MCP server
    pub async fn list_tools(&self) -> Result<Vec<Tool>> {
        let id = self.next_id();
        let resp = self
            .request_with_timeout(id, "tools/list", None, self.limits.list_timeout, None, None)
            .await?;

        if let Some(err) = resp.error {
            return Err(anyhow::anyhow!("MCP error {}: {}", err.code, err.message));
        }

        let tools: Vec<Tool> = resp
            .result
            .and_then(|r| r.get("tools").cloned())
            .and_then(|t| serde_json::from_value(t).ok())
            .unwrap_or_default();

        Ok(tools)
    }

    #[cfg(test)]
    async fn call_tool(&self, name: &str, arguments: Value) -> Result<String> {
        let (_, request_digest) =
            crate::agent::metadata_safe::metadata_safe_value_digest(&serde_json::json!({
                "method": "tools/call",
                "name": name,
                "arguments": &arguments,
            }));
        let tracker = ToolExecutionReceiptTracker::new(
            None,
            None,
            request_digest,
            ToolActionEffect::Unknown,
            ToolIdempotencyContract::Unspecified,
        );
        self.call_tool_with_receipt_tracker(name, arguments, tracker, None)
            .await
    }

    pub(crate) async fn call_tool_with_receipt_tracker(
        &self,
        name: &str,
        arguments: Value,
        receipt_tracker: ToolExecutionReceiptTracker,
        started_observer: Option<&dyn ToolStartedTransitionObserver>,
    ) -> Result<String> {
        let id = self.next_id();
        let response = self
            .request_with_timeout(
                id,
                "tools/call",
                Some(serde_json::json!({
                "name": name,
                "arguments": arguments
                })),
                self.limits.call_timeout,
                Some(receipt_tracker.clone()),
                started_observer,
            )
            .await;
        let resp = match response {
            Ok(response) => response,
            Err(error) => {
                receipt_tracker.mark_effect_unknown_if_dispatched();
                receipt_tracker.finish();
                return Err(error);
            }
        };

        if let Some(err) = resp.error {
            receipt_tracker.mark_execution_failed();
            receipt_tracker.mark_effect_unknown_if_dispatched();
            receipt_tracker.finish();
            return Err(anyhow::anyhow!("MCP error {}: {}", err.code, err.message));
        }

        // Extract content from result
        let content = resp
            .result
            .and_then(|r| r.get("content").cloned())
            .and_then(|c| c.as_array().cloned())
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| item.get("text").and_then(|t| t.as_str()))
                    .collect::<Vec<_>>()
                    .join("")
            })
            .unwrap_or_default();

        receipt_tracker.mark_execution_succeeded();
        receipt_tracker.mark_effect_confirmed();
        receipt_tracker.finish();
        Ok(content)
    }
}

async fn read_frame_limited(
    reader: &mut BufReader<ChildStdout>,
    max_bytes: usize,
) -> Result<Vec<u8>> {
    let mut frame = Vec::new();
    loop {
        let available = reader.fill_buf().await?;
        if available.is_empty() {
            anyhow::bail!("MCP server closed stdout before a complete response frame");
        }
        let newline = available.iter().position(|byte| *byte == b'\n');
        let take = newline.map_or(available.len(), |position| position + 1);
        if frame.len().saturating_add(take) > max_bytes {
            reader.consume(take);
            anyhow::bail!("MCP response frame exceeds {} bytes", max_bytes);
        }
        frame.extend_from_slice(&available[..take]);
        reader.consume(take);
        if newline.is_some() {
            while matches!(frame.last(), Some(b'\n' | b'\r')) {
                frame.pop();
            }
            if frame.is_empty() {
                anyhow::bail!("MCP server returned an empty response frame");
            }
            return Ok(frame);
        }
    }
}

pub type BuiltinFn = Box<dyn Fn(Value) -> Result<String> + Send + Sync>;
type SharedBuiltinFn = Arc<dyn Fn(Value) -> Result<String> + Send + Sync>;

const EXECUTOR_INSTANCE_RETIRED: u64 = 1_u64 << 63;
const EXECUTOR_INSTANCE_INFLIGHT_MASK: u64 = EXECUTOR_INSTANCE_RETIRED - 1;

/// Cross-snapshot dispatch authority for one concrete callback/client
/// instance. Registry snapshots share this gate, so replacing or unregistering
/// the live instance can retire stale snapshots without holding a registry
/// guard across adapter I/O.
#[derive(Debug)]
struct ExecutorInstanceGate {
    instance_id: String,
    state: AtomicU64,
}

impl ExecutorInstanceGate {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            instance_id: uuid::Uuid::new_v4().to_string(),
            state: AtomicU64::new(0),
        })
    }

    fn instance_id(&self) -> &str {
        &self.instance_id
    }

    fn retire(&self) {
        self.state
            .fetch_or(EXECUTOR_INSTANCE_RETIRED, Ordering::AcqRel);
    }

    fn is_retired(&self) -> bool {
        self.state.load(Ordering::Acquire) & EXECUTOR_INSTANCE_RETIRED != 0
    }

    fn try_acquire(self: &Arc<Self>) -> Result<ExecutorInstanceLease> {
        let mut current = self.state.load(Ordering::Acquire);
        loop {
            if current & EXECUTOR_INSTANCE_RETIRED != 0 {
                anyhow::bail!("mcp_registry_dispatch_instance_retired");
            }
            let inflight = current & EXECUTOR_INSTANCE_INFLIGHT_MASK;
            if inflight == EXECUTOR_INSTANCE_INFLIGHT_MASK {
                anyhow::bail!("mcp_registry_dispatch_instance_inflight_exhausted");
            }
            match self.state.compare_exchange_weak(
                current,
                current + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    return Ok(ExecutorInstanceLease {
                        gate: Arc::clone(self),
                    });
                }
                Err(observed) => current = observed,
            }
        }
    }
}

#[derive(Debug)]
struct ExecutorInstanceLease {
    gate: Arc<ExecutorInstanceGate>,
}

impl Drop for ExecutorInstanceLease {
    fn drop(&mut self) {
        let previous = self.gate.state.fetch_sub(1, Ordering::AcqRel);
        debug_assert!(previous & EXECUTOR_INSTANCE_INFLIGHT_MASK > 0);
    }
}

/// Registry for multiple MCP clients and built-in tools
#[derive(Clone)]
pub struct McpRegistry {
    clients: HashMap<String, McpClient>,
    server_tools: HashMap<String, Vec<Tool>>,
    server_manifests: HashMap<String, Vec<ToolManifest>>,
    tools_cache: Vec<Tool>,
    privacy_engine: PrivacyEngine,
    builtins: HashMap<String, SharedBuiltinFn>,
    builtin_manifests: Vec<ToolManifest>,
    registry_generation: u64,
    execution_instance_gates: HashMap<String, Arc<ExecutorInstanceGate>>,
}

/// Opaque execution-instance binding captured from one registry snapshot.
/// A manifest contract may remain byte-for-byte identical while its callback
/// or MCP client is replaced; this binding makes that replacement observable
/// to policy/receipt observers. The shared ExecutorInstanceGate remains the
/// concrete adapter-edge linearization authority after observer awaits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpRegistryDispatchBinding {
    registry_generation: u64,
    executor_instance_id: String,
}

impl McpRegistryDispatchBinding {
    pub fn registry_generation(&self) -> u64 {
        self.registry_generation
    }

    pub fn executor_instance_id(&self) -> &str {
        &self.executor_instance_id
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct McpServerInfo {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub tool_count: usize,
}

/// Fully probed registration candidate that has not yet mutated registry
/// state. Preparing it may spawn a process and await MCP I/O; committing it is
/// synchronous and is therefore safe to perform while holding a shared
/// registry mutex.
pub struct PreparedMcpRegistration {
    name: String,
    client: McpClient,
    tools: Vec<Tool>,
    manifests: Vec<ToolManifest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpPrivacyFinding {
    pub path: String,
    pub privacy_type: String,
    pub matched: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpArgumentInspection {
    pub permission_level: String,
    pub pii_found: bool,
    pub findings: Vec<McpPrivacyFinding>,
    pub sanitized_arguments: Value,
    pub requires_confirmation: bool,
}

fn validate_discovered_tools(tools: &[Tool]) -> Result<()> {
    for tool in tools {
        if tool.name.trim().is_empty() {
            anyhow::bail!("MCP server returned a tool without a name");
        }
        if !tool.parameters.is_object() {
            anyhow::bail!(
                "MCP tool '{}' returned an invalid parameter schema",
                tool.name
            );
        }
    }
    Ok(())
}

fn validate_typed_mcp_manifests(
    server_name: &str,
    discovered: &[Tool],
    manifests: &[ToolManifest],
) -> Result<()> {
    if discovered.len() != manifests.len() {
        anyhow::bail!(
            "MCP server '{}' manifest count does not match discovered tools",
            server_name
        );
    }
    let mut seen = std::collections::HashSet::new();
    for manifest in manifests {
        if !seen.insert(manifest.name.as_str()) {
            anyhow::bail!("duplicate typed MCP manifest '{}': rejected", manifest.name);
        }
        if !matches!(
            &manifest.source,
            ToolSource::Mcp { server_name: source_server } if source_server == server_name
        ) {
            anyhow::bail!(
                "typed MCP manifest '{}' has the wrong server authority",
                manifest.name
            );
        }
        if manifest.id.trim().is_empty()
            || manifest.name.trim().is_empty()
            || manifest.permission_level.trim().is_empty()
            || manifest.risk_level.trim().is_empty()
            || manifest.action_type.trim().is_empty()
            || manifest.capabilities.is_empty()
            || manifest.idempotency_contract == ToolIdempotencyContract::Unspecified
            || !manifest.enabled
            || manifest.declarative_only
            || manifest
                .tags
                .iter()
                .any(|tag| tag.starts_with("migration:name_inferred_contract"))
        {
            anyhow::bail!(
                "typed MCP manifest '{}' is incomplete or non-executable",
                manifest.name
            );
        }
        let tool = discovered
            .iter()
            .find(|tool| tool.name == manifest.name)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "typed MCP manifest '{}' was not discovered from server '{}'",
                    manifest.name,
                    server_name
                )
            })?;
        if tool.parameters != manifest.parameters {
            anyhow::bail!(
                "typed MCP manifest '{}' parameter schema differs from discovery",
                manifest.name
            );
        }
        crate::agent::tool_gateway::validate_manifest_execution_contract(manifest).map_err(
            |error| {
                anyhow::anyhow!(
                    "typed MCP manifest '{}' execution contract is invalid: {error}",
                    manifest.name
                )
            },
        )?;
    }
    Ok(())
}

fn registry_execution_instance_key(manifest: &ToolManifest) -> String {
    format!("{}\0{}\0{}", manifest.source, manifest.id, manifest.name)
}

fn memory_archive_owner_parameters() -> Value {
    let owner = serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "ownerKind": {
                "type": "string",
                "enum": ["memory_lifecycle", "memory_record", "knowledge_note"]
            },
            "ownerId": { "type": "string", "minLength": 1, "maxLength": 256 }
        },
        "required": ["ownerKind", "ownerId"]
    });
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "owner": owner.clone(),
            "owners": {
                "type": "array",
                "items": owner,
                "minItems": 1,
                "maxItems": 200,
                "uniqueItems": true
            },
            "reason": { "type": "string", "maxLength": 512 }
        },
        "oneOf": [
            { "required": ["owner"], "not": { "required": ["owners"] } },
            { "required": ["owners"], "not": { "required": ["owner"] } }
        ]
    })
}

fn memory_write_parameters() -> Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "content": { "type": "string", "minLength": 1, "maxLength": 65536 },
            "scope": {
                "type": "string",
                "enum": ["global", "workspace", "conversation", "project"]
            },
            "category": {
                "type": "string",
                "enum": ["fact", "workflow", "preference", "boundary"]
            },
            "candidateKind": {
                "type": "string",
                "enum": [
                    "episodic_life_event",
                    "semantic_user_fact",
                    "procedural_rule",
                    "preference",
                    "identity_or_role"
                ]
            }
        },
        "required": ["content"]
    })
}

impl McpRegistry {
    pub fn new() -> Self {
        let mut reg = Self {
            clients: HashMap::new(),
            server_tools: HashMap::new(),
            server_manifests: HashMap::new(),
            tools_cache: Vec::new(),
            privacy_engine: PrivacyEngine::new(),
            builtins: HashMap::new(),
            builtin_manifests: Vec::new(),
            registry_generation: 0,
            execution_instance_gates: HashMap::new(),
        };
        reg.register_default_builtins();
        reg
    }

    /// Build the registry used by the default product release.
    ///
    /// Core governed capabilities remain available, including Web, bounded
    /// file reads, tasks, and Memory proposals. Test utilities and generic
    /// extension dispatch stay out of the release product path.
    pub fn new_release_product() -> Self {
        let mut registry = Self::new();
        registry.remove_builtin_by_name("builtin_echo");
        registry.remove_builtin_by_name("mcp.call_tool");
        registry
    }

    fn remove_builtin_by_name(&mut self, name: &str) {
        let removed = self
            .builtin_manifests
            .iter()
            .filter(|manifest| manifest.name == name)
            .cloned()
            .collect::<Vec<_>>();
        for manifest in &removed {
            if let Some(gate) = self
                .execution_instance_gates
                .remove(&registry_execution_instance_key(manifest))
            {
                gate.retire();
            }
            self.builtins.remove(&manifest.name);
        }
        self.builtin_manifests
            .retain(|manifest| manifest.name != name);
        if !removed.is_empty() {
            self.registry_generation = self.registry_generation.saturating_add(1);
        }
    }

    pub(crate) fn register_default_builtins(&mut self) {
        // Built-in: echo (test utility)
        let echo_manifest = ToolManifest {
            id: "builtin_echo".into(),
            name: "builtin_echo".into(),
            description: "返回传入的文本内容，用于测试工具链路。".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "要回显的文本" }
                },
                "required": ["text"]
            }),
            permission_level: "low".into(),
            risk_level: "low".into(),
            version: "1.0.0".into(),
            source: ToolSource::BuiltIn,
            capabilities: vec!["read".into()],
            requires_confirmation: false,
            enabled: true,
            declarative_only: false,
            action_type: "read".into(),
            idempotency_contract: ToolIdempotencyContract::Idempotent,
            tags: vec!["test".into(), "utility".into()],
        };
        self.register_builtin(
            echo_manifest,
            Box::new(|args| {
                let text = args.get("text").and_then(|v| v.as_str()).unwrap_or("");
                Ok(text.to_string())
            }),
        );

        // Core OS Tools: Read-only
        self.register_core_os_tool(
            "life_model.read",
            "读取当前 LifeModel 的四维数据（Identity/Goals/Capabilities/State）",
            "low",
            vec!["read".into(), "lifemodel".into()],
            "read",
            ToolIdempotencyContract::Idempotent,
        );

        self.register_core_os_tool(
            "tool.list_available",
            "列出所有已注册且可用的工具",
            "low",
            vec!["read".into()],
            "read",
            ToolIdempotencyContract::Idempotent,
        );

        self.register_core_os_tool(
            "goal.read",
            "读取 LifeModel 长期目标与 StateStore 当前每日任务（明确标注各自权威）",
            "low",
            vec!["read".into(), "lifemodel".into(), "state_store".into()],
            "read",
            ToolIdempotencyContract::Idempotent,
        );

        self.register_core_os_tool(
            "state.read",
            "读取 StateStore 当前任务与状态观察；不从 LifeModel 兼容字段重建事实",
            "low",
            vec!["read".into(), "state_store".into()],
            "read",
            ToolIdempotencyContract::Idempotent,
        );

        self.register_core_os_tool(
            "memory.search",
            "搜索向量记忆库，返回相关记忆片段",
            "low",
            vec!["read".into(), "memory".into()],
            "read",
            ToolIdempotencyContract::Idempotent,
        );

        self.register_core_os_tool(
            "proposal.list",
            "列出当前待处理的 Proposal",
            "low",
            vec!["read".into()],
            "read",
            ToolIdempotencyContract::Idempotent,
        );

        // Permission tools: let the agent inspect and request tool permissions.
        self.register_core_os_tool(
            "permission.check",
            "查询指定工具当前的权限状态（允许/阻断/需确认及原因）",
            "low",
            vec!["read".into()],
            "read",
            ToolIdempotencyContract::Idempotent,
        );

        self.register_core_os_tool(
            "permission.request",
            "为指定工具请求权限（生成 ToolPermission Proposal 供用户审批）",
            "medium",
            vec!["read".into()],
            "read",
            ToolIdempotencyContract::NonIdempotent,
        );

        // snapshot.create is manifest-only until the Version Control executor is configured.
        self.register_declarative_stub(
            "snapshot.create",
            "创建快照（当前能力需要在 Version Control 页面手动执行）",
        );

        // Core OS Tools: Write (Proposal-First)
        self.register_core_os_tool(
            "life_model.propose_patch",
            "提议修改 LifeModel（生成 Proposal，不直接写入）",
            "high",
            vec!["write".into(), "lifemodel".into()],
            "write",
            ToolIdempotencyContract::NonIdempotent,
        );

        self.register_core_os_tool_with_parameters(
            "memory.propose_write",
            "提议写入记忆（生成 Proposal，不直接写入）",
            "medium",
            vec!["write".into(), "memory".into()],
            "write",
            ToolIdempotencyContract::NonIdempotent,
            memory_write_parameters(),
        );

        self.register_core_os_tool_with_parameters(
            "memory.propose_archive",
            "提议归档记忆（生成 Proposal，不直接归档）",
            "medium",
            vec!["write".into(), "memory".into()],
            "write",
            ToolIdempotencyContract::NonIdempotent,
            memory_archive_owner_parameters(),
        );

        // Execution Tools: P1 (file, web)
        self.register_execution_tool(
            "file.read",
            "读取指定路径的文件内容（仅限 safe_paths）",
            "low",
            vec!["read".into(), "filesystem".into()],
            "read",
            ToolIdempotencyContract::Idempotent,
        );

        self.register_builtin(
            ToolManifest {
                id: "document.read".into(),
                name: "document.read".into(),
                description: "读取当前任务已绑定并完成解析的文档资源".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "message_id": { "type": "string", "minLength": 1, "maxLength": 256 },
                        "query": { "type": "string", "minLength": 1, "maxLength": 262144 },
                        "selection_request_id": { "type": "string", "format": "uuid" },
                        "privacy_decision_id": { "type": "string", "minLength": 1, "maxLength": 256 }
                    },
                    "required": ["message_id", "query", "selection_request_id", "privacy_decision_id"]
                }),
                permission_level: "low".into(),
                risk_level: "low".into(),
                version: "1.0.0".into(),
                source: ToolSource::BuiltIn,
                capabilities: vec!["read".into(), "imported_resource".into()],
                requires_confirmation: false,
                enabled: true,
                declarative_only: false,
                action_type: "read".into(),
                idempotency_contract: ToolIdempotencyContract::Idempotent,
                tags: vec!["execution".into(), "document".into()],
            },
            Box::new(|_args| {
                Ok("document.read completed with the current governed resource executor".into())
            }),
        );

        self.register_execution_tool(
            "file.write_proposal",
            "提议写入文件（生成 ExternalWriteAction Proposal，不直接写入）",
            "high",
            vec!["write".into(), "filesystem".into()],
            "write",
            ToolIdempotencyContract::NonIdempotent,
        );

        self.register_execution_tool(
            "web.fetch",
            "获取指定 URL 的内容",
            "medium",
            vec!["network".into()],
            "network",
            ToolIdempotencyContract::Idempotent,
        );

        self.register_builtin(
            ToolManifest {
                id: "web.search".into(),
                name: "web.search".into(),
                description: "联网搜索网页，输入 query 返回搜索结果摘要".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Search query" },
                        "max_results": { "type": "integer", "description": "Maximum number of results, default 5" }
                    },
                    "required": ["query"]
                }),
                permission_level: "medium".into(),
                risk_level: "medium".into(),
                version: "1.0.0".into(),
                source: ToolSource::BuiltIn,
                capabilities: vec!["network".into(), "read".into()],
                requires_confirmation: false,
                enabled: true,
                declarative_only: false,
                action_type: "read".into(),
                idempotency_contract: ToolIdempotencyContract::Idempotent,
                tags: vec!["execution".into(), "web".into()],
            },
            Box::new(|_args| Ok("web.search executed".to_string())),
        );

        self.register_builtin(
            ToolManifest {
                id: "mcp.call_tool".into(),
                name: "mcp.call_tool".into(),
                description: "调用已注册的 MCP 工具（通用入口）".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "tool_name": { "type": "string", "description": "目标 MCP 工具名" },
                        "server": { "type": "string", "description": "可选：MCP server 名称" },
                        "arguments": { "type": "object" }
                    },
                    "required": ["tool_name", "arguments"]
                }),
                permission_level: "medium".into(),
                risk_level: "medium".into(),
                version: "1.0.0".into(),
                source: ToolSource::BuiltIn,
                capabilities: vec!["network".into(), "external_side_effect".into()],
                requires_confirmation: false,
                enabled: true,
                declarative_only: false,
                action_type: "external_side_effect".into(),
                idempotency_contract: ToolIdempotencyContract::NonIdempotent,
                tags: vec!["execution".into(), "mcp_wrapper".into()],
            },
            Box::new(|_args| Ok("mcp.call_tool executed".to_string())),
        );

        // Execution Tools: P1 calendar.read (reads ICS files).
        // Note: "filesystem" capability intentionally omitted — the handler validates
        // against calendar_ics_paths + safe_paths using the "source" arg, not "path".
        self.register_execution_tool(
            "calendar.read",
            "读取日历事件（从配置的 ICS 文件中解析 VEVENT）",
            "low",
            vec!["read".into(), "calendar".into()],
            "read",
            ToolIdempotencyContract::Idempotent,
        );

        // Execution Tools: P1 proposal-only governed executors.
        self.register_execution_tool(
            "calendar.propose_event",
            "提议日历事件（仅生成 ScheduledTask Proposal，不直接写入日历）",
            "medium",
            vec!["write".into()],
            "write",
            ToolIdempotencyContract::NonIdempotent,
        );

        // email.read remains provider-gated until IMAP config is available.
        self.register_declarative_stub("email.read", "读取邮件（需要配置 IMAP account 后启用）");

        self.register_execution_tool(
            "email.propose_draft",
            "提议邮件草稿（仅生成 DataExport/email-draft Proposal，不发送邮件）",
            "medium",
            vec!["write".into()],
            "write",
            ToolIdempotencyContract::NonIdempotent,
        );

        // P1 task.create_proposal: creates real local tasks via TaskStore
        self.register_execution_tool(
            "task.create_proposal",
            "创建本地任务/提醒/待办事项（P1：持久化到本地 TaskStore）",
            "medium",
            vec!["write".into()],
            "write",
            ToolIdempotencyContract::NonIdempotent,
        );
    }

    /// Helper to register a Core OS tool with standard metadata.
    pub fn register_core_os_tool(
        &mut self,
        id: &str,
        description: &str,
        risk_level: &str,
        capabilities: Vec<String>,
        action_type: &str,
        idempotency_contract: ToolIdempotencyContract,
    ) {
        self.register_core_os_tool_with_parameters(
            id,
            description,
            risk_level,
            capabilities,
            action_type,
            idempotency_contract,
            serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        );
    }

    // The typed built-in manifest registers each risk and capability field explicitly.
    #[expect(
        clippy::too_many_arguments,
        reason = "owner=backend-platform; expires=2026-10-01; replace positional boundary with a typed request object"
    )]
    fn register_core_os_tool_with_parameters(
        &mut self,
        id: &str,
        description: &str,
        risk_level: &str,
        capabilities: Vec<String>,
        action_type: &str,
        idempotency_contract: ToolIdempotencyContract,
        parameters: Value,
    ) {
        let manifest = ToolManifest {
            id: id.into(),
            name: id.into(),
            description: description.into(),
            parameters,
            permission_level: risk_level.into(),
            risk_level: risk_level.into(),
            version: "1.0.0".into(),
            source: ToolSource::BuiltIn,
            capabilities,
            requires_confirmation: risk_level == "high",
            enabled: true,
            declarative_only: false,
            action_type: action_type.into(),
            idempotency_contract,
            tags: vec!["core_os".into()],
        };
        let id_owned = id.to_string();
        self.register_builtin(
            manifest,
            Box::new(move |_args| {
                Ok(format!(
                    "Core OS tool '{}' completed with the current local capability handler",
                    id_owned
                ))
            }),
        );
    }

    /// Helper to register an Execution tool.
    fn register_execution_tool(
        &mut self,
        id: &str,
        description: &str,
        risk_level: &str,
        capabilities: Vec<String>,
        action_type: &str,
        idempotency_contract: ToolIdempotencyContract,
    ) {
        let manifest = ToolManifest {
            id: id.into(),
            name: id.into(),
            description: description.into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path or URL" }
                }
            }),
            permission_level: risk_level.into(),
            risk_level: risk_level.into(),
            version: "1.0.0".into(),
            source: ToolSource::BuiltIn,
            capabilities,
            requires_confirmation: risk_level == "high",
            enabled: true,
            declarative_only: false,
            action_type: action_type.into(),
            idempotency_contract,
            tags: vec!["execution".into()],
        };
        let id_owned = id.to_string();
        self.register_builtin(
            manifest,
            Box::new(move |_args| {
                Ok(format!(
                    "Execution tool '{}' completed with the current governed executor",
                    id_owned
                ))
            }),
        );
    }

    /// Helper to register a manifest-only tool that requires provider configuration.
    fn register_declarative_stub(&mut self, id: &str, description: &str) {
        let manifest = ToolManifest {
            id: id.into(),
            name: id.into(),
            description: description.into(),
            parameters: serde_json::json!({"type": "object"}),
            permission_level: "low".into(),
            risk_level: "low".into(),
            version: "1.0.0".into(),
            source: ToolSource::BuiltIn,
            capabilities: vec!["read".into()],
            requires_confirmation: false,
            enabled: true,
            declarative_only: true,
            action_type: "read".into(),
            idempotency_contract: ToolIdempotencyContract::NonIdempotent,
            tags: vec!["execution".into(), "manifest_only".into()],
        };
        self.register_builtin(
            manifest,
            Box::new(move |_args| {
                Ok(
                    "This capability is manifest-only until the required provider is configured."
                        .to_string(),
                )
            }),
        );
    }

    /// Register a built-in tool with its manifest.
    pub fn register_builtin(&mut self, manifest: ToolManifest, func: BuiltinFn) {
        let replaced = self
            .builtin_manifests
            .iter()
            .filter(|existing| existing.id == manifest.id || existing.name == manifest.name)
            .cloned()
            .collect::<Vec<_>>();
        for existing in &replaced {
            if let Some(gate) = self
                .execution_instance_gates
                .remove(&registry_execution_instance_key(existing))
            {
                gate.retire();
            }
            self.builtins.remove(&existing.name);
        }
        self.builtin_manifests
            .retain(|existing| existing.id != manifest.id && existing.name != manifest.name);
        self.registry_generation = self.registry_generation.saturating_add(1);
        self.execution_instance_gates.insert(
            registry_execution_instance_key(&manifest),
            ExecutorInstanceGate::new(),
        );
        self.builtins.insert(manifest.name.clone(), Arc::from(func));
        self.builtin_manifests.push(manifest);
    }

    /// Remove built-in tools by source (e.g., remove all plugin tools).
    pub fn remove_builtins_by_source(&mut self, source_filter: impl Fn(&ToolSource) -> bool) {
        let removed_manifests = self
            .builtin_manifests
            .iter()
            .filter(|manifest| source_filter(&manifest.source))
            .cloned()
            .collect::<Vec<_>>();
        for manifest in &removed_manifests {
            if let Some(gate) = self
                .execution_instance_gates
                .remove(&registry_execution_instance_key(manifest))
            {
                gate.retire();
            }
        }
        for manifest in &removed_manifests {
            self.builtins.remove(&manifest.name);
        }
        self.builtin_manifests.retain(|m| !source_filter(&m.source));
        if !removed_manifests.is_empty() {
            self.registry_generation = self.registry_generation.saturating_add(1);
        }
    }

    /// Register and start an MCP server
    pub async fn register(&mut self, name: &str, command: &str, args: &[&str]) -> Result<()> {
        self.register_with_env(name, command, args, &HashMap::new())
            .await
    }

    /// Register and start an MCP server with environment variables.
    pub async fn register_with_env(
        &mut self,
        _name: &str,
        _command: &str,
        _args: &[&str],
        _env: &HashMap<String, String>,
    ) -> Result<()> {
        anyhow::bail!(
            "MCP registration requires explicit typed manifests; discovered names cannot authorize execution"
        )
    }

    pub async fn register_with_env_and_manifests(
        &mut self,
        name: &str,
        command: &str,
        args: &[&str],
        env: &HashMap<String, String>,
        manifests: Vec<ToolManifest>,
    ) -> Result<()> {
        if self.clients.contains_key(name) {
            anyhow::bail!("MCP server '{}' is already registered", name);
        }
        let prepared = Self::prepare_registration(name, command, args, env, manifests).await?;
        self.commit_prepared_registration(prepared)
    }

    pub async fn prepare_registration(
        name: &str,
        command: &str,
        args: &[&str],
        env: &HashMap<String, String>,
        manifests: Vec<ToolManifest>,
    ) -> Result<PreparedMcpRegistration> {
        if name.trim().is_empty() {
            anyhow::bail!("MCP server name must not be empty");
        }
        if manifests.is_empty() {
            anyhow::bail!("MCP server '{}' has no typed manifest contracts", name);
        }
        let client = McpClient::new(command, args, env).await?;
        let tools = client
            .list_tools()
            .await
            .with_context(|| format!("failed to list tools for MCP server '{}'", name))?;
        validate_discovered_tools(&tools)?;
        validate_typed_mcp_manifests(name, &tools, &manifests)?;
        Ok(PreparedMcpRegistration {
            name: name.to_string(),
            client,
            tools,
            manifests,
        })
    }

    pub fn commit_prepared_registration(
        &mut self,
        prepared: PreparedMcpRegistration,
    ) -> Result<()> {
        if self.clients.contains_key(&prepared.name) {
            anyhow::bail!("MCP server '{}' is already registered", prepared.name);
        }
        self.registry_generation = self.registry_generation.saturating_add(1);
        for manifest in &prepared.manifests {
            self.execution_instance_gates.insert(
                registry_execution_instance_key(manifest),
                ExecutorInstanceGate::new(),
            );
        }
        self.server_tools
            .insert(prepared.name.clone(), prepared.tools);
        self.server_manifests
            .insert(prepared.name.clone(), prepared.manifests);
        self.clients.insert(prepared.name, prepared.client);
        self.rebuild_tools_cache();
        Ok(())
    }

    /// Unregister an MCP server
    pub fn unregister(&mut self, name: &str) -> Result<()> {
        if !self.clients.contains_key(name) {
            return Err(anyhow::anyhow!("server '{}' not found", name));
        }
        if let Some(manifests) = self.server_manifests.get(name) {
            for manifest in manifests {
                let key = registry_execution_instance_key(manifest);
                if let Some(gate) = self.execution_instance_gates.remove(&key) {
                    gate.retire();
                }
            }
        }
        self.clients.remove(name);
        self.server_tools.remove(name);
        self.server_manifests.remove(name);
        self.registry_generation = self.registry_generation.saturating_add(1);
        self.rebuild_tools_cache();
        Ok(())
    }

    fn rebuild_tools_cache(&mut self) {
        self.tools_cache = self
            .server_tools
            .values()
            .flat_map(|tools| tools.iter().cloned())
            .collect();
    }

    /// List registered servers with metadata
    pub fn list_servers(&self) -> Vec<McpServerInfo> {
        self.clients
            .iter()
            .map(|(name, client)| McpServerInfo {
                name: name.clone(),
                command: client.command.clone(),
                args: client.args.clone(),
                tool_count: self.server_tools.get(name).map_or(0, Vec::len),
            })
            .collect()
    }

    /// Get all available tools from all registered servers
    pub fn list_all_tools(&self) -> &[Tool] {
        &self.tools_cache
    }

    /// Return unified manifests for both MCP tools and built-in tools.
    pub fn list_manifests(&self) -> Vec<ToolManifest> {
        let mut out: Vec<ToolManifest> = self
            .builtin_manifests
            .clone()
            .into_iter()
            .map(ToolManifest::normalized)
            .collect();
        for manifests in self.server_manifests.values() {
            for manifest in manifests {
                out.push(manifest.clone().normalized());
            }
        }
        out
    }

    pub fn dispatch_binding(&self, manifest: &ToolManifest) -> Result<McpRegistryDispatchBinding> {
        let exact = self
            .list_manifests()
            .into_iter()
            .filter(|candidate| {
                candidate.id == manifest.id
                    && candidate.name == manifest.name
                    && candidate.source.to_string() == manifest.source.to_string()
                    && candidate.execution_contract_digest() == manifest.execution_contract_digest()
            })
            .collect::<Vec<_>>();
        let [registered] = exact.as_slice() else {
            anyhow::bail!("mcp_registry_dispatch_manifest_identity_not_unique");
        };
        let executor_instance = self
            .execution_instance_gates
            .get(&registry_execution_instance_key(registered))
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("mcp_registry_dispatch_instance_missing"))?;
        if executor_instance.is_retired() {
            anyhow::bail!("mcp_registry_dispatch_instance_retired");
        }
        Ok(McpRegistryDispatchBinding {
            registry_generation: self.registry_generation,
            executor_instance_id: executor_instance.instance_id().to_string(),
        })
    }

    fn acquire_execution_instance(&self, manifest: &ToolManifest) -> Result<ExecutorInstanceLease> {
        self.execution_instance_gates
            .get(&registry_execution_instance_key(manifest))
            .ok_or_else(|| anyhow::anyhow!("mcp_registry_dispatch_instance_missing"))?
            .try_acquire()
    }

    /// Return manifest snapshots already held by the registry without asking
    /// registered MCP servers to refresh their tool lists.
    pub fn list_cached_manifest_snapshots(&self) -> Vec<ToolManifest> {
        let mut out: Vec<ToolManifest> = self
            .builtin_manifests
            .clone()
            .into_iter()
            .map(ToolManifest::normalized)
            .collect();
        out.extend(self.tools_cache.iter().map(|tool| {
            ToolManifest {
                id: format!("mcp:cached_registry_snapshot:{}", tool.name),
                name: tool.name.clone(),
                description: tool.description.clone(),
                parameters: tool.parameters.clone(),
                permission_level: String::new(),
                risk_level: String::new(),
                version: "1.0.0".into(),
                source: ToolSource::Mcp {
                    server_name: "cached_registry_snapshot".into(),
                },
                capabilities: Vec::new(),
                requires_confirmation: true,
                enabled: true,
                declarative_only: false,
                action_type: String::new(),
                idempotency_contract: ToolIdempotencyContract::Unspecified,
                tags: vec!["migration:name_inferred_contract_warning".into()],
            }
            .normalized()
        }));
        out
    }

    fn execute_manifest_body(&self, manifest: &ToolManifest, arguments: Value) -> Result<String> {
        match &manifest.source {
            ToolSource::BuiltIn => {
                if let Some(func) = self.builtins.get(&manifest.name) {
                    func(arguments)
                } else {
                    Err(anyhow::anyhow!(
                        "built-in tool '{}' not found",
                        manifest.name
                    ))
                }
            }
            ToolSource::Mcp { .. } => Err(anyhow::anyhow!(
                "MCP manifest execution requires the asynchronous transport"
            )),
            ToolSource::Plugin { plugin_id } => Err(anyhow::anyhow!(
                "Plugin tool '{}' from '{}' requires a configured executor/provider before it can run",
                manifest.name,
                plugin_id
            )),
        }
    }

    /// Execute only after the caller owns the concrete instance lease. Keeping
    /// the lease in the type-level call boundary prevents a future product
    /// path from bypassing the retire/acquire linearization point.
    fn execute_manifest_after_instance_acquire(
        &self,
        manifest: &ToolManifest,
        arguments: Value,
        _instance_lease: &ExecutorInstanceLease,
    ) -> Result<String> {
        self.execute_manifest_body(manifest, arguments)
    }

    #[cfg(test)]
    fn execute_manifest(&self, manifest: &ToolManifest, arguments: Value) -> Result<String> {
        self.execute_manifest_body(manifest, arguments)
    }

    pub(crate) async fn execute_manifest_async_with_receipt_tracker(
        &self,
        manifest: &ToolManifest,
        arguments: Value,
        receipt_tracker: ToolExecutionReceiptTracker,
        started_observer: Option<&dyn ToolStartedTransitionObserver>,
    ) -> Result<String> {
        // This atomic instance lease is the concrete adapter-edge
        // linearization point. A concurrent replacement either retires first
        // (this attempt fails before the callback/client sees arguments) or
        // this acquire wins (the in-flight attempt remains bound to the old
        // instance while replacement affects only subsequent attempts).
        let instance_lease = self.acquire_execution_instance(manifest)?;
        match &manifest.source {
            ToolSource::Mcp { server_name } => {
                self.call_tool_on_server_after_instance_acquire(
                    server_name,
                    &manifest.name,
                    arguments,
                    receipt_tracker,
                    started_observer,
                    &instance_lease,
                )
                .await
            }
            ToolSource::BuiltIn => {
                receipt_tracker.mark_local_dispatch_attempted();
                let result = self.execute_manifest_after_instance_acquire(
                    manifest,
                    arguments,
                    &instance_lease,
                );
                // A local callback return is the first concrete boundary we
                // can prove. Entering the callback alone remains ambiguous if
                // it panics, blocks forever, or the owning future is aborted.
                receipt_tracker.mark_local_dispatch_observed();
                if let Some(observer) = started_observer {
                    observer.after_dispatch(&receipt_tracker.snapshot()).await?;
                }
                receipt_tracker.mark_response_observed();
                if result.is_ok() {
                    receipt_tracker.mark_execution_succeeded();
                    receipt_tracker.mark_effect_confirmed();
                } else {
                    receipt_tracker.mark_execution_failed();
                    receipt_tracker.mark_effect_unknown_if_dispatched();
                }
                receipt_tracker.finish();
                result
            }
            ToolSource::Plugin { .. } => {
                receipt_tracker.finish();
                self.execute_manifest_after_instance_acquire(manifest, arguments, &instance_lease)
            }
        }
    }

    async fn call_tool_on_server_after_instance_acquire(
        &self,
        server_name: &str,
        name: &str,
        arguments: Value,
        receipt_tracker: ToolExecutionReceiptTracker,
        started_observer: Option<&dyn ToolStartedTransitionObserver>,
        _instance_lease: &ExecutorInstanceLease,
    ) -> Result<String> {
        self.call_tool_on_server_with_receipt_tracker_body(
            server_name,
            name,
            arguments,
            receipt_tracker,
            started_observer,
        )
        .await
    }

    async fn call_tool_on_server_with_receipt_tracker_body(
        &self,
        server_name: &str,
        name: &str,
        arguments: Value,
        receipt_tracker: ToolExecutionReceiptTracker,
        started_observer: Option<&dyn ToolStartedTransitionObserver>,
    ) -> Result<String> {
        let client = self
            .clients
            .get(server_name)
            .ok_or_else(|| anyhow::anyhow!("MCP server '{}' not found", server_name))?;

        // 1. Detect and desensitize arguments
        let args_str = arguments.to_string();
        let pii = self.privacy_engine.detect(&args_str);
        let (desensitized_str, map) = if pii.is_empty() {
            (args_str, HashMap::new())
        } else {
            self.privacy_engine.desensitize(&args_str)
        };
        let desensitized_args: Value =
            serde_json::from_str(&desensitized_str).unwrap_or_else(|_| arguments.clone());

        // 2. Execute on the specific server
        let result = client
            .call_tool_with_receipt_tracker(
                name,
                desensitized_args,
                receipt_tracker,
                started_observer,
            )
            .await?;

        // 3. Reconstruct any placeholders in the result
        let final_result = if map.is_empty() {
            result
        } else {
            self.privacy_engine.reconstruct(&result, &map)
        };

        Ok(final_result)
    }

    /// Generate a system prompt snippet describing available tools
    pub fn tools_prompt(&self) -> String {
        let manifests = self.list_manifests();
        if manifests.is_empty() {
            return "".into();
        }
        let mut lines = vec!["\n你可以使用以下工具:\n".to_string()];
        for m in manifests
            .iter()
            .filter(|m| m.enabled && !m.declarative_only)
        {
            lines.push(format!("- {}: {}", m.name, m.description));
        }
        if lines.len() == 1 {
            return "".into();
        }
        lines.push(
            "\n如果需要使用工具，只回复一个合法 JSON 对象，不要使用 markdown 代码块，不要附加解释。格式：{\"tool_calls\": [{\"name\": \"web.search\", \"arguments\": {\"query\": \"今天日期\"}}]} 或 {\"tool_calls\": [{\"name\": \"web.fetch\", \"arguments\": {\"url\": \"https://example.com\"}}]}。工具名必须完整匹配上面的名称，URL 必须包含 http:// 或 https://。"
                .into(),
        );
        lines.join("\n")
    }

    /// Scan arguments for PII and return findings without calling the tool.
    pub fn scan_pii(&self, arguments: &Value) -> Vec<(crate::privacy::PrivacyType, String)> {
        self.privacy_engine.detect(&arguments.to_string())
    }

    pub fn inspect_call_arguments(&self, name: &str, arguments: &Value) -> McpArgumentInspection {
        let permission_level = self.tool_permission_level(name);
        let is_builtin = self.builtins.contains_key(name);
        let findings = collect_privacy_findings(&self.privacy_engine, arguments, "$");
        let pii_found = !findings.is_empty();
        let args_str = arguments.to_string();
        let sanitized_arguments = if pii_found {
            let (masked, _) = self.privacy_engine.desensitize(&args_str);
            serde_json::from_str(&masked).unwrap_or_else(|_| arguments.clone())
        } else {
            arguments.clone()
        };
        let requires_confirmation =
            !is_builtin || permission_level == "high" || (pii_found && permission_level != "low");
        McpArgumentInspection {
            permission_level,
            pii_found,
            findings,
            sanitized_arguments,
            requires_confirmation,
        }
    }

    /// Determine the permission level of a tool by its name.
    /// Returns "high" for filesystem-modifying or shell-like tools, "medium" for search/fetch,
    /// and "low" for read-only or safe tools.
    pub fn tool_permission_level(&self, name: &str) -> String {
        ToolManifest::infer_permission_level(name)
    }

    /// Recommend tools based on goal-capability gap analysis strings.
    /// Simple v1 engine: score by keyword overlap between gap text and manifest tags.
    pub fn recommend_manifests(&self, gaps: &[String], top_k: usize) -> Vec<ToolManifest> {
        let manifests = self.list_manifests();
        let mut scored: Vec<(i32, &ToolManifest)> = manifests
            .iter()
            .map(|m| {
                let mut score = 0i32;
                for gap in gaps {
                    let gap_lower = gap.to_lowercase();
                    for tag in &m.tags {
                        if gap_lower.contains(&tag.to_lowercase()) {
                            score += 1;
                        }
                    }
                    // heuristic boost for keywords in name/description if tags are empty
                    if m.tags.is_empty() {
                        let text = format!("{} {}", m.name, m.description).to_lowercase();
                        let keywords = [
                            "write", "read", "search", "fetch", "file", "code", "git", "db", "sql",
                            "web",
                        ];
                        for kw in &keywords {
                            if text.contains(kw) && gap_lower.contains(kw) {
                                score += 1;
                            }
                        }
                    }
                }
                (score, m)
            })
            .filter(|(s, _)| *s > 0)
            .collect();
        scored.sort_by_key(|item| Reverse(item.0));
        scored.truncate(top_k);
        scored.into_iter().map(|(_, m)| m.clone()).collect()
    }
}

impl Default for McpRegistry {
    fn default() -> Self {
        Self::new()
    }
}

fn collect_privacy_findings(
    engine: &PrivacyEngine,
    value: &Value,
    path: &str,
) -> Vec<McpPrivacyFinding> {
    let mut findings = Vec::new();
    match value {
        Value::Object(map) => {
            for (key, nested) in map {
                let next_path = format!("{}.{}", path, key);
                findings.extend(collect_privacy_findings(engine, nested, &next_path));
            }
        }
        Value::Array(items) => {
            for (idx, nested) in items.iter().enumerate() {
                let next_path = format!("{}[{}]", path, idx);
                findings.extend(collect_privacy_findings(engine, nested, &next_path));
            }
        }
        Value::String(text) => {
            for (ptype, matched) in engine.detect(text) {
                findings.push(McpPrivacyFinding {
                    path: path.to_string(),
                    privacy_type: format!("{:?}", ptype),
                    matched,
                });
            }
        }
        _ => {}
    }
    findings
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool_execution_receipt::ToolExecutionReceipt;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    struct StartedAfterBuiltinCallbackObserver {
        stage: Arc<AtomicUsize>,
    }

    #[async_trait::async_trait]
    impl ToolStartedTransitionObserver for StartedAfterBuiltinCallbackObserver {
        async fn after_dispatch(&self, receipt: &ToolExecutionReceipt) -> Result<()> {
            anyhow::ensure!(
                self.stage.load(Ordering::SeqCst) == 1,
                "tool.started was observed before the builtin callback returned"
            );
            anyhow::ensure!(receipt.dispatch_observed);
            anyhow::ensure!(receipt.dispatched_at.is_some());
            self.stage.store(2, Ordering::SeqCst);
            Ok(())
        }
    }

    #[derive(Default)]
    struct FlushNeverCompletesWriter {
        bytes: Vec<u8>,
    }

    impl AsyncWrite for FlushNeverCompletesWriter {
        fn poll_write(
            mut self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
            buffer: &[u8],
        ) -> std::task::Poll<std::io::Result<usize>> {
            self.bytes.extend_from_slice(buffer);
            std::task::Poll::Ready(Ok(buffer.len()))
        }

        fn poll_flush(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            std::task::Poll::Pending
        }

        fn poll_shutdown(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            std::task::Poll::Ready(Ok(()))
        }
    }

    #[tokio::test]
    async fn builtin_started_observer_runs_only_after_callback_returns() {
        let mut registry = McpRegistry::new();
        let manifest = executor_gate_builtin_manifest("builtin_started_after_callback");
        let stage = Arc::new(AtomicUsize::new(0));
        let callback_stage = Arc::clone(&stage);
        registry.register_builtin(
            manifest.clone(),
            Box::new(move |_arguments| {
                assert_eq!(callback_stage.load(Ordering::SeqCst), 0);
                callback_stage.store(1, Ordering::SeqCst);
                Ok("callback-returned".into())
            }),
        );
        let tracker = ToolExecutionReceiptTracker::new(
            Some("run-builtin-started-after-callback".into()),
            Some(manifest.id.clone()),
            "builtin-started-after-callback".into(),
            ToolActionEffect::ReadOnly,
            ToolIdempotencyContract::Idempotent,
        );
        let observer = StartedAfterBuiltinCallbackObserver {
            stage: Arc::clone(&stage),
        };

        let result = registry
            .execute_manifest_async_with_receipt_tracker(
                &manifest,
                serde_json::json!({}),
                tracker.clone(),
                Some(&observer),
            )
            .await
            .expect("execute builtin callback");

        assert_eq!(result, "callback-returned");
        assert_eq!(stage.load(Ordering::SeqCst), 2);
        let receipt = tracker.snapshot();
        assert!(receipt.dispatch_observed);
        assert_eq!(
            receipt.transport_status,
            crate::tool_execution_receipt::ToolTransportStatus::ResponseObserved
        );
        receipt
            .mechanically_valid_terminal()
            .expect("builtin callback receipt is mechanically valid");
    }

    fn legacy_product_copy_regex() -> regex::Regex {
        let terms = [
            format!(r"\b{}\b", ["Be", "ta"].concat()),
            format!(r"\b{}\b", ["M", "VP"].concat()),
            format!(r"\b{}\b", ["st", "ub"].concat()),
            ["legacy", "stream"].join("_"),
            ["declarative", "only"].join("-"),
        ];
        regex::Regex::new(&terms.join("|")).expect("legacy product copy regex")
    }

    #[tokio::test]
    async fn mcp_delimiter_write_is_dispatch_even_when_following_flush_never_finishes() {
        let mut writer = FlushNeverCompletesWriter::default();
        let tracker = ToolExecutionReceiptTracker::new(
            Some("run-flush-window".into()),
            Some("mcp:test:flush-window".into()),
            "request-digest-flush-window".into(),
            ToolActionEffect::ExternalMutation,
            ToolIdempotencyContract::NonIdempotent,
        );
        let request = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id: 7,
            method: "tools/call".into(),
            params: Some(serde_json::json!({"name":"side_effect"})),
        };

        let timed_out = tokio::time::timeout(
            std::time::Duration::from_millis(20),
            write_json_rpc_request_frame(
                &mut writer,
                &request,
                MCP_MAX_FRAME_BYTES,
                Some(&tracker),
            ),
        )
        .await;
        assert!(timed_out.is_err(), "test writer must remain stuck in flush");
        assert!(
            writer.bytes.ends_with(b"\n"),
            "the peer-visible frame delimiter crossed the pipe boundary"
        );

        let dispatched = tracker.snapshot();
        assert_eq!(
            dispatched.transport_status,
            crate::tool_execution_receipt::ToolTransportStatus::Dispatched
        );
        assert_eq!(
            dispatched.dispatch_kind,
            crate::tool_execution_receipt::ToolDispatchKind::McpStdio
        );
        assert_eq!(dispatched.dispatch_attempt_count, 1);
        assert!(dispatched.dispatched_at.is_some());
        assert!(dispatched.response_observed_at.is_none());

        tracker.mark_local_aborted();
        tracker.finish();
        let terminal = tracker.snapshot();
        assert_eq!(
            terminal.transport_status,
            crate::tool_execution_receipt::ToolTransportStatus::RemoteUnknown
        );
        assert_eq!(
            terminal.effect_status,
            crate::tool_execution_receipt::ToolEffectStatus::Unknown
        );
        assert!(!terminal.automatic_retry_safe());
        terminal
            .mechanically_valid_terminal()
            .expect("flush-window cancellation must produce a valid unknown receipt");
    }

    #[test]
    fn memory_write_manifest_exposes_the_reviewed_candidate_contract() {
        let registry = McpRegistry::new();
        let manifest = registry
            .list_manifests()
            .into_iter()
            .find(|manifest| manifest.name == "memory.propose_write")
            .expect("memory write manifest");

        assert_eq!(manifest.parameters["type"], serde_json::json!("object"));
        assert_eq!(
            manifest.parameters["additionalProperties"],
            serde_json::json!(false)
        );
        assert_eq!(
            manifest.parameters["required"],
            serde_json::json!(["content"])
        );
        assert_eq!(
            manifest.parameters["properties"]["content"]["maxLength"],
            serde_json::json!(65536)
        );
        assert!(manifest.parameters["properties"]["candidateKind"]["enum"]
            .as_array()
            .is_some_and(|values| values.iter().any(|value| value == "identity_or_role")));
    }

    #[test]
    fn memory_archive_manifest_requires_typed_canonical_owners() {
        let registry = McpRegistry::new();
        let manifest = registry
            .list_manifests()
            .into_iter()
            .find(|manifest| manifest.name == "memory.propose_archive")
            .expect("memory archive manifest");

        assert_eq!(manifest.parameters["type"], serde_json::json!("object"));
        assert_eq!(
            manifest.parameters["additionalProperties"],
            serde_json::json!(false)
        );
        assert_eq!(
            manifest.parameters["properties"]["owner"]["required"],
            serde_json::json!(["ownerKind", "ownerId"])
        );
        assert_eq!(
            manifest.parameters["properties"]["owners"]["minItems"],
            serde_json::json!(1)
        );
        assert_eq!(
            manifest.parameters["properties"]["owners"]["maxItems"],
            serde_json::json!(200)
        );
        assert_eq!(manifest.parameters["oneOf"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn typed_mcp_manifest_without_idempotency_contract_is_rejected_at_registration_boundary() {
        let parameters = serde_json::json!({"type": "object"});
        let discovered = vec![Tool {
            name: "typed.read".into(),
            description: "Typed read".into(),
            parameters: parameters.clone(),
        }];
        let manifests = vec![ToolManifest {
            id: "mcp:typed:typed.read".into(),
            name: "typed.read".into(),
            description: "Typed read".into(),
            parameters,
            permission_level: "low".into(),
            risk_level: "low".into(),
            version: "1.0.0".into(),
            source: ToolSource::Mcp {
                server_name: "typed".into(),
            },
            capabilities: vec!["read".into()],
            requires_confirmation: false,
            enabled: true,
            declarative_only: false,
            action_type: "read".into(),
            idempotency_contract: ToolIdempotencyContract::Unspecified,
            tags: vec!["typed_contract".into()],
        }];

        let error = validate_typed_mcp_manifests("typed", &discovered, &manifests)
            .expect_err("missing idempotency is an incomplete MCP manifest")
            .to_string();
        assert!(error.contains("incomplete or non-executable"));
    }

    #[test]
    fn typed_mcp_manifest_with_unknown_permission_risk_or_action_is_rejected_at_registration_boundary(
    ) {
        let parameters = serde_json::json!({"type": "object"});
        let discovered = vec![Tool {
            name: "typed.read".into(),
            description: "Typed read".into(),
            parameters: parameters.clone(),
        }];
        for (permission_level, risk_level, action_type) in [
            ("mystery", "low", "read"),
            ("low", "mystery", "read"),
            ("low", "low", "mystery"),
        ] {
            let manifest = ToolManifest {
                id: "mcp:typed:typed.read".into(),
                name: "typed.read".into(),
                description: "Typed read".into(),
                parameters: parameters.clone(),
                permission_level: permission_level.into(),
                risk_level: risk_level.into(),
                version: "1.0.0".into(),
                source: ToolSource::Mcp {
                    server_name: "typed".into(),
                },
                capabilities: vec!["read".into()],
                requires_confirmation: false,
                enabled: true,
                declarative_only: false,
                action_type: action_type.into(),
                idempotency_contract: ToolIdempotencyContract::Idempotent,
                tags: vec!["typed_contract".into()],
            };

            let error = validate_typed_mcp_manifests("typed", &discovered, &[manifest])
                .expect_err("unknown typed execution vocabulary must fail at registration")
                .to_string();
            assert!(error.contains("execution contract is invalid"));
        }
    }

    #[test]
    fn user_visible_mcp_manifest_copy_has_no_legacy_product_terms() {
        let registry = McpRegistry::new();
        let legacy_terms = legacy_product_copy_regex();
        let mut violations = Vec::new();
        for manifest in registry.list_manifests() {
            let copy = [
                manifest.id,
                manifest.name,
                manifest.description,
                manifest.capabilities.join(" "),
                manifest.tags.join(" "),
            ]
            .join(" ");
            if legacy_terms.is_match(&copy) {
                violations.push(copy);
            }
        }
        assert!(
            violations.is_empty(),
            "legacy product terms leaked in MCP manifest copy: {violations:?}"
        );
    }

    #[test]
    fn release_product_registry_keeps_core_capabilities_without_extension_dispatch() {
        let registry = McpRegistry::new_release_product();
        let names = registry
            .list_manifests()
            .into_iter()
            .map(|manifest| manifest.name)
            .collect::<std::collections::BTreeSet<_>>();

        assert!(names.contains("web.search"));
        assert!(names.contains("file.read"));
        assert!(names.contains("document.read"));
        assert!(names.contains("memory.propose_write"));
        assert!(!names.contains("builtin_echo"));
        assert!(!names.contains("mcp.call_tool"));
    }

    #[test]
    fn user_visible_tool_result_copy_has_no_legacy_product_terms() {
        let registry = McpRegistry::new();
        let legacy_terms = legacy_product_copy_regex();
        let mut copies = Vec::new();

        let manifest_only = registry
            .list_manifests()
            .into_iter()
            .find(|manifest| manifest.name == "snapshot.create")
            .expect("snapshot.create manifest");
        copies.push(
            registry
                .execute_manifest(&manifest_only, serde_json::json!({}))
                .expect("manifest-only capability result"),
        );

        let plugin_manifest = ToolManifest {
            id: "plugin.example.read".into(),
            name: "plugin.example.read".into(),
            description: "Read example data from a configured plugin provider.".into(),
            parameters: serde_json::json!({"type": "object"}),
            permission_level: "low".into(),
            risk_level: "low".into(),
            version: "1.0.0".into(),
            source: ToolSource::Plugin {
                plugin_id: "example-plugin".into(),
            },
            capabilities: vec!["read".into()],
            requires_confirmation: false,
            enabled: true,
            declarative_only: true,
            action_type: "read".into(),
            idempotency_contract: ToolIdempotencyContract::NonIdempotent,
            tags: vec!["read".into(), "manifest_only".into()],
        };
        copies.push(
            registry
                .execute_manifest(&plugin_manifest, serde_json::json!({}))
                .expect_err("plugin manifest requires configured executor")
                .to_string(),
        );

        let violations = copies
            .into_iter()
            .filter(|copy| legacy_terms.is_match(copy))
            .collect::<Vec<_>>();
        assert!(
            violations.is_empty(),
            "legacy product terms leaked in MCP tool result copy: {violations:?}"
        );
    }

    #[test]
    fn inspect_call_arguments_marks_medium_with_pii_for_confirmation() {
        let registry = McpRegistry::new();
        let inspection = registry.inspect_call_arguments(
            "web_search",
            &serde_json::json!({
                "query": "帮我搜索 test@example.com 的公开信息"
            }),
        );
        assert_eq!(inspection.permission_level, "medium");
        assert!(inspection.pii_found);
        assert!(inspection.requires_confirmation);
        assert_eq!(inspection.findings[0].path, "$.query");
        assert_ne!(
            inspection.sanitized_arguments["query"],
            "帮我搜索 test@example.com 的公开信息"
        );
    }

    #[test]
    fn inspect_web_search_does_not_treat_ordinary_chinese_copy_as_a_name() {
        let registry = McpRegistry::new();
        let inspection = registry.inspect_call_arguments(
            "web.search",
            &serde_json::json!({
                "query": "搜索 Example Domain 官方页面的标题",
                "max_results": 5,
                "governedInputSource": "kernel_web_search_query_from_user_text"
            }),
        );

        assert!(!inspection.pii_found, "{:?}", inspection.findings);
        assert!(!inspection.requires_confirmation);
    }

    #[test]
    fn inspect_call_arguments_keeps_low_risk_without_pii() {
        let registry = McpRegistry::new();
        let inspection = registry.inspect_call_arguments(
            "builtin_echo",
            &serde_json::json!({ "text": "hello world" }),
        );
        assert_eq!(inspection.permission_level, "low");
        assert!(!inspection.pii_found);
        assert!(!inspection.requires_confirmation);
    }

    #[test]
    fn inspect_call_arguments_requires_confirmation_for_external_low_risk_tools() {
        let registry = McpRegistry::new();
        let inspection = registry
            .inspect_call_arguments("calculator", &serde_json::json!({ "expression": "1 + 1" }));
        assert_eq!(inspection.permission_level, "low");
        assert!(!inspection.pii_found);
        assert!(inspection.requires_confirmation);
    }

    #[test]
    fn web_search_is_executable_and_in_tools_prompt() {
        let registry = McpRegistry::new();
        let manifest = registry
            .list_manifests()
            .into_iter()
            .find(|m| m.name == "web.search")
            .expect("web.search manifest should be registered");

        assert!(manifest.enabled);
        assert!(!manifest.declarative_only);
        assert_eq!(manifest.action_type, "read");
        assert!(manifest.capabilities.iter().any(|c| c == "network"));

        let prompt = registry.tools_prompt();
        assert!(prompt.contains("web.search"));
        assert!(prompt.contains("\"query\""));
    }

    fn test_limits() -> McpClientLimits {
        McpClientLimits {
            handshake_timeout: std::time::Duration::from_secs(2),
            list_timeout: std::time::Duration::from_secs(2),
            call_timeout: std::time::Duration::from_millis(200),
            max_frame_bytes: 4096,
        }
    }

    fn executor_gate_builtin_manifest(name: &str) -> ToolManifest {
        let mut manifest = ToolManifest::new(
            name,
            "Executor instance gate test.",
            serde_json::json!({"type": "object"}),
            "low",
            "1",
            ToolSource::BuiltIn,
        )
        .with_capabilities(vec!["read".into()])
        .with_idempotency_contract(ToolIdempotencyContract::Idempotent);
        manifest.action_type = "read".into();
        manifest
    }

    #[tokio::test]
    async fn executor_instance_gate_linearizes_builtin_replacement_at_adapter_edge() {
        use std::sync::atomic::AtomicUsize;

        // A: replacement retires first. The stale registry snapshot cannot
        // pass arguments to either the old or replacement callback and its
        // receipt remains pre-dispatch.
        let stale_count = Arc::new(AtomicUsize::new(0));
        let replacement_count = Arc::new(AtomicUsize::new(0));
        let mut live = McpRegistry::new();
        let manifest = executor_gate_builtin_manifest("executor_gate_retire_wins");
        let stale_counter = Arc::clone(&stale_count);
        live.register_builtin(
            manifest.clone(),
            Box::new(move |_arguments| {
                stale_counter.fetch_add(1, Ordering::SeqCst);
                Ok("stale".into())
            }),
        );
        let snapshot = live.clone();
        let snapshot_manifest = snapshot
            .list_manifests()
            .into_iter()
            .find(|candidate| candidate.id == manifest.id)
            .expect("snapshot manifest");
        let stale_binding = snapshot
            .dispatch_binding(&snapshot_manifest)
            .expect("snapshot binding");
        let replacement_counter = Arc::clone(&replacement_count);
        live.register_builtin(
            snapshot_manifest.clone(),
            Box::new(move |_arguments| {
                replacement_counter.fetch_add(1, Ordering::SeqCst);
                Ok("replacement".into())
            }),
        );
        let live_manifest = live
            .list_manifests()
            .into_iter()
            .find(|candidate| candidate.id == manifest.id)
            .expect("replacement manifest");
        let live_binding = live
            .dispatch_binding(&live_manifest)
            .expect("replacement binding");
        assert_ne!(
            stale_binding.executor_instance_id(),
            live_binding.executor_instance_id()
        );
        let rejected_tracker = ToolExecutionReceiptTracker::new(
            Some("run-executor-gate-retire-wins".into()),
            Some(snapshot_manifest.id.clone()),
            "request-digest-executor-gate-retire-wins".into(),
            ToolActionEffect::ReadOnly,
            ToolIdempotencyContract::Idempotent,
        );
        let error = snapshot
            .execute_manifest_async_with_receipt_tracker(
                &snapshot_manifest,
                serde_json::json!({"secret": "must-not-cross"}),
                rejected_tracker.clone(),
                None,
            )
            .await
            .expect_err("retired snapshot must fail before callback dispatch")
            .to_string();
        assert!(error.contains("mcp_registry_dispatch_instance_retired"));
        assert_eq!(stale_count.load(Ordering::SeqCst), 0);
        assert_eq!(replacement_count.load(Ordering::SeqCst), 0);
        let rejected_receipt = rejected_tracker.snapshot();
        assert_eq!(rejected_receipt.dispatch_attempt_count, 0);
        assert_eq!(
            rejected_receipt.transport_status,
            crate::tool_execution_receipt::ToolTransportStatus::NotAttempted
        );

        // B: acquire wins. This directly exercises only the continuation below
        // the production acquire point (not a second full-path race): the one
        // invocation is already linearized against the old instance,
        // replacement returns immediately, and only later attempts are
        // affected.
        let old_count = Arc::new(AtomicUsize::new(0));
        let next_count = Arc::new(AtomicUsize::new(0));
        let mut live = McpRegistry::new();
        let manifest = executor_gate_builtin_manifest("executor_gate_acquire_wins");
        let old_counter = Arc::clone(&old_count);
        live.register_builtin(
            manifest.clone(),
            Box::new(move |_arguments| {
                old_counter.fetch_add(1, Ordering::SeqCst);
                Ok("old-linearized".into())
            }),
        );
        let snapshot = live.clone();
        let snapshot_manifest = snapshot
            .list_manifests()
            .into_iter()
            .find(|candidate| candidate.id == manifest.id)
            .expect("snapshot manifest");
        let instance_lease = snapshot
            .acquire_execution_instance(&snapshot_manifest)
            .expect("old instance acquire wins before replacement");
        let next_counter = Arc::clone(&next_count);
        live.register_builtin(
            snapshot_manifest.clone(),
            Box::new(move |_arguments| {
                next_counter.fetch_add(1, Ordering::SeqCst);
                Ok("next".into())
            }),
        );
        assert_eq!(
            snapshot
                .execute_manifest_after_instance_acquire(
                    &snapshot_manifest,
                    serde_json::json!({"authorized": true}),
                    &instance_lease,
                )
                .expect("already-acquired old instance may finish"),
            "old-linearized"
        );
        drop(instance_lease);
        assert_eq!(old_count.load(Ordering::SeqCst), 1);
        assert_eq!(next_count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn async_stdio_transport_completes_handshake_list_and_call() {
        let script = r#"
import json, sys
for line in sys.stdin:
    message = json.loads(line)
    method = message.get('method')
    if method == 'initialize':
        print(json.dumps({'jsonrpc':'2.0','id':message['id'],'result':{'protocolVersion':'2024-11-05','capabilities':{}}}), flush=True)
    elif method == 'tools/list':
        print(json.dumps({'jsonrpc':'2.0','id':message['id'],'result':{'tools':[{'name':'echo','description':'echo','parameters':{'type':'object'}}]}}), flush=True)
    elif method == 'tools/call':
        print(json.dumps({'jsonrpc':'2.0','id':message['id'],'result':{'content':[{'type':'text','text':'pong'}]}}), flush=True)
"#;
        let client = McpClient::new_with_limits(
            "python3",
            &["-u", "-c", script],
            &HashMap::new(),
            test_limits(),
        )
        .await
        .unwrap();
        let tools = client.list_tools().await.unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "echo");
        let receipt_tracker = crate::tool_execution_receipt::ToolExecutionReceiptTracker::new(
            Some("run-successful-mcp".into()),
            Some("mcp:test:echo".into()),
            "request-digest-success".into(),
            crate::tool_execution_receipt::ToolActionEffect::ReadOnly,
            ToolIdempotencyContract::Idempotent,
        );
        assert_eq!(
            client
                .call_tool_with_receipt_tracker(
                    "echo",
                    serde_json::json!({"value": "ping"}),
                    receipt_tracker.clone(),
                    None,
                )
                .await
                .unwrap(),
            "pong"
        );
        let receipt = receipt_tracker.snapshot();
        assert_eq!(
            receipt.transport_status,
            crate::tool_execution_receipt::ToolTransportStatus::ResponseObserved
        );
        assert_eq!(
            receipt.effect_status,
            crate::tool_execution_receipt::ToolEffectStatus::NotAttempted
        );
        assert!(receipt.dispatched_at.is_some());
        assert!(receipt.response_observed_at.is_some());
        assert_eq!(
            client
                .call_tool("echo", serde_json::json!({"value": "second"}))
                .await
                .unwrap(),
            "pong",
            "a fully matched response keeps the transport reusable"
        );
    }

    fn executor_gate_mcp_manifest(server_name: &str) -> ToolManifest {
        ToolManifest {
            id: format!("mcp:{server_name}:echo"),
            name: "echo".into(),
            description: "Echo bounded text".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {"text": {"type": "string"}}
            }),
            permission_level: "low".into(),
            risk_level: "low".into(),
            version: "1.0.0".into(),
            source: ToolSource::Mcp {
                server_name: server_name.into(),
            },
            capabilities: vec!["read".into()],
            requires_confirmation: false,
            enabled: true,
            declarative_only: false,
            action_type: "read".into(),
            idempotency_contract: ToolIdempotencyContract::Idempotent,
            tags: vec!["read".into()],
        }
    }

    #[tokio::test]
    async fn executor_instance_gate_linearizes_mcp_unregister_at_adapter_edge() {
        let script = r#"
import json, sys
for line in sys.stdin:
    message = json.loads(line)
    method = message.get('method')
    if method == 'initialize':
        print(json.dumps({'jsonrpc':'2.0','id':message['id'],'result':{'protocolVersion':'2024-11-05','capabilities':{}}}), flush=True)
    elif method == 'tools/list':
        print(json.dumps({'jsonrpc':'2.0','id':message['id'],'result':{'tools':[{'name':'echo','description':'echo','parameters':{'type':'object','properties':{'text':{'type':'string'}}}}]}}), flush=True)
    elif method == 'tools/call':
        text = message.get('params', {}).get('arguments', {}).get('text', '')
        print(json.dumps({'jsonrpc':'2.0','id':message['id'],'result':{'content':[{'type':'text','text':text}]}}), flush=True)
"#;

        // A: unregister retires first, so the stale snapshot never writes a
        // tools/call frame and its receipt remains pre-dispatch.
        let mut live = McpRegistry::new();
        let args = ["-u", "-c", script];
        live.register_with_env_and_manifests(
            "gate-retire-wins",
            "python3",
            &args,
            &HashMap::new(),
            vec![executor_gate_mcp_manifest("gate-retire-wins")],
        )
        .await
        .expect("register MCP gate fixture");
        let snapshot = live.clone();
        let manifest = snapshot
            .list_manifests()
            .into_iter()
            .find(|manifest| manifest.id == "mcp:gate-retire-wins:echo")
            .expect("MCP snapshot manifest");
        live.unregister("gate-retire-wins")
            .expect("retire live MCP instance");
        let rejected_tracker = ToolExecutionReceiptTracker::new(
            Some("run-mcp-gate-retire-wins".into()),
            Some(manifest.id.clone()),
            "request-digest-mcp-gate-retire-wins".into(),
            ToolActionEffect::ReadOnly,
            ToolIdempotencyContract::Idempotent,
        );
        let error = snapshot
            .execute_manifest_async_with_receipt_tracker(
                &manifest,
                serde_json::json!({"text": "must-not-cross"}),
                rejected_tracker.clone(),
                None,
            )
            .await
            .expect_err("retired MCP snapshot must fail before transport")
            .to_string();
        assert!(error.contains("mcp_registry_dispatch_instance_retired"));
        let rejected_receipt = rejected_tracker.snapshot();
        assert_eq!(rejected_receipt.dispatch_attempt_count, 0);
        assert_eq!(
            rejected_receipt.transport_status,
            crate::tool_execution_receipt::ToolTransportStatus::NotAttempted
        );

        // B: acquire wins. This directly exercises the MCP continuation below
        // the production acquire point (not a second full-path race):
        // unregister does not wait for the remote call, and the
        // already-linearized snapshot may complete exactly that call.
        let mut live = McpRegistry::new();
        live.register_with_env_and_manifests(
            "gate-acquire-wins",
            "python3",
            &args,
            &HashMap::new(),
            vec![executor_gate_mcp_manifest("gate-acquire-wins")],
        )
        .await
        .expect("register second MCP gate fixture");
        let snapshot = live.clone();
        let manifest = snapshot
            .list_manifests()
            .into_iter()
            .find(|manifest| manifest.id == "mcp:gate-acquire-wins:echo")
            .expect("second MCP snapshot manifest");
        let instance_lease = snapshot
            .acquire_execution_instance(&manifest)
            .expect("MCP instance acquire wins before unregister");
        live.unregister("gate-acquire-wins")
            .expect("unregister returns without waiting on in-flight lease");
        let accepted_tracker = ToolExecutionReceiptTracker::new(
            Some("run-mcp-gate-acquire-wins".into()),
            Some(manifest.id.clone()),
            "request-digest-mcp-gate-acquire-wins".into(),
            ToolActionEffect::ReadOnly,
            ToolIdempotencyContract::Idempotent,
        );
        assert_eq!(
            snapshot
                .call_tool_on_server_after_instance_acquire(
                    "gate-acquire-wins",
                    "echo",
                    serde_json::json!({"text": "already-authorized"}),
                    accepted_tracker.clone(),
                    None,
                    &instance_lease,
                )
                .await
                .expect("already-acquired MCP instance may finish"),
            "already-authorized"
        );
        drop(instance_lease);
        assert_eq!(accepted_tracker.snapshot().dispatch_attempt_count, 1);
    }

    #[tokio::test]
    async fn hung_mcp_call_is_bounded_by_transport_timeout() {
        let script = r#"
import json, sys, time
for line in sys.stdin:
    message = json.loads(line)
    method = message.get('method')
    if method == 'initialize':
        print(json.dumps({'jsonrpc':'2.0','id':message['id'],'result':{'protocolVersion':'2024-11-05','capabilities':{}}}), flush=True)
    elif method == 'tools/call':
        time.sleep(30)
"#;
        let client = McpClient::new_with_limits(
            "python3",
            &["-u", "-c", script],
            &HashMap::new(),
            test_limits(),
        )
        .await
        .unwrap();
        let receipt_tracker = crate::tool_execution_receipt::ToolExecutionReceiptTracker::new(
            Some("run-hung-mcp".into()),
            Some("mcp:test:hang".into()),
            "request-digest-hung".into(),
            crate::tool_execution_receipt::ToolActionEffect::ExternalMutation,
            ToolIdempotencyContract::NonIdempotent,
        );
        let started = std::time::Instant::now();
        let error = client
            .call_tool_with_receipt_tracker(
                "hang",
                serde_json::json!({}),
                receipt_tracker.clone(),
                None,
            )
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("timed out"));
        assert!(started.elapsed() < std::time::Duration::from_secs(2));
        let receipt = receipt_tracker.snapshot();
        assert_eq!(
            receipt.transport_status,
            crate::tool_execution_receipt::ToolTransportStatus::RemoteUnknown
        );
        assert_eq!(
            receipt.effect_status,
            crate::tool_execution_receipt::ToolEffectStatus::Unknown
        );
        assert!(receipt.dispatched_at.is_some());
        assert!(receipt.response_observed_at.is_none());

        let retry_started = std::time::Instant::now();
        let retry_error = client
            .call_tool("after-timeout", serde_json::json!({}))
            .await
            .unwrap_err()
            .to_string();
        assert!(retry_error.contains("transport is unavailable"));
        assert!(
            retry_started.elapsed() < std::time::Duration::from_secs(1),
            "a timed-out transport must fail closed instead of waiting for or consuming a late frame"
        );
    }

    #[tokio::test]
    async fn pre_dispatch_guard_drop_stays_not_attempted_and_finishes_receipt() {
        let script = r#"
import json, sys
for line in sys.stdin:
    message = json.loads(line)
    if message.get('method') == 'initialize':
        print(json.dumps({'jsonrpc':'2.0','id':message['id'],'result':{'protocolVersion':'2024-11-05','capabilities':{}}}), flush=True)
"#;
        let client = McpClient::new_with_limits(
            "python3",
            &["-u", "-c", script],
            &HashMap::new(),
            test_limits(),
        )
        .await
        .unwrap();
        let tracker = crate::tool_execution_receipt::ToolExecutionReceiptTracker::new(
            Some("run-pre-dispatch-drop".into()),
            Some("mcp:test:pre-dispatch".into()),
            "request-digest-pre-dispatch".into(),
            crate::tool_execution_receipt::ToolActionEffect::ReadOnly,
            ToolIdempotencyContract::Idempotent,
        );

        {
            let mut session = client.session.lock().await;
            let guard = McpInFlightRequest::begin(&mut session, Some(tracker.clone())).unwrap();
            drop(guard);
            assert_eq!(session.transport_state, McpTransportState::Poisoned);
        }

        let receipt = tracker.snapshot();
        assert_eq!(
            receipt.transport_status,
            crate::tool_execution_receipt::ToolTransportStatus::NotAttempted
        );
        assert_eq!(
            receipt.effect_status,
            crate::tool_execution_receipt::ToolEffectStatus::NotAttempted
        );
        assert!(receipt.dispatched_at.is_none());
        assert!(receipt.finished_at.is_some());
    }

    #[tokio::test]
    async fn caller_cancellation_poisons_transport_before_a_late_response_can_be_reused() {
        let marker_dir = tempfile::tempdir().unwrap();
        let marker = marker_dir.path().join("request-observed");
        let script = r#"
import json, os, sys, time
for line in sys.stdin:
    message = json.loads(line)
    method = message.get('method')
    if method == 'initialize':
        print(json.dumps({'jsonrpc':'2.0','id':message['id'],'result':{'protocolVersion':'2024-11-05','capabilities':{}}}), flush=True)
    elif method == 'tools/call':
        with open(os.environ['MCP_TEST_MARKER'], 'w', encoding='utf-8') as marker:
            marker.write(str(message['id']))
        time.sleep(1)
        print(json.dumps({'jsonrpc':'2.0','id':message['id'],'result':{'content':[{'type':'text','text':'late'}]}}), flush=True)
"#;
        let mut env = HashMap::new();
        env.insert(
            "MCP_TEST_MARKER".to_string(),
            marker.to_string_lossy().into_owned(),
        );
        let limits = McpClientLimits {
            call_timeout: std::time::Duration::from_secs(5),
            ..test_limits()
        };
        let client = McpClient::new_with_limits("python3", &["-u", "-c", script], &env, limits)
            .await
            .unwrap();
        let receipt_tracker = crate::tool_execution_receipt::ToolExecutionReceiptTracker::new(
            Some("run-cancelled-mcp".into()),
            Some("mcp:test:slow".into()),
            "request-digest-cancelled".into(),
            crate::tool_execution_receipt::ToolActionEffect::ExternalMutation,
            ToolIdempotencyContract::NonIdempotent,
        );
        let call_client = client.clone();
        let call_receipt_tracker = receipt_tracker.clone();
        let call = tokio::spawn(async move {
            call_client
                .call_tool_with_receipt_tracker(
                    "slow",
                    serde_json::json!({}),
                    call_receipt_tracker,
                    None,
                )
                .await
        });

        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while !marker.exists() {
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("the server observes the request before the caller cancels it");
        call.abort();
        assert!(call.await.unwrap_err().is_cancelled());
        let receipt = receipt_tracker.snapshot();
        assert_eq!(
            receipt.transport_status,
            crate::tool_execution_receipt::ToolTransportStatus::RemoteUnknown
        );
        assert_eq!(
            receipt.effect_status,
            crate::tool_execution_receipt::ToolEffectStatus::Unknown
        );
        assert!(receipt.dispatched_at.is_some());
        assert!(receipt.finished_at.is_some());

        let error = client
            .call_tool("next", serde_json::json!({}))
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("transport is unavailable"));
    }

    #[tokio::test]
    async fn oversized_mcp_frame_is_rejected_during_handshake() {
        let script = r#"
import json, sys
message = json.loads(sys.stdin.readline())
print(json.dumps({'jsonrpc':'2.0','id':message['id'],'result':{'padding':'x' * 8192}}), flush=True)
"#;
        let error = McpClient::new_with_limits(
            "python3",
            &["-u", "-c", script],
            &HashMap::new(),
            test_limits(),
        )
        .await
        .err()
        .expect("oversized frame must fail")
        .to_string();
        assert!(error.contains("handshake failed"));
    }
}
