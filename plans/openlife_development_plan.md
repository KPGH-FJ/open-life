# OpenLife Development Plan

> Version: 2026-05-01
> Current direction: From Alpha+ framework skeleton to ReAct Beta execution kernel
> Architecture source of truth: [`openlife_agent_framework_architecture.md`](/Users/fujing/Desktop/偶来福/plans/openlife_agent_framework_architecture.md)
> Beta roadmap: [`openlife_react_beta_roadmap.md`](/Users/fujing/Desktop/偶来福/plans/openlife_react_beta_roadmap.md)

## 1. Current Strategic Reset

OpenLife is now defined as a **local-first, ReAct-driven personal Agent framework**, not a conventional desktop app.

The existing project already contains many working or partially working modules: LifeModel, Builder, Chat, Memory, MCP, A2A, Calibration, VersionControl, Diagnostics, and model scheduling. The next stage should not add more isolated features. The next stage should introduce the missing architectural spine:

```text
AgentTask -> ContextAssembler -> ModelRouter -> AgentLoop -> Tool/Skill Action -> Observation -> Proposal/Permission -> Apply/Replay -> AgentRun Trace
```

All future development should be judged by one question:

```text
Does this make OpenLife more like a trustworthy ReAct personal Agent OS?
```

Beta must not be declared because one blocker is fixed. Beta requires the Beta Gates in `openlife_react_beta_roadmap.md` to pass, especially ReAct execution, tool registry/action execution, permission replay, LifeModel/Memory governance, skill runtime, ModelRouter privacy, and Runs traceability.

Execution tools are part of the Beta definition. OpenLife must support OpenClaw-like execution seriousness through governed tools such as `mcp.call_tool`, `a2a.call_agent`, `file.read`, `file.write_proposal`, `web.search`, `web.fetch`, `calendar.read`, `calendar.propose_event`, `email.read`, `email.propose_draft`, and `task.create_proposal`. Connectors that cannot truly execute must be registered as disabled/declarative-only and may only create proposals.

## 2. What Is Already Implemented

| Area | Current status | Assessment |
|---|---|---|
| LifeModel | Four-dimensional model exists with Identity, Goals, Capabilities, State, preferences, relationships, snapshots | Strong base, but needs unified patch/proposal semantics |
| Builder | Quick, incremental, and Socratic construction flows exist | Valuable, but should become a LifeModel-building AgentTask |
| Chat | Streaming chat, sessions, persistence, diagnostics, model readiness UI exist | Main surface, but should become the first Agent execution interface |
| Model scheduling | Ollama + cloud provider routing exists | Needs provider-agnostic ModelRouter and per-run route trace |
| Memory | SQLite messages, semantic vectors, memory search and recovery exist | Needs governance, source tracking, and AgentRun linkage |
| MCP/A2A | Tool and external Agent integration foundations exist | Needs ActionExecutor, deny-by-default policy, and consistent audit |
| Execution tools | MCP/A2A exist; file/web/calendar/email/task tools are not yet a complete governed set | Needs Beta tool contracts, capability/risk metadata, disabled/declarative-only handling, and proposal paths for writes |
| Calibration/Evolution | Feedback and model improvement suggestions exist | Needs unified Proposal/Confirmation layer |
| Diagnostics/Safe Mode | Recovery and readiness mechanisms exist | Good foundation for control plane |
| Frontend | Many pages exist and can support trial flow | Needs information architecture reset around Workspace / Agent / LifeModel / Memory / Runs / Settings |

## 3. Current Gaps

### 3.1 Architecture Gaps

- ~~No first-class `AgentTask`.~~ ✅ 已实现
- ~~No first-class `AgentRun`.~~ ✅ 已实现（Chat/Builder/Calibration 全链路追踪）
- ~~No central `AgentRuntime`.~~ ✅ 已实现（LayeredReasoner + DirectReasoner 策略注册）
- ~~No unified `AgentProposal`.~~ ✅ 已实现（ProposalEngine + ProposalStore）
- ~~No consistent representation of tool actions, model actions, memory writes, and LifeModel patches.~~ ✅ 已实现（统一 Proposal 结构）
- ~~Chat, Builder, Calibration, and Evolution still use separate pipelines.~~ ✅ 已完成（Builder/Calibration/Chat 已接入 Proposal 流；Chat Proposal 持久化和 AgentRun 关联已抽共享 helper）

### 3.2 Product Gaps

- UI still looks like a set of app pages, not an Agent framework.
- Users cannot clearly see what context was used by the model.
- Users cannot clearly see why local or cloud model was chosen.
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

## 7. Completed Work Summary (2026-05-02)

### Phase 1-5 全部完成

| Phase | 状态 | 关键交付 |
|-------|------|----------|
| Phase 1: AgentRun Baseline | ✅ | AgentTask/AgentRun/AgentRunStore/SQLite/Tauri 命令 |
| Phase 2: Chat as Agent Surface | ✅ | Run Trace 面板（provider/model/context/memory/error） |
| Phase 3: Unified Proposal Layer | ✅ | ProposalEngine/ProposalStore/全类型 Proposal/Builder+Calibration+Chat 接入 |
| Phase 4: ModelRouter Upgrade | ✅ | Provider registry/health diagnostics/privacy routing/route trace |
| Phase 5: Workspace Frontend Restructure | ✅ | Workspace/Agent/LifeModel/Memory/Runs/Settings 导航收敛 |

### Beta Execution Tools 落地

- **P1 真实可执行**: `file.read`, `file.write_proposal`, `web.fetch`, `mcp.call_tool`, `permission.*`, Core OS Tools
- **P2 declarative-only**: `web.search`, `a2a.call_agent`, `calendar.*`, `email.*`, `task.create_proposal`
- **安全加固**: safe_paths strict canonical parent, web.fetch DNS 私网拦截, ExternalWriteAction 二次校验
- **权限闭环**: peek() + check(), replay 预检查, ToolPermission Proposal, Review Center 授权

### 测试覆盖

- Rust: 270 passed (openlife-core) + 25 passed (openlife-tauri)
- Frontend: 133 passed
- `make ci`: 通过

## 8. Current Next Step

The next concrete development task is:

```text
Phase 6: Proactive Agent MVP
```

Let OpenLife safely initiate useful check-ins without becoming intrusive.

## 8. Historical Plans

Older Alpha/Beta plans are still useful for context, but they are no longer the primary source of truth.

Use this file and `openlife_agent_framework_architecture.md` for future planning.
