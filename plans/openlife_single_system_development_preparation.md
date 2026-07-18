# OpenLife Single-System Development Preparation

> Date: 2026-07-06
> Status: development-preparation artifact, not implementation completion
> Scope: prepare the next large development round whose acceptance standard is
> that every product domain has one authoritative system and old routes are
> removed, not hidden.

## 1. Objective

This preparation plan exists because the next round is not a normal feature
iteration. The target is a cleaner product architecture:

- one ordinary Main Chat runtime;
- one intent/policy router;
- one durable-write governance path;
- one proposal/review workflow;
- one memory/LifeModel write gateway;
- one tool execution gateway;
- one product read model for frontend state;
- no product-visible legacy, beta, stage, migration, cutover, or fallback route.

The development rule is: a phase is not complete if it only adds a new system
while leaving the old product route alive.

## 2. Preparation Work Completed In This Pass

Current local baseline:

- Branch checked: `codex/openlife-product-core-baseline`.
- Worktree checked: clean before this document edit.
- Product source scan covered `src-tauri/src`, `openlife-core/src`, and
  `frontend/src`.
- Active command surface scan covered `src-tauri/src/lib.rs`,
  `src-tauri/src/commands/**`, and `frontend/src/tauri.ts`.
- Direct durable write scan covered proposal, LifeModel, memory, evidence,
  LifeEvent, patch, and state write symbols.
- Frontend read-model scan covered Today, Mailbox, Chat, Companion,
  LifeModel, Settings, and LifeModelEditor state sources.
- Industry-practice check is anchored to primary or official references only:
  OpenAI Agents SDK guardrails/tracing, Anthropic/Claude tool and prompt
  safety guidance, LangGraph persistence/HITL docs, and OWASP LLM risk
  guidance.

This document intentionally does not claim the implementation is done. It is the
entry contract for doing the implementation cleanly.

## 3. Current Architecture Findings That Drive The Plan

### 3.1 Main Chat still has multiple product-shaped runtimes

Observed objects:

- `src-tauri/src/main_chat_turn_pipeline.rs`
- `src-tauri/src/main_chat_kernel.rs`
- `src-tauri/src/main_chat_turn_runtime.rs`
- `src-tauri/src/main_chat_react_runtime.rs`
- `src-tauri/src/main_chat_react_execution.rs`
- `src-tauri/src/main_chat_react_tool_selection.rs`
- `openlife-core/src/agent/main_chat_agent_v1.rs`

The ordinary send/stream entrypoints are thin, which is good. The problem is
inside the pipeline: it still carries source-map residue from old strategy,
tool-loop, route-preview, blocker, and fallback-shaped product paths. Those old
module names are historical/deleted residue, not files to restore as cleanup.
The remaining work is to prove the current send/stream path has a single
runtime owner.

### 3.2 Intent routing is split across several layers

Observed objects:

- `openlife-core/src/agent/main_chat_agent_v1.rs` `StrategyRouter`
- `openlife-core/src/agent/main_chat_governance_intent.rs`
- `openlife-core/src/agent/main_chat_memory_candidate.rs`
- `src-tauri/src/main_chat_preprocess.rs`
- planner/tool/governance keyword helpers such as
  `main_chat_governance_intent.rs` and `main_chat_memory_candidate.rs`

The current problem is not that the keyword list is short. The deeper problem
is that many components and historical/deleted router concepts still influence
intent documentation and guard interpretation. That makes realistic Chinese
daily-life inputs brittle.

### 3.3 Proposal creation is not a single workflow

Observed direct product-shaped proposal writers include:

- `src-tauri/src/commands/builder.rs`
- `src-tauri/src/commands/calibration.rs`
- `src-tauri/src/main_chat_kernel.rs`
- `src-tauri/src/provider_network_consent.rs`
- `src-tauri/src/main_chat_generation_support.rs`
- `src-tauri/src/commands/agent.rs`
- `src-tauri/src/commands/execution.rs`
- `src-tauri/src/commands/proactive.rs`
- `src-tauri/src/commands/proposal.rs`
- `openlife-core/src/agent/action_executor/**`
- `openlife-core/src/agent/maturation.rs`
- `openlife-core/src/agent/plan_execute.rs`

The former `openlife-core/src/agent/proposal_engine.rs` made this split more
obvious: it was wired into `AppState`, bootstrap, ordinary Main Chat
finalization, and AgentRun replay, so it was not dead code. It was deleted with
those consumers because raw-output generators created caller-shaped proposals
without PolicyRouter authorization before ReviewWorkflow submission. The
remaining direct writers above still have to converge on PolicyRouter proof
consumed by ReviewWorkflow.

### 3.4 Memory and LifeModel state have too many product entrypoints

Observed stores and product-facing access points:

- `MemoryStore`
- `VectorStore`
- `MemoryLifecycleStore`
- `LifeEventStore`
- `EvidenceStore`
- `HotMemoryCache`
- `LifeModelManager`
- `PatchStore`
- `create_knowledge_note` (typed KnowledgeNote asset; not accepted long-term Memory truth)
- `save_life_model`
- state/daily-goal compatibility writes
- proposal accept/materialization writes

Multiple storage tables are acceptable. Multiple product write authorities are
not acceptable.

### 3.5 Tool execution combines execution, permission, proposal, and fallback

Observed objects:

- `openlife-core/src/agent/action_executor/mod.rs`
- `openlife-core/src/agent/action_executor/**`
- `openlife-core/src/tool_manifest.rs`
- `openlife-core/src/mcp.rs`
- `src-tauri/src/main_chat_react_*`

The important issue is that tool ability/risk can still be inferred from names
in `ToolManifest`, and product-shaped fallbacks still exist around ReAct/tool
execution. The final product route needs explicit tool contracts and a single
ToolGateway.

### 3.6 Frontend product state is assembled from several sources

Observed pages:

- `frontend/src/pages/TodayPage.tsx`
- `frontend/src/pages/MailboxPage.tsx`
- `frontend/src/pages/ChatPage.tsx`
- `frontend/src/pages/CompanionPage.tsx`
- `frontend/src/pages/LifeModelPage.tsx`
- `frontend/src/pages/LifeModelEditor.tsx`
- `frontend/src/pages/SettingsPage.tsx`
- `frontend/src/pages/settings/MultiStrategyPreviewSection.tsx`

Examples:

- Today reads diagnostics, daily goals, and proposals separately.
- Mailbox reads proposals, config, and diagnostics separately.
- Chat reads diagnostics, scheduler config, LifeModel, daily goals, task state,
  skill detail, and agent snapshots.
- Settings presents readiness from diagnostics, runtime status, router status,
  model router status, tool permissions, provider config, and hot memory cache.

This creates inconsistent state even when backend data is correct.

### 3.7 Product command surface still exposes development and migration systems

Observed active registrations in `src-tauri/src/lib.rs` include:

- beta readiness gates;
- stage1/stage2/stage3/stage4/stage5 reports;
- productization/maturity gates;
- execution-v1/final-acceptance gates;
- controlled pilot/migration/cutover ladder commands;
- stage browser dogfood setup commands;
- retired Builder/Calibration direct-write surfaces;
- debug bundle/report surfaces.

Some of these are useful engineering tools, but they should not remain in the
same product command surface as ordinary user functionality.

## 4. Industry Practice Baseline, Adapted To OpenLife

The plan uses external practice only where it maps cleanly to OpenLife.

1. Agent orchestration should have a small number of primitives and one managed
   run loop. OpenAI's Agents SDK frames the core around agents, runner/session,
   tools, guardrails, and tracing. OpenLife should not copy that SDK, but should
   adopt the shape: one turn runner, explicit tools, guardrails, sessions, and
   trace events.

2. Guardrails should sit before and after tool execution, not only in prompt
   text. OpenAI tool guardrails and OWASP excessive-agency guidance both point
   to explicit pre-execution policy, post-execution validation, and bounded
   autonomy. OpenLife should implement this as ToolGateway plus
   DurableWritePolicy.

3. Long-running agents need persisted state and resumable interruptions.
   LangGraph separates checkpointers for thread-scoped execution state from
   stores for cross-thread long-term memory, and its HITL pattern pauses risky
   tool calls for approval. OpenLife should use the same distinction:
   task/session checkpoints are not long-term memory, and proposal approval is
   not the same as execution success.

4. Tracing is product infrastructure, not a debug afterthought. OpenAI tracing
   records generations, tool calls, guardrails, and handoffs. OpenLife already
   has a strong evidence culture; this round should consolidate it into the
   single runtime instead of keeping many parallel eval/status reports.

5. Prompt injection and excessive agency require least privilege. OWASP and
   Claude tool/computer-use guidance both emphasize isolating untrusted content,
   limiting permissions, and requiring user review for consequential actions.
   OpenLife's local-first design helps privacy, but it does not remove the need
   for explicit permission and write governance.

Reference sources selected as verification anchors for this preparation:

- OpenAI Agents SDK overview:
  `https://openai.github.io/openai-agents-python/`
- OpenAI Agents SDK guardrails:
  `https://openai.github.io/openai-agents-python/guardrails/`
- OpenAI Agents SDK tracing:
  `https://openai.github.io/openai-agents-python/tracing/`
- Anthropic tool-use overview:
  `https://platform.claude.com/docs/en/agents-and-tools/tool-use/overview`
- Anthropic computer-use security precautions:
  `https://platform.claude.com/docs/en/agents-and-tools/tool-use/computer-use-tool`
- Anthropic prompt-injection mitigation:
  `https://platform.claude.com/docs/en/test-and-evaluate/strengthen-guardrails/mitigate-jailbreaks`
- LangGraph persistence:
  `https://docs.langchain.com/oss/python/langgraph/persistence`
- LangGraph/LangChain human-in-the-loop:
  `https://docs.langchain.com/oss/python/langchain/human-in-the-loop`
- LangChain memory overview:
  `https://docs.langchain.com/oss/python/concepts/memory`
- OWASP Top 10 for LLM Applications:
  `https://owasp.org/www-project-top-10-for-large-language-model-applications/`

## 5. Non-Negotiable Engineering Rules

- No soft deprecation for product routes. Old product routes must be deleted or
  moved out of the product command surface.
- No new system without deletion of the system it replaces.
- No keyword-router patching as the final routing architecture.
- No direct durable writes from product features except through the designated
  gateway for that domain.
- No frontend page may become an independent authority for readiness, pending
  work, memory state, or LifeModel state.
- No tool execution credit from inferred tool names.
- No final delivery status based only on assistant prose.
- No broad heavy test matrix as a substitute for clear architecture. Use narrow
  contract gates plus realistic Computer Use trials.

## 6. Seven-Phase Preparation Plan

### Phase 1: Single-System Authority Map And Deletion Contract

Role in the large development:

Phase 1 prevents the rest of the work from creating another parallel system.
It defines which system owns each product domain and what must be removed.

Problems to solve:

- Old plans and modules still steer work.
- Product and development command surfaces are mixed.
- There is no repo-level deletion manifest.
- Tests still sometimes protect "visible fallback" instead of banning old
  product routes.

Objects to inspect before coding:

- `plans/README.md`
- `src-tauri/src/lib.rs`
- `frontend/src/tauri.ts`
- `src-tauri/src/commands/agent_runtime/**`
- `src-tauri/src/legacy_write_convergence.rs`
- `docs/ARCHITECTURE.md`
- all `main_chat_*stage*`, `main_chat_*beta*`, `main_chat_*productization*`,
  and `main_chat_*maturity*` files.

Solution:

- Create a product authority table.
- Create a deletion manifest for old modules, commands, frontend wrappers, and
  tests.
- Define static guards before implementation starts.
- Mark historical docs as archive/reference only or delete them during the
  relevant phase.

Start-before checklist:

- Confirm clean branch.
- Run source inventory for backend, core, frontend, and plans.
- Classify each artifact as `keep`, `absorb_then_delete`, `delete`,
  `test_fixture_only`, or `archive_reference`.
- Define the exact target module/API name for each product authority.

Acceptance:

- Every phase has a deletion list.
- Every old route has an owner and final disposition.
- There is no "temporary indefinite compatibility" bucket.

### Phase 2: Main Chat Single Turn Runtime

Role in the large development:

Main Chat is the product spine. If it remains multi-route, every later memory,
proposal, tool, and UI fix will keep inheriting inconsistent behavior.

Problems to solve:

- `main_chat_turn_pipeline.rs` still dispatches among kernel, tool loop,
  strategy helper, and blocker paths.
- Historical/deleted strategy, tool-loop, legacy-agent-loop, and route-preview
  module names still appear in source-map residue and guard interpretation.
- The current pipeline must prove that any fallback-shaped behavior is explicit
  blocker/HITL state, not a second product runtime.

Objects to inspect before coding:

- `src-tauri/src/main_chat_send.rs`
- `src-tauri/src/main_chat_streaming.rs`
- `src-tauri/src/main_chat_turn_pipeline.rs`
- `src-tauri/src/main_chat_kernel.rs`
- `src-tauri/src/main_chat_turn_runtime.rs`
- `src-tauri/src/main_chat_runtime_support.rs`
- `src-tauri/src/main_chat_event_stream.rs`
- `src-tauri/src/main_chat_react_runtime.rs`
- `src-tauri/src/main_chat_react_execution.rs`
- `openlife-core/src/agent/main_chat_agent_v1.rs`

Solution:

- Introduce one product runtime concept: `OpenLifeTurnRuntime`.
- Treat direct answer, read, tool, plan, proposal, blocker, and final delivery
  as states inside one runtime, not separate product runtimes.
- Absorb useful code from kernel/tool/strategy modules into the runtime or
  subordinate non-routing services.
- Keep deleted old strategy/legacy module names absent from product use, and
  remove any remaining single-step fallback product behavior.
- Keep send and stream as transport wrappers only.

Start-before checklist:

- Draw the exact send/stream call graph.
- List every runtime state and terminal status.
- Decide which existing structs are reused versus replaced.
- Prepare parity tests for send/stream over the new runtime.
- Prepare static guard that send/stream cannot call old strategy/fallback
  helpers.

Acceptance:

- `send_message` and `start_stream_message` call the same runtime owner.
- Runtime output includes structured final delivery.
- No product path records or executes legacy fallback.
- ReAct/tool failures become explicit blockers or HITL requests, not hidden
  fallback completions.

### Phase 3: IntentFrame And PolicyRouter

Role in the large development:

This phase fixes the root cause behind brittle life-scenario behavior. The
router must understand the user's actual task and governance implications
instead of matching scattered keywords.

Problems to solve:

- `StrategyRouter`, `StrategySelector`, `IntentRouter`, `LayerRouter`, route
  preview, and several helper classifiers all participate in route decisions.
- Chinese daily-life expressions such as "以后我做计划时..." can be misrouted.
- External read, durable write, memory, LifeModel proposal, plan, and direct
  answer decisions are split.

Objects to inspect before coding:

- `openlife-core/src/agent/main_chat_agent_v1.rs`
- `openlife-core/src/agent/main_chat_governance_intent.rs`
- `openlife-core/src/agent/main_chat_memory_candidate.rs`
- `src-tauri/src/main_chat_preprocess.rs`

Solution:

- Define `IntentFrame` as the only semantic input to policy routing.
- Define `PolicyRouter` as the only product router.
- Separate semantic classification from policy:
  - semantic frame: what the user is asking for;
  - policy route: what the system may safely do;
  - model route: which provider/model to use.
- Use model assistance only as a bounded classifier where configured; keep a
  deterministic local uncertainty path that asks for clarification or blocks
  rather than pretending to be confident.
- Delete or absorb old keyword routers.

Start-before checklist:

- Build an intent taxonomy from real life scenarios, not feature names.
- Define expected structured output and confidence/uncertainty behavior.
- Define policy outcomes: answer, read, propose write, ask user, block, plan,
  tool.
- Prepare a small realistic Chinese/English scenario eval set.
- Prepare guard that product route decisions cannot call old routers.

Acceptance:

- Only `PolicyRouter` chooses the product route.
- Keyword helpers, if retained temporarily, are private feature extractors under
  IntentFrame construction and cannot directly route.
- Unknown or ambiguous input becomes ask-user/blocker, not wrong route.

### Phase 4: ReviewWorkflow And DurableWritePolicy

Role in the large development:

This phase makes "what gets remembered" and "what needs approval" consistent.
It is the root fix for memory/proposal confusion.

Problems to solve:

- Many modules create proposals directly.
- The former product-wired ProposalEngine and its post-hoc Main Chat/replay
  consumers have been deleted; no replacement proposal authority may be
  introduced beside PolicyRouter and ReviewWorkflow.
- ToolPermission, LifeModelUpdate, Memory, Builder, Calibration, Maturation,
  PlanExecute, and Proactive proposals are created by different code paths.
- Assistant text can imply "done" when the durable change is only pending.

Objects to inspect before coding:

- `openlife-core/src/agent/proposal_store.rs`
- `openlife-core/src/agent/review_workflow.rs`
- `src-tauri/src/main_chat_kernel.rs`
- `src-tauri/src/provider_network_consent.rs`
- `src-tauri/src/commands/proposal.rs`
- `src-tauri/src/commands/builder.rs`
- `src-tauri/src/commands/calibration.rs`
- `src-tauri/src/commands/proactive.rs`
- `src-tauri/src/commands/execution.rs`
- `openlife-core/src/agent/action_executor/**`
- `openlife-core/src/agent/maturation.rs`
- `openlife-core/src/agent/plan_execute.rs`

Solution:

- Define `DurableWriteRequest` and `DurableWriteDecision`.
- Define one `ReviewWorkflow` that creates proposals, records evidence, and
  handles accept/edit/reject/postpone.
- Replace direct proposal creation in product paths with ReviewWorkflow calls.
- Preserve ProposalStore as storage only.
- Remove placeholder proposal generators or convert them into internal
  adapters owned by ReviewWorkflow.

Start-before checklist:

- Inventory all direct `create_proposal` callsites and classify each one.
- Define proposal source taxonomy and risk taxonomy.
- Define which memory writes do not enter proposal and why.
- Define final delivery wording/status for pending proposals.
- Prepare static guard banning direct product `ProposalStore::create_proposal`.

Acceptance:

- Product proposal creation has one gateway.
- Builder, Calibration, Main Chat, ToolPermission, Maturation, PlanExecute, and
  Proactive use the same workflow.
- Creating a proposal is never presented as a completed durable write.

### Phase 5: MemoryGateway And LifeModelWriteGateway

Role in the large development:

This phase makes local memory powerful without making it unsafe or incoherent.
It also separates raw observations from canonical LifeModel truth.

Problems to solve:

- MemoryStore, VectorStore, MemoryLifecycleStore, LifeEventStore, EvidenceStore,
  HotMemoryCache, and LifeModelManager are all reachable from product code.
- The retired `index_memory_chunk` direct-Memory route must remain absent;
  `create_knowledge_note` owns only private KnowledgeNote assets, while durable
  user facts remain governed by MemoryLifecycle/ReviewWorkflow.
- `save_life_model`, state/daily-goal writes, proposal accept, restore/import,
  and auto-checkin all interact with LifeModel persistence.
- It is unclear which user statements become memory, which become proposal, and
  which remain context only.

Objects to inspect before coding:

- `src-tauri/src/commands/memory.rs`
- `src-tauri/src/commands/life_model.rs`
- `src-tauri/src/commands/state.rs`
- `src-tauri/src/commands/settings.rs`
- `src-tauri/src/commands/proposal.rs`
- `src-tauri/src/main_chat_preprocess.rs`
- `src-tauri/src/main_chat_kernel.rs`
- `openlife-core/src/memory.rs`
- `openlife-core/src/vectors.rs`
- `openlife-core/src/agent/memory_lifecycle.rs`
- `openlife-core/src/agent/evidence_store.rs`
- `openlife-core/src/agent/lifemodel_backend_completion.rs`
- `openlife-core/src/life_model.rs`

Solution:

- Define one `MemoryGateway` for product memory reads/writes.
- Define one `LifeModelWriteGateway` for canonical LifeModel updates.
- Separate memory lanes:
  - turn context;
  - episodic life events;
  - semantic facts/preferences;
  - procedural rules;
  - evidence records;
  - canonical LifeModel truth.
- Old data remains readable through migration/read compatibility, but all new
  product writes go through the gateways.

Start-before checklist:

- Inventory all direct memory/LifeModel write callsites.
- Define lane rules and proposal thresholds.
- Define conflict/rollback behavior.
- Define how accepted proposals materialize into Memory/LifeModel.
- Prepare static guards against direct product writes to MemoryStore,
  VectorStore, LifeModelManager, and PatchStore.

Acceptance:

- Product code cannot directly write long-term memory or canonical LifeModel.
- Direct manual editor save, if retained, is clearly a governed manual override,
  not automated learning.
- Food, health, preference, routine, and planning data are remembered according
  to lane policy and local privacy expectations.

### Phase 6: ToolGateway, FinalDelivery, And LifeStateProjection

Role in the large development:

This phase aligns execution, result reporting, and UI truth. It prevents the
user from seeing "completed" when the system only created a proposal, hit a
blocker, or used stale page state.

Problems to solve:

- ActionExecutor owns too many domains.
- ToolManifest can infer risk/capability from names.
- ToolLoop still has fallback-shaped execution.
- FinalDelivery exists in several productization/stage forms instead of one
  canonical runtime output.
- Today/Mailbox/Chat/LifeModel/Settings read different state sources.

Objects to inspect before coding:

- `openlife-core/src/agent/action_executor/mod.rs`
- `openlife-core/src/agent/action_executor/**`
- `openlife-core/src/tool_manifest.rs`
- `openlife-core/src/mcp.rs`
- `src-tauri/src/main_chat_react_tool_selection.rs`
- `src-tauri/src/main_chat_react_runtime.rs`
- `plans/main_chat_final_delivery_contract_v1.md`
- `frontend/src/pages/TodayPage.tsx`
- `frontend/src/pages/MailboxPage.tsx`
- `frontend/src/pages/ChatPage.tsx`
- `frontend/src/pages/LifeModelPage.tsx`
- `frontend/src/pages/SettingsPage.tsx`
- `frontend/src/tauri.ts`

Solution:

- Define `ToolGateway` as the only product execution authority.
- Tool manifests must use explicit capability/risk/action metadata. Name
  inference may only be a migration warning, not execution credit.
- Define one canonical `FinalDeliveryView` in the runtime output.
- Define `LifeStateProjection` as the product read model for common pages.
- Refactor frontend pages to consume the projection instead of combining raw
  diagnostics/proposals/config/LifeModel locally.

Start-before checklist:

- Inventory every tool action type and permission path.
- Define manifest contract and rejection behavior for incomplete manifests.
- Define FinalDelivery sections and status mapping.
- Define LifeStateProjection schema.
- Prepare frontend import guard preventing pages from reading raw product state
  sources where projection is required.

Acceptance:

- No product tool execution from inferred names.
- No single-step fallback execution.
- Final delivery distinguishes completed, completed with pending items, blocked,
  failed, and cancelled.
- Today, Mailbox, Chat, Companion, LifeModel, and Settings agree on pending
  proposals/readiness/task state.

### Phase 7: Old-System Removal, Lean Verification, And Real Computer Use Trial

Role in the large development:

This phase proves the project is clean and usable. It deletes leftover old
systems and validates the actual desktop product through realistic use.

Problems to solve:

- Active Tauri handler exposes beta/stage/migration/cutover/dev commands.
- Frontend wrappers and tests still reference old product concepts.
- Plans and docs can keep stale authority alive.
- Heavy tests can hide architecture confusion and slow development.

Objects to inspect before coding:

- `src-tauri/src/lib.rs`
- `src-tauri/src/commands/agent_runtime/**`
- `frontend/src/tauri.ts`
- `frontend/src/test/**`
- `frontend/e2e/**`
- all `main_chat_*stage*`, `main_chat_*beta*`, `main_chat_*productization*`,
  `main_chat_*maturity*`, and `legacy_*` files
- `plans/**`
- `docs/**`

Solution:

- Remove old product commands from the shipped handler.
- Delete or archive old modules after their useful assertions are converted to
  the new contracts.
- Convert tests from "old route visible and counted" to "old route cannot be
  called".
- Keep verification lean:
  - compile/typecheck gates;
  - focused contract tests;
  - static old-route guards;
  - small realistic scenario eval;
  - Computer Use product trial.

Start-before checklist:

- Re-run the deletion manifest and mark every object done/not-done.
- List command surface before/after.
- List frontend wrapper exports before/after.
- Define final Computer Use scenarios:
  - first LifeModel construction;
  - ordinary daily planning;
  - food/preference memory;
  - external-fact request with evidence/blocker behavior;
  - proposal approval/edit/reject;
  - task resume after permission;
  - cross-page state consistency.

Acceptance:

- No old product runtime remains.
- No old product command remains exposed.
- No frontend page depends on old product status fields.
- No active plan document can override the single-system contract.
- Computer Use trial verifies the product path end to end.

## 7. Cross-Phase Anti-Contradiction Checks

Use these checks before and after every phase:

- If the goal says "clean", do not add an adapter that becomes a permanent
  second route.
- If the goal says "best effect", do not claim keyword patches are an excellent
  semantic router.
- If the goal says "local private memory is safe enough to remember life data",
  do not block all memory writes by default; route them through lane policy and
  review thresholds.
- If the goal says "proposal governance", do not answer "remembered" before
  acceptance unless the chosen memory lane explicitly allows direct memory.
- If the goal says "single system", do not keep stage/beta/migration commands
  in the product handler.
- If the goal says "avoid heavy tests", do not replace architecture cleanup with
  another large brittle matrix.

## 8. Required Static Guards

The implementation phase should add focused static guards for:

- ordinary send/stream cannot call retired strategy/fallback helpers;
- product code cannot call `ProposalStore::create_proposal` outside
  ReviewWorkflow/test fixtures;
- product code cannot call direct MemoryStore/VectorStore/LifeModel writes
  outside MemoryGateway/LifeModelWriteGateway;
- shipped Tauri handler cannot register migration/cutover/stage/beta commands;
- frontend product pages cannot import raw diagnostics/proposals/config for
  state covered by LifeStateProjection;
- tool execution cannot credit inferred capability/risk labels;
- final delivery cannot mark proposal-only/blocker-only/tool-not-run scenarios
  as `completed`.

## 9. Self-Review Of This Preparation

Review pass 1, scope:

- This plan covers all seven phases and names concrete source objects for each.
- It avoids claiming implementation readiness.
- It makes deletion part of every phase.

Review pass 2, consistency:

- The plan does not propose keeping multiple routers.
- It does not propose keeping direct proposal writes.
- It does not propose using diagnostics as product read-model truth.
- It does not propose keeping name-inference as product tool authority.

Review pass 3, risk:

- The plan is aggressive and will touch high-blast-radius files.
- The right mitigation is not more fallback routes. The mitigation is smaller
  phase boundaries, static guards, and contract tests.
- Heavy e2e matrices should be deferred until the single product route exists.

## 10. Next Development Entry

The next coding step should start with Phase 1 only:

1. create the authority/deletion guard files or tests;
2. update active plan governance so old plans cannot steer new work;
3. establish the first static checks over command surface and direct writes;
4. only then enter Main Chat runtime code changes.

## 11. Stage4C Repository Missing-Record Closure

Stage4C keeps this preparation document subordinate to the Phase7 deletion
manifest and closes the repository-link baseline's remaining active missing
records as expected-absent evidence.

| Category | Records |
| --- | ---: |
| `active_doc_missing_records` | 37 |
| `active_expected_absent_records` | 37 |
| `stage4c_verified_expected_absent_records` | 37 |
| `active_actionable_repair_records` | 0 |
| `active_future_blocked_records` | 0 |
| `active_adr_blocked_records` | 0 |
| `active_unresolved_missing_records` | 0 |

This is not Phase7 completion. The Computer Use trial remains
`red-until-trial-green`, Main Chat Agent Execution v1 remains incomplete, and
live provider evidence remains incomplete. Stage5A later changed the
runtime-module guard result: `cargo test -p openlife-tauri
main_chat_runtime_module -- --nocapture` now passes under the current Phase7
owner-shape guard, which only removes that inherited blocker and does not
complete Phase7, Main Chat Agent Execution v1, or external live-provider
evidence.
