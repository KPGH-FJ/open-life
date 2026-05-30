# OpenLife - AI 助手上下文指南

> 本文档面向 AI Agent 和开发协作者，提供快速理解项目所需的一切上下文信息。

---

## 📋 项目概览

- **项目类型**：本地优先的个人 Agent 框架 / 个人 AI 操作系统（Tauri 桌面壳 + React 前端 + Rust 核心引擎）
- **技术栈**：Rust (Tauri 2.x + 自定义核心库) + React 18 + TypeScript + Tailwind CSS + SQLite
- **核心范式**：`LifeModel-HS Protocol Layer + Governed Agent Runtime + ReAct Default Strategy + Tool/Skill Execution + Memory/Feedback/Maturation Loop`
- **产品定义**：OpenLife 不是单纯聊天应用，也不是普通成长管理 App。它应当让用户用私人 LifeModel 驱动本地或云端模型完成对话、规划、写作、复盘、工具调用和状态更新，并在用户确认下持续更新对用户的理解。
- **当前阶段**：W17 Runtime integration hardening / Chat migration gate 已完成。MultiStrategy Runtime 当前是 adapter/registry 化的 preview/audit-ready 路径，不是默认 Chat 主链路；`check_runtime_migration_gate` 只是只读诊断。ReAct 执行闭环、Tool Registry、Permission/Proposal/Replay、ModelRouter、Tool Taxonomy 仍是当前稳定基础。`make ci` 为发布门控。
- **仓库链接**：（需要人工补充）

### 当前架构文档优先级

后续 Agent 进入项目时，优先阅读：

1. [`plans/README.md`](plans/README.md)：文档权威地图。仓库和 GitHub 中旧计划很多，若文档互相冲突，以这里的优先级为准。
2. [`plans/openlife_lifemodel_governed_agent_runtime.md`](plans/openlife_lifemodel_governed_agent_runtime.md)：下一阶段总纲。定义 LifeModel-HS 作为协议层、ReAct 作为默认策略、Maturation Loop 与未来 Multi-Strategy Runtime 的开发顺序，优先级最高。
3. [`plans/lifemodel_governed_runtime_progress.md`](plans/lifemodel_governed_runtime_progress.md)：W1-W17 完成度与当前 non-default preview / migration gate 状态索引；不是第二套路线图。
4. [`plans/openlife_agent_framework_architecture.md`](plans/openlife_agent_framework_architecture.md)：Agent Framework 架构基准。现在应与总纲合读：ReAct 是当前默认 runtime strategy，不是唯一未来架构。
5. [`plans/openlife_react_beta_roadmap.md`](plans/openlife_react_beta_roadmap.md)：Alpha+ 到 Beta 的 ReAct 执行能力路线图，定义 Beta Gate 和工具执行严肃性。
6. [`plans/lifemodel_hs_mvp_task_specs.md`](plans/lifemodel_hs_mvp_task_specs.md)：Post-Beta LifeModel-HS MVP 的 coding-ready task specs。
7. [`plans/adr/0013-lifemodel-hs-source-of-truth-governance.md`](plans/adr/0013-lifemodel-hs-source-of-truth-governance.md)：LifeModel-HS 的 source-of-truth、governance、privacy 和 materialized-view 硬约束。
8. [`plans/lifemodel_hs_legacy_write_path_audit.md`](plans/lifemodel_hs_legacy_write_path_audit.md)：Legacy direct-write 收口地图，后续治理化开发必须参考。
9. [`plans/lifemodel_hs_architecture_plan.md`](plans/lifemodel_hs_architecture_plan.md)：LifeModel-HS 设计基线，已由 ADR 0013、MVP specs 和总纲接管实现入口。
10. [`OpenLife_PRD_v2_Agent_Framework.md`](OpenLife_PRD_v2_Agent_Framework.md)：产品定义与需求基准；实现顺序不得覆盖 LifeModel-Governed 总纲。
11. [`plans/openlife_development_plan.md`](plans/openlife_development_plan.md)：当前开发路线，已按 Agent Framework 重写。
12. [`README.md`](README.md)：面向用户与新开发者的当前状态说明。
13. [`OpenLife_Final_PRD.md`](OpenLife_Final_PRD.md)：旧版 PRD，仅作为历史参考，不再作为当前架构唯一依据。

### 后续开发总原则

- 不推倒重写，继续复用现有模块。
- 不继续平铺新页面，优先建立 Agent Runtime 主线。
- ReAct 是当前默认执行策略：后续核心能力必须先收敛到 `Reason -> Act(tool/skill) -> Observe -> Follow-up -> Proposal/Permission -> Apply/Replay -> Audit`，但架构上要为 Plan-Execute、Workflow、Proactive 等 RuntimeStrategy 留出位置。
- 当前分支已完成 W1-W17；下一步只能在 gate evidence 干净后继续受控 Chat migration，不能直接替换默认 Chat 主路径。
- `run_multi_strategy_agent_preview` 是 preview/beta command。它可用于非默认调试和审计验证，不代表 MultiStrategy Runtime 已产品化。
- `check_runtime_migration_gate` 只读检查既有 preview AgentRun / audit；不得在 gate 中执行 ReAct、PlanExecute、工具调用或外部写入。
- W10 的 preview AgentRun audit 是 metadata-safe 外层 run。ReAct payload 里的 inner run id 只能作为 child metadata 存在，不是 Runs 查询和产品 trace 的主 id。
- 后续 Agent 不得默认替换 `send_message`、`start_stream_message` 或 Chat 主流程；任何迁移必须先从非默认 preview/debug 入口或受控子路径开始，并保留稳定 fallback。
- Tools 是 Agent 的执行能力，不是附属页面。OpenLife Beta 必须具备 OpenClaw-like 的 tool execution seriousness，但必须叠加 LifeModel、Privacy、Permission、Proposal、Audit 约束。
- Beta 的 Execution Tools 至少要覆盖 MCP、A2A、file、web、calendar、email、task proposal 等类别；未实现真实 executor 的工具必须 disabled/declarative-only，不能伪装成可执行。
- `calendar.propose_event` / `email.propose_draft` 当前是 P1 proposal-only governed executors：只创建 `ScheduledTask` / `DataExport` proposal，不执行真实日历写入、邮件发送或 `ExternalWriteAction` fallback。后续若接入真实 provider executor，必须重新补治理测试和 taxonomy。
- 文档入口和工具 taxonomy 必须与代码状态同步；过期 P1/P2 标签会误导后续 Agent，视为架构阻塞项。
- 新功能必须能挂到 `AgentTask`、`AgentRun`、`AgentAction`、`AgentProposal`、`LifeModel`、`Memory`、`ModelRouter` 或 `Workspace` 中。
- Chat、Builder、Calibration、Dashboard 都只是 Agent Framework 的不同表面，不是彼此孤立的产品中心。
- 高风险 LifeModel 更新、外部工具写操作、敏感数据上云必须可解释、可确认、可回滚。
- LifeModel-HS 开发必须遵守 ADR 0013：增量落地、Proposal-first、privacy as hard Policy、metadata-safe audit、YAML 仅作为 compatibility materialized view。
- LifeModel-HS 不是孤立功能区，而是跨 Chat、Builder、Calibration、Memory、Tools、ModelRouter、AgentRun、Proposal 的协议层；后续成熟化开发必须沿着 `LifeEvent -> Signal -> Evidence -> Governor -> Proposal -> Materialized View` 收敛。
- 插件在 Beta 阶段默认是本地 Manifest / declarative-only；除非存在真实安全 executor，否则 plugin-declared tool 不能显示为可执行能力。

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
│       │   ├── ReasoningTracePanel.tsx
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
│       ├── llm.rs                # DeepSeek / OpenAI-compatible 云端模型调用
│       ├── ollama.rs             # Ollama 本地模型调用
│       ├── scheduler.rs          # 推理调度器（本地优先策略）
│       ├── reasoning/            # 推理策略模块
│       │   ├── mod.rs            # ReasoningStrategy trait
│       │   ├── layered.rs        # 三层推理策略 (Meaning→Strategy→Generation)
│       │   └── direct.rs         # 直接推理策略
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
│       │   ├── reasoning.rs      # 推理策略命令
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
    ├── openlife_agent_framework_architecture.md
    ├── openlife_development_plan.md
    ├── openlife_codex_execution_playbook.md
    ├── frontend_experience_rebuild_plan.md
    └── engineering_structure_notes.md
```

### 核心模块

| 模块 | 文件路径 | 职责 | 依赖关系 |
|------|----------|------|----------|
| **AgentRun** | [`openlife-core/src/agent/`](openlife-core/src/agent/) | AgentRun 追踪：每次 Chat/Builder/Calibration 生成可查询的运行记录，包含模型路由 trace、上下文摘要、成功/失败状态 | 被 chat.rs、builder.rs 使用，存储在独立 SQLite agent_runs.db |
| **Proposal** | [`openlife-core/src/agent/proposal_store.rs`](openlife-core/src/agent/proposal_store.rs) | Proposal 统一层：LifeModel/Memory/Tool 权限变更必须经过用户确认（accept/reject/edit/postpone），应用前自动创建 snapshot | 被 builder.rs、calibration.rs 使用，存储在 SQLite proposals.db |
| **LifeModel** | [`openlife-core/src/life_model.rs`](openlife-core/src/life_model.rs) | 四维人生模型：Identity（身份/价值观）、Goals（短中长期目标）、Capabilities（技能/资源）、State（当前状态/情绪/健康） | 被 reasoning.rs、scheduler.rs、memory.rs 消费 |
| **LayeredReasoner** | [`openlife-core/src/agent/reasoning/layered.rs`](openlife-core/src/agent/reasoning/layered.rs) | 三层推理策略：MeaningPhase（语义理解/禁忌检测）→ StrategyPhase（策略规划）→ GenerationPhase（回复生成），SafetyChecker 安全检查。作为 AgentRuntime 的默认推理策略 | 依赖 scheduler.rs、life_model.rs |
| **InferenceScheduler** | [`openlife-core/src/scheduler.rs`](openlife-core/src/scheduler.rs) | 智能调度云端/本地模型：tool prompt → 强制云端；Ollama 可用 + prefer_local → 本地；否则 fallback 云端 | 依赖 llm.rs、ollama.rs |
| **MemoryStore** | [`openlife-core/src/memory.rs`](openlife-core/src/memory.rs) | SQLite 持久化：聊天记录、会话管理、人生模型快照、状态历史、自定义记忆记录 | 独立，被 lib.rs 调用 |
| **VectorStore** | [`openlife-core/src/vectors.rs`](openlife-core/src/vectors.rs) | 向量记忆 Tier 3：存储 embedding，支持余弦相似度检索、session 过滤、tier 升降维护 | 依赖 tract-onnx/tokenizers 做本地 embedding |
| **McpRegistry** | [`openlife-core/src/mcp.rs`](openlife-core/src/mcp.rs) | MCP 客户端管理：注册/注销服务器、list_tools、call_tool、内置工具、参数隐私检查 | 依赖 privacy.rs、tool_manifest.rs |
| **MultiStrategyRuntime** | [`openlife-core/src/agent/multi_strategy_runtime.rs`](openlife-core/src/agent/multi_strategy_runtime.rs) | Preview/core orchestrator：用 StrategySelector 在 ReAct、PlanExecute、Blocked payload 间选择，并输出 warnings | 仅通过 `run_multi_strategy_agent_preview` 非默认命令暴露；尚未接管 Chat |
| **Preview Audit** | [`src-tauri/src/commands/agent_runtime.rs`](src-tauri/src/commands/agent_runtime.rs) + [`frontend/src/utils/previewAudit.ts`](frontend/src/utils/previewAudit.ts) | W10 metadata-safe 外层 AgentRun audit；Runs / Trace 识别 preview strategy/payload/governance/warnings | ReAct inner run id 只作为 child metadata，不作为主查询 id |
| **Tauri Commands** | [`src-tauri/src/lib.rs`](src-tauri/src/lib.rs) | 30+ 个 `#[tauri::command]`：聊天、MCP、A2A、记忆、版本控制、Builder、进化、校准、系统诊断 | 依赖 openlife-core 全部模块 |
| **Frontend API** | [`frontend/src/tauri.ts`](frontend/src/tauri.ts) | TypeScript 封装层：所有后端调用的唯一入口，约 40+ 个 invoke 函数 | 仅依赖 `@tauri-apps/api/core` |

### 目标架构主线

当前代码已有 AgentRun / RuntimeInput-Output / HS packet / StrategySelector / MultiStrategy preview 等关键部件，但默认 Chat 主链路仍保留在现有 ReAct/AgentLoop 路径。后续开发应从非默认 preview/debug 入口向下面这条主线受控迁移：

```
用户意图 / 主动触发
    │
    ▼
AgentTask
    │
    ▼
AgentRun
    │
    ├── ContextAssembler：选择 LifeModel、记忆、会话上下文
    ├── ModelRouter：选择本地模型、云端模型或混合路径
    ├── ReAct Engine：Reason → Act → Observe → Reflect
    ├── ActionExecutor：内部动作 / MCP / A2A / LifeModel patch / Memory write
    └── ProposalEngine：生成待确认的 LifeModel、Memory、Tool 权限变更
    │
    ▼
用户确认 / 编辑 / 拒绝
    │
    ▼
LifeModel / Memory / Audit / Snapshot 持久化
```

未来新增模块时，优先放入这些概念，而不是继续增加互相孤立的页面级逻辑。

### 数据流

```
用户输入（ChatPage.tsx）
    │
    ▼
[frontend/src/tauri.ts] ──invoke──► [src-tauri/src/lib.rs]
    │                                    │
    │    ┌───────────────────────────────┼───────────────────────────────┐
    │    ▼                               ▼                               ▼
[Stream UI]                    [LayeredReasoner]                 [MemoryStore]
(逐字显示)               Meaning→Strategy→Generation        (保存消息/快照)
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
1. **输入预处理**（[`preprocess_chat_input`](src-tauri/src/lib.rs:391)）：用户消息 → 向量检索相关记忆 → LayeredReasoner 请求构建
2. **三层推理**（[`LayeredReasoner::reason`](openlife-core/src/agent/reasoning/layered.rs)）：MeaningPhase（语义理解）→ StrategyPhase（JSON 策略）→ GenerationPhase（回复生成）
3. **模型调度**（[`InferenceScheduler::generate`](openlife-core/src/scheduler.rs:71)）：根据 tool prompt 和 Ollama 可用性决定使用本地或云端模型
4. **工具调用**（[`execute_tool_call_internal`](src-tauri/src/lib.rs:264)）：MCP 工具执行 + 隐私参数脱敏 + 审计日志
5. **流式输出**（[`start_stream_message`](src-tauri/src/lib.rs:822)）：SSE 风格流式传输到前端

> 注意：上面的数据流描述的是当前实现。目标架构会把这条链路收敛到 `AgentRun`，并让每次模型调用、上下文选择、工具调用和 LifeModel 更新都具备可查询 trace。

---

## 🛠️ 开发规范

### 命名约定

| 范畴 | 约定 | 示例 |
|------|------|------|
| **Rust 文件/目录** | `snake_case` | `life_model.rs`, `mcp_audit.rs` |
| **Rust 结构体/枚举** | `PascalCase` | `LifeModel`, `LayeredReasoner`, `AlertLevel` |
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
| **AgentTask** | `id`, `kind`, `user_intent`, `life_context_scope`, `execution_policy`, `status` | 目标架构中的任务入口；Chat、Builder、Calibration、Proactive Check-in 都应逐步映射为 AgentTask |
| **AgentRun** | `task_id`, `model_route`, `context_summary`, `actions`, `observations`, `output`, `proposals`, `errors` | 目标架构中的一次可追踪执行；后续用于解释“用了什么上下文、哪个模型、做了什么动作” |
| **AgentProposal** | `proposal_type`, `affected_path`, `before`, `after`, `reason`, `confidence`, `risk_level`, `status` | Builder、Calibration、Evolution、Memory 更新的统一确认结构 |
| **LifeModel** | `metadata`, `identity`, `goals`, `capabilities`, `state`, `relationships`, `preferences` | 1 个用户 1 个当前 LifeModel；支持快照版本控制 |
| **Identity** | `name`, `values[]`, `personality_traits[]`, `life_philosophy`, `mission_statement`, `role_definition`, `voice_style` | 属于 LifeModel 的子维度 |
| **Goals** | `short_term[]`, `medium_term[]`, `long_term[]`, `life_goals[]`, `daily[]` | 每个 GoalItem 有 `priority`, `progress`, `deadline`, `milestones[]` |
| **State** | `current_focus`, `health_status`, `emotional_state`, `habit_streaks[]`, `custom_dimensions[]`, `alerts[]` | 支持自定义维度 + 阈值预警 |
| **ChatMessage** | `role` (system/user/assistant), `content`, `tool_calls?`, `name?` | 属于 ChatSession；持久化到 SQLite |
| **MemoryChunk** | `session_id`, `content`, `embedding[]`, `source`, `tier`, `access_count` | 向量记忆，tier 1/2/3 分层 |
| **ToolManifest** | `name`, `description`, `source` (builtin/mcp/external), `parameters` | 注册到 McpRegistry |
| **ReasoningTrace** | `meaning_result`, `strategy_result`, `generation_result`, `safety_check_result` | 一次对话请求的推理过程痕迹 |

### 状态机和流程

#### 1. 目标 Agent Runtime 流程

```
用户请求 / 主动触发
    │
    ▼
AgentTask 创建
    │
    ▼
AgentRun 执行
    │
    ├── 组装 LifeModel + Memory + 会话上下文
    ├── 根据隐私、能力、成本、工具需求选择模型路径
    ├── Reason / Act / Observe 循环
    ├── 生成用户输出
    └── 生成 LifeModel / Memory / Tool 权限 Proposal
    │
    ▼
用户确认、编辑或拒绝 Proposal
    │
    ▼
写入 LifeModel / Memory / Snapshot / Audit
```

#### 2. 当前 LayeredReasoner 三层推理流程

```
用户输入
    │
    ▼
┌─────────────┐    语义理解 + 禁忌话题检测
│ MeaningPhase│ ──► 输出：user_text, forbidden_topics[]
└─────────────┘
    │
    ▼
┌───────────────┐    策略规划（JSON 输出）
│ StrategyPhase │ ──► 输出：strategy_json（含工具调用意图）
└───────────────┘
    │
    ▼
┌─────────────────┐    回复生成（最终回复）
│ GenerationPhase │ ──► 输出：assistant_reply
└─────────────────┘
    │
    ▼
┌──────────────┐    安全检查：验证输出是否偏离策略/意义
│ SafetyChecker│ ──► 最终返回给用户
└──────────────┘
```

每层有独立超时（Meaning 5s / Strategy 15s / Generation 30s，可配置），失败时由 AgentRuntime 决定降级策略。

LayeredReasoner 是 AgentRuntime 的默认推理策略，通过 `ReasoningStrategy` trait 注册。未来可扩展 DirectReasoner（直接推理）、ReActReasoner（工具循环推理）等策略。

#### 3. 当前模型调度策略

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

目标状态下，这条逻辑应升级为 `ModelRouter`：

- 按任务类型选择模型角色：chat / planner / tool_use / summarizer / extractor / embedding
- 按隐私策略决定本地、云端或摘要上云
- 记录每次 AgentRun 的 provider、model、fallback、redaction trace

#### 4. 记忆检索流程

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

#### 5. 每日目标自动打卡流程

[`try_auto_checkin_daily_goals`](src-tauri/src/lib.rs:357) 会在每次 assistant 回复后检查内容中是否提到完成了某个 daily goal 的名称，自动将 `done` 标记为 `true`。

### 业务规则约束

1. **AgentRun 优先**：后续任何重要 AI 执行都应逐步记录为 AgentRun，而不是只在页面 state 中完成。
2. **LLM 后端最低要求**：至少配置一个 LLM 后端（Ollama 或云端 API Key）才能使用模型对话功能。
3. **工具调用默认保守**：MCP/A2A/external 写操作必须走确认或 allowlist，不能只依赖前端隐藏结果。
4. **LifeModel 高风险更新需确认**：身份、价值观、人生使命、长期目标等字段必须由用户确认后写入。
5. **PII 本地拦截**：所有 outgoing 请求经过 [`PrivacyEngine`](openlife-core/src/privacy.rs) 检测，高敏感度 PII 应阻止发送或脱敏。
6. **消息 checksum**：保存消息到 SQLite 时，根据 `content + session_id + created_at` 生成 SHA256 checksum，用于完整性校验。
7. **Ollama 缓存 10 秒**：`ollama.rs` 每 10 秒缓存一次模型可用性检查，状态变化不会立即反映。
8. **向量记忆 tier 维护**：`vectors.rs` 定期运行 `run_tier_maintenance()`，高频访问 chunk 晋升 tier，低频降级。
9. **HashRouter 强制使用**：前端必须使用 `HashRouter` 而非 `BrowserRouter`，因为 Tauri 桌面应用基于 `file://` 协议。
10. **数据目录统一**：应用数据目录已统一为 `ai.openlife.app`（与 `tauri.conf.json` 的 `identifier` 一致），macOS 路径为 `~/Library/Application Support/ai.openlife.app/`。旧版本数据在 `com.openlife.app`，如需迁移请手动复制。

### Tool Taxonomy（Beta 工具分类）

OpenLife Beta 将工具按执行能力分为 **P1（真实可执行）**、**P2（declarative-only stub）** 和 **治理待校准**。未实现真实 executor 或治理语义未闭合的工具不得伪装为完成 P1。入口文档与本 taxonomy 必须随代码状态同步更新。

#### Core OS Tools（P1 — 真实可执行）

| 工具 | 用途 | 风险等级 | 状态 |
|------|------|----------|------|
| `life_model.read` | 读取 LifeModel 字段 | low | ✅ P1 |
| `life_model.propose_patch` | 提议 LifeModel 变更 | medium | ✅ P1（生成 Proposal） |
| `goal.read` | 读取目标 | low | ✅ P1 |
| `goal.propose_update` | 提议目标更新 | medium | ✅ P1（生成 Proposal） |
| `state.read` | 读取状态 | low | ✅ P1 |
| `memory.search` | 检索记忆 | low | ✅ P1 |
| `memory.propose_write` | 提议记忆写入 | medium | ✅ P1（生成 MemoryWrite Proposal） |
| `memory.propose_archive` | 提议归档记忆 | medium | ✅ P1（生成 MemoryArchive Proposal） |
| `proposal.create` | 创建提案 | low | ✅ P1 |
| `proposal.list` | 列出提案 | low | ✅ P1 |
| `agent_run.lookup` | 查询运行记录 | low | ✅ P1 |
| `snapshot.create` | 创建快照 | low | ⚠️ declarative-only（Beta 不进入 tools prompt） |
| `tool.list_available` | 列出可用工具 | low | ✅ P1 |
| `permission.check` | 检查权限策略 | low | ✅ P1 |
| `permission.request` | 请求权限 | medium | ✅ P1（生成 ToolPermission Proposal） |
| `permission.replay_action` | 重放已授权动作 | medium | ✅ P1 |

#### Execution Tools（混合 P1/P2）

| 工具 | 用途 | 风险等级 | 状态 |
|------|------|----------|------|
| `file.read` | 读取本地文件 | low/medium | ✅ P1（受 safe_paths 限制） |
| `file.write_proposal` | 提议文件写入 | high | ✅ P1（生成 ExternalWriteAction Proposal） |
| `web.search` | 搜索网页 | medium | ✅ P1（DuckDuckGo/Brave/SearXNG 三后端 + rate limit） |
| `web.fetch` | 获取 URL | medium | ✅ P1（受私网拦截 + summarize Ollama 支持） |
| `mcp.call_tool` | 调用 MCP 工具 | 取决于目标 | ✅ P1（wrapper，权限落在目标 tool scope） |
| `a2a.call_agent` | 调用 A2A Agent | medium | ✅ P1（30s超时+私网拦截） |
| `calendar.read` | 读取日历 | low | ✅ P1（ICS parser） |
| `calendar.propose_event` | 提议日历事件 | medium | ✅ P1 proposal-only governed executor（只生成 `ScheduledTask` Proposal；不写日历，不生成 `ExternalWriteAction`） |
| `email.read` | 读取邮件 | low | ⚠️ P2（declarative-only，需配置 IMAP account） |
| `email.propose_draft` | 提议邮件草稿 | medium | ✅ P1 proposal-only governed executor（只生成 `DataExport`/email-draft Proposal；不发送邮件，不生成 `ExternalWriteAction`） |
| `task.create_proposal` | 提议创建任务 | medium | ✅ P1（ScheduledTask Proposal + TaskStore） |

> **P1 / P2 判定标准**：P1 工具必须具备真实 executor 或明确的 proposal-only governed executor、治理语义、proposal/apply/replay 路径和集成测试；P2 工具仅有 manifest 声明，无真实执行能力，标记为 `declarative_only: true`。P2 工具不会进入模型的 tools prompt，前端 Tool Registry 中显示为 "⚠️ 声明-only"。治理待校准工具必须先进入 W1 Tool Proposal Hygiene，不能被未来 Agent 当成已完成 P1。

---

## ⚙️ 环境配置

### 必需的环境变量

| 变量名 | 用途 | 示例值 | 是否必须 |
|--------|------|--------|----------|
| `DEEPSEEK_API_KEY` | DeepSeek API Key（当前推荐试用） | `sk-xxxxxxxx` | 否（三选一） |
| `OPENROUTER_API_KEY` | OpenRouter API Key | `sk-or-v1-xxxxxxxx` | 否（三选一） |
| `OPENAI_API_KEY` | OpenAI API Key | `sk-xxxxxxxx` | 否（三选一） |
| `OPENAI_API_BASE` | 自定义 API Base URL | `https://api.openai.com/v1` | 否（有默认值） |
| `A2A_PORT` | A2A 独立服务器端口 | `8765` | 否（默认 8765） |
| `PORT` | Vite 开发服务器端口 | `5173` | 否（默认 5173） |
| `TAURI_DEBUG` | Tauri 调试日志开关 | `1` | 否 |

> 至少配置 `DEEPSEEK_API_KEY`、`OPENROUTER_API_KEY` 或 `OPENAI_API_KEY` 之一才能使用云端模型对话。如果不配置，必须本地运行 Ollama。

### 外部服务依赖

| 服务 | 用途 | 配置位置 | 本地替代方案 |
|------|------|----------|-------------|
| **Ollama** (localhost:11434) | 本地 LLM 推理 | `.env` / `config.yaml` | 无替代，需本地安装 |
| **DeepSeek API** | 当前推荐云端试用 Provider | `.env` / `config.yaml` | OpenAI-compatible Provider |
| **OpenRouter API** | 云端 LLM（多模型聚合） | `.env` / `config.yaml` | DeepSeek / OpenAI API |
| **OpenAI API** | 云端 LLM（官方） | `.env` / `config.yaml` | DeepSeek / OpenRouter API |
| **SQLite** | 本地数据持久化 | 自动 bundled | 无需替代，零配置 |

配置优先级：**环境变量 > `config.yaml` > 代码默认值**

运行时配置文件路径（macOS）：`~/Library/Application Support/ai.openlife.app/config.yaml`

### SystemConfig 配置项

`config.yaml` 中新增 `system` 字段，支持以下配置：

| 配置项 | 类型 | 默认值 | 说明 |
|--------|------|--------|------|
| `system.ollama_cache_ttl_seconds` | u64 | 10 | Ollama 模型可用性缓存 TTL |
| `system.memory_search_top_k` | usize | 3 | Chat 流程中记忆检索数量 |

### 启动和测试命令

#### 开发模式

```bash
# 一键初始化（新开发者首选）
make setup
# 或
./scripts/setup.sh      # macOS/Linux
.\setup.ps1             # Windows

# 快速开发启动
make dev
# 或
./scripts/dev.sh        # macOS/Linux
.\dev.ps1               # Windows

# 一体化脚本启动
./scripts/startup.sh dev # macOS/Linux
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

### 包管理器策略

- **统一使用 pnpm**：项目通过 `packageManager` 字段锁定 `pnpm@9.1.0`
- **启用 corepack**：`corepack enable && corepack prepare pnpm@9.1.0 --activate`
- **禁止 npm**：所有脚本已移除 npm fallback，不会生成 `package-lock.json`
- **CI 保护**：`make check-lockfile` 会检查是否误生成 `package-lock.json`

### 启动命令

```bash
# 一键初始化（新开发者首选）
make setup
# 或
./scripts/setup.sh      # macOS/Linux
.\setup.ps1             # Windows

# 快速开发启动
make dev
# 或
./scripts/dev.sh        # macOS/Linux
.\dev.ps1               # Windows

# 一体化脚本启动
./scripts/startup.sh dev # macOS/Linux
.\startup.ps1 dev       # Windows

# 检查环境
make check
# 或
./scripts/startup.sh check
```

#### 构建生产版本

```bash
# macOS
make build
# 或
./scripts/start.sh
# 或
pnpm tauri build --target universal-apple-darwin

# Windows
.\start.ps1
# 或
pnpm tauri build --target x86_64-pc-windows-msvc

# Linux
./scripts/start.sh
# 或
pnpm tauri build --target x86_64-unknown-linux-gnu
```

---

## 🐛 已知问题和注意事项

### 历史遗留问题

1. **reqwest 版本不一致**：`openlife-core` 使用 `reqwest 0.11`，`src-tauri` 使用 `reqwest 0.12`。目前编译通过，但建议统一版本以避免潜在兼容性问题。
2. ~~Ollama 缓存固定 10 秒~~ **已修复**：`ollama_cache_ttl_seconds` 已加入 `SystemConfig`，可通过 `config.yaml` 配置，默认仍为 10 秒。
3. ~~数据目录与 Tauri identifier 不一致~~ **已修复**：数据目录已统一为 `ai.openlife.app`。旧版本数据如需迁移请手动复制。
4. **MCP 审计日志单独数据库**：`mcp_audit.db` 与 `messages.db`/`vectors.db` 分开存储，这是设计上的隔离，但备份/迁移时容易遗漏。

### 常见陷阱

1. **忘记使用 HashRouter**：如果新开发者习惯性使用 `BrowserRouter`，Tauri 桌面应用在 `file://` 协议下会白屏。
2. **工具调用时 local model 被跳过**：如果配置了 `prefer_local=true` 但消息触发了 tool prompt，会静默切换到云端模型，前端无显式提示。
3. **忘记 bump_access**：手动操作 `VectorStore` 后如果不调用 `bump_access_for_chunks`，tier 维护不会正确晋升高频记忆。
4. **PII 检测导致 MCP 调用失败**：如果工具参数包含被标记为高风险的 PII，`McpRegistry` 会阻止调用，错误信息可能不够明确。
5. **`.env` 修改后需重启**：Tauri dev 不会热重载 `.env` 变更，修改 API Key 后需要重启开发服务器。

### 性能敏感区域

1. **向量检索余弦相似度**：[`cosine_similarity`](openlife-core/src/vectors.rs:285) 已改为 4-wide 手动向量化，提升指令吞吐。
2. **embedding 生成**：已实现 LRU Embedding 缓存（默认 1000 条，1 小时 TTL），避免重复计算相同文本的 embedding。
3. **LayeredReasoner 三层串行调用**：Meaning → Strategy → Generation 是串行的，每层都有 LLM 请求，总延迟 = 三层之和。Strategy 层要求输出合法 JSON，重试逻辑可能增加额外延迟。作为 AgentRuntime 的默认推理策略，可通过配置调整超时或切换为 DirectReasoner 降低延迟。
4. **SQLite 写入锁**：`MemoryStore` 使用 `Mutex<Connection>`，高并发写入（如同时保存消息 + 向量化 + 审计日志）会串行化。
5. **Ollama 首次加载延迟**：本地模型首次加载到 GPU 内存时可能有数秒延迟，缓存机制只检查可用性，不预热模型。

### 待重构区域

1. ~~**reqwest 版本统一**：将 `openlife-core` 升级到 `reqwest 0.12`。~~ ✅ 已完成（Gate 0）
2. ~~**Agent Runtime 引入**：新增 `AgentTask`、`AgentRun`、`AgentAction`、`AgentProposal` 和 `AgentRunStore`，先从 Chat 主链路接入。~~ ✅ 已完成（AgentRun/Proposal 基线已落地）
3. ~~**ModelRouter 升级**：灰度中；已补齐 provider health 语义和隐私优先路由，后续继续做 role-aware 策略与真实探针覆盖。~~ ✅ 已完成（Gate 6：已毕业，移除 experimental flag）
4. ~~**Proposal 统一**：Builder、Calibration、Evolution、Memory 更新应统一走 Proposal/Confirmation，而不是各自实现审批流。~~ ✅ 已完成（Builder/Calibration/Chat/Memory/Tool Permission MVP 已接入 Proposal 流）
5. ~~**LayeredReasoner / ReAct 边界重构**：三层推理总线应纳入 AgentRuntime 或成为其中一种策略。~~ ✅ 已完成（LayeredReasoner 已作为 AgentRuntime 的默认推理策略，通过 ReasoningStrategy trait 注册）
6. ~~**前端信息架构重构**：从多页面工具箱收敛为 Workspace / Agent / LifeModel / Memory / Runs / Settings。~~ ✅ 已完成（Gate 7：导航收敛为 Chat/Review/Runs/Settings）
7. **前端 ErrorBoundary 过于简单**：目前只显示红色背景文本，可以添加重试按钮或错误上报。
8. ~~**核心逻辑测试覆盖**：Rust 测试集中在 config.rs、vectors.rs、builder.rs、versioning.rs，核心逻辑（AgentRuntime、ModelRouter、LayeredReasoner、scheduler）需要补充测试。~~ ✅ 已补充（AgentRuntime 4 个核心测试 + Tauri 命令 6 个测试 + 10 个集成测试）
9. ~~**Chat 流 Proposal 接入**：当前 Chat 对话不生成 LifeModel 更新 Proposal，未来应支持 Chat 中 AI 建议修改 LifeModel 时走 Proposal 确认流。~~ ✅ 已完成（Chat 流程自动调用 ProposalEngine 生成提案）
10. **Execution Tools 继续加固**：file/web/calendar/task/MCP/A2A 已有 P1 路径；后续重点是 provider 覆盖、失败可解释性和 taxonomy 同步，不能把 proposal-only 工具描述成真实外部写入。
11. **MultiStrategy 受控迁移**：当前 MultiStrategy 仅通过 preview/beta command 暴露；后续先做非默认 preview UI/debug entry，再做受控 Chat 子路径迁移，不能直接替换 Chat 主流程。

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
| 文档权威地图 | [`plans/README.md`](plans/README.md) | 解决旧计划/新计划冲突的最高优先级索引 |
| 下一阶段总纲 | [`plans/openlife_lifemodel_governed_agent_runtime.md`](plans/openlife_lifemodel_governed_agent_runtime.md) | LifeModel-Governed Runtime、Maturation Loop、多策略 Runtime 的当前开发顺序 |
| 架构基准 | [`plans/openlife_agent_framework_architecture.md`](plans/openlife_agent_framework_architecture.md) | Agent Framework 架构基准，需与总纲合读 |
| 产品需求 v2 | [`OpenLife_PRD_v2_Agent_Framework.md`](OpenLife_PRD_v2_Agent_Framework.md) | 产品定义与需求基准；不覆盖当前实现顺序 |
| 开发计划 | [`plans/openlife_development_plan.md`](plans/openlife_development_plan.md) | 当前开发路线图，按 LifeModel-Governed Runtime 迁移路线维护 |
| 执行手册 | [`plans/openlife_codex_execution_playbook.md`](plans/openlife_codex_execution_playbook.md) | 单轮任务切分与验证方式 |
| 前端重构计划 | [`plans/frontend_experience_rebuild_plan.md`](plans/frontend_experience_rebuild_plan.md) | 历史前端体验重构计划，后续需按 Agent Workspace 更新 |
| 工程治理笔记 | [`plans/engineering_structure_notes.md`](plans/engineering_structure_notes.md) | 工程拆分和治理记录 |
| 用户文档 | [`README.md`](README.md) | 面向用户的快速开始指南 |
| 历史 PRD | [`OpenLife_Final_PRD.md`](OpenLife_Final_PRD.md) | 旧版完整需求，作为历史参考，不再覆盖新的 Agent Framework 定义 |

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
| 2026-04-24 | 将项目上下文从“桌面 AI 伴侣应用”更新为“本地优先个人 Agent 框架”，新增 Agent Runtime、AgentRun、Proposal、ModelRouter 作为后续开发主线 | AI Agent |
| 2026-04-26 | Proposal/Confirmation 统一层收敛完成：Builder 和 Calibration 的 LifeModel 更新默认走 Proposal → Review Center → 用户确认 → Snapshot → Apply 链路；AgentRun ↔ Proposal 双向关联溯源；Safe Mode 限制 Proposal 操作；Review Center 强化（分类/风险筛选/编辑/批量/空状态/失败态/Dashboard 提醒） | AI Agent |
| 2026-04-28 | 推理架构治理：LayeredReasoner 成为 AgentRuntime 的默认推理策略，通过 ReasoningStrategy trait 注册；新增 DirectReasoner 作为备选策略；统一超时配置；SafetyChecker 替代 Arbitrator；更新 AGENTS.md 和架构文档 | AI Agent |
| 2026-04-29 | Stabilization / Spine Consolidation：Builder 默认 Proposal-Only；MemoryWrite/MemoryArchive/ToolPermission Proposal MVP 可应用；Chat Proposal 与 AgentRun.generated_proposals 关联收敛；ModelRouter provider health 与隐私优先路由增强；make ci 增加前端生产构建/typecheck | AI Agent |
| 2026-05-01 | **Beta 开发完成（Gates 0-8）**：reqwest 0.12 升级；AgentLoop 双轨（feature flag）；Action Parser JSON envelope + fail-soft；Core OS Tools + Execution Tools 注册；Permission/Replay 闭环；ModelRouter 毕业；UI 导航收敛；Settings 新增 safe paths / AgentLoop toggle；`make ci` 持续通过 | AI Agent |
| 2026-05-01 | **Week 1: ExternalWriteAction 闭环**：`is_path_in_safe_paths` 公共化；`file.write_proposal` 自动创建 Proposal（含 content_hash/size_bytes/operation）；ExternalWriteAction apply 真实文件写入（safe_paths + 100KB 限制 + 自动创建父目录） | AI Agent |
| 2026-05-08 | **Week 2: Stub 转 Proposal + Task MVP**：calendar.propose_event / email.propose_draft 生成 ScheduledTask/DataExport Proposal；ScheduledTask MVP（本地 JSON）；DataExport MVP（safe_paths 内文件导出） | AI Agent |
| 2026-05-08 | **Week 3: AgentLoop Stream + Fallback**：AgentLoop 句子级分 chunk 流式输出（30ms 间隔）；AgentLoop graceful fallback 到 legacy direct generation；保持默认关闭 | AI Agent |
| 2026-05-08 | **Week 4: 可观察性 + Feature Flag**：stream-message-start 传递真实 ReasoningTrace（AgentLoop actions/observations）；debug 构建默认启用 AgentLoop | AI Agent |
| 2026-05-08 | **Week 5: Workspace + Chat Trace**：Dashboard 新增 Workspace 概览卡片（Pending Proposals / Recent Runs / New Chat）；Chat 最后一条 assistant 消息显示模型/provider/工具数/Proposal 数/fallback 标记 | AI Agent |
| 2026-05-08 | **Week 6: Review Center + 页面收敛**：Chat 顶部 pending proposals banner；主导航从 7 个收敛到 6 个（Memory 移出主导航，保留路由） | AI Agent |
| 2026-05-08 | **Week 7: 测试补齐 + AgentLoop 默认启用**：新增 ExternalWriteAction/ScheduledTask/DataExport apply 测试；AgentLoop 生产环境默认启用 | AI Agent |
| 2026-05-08 | **Week 8: 收口回归**：文档更新；连续 8 周 `make ci` 通过 | AI Agent |
| 2026-05-03 | **Sprint 9: 架构优化**：action_executor.rs 拆分为 8 个模块 (core_os_tools/execution_tools/memory_ops/life_model_ops/declarative_stubs/helpers/tool_executor/mod.rs)；AgentLoop 实现真实 token 级流式输出（StreamingCallback trait + run_streaming + TauriStreamingCallback 适配）；P2 工具升级：calendar.read 从 P2→P1（ICS 文件解析器），task.create_proposal 从 P2→P1（TaskStore + ScheduledTask 本地持久化）；移除 sentence-based 伪流式（split_into_sentences）；config.rs 新增 calendar_ics_paths | AI Agent |
| 2026-05-05 | **Sprint 10: CI修复 + 技术债务**：P0 clippy 修复（AgentLoopContext 重构消除 too_many_arguments、privacy.rs manual_is_multiple_of、dead_code test_app_state）；web.search DuckDuckGo 双正则 fallback + 5s rate limit；AgentLoop 参数配置化（max_steps/tool_calls/timeout 进 SystemConfig）；lib.rs bootstrap 提取到 src-tauri/src/bootstrap.rs（3234→2821 行） | AI Agent |
| 2026-05-05 | **Sprint 11: 执行工具闭环**：a2a.call_agent P2→P1（30s超时+私网拦截+真实 A2AClient 调用）；calendar.propose_event / email.propose_draft 已在 W1 复核为 P1 proposal-only governed executors；ChatProposalGenerator LLM 升级（Ollama 信号提取优先，静默降级关键词匹配） | AI Agent |
| 2026-05-05 | **Sprint 12: Agent 深度能力**：AgentRole（Generalist/Planner）+ role_system_instruction 注入 tools prompt；scheduler_runner.rs（60s 轮询 scheduled_tasks.json + AgentLoop 自动执行）；E2E integration tests（AgentRole config 验证）；LifeModel 字段 GoalItem.updated_at、State.last_updated | AI Agent |
| 2026-05-05 | **Sprint 13: Proactive Agent MVP（Phase 6）**：ProactiveEngine（每日简报、每周复盘、目标陈旧检测、提案提醒、状态签到）；ProactiveConfig 集成 SystemConfig；Tauri 命令 get_proactive_suggestions；record_state 自动更新 last_updated；toolset_allowlist 过滤 AgentLoop 执行 | AI Agent |
| 2026-05-05 | **Sprint 14: Dashboard + 搜索增强**：Dashboard 主动建议卡片前端集成；web.search 多Provider支持（DuckDuckGo 默认/Brave API/SearXNG）；web.fetch 新增 summarize 参数 → Ollama 中文摘要；search_provider 配置入 SystemConfig | AI Agent |
| 2026-05-05 | **Sprint 15: Engineering Consolidation**：AGENTS.md、development_plan.md 文档同步；工具 Taxonomy 表更新（P2→P1 标记校正）；ProviderTab act() 测试警告修复；Email Settings 配置区；前端 ErrorBoundary 完善（重试+错误详情） | AI Agent |
| 2026-05-30 | **W11: LifeModel-Governed Runtime 状态同步**：W1-W10 标记完成；明确 MultiStrategy preview/audit-ready 但非默认 Chat；记录 W10 metadata-safe 外层 AgentRun audit 与下一步受控迁移顺序 | AI Agent |
| 2026-05-30 | **W16: RuntimeStrategy Trait Foundation**：W1-W16 标记完成；MultiStrategy Runtime 通过固定 ReAct / PlanExecute adapter registry 执行；该阶段不允许直接替换默认 Chat | AI Agent |
| 2026-05-30 | **W17: Runtime integration hardening / Chat migration gate**：新增只读 `check_runtime_migration_gate`，诊断默认 Chat 未替换、preview audit 健康、metadata-safe trace、fallback、无外部写入和 proposal-first 边界；下一步仍不能直接替换默认 Chat | AI Agent |

---

*本文档基于代码实际状态编写。如内容过时，请同步更新此文件。*
