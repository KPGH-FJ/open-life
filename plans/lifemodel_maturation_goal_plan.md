# LifeModel Maturation Loop End-to-End Goal Plan

> Last updated: 2026-06-03
> Status: W78 run trace visibility proof complete

This document is the entry point for the next Goal-mode development block after
W72. It does not authorize default Chat route migration, controlled adapter
executor attachment, or direct LifeModel writes.

## 1. Goal Boundary

Build one narrow, reversible LifeModel maturation loop end-to-end:

```text
Runtime / Chat / Feedback / Calibration source
  -> LifeEventDraft
  -> Signal / maturation candidate
  -> Evidence
  -> Governor
  -> Proposal
  -> user accept / reject / edit
  -> future RuntimeHSPacket-visible collaboration guidance
```

The first product domain is:

```text
low-energy / low-pressure planning preference
```

This is intentionally narrow. It is lower risk than identity, values,
relationships, health, finance, or long-term goal rewriting, and it can improve
future planning behavior without making LifeModel-HS truth mutable by raw model
output.

## 2. Current Baseline

Current completed default Chat adapter state:

- W65-W72 backend-only controlled adapter descriptor / contract / invocation /
  send proof / stream proof / attachment gate / disabled skeleton / binding
  integrity proof stack is complete.
- The proof stack is metadata-safe, no-run, no-write, no-stream, no-command,
  no-frontend, no-executor-attachment, no-route-cutover.
- Default Chat remains `legacy_stream`.
- Ordinary `send_message` / `start_stream_message` may only use the W49-W55
  pure ordinary-entry guard/preflight and must not call W67-W72 proof/skeleton
  code or W73-W78 LifeModel maturation helper code.

Current LifeModel maturation baseline:

- `openlife-core/src/agent/runtime_contract.rs`
  - `RuntimeOutput` can carry `life_event_candidates`.
  - `LifeEventDraft` exists as a draft-only candidate shape.
  - `RuntimeOutput::from_agent_loop_result` currently emits no candidates.
- `openlife-core/src/agent/maturation.rs`
  - `LifeModelMaturationService` converts LifeEventDrafts into proposal
    candidates.
  - `MaturationService::mature_runtime_output` creates governed Evidence and
    Proposal records from RuntimeOutput candidates.
  - High-risk LifeModel candidates remain proposal-first.
  - Raw user input / assistant output is not copied into evidence/report audit.
- `openlife-core/src/agent/proposal_outcome.rs`
  - `MaturationProposalOutcome` and
    `MaturationProposalOutcomeEvidenceReport` model proposal accept/reject/edit
    outcome evidence.
  - `evaluate_maturation_proposal_outcome_evidence` is pure/report-only.
  - `record_maturation_proposal_outcome_evidence` writes metadata-safe
    `ProposalOutcome` evidence only for maturation lineage proposals.
- `openlife-core/src/agent/maturation.rs`
  - `LowEnergyCollaborationRuleCandidateInput` and
    `LowEnergyCollaborationRuleCandidateReport` model W76 rule candidate
    aggregation.
  - `evaluate_low_energy_collaboration_rule_candidate` is pure/report-only.
  - `propose_low_energy_collaboration_rule_candidate` writes only a pending
    ProposalStore candidate proposal when evidence is sufficient and not
    blocked by opposing outcome evidence.
  - `AcceptedLowEnergyRuleSelectionInput`,
    `AcceptedLowEnergyRuleSelectionReport`, and
    `AcceptedLowEnergyRuleSelectionHSPacketAuditProof` model W77 accepted rule
    selection proof.
  - `evaluate_accepted_low_energy_rule_selection` is pure/report-only.
  - `ensure_accepted_low_energy_rule_selection` returns the same proof when
    selected and fails closed otherwise.
  - `LowEnergyRuleTraceVisibilityInput`,
    `LowEnergyRuleTraceVisibilityReport`, and `LowEnergyRuleTraceMetadata`
    model W78 metadata-safe run trace visibility proof.
  - `evaluate_low_energy_rule_trace_visibility` is pure/report-only.
  - `ensure_low_energy_rule_trace_visibility` returns the same proof when
    trace visibility is ready and fails closed otherwise.
- `src-tauri/src/commands/proposal.rs`
  - `accept_proposal_with_state`, `reject_proposal_with_state`, and
    `edit_proposal_with_state` call the W75 helper after successful proposal
    status updates.
  - Non-maturation proposals no-op; EvidenceStore/lineage issues do not block
    existing proposal accept/reject/edit flows.
- `openlife-core/src/agent/evidence_store.rs`
  - EvidenceStore supports metadata-safe candidate evidence, source refs,
    proposal links, AgentRun links, weakening/archive/contradiction/tombstone
    lifecycle.
- `openlife-core/src/agent/governor.rs`
  - LifeModelGovernor already gates maturation candidates and blocks
    `proposal_only=false`.
- Existing maturation tests pass:
  - `cargo test -p openlife-core proposal_outcome -- --nocapture`
  - `cargo test -p openlife-core maturation_loop -- --nocapture`
  - `cargo test -p openlife-core runtime_output_life_event_candidates_do_not_persist_to_lifemodel_or_hs_stores -- --nocapture`

## 3. Non-Goals

Do not do any of the following in this Goal block unless a later explicit stage
changes the boundary:

- Do not migrate default Chat away from `legacy_stream`.
- Do not attach or run the controlled default Chat adapter executor.
- Do not call W67-W72 proof/skeleton functions from ordinary Chat entries.
- Do not make raw model output accepted LifeModel truth.
- Do not directly write LifeModel, MemoryStore, HeuristicStore active rules, or
  materialized YAML from a LifeEventDraft.
- Do not add broad identity/values/relationship/health/finance maturation in
  the first slice.
- Do not add a large UI/editor before the backend loop is proven.
- Do not use cloud extraction over raw sensitive content as an MVP dependency.

## 4. Hard Acceptance Rules

Every slice in this Goal block must preserve:

- Proposal-first: LifeModel and memory changes must be reviewable proposals
  before apply.
- Metadata safety: evidence, reports, run metadata, and debug dumps must not
  contain raw prompt, raw assistant output, raw memory context, tool payloads,
  secrets, emails, phone numbers, or file body content.
- Reversibility: accepted collaboration guidance must have lineage and an
  obvious path to weaken/archive/reject.
- Negative evidence: rejection should become useful evidence against repeated
  similar suggestions.
- Narrow behavior: the first behavior change may only affect low-energy /
  low-pressure planning guidance.
- Runtime visibility: when a collaboration rule affects future behavior, the
  RuntimeHSPacket or run trace must show metadata-safe selected guidance and
  evidence lineage.
- Default Chat isolation: no work in this block may change default Chat routing.

## 5. Recommended W-Slice Plan

### W73: Maturation End-to-End Readiness Report

Status: Done.

Goal:

Added a pure/read-only backend report that evaluates whether the current
MaturationService, EvidenceStore, ProposalStore, Governor, RuntimeOutput
candidate shape, and default Chat isolation are ready for a non-default
end-to-end maturation invocation.

Expected shape:

- Internal Rust report/evaluator first.
- Optional explicit read-only Tauri command only if it is useful for future
  Settings visibility; if added, it must be read-only and metadata-safe.
- No runtime/model/tool execution.
- No Evidence/Proposal/LifeModel/Memory/Heuristic writes.
- No default Chat route change.

Acceptance:

- Reports existing maturation primitives and blockers.
- Confirms default Chat remains isolated on `legacy_stream`.
- Confirms ordinary Chat entries do not call maturation readiness code.
- Fails closed when a synthetic candidate would be raw-content-bearing,
  unsupported, low confidence, proposal_only=false, or outside the low-energy
  / planning domain.

### W74: Non-Default Maturation Invocation Command

Status: Done.

Goal:

Add an explicit non-default command or backend harness that takes a
metadata-safe RuntimeOutput candidate and runs `MaturationService` into
EvidenceStore + ProposalStore.

Acceptance:

- Creates Evidence and pending Proposal only.
- Does not write LifeModel, MemoryStore, HeuristicStore active records, Chat
  messages, MCP audit, external write actions, or default Chat adapter records.
- Stores only candidate digest, source refs, proposal id, AgentRun id, risk,
  confidence, reason code, and metadata-safe summary.
- Rejects or redacts raw-content-bearing metadata.
- Implemented as pure core explicit non-default harness/report in
  `openlife-core/src/agent/maturation.rs`; no Tauri command or frontend surface
  was added.

### W75: Proposal Outcome Evidence Link

Status: Done.

Goal:

Make proposal accept/reject/edit outcomes produce metadata-safe outcome
evidence for maturation candidates.

Acceptance:

- Accepted proposal outcome links back to evidence/proposal/source run lineage.
- Rejected proposal outcome creates negative evidence or opposing refs.
- Edited proposal outcome records edit metadata without storing raw reviewer
  text outside existing proposal semantics.
- Existing proposal apply semantics remain unchanged.
- Implemented as core helper/report plus minimal internal proposal command
  wiring. It is not a maturation runtime migration and not a default Chat
  migration.

### W76: Low-Energy Collaboration Rule Candidate

Status: Done.

Goal:

Aggregate repeated low-energy / low-pressure planning signals into a
reviewable collaboration rule proposal.

Acceptance:

- Implemented as pure core evaluator/report plus optional ProposalStore
  proposer in `openlife-core/src/agent/maturation.rs`.
- Aggregates metadata-safe accepted/edited/rejected W75 ProposalOutcome
  evidence in the low-energy / low-pressure planning collaboration scope.
- Preserves accepted/rejected/edited outcome evidence ids, source evidence ids,
  linked proposal ids, and linked AgentRun ids.
- Produces only a pending reviewable candidate proposal; it does not activate a
  Heuristic and does not write an active rule.
- Rejected/negative/opposing outcome evidence blocks or weakens repeated
  similar rule suggestions.
- Edited outcome evidence participates only through metadata-safe ids/digests
  and does not leak raw edited payload.
- Non-low-energy domains and outcome evidence outside the collaboration scope
  fail closed.
- No Tauri command, frontend surface, runtime/model/tool call, ordinary Chat
  integration, or direct LifeModel/Memory/Heuristic write was added.

### W77: Accepted Rule To RuntimeHSPacket Selection Proof

Status: Done.

Goal:

After user acceptance, prove a narrow collaboration rule can be selected into a
future RuntimeHSPacket for planning tasks.

Acceptance:

- Implemented as pure core evaluator/report/ensure in
  `openlife-core/src/agent/maturation.rs`.
- Only accepted W76 candidate proposals can be selected.
- Pending/rejected/non-W76 proposals fail closed.
- Only planning tasks and the low-energy / low-pressure planning domain are
  affected; non-planning and non-low-energy targets fail closed.
- Privacy policy cannot be relaxed by the rule; LocalOnly policy is preserved
  or strengthened.
- The selected guidance appears in metadata-safe HS packet audit/proof fields.
- Outcome evidence, source proposal, and AgentRun lineage are retained by ids.
- No Tauri command, frontend surface, runtime/model/tool call, ordinary Chat
  integration, direct LifeModel/Memory/Heuristic write, or Heuristic activation
  was added.

### W78: Run Trace Visibility

Status: Done.

Goal:

Prove the selected collaboration rule and evidence lineage can be exposed in
future run trace or read-only diagnostics metadata without running runtime/model/tool
or writing a persistent AgentRun trace.

Acceptance:

- Implemented as pure core evaluator/report/ensure in
  `openlife-core/src/agent/maturation.rs`.
- W77 selected guidance is visible as metadata-safe summary/hash and future
  RuntimeHSPacket guidance hash.
- Candidate proposal id/hash, candidate rule digest, selected policy ids,
  enforced route policy, and report/payload hashes are visible.
- Evidence/proposal/AgentRun lineage is visible only as id/hash/count/status/type.
- Blocked or non-selected W77 reports fail closed.
- Pending/rejected/non-W76, non-planning, and non-low-energy selections remain
  fail-closed through W77 blockers.
- Trace payloads containing raw prompt, assistant output, tool payload,
  LifeModel raw text, memory raw text, secrets, or raw edited payload fail
  closed and are not echoed in the report.
- Trace payloads attempting privacy/model route relaxation, local-only policy
  weakening, default Chat route cutover, runtime/model/tool execution, AgentRun
  writes, or Heuristic activation fail closed.
- No Tauri command, frontend surface, runtime/model/tool call, ordinary Chat
  integration, AgentRun store write, direct LifeModel/Memory/Heuristic write,
  or Heuristic activation was added.

## 6. Post-W78 Guardrail

```text
W78 is complete. Any future runtime trace integration must be requested as a
separate slice and must start from the W78 metadata-safe contract. It must not
write persistent AgentRun trace records, attach default Chat, run
runtime/model/tool, activate Heuristic truth, or relax privacy/model route
policy unless that later task explicitly scopes, reviews, implements, and tests
those changes.
```

## 7. Goal-Mode Operating Rules

- Use one W-slice per Agent iteration.
- 验收通过后再提交推送。
- If a slice touches runtime behavior, run `make ci` before commit.
- If a slice is docs-only or pure internal report-only, run `git diff --check`
  plus targeted tests/rg checks.
- Update `AGENTS.md`, `plans/README.md`, and
  `plans/lifemodel_governed_runtime_progress.md` whenever runtime authority,
  LifeModel source-of-truth, proposal semantics, privacy boundaries, or default
  Chat routing assumptions change.
