# OpenLife Plans Document Governance

> Last updated: 2026-05-31
> Status: authoritative document index for Agents

This file prevents old planning documents from accidentally steering new Agent
work. If two documents disagree, use the precedence below.

## 1. Precedence

1. `AGENTS.md`
   - Project-wide Agent instructions and current Tool Taxonomy.
2. `plans/README.md`
   - This document authority map.
3. `plans/openlife_lifemodel_governed_agent_runtime.md`
   - Current implementation program and next development order.
4. `plans/lifemodel_governed_runtime_progress.md`
   - Compact W1-W29 completion/status index. This is not a second roadmap.
5. Hard governance baselines:
   - `plans/adr/0013-lifemodel-hs-source-of-truth-governance.md`
   - `plans/openlife_react_beta_roadmap.md`
   - `plans/lifemodel_hs_mvp_task_specs.md`
   - `plans/lifemodel_hs_legacy_write_path_audit.md`
6. Scoped architecture/product baselines:
   - `plans/openlife_agent_framework_architecture.md`
   - `OpenLife_PRD_v2_Agent_Framework.md`
7. Current execution helpers:
   - `plans/openlife_development_plan.md`
   - `plans/openlife_codex_execution_playbook.md`
8. Historical/reference documents.
   - These can explain why earlier decisions were made, but cannot override
     the current program.

## 2. Current Development Order

```text
tool/proposal hygiene
-> thin runtime spine
-> ReAct convergence
-> maturation loop
-> governor
-> Plan-Execute
-> strategy abstraction
```

Current implementation has completed W1-W29 through sustained Runtime Migration
Gate evidence, controlled Chat pilot eligibility, a very small explicit Chat
Controlled Pilot with fallback, reviewed pilot response promotion,
source-bound post-promotion validation, metadata-safe promotion evidence, a
read-only promotion readiness gate, and a reviewed migration plan draft
generator plus metadata-safe manual migration review decision evidence and a
read-only implementation gate plus a non-default controlled migration shadow
run plus metadata-safe manual shadow review evidence. The next practical
sequence is:

```text
use shadow review evidence only for human review; default Chat remains unchanged
```

## 3. Current Authoritative Entry Points

| Document | Use for |
| --- | --- |
| `AGENTS.md` | Agent instructions, project context, Tool Taxonomy, current constraints. |
| `plans/openlife_lifemodel_governed_agent_runtime.md` | Next implementation order and LifeModel-Governed Runtime program. |
| `plans/lifemodel_governed_runtime_progress.md` | W1-W29 completion/status index and preview/not-default/migration-gate/pilot-eligibility/controlled-pilot/promotion-validation/evidence-readiness/draft-planning/review-decision/implementation-gate/shadow-run/shadow-review boundary. |
| `plans/adr/0013-lifemodel-hs-source-of-truth-governance.md` | LifeModel-HS source-of-truth, proposal-first, privacy, materialized-view hard rules. |
| `plans/openlife_react_beta_roadmap.md` | ReAct execution seriousness, Beta gates, tool/action/audit baseline. |
| `plans/lifemodel_hs_mvp_task_specs.md` | Coding-ready LifeModel-HS MVP task specs. |
| `plans/lifemodel_hs_legacy_write_path_audit.md` | Direct-write convergence backlog and safety map. |
| `plans/openlife_development_plan.md` | Current execution route, already aligned to the LifeModel-Governed program. |
| `plans/openlife_codex_execution_playbook.md` | How to slice and verify individual Codex tasks. |

## 4. Historical Or Scoped Reference Documents

These files are useful context, but they are not current execution authority:

| Document | Status |
| --- | --- |
| `OpenLife_Final_PRD.md` | Historical long-form PRD. Do not use for current task order. |
| `plans/openlife_alpha_beta_plan.md` | Historical Alpha to Beta productization plan. |
| `plans/openlife_remaining_tasks_plan.md` | Historical sprint debt plan. Re-check code before using any item. |
| `plans/openlife_stabilization_and_spine_consolidation_plan.md` | Historical stabilization plan. |
| `plans/builder_life_model_design.md` | Builder UX/domain reference only; LifeModel-HS governance overrides direct-write assumptions. |
| `plans/frontend_experience_rebuild_plan.md` | Frontend UX reference only; current IA is governed by Agent/LifeModel-HS docs. |
| `plans/engineering_structure_notes.md` | Engineering history/reference only. |
| `architecture_diagram.md` | Snapshot diagram; verify against code and current program. |
| `BETA_CHECKLIST.md` | Historical checklist; current Beta/tool status is in AGENTS and roadmap. |
| `docs/ARCHITECTURE.md` | Quick architecture explainer; defer to current program for implementation order. |
| `docs/DEV_HANDOVER.md` | General handover; defer to this index and AGENTS for current Agent work. |

## 5. Tool Status Guardrail

`calendar.propose_event` and `email.propose_draft` are P1 proposal-only
governed executors. They create `ScheduledTask` / `DataExport` proposals and
must not perform real calendar writes, email sends, or `ExternalWriteAction`
fallback unless a future governed provider executor and tests are added.

`ExternalWriteAction` proposal creation must enforce pre-insert size limits and
payload minimization. This is a hard acceptance gate.

`run_multi_strategy_agent_preview` is a preview/beta command. Its W10 AgentRun
audit is a metadata-safe outer run; any ReAct inner run id is child metadata and
must not become the product trace's primary query id. Do not replace
`send_message` or the default Chat path just because the preview path works.

`check_runtime_migration_gate` and the Settings Runtime Migration Gate panel are
read-only evidence surfaces over existing preview audit state. They must not run
preview, ReAct, PlanExecute, tools, proposal apply, external writes, or
LifeModel/Memory writes, and they are not Chat migration switches. Controlled
Chat migration may only proceed as a smaller pilot after gate evidence stays
clean across runs.

`check_controlled_chat_pilot_eligibility` and the Settings Pilot eligibility
panel are also read-only. They default to the latest 3 MultiStrategy preview
AgentRuns, recompute gate reports, and expose `eligible`, clean run count,
checked run ids, blocking reasons, and the latest gate report. They must not
create AgentRuns, Proposals, Actions, Observations, audit rows, LifeModel/Memory
writes, or run any runtime/tool/proposal-apply path.

W20 adds only a very small Chat-page Controlled Pilot. It is explicit, single
turn, and fallback-preserving: normal Send does not call eligibility/gate/preview;
the pilot calls eligibility first, does not call preview when blocked, runs
`run_multi_strategy_agent_preview` only after eligibility passes, forces
`allowWrites=false`, and displays success as “Pilot response” outside normal
assistant history. Default Chat is still not migrated. Reviewed pilot response
promotion is a later phase, not part of W20.

W24 adds only `check_controlled_pilot_promotion_readiness` and its Settings
panel. The gate reads existing W23 promotion evidence, defaults to 3 required
metadata-safe promotions, accepts `sessionId` for a future filtered store path
but currently reports a global EvidenceStore summary, and must not create
AgentRuns, Proposals, Actions, Observations, LifeModel/Memory writes, external
tool writes, or new evidence. A ready result means discussion eligibility only;
it is not permission to migrate default Chat.

W25 adds only `draft_controlled_chat_migration_plan` and its Settings Draft
Migration Plan panel. The command reuses W24 readiness output. When readiness is
blocked it returns `draftReady=false` plus blockers and does not generate plan
sections. When readiness passes it returns a human-review-only scope,
preconditions, rollback plan, fallback plan, and test plan with
`manualReviewRequired=true` and `notAutomaticMigration=true`. It must not
replace default Chat, modify default runtime feature flags, create AgentRuns,
Proposals, Memory writes, LifeModel patches, promotion evidence, or output raw
user content, raw assistant output, or tool payloads.

W26 adds only `record_controlled_chat_migration_review_decision`,
`get_controlled_chat_migration_review_decision_summary`, and the Settings
Migration Review Decision panel. The record command first calls W25 draft,
rejects blocked-draft `approve` without writing evidence, and records ready
draft `approve` / `reject` / `request_rework` as metadata-safe EvidenceStore
decision evidence only. Evidence metadata must include
`evidenceKind=migration_review_decision`, `metadataSafe=true`, `draftReady`,
`decisionKind`, readiness counts, draft hash, and `createdAt`; reviewer notes
are stored only as length, checksum, and bounded category. The summary command
is read-only and must not read raw transcript or create AgentRuns, Proposals,
Memory writes, LifeModel patches, or external tool results. Approval only allows
next-stage implementation discussion; it is not Chat migration permission.

W27 adds only `check_controlled_chat_migration_implementation_gate` and the
Settings Implementation Gate panel. The command reads current W24 readiness,
the current W25 draft hash, and existing W26 metadata-safe review decision
evidence. It returns implementation eligibility, latest decision, readiness
report, draft hash match, approved-after-latest-draft status, and blockers. The
latest metadata-safe decision must be `approve`; latest `reject` or
`request_rework`, approved draft hash mismatch, or current readiness failure
must block. It must not replace default Chat, modify feature flags, create new
review evidence, create AgentRuns, Proposals, Memory writes, LifeModel patches,
or invoke external tools. Eligible means controlled migration implementation
discussion only; it is not permission to switch default Chat.

W28 adds only `run_controlled_chat_migration_shadow_run` and the Settings
Shadow Run panel. The command first calls the W27 implementation gate. If the
gate is blocked, it returns blockers and does not execute runtime. If eligible,
it runs a bounded non-default controlled runtime preview with `allowWrites=false`
and returns only `shadowRunReady`, the implementation gate report,
strategy/payload kind, metadata-safe summary, warnings, and blockers. It may
create a metadata-safe `controlled_migration_shadow_run` AgentRun audit, but it
must not create Proposal, Memory, LifeModel patch, Evidence, chat message, or
external tool result records. It must not expose raw user prompt, raw assistant
output, or full tool payload. Default Send, `send_message`, and
`start_stream_message` must not call the shadow run command.

W29 adds only `record_controlled_chat_migration_shadow_review_decision`,
`get_controlled_chat_migration_shadow_review_summary`, and the Settings Shadow
Review panel. The record command reads an existing shadow AgentRun and records
human `approve` / `reject` / `request_rework` decisions as metadata-safe
EvidenceStore evidence only. Every decision is allowed only when the AgentRun exists,
has `reasoning_strategy=controlled_migration_shadow_run`, is completed,
`allowWrites=false`, `metadataSafe=true`, and has no Chat message, Proposal,
Memory, LifeModel patch, or external-write side effects. Evidence metadata is
strictly limited to `shadowRunId`, `decisionKind`, `reviewerNoteChecksum`,
`reviewerNoteLength`, `reviewerNoteCategory`, `readinessSummaryDigest`, and
`createdAt`; it must not store reviewer raw text, shadow prompt, shadow output,
or tool payload. The summary command is read-only. Default Send,
`send_message`, and `start_stream_message` must not call shadow review commands.
W29 is review evidence, not Chat migration.

## 6. Agent Rules

- Always read `AGENTS.md`, this file, and
  `plans/openlife_lifemodel_governed_agent_runtime.md` before starting a new
  architecture/runtime/LifeModel/tool task.
- Do not use historical plans to override current ordering or current Tool
  Taxonomy.
- If implementation changes tool status, proposal semantics, runtime authority,
  model routing, LifeModel source-of-truth, or privacy boundaries, update the
  relevant docs in the same task.
- If an old document conflicts with the current program, treat the old document
  as historical unless the user explicitly asks to revive or rewrite it.
