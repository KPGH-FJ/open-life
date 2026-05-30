# OpenLife LifeModel-Governed Agent Runtime Program

> Date: 2026-05-30
> Status: architecture preparation baseline
> Scope: post-LifeModel-HS MVP convergence, runtime strategy direction, and next implementation order

## 1. Purpose

This document is the preparation baseline for the next OpenLife development
cycle.

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

## 4. Current Code Baseline

As of this preparation document, the project already has meaningful primitives:

| Area | Current implementation | Current maturity |
| --- | --- | --- |
| AgentLoop | `openlife-core/src/agent/agent_loop.rs` | Real ReAct-style loop with parse, action execution, observation, follow-up, budgets, streaming callbacks. |
| AgentRuntime | `openlife-core/src/agent/runtime.rs` | Context assembly plus Direct/Layered reasoning strategy registration. Not yet a multi-strategy runtime abstraction. |
| RuntimeHSPacket | `openlife-core/src/agent/hs_selector.rs` | Deterministic policy/heuristic packet with metadata-safe audit. |
| PolicyStore | `openlife-core/src/agent/policy_store.rs` | Built-in hard policy MVP: sensitive LocalOnly and external write proposal-first. Not persisted/user-governed yet. |
| EvidenceStore | `openlife-core/src/agent/evidence_store.rs` | Persisted candidate evidence layer with source refs and digests. No full LifeEvent/Signal pipeline yet. |
| HeuristicStore | `openlife-core/src/agent/heuristic_store.rs` | Persisted collaboration guidance store with lifecycle and seeded MVP heuristics. |
| RegressionSuite | `openlife-core/src/agent/regression_suite.rs` | Deterministic MVP behavior checks. Not yet a durable user scenario store. |
| ProposalStore | `openlife-core/src/agent/proposal_store.rs` | Unified proposal storage and review states. |
| Proposal apply | `src-tauri/src/commands/proposal.rs` | Main convergence target for LifeModel, memory, tool permission, scheduled task, data export, and external write application. |
| Model routing | `openlife-core/src/agent/model_router.rs`, `openlife-core/src/scheduler.rs` | Role/privacy-aware router plus scheduler integration; HS LocalOnly can fail closed. |
| Compatibility LifeModel | `openlife-core/src/life_model.rs` | YAML/struct compatibility view remains broadly used. It is not the final HS source of truth. |

This means the next phase should not start from blank design. It should
converge existing primitives into a stronger spine.

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
- There is no first-class `RuntimeStrategy` trait yet.
- Plan-Execute should be implemented only after the LifeModel-governed spine and
  one maturation loop are working.

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

Immediate known items:

- `calendar.propose_event` and `email.propose_draft` are governance
  inconsistency items. They must not be treated as completed P1 anywhere until
  taxonomy, code behavior, and tests agree.
- `calendar.propose_event` must create a `ScheduledTask` proposal or be marked
  disabled/declarative-only if no product executor exists.
- `email.propose_draft` must create a `DataExport`/email-draft proposal or be
  marked disabled/declarative-only. It must not be misclassified into
  `ExternalWriteAction` file writes.
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
- No document or taxonomy table labels `calendar.propose_event` or
  `email.propose_draft` as completed P1 until those tests pass.
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

- Do not introduce a broad `RuntimeStrategy` trait until existing Direct,
  Layered, and ReAct paths can be adapted without churn.
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

## 9. First Work Packages For Future Agents

### W0: Read And Confirm Baseline

Read:

- `AGENTS.md`
- this document
- `plans/adr/0013-lifemodel-hs-source-of-truth-governance.md`
- `plans/lifemodel_hs_legacy_write_path_audit.md`
- `plans/openlife_react_beta_roadmap.md`

Run:

```bash
make ci
```

### W1: Tool Proposal Hygiene

Fix:

- `calendar.propose_event` governance classification and proposal semantics.
- `email.propose_draft` governance classification and proposal semantics.
- ExternalWriteAction hard pre-insert size limit.
- ExternalWriteAction hard pre-insert payload minimization.
- Documentation entry points and Tool Taxonomy status sync.

Verify:

- Rust integration tests.
- No docs mark `calendar.propose_event` or `email.propose_draft` as completed
  P1 before the tests and taxonomy agree.
- `make ci`.

### W2: Runtime Spine Contract Draft

Add a narrow internal contract around runtime input/output and HS packet use.
Include a test that prevents full `tools_prompt`/tool-catalog presence from
being interpreted as current-task write or external-side-effect intent.

Do not:

- add Plan-Execute,
- rewrite AgentLoop,
- replace AgentRuntime broadly.

### W3: ReAct Convergence

Make the existing ReAct path consume the thin runtime boundary and HS packet
consistently.

Verify:

- tool-rich read-only prompts do not trigger write/external HS decisions,
- ReAct proposals/observations appear in Run trace,
- `make ci`.

### W4: Direct-Write Guard Tests

Add tests that fail if high-risk product paths silently persist durable
LifeModel changes outside proposal/governor/manual-override paths.

### W5: LifeEventStore MVP

Add a persisted LifeEventStore with redacted summaries and digests.

### W6: SignalExtractor MVP

Start deterministic extraction for the first maturation domain:

```text
state/energy + planning intensity
```

### W7: LifeModelGovernor MVP

Connect candidate evidence to proposal decisions for the first domain.

### W8: Maturation Loop V1

Wire Chat/Feedback/Calibration into the first loop.

### W9: Plan-Execute Slice

Implement one reviewed weekly planning flow after W1-W8.

## 10. What Not To Do Next

Do not:

- Build a complete Multi-Strategy runtime before one Plan-Execute slice exists.
- Add many new LifeModel fields before the evidence/governor path exists.
- Treat current YAML as the canonical HS database.
- Let extracted signals auto-write identity/values/goals.
- Add new tool manifests that appear executable without real executors.
- Mark `calendar.propose_event` or `email.propose_draft` as completed P1 before
  governance semantics, tests, and taxonomy are synchronized.
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
