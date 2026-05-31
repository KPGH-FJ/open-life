# LifeModel-Governed Runtime Progress

> Last updated: 2026-05-31
> Status: compact progress index, not a second roadmap

This file summarizes implementation status for Agents entering the project. It
does not replace `openlife_lifemodel_governed_agent_runtime.md`; when planning
next work, use the program document for ordering and this file only as a
completion/status index.

## Current Position

W1-W32 are complete. The project now has a governed PlanExecute V1 vertical
slice, a lightweight fixed `RuntimeStrategy` trait foundation for ReAct and
PlanExecute adapters, a read-only Runtime Migration Gate for Chat migration
diagnostics, a Settings evidence surface that makes the gate result visible
without changing Chat behavior, and a read-only controlled Chat pilot
eligibility check for sustained clean gate evidence. Chat also has a very small
explicit Controlled Pilot entry with fallback plus reviewed pilot response
promotion with source-bound post-promotion validation and metadata-safe
promotion evidence recording. W24 adds a read-only promotion evidence readiness
gate that answers only whether the existing W23 evidence is sufficient to
discuss the next Chat migration step. W25 adds a read-only reviewed migration
plan draft generator that reuses W24 readiness and produces a human-review-only
draft when readiness passes. W26 adds an explicit manual review decision stage
that records approve/reject/request_rework as metadata-safe decision evidence
only. W27 adds a read-only implementation gate that checks whether latest
metadata-safe approval, current W25 draft hash, and current W24 readiness are
aligned before implementation discussion can begin. W28 adds an explicit
non-default controlled migration shadow run that can execute only after W27
eligibility and returns metadata-safe readiness comparison output without
writing Chat, Proposal, Memory, LifeModel, Evidence, or external tool results.
W29 adds an explicit human shadow review evidence loop that records
approve/reject/request_rework decisions for existing shadow runs using only
metadata-safe whitelisted fields. W30 adds a read-only cutover planning
readiness gate that verifies W27 eligibility, latest W29 approval, and the
approved shadow AgentRun's current write-disabled metadata-safe side-effect-free
state before allowing implementation discussion. It is cutover planning
readiness, not Chat migration. W31 adds a non-default cutover candidate adapter
that first checks W30 readiness and, only when eligible, runs one
write-disabled zero-tool candidate to validate Chat-compatible contract shape.
It is a candidate adapter, not default Chat migration. W32 adds a manual
cutover candidate review evidence loop that records approve/reject/request_rework
with metadata-safe whitelisted fields only. It is candidate review evidence,
not default Chat migration.

The key boundary is unchanged:

- MultiStrategy Runtime is preview/audit-ready, not the default Chat runtime.
- `run_multi_strategy_agent_preview` is a preview/beta command.
- The default Chat path must not be replaced directly.
- Chat now has an explicit write-disabled Governed Preview path for runtime
  inspection; normal Send still uses the existing stream path.
- LifeModel-HS remains the protocol-layer direction; Maturation V1 exists as an
  explicit service entry, while automatic Chat application remains out of scope.
- PlanExecute V1 is a governed runtime slice, not a productized weekly-planning
  workflow.
- `RuntimeStrategy` now exists as a light adapter/registry boundary; it is not
  plugin loading and it does not migrate the default Chat path.
- `check_runtime_migration_gate` is a read-only diagnostic over existing
  preview AgentRun audit state. It does not execute ReAct, PlanExecute, tools,
  or external writes.
- The Settings Runtime Migration Gate panel only displays pass/block evidence
  and blocking reasons from `check_runtime_migration_gate`; it is not a Chat
  switching control and it does not auto-run preview.
- `check_controlled_chat_pilot_eligibility` defaults to the latest 3
  MultiStrategy preview AgentRuns, recomputes gate reports, and returns
  eligibility, clean run count, checked run ids, blocking reasons, and the
  latest gate report. It does not create AgentRuns, Proposals, Actions,
  Observations, audit rows, or LifeModel/Memory writes.
- The Settings Pilot eligibility panel displays controlled Chat migration pilot
  qualification only. It is not a Chat switching control; even an eligible
  result cannot automatically replace default Chat.
- W20 Controlled Pilot is explicit, single turn, and fallback-preserving. Normal
  Send still does not call eligibility/gate/preview. The pilot checks
  eligibility first, does not run preview when blocked, forces `allowWrites=false`
  when eligible, and renders success as “Pilot response” outside normal assistant
  history unless the user explicitly reviews and promotes it.
- W21 Reviewed Pilot Response Promotion keeps successful pilot output isolated
  by default, shows `Promote Pilot Response` only for successful pilot results
  with `userOutput`, opens a review state with response text, runId, selected
  strategy, governance summary, payload summary, and an explicit history-write
  warning, then writes exactly one ordinary assistant chat message through the
  existing chat message save path after confirmation. Cancel, blocked, failed,
  and no-output pilot states write nothing. The promoted assistant message may
  carry the existing `run_id` trace field when present; no LifeModel, Memory,
  Proposal, or external tool result is written by promotion.
- W22 Post-Promotion Validation binds each successful Controlled Pilot result
  to the chat session where it was run. Promotion review displays source
  session, target session, runId, strategy, and governance summary. Confirming
  promotion first verifies the current target session still matches the pilot
  source session. If the user switched sessions, promotion is blocked, no
  `save_chat_message` call is made, and the UI tells the user to rerun
  Controlled Pilot in the current session or switch back to the source session.
- W23 Controlled Pilot Promotion Evidence Recorder records metadata-safe
  evidence after a reviewed promotion successfully saves its ordinary assistant
  message. Evidence uses `EvidenceStore` and contains pilotRunId,
  sourceSessionId, targetSessionId, strategyKind, payloadKind,
  governanceDecisionKind, promotedMessageLength, promotedMessageHash, and
  promotedAt. It does not persist raw pilot responses, raw user prompts, full
  tool payloads, LifeModel/Memory/Proposal/Action/Observation data, or external
  tool results. Settings displays a read-only promotion evidence summary.
  Evidence failure leaves the promotion degraded and retrying records evidence
  only; it does not write the chat message again.
- W24 Promotion Evidence Readiness Gate adds
  `check_controlled_pilot_promotion_readiness`, a read-only command over
  existing promotion evidence. It defaults to 3 required metadata-safe
  promotions, returns ready/counts/recent pilot run ids/latest timestamp/source
  mismatch block count/metadataSafeEvidenceReady/defaultChatUnchanged/blocking
  reasons, and creates no AgentRun, Proposal, Action, Observation, LifeModel,
  Memory, external tool write, or new evidence. `sessionId` is accepted for a
  future filtered EvidenceStore path; current behavior is documented as a
  global summary. A ready result means discussion eligibility only, not
  automatic migration permission.
- W25 Reviewed Migration Plan Draft Generator adds
  `draft_controlled_chat_migration_plan`, a read-only command that reuses W24
  readiness output. When readiness is blocked it returns `draftReady=false`,
  the readiness report, and blocking reasons without executable plan sections.
  When readiness passes it returns migration scope, required preconditions,
  rollback plan, fallback plan, and test plan for human review only, with
  `manualReviewRequired=true` and `notAutomaticMigration=true`. It does not
  replace default Chat, modify default runtime feature flags, create AgentRuns,
  Proposals, Memory writes, LifeModel patches, promotion evidence, or output raw
  user content, raw assistant output, or tool payloads.
- W26 Manual Migration Review Decision Evidence adds
  `record_controlled_chat_migration_review_decision` and
  `get_controlled_chat_migration_review_decision_summary`. The record command
  first calls W25 draft, rejects blocked-draft approve without writing evidence,
  and records ready-draft `approve`, `reject`, or `request_rework` only as
  metadata-safe EvidenceStore evidence. Decision metadata includes
  `evidenceKind=migration_review_decision`, `metadataSafe=true`, `draftReady`,
  `decisionKind`, readiness counts, draft hash, and `createdAt`; reviewer notes
  are not stored raw and are reduced to length, checksum, and bounded category.
  The summary command reads decision evidence only and returns latest decision,
  approved count, rework/reject count, latest timestamp, and blockers. Approval
  means permission to discuss a separate implementation stage, not Chat
  migration permission.
- W27 Approved Migration Implementation Gate adds
  `check_controlled_chat_migration_implementation_gate`, a read-only command
  over current W24 readiness, current W25 draft hash, and W26 metadata-safe
  review decision evidence. It returns `implementationEligible`,
  `latestDecision`, `readinessReport`, `draftHashMatched`,
  `approvedAfterLatestDraft`, and `blockingReasons`. The latest metadata-safe
  review decision must be `approve`; latest `reject` or `request_rework`,
  approved draft hash mismatch, or current readiness failure blocks
  eligibility. Eligible means implementation development discussion only. It
  does not migrate Chat, replace default Chat, modify feature flags, write
  evidence, create AgentRuns/Proposals/Memory/LifeModel patches, or invoke
  external tools.
- W28 Non-Default Controlled Migration Shadow Run adds
  `run_controlled_chat_migration_shadow_run`, an explicit Settings-only
  migration comparison entry. It first calls W27 implementation gate; blocked
  gates return blockers and do not execute runtime. Eligible gates run a
  bounded controlled runtime preview with `allowWrites=false`, using a bounded
  prompt descriptor rather than raw prompt text. The returned output contains
  `shadowRunReady`, the implementation gate report, strategy/payload kind,
  metadata-safe summary, warnings, and blockers only. It may create a
  metadata-safe `controlled_migration_shadow_run` AgentRun audit, but it does
  not save assistant output to Chat history, create Proposal/Memory/LifeModel
  patch/Evidence records, or write external tool results. It does not expose
  raw user prompt, raw assistant output, or full tool payload, and default
  Send / `send_message` / `start_stream_message` do not call it.
- W29 Controlled Chat Migration Shadow Review Evidence adds
  `record_controlled_chat_migration_shadow_review_decision` and
  `get_controlled_chat_migration_shadow_review_summary`. It reads existing
  shadow AgentRuns and records explicit human `approve`, `reject`, or
  `request_rework` decisions as metadata-safe EvidenceStore records only.
  Every decision is blocked unless the AgentRun exists, has
  `reasoning_strategy=controlled_migration_shadow_run`, is completed, has
  `allowWrites=false`, has `metadataSafe=true`, and has no Chat message,
  Proposal, Memory, LifeModel patch, or external-write side effects. The
  evidence metadata whitelist is exactly `shadowRunId`, `decisionKind`,
  `reviewerNoteChecksum`, `reviewerNoteLength`, `reviewerNoteCategory`,
  `readinessSummaryDigest`, and `createdAt`; reviewer raw text, shadow prompt,
  shadow output, and tool payload are not stored. The Settings Shadow Review
  panel only triggers record/summary commands by explicit user action. Default
  Send / `send_message` / `start_stream_message` do not call shadow review
  commands.
- W30 Controlled Chat Cutover Planning Readiness Gate adds
  `check_controlled_chat_cutover_readiness`, a read-only command over current
  W27 implementation eligibility, latest W29 shadow review decision evidence,
  and the approved shadow AgentRun. It returns `cutoverPlanningEligible`, the
  implementation gate report, latest shadow review decision, verified shadow run
  id, readiness digest, `defaultChatUnchanged`, `requiredEvidenceReady`,
  blockers, and metadata-safe summary only. It blocks unless latest W29 decision
  is `approve` and the approved shadow run still exists, is completed, uses
  `reasoning_strategy=controlled_migration_shadow_run`, has `allowWrites=false`,
  has `metadataSafe=true`, and has no Chat message, Proposal, Memory,
  LifeModel patch, or external-write side effects. It creates no AgentRun,
  Evidence, Proposal, Memory, LifeModel patch, MCP audit row, or chat message;
  it does not run ReAct, PlanExecute, preview, or shadow run. The Settings
  Cutover Readiness panel only calls it on explicit click. Default Send /
  `send_message` / `start_stream_message` do not call it. Eligible means
  cutover implementation discussion only, not default Chat migration.
- W31 Non-Default Controlled Chat Cutover Candidate Adapter adds
  `run_controlled_chat_cutover_candidate`, an explicit Settings-only candidate
  command for Chat contract-shape validation. It calls W30 readiness first and
  blocks without runtime when readiness is not eligible. Eligible runs execute
  one controlled runtime candidate with `allowWrites=false`, `maxToolCalls=0`,
  no proposal apply, no Memory write, no LifeModel patch, and no external
  write. The returned output contains `candidateReady`, `candidateRunId`,
  `outputPreview` or `userOutput`, `contractShape`, metadata-safe summary,
  warnings, and blockers. It may create a metadata-safe
  `controlled_chat_cutover_candidate` AgentRun audit, but it does not save raw
  user prompts, raw assistant output, tool payload, Chat messages, Proposals,
  Memory, LifeModel patches, Evidence, MCP audit rows, or external tool
  results. The Settings Cutover Candidate panel only runs it on explicit click,
  and default Send / `send_message` / `start_stream_message` do not call it.
  Candidate ready means contract validation only, not default Chat migration.
- W32 Controlled Chat Cutover Candidate Review Evidence adds
  `record_controlled_chat_cutover_candidate_review_decision`,
  `get_controlled_chat_cutover_candidate_review_summary`, and the Settings
  Cutover Candidate Review panel. The record command reads an existing W31
  candidate AgentRun and records human `approve` / `reject` / `request_rework`
  decisions as metadata-safe EvidenceStore evidence only. Approve requires the
  AgentRun to exist, be completed, have
  `reasoning_strategy=controlled_chat_cutover_candidate`,
  `contractShape=send_message_compatible`, `candidateReady=true`,
  `allowWrites=false`, `maxToolCalls=0`, `metadataSafe=true`, and no
  Chat/Proposal/Memory/LifeModel/Evidence/MCP audit/external write side
  effects. Evidence metadata is strictly limited to `candidateRunId`,
  `decisionKind`, `contractShape`, `candidateSummaryDigest`,
  `reviewerNoteChecksum`, `reviewerNoteLength`, `reviewerNoteCategory`, and
  `createdAt`; it does not store reviewer raw text, candidate userOutput, raw
  prompt, raw assistant output, tool payload, or candidate output. The summary
  command is read-only. Default Send / `send_message` /
  `start_stream_message` do not call candidate review commands. Candidate review
  approval is evidence only, not default Chat migration.

## Work Package Status

| Work Package | Status | Code Area | Notes |
| --- | --- | --- | --- |
| W1 Tool / Proposal Hygiene | Done | `openlife-core/src/agent/action_executor/`, proposal commands, Tool Taxonomy | `calendar.propose_event` and `email.propose_draft` are P1 proposal-only governed executors; no real calendar write, email send, or `ExternalWriteAction` fallback. |
| W2 Thin Runtime Spine | Done | `openlife-core/src/agent/runtime_contract.rs`, `RuntimeInput`, `RuntimeOutput` | Shared runtime boundary exists; broad tool catalog must not imply write/external intent. |
| W3 ReAct Runtime Contract Convergence | Done | `AgentRuntime`, `AgentLoop`, runtime convergence tests | ReAct consumes HS/runtime contract pieces and remains the stable default Chat strategy. |
| W4 LifeModel Maturation Loop Foundation | Done | `maturation.rs`, `evidence_store.rs`, maturation tests | Foundations exist for events/signals/evidence, but V1 end-to-end loop is still future work. |
| W5 LifeModel Governor MVP | Done | `governor.rs`, HS policy/guidance selection | Governor/policy decisions exist for MVP domains; mature feedback loop remains incomplete. |
| W6 PlanExecute Core MVP | Done | `plan_execute.rs` | Can produce governed plan payloads; not a productized weekly-plan flow. |
| W7 Strategy Selector | Done | `strategy.rs`, selector tests | Selects ReAct vs PlanExecute/Blocked with metadata-safe summaries. |
| W8 MultiStrategy Runtime Orchestrator | Done | `multi_strategy_runtime.rs` | Orchestrates preview/core payloads; this is not a formal `RuntimeStrategy` trait. |
| W9 MultiStrategy Preview Command | Done | `src-tauri/src/commands/agent_runtime.rs`, `frontend/src/tauri.ts` | `run_multi_strategy_agent_preview` exists as non-default preview/beta command. |
| W10 MultiStrategy Preview AgentRun Audit Persistence | Done | `agent_runtime.rs`, `previewAudit.ts`, Runs/Trace UI | Writes metadata-safe outer AgentRun audit with strategy, payload, governance, warnings; ReAct inner run id is child metadata only. |
| W11 Documentation Status Sync | Done | README, AGENTS, plans | Entry docs synchronized with code status and premature Chat replacement blocked. |
| W12 Non-Default MultiStrategy Preview UI / Debug Entry | Done | Settings experimental tab, preview form tests | Settings exposes a folded preview/beta panel that calls `run_multi_strategy_agent_preview`, displays metadata-safe strategy/payload/governance/warnings, and links to Runs trace without replacing Chat. |
| W13 Guarded Chat Subpath Migration | Done | Chat governed preview panel, Chat tests | Chat exposes an explicit Governed Preview path that calls `run_multi_strategy_agent_preview` with `allowWrites=false`, displays metadata-safe runtime output, links to Runs trace, and leaves normal Send on the existing stream path. |
| W14 LifeModel Maturation Loop V1 | Done | `maturation.rs`, evidence/proposal stores, maturation tests | `MaturationService::mature_runtime_output` converts RuntimeOutput candidates into proposal-first evidence/proposals, records structured drop reasons and governance audit, and keeps evidence/report metadata-safe. |
| W15 PlanExecute Governed Vertical Slice | Done | `plan_execute.rs`, MultiStrategy PlanExecute payload, PlanExecute tests | `PlanExecuteReport` records plan id, source run id, step counts, governance summaries, read-only observations, warnings, and metadata-safe summary; write-like steps require proposal and are not executed. |
| W16 RuntimeStrategy Trait Foundation | Done | `strategy_runtime.rs`, `multi_strategy_runtime.rs`, MultiStrategy tests | Defines the lightweight `RuntimeStrategy` trait, ReAct/PlanExecute adapters, and registry-backed MultiStrategy execution while preserving ReAct/PlanExecute/Blocked payload compatibility and metadata-safe summaries. |
| W17 Runtime Integration Hardening / Chat Migration Gate | Done | `runtime_migration_gate.rs`, `agent_runtime.rs`, Tauri wrapper/tests | Adds the read-only migration gate report: default Chat unchanged, preview path healthy, metadata-safe trace ready, fallback available, no external writes, proposal-first preserved, and blocking reasons. |
| W18 Runtime Migration Gate Evidence Surface | Done | Settings experimental panel, frontend tests, docs | Displays `check_runtime_migration_gate` pass/block fields and blocking reasons as a read-only evidence surface. Normal Chat Send still does not call the gate or `run_multi_strategy_agent_preview`. |
| W19 Sustained Gate Evidence / Pilot Eligibility | Done | `runtime_migration_gate.rs`, `agent_runtime.rs`, `frontend/src/pages/settings/MultiStrategyPreviewSection.tsx`, frontend/Rust tests, docs | Adds `check_controlled_chat_pilot_eligibility`: read-only eligibility over the latest 3 preview gate reports with clean count, checked run ids, blockers, latest gate report, and default Chat unchanged. It creates no AgentRun/Proposal/Action/Observation and normal Chat Send does not call it. |
| W20 Very Small Controlled Chat Migration Pilot With Fallback | Done | `frontend/src/pages/ChatPage.tsx`, `frontend/src/pages/ChatPage.test.tsx`, docs | Adds explicit `Run Controlled Pilot` in Chat. It calls eligibility before preview, blocks without preview when ineligible, runs single-turn `run_multi_strategy_agent_preview` only when eligible with `allowWrites=false`, shows “Pilot response” separately, keeps normal Send unchanged, and performs no automatic ordinary assistant chat-history write. |
| W21 Reviewed Pilot Response Promotion | Done | `frontend/src/pages/ChatPage.tsx`, `frontend/src/pages/ChatPage.test.tsx`, docs | Adds explicit reviewed promotion for successful Controlled Pilot results with `userOutput`. Promotion requires user review/confirmation, writes one assistant chat message via `save_chat_message` with existing `run_id` metadata when available, prevents duplicate promotion for the same pilot response, and leaves normal Send / blocked / failed / no-output pilot paths unchanged. |
| W22 Post-Promotion Validation And Source Binding | Done | `frontend/src/pages/ChatPage.tsx`, `frontend/src/pages/ChatPage.test.tsx`, docs | Binds each Controlled Pilot result to its source chat session, displays source session / target session / runId / strategy / governance summary in promotion review, blocks promotion when the current target session differs from the source session, shows rerun fallback guidance, and prevents mismatch writes to a different chat session. |
| W23 Controlled Pilot Promotion Evidence Recorder | Done | `src-tauri/src/commands/agent_runtime.rs`, `frontend/src/pages/ChatPage.tsx`, `frontend/src/pages/settings/MultiStrategyPreviewSection.tsx`, frontend/Rust tests, docs | Records metadata-safe promotion evidence only after reviewed promotion saves one assistant message. Evidence includes run/session/strategy/payload/governance/length/checksum/timestamp only, is idempotent by pilotRunId/checksum, exposes a read-only Settings summary, and keeps default Send / `send_message` / `start_stream_message` isolated from eligibility/gate/preview/promotion/evidence. |
| W24 Promotion Evidence Readiness Gate | Done | `src-tauri/src/commands/agent_runtime.rs`, `src-tauri/src/lib.rs`, `frontend/src/tauri.ts`, `frontend/src/pages/settings/MultiStrategyPreviewSection.tsx`, frontend/Rust tests, docs | Adds `check_controlled_pilot_promotion_readiness` as a read-only gate over existing W23 metadata-safe promotion evidence. It defaults to 3 required promotions, surfaces pass/block counts, recent run ids, latest timestamp, mismatch block count, metadata-safe/default-chat flags, and blocking reasons in Settings. It does not migrate Chat, does not create evidence/runs/proposals/actions/observations, and does not read raw pilot response or raw user input. |
| W25 Reviewed Migration Plan Draft Generator | Done | `src-tauri/src/commands/agent_runtime.rs`, `src-tauri/src/lib.rs`, `frontend/src/tauri.ts`, `frontend/src/pages/settings/MultiStrategyPreviewSection.tsx`, frontend/Rust tests, docs | Adds `draft_controlled_chat_migration_plan` as a read-only command over W24 readiness output. Blocked readiness returns `draftReady=false` and blockers with empty plan sections; passed readiness returns human-review-only scope, preconditions, rollback, fallback, and test plan with `manualReviewRequired=true` and `notAutomaticMigration=true`. It does not replace default Chat, modify default runtime feature flags, create evidence/runs/proposals/memory/lifemodel patches, or expose raw user/assistant/tool payload content. |
| W26 Manual Migration Review Decision Evidence | Done | `src-tauri/src/commands/agent_runtime.rs`, `src-tauri/src/lib.rs`, `frontend/src/tauri.ts`, `frontend/src/pages/settings/MultiStrategyPreviewSection.tsx`, frontend/Rust tests, docs | Adds explicit approve/reject/request_rework review decision recording after W25 draft. Blocked-draft approve returns blockers and writes no evidence; ready drafts write only metadata-safe `migration_review_decision` evidence with readiness counts, draft hash, createdAt, and sanitized reviewer-note metadata. Summary is read-only and normal Send / `send_message` / `start_stream_message` do not call these commands. |
| W27 Approved Migration Implementation Gate | Done | `src-tauri/src/commands/agent_runtime.rs`, `src-tauri/src/lib.rs`, `frontend/src/tauri.ts`, `frontend/src/pages/settings/MultiStrategyPreviewSection.tsx`, frontend/Rust tests, docs | Adds `check_controlled_chat_migration_implementation_gate` as a read-only gate over W24 readiness, W25 current draft hash, and W26 metadata-safe review decision evidence. It requires the latest metadata-safe decision to be approve, blocks latest reject/request_rework, blocks draft hash mismatch, blocks current readiness failure, creates no evidence/runs/proposals/memory/lifemodel patches, and normal Send / `send_message` / `start_stream_message` do not call it. |
| W28 Non-Default Controlled Migration Shadow Run | Done | `src-tauri/src/commands/agent_runtime.rs`, `src-tauri/src/lib.rs`, `frontend/src/tauri.ts`, `frontend/src/pages/settings/MultiStrategyPreviewSection.tsx`, frontend/Rust tests, docs | Adds `run_controlled_chat_migration_shadow_run` as an explicit non-default shadow command. It calls W27 implementation gate first, blocks without runtime when ineligible, runs bounded controlled runtime preview only when eligible with `allowWrites=false`, returns metadata-safe strategy/payload/summary/warnings/blockers, may create a metadata-safe shadow AgentRun audit, and writes no Chat message, Proposal, Memory, LifeModel patch, Evidence, or external tool result. Normal Send / `send_message` / `start_stream_message` do not call it. |
| W29 Controlled Chat Migration Shadow Review Evidence | Done | `src-tauri/src/commands/agent_runtime.rs`, `src-tauri/src/lib.rs`, `frontend/src/tauri.ts`, `frontend/src/pages/settings/MultiStrategyPreviewSection.tsx`, frontend/Rust tests, docs | Adds explicit `approve`/`reject`/`request_rework` review evidence for existing shadow runs. Every decision is blocked unless the shadow AgentRun is completed, write-disabled, metadata-safe, and side-effect-free. Evidence stores only shadowRunId, decisionKind, reviewer-note checksum/length/category, readiness digest, and createdAt. Summary is read-only and normal Send / `send_message` / `start_stream_message` do not call shadow review commands. This is review evidence, not Chat migration. |
| W30 Controlled Chat Cutover Planning Readiness Gate | Done | `src-tauri/src/commands/agent_runtime.rs`, `src-tauri/src/lib.rs`, `frontend/src/tauri.ts`, `frontend/src/pages/settings/MultiStrategyPreviewSection.tsx`, frontend/Rust tests, docs | Adds read-only `check_controlled_chat_cutover_readiness`. It requires current W27 eligible, latest W29 shadow review approve, and the approved shadow AgentRun to still be completed/write-disabled/metadata-safe/side-effect-free. It returns metadata-safe readiness fields and blockers only, creates no records, runs no runtime, and normal Send / `send_message` / `start_stream_message` do not call it. This is cutover planning readiness for implementation discussion, not default Chat migration. |
| W31 Non-Default Controlled Chat Cutover Candidate Adapter | Done | `src-tauri/src/commands/agent_runtime.rs`, `src-tauri/src/lib.rs`, `frontend/src/tauri.ts`, `frontend/src/pages/settings/MultiStrategyPreviewSection.tsx`, frontend tests, Rust tests, docs | Adds explicit `run_controlled_chat_cutover_candidate`. It calls W30 readiness first, blocks without runtime when ineligible, and only then runs one controlled runtime candidate with `allowWrites=false`, `maxToolCalls=0`, no proposal apply, no Memory write, no LifeModel patch, and no external write. It returns Chat-compatible contract-shape fields and metadata-safe summary, may create metadata-safe candidate AgentRun audit, and writes no Chat/Proposal/Memory/LifeModel/Evidence/MCP audit/external tool result. Normal Send / `send_message` / `start_stream_message` do not call it. This is a non-default candidate adapter, not default Chat migration. |
| W32 Controlled Chat Cutover Candidate Review Evidence | Done | `src-tauri/src/commands/agent_runtime.rs`, `src-tauri/src/lib.rs`, `frontend/src/tauri.ts`, `frontend/src/pages/settings/MultiStrategyPreviewSection.tsx`, frontend tests, Rust tests, docs | Adds explicit `record_controlled_chat_cutover_candidate_review_decision` and read-only summary. Approve requires a completed, ready, send_message-compatible, write-disabled, zero-tool, metadata-safe, side-effect-free candidate AgentRun. Evidence stores only candidateRunId, decisionKind, contractShape, candidateSummaryDigest, reviewer-note checksum/length/category, and createdAt; it stores no reviewer raw text, candidate output, raw prompt/output, or tool payload. Normal Send / `send_message` / `start_stream_message` do not call it. This is candidate review evidence, not default Chat migration. |

## Next Recommended Sequence

```text
use cutover candidate review only for explicit human evidence; default Chat remains unchanged
```

The next phase still must not directly replace the default Chat path. W21 only
added an explicit review-and-confirm promotion step for successful pilot output,
W22 only added source binding plus target-session validation for that promotion
step, W23 only records/reads metadata-safe promotion evidence, W24 only checks
readiness for discussing the next migration step, W25 only generates a
read-only human-review draft, W26 only records metadata-safe manual review
decision evidence, W27 only checks whether current evidence qualifies for
implementation discussion, W28 only runs a non-default write-disabled shadow
comparison after W27 eligibility, W29 only records metadata-safe human shadow
review evidence, W30 only checks cutover planning readiness for entering
implementation discussion, W31 only runs an explicit non-default cutover
candidate for contract-shape validation after W30 eligibility, and W32 only
records metadata-safe human candidate review evidence. Default `Send`,
`send_message`, and `start_stream_message` remain unchanged until a later
reviewed migration stage with separate implementation work and explicit human
approval.

`make ci` remains the release gate for every implementation task, including
documentation-only status syncs.
