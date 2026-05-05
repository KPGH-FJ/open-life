# OpenLife

OpenLife 是一个**本地优先的个人 Agent 框架**。它不是单纯的聊天应用，也不是普通的目标管理工具，而是围绕用户私人数据构建的个人 AI 操作系统雏形。

OpenLife 的核心范式是：

```text
LifeModel + Local/Cloud Model Router + ReAct Agent Runtime + Tool/Skill Execution + Memory/Feedback Loop
```

用户先构建自己的 LifeModel，包括身份、目标、能力、状态、偏好和关系等私人上下文。之后，OpenLife 会让本地模型或云端模型在这个人生模型的约束下完成对话、规划、写作、复盘、工具调用、状态更新和长期反馈。系统不只是回答问题，还应该逐步理解用户，并在用户确认下持续更新 LifeModel。

## 当前定位

当前项目处于 **Agent Framework Beta** 阶段：

- **ReAct 执行闭环已建立**：AgentLoop 迭代执行、Action Parser JSON envelope、Tool Registry 统一注册、Permission/Proposal/Replay 闭合。
- **ModelRouter 已毕业**：移除 experimental flag，成为默认路由基础设施。
- **Execution Tools 分层落地**：Core OS tools 与多类 execution tools 已接入，真实执行能力和 declarative-only 能力必须继续严格区分。
- **Core OS Tools 注册**：life_model.read、goal.read、memory.search、proposal.list 等 9 个 builtin 工具。
- **AgentLoop 成为主执行路径**：Chat/streaming/fallback/scheduled/proactive 等路径仍需要在 vNext 中进一步收束到统一 runtime 语义。
- **UI 最小收敛**：导航聚焦 Chat/Review/Runs/Settings，Settings 新增 safe paths 和 AgentLoop toggle。
- **`make ci` 为发布门控**：Rust、前端测试与生产构建共同作为发布门控。

下一大阶段是 **vNext Agent Framework Upgrade**。目标不是继续堆页面或工具，而是把 OpenLife 升级为：

```text
LifeModel-governed Personal Agent Framework
```

vNext 的重点包括：

- `AgentRunEvent`：append-only runtime trace。
- `ToolRuntime`：工具元数据、权限、declarative-only 过滤和执行审计。
- `PromptStack`：system prompt / planning prompt / tool prompt / privacy prompt 的一等架构。
- `MemoryEvidence`：让记忆成为 LifeModel 进化的证据层，而不只是检索上下文。
- `AgentSpec / AgentPlan`：为 PlanMode 和 SubAgentRuntime 奠定结构。
- AI coding governance：高风险边界采用 ADR-first 流程。

新的架构基准文档见：

- [OpenLife Agent Framework Architecture](/Users/fujing/Desktop/偶来福/plans/openlife_agent_framework_architecture.md)
- [OpenLife ReAct Beta Roadmap](/Users/fujing/Desktop/偶来福/plans/openlife_react_beta_roadmap.md)
- [OpenLife vNext Architecture Principles](/Users/fujing/Desktop/偶来福/plans/openlife_vnext_architecture_principles.md)
- [OpenLife vNext Migration Plan](/Users/fujing/Desktop/偶来福/plans/openlife_vnext_migration_plan.md)

## 核心能力

| 能力 | 当前状态 | 目标形态 |
|---|---|---|
| LifeModel | 已有四维模型和编辑器 | 成为所有 Agent 任务的私人上下文层 |
| Builder | 已支持快速、渐进、苏格拉底式构建；默认只创建 Proposal | 通过 Review Center 确认后安全写入 LifeModel |
| Chat | 已支持流式对话、历史持久化、AgentRun 和 Chat Proposal | 继续收敛共享执行核心，展示上下文、模型路由和运行轨迹 |
| **ModelRouter** | ✅ **任务/隐私感知路由灰度中，带真实健康检查语义** | 按任务类型、隐私需求、成本和延迟智能选择模型 |
| Memory | 已有 SQLite 与向量记忆；Memory Proposal 可写入/归档 | 升级为可治理、可归档、可追踪来源的长期记忆层 |
| MCP/A2A | 已有工具和外部 Agent 接入基础 | 成为 AgentAction 执行层，并默认受权限和审计保护 |
| Tools/Skills | 已有 ToolManifest、MCP/A2A、内置 Skill MVP | 成为 ReAct Agent 的执行能力层，覆盖 Core OS tools、Execution tools、Governance tools、Skill tools |
| Calibration/Evolution | 已有建议和校准雏形 | 统一进入 Proposal/Confirmation 机制 |
| Diagnostics/Safe Mode | 已有试用稳定化能力 | 成为系统控制台和恢复中枢 |
| **Chat Proposal** | ✅ **自动从对话中提取目标/状态/能力** | 自动感知用户意图并生成 LifeModel 更新提案 |
| **ContextAssembler** | ✅ **模块化上下文组装（V2 灰度中）** | 可插拔的记忆/隐私/工具上下文组装 |
| **Workspace** | ✅ **驾驶舱首页，实时状态概览** | 统一的 Agent 任务入口和监控中心 |
| **Feedback Loop** | ✅ **应用内反馈收集** | Chat 消息 👍/👎 反馈，诊断报告导出，Workspace 统计 |
| **Memory Governance** | ✅ **显式/隐式记忆提取** | "记住这个"生成 Proposal，自动记忆建议，异步 Embedding |
| **Skill Runtime** | ✅ **内置 Skill MVP** | weekly_review、goal_breakdown 等 Skill 可执行并生成 Proposal |
| **Network Policy** | ✅ **网络访问策略配置** | 域名白名单/黑名单，工具级覆盖，Privacy  tab 配置 |

## 技术栈

| 层级 | 技术 |
|---|---|
| 前端 | React 18 + TypeScript + Tailwind CSS + Vite |
| 桌面壳 | Tauri 2.x |
| 后端核心 | Rust Workspace (`openlife-core` + `openlife-tauri`) |
| 本地模型 | Ollama |
| 云端模型 | DeepSeek / OpenAI / OpenRouter / Custom OpenAI-compatible |
| 数据存储 | SQLite + YAML |

## 项目结构

```text
.
├── frontend/                     # React 前端
│   └── src/
│       ├── pages/                # Chat / Dashboard / Builder / Settings 等当前页面
│       ├── components/           # 通用组件
│       ├── tauri.ts              # Tauri command 封装层
│       └── App.tsx               # 路由与全局布局
├── openlife-core/                # Rust 核心业务库
│   └── src/
│       ├── life_model.rs         # LifeModel
│       ├── builder/              # LifeModel 构建与 Review
│       ├── agent/                # AgentRuntime、AgentLoop、Proposal、ModelRouter
│       ├── scheduler.rs          # 当前模型调度器
│       ├── llm.rs / ollama.rs    # 云端与本地模型调用
│       ├── memory.rs             # 消息、会话、状态等 SQLite 存储
│       ├── vectors.rs            # 向量记忆
│       ├── mcp.rs / a2a.rs       # 工具与外部 Agent 接入
│       ├── privacy.rs            # 隐私检测与脱敏
│       ├── feedback.rs           # 反馈信号
│       ├── evolution.rs          # 微进化
│       └── versioning.rs         # 快照与回滚
├── src-tauri/                    # Tauri 命令层和桌面壳
│   └── src/
│       ├── lib.rs                # 核心状态与聊天主链路
│       └── commands/             # 按领域拆分的 Tauri commands
├── plans/                        # 架构与开发计划
└── OpenLife_Final_PRD.md         # 旧版 PRD，当前作为历史参考
```

## 推荐阅读顺序

### vNext 大阶段开发

1. [Current Agent Runtime Audit](/Users/fujing/Desktop/偶来福/plans/current_agent_runtime_audit.md)
2. [OpenLife vNext Architecture Principles](/Users/fujing/Desktop/偶来福/plans/openlife_vnext_architecture_principles.md)
3. [OpenLife vNext Architecture Diagrams](/Users/fujing/Desktop/偶来福/plans/openlife_vnext_architecture_diagrams.md)
4. [OpenLife vNext Core Primitives and Boundaries](/Users/fujing/Desktop/偶来福/plans/openlife_vnext_core_primitives_and_boundaries.md)
5. [OpenLife vNext Migration Plan](/Users/fujing/Desktop/偶来福/plans/openlife_vnext_migration_plan.md)
6. [OpenLife vNext P0/P1 Task Specs](/Users/fujing/Desktop/偶来福/plans/openlife_vnext_p0_p1_task_specs.md)
7. [OpenLife vNext Test and Acceptance Matrix](/Users/fujing/Desktop/偶来福/plans/openlife_vnext_test_and_acceptance_matrix.md)
8. [OpenLife AI Coding Governance](/Users/fujing/Desktop/偶来福/plans/openlife_ai_coding_governance.md)
9. [ADR Backlog](/Users/fujing/Desktop/偶来福/plans/adr/README.md)

### 现有架构背景

1. [OpenLife Agent Framework Architecture](/Users/fujing/Desktop/偶来福/plans/openlife_agent_framework_architecture.md)
2. [OpenLife ReAct Beta Roadmap](/Users/fujing/Desktop/偶来福/plans/openlife_react_beta_roadmap.md)
3. [OpenLife PRD v2: Personal Agent Framework](/Users/fujing/Desktop/偶来福/OpenLife_PRD_v2_Agent_Framework.md)
4. [OpenLife Development Plan](/Users/fujing/Desktop/偶来福/plans/openlife_development_plan.md)
5. [Codex Execution Playbook](/Users/fujing/Desktop/偶来福/plans/openlife_codex_execution_playbook.md)
6. [OpenLife Final PRD](/Users/fujing/Desktop/偶来福/OpenLife_Final_PRD.md)，仅作为历史需求参考

## 快速开始

### 前置要求

- Rust >= 1.75
- Node.js 18+
- pnpm 9.x（推荐通过 Corepack 启用）
- 可选：Ollama，本地模型服务

### 安装依赖

```bash
corepack enable
corepack prepare pnpm@9.1.0 --activate
cd frontend && pnpm install
cd ..
```

项目统一使用 pnpm；请不要使用 npm 安装依赖或提交 `package-lock.json`。

### 配置模型

当前推荐先使用 DeepSeek 跑通云端试用链路，也可以使用 Ollama 本地模型。

```bash
# DeepSeek，推荐试用路径
export DEEPSEEK_API_KEY="sk-..."

# OpenRouter
export OPENROUTER_API_KEY="sk-..."

# OpenAI
export OPENAI_API_KEY="sk-..."
```

桌面端中进入 `Settings`，选择 Provider，填写 Key，点击测试连接，成功后保存。

### 开发运行

```bash
# 首次使用：初始化环境
make setup

# 启动开发模式
make dev
# 或
./scripts/dev.sh
```

### 测试

```bash
# Rust 测试
cargo test -q

# 前端测试
cd frontend && pnpm test

# 前端生产构建
cd frontend && pnpm run build

# 完整 CI 检查
make ci
```

## 当前推荐试用路径

### 主线体验（推荐）

1. **`Workspace`**（首页驾驶舱）查看系统状态、待处理 Proposal、今日 AgentRun 统计。
2. **`Agent`** 发起个性化对话，观察 Chat Proposal 自动提取目标和状态。
3. **`Review`** 审查 AI 生成的 LifeModel 更新提案，确认或拒绝。
4. **`Builder`** 完成一次快速构建，或恢复待确认 Review。
5. **`Runs`** 查看所有 Agent 执行记录，按状态/类型过滤，批量管理。
6. **`Settings`** 完成模型配置、诊断检查，开启实验性功能（ContextAssembler V2 / ModelRouter）。

### 实验性功能（灰度测试）

在 Settings → 实验性功能中可开启：

- **ContextAssembler V2**：使用模块化组装器构建对话上下文（灰度中，可回滚）
- **ModelRouter**：智能路由选择本地/云端模型（灰度中，云端 Provider 需配置并通过轻量健康检查）

```text
Workspace -> Agent Task -> Agent Run Trace -> Proposal Review -> LifeModel/Memory Update
```

## 最近完成的重要更新

### Phase 1-3: Agent Runtime 基础设施
- ✅ AgentRun 增强（RedactionLevel、AgentAction、AgentObservation）
- ✅ LifeModel Patch 系统（5 种操作、冲突检测、自动解决）
- ✅ Proposal 统一层（Builder/Calibration/Feedback/Memory 统一确认流）

### Phase 4-5: 路由与上下文
- ✅ **ModelRouter**：任务类型感知、隐私级别、Provider 健康检查
- ✅ **ContextAssembler**：模块化 LifeModel/Memory/Privacy/Tools 组装

### Phase 2.5: Chat Proposal
- ✅ 关键词提取（中英文目标/状态/能力识别）
- ✅ 动态置信度计算（信号强度 + 强调标记）
- ✅ 可配置冷却时间和阈值

### Phase 6: Workspace 重设计
- ✅ Workspace 驾驶舱（系统状态、待处理 Proposal、Run 统计）
- ✅ Runs 页面增强（过滤、搜索、分页、批量操作、回收站）
- ✅ 导航重构（Workspace 为默认首页）

### Phase 7: Stabilization / Spine Consolidation
- ✅ Builder 正常路径改为 Proposal-Only，legacy direct apply 仅保留给迁移/调试。
- ✅ Proposal 应用器覆盖 LifeModel/Goal、MemoryWrite、MemoryArchive、ToolPermission MVP。
- ✅ Chat Proposal 持久化与 AgentRun.generated_proposals 关联收敛到共享 helper。
- ✅ `make ci` 覆盖格式检查、Rust tests、frontend tests、frontend production build/typecheck。

## 当前重要开发方向

当前开发重心已转向 vNext Agent Framework Upgrade：

1. 审阅并接受首批 ADR：AgentRunEvent、PromptStack、ToolRuntime、LifeModel risk、MemoryEvidence、Cloud privacy。
2. 按 [P0/P1 Task Specs](/Users/fujing/Desktop/偶来福/plans/openlife_vnext_p0_p1_task_specs.md) 执行第一批小步迁移。
3. 先做 runtime trace 和 execution path convergence，不急于实现 SubAgent 或 Bash。
4. 建立 PromptStack/system prompt 架构，避免新增散落 prompt。
5. 将 Memory 升级为 LifeModel evolution 的证据层，但所有 evolution 只生成 Proposal，不直接改 LifeModel。
6. 按 [Test and Acceptance Matrix](/Users/fujing/Desktop/偶来福/plans/openlife_vnext_test_and_acceptance_matrix.md) 为每个阶段设置门控。

## 常见问题

- API Key 测试失败：确认 Provider、Base URL、模型名和 API Key 匹配。
- Ollama 连接失败：确认 Ollama 已启动，且模型名称存在。
- Safe Mode：说明当前数据环境存在风险，先去 Settings 的恢复控制台导出备份并修复。
- Chat 无响应或一直思考：先查看 Settings 诊断，再检查模型 Provider 测试结果。
- Builder Review 后模型没有变化：先确认 Proposal 是否仍在 Review Center 待处理；Builder 默认不会绕过确认直接写入。

## License

MIT License
