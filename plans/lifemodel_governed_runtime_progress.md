# LifeModel-Governed Runtime Progress

> Last updated: 2026-06-02
> Status: W68 backend-only default Chat adapter send-compatible proof complete

This file is the compact completion/status index for Agents entering the
LifeModel-Governed Runtime work. It does not replace
`plans/openlife_lifemodel_governed_agent_runtime.md`; use that program document
for implementation order, and use this file to avoid re-reading stale long
route text.

## Current Position

Current latest status is **W68 backend-only send-compatible proof complete**.
W61-W64 were documentation/index整理 and authority compression stages only. W65
adds a pure Rust descriptor mapper in `src-tauri/src/default_chat_adapter.rs`
for a future controlled adapter candidate contract. W66 adds a pure Rust
controlled adapter contract report/evaluator/ensure over that descriptor. W67
adds a pure Rust backend-only non-default invocation harness that reads/reuses
only the W66 contract report and proves the future controlled adapter candidate
invocation shape is metadata-safe, zero-side-effect, and executor
disabled/unattached. W68 adds a pure Rust backend-only send-compatible
proof/evaluator/ensure that reads/reuses only W65 descriptor, W66 contract, and
W67 harness metadata to prove the controlled adapter candidate can map to a
SendMessageResult-compatible metadata-safe shape. It allows only the SendMessage
callsite to become proof ready; stream callsites fail closed. W65-W68 add no
command, no frontend surface, no Settings surface, no runtime/model/tool call,
no store write, no executor attachment, and no default Chat routing change.

Hard boundaries:

- default Chat remains `legacy_stream`.
- Default `Send`, ordinary `send_message`, and ordinary
  `start_stream_message` may enter only the legacy route, with the W49-W55 pure
  guards/preflight allowed to fail closed.
- W19-W60 readiness/review/preview/gate results are not migration permission.
- W65-W68 backend-only descriptor/contract/harness/proof work is not migration
  permission and must keep the controlled adapter executor disabled/unattached.
  W67 `harness_ready` only means the non-default invocation shape proof is
  safe; W68 `proof_ready` only means the SendMessageResult-compatible metadata
  shape proof is safe, not that default Chat may migrate.
- Ordinary `send_message` / `start_stream_message` must not call any W19-W60
  command surface.
- Ordinary `send_message` / `start_stream_message` must not call the W67
  non-default invocation harness.
- Ordinary `send_message` / `start_stream_message` must not call the W68
  send-compatible proof.
- W61-W63 are docs/index整理 only and cannot affect default Chat.

## Authority And Conflict Rule

When old plans conflict, use this order:

1. `AGENTS.md`
2. `plans/README.md`
3. `plans/openlife_lifemodel_governed_agent_runtime.md`
4. This W1-W68 progress index
5. Historical/reference plans

If a historical paragraph says a readiness, approval, draft, preview, or gate
authorizes migration, treat that paragraph as stale. The current boundary is:
readiness means discussion or review eligibility only; it is not migration
permission.

## Safety Legend

- `RO`: read-only.
- `WD`: write-disabled.
- `MS`: metadata-safe.
- `Pure guard`: local guard/preflight only; no runtime/model/tool/business
  write.
- `Docs`: documentation/index整理 only.
- `Default Chat impact`: whether the stage may change ordinary default Chat
  behavior. `No` means no routing change and no migration permission.

## W1-W68 Structured Index

| Stage | Name | Status | Command/surface type | Safety | Default Chat impact | Next dependency |
| --- | --- | --- | --- | --- | --- | --- |
| W1 | Tool / Proposal Hygiene | Done | Core tool policy | Proposal-only governed executors | No | W2 |
| W2 | Thin Runtime Spine | Done | Runtime contract foundation | Metadata-safe runtime boundary | No | W3 |
| W3 | ReAct Runtime Contract Convergence | Done | Runtime convergence | ReAct remains stable legacy default | Keeps legacy default | W4 |
| W4 | LifeModel Maturation Loop Foundation | Done | LifeModel/evidence foundation | Governed evidence foundation | No | W5 |
| W5 | LifeModel Governor MVP | Done | Governor/policy foundation | Policy-guided, proposal-first direction | No | W6 |
| W6 | PlanExecute Core MVP | Done | Runtime implementation | Governed plan payloads | No | W7 |
| W7 | Strategy Selector | Done | Runtime selector | Metadata-safe strategy summaries | No | W8 |
| W8 | MultiStrategy Runtime Orchestrator | Done | Runtime orchestrator | Preview/core payload orchestration | No | W9 |
| W9 | MultiStrategy Preview Command | Done | Non-default preview command | WD / MS preview command | No | W10 |
| W10 | MultiStrategy Preview AgentRun Audit Persistence | Done | Preview audit | MS outer AgentRun audit | No | W11 |
| W11 | Documentation Status Sync | Done | Docs | Docs sync only | No | W12 |
| W12 | Non-Default MultiStrategy Preview UI / Debug Entry | Done | Settings preview surface | WD / MS explicit debug surface | No | W13 |
| W13 | Guarded Chat Subpath Migration | Done | Explicit governed preview subpath | WD / MS, normal Send unchanged | No ordinary path change | W14 |
| W14 | LifeModel Maturation Loop V1 | Done | Service entry | Proposal-first, metadata-safe audit | No | W15 |
| W15 | PlanExecute Governed Vertical Slice | Done | Governed runtime slice | Read-only observations; write-like steps require proposal | No | W16 |
| W16 | RuntimeStrategy Trait Foundation | Done | Adapter/registry foundation | Compatibility-preserving summaries | No | W17 |
| W17 | Runtime Integration Hardening / Chat Migration Gate | Done | `check_runtime_migration_gate` | RO / MS diagnostic | No | W18 |
| W18 | Runtime Migration Gate Evidence Surface | Done | Settings evidence surface | RO / MS display | No | W19 |
| W19 | Sustained Gate Evidence / Pilot Eligibility | Done | `check_controlled_chat_pilot_eligibility` | RO / MS; no new AgentRun/Proposal/Action/Observation | No; not migration permission | W20 |
| W20 | Very Small Controlled Chat Migration Pilot With Fallback | Done | Explicit Chat pilot button | WD / MS, `allowWrites=false` | No ordinary Send impact | W21 |
| W21 | Reviewed Pilot Response Promotion | Done | Explicit review/confirm promotion | User-confirmed single chat write only | No routing impact | W22 |
| W22 | Post-Promotion Validation And Source Binding | Done | Promotion validation surface | Source/target session bound | No routing impact | W23 |
| W23 | Controlled Pilot Promotion Evidence Recorder | Done | Evidence recorder + summary | MS evidence only | No | W24 |
| W24 | Promotion Evidence Readiness Gate | Done | `check_controlled_pilot_promotion_readiness` | RO / MS | No; not migration permission | W25 |
| W25 | Reviewed Migration Plan Draft Generator | Done | `draft_controlled_chat_migration_plan` | RO / MS human-review draft | No; not migration permission | W26 |
| W26 | Manual Migration Review Decision Evidence | Done | Review decision record + summary | MS evidence; blocked approve writes no evidence | No; approval is not migration permission | W27 |
| W27 | Approved Migration Implementation Gate | Done | `check_controlled_chat_migration_implementation_gate` | RO / MS gate | No; eligibility is discussion only | W28 |
| W28 | Non-Default Controlled Migration Shadow Run | Done | Explicit shadow command | WD / MS; may create MS shadow AgentRun | No; ordinary entries do not call it | W29 |
| W29 | Controlled Chat Migration Shadow Review Evidence | Done | Review evidence record + summary | MS evidence over existing safe shadow run | No; not migration permission | W30 |
| W30 | Controlled Chat Cutover Planning Readiness Gate | Done | `check_controlled_chat_cutover_readiness` | RO / MS | No; planning readiness only | W31 |
| W31 | Non-Default Controlled Chat Cutover Candidate Adapter | Done | Explicit candidate command | WD / zero-tool / MS candidate | No; non-default only | W32 |
| W32 | Controlled Chat Cutover Candidate Review Evidence | Done | Review evidence record + summary | MS evidence over safe candidate | No; not migration permission | W33 |
| W33 | Controlled Chat Cutover Candidate Promotion Readiness Gate | Done | `check_controlled_chat_cutover_candidate_promotion_readiness` | RO / MS | No; implementation-planning readiness only | W34 |
| W34 | Default Chat Runtime Boundary Status | Done | `get_default_chat_runtime_boundary_status` | RO / MS boundary observability | No; reports `legacy_stream` | W35 |
| W35 | Default Chat Adapter Activation Plan Draft | Done | `draft_default_chat_adapter_activation_plan` | RO / MS human-review draft | No; activation planning only | W36 |
| W36 | Default Chat Adapter Activation Review Decision Evidence | Done | Review evidence record + summary | MS evidence; blocked approve writes no evidence | No; approval is not migration permission | W37 |
| W37 | Default Chat Adapter Activation Implementation Gate | Done | `check_default_chat_adapter_activation_implementation_gate` | RO / MS gate | No; separate implementation discussion only | W38 |
| W38 | Default Chat Adapter Disabled Routing Scaffold | Done | `get_default_chat_adapter_routing_status` | RO / MS routing status | No; reports disabled adapter and `legacy_stream` | W39 |
| W39 | Default Chat Adapter Contract Harness | Done | `check_default_chat_adapter_contract_harness` | RO / MS contract check | No; ordinary entries do not call it | W40 |
| W40 | Default Chat Adapter Dry-Run Invocation Boundary | Done | Explicit dry-run command | WD / zero-tool / MS result | No; non-default dry run only | W41 |
| W41 | Default Chat Adapter Dry-Run Review Evidence | Done | Review evidence record + summary | MS evidence; blocked approve writes no evidence | No; not migration permission | W42 |
| W42 | Default Chat Adapter Implementation Readiness Gate | Done | `check_default_chat_adapter_implementation_readiness` | RO / MS gate | No; readiness only | W43 |
| W43 | Default Chat Adapter Controlled Preview | Done | Explicit controlled preview command | WD / zero-tool / MS; may create MS preview AgentRun | No; non-default only | W44 |
| W44 | Default Chat Adapter Controlled Preview Review Evidence | Done | Review evidence record + summary | MS evidence over safe preview | No; approval is not migration permission | W45 |
| W45 | Default Chat Adapter Controlled Preview Approval Readiness Gate | Done | `check_default_chat_adapter_controlled_preview_approval_readiness` | RO / MS gate | No; approval readiness only | W46 |
| W46 | Default Chat Adapter Cutover Implementation Plan Draft | Done | `draft_default_chat_adapter_cutover_implementation_plan` | RO / MS human-review draft | No; planning only | W47 |
| W47 | Default Chat Adapter Cutover Plan Review Evidence | Done | Review evidence record + summary | MS evidence; blocked approve writes no evidence | No; not migration permission | W48 |
| W48 | Default Chat Adapter Cutover Plan Approval Readiness Gate | Done | `check_default_chat_adapter_cutover_plan_approval_readiness` | RO / MS gate | No; implementation-discussion readiness only | W49 |
| W49 | Default Chat Adapter Cutover Route Guard Scaffold | Done | Pure ordinary-entry route guard | Pure guard / fail-closed / MS status | Guard only; route stays `legacy_stream` | W50 |
| W50 | Default Chat Adapter Cutover Invocation Harness | Done | Pure ordinary-entry harness | Pure guard / WD / zero-tool / no runtime/model/tool/write | Guard only; route stays `legacy_stream` | W51 |
| W51 | Default Chat Adapter Invocation Plan | Done | Pure ordinary-entry invocation plan | Pure guard; selects `legacy_stream`; controlled adapter disabled | Guard only; route stays `legacy_stream` | W52 |
| W52 | Default Chat Adapter Invocation Boundary | Done | Pure ordinary-entry boundary | Pure guard; side-effect-free before legacy entry | Guard only; route stays `legacy_stream` | W53 |
| W53 | Default Chat Adapter Typed Callsite Contract | Done | Pure typed send/stream callsite contract | Pure guard; send/stream bound to legacy route path | Guard only; route stays `legacy_stream` | W54 |
| W54 | Authority Roadmap Sync | Done | Docs | Docs sync only | No | W55 |
| W55 | Default Chat Adapter Ordinary Entry Preflight | Done | Pure ordinary-entry preflight | Pure guard; typed contract ready, executor unattached, migration disabled, zero pre-entry budget | Guard only; route stays `legacy_stream` | W56 |
| W56 | Default Chat Adapter Ordinary Entry Preflight Status | Done | `get_default_chat_adapter_ordinary_entry_preflight_status` | RO / MS Settings status | No; ordinary entries must not call it | W57 |
| W57 | Default Chat Adapter Narrow Implementation Discussion Gate | Done | `check_default_chat_adapter_narrow_implementation_discussion_gate` | RO / MS gate over W48/W56 | No; discussion eligibility only | W58 |
| W58 | Default Chat Adapter Narrow Implementation Plan Draft | Done | `draft_default_chat_adapter_narrow_implementation_plan` | RO / MS human-review draft | No; `draftReady` is not migration permission | W59 |
| W59 | Default Chat Adapter Narrow Implementation Plan Review Evidence | Done | Review evidence record + summary | MS evidence; blocked approve writes no evidence | No; approval is not migration permission | W60 |
| W60 | Default Chat Adapter Narrow Implementation Plan Approval Readiness Gate | Done | `check_default_chat_adapter_narrow_implementation_plan_approval_readiness` | RO / MS gate | No; ready is not migration permission | W61 |
| W61 | Progress Index Compression Prep | Done | Docs/index surface | Docs only | No | W62 |
| W62 | Plans README Authority Compression | Done | Docs/index surface | Docs only | No | W63 |
| W63 | Narrow Adapter Implementation Entry Index Freeze | Done | Docs/index surface | Docs only | No; prepares future implementation only | W64 |
| W64 | W1-W63 Authority Compression Validation | Done | Docs/index surface | Docs only | No | W65 |
| W65 | Default Chat Adapter Backend-Only Descriptor Skeleton | Done | Pure internal mapper in `default_chat_adapter.rs` | MS descriptor only; input length/hash, route metadata, disabled/unattached executor, zero side-effect budget | No; ordinary send/stream stay `legacy_stream` | W66 |
| W66 | Default Chat Adapter Controlled Contract Report | Done | Pure internal contract evaluator in `default_chat_adapter.rs` | MS report only; descriptor readiness, send/stream contract shape, disabled/unattached executor, zero side-effect budget, migration permission false | No; ordinary send/stream stay `legacy_stream` | W67 |
| W67 | Default Chat Adapter Non-Default Controlled Invocation Harness | Done | Pure internal harness in `default_chat_adapter.rs` | MS harness only; reads W66 report, input length/hash only, executor disabled/unattached, zero side-effect budget, migration permission false | No; ordinary send/stream stay `legacy_stream` and do not call it | W68 |
| W68 | Default Chat Adapter Send-Compatible Contract Proof | Done | Pure internal proof/evaluator in `default_chat_adapter.rs` | MS send-compatible proof only; reads W65/W66/W67 metadata, SendMessage only ready, stream fail-closed, executor disabled/unattached, zero side-effect budget, migration permission false | No; ordinary send/stream stay `legacy_stream` and do not call it | Future controlled adapter implementation discussion only |

## Folded Boundary Summary

The old W20-W60 long-form route text is intentionally folded into the table
above. The boundary meaning is preserved:

- Readiness, review approval, preview success, draft readiness, cutover
  readiness, implementation readiness, and approval readiness are evidence or
  discussion gates only.
- W28, W31, W40, and W43 are explicit non-default commands. They are
  write-disabled, zero-tool where required, metadata-safe, and must not be
  called by ordinary Chat entries.
- W49-W55 may sit on the ordinary-entry path only as pure fail-closed guards.
  They may verify the route and block drift; they may not switch default Chat.
- W56-W60 are Settings/status/draft/review/readiness surfaces and ordinary
  `send_message` / `start_stream_message` must not call them.
- W61-W63 are documentation/index整理, not migration permission, not code work,
  and not default Chat migration.
- W65-W68 descriptor/contract/harness/proof work is internal backend code only.
  It may describe and validate a future controlled adapter candidate with
  metadata-safe fields, a non-default invocation shape proof, and a
  SendMessageResult-compatible metadata shape proof, but it must not execute or
  attach that adapter, run runtime/model/tool, write business records, or change
  default Chat routing.

## Next Recommended Sequence

```text
W63 complete -> W64 authority compression validated -> W65 backend-only
descriptor skeleton complete -> W66 controlled adapter contract report complete
-> W67 non-default invocation harness complete -> W68 send-compatible proof
complete -> future controlled adapter implementation discussion may build on
the proof only through a separately reviewed task; keep default Chat on
legacy_stream unless that separate task explicitly implements, reviews,
verifies, and authorizes a route change.
```

`make ci` remains the release gate for implementation tasks. For docs-only
index整理, `git diff --check` plus targeted `rg` validation is sufficient unless
code or package configuration changes.
