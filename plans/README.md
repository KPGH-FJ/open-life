# OpenLife Plans Document Governance

> Last updated: 2026-06-01
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
   - Compact W1-W46 completion/status index. This is not a second roadmap.
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

Current implementation has completed W1-W46 through sustained Runtime Migration
Gate evidence, controlled Chat pilot eligibility, a very small explicit Chat
Controlled Pilot with fallback, reviewed pilot response promotion,
source-bound post-promotion validation, metadata-safe promotion evidence, a
read-only promotion readiness gate, and a reviewed migration plan draft
generator plus metadata-safe manual migration review decision evidence and a
read-only implementation gate plus a non-default controlled migration shadow
run plus metadata-safe manual shadow review evidence and a read-only cutover
planning readiness gate plus a non-default cutover candidate adapter for Chat
contract-shape validation plus metadata-safe cutover candidate review evidence
plus a read-only cutover candidate promotion readiness gate plus a read-only
default Chat runtime boundary status plus a human-review-only default Chat
adapter activation plan draft plus metadata-safe activation review decision
evidence plus a read-only default Chat adapter activation implementation gate
plus a read-only default Chat adapter disabled routing scaffold plus a read-only
default Chat adapter contract harness plus a write-disabled default Chat adapter
dry-run invocation boundary plus metadata-safe default Chat adapter dry-run
review evidence plus a read-only default Chat adapter implementation readiness
gate plus an explicit non-default default Chat adapter controlled preview plus
metadata-safe human review decision evidence over that controlled preview plus
a read-only controlled preview approval readiness gate over W42/W44 evidence
and the approved W43 preview AgentRun current safety state plus a read-only
default Chat adapter cutover implementation plan draft over W45 readiness.
The next practical sequence is:

```text
use cutover implementation plan draft only as human-review planning evidence; default Chat remains unchanged
```

## 3. Current Authoritative Entry Points

| Document | Use for |
| --- | --- |
| `AGENTS.md` | Agent instructions, project context, Tool Taxonomy, current constraints. |
| `plans/openlife_lifemodel_governed_agent_runtime.md` | Next implementation order and LifeModel-Governed Runtime program. |
| `plans/lifemodel_governed_runtime_progress.md` | W1-W46 completion/status index and preview/not-default/migration-gate/pilot-eligibility/controlled-pilot/promotion-validation/evidence-readiness/draft-planning/review-decision/implementation-gate/shadow-run/shadow-review/cutover-readiness/cutover-candidate/candidate-review/candidate-promotion-readiness/default-chat-boundary/activation-plan/activation-review/activation-implementation-gate/disabled-routing-scaffold/contract-harness/dry-run boundary/dry-run review evidence/implementation readiness/controlled preview/controlled preview review evidence/controlled preview approval readiness/cutover implementation plan draft. |
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

W30 adds only `check_controlled_chat_cutover_readiness` and the Settings
Cutover Readiness panel. The command is read-only and requires the current W27
implementation gate to be eligible, the latest W29 shadow review decision to be
`approve`, and the approved `shadowRunId` AgentRun to still exist as a
completed `controlled_migration_shadow_run` with `allowWrites=false`,
`metadataSafe=true`, and no Chat/Proposal/Memory/LifeModel patch or external
write side effects. It returns only metadata-safe readiness summary fields and
blockers. It must not create AgentRuns, Evidence, Proposals, Memory writes,
LifeModel patches, MCP audit rows, or chat messages; it must not run ReAct,
PlanExecute, preview, or shadow run. W30 is cutover planning readiness for
implementation discussion, not default Chat migration.

W31 adds only `run_controlled_chat_cutover_candidate` and the Settings Cutover
Candidate panel. The command first calls W30 cutover readiness; if it is not
eligible, W31 returns blocked output and must not execute runtime. Only after
W30 is eligible may it run one non-default controlled runtime candidate with
`allowWrites=false`, `maxToolCalls=0`, no proposal apply, no Memory write, no
LifeModel patch, and no external write. The result is for Chat contract-shape
validation only: `candidateReady`, `candidateRunId`, `outputPreview` or
`userOutput`, `contractShape`, metadata-safe summary, warnings, and blockers.
It may create a metadata-safe `controlled_chat_cutover_candidate` AgentRun
audit, but must not save raw user prompt, raw assistant output, tool payload,
Chat message, Proposal, Memory, LifeModel patch, Evidence, MCP audit, or
external tool result. Default Send, `send_message`, and
`start_stream_message` must not call this command. W31 is a non-default
candidate adapter, not default Chat migration.

W32 adds only `record_controlled_chat_cutover_candidate_review_decision`,
`get_controlled_chat_cutover_candidate_review_summary`, and the Settings
Cutover Candidate Review panel. The record command reads an existing W31
candidate AgentRun and records human `approve` / `reject` / `request_rework`
decisions as metadata-safe EvidenceStore evidence only. Approve requires the
AgentRun to exist, be completed, have
`reasoning_strategy=controlled_chat_cutover_candidate`,
`contractShape=send_message_compatible`, `candidateReady=true`,
`allowWrites=false`, `maxToolCalls=0`, `metadataSafe=true`, and no
Chat/Proposal/Memory/LifeModel/Evidence/MCP audit/external write side effects.
Evidence metadata is strictly limited to `candidateRunId`, `decisionKind`,
`contractShape`, `candidateSummaryDigest`, `reviewerNoteChecksum`,
`reviewerNoteLength`, `reviewerNoteCategory`, and `createdAt`; it must not
store reviewer raw text, candidate userOutput, raw prompt, raw assistant output,
tool payload, or candidate output. The summary command is read-only. Default
Send, `send_message`, and `start_stream_message` must not call candidate review
commands. W32 is candidate review evidence, not default Chat migration.

W33 adds only `check_controlled_chat_cutover_candidate_promotion_readiness`
and the Settings Candidate Promotion Readiness panel. The command is read-only:
it reuses W30 cutover readiness, reads W32 metadata-safe review evidence, checks
that the latest candidate review decision is `approve`, verifies each approved
candidate AgentRun still exists and is completed, send-message-compatible,
write-disabled, zero-tool, metadata-safe, and side-effect-free, and confirms
default Chat is unchanged. It must not create AgentRuns, Evidence, Proposals,
Memory, LifeModel patches, MCP audit rows, chat messages, or runtime/tool/model
calls. Default Send, `send_message`, and `start_stream_message` must not call
candidate promotion readiness. W33 is implementation-planning readiness, not
default Chat migration.

W34 adds only `get_default_chat_runtime_boundary_status` and the Settings
Default Chat Runtime Boundary panel. The command is read-only and reports that
the current default Chat runtime remains `legacy_stream`, that automatic
migration is disabled, that no controlled candidate is available on the default
path, and that candidate promotion readiness remains required before any future
activation planning. It must not call W19-W33 gates, run runtime/tool/model
paths, or create AgentRuns, Evidence, Proposals, Memory, LifeModel patches, MCP
audit rows, or chat messages. Default Send, `send_message`, and
`start_stream_message` must not call default Chat boundary status. W34 is
boundary observability, not default Chat migration.

W35 adds only `draft_default_chat_adapter_activation_plan` and the Settings
Default Chat Adapter Activation Plan panel. The command is read-only and
combines W33 candidate promotion readiness with W34 default Chat runtime
boundary status. When blocked, it returns blockers and no plan sections. When
ready, it returns only a human-review activation scope, required preconditions,
adapter contract checks, fallback, rollback, observability, and test plan, with
`manualReviewRequired=true`, `notAutomaticMigration=true`, and
`requiresSeparateImplementation=true`. It must not switch default Chat, modify
feature flags, run runtime/tool/model paths, or create AgentRuns, Evidence,
Proposals, Memory, LifeModel patches, MCP audit rows, or chat messages. Default
Send, `send_message`, and `start_stream_message` must not call the activation
plan draft. W35 is activation planning, not default Chat migration.

W36 adds only `record_default_chat_adapter_activation_review_decision`,
`get_default_chat_adapter_activation_review_summary`, and the Settings Default
Chat Adapter Activation Review Decision panel. The record command first calls
the W35 activation plan draft. `approve` is rejected without evidence when the
draft is blocked; ready drafts may record approve/reject/request_rework as
metadata-safe EvidenceStore records. Evidence metadata stores only decision
kind, draftReady, activationPlanDigest, candidatePromotionReady, currentMode,
automaticMigrationEnabled, reviewerNote checksum/length/category, and createdAt.
The summary command is read-only. Default Send, `send_message`, and
`start_stream_message` must not call activation review commands. W36 is review
evidence for implementation gate discussion, not default Chat migration.

W37 adds only `check_default_chat_adapter_activation_implementation_gate` and
the Settings Default Chat Adapter Activation Implementation Gate panel. The
command is read-only and combines the current W35 stable activation plan digest
with W36 metadata-safe latest activation review decision evidence. It requires
current draft ready, latest approve, digest match, candidate promotion ready,
default Chat unchanged, `currentMode=legacy_stream`, and automatic migration
disabled. It must not create AgentRuns, Evidence, Proposals, Memory, LifeModel
patches, MCP audit rows, chat messages, runtime/tool/model calls, feature flags,
or default Chat routing changes. Default Send, `send_message`, and
`start_stream_message` must not call the activation implementation gate. W37 is
implementation gate readiness for separate implementation discussion, not
default Chat migration.

W38 adds only `get_default_chat_adapter_routing_status` and the Settings Default
Chat Adapter Routing Status panel. The command is read-only: it calls the W37
activation implementation gate, reports `currentMode=legacy_stream`,
`adapterScaffoldPresent=true`, `controlledAdapterEnabled=false`,
`defaultSendPath=legacy_stream`, `startStreamPath=legacy_stream`,
`activationImplementationGateEligible`, blockers, and a metadata-safe summary.
It must not create AgentRuns, Evidence, Proposals, Memory, LifeModel patches,
MCP audit rows, chat messages, runtime/tool/model calls, feature flags, or
default Chat routing changes. Default Send, `send_message`, and
`start_stream_message` must not call adapter routing status. W38 is disabled
routing scaffold observability, not default Chat migration.

W39 adds only `check_default_chat_adapter_contract_harness` and the Settings
Default Chat Adapter Contract Harness panel. The command is read-only: it calls
W38 routing status and validates the disabled adapter contract, including
`send_message` and `start_stream_message` remaining on `legacy_stream`,
controlled adapter disabled, and activation implementation gate eligibility. It
must not create AgentRuns, Evidence, Proposals, Memory, LifeModel patches, MCP
audit rows, chat messages, runtime/tool/model calls, feature flags, or default
Chat routing changes. Default Send, `send_message`, and `start_stream_message`
must not call contract harness. W39 is contract observability, not default Chat
migration.

W40 adds only `run_default_chat_adapter_dry_run` and the Settings Default Chat
Adapter Dry Run panel. The command is explicit and non-default: it calls the W39
contract harness first, blocks without dry-run output when the harness is not
ready, and when ready returns only a metadata-safe invocation contract result
with `allowWrites=false`, `maxToolCalls=0`, and
`defaultChatPathUnchanged=true`. It must not create AgentRuns, Evidence,
Proposals, Memory, LifeModel patches, MCP audit rows, chat messages,
runtime/tool/model calls, external writes, feature flags, or default Chat
routing changes. Default Send, `send_message`, and `start_stream_message` must
not call adapter dry run. W40 is write-disabled invocation contract
observability, not default Chat migration.

W41 adds only `record_default_chat_adapter_dry_run_review_decision`,
`get_default_chat_adapter_dry_run_review_summary`, and the Settings Default Chat
Adapter Dry Run Review panel. The record command re-runs W40 dry run before
recording evidence. `approve` records only when dry run is ready; blocked dry-run
approval writes no evidence. `reject` and `request_rework` record only
metadata-safe evidence. Reviewer notes are reduced to checksum, length, and
bounded category. It must not create AgentRuns, Proposals, Memory, LifeModel
patches, MCP audit rows, chat messages, runtime/tool/model calls, external
writes, feature flags, or default Chat routing changes. Default Send,
`send_message`, and `start_stream_message` must not call dry-run review commands.
W41 is dry-run review evidence, not default Chat migration.

W42 adds only `check_default_chat_adapter_implementation_readiness` and the
Settings Default Chat Adapter Implementation Readiness panel. The command is
read-only: it combines W37 activation implementation gate, W39 contract harness,
W40 dry run, and W41 latest dry-run review evidence; ready requires latest
approve, current dry-run digest match, default Chat unchanged, controlled
adapter disabled, automatic migration disabled, and send/stream paths still on
`legacy_stream`. It must not create AgentRuns, Evidence, Proposals, Memory,
LifeModel patches, MCP audit rows, chat messages, runtime/tool/model calls,
external writes, feature flags, or default Chat routing changes. Default Send,
`send_message`, and `start_stream_message` must not call implementation
readiness. W42 is implementation readiness, not default Chat migration.

W43 adds only `run_default_chat_adapter_controlled_preview` and the Settings
Default Chat Adapter Controlled Preview panel. The command is explicit and
non-default: it calls W42 implementation readiness first, blocks without runtime
or AgentRun when readiness is not ready, and when ready runs one controlled
preview with `allowWrites=false` and `maxToolCalls=0`. Ready output returns a
SendMessageResult-compatible shape for inspection and may create only a
metadata-safe adapter preview AgentRun audit. It must not create Evidence,
Proposals, Memory, LifeModel patches, MCP audit rows, chat messages, external
writes, feature flags, or default Chat routing changes. Default Send,
`send_message`, and `start_stream_message` must not call controlled preview.
W43 is controlled implementation preview, not default Chat migration.

W44 adds only `record_default_chat_adapter_controlled_preview_review_decision`,
`get_default_chat_adapter_controlled_preview_review_summary`, and the Settings
Default Chat Adapter Controlled Preview Review panel. Approve requires a
completed W43 preview AgentRun with
`reasoning_strategy=default_chat_adapter_controlled_preview`,
`contractShape=send_message_compatible`, `previewReady=true`,
`allowWrites=false`, `maxToolCalls=0`, `metadataSafe=true`, and no side effects.
Evidence metadata is limited to previewRunId, decisionKind, contractShape,
previewSummaryDigest, reviewer-note checksum/length/category, and createdAt;
summary is read-only. Default Send, `send_message`, and
`start_stream_message` must not call controlled preview review commands. W44 is
review evidence, not default Chat migration.

W45 adds only
`check_default_chat_adapter_controlled_preview_approval_readiness` and the
Settings Default Chat Adapter Controlled Preview Approval Readiness panel. It is
a read-only gate over current W42 implementation readiness, W44 latest
metadata-safe review approval, required approved preview count, digest match,
and the approved W43 preview AgentRun's current completed/send-compatible/
previewReady/write-disabled/zero-tool/metadata-safe/side-effect-free state. It
must not create AgentRuns, Evidence, Proposals, Memory, LifeModel patches, MCP
audit rows, chat messages, controlled preview/runtime/tool/model calls,
external writes, feature flags, or default Chat routing changes. Default Send,
`send_message`, and `start_stream_message` must not call controlled preview
approval readiness. W45 is approval readiness for later adapter cutover
implementation discussion, not default Chat migration.

W46 adds only `draft_default_chat_adapter_cutover_implementation_plan` and the
Settings Default Chat Adapter Cutover Implementation Plan panel. It is a
read-only draft over current W45 approval readiness. Blocked readiness must
return `draftReady=false`, propagated blockers, and no plan sections; ready
output may return only metadata-safe human-review implementation scope, adapter
contract requirements, routing boundary, safety preconditions, fallback,
rollback, observability, test plan, explicit non-goals, and a stable plan digest.
It must not create AgentRuns, Evidence, Proposals, Memory, LifeModel patches, MCP
audit rows, chat messages, controlled preview/runtime/tool/model calls, external
writes, feature flags, or default Chat routing changes. Default Send,
`send_message`, and `start_stream_message` must not call cutover implementation
plan draft. W46 is cutover implementation planning, not default Chat migration.

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
