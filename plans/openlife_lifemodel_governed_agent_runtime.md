# OpenLife LifeModel-Governed Agent Runtime Program

> Date: 2026-06-02
> Status: W57 Default Chat Adapter Narrow Implementation Discussion Gate complete; default Chat remains unchanged
> Scope: post-LifeModel-HS MVP convergence, runtime strategy direction, and next implementation order

## 1. Purpose

This document is the program baseline for the next OpenLife development cycle.
As of W57, it records that MultiStrategy work is preview/audit-ready with a
lightweight fixed RuntimeStrategy adapter boundary, read-only migration gates,
Settings evidence surfaces, explicit controlled pilot/shadow/candidate paths,
metadata-safe review evidence, and a disabled default Chat adapter guard stack
through typed callsite contracts, an ordinary-entry preflight / side-effect
lock, a read-only ordinary-entry preflight status surface, and a read-only
narrow implementation discussion gate. MultiStrategy is still not the default
Chat runtime.

It updates the project framing from:

```text
OpenLife = LifeModel + ReAct Agent Runtime + Tools + Memory
```

to:

```text
OpenLife = LifeModel-HS Protocol Layer
         + Governed Agent Runtime
         + Runtime Strategies
         + LifeModel Maturation Loop
```

The goal is not to replace the existing architecture documents. The goal is to
unify them into one development program so future Agents do not overfit to only
one axis:

- LifeModel is not merely a product section or profile editor.
- ReAct is not the only possible execution architecture.
- Multi-strategy runtime should not be built before the protocol spine is real.
- LifeModel maturation should not proceed through scattered direct writes.

Current status details are summarized in
`plans/lifemodel_governed_runtime_progress.md`. That file is a compact status
index, not a replacement roadmap.

## 2. Strategic Thesis

OpenLife's durable advantage should be:

```text
A personal Agent OS whose actions are driven, constrained, explained, and
continuously improved by the user's private LifeModel.
```

LifeModel-HS provides the personal governance and continuity layer.
Runtime strategies provide the execution layer.

They are different responsibilities:

| Layer | Responsibility |
| --- | --- |
| LifeModel-HS | Who the user is, what matters, what is sensitive, what can be inferred, what can change, what must be confirmed. |
| Governed Runtime | How a task is executed, which model is used, which tools are called, what is observed, what is proposed, what is audited. |
| RuntimeStrategy | The concrete execution pattern: Direct, Layered, ReAct, Plan-Execute, Workflow, Proactive, Reflective. |
| Maturation Loop | How real use creates events, signals, evidence, proposals, accepted updates, and future behavioral improvement. |

The correct target is therefore:

```text
LifeModel-Governed Multi-Strategy Agent Runtime
```

But the correct implementation order is narrower:

```text
tool/proposal hygiene
-> thin runtime spine
-> ReAct convergence
-> maturation loop
-> governor
-> Plan-Execute
-> strategy abstraction
```

## 3. Relationship To Existing Documents

This document sits above these existing baselines:

1. `plans/openlife_agent_framework_architecture.md`
   - Still the Agent Framework baseline.
   - ReAct should now be interpreted as the current default runtime strategy,
     not the final boundary of the architecture.

2. `plans/openlife_react_beta_roadmap.md`
   - Still the Beta execution seriousness baseline.
   - Its tool, permission, replay, audit, and AgentRun gates remain mandatory.

3. `plans/adr/0013-lifemodel-hs-source-of-truth-governance.md`
   - Still the hard governance baseline for LifeModel-HS.
   - Its source-of-truth, proposal-first, privacy, and materialized-view rules
     are non-negotiable.

4. `plans/lifemodel_hs_mvp_task_specs.md`
   - Documents the completed/near-completed LifeModel-HS MVP work package.
   - Future work should build on it, not re-run it as if the stores do not
     exist.

5. `plans/lifemodel_hs_legacy_write_path_audit.md`
   - The current map of legacy direct writes.
   - This is the starting point for convergence tasks.

6. `plans/lifemodel_governed_runtime_progress.md`
   - Compact W1-W57 status table and preview/not-default/gate evidence/pilot eligibility/controlled pilot/promotion-validation/evidence/readiness/planning/review/shadow/cutover/default-adapter guard boundary/status.
   - It must not override the strategic order in this program.

## 4. Current Code Baseline

As of this preparation document, the project already has meaningful primitives:

| Area | Current implementation | Current maturity |
| --- | --- | --- |
| AgentLoop | `openlife-core/src/agent/agent_loop.rs` | Real ReAct-style loop with parse, action execution, observation, follow-up, budgets, streaming callbacks. |
| AgentRuntime | `openlife-core/src/agent/runtime.rs` | Context assembly plus Direct/Layered reasoning strategy registration and shared runtime contract. |
| RuntimeHSPacket | `openlife-core/src/agent/hs_selector.rs` | Deterministic policy/heuristic packet with metadata-safe audit. |
| PolicyStore | `openlife-core/src/agent/policy_store.rs` | Built-in hard policy MVP: sensitive LocalOnly and external write proposal-first. Not persisted/user-governed yet. |
| EvidenceStore | `openlife-core/src/agent/evidence_store.rs` | Persisted candidate evidence layer with source refs and digests. No full LifeEvent/Signal pipeline yet. |
| HeuristicStore | `openlife-core/src/agent/heuristic_store.rs` | Persisted collaboration guidance store with lifecycle and seeded MVP heuristics. |
| RegressionSuite | `openlife-core/src/agent/regression_suite.rs` | Deterministic MVP behavior checks. Not yet a durable user scenario store. |
| PlanExecute | `openlife-core/src/agent/plan_execute.rs` | Core MVP for governed plan payloads. Not a productized weekly planning flow. |
| StrategySelector | `openlife-core/src/agent/strategy.rs` | Selects ReAct, PlanExecute, or Blocked with metadata-safe summaries. Not a formal strategy trait. |
| MultiStrategyRuntime | `openlife-core/src/agent/multi_strategy_runtime.rs` | Preview/core orchestrator for selected payloads. Not the default Chat runtime. |
| Preview command | `src-tauri/src/commands/agent_runtime.rs::run_multi_strategy_agent_preview` | Non-default preview/beta command. W10 persists a metadata-safe outer AgentRun audit. |
| Runtime Migration Gate | `openlife-core/src/agent/runtime_migration_gate.rs`, `check_runtime_migration_gate` | Read-only diagnostic over existing preview AgentRun audit. Does not execute ReAct, PlanExecute, tools, or external writes. |
| Gate evidence surface | `frontend/src/pages/settings/MultiStrategyPreviewSection.tsx` | Settings-only Runtime Migration Gate panel that displays pass/block fields and blocking reasons. It is not a Chat switching control and does not run preview automatically. |
| Pilot Eligibility | `openlife-core/src/agent/runtime_migration_gate.rs`, `check_controlled_chat_pilot_eligibility`, Settings Pilot eligibility panel | W19 read-only sustained evidence check over the latest 3 preview gate reports. It returns eligibility, clean count, checked run ids, blockers, and latest gate report; it creates no AgentRun/Proposal/Action/Observation and is not a Chat switch. |
| Controlled Chat Pilot / Promotion | `frontend/src/pages/ChatPage.tsx` | W20 explicit single-turn pilot plus W21 reviewed promotion and W22 source-bound validation. The pilot calls eligibility before preview, blocks without preview when ineligible, runs `run_multi_strategy_agent_preview` only when eligible with `allowWrites=false`, displays “Pilot response” separately, and keeps normal Send unchanged. Promotion can write one ordinary assistant chat message with existing `run_id` metadata only after explicit review/confirmation and only when the current target session matches the pilot source session. |
| Promotion / migration evidence ladder | `src-tauri/src/commands/agent_runtime.rs`, `frontend/src/pages/settings/MultiStrategyPreviewSection.tsx` | W23-W33 add metadata-safe promotion evidence, readiness, reviewed migration plan, review decision evidence, implementation gate, shadow run/review, cutover readiness, candidate adapter/review, and candidate promotion readiness. These are explicit, evidence-backed, and non-default; readiness means implementation discussion, not migration permission. |
| Default Chat adapter guard ladder | `src-tauri/src/default_chat_adapter.rs`, `src-tauri/src/lib.rs`, Settings panels | W34-W57 make the default Chat boundary observable, activation-planned, reviewable, implementation-gated, disabled-routed, contract-checked, dry-run-reviewed, implementation-readiness-checked, controlled-preview-reviewed, cutover-plan-reviewed, route-guarded, invocation-guarded, typed-callsite-guarded, ordinary-entry-preflighted, preflight-status-visible, and narrow-discussion-gated. Ordinary `send_message` / `start_stream_message` still enter `legacy_stream`; controlled adapter execution remains disabled and unattached. |
| Authority roadmap sync | `AGENTS.md`, `README.md`, `plans/README.md`, this document, `plans/openlife_development_plan.md`, `plans/lifemodel_governed_runtime_progress.md` | W54 realigns high-priority route documents with W1-W53 code status so future Agents do not follow stale W22 instructions. This is documentation governance, not runtime migration. |
| Preview trace UI | `frontend/src/utils/previewAudit.ts`, Runs, `RunTracePanel` | Displays preview strategy, payload, governance, warnings, and metadata-safe trace fields. |
| ProposalStore | `openlife-core/src/agent/proposal_store.rs` | Unified proposal storage and review states. |
| Proposal apply | `src-tauri/src/commands/proposal.rs` | Main convergence target for LifeModel, memory, tool permission, scheduled task, data export, and external write application. |
| Model routing | `openlife-core/src/agent/model_router.rs`, `openlife-core/src/scheduler.rs` | Role/privacy-aware router plus scheduler integration; HS LocalOnly can fail closed. |
| Compatibility LifeModel | `openlife-core/src/life_model.rs` | YAML/struct compatibility view remains broadly used. It is not the final HS source of truth. |

This means the next phase should not start from blank design. It should
converge existing primitives into a stronger spine.

## 4.1 Current Actual Progress

The original strategic order remains valid:

```text
tool/proposal hygiene
-> thin runtime spine
-> ReAct convergence
-> maturation loop
-> governor
-> Plan-Execute
-> strategy abstraction
```

The implementation has moved ahead of the original work-package text in a few
places:

- W1-W57 are complete through Runtime Migration Gate evidence surface, Pilot
  Eligibility, the very small controlled Chat pilot with fallback, reviewed
  pilot response promotion with source-bound validation, promotion and cutover
  evidence ladders, disabled default Chat adapter planning/review/readiness,
  route/invocation/typed-callsite guards, authority roadmap sync,
  ordinary-entry preflight / side-effect lock, ordinary-entry preflight status
  surface, and narrow implementation discussion gate.
- StrategySelector, MultiStrategyRuntime orchestrator, and
  `run_multi_strategy_agent_preview` exist earlier than the original plan
  expected.
- Preview runs now persist metadata-safe outer AgentRun audit records; Runs and
  Trace can show strategy, payload, governance, and warnings.
- `check_runtime_migration_gate` can diagnose an existing preview AgentRun
  without changing default Chat behavior or executing runtime/tool paths.
- Settings now exposes this gate as a read-only evidence surface so developers
  and product can inspect pass/block state and blocking reasons without
  migrating default Chat.
- `check_controlled_chat_pilot_eligibility` now checks whether recent preview
  gate evidence has been continuously clean enough for the minimum controlled
  Chat migration pilot qualification. It is read-only and does not write
  AgentRun, Proposal, Action, Observation, audit, LifeModel, or Memory records.
- Chat now exposes an explicit `Run Controlled Pilot` entry. It is a single-turn
  pilot, not an input takeover: normal Send does not call eligibility/gate/preview;
  blocked eligibility does not call preview; eligible preview forces
  `allowWrites=false`; success is shown as “Pilot response” outside normal
  assistant history.
- A successful Controlled Pilot response with `userOutput` can now be explicitly
  reviewed and promoted by the user. Promotion shows the response text, runId,
  selected strategy, governance summary, payload summary, and a clear warning
  that confirmation writes to current chat history. Confirming writes exactly one
  assistant chat message through the existing chat message save path; cancel,
  blocked, failed, and no-output pilot states write nothing.
- Controlled Pilot promotion now binds the pilot result to the source chat
  session where it was run. Review shows source session, target session, runId,
  selected strategy, and governance summary. Confirmation blocks if the user has
  switched to a different target session, writes nothing, and tells the user to
  rerun Controlled Pilot in the current session or switch back.
- W23-W57 then add metadata-safe promotion evidence, migration planning/review,
  implementation and shadow gates, cutover candidate review, default Chat
  boundary and activation planning, disabled adapter routing, dry-run and
  controlled-preview review, cutover plan approval readiness, a pure
  route/invocation/typed-callsite guard stack, ordinary-entry preflight,
  ordinary-entry preflight status, and a narrow implementation discussion gate.
  All are non-default evidence, status surfaces, discussion gates, or guardrails;
  none replace default Chat.
- W54 syncs this high-priority roadmap with the progress index and current
  entry docs so future Agents do not follow stale W22 instructions.
- W55 adds a pure ordinary-entry preflight / side-effect lock before the legacy
  Chat entry, requiring typed contract readiness, legacy entry, controlled
  executor unattached, migration disabled, and zero runtime/model/tool/write
  pre-entry budget.
- W56 adds a read-only Settings-visible status surface over W55 send/stream
  preflight, reporting readiness, blockers, side-effect lock state, and
  metadata-safe summary without running runtime/model/tool paths or writing
  records.
- W57 adds a read-only Settings-visible narrow implementation discussion gate
  over W48 cutover plan approval readiness and W56 ordinary-entry preflight
  status, reporting only whether a narrow adapter implementation slice can be
  discussed. It does not run runtime/model/tool paths, write records, change
  routing, or migrate default Chat.

These early pieces do not change the boundary:

- Chat main-path migration is not complete.
- The default `send_message` / Chat path must not be directly replaced by
  MultiStrategy Runtime.
- The LifeEvent / Signal / Evidence / Governor loop is not end-to-end.
- PlanExecute is only a core MVP, not a product weekly-planning vertical slice.
- `RuntimeStrategy` now exists as a lightweight ReAct/PlanExecute adapter
  boundary; it remains fixed to those adapters and is not plugin loading.
- The next step is still not direct default Chat replacement. W20-W57 evidence,
  review, readiness, activation, dry-run, controlled preview, cutover plan, and
  adapter guard/status/discussion-gate success are implementation-discussion
  artifacts, not migration permission.

## 5. Target Spine

The target spine for every meaningful AI task is:

```text
AgentTask
  -> RuntimeHSPacket
  -> RuntimeStrategy
  -> AgentRun
  -> Action / Observation
  -> LifeEvent
  -> Signal
  -> Evidence
  -> LifeModelGovernor
  -> Proposal
  -> User decision
  -> Accepted HS asset / Memory / Materialized LifeModel view
```

In MVP form this can be thinner:

```text
AgentTask
  -> RuntimeHSPacket
  -> existing ReAct AgentLoop
  -> AgentRun + Proposal
  -> candidate LifeEvent/Signal/Evidence
  -> Governor MVP
```

The important rule is that all layers agree on the same ownership boundaries.

## 6. Non-Negotiable Invariants

1. LifeModel-HS is the personal protocol layer, not a giant YAML profile.
2. ReAct is the current default runtime strategy, not the only future strategy.
3. Raw chat, memory, files, tool outputs, feedback, and imported data are raw
   life data. They are not accepted LifeModel truth.
4. Signal is weaker than evidence. Evidence is weaker than accepted LifeModel
   state or active collaboration guidance.
5. High-risk identity, values, mission, long-term goals, sensitive
   relationships, health/finance/privacy boundaries, and durable policy changes
   must be proposal-first.
6. Heuristics cannot relax policies. Policy is a hard boundary.
7. Sensitive-topic LocalOnly must be enforced before prompt/model fallback can
   leak data.
8. External writes must be proposal-first unless a specific accepted replay or
   confirmation path says otherwise.
9. YAML/current `LifeModel` is a compatibility materialized view during
   migration.
10. Every runtime strategy must produce traceable `AgentRun` records and must
    use the same ActionExecutor/Permission/Proposal/Audit path.
11. Multi-strategy runtime must be extracted from proven vertical slices, not
    invented as a broad abstraction up front.
12. `make ci` remains the release gate after any implementation task.
13. Documentation entry points and Tool Taxonomy must stay synchronized with
    actual code status. Stale P1/P2 labels are architecture bugs because they
    mislead future Agents.

## 7. Runtime Strategy Model

OpenLife should eventually support multiple runtime strategies:

| Strategy | Purpose | When to use |
| --- | --- | --- |
| Direct | Simple answer or low-risk generation. | Short, low-risk, no tool loop. |
| Layered | Meaning -> strategy -> generation with safety check. | Higher-quality response generation and structured reasoning. |
| ReAct | Reason -> Act -> Observe -> follow-up. | Open-ended tasks with tools, search, files, MCP/A2A, or uncertain next steps. |
| Plan-Execute | Plan -> review -> execute steps -> observe -> reflect. | Longer goals, multi-step plans, project execution, weekly planning. |
| Workflow | Fixed ordered process. | Calibration, import/export, weekly review, onboarding, data maintenance. |
| Proactive | Monitor -> suggest -> proposal/check-in. | Reminders, goal drift, maintenance inbox, state changes. |
| Reflective | Review events/runs -> extract signals -> propose updates. | LifeModel maturation, memory consolidation, evidence maintenance. |

Current implementation note:

- Direct and Layered exist as reasoning strategies inside `AgentRuntime`.
- ReAct exists as `AgentLoop`.
- StrategySelector and MultiStrategyRuntime exist for preview/core orchestration.
- `run_multi_strategy_agent_preview` exposes the orchestrator as a non-default
  preview/beta command and persists metadata-safe outer AgentRun audit.
- A lightweight first-class `RuntimeStrategy` trait now exists for fixed
  ReAct/PlanExecute adapters.
- PlanExecute exists as a governed V1 runtime payload/report path, but the
  product weekly-plan flow still requires explicit review/edit UX and migration
  gates.

## 8. Development Order

### Phase 0: Preparation And Baseline Alignment

Goal:

Make future work share one vocabulary and one architecture order.

Deliverables:

- This document.
- Entry-point docs link to this document.
- Existing ReAct and LifeModel-HS docs remain valid but are scoped.

Acceptance:

- Future Agents can identify the intended order:
  `tool/proposal hygiene -> thin runtime spine -> ReAct convergence ->
  maturation loop -> governor -> Plan-Execute -> strategy abstraction`.

### Phase 1: Ground Hygiene Before More Architecture

Goal:

Remove known tool-governance inconsistencies before adding new runtime layers.

W1 proposal hygiene baseline:

- `calendar.propose_event` and `email.propose_draft` are P1 proposal-only
  governed executors. They must create `ScheduledTask` / `DataExport`
  proposals only, with no real calendar write, email send, or
  `ExternalWriteAction` fallback.
- ExternalWriteAction proposal creation must enforce content size limits before
  proposal storage.
- ExternalWriteAction proposal payloads must minimize stored payload before
  proposal storage. Do not store duplicate/raw sensitive content in `arguments`
  when `content`, hash, size, and preview already suffice.
- Documentation entry points and Tool Taxonomy must be updated in the same work
  package as any tool status change.

Acceptance:

- Integration tests cover `calendar.propose_event`, `email.propose_draft`,
  direct external write, and `mcp.call_tool` wrapped external write.
- Unknown/unimplemented enabled tools do not appear as executable.
- ExternalWriteAction rejects oversized payloads before insertion.
- ExternalWriteAction stores only the minimized proposal payload needed for
  review, hash validation, and apply/replay.
- Documents and taxonomy label `calendar.propose_event` and
  `email.propose_draft` only as P1 proposal-only governed executors unless a
  future governed provider executor and tests are added.
- `make ci` passes.

### Phase 2: Thin LifeModel-Governed Runtime Spine

Goal:

Define a thin common runtime contract without implementing every future
strategy.

Expected shape:

```rust
// Conceptual, not mandatory exact code.
RuntimeInput {
    task: AgentTask,
    hs_packet: Option<RuntimeHSPacket>,
    life_model_compat: LifeModel,
    memory_context: Option<String>,
    tools_prompt: String,
    execution_policy: AgentExecutionBudget,
}

RuntimeOutput {
    agent_run: AgentRun,
    user_output: String,
    actions: Vec<AgentAction>,
    observations: Vec<AgentObservation>,
    proposal_candidates: Vec<AgentProposal>,
    life_event_candidates: Vec<LifeEventDraft>,
    warnings: Vec<String>,
}
```

Rules:

- Keep the current `RuntimeStrategy` boundary lightweight and fixed to proven
  adapters; do not expand it into dynamic plugin loading or broad rewrites.
- First extract shared runtime input/output boundaries and contract tests.
- Stream and non-stream chat should continue moving toward shared execution
  semantics.
- `tools_prompt` is a capability surface, not an intent signal. The HS packet
  selector must not infer that the current task needs a write, external side
  effect, or cloud/tool route merely because the full tool catalog contains
  write/external tools.
- Write/external-side-effect routing must come from explicit user intent,
  parsed/planned action, selected task type, or concrete executor request.

Acceptance:

- ReAct/AgentLoop path consumes `RuntimeHSPacket` consistently.
- Runtime output can carry future LifeEvent candidates without directly writing
  HS truth.
- AgentRun stores HS audit and behavior checks.
- Contract tests cover the `tools_prompt` false-positive case: a broad tool
  catalog alone must not trigger write/external-side-effect HS decisions.

### Phase 3: ReAct Convergence

Goal:

Make the existing ReAct `AgentLoop` the first fully LifeModel-governed runtime
path, without rewriting it or hiding proposal/permission/audit behind page
logic.

Required convergence:

- ReAct consumes the thin `RuntimeInput`/`RuntimeOutput` boundary from Phase 2.
- ReAct uses `RuntimeHSPacket` as task governance context.
- ReAct records selected HS policy/guidance behavior checks into `AgentRun`.
- ReAct action execution continues through the shared
  ActionExecutor/Permission/Proposal/Audit path.
- ReAct follow-up output can emit LifeEvent candidates without mutating HS truth.
- ReAct must not infer write/external intent from full `tools_prompt` catalog
  presence alone.

Acceptance:

- Stream and non-stream ReAct paths have matching governance semantics.
- A tool-rich prompt for a read-only task does not produce write/external HS
  routing decisions.
- ReAct-generated proposals and observations are visible from Run trace.
- `make ci` passes.

#### Required Guardrail: Legacy Direct-Write Convergence

Goal:

Make Review Center and accepted proposals the normal durable write path.

Use `plans/lifemodel_hs_legacy_write_path_audit.md` as the backlog.

Priority order:

1. High-risk LifeModel writes.
2. Feedback evolution direct writes.
3. Builder legacy direct apply and no-signal completion edge cases.
4. Calibration direct/micro-evolution persistence.
5. Manual LifeModel editor save as explicit manual override with audit, or
   patch/proposal review.
6. Snapshot restore/import as explicit rollback/migration paths with audit.
7. State/daily goal direct writes remain short-lived only; durable promotion
   requires proposal/governor.

Acceptance:

- No product path silently updates identity, values, mission, long-term goals,
  sensitive relationships, health/finance/privacy boundaries.
- Direct write paths are either removed, dev-gated, or explicitly audited.
- Tests prevent regression.

### Phase 4: LifeModel Maturation Loop Foundations

Goal:

Begin the LifeModel Maturation Loop without treating raw data as truth.

First sources:

- Chat interaction summary.
- Calibration proposal creation/result.
- Feedback thumbs/events.
- Proposal accepted/rejected/edited.
- Tool outcome metadata.

Core objects:

```text
LifeEvent = immutable normalized thing that happened
Signal = possible meaning extracted from events
Evidence = supported claim candidate with lineage and lifecycle
```

MVP constraints:

- Store digests and redacted summaries, not raw sensitive payloads.
- No signal directly mutates LifeModel.
- Low confidence signals can be dropped or stored as weak candidates.
- Cloud extraction over raw sensitive content is not required.

Acceptance:

- Chat/Feedback/Calibration can emit LifeEvent drafts.
- A deterministic SignalExtractor can produce at least one state/energy or
  planning-style signal.
- EvidenceStore receives candidate evidence with source lineage.

### Phase 5: LifeModelGovernor MVP

Goal:

Create the first formal gate between evidence and user-visible LifeModel or
collaboration rule changes.

Governor responsibilities:

- Classify risk.
- Check evidence sufficiency.
- Detect conflicts with current accepted view or policies.
- Run deterministic behavior checks where relevant.
- Decide one of:
  - create proposal,
  - keep as candidate evidence,
  - weaken/archive,
  - reject as unsafe/insufficient,
  - low-risk transient state auto-update if allowed.

MVP domain:

Start with one narrow, useful domain:

```text
state/energy + planning intensity
```

Why:

- Lower risk than identity/values/long-term goals.
- Already has a seeded low-energy planning heuristic.
- It can visibly improve Chat/ReAct and future Plan-Execute behavior.

Acceptance:

- Repeated low-energy signals can create candidate evidence.
- Governor can produce a proposal or update transient state according to ADR
  0013 limits.
- Accepted/rejected decisions feed future selection behavior.

### Phase 6: LifeModel Maturation Loop V1

Goal:

Make one real loop work end-to-end.

Target loop:

```text
Chat / Feedback / Calibration
  -> LifeEvent
  -> Signal
  -> Evidence
  -> Governor
  -> Proposal
  -> User accept/reject/edit
  -> RuntimeHSPacket changes later behavior
```

Recommended first product behavior:

```text
When the user repeatedly indicates low energy or rejects high-pressure plans,
OpenLife learns a reviewable collaboration rule that future planning should be
shorter, lower-pressure, and easier to start.
```

Acceptance:

- User can see why OpenLife thinks this.
- Rejection becomes negative evidence.
- Future runs show the selected collaboration rule in Run trace.
- The effect is narrow and reversible.

### Phase 7: Plan-Execute Vertical Slice

Goal:

Introduce Plan-Execute through one real scenario, not a broad framework rewrite.

Scenario:

```text
Use my LifeModel to plan this week.
```

Shape:

```text
Plan
  -> user review/edit
  -> execute one step or create proposals
  -> observe outcome
  -> reflect into LifeEvent/Signal/Evidence
```

LifeModel-HS must influence:

- Goal priority.
- Energy/current state.
- Preferred planning intensity.
- Privacy/model route.
- Proposal boundaries.

Acceptance:

- Plan-Execute uses the same RuntimeHSPacket and Proposal path as ReAct.
- Plan review is explicit before external writes or durable LifeModel changes.
- Results create LifeEvent candidates.

### Phase 8: Multi-Strategy Runtime Abstraction

Goal:

Only after ReAct spine, maturation loop, and Plan-Execute slice exist, extract
the shared strategy interface.

Possible shape:

```rust
trait RuntimeStrategy {
    fn name(&self) -> &'static str;
    fn supports(&self, task: &AgentTask) -> StrategySupport;
    async fn run(&self, input: RuntimeInput) -> Result<RuntimeOutput>;
}
```

Do not implement this prematurely. The interface should be extracted from real
Direct/Layered/ReAct/Plan-Execute needs.

Acceptance:

- ReAct remains the default for open-ended tool tasks.
- Direct/Layered remain available for low-risk/simple tasks.
- Plan-Execute handles the first planning slice.
- All strategies produce AgentRun, obey HS policies, use ActionExecutor, and
  output proposals/events.

## 9. Work Package Status And Next Order

### W0: Read And Confirm Baseline

Read:

- `AGENTS.md`
- this document
- `plans/lifemodel_governed_runtime_progress.md`
- `plans/adr/0013-lifemodel-hs-source-of-truth-governance.md`
- `plans/lifemodel_hs_legacy_write_path_audit.md`
- `plans/openlife_react_beta_roadmap.md`

Run:

```bash
make ci
```

### Completed W1-W57

| Work Package | Status | Completion boundary |
| --- | --- | --- |
| W1 Tool / Proposal Hygiene | Done | Proposal-only calendar/email semantics, ExternalWriteAction size/minimization, docs/taxonomy sync. |
| W2 Thin Runtime Spine | Done | Runtime input/output contract and HS packet boundary. |
| W3 ReAct Runtime Contract Convergence | Done | ReAct path consumes runtime/HS contract pieces and keeps AgentRun traceability. |
| W4 LifeModel Maturation Loop Foundation | Done | Foundations for LifeEvent/Signal/Evidence exist; not an end-to-end V1 loop. |
| W5 LifeModel Governor MVP | Done | Governor/policy MVP exists for narrow decisions; not full mature learning. |
| W6 PlanExecute Core MVP | Done | Core governed plan payload exists; not product weekly planning. |
| W7 Strategy Selector | Done | Metadata-safe strategy selection exists. |
| W8 MultiStrategy Runtime Orchestrator | Done | Preview/core orchestrator exists. |
| W9 MultiStrategy Preview Command | Done | `run_multi_strategy_agent_preview` exists as non-default preview/beta command. |
| W10 Preview AgentRun Audit Persistence | Done | Metadata-safe outer AgentRun audit persists preview strategy/payload/governance/warnings. |
| W11 Documentation Status Sync | Done | Entry docs and progress index synced with code status. |
| W12 Non-Default Preview UI / Debug Entry | Done | Settings preview panel calls preview command without replacing Chat. |
| W13 Guarded Chat Subpath Migration | Done | Chat has explicit write-disabled Governed Preview while normal Send stays unchanged. |
| W14 Maturation Loop V1 | Done | RuntimeOutput candidates mature into governed evidence/proposals without direct LifeModel/Memory writes. |
| W15 PlanExecute Governed Vertical Slice | Done | PlanExecuteReport records metadata-safe plan/governance/read-only observation summaries. |
| W16 RuntimeStrategy Trait | Done | ReAct and PlanExecute execute through lightweight fixed adapters and registry. |
| W17 Runtime Integration Hardening / Chat Migration Gate | Done | Read-only gate reports default Chat unchanged, preview health, metadata-safe trace, fallback, no external writes, proposal-first, and blocking reasons. |
| W18 Runtime Migration Gate Evidence Surface | Done | Settings exposes the gate report as a read-only pass/block evidence panel with visible blocking reasons; normal Chat Send still does not call gate or preview. |
| W19 Sustained Gate Evidence / Pilot Eligibility | Done | Read-only eligibility checks the latest 3 preview gate reports, clean run count, checked run ids, blockers, and latest gate report; it creates no AgentRun/Proposal/Action/Observation. |
| W20 Very Small Controlled Chat Migration Pilot With Fallback | Done | Chat exposes explicit `Run Controlled Pilot`; eligibility is checked before preview, blocked does not call preview, eligible preview forces `allowWrites=false`, success is rendered as “Pilot response”, and normal Send remains unchanged. |
| W21 Reviewed Pilot Response Promotion | Done | Successful Controlled Pilot output remains isolated by default, but users can explicitly open review and confirm promotion into one ordinary assistant chat message with existing `run_id` metadata when available. Blocked, failed, canceled, no-output, and repeated promotion paths write nothing. |
| W22 Post-Promotion Validation And Source Binding | Done | Controlled Pilot results bind to the source chat session. Promotion review shows source session, target session, runId, selected strategy, and governance summary; confirmation blocks source/target mismatch without calling `save_chat_message` and prompts the user to rerun the pilot in the current session. |
| W23-W29 Promotion Evidence To Shadow Review | Done | Promotion evidence, promotion readiness, reviewed migration plan, migration review decision evidence, implementation gate, controlled migration shadow run, and shadow review evidence exist as metadata-safe, non-default steps. |
| W30-W33 Cutover Candidate Evidence | Done | Cutover planning readiness, non-default cutover candidate adapter, candidate review evidence, and candidate promotion readiness exist for contract-shape validation and implementation discussion only. |
| W34-W37 Default Chat Activation Boundary | Done | Default Chat runtime boundary status, activation plan draft, activation review evidence, and activation implementation gate exist, all read-only/reviewed and not default Chat migration. |
| W38-W42 Disabled Adapter Readiness Ladder | Done | Disabled routing scaffold, contract harness, dry-run boundary, dry-run review evidence, and implementation readiness gate exist while default Chat remains `legacy_stream`. |
| W43-W48 Controlled Preview To Cutover Plan Approval | Done | Explicit non-default adapter controlled preview, controlled preview review evidence, approval readiness, cutover implementation plan draft, cutover plan review evidence, and cutover plan approval readiness exist without changing default Chat. |
| W49-W57 Default Chat Adapter Guard Stack | Done | Route guard scaffold, cutover invocation harness, invocation plan, invocation boundary, typed callsite contract, ordinary-entry preflight, ordinary-entry preflight status, and narrow implementation discussion gate keep ordinary send/stream fail-closed and observable on `legacy_stream` with controlled executor disabled and unattached. |
| W54 Authority Roadmap Sync | Done | High-priority roadmap and execution documents are synced with W1-W53 code status so future Agents do not follow stale W22 instructions. This is documentation governance, not runtime migration. |
| W55 Default Chat Adapter Ordinary Entry Preflight | Done | Ordinary `send_message` / `start_stream_message` entries now call a pure preflight guard that requires typed contract readiness, legacy entry allowed, controlled executor unattached, migration disabled, and zero pre-entry side-effect budget. |
| W56 Default Chat Adapter Ordinary Entry Preflight Status | Done | Settings can explicitly refresh a read-only status over W55 send/stream preflight readiness, route state, blockers, side-effect lock, and metadata-safe summary. It does not run runtime/model/tool paths, write records, change routing, or migrate default Chat. |
| W57 Default Chat Adapter Narrow Implementation Discussion Gate | Done | Settings can explicitly check a read-only gate over W48 cutover plan approval readiness and W56 ordinary-entry preflight status. Eligible means only that a narrow adapter implementation slice may be discussed; it runs no runtime/model/tool path, writes no records, changes no routing, and is not default Chat migration. |

### W23-W57: Status Bridge

W23-W57 extend the W20-W22 controlled pilot ladder into a long, reviewed
evidence and adapter-guard chain. The important invariant is unchanged:
successful preview, promotion, readiness, approval, dry-run, controlled preview,
cutover plan approval, route guard, invocation guard, and typed callsite
contract, ordinary-entry preflight, ordinary-entry preflight status, and narrow
implementation discussion gate are all evidence, status surfaces, discussion
gates, or guardrails. They do not authorize automatic migration. Default `Send`,
`send_message`, and
`start_stream_message` remain on the legacy stream path until a later separate
implementation is explicitly reviewed and accepted.

### W11: Documentation Status Sync

Current task:

- Synchronize README, AGENTS, plans, and progress status with code.
- Mark MultiStrategy Runtime as preview/audit-ready and not default Chat.
- Keep Tool Taxonomy, proposal-only semantics, metadata-safe audit, and
  AgentRun trace rules visible at entry points.
- Run `make ci` even for Markdown-only changes.

### W12: Non-Default Preview UI / Debug Entry

Status: Done.

- Settings exposes a non-default preview/debug entry that calls
  `run_multi_strategy_agent_preview`.
- Keep it clearly marked as preview/beta.
- Do not add a default user-facing Chat replacement.
- Ensure Runs / Trace remains the source of truth for preview audit.

### W13: Guarded Chat Subpath Migration

Status: Done.

- Chat exposes one explicit guarded preview subpath.
- `send_message` / existing Chat fallback remain preserved.
- The W10 outer AgentRun audit remains the primary trace record.

### W14: Maturation Loop V1

Status: Done for V1 service foundation.

- `MaturationService::mature_runtime_output` converts RuntimeOutput candidates
  into governed evidence/proposals.
- Raw data stays out of accepted LifeModel truth until proposal/user decision.
- Visible product loop and automatic Chat application remain future gated work.

### W15: PlanExecute Governed Vertical Slice

Status: Done for runtime V1 slice.

- `PlanExecuteReport` records metadata-safe plan id, source run id, step
  counts, governance summaries, read-only observations, and warnings.
- Read-only internal steps can execute; write-like steps require proposal and
  are not executed.
- Product weekly planning remains future work.

### W16: RuntimeStrategy Trait

Status: Done.

`openlife-core/src/agent/strategy_runtime.rs` now defines the lightweight
`RuntimeStrategy` trait, ReAct and PlanExecute adapters, and a fixed registry
used by MultiStrategyRuntime. This is an adapter boundary, not plugin loading,
and it does not replace the default Chat path.

Runtime migration evidence surfacing continued in W18, and sustained evidence
qualification is captured in W19 below.

### W17: Runtime Integration Hardening / Chat Migration Gate

Status: Done.

`openlife-core/src/agent/runtime_migration_gate.rs` defines the pure evaluator
and `check_runtime_migration_gate` exposes it as an explicit Tauri diagnostic.
The gate reads existing preview AgentRun audit state only. It does not execute
ReAct, PlanExecute, tools, proposal application, or external writes. Broader
Chat migration remains blocked until the gate has no blocking reason, fallback
is available, traces are metadata-safe, the preview outer AgentRun stays the
primary trace, any inner ReAct run id is child metadata only, no real external
writes occur, proposal-first behavior is preserved, and `make ci` passes.

### W18: Runtime Migration Gate Evidence Surface

Status: Done.

Settings exposes a small read-only Runtime Migration Gate panel near the
MultiStrategy preview/debug entry. The panel explicitly calls
`check_runtime_migration_gate`, displays the pass/block state for every gate
field, and keeps `blockingReasons` visible. It is an evidence surface only: it
does not auto-run preview, does not execute ReAct, PlanExecute, tools, proposal
apply, external writes, or LifeModel/Memory writes, and does not replace
`send_message` / `start_stream_message`. W19 adds the sustained clean evidence
qualification before any pilot can be considered.

### W19: Sustained Gate Evidence / Pilot Eligibility

Status: Done.

`openlife-core/src/agent/runtime_migration_gate.rs` now also defines
`evaluate_controlled_chat_pilot_eligibility`, and Tauri exposes it through
`check_controlled_chat_pilot_eligibility`. The command defaults to the latest 3
MultiStrategy preview AgentRuns, recomputes each gate report, and returns
`eligible`, `requiredCleanRuns`, `cleanRunCount`, `checkedRunIds`,
`blockingReasons`, `lastGateReport`, and `defaultChatUnchanged`.

This is read-only qualification for a controlled Chat migration pilot. It does
not execute ReAct, PlanExecute, tool calls, proposal apply, external writes, or
LifeModel/Memory writes, and it does not create AgentRun, Proposal, Action,
Observation, or audit records. Settings displays the result as "Pilot
eligibility" and explicitly states that it is not a Chat switching control. Even
when eligible, it cannot automatically replace default Chat.

W20 uses this eligibility as the required gate before any Chat-page pilot run.

### W20: Very Small Controlled Chat Migration Pilot With Fallback

Status: Done.

Chat now has a small explicit `Run Controlled Pilot` entry near the existing
governed preview/debug area. It does not intercept the input box and does not
change normal Send. The pilot first calls
`check_controlled_chat_pilot_eligibility`; when `eligible=false`, it displays
blocking reasons and fallback guidance and must not call
`run_multi_strategy_agent_preview`. When `eligible=true`, it runs exactly one
preview turn with `allowWrites=false`, no automatic retry, and no external
write / proposal apply / LifeModel / Memory write path.

The result is rendered as “Pilot response” and is not automatically written as
a normal assistant message. Default `send_message` / `start_stream_message`
remains the stable Chat path. Reviewed pilot response promotion is handled
separately in W21 and is not part of W20.

### W21: Reviewed Pilot Response Promotion

Status: Done.

Chat now offers `Promote Pilot Response` only when the Controlled Pilot
completed successfully and returned `userOutput`. The button opens an explicit
review state showing the pilot response text, runId, selected strategy,
governance summary, payload summary, and a clear warning that confirmation will
write to the current chat history. Canceling review writes nothing.

Confirming promotion uses the existing chat message save path to write exactly
one ordinary assistant message. When the existing message schema supports it,
the promoted message carries the pilot `run_id` trace. Promotion does not write
LifeModel, Memory, Proposal, Action, Observation, external tool output, or
runtime audit records. The same pilot response cannot be promoted twice.

Blocked and failed pilots still show no promotion action. Default `Send`,
`send_message`, and `start_stream_message` still do not call eligibility, the
migration gate, or preview.

### W22: Post-Promotion Validation And Source Binding

Status: Done.

Controlled Pilot output now records the chat session that produced it. The
promotion review panel displays source session, target session, runId, selected
strategy, and governance summary before the user can confirm.

Confirmation validates that the current target session still matches the pilot
source session. If a user switches sessions after running the pilot, promotion
is blocked, no `save_chat_message` call is made, and the UI shows a clear
blocking/fallback message that asks the user to rerun Controlled Pilot in the
current session or switch back to the source session.

This does not change default Chat. Default `Send`, `send_message`, and
`start_stream_message` still do not call eligibility, the migration gate,
preview, or promotion.

## 10. What Not To Do Next

Do not:

- Replace default Chat with MultiStrategy Runtime just because the preview
  command works.
- Treat the Runtime Migration Gate panel as a Chat switching control.
- Treat Pilot eligibility as a Chat switching control or automatic migration
  trigger.
- Treat W20-W57 controlled pilot, promotion, readiness, shadow, candidate,
  activation, dry-run, controlled preview, cutover plan, route guard,
  invocation guard, typed callsite contract, ordinary-entry preflight, or
  ordinary-entry preflight status / narrow implementation discussion gate
  success as default Chat migration.
- Treat pilot success as automatic permission to write the pilot answer into
  ordinary assistant history; W21 promotion requires explicit review and
  confirmation.
- Present `run_multi_strategy_agent_preview` as a production Chat path.
- Treat W10 outer AgentRun audit as permission to skip metadata-safe review;
  ReAct inner run id remains child metadata, not the product trace's primary id.
- Treat the new `RuntimeStrategy` adapter boundary as permission to directly
  replace default Chat without runtime hardening and explicit migration gates.
- Add many new LifeModel fields before the evidence/governor path exists.
- Treat current YAML as the canonical HS database.
- Let extracted signals auto-write identity/values/goals.
- Add new tool manifests that appear executable without real executors.
- Treat `calendar.propose_event` or `email.propose_draft` as real provider write
  executors; they are currently P1 proposal-only governed executors.
- Build a large LifeModel management UI before the backend governance path is
  real.
- Use cloud extraction on raw sensitive LifeModel/memory/file data as a required
  MVP path.

## 11. Product Interpretation

The user-facing product story should become:

```text
OpenLife is a personal Agent OS.
It understands you through a user-governed LifeModel.
It acts through governed runtime strategies.
It learns only through evidence, proposals, and your decisions.
```

The internal architecture story should become:

```text
LifeModel-HS is the protocol layer.
Runtime strategies are execution engines.
AgentRun, Proposal, Evidence, Policy, and Audit are the trust spine.
```

## 12. Exit Criteria For This Program Stage

This program stage is successful when:

- Future Agent tasks start from this document.
- ReAct is documented and implemented as the current default strategy.
- LifeModel-HS is used as a cross-runtime protocol layer, not a feature page.
- One LifeModel Maturation Loop works end-to-end.
- Plan-Execute exists as one real vertical slice.
- RuntimeStrategy abstraction is extracted from proven code, not imagined first.
- High-risk direct writes are closed or explicitly audited.
- Tool Taxonomy and entry-point docs match actual code status.
- `make ci` remains green.
