# OpenLife Plans Document Governance

> Last updated: 2026-06-02
> Status: authoritative document index for Agents, W77 accepted rule selection proof complete

This file prevents old planning documents from steering new Agent work. If two
documents disagree, use the precedence below and treat lower-priority stale text
as reference only.

## 1. Precedence

1. `AGENTS.md`
   - Project-wide Agent instructions, current constraints, and Tool Taxonomy.
2. `plans/README.md`
   - This authority map and current entry point.
3. `plans/openlife_lifemodel_governed_agent_runtime.md`
   - Current implementation program and next development order.
4. `plans/lifemodel_governed_runtime_progress.md`
   - Compact W1-W77 completion/status index. This is not a second roadmap.
5. `plans/lifemodel_maturation_goal_plan.md`
   - Current Goal-mode preparation plan for LifeModel Maturation Loop
     End-to-End after W72.
6. Hard governance baselines:
   - `plans/adr/0013-lifemodel-hs-source-of-truth-governance.md`
   - `plans/openlife_react_beta_roadmap.md`
   - `plans/lifemodel_hs_mvp_task_specs.md`
   - `plans/lifemodel_hs_legacy_write_path_audit.md`
7. Scoped architecture/product baselines:
   - `plans/openlife_agent_framework_architecture.md`
   - `OpenLife_PRD_v2_Agent_Framework.md`
8. Current execution helpers:
   - `plans/openlife_development_plan.md`
   - `plans/openlife_codex_execution_playbook.md`
9. Historical/reference documents.
   - Useful for context, but never authoritative for current task order.

## 2. Current Position

Current latest status is **W77 accepted rule selection proof complete**.
W64 validated the compressed W1-W63 authority/index entry. W65 adds a pure Rust
descriptor mapper in `src-tauri/src/default_chat_adapter.rs` for a future
controlled adapter candidate contract. W66 adds a pure Rust controlled adapter
contract report/evaluator/ensure over that descriptor. W67 adds a pure Rust
backend-only non-default invocation harness that reads/reuses only the W66
contract report and proves the future controlled adapter candidate invocation
shape is metadata-safe, zero-side-effect, and executor-disabled/unattached.
W68 adds a pure Rust backend-only send-compatible proof/evaluator/ensure that
reads/reuses only W65 descriptor, W66 contract, and W67 harness metadata to
prove the controlled adapter candidate can map to a SendMessageResult-compatible
metadata-safe shape. It allows only the SendMessage callsite to become proof
ready; stream callsites fail closed. W69 adds a pure Rust backend-only
stream-compatible boundary proof/evaluator/ensure that reads/reuses only W65
descriptor, W66 contract, and W67 harness metadata to prove the controlled
adapter candidate can form a `start_stream_message`-compatible metadata
boundary. It allows only the StartStreamMessage callsite to become proof ready;
SendMessage fails closed with `callsite_not_start_stream_message`. W69 does not
emit a real stream, open an event channel, attach an executor, or authorize a
route cutover. W70 adds a pure Rust backend-only executor attachment gate
report/evaluator/ensure that simultaneously reuses W65-W67 metadata-safe
descriptor/contract/harness results, the W68 send-compatible proof, and the W69
stream-compatible boundary proof. W70 can report that the proof stack is
metadata-ready for the next executor skeleton discussion, but it keeps
executor_attachment_allowed=false, executor_attached=false,
executor_enabled=false, route_cutover_permission=false, and
migrationPermission=false. Executor implementation missing, human review
missing, and route cutover not authorized remain explicit blockers. W65-W70 add
no command, no frontend change, no Settings surface, no runtime/model/tool call,
no store write, no executor attachment, and no routing change.
W71 adds a pure Rust backend-only disabled controlled executor skeleton
contract/evaluator/ensure in `src-tauri/src/default_chat_adapter.rs`. It reuses
the W70 gate report and stores only metadata-safe callsite, route metadata,
input length/hash, and requested shape. Known send/stream shapes return
metadata-only placeholders; unknown shapes fail closed. W71 fixes
executor_skeleton_present=true, executor_enabled=false, executor_attached=false,
executor_runnable=false, invocation_allowed=false,
route_cutover_permission=false, and migrationPermission=false. W71 adds no
command, no frontend change, no Settings surface, no runtime/model/tool call,
no stream emission, no event channel, no business write, no executor
attachment, no route cutover, and no migration permission.
W72 adds a pure Rust backend-only disabled skeleton binding integrity
report/evaluator/ensure in `src-tauri/src/default_chat_adapter.rs`. It reuses
the W71 disabled skeleton, W71 skeleton input, and W70 gate report to verify
that input length/hash, route metadata, requested shape/callsite, skeleton
output shape, legacy route metadata, gate metadata, and disabled/no-run/no-write/no-stream
constraints are bound consistently. W72 keeps executor_enabled=false,
executor_attached=false, executor_runnable=false, invocation_allowed=false,
route_cutover_permission=false, migrationPermission=false, and
selected_adapter_path=legacy_stream. W72 is not executor implementation, not
executor attachment, not route cutover, and not migration permission.
W73 adds a pure core LifeModel maturation readiness report/evaluator/ensure in
`openlife-core/src/agent/maturation.rs`. It validates only the narrow
low-energy / low-pressure planning preference domain, checks that candidate
metadata is safe, proposal-first, source-lineage-ready, and does not require
direct LifeModel/Memory/Heuristic writes, keeps a zero side-effect budget, and
returns `nextAllowedStep=non_default_maturation_invocation` only when clean.
W73 adds no command, no frontend surface, no runtime/model/tool call, no
Evidence/Proposal/LifeModel/Memory/Heuristic/Chat/MCP audit/external write, no
ordinary Chat auto-maturation, and no default Chat route change.
W74 adds a pure core explicit non-default LifeModel maturation invocation
harness/report in `openlife-core/src/agent/maturation.rs`. It must call W73
readiness first; when readiness is blocked it writes no stores, and when ready
it only writes governed candidate EvidenceStore records and pending
ProposalStore records. W74 keeps no runtime/model/tool execution, no
LifeModel/Memory/Heuristic/Chat/AgentRun/MCP audit/external write, no Tauri
command, no frontend surface, no ordinary Chat auto-maturation, and no default
Chat route change.
W75 adds `openlife-core/src/agent/proposal_outcome.rs` with
`MaturationProposalOutcome`, `MaturationProposalOutcomeEvidenceReport`,
`evaluate_maturation_proposal_outcome_evidence`, and
`record_maturation_proposal_outcome_evidence`. It minimally wires
`src-tauri/src/commands/proposal.rs` after successful proposal accept/reject/edit
state updates. Only maturation lineage proposals record metadata-safe
`ProposalOutcome` evidence; rejected proposals record negative/opposing outcome
evidence, edited proposals do not persist raw edited payload in the outcome
report/evidence, and non-maturation proposals no-op. W75 does not add a
command/frontend surface, does not run runtime/model/tool, does not change
default Chat, and is not a maturation runtime migration.
W76 adds pure core low-energy collaboration rule candidate aggregation in
`openlife-core/src/agent/maturation.rs` with
`LowEnergyCollaborationRuleCandidateInput`,
`LowEnergyCollaborationRuleCandidateReport`,
`evaluate_low_energy_collaboration_rule_candidate`, and
`propose_low_energy_collaboration_rule_candidate`. It aggregates only
metadata-safe accepted/edited/rejected maturation ProposalOutcome evidence,
preserves accepted/rejected/edited outcome evidence ids, source evidence ids,
linked proposal ids, and linked AgentRun ids, and opposing/negative evidence
blocks or weakens repeated similar candidate rules. When ready, W76 may write
only a pending ProposalStore candidate proposal; it does not activate a
Heuristic, does not write active rules, adds no command/frontend surface, runs
no runtime/model/tool, writes no LifeModel/Memory/Heuristic truth, and does not
affect default Chat.
W77 adds pure core accepted low-energy rule selection proof in
`openlife-core/src/agent/maturation.rs` with
`AcceptedLowEnergyRuleSelectionInput`,
`AcceptedLowEnergyRuleSelectionReport`,
`AcceptedLowEnergyRuleSelectionHSPacketAuditProof`,
`evaluate_accepted_low_energy_rule_selection`, and
`ensure_accepted_low_energy_rule_selection`. It selects only user-accepted W76
candidate proposals into a future RuntimeHSPacket metadata-safe planning
guidance proof, preserves outcome evidence / proposal / AgentRun lineage, and
fails closed for pending/rejected/non-W76 proposals, non-planning tasks, and
non-low-energy domains. If privacy policy or an existing packet requires
LocalOnly, W77 keeps or strengthens that route; the rule cannot override or
relax privacy/model route policy. W77 adds no command/frontend surface, runs no
runtime/model/tool, writes no LifeModel/Memory/Heuristic truth, does not
activate a Heuristic, and does not affect default Chat.

Any next controlled adapter work must arrive through a separate task that
explicitly asks for it and preserves default Chat `legacy_stream` until a
reviewed route change is implemented and verified.

The next active Goal-mode preparation entry is
`plans/lifemodel_maturation_goal_plan.md`. It starts LifeModel Maturation Loop
End-to-End with a narrow low-energy / low-pressure planning domain. That Goal
must not migrate default Chat, attach the controlled adapter executor, directly
write LifeModel/Memory/Heuristic truth, or bypass proposal-first governance.
After W77, the next allowed slice is W78 run trace visibility.

Hard current constraints:

- default Chat remains `legacy_stream`.
- W19-W60 readiness/review/preview/gate outputs are not migration permission.
- W61-W63 are整理阶段, not default Chat migration.
- Ordinary `send_message` / `start_stream_message` must not call W19-W60
  command surfaces.
- Ordinary `send_message` / `start_stream_message` must not call the W67
  non-default invocation harness.
- Ordinary `send_message` / `start_stream_message` must not call the W68
  send-compatible proof.
- Ordinary `send_message` / `start_stream_message` must not call the W69
  stream-compatible boundary proof.
- Ordinary `send_message` / `start_stream_message` must not call the W70
  executor attachment gate.
- Ordinary `send_message` / `start_stream_message` must not call the W71
  disabled executor skeleton.
- Ordinary `send_message` / `start_stream_message` must not call the W72
  skeleton binding integrity report.
- Ordinary `send_message` / `start_stream_message` must not call the W73
  LifeModel maturation readiness report.
- Ordinary `send_message` / `start_stream_message` must not call the W74
  non-default LifeModel maturation invocation.
- Ordinary `send_message` / `start_stream_message` must not call the W75
  proposal outcome evidence helper.
- Ordinary `send_message` / `start_stream_message` must not call the W76
  low-energy collaboration rule candidate helper.
- Ordinary `send_message` / `start_stream_message` must not call the W77
  accepted low-energy rule selection helper.
- Ordinary default Chat may call only the W49-W55 pure ordinary-entry guards /
  preflight, and those guards may only fail closed while preserving
  `legacy_stream`.
- W65-W72 backend-only descriptor/contract/harness/proof/gate/skeleton/binding work is metadata only
  and is not migration permission. W67 `harness_ready` only means the
  non-default invocation shape proof is safe; W68 `proof_ready` only means the
  SendMessageResult-compatible metadata shape proof is safe; W69 `proof_ready`
  only means the stream-compatible metadata boundary proof is safe; W70
  `gate_report_metadata_ready` only means the attachment gate report metadata is
  ready for executor skeleton discussion; W71 `skeleton_contract_ready` only
  means the disabled skeleton contract metadata is safe and still no-run; W72
  `binding_integrity_ready` only means the disabled skeleton binding metadata is
  internally consistent and still no-run.

## 3. W1-W77 Compression Map

For the row-level structured index, use
`plans/lifemodel_governed_runtime_progress.md`. It lists every stage with:
stage id, name, status, command/surface type, read-only/write-disabled/
metadata-safe safety, default Chat impact, and next dependency.

| Range | Compressed meaning | Default Chat authority |
| --- | --- | --- |
| W1-W8 | Runtime, LifeModel, Strategy, and MultiStrategy foundations | No migration authority |
| W9-W18 | Non-default preview, preview audit, and migration gate evidence surfaces | No migration authority |
| W19-W23 | Controlled pilot eligibility, explicit pilot, reviewed promotion, source binding, promotion evidence | Explicit pilot/promotion only; ordinary Send unchanged |
| W24-W27 | Promotion readiness, migration plan draft, review evidence, implementation gate | Readiness/approval is discussion only, not migration permission |
| W28-W33 | Shadow run/review, cutover readiness, candidate adapter/review, candidate promotion readiness | Non-default write-disabled validation only |
| W34-W42 | Default Chat boundary, activation plan/review/gate, disabled routing, contract harness, dry run/review, implementation readiness | Read-only or non-default evidence only |
| W43-W48 | Controlled preview/review/readiness and cutover implementation plan/review/readiness | Non-default preview and planning only |
| W49-W55 | Route guard, invocation harness/plan/boundary, typed callsite contract, ordinary-entry preflight | Pure fail-closed guard only; route stays `legacy_stream` |
| W56-W60 | Ordinary-entry status, narrow discussion gate, narrow plan draft/review/approval readiness | Settings/status/planning only; ordinary entries must not call commands |
| W61-W64 | Docs/index整理, W1-W63 compression freeze, and authority validation | Docs only; no default Chat effect |
| W65 | Backend-only controlled adapter descriptor skeleton | Internal metadata-safe mapper only; no default Chat effect |
| W66 | Backend-only controlled adapter contract report | Internal metadata-safe contract evaluator only; no default Chat effect |
| W67 | Backend-only non-default controlled invocation harness | Internal metadata-safe shape proof only; no command, executor, runtime, write, routing, or default Chat effect |
| W68 | Backend-only send-compatible contract proof | Internal SendMessageResult-compatible metadata proof only; stream fails closed; no command, executor, runtime, write, routing, or default Chat effect |
| W69 | Backend-only stream-compatible boundary proof | Internal `start_stream_message`-compatible metadata boundary proof only; SendMessage fails closed; no real stream, event channel, command, executor, runtime, write, routing, or default Chat effect |
| W70 | Backend-only executor attachment gate report | Internal metadata-ready gate report only; executor attachment/cutover/migration permission all false; no command, executor, runtime, write, routing, or default Chat effect |
| W71 | Backend-only disabled executor skeleton contract | Internal metadata-only placeholder contract only; executor disabled/unattached/not runnable, invocation disallowed, no stream/event channel, no command, runtime, write, routing, or default Chat effect |
| W72 | Backend-only disabled skeleton binding integrity report | Internal metadata binding report only; verifies W71 input/skeleton and W70 gate consistency, no executor implementation/attachment/cutover/migration permission, no command, runtime, write, routing, or default Chat effect |
| W73 | LifeModel maturation readiness report | Pure core metadata-safe readiness report only; low-energy planning domain, proposal-first, no writes, no command, no ordinary Chat effect |
| W74 | LifeModel non-default maturation invocation | Pure core explicit invocation only; calls W73 first, blocked writes no stores, ready writes EvidenceStore + ProposalStore only, no command, no ordinary Chat effect |
| W75 | Proposal outcome evidence link | Core helper plus minimal proposal accept/reject/edit internal wiring; writes metadata-safe ProposalOutcome evidence only for maturation lineage proposals; no command/frontend/runtime/default Chat effect |
| W76 | Low-energy collaboration rule candidate | Pure core evaluator/proposer only; aggregates metadata-safe ProposalOutcome evidence into a pending candidate proposal, blocks/weakens on opposing evidence, no active Heuristic/rule, no command/frontend/runtime/default Chat effect |
| W77 | Accepted rule to RuntimeHSPacket selection proof | Pure core evaluator/report/ensure only; accepted W76 candidate proposal, planning task, low-energy domain, metadata-safe guidance, lineage retained, privacy/model route policy not relaxed, no command/frontend/runtime/default Chat effect |

## 4. Current Authoritative Entry Points

| Document | Use for |
| --- | --- |
| `AGENTS.md` | Agent instructions, project context, Tool Taxonomy, and current hard constraints. |
| `plans/openlife_lifemodel_governed_agent_runtime.md` | Next implementation order and LifeModel-Governed Runtime program. |
| `plans/lifemodel_governed_runtime_progress.md` | W1-W77 structured status index and compressed guardrail map. |
| `plans/lifemodel_maturation_goal_plan.md` | Current Goal-mode preparation plan for LifeModel Maturation Loop End-to-End. |
| `plans/adr/0013-lifemodel-hs-source-of-truth-governance.md` | LifeModel-HS source-of-truth, proposal-first, privacy, materialized-view hard rules. |
| `plans/openlife_react_beta_roadmap.md` | ReAct execution seriousness, Beta gates, tool/action/audit baseline. |
| `plans/lifemodel_hs_mvp_task_specs.md` | Coding-ready LifeModel-HS MVP task specs. |
| `plans/lifemodel_hs_legacy_write_path_audit.md` | Direct-write convergence backlog and safety map. |
| `plans/openlife_development_plan.md` | Current execution route, already aligned to the LifeModel-Governed program. |
| `plans/openlife_codex_execution_playbook.md` | How to slice and verify individual Codex tasks. |

## 5. Historical Or Scoped Reference Documents

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

## 6. Tool Status Guardrail

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

`check_runtime_migration_gate`, W19 pilot eligibility, W24/W27/W30/W33/W37/
W42/W45/W48/W57/W60 readiness gates, W25/W35/W46/W58 plan drafts, W26/W29/W32/
W36/W41/W44/W47/W59 review evidence, W28/W31/W40/W43 non-default run/preview
commands, and W56 status commands are not migration permission. They are
readiness, review, preview, draft, evidence, or status surfaces only.

Default `Send`, ordinary `send_message`, and ordinary `start_stream_message`
must remain on `legacy_stream`. They must not call W19-W60 command surfaces.
The only allowed ordinary-entry adapter code is W49-W55 pure guard/preflight
logic, which is read-only/pure, write-disabled, metadata-safe, side-effect-free,
and fail-closed.

W67 is backend-only non-default harness code, W68 is backend-only
send-compatible proof code, W69 is backend-only stream-compatible boundary
proof code, W70 is backend-only executor attachment gate report code, W71 is
backend-only disabled executor skeleton contract code, and W72 is backend-only
disabled skeleton binding integrity report code. They do not add a
Tauri command, frontend surface, Settings surface, runtime/model/tool execution,
business write, controlled executor attachment, real stream emission, event
channel, route cutover, or migration permission.
Ordinary default Chat entries must not call any of them. W68 only proves a
SendMessageResult-compatible metadata shape for a controlled adapter candidate;
W69 only proves a `start_stream_message`-compatible metadata boundary with
streamStarted/eventChannelOpened/streamEventsEmitted=false; W70 only reports
metadata readiness for an executor skeleton discussion while keeping
executor_attachment_allowed=false, route_cutover_permission=false, and
migrationPermission=false; W71 only defines disabled/unattached/no-run
send/stream metadata-only placeholders while keeping executor_runnable=false
and invocation_allowed=false; W72 only verifies W71 input/skeleton and W70 gate
binding integrity while keeping executor_runnable=false, invocation_allowed=false,
route_cutover_permission=false, and migrationPermission=false; and default Chat
remains `legacy_stream`.
W73/W74/W75/W76/W77 are LifeModel maturation slices only: readiness, non-default
invocation, proposal outcome evidence link, and low-energy collaboration rule
candidate aggregation plus accepted-rule selection proof. They do not add
default Chat routing authority or ordinary Chat auto-maturation.

## 7. Agent Rules

- Always read `AGENTS.md`, this file, and
  `plans/openlife_lifemodel_governed_agent_runtime.md` before starting a new
  architecture/runtime/LifeModel/tool task.
- Use `plans/lifemodel_governed_runtime_progress.md` for W1-W77 status, not as
  an implementation roadmap.
- Do not use historical plans to override current ordering, current Tool
  Taxonomy, or the default Chat `legacy_stream` boundary.
- If implementation changes tool status, proposal semantics, runtime authority,
  model routing, LifeModel source-of-truth, privacy boundaries, or default Chat
  routing, update the relevant docs in the same task and run the implementation
  verification gate.

## 8. Next Recommended Sequence

```text
W63 complete -> W64 authority compression validated -> W65 backend-only
descriptor skeleton complete -> W66 controlled adapter contract report complete
-> W67 non-default invocation harness complete -> W68 send-compatible proof
complete -> W69 stream-compatible boundary proof complete -> W70 executor
attachment gate report complete -> W71 disabled executor skeleton contract
complete -> W72 disabled skeleton binding integrity report complete -> W73
LifeModel maturation readiness report complete -> W74 non-default maturation
invocation complete -> W75 proposal outcome evidence link complete -> W76
low-energy collaboration rule candidate complete -> W77 accepted rule to
RuntimeHSPacket selection proof complete -> W78 run trace visibility next. Any
future default Chat executor implementation or route cutover remains a separate
reviewed task that preserves default Chat
legacy_stream until a route change is explicitly implemented, reviewed,
verified, and authorized.
```

For docs-only index整理, `git diff --check` plus targeted `rg` validation is
enough. Run `make ci` when code, tests, package configuration, or runtime
behavior changes.
