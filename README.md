# OpenLife

OpenLife 是一个**本地优先的个人 Agent 框架**。它不是单纯的聊天应用，也不是普通的目标管理工具，而是围绕用户私人数据构建的个人 AI 操作系统雏形。

OpenLife 的核心范式正在从单一 ReAct 叙述升级为：

```text
LifeModel-HS Protocol Layer
  + Governed Agent Runtime
  + ReAct Default Strategy
  + Tool/Skill Execution
  + Memory/Feedback/Maturation Loop
```

用户先构建自己的 LifeModel，包括身份、目标、能力、状态、偏好和关系等私人上下文。之后，OpenLife 会让本地模型或云端模型在这个人生模型的约束下完成对话、规划、写作、复盘、工具调用、状态更新和长期反馈。系统不只是回答问题，还应该通过 LifeEvent、Signal、Evidence、Governor、Proposal 和用户确认持续打磨 LifeModel，并让不同 RuntimeStrategy 在同一套 LifeModel-HS 协议约束下执行。

## 当前定位

当前项目处于 **W20 Very Small Controlled Chat Migration Pilot With Fallback** 阶段：

- **ReAct 执行闭环已建立**：AgentLoop 迭代执行、Action Parser JSON envelope、Tool Registry 统一注册、Permission/Proposal/Replay 闭合。
- **W1-W20 已完成**：当前已经建立 Runtime Migration Gate 只读诊断层、Settings evidence surface、controlled Chat migration pilot eligibility 只读资格检查，以及 Chat 页面显式单轮 Controlled Pilot；完整状态索引见 [LifeModel-Governed Runtime Progress](/Users/fujing/Desktop/偶来福/plans/lifemodel_governed_runtime_progress.md)。
- **ReAct 仍是当前默认 Chat 主链路**：MultiStrategy Runtime 已有 preview command 和 audit-ready 路径，但尚未接管默认 `send_message` / Chat 主流程。
- **MultiStrategy preview 已可审计**：`run_multi_strategy_agent_preview` 已存在，preview run 会写入 metadata-safe 外层 AgentRun audit；Runs / Trace 已能展示 preview strategy、payload、governance 和 warnings。
- **Runtime Migration Gate 已建立**：`check_runtime_migration_gate` 只读取既有 preview AgentRun / audit，输出 `defaultChatUnchanged`、`previewPathHealthy`、`metadataSafeTraceReady`、`fallbackAvailable`、`noExternalWrites`、`proposalFirstPreserved` 和 `blockingReasons`；它不执行 ReAct、PlanExecute、工具调用或外部写入。
- **Gate evidence surface 已可见**：Settings / 实验区域的 Runtime Migration Gate 面板可显式调用 `check_runtime_migration_gate`，展示 pass/block 与 blocking reasons；它不是 Chat 切换开关，也不会自动运行 preview。
- **Pilot Eligibility 已可见**：`check_controlled_chat_pilot_eligibility` 默认只读检查最近 3 条 MultiStrategy preview AgentRun 的 gate report 是否连续干净，并返回 `eligible`、clean count、checked run ids、blocking reasons 和 last gate report。Settings / 实验区域展示 controlled Chat migration pilot 资格；它不是 Chat 切换开关，即使 eligible 也不会自动替换默认 Chat。
- **Controlled Pilot 已进入 Chat 页面**：用户必须显式点击 `Run Controlled Pilot`；执行前先调用 `check_controlled_chat_pilot_eligibility`，blocked 时只展示 blocking reasons 和 fallback，不调用 preview；eligible 时才调用 `run_multi_strategy_agent_preview`，并强制 `allowWrites=false`。成功结果以 “Pilot response” 展示，不作为普通 assistant message，不写入普通 chat history。
- **PlanExecute V1 是受治理 runtime slice**：当前可通过 MultiStrategy preview 产生 planExecute payload/report，但不是产品化周计划流程。
- **LifeModel-HS 仍是协议层方向**：Maturation V1 service、Evidence/Governor 等基础能力已存在，但 Chat 自动成熟化和产品化反馈闭环仍需 gate。
- **RuntimeStrategy trait 已成型**：MultiStrategy Runtime 通过固定 ReAct / PlanExecute adapter registry 执行；这不是插件化加载，也不是默认 Chat 替换。
- **ModelRouter 已毕业**：移除 experimental flag，成为默认路由基础设施。
- **Execution Tools 分层落地**：P1 工具必须有真实 executor 或明确的 proposal-only governed executor 和治理测试；`calendar.propose_event` / `email.propose_draft` 当前只创建 `ScheduledTask` / `DataExport` proposal，不执行真实日历写入或邮件发送。
- **Core OS Tools 注册**：life_model.read、goal.read、memory.search、proposal.list 等 9 个 builtin 工具。
- **下一步仍不能直接替换默认 Chat**：W20 只是 very small controlled pilot。默认 Chat 仍未迁移；下一阶段才允许考虑 reviewed pilot response promotion。
- **文档与 taxonomy 同步**：入口文档和 Tool Taxonomy 必须随代码状态更新，避免后续 Agent 按过期 P1/P2 标签开发。
- **双轨架构**：`use_agent_loop` feature flag 控制 Chat 路径，旧路径完整保留作为 fallback。
- **UI 最小收敛**：导航聚焦 Chat/Review/Runs/Settings，Settings 新增 safe paths 和 AgentLoop toggle。
- **`make ci` 为发布门控**：文档不写死测试数量；以本地 `make ci` 最新结果为准。

下一阶段总纲和架构基准文档见：

- [Plans Document Governance](/Users/fujing/Desktop/偶来福/plans/README.md)
- [OpenLife LifeModel-Governed Agent Runtime Program](/Users/fujing/Desktop/偶来福/plans/openlife_lifemodel_governed_agent_runtime.md)
- [LifeModel-Governed Runtime Progress](/Users/fujing/Desktop/偶来福/plans/lifemodel_governed_runtime_progress.md)
- [OpenLife Agent Framework Architecture](/Users/fujing/Desktop/偶来福/plans/openlife_agent_framework_architecture.md)
- [OpenLife ReAct Beta Roadmap](/Users/fujing/Desktop/偶来福/plans/openlife_react_beta_roadmap.md)

Post-Beta 的下一阶段是 LifeModel-HS MVP：把当前 LifeModel 从 YAML 兼容视图升级为受治理的 Personal Heuristic System。实现入口见：

- [LifeModel-HS MVP Task Specifications](/Users/fujing/Desktop/偶来福/plans/lifemodel_hs_mvp_task_specs.md)
- [ADR 0013: LifeModel-HS Source Of Truth And Governance](/Users/fujing/Desktop/偶来福/plans/adr/0013-lifemodel-hs-source-of-truth-governance.md)
- [LifeModel-HS Architecture Plan](/Users/fujing/Desktop/偶来福/plans/lifemodel_hs_architecture_plan.md)

## 核心能力

| 能力 | 当前状态 | 目标形态 |
|---|---|---|
| LifeModel | 已有四维模型和编辑器 | 成为所有 Agent 任务的私人上下文层 |
| Builder | 已支持快速、渐进、苏格拉底式构建；默认只创建 Proposal | 通过 Review Center 确认后安全写入 LifeModel |
| Chat | 已支持流式对话、历史持久化、AgentRun 和 Chat Proposal；默认主链路尚未切到 MultiStrategy Runtime | 继续稳定迁移受控子路径，展示上下文、模型路由和运行轨迹 |
| MultiStrategy Runtime | Preview/audit-ready：`run_multi_strategy_agent_preview` 可选择 ReAct/PlanExecute/Blocked payload，并写入 metadata-safe 外层 AgentRun audit；Settings Runtime Migration Gate 和 Pilot eligibility 只读展示 gate evidence / pilot 资格；Chat 有 W20 显式 Controlled Pilot 单轮入口 | 继续保持默认 Chat 不迁移；下一阶段才能考虑 reviewed pilot response promotion |
| Runs / Trace | 已能展示 MultiStrategy preview strategy / payload / governance / warnings | 成为所有 runtime strategy 的统一 metadata-safe trace viewer |
| **ModelRouter** | ✅ **任务/隐私感知路由已毕业，带真实健康检查语义** | 按任务类型、隐私需求、成本和延迟智能选择模型 |
| Memory | 已有 SQLite 与向量记忆；Memory Proposal 可写入/归档 | 升级为可治理、可归档、可追踪来源的长期记忆层 |
| MCP/A2A | 已有工具和外部 Agent 接入基础 | 成为 AgentAction 执行层，并默认受权限和审计保护 |
| Tools/Skills | 已有 ToolManifest、MCP/A2A、内置 Skill MVP | 成为 ReAct Agent 的执行能力层，覆盖 Core OS tools、Execution tools、Governance tools、Skill tools |
| Calibration/Evolution | 已有建议和校准雏形 | 统一进入 Proposal/Confirmation 机制 |
| Diagnostics/Safe Mode | 已有试用稳定化能力 | 成为系统控制台和恢复中枢 |
| **Chat Proposal** | ✅ **自动从对话中提取目标/状态/能力** | 自动感知用户意图并生成 LifeModel 更新提案 |
| **ContextAssembler** | ✅ **模块化上下文组装（V2 灰度中）** | 可插拔的记忆/隐私/工具上下文组装 |
| PlanExecute | Governed V1 runtime slice，可在 preview 中生成受治理计划 payload/report | 产品化周计划 vertical slice，必须先经过用户 review/edit |
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

1. [Plans Document Governance](/Users/fujing/Desktop/偶来福/plans/README.md)
2. [OpenLife LifeModel-Governed Agent Runtime Program](/Users/fujing/Desktop/偶来福/plans/openlife_lifemodel_governed_agent_runtime.md)
3. [LifeModel-Governed Runtime Progress](/Users/fujing/Desktop/偶来福/plans/lifemodel_governed_runtime_progress.md)
4. [OpenLife Agent Framework Architecture](/Users/fujing/Desktop/偶来福/plans/openlife_agent_framework_architecture.md)
5. [OpenLife ReAct Beta Roadmap](/Users/fujing/Desktop/偶来福/plans/openlife_react_beta_roadmap.md)
6. [LifeModel-HS MVP Task Specifications](/Users/fujing/Desktop/偶来福/plans/lifemodel_hs_mvp_task_specs.md)
7. [ADR 0013: LifeModel-HS Source Of Truth And Governance](/Users/fujing/Desktop/偶来福/plans/adr/0013-lifemodel-hs-source-of-truth-governance.md)
8. [LifeModel-HS Legacy Write Path Audit](/Users/fujing/Desktop/偶来福/plans/lifemodel_hs_legacy_write_path_audit.md)
9. [LifeModel-HS Architecture Plan](/Users/fujing/Desktop/偶来福/plans/lifemodel_hs_architecture_plan.md)
10. [OpenLife PRD v2: Personal Agent Framework](/Users/fujing/Desktop/偶来福/OpenLife_PRD_v2_Agent_Framework.md)
11. [OpenLife Development Plan](/Users/fujing/Desktop/偶来福/plans/openlife_development_plan.md)
12. [Codex Execution Playbook](/Users/fujing/Desktop/偶来福/plans/openlife_codex_execution_playbook.md)
13. [OpenLife Final PRD](/Users/fujing/Desktop/偶来福/OpenLife_Final_PRD.md)，仅作为历史需求参考

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
- **ModelRouter**：智能路由选择本地/云端模型（默认路由基础设施，云端 Provider 需配置并通过轻量健康检查）

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

### W1-W20: LifeModel-Governed Runtime Preview, Gate Evidence, And Controlled Pilot
- ✅ Tool / Proposal Hygiene、Thin Runtime Spine、ReAct Runtime Contract Convergence。
- ✅ LifeModel Maturation Loop Foundation、LifeModel Governor MVP、PlanExecute Core MVP。
- ✅ StrategySelector、MultiStrategy Runtime Orchestrator、Preview Command。
- ✅ MultiStrategy Preview AgentRun Audit Persistence：metadata-safe 外层 run 可在 Runs / Trace 展示。
- ✅ Non-default Settings preview、guarded Chat preview subpath、Maturation V1 service。
- ✅ PlanExecute governed V1 report、RuntimeStrategy trait、ReAct / PlanExecute adapter registry。
- ✅ Runtime Migration Gate：只读诊断默认 Chat 未替换、preview 健康、metadata-safe trace、fallback、无外部写入和 proposal-first 边界，并在 Settings 显式展示 evidence。
- ✅ Sustained Gate Evidence / Pilot Eligibility：只读检查最近 3 条 preview gate report 是否连续干净，展示 clean count、checked run ids、blocking reasons；不创建 AgentRun、Proposal、Action、Observation。
- ✅ Very Small Controlled Chat Migration Pilot With Fallback：Chat 页面新增显式 `Run Controlled Pilot` 单轮入口；先查 eligibility，blocked 不调用 preview；eligible 后才运行 `allowWrites=false` preview；成功只显示 “Pilot response”，不写普通 chat history，默认 Send 不变。

## 当前重要开发方向

1. 保持 `send_message` / `start_stream_message` 默认 Chat 主路径稳定，不能直接替换。
2. 用 Settings Runtime Migration Gate 或 `check_runtime_migration_gate` 对最近 preview AgentRun 做只读迁移诊断。
3. 用 Settings Pilot eligibility 或 `check_controlled_chat_pilot_eligibility` 对最近 3 条 preview gate evidence 做只读资格检查；普通 Chat Send 不调用该 command。
4. Chat 页面 Controlled Pilot 只能由用户显式点击触发；blocked/failed 时显示 fallback，不自动重试；普通 Send 保持可用且不调用 eligibility/gate/preview。
5. 下一阶段才允许考虑 reviewed pilot response promotion；promotion 之前，pilot response 不能伪装成普通 assistant message，也不能自动写入 chat history。

## 常见问题

- API Key 测试失败：确认 Provider、Base URL、模型名和 API Key 匹配。
- Ollama 连接失败：确认 Ollama 已启动，且模型名称存在。
- Safe Mode：说明当前数据环境存在风险，先去 Settings 的恢复控制台导出备份并修复。
- Chat 无响应或一直思考：先查看 Settings 诊断，再检查模型 Provider 测试结果。
- Builder Review 后模型没有变化：先确认 Proposal 是否仍在 Review Center 待处理；Builder 默认不会绕过确认直接写入。

## License

MIT License
