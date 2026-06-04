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

- **当前阶段是 W123 ReAct Beta Execution Hardening complete**：W114-W123 已在 W106-W113 RuntimeStrategy maturity 之上完成 ReAct readiness/status、AgentLoop typed action schema 与 fail-soft parser、Tool Registry Beta taxonomy/readiness、manifest-authoritative ActionExecutor、`react_trace` envelope、permission/replay hardening、proposal-first write hardening、非默认 `get_react_beta_execution_status`、Runs/Trace lifecycle UI hardening 和 docs/progress sync。
- **六个大板块按各自 scope 已完成**：Default Chat Adapter guard/prep（W65-W72）、LifeModel Maturation proof slice（W73-W78）、Legacy Direct-Write Convergence（W90-W97）、Plan-Execute Product Vertical（W98-W105）、RuntimeStrategy / Multi-Strategy Runtime Maturity（W106-W113）、ReAct Beta Execution Hardening（W114-W123）。
- **Default Chat 仍保持 `legacy_stream`**：普通 `Send` / `send_message` / `start_stream_message` 只允许进入 legacy path，并只能调用 W49-W55 pure ordinary-entry guard/preflight。W19-W123 的 readiness/status/proof/report/review/trace 结果都不是 migration permission。
- **Plan-Execute 已有非默认产品纵切**：W98-W105 提供 weekly planning session、review/edit/finalize、proposal-first step execution、AgentRun/trace linkage 和 Workspace/Runs surface；它不是 default Chat migration，也不是外部 provider 写入。
- **完整 Beta 尚未宣告**：W114-W123 提升了 ReAct 执行严肃性和可观察性，但完整 Beta 仍需要 Skill Runtime、ModelRouter/Privacy、跨产品 LifeModel/Memory governance golden path，以及任何 default Chat route migration 的单独人工 review。
- **文档与 taxonomy 是硬约束**：入口文档、progress index、Tool Taxonomy 和代码状态必须同步。过期 P1/P2 标签、旧 W60/W65 当前状态、或把 readiness 当迁移许可的文案都视为开发阻塞项。

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
| LifeModel | 已有四维模型、编辑器、Proposal-first 更新基础和 W73-W78 maturation proof slice | 成为所有 AgentTask 的私人协议层和可治理 source-of-truth |
| Builder / Calibration / Feedback | 已收敛到 Proposal / Review Center，W90-W97 移除高风险 legacy direct-write | 继续作为 LifeModel maturation 和用户确认闭环的输入面 |
| Chat | 支持流式对话、历史持久化、AgentRun 和 Chat Proposal；default Chat 仍是 `legacy_stream` | 任何 route migration 必须单独规划、人工验收，不能由 readiness/status 自动授权 |
| Default Chat Adapter Guard | W65-W72 完成 backend-only descriptor、contract、harness、send/stream proof、gate、disabled skeleton 和 integrity proof | 仅作为未来受控迁移准备；当前不接 ordinary Chat，不接 executor |
| MultiStrategy Runtime | W106-W113 完成 strategy descriptor、registry readiness、selection matrix、execution envelope、status command 和 trace vocabulary | 支持 ReAct / PlanExecute 之外的未来策略，但 disabled/declarative-only 策略不能伪装可执行 |
| ReAct Execution | W114-W123 完成 Beta execution hardening：action schema/parser、Tool Registry readiness、manifest authority、trace、permission/replay、proposal-first writes | 继续补齐 Skill Runtime、ModelRouter/Privacy 和产品 golden path 后再评估完整 Beta |
| PlanExecute | W98-W105 完成非默认 weekly planning 产品纵切：session lifecycle、review/edit/finalize、proposal-first step execution、trace linkage | 扩展更多 Plan-Execute 产品场景，并保持非默认/受治理边界 |
| Runs / Trace | 支持 preview/product/ReAct trace lifecycle 的 metadata-safe 展示 | 成为所有 runtime strategy 的统一可审计视图 |
| ModelRouter | 已具备任务/隐私感知路由和健康检查语义 | 继续强化 privacy policy、local-only 阻断和 route trace |
| Memory | 已有 SQLite、向量记忆、Memory Proposal 和治理化归档基础 | 升级为来源可追踪、可回滚、可审计的长期记忆层 |
| Tools / Skills | ToolManifest、MCP/A2A、proposal-only file/calendar/email/task 工具和内置 Skill MVP 已存在 | 完整 Skill Runtime 和真实 executor 接入必须遵守 permission/proposal/audit |
| Workspace / Review / Settings | 已有 Workspace、Review Center、Runs、Settings evidence/status surfaces | 继续作为 Agent OS control plane，而不是新增孤立页面 |
| Diagnostics / Safe Mode | 已有恢复、诊断、网络策略和安全模式基础 | 成为系统恢复、策略检查和发布门控的一部分 |

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

### W1-W60: LifeModel-Governed Runtime Preview, Gate Evidence, Controlled Pilot, Promotion Evidence, Readiness, Draft Planning, Review Decision Evidence, Implementation Gate, Shadow Run, Shadow Review Evidence, Cutover Readiness, Cutover Candidate Adapter, Candidate Review Evidence, Candidate Promotion Readiness, Default Chat Runtime Boundary, Activation Plan Draft, Activation Review Evidence, Activation Implementation Gate, Disabled Routing Scaffold, Contract Harness, Dry-Run Invocation Boundary, Dry-Run Review Evidence, Implementation Readiness Gate, Controlled Preview, Controlled Preview Review Evidence, Controlled Preview Approval Readiness, Cutover Implementation Plan Draft, Cutover Plan Review Evidence, Cutover Plan Approval Readiness, Route Guard Scaffold, Cutover Invocation Harness, Invocation Plan, Invocation Boundary, Typed Callsite Contract, Authority Roadmap Sync, Ordinary Entry Preflight, Ordinary Entry Preflight Status, Narrow Implementation Discussion Gate, Narrow Implementation Plan Draft, Narrow Implementation Plan Review Evidence, And Narrow Implementation Plan Approval Readiness
- ✅ Tool / Proposal Hygiene、Thin Runtime Spine、ReAct Runtime Contract Convergence。
- ✅ LifeModel Maturation Loop Foundation、LifeModel Governor MVP、PlanExecute Core MVP。
- ✅ StrategySelector、MultiStrategy Runtime Orchestrator、Preview Command。
- ✅ MultiStrategy Preview AgentRun Audit Persistence：metadata-safe 外层 run 可在 Runs / Trace 展示。
- ✅ Non-default Settings preview、guarded Chat preview subpath、Maturation V1 service。
- ✅ PlanExecute governed V1 report、RuntimeStrategy trait、ReAct / PlanExecute adapter registry。
- ✅ Runtime Migration Gate：只读诊断默认 Chat 未替换、preview 健康、metadata-safe trace、fallback、无外部写入和 proposal-first 边界，并在 Settings 显式展示 evidence。
- ✅ Sustained Gate Evidence / Pilot Eligibility：只读检查最近 3 条 preview gate report 是否连续干净，展示 clean count、checked run ids、blocking reasons；不创建 AgentRun、Proposal、Action、Observation。
- ✅ Very Small Controlled Chat Migration Pilot With Fallback：Chat 页面新增显式 `Run Controlled Pilot` 单轮入口；先查 eligibility，blocked 不调用 preview；eligible 后才运行 `allowWrites=false` preview；成功默认只显示 “Pilot response”，不自动写普通 chat history，默认 Send 不变。
- ✅ Reviewed Pilot Response Promotion：成功且包含 `userOutput` 的 pilot response 可由用户显式 review/confirm 后提升为一条 ordinary assistant message；取消、blocked、failed、no-output、重复 promotion 均不写入，promotion 不写 LifeModel/Memory/Proposal/外部工具结果。
- ✅ Post-Promotion Validation And Source Binding：Controlled Pilot 结果绑定 source session；review 展示 source/target session、runId、strategy 和 governance summary；确认前校验 source/target 一致，session mismatch 时不调用 `save_chat_message`，显示 blocking/fallback 和重新运行 pilot 提示。
- ✅ Controlled Pilot Promotion Evidence Recorder：确认 promotion 且 assistant message 保存成功后，写入一条 metadata-safe runtime evidence；只保存 pilotRunId、source/target session、strategy/payload/governance、message length、checksum 和 promotedAt。Settings 实验区只读 summary 展示 promoted count、recent pilot run ids、latest timestamp 和 mismatch block count；evidence 失败显示 degraded/error，重试不重复写 chat message。
- ✅ Promotion Evidence Readiness Gate：新增只读 `check_controlled_pilot_promotion_readiness`，默认要求 3 条 metadata-safe promotion evidence；Settings 实验区展示 ready/block、counts、recent pilot run ids、blocking reasons 和 mismatch block count。`sessionId` 已预留，当前 EvidenceStore 不支持时按 global summary 读取；默认 Send 不调用该 gate。
- ✅ Reviewed Migration Plan Draft Generator：新增只读 `draft_controlled_chat_migration_plan`，复用 W24 readiness gate。blocked 时不生成 plan sections；passed 时生成仅供人工评审的 scope/preconditions/rollback/fallback/test plan。Settings 实验区展示 Draft Migration Plan 面板；默认 Send 不调用该 command。
- ✅ Manual Migration Review Decision Evidence：新增 `record_controlled_chat_migration_review_decision` 和只读 summary command。record 先调用 W25 draft；blocked draft approve 不写 evidence；ready draft 可记录 approve/reject/request_rework metadata-safe evidence，reviewer note 仅存 length/checksum/category。Settings 实验区展示 Migration Review Decision 面板；approval 不是 Chat migration。
- ✅ Approved Migration Implementation Gate：新增只读 `check_controlled_chat_migration_implementation_gate`。它要求 latest metadata-safe decision 为 approve、当前 W25 draft hash 与 approved evidence draftHash 匹配、当前 W24 readiness 通过；reject/request_rework、hash mismatch 或 readiness blocked 均阻断。Settings 实验区展示 Implementation Gate；eligible 也不会切换 default Chat。
- ✅ Non-Default Controlled Migration Shadow Run：新增 `run_controlled_chat_migration_shadow_run`。先查 W27 implementation gate；blocked 不执行 runtime；eligible 后才运行 write-disabled bounded controlled runtime preview，并只返回 metadata-safe strategy/payload/summary/warnings/blockers。可写 metadata-safe shadow AgentRun audit，但不写 Chat message、Proposal、Memory、LifeModel patch、Evidence 或外部工具结果；默认 Send 不调用它。
- ✅ Controlled Chat Migration Shadow Review Evidence：新增 `record_controlled_chat_migration_shadow_review_decision` 和只读 summary command。只记录人工 approve/reject/request_rework；所有 decision 都必须绑定已完成且 metadata-safe、write-disabled、无副作用的 shadow AgentRun。Evidence metadata 只保存 shadowRunId、decisionKind、reviewerNote checksum/length/category、readiness digest 和 createdAt；Settings Shadow Review 不自动触发，默认 Send 不调用它。
- ✅ Controlled Chat Cutover Planning Readiness Gate：新增只读 `check_controlled_chat_cutover_readiness`。它要求 W27 implementation gate 当前 eligible、latest W29 shadow review decision 为 approve、approved shadowRunId 对应 AgentRun 仍存在且 completed/write-disabled/metadata-safe/side-effect-free。Settings Cutover Readiness 只能显式点击检查；pass 只表示可进入默认 Chat 迁移实现讨论，不迁移默认 Chat。
- ✅ Non-Default Controlled Chat Cutover Candidate Adapter：新增显式 `run_controlled_chat_cutover_candidate`。它先调用 W30 readiness；blocked 时不运行 runtime，eligible 后才执行一次 `allowWrites=false`、`maxToolCalls=0` 的 controlled runtime candidate，返回 `candidateReady`、`candidateRunId`、`outputPreview`/`userOutput`、`contractShape`、metadata-safe summary、warnings 和 blockers。允许 metadata-safe AgentRun audit；不保存 raw prompt/output/tool payload，不写 Chat/Proposal/Memory/LifeModel/Evidence/MCP audit/外部工具结果。Settings Cutover Candidate 只能人工点击运行，默认 Send 不调用它。
- ✅ Controlled Chat Cutover Candidate Review Evidence：新增 `record_controlled_chat_cutover_candidate_review_decision` 和只读 summary command。只允许人工 approve/reject/request_rework；approve 要求 candidate AgentRun 已完成、strategy/contract shape/candidateReady/runtime limits/storage/side-effect audit 全部符合 W32 约束。Evidence 只保存 candidateRunId、decisionKind、contractShape、candidateSummaryDigest、reviewerNote checksum/length/category 和 createdAt；不保存 reviewer 原文、candidate output、raw prompt/output 或 tool payload。Settings Cutover Candidate Review 只能显式记录/刷新，默认 Send 不调用它。
- ✅ Controlled Chat Cutover Candidate Promotion Readiness Gate：新增只读 `check_controlled_chat_cutover_candidate_promotion_readiness`。它复用 W30 readiness，读取 W32 candidate review evidence，要求 latest decision 为 approve、approved candidate run 仍存在且 completed/send_message-compatible/write-disabled/zero-tool/metadata-safe/side-effect-free，并返回 ready/blockers/approved candidate counts/latest decision/defaultChatUnchanged/metadata-safe summary。Settings 只能显式刷新，默认 Send 不调用它。
- ✅ Default Chat Runtime Boundary Status：新增只读 `get_default_chat_runtime_boundary_status`。它固定返回 `currentMode=legacy_stream`、`defaultChatUnchanged=true`、`automaticMigrationEnabled=false`、`controlledCandidateAvailable=false` 和 `candidatePromotionReadinessRequired=true`，只用于显式观察默认 Chat 仍未迁移；不读取/写入任何 runtime/evidence/proposal/memory/lifemodel/chat/tool/model 状态。Settings 只能显式刷新，默认 Send 不调用它。
- ✅ Default Chat Adapter Activation Plan Draft：新增只读 `draft_default_chat_adapter_activation_plan`。它组合 W33 candidate promotion readiness 与 W34 default Chat boundary status；blocked 时不生成 plan sections，ready 时只返回 human-review-only activation scope、preconditions、adapter contract checks、fallback、rollback、observability 和 test plan，并固定 `manualReviewRequired=true`、`notAutomaticMigration=true`、`requiresSeparateImplementation=true`。Settings 只能显式刷新，默认 Send 不调用它。
- ✅ Default Chat Adapter Activation Review Decision Evidence：新增 `record_default_chat_adapter_activation_review_decision` 和只读 `get_default_chat_adapter_activation_review_summary`。record 会先调用 W35 draft；blocked draft approve 不写 evidence；ready draft 可记录 approve/reject/request_rework metadata-safe evidence，reviewer note 仅存 checksum/length/category。Settings 只能显式记录/刷新，默认 Send 不调用它。
- ✅ Default Chat Adapter Activation Implementation Gate：新增只读 `check_default_chat_adapter_activation_implementation_gate`。它组合当前 W35 stable activation plan digest 与 W36 latest metadata-safe activation review decision evidence；latest approve、draft ready、digest match、candidate promotion ready、default Chat 仍为 legacy stream 且 automatic migration disabled 时才 eligible。Settings 只能显式检查，默认 Send 不调用它。
- ✅ Default Chat Adapter Disabled Routing Scaffold：新增只读 `get_default_chat_adapter_routing_status`。它调用 W37 gate，但固定保持 `currentMode=legacy_stream`、`adapterScaffoldPresent=true`、`controlledAdapterEnabled=false`、`defaultSendPath=legacy_stream` 和 `startStreamPath=legacy_stream`，只展示 disabled scaffold 状态与 blockers。Settings 只能显式刷新，默认 Send 不调用它。
- ✅ Default Chat Adapter Contract Harness：新增只读 `check_default_chat_adapter_contract_harness`。它调用 W38 routing status，检查 send_message / start_stream_message contract 仍为 legacy stream、controlled adapter disabled、activation implementation gate eligible，并返回 metadata-safe contract checks。Settings 只能显式检查，默认 Send 不调用它。
- ✅ Default Chat Adapter Dry-Run Invocation Boundary：新增显式 `run_default_chat_adapter_dry_run`。它先检查 W39 contract harness；blocked 时不运行 dry run，ready 时只返回 metadata-safe dry-run contract result，强制 `allowWrites=false`、`maxToolCalls=0`、`defaultChatPathUnchanged=true`，不保存 Chat、不创建 AgentRun/Evidence/Proposal/Memory/LifeModel/MCP audit/external write、不运行 runtime/tool/model call、不切换 routing。Settings 只能显式运行 dry run，默认 Send 不调用它。
- ✅ Default Chat Adapter Dry-Run Review Evidence：新增 `record_default_chat_adapter_dry_run_review_decision` 和只读 `get_default_chat_adapter_dry_run_review_summary`。record 会先重新运行 W40 dry run；approve 只在 dry run ready 时写 metadata-safe evidence，blocked approve 不写 evidence，reject/request_rework 只写白名单 metadata。reviewer note 仅存 checksum/length/category；默认 Send 不调用它。
- ✅ Default Chat Adapter Implementation Readiness Gate：新增只读 `check_default_chat_adapter_implementation_readiness`。它组合 W37/W39/W40/W41 当前证据，要求 activation implementation gate eligible、contract harness ready、dry run ready、latest dry-run review approve、dry-run digest match、default Chat unchanged、controlled adapter disabled、automatic migration disabled、send/stream 均保持 `legacy_stream`。Settings 只能显式检查，默认 Send 不调用它。
- ✅ Default Chat Adapter Controlled Preview：新增显式非默认 `run_default_chat_adapter_controlled_preview`。它先检查 W42 implementation readiness；blocked 不运行 runtime、不创建 AgentRun；ready 后才运行一次 write-disabled/zero-tool controlled preview，返回 SendMessageResult-compatible shape，并只写 metadata-safe adapter preview AgentRun audit；不保存 Chat、不 promotion、不切换 routing。Settings 只能显式运行 preview，默认 Send 不调用它。
- ✅ Default Chat Adapter Controlled Preview Review Evidence：新增 `record_default_chat_adapter_controlled_preview_review_decision` 和只读 `get_default_chat_adapter_controlled_preview_review_summary`。approve 必须绑定 completed / `default_chat_adapter_controlled_preview` / send-message-compatible / previewReady / write-disabled / zero-tool / metadata-safe / side-effect-free preview AgentRun；reject/request_rework 也只写白名单 metadata。reviewer note 仅保存 checksum/length/category，不保存原文、preview output、raw prompt/output 或 tool payload；默认 Send 不调用它。
- ✅ Default Chat Adapter Controlled Preview Approval Readiness Gate：新增只读 `check_default_chat_adapter_controlled_preview_approval_readiness`。它组合 W42 implementation readiness、W44 latest approve evidence、required approved preview count、digest match 和 approved W43 preview AgentRun 当前安全状态；不创建记录、不运行 preview/runtime/tool/model call、不切换 routing；默认 Send 不调用它。
- ✅ Default Chat Adapter Cutover Implementation Plan Draft：新增只读 `draft_default_chat_adapter_cutover_implementation_plan`。它只调用 W45 readiness；blocked 时不生成 plan sections，ready 时只返回 metadata-safe human-review implementation scope、adapter contract requirements、routing boundary、safety preconditions、fallback、rollback、observability、test plan、explicit non-goals 和 stable plan digest；不创建记录、不运行 preview/runtime/tool/model call、不切换 routing；默认 Send 不调用它。
- ✅ Default Chat Adapter Cutover Plan Review Evidence：新增 `record_default_chat_adapter_cutover_plan_review_decision` 和只读 `get_default_chat_adapter_cutover_plan_review_summary`。record 会先调用 W46 draft；blocked draft approve 不写 evidence，reject/request_rework 可写 metadata-safe evidence；reviewer note 仅存 checksum/length/category；默认 Send 不调用它。
- ✅ Default Chat Adapter Cutover Plan Approval Readiness Gate：新增只读 `check_default_chat_adapter_cutover_plan_approval_readiness`。它组合当前 W46 draft、W47 latest approve evidence、plan digest match、W45 readiness 与 default Chat isolation；ready 只表示可进入后续 adapter implementation discussion，不迁移 default Chat；默认 Send 不调用它。
- ✅ Default Chat Adapter Cutover Route Guard Scaffold：新增共享 `default_chat_adapter` route resolver 和 fail-closed guard。`get_default_chat_adapter_routing_status`、`send_message`、`start_stream_message` 使用同一 route source-of-truth；默认仍为 `legacy_stream` 且 controlled adapter / automatic migration disabled。若未来路径漂移或 adapter 被误启用，默认 Chat 入口会阻断而不是静默切换；不调用 W19-W48 gates，不运行 runtime/tool/model call，不写任何业务数据。
- ✅ Default Chat Adapter Cutover Invocation Harness：新增纯后端 `DefaultChatAdapterCutoverHarness`、`evaluate_default_chat_adapter_cutover_harness` 与 `ensure_default_chat_cutover_harness`。默认 Send / `send_message` / `start_stream_message` 现在只通过该 harness guard 确认 `legacy_guarded` invocation mode、write-disabled/zero-tool/no-runtime/no-model/no-tool/no-business-write 边界；route drift、adapter scaffold 缺失、controlled adapter/automatic migration 误启用或 separate implementation 约束消失时 fail closed。它不是 default Chat migration。
- ✅ Default Chat Adapter Invocation Plan：新增纯后端 `DefaultChatAdapterInvocationPlan`、`plan_default_chat_adapter_invocation` 与 `ensure_default_chat_adapter_invocation_plan`。默认 Send / `send_message` / `start_stream_message` 现在通过 invocation plan guard 明确选择 `legacy_stream`，保留 `controlled_adapter` 为 disabled candidate，并固定 send/stream contract shape、write-disabled、zero-tool、no-runtime/no-model/no-tool/no-business-write 边界；W50 harness blocking 会让 plan blocking。它不是 default Chat migration。
- ✅ Default Chat Adapter Invocation Boundary：新增纯后端 `DefaultChatAdapterInvocationBoundary`、`evaluate_default_chat_adapter_invocation_boundary` 与 `ensure_default_chat_adapter_invocation_boundary`。默认 Send / `send_message` / `start_stream_message` 现在通过 invocation boundary guard 复用 W51 plan，只允许进入 `legacy_stream` callsite，要求 controlled executor unattached、write-disabled、zero-tool、side-effect-free before legacy entry；W51 plan blocking 会让 boundary blocking。它不是 default Chat migration。
- ✅ Default Chat Adapter Typed Callsite Contract：新增纯后端 `DefaultChatAdapterCallsite`、`DefaultChatAdapterCallsiteContract`、`evaluate_default_chat_adapter_callsite_contract` 与 `ensure_default_chat_adapter_callsite_contract`。默认 Send / `send_message` / `start_stream_message` 现在通过 typed callsite contract guard 分别声明 send/stream contract shape，并校验各自 actual route path 必须保持 `legacy_stream`；W52 boundary blocking 或 callsite route drift 都会 fail closed。它不是 default Chat migration。
- ✅ Authority Roadmap Sync：W54 将高优先级 roadmap 与 execution docs 从旧 W22 状态同步到 W54/W1-W53 当前代码状态，避免后续 Agent 按过期路线开发。它不是 default Chat migration。
- ✅ Default Chat Adapter Ordinary Entry Preflight：W55 新增纯后端 ordinary-entry preflight / side-effect lock。默认 Send / `send_message` / `start_stream_message` 现在通过 preflight guard 明确要求 typed contract ready、legacy entry allowed、controlled executor unattached、default migration disabled 和零副作用预算；route drift 或 contract blocking 会 fail closed。它不是 default Chat migration。
- ✅ Default Chat Adapter Ordinary Entry Preflight Status：W56 新增只读 status command、frontend wrapper 和 Settings evidence surface。它只展示 send/stream W55 preflight 状态、side-effect lock 和 metadata-safe summary；不运行 runtime/model/tool，不写任何业务数据，不迁移 default Chat。
- ✅ Default Chat Adapter Narrow Implementation Discussion Gate：W57 新增只读 discussion gate、frontend wrapper 和 Settings evidence surface。它组合 W48 cutover plan approval readiness 与 W56 ordinary-entry preflight status；eligible 只表示可讨论更窄 adapter implementation slice，不运行 runtime/model/tool，不写记录，不切换 routing，不迁移 default Chat。
- ✅ Default Chat Adapter Narrow Implementation Plan Draft：W58 新增只读 `draft_default_chat_adapter_narrow_implementation_plan`、frontend wrapper 和 Settings evidence surface。它先调用 W57 gate；blocked 时不生成 plan sections，eligible 时只返回 metadata-safe human-review plan sections 与 stable digest；不创建记录、不运行 runtime/model/tool/preview、不切换 routing，默认 Send 不调用它。
- ✅ Default Chat Adapter Narrow Implementation Plan Review Evidence：W59 新增 `record_default_chat_adapter_narrow_implementation_plan_review_decision` 与只读 summary、frontend wrapper 和 Settings evidence surface。它先调用 W58 draft；blocked draft approve 不写 evidence，ready draft decision 只写 metadata-safe Evidence；reviewer note 仅 checksum/length/category，默认 Send 不调用它。
- ✅ Default Chat Adapter Narrow Implementation Plan Approval Readiness Gate：W60 新增只读 `check_default_chat_adapter_narrow_implementation_plan_approval_readiness`、frontend wrapper 和 Settings evidence surface。它组合当前 W58 draft、W59 latest approve evidence、digest match、W57 eligible 与 default Chat isolation；不写记录、不运行 runtime/model/tool/preview、不切换 routing，默认 Send 不调用它。

## 当前重要开发方向

1. 保持 `send_message` / `start_stream_message` 默认 Chat 主路径稳定，不能直接替换。
2. 用 Settings Runtime Migration Gate 或 `check_runtime_migration_gate` 对最近 preview AgentRun 做只读迁移诊断。
3. 用 Settings Pilot eligibility 或 `check_controlled_chat_pilot_eligibility` 对最近 3 条 preview gate evidence 做只读资格检查；普通 Chat Send 不调用该 command。
4. Chat 页面 Controlled Pilot 只能由用户显式点击触发；blocked/failed 时显示 fallback，不自动重试；普通 Send 保持可用且不调用 eligibility/gate/preview。
5. Pilot response 默认隔离；只有用户显式点击 `Promote Pilot Response`、确认 review，且当前 target session 与 pilot source session 一致后，才写入一条 ordinary assistant message，并记录 metadata-safe promotion evidence。不得自动 promotion，不得把 promotion 当成默认 Chat 迁移；默认 Send 路径不得调用 evidence recorder。
6. 用 Settings Promotion readiness 或 `check_controlled_pilot_promotion_readiness` 只读判断是否具备讨论下一步 Chat migration 的资格；ready 不是自动迁移许可。
7. 用 Settings Draft Migration Plan 和 Migration Review Decision 进行人工决策记录；approve 只允许进入下一阶段 implementation discussion，不是默认 Chat migration，默认 Send 路径不得调用 review decision record/summary。
8. 用 Settings Implementation Gate 或 `check_controlled_chat_migration_implementation_gate` 只读判断是否具备进入 controlled Chat migration implementation discussion 的资格；eligible 不是默认 Chat migration，默认 Send 路径不得调用 implementation gate。
9. 用 Settings Shadow Run 或 `run_controlled_chat_migration_shadow_run` 做非默认 controlled migration shadow 对比；只有 implementation gate eligible 才执行 runtime，且必须 `allowWrites=false`、metadata-safe、不写 Chat/Proposal/Memory/LifeModel/Evidence/外部工具结果。默认 Send 路径不得调用 shadow run。
10. 用 Settings Shadow Review 或 `record_controlled_chat_migration_shadow_review_decision` 人工记录 shadow run 审阅证据；approve 只是 evidence，不是默认 Chat 迁移许可。默认 Send 路径不得调用 shadow review record/summary。
11. 用 Settings Cutover Readiness 或 `check_controlled_chat_cutover_readiness` 只读判断是否可以进入默认 Chat 迁移实现讨论；eligible 不是默认 Chat migration，默认 Send 路径不得调用 cutover readiness。
12. 用 Settings Cutover Candidate 或 `run_controlled_chat_cutover_candidate` 显式验证 controlled runtime candidate 是否产出 Chat-compatible contract shape；candidateReady 不是默认 Chat migration，默认 Send 路径不得调用 cutover candidate。
13. 用 Settings Cutover Candidate Review 或 `record_controlled_chat_cutover_candidate_review_decision` 人工记录 candidate review evidence；approve 只是 metadata-safe evidence，不是默认 Chat migration，默认 Send 路径不得调用 candidate review record/summary。
14. 用 Settings Candidate Promotion Readiness 或 `check_controlled_chat_cutover_candidate_promotion_readiness` 只读判断 W30/W32 证据是否足以进入后续 adapter boundary / activation planning；ready 不是默认 Chat migration，默认 Send 路径不得调用该 gate。
15. 用 Settings Default Chat Runtime Boundary 或 `get_default_chat_runtime_boundary_status` 只读观察默认 Chat 仍是 legacy stream path；它不是 activation control，默认 Send 路径不得调用该 command。
16. 用 Settings Default Chat Adapter Activation Plan 或 `draft_default_chat_adapter_activation_plan` 只读生成人工 activation plan draft；draftReady 不是 migration approval，默认 Send 路径不得调用该 command。
17. 用 Settings Default Chat Adapter Activation Review Decision 或 `record_default_chat_adapter_activation_review_decision` 人工记录 activation plan 审阅证据；approve 不是默认 Chat 迁移许可，默认 Send 路径不得调用 record/summary command。
18. 用 Settings Default Chat Adapter Activation Implementation Gate 或 `check_default_chat_adapter_activation_implementation_gate` 只读判断 W35/W36 证据是否足以进入 separate implementation discussion；eligible 不是默认 Chat migration，默认 Send 路径不得调用该 gate。
19. 用 Settings Default Chat Adapter Routing Status 或 `get_default_chat_adapter_routing_status` 只读观察 adapter scaffold 仍为 disabled；它不是 routing switch，默认 Send 路径不得调用该 command。
20. 用 Settings Default Chat Adapter Contract Harness 或 `check_default_chat_adapter_contract_harness` 只读验证 disabled adapter contract；它不是 adapter implementation，默认 Send 路径不得调用该 command。
21. 用 Settings Default Chat Adapter Dry Run 或 `run_default_chat_adapter_dry_run` 显式验证未来 adapter invocation contract 的 write-disabled 形状；它不是默认 Chat migration，默认 Send 路径不得调用该 command。
22. 用 Settings Default Chat Adapter Dry Run Review 或 `record_default_chat_adapter_dry_run_review_decision` 人工记录 dry-run review evidence；approve 只是 metadata-safe evidence，不是默认 Chat migration，默认 Send 路径不得调用 record/summary command。
23. 用 Settings Default Chat Adapter Implementation Readiness 或 `check_default_chat_adapter_implementation_readiness` 只读判断 W37/W39/W40/W41 证据是否足以进入真正 adapter implementation coding discussion；implementationReady 不是默认 Chat migration，默认 Send 路径不得调用该 command。
24. 用 Settings Default Chat Adapter Controlled Preview 或 `run_default_chat_adapter_controlled_preview` 显式验证 W42 之后的非默认 adapter preview 是否能返回 Send-compatible shape；previewReady 不是默认 Chat migration，默认 Send 路径不得调用该 command。
25. 用 Settings Default Chat Adapter Controlled Preview Review 或 `record_default_chat_adapter_controlled_preview_review_decision` 人工记录 controlled preview review evidence；approve 只是 metadata-safe evidence，不是默认 Chat migration，默认 Send 路径不得调用 record/summary command。
26. 用 Settings Default Chat Adapter Controlled Preview Approval Readiness 或 `check_default_chat_adapter_controlled_preview_approval_readiness` 只读判断 W42/W44 证据和 approved preview AgentRun 当前安全状态是否足以进入后续 adapter cutover implementation discussion；ready 不是默认 Chat migration，默认 Send 路径不得调用该 command。
27. 用 Settings Default Chat Adapter Cutover Implementation Plan 或 `draft_default_chat_adapter_cutover_implementation_plan` 只读生成 W45 readiness 之后的人工 cutover implementation plan draft；draftReady 不是默认 Chat migration，默认 Send 路径不得调用该 command。
28. 用 Settings Default Chat Adapter Cutover Plan Review 或 `record_default_chat_adapter_cutover_plan_review_decision` 人工记录 cutover plan review evidence；approve 只是 metadata-safe evidence，不是默认 Chat migration，默认 Send 路径不得调用 record/summary command。
29. 用 Settings Default Chat Adapter Cutover Plan Approval Readiness 或 `check_default_chat_adapter_cutover_plan_approval_readiness` 只读判断 W46/W47 证据、plan digest match 和 default Chat isolation 是否足以进入后续 adapter implementation discussion；ready 不是默认 Chat migration，默认 Send 路径不得调用该 command。
30. W49 的 default Chat route guard 是纯后端 fail-closed 守卫；默认 Send / `send_message` / `start_stream_message` 可以调用它确认当前仍为 `legacy_stream`，但不能借此启用 controlled adapter 或自动迁移。
31. W50 的 default Chat adapter cutover invocation harness 是纯后端 guard；默认 Send / `send_message` / `start_stream_message` 只能用它确认 `legacy_guarded`、write-disabled、zero-tool、no-runtime/no-model/no-tool/no-business-write 边界，不能借此调用 controlled adapter 或自动迁移。
32. W51 的 default Chat adapter invocation plan 是纯后端 guard；默认 Send / `send_message` / `start_stream_message` 只能用它选择 `legacy_stream` 并声明 `controlled_adapter` 仍是 disabled candidate，不能借此 attach controlled executor 或自动迁移。
33. W52 的 default Chat adapter invocation boundary 是纯后端 guard；默认 Send / `send_message` / `start_stream_message` 只能用它确认当前 callsite 必须进入 `legacy_stream` 且在 legacy entry 前无 runtime/model/tool/business write 副作用，不能借此接入 controlled executor 或自动迁移。
34. W53 的 default Chat adapter typed callsite contract 是纯后端 guard；默认 Send / `send_message` / `start_stream_message` 只能用它通过类型化 callsite 绑定 `send_message_compatible` / `stream_message_compatible` contract shape 和各自 legacy route path，不能借此接入 controlled executor 或自动迁移。
35. W54 的 authority roadmap sync 是文档治理工作；高优先级路线文件必须与当前代码状态同步，不能再按旧 W22 “下一步”误导后续开发。
36. W55 的 default Chat adapter ordinary-entry preflight 是纯后端 side-effect lock；默认 Send / `send_message` / `start_stream_message` 只能用它确认 typed contract ready、legacy entry allowed、controlled executor unattached 和零副作用预算，不能借此调用 controlled adapter 或自动迁移。
37. W56 的 default Chat adapter ordinary-entry preflight status 是只读 evidence surface；Settings 可显式刷新它，但普通 Send 路径不得调用该 command，也不能把 statusReady 解释为迁移许可。
38. W57 的 default Chat adapter narrow implementation discussion gate 是只读讨论资格 gate；Settings 可显式检查 W48 cutover plan approval readiness 与 W56 ordinary-entry preflight status 是否同时干净，但普通 Send 路径不得调用该 command，也不能把 eligible 解释为迁移许可。
39. W58 的 default Chat adapter narrow implementation plan draft 是只读人工评审草案；Settings 可显式生成 metadata-safe plan sections 和 stable digest，但普通 Send 路径不得调用该 command，也不能把 draftReady 解释为迁移许可。
40. W59 的 default Chat adapter narrow implementation plan review evidence 只记录人工 review metadata；Settings 可显式 approve/reject/request_rework 并读取 summary，但普通 Send 路径不得调用该 command，也不能把 approval 解释为迁移许可。
41. W60 的 default Chat adapter narrow implementation plan approval readiness 是只读 gate；Settings 可显式检查当前 W58 draft 与 W59 approve evidence 是否仍匹配且 default Chat isolation 仍干净，但普通 Send 路径不得调用该 command，也不能把 ready 解释为迁移许可。

## 常见问题

- API Key 测试失败：确认 Provider、Base URL、模型名和 API Key 匹配。
- Ollama 连接失败：确认 Ollama 已启动，且模型名称存在。
- Safe Mode：说明当前数据环境存在风险，先去 Settings 的恢复控制台导出备份并修复。
- Chat 无响应或一直思考：先查看 Settings 诊断，再检查模型 Provider 测试结果。
- Builder Review 后模型没有变化：先确认 Proposal 是否仍在 Review Center 待处理；Builder 默认不会绕过确认直接写入。

## License

MIT License
