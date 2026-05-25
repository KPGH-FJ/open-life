# OpenLife Post-Beta 完整发展计划

Date: 2026-05-10

Status: active

> 本文档是当前阶段的最高优先级行动指南。P0-P12 全部 vNext 原语已完成代码实现，`make ci` 全绿 799 测试。当前进入 Post-Beta 架构稳固阶段。

## Codex-Level Upgrade Entry

2026-05-15 新增 Codex / Claude Code 级别 Agent Runtime 升级准备文档。后续涉及 Agent Runtime、ToolRuntime、Replay、Proposal、MCP、AgentSpec、PromptStack、Memory Evolution 的开发任务，必须优先对齐以下文档：

1. [`openlife_codex_level_upgrade_plan.md`](openlife_codex_level_upgrade_plan.md): 总体升级目标、硬约束、P0/P1 阻断项和批次策略。
2. [`openlife_codex_level_acceptance_matrix.md`](openlife_codex_level_acceptance_matrix.md): 行为验收矩阵，`make ci` 之外的发布门槛。
3. [`openlife_codex_level_task_breakdown.md`](openlife_codex_level_task_breakdown.md): 可直接分配给 Agent 的批次任务、失败测试要求和审查清单。
4. [`openlife_codex_level_phase2_execution_facade_prep.md`](openlife_codex_level_phase2_execution_facade_prep.md): Phase 2 执行路径收敛准备文档，定义 Tauri-side ExecutionFacade 的首批开发边界、非目标、测试和 Agent 指令。
5. [`openlife_codex_level_execution_facade_coverage_audit.md`](openlife_codex_level_execution_facade_coverage_audit.md): ExecutionFacade coverage audit / migration boundary，记录 Chat / StreamChat、Direct Tool Execution、Scheduled-specific facade wrapper、Replay-specific facade wrapper、Plan-specific facade wrapper 已完成迁移，Builder / Calibration / Skill runtime 暂不迁移，并给出下一批最小候选。
6. [`openlife_prompt_stack_coverage_audit.md`](openlife_prompt_stack_coverage_audit.md): PromptStack 全路径审计矩阵，区分 PromptStack-governed、intentionally legacy/ad hoc、not applicable，并记录 trace / privacy 事实。

这些文档不替代 vNext 原语文档，而是在 Post-Beta 阶段把原语升级为“可信、可审计、可恢复、真实执行”的顶级 Agent 产品标准。

2026-05-25 当前阶段为 **PromptStack Full-Path Audit / Safety Net**：Chat / StreamChat 已完成 Tauri ExecutionFacade 收敛并通过 `AgentRuntime::execute_task_with_spec` / PromptStack；Direct Tool Execution、Replay、Plan Execution 是无模型 PromptStack 的专用 facade/wrapper 或 action-only 路径；Scheduled execution 已迁移为 Scheduled-specific wrapper 并通过 PromptStack；Proactive suggestions 保持 suggestion-only，不创建 AgentRun / PromptStack trace。Skill runtime 仍不进入 ExecutionFacade：真实路径是 SkillRegistry legacy skill prompt 构造 → 必需 stored `AgentSpec` → `AgentRuntime::execute_task_with_spec` / PromptStack / context governance → `InferenceScheduler::generate_governed` → skill JSON envelope 解析和 ProposalStore 写入。Builder / Calibration Proposal-first 安全网继续有效，但 Builder prompt、Calibration prompt、Skill-specific facade 边界、Proactive suggestion trace、Chat proposal extraction 和 web summarization helper 仍未 PromptStack 完成，详见 `openlife_prompt_stack_coverage_audit.md`。

---

## 零、当前状态基线

### P0-P12 实现清单

| 原语 | 状态 | 代码证据 |
|------|------|----------|
| AgentRun 追踪 | ✅ P0 | `agent/store.rs` (834行), Tauri commands |
| AgentRunEvent (41种) | ✅ P0/P1 | `agent/event_store.rs` (804行), `agent/types/mod.rs` (2183行) |
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
| CI 状态 | ✅ 全绿: 140+ Rust 测试 (core + tauri) |
| 前端测试 | ✅ 214 passed |
| 前端构建 | ✅ 生产构建 3.87s, 57 chunks |
| Rust clippy | ✅ 零警告 |
| 已知技术债标记 | 0 (agent模块内零TODO/FIXME/HACK) |
| lib.rs 大小 | 3198 行 (需瘦身) |
| ChatPage.tsx 大小 | 1681 行 (暂不重构) |

### 已知限制

| 限制 | 影响 | 优先级 |
|------|------|--------|
| lib.rs 执行路径未收敛 | 多条入口链 (send_message/send_message_with_agent_loop/start_stream_message等5+条) | P0 |
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
- [ ] 同步 `current_agent_runtime_audit.md`: 反映 P0-P12 实现后的新事实
- [ ] 同步 `README.md`: 更新阶段标记和文档引用

### 1.3 工程清理

- [ ] 删除前端冗余文件 (`DashboardPage.tsx.bak`)
- [ ] 验证 `make clean-tract` 跨环境有效性
- [ ] CI 全绿重新验证

### Phase 1 门控

- [ ] RC 报告为 `go`
- [ ] 文档与代码事实一致
- [ ] `make ci` 通过

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
| **Fallback 可追踪** | fallback 到 legacy 直接生成时创建 AgentRunEvent |
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
- [x] Plan-specific facade/wrapper 正式迁移：`execute_agent_plan` / `retry_agent_plan` 调用 `run_tauri_plan_execution`；command 层不再直接构造 plan step `ActionExecutor`；不使用 Chat facade；保留 confirmation/review/deviation/retry/status/trace 语义；Builder / Calibration / Skill runtime 仍未迁移
- [x] Scheduled/Proactive 事件创建验证
- [x] Builder/Calibration 事件创建验证：`builder_create_proposals` 与 Calibration Proposal-first 路径写 metadata-only `proposal.created` events；payload 不包含 raw prompt、`before` / `after`、完整 LifeModel；source audit 证明无 Chat facade / fallback；ProposalStatus-gated apply 已有测试保护；`apply_calibration` 缺省 mode 和前端正式按钮默认创建 proposals，legacy `direct` 默认关闭并由 `system.allow_legacy_calibration_direct_apply` 门控，普通 UI 不暴露
- [x] Skill runtime 迁移前审计与安全网：确认 `run_skill` 仍是模型生成 + skill envelope/proposal 执行的混合路径，不是 Chat facade；补 missing AgentSpec fail-closed、AgentSpec restricted toolset、PromptStack/model failure observability、payload 脱敏、success response shape、no Chat fallback / no wrapper masquerade source audit 测试；Skill runtime 仍未迁移

### 2.3 PromptStack 全路径审计

- [x] 列出所有 prompt 组装点 (Chat / StreamChat / Scheduled / PlanMode / Plan execution / Replay / Skill / Builder / Calibration / Proactive / Direct Tool，以及 Chat proposal extraction、web summarization、LayeredReasoner strategy prompt、legacy scheduler generation)
- [x] 逐一核对是否通过 PromptStack、intentionally legacy/ad hoc、或 not applicable
- [x] Scheduled-specific facade wrapper 路径测试锁定：`AgentSpec`、`PromptBlockRegistry`、`NetworkPolicy`、`ExecutionSandbox`、restricted toolset 均由 facade assembly helpers 提供，`scheduler_runner.rs` 不再裸调 `AgentLoop::run`
- [x] Skill runtime 路径测试锁定：`AgentRuntime::execute_task_with_spec` 负责 PromptStack 组装；未知 PromptBlock fail closed 并持久化 failed Skill `AgentRun`，事件 payload 仅记录错误摘要和 AgentSpec/PromptBlock metadata，不写 raw prompt 或 raw LifeModel
- [x] 新增 `plans/openlife_prompt_stack_coverage_audit.md` 覆盖矩阵：列出 entrypoint、prompt source、PromptStack-governed、AgentSpec source、event trace emitted、privacy behavior、remaining risk、next required migration
- [x] 补 source audit：Chat / StreamChat / Scheduled 锁定 PromptStack registry；Skill / Builder / Calibration 明确 intentionally legacy / not complete；Replay / Plan execution / Direct Tool / Proactive suggestion-only 不伪造 PromptStack trace
- [x] 补行为测试：StreamChat unknown PromptBlock fail closed；既有 Chat / Scheduled / Skill unknown PromptBlock 测试继续覆盖；payload contract 继续锁定 no raw prompt / no raw LifeModel
- [ ] 接入尚未通过 PromptStack 的路径
- [ ] PromptBlock 版本记录到 AgentRunEvent
- [ ] 将 Builder prompt、Calibration prompt（若继续为模型生成）、Skill-specific prompt、Chat proposal extraction、web summarization helper 迁入专用 PromptStack 边界

### Phase 2 门控

- [ ] lib.rs 降至 ~2000 行
- [ ] 所有会创建 AgentRun 的模型执行路径产生可追踪 AgentRunEvent；Proactive suggestion-only / Direct Tool / Replay / Plan action-only 不伪造 PromptStack trace
- [ ] PromptStack 覆盖率目标仅适用于模型 prompt entrypoint；legacy/ad hoc exceptions 必须在矩阵中显式列出
- [ ] 无新增编译警告
- [ ] `make ci` 通过

---

## 三、Phase 3: LifeModel Evolution 管线闭环 (2-4周, 可与 Phase 2 并行)

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
