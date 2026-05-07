use crate::privacy::PrivacyEngine;
use crate::tool_manifest::{ToolManifest, ToolSource};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

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

/// MCP Client using Stdio transport
pub struct McpClient {
    child: Arc<Mutex<Child>>,
    request_id: Arc<Mutex<u64>>,
    pub command: String,
    pub args: Vec<String>,
}

impl McpClient {
    /// Start an MCP server subprocess and create a client
    pub fn new(command: &str, args: &[&str], env: &HashMap<String, String>) -> Result<Self> {
        let mut cmd = Command::new(command);
        cmd.args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        for (k, v) in env {
            cmd.env(k, v);
        }
        let mut child = cmd
            .spawn()
            .with_context(|| format!("failed to spawn MCP server: {}", command))?;

        // Initialize the server
        {
            let stdin = child.stdin.as_mut().context("failed to get stdin")?;
            let init_req = JsonRpcRequest {
                jsonrpc: "2.0".into(),
                id: 0,
                method: "initialize".into(),
                params: Some(serde_json::json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {},
                    "clientInfo": { "name": "openlife", "version": "0.1.0" }
                })),
            };
            Self::send_request_raw(stdin, &init_req)?;
        }

        // Read initialization response (consume it)
        {
            let stdout = child.stdout.as_mut().context("failed to get stdout")?;
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();
            reader
                .read_line(&mut line)
                .context("failed to read init response")?;
            // We could parse and validate, but for now we just consume
        }

        Ok(Self {
            child: Arc::new(Mutex::new(child)),
            request_id: Arc::new(Mutex::new(1)),
            command: command.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
        })
    }

    fn send_request_raw(stdin: &mut std::process::ChildStdin, req: &JsonRpcRequest) -> Result<()> {
        let json = serde_json::to_string(req)?;
        writeln!(stdin, "{}", json)?;
        stdin.flush()?;
        Ok(())
    }

    fn next_id(&self) -> u64 {
        let mut id = self.request_id.lock().unwrap();
        let current = *id;
        *id += 1;
        current
    }

    /// List available tools from the MCP server
    pub fn list_tools(&self) -> Result<Vec<Tool>> {
        let mut child = self
            .child
            .lock()
            .map_err(|e| anyhow::anyhow!("mcp child mutex poison: {}", e))?;
        let id = self.next_id();

        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id,
            method: "tools/list".into(),
            params: None,
        };

        let stdin = child.stdin.as_mut().context("stdin unavailable")?;
        Self::send_request_raw(stdin, &req)?;

        let stdout = child.stdout.as_mut().context("stdout unavailable")?;
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .context("failed to read response")?;

        let resp: JsonRpcResponse = serde_json::from_str(&line)
            .with_context(|| format!("invalid JSON response: {}", line))?;

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

    /// Call a tool with the given arguments
    pub fn call_tool(&self, name: &str, arguments: Value) -> Result<String> {
        let mut child = self
            .child
            .lock()
            .map_err(|e| anyhow::anyhow!("mcp child mutex poison: {}", e))?;
        let id = self.next_id();

        let req = JsonRpcRequest {
            jsonrpc: "2.0".into(),
            id,
            method: "tools/call".into(),
            params: Some(serde_json::json!({
                "name": name,
                "arguments": arguments
            })),
        };

        let stdin = child.stdin.as_mut().context("stdin unavailable")?;
        Self::send_request_raw(stdin, &req)?;

        let stdout = child.stdout.as_mut().context("stdout unavailable")?;
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .context("failed to read response")?;

        let resp: JsonRpcResponse = serde_json::from_str(&line)
            .with_context(|| format!("invalid JSON response: {}", line))?;

        if let Some(err) = resp.error {
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

        Ok(content)
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
        }
    }
}

pub type BuiltinFn = Box<dyn Fn(Value) -> Result<String> + Send + Sync>;

/// Registry for multiple MCP clients and built-in tools
pub struct McpRegistry {
    clients: HashMap<String, McpClient>,
    tools_cache: Vec<Tool>,
    privacy_engine: PrivacyEngine,
    builtins: HashMap<String, BuiltinFn>,
    builtin_manifests: Vec<ToolManifest>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct McpServerInfo {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub tool_count: usize,
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

impl McpRegistry {
    pub fn new() -> Self {
        let mut reg = Self {
            clients: HashMap::new(),
            tools_cache: Vec::new(),
            privacy_engine: PrivacyEngine::new(),
            builtins: HashMap::new(),
            builtin_manifests: Vec::new(),
        };
        reg.register_default_builtins();
        reg
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
        );

        self.register_core_os_tool(
            "tool.list_available",
            "列出所有已注册且可用的工具",
            "low",
            vec!["read".into()],
            "read",
        );

        self.register_core_os_tool(
            "goal.read",
            "读取当前 Goals 和 Daily Goals",
            "low",
            vec!["read".into(), "lifemodel".into()],
            "read",
        );

        self.register_core_os_tool(
            "state.read",
            "读取当前 State（情绪、健康、焦点、习惯等）",
            "low",
            vec!["read".into(), "lifemodel".into()],
            "read",
        );

        self.register_core_os_tool(
            "memory.search",
            "搜索向量记忆库，返回相关记忆片段",
            "low",
            vec!["read".into(), "memory".into()],
            "read",
        );

        self.register_core_os_tool(
            "proposal.list",
            "列出当前待处理的 Proposal",
            "low",
            vec!["read".into()],
            "read",
        );

        self.register_core_os_tool(
            "agent_run.lookup",
            "按 ID 查询 AgentRun 执行记录",
            "low",
            vec!["read".into()],
            "read",
        );

        // Permission tools: let the agent inspect and request tool permissions.
        self.register_core_os_tool(
            "permission.check",
            "查询指定工具当前的权限状态（允许/阻断/需确认及原因）",
            "low",
            vec!["read".into()],
            "read",
        );

        self.register_core_os_tool(
            "permission.request",
            "为指定工具请求权限（生成 ToolPermission Proposal 供用户审批）",
            "medium",
            vec!["read".into()],
            "read",
        );

        self.register_core_os_tool(
            "permission.replay_action",
            "在权限已授权后重放之前被阻断的工具操作",
            "medium",
            vec!["write".into()],
            "write",
        );

        // snapshot.create is declarative-only in Beta: use Version Control page instead
        self.register_declarative_stub(
            "snapshot.create",
            "创建快照（Beta declarative-only：请使用 Version Control 页面手动创建）",
        );

        // Core OS Tools: Write (Proposal-First)
        self.register_core_os_tool(
            "life_model.propose_patch",
            "提议修改 LifeModel（生成 Proposal，不直接写入）",
            "high",
            vec!["write".into(), "lifemodel".into()],
            "write",
        );

        self.register_core_os_tool(
            "memory.propose_write",
            "提议写入记忆（生成 Proposal，不直接写入）",
            "medium",
            vec!["write".into(), "memory".into()],
            "write",
        );

        self.register_core_os_tool(
            "memory.propose_archive",
            "提议归档记忆（生成 Proposal，不直接归档）",
            "medium",
            vec!["write".into(), "memory".into()],
            "write",
        );

        // Execution Tools: P1 (file, web)
        self.register_execution_tool(
            "file.read",
            "读取指定路径的文件内容（仅限 safe_paths）",
            "low",
            vec!["read".into(), "filesystem".into()],
            "read",
        );

        self.register_execution_tool(
            "file.write_proposal",
            "提议写入文件（生成 ExternalWriteAction Proposal，不直接写入）",
            "high",
            vec!["write".into(), "filesystem".into()],
            "write",
        );

        self.register_execution_tool(
            "web.fetch",
            "获取指定 URL 的内容",
            "medium",
            vec!["network".into()],
            "network",
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
        );

        // Execution Tools: P1 (real executors)
        self.register_execution_tool(
            "calendar.propose_event",
            "提议创建日历事件并生成 ICS 文件",
            "medium",
            vec!["write".into()],
            "write",
        );

        // email.read remains P2 (requires IMAP config)
        self.register_declarative_stub(
            "email.read",
            "读取邮件（Beta stub：需要配置 IMAP account）",
        );

        self.register_execution_tool(
            "email.propose_draft",
            "提议邮件草稿并通过系统邮件客户端打开",
            "medium",
            vec!["write".into()],
            "write",
        );

        // P1 task.create_proposal: creates real local tasks via TaskStore
        self.register_execution_tool(
            "task.create_proposal",
            "创建本地任务/提醒/待办事项（P1：持久化到本地 TaskStore）",
            "medium",
            vec!["write".into()],
            "write",
        );

        // A2A: now P1 with real A2AClient executor
        self.register_execution_tool(
            "a2a.call_agent",
            "调用外部 A2A Agent（30s超时+私网拦截）",
            "medium",
            vec!["write".into(), "network".into()],
            "write",
        );

        // P9: shell.run — default-off, declarative-only, high-risk.
        // No executor yet. Must be explicitly enabled via sandbox + AgentSpec.
        self.register_builtin(
            ToolManifest {
                id: "shell.run".into(),
                name: "shell.run".into(),
                description: "在 ExecutionSandbox 治理下执行非交互式结构化命令".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "command": { "type": "string", "description": "要执行的命令（不含参数）" },
                        "args": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "命令参数列表"
                        },
                        "cwd": { "type": "string", "description": "工作目录" },
                        "env": {
                            "type": "object",
                            "additionalProperties": { "type": "string" },
                            "description": "环境变量名-值映射"
                        },
                        "reason": { "type": "string", "description": "可选：执行原因" }
                    },
                    "required": ["command"]
                }),
                permission_level: "high".into(),
                risk_level: "high".into(),
                version: "1.0.0".into(),
                source: ToolSource::BuiltIn,
                capabilities: vec![
                    "write".into(),
                    "filesystem".into(),
                    "external_side_effect".into(),
                ],
                requires_confirmation: true,
                enabled: false,
                declarative_only: true,
                action_type: "external_side_effect".into(),
                tags: vec!["shell".into(), "execution".into(), "p9".into()],
            },
            Box::new(|_args| {
                Err(anyhow::anyhow!(
                    "shell.run is declarative-only and cannot execute yet"
                ))
            }),
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
    ) {
        let manifest = ToolManifest {
            id: id.into(),
            name: id.into(),
            description: description.into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {}
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
            tags: vec!["core_os".into()],
        };
        let id_owned = id.to_string();
        self.register_builtin(
            manifest,
            Box::new(move |_args| {
                Ok(format!(
                    "Core OS tool '{}' executed (Beta MVP stub)",
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
            tags: vec!["execution".into()],
        };
        let id_owned = id.to_string();
        self.register_builtin(
            manifest,
            Box::new(move |_args| {
                Ok(format!(
                    "Execution tool '{}' executed (Beta MVP stub)",
                    id_owned
                ))
            }),
        );
    }

    /// Helper to register a declarative-only stub tool.
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
            tags: vec!["execution".into(), "stub".into()],
        };
        self.register_builtin(
            manifest,
            Box::new(move |_args| {
                Ok("This tool is a declarative-only stub for Beta. Configure the appropriate provider to enable it.".to_string())
            }),
        );
    }

    /// Register a built-in tool with its manifest.
    pub fn register_builtin(&mut self, manifest: ToolManifest, func: BuiltinFn) {
        self.builtins.insert(manifest.name.clone(), func);
        self.builtin_manifests.push(manifest);
    }

    /// Remove built-in tools by source (e.g., remove all plugin tools).
    pub fn remove_builtins_by_source(&mut self, source_filter: impl Fn(&ToolSource) -> bool) {
        let names_to_remove: Vec<String> = self
            .builtin_manifests
            .iter()
            .filter(|m| source_filter(&m.source))
            .map(|m| m.name.clone())
            .collect();
        for name in &names_to_remove {
            self.builtins.remove(name);
        }
        self.builtin_manifests.retain(|m| !source_filter(&m.source));
    }

    /// Register and start an MCP server
    pub fn register(&mut self, name: &str, command: &str, args: &[&str]) -> Result<()> {
        self.register_with_env(name, command, args, &HashMap::new())
    }

    /// Register and start an MCP server with environment variables.
    pub fn register_with_env(
        &mut self,
        name: &str,
        command: &str,
        args: &[&str],
        env: &HashMap<String, String>,
    ) -> Result<()> {
        let client = McpClient::new(command, args, env)?;
        let tools = client.list_tools().unwrap_or_default();
        self.tools_cache.extend(tools);
        self.clients.insert(name.to_string(), client);
        Ok(())
    }

    /// Unregister an MCP server
    pub fn unregister(&mut self, name: &str) -> Result<()> {
        let removed = self.clients.remove(name);
        if removed.is_none() {
            return Err(anyhow::anyhow!("server '{}' not found", name));
        }
        // rebuild tools cache
        self.tools_cache.clear();
        for client in self.clients.values() {
            let tools = client.list_tools().unwrap_or_default();
            self.tools_cache.extend(tools);
        }
        Ok(())
    }

    /// List registered servers with metadata
    pub fn list_servers(&self) -> Vec<McpServerInfo> {
        self.clients
            .iter()
            .map(|(name, client)| McpServerInfo {
                name: name.clone(),
                command: client.command.clone(),
                args: client.args.clone(),
                tool_count: client.list_tools().unwrap_or_default().len(),
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
        for (server_name, client) in &self.clients {
            if let Ok(tools) = client.list_tools() {
                for tool in tools {
                    out.push(
                        ToolManifest {
                            id: format!("mcp:{}:{}", server_name, tool.name),
                            name: tool.name.clone(),
                            description: tool.description.clone(),
                            parameters: tool.parameters.clone(),
                            permission_level: ToolManifest::infer_permission_level(&tool.name),
                            risk_level: ToolManifest::infer_permission_level(&tool.name),
                            version: "1.0.0".into(),
                            source: ToolSource::Mcp {
                                server_name: server_name.clone(),
                            },
                            capabilities: ToolManifest::infer_capabilities(&tool.name),
                            requires_confirmation: ToolManifest::infer_permission_level(&tool.name)
                                == "high",
                            enabled: true,
                            declarative_only: false,
                            action_type: ToolManifest::infer_action_type(&tool.name),
                            tags: Vec::new(),
                        }
                        .normalized(),
                    );
                }
            }
        }
        out
    }

    /// Execute a manifest by its source.
    pub fn execute_manifest(&self, manifest: &ToolManifest, arguments: Value) -> Result<String> {
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
            ToolSource::Mcp { server_name } => {
                self.call_tool_on_server(server_name, &manifest.name, arguments)
            }
            ToolSource::A2A { .. } => Err(anyhow::anyhow!("A2A tool execution is not wired yet")),
            ToolSource::Plugin { plugin_id } => Err(anyhow::anyhow!(
                "Plugin tool '{}' from '{}' is declarative-only and not executable in this Beta",
                manifest.name,
                plugin_id
            )),
        }
    }

    /// Call a tool by name (searches all registered servers).
    /// Arguments are desensitized before sending and reconstructed after receiving the result.
    pub fn call_tool(&self, name: &str, arguments: Value) -> Result<String> {
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

        let mut last_error: Option<anyhow::Error> = None;
        for client in self.clients.values() {
            match client.call_tool(name, desensitized_args.clone()) {
                Ok(result) => {
                    // 2. Reconstruct any placeholders in the result
                    let final_result = if map.is_empty() {
                        result
                    } else {
                        self.privacy_engine.reconstruct(&result, &map)
                    };
                    return Ok(final_result);
                }
                Err(e) => {
                    let msg = e.to_string();
                    if msg.contains("not found") || msg.contains("Unknown tool") {
                        last_error = Some(e);
                        continue;
                    }
                    return Err(e);
                }
            }
        }
        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("tool {} not found", name)))
    }

    /// Call a tool on a specific MCP server by name.
    /// Requires the server to be registered. Returns error if server or tool is not found.
    pub fn call_tool_on_server(
        &self,
        server_name: &str,
        name: &str,
        arguments: Value,
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
        let result = client.call_tool(name, desensitized_args)?;

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
        scored.sort_by(|a, b| b.0.cmp(&a.0));
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

    // ── P9-2: shell.run manifest tests ─────────────────────────────────

    #[test]
    fn test_shell_run_is_high_risk() {
        let registry = McpRegistry::new();
        let manifest = registry
            .list_manifests()
            .into_iter()
            .find(|m| m.name == "shell.run")
            .expect("shell.run manifest should be registered");
        assert_eq!(manifest.permission_level, "high");
        assert_eq!(manifest.risk_level, "high");
        assert!(manifest.requires_confirmation);
    }

    #[test]
    fn test_shell_run_default_not_model_callable() {
        let registry = McpRegistry::new();
        let manifest = registry
            .list_manifests()
            .into_iter()
            .find(|m| m.name == "shell.run")
            .unwrap();
        assert!(!manifest.enabled, "shell.run must be disabled by default");
        assert!(
            manifest.declarative_only,
            "shell.run must be declarative-only by default"
        );
        // Not in model-callable tools prompt
        let prompt = registry.tools_prompt();
        assert!(
            !prompt.contains("shell.run"),
            "shell.run must not appear in tools prompt"
        );
    }

    #[test]
    fn test_shell_run_blocked_by_declarative_only() {
        let registry = McpRegistry::new();
        let manifest = registry
            .list_manifests()
            .into_iter()
            .find(|m| m.name == "shell.run")
            .unwrap();
        assert!(manifest.declarative_only);
        // Declarative-only tools are filtered from is_model_callable
        let prompt = registry.tools_prompt();
        assert!(!prompt.contains("shell.run"));
    }

    #[test]
    fn test_shell_run_not_executable_yet() {
        let registry = McpRegistry::new();
        let manifest = registry
            .list_manifests()
            .into_iter()
            .find(|m| m.name == "shell.run")
            .unwrap();
        assert!(!manifest.enabled);
        assert!(manifest.declarative_only);
        // Even if we try to execute via manifest, it will be blocked by
        // declarative_only check in execute_manifest or return error.
    }
}
