# OpenLife - AI 助手上下文指南

> 本文档面向 AI Agent 和开发协作者，提供快速理解项目所需的一切上下文信息。

---

## 📋 项目概览

- **项目类型**：桌面端 AI 伴侣应用（Tauri 桌面壳 + React 前端 + Rust 核心引擎）
- **技术栈**：Rust (Tauri 2.x + 自定义核心库) + React 18 + TypeScript + Tailwind CSS + SQLite
- **主要功能**：OpenLife 定位为用户的"终身成长合伙人"，通过四维人生模型（Identity / Goals / Capabilities / State）管理用户成长，整合本地（Ollama）和云端（OpenRouter/OpenAI）大模型进行对话，支持 MCP 工具调用与 A2A Agent 互联。
- **仓库链接**：（需要人工补充）

---

## 🏗️ 架构说明

### 目录结构

```
.
├── Cargo.toml                    # Rust Workspace 定义
├── README.md                     # 用户面向文档
├── AGENTS.md                     # 本文档 ← AI 助手上下文
├── .env.template                 # 环境变量模板
├── .env.example                  # 环境变量完整示例
├── Makefile                      # 跨平台快捷命令
├── setup.sh / setup.ps1          # 环境初始化脚本
├── dev.sh / dev.ps1              # 快速开发启动
├── start.sh / start.ps1          # 生产构建
├── startup.sh / startup.ps1      # 一体化工具（dev / a2a / check）
│
├── frontend/                     # React 前端（Vite 构建）
│   ├── package.json              # pnpm 依赖
│   ├── vite.config.ts            # Vite 配置 (port 5173)
│   ├── vitest.config.ts          # 测试配置 (Vitest + jsdom)
│   ├── tailwind.config.js        # Tailwind CSS 配置
│   └── src/
│       ├── main.tsx              # 入口 (HashRouter)
│       ├── App.tsx               # 路由 + 导航 + ErrorBoundary
│       ├── tauri.ts              # ⭐ Tauri Command 封装层
│       ├── types.ts              # 全局类型定义
│       ├── index.css             # Tailwind 导入
│       ├── test/
│       │   ├── setup.ts          # 测试初始化 (mock ResizeObserver 等)
│       │   └── mocks/tauri.ts    # Tauri invoke mock（约 30+ 命令）
│       ├── components/           # 通用组件
│       │   ├── HermesTracePanel.tsx
│       │   ├── ToolCallCard.tsx
│       │   └── LoadingSpinner.tsx
│       └── pages/                # 页面组件
│           ├── ChatPage.tsx
│           ├── DashboardPage.tsx
│           ├── LifeModelEditor.tsx
│           ├── BuilderPage.tsx
│           ├── A2APage.tsx
│           ├── McpPage.tsx
│           ├── MemorySearch.tsx
│           ├── VersionControl.tsx
│           ├── SettingsPage.tsx
│           └── CalibrationPage.tsx
│
├── openlife-core/                # Rust 核心业务库
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs                # 模块暴露入口
│       ├── config.rs             # AppConfig (YAML + env 覆盖)
│       ├── life_model.rs         # 四维人生模型（Identity/Goals/Capabilities/State）
│       ├── llm.rs                # OpenRouter / OpenAI API 调用
│       ├── ollama.rs             # Ollama 本地模型调用
│       ├── scheduler.rs          # 推理调度器（本地优先策略）
│       ├── hermes.rs             # 三层决策总线 (Meaning→Strategy→Execution)
│       ├── memory.rs             # SQLite 消息/会话/状态存储
│       ├── vectors.rs            # 向量记忆 Tier 3（SQLite + 本地 embedding）
│       ├── router.rs             # 意图路由
│       ├── layer_router.rs       # 层路由
│       ├── mcp.rs                # MCP 客户端（JSON-RPC stdio）
│       ├── mcp_audit.rs          # MCP 调用审计
│       ├── a2a.rs                # A2A 协议实现
│       ├── privacy.rs            # PII 检测与脱敏
│       ├── versioning.rs         # Git-like 快照与回滚
│       ├── feedback.rs           # 用户反馈与进化信号
│       ├── builder.rs            # 构建模式（引导式人生模型创建）
│       ├── reflex_engine.rs      # 反射引擎
│       ├── evolution.rs          # 微进化系统
│       └── tool_manifest.rs      # 工具清单定义
│
├── src-tauri/                    # Tauri 应用壳
│   ├── Cargo.toml                # Tauri 依赖 + bin 定义
│   ├── tauri.conf.json           # Tauri 配置（identifier: ai.openlife.app）
│   ├── capabilities/default.json # Tauri 权限声明
│   └── src/
│       ├── main.rs               # 桌面应用入口
│       ├── lib.rs                # ⭐ 核心注册地（~1380 行，共享类型/辅助函数/send_message/start_stream_message）
│       ├── a2a_server.rs         # A2A 服务器模块
│       ├── a2a_sidecar.rs        # A2A 侧车进程管理
│       ├── commands/             # 67+ 命令按领域拆分为 13 个模块
│       │   ├── mod.rs            # 模块声明入口
│       │   ├── a2a.rs            # 7 个 A2A 命令
│       │   ├── builder.rs        # 11 个 Builder 命令
│       │   ├── calibration.rs    # 6 个 Calibration 命令
│       │   ├── chat.rs           # 6 个 Chat 会话命令
│       │   ├── diagnostics.rs    # 5 个诊断命令
│       │   ├── feedback.rs       # 5 个反馈命令
│       │   ├── hermes.rs         # 1 个 Hermes 命令
│       │   ├── life_model.rs     # 2 个 LifeModel 命令
│       │   ├── mcp.rs            # 8 个 MCP 命令
│       │   ├── memory.rs         # 10 个 Memory 命令
│       │   ├── settings.rs       # 12 个 Settings 命令
│       │   ├── state.rs          # 8 个 State 命令
│       │   └── version.rs        # 4 个版本命令
│       └── bin/
│           └── a2a_server.rs     # 独立 A2A HTTP 服务器（Axum，port 8765）
│
├── scripts/
│   └── quantize_int8.py          # ONNX INT8 量化工具
│
└── plans/                        # 项目规划文档
    ├── openlife_development_plan.md
    ├── openlife_codex_execution_playbook.md
    └── sprint_7_8_9_plan.md
```

### 核心模块

| 模块 | 文件路径 | 职责 | 依赖关系 |
|------|----------|------|----------|
| **LifeModel** | [`openlife-core/src/life_model.rs`](openlife-core/src/life_model.rs) | 四维人生模型：Identity（身份/价值观）、Goals（短中长期目标）、Capabilities（技能/资源）、State（当前状态/情绪/健康） | 被 hermes.rs、scheduler.rs、memory.rs 消费 |
| **Hermes Bus** | [`openlife-core/src/hermes.rs`](openlife-core/src/hermes.rs) | 三层决策总线：MeaningNode（语义理解/禁忌检测）→ StrategyNode（策略规划）→ ExecutionNode（执行生成），Arbitrator 仲裁最终输出 | 依赖 scheduler.rs、life_model.rs |
| **InferenceScheduler** | [`openlife-core/src/scheduler.rs`](openlife-core/src/scheduler.rs) | 智能调度云端/本地模型：tool prompt → 强制云端；Ollama 可用 + prefer_local → 本地；否则 fallback 云端 | 依赖 llm.rs、ollama.rs |
| **MemoryStore** | [`openlife-core/src/memory.rs`](openlife-core/src/memory.rs) | SQLite 持久化：聊天记录、会话管理、人生模型快照、状态历史、自定义记忆记录 | 独立，被 lib.rs 调用 |
| **VectorStore** | [`openlife-core/src/vectors.rs`](openlife-core/src/vectors.rs) | 向量记忆 Tier 3：存储 embedding，支持余弦相似度检索、session 过滤、tier 升降维护 | 依赖 tract-onnx/tokenizers 做本地 embedding |
| **McpRegistry** | [`openlife-core/src/mcp.rs`](openlife-core/src/mcp.rs) | MCP 客户端管理：注册/注销服务器、list_tools、call_tool、内置工具、参数隐私检查 | 依赖 privacy.rs、tool_manifest.rs |
| **Tauri Commands** | [`src-tauri/src/lib.rs`](src-tauri/src/lib.rs) | 30+ 个 `#[tauri::command]`：聊天、MCP、A2A、记忆、版本控制、Builder、进化、校准、系统诊断 | 依赖 openlife-core 全部模块 |
| **Frontend API** | [`frontend/src/tauri.ts`](frontend/src/tauri.ts) | TypeScript 封装层：所有后端调用的唯一入口，约 40+ 个 invoke 函数 | 仅依赖 `@tauri-apps/api/core` |

### 数据流

```
用户输入（ChatPage.tsx）
    │
    ▼
[frontend/src/tauri.ts] ──invoke──► [src-tauri/src/lib.rs]
    │                                    │
    │    ┌───────────────────────────────┼───────────────────────────────┐
    │    ▼                               ▼                               ▼
[Stream UI]                    [Hermes Bus]                      [MemoryStore]
(逐字显示)               Meaning→Strategy→Execution         (保存消息/快照)
    │                            │
    │                    [InferenceScheduler]
    │                    ┌────────┴────────┐
    │                    ▼                 ▼
    │              [Ollama]          [OpenRouter]
    │            (localhost)         (云端 API)
    │                    │
    │                    ▼
    │              [VectorStore] ← 嵌入检索上下文
    │                    │
    └────────────────────┘
                         │
                    [McpRegistry] ← 工具调用（如有）
                         │
                    [PrivacyEngine] ← PII 检测/脱敏
```

关键处理节点：
1. **输入预处理**（[`preprocess_chat_input`](src-tauri/src/lib.rs:391)）：用户消息 → 向量检索相关记忆 → Hermes 请求构建
2. **三层决策**（[`HermesBus::dispatch`](openlife-core/src/hermes.rs:122)）：Meaning（语义理解）→ Strategy（JSON 策略）→ Execution（最终回复）
3. **模型调度**（[`InferenceScheduler::generate`](openlife-core/src/scheduler.rs:71)）：根据 tool prompt 和 Ollama 可用性决定使用本地或云端模型
4. **工具调用**（[`execute_tool_call_internal`](src-tauri/src/lib.rs:264)）：MCP 工具执行 + 隐私参数脱敏 + 审计日志
5. **流式输出**（[`start_stream_message`](src-tauri/src/lib.rs:822)）：SSE 风格流式传输到前端

---

## 🛠️ 开发规范

### 命名约定

| 范畴 | 约定 | 示例 |
|------|------|------|
| **Rust 文件/目录** | `snake_case` | `life_model.rs`, `mcp_audit.rs` |
| **Rust 结构体/枚举** | `PascalCase` | `LifeModel`, `HermesBus`, `AlertLevel` |
| **Rust 方法/函数** | `snake_case` | `save_message()`, `run_tier_maintenance()` |
| **Rust 常量** | `UPPER_SNAKE_CASE` | `OLLAMA_CACHE_TTL = 10` |
| **Rust 模块** | `snake_case` | `mod memory;` |
| **TS/前端 组件文件** | `PascalCase` | `App.tsx`, `ChatPage.tsx`, `LifeModelEditor.tsx` |
| **TS/前端 工具文件** | `camelCase` | `tauri.ts`, `modelEmpty.ts`, `setup.ts` |
| **TS/前端 组件/类** | `PascalCase` | `ErrorBoundary`, `ToolCallCard` |
| **TS/前端 函数/方法** | `camelCase` | `sendMessage()`, `safeInvoke()` |
| **TS/前端 接口/类型** | `PascalCase` | `AppConfig`, `ChatMessage`, `SendMessageResult` |
| **TS/前端 常量** | `UPPER_SNAKE_CASE` | （需要人工补充具体命名） |
| **YAML 配置键** | `snake_case` | `prefer_local_model`, `openai_base` |
| **数据库表名** | `snake_case` 复数 | `messages`, `chat_sessions`, `vector_chunks` |

### 代码风格

| 语言 | 项目 | 说明 |
|------|------|------|
| **Rust** | 缩进 | 4 空格 |
| **Rust** | 引号 | 双引号 |
| **Rust** | 行尾分号 | 使用 |
| **Rust** | 最大行宽 | 约 100-120 字符（推断） |
| **Rust** | 注释 | 行内注释 `//`，Doc 注释 `///` |
| **TypeScript** | 缩进 | 2 空格 |
| **TypeScript** | 引号 | 双引号（从 `App.tsx` 观察） |
| **TypeScript** | 行尾分号 | 使用（从 `App.tsx` 观察） |
| **TypeScript** | 最大行宽 | 约 100-120 字符（推断） |
| **TypeScript** | 严格模式 | `tsconfig.json` 启用 `strict: true`，`noUnusedLocals: true`，`noUnusedParameters: true` |
| **CSS** | 类名 | Tailwind 工具类为主，无自定义 BEM |
| **Python** | 缩进 | 4 空格（scripts/quantize_int8.py） |

### Git 提交规范

> ⚠️ 以下内容为合理推测，**需要人工补充**确认实际规范。

- **提交消息格式**：建议采用 [Conventional Commits](https://www.conventionalcommits.org/)（如 `feat:`, `fix:`, `refactor:`, `docs:`）
- **分支命名**：建议 `feature/xxx`, `bugfix/xxx`, `refactor/xxx`
- **PR 流程**：（需要人工补充）

---

## 📐 业务逻辑规则

### 核心业务实体

| 实体 | 关键属性 | 关系 |
|------|----------|------|
| **LifeModel** | `metadata`, `identity`, `goals`, `capabilities`, `state`, `relationships`, `preferences` | 1 个用户 1 个当前 LifeModel；支持快照版本控制 |
| **Identity** | `name`, `values[]`, `personality_traits[]`, `life_philosophy`, `mission_statement`, `role_definition`, `voice_style` | 属于 LifeModel 的子维度 |
| **Goals** | `short_term[]`, `medium_term[]`, `long_term[]`, `life_goals[]`, `daily[]` | 每个 GoalItem 有 `priority`, `progress`, `deadline`, `milestones[]` |
| **State** | `current_focus`, `health_status`, `emotional_state`, `habit_streaks[]`, `custom_dimensions[]`, `alerts[]` | 支持自定义维度 + 阈值预警 |
| **ChatMessage** | `role` (system/user/assistant), `content`, `tool_calls?`, `name?` | 属于 ChatSession；持久化到 SQLite |
| **MemoryChunk** | `session_id`, `content`, `embedding[]`, `source`, `tier`, `access_count` | 向量记忆，tier 1/2/3 分层 |
| **ToolManifest** | `name`, `description`, `source` (builtin/mcp/external), `parameters` | 注册到 McpRegistry |
| **HermesTrace** | `meaning_result`, `strategy_result`, `execution_result`, `arbitration` | 一次对话请求的三层决策痕迹 |

### 状态机和流程

#### 1. Hermes 三层决策流程

```
用户输入
    │
    ▼
┌─────────────┐    语义理解 + 禁忌话题检测
│ MeaningNode │ ──► 输出：user_text, forbidden_topics[]
└─────────────┘
    │
    ▼
┌───────────────┐    策略规划（JSON 输出）
│ StrategyNode  │ ──► 输出：strategy_json（含工具调用意图）
└───────────────┘
    │
    ▼
┌───────────────┐    执行生成（最终回复）
│ ExecutionNode │ ──► 输出：assistant_reply
└───────────────┘
    │
    ▼
┌─────────────┐    仲裁：选择最佳层输出或合并
│ Arbitrator  │ ──► 最终返回给用户
└─────────────┘
```

每层有独立超时（Meaning 30s / Strategy 45s / Execution 60s），失败时由 Arbitrator 决定 fallback。

#### 2. 模型调度策略

```
用户消息
    │
    ▼
是否有 tools_prompt ?
    │
   YES ──► 跳过本地模型，强制使用云端（7B 本地模型工具调用不可靠）
    │
   NO ──► Ollama 可用 && prefer_local=true ?
            │
           YES ──► 使用 Ollama（localhost:11434）
            │
           NO  ──► 有 API Key ?
                    │
                   YES ──► 使用云端（OpenRouter / OpenAI）
                    │
                   NO  ──► 返回"未配置后端"提示
```

#### 3. 记忆检索流程

```
用户输入
    │
    ▼
提取用户消息文本
    │
    ▼
生成 embedding（本地 tract-onnx 或 OpenAI API）
    │
    ▼
VectorStore.search(query_embedding, top_k=5)
    │
    ▼
计算余弦相似度 + tier 加分（tier 1 > tier 2 > tier 3）
    │
    ▼
返回 MemoryChunk[] 作为上下文注入 prompt
    │
    ▼
访问计数 +1（bump_access_for_chunks）
```

#### 4. 每日目标自动打卡流程

[`try_auto_checkin_daily_goals`](src-tauri/src/lib.rs:357) 会在每次 assistant 回复后检查内容中是否提到完成了某个 daily goal 的名称，自动将 `done` 标记为 `true`。

### 业务规则约束

1. **LLM 后端最低要求**：至少配置一个 LLM 后端（Ollama 或 OpenRouter/OpenAI API Key）才能使用对话功能。
2. **工具调用强制云端**：当消息包含 tools_prompt 时，强制使用云端模型，因为 7B 参数的本地模型在工具调用上不可靠。
3. **PII 本地拦截**：所有 outgoing 请求经过 [`PrivacyEngine`](openlife-core/src/privacy.rs) 检测，高敏感度 PII（如身份证号、银行卡号）会阻止发送或脱敏。
4. **消息 checksum**：保存消息到 SQLite 时，根据 `content + session_id + created_at` 生成 SHA256 checksum，用于完整性校验。
5. **Ollama 缓存 10 秒**：`ollama.rs` 每 10 秒缓存一次模型可用性检查，状态变化不会立即反映。
6. **向量记忆 tier 维护**：`vectors.rs` 定期运行 `run_tier_maintenance()`，高频访问 chunk 晋升 tier，低频降级。
7. **HashRouter 强制使用**：前端必须使用 `HashRouter` 而非 `BrowserRouter`，因为 Tauri 桌面应用基于 `file://` 协议。
8. **数据目录硬编码**：应用数据目录在代码中硬编码为 `com.openlife.app`（而非从 `tauri.conf.json` 的 `identifier: ai.openlife.app` 读取），macOS 路径为 `~/Library/Application Support/com.openlife.app/`。

---

## ⚙️ 环境配置

### 必需的环境变量

| 变量名 | 用途 | 示例值 | 是否必须 |
|--------|------|--------|----------|
| `OPENROUTER_API_KEY` | 云端 LLM API Key（优先使用） | `sk-or-v1-xxxxxxxx` | 否（二选一） |
| `OPENAI_API_KEY` | OpenAI API Key（备用） | `sk-xxxxxxxx` | 否（二选一） |
| `OPENAI_API_BASE` | 自定义 API Base URL | `https://api.openai.com/v1` | 否（有默认值） |
| `A2A_PORT` | A2A 独立服务器端口 | `8765` | 否（默认 8765） |
| `PORT` | Vite 开发服务器端口 | `5173` | 否（默认 5173） |
| `TAURI_DEBUG` | Tauri 调试日志开关 | `1` | 否 |

> 至少配置 `OPENROUTER_API_KEY` 或 `OPENAI_API_KEY` 之一才能使用云端模型对话。如果不配置，必须本地运行 Ollama。

### 外部服务依赖

| 服务 | 用途 | 配置位置 | 本地替代方案 |
|------|------|----------|-------------|
| **Ollama** (localhost:11434) | 本地 LLM 推理 | `.env` / `config.yaml` | 无替代，需本地安装 |
| **OpenRouter API** | 云端 LLM（多模型聚合） | `.env` / `config.yaml` | OpenAI API |
| **OpenAI API** | 云端 LLM（官方） | `.env` / `config.yaml` | OpenRouter API |
| **SQLite** | 本地数据持久化 | 自动 bundled | 无需替代，零配置 |

配置优先级：**环境变量 > `config.yaml` > 代码默认值**

运行时配置文件路径（macOS）：`~/Library/Application Support/com.openlife.app/config.yaml`

### 启动和测试命令

#### 开发模式

```bash
# 一键初始化（新开发者首选）
make setup
# 或
./setup.sh              # macOS/Linux
.\setup.ps1             # Windows

# 快速开发启动
make dev
# 或
./dev.sh                # macOS/Linux
.\dev.ps1               # Windows

# 一体化脚本启动
./startup.sh dev        # macOS/Linux
.\startup.ps1 dev       # Windows
```

#### 运行测试

```bash
# 前端测试
make test-front
# 或
cd frontend && pnpm test

# Rust 测试
make test-rust
# 或
cargo test -p openlife-core
cargo test -p openlife-tauri

# 全部测试
make test
```

#### 构建生产版本

```bash
# macOS
make build
# 或
./start.sh
# 或
pnpm tauri build --target universal-apple-darwin

# Windows
.\start.ps1
# 或
pnpm tauri build --target x86_64-pc-windows-msvc

# Linux
./start.sh
# 或
pnpm tauri build --target x86_64-unknown-linux-gnu
```

---

## 🐛 已知问题和注意事项

### 历史遗留问题

1. **reqwest 版本不一致**：`openlife-core` 使用 `reqwest 0.11`，`src-tauri` 使用 `reqwest 0.12`。目前编译通过，但建议统一版本以避免潜在兼容性问题。
2. **Ollama 缓存固定 10 秒**：`ollama.rs` 中 `OLLAMA_CACHE_TTL = 10s` 硬编码。如果用户刚启动 Ollama，需要等缓存过期才能被检测到。
3. **数据目录与 Tauri identifier 不一致**：`tauri.conf.json` 中 `identifier` 是 `ai.openlife.app`，但代码中 `app_data_dir()` 硬编码返回 `com.openlife.app`。这可能导致 Tauri 的某些 API 与手动构造的路径不一致。
4. **MCP 审计日志单独数据库**：`mcp_audit.db` 与 `messages.db`/`vectors.db` 分开存储，这是设计上的隔离，但备份/迁移时容易遗漏。

### 常见陷阱

1. **忘记使用 HashRouter**：如果新开发者习惯性使用 `BrowserRouter`，Tauri 桌面应用在 `file://` 协议下会白屏。
2. **工具调用时 local model 被跳过**：如果配置了 `prefer_local=true` 但消息触发了 tool prompt，会静默切换到云端模型，前端无显式提示。
3. **忘记 bump_access**：手动操作 `VectorStore` 后如果不调用 `bump_access_for_chunks`，tier 维护不会正确晋升高频记忆。
4. **PII 检测导致 MCP 调用失败**：如果工具参数包含被标记为高风险的 PII，`McpRegistry` 会阻止调用，错误信息可能不够明确。
5. **`.env` 修改后需重启**：Tauri dev 不会热重载 `.env` 变更，修改 API Key 后需要重启开发服务器。

### 性能敏感区域

1. **向量检索余弦相似度**：[`cosine_similarity`](openlife-core/src/vectors.rs:285) 在 Rust 中逐对计算，大规模向量库时可能成为瓶颈。目前使用 `f32` 运算，可考虑 SIMD 优化。
2. **embedding 生成**：每次用户输入都调用 embedding API（或本地 ONNX 模型），是延迟的主要来源。可考虑缓存高频查询的 embedding。
3. **Hermes 三层串行调用**：Meaning → Strategy → Execution 是串行的，每层都有 LLM 请求，总延迟 = 三层之和。Strategy 层要求输出合法 JSON，重试逻辑可能增加额外延迟。
4. **SQLite 写入锁**：`MemoryStore` 使用 `Mutex<Connection>`，高并发写入（如同时保存消息 + 向量化 + 审计日志）会串行化。
5. **Ollama 首次加载延迟**：本地模型首次加载到 GPU 内存时可能有数秒延迟，缓存机制只检查可用性，不预热模型。

### 待重构区域

1. **reqwest 版本统一**：将 `openlife-core` 升级到 `reqwest 0.12`。
2. **数据目录统一**：将 `app_data_dir()` 改为从 Tauri 配置读取 identifier，而非硬编码。
3. **Hermes 层并行化**：Meaning 和 Strategy 理论上可以部分并行（如先并行获取 Meaning + 检索向量记忆）。
4. **前端 ErrorBoundary 过于简单**：目前只显示红色背景文本，可以添加重试按钮或错误上报。
5. **核心逻辑测试覆盖**：Rust 测试集中在 config.rs、vectors.rs、builder.rs、versioning.rs，核心逻辑（hermes.rs、scheduler.rs）缺乏单元测试。

---

## 🧪 测试策略

### 测试类型

| 类型 | 工具/框架 | 覆盖范围 | 说明 |
|------|-----------|----------|------|
| **前端单元测试** | Vitest + jsdom + @testing-library/react | `src/**/*.test.{ts,tsx}` | 组件渲染、交互逻辑、Tauri mock 调用验证 |
| **前端覆盖率** | Vitest coverage | `src/**/*.{ts,tsx}` | reporter: text/json/html；排除 `src/test/**/*` |
| **Rust 单元测试** | `cargo test` | `openlife-core/src/**`, `src-tauri/src/**` | `#[cfg(test)]` 模块，使用 `tempfile` 做临时目录 |
| **Rust 集成测试** | （需要人工补充） | 端到端 Tauri 命令测试 | 目前缺乏 |
| **端到端测试** | （需要人工补充） | 完整用户流程 | 目前缺乏 |

### 测试数据

- **前端 Mock**：[`frontend/src/test/mocks/tauri.ts`](frontend/src/test/mocks/tauri.ts) 提供完整的 Tauri `invoke` mock，覆盖约 30+ 个命令。新增 command 时**必须同步更新此 mock**，否则组件测试会失败。
- **Rust 测试数据**：使用 `tempfile::TempDir` 创建临时 SQLite 数据库，每个测试独立隔离。
- **LifeModel 测试数据**：YAML 序列化/反序列化测试使用内存中的字符串，无需外部文件。

### 关键测试文件

| 文件 | 测试内容 |
|------|----------|
| [`openlife-core/src/config.rs`](openlife-core/src/config.rs:96) | YAML 保存/加载往返、默认值、环境变量覆盖 |
| [`openlife-core/src/vectors.rs`](openlife-core/src/vectors.rs:369) | 向量插入/批量插入/检索/tier 维护/导入导出/清空 |
| [`openlife-core/src/memory.rs`](openlife-core/src/memory.rs:702) | 消息保存加载、快照、会话管理、状态历史、迁移兼容 |
| [`openlife-core/src/scheduler.rs`](openlife-core/src/scheduler.rs:191) | 本地/云端调度策略逻辑（纯函数测试） |
| [`openlife-core/src/mcp.rs`](openlife-core/src/mcp.rs:536) | 参数隐私检查（低/中风险分级） |
| [`frontend/src/components/ToolCallCard.test.tsx`](frontend/src/components/ToolCallCard.test.tsx) | 工具调用卡片渲染 |
| [`frontend/src/pages/ChatPage.test.tsx`](frontend/src/pages/ChatPage.test.tsx) | 聊天页面交互 |

---

## 📚 参考资料

### 相关文档

| 文档 | 路径 | 说明 |
|------|------|------|
| 产品需求文档 | [`OpenLife_Final_PRD.md`](OpenLife_Final_PRD.md) | 完整产品需求 |
| 开发计划 | [`plans/openlife_development_plan.md`](plans/openlife_development_plan.md) | 开发路线图 |
| 执行手册 | [`plans/openlife_codex_execution_playbook.md`](plans/openlife_codex_execution_playbook.md) | 详细执行方案 |
| Sprint 计划 | [`plans/sprint_7_8_9_plan.md`](plans/sprint_7_8_9_plan.md) | 近期迭代计划 |
| 用户文档 | [`README.md`](README.md) | 面向用户的快速开始指南 |

### 学习资源

| 资源 | 链接 | 说明 |
|------|------|------|
| Tauri 官方文档 | https://tauri.app | 桌面应用开发框架 |
| React 官方文档 | https://react.dev | 前端 UI 框架 |
| Rust 官方文档 | https://doc.rust-lang.org | 后端核心语言 |
| Tailwind CSS | https://tailwindcss.com | 原子化 CSS |
| MCP 协议规范 | https://modelcontextprotocol.io | 模型上下文协议 |
| A2A 协议规范 | （需要人工补充） | Agent-to-Agent 协议 |
| Ollama API | https://github.com/ollama/ollama/blob/main/docs/api.md | 本地模型服务 |
| OpenRouter API | https://openrouter.ai/docs | 云端模型聚合 API |

---

## 🔄 更新日志

| 日期 | 更新内容 | 作者 |
|------|----------|------|
| 2026-04-20 | 初始版本：完成项目结构、技术栈、启动脚本、环境配置、业务逻辑、测试策略 | AI Agent |
| 2026-04-20 | 新增完整启动脚本集合（setup/dev/start/startup + Makefile） | AI Agent |
| 2026-04-20 | 更新 AGENTS.md 为完整模板格式（命名约定、代码风格、业务规则、已知问题、测试策略） | AI Agent |
| 2026-04-22 | 拆分 src-tauri/src/lib.rs：67+ 命令按领域拆分为 13 个 commands/ 模块，lib.rs 保留共享类型和核心聊天命令 | AI Agent |
| 2026-04-22 | 清理未使用 import，cargo check 零警告零错误；前端 86 测试 + Rust 129 测试全部通过 | AI Agent |

---

*本文档基于代码实际状态编写。如内容过时，请同步更新此文件。*
