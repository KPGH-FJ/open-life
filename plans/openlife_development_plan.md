# OpenLife Development Plan

> Version: 2026-06-04 W123 ReAct Beta Execution Hardening complete
> Current direction: W114-W123 execution hardening is complete; default Chat remains unchanged
> Architecture program baseline: [`openlife_lifemodel_governed_agent_runtime.md`](/Users/fujing/Desktop/偶来福/plans/openlife_lifemodel_governed_agent_runtime.md)
> Progress index: [`lifemodel_governed_runtime_progress.md`](/Users/fujing/Desktop/偶来福/plans/lifemodel_governed_runtime_progress.md)
> Architecture source of truth: [`openlife_agent_framework_architecture.md`](/Users/fujing/Desktop/偶来福/plans/openlife_agent_framework_architecture.md)
> Beta roadmap: [`openlife_react_beta_roadmap.md`](/Users/fujing/Desktop/偶来福/plans/openlife_react_beta_roadmap.md)

## 1. Current Strategic Reset

OpenLife is now defined as a **local-first, LifeModel-governed personal Agent framework**, not a conventional desktop app.

ReAct remains the current default Chat execution strategy and Beta execution
kernel, but the long-term architecture is not "ReAct only." W1-W60 have already
implemented the thin runtime contract, LifeModel governance foundations,
PlanExecute core/vertical slices, StrategySelector, MultiStrategy preview/audit,
the lightweight RuntimeStrategy adapter foundation, read-only migration gates,
visible Settings evidence surfaces, explicit controlled pilot/shadow/candidate
paths, metadata-safe review evidence, a disabled default Chat adapter guard
stack through typed callsite contracts, an authority roadmap sync so later
Agents no longer follow stale W22 route instructions, an ordinary-entry
preflight / side-effect lock before the legacy Chat entry, and a read-only
ordinary-entry preflight status surface for Settings review, a read-only
narrow implementation discussion gate over W48/W56 evidence, a read-only
narrow implementation plan draft over W57, metadata-safe human review evidence
over that W58 draft, and a read-only narrow implementation plan approval
readiness gate over the current draft/review digest. The W65 descriptor
slice adds only a pure backend mapper for a future controlled adapter candidate
contract; it stores input length/hash and route/executor metadata, keeps the
controlled executor disabled/unattached, writes nothing, runs nothing, and does
not route default Chat.

The important current boundary is that MultiStrategy Runtime is
descriptor/readiness/report/status-ready and preview/audit-ready, and ReAct
Beta execution has now been hardened across readiness/status, action parsing,
Tool Registry taxonomy, ActionExecutor manifest authority, trace envelopes,
permission/replay, proposal-first writes, and Runs trace visibility. W106-W113
status/readiness/maturity reports and W114-W123 ReAct Beta readiness/status
reports are not migration permission. This work does not productize ReAct as
the default Chat path and does not replace `send_message` or the existing Chat
main flow.

The immediate order is defined in
[`openlife_lifemodel_governed_agent_runtime.md`](/Users/fujing/Desktop/偶来福/plans/openlife_lifemodel_governed_agent_runtime.md):

```text
tool/proposal hygiene
-> thin runtime spine
-> ReAct convergence
-> maturation loop
-> governor
-> Plan-Execute
-> strategy abstraction
```

The realized W11-W60 sequence after W10 is:

```text
docs/status sync
-> non-default preview UI/debug entry
-> guarded Chat subpath migration
-> maturation loop V1
-> PlanExecute vertical slice
-> RuntimeStrategy trait
-> Runtime integration hardening / Chat migration gate
-> Runtime Migration Gate evidence surface
-> sustained gate evidence / pilot eligibility
-> very small controlled Chat migration pilot with fallback
-> reviewed pilot response promotion
-> post-promotion validation and source binding
-> promotion/readiness/review/shadow/cutover candidate evidence ladder
-> default Chat boundary/activation/disabled adapter planning ladder
-> dry-run/controlled preview/cutover plan review ladder
-> route guard/invocation guard/typed callsite guard stack
-> authority roadmap sync
-> ordinary-entry preflight / side-effect lock
-> ordinary-entry preflight status surface
-> narrow implementation discussion gate
-> narrow implementation plan draft
-> narrow implementation plan review evidence
-> narrow implementation plan approval readiness
```

W114-W123 completed this sequence:

```text
ReAct Beta readiness contract
-> AgentLoop action schema and parser hardening
-> Tool Registry Beta taxonomy/readiness
-> ActionExecutor manifest authority
-> AgentRun action/observation trace envelope
-> permission proposal and replay hardening
-> proposal-first write hardening
-> non-default ReAct Beta status harness
-> Runs/Trace UI hardening
-> docs/progress/verification sync
```

`calendar.propose_event` and `email.propose_draft` are now P1 proposal-only
governed executors: they create `ScheduledTask` / `DataExport` proposals and do
not perform real calendar writes, email sends, or `ExternalWriteAction`
fallback. Documentation entry points and tool taxonomy must be updated in the
same work package as any tool status change; stale status labels are treated as
blockers because they mislead future development Agents.

The existing project already contains many working or partially working modules:
LifeModel, Builder, Chat, Memory, MCP, A2A, Calibration, VersionControl,
Diagnostics, model scheduling, HS packet selection, evidence/governor
foundations, StrategySelector, PlanExecute core MVP, and MultiStrategy preview.
The next stage should not add more isolated features. It should continue
hardening the architectural spine:

```text
AgentTask -> ContextAssembler -> ModelRouter -> AgentLoop -> Tool/Skill Action -> Observation -> Proposal/Permission -> Apply/Replay -> AgentRun Trace
```

All future development should be judged by two questions:

```text
Does this make OpenLife more like a trustworthy ReAct personal Agent OS?
Does this make LifeModel-HS more like the shared personal protocol layer for all runtime strategies?
```

Before starting any implementation task, the Agent must check whether entry docs
and actual code status still agree. Stale runtime authority, tool taxonomy,
proposal semantics, metadata-safe audit, or AgentRun trace descriptions are
treated as blockers because they mislead later work.

Beta must not be declared because one blocker is fixed. W114-W123 makes ReAct
execution harder and more inspectable, but full Beta still requires the Beta
Gates in `openlife_react_beta_roadmap.md` to pass, especially Skill Runtime,
ModelRouter privacy, LifeModel/Memory governance across product paths, and a
product golden path.

Execution tools are part of the Beta definition. OpenLife must support OpenClaw-like execution seriousness through governed tools such as `mcp.call_tool`, `a2a.call_agent`, `file.read`, `file.write_proposal`, `web.search`, `web.fetch`, `calendar.read`, `calendar.propose_event`, `email.read`, `email.propose_draft`, and `task.create_proposal`. Connectors that cannot truly execute must be registered as disabled/declarative-only and may only create proposals.

## 2. What Is Already Implemented

| Area | Current status | Assessment |
|---|---|---|
| LifeModel | Four-dimensional model exists with Identity, Goals, Capabilities, State, preferences, relationships, snapshots | Strong base, but needs unified patch/proposal semantics |
| Builder | Quick, incremental, and Socratic construction flows exist | Valuable, but should become a LifeModel-building AgentTask |
| Chat | Streaming chat, sessions, persistence, diagnostics, model readiness UI exist | Main surface, but should become the first Agent execution interface |
| Model scheduling | Ollama + cloud provider routing and ModelRouter exist | Continue provider diagnostics and privacy-aware route trace hardening |
| Memory | SQLite messages, semantic vectors, memory search and recovery exist | Needs governance, source tracking, and AgentRun linkage |
| MCP/A2A | Tool and external Agent integrations exist with governed execution paths | Continue provider coverage and audit/replay hardening |
| Execution tools | Core OS, file/web/calendar/email/task/MCP/A2A taxonomy is synchronized; calendar/email write-like tools are proposal-only governed executors | Keep proposal-only semantics, disabled/declarative-only handling, and tests aligned |
| Calibration/Evolution | Feedback and model improvement suggestions exist and proposal-first paths are in place | Needs maturation loop V1 rather than scattered direct writes |
| LifeModel-HS foundations | RuntimeHSPacket, PolicyStore, EvidenceStore, HeuristicStore, RegressionSuite, Governor MVP exist | Still needs end-to-end LifeEvent/Signal/Evidence/Governor/Proposal loop |
| PlanExecute | Governed V1 runtime slice exists and can appear as a MultiStrategy preview payload/report | Not yet a productized weekly planning vertical slice |
| MultiStrategy preview | `run_multi_strategy_agent_preview` persists metadata-safe outer AgentRun audit and Runs/Trace can display it | Preview/beta only; not default Chat |
| Runtime Migration Gate evidence surface | Settings experimental panel can explicitly display `check_runtime_migration_gate` pass/block evidence and blocking reasons | Read-only diagnostic surface; not a Chat switch and not a preview runner |
| Pilot eligibility | `check_controlled_chat_pilot_eligibility` and Settings Pilot eligibility check recent preview gate evidence for sustained clean runs | Read-only qualification only; not a Chat switch, not a migration trigger, and creates no AgentRun/Proposal/Action/Observation |
| Controlled Chat Pilot / Promotion | Chat page exposes explicit `Run Controlled Pilot`; it checks eligibility first, blocks without preview when ineligible, runs one write-disabled preview when eligible, and renders “Pilot response” separately. Successful output with `userOutput` can be reviewed and explicitly promoted into one assistant chat message with `run_id` trace metadata when available | W22 promotion is user-confirmed and source-bound; source/target session mismatch blocks without writing, and default Chat is still not migrated |
| Migration evidence ladder | W23-W33 promotion evidence, readiness, reviewed migration plan, review decision evidence, implementation gate, shadow run/review, cutover readiness, cutover candidate/review, and candidate promotion readiness are implemented | All steps are explicit, metadata-safe, and non-default; readiness or approval only means implementation discussion evidence |
| Default Chat adapter guard ladder | W34-W60 default Chat boundary, activation planning/review/gate, disabled routing, contract harness, dry run/review, implementation readiness, controlled preview/review/readiness, cutover plan/review/readiness, route guard, invocation harness/plan/boundary, typed callsite contract, ordinary-entry preflight, ordinary-entry preflight status, narrow implementation discussion gate, narrow implementation plan draft, narrow implementation plan review evidence, and narrow implementation plan approval readiness are implemented | Ordinary `send_message` / `start_stream_message` still enter `legacy_stream`; controlled adapter executor remains disabled and unattached |
| Backend-only descriptor skeleton | W65 pure `default_chat_adapter.rs` descriptor/mapper describes a future controlled adapter candidate with callsite kind, contract shape, route metadata, input length/hash, disabled/unattached executor state, `allowWrites=false`, `maxToolCalls=0`, and zero side-effect budget | Not a command, not a Settings/Chat surface, not a preview/draft/readiness permission, and not default Chat migration |
| Authority roadmap sync | W54 syncs high-priority route documents with W1-W53 code status | Documentation governance step; prevents stale W22 instructions from steering future Agents |
| Ordinary-entry preflight | W55 adds a pure default Chat adapter preflight / side-effect lock before ordinary send/stream legacy entry | Requires typed contract readiness, legacy entry, controlled executor unattached, migration disabled, and zero pre-entry runtime/model/tool/write budget |
| Ordinary-entry preflight status | W56 adds a read-only status command and Settings surface over W55 send/stream preflight | Reports metadata-safe readiness/blockers only; no runtime/model/tool call, no business writes, no migration |
| Narrow implementation discussion gate | W57 adds a read-only gate over W48 cutover plan approval readiness and W56 ordinary-entry preflight status | Eligible only means a narrow adapter implementation slice may be discussed; no runtime/model/tool call, no records, no routing change, no migration |
| Narrow implementation plan draft | W58 adds a read-only plan draft over W57 discussion gate | Blocked gate returns no sections or digest; eligible gate returns metadata-safe human-review sections and stable digest only; no runtime/model/tool/preview call, no records, no routing change, no migration |
| Narrow implementation plan review evidence | W59 adds metadata-safe human review evidence over the W58 draft | Blocked draft approve writes no evidence; ready draft approve/reject/request_rework writes only metadata-safe Evidence; no raw notes/content/tool payload, no runtime/model/tool/preview call, no routing change, no migration |
| Narrow implementation plan approval readiness | W60 adds a read-only gate over current W58 draft and latest W59 review evidence | Requires latest approve, digest match, W57 eligible, default Chat unchanged, controlled adapter disabled, automatic migration disabled, and legacy send/stream paths; no records, runtime/model/tool/preview call, routing change, or migration |
| Diagnostics/Safe Mode | Recovery and readiness mechanisms exist | Good foundation for control plane |
| Frontend | Workspace/Chat/Review/Runs/Settings surfaces exist; Settings and Chat expose non-default governed preview/debug paths; Settings also exposes read-only gate evidence, pilot eligibility, W20 controlled pilot, W21 reviewed promotion, and W22 source-bound promotion validation | Further migration planning must remain reviewed and evidence-backed; default Chat stays unchanged |

## 3. Current Gaps

### 3.1 Architecture Gaps

- ~~No first-class `AgentTask`.~~ ✅ 已实现
- ~~No first-class `AgentRun`.~~ ✅ 已实现（Chat/Builder/Calibration 全链路追踪）
- ~~No central `AgentRuntime`.~~ ✅ 已实现（LayeredReasoner + DirectReasoner 策略注册）
- ~~No unified `AgentProposal`.~~ ✅ 已实现（ProposalEngine + ProposalStore）
- ~~No consistent representation of tool actions, model actions, memory writes, and LifeModel patches.~~ ✅ 已实现（统一 Proposal 结构）
- ~~Chat, Builder, Calibration, and Evolution still use separate pipelines.~~ ✅ 已完成（Builder/Calibration/Chat 已接入 Proposal 流；Chat Proposal 持久化和 AgentRun 关联已抽共享 helper）
- ~~No StrategySelector / MultiStrategy preview path.~~ ✅ 已完成（W7-W10：selector、orchestrator、preview command、metadata-safe AgentRun audit）
- ~~No formal `RuntimeStrategy` trait.~~ ✅ 已完成（W16：lightweight adapter/registry foundation for ReAct and PlanExecute）
- ~~No Runtime Migration Gate.~~ ✅ 已完成（W17：read-only gate for preview audit, fallback, metadata-safe trace, external-write, and proposal-first boundaries）
- **Default Chat is not migrated to MultiStrategy Runtime.** This is intentional; do not treat it as a gap to close in one direct replacement.
- **Current boundary: W65 backend-only descriptor skeleton after W64 authority compression validation.** Do not treat W19-W60 eligibility, preview, promotion, evidence, readiness, review, activation, dry-run, controlled preview, cutover plan, route guard, invocation guard, typed callsite, preflight, preflight status, narrow implementation discussion, narrow implementation draft, narrow implementation plan review approval, narrow implementation plan approval readiness, or W65 descriptor readiness as permission to replace default Chat directly.

### 3.2 Product Gaps

- MultiStrategy preview is still not the default Chat path; the migration gate evidence surface, pilot eligibility, controlled pilot/promotion, promotion evidence, migration review, shadow/cutover candidate, activation planning, dry-run/controlled preview, cutover plan approval, and default Chat adapter guard stack exist, but broader Chat migration still requires a separate reviewed implementation phase.
- Users cannot yet use a productized LifeModel-governed weekly planning flow.
- LifeModel maturation V1 exists as a service, but is not yet a visible end-to-end product loop.
- ~~LifeModel updates are not yet presented as one consistent reviewable proposal stream.~~ ✅ 已完成（Builder/Calibration/Chat 统一走 Proposal → Review Center）
- Dashboard is still closer to a summary page than an operating workspace.

### 3.3 Engineering Gaps

- Several large files are difficult to maintain, especially Builder and page components.
- Tauri command contracts are still manually maintained;新增 command 必须同步 Rust、TS wrapper、mock、页面调用和测试。
- ~~Provider configuration is better than before, but ModelRouter semantics are still incomplete.~~ ✅ 已补充（provider health 字段、云端 key/probe 检查、High/Critical privacy local-only）
- ~~ReAct execution and tool permissioning are not yet represented by one action model.~~ ✅ 已补充（错误处理增强 + 安全模式）

### 3.4 Recently Completed (2026-04-29)

- **Phase 1: 稳定性增强**
  - 生产代码 unwrap() 清理（0 个剩余）
  - hot_cache Mutex → RwLock 修复
  - AgentRuntime 4 个核心测试
- **Phase 2: Chat 提案流**
  - ProposalEngine 存根实现（含 ChatProposalGeneratorAdapter）
  - Chat 主流程接入 Proposal 生成
  - 错误处理增强（关键操作静默忽略修复）
- **Phase 3: 性能优化**
  - 硬编码值配置化（Ollama TTL、memory top_k）
  - LRU Embedding 缓存（1000 条，1h TTL）
  - 余弦相似度 4-wide SIMD 向量化
- **Phase 4: 集成测试**
  - Tauri 命令测试 +6（life_model 2 + chat 2 + state 2）
  - 前端类型同步（旧 trace 类型 → ReasoningTrace）
- **Phase 5: 稳定化与架构脊柱收敛**
  - Builder 正常用户路径改为 Proposal-Only，`builder_apply_signals` 仅作为 legacy/migration/debug 命令保留
  - Proposal 应用器覆盖 MemoryWrite、MemoryArchive、ToolPermission MVP，并对缺失 payload 明确失败
  - ModelRouter 不再把云端 provider 无条件标记可用，隐私约束先于云端 fallback
  - `make ci` 纳入 frontend production build/typecheck

## 4. Development Principles

### 4.1 Do Not Rewrite

The project should not be thrown away. Existing modules are useful. The correct move is to introduce the Agent Runtime as a new spine and gradually route existing features through it.

### 4.2 Do Not Add Isolated Features

New features should attach to one of these concepts:

- AgentTask
- AgentRun
- AgentAction
- AgentProposal
- LifeModel patch
- Memory governance
- ModelRouter
- Workspace

If a feature cannot attach to one of these, it should wait.

### 4.3 Keep User Control Central

The system may infer, suggest, summarize, and prepare actions. It should not silently rewrite high-impact parts of the user’s LifeModel.

### 4.4 Preserve Trial Stability

The current Settings -> Builder -> Chat -> Dashboard path must remain usable while the architecture migrates.

## 5. Phase Plan

## Phase 1: AgentRun Baseline ✅

Goal:

Create the minimum Agent Runtime spine without disrupting current Chat.

Deliverables:

- ✅ Add `agent` module in `openlife-core`.
- ✅ Define `AgentTask`, `AgentRun`, `AgentRunStatus`, `ModelRouteTrace`, `ContextSummary`, `AgentRunError`.
- ✅ Add `AgentRunStore` backed by SQLite.
- ✅ Add Tauri commands:
  - `create_agent_task`
  - `get_agent_run`
  - `list_agent_runs`
- ✅ Wrap `start_stream_message` so every normal chat creates an AgentRun.
- ✅ Record session id, user input, context summary, provider/model route, output status, and error state.

Acceptance criteria:

- ✅ Sending one chat message creates an AgentRun.
- ✅ The run can be queried after refresh.
- ✅ Runtime errors are attached to the run.
- ✅ Existing Chat history still works.

## Phase 2: Chat as Agent Surface ✅

Goal:

Turn Chat from a generic message page into the first Agent execution surface.

Deliverables:

- Add a lightweight Run Trace panel to Chat.
- Show:
  - model provider
  - model name
  - local/cloud decision
  - LifeModel context summary
  - memory hits summary
  - runtime error if any
- Keep streaming UI simple and stable.
- Ensure direct replies, errors, retries, and normal replies all go through one persistence path.

Acceptance criteria:

- User can answer “why did OpenLife respond this way?”
- User can answer “which model was used?”
- User can answer “what personal context was considered?”

## Phase 3: Unified Proposal Layer ✅

Goal:

Unify Builder, Calibration, Evolution, and proactive updates through one Proposal/Confirmation model.

Deliverables:

- Define `AgentProposal`.
- Define proposal types:
  - `life_model_patch`
  - `goal_update`
  - `state_update`
  - `preference_update`
  - `capability_update`
  - `memory_write`
  - `memory_archive`
  - `tool_permission`
- Add proposal store.
- Add commands:
  - `list_pending_proposals`
  - `confirm_agent_proposal`
  - `reject_agent_proposal`
  - `edit_agent_proposal`
- Route Builder Review through proposals.
- Route Calibration suggestions through proposals.

Acceptance criteria:

- Builder and Calibration no longer feel like separate confirmation systems.
- Every LifeModel-changing suggestion has source, reason, confidence, risk level, and decision state.
- High-risk fields default to explicit confirmation.

## Phase 4: ModelRouter Upgrade ✅

Goal:

Replace scattered provider assumptions with a provider-agnostic routing layer.

Deliverables:

- Introduce provider registry.
- Support provider classes:
  - Ollama
  - DeepSeek
  - OpenAI
  - OpenRouter
  - Custom OpenAI-compatible
- Define model roles:
  - chat
  - planner
  - tool_use
  - summarizer
  - extractor
  - embedding
- Record per-run route trace.
- Enforce privacy and cloud eligibility at routing time.
- Add provider health diagnostics.

Acceptance criteria:

- Provider support is not hardcoded into UI pages.
- Each AgentRun can explain its model route.
- Custom providers do not accidentally use the wrong environment key.

## Phase 5: Workspace Frontend Restructure ✅

Goal:

Make the frontend reflect the Agent framework.

Target navigation:

| Section | Purpose |
|---|---|
| Workspace | Today’s operating surface: readiness, active task, next action, pending proposals |
| Agent | Chat/task execution surface |
| LifeModel | Build, inspect, edit, and version the model |
| Memory | Search, manage, archive, restore |
| Runs | Agent execution history and traces |
| Settings | Providers, privacy, recovery, diagnostics |

Deliverables:

- Convert Dashboard into Workspace.
- Move Chat toward Agent execution.
- Add Runs history view.
- Add unified proposal review area.
- Reduce top-level page sprawl.

Acceptance criteria:

- A new user can understand the product without knowing the codebase.
- The UI communicates “personal Agent framework”, not “many unrelated tools”.

## Phase 6: Proactive Agent MVP

Goal:

Let OpenLife safely initiate useful check-ins without becoming intrusive.

Deliverables:

- Daily brief task.
- Weekly review task.
- Pending proposal reminder.
- Stale goal detection.
- State check-in card.
- All proactive outputs create AgentRuns and proposals, not silent mutations.

Acceptance criteria:

- OpenLife can proactively surface a useful next action.
- The user can inspect why the proactive suggestion appeared.
- The user can dismiss or accept it.

## Phase 7: Engineering Consolidation

Goal:

Make the codebase maintainable enough for continued Beta work.

Deliverables:

- Split oversized Builder internals into smaller modules.
- Split very large frontend pages where doing so reduces real complexity.
- Add Tauri command contract tests or generated wrappers. 当前手动 checklist 见 [`docs/tauri_command_contract_checklist.md`](../docs/tauri_command_contract_checklist.md)。
- Keep AGENTS.md, README.md, and this plan aligned.
- Establish a fixed smoke sequence.

Acceptance criteria:

- New development starts from the Agent Runtime concepts.
- Documentation and actual behavior do not contradict each other.
- Smoke testing catches broken main paths before manual trial.

## 6. Fixed Smoke Path

Every major development round should verify:

1. Start desktop app.
2. Open Settings and confirm provider diagnostics.
3. Build or restore a LifeModel Review.
4. Apply Review and confirm LifeModel changes.
5. Send a Chat message.
6. Inspect AgentRun trace once available.
7. Open Workspace/Dashboard and confirm next action.
8. Open Memory and VersionControl to confirm traceability and recovery.

## 7. Completed Work Summary (2026-05-05)

### Phase 1-6 全部完成

| Phase | 状态 | 关键交付 |
|-------|------|----------|
| Phase 1: AgentRun Baseline | ✅ | AgentTask/AgentRun/AgentRunStore/SQLite/Tauri 命令 |
| Phase 2: Chat as Agent Surface | ✅ | Run Trace 面板（provider/model/context/memory/error） |
| Phase 3: Unified Proposal Layer | ✅ | ProposalEngine/ProposalStore/全类型 Proposal/Builder+Calibration+Chat 接入 |
| Phase 4: ModelRouter Upgrade | ✅ | Provider registry/health diagnostics/privacy routing/route trace |
| Phase 5: Workspace Frontend Restructure | ✅ | Workspace/Agent/LifeModel/Memory/Runs/Settings 导航收敛 |
| Phase 6: Proactive Agent MVP | ✅ | ProactiveEngine, scheduler runner, Dashboard proactive card, State check-in |

### Sprint 10-12: Beta Hardening (2026-05-05)

| Sprint | 内容 | 状态 |
|--------|------|------|
| **10: CI修复 + 技术债务** | P0 clippy修复, AgentLoopContext重构, web.search加固+rate limit, AgentLoop参数配置化 (SystemConfig), lib.rs bootstrap提取 (3234→2821行) | ✅ |
| **11: 执行工具闭环** | a2a.call_agent 真实 A2A 执行器；calendar/email proposal 工具已校准为 P1 proposal-only governed executor；ChatProposalGenerator LLM升级 (Ollama信号提取) | ✅ |
| **12: Agent深度能力** | AgentRole (Generalist/Planner) + role_system_instruction, scheduler_runner (定时任务执行器), E2E integration tests | ✅ |

### Beta Execution Tools 落地（更新）

- **P1 真实可执行**: `file.read`, `file.write_proposal`, `web.fetch`, `web.search` (DuckDuckGo+fallback), `calendar.read` (ICS parser), `mcp.call_tool`, `a2a.call_agent` (30s超时+私网拦截), `task.create_proposal`, `permission.*`, Core OS Tools
- **P1 proposal-only governed executor**: `calendar.propose_event` (ScheduledTask Proposal only), `email.propose_draft` (DataExport/email-draft Proposal only)
- **P2 declarative-only**: `email.read` (需IMAP配置)
- **安全加固**: safe_paths strict canonical parent, web.fetch DNS 私网拦截, ExternalWriteAction 二次校验；ExternalWriteAction 入库前 size limit + payload minimization 是硬验收；web.search 5秒rate limit
- **权限闭环**: peek() + check(), replay 预检查, ToolPermission Proposal, Review Center 授权

### 测试覆盖

- Do not hardcode test counts in planning docs; they drift quickly.
- `make ci` is the release gate and includes format-check, Rust tests, frontend tests, and frontend build/typecheck.

### W1-W60 LifeModel-Governed Runtime Progress (2026-06-02)

| Work Package | 状态 | 边界 |
| --- | --- | --- |
| W1-W3 | ✅ Done | Tool/proposal hygiene, thin runtime spine, ReAct runtime contract convergence. |
| W4-W5 | ✅ Done | Maturation/Governor foundations exist. |
| W6 | ✅ Done | Historical core MVP slice; PlanExecute weekly planning is now productized as W98-W105 and RuntimeStrategy maturity is complete as W106-W113. |
| W7-W8 | ✅ Done | StrategySelector and MultiStrategy orchestrator exist. |
| W9-W10 | ✅ Done | Preview command exists and writes metadata-safe outer AgentRun audit. |
| W11-W13 | ✅ Done | Docs/status sync, non-default preview UI, and guarded Chat preview subpath exist. |
| W14-W16 | ✅ Done | Maturation V1 service, PlanExecute governed V1 report, and RuntimeStrategy adapter/registry foundation exist. |
| W17 | ✅ Done | Runtime Migration Gate provides read-only diagnostics for default Chat unchanged, preview health, metadata-safe trace, fallback, no external writes, proposal-first, and blocking reasons. |
| W18 | ✅ Done | Settings Runtime Migration Gate exposes pass/block evidence and blocking reasons without running preview or changing default Chat. |
| W19 | ✅ Done | Sustained Gate Evidence / Pilot Eligibility checks the latest 3 preview gate reports read-only and exposes pilot qualification without creating AgentRun/Proposal/Action/Observation. |
| W20 | ✅ Done | Very small Chat Controlled Pilot adds explicit single-turn `Run Controlled Pilot`: eligibility first, blocked means no preview, eligible means `allowWrites=false` preview, success renders “Pilot response”, normal Send unchanged. |
| W21 | ✅ Done | Reviewed Pilot Response Promotion adds explicit review/confirmation for successful pilot `userOutput`, writes one assistant chat message with existing `run_id` metadata when available, prevents duplicate promotion, and keeps blocked/failed/canceled/no-output/default Send paths unchanged. |
| W22 | ✅ Done | Post-Promotion Validation binds pilot results to source chat sessions, shows source/target session plus runId/strategy/governance in review, blocks source/target mismatch without `save_chat_message`, and prompts rerunning the pilot in the current session. |
| W23-W29 | ✅ Done | Promotion evidence, readiness, reviewed migration draft, migration review decision, implementation gate, controlled shadow run, and shadow review evidence are metadata-safe and non-default. |
| W30-W33 | ✅ Done | Cutover readiness, cutover candidate adapter, candidate review evidence, and candidate promotion readiness validate contract shape and evidence only; they do not migrate default Chat. |
| W34-W37 | ✅ Done | Default Chat boundary status, activation plan draft, activation review evidence, and activation implementation gate expose reviewed activation planning without switching routing. |
| W38-W42 | ✅ Done | Disabled routing scaffold, contract harness, dry-run boundary, dry-run review evidence, and implementation readiness keep adapter work observable and write-disabled. |
| W43-W48 | ✅ Done | Controlled preview, controlled preview review, approval readiness, cutover implementation plan draft, cutover plan review, and cutover plan approval readiness are explicit non-default review steps. |
| W49-W53 | ✅ Done | Route guard scaffold, cutover invocation harness, invocation plan, invocation boundary, and typed callsite contract keep ordinary send/stream fail-closed on `legacy_stream`. |
| W54 | ✅ Done | Authority roadmap sync updates high-priority planning docs from stale W22 status to W54/W1-W53 current state. |
| W55 | ✅ Done | Ordinary-entry preflight / side-effect lock requires typed contract ready, legacy entry allowed, controlled executor unattached, migration disabled, and zero pre-entry runtime/model/tool/write budget before default Chat enters `legacy_stream`. |
| W56 | ✅ Done | Ordinary-entry preflight status exposes W55 send/stream preflight readiness, route state, blockers, side-effect lock, and metadata-safe summary to Settings without runtime/model/tool calls, records, routing changes, or migration. |
| W57 | ✅ Done | Narrow implementation discussion gate combines W48 cutover plan approval readiness with W56 ordinary-entry preflight status and only reports whether a narrow adapter implementation slice may be discussed; it runs no runtime/model/tool call, writes no records, changes no routing, and is not migration. |
| W58 | ✅ Done | Narrow implementation plan draft calls W57 first; blocked gates return no sections or digest, eligible gates return metadata-safe human-review sections plus stable digest, and it runs no runtime/model/tool/preview call, writes no records, changes no routing, and is not migration. |
| W59 | ✅ Done | Narrow implementation plan review evidence calls W58 first; blocked draft approve writes no evidence, ready draft approve/reject/request_rework writes metadata-safe Evidence only, reviewer notes are stored as checksum/length/category, and it runs no runtime/model/tool/preview call, changes no routing, and is not migration. |
| W60 | ✅ Done | Narrow implementation plan approval readiness calls current W58 draft and W59 review summary; ready requires latest approve, digest match, W57 eligible, default Chat unchanged, controlled adapter disabled, automatic migration disabled, and legacy send/stream paths. It writes no records, runs no runtime/model/tool/preview call, changes no routing, and is not migration. |
| W65 | ✅ Done | Backend-only descriptor skeleton adds a pure metadata-safe mapper for future controlled adapter candidate contract prep. It contains no raw transcript/prompt/tool/LifeModel/Memory content, attaches no executor, writes no records, calls no runtime/model/tool/preview path, and keeps default Chat on `legacy_stream`. |

## 8. Current Next Step

The latest completed development tasks are:

```text
W49: Default Chat adapter route guard scaffold
W50: Default Chat adapter cutover invocation harness
W51: Default Chat adapter invocation plan
W52: Default Chat adapter invocation boundary
W53: Default Chat adapter typed callsite contract
W54: Authority roadmap sync
W55: Default Chat adapter ordinary-entry preflight / side-effect lock
W56: Default Chat adapter ordinary-entry preflight status surface
W57: Default Chat adapter narrow implementation discussion gate
W58: Default Chat adapter narrow implementation plan draft
W59: Default Chat adapter narrow implementation plan review evidence
W60: Default Chat adapter narrow implementation plan approval readiness
W65: Default Chat adapter backend-only descriptor skeleton
```

After W65, the next possible step is:

```text
only consider separately reviewed controlled adapter contract work after the descriptor skeleton remains clean; default Chat still must not migrate
```

Guardrails:

- `run_multi_strategy_agent_preview` remains preview/beta.
- The default Chat path must not be replaced directly.
- Settings Runtime Migration Gate is an evidence surface, not a Chat switching
  control and not a preview runner.
- Settings Pilot eligibility only answers whether recent preview gate evidence
  meets the minimum controlled Chat migration pilot qualification. It is not a
  Chat switching control and cannot automatically replace default Chat.
- W10 AgentRun audit is a metadata-safe outer run; any inner ReAct run id is
  child metadata only.
- `check_runtime_migration_gate` is read-only and must not execute ReAct,
  PlanExecute, tools, or external writes.
- `check_controlled_chat_pilot_eligibility` is read-only and must not create
  AgentRuns, Proposals, Actions, Observations, audit rows, or LifeModel/Memory
  writes.
- W20 Controlled Pilot is explicit, single-turn, and fallback-preserving: normal
  Send does not call eligibility/gate/preview; blocked does not call preview;
  eligible preview forces `allowWrites=false`; success is shown as “Pilot
  response” and is not automatically written as a normal assistant
  message/history entry.
- W21 Reviewed Pilot Response Promotion is explicit user review only: cancel,
  blocked, failed, no-output, and repeated promotion paths write nothing; confirm
  writes only one ordinary assistant chat message and does not write LifeModel,
  Memory, Proposal, or external tool results.
- W22 Post-Promotion Validation binds each pilot result to its source session:
  review must show source/target session, runId, strategy, and governance
  summary; confirmation must block source/target mismatch without calling
  `save_chat_message` and must show rerun fallback guidance.
- W23-W60 evidence, readiness, review, shadow, candidate, activation, dry-run,
  controlled preview, cutover plan, route guard, invocation guard, and typed
  callsite contract / preflight / preflight status / narrow discussion gate /
  narrow implementation plan draft work are non-default guardrails only. They do
  not authorize automatic Chat migration.
- W54 Authority Roadmap Sync means high-priority documents must now be treated
  as aligned with W1-W53 code status; if a future Agent finds a stale W22 route,
  fixing the document is part of the task.
- W55 Default Chat Adapter Ordinary Entry Preflight is a pure guard before
  legacy entry. It does not call controlled preview, runtime/model/tool paths,
  evidence commands, or proposal apply, and it does not migrate default Chat.
- W56 Default Chat Adapter Ordinary Entry Preflight Status is a read-only
  evidence surface over W55. It does not run runtime/model/tool paths, write
  records, change routing, or migrate default Chat.
- W57 Default Chat Adapter Narrow Implementation Discussion Gate is a read-only
  gate over W48/W56. It does not run runtime/model/tool paths, write records,
  change routing, or migrate default Chat; eligible only means discussion-ready.
- W58 Default Chat Adapter Narrow Implementation Plan Draft is a read-only
  draft over W57. It does not run runtime/model/tool/preview paths, write
  records, change routing, or migrate default Chat; draftReady only means
  human-review planning material is available.
- W59 Default Chat Adapter Narrow Implementation Plan Review Evidence records
  only metadata-safe human review evidence over W58. It does not run
  runtime/model/tool/preview paths, change routing, or migrate default Chat;
  approval only means review evidence exists for later implementation discussion.
- W60 Default Chat Adapter Narrow Implementation Plan Approval Readiness Gate
  is read-only over current W58 draft and W59 review summary. It does not write
  records, run runtime/model/tool/preview paths, change routing, or migrate
  default Chat; ready only means discussion evidence remains current.
- Default Chat must remain unchanged until a later reviewed migration stage.
- `make ci` remains the publication gate.

## 9. Historical Plans

Older Alpha/Beta plans are still useful for context, but they are no longer the primary source of truth.

Use this file, `plans/README.md`, and
`openlife_lifemodel_governed_agent_runtime.md` for future planning.
