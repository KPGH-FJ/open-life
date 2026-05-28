# OpenLife LifeModel-HS MVP Task Specifications

Date: 2026-05-28

Status: next

Package:

```text
Post-Beta LifeModel-HS MVP
```

This document converts the LifeModel-HS architecture plan and ADR 0013 into
coding-ready task specifications. It is a preparation artifact for the next
development session. It should guide implementation, review, and acceptance,
but it does not itself start implementation.

LifeModel-HS is the first step toward a softwareized personal model system:
accepted evidence, policies, heuristics, state, regression checks, audit, and
materialized views. The MVP must prove that this system can improve one real
Agent run without rewriting the current Agent framework or pretending that a
single YAML profile is already a dynamic model.

Reference documents:

- `plans/lifemodel_hs_architecture_plan.md`
- `plans/adr/0013-lifemodel-hs-source-of-truth-governance.md`
- `plans/openlife_agent_framework_architecture.md`
- `plans/openlife_stabilization_and_spine_consolidation_plan.md`
- `docs/decisions/0001-lifemodel-patch.md`
- `docs/decisions/0002-proposal-unified.md`
- `docs/decisions/0003-agent-run-tracking.md`

## Baseline Review

Before LifeModel-HS MVP implementation:

- P0-P12 Agent Framework primitives are already in place.
- `AgentRun` trace, `ContextAssembler`, `ProposalStore`, chat/memory proposal
  generators, `ModelRouter`, `ActionExecutor`, and governed runtime paths
  exist.
- Current `LifeModel` YAML remains the compatibility runtime view.
- Memory currently supports chat/session/state/custom memory plus vector
  retrieval, but it is not yet a complete evidence layer.
- Existing LifeModel evolution is proposal-first in the important paths, but
  legacy direct-write risks still need auditing during convergence.
- ADR 0013 is accepted and is the hard governance baseline for this package.

## Global Rules

- Execute exactly one LifeModel-HS task spec at a time.
- Keep the MVP additive. Do not replace current YAML source paths in one step.
- Do not rewrite `ChatPage`, `AgentRuntime`, `ContextAssembler`, or
  `LifeModelManager` broadly.
- Do not create autonomous identity, value, mission, long-term goal, sensitive
  relationship, or privacy-boundary updates.
- Do not use cloud extraction over raw LifeModel, raw memory, raw files, or
  raw sensitive chat as an MVP requirement.
- Raw Life Data must produce weak signals or evidence candidates, not accepted
  truth.
- Privacy is hard `Policy`. Heuristics may make policy stricter but cannot
  relax it.
- Risky HS mutation remains Proposal-first. High-risk mutation requires
  explicit user confirmation.
- Low-risk automatic updates are limited to transient state with TTL and
  low-risk maintenance metadata.
- Runtime audit must be metadata-safe. Do not store raw sensitive payloads in
  selection audit or regression result records.
- Start regression with deterministic selector, route, prompt, and tool-policy
  checks. Do not require LLM-judged regression for MVP acceptance.
- Preserve existing public command contracts unless the task explicitly says
  otherwise.
- Add focused tests for each store, selector, policy, and regression behavior.
- `make ci` remains the final gate for the full package.

## MVP Product Claim

The MVP is accepted only if OpenLife can demonstrate these four user-visible
truths:

1. OpenLife can explain which personal collaboration rules or policies affected
   a run.
2. Sensitive topics are governed by hard policy before prompts or model routing
   can leak them.
3. Rejected behavior becomes negative evidence that changes future behavior in
   a narrow, testable way.
4. The current YAML LifeModel remains usable as a compatibility view without
   becoming a dumping ground for every evidence item and heuristic.

## LMHS-0: Phase Sync And Spec Discoverability

Goal:

Make the LifeModel-HS MVP discoverable as the next post-Beta design/coding
package without implying that implementation is already complete.

Expected behavior:

- Standard planning entry points link to this task spec and ADR 0013.
- Documentation states that LifeModel-HS starts as an additive MVP.
- Documentation states that current YAML remains a compatibility materialized
  view during migration.
- Non-goals are visible: no full source-of-truth switch, no autonomous identity
  rewrite, no broad compression engine, no LLM-judge regression requirement.

Allowed edit areas:

- `AGENTS.md`
- `README.md`
- `plans/openlife_development_plan.md`
- `plans/openlife_remaining_tasks_plan.md`
- `plans/openlife_react_beta_roadmap.md`
- `plans/lifemodel_hs_mvp_task_specs.md`
- `plans/adr/0013-lifemodel-hs-source-of-truth-governance.md`

Constraints:

- Do not edit runtime code in this task.
- Do not claim MVP behavior exists before the relevant implementation task has
  landed.

Verification:

- `rg -n "LifeModel-HS|lifemodel_hs_mvp_task_specs|ADR 0013|compatibility materialized view" AGENTS.md README.md plans`
- `git diff --name-only` contains documentation files only for this task.

## LMHS-1: EvidenceStore MVP Skeleton

Goal:

Introduce a persisted EvidenceStore as a curated evidence layer separate from
raw MemoryStore, VectorStore, and current chat/memory proposal extraction
helpers.

Expected behavior:

- Evidence is stored as a first-class local asset with stable ids, source refs,
  affected path, evidence type, confidence, risk, privacy level, status,
  recency, support count, opposing refs, and optional tombstone metadata.
- Evidence can be created, queried, weakened, archived, contradicted, and linked
  to proposals, AgentRun records, or run metadata.
- Evidence records store source references and digests, not raw sensitive
  payloads by default.
- Existing chat/memory proposal extraction output can be mapped into
  EvidenceStore candidates without changing runtime behavior.
- No evidence item becomes an accepted LifeModel fact merely because it was
  extracted.

Allowed edit areas:

- `openlife-core/src/agent/proposal_generators/chat.rs`
- `openlife-core/src/agent/proposal_engine.rs`
- `openlife-core/src/agent/memory_service.rs`
- new `openlife-core/src/agent/evidence_store.rs`
- `openlife-core/src/agent/mod.rs`
- `openlife-core/src/json_utils.rs` if schema helpers are needed
- focused Rust tests under `openlife-core/src/agent/tests/`

Constraints:

- Do not migrate all MemoryStore data.
- Do not change existing Proposal apply semantics.
- Do not add frontend UI in this task.
- Do not write raw chat/file content into evidence audit fields.

Verification:

- Unit tests cover create/query/weaken/archive/contradict/tombstone basics.
- Unit tests prove raw payload is not required in stored evidence records.
- Existing chat proposal and memory proposal tests still pass.
- `cargo test -p openlife-core evidence`

## LMHS-2: HeuristicStore MVP Skeleton

Goal:

Introduce a persisted HeuristicStore for executable personal collaboration
guidance without yet wiring it broadly into runtime behavior.

Expected behavior:

- Heuristics have stable ids, domain, trigger, conditions, guidance, priority,
  risk, privacy level, lifecycle status, evidence refs, opposing evidence refs,
  validation state, source proposal id, version, and usage metadata.
- Lifecycle statuses include at least `candidate`, `trial`, `active`,
  `weakened`, `archived`, and `rejected`.
- Domain caps are represented or diagnosable: default 5 active heuristics per
  domain and 8 active plus trial heuristics per domain.
- Store APIs support create, update lifecycle, query by domain/status/task
  metadata, record usage, and fetch lineage.
- Candidate and high-risk heuristics cannot become active without an explicit
  accepted proposal or seeded built-in policy decision.

Allowed edit areas:

- new `openlife-core/src/agent/heuristic_store.rs`
- `openlife-core/src/agent/mod.rs`
- focused Rust tests under `openlife-core/src/agent/tests/`

Constraints:

- Do not build a complex heuristic editor.
- Do not let HeuristicStore override privacy policy.
- Do not inject all heuristics into prompt/context assembly.
- Do not promote extracted signals directly into active heuristics.

Verification:

- Unit tests cover lifecycle transitions and invalid transitions.
- Unit tests cover domain cap warning or diagnostic behavior.
- Unit tests prove high-risk active promotion is blocked without accepted
  governance metadata.
- `cargo test -p openlife-core heuristic`

## LMHS-3: Policy/Heuristic Boundary And Built-In MVP Assets

Goal:

Define the hard boundary between policies and heuristics, then seed the minimum
MVP assets needed for runtime proof.

Expected behavior:

- Privacy-sensitive topics are represented as hard policy, not soft heuristic.
- Tool write behavior is represented as governed tool policy or high-priority
  runtime rule that cannot be bypassed by heuristic text.
- Low-energy planning is represented as a soft planning heuristic.
- Rejected reminders reducing proactive frequency is represented as a soft
  proactive heuristic or negative-evidence rule.
- Heuristics cannot relax policy. If a selected heuristic conflicts with a
  policy, policy wins and the conflict is audited.

MVP built-in assets:

- Policy: sensitive health, relationship, identity, finance, and private-file
  topics default to LocalOnly unless the user explicitly overrides.
- Policy or governed rule: external write actions require draft/proposal-first
  execution unless already confirmed by the user.
- Heuristic: when current energy is low, planning should reduce intensity,
  step count, and pressure.
- Heuristic: when proactive reminders are rejected, similar reminders should be
  weakened or delayed.

Allowed edit areas:

- new `openlife-core/src/agent/policy_store.rs` if needed
- `openlife-core/src/privacy.rs`
- `openlife-core/src/agent/heuristic_store.rs`
- `openlife-core/src/agent/action_executor/`
- `openlife-core/src/tool_permissions.rs`
- `openlife-core/src/tool_manifest.rs`
- focused Rust tests

Constraints:

- Do not reduce existing privacy protections.
- Do not rely on natural-language prompt instructions as the only enforcement
  point for privacy or external writes.
- Do not expose seeded policy assets as user-editable unless a review path
  already exists.

Verification:

- Tests prove a privacy policy cannot be relaxed by a heuristic.
- Tests prove external writes remain draft/proposal-first under the MVP rule.
- Tests prove low-energy planning remains heuristic guidance, not hard policy.
- `cargo test -p openlife-core policy`

## LMHS-4: ContextSelector And HeuristicSelector MVP

Goal:

Add deterministic selector gates that choose only relevant HS assets for a task
instead of injecting the whole LifeModel or all heuristics.

Expected behavior:

- Selector input includes task kind, intent summary, privacy classification,
  risk level, tool requirements, current state hints, token budget, and
  optional AgentTask or AgentRun context.
- ContextSelector selects relevant state summaries, policy refs, evidence
  summaries, and compatibility LifeModel fields.
- HeuristicSelector selects active/trial heuristics by domain, trigger,
  conditions, privacy, risk, and priority.
- Policy assets are selected through hard filters before soft heuristic scoring.
- Selector output is a `RuntimeHSPacket` or equivalent metadata structure with
  included assets, excluded assets, reasons, source ids, digests, and token
  estimates.
- Selection audit is metadata-safe and can be attached to AgentRun or run detail
  metadata without raw sensitive payloads.

Allowed edit areas:

- new `openlife-core/src/agent/hs_selector.rs`
- `openlife-core/src/agent/context_assembler.rs`
- `openlife-core/src/agent/runtime.rs`
- `openlife-core/src/agent/types.rs`
- `openlife-core/src/agent/store.rs`
- focused Rust tests

Constraints:

- Do not replace ContextAssembler wholesale.
- Do not add semantic retrieval over heuristics in MVP unless it is already
  locally available and deterministic enough for tests.
- Do not select inactive, rejected, archived, or policy-conflicting heuristics.
- Do not put raw evidence text into the runtime packet.

Verification:

- Tests cover task-level selection for the three MVP policies/heuristics.
- Tests cover exclusion reasons for rejected, archived, over-budget, or
  policy-conflicting assets.
- Tests cover metadata-safe audit serialization.
- `cargo test -p openlife-core selector`

## LMHS-5: Deterministic RegressionSuite MVP

Goal:

Add user-level behavior regression checks for LifeModel-HS assets without
depending on flaky model-judged evaluation.

Expected behavior:

- Regression scenarios can assert selector, route, prompt metadata, and
  tool-policy requirements.
- MVP scenarios cover:
  - sensitive topic must select LocalOnly policy,
  - selected heuristic must not relax LocalOnly policy,
  - external write must require draft/proposal-first,
  - low-energy planning must select low-intensity guidance,
  - rejected reminders must weaken similar proactive suggestions.
- Regression results record scenario id, asset ids, pass/fail, concise reason,
  and metadata-safe run details.
- Regression can be run before promoting a candidate heuristic or policy and in
  normal test suites.

Allowed edit areas:

- new `openlife-core/src/agent/regression_suite.rs`
- `openlife-core/src/agent/hs_selector.rs`
- `openlife-core/src/agent/heuristic_store.rs`
- `openlife-core/src/agent/policy_store.rs`
- focused Rust tests

Constraints:

- Do not require LLM judge results.
- Do not store raw user prompts as regression fixtures unless explicitly
  sanitized.
- Do not block all low-risk metadata updates on full regression.

Verification:

- Unit tests prove each MVP regression passes under intended assets.
- Unit tests prove a candidate that violates LocalOnly fails regression.
- Unit tests prove regression result serialization is metadata-safe.
- `cargo test -p openlife-core regression`

## LMHS-6: Runtime Integration MVP

Goal:

Wire selected HS assets into the existing Agent runtime only for the narrow MVP
behaviors, proving usefulness without broad architecture churn.

Expected behavior:

- For privacy-sensitive topics, the selected hard policy affects ModelRouter so
  the run uses LocalOnly or fails closed when LocalOnly is unavailable.
- For external write actions, the selected policy/rule affects ActionExecutor or
  tool-permission checks so the action becomes draft/proposal-first unless
  confirmed.
- For low-energy planning, selected heuristic guidance affects ContextAssembler,
  AgentRuntime prompt construction, or plan generation so generated plans are
  smaller and lower pressure.
- Runtime output records which HS assets affected the run through metadata-safe
  selection audit.
- Existing non-HS flows continue to work if no relevant HS assets are selected.

Allowed edit areas:

- `openlife-core/src/agent/context_assembler.rs`
- `openlife-core/src/agent/runtime.rs`
- `openlife-core/src/agent/agent_loop.rs`
- `openlife-core/src/agent/model_router.rs`
- `openlife-core/src/scheduler.rs`
- `openlife-core/src/agent/action_executor/`
- `openlife-core/src/tool_permissions.rs`
- `openlife-core/src/agent/types.rs`
- focused tests

Constraints:

- Do not create a second runtime path.
- Do not route around `AgentRuntime`, `ContextAssembler`, `ModelRouter`,
  `ActionExecutor`, or tool-permission governance.
- Do not add broad prompt injection of all HS assets.
- Do not make cloud calls for sensitive raw HS extraction.

Verification:

- Test: sensitive topic selects LocalOnly policy and does not assemble a cloud
  disallowed prompt block.
- Test: heuristic cannot relax LocalOnly policy.
- Test: external write action is converted into draft/proposal-first behavior.
- Test: low-energy planning produces constrained planning metadata or prompt
  guidance.
- Existing Agent runtime tests still pass.
- `cargo test -p openlife-core agent`

## LMHS-7: Negative Evidence Loop MVP

Goal:

Make one narrow rejected-behavior loop real: rejected reminders reduce similar
future proactive reminders.

Expected behavior:

- A rejected proactive reminder or reminder proposal creates negative evidence
  linked to the source proposal/run/action.
- Similar future reminder candidates query negative evidence before surfacing.
- Negative evidence weakens or delays similar reminders without globally
  disabling proactive help.
- The behavior is auditable and reversible through evidence weakening/archive
  semantics.

Allowed edit areas:

- `openlife-core/src/agent/evidence_store.rs`
- `openlife-core/src/proactive.rs`
- `openlife-core/src/agent/proposal_store.rs`
- `openlife-core/src/feedback.rs`
- focused Rust tests

Constraints:

- Do not build a broad recommender system.
- Do not silently disable all reminders.
- Do not treat one rejection as a permanent stable preference.
- Do not write raw reminder text into evidence if a digest and summary are
  sufficient.

Verification:

- Test: rejected reminder creates negative evidence with source refs.
- Test: similar reminder is weakened or delayed.
- Test: unrelated reminder is not suppressed.
- Test: user archive/forget semantics can remove or weaken the negative
  evidence effect.
- `cargo test -p openlife-core proactive`

## LMHS-8: Materialized YAML Compatibility View MVP

Goal:

Keep current YAML usable while preventing it from becoming the canonical HS
database or a raw evidence dump.

Expected behavior:

- Materialized compatibility output may include current state summary, existing
  LifeModel fields, concise collaboration summaries, HS asset refs, and digest
  metadata.
- Materialized YAML must not include full heuristic lists, raw evidence,
  opposing evidence, raw source text, regression internals,
  privacy-sensitive reasoning, or full audit history.
- Materialization records source asset ids and content digests so it can be
  rebuilt and audited.
- Existing YAML consumers continue to work.

Allowed edit areas:

- `openlife-core/src/life_model.rs`
- `openlife-core/src/versioning.rs`
- `openlife-core/src/agent/evidence_store.rs`
- `openlife-core/src/agent/heuristic_store.rs`
- focused Rust tests

Constraints:

- Do not switch canonical source of truth in this task.
- Do not remove existing YAML fields required by current runtime or UI.
- Do not serialize raw HS internals into YAML for convenience.

Verification:

- Tests prove allowed compatibility sections serialize.
- Tests prove disallowed raw evidence and full heuristic internals are omitted.
- Tests prove source refs and digests are present for materialized summaries.
- Existing LifeModel serialization tests still pass.
- `cargo test -p openlife-core life_model`

## LMHS-9: Minimal Review And Trace Surface

Goal:

Expose enough LifeModel-HS runtime behavior for users and developers to inspect
what happened, without building a full HS management UI.

Expected behavior:

- Run detail or trace metadata can show which policies/heuristics were selected
  for a run.
- Proposal/review surfaces can show concise evidence summaries and why a
  candidate exists.
- Regression results can be summarized as behavior checks, not internal
  jargon.
- User-facing labels prefer "collaboration rule", "AI collaboration style",
  "why OpenLife thinks this", and "behavior check" over raw "heuristic" terms.

Allowed edit areas:

- `frontend/src/components/RunTracePanel.tsx`
- `frontend/src/components/ReasoningTracePanel.tsx`
- `frontend/src/pages/AgentRunDetail.tsx`
- `frontend/src/pages/ProposalReviewPage.tsx`
- `frontend/src/types.ts`
- `frontend/src/tauri.ts`
- relevant Tauri commands only if a read-only command is needed
- frontend tests/mocks

Constraints:

- Do not build a full heuristic editor.
- Do not expose raw sensitive evidence.
- Do not add mutation controls unless backed by Proposal/Governor paths.
- Do not clutter existing trace UI with large raw JSON blobs.

Verification:

- Frontend tests cover rendering selected policy/heuristic summaries.
- Frontend tests cover empty state when no HS assets affect a run.
- Mocked data includes no raw sensitive payload.
- `pnpm --dir frontend typecheck`
- `pnpm --dir frontend test`

## LMHS-10: Legacy Evolution Path Audit

Goal:

Audit existing LifeModel, Memory, Feedback, Builder, Calibration, and Proposal
paths so the HS MVP does not coexist with hidden direct-write behavior.

Expected behavior:

- Produce a concise audit section or document listing current write paths into
  LifeModel, memory, state history, proposals, snapshots, and evolution signals.
- Classify each path as:
  - already proposal-first,
  - low-risk transient state,
  - read-only/materialized,
  - legacy direct write requiring future convergence,
  - disabled/declarative-only.
- Any new HS write path introduced by LMHS-1 through LMHS-9 is listed.
- No hidden high-risk direct-write path is introduced by the MVP.
- Completion artifact: `plans/lifemodel_hs_legacy_write_path_audit.md`.

Allowed edit areas:

- `plans/lifemodel_hs_mvp_task_specs.md`
- optional new audit document under `plans/`
- minimal code comments only where they clarify existing write-path ownership

Constraints:

- Do not refactor audited paths in this task unless a newly introduced
  high-risk bug must be fixed immediately.
- Do not claim all legacy paths are converged unless verified.

Verification:

- `rg -n "save_life_model|update_life_model|apply.*proposal|save_memory|save_state|evolution|calibration|builder" openlife-core src-tauri`
- Audit document includes path, risk class, current guard, and future action.
- `git diff --name-only` contains only docs or minimal comments for this task.

## Package Exit Criteria

LifeModel-HS MVP is complete when:

- EvidenceStore and HeuristicStore exist with focused tests.
- Privacy-sensitive LocalOnly policy is enforced as hard policy.
- External write actions remain draft/proposal-first.
- Low-energy planning changes runtime guidance in a bounded, testable way.
- Rejected reminder behavior creates negative evidence and affects similar
  future reminders narrowly.
- ContextSelector and HeuristicSelector select assets by task, privacy, risk,
  lifecycle, and budget without injecting the whole HS.
- Runtime selection audit is metadata-safe and inspectable.
- RegressionSuite has deterministic checks for the MVP behaviors.
- YAML remains a compatibility materialized view and does not contain raw HS
  internals.
- No high-risk identity, value, mission, long-term goal, sensitive relationship,
  or privacy-boundary update can auto-apply.
- Legacy write-path audit exists and does not reveal newly introduced hidden
  high-risk direct writes.
- `make ci` passes.

## Suggested Implementation Order

1. LMHS-0: make the package discoverable.
2. LMHS-1: add EvidenceStore skeleton.
3. LMHS-2: add HeuristicStore skeleton.
4. LMHS-3: encode policy/heuristic boundary and built-in MVP assets.
5. LMHS-4: add selectors and metadata-safe runtime packet.
6. LMHS-5: add deterministic regression checks before broad runtime wiring.
7. LMHS-6: wire the three runtime MVP behaviors.
8. LMHS-7: implement the rejected-reminder negative evidence loop.
9. LMHS-8: add compatibility materialized-view guardrails.
10. LMHS-9: add minimal review/trace visibility.
11. LMHS-10: complete legacy path audit and update convergence backlog.

## Coding Prompt Template

Use this template when assigning an individual task to an Agent:

```text
You are working in /Users/fujing/Desktop/偶来福.

Implement exactly one task from plans/lifemodel_hs_mvp_task_specs.md:
<TASK_ID>: <TASK_TITLE>

Follow ADR 0013 as the hard governance baseline. Keep the change additive,
proposal-first, privacy-governed, and metadata-safe. Do not implement adjacent
LifeModel-HS tasks unless the selected task explicitly requires it. Do not
rewrite ChatPage, AgentRuntime, ContextAssembler, or LifeModelManager broadly.

Before editing, inspect the relevant existing modules and tests. After editing,
run the task-specific verification commands from the spec. If a verification
command is too broad or blocked, explain why and run the closest focused
alternative. End with changed files, verification results, and any remaining
risks.
```
