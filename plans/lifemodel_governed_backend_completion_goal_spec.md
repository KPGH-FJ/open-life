# LifeModel-Governed Backend Completion Goal Spec

> Date: 2026-06-05
> Status: W146 Backend Completion Goal 7 complete; next Goal-mode entry is Goal 8
> Baseline: W146 Backend Golden Paths complete; default Chat remains `legacy_stream`
> Scope: backend/kernel work required before large-scale product UI/UX

## 1. Purpose

This document defines the next large OpenLife development stage before
large-scale product UI/UX work.

The goal is not to add another isolated feature. The goal is to complete the
backend kernel that makes OpenLife a LifeModel-governed Personal Agent OS:

```text
User behavior / task outcome
  -> LifeEvent
  -> Signal
  -> Evidence
  -> Maturation / Governor
  -> Proposal / accepted asset
  -> Materialized LifeModel view
  -> RuntimeHSPacket guidance
  -> ReAct / Plan-Execute / tools / model route behavior
  -> Trace / outcome evidence
```

When this stage is complete, OpenLife should be ready for serious UI/UX
productization because the backend contracts for Learning Inbox, Evidence
Timeline, Proposal Review, Runtime Trace, Guidance Impact, Privacy Controls,
and LifeModel Overview will be stable enough to design against.

## 2. Strategic Completion Definition

This stage is complete only when all of the following are true:

- LifeModel can continuously learn from real use through governed
  `LifeEvent -> Signal -> Evidence -> Proposal -> accepted guidance` loops.
- LifeModel does not autonomously rewrite durable personal truth. High-risk
  identity, values, relationships, health, finance, privacy, and long-term
  direction changes remain explicit proposal-first or manual-confirmation-only.
- Runtime behavior is materially affected by accepted LifeModel-HS guidance,
  not merely by appending the LifeModel YAML into a prompt.
- ReAct and Plan-Execute both consume the same LifeModel-HS protocol layer.
- Model routing, tool execution, external writes, and memory/LifeModel updates
  are governed by the same policy/guidance boundary.
- Trace/read models can explain what evidence or guidance affected a run
  without exposing raw sensitive payloads.
- The backend has at least three product-grade golden paths that UI can later
  expose directly.

The intended completion statement is:

```text
OpenLife has completed the pre-UI LifeModel-Governed Backend Kernel.
LifeModel has a governed self-evolution loop.
Runtime is materially constrained and guided by LifeModel-HS.
Backend read models are stable enough for product UI/UX.
```

This is not a final claim that LifeModel is finished forever. It is the
highest-quality backend baseline that can reasonably be built with the current
project primitives before productization.

## 3. Non-Goals

Do not include these in this backend completion stage unless a later reviewed
spec explicitly changes the boundary:

- Do not migrate ordinary `send_message` / `start_stream_message` away from
  `legacy_stream`.
- Do not treat W19-W123 readiness, status, proof, review, or trace reports as
  default Chat migration permission.
- Do not build the large final UI/UX redesign.
- Do not auto-promote high-risk LifeModel claims into durable truth.
- Do not send raw sensitive LifeModel, memory, tool payload, prompt, or
  user-output content into metadata-safe traces or reports.
- Do not introduce hidden direct-write paths around ProposalStore, PatchStore,
  Governor, or materializer caller restrictions.
- Do not present declarative-only tools, plugin tools, calendar/email proposal
  helpers, or unsupported providers as real executable side-effect capability.
- Do not broaden to identity/values/relationships/health/finance automatic
  maturation before low-risk preference/state/work-style domains are proven.

## 4. Hard Invariants

These invariants are mandatory in every slice:

- Default Chat isolation remains intact unless a separate route migration Goal
  is approved.
- `legacy_stream` remains the ordinary Chat path.
- Durable LifeModel-HS truth changes are proposal-first, governed manual
  override, accepted proposal apply, governed restore/import, or other
  explicitly classified materializer caller contexts only.
- State/Daily Goal compatibility writes remain source-data compatibility, not
  accepted durable HS truth.
- External writes require proposal-first behavior, payload minimization, size
  limits, safe path validation when applicable, and replay scope validation.
- HS guidance can constrain policy or behavior, but must not relax privacy,
  model route, tool permission, or proposal-first requirements.
- All debug/status/readiness surfaces must identify whether they are
  non-default, read-only, proof-only, or product runtime.
- Every new command must be mirrored across Rust command registration,
  TypeScript wrapper, frontend mock, and tests.
- Every new status/proof/readiness result must avoid naming that implies
  migration permission or runtime authority if it is not one.
- `AGENTS.md`, `plans/README.md`, and the progress index must be synchronized
  whenever task order, authority, or completed W-slice state changes.

## 5. Current Baseline

The project already has meaningful primitives. This stage must converge them
rather than rebuild from scratch.

| Area | Current state | Required next maturity |
| --- | --- | --- |
| LifeModel compatibility model | `openlife-core/src/life_model.rs` has structured Identity/Goals/Capabilities/State/Relationships/Preferences plus YAML compatibility materialization | Treat YAML as materialized view; durable HS source must be evidence/proposal/guidance governed |
| Patch / PatchStore | LifeModel patches and source-specific PatchSource mapping exist | Ensure every materialized change has source/evidence/proposal provenance |
| ProposalStore | Unified proposals and review states exist | Become the central product learning/control queue, not only patch application |
| EvidenceStore | Persisted evidence records with source refs, status, tombstone, weaken/archive/contradict | Mature into Evidence Graph v1 with support/opposition, dedupe, conflict, decay, cooldown, source weighting |
| HeuristicStore | Persisted heuristics with lifecycle and seeded MVP heuristics | Add accepted guidance lifecycle and runtime selection from user-governed outcomes |
| PolicyStore / Governor | Built-in hard policies and maturation/tool/model governance exist | Unify risk, privacy, model route, external write, and LifeModel maturation decisions |
| RuntimeHSPacket | Metadata-safe selected policy/heuristic packet exists | Become mandatory guidance contract for ReAct and Plan-Execute product runtime paths |
| AgentLoop / ReAct | ReAct Beta execution hardening W114-W123 complete | Consume HS guidance materially and emit outcome candidates/trace linkage |
| Plan-Execute | W98-W105 weekly planning vertical exists | Consume HS guidance and produce outcome evidence for future behavior |
| ModelRouter | Has `route_with_hs_packet` LocalOnly enforcement plus W141 hard filtering so High/Critical privacy and HS LocalOnly cannot select cloud providers or cloud fallback | Prove privacy/model route behavior in backend golden paths |
| ActionExecutor | Manifest authority, proposal-first write hardening, and W142 HS tool governance block unsupported Plugin/A2A sources before permission replay/execution | Prove tool/write governance in backend golden paths |
| Read models | Runs/Trace, Proposal Review, Settings proof/status surfaces exist | Freeze UI-facing backend read models for Learning Inbox, Evidence Timeline, Guidance Impact, and Privacy Controls |

## 6. Target Backend Concepts

### 6.1 LifeEvent

A normalized, immutable event derived from a user action, runtime observation,
proposal outcome, task result, manual correction, or tool/provider result.

Minimum fields:

- `id`
- `event_type`
- `source_type`
- `source_id`
- `occurred_at`
- `domain`
- `risk_level`
- `privacy_level`
- `summary`
- `payload_digest`
- `metadata`
- `contains_raw_content=false` for metadata-safe reports

LifeEvents are not durable LifeModel truth.

### 6.2 Signal

A weak candidate interpretation extracted from one or more LifeEvents.

Minimum fields:

- `id`
- `signal_type`
- `domain`
- `claim_summary`
- `polarity`: supporting / opposing / corrective / uncertain
- `confidence`
- `risk_level`
- `privacy_level`
- `source_event_ids`
- `extractor_id`
- `extractor_version`
- `uncertainty_reasons`
- `dedupe_key`
- `metadata`

Signals are weaker than Evidence and cannot directly update LifeModel truth.

### 6.3 Evidence

A supported claim with provenance and lifecycle.

Evidence must support:

- source signal/event/proposal/run/tool refs
- support/opposition polarity
- confidence
- recency
- source weight
- conflict links
- decay metadata
- tombstone/archive/weaken/contradict
- linked proposal ids
- linked AgentRun ids

Evidence is still not automatically accepted LifeModel truth.

### 6.4 Maturation Candidate

A proposed interpretation, rule, preference, or state update generated from
Evidence.

Examples:

- User prefers small next steps when energy is low.
- User often wants structure before execution.
- User rejects high-pressure weekly plans.
- User prefers morning deep-work scheduling.
- User corrected a repeated preference inference.

Candidates must carry support evidence, opposing evidence, risk, domain,
confidence, stability, and proposal requirements.

### 6.5 Accepted Guidance / Heuristic

An accepted or trial runtime guidance asset. It may affect future runtime
behavior through RuntimeHSPacket selection.

It must have:

- source proposal id
- source evidence ids
- lifecycle status
- domain
- trigger
- guidance text or typed guidance payload
- priority
- privacy/model/tool constraints
- usage metadata
- rollback/deactivation path

### 6.6 Runtime Guidance

The selected subset of policies and accepted guidance that an explicit
non-default/runtime execution may honor when runtime guidance consumption mode
is enabled. Ordinary Chat keeps guidance consumption disabled.

Runtime guidance must be visible in traces by id/hash/type/count/summary, not
by raw sensitive content.

## 7. Required Golden Paths

Before UI/UX, the backend must prove at least these paths end to end.

### 7.1 Weekly Planning Guidance Loop

```text
weekly planning intent
  -> RuntimeHSPacket selected guidance
  -> Plan-Execute draft/finalize/step execution
  -> proposal-first write-like steps
  -> outcome evidence
  -> future planning guidance
```

Acceptance:

- Plan-Execute behavior changes only when accepted guidance is present and
  explicit runtime guidance consumption mode is enabled.
- Write-like steps still create proposals only.
- Trace shows selected guidance metadata.
- Outcome evidence links back to plan session/run/proposals.

### 7.2 Low-Energy Support Loop

```text
low-energy user signal
  -> LifeEvent / Signal / Evidence
  -> low-pressure planning candidate
  -> proposal accept/edit/reject
  -> accepted/trial guidance
  -> future ReAct or Plan-Execute response changes
```

Acceptance:

- Accepted guidance makes suggestions smaller/gentler.
- Rejection creates negative evidence and cooldown.
- Edit creates corrected evidence.
- Trace shows guidance impact without raw sensitive text.

### 7.3 Preference Correction Loop

```text
Agent makes a wrong inference
  -> user rejects or edits
  -> negative/corrective signal
  -> evidence opposition/conflict
  -> repeated candidate suppressed or corrected
  -> future runtime behavior changes
```

Acceptance:

- Rejected similar candidates are not repeatedly proposed.
- Corrected preference can replace weaker earlier evidence.
- Evidence conflict is visible to backend read models.

## 8. Backend Read Models Required Before UI/UX

This stage must freeze read models for:

- `LifeModelOverviewReadModel`
  - compatibility LifeModel summary, selected accepted guidance counts,
    high-risk pending proposals count, last materialized view digest.
- `LearningInboxReadModel`
  - pending maturation candidates, proposal status, risk, domain, support and
    opposition counts, suggested action, cooldown state.
- `EvidenceTimelineReadModel`
  - events/signals/evidence grouped by domain/time, support/opposition,
    linked run/proposal/tool ids, metadata-safe summaries.
- `GuidanceImpactReadModel`
  - which accepted guidance affected which runs, with id/hash/type/count,
    behavior check results, and outcome links.
- `RuntimeTraceReadModel`
  - ReAct/Plan-Execute strategy trace, selected HS policies/heuristics, model
    route, tool policy, proposal/write boundaries.
- `PrivacyPolicyReadModel`
  - local-only policy, cloud/redaction status, sensitive domain blockers,
    external write policy, tool permission scope.
- `LifeModelVersionDiffReadModel`
  - materialized view version, source proposals/evidence/patches, diff summary,
    rollback/snapshot refs.

These read models can be backend structs and tests first. Large UI is explicitly
out of scope for this stage.

## 9. W-Slice Plan

The exact W numbers can be adjusted by the developer, but the order should not
be casually changed. Each W-slice must be independently testable.

### Goal 1: Master Contract And Schemas

- **W124: Backend completion contract/readiness report**
  - Add a pure backend readiness report that checks current prerequisites for
    LifeModel-Governed Backend Completion.
  - No runtime/model/tool execution.
  - No business writes.
  - Output gaps against this spec.
- **W125: LifeEvent schema and store contract**
  - Add typed LifeEvent structures and a local store or store skeleton with
    metadata-safe create/query APIs.
  - Add tests for privacy metadata, digest, dedupe, and raw-content blocking.
- **W126: Signal schema and extractor contract**
  - Add typed Signal structures and deterministic extractor boundary.
  - Extract from existing low-risk sources only.
  - No LLM extractor required in first slice.
- **W127: LifeEvent/Signal/Evidence bridge**
  - Convert accepted safe signals into EvidenceStore records with lineage.
  - Low confidence, sensitive/high-risk, and raw-content signals fail closed.

### Goal 2: Evidence Graph v1 (complete)

- **W128: Evidence support/opposition/dedupe graph**
  - Complete: added support/opposition links, dedupe clusters, source weights,
    and cluster summaries.
- **W129: Conflict, decay, and cooldown**
  - Complete: added conflict detection, injected-now decay metadata, and
    rejected-similar cooldown support.
- **W130: Evidence read model**
  - Complete: added metadata-safe Evidence Timeline backend read model.

### Goal 3: Maturation Engine v1 (complete)

- **W131: Low-risk multi-domain candidate generation**
  - Complete: expanded beyond the W73-W78 low-energy proof into planning
    preference, energy pattern, work style, and communication preference using
    Evidence Graph clusters.
  - Complete: high-risk identity, values, relationships, health, finance,
    privacy, and long-term direction clusters fail closed with no automatic
    materialization.
- **W132: Proposal outcome to evidence convergence**
  - Complete: accepted/edited/rejected outcomes create positive/corrective/
    negative ProposalOutcome evidence metadata while preserving proposal/run/
    evidence lineage and omitting raw edited payloads.
- **W133: Candidate suppression and correction**
  - Complete: candidate suppression/correction uses opposing evidence,
    rejected-similar cooldowns, conflict state, decay state, and rejected
    history deterministically with ids/hashes/counts only.

### Goal 4: Accepted Guidance And Materialization (complete)

- **W134: Accepted guidance lifecycle**
  - Complete: converts accepted maturation candidates into accepted/trial guidance assets
    with HeuristicStore lifecycle and provenance.
- **W135: Governed materialized LifeModel view provenance**
  - Complete: materialized compatibility view carries source proposal/evidence/
    patch/heuristic digests.
- **W136: Version diff and rollback read model**
  - Complete: backend read model exposes LifeModel version/diff/rollback
    references linked to accepted guidance and materialized view provenance.

### Goal 5: Runtime Guidance Integration (complete)

- **W137: RuntimeHSPacket v2 guidance contract**
  - Complete: packet metadata supports accepted/trial guidance impact, risk,
    privacy, and source lineage summaries. Seeded built-in heuristics can be
    selected heuristics, but are not `guidance_refs`.
- **W138: ReAct guidance consumption**
  - Complete: explicit non-default ReAct materially consumes selected guidance
    in prompt/config/action boundaries only when
    `RuntimeGuidanceConsumptionMode::ExplicitRuntime` is enabled, with tests
    proving behavior/trace change. The default mode is disabled.
- **W139: Plan-Execute guidance consumption**
  - Complete: explicit Plan-Execute materially consumes selected guidance for
    weekly planning only when runtime guidance consumption mode is enabled,
    with tests proving plan shape changes.
- **W140: Runtime guidance trace/read model**
  - Complete: metadata-safe Guidance Impact read model and trace linkage.

### Goal 6: Policy / Privacy / Tool Governance Hardening (complete)

- **W141: ModelRouter/Privacy HS hardening**
  - Complete: High/Critical privacy hard-filters non-local providers before
    scoring; HS LocalOnly selects local `ollama`, emits a metadata-safe
    `local_only` governor report, removes cloud fallback, and fails closed when
    no local model is available.
- **W142: ActionExecutor HS tool governance**
  - Complete: unsupported Plugin/A2A tools remain disabled/declarative-only
    before permission replay or execution; HS direct external write paths remain
    proposal-first with metadata-safe governance reports.
- **W143: Governor unified decision report**
  - Complete: shared Governor decision/report shape classifies allow, block,
    confirm, proposal-first, and local-only decisions for LifeModel maturation,
    model route, tool action, memory write, and external write decisions without
    raw prompt/user text/assistant output/memory/LifeModel/tool payload leakage.

### Goal 7: Backend Golden Paths (complete)

- **W144: Weekly Planning golden path**
  - Complete: pure backend/core proof for the weekly planning guidance loop
    across selected RuntimeHSPacket guidance, Plan-Execute draft/finalize/step
    execution, proposal-first write-like step metadata, outcome evidence, and
    future planning guidance refs.
- **W145: Low-Energy Support golden path**
  - Complete: pure backend/core proof for low-energy support from LifeEvent /
    Signal / Evidence through accepted guidance to explicit runtime behavior
    change, without automatic high-risk truth materialization.
- **W146: Preference Correction golden path**
  - Complete: pure backend/core proof that rejection/edit outcomes create
    negative/corrective evidence and deterministically suppress or change future
    behavior.
  - Goal 7 adds no default Chat migration, no ordinary `send_message` /
    `start_stream_message` replacement, no Tauri command, no UI, no runtime
    executor/model/tool call, no durable LifeModel/Memory/external provider
    state write, and no migration permission. Ordinary Chat must not call
    W144-W146 golden path helpers or treat golden path ready as migration
    permission.

### Goal 8: Pre-UI Backend Contract Freeze (next)

- **W147: UI read model contract freeze**
  - Add backend structs/commands only if needed for read models. No large UI.
- **W148: Final backend completion gate**
  - One read-only report proving all required gates pass or listing blockers.
- **W149: Docs/progress/verification sync**
  - Update `AGENTS.md`, `plans/README.md`, progress index, and relevant docs.
  - Make stale docs explicitly defer to this spec.

## 10. Acceptance Gates

### 10.1 LifeModel Maturity Gate

Must pass:

- LifeEvent and Signal pipeline exists.
- Evidence Graph supports support/opposition/conflict/decay/dedupe/cooldown.
- Maturation Engine generates low-risk candidates from evidence.
- Proposal outcomes create positive/negative/corrective evidence.
- Accepted guidance can be selected into RuntimeHSPacket.
- Materialized LifeModel view has provenance.

### 10.2 Runtime Driven Gate

Must pass:

- ReAct consumes selected guidance and traces it.
- Plan-Execute consumes selected guidance and traces it.
- ModelRouter respects HS privacy policy.
- ActionExecutor respects HS tool/write policy.
- Guidance changes behavior but never relaxes hard policy.

### 10.3 Governance / Privacy Gate

Must pass:

- High-risk domains cannot auto-materialize.
- External writes cannot bypass proposal-first.
- Raw sensitive payloads do not enter normal traces/readiness reports.
- Cloud routing cannot bypass LocalOnly.
- Materialized LifeModel changes can be traced to source proposals/evidence.

### 10.4 UI Read Model Gate

Must pass:

- Learning Inbox read model exists.
- Evidence Timeline read model exists.
- Guidance Impact read model exists.
- Runtime Trace read model includes HS influence.
- Privacy/Policy read model exists.
- Version/Diff/Rollback read model exists.

## 11. Testing And Verification Requirements

Every W-slice must add focused tests. The final backend completion gate must
run at least:

```bash
cargo test -p openlife-core agent::tests -- --nocapture
cargo test -p openlife-tauri legacy_write_convergence -- --nocapture
cargo test -p openlife-tauri default_chat_entrypoints_do_not_call_w19_w60_command_surfaces -- --nocapture
cargo test -p openlife-tauri default_chat_entrypoints_do_not_call_w98_w123_surfaces -- --nocapture
make ci
```

Use exact available test names when repo naming differs. Add or update focused
tests for:

- metadata safety and raw-content blocking
- default Chat isolation
- no hidden direct writes
- proposal-first LifeModel/Memory/external writes
- high-risk domain fail-closed behavior
- LocalOnly model route enforcement
- guidance selected vs not selected behavior difference
- rejection/edit negative evidence behavior
- idempotency of golden paths
- read model payload minimization

Also run targeted `rg` checks before each submission:

```bash
rg -n "send_message|start_stream_message" src-tauri/src/lib.rs src-tauri/src/commands
rg -n "persist_life_model\\(" src-tauri/src openlife-core/src
rg -n "LifeEvent|Signal|Evidence|RuntimeHSPacket|LocalOnly|proposal-first" openlife-core/src src-tauri/src plans AGENTS.md
```

## 12. Goal-Mode Execution Rules

- Use one CLI Goal for one major Goal section, not for the entire W124-W149
  program unless explicitly requested.
- Keep W-slices serial and validated.
- Do not push from the implementation Agent. Human/user review decides commit
  and push.
- If a slice reveals a new blocker, classify it under one of the acceptance
  gates. Do not create an unbounded side quest.
- If a planned slice is too large, split it, but keep the original gate.
- If an implementation needs a command surface, justify why a pure backend
  contract/read model is insufficient.
- Prefer pure Rust/core tests before Tauri/frontend surfaces.
- Keep product UI out of scope except minimal read-model wrappers/mocks needed
  to keep contracts testable.

## 13. Next CLI Goal Prompt

Use this prompt for the next implementation Goal.

```text
You are implementing Goal 8 of the LifeModel-Governed Backend Completion stage:
Pre-UI Backend Contract Freeze.

Read these files first:
- AGENTS.md
- plans/README.md
- plans/lifemodel_governed_backend_completion_goal_spec.md
- plans/openlife_lifemodel_governed_agent_runtime.md
- plans/lifemodel_governed_runtime_progress.md
- plans/adr/0013-lifemodel-hs-source-of-truth-governance.md

Current baseline:
- W124-W146 Backend Completion Goals 1-7 are complete.
- Default Chat remains `legacy_stream`.
- Ordinary `send_message` / `start_stream_message` must not call W19-W146
  readiness/status/proof/review/product/maturity/schema/bridge/graph/timeline/
  maturation/guidance/materialization/runtime-guidance/policy-privacy-tool
  governance/golden-path helpers or commands.
- Legacy Direct-Write Convergence remains complete; do not reintroduce hidden
  durable LifeModel writes.
- W73-W78 LifeModel maturation proof exists and remains non-default.
- W128-W130 Evidence Graph v1 exists as a pure backend graph/timeline read
  model with support/opposition/dedupe/conflict/decay/cooldown metadata.
- W131-W133 Maturation Engine v1 exists as pure backend low-risk candidate
  generation, proposal outcome evidence convergence, and deterministic
  suppression/correction. It does not materialize LifeModel truth or activate
  heuristics.
- W134-W136 Accepted Guidance And Materialization exists as pure backend
  accepted guidance lifecycle, governed LifeModel compatibility materialized
  view provenance, and metadata-safe version diff/rollback read model. Trial
  guidance assets preserve proposal/evidence/run lineage and constraints; the
  compatibility materialized view remains derived, not accepted source-of-truth.
- W137-W140 Runtime Guidance Integration exists as RuntimeHSPacket v2
  accepted/trial guidance metadata, non-default ReAct and Plan-Execute guidance
  consumption gated by `RuntimeGuidanceConsumptionMode::ExplicitRuntime`, and
  metadata-safe Guidance Impact trace/read model. It does not change ordinary
  Chat routing, consume accepted guidance in ordinary Chat, or relax
  policy/proposal-first boundaries.
- W141-W143 Policy / Privacy / Tool Governance Hardening exists as ModelRouter
  High/Critical local-only hard filtering, ActionExecutor HS tool governance,
  and shared metadata-safe Governor decision reports. It does not change
  ordinary Chat routing, grant migration permission, or allow cloud fallback for
  LocalOnly/High/Critical routes.
- W144-W146 Backend Golden Paths exist as pure backend/core proofs for Weekly
  Planning, Low-Energy Support, and Preference Correction. They do not migrate
  default Chat, replace ordinary send/stream, add Tauri commands, add UI, run
  runtime/model/tool calls, write durable LifeModel/Memory/external provider
  state, or grant migration permission.
- EvidenceStore, HeuristicStore, PolicyStore, ProposalStore, PatchStore,
  RuntimeHSPacket, ReAct, Plan-Execute, ModelRouter, and ActionExecutor already
  exist. Reuse them.

Implement W147-W149 only:

W147:
- Freeze backend read model contracts needed before large UI/UX work. Cover the
  Learning Inbox, Evidence Timeline, Proposal Review, Runtime Trace, Guidance
  Impact, Privacy Controls, and LifeModel Overview surfaces with stable,
  metadata-safe structs or read-model wrappers. Add backend command wrappers
  only if the contract cannot otherwise be tested, and keep them read-only.

W148:
- Add one final backend completion gate report that proves all required gates
  pass or lists blockers. The report must be metadata-safe, read-only, and
  explicit about default Chat isolation, proposal-first boundaries, raw-content
  exclusion, local-only privacy behavior, tool governance, golden path coverage,
  and remaining Beta blockers.

W149:
- Sync authority docs, progress index, verification matrix, and stale-reference
  guidance. Make old docs explicitly defer to this spec where they conflict.

Hard constraints:
- Do not migrate default Chat.
- Do not modify ordinary Chat routing.
- Do not call W144-W146 golden path helpers from ordinary Chat.
- Do not treat Goal 7 golden path readiness as migration permission.
- Do not bypass ProposalStore/governor/materializer caller restrictions.
- Do not add large UI/UX.
- Do not leak raw prompt, raw user text, assistant output, memory content,
  LifeModel raw fields, or tool payloads in metadata-safe reports.
- Do not push. Do not commit unless explicitly instructed by the reviewer.

Verification:
- Run focused cargo tests for the new contract-freeze code.
- Run affected RuntimeHSPacket, Evidence Graph, Maturation Engine, accepted
  guidance, Plan-Execute, ReAct, Governor, proposal, golden path, and runtime
  contract tests.
- Run `make ci` if the focused tests pass.
- Run `rg` checks proving ordinary Chat did not call W144-W146 golden path
  helpers or any new Goal 8 read-model/gate helpers.

Output:
- W147-W149 change summary.
- New structs/functions/files.
- Tests run and results.
- Remaining blockers mapped to the master spec gates.
- Whether Goal 8 is complete.
```

## 14. Handoff Standard

At the end of each Goal, the implementation Agent must provide:

- completed W-slices
- files changed
- new public/internal APIs
- behavior changes
- commands added or explicitly not added
- store writes introduced or explicitly not introduced
- migration/default Chat impact
- raw-content/privacy review
- tests run
- blockers
- next recommended Goal

The reviewer then performs validation, commit, and push. The implementation
Agent must not push by default.
