# OpenLife 工程结构治理记录

> 目标：在开始试用前，让后端命令边界更清楚，降低后续 Debug 和 Agent 接手成本。

## 当前拆分状态

### 已拆出模块

| 模块 | 文件 | 职责 |
| --- | --- | --- |
| 本地存储路径与小文件 | `src-tauri/src/storage.rs` | app data dir、privacy policy、onboarding 状态、MCP audit keyring 的读写 |
| Settings 命令 | `src-tauri/src/commands/settings.rs` | 配置读写、数据导入导出、API Key 测试、MCP audit 导出/清理/轮换、隐私策略、onboarding |
| Memory 命令 | `src-tauri/src/commands/memory.rs` | 记忆索引/搜索、tier maintenance、hot cache、归档/恢复、tier stats |

### 仍留在 `lib.rs` 的主要职责

- `AppState` 和 Tauri app 初始化。
- Chat 主链路：消息预处理、AgentRuntime/ReasoningStrategy、stream、工具调用。
- LifeModel/Version/Dashboard 基础命令。
- Builder、Calibration、Evolution、A2A/MCP 注册等领域命令。

## 后续拆分路线

### 1. 优先拆 Chat 命令

建议新建：

- `src-tauri/src/commands/chat.rs`
- `src-tauri/src/chat_pipeline.rs`

拆分目标：

- `send_message`
- `start_stream_message`
- `get_chat_history`
- `save_chat_message`
- `list_chat_sessions`
- `create_chat_session`
- `rename_chat_session`
- `delete_chat_session`
- `preprocess_chat_input`
- `capture_conversation_signals`

注意：Chat 当前耦合 `persist_life_model`、vector search、MCP audit、AgentRuntime、feedback signals。拆分时不要一次移动所有 helper，先把 command 外壳移走，再移动 pipeline。

### 2. 再拆 Builder / Calibration

建议新建：

- `src-tauri/src/commands/builder.rs`
- `src-tauri/src/commands/calibration.rs`

拆分目标：

- Builder session start/step/list/delete。
- 4D completion。
- Micro evolution。
- Calibration report/apply/mark shown。

注意：这组会修改 LifeModel，必须继续复用 `persist_life_model`，不要绕开快照与 versioning 准备逻辑。

### 3. 最后拆 MCP / A2A

建议新建：

- `src-tauri/src/commands/mcp.rs`
- `src-tauri/src/commands/a2a.rs`

拆分目标：

- MCP server register/unregister/list/tools/templates/recommend/audit list/clear。
- A2A discover/send/local card/bridge/sidecar restart/stop。

注意：MCP 高风险工具执行确认流是安全边界，拆分时必须保留 `inspect_mcp_call` 与 `execute_tool_call_internal` 的关系。

## Agent 开发约束

- 新增 Tauri command 时，优先放入对应 `commands/*` 模块，不要继续堆到 `lib.rs`。
- 涉及本地文件路径或状态文件时，优先复用 `storage.rs`。
- 涉及 LifeModel 保存时，必须走 `persist_life_model`。
- 每次拆分后至少跑 `cargo test -q`。
- 如果改了前端 API 或页面契约，还要跑 `cd frontend && npm test -- --run` 和 `cd frontend && npm run build`。

## 当前试用前建议

在继续堆功能前，建议再做 1-2 轮小型治理：

1. 拆出 Chat command 外壳，保留 pipeline helper 在 `lib.rs` 或 `chat_pipeline.rs`。
2. 拆出 Builder/Calibration command，确认 Builder -> Dashboard -> Chat -> Calibration 的试用路径仍然全绿。
3. 更新 README 的“试用前检查”，让非开发者知道 Ollama/API Key/数据目录怎么确认。
