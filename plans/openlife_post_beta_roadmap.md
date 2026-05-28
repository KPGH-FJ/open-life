# OpenLife Post-Beta 完整发展计划

Date: 2026-05-10 (updated: LifeModel-HS spec prep, 2026-05-28)

Status: active / LifeModel-HS planning gate

> 本文档是 Post-Beta / Codex-level 阶段索引。P0-P12 全部 vNext 原语已完成代码实现，Codex-level 稳定化边界已经收口；当前已经进入 LifeModel-HS 设计与 spec 准备门控。LifeModel-HS 开发必须先对齐 ADR 0013 与 MVP task specs，不允许绕过治理边界直接做 LifeModel 深度开发或旧式 evolution 扩张。

## Codex-Level Upgrade Entry

2026-05-15 新增 Codex / Claude Code 级别 Agent Runtime 升级准备文档。后续涉及 Agent Runtime、ToolRuntime、Replay、Proposal、MCP、AgentSpec、PromptStack、Memory Evolution 的开发任务，必须优先对齐以下文档：

1. [`openlife_codex_level_upgrade_plan.md`](openlife_codex_level_upgrade_plan.md): 总体升级目标、硬约束、P0/P1 阻断项和批次策略。
2. [`openlife_codex_level_acceptance_matrix.md`](openlife_codex_level_acceptance_matrix.md): 行为验收矩阵，`make ci` 之外的发布门槛。
3. [`openlife_codex_level_task_breakdown.md`](openlife_codex_level_task_breakdown.md): 可直接分配给 Agent 的批次任务、失败测试要求和审查清单。
4. [`openlife_codex_level_phase2_execution_facade_prep.md`](openlife_codex_level_phase2_execution_facade_prep.md): Phase 2 执行路径收敛准备文档，定义 Tauri-side ExecutionFacade 的首批开发边界、非目标、测试和 Agent 指令。
5. [`openlife_codex_level_execution_facade_coverage_audit.md`](openlife_codex_level_execution_facade_coverage_audit.md): ExecutionFacade coverage audit / migration boundary，记录 Chat / StreamChat、Direct Tool Execution、Scheduled-specific facade wrapper、Replay-specific facade wrapper、Plan-specific facade wrapper 已完成迁移，Builder / Calibration / Skill runtime 暂不迁移，并给出下一批最小候选。
6. [`openlife_prompt_stack_coverage_audit.md`](openlife_prompt_stack_coverage_audit.md): PromptStack 全路径审计矩阵，区分 PromptStack-governed、legacy compatibility、not applicable，并记录 trace / privacy 事实。
7. [`openlife_codex_level_closeout_acceptance_report.md`](openlife_codex_level_closeout_acceptance_report.md): Codex-level Final Closeout 验收报告，作为进入 LifeModel Evolution / Evidence / Proposal / Editor / Review 阶段前的事实源和门控记录。

这些文档不替代 vNext 原语文档，而是在 Post-Beta 阶段把原语升级为“可信、可审计、可恢复、真实执行”的顶级 Agent 产品标准。

2026-05-26 当前阶段为 **Runtime fallback boundary 最终收口**：Chat / StreamChat 已完成 Tauri ExecutionFacade 收敛并通过 `AgentRuntime::execute_task_with_spec` / PromptStack；Direct Tool Execution、Replay、Plan Execution 是无模型 PromptStack 的专用 facade/wrapper 或 action-only 路径；Scheduled execution 已迁移为 Scheduled-specific wrapper 并通过 PromptStack；Proactive suggestions 保持 suggestion-only，不创建 AgentRun / PromptStack trace。Skill runtime 仍不进入 Chat ExecutionFacade：真实路径是 SkillManifest 派生 Skill-specific PromptBlocks → 必需 stored `AgentSpec` → 追加 Skill PromptBlock IDs 的有效 AgentSpec → `AgentRuntime::execute_task_with_spec` / PromptStack / context governance → `InferenceScheduler::generate_governed` → skill JSON envelope 解析和 ProposalStore 写入。Chat proposal extraction 已从 ad hoc prompt 迁入 Proposal-specific PromptStack helper，web content summarization helper 已从 raw summarizer prompt 迁入 Web Summarization PromptStack helper。LayeredReasoner meaning / strategy / generation / safety internal prompts 已迁入 LayeredReasoner-specific PromptBlocks，并作为 metadata-only internal strategy boundary 写入 `ReasoningTrace.prompt_block_traces`；该层不伪造 `prompt_stack.assembled` AgentRunEvent。`InferenceScheduler::generate` / `generate_stream` 和 `llm::build_system_prompt` 现在明确为 legacy compatibility boundary；正式 AgentRuntime / ExecutionFacade governed path 及其 runtime fallback 不再调用 legacy scheduler generation。Chat / StreamChat runtime fallback 保留为 governed legacy compatibility retry：只处理 Runtime/model failure，Governance failure fail-closed；fallback.started / fallback.completed / fallback.failed 统一使用 metadata-only payload builder，保留 `agent_spec_id`、`privacy_policy`、`generation_path`、PromptStack source 和 sanitized error summary，不写 raw prompt、raw user、raw LifeModel、raw memory 或完整模型输出。Builder 模型辅助提取已收口为 Builder-specific PromptBlocks + `generate_raw_governed(..., LocalOnly)`，不走 legacy raw/scheduler generation，且 Builder 仍保持 Proposal-first；Calibration 判定为 deterministic / proposal-only / UI metadata，暂不迁入 PromptStack，direct apply 仅保留显式 legacy compatibility gate。

2026-05-26 Codex-level Final Closeout：Runtime fallback boundary 之后不再继续扩展底座。本轮新增 `openlife_codex_level_closeout_acceptance_report.md`，把 P0-P12、ExecutionFacade、PromptStack、AgentRunEvent/Audit、Proposal-first、runtime fallback、Builder/Calibration 边界和测试门控统一封口。若 `make ci` 继续全绿，下一阶段应进入 LifeModel Evolution / Evidence / Proposal / Editor / Review 的准入后开发，而不是重复做 AgentRuntime 基座扩张。

## LifeModel-HS Entry

2026-05-28 LifeModel-HS 设计门控已完成第一轮沉淀。后续 LifeModel 相关开发必须优先阅读：

1. [`lifemodel_hs_architecture_plan.md`](lifemodel_hs_architecture_plan.md): LifeModel-HS / Personal Heuristic System 总体架构设计。
2. [`adr/0013-lifemodel-hs-source-of-truth-governance.md`](adr/0013-lifemodel-hs-source-of-truth-governance.md): Source of truth、治理、自动更新、retention、删除、Policy/Heuristic 边界、MVP 范围的 accepted ADR。
3. [`lifemodel_hs_mvp_task_specs.md`](lifemodel_hs_mvp_task_specs.md): 可交给 Agent 执行的 LifeModel-HS MVP coding specs。

LifeModel-HS MVP 的默认路线是 additive：先建立 EvidenceStore、HeuristicStore、Policy/Heuristic boundary、ContextSelector/HeuristicSelector、deterministic RegressionSuite、negative evidence loop 和 YAML compatibility materialized view guardrails。当前 YAML LifeModel 仍是兼容视图，不在 MVP 中一次性切换为完整 HS canonical source。

---

## 零、当前状态基线

### P0-P12 实现清单

| 原语 | 状态 | 代码证据 |
|------|------|----------|
| AgentRun 追踪 | ✅ P0 | `agent/store.rs` (834行), Tauri commands |
| AgentRunEvent (45种，含 Runtime fallback metadata events) | ✅ P0/P1 | `agent/event_store.rs`, `agent/types/mod.rs` |
| AgentLoop ReAct 循环 | ✅ P1 | `agent/agent_loop.rs` (3056行) |
| PromptStack (10 Block) | ✅ P4/P6 | `agent/prompt_stack.rs` (922行) |
| ToolRuntime + ActionExecutor | ✅ P3 | `agent/action_executor/` (6文件, ~3500行) |
| ExecutionSandbox | ✅ P9 | `agent/execution_sandbox.rs` (1094行) |
| ShellExecutor | ✅ P9 | `agent/shell_executor.rs` (1651行, 默认关闭) |
| PlanMode + PlanExecutor | ✅ P4/P5 | `agent/plan_mode.rs` (879行), `agent/plan_executor.rs` (1726行) |
| AgentSpec + AgentSpecStore | ✅ P6/P7 | `agent/agent_spec_store.rs` (818行) |
| SubAgentRuntime | ✅ P7 | `agent/sub_agent.rs` (873行) |
| Compaction | ✅ P8 | `agent/compaction.rs` (1379行) |
| MemoryEvidence | ✅ P5 | `agent/memory_evidence.rs` (421行) |
| Proposal 统一确认层 | ✅ P3 | 7种类型, `agent/proposal_engine.rs` (838行) |
| ModelRouter (已毕业) | ✅ | `agent/model_router.rs` (736行) |
| ContextAssembler | ✅ | `agent/context_assembler.rs` (603行) |
| Proactive Engine | ✅ P6 | scheduler_runner + ProactiveEngine |
| 前端 Agent Workspace | ✅ P10 | Workspace/Review/Runs/Proposal 面板 |
| Beta 试用路径矩阵 | ✅ P11 | 8条烟囱路径, 诊断/SafeMode/隐私脱敏 |
| 用户试用指南 | ✅ P12 | `BETA_TRIAL_GUIDE.md` |
| 发布构建 | ✅ P12 | `OpenLife_0.1.0_aarch64.dmg` (25MB) |

### 核心指标

| 指标 | 数值 |
|------|------|
| CI 状态 | ✅ 全绿: `make ci` 覆盖 frontend / core / tauri / a2a / build |
| Rust 测试 | ✅ core 960 passed, 1 ignored；tauri 207 passed；a2a 5 passed |
| 前端测试 | ✅ 431 passed |
| 前端构建 | ✅ 生产构建通过 |
| Rust clippy | ✅ 零警告 |
| 已知技术债标记 | 0 (agent模块内零TODO/FIXME/HACK) |
| lib.rs 大小 | 大文件仍需瘦身，但不是 LifeModel 阶段入口阻断项 |
| ChatPage.tsx 大小 | 1681 行 (暂不重构) |

### 已知限制

| 限制 | 影响 | 优先级 |
|------|------|--------|
| lib.rs 仍偏大 | 执行入口已通过 facade/wrapper 收敛，剩余是文件规模与可维护性问题 | P2 |
| Universal binary 未打通 | 当前仅 aarch64, 缺 x86_64 | P1 |
| 代码未签名公证 | macOS 需手动允许运行 | P2 |
| Windows/Linux 未测试 | 仅 macOS 平台验证 | P2 |
| ChatPage 未重构 | 1681行, ADR 0010 已accepted但受P12约束 | P3 |
| 编译缓存偶发错误 | `make clean-tract` 可修复 (tract-onnx rlib format) | P3 |

---

## 一、Phase 1: P12 交付收尾 + 文档同步 (当前, 1-2周)

### 1.1 RC 报告最终化

- [ ] 填写 P11 烟囱测试人工结果 (S1-S8)
- [ ] RC 报告从 `conditional-go` 升级为 `go`
- [ ] 记录 4 个已知 P3 项的处置决定

### 1.2 文档同步

- [x] AGENTS.md: 标记 P12 已验收, 指向本计划
- [x] 新增 `plans/openlife_post_beta_roadmap.md` (本文档)
- [ ] 同步 `migration_plan.md`: Phase 0-9 与 P0-P12 实现结果对齐
- [x] 同步 `current_agent_runtime_audit.md`: 反映 P0-P12 与 Codex-level Final Closeout 后的新事实
- [x] 同步 `README.md`: 更新阶段标记和文档引用
- [x] 新增 `plans/openlife_codex_level_closeout_acceptance_report.md`: Codex-level 验收报告与 LifeModel 入口门控

### 1.3 工程清理

- [ ] 删除前端冗余文件 (`DashboardPage.tsx.bak`)
- [ ] 验证 `make clean-tract` 跨环境有效性
- [ ] CI 全绿重新验证

### Phase 1 门控

- [ ] RC 报告为 `go`
- [x] 文档与代码事实一致
- [x] `make ci` 通过

---

## 二、Phase 2: 执行路径收敛 (2-4周)

### 2.1 Tauri 侧 ExecutionFacade 提取

**问题**: `src-tauri/src/lib.rs` (3198行) 有 5+ 条执行入口:
- `send_message` / `send_message_with_agent_loop`
- `start_stream_message` / `start_stream_message_with_agent_loop`
- L1 reflex 路径
- AgentLoop fallback 路径
- Scheduled proactive 路径

**方案**: 参考 `agent/execution_facade.rs` (364行), 在 `src-tauri/` 侧建立 facade,

统一入口: `run_agent_task(task, mode, stream_adapter)`

| 任务 | 详情 |
|------|------|
| **提取 facade 模块** | `src-tauri/src/execution_facade.rs` 统一 dispatch |
| **mode 枚举** | `chat / stream_chat / scheduled / proactive / calibration / builder / replay` |
| **Fallback 可追踪** | Runtime/model failure fallback 保留为 governed legacy compatibility retry，并记录 metadata-only fallback AgentRunEvent；Governance failure fail-closed |
| **L1 reflex 归类** | 标记为非 AgentLoop 或转为轻量 AgentRun mode |
| **lib.rs 瘦身** | 目标: 3198 → ~2000 行 |

### 2.2 事件追踪全覆盖

- [ ] Chat 双路径 (send/stream) 事件完整性测试
- [ ] Fallback 事件保留测试
- [x] Scheduled/Proactive 迁移前安全网：lease 短锁、stale running recovery、missing AgentSpec fail-closed、无 Chat fallback、失败写回 task error 字段
- [x] Scheduled/Proactive facade wrapper 验证：Scheduled-specific wrapper 已迁移；failed-run observability 已修复；保持 scheduler task failure 语义，不继承 Chat fallback
- [x] Scheduled/Proactive 事件创建验证：成功 / runtime failure 有 run_id 和 AgentRunEvent 追踪；missing AgentSpec 只记录 scheduler task failure/status；NetworkPolicy hard deny/ask 与 Sandbox deny 写 typed `tool.call_blocked`；scheduler failure 不标 completed；late completion 不覆盖并发 terminal state；Proactive suggestions 保持 suggestion-only 且无 Chat fallback
- [x] Proposal Replay / Replay hardening preparation：审计 `accept_proposal_with_state`、`replay_agent_action` / `replay_action_internal`、Tauri ExecutionFacade assembly、ActionExecutor、Replay typed events 和 ProposalStore；补 missing AgentSpec、original AgentSpec、no tool escalation、ToolPermission deny、NetworkPolicy deny、ExecutionSandbox deny、Proposal status source of truth、typed payload contract、no Chat fallback 测试
- [x] Replay-specific facade/wrapper 正式迁移：`replay_action_internal` 调用 `run_tauri_replay_execution`；不直接构造 `ActionExecutor`；不使用 Chat facade；不写 Proposal status
- [x] Plan-specific facade/wrapper 正式迁移：`execute_agent_plan` / `retry_agent_plan` 调用 `run_tauri_plan_execution`；command 层不再直接构造 plan step `ActionExecutor`；不使用 Chat facade；保留 confirmation/review/deviation/retry/status/trace 语义；Builder / Calibration 不属于 Plan wrapper 迁移对象，分别保持 Builder-specific PromptStack / deterministic proposal-only 边界；Skill runtime 保持 Skill-specific PromptStack 边界，不迁入 Chat ExecutionFacade
- [x] Scheduled/Proactive 事件创建验证
- [x] Builder/Calibration 事件创建验证：`builder_create_proposals` 与 Calibration Proposal-first 路径写 metadata-only `proposal.created` events；payload 不包含 raw prompt、`before` / `after`、完整 LifeModel；source audit 证明无 Chat facade / fallback；ProposalStatus-gated apply 已有测试保护；`apply_calibration` 缺省 mode 和前端正式按钮默认创建 proposals，legacy `direct` 默认关闭并由 `system.allow_legacy_calibration_direct_apply` 门控，普通 UI 不暴露
- [x] Skill runtime 迁移前审计与安全网：确认 `run_skill` 仍是模型生成 + skill envelope/proposal 执行的混合路径，不是 Chat facade；补 missing AgentSpec fail-closed、AgentSpec restricted toolset、PromptStack/model failure observability、payload 脱敏、success response shape、no Chat fallback / no wrapper masquerade source audit 测试；该项为 Skill-specific PromptStack 迁移前安全网，ExecutionFacade 迁移仍未进行
- [x] Skill-specific PromptStack boundary migration：`run_skill_with_state` 不再把 `SkillRegistry::build_system_prompt` / `build_skill_prompt` 作为正式 prompt path；SkillManifest / skill_id 生成 Skill contract PromptBlocks，并追加到有效 AgentSpec 后继续通过 `AgentRuntime::execute_task_with_spec`；raw user input 改由 user message 承载，不参与 Skill system block 边界；SummaryOnly 云端路径保留非敏感 Skill contract / JSON envelope / proposal policy，同时过滤 raw user input、raw LifeModel、raw memory、recent runs、chat history，并有 marker injection 测试保护；保持 missing AgentSpec fail-closed、restricted toolset、failed Skill AgentRun、metadata-only events 和前端 response shape

### 2.3 PromptStack 全路径审计

- [x] 列出所有 prompt 组装点 (Chat / StreamChat / Scheduled / PlanMode / Plan execution / Replay / Skill / Builder / Calibration / Proactive / Direct Tool，以及 Chat proposal extraction、web summarization、LayeredReasoner strategy prompt、legacy scheduler generation)
- [x] 逐一核对是否通过 PromptStack、intentionally legacy/ad hoc、或 not applicable
- [x] Scheduled-specific facade wrapper 路径测试锁定：`AgentSpec`、`PromptBlockRegistry`、`NetworkPolicy`、`ExecutionSandbox`、restricted toolset 均由 facade assembly helpers 提供，`scheduler_runner.rs` 不再裸调 `AgentLoop::run`
- [x] Skill runtime 路径测试锁定：`AgentRuntime::execute_task_with_spec` 负责 PromptStack 组装；未知 PromptBlock fail closed 并持久化 failed Skill `AgentRun`，事件 payload 仅记录错误摘要和 AgentSpec/PromptBlock metadata，不写 raw prompt 或 raw LifeModel
- [x] 新增 `plans/openlife_prompt_stack_coverage_audit.md` 覆盖矩阵：列出 entrypoint、prompt source、PromptStack-governed、AgentSpec source、event trace emitted、privacy behavior、remaining risk、next required migration
- [x] 补 source audit：Chat / StreamChat / Scheduled 锁定 PromptStack registry；Skill / Builder / Calibration 明确 intentionally legacy / not complete；Replay / Plan execution / Direct Tool / Proactive suggestion-only 不伪造 PromptStack trace
- [x] 补行为测试：StreamChat unknown PromptBlock fail closed；既有 Chat / Scheduled / Skill unknown PromptBlock 测试继续覆盖；payload contract 继续锁定 no raw prompt / no raw LifeModel
- [x] Chat proposal extraction PromptStack boundary migration：`try_llm_extract` 不再拼 ad hoc extraction prompt，也不再通过 `chat_with_ollama` 注入完整 LifeModel；改为 Proposal-specific PromptBlocks + privacy-scoped task block + local `chat_with_ollama_raw`；unknown PromptBlock / PromptStack validation / local model unavailable / model JSON parse failure 均返回结构化 failure reason 并进入显式 heuristic fallback；audit metadata 只记录 prompt block trace、privacy、route、failure/fallback reason，不写 raw prompt、raw user message、raw LifeModel 或完整模型输出；SummaryOnly 只包含摘要 contract，LocalOnly 不偷偷走云端；该层无 AgentRunEvent store，事件接入待后续阶段
- [x] Web content summarization helper PromptStack boundary migration：`summarize_content_blocking` 不再拼 raw summarizer system prompt；改为 Web Summarization PromptBlocks (`web_summarization.role` / `output_contract` / `privacy_rules` / `task_input`) + local `chat_with_ollama_raw`；unknown PromptBlock / PromptStack validation / local model unavailable / local timeout / invalid model output 均返回结构化 failure reason 和非原文 fallback；audit metadata 只记录 prompt block trace、privacy、route/provider、failure/fallback reason、source type、content length，不写 raw prompt、raw web content、raw URL query、raw LifeModel 或完整模型输出；成功输出也使用 sanitized source display，仅保留 scheme/host/path 并移除 query/fragment/userinfo；SummaryOnly 只保留长度/来源类别/摘要 contract/非敏感统计，LocalOnly 不偷偷走云端，CloudAllowed 目前不启用云端 route；该层无 AgentRunEvent store，事件接入待后续阶段
- [x] LayeredReasoner internal prompts PromptStack boundary migration：meaning / strategy / generation / safety 的 role / output contract / privacy rules 已成为稳定 PromptBlocks；strategy 和 generation 不再在 `layered.rs` 拼裸 system prompt；unknown internal PromptBlock fail closed，PromptStack validation failure 返回稳定 reason；SummaryOnly internal prompts 只保留计数/非识别信号，LocalOnly 继续由 `generate_raw_governed` 阻止云端 fallback，CloudAllowed 的 trace 仍只记录 metadata-only block trace；该层没有单独 AgentRunEvent store，不伪造 `prompt_stack.assembled`
- [x] Legacy scheduler generation boundary 收口：`InferenceScheduler::generate` / `generate_stream` 标记为 legacy compatibility；`generate_governed` / `generate_stream_governed` 不再委托 legacy scheduler generation；Chat / StreamChat runtime fallback 先组装 stored AgentSpec PromptStack，再调用 governed fallback；SummaryOnly compatibility payload 和 LocalOnly cloud-blocking 测试锁定隐私边界
- [x] Builder / Calibration prompt boundary 决策与收口：Builder 的模型辅助 signal extraction / draft-to-LifeModel extraction 迁入 Builder-specific PromptBlocks，并强制通过 `generate_raw_governed(..., LocalOnly)`，不走 legacy raw/scheduler generation；Builder direct apply 默认关闭，正式路径仍是 Review Center Proposal-first；Calibration 判定为 deterministic / proposal-only / UI metadata，source audit 锁定无模型生成、无 Chat fallback、无 legacy scheduler generation，direct apply 默认关闭并保留为显式 legacy compatibility gate
- [x] Runtime fallback boundary 最终收口：保留 Chat / StreamChat governed legacy compatibility retry，不升级为 first-class fallback mode；Runtime/model failure 可 fallback，Governance failure fail-closed；fallback payload builder 覆盖 started/completed/failed，metadata-only 且不包含 raw prompt / raw user / raw LifeModel / raw memory / full model output；source audit 锁定不调用 legacy `generate` / `generate_stream`
- [ ] 后续新增的模型 prompt entrypoint 必须先归类为 governed / legacy compatibility / not applicable；当前 Calibration 保持 not applicable，除非未来变成模型生成路径。若本轮 `make ci` 通过，下一步应进入 Codex-level 总体验收报告 / LifeModel 阶段准入检查，而不是继续扩展底座。
- [x] PromptBlock version -> AgentRunEvent contract 收口：正式 Chat / StreamChat / Scheduled / Skill governed path 的 `prompt_stack.assembled` 统一通过 typed `build_prompt_stack_assembled_payload`，`prompt_blocks[]` 记录 `id`、`version`、`purpose`、`privacy_level`、`cloud_allowed`、`token_budget`、`applies_to`、`estimated_tokens`，不写 raw prompt / raw LifeModel / raw memory / raw user content；helper-only PromptStack 路径不伪造 AgentRunEvent
- [x] 将 Builder prompt、Calibration prompt（若继续为模型生成）迁入或明确保留为专用边界；legacy scheduler generation 已明确保留为 compatibility-only，不允许正式 governed path 调用

### Phase 2 门控

- [ ] lib.rs 降至 ~2000 行（非阻塞工程债）
- [x] 所有会创建 AgentRun 的正式模型执行路径产生可追踪 AgentRunEvent；Proactive suggestion-only / Direct Tool / Replay / Plan action-only 不伪造 PromptStack trace
- [x] PromptStack 覆盖率目标仅适用于模型 prompt entrypoint；legacy compatibility / not applicable exceptions 已在矩阵中显式列出
- [x] 无新增编译警告
- [x] `make ci` 通过

---

## 三、Phase 3: LifeModel-HS MVP / Evolution 管线闭环 (2-4周, 可与 Phase 2 并行)

> 本节保留历史 Evolution Pipeline 任务语义，但新的 LifeModel 开发入口已经升级为 LifeModel-HS。具体 coding 必须按 `lifemodel_hs_mvp_task_specs.md` 单任务推进，并遵守 ADR 0013。旧式 MemoryEvidence -> Evolution Proposal 管线只能作为 HS Evidence / Proposal / negative evidence 的兼容输入或迁移对象，不能绕过 EvidenceStore、Policy、Selector、Regression 和 Proposal-first 治理。

### 3.1 MemoryEvidence → Evolution Pipeline

**当前状态**: MemoryEvidence (`agent/memory_evidence.rs`) 已实现信号提取 (RepeatedPreference, RecurringGoal, CapabilitySignal, StateTrend, Contradiction, ValueSignal), 但 `LifeModelEvolutionEngine` 到生成 Evolution Proposal 的完整管线尚未端到端测试。

| 任务 | 详情 |
|------|------|
| **EvolutionEngine 补全** | 聚合 accepted memory → 检测 pattern/contradiction/trend → 生成 evidence-backed Proposal |
| **管线端到端测试** | repeated preference → proposal 生成 → 用户确认 → 应用 |
| **Contradiction 处理** | 矛盾时不生成 confident patch, 记录冲突供用户裁决 |
| **Rejected 反馈** | 被拒绝的 proposal 影响后续 evidence scoring |
| **高风险字段保护** | Identity/Values/Mission/Long-term goals 永不 auto-apply |

### 3.2 集成测试

- [ ] Memory → Evidence → Proposal 完整链路
- [ ] High-risk field review requirement
- [ ] Rejected proposal negative evidence
- [ ] No raw unaccepted transcript as evidence

### Phase 3 门控

- [ ] LifeModel Evolution 管线端到端可走通
- [ ] 集成测试通过
- [ ] 高风险字段保护不可绕过

---

## 四、Phase 4: 生产就绪 (6-8周)

### 4.1 发布工程

| 任务 | 详情 |
|------|------|
| Universal Binary | 安装 x86_64-apple-darwin target, 打通 aarch64+x86_64 |
| 代码签名 | macOS Developer ID 签名 |
| 公证 | Apple notarization |
| Windows 构建 | x86_64 Windows 构建 + 基础冒烟 |
| Linux 构建 | AppImage/deb + 基础冒烟 |

### 4.2 ChatPage 重构 (解锁 ADR 0010)

- 按组件边界拆分: ChatSurface / AgentStatusBar / RunTraceInline / ProposalBanner / MessageList
- 渐进迁移, 不一次性推倒重写

### 4.3 安全审计

- Safe Paths 默认值审查
- 云端数据足迹审核
- 诊断导出白名单逐字段审计
- 外部工具权限矩阵测试

### Phase 4 门控

- [ ] macOS universal binary + 签名 + 公证
- [ ] Windows/Linux 构建 + 基础冒烟
- [ ] ChatPage 重构完成
- [ ] 安全审计通过

---

## 五、Phase 5: v1.0 公开发布 (2026-08+)

- 官网 / 下载页 / 用户文档 / 开发者文档
- Plugin/Skill 外部注册
- 多语言支持
- 社区建设

---

## 六、执行优先级总结

```
Phase 1 (当前)      Phase 2+3 (并行)          Phase 4                  Phase 5
文档同步 + RC 闭环 → 执行路径收敛 + Evolution → 生产就绪 + 发布工程 → v1.0 公开
```

每个 Phase 以 `make ci` 为最低门控。

---

*本文档根据 2026-05-10 代码审计和 CI 结果编写。*
