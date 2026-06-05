# LifeModel-Governed Runtime Progress

> Last updated: 2026-06-05
> Status: W149 Backend Completion Goal 8 complete

This file is the compact completion/status index for Agents entering the
LifeModel-Governed Runtime work. It does not replace
`plans/openlife_lifemodel_governed_agent_runtime.md`; use that program document
for implementation order, and use this file to avoid re-reading stale long
route text.

## Current Position

Current latest status is **W149 Backend Completion Goal 8 complete**.
W90 retires Builder legacy direct apply. W91 retires Calibration direct and
micro-evolution durable LifeModel writes. W92 retires Feedback evolution durable
LifeModel / `evolution_rules` writes. W93 converts Snapshot restore and Data
import into explicit governed operations with validation, pre-change snapshots,
and metadata-safe audit/count/hash results. W94 converts manual LifeModel editor
save into a governed manual override with explicit user intent, risk
acknowledgement, pre-change snapshot, typed materializer context, and
metadata-safe audit. W95 closes ProposalSource -> PatchSource mapping with exact
source variants and no Manual fallback blocker. W96 keeps State/Daily Goal as
source-data compatibility materialization only. W97 marks the inventory and
materializer matrix converged with no high-risk legacy direct-write blockers,
`proposal_first_convergence_complete=true`, metadata-safe reports, no runtime /
model / tool execution, and default Chat still on `legacy_stream`.
W98-W105 add the first Plan-Execute product vertical on top of that governed
write baseline: a non-default weekly planning scenario with a typed product
contract, durable `PlanExecuteSession` lifecycle, explicit non-default Tauri
commands, review/edit/finalize gating, proposal-first write-like step
execution, metadata-safe AgentRun/proposal linkage, Workspace weekly planning
UI, Runs/trace visibility, and regression hardening. Write-like steps create
Review Center proposals only; they do not directly write durable LifeModel-HS
truth, Memory, external provider state, calendar, email, files, or plugin
state. Ordinary `send_message` / `start_stream_message` still remain on
`legacy_stream` and must not call W98-W105 product commands or helpers.
W106-W113 mature the RuntimeStrategy / Multi-Strategy Runtime layer without
migrating default Chat: executable ReAct and PlanExecute strategies now have
metadata-safe capability descriptors; registry readiness fails closed for
missing, duplicate, mismatched, unsafe-write, metadata-unsafe, or default Chat
migration-granting descriptors; StrategySelector emits a metadata-safe
candidate matrix/explanation; MultiStrategy outputs preserve a stable execution
report envelope; `get_runtime_strategy_registry_status` is an explicit
non-default read-only maturity command with no runtime/model/tool execution and
no business writes; preview and Plan-Execute product traces share strategy trace
vocabulary; and Direct, Layered, Workflow, Proactive, and Reflective are
future/declarative-only descriptors, not executable capabilities. W106-W113 is
not ReAct Beta execution hardening and is not migration permission.
W114-W123 harden the ReAct execution spine without migrating default Chat:
ReAct Beta readiness/status is metadata-safe and not migration permission;
AgentLoop uses a typed action schema with fail-soft parser warnings; Tool
Registry Beta readiness classifies executable reads, proposal-only tools,
permission-gated helpers, disabled/declarative-only stubs, unsupported tools,
and plugin declarations; ActionExecutor resolves manifest authority before
execution and blocks unknown/disabled/declarative-only/direct-write-like paths;
AgentAction and AgentObservation now carry metadata-safe `react_trace`
envelopes; ToolPermission proposals/replay preserve canonical blocked action
scope without raw risky payloads; write-like LifeModel/Memory/file/calendar/
email/task tools remain proposal-first; `get_react_beta_execution_status` is
explicitly non-default/read-only; Runs/Trace UI renders lifecycle metadata
without raw payload leakage. W114-W123 is not a full Beta declaration; Skill
Runtime and product golden path work may still be needed.
Ordinary `send_message` / `start_stream_message` remain on `legacy_stream`.
W124-W127 complete Backend Completion Goal 1 / Master Contract And Schemas
without migrating default Chat: W124 adds a pure backend readiness/contract
report for the LifeModel-Governed Backend Completion stage; W125 adds typed
LifeEvent schema plus a metadata-safe store skeleton with source refs, digest,
risk/privacy/domain, and raw-content blocking; W126 adds typed Signal schema
plus a deterministic low-risk extractor for low-energy planning signals; W127
adds a safe bridge that writes EvidenceStore candidate records only when a
signal is metadata-safe, low-risk, sufficiently confident, supported by
lineage, and in an allowed domain. High-risk, raw-content, low-confidence,
missing-lineage, and unsupported signals fail closed. W124-W127 add no Tauri
command, no frontend surface, no runtime/model/tool execution, no LifeModel /
Memory / Heuristic / Chat / AgentRun / MCP audit / external writes, and no
default Chat routing change.
W128-W130 complete Backend Completion Goal 2 / Evidence Graph v1 without
migrating default Chat: W128 adds a pure backend evidence graph layer with
support/opposition links, affected-path dedupe clusters, source weights, and
cluster summaries; W129 adds deterministic conflict detection from
`opposing_refs`, `Contradicted` status, rejected `ProposalOutcome` evidence,
and same affected-path cluster opposition, plus injected-now decay and
rejected-similar cooldown metadata; W130 adds a metadata-safe Evidence Timeline
read model with ids, type, path, status, confidence, risk/privacy, polarity,
link counts, proposal/run refs, cluster id/hash, conflict/decay/cooldown state,
and timestamps. W128-W130 add no Tauri command, no frontend surface, no
runtime/model/tool execution, no LifeModel / Memory / Heuristic / Chat /
AgentRun / MCP audit / external writes, no durable truth materialization, and
no default Chat routing change.
W131-W133 complete Backend Completion Goal 3 / Maturation Engine v1 without
migrating default Chat: W131 adds a pure backend Maturation Engine evaluator
over Evidence Graph clusters for planning preference, energy pattern, work
style, and communication preference only; high-risk identity, values,
relationships, health, finance, privacy, and long-term direction clusters fail
closed. W132 extends maturation proposal outcome evidence so accepted, edited,
and rejected outcomes carry positive, corrective, and negative metadata while
preserving proposal/run/evidence lineage and omitting raw edited payloads. W133
adds deterministic candidate suppression/correction using opposing evidence,
conflict, decay, rejected-similar cooldown, and rejected-similar history with
ids/hashes/counts only. W131-W133 add no Tauri command, no frontend surface, no
runtime/model/tool execution, no LifeModel / Memory / Heuristic / Chat /
AgentRun / MCP audit / external writes, no durable truth materialization, and
no default Chat routing change.
W134-W136 complete Backend Completion Goal 4 / Accepted Guidance And
Materialization without migrating default Chat: W134 adds a pure backend
accepted guidance lifecycle that converts accepted maturation candidate
proposals into Trial HeuristicStore guidance with source proposal/evidence/run
lineage, privacy/model/tool constraints, usage metadata, and rollback/archive
paths while blocking unsafe activation or policy relaxation. W135 extends the
LifeModel compatibility materialized YAML view with proposal/evidence/patch/
heuristic source digests and explicit compatibility-view provenance so it is
not represented as accepted durable source-of-truth. W136 adds a metadata-safe
LifeModel version diff / rollback read model linked to accepted guidance and
materialized view provenance. W134-W136 add no Tauri command, no frontend
surface, no runtime/model/tool execution, no ordinary Chat routing change, no
Memory/Chat/AgentRun/MCP audit/external writes, and no silent durable
LifeModel-HS truth materialization.
W137-W140 complete Backend Completion Goal 5 / Runtime Guidance Integration
without migrating default Chat: W137 extends RuntimeHSPacket with metadata-safe
selected guidance refs and hard policy-boundary summaries; W138 makes
non-default ReAct consume guidance through metadata-safe prompt summaries, config
caps, action-boundary packet propagation, behavior checks, and trace metadata;
W139 makes the explicit Plan-Execute weekly planning product path consume
gentle planning guidance while keeping write-like steps proposal-first; W140
adds a metadata-safe Guidance Impact read model / trace linkage using only
ids/digests/counts/status/type/impact fields. W137-W140 add no ordinary Chat
routing change, no migration permission, no direct LifeModel/Memory/external
write, and no raw prompt/user text/assistant output/memory/LifeModel/tool
payload leakage in read models.
W141-W143 complete Backend Completion Goal 6 / Policy / Privacy / Tool
Governance Hardening without migrating default Chat: W141 hardens ModelRouter
and privacy enforcement so High/Critical privacy and HS LocalOnly hard-filter
non-local providers, select local `ollama`, remove cloud fallback, and fail
closed when no local model is available; W142 hardens ActionExecutor HS tool
governance so unsupported Plugin/A2A tools remain disabled/declarative-only
before permission replay or execution and HS write-like paths remain
proposal-first; W143 adds a shared metadata-safe Governor decision report for
maturation, model route, tool action, memory write, and external write
decisions. W141-W143 add no ordinary Chat routing change, no migration
permission, no direct LifeModel/Memory/external write, and no raw prompt/user
text/assistant output/memory/LifeModel/tool payload leakage.
W144-W146 complete Backend Completion Goal 7 / Backend Golden Paths without
migrating default Chat: W144 proves the Weekly Planning golden path as a pure
backend/core planning guidance loop across selected guidance, Plan-Execute
metadata, proposal-first write-like step refs, outcome evidence, and future
planning guidance; W145 proves the Low-Energy Support golden path from
LifeEvent/Signal/Evidence through accepted guidance to explicit runtime
behavior-change metadata without high-risk truth materialization; W146 proves
the Preference Correction golden path where rejection/edit outcomes create
negative/corrective evidence and deterministically suppress or change future
behavior. W144-W146 add no ordinary Chat routing change, no ordinary
`send_message` / `start_stream_message` replacement, no Tauri command, no UI,
no runtime/model/tool call, no durable LifeModel/Memory/external provider state
write, and no migration permission. Ordinary Chat must not call W144-W146
golden path helpers or treat golden path ready as migration permission.
W147-W149 complete Backend Completion Goal 8 / Pre-UI Backend Contract Freeze
without migrating default Chat: W147 freezes pure backend/core metadata-safe
read-model contracts for Learning Inbox, Evidence Timeline, Proposal Review,
Runtime Trace, Guidance Impact, Privacy Controls, and LifeModel Overview; W148
adds a metadata-safe read-only final backend completion gate report with
acceptance-gate blockers, default Chat isolation, proposal-first boundaries,
raw-content exclusion, LocalOnly privacy behavior, tool governance, golden path
coverage, and remaining Beta blockers; W149 syncs authority docs, progress
index, verification matrix, and stale-reference guidance. W147-W149 add no
ordinary Chat routing change, no ordinary `send_message` / `start_stream_message`
replacement, no Tauri command, no UI, no runtime/model/tool call, no durable
LifeModel/Memory/external provider state write, and no migration permission.
Ordinary Chat must not call W147-W149 contract/gate helpers or treat contract
frozen/final gate ready as migration permission.
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
`proposal_first_convergence_complete=false` pending W89 source-specific patch
audit/readiness and fallback policy.
W89 adds a backend-only/internal readiness entry/report/evaluator/ensure in
`src-tauri/src/commands/proposal.rs` for the W88 accepted LifeModel proposal
PatchSource mapping. It proves exact mappings for BuilderReview, CalibrationRun,
FeedbackEvolution, and Manual; metadata-safe Manual fallback mappings for
ChatConversation, ProactiveAgent, SkillRuntime, Plugin, and MemoryGovernance;
`unsupported_or_unclassified_count=0`; BuilderReview is used only for
BuilderReview; `apply_proposal_to_state` still calls the W88 mapping ensure and
resolver before `LifeModelPatch::from_proposal`; the apply path does not
hardcode BuilderReview; and ordinary `send_message` / `start_stream_message`
do not call W88/W89 proposal PatchSource helpers. The report is metadata-safe
and raw-payload-free. W89 adds no command/frontend/Settings surface, runs no
runtime/model/tool, changes no product behavior or default Chat routing, retires
no legacy path, and keeps fallback blockers plus
`proposal_first_convergence_complete=false`.

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
- Ordinary `send_message` / `start_stream_message` must not call the W89
  proposal PatchSource readiness helper.
- W61-W63 are docs/index整理 only and cannot affect default Chat.

## Authority And Conflict Rule

When old plans conflict, use this order:

1. `AGENTS.md`
2. `plans/README.md`
3. `plans/openlife_lifemodel_governed_agent_runtime.md`
4. This W1-W149 progress index
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

## W1-W149 Structured Index

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
| W81 | Builder Legacy Direct Apply Dev-Gate / No-Signal Completion Guard | Done | Backend Builder command guard in `commands/builder.rs` plus W79 inventory update | Historical guard slice; superseded by W90 retirement | No; ordinary send/stream stay `legacy_stream` and do not call it | W90 retirement |
| W82 | Calibration Direct Apply Legacy Gate / Proposal-First Default | Done | Backend Calibration command guard in `commands/calibration.rs`, Dashboard normal-flow proposal update, and W79 inventory update | Historical guard slice; superseded by W91 retirement | No; ordinary send/stream stay `legacy_stream` and do not call it | W91 retirement |
| W83 | Feedback Evolution Legacy Direct Apply Gate / Proposal-First Candidate Path | Done | Backend Feedback command guard in `commands/feedback.rs`, settings read-only copy update, and W79 inventory update | Historical guard/read-only report slice; superseded by W92 retirement | No; ordinary send/stream stay `legacy_stream` and do not call it | W92 retirement |
| W84 | Snapshot Restore / Data Import Legacy Direct Write Gate | Done | Backend Version/Settings command guards in `commands/version.rs` and `commands/settings.rs`, plus W79 inventory update | Historical guard slice; superseded by W93 governed restore/import operations | No; ordinary send/stream stay `legacy_stream` and do not call it | W93 governed restore/import |
| W85 | State / Daily Goal Source Data Boundary Proof | Done | Internal Rust report/evaluator/ensure in `legacy_write_convergence.rs` | MS source-data boundary proof only; `state_daily_goal_direct_writes` remains LowRiskTransientSourceData and explicitly lists `persist_life_model`; compatibility_lifemodel_materialized_write=true, writes_current_lifemodel_compatibility_view=true, accepted_durable_hs_truth_write=false, active_hs_lifemodel_patch=false, proposal_required_for_hs_truth_promotion=true; no command/frontend/runtime/model/tool/store behavior change, not proposal-first conversion, not fully converged | No; ordinary send/stream stay `legacy_stream` and do not call it | Future StateStore TTL/source/confidence split or separate proposal-first truth promotion bridge |
| W86 | LifeModel Compatibility Materializer Caller Matrix | Done | Internal Rust matrix/report/evaluator/ensure in `legacy_write_convergence.rs` | Historical matrix slice; superseded by W97 final matrix | No; ordinary send/stream stay `legacy_stream` and do not call it | W87 restriction, then W97 final matrix |
| W87 | LifeModel Materializer Caller Restriction | Done | Internal typed caller context/restriction evaluator in `legacy_write_convergence.rs`, `persist_life_model` signature update, and snapshot restore direct-save guard | Typed restriction remains active; W97 matrix admits only classified source-data compatibility, governed manual override, governed restore/import, accepted proposal apply, and materializer root callers | No; ordinary send/stream stay `legacy_stream` and only pass source-data compatibility context for existing daily-goal auto-checkin | W97 final matrix |
| W88 | Proposal Application Source-Specific Patch Mapping | Done | Backend-only private mapper/report/ensure in `commands/proposal.rs` | Historical mapping slice; superseded by W95 exact ProposalSource -> PatchSource variants | No; ordinary send/stream stay `legacy_stream` and do not call it | W95 mapping closure |
| W89 | Proposal Application Source-Specific Patch Audit / Readiness | Done | Backend-only private readiness entry/report/evaluator/ensure in `commands/proposal.rs` | Historical readiness slice; superseded by W95 exact mapping and W97 proposal-first convergence completion | No; ordinary send/stream stay `legacy_stream` and do not call W88/W89 helpers | W90-W97 convergence |
| W90 | Legacy Override Retirement: Builder Direct Apply | Done | Backend Builder command retirement in `commands/builder.rs` | `builder_apply_signals` is retired/fail-closed and writes no LifeModel; normal product flow remains `builder_create_proposals`; no dev/migration direct-apply override remains | No; ordinary send/stream stay `legacy_stream` and do not call it | W91 |
| W91 | Legacy Override Retirement: Calibration Direct/Evolution | Done | Backend Calibration command retirement in `commands/calibration.rs` | `apply_calibration(mode="direct")` and `run_micro_evolution` are retired/fail-closed for durable LifeModel writes; normal flow remains `calibration_create_proposals` / proposal mode | No; ordinary send/stream stay `legacy_stream` and do not call it | W92 |
| W92 | Legacy Override Retirement: Feedback Evolution | Done | Backend Feedback command retirement/read-only report in `commands/feedback.rs` | `apply_feedback_evolution` is retired/fail-closed for LifeModel and `evolution_rules` writes; reports are metadata-safe/read-only | No; ordinary send/stream stay `legacy_stream` and do not call it | W93 |
| W93 | Governed Snapshot Restore / Data Import | Done | Backend Version/Settings governed request flows and frontend request wrappers | `restore_snapshot` and `import_all_data` require explicit governed requests, pre-change snapshots, validation, and metadata-safe audit/count/hash results; no legacy override remains | No; ordinary send/stream stay `legacy_stream` and do not call it | W94 |
| W94 | Governed Manual LifeModel Editor Override | Done | Backend LifeModel editor governed request flow and frontend wrapper | `save_life_model` requires explicit user intent, risk acknowledgement, pre-change snapshot, typed materializer context, and metadata-safe audit | No; ordinary send/stream stay `legacy_stream` and do not call it | W95 |
| W95 | Proposal PatchSource Mapping Closure | Done | Core PatchSource variants plus proposal mapper/readiness update | Every ProposalSource maps to a dedicated PatchSource; accepted proposal apply has no Manual fallback blocker and `proposal_first_convergence_complete=true` | No; ordinary send/stream stay `legacy_stream` and do not call it | W96 |
| W96 | State / Daily Goal Boundary Reconciliation | Done | Legacy convergence boundary report/evaluator | State/Daily Goal remains source-data compatibility materialization only, not accepted durable LifeModel-HS truth | No; ordinary send/stream stay `legacy_stream`; existing auto-checkin remains source-data compatibility context only | W97 |
| W97 | Final Legacy Direct-Write Convergence Inventory | Done | Final inventory/materializer matrix/report tests in `legacy_write_convergence.rs` | `overall_converged=true`, `all_direct_writes_converged=true`, `high_risk_legacy_direct_write_count=0`, `proposal_first_convergence_complete=true`, metadata-safe reports, no raw payloads, no runtime/model/tool execution | No; ordinary send/stream stay `legacy_stream` and do not call W79-W97 helpers | W98 |
| W98 | Plan-Execute Product Contract / Weekly Scenario | Done | Core product contract in `plan_execute.rs` | Weekly-only scenario, max-step/risk/action bounds, metadata-safe authority/contract reports, direct writes/external side effects disallowed | No; ordinary send/stream stay `legacy_stream` and do not call it | W99 |
| W99 | Plan-Execute Session Store / Non-Default Commands | Done | Durable session store plus explicit Tauri commands | `PlanExecuteSession` persisted; create/get/list/update/finalize/cancel/execute commands are non-default product surface only | No; ordinary send/stream stay `legacy_stream` and do not call commands | W100 |
| W100 | Plan-Execute Review/Edit/Finalize Lifecycle | Done | Draft lifecycle and validation | Draft edits are bounded by contract; execution requires finalized/in-progress session; cancel/fail-closed paths covered | No; product session only | W101 |
| W101 | Proposal-First Step Execution | Done | Step execution helper plus ProposalStore integration | Read-only steps produce metadata-safe observations; write-like steps create idempotent Review Center proposals, no direct durable truth/external writes | No; product session only | W102 |
| W102 | Plan-Execute AgentRun Trace / Proposal Linkage | Done | `plan_execute_product` AgentRun trace | Metadata-safe session/status/governance/proposal counts; source run/session linkage; raw prompt/plan/LifeModel/memory/tool/proposal payloads not stored | No; product traces only | W103 |
| W103 | Frontend Weekly Planning Surface | Done | Workspace panel plus Tauri wrappers and tests | Create, edit, save draft, finalize, execute, observation/proposal/source-run links; explicit product commands only | No; ordinary Chat unchanged | W104 |
| W104 | Safety / Isolation / Regression Hardening | Done | Regression tests and command guard updates | Default Chat entrypoint forbidden list includes product commands; `PlanningSession` proposal/patch source mapping; metadata-safe UI trace tests | No; guard only for ordinary Chat | W105 |
| W105 | Docs / Progress / Final Verification Sync | Done | Docs/progress index plus verification matrix | W105 authority docs and UI trace status synced; default Chat remains `legacy_stream`; RuntimeStrategy maturity was still future work at the W105 boundary | No; docs/status only | W106 |
| W106 | RuntimeStrategy Descriptor / Registry Readiness | Done | Core RuntimeStrategy descriptor/readiness report | ReAct/PlanExecute executable descriptors are metadata-safe; readiness fails closed for missing, duplicate, mismatched, write-without-proposal-first, migration-granting, or metadata-unsafe descriptors | No; readiness only, no adapter execution | W107 |
| W107 | Strategy Selection Candidate Matrix | Done | Core StrategySelector report | Candidate matrix/explanation covers ReAct/PlanExecute support, reason code, governance, risk, planning, local model, HS packet, fallback and blocked state without raw prompt/tools/memory | No; selector only, no adapter/store writes | W108 |
| W108 | MultiStrategy Runtime Execution Report Envelope | Done | Core MultiStrategyRuntime output report | Output includes selector, registry, descriptor, payload, governance, side-effect budget, metadata-safe adapter summary, blocked state, and default Chat unchanged | No; preview/runtime surface only, default Chat unchanged | W109 |
| W109 | Runtime Strategy Registry Status Command | Done | Explicit Tauri command `get_runtime_strategy_registry_status` plus frontend wrapper | Non-default read-only maturity report lists executable and declarative future descriptors, reports no runtime/model/tool execution, no business writes, migration permission false | No; command is read-only and ordinary Chat does not call it | W110 |
| W110 | Preview/Product Strategy Trace Convergence | Done | Preview audit, Plan-Execute product trace, frontend trace parsing/UI | Shared vocabulary includes runtimeStrategyTraceKind, selectedStrategyKind, payloadKind, strategyDescriptorId, strategyCapabilityIds, selectionReasonCode, governanceDecisionKind, sideEffectBudget, registryReady, metadataSafe, defaultChatUnchanged | No; trace metadata only | W111 |
| W111 | Future Strategy Boundary Descriptors | Done | Declarative future strategy descriptors | Direct, Layered, Workflow, Proactive, Reflective appear as disabled/declarative-only future descriptors and are not executable/selectable capabilities | No; future taxonomy only | W112 |
| W112 | Default Chat Isolation / Side-Effect Hardening | Done | Regression tests and forbidden-call guard updates | Ordinary send/stream forbidden list includes W106-W113 command/helpers; readiness/status does not create AgentRun/Proposal/Evidence/Memory/LifeModel/MCP/Chat/external writes and is not migration authority | No; default Chat remains `legacy_stream` | W113 |
| W113 | Docs / Progress / Final Verification Sync | Done | Docs/progress index plus verification matrix | Historical RuntimeStrategy maturity handoff; superseded by W114-W123 ReAct Beta execution hardening | No; docs/status only | W114 |
| W114 | ReAct Beta Readiness Contract | Done | Core `react_beta` readiness report/evaluator | Covers loop/schema/registry/executor/trace/permission/proposal/UI/default Chat isolation, is metadata-safe, pure, and fixes `migration_permission=false` | No; readiness only | W115 |
| W115 | AgentLoop Action Schema / Parser Hardening | Done | Typed action request parsing and parser warnings | New `actions` schema and legacy `tool_calls` normalize into the same request shape; missing names/invalid args fail soft; broad prompt text remains final-only; raw model replies stay out of reports | No; internal parser only | W116 |
| W116 | Tool Registry Beta Taxonomy / Readiness | Done | `ToolRegistryBetaReadinessReport` | Required tool ids are classified as executable read, proposal-only, permission-gated, disabled/declarative-only, unsupported, or plugin-declared; calendar/email proposal tools stay proposal-only; plugin tools are not executable without executor evidence | No; registry/readiness only | W117 |
| W117 | ActionExecutor Manifest Authority | Done | Manifest-governed execution gate | Every execution path resolves manifest authority; unknown/disabled/declarative-only tools block; direct write/external side-effect tools respect `allow_writes=false`; errors and previews are metadata-safe | No; execution hardening only | W118 |
| W118 | AgentRun Action/Observation Trace Envelope | Done | `ReactActionTraceEnvelope` on `AgentAction` and `AgentObservation` | Records run/action/observation ids, step/tool indices, tool/source/category/risk/permission/status/proposal id, output preview/hash/counts, and timing metadata without raw payloads | No; trace metadata only | W119 |
| W119 | Permission Proposal / Replay Hardening | Done | Canonical ToolPermission proposal scope and replay preservation | Proposal payloads include canonical tool scope and risky input hash/length/preview instead of raw payload; replay keeps original action/observation identity and blocks unknown/declarative-only tools | No; explicit Review Center / replay only | W120 |
| W120 | Proposal-First Write Hardening | Done | Proposal-only write semantics and tests | LifeModel/Memory/file/calendar/email/task write-like ReAct tools create governed proposals only; `ExternalWriteAction` fallback remains blocked where taxonomy forbids it and size/minimization gates stay enforced | No; proposals only, no silent writes | W121 |
| W121 | Non-Default ReAct Beta Status Harness | Done | Tauri `get_react_beta_execution_status` and frontend wrapper | Returns readiness plus tool registry readiness and zero side-effect proof; runs no runtime/model/tool calls, writes no stores, and ordinary send/stream forbidden-call tests exclude it | No; explicit read-only status only | W122 |
| W122 | Runs/Trace UI Action Lifecycle Hardening | Done | Runs detail/list, ToolCallCard, RunTracePanel | UI shows tool/source/status/risk/permission/proposal/replay/observation metadata and redacted previews; raw prompt/tool/output/memory/file/web/email/PII payloads are not rendered | No; inspection surface only | W123 |
| W123 | Docs / Progress / Verification Sync | Done | Authority docs/progress index plus verification matrix | Docs mark W114-W123 complete, scope W113 as historical, preserve default Chat `legacy_stream`, state readiness/status are not migration permission, and record remaining Beta dependencies | No; docs/status only | W124 |
| W124 | Backend Completion Readiness / Contract Report | Done | Pure core report/evaluator in `lifemodel_backend_completion.rs` | Metadata-safe readiness/contract report only; reports prerequisites, blockers, default Chat isolation, governance readiness, and next schemas; no runtime/model/tool/business write/Tauri command | No; ordinary send/stream stay `legacy_stream` and do not call it | W125 |
| W125 | LifeEvent Schema And Store Contract | Done | Core typed schema plus `LifeEventStore` skeleton | Metadata-safe LifeEvent records with source refs, risk/privacy/domain, digest, safe summary, dedupe key, and raw-content blocking; LifeEvents are not durable LifeModel truth | No; no command/frontend/runtime/model/tool/default Chat effect | W126 |
| W126 | Signal Schema And Deterministic Extractor | Done | Core typed Signal schema plus deterministic extractor | Extracts only low-risk low-energy planning signals; includes confidence, polarity, uncertainty reasons, dedupe key, source event refs, extractor id/version, risk/privacy/domain; no LLM/model/runtime/tool execution | No; no command/frontend/default Chat effect | W127 |
| W127 | LifeEvent / Signal / Evidence Bridge | Done | Core bridge to `EvidenceStore` | Writes EvidenceStore candidate records only for metadata-safe, low-risk, sufficiently confident signals with lineage; high-risk/raw/low-confidence/missing-lineage/unsupported signals fail closed; writes no LifeModel/Memory/Heuristic/Chat/AgentRun/MCP audit/external records | No; ordinary send/stream stay `legacy_stream` and do not call it | W128 |
| W128 | Evidence Support / Opposition / Dedupe Graph | Done | Pure core evidence graph in `evidence_graph.rs` | Builds support/opposition links, affected-path dedupe clusters, source weights, and cluster summaries from existing EvidenceStore records; no store schema migration or business write | No; ordinary send/stream stay `legacy_stream` and do not call it | W129 |
| W129 | Conflict / Decay / Cooldown | Done | Pure graph metadata evaluator with injected `now` | Detects conflicts from opposing refs, Contradicted status, rejected ProposalOutcome evidence, and same affected-path cluster opposition; computes deterministic decay and rejected-similar cooldown metadata | No; no command/frontend/runtime/model/tool/default Chat effect | W130 |
| W130 | Evidence Timeline Read Model | Done | Metadata-safe timeline read model | Timeline exposes ids/type/path/status/confidence/risk/privacy/polarity/link counts/proposal and run refs/cluster id and hash/conflict-decay-cooldown state/timestamps; omits raw prompt/user text/assistant output/tool payload/LifeModel raw content | No; no command/frontend/runtime/model/tool/default Chat effect | W131 |
| W131 | Low-Risk Multi-Domain Maturation Candidate Generation | Done | Pure core `evaluate_maturation_engine_v1` over Evidence Graph clusters | Generates metadata-safe reviewable candidates for planning preference, energy pattern, work style, and communication preference only; high-risk identity/values/relationships/health/finance/privacy/long-term direction clusters fail closed | No; no command/frontend/runtime/model/tool/store/default Chat effect | W132 |
| W132 | Proposal Outcome To Evidence Convergence | Done | Core `proposal_outcome.rs` evidence convergence metadata | Accepted/edited/rejected maturation proposal outcomes create positive/corrective/negative ProposalOutcome evidence metadata with proposal/run/evidence lineage; edited payload is digest-only/not included; high-risk outcome risk fails closed | No default Chat effect; existing proposal command integration remains proposal outcome only | W133 |
| W133 | Candidate Suppression And Correction | Done | Pure core Maturation Engine suppression report | Suppresses candidates deterministically using opposing evidence, graph conflict, decay, rejected-similar cooldown, and rejected-similar history; reports ids/hashes/counts/reasons only, no raw source content | No; no command/frontend/runtime/model/tool/store/default Chat effect | W134 |
| W134 | Accepted Guidance Lifecycle | Done | Pure core accepted guidance lifecycle in `accepted_guidance.rs` plus HeuristicStore constraint metadata | Converts accepted maturation candidate proposals into Trial HeuristicStore guidance with source proposal/evidence/run lineage, domain/trigger/guidance digest, priority, privacy/model/tool constraints, usage metadata, and rollback/archive path; unsafe active activation or policy relaxation fails closed | No; no command/frontend/runtime/model/tool/default Chat effect; writes only the explicit Trial heuristic asset | W135 |
| W135 | Governed Materialized LifeModel View Provenance | Done | `LifeModel` compatibility materializer provenance | Compatibility YAML carries proposal/evidence/patch/heuristic source ids and digests, explicit compatibility-materialized-view provenance, and accepted_source_of_truth=false / durable_truth_materialized=false | No; no command/frontend/runtime/model/tool/default Chat effect | W136 |
| W136 | Version Diff And Rollback Read Model | Done | Pure core LifeModel version read model in `accepted_guidance.rs` | Adds metadata-safe diff/rollback references for materialized view provenance and accepted guidance ids/digests/status/source refs; rollback references require proposal and omit raw LifeModel/guidance content | No; no command/frontend/runtime/model/tool/default Chat effect | W137 |
| W137 | RuntimeHSPacket V2 Guidance Contract | Done | Core `hs_selector.rs` packet/audit metadata | Adds metadata-safe selected guidance refs with id/digest/type/status/domain/impact/risk/privacy/source-lineage/policy-boundary summaries for accepted/trial accepted guidance assets only; seeded built-in heuristics remain `selected_heuristics` and are not emitted as `guidance_refs`; policy-relaxing guidance fails closed and raw guidance is omitted from audit/read-model metadata | No; ordinary send/stream stay `legacy_stream` and do not call runtime guidance pipeline | W138 |
| W138 | ReAct Guidance Consumption | Done | Non-default/runtime `AgentLoop` + `AgentRuntime` path | ReAct runtime consumes selected guidance through metadata-safe prompt summaries, gentle-planning config caps, action-boundary HS packet propagation, behavior checks, and trace metadata only when `RuntimeGuidanceConsumptionMode::ExplicitRuntime` is enabled; default mode is disabled and preserves ordinary prompt/config shape | No; default Chat routing unchanged and ordinary Chat does not consume accepted guidance | W139 |
| W139 | Plan-Execute Guidance Consumption | Done | `PlanExecuteService` weekly planning product path | Weekly planning drafts materially change under selected gentle planning guidance only when explicit runtime guidance consumption mode is enabled; write-like steps remain Review Center proposal-first; product contract reports selected guidance metadata only in explicit mode | No; only existing explicit Plan-Execute product path, no ordinary Chat route change | W140 |
| W140 | Runtime Guidance Trace And Read Model | Done | `GuidanceImpactReadModel` in `hs_selector.rs`, Plan-Execute report linkage, AgentRun HS audit refs | Guidance Impact read model links selected guidance to run/strategy/affected surfaces using ids/digests/counts/status/type/impact only; omits raw guidance, prompt, user text, assistant output, memory, LifeModel, and tool payloads | No; read model/trace metadata only and no default Chat migration permission | W141 |
| W141 | ModelRouter / Privacy HS Hardening | Done | `ModelRouter::score_provider`, `route`, and `route_with_hs_packet` | High/Critical privacy hard-filters non-local providers before scoring; HS LocalOnly from selected policy refs or audit ids selects local `ollama`, route_type `local`, prefer_local=true, privacy LocalOnly, no cloud fallback, metadata-safe `local_only` governor report, and fail-closed no-local behavior | No; ordinary send/stream stay `legacy_stream` and must not call W141 helpers except existing fail-closed HS packet boundary | W142 |
| W142 | ActionExecutor HS Tool Governance | Done | `ActionExecutor` manifest/source gate and HS proposal-first write paths | Unsupported Plugin/A2A tools block before permission replay or execution; HS direct external write paths remain proposal-first and attach metadata-safe governance reports; no real provider/plugin executor is added or advertised | No; non-default/runtime executor governance only, no ordinary Chat route change | W143 |
| W143 | Governor Unified Decision Report | Done | `LifeModelGovernor`, `GovernorDecisionReport`, and shared governance inputs | Shared metadata-safe report classifies allow/block/confirm/proposal-first/local-only decisions for maturation, model route, tool action, memory write, and external write; omits raw prompt/user text/assistant output/memory/LifeModel/tool payload | No; report/read model only and not migration permission | W144 |
| W144 | Weekly Planning Golden Path | Done | Pure core `run_weekly_planning_golden_path` | Proves the weekly planning guidance loop across selected RuntimeHSPacket guidance, Plan-Execute draft/finalize/step metadata, proposal-first write-like step refs, outcome evidence, and future planning guidance without raw payloads | No; pure backend/core proof only, ordinary send/stream stay `legacy_stream` and do not call golden path helpers | W145 |
| W145 | Low-Energy Support Golden Path | Done | Pure core `run_low_energy_support_golden_path` | Proves LifeEvent/Signal/Evidence to accepted guidance to explicit runtime behavior-change metadata, while high-risk truth materialization, runtime/model/tool calls, command/frontend surfaces, and durable LifeModel/Memory/external provider writes remain absent | No; pure backend/core proof only and not migration permission | W146 |
| W146 | Preference Correction Golden Path | Done | Pure core `run_preference_correction_golden_path` | Proves rejection/edit outcomes create negative/corrective evidence and deterministically suppress or change future behavior; no Tauri command, UI, ordinary send/stream replacement, runtime executor, model call, real tool call, or durable LifeModel/Memory/external provider write is added | No; ordinary Chat must not call W144-W146 helpers or treat golden path ready as migration permission | W147 |
| W147 | UI Read Model Contract Freeze | Done | Pure core `backend_contract_freeze.rs` read-model wrappers | Freezes metadata-safe contracts for Learning Inbox, Evidence Timeline, Proposal Review, Runtime Trace, Guidance Impact, Privacy Controls, and LifeModel Overview; payloads contain ids/status/counts/paths/digests/booleans only and omit raw prompt/user/assistant/memory/LifeModel/tool/guidance content | No; no command/frontend/runtime/model/tool/store/default Chat effect | W148 |
| W148 | Final Backend Completion Gate | Done | Pure core `evaluate_final_backend_completion_gate` | Metadata-safe read-only final gate report proves or lists blockers for LifeModel Maturity, Runtime Driven, Governance/Privacy, and UI Read Model gates; explicitly reports default Chat isolation, proposal-first boundaries, raw-content exclusion, LocalOnly privacy, tool governance, golden path coverage, remaining Beta blockers, and zero side-effect counts | No; not migration permission and ordinary Chat must not call it | W149 |
| W149 | Docs / Progress / Verification Sync | Done | Docs/progress/index sync | `AGENTS.md`, `README.md`, `plans/README.md`, backend completion spec, runtime program, and this progress index mark W149 complete and make stale Goal 8-next/default-Chat-migration references defer to the updated authority docs | No; docs/status only | Future Skill Runtime, product surface design, or separately reviewed default Chat route migration Goal |

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
- W89 adds only backend/internal proposal application PatchSource
  audit/readiness proof. It verifies the W88 exact/fallback mapping table,
  apply-path ensure/resolver usage, no hardcoded BuilderReview reintroduction,
  and no default Chat helper calls. Fallback source strategy remains a blocker,
  so proposal-first convergence is still not complete.
- W114-W123 are ReAct Beta Execution Hardening only. They make ReAct action
  parsing, tool taxonomy/readiness, manifest authority, trace visibility,
  permission/replay, and proposal-first write behavior harder and more
  inspectable, but they do not replace default Chat, do not attach a controlled
  executor to ordinary send/stream, do not grant migration permission, and do
  not declare full Beta complete.
- W124-W127 are Backend Completion Goal 1 / Master Contract And Schemas only.
  They add pure backend schemas, report, deterministic extraction, and a safe
  Signal -> EvidenceStore candidate bridge. They do not run runtime/model/tool
  paths, add commands or frontend surfaces, write durable LifeModel/Memory/
  Heuristic truth, write AgentRun/MCP audit/external records, or affect default
  Chat routing.
- W128-W130 are Backend Completion Goal 2 / Evidence Graph v1 only. They add a
  pure backend graph/timeline read model, conflict/decay/cooldown state, source
  weights, and rejected-similar cooldown metadata. They do not run
  runtime/model/tool paths, add commands or frontend surfaces, write durable
  LifeModel/Memory/Heuristic truth, write AgentRun/MCP audit/external records,
  materialize accepted truth, or affect default Chat routing.
- W131-W133 are Backend Completion Goal 3 / Maturation Engine v1 only. They add
  low-risk graph-based candidate generation, proposal outcome evidence
  convergence, and deterministic suppression/correction. They do not
  materialize LifeModel truth, activate heuristics, add commands or frontend
  surfaces, run runtime/model/tool paths, or affect default Chat routing.

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
Readiness complete -> W90 Builder legacy direct apply retirement complete ->
W91 Calibration direct/evolution retirement complete -> W92 Feedback evolution
retirement complete -> W93 governed Snapshot restore / Data import complete ->
W94 governed manual LifeModel editor override complete -> W95 Proposal
PatchSource mapping closure complete -> W96 State / Daily Goal boundary
reconciliation complete -> W97 final Legacy Direct-Write Convergence inventory
complete -> W98 Plan-Execute product contract complete -> W99 durable session
store and non-default command surface complete -> W100 review/edit/finalize
lifecycle complete -> W101 proposal-first step execution complete -> W102
AgentRun trace/proposal linkage complete -> W103 frontend weekly planning
surface complete -> W104 safety/isolation regression hardening complete -> W105
docs/progress/verification sync complete -> W106 RuntimeStrategy
descriptor/readiness complete -> W107 selection candidate matrix complete ->
W108 execution report envelope complete -> W109 non-default registry status
command complete -> W110 preview/product trace convergence complete -> W111
future strategy declarative boundary complete -> W112 default Chat isolation
hardening complete -> W113 docs/progress/verification sync complete -> W114
ReAct Beta readiness contract complete -> W115 AgentLoop action schema/parser
hardening complete -> W116 Tool Registry Beta taxonomy/readiness complete ->
W117 ActionExecutor manifest authority complete -> W118 AgentRun
action/observation trace envelope complete -> W119 permission proposal/replay
hardening complete -> W120 proposal-first write hardening complete -> W121
non-default ReAct Beta status harness complete -> W122 Runs/Trace lifecycle UI
hardening complete -> W123 docs/progress/verification sync complete ->
W124 backend completion readiness/contract report complete -> W125 LifeEvent
schema/store contract complete -> W126 Signal schema/deterministic extractor
complete -> W127 safe LifeEvent/Signal/Evidence bridge complete -> W128
Evidence support/opposition/dedupe graph complete -> W129 conflict/decay/
cooldown complete -> W130 Evidence Timeline read model complete -> W131
low-risk multi-domain maturation candidate generation complete -> W132
proposal outcome to evidence convergence complete -> W133 candidate
suppression/correction complete -> W134 accepted guidance lifecycle complete
-> W135 governed materialized LifeModel view provenance complete -> W136
version diff and rollback read model complete -> W137 RuntimeHSPacket v2
guidance contract complete -> W138 ReAct guidance consumption complete -> W139
Plan-Execute guidance consumption complete -> W140 Guidance Impact read model
complete -> W141 ModelRouter/Privacy HS hardening complete -> W142
ActionExecutor HS tool governance complete -> W143 Governor unified decision
report complete -> W144 Weekly Planning golden path complete -> W145
Low-Energy Support golden path complete -> W146 Preference Correction golden
path complete -> W147 UI read model contract freeze complete -> W148 final
backend completion gate complete -> W149 docs/progress/verification sync
complete.
Backend Completion Goal 8 is complete. Future Beta hardening can move to Skill
Runtime or product surface work from the W149 backend completion baseline.
Future default Chat
executor implementation discussion may build on the W65-W72 proofs only through
a separately reviewed task; keep default Chat on legacy_stream unless that
separate task explicitly implements, reviews, verifies, and authorizes a route
change.
```

`make ci` remains the release gate for implementation tasks. For docs-only
index整理, `git diff --check` plus targeted `rg` validation is sufficient unless
code or package configuration changes.
