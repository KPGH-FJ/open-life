# LifeModel-Governed Runtime Progress

> Last updated: 2026-06-03
> Status: W88 Proposal Application Source-Specific Patch Mapping complete

This file is the compact completion/status index for Agents entering the
LifeModel-Governed Runtime work. It does not replace
`plans/openlife_lifemodel_governed_agent_runtime.md`; use that program document
for implementation order, and use this file to avoid re-reading stale long
route text.

## Current Position

Current latest status is **W88 Proposal Application Source-Specific Patch Mapping complete**.
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
callsite to become proof ready; stream callsites fail closed. W69 adds a pure
Rust backend-only stream-compatible boundary proof/evaluator/ensure that
reads/reuses only W65 descriptor, W66 contract, and W67 harness metadata to
prove the controlled adapter candidate can form a stream-compatible metadata
boundary for `start_stream_message`. It allows only the StartStreamMessage callsite to
become proof ready; SendMessage fails closed with
`callsite_not_start_stream_message`. W69 does not emit a real stream, open an
event channel, attach an executor, run runtime/model/tool, write business
records, or authorize a default Chat route cutover. W70 adds a pure Rust
backend-only executor attachment gate report/evaluator/ensure that simultaneously
reuses W65-W67 metadata-safe descriptor/contract/harness results, the W68
send-compatible proof, and the W69 stream-compatible boundary proof. It can
report attachment gate metadata readiness for the next executor skeleton
discussion, but executor_attachment_allowed=false, executor_attached=false,
executor_enabled=false, route_cutover_permission=false, and
migrationPermission=false remain fixed. Executor implementation missing, human
review missing, and route cutover not authorized are explicit blockers. W65-W70
add no command, no frontend surface, no Settings surface, no runtime/model/tool
call, no store write, no executor attachment, and no default Chat routing
change.
W71 adds a pure Rust backend-only disabled controlled executor skeleton
contract/evaluator/ensure in `src-tauri/src/default_chat_adapter.rs`. It reuses
the W70 gate report and accepts only metadata-safe callsite kind, route metadata,
input length/hash, and requested shape. It produces metadata-only placeholders
for `send_message_result` and `stream_boundary`, fails closed for unknown
shapes, and keeps executor_skeleton_present=true, executor_enabled=false,
executor_attached=false, executor_runnable=false, invocation_allowed=false,
route_cutover_permission=false, and migrationPermission=false. W71 adds no
command, no frontend surface, no Settings surface, no runtime/model/tool call,
no stream emission, no event channel, no business write, no executor
attachment, no route cutover, and no default Chat routing change.
W72 adds a pure Rust backend-only disabled skeleton binding integrity
report/evaluator/ensure in `src-tauri/src/default_chat_adapter.rs`. It reuses
the W71 disabled skeleton, W71 skeleton input, and W70 gate report to verify
input length/hash, route metadata, requested shape/callsite, skeleton output
shape, legacy route metadata, gate metadata, and disabled/no-run/no-write/no-stream
constraints are bound consistently. W72 keeps executor_enabled=false,
executor_attached=false, executor_runnable=false, invocation_allowed=false,
route_cutover_permission=false, migrationPermission=false, and
selected_adapter_path=legacy_stream. W72 adds no command, no frontend surface,
no Settings surface, no runtime/model/tool call, no stream emission, no event
channel, no business write, no executor implementation, no executor attachment,
no route cutover, no migration permission, and no default Chat routing change.
W73 adds a pure core LifeModel maturation readiness report/evaluator/ensure in
`openlife-core/src/agent/maturation.rs`. It validates only a narrow low-energy
/ low-pressure planning preference LifeEventDraft, requires metadata safety,
proposal-first semantics, source lineage, default Chat unchanged, ordinary Chat
unchanged, no direct LifeModel/Memory/Heuristic writes, and a zero side-effect
budget. W73 returns `nextAllowedStep=non_default_maturation_invocation` only
when clean. It adds no Tauri command, no frontend surface, no runtime/model/tool
call, no Evidence/Proposal/LifeModel/Memory/Heuristic/Chat/MCP audit/external
write, no ordinary Chat auto-maturation, and no default Chat route change.
W74 adds a pure core explicit non-default LifeModel maturation invocation
harness/report in `openlife-core/src/agent/maturation.rs`. It calls W73
readiness before invoking maturation, writes no stores when readiness blocks,
and when ready writes only governed EvidenceStore records plus pending
ProposalStore records. It adds no Tauri command, no frontend surface, no
runtime/model/tool call, no LifeModel/Memory/Heuristic/Chat/AgentRun/MCP
audit/external write, no ordinary Chat auto-maturation, and no default Chat
route change.
W75 adds a pure core proposal outcome evidence helper/report in
`openlife-core/src/agent/proposal_outcome.rs` and a minimal internal Tauri
proposal command integration in `src-tauri/src/commands/proposal.rs`. After
successful proposal accept/reject/edit status updates, only maturation lineage
proposals record metadata-safe `ProposalOutcome` evidence. Rejections become
negative/opposing outcome evidence; edits record outcome metadata without raw
edited payload leakage. W75 preserves existing proposal apply semantics, adds
no command/frontend surface, runs no runtime/model/tool, writes no new direct
LifeModel/Memory/Heuristic truth, and does not affect default Chat.
W76 adds a pure core low-energy collaboration rule candidate evaluator/report
and proposer in `openlife-core/src/agent/maturation.rs`. It aggregates only
metadata-safe W75 ProposalOutcome evidence for the low-energy / low-pressure
planning collaboration scope, preserves accepted/rejected/edited outcome
evidence ids, source evidence ids, linked proposal ids, and linked AgentRun
ids, and fails closed for non-low-energy domains or outcome evidence outside
the collaboration scope. Negative/opposing outcome evidence blocks or weakens
repeated similar candidate suggestions. When ready, W76 may write only a
pending ProposalStore candidate proposal; it does not activate a Heuristic, does
not write active rules, adds no command/frontend surface, runs no
runtime/model/tool, writes no LifeModel/Memory/Heuristic truth, and does not
affect default Chat.
W77 adds a pure core accepted low-energy rule selection proof in
`openlife-core/src/agent/maturation.rs`. It defines
`AcceptedLowEnergyRuleSelectionInput`,
`AcceptedLowEnergyRuleSelectionReport`,
`AcceptedLowEnergyRuleSelectionHSPacketAuditProof`,
`evaluate_accepted_low_energy_rule_selection`, and
`ensure_accepted_low_energy_rule_selection`. W77 only proves that a
user-accepted W76 candidate proposal can be selected into future
RuntimeHSPacket metadata-safe planning guidance. Pending/rejected/non-W76
proposals, non-planning tasks, and non-low-energy domains fail closed. The
proof preserves source outcome evidence ids, linked proposal ids, and linked
AgentRun ids. Privacy/model route policy cannot be relaxed; local-only policy
is kept or strengthened. W77 adds no command/frontend surface, runs no
runtime/model/tool, writes no LifeModel/Memory/Heuristic truth, does not
activate a Heuristic, and does not affect default Chat.
W78 adds a pure core low-energy rule trace visibility proof in
`openlife-core/src/agent/maturation.rs`. It defines
`LowEnergyRuleTraceVisibilityInput`,
`LowEnergyRuleTraceVisibilityReport`, `LowEnergyRuleTraceMetadata`,
`evaluate_low_energy_rule_trace_visibility`, and
`ensure_low_energy_rule_trace_visibility`. W78 proves that W77 selected
guidance can be exposed by future runtime/run trace metadata without raw
content: selected guidance summary/hash, candidate proposal id/hash, candidate
rule digest, evidence/proposal/AgentRun lineage id/hash/count/status/type,
selected policy ids, enforced route policy, and report/payload hashes. Blocked
or non-selected W77 reports, non-planning tasks, non-low-energy domains, raw
trace payloads, privacy/model route relaxation, default Chat cutover hints,
runtime/model/tool execution hints, AgentRun writes, and Heuristic activation
fail closed. W78 adds no command/frontend surface, runs no runtime/model/tool,
writes no AgentRun/LifeModel/Memory/Heuristic truth, does not activate a
Heuristic, and does not affect default Chat.
W79 adds a backend-only/internal Rust legacy direct-write convergence inventory
guard in `src-tauri/src/legacy_write_convergence.rs`. It defines
`LegacyWriteRiskClass`, `LegacyWriteConvergenceStatus`,
`LegacyWritePathKind`, `LegacyWriteInventoryEntry`,
`LegacyWriteConvergenceReport`, `legacy_write_convergence_inventory`,
`evaluate_legacy_write_convergence_inventory`, and
`ensure_legacy_write_convergence_inventory_guard`. W79 covers the legacy audit
map as machine-readable metadata-safe inventory: LifeModel save primitive,
manual editor, Builder proposal/direct paths, Calibration proposal/direct
paths, Feedback evolution, restore/import, state/daily goals, raw chat/memory
and vector source writes, Proposal Review Center apply/edit, and external
proposal/declarative paths. It marks high-risk direct writes as blockers,
marks proposal-first paths as targets/already proposal-first, keeps low-risk
transient/source-data paths out of durable LifeModel truth, and confirms
calendar/email propose tools are proposal-only rather than real provider write
executors. W79 adds no command/frontend surface, runs no runtime/model/tool,
writes no stores, changes no product behavior, does not affect default Chat,
and does not converge any direct-write path.
W80 adds a backend-only/internal manual LifeModel editor explicit override audit
guard in `src-tauri/src/commands/life_model.rs`. It defines
`ManualLifeModelOverrideAuditReport`,
`evaluate_manual_lifemodel_override_audit`, and
`record_manual_lifemodel_override_audit_with_state`. The existing
`save_life_model_with_state` editor save remains available; after a successful
save it records a metadata-safe `manual_lifemodel_override_audit` analytics
event with source, before/after hashes, rough changed section names/count, risk
class, timestamp, command/function name, and
manualOverride/proposalFirst/stillLegacyDirectWrite flags only. It does not
record raw LifeModel JSON, identity values, goals, relationships,
health/finance/privacy text, prompts, outputs, tool payloads, or full
before/after payloads. It does not create Proposal/AgentRun/Heuristic/Patch
records, run runtime/model/tool, or affect default Chat. The W79 inventory now
marks the manual editor guard present while keeping `manual_lifemodel_editor`
as a high-risk legacy direct-write blocker and keeping
`overall_converged=false` / `all_direct_writes_converged=false`.
W81 adds a backend-only Builder legacy direct apply dev/migration gate in
`src-tauri/src/commands/builder.rs`. `builder_apply_signals` now fails closed
by default and only enters the legacy direct write path when an explicit
dev/migration override is supplied. The direct-apply response is metadata-safe
and no longer returns raw model payloads, snapshots, feedback audit detail, or
run ids. `builder_step_with_state` no-signal completion performs session-only
cleanup and returns `durable_lifemodel_write=false`, without persisting durable
LifeModel truth. The normal Builder product path remains
`builder_create_proposals`; Builder legacy direct apply remains a high-risk
legacy direct-write blocker and convergence remains false.
W82 adds a backend Calibration legacy direct apply dev/migration gate in
`src-tauri/src/commands/calibration.rs`. `apply_calibration(mode="direct")`
and `run_micro_evolution` now fail closed by default and only enter legacy
direct persistence when an explicit `CalibrationLegacyDirectApplyDevMigrationOverride`
is supplied. The direct/evolution legacy response is metadata-safe and does not
return raw LifeModel, raw calibration change/reason, or raw evolution payloads.
Normal Calibration and Dashboard product flow uses `calibration_create_proposals`
/ proposal mode and writes ProposalStore entries. Calibration proposal flow is
the normal proposal-first target; Calibration direct/evolution remains a
high-risk legacy direct-write blocker and convergence remains false.
W83 adds a backend Feedback evolution legacy direct apply dev/migration gate in
`src-tauri/src/commands/feedback.rs`. `apply_feedback_evolution` now fails
closed by default and only enters the legacy direct write path when an explicit
`FeedbackEvolutionLegacyDirectApplyOverride` is supplied. The legacy response
is metadata-safe and does not return raw feedback text, raw conversation
inference, raw LifeModel, or raw evolution rule payloads. `generate_evolution_report`
is now read-only and returns metadata-safe counts/status only; it does not
write LifeModel or `evolution_rules` truth. The settings UI presents the result
as a read-only candidate report. The W79 inventory now separates Feedback
signals as low-risk source data and the read-only report from the Feedback
evolution direct-apply blocker. Feedback evolution direct apply remains a
high-risk legacy direct-write blocker and convergence remains false.
W84 adds backend Snapshot restore and Data import legacy direct write gates in
`src-tauri/src/commands/version.rs` and `src-tauri/src/commands/settings.rs`.
`restore_snapshot` and `import_all_data` now fail closed by default and only
enter legacy direct write paths when explicit dev/migration/manual restore
overrides are supplied. Legacy responses are metadata-safe and return snapshot
ids/counts/status only; they do not return raw LifeModel, raw memory/vector
content, raw imported payloads, or snapshot YAML. Export/read-only paths remain
unchanged. Snapshot restore and Data import remain high-risk legacy direct-write
blockers and convergence remains false.
W85 adds a backend-only/internal State / Daily Goal source-data boundary proof
in `src-tauri/src/legacy_write_convergence.rs`. It defines
`StateSourceDataBoundaryReport`, `evaluate_state_source_data_boundary`, and
`ensure_state_source_data_boundary` over the existing
`state_daily_goal_direct_writes` inventory entry. The report is metadata-safe
and includes only path ids, fixed source-data / low-risk transient
classification, compatibility_lifemodel_materialized_write=true,
writes_current_lifemodel_compatibility_view=true,
accepted_durable_hs_truth_write=false, active_hs_lifemodel_patch=false,
proposal_required_for_hs_truth_promotion=true, ordinary_chat_unchanged=true,
default_chat_unchanged=true, and blocker codes. The inventory must explicitly
list `persist_life_model`, because State / Daily Goal currently writes the
current LifeModel compatibility view / YAML; W85 classifies that write as
source-data compatibility materialized state, not accepted durable
LifeModel-HS truth.
W85 is not proposal-first conversion, changes no current State/Daily Goal
product behavior, does not add a command/frontend surface, does not create
ProposalStore/EvidenceStore/AgentRun writes, does not affect default Chat, and
does not mark State/Daily Goal fully converged. Promotion from state source
data into durable LifeModel-HS truth remains a separate future proposal-first
slice.
W86 adds a backend-only/internal LifeModel compatibility materializer caller
matrix in `src-tauri/src/legacy_write_convergence.rs`. It defines
`LifeModelMaterializerCallerKind`,
`LifeModelMaterializerCallerRisk`,
`LifeModelMaterializerCallerGovernanceState`,
`LifeModelMaterializerCallerMatrixEntry`,
`LifeModelMaterializerCallerMatrixReport`,
`lifemodel_materializer_caller_matrix`,
`evaluate_lifemodel_materializer_caller_matrix`, and
`ensure_lifemodel_materializer_caller_matrix`. The matrix classifies every
current production materializer/save entry in scope: 16 `persist_life_model`
callsites plus 3 production `LifeModelManager::save` related entries. It
distinguishes materializer root, ordinary Chat daily-goal auto-checkin
source-data compatibility writes, State/Daily Goal source-data compatibility
materialization, accepted proposal apply, audited manual override,
Builder/Calibration/Feedback guarded legacy dev-migration override paths, and
Snapshot restore/Data import gated override paths. W86 is metadata-safe and
contains no raw LifeModel, memory, chat, or daily-goal payloads. It adds no
command/frontend surface, changes no default Chat routing, does not change the
`persist_life_model` signature, does not retire any legacy path, and fixes
migration_permission=false, runtime_authority_granted=false, and
proposal_first_convergence_complete=false. W86 is not convergence complete; it
is the preparation layer for W87 caller restriction.
W87 adds typed caller-purpose restriction for the LifeModel compatibility
materializer. `persist_life_model` now requires an explicit
`LifeModelMaterializerCallerContext`, and each production caller passes the W86
stable id, kind, and governance purpose for its classified path. The W87
restriction evaluator fails closed for unknown stable ids, kind/purpose
mismatches, metadata-unsafe entries, migration/runtime authority grants,
source-data callers marked as accepted LifeModel-HS truth, manual editor paths
marked proposal-first/converged, and restore/import/dev migration paths marked
fully converged. Snapshot restore's direct `LifeModelManager::save` now has an
explicit W87 guard after the existing W84 override gate and before the actual
save. W87 adds no Tauri command, frontend/Settings surface, runtime/model/tool
execution, Chat/AgentRun/Evidence/Proposal/Memory/MCP audit/external write, or
default Chat routing change. It does not retire legacy paths and does not
complete source-specific proposal patch mapping.
W88 adds a backend-only/internal PatchSource mapping report/ensure/resolver in
`src-tauri/src/commands/proposal.rs` for accepted LifeModel proposal apply.
`apply_proposal_to_state` no longer hardcodes `PatchSource::BuilderReview`.
BuilderReview maps to BuilderReview, CalibrationRun maps to Calibration,
FeedbackEvolution maps to Evolution, and Manual maps to Manual.
ChatConversation, ProactiveAgent, SkillRuntime, Plugin, and MemoryGovernance
use explicit metadata-safe Manual fallback with W89 follow-up/blocking metadata
because PatchSource has no dedicated variants for those proposal sources. W88
adds no command/frontend/Settings surface, runs no runtime/model/tool, changes
no default Chat routing, retires no legacy path, and keeps
`proposal_first_convergence_complete=false` until W89 source-specific patch
audit/readiness.

Hard boundaries:

- default Chat remains `legacy_stream`.
- Default `Send`, ordinary `send_message`, and ordinary
  `start_stream_message` may enter only the legacy route, with the W49-W55 pure
  guards/preflight allowed to fail closed.
- W19-W60 readiness/review/preview/gate results are not migration permission.
- W65-W72 backend-only descriptor/contract/harness/proof/gate/skeleton/binding work is not
  migration permission and must keep the controlled adapter executor
  disabled/unattached.
  W67 `harness_ready` only means the non-default invocation shape proof is
  safe; W68 `proof_ready` only means the SendMessageResult-compatible metadata
  shape proof is safe; W69 `proof_ready` only means the stream-compatible
  metadata boundary proof is safe; W70 `gate_report_metadata_ready` only means
  the executor attachment gate report is metadata-ready for skeleton
  discussion; W71 `skeleton_contract_ready` only means the disabled skeleton
  contract metadata is safe and still no-run; W72 `binding_integrity_ready` only
  means disabled skeleton binding metadata is internally consistent and still
  no-run, not that default Chat may migrate.
- Ordinary `send_message` / `start_stream_message` must not call any W19-W60
  command surface.
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
- Ordinary `send_message` / `start_stream_message` must not call the W78
  low-energy rule trace visibility helper.
- Ordinary `send_message` / `start_stream_message` must not call the W79
  legacy write convergence inventory guard.
- Ordinary `send_message` / `start_stream_message` must not call the W80
  manual LifeModel override audit helper.
- Ordinary `send_message` / `start_stream_message` must not call the W81
  Builder legacy direct apply helper or override.
- Ordinary `send_message` / `start_stream_message` must not call the W82
  Calibration legacy direct apply helper or override.
- Ordinary `send_message` / `start_stream_message` must not call the W83
  Feedback evolution legacy direct apply helper or override.
- Ordinary `send_message` / `start_stream_message` must not call the W84
  Snapshot restore / Data import legacy direct apply helper or override.
- Ordinary `send_message` / `start_stream_message` must not call the W85 State
  / Daily Goal source-data boundary helper.
- Ordinary `send_message` / `start_stream_message` must not call the W86
  LifeModel materializer caller matrix helper.
- Ordinary `send_message` / `start_stream_message` must not call the W87
  LifeModel materializer caller restriction evaluator/ensure helpers. Passing a
  typed W87 source-data context into `persist_life_model` for the existing
  daily-goal auto-checkin compatibility write is not a route change and grants
  no migration/runtime authority.
- Ordinary `send_message` / `start_stream_message` must not call the W88
  proposal PatchSource mapping helper.
- W61-W63 are docs/index整理 only and cannot affect default Chat.

## Authority And Conflict Rule

When old plans conflict, use this order:

1. `AGENTS.md`
2. `plans/README.md`
3. `plans/openlife_lifemodel_governed_agent_runtime.md`
4. This W1-W88 progress index
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

## W1-W88 Structured Index

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
| W68 | Default Chat Adapter Send-Compatible Contract Proof | Done | Pure internal proof/evaluator in `default_chat_adapter.rs` | MS send-compatible proof only; reads W65/W66/W67 metadata, SendMessage only ready, stream fail-closed, executor disabled/unattached, zero side-effect budget, migration permission false | No; ordinary send/stream stay `legacy_stream` and do not call it | W69 |
| W69 | Default Chat Adapter Stream-Compatible Boundary Proof | Done | Pure internal proof/evaluator in `default_chat_adapter.rs` | MS stream boundary proof only; reads W65/W66/W67 metadata, StartStreamMessage only ready, SendMessage fail-closed, no real stream/event channel, executor disabled/unattached, zero side-effect budget, migration permission false | No; ordinary send/stream stay `legacy_stream` and do not call it | W70 |
| W70 | Default Chat Adapter Controlled Executor Attachment Gate Report | Done | Pure internal gate report/evaluator in `default_chat_adapter.rs` | MS executor attachment gate report only; reads W65-W67 metadata-safe layers plus W68/W69 proofs, executor attachment/cutover/migration permission all false, explicit executor implementation/human review/cutover blockers, zero side-effect budget | No; ordinary send/stream stay `legacy_stream` and do not call it | Future executor skeleton discussion only |
| W71 | Default Chat Adapter Disabled Controlled Executor Skeleton Contract | Done | Pure internal skeleton contract/evaluator in `default_chat_adapter.rs` | MS disabled skeleton only; reads W70 gate report, input length/hash and route metadata only, send/stream metadata-only placeholders, executor disabled/unattached/not runnable, invocation disallowed, zero side-effect budget, migration permission false | No; ordinary send/stream stay `legacy_stream` and do not call it | Future executor implementation discussion only |
| W72 | Default Chat Adapter Disabled Executor Skeleton Binding Integrity Report | Done | Pure internal binding integrity evaluator in `default_chat_adapter.rs` | MS binding report only; reads W71 skeleton/input and W70 gate report, verifies input hash/length, route metadata, requested shape/callsite, skeleton output shape, legacy route, disabled/no-run/no-write/no-stream constraints | No; ordinary send/stream stay `legacy_stream` and do not call it | Future executor implementation discussion only |
| W73 | LifeModel Maturation End-to-End Readiness Report | Done | Pure core evaluator in `maturation.rs` | RO / MS readiness report only; low-energy planning candidate, proposal-first, source-lineage-required, no direct LifeModel/Memory/Heuristic writes, zero side-effect budget | No; ordinary send/stream stay `legacy_stream` and do not call it | W74 non-default maturation invocation |
| W74 | Non-Default LifeModel Maturation Invocation | Done | Pure core explicit invocation harness/report in `maturation.rs` | MS non-default invocation only; calls W73 first, blocked writes no stores, ready writes EvidenceStore + ProposalStore only, no runtime/model/tool, no direct LifeModel/Memory/Heuristic/Chat/AgentRun/MCP audit/external write | No; ordinary send/stream stay `legacy_stream` and do not call it | W75 proposal outcome evidence link |
| W75 | LifeModel Maturation Proposal Outcome Evidence Link | Done | Core helper/report plus minimal internal proposal accept/reject/edit wiring | MS ProposalOutcome evidence only for maturation lineage proposals; accept/edit keep existing apply semantics, reject records negative/opposing evidence without apply; no runtime/model/tool, no new direct LifeModel/Memory/Heuristic writes | No; ordinary send/stream stay `legacy_stream` and do not call it | W76 low-energy collaboration rule candidate |
| W76 | Low-Energy Collaboration Rule Candidate | Done | Pure core evaluator/proposer in `maturation.rs` | MS candidate aggregation only; accepted/edited/rejected outcome ids plus source/proposal/run lineage, opposing evidence blocks/weakens, ready writes only pending ProposalStore candidate proposal, no active Heuristic/rule | No; ordinary send/stream stay `legacy_stream` and do not call it | W77 accepted rule selection proof |
| W77 | Accepted Rule To RuntimeHSPacket Selection Proof | Done | Pure core evaluator/report in `maturation.rs` | MS selection proof only; requires accepted W76 candidate proposal, planning task, low-energy domain, metadata-safe guidance, lineage retained, privacy/model route policy not relaxed, zero side-effect counters | No; ordinary send/stream stay `legacy_stream` and do not call it | W78 run trace visibility proof |
| W78 | LifeModel Maturation Run Trace Visibility Proof | Done | Pure core evaluator/report in `maturation.rs` | MS trace visibility proof only; W77 selected guidance and lineage visible as summary/hash/id/count/status/type metadata, privacy/local-only policy preserved, raw payload/policy relaxation/execution/cutover hints fail closed, zero side-effect counters | No; ordinary send/stream stay `legacy_stream` and do not call it | W79 legacy direct-write inventory guard |
| W79 | Legacy Direct-Write Convergence Inventory Guard | Done | Internal Rust inventory/report/ensure in `legacy_write_convergence.rs` | MS inventory guard only; reports high-risk direct-write blockers, proposal-first targets, low-risk source-data paths, and external proposal-only paths; overallConverged=false/allDirectWritesConverged=false | No; ordinary send/stream stay `legacy_stream` and do not call it | W80 manual editor override audit guard |
| W80 | Manual LifeModel Editor Explicit Override Audit Guard | Done | Internal backend save-path audit helper in `commands/life_model.rs` | MS audit guard only; successful manual editor save records source, before/after hashes, rough changed sections, risk class, timestamp, command, and manualOverride/proposalFirst/stillLegacyDirectWrite flags; no raw payloads, Proposal, AgentRun, Heuristic, Patch, runtime/model/tool | No; ordinary send/stream stay `legacy_stream` and do not call it | W81 Builder legacy direct apply dev-gate |
| W81 | Builder Legacy Direct Apply Dev-Gate / No-Signal Completion Guard | Done | Backend Builder command guard in `commands/builder.rs` plus W79 inventory update | MS guard only; `builder_apply_signals` defaults fail closed and requires explicit dev/migration override for remaining legacy direct apply, direct response omits raw model/run/audit payloads, no-signal completion is session-only/no durable LifeModel write, normal `builder_create_proposals` remains proposal-first; convergence false | No; ordinary send/stream stay `legacy_stream` and do not call it | Remove Builder legacy direct apply or convert fully to proposal-first |
| W82 | Calibration Direct Apply Legacy Gate / Proposal-First Default | Done | Backend Calibration command guard in `commands/calibration.rs`, Dashboard normal-flow proposal update, and W79 inventory update | MS guard only; `apply_calibration(mode="direct")` and `run_micro_evolution` default fail closed and require explicit Calibration legacy direct apply dev/migration override, legacy output omits raw LifeModel/calibration/evolution payloads, normal `calibration_create_proposals` / proposal mode writes ProposalStore; convergence false | No; ordinary send/stream stay `legacy_stream` and do not call it | Remove Calibration direct/evolution legacy persistence or convert fully to proposal-first |
| W83 | Feedback Evolution Legacy Direct Apply Gate / Proposal-First Candidate Path | Done | Backend Feedback command guard in `commands/feedback.rs`, settings read-only copy update, and W79 inventory update | MS guard/read-only report only; `apply_feedback_evolution` defaults fail closed and requires explicit Feedback evolution legacy direct apply dev/migration override, legacy output omits raw feedback/inference/LifeModel/evolution rule payloads, `generate_evolution_report` is read-only and writes no LifeModel/`evolution_rules`; convergence false | No; ordinary send/stream stay `legacy_stream` and do not call it | Remove Feedback evolution legacy direct apply or convert fully to proposal/evidence-first |
| W84 | Snapshot Restore / Data Import Legacy Direct Write Gate | Done | Backend Version/Settings command guards in `commands/version.rs` and `commands/settings.rs`, plus W79 inventory update | MS guard only; `restore_snapshot` and `import_all_data` default fail closed and require explicit dev/migration/manual restore override, legacy outputs omit raw LifeModel/memory/vector/import payload/snapshot YAML and return snapshot id/count/status only; export/read-only paths unchanged; convergence false | No; ordinary send/stream stay `legacy_stream` and do not call it | Remove restore/import legacy override capability or convert to governed rollback/migration audit flow |
| W85 | State / Daily Goal Source Data Boundary Proof | Done | Internal Rust report/evaluator/ensure in `legacy_write_convergence.rs` | MS source-data boundary proof only; `state_daily_goal_direct_writes` remains LowRiskTransientSourceData and explicitly lists `persist_life_model`; compatibility_lifemodel_materialized_write=true, writes_current_lifemodel_compatibility_view=true, accepted_durable_hs_truth_write=false, active_hs_lifemodel_patch=false, proposal_required_for_hs_truth_promotion=true; no command/frontend/runtime/model/tool/store behavior change, not proposal-first conversion, not fully converged | No; ordinary send/stream stay `legacy_stream` and do not call it | Future StateStore TTL/source/confidence split or separate proposal-first truth promotion bridge |
| W86 | LifeModel Compatibility Materializer Caller Matrix | Done | Internal Rust matrix/report/evaluator/ensure in `legacy_write_convergence.rs` | MS caller matrix only; covers current production `persist_life_model` callsites and production `LifeModelManager::save` related entries; classifies materializer root, ordinary Chat auto-checkin source-data compatibility, State/Daily Goal source-data compatibility, accepted proposal apply, audited manual override, guarded legacy dev-migration overrides, and gated restore/import overrides; no command/frontend/runtime/model/tool/store behavior change, no `persist_life_model` signature change, no legacy path retirement, migration_permission=false, runtime_authority_granted=false, proposal_first_convergence_complete=false | No; ordinary send/stream stay `legacy_stream` and do not call it | W87 LifeModel materializer caller restriction |
| W87 | LifeModel Materializer Caller Restriction | Done | Internal typed caller context/restriction evaluator in `legacy_write_convergence.rs`, `persist_life_model` signature update, and snapshot restore direct-save guard | MS caller restriction only; all 16 production `persist_life_model` callsites pass explicit W86 stable id + kind + purpose context; snapshot restore direct `LifeModelManager::save` has a W87 guard after the W84 override; unknown/mismatched callers fail closed; metadata-safe and raw-content-free; keeps migration_permission=false/runtime_authority_granted=false/proposal_first_convergence_complete=false; not full convergence, no legacy path retirement | No; ordinary send/stream stay `legacy_stream` and only pass source-data compatibility context for existing daily-goal auto-checkin | W88 mapping, then W89 audit/readiness |
| W88 | Proposal Application Source-Specific Patch Mapping | Done | Backend-only private mapper/report/ensure in `commands/proposal.rs` | MS mapping only; accepted LifeModel proposal apply no longer hardcodes BuilderReview; BuilderReview->BuilderReview, CalibrationRun->Calibration, FeedbackEvolution->Evolution, Manual->Manual; ChatConversation/ProactiveAgent/SkillRuntime/Plugin/MemoryGovernance use metadata-safe Manual fallback with W89 follow-up/blocking metadata; no command/frontend/runtime/model/tool/store behavior change, no legacy path retirement, proposal_first_convergence_complete=false | No; ordinary send/stream stay `legacy_stream` and do not call it | W89 Proposal Application Source-Specific Patch Audit / Readiness |

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
- W65-W72 descriptor/contract/harness/proof/gate/skeleton/binding work is internal backend code only.
  It may describe and validate a future controlled adapter candidate with
  metadata-safe fields, a non-default invocation shape proof, and a
  SendMessageResult-compatible metadata shape proof, plus a
  `start_stream_message`-compatible metadata boundary proof, W70 may report
  the attachment gate metadata-ready for executor skeleton discussion, and W71
  may define disabled send/stream metadata-only placeholders, and W72 may verify
  W71 input/skeleton plus W70 gate binding integrity, but it must not execute or
  attach that adapter, emit a real stream, open an event channel, run
  runtime/model/tool, write business records, grant route cutover, grant
  migration permission, or change default Chat routing.
- W73-W78 are LifeModel maturation slices only: readiness, explicit
  non-default invocation, proposal outcome evidence link, and low-energy
  collaboration rule candidate aggregation plus accepted-rule selection proof
  and trace visibility proof.
  They do not migrate maturation into runtime execution, do not add ordinary
  Chat auto-maturation, and do not authorize default Chat routing changes.
- W79 is a Legacy Direct-Write Convergence inventory guard only. It makes the
  old audit map testable, but it does not remove, gate, rewrite, or mark any
  high-risk direct-write blocker as converged.
- W80 adds metadata-safe audit to the manual LifeModel editor direct save path,
  but the path remains a high-risk legacy direct-write blocker and is not
  proposal-first converged.
- W81 adds a default fail-closed dev/migration gate to Builder legacy direct
  apply and proves no-signal completion does not write durable LifeModel truth,
  but the remaining override capability means the path remains a high-risk
  blocker until removed or converted to proposal-first.
- W82 adds a default fail-closed dev/migration gate to Calibration direct apply
  and micro-evolution persistence, keeps legacy responses metadata-safe, and
  keeps normal Calibration/Dashboard flow on `calibration_create_proposals`;
  the remaining override capability means Calibration direct/evolution remains
  a high-risk blocker until removed or converted to proposal-first.
- W83 adds a default fail-closed dev/migration gate to Feedback evolution
  direct apply, keeps legacy responses metadata-safe, and makes
  `generate_evolution_report` read-only/no active `evolution_rules` write; the
  remaining override capability means Feedback evolution direct apply remains a
  high-risk blocker until removed or converted to proposal-first.
- W84 adds default fail-closed dev/migration/manual-restore gates to Snapshot
  restore and Data import, keeps legacy responses metadata-safe, and leaves
  export/read-only paths unchanged; the remaining override capability means
  restore/import remain high-risk blockers until removed or converted to
  governed rollback/migration flows.
- W85 adds only a backend/internal source-data boundary proof for State /
  Daily Goal. It proves the current `persist_life_model` compatibility view /
  YAML write is low-risk transient/source-data compatibility materialized
  state rather than accepted durable LifeModel-HS truth, while keeping default
  Chat unchanged, ordinary Chat unchanged, accepted_durable_hs_truth_write=false,
  active_hs_lifemodel_patch=false, and
  proposal_required_for_hs_truth_promotion=true for any future HS truth
  promotion. It is not proposal-first conversion and not a fully-converged
  marker.
- W86 adds only a backend/internal LifeModel materializer caller matrix. It
  classifies current production `persist_life_model` and
  `LifeModelManager::save` related entries, keeps reports metadata-safe, keeps
  default Chat unchanged, does not change the `persist_life_model` signature,
  does not retire any legacy path, and grants no migration permission or
  runtime authority. It is not convergence completion; W87 must perform the
  actual caller restriction.
- W87 adds the actual backend/internal caller restriction on top of the W86
  matrix. It requires typed caller context at `persist_life_model` production
  callsites and adds a W87 guard to snapshot restore's direct manager save, but
  it still does not change default Chat routing, retire legacy paths, grant
  migration/runtime authority, or complete proposal-first source-specific patch
  mapping.
- W88 adds only backend/internal proposal application PatchSource mapping. It
  fixes accepted LifeModel proposal PatchStore/audit source semantics, but it
  still does not change default Chat routing, retire legacy paths, grant
  migration/runtime authority, or mark proposal-first convergence complete.
  W89 must complete source-specific patch audit/readiness.

## Next Recommended Sequence

```text
W63 complete -> W64 authority compression validated -> W65 backend-only
descriptor skeleton complete -> W66 controlled adapter contract report complete
-> W67 non-default invocation harness complete -> W68 send-compatible proof
complete -> W69 stream-compatible boundary proof complete -> W70 executor
attachment gate report complete -> W71 disabled executor skeleton contract
complete -> W72 disabled skeleton binding integrity report complete -> enter
LifeModel Maturation Loop End-to-End Goal preparation through
plans/lifemodel_maturation_goal_plan.md -> W73 LifeModel maturation readiness
report complete -> W74 non-default maturation invocation complete -> W75
proposal outcome evidence link complete -> W76 low-energy collaboration rule
candidate complete -> W77 accepted rule selection proof complete -> W78 run
trace visibility proof complete -> W79 legacy direct-write convergence
inventory guard complete -> W80 manual LifeModel override audit guard complete
-> W81 Builder legacy direct apply dev-gate complete -> W82 Calibration direct
apply legacy gate complete -> W83 Feedback evolution legacy direct apply gate
complete -> W84 Snapshot restore / data import legacy direct write gate
complete -> W85 State / Daily Goal source-data boundary proof complete -> W86
LifeModel materializer caller matrix complete -> W87 LifeModel materializer
caller restriction complete -> W88 Proposal Application Source-Specific Patch
Mapping complete -> W89 Proposal Application Source-Specific Patch Audit /
Readiness.
Future direct-write convergence slices must start from the W79 inventory, the
W80 manual editor audit state, W81 Builder guard state, W82 Calibration guard
state, W83 Feedback evolution guard state, W84 restore/import guard state, W85
State/Daily Goal source-data boundary state, W86 materializer caller matrix,
and W87 caller restriction state, and must not mark a blocker
converged until the actual path is changed and verified. Future default Chat executor implementation
discussion may build on the W65-W72 proofs only through a separately reviewed
task; keep default Chat on legacy_stream unless that separate task explicitly
implements, reviews, verifies, and authorizes a route change.
```

`make ci` remains the release gate for implementation tasks. For docs-only
index整理, `git diff --check` plus targeted `rg` validation is sufficient unless
code or package configuration changes.
