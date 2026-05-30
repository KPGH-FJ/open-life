# LifeModel-Governed Runtime Progress

> Last updated: 2026-05-30
> Status: compact progress index, not a second roadmap

This file summarizes implementation status for Agents entering the project. It
does not replace `openlife_lifemodel_governed_agent_runtime.md`; when planning
next work, use the program document for ordering and this file only as a
completion/status index.

## Current Position

W1-W15 are complete. The project now has a governed PlanExecute V1 vertical
slice with metadata-safe reports, internal read-only observations, and
Governor-enforced write interception.

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
- A formal `RuntimeStrategy` trait has not started and must not be introduced
  ahead of proven vertical slices.

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

## Next Recommended Sequence

```text
RuntimeStrategy trait
```

`make ci` remains the release gate for every implementation task, including
documentation-only status syncs.
