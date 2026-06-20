# LifeModel-HS Legacy Write Path Audit

Date: May 28, 2026
Last updated: June 3, 2026
Status: W97 Legacy Direct-Write Convergence complete

Scope: LMHS-10 audit for the Post-Beta LifeModel-HS MVP. This document uses
ADR 0013 as the governance baseline and treats YAML LifeModel as a compatibility
view during migration, not the target HS source of truth.

Verification command run for this audit:

```sh
rg -n "save_life_model|update_life_model|apply.*proposal|save_memory|save_state|evolution|calibration|builder" openlife-core src-tauri
```

Risk classes use the LMHS-10 vocabulary:

- already proposal-first
- low-risk transient state
- read-only/materialized
- legacy direct write requiring future convergence
- disabled/declarative-only

## W79 Machine-Readable Inventory Guard

W79 adds `src-tauri/src/legacy_write_convergence.rs` as a backend-only/internal
Rust inventory guard over this audit map. The guard defines
`LegacyWriteRiskClass`, `LegacyWriteConvergenceStatus`,
`LegacyWritePathKind`, `LegacyWriteInventoryEntry`,
`LegacyWriteConvergenceReport`, `legacy_write_convergence_inventory`,
`evaluate_legacy_write_convergence_inventory`, and
`ensure_legacy_write_convergence_inventory_guard`.

This is an inventory and regression guard only. It does not remove, gate,
rewrite, or converge any direct-write path; it does not add a Tauri command,
frontend surface, runtime/model/tool execution, store write, product behavior
change, or default Chat change. Its expected report has
`inventory_ready=true` while keeping `overall_converged=false` and
`all_direct_writes_converged=false`, because high-risk legacy direct-write
blockers remain.

W79 makes the table below a testable development entry for future convergence
slices. Future work must update both the machine-readable inventory and this
audit when an actual path is changed, and must not mark a blocker converged
until the implementation and regression tests prove that convergence.

## W80 Manual LifeModel Editor Override Audit Guard

W80 adds a backend-only/internal metadata-safe audit guard to
`src-tauri/src/commands/life_model.rs::save_life_model_with_state`. The manual
editor save behavior remains available and remains a legacy direct write. After
a successful save, `record_manual_lifemodel_override_audit_with_state` writes a
`manual_lifemodel_override_audit` analytics event with only
`source=manual_lifemodel_editor`, before/after hashes, rough changed section
names/count, risk class, timestamp, command/function name, and
manualOverride/proposalFirst/stillLegacyDirectWrite flags.

The audit must not contain raw LifeModel JSON, raw identity values, raw goals,
raw relationships, raw health/finance/privacy text, raw prompt/output/tool
payload, or full before/after model payload. W80 does not create a Proposal,
AgentRun, Heuristic, Patch, runtime/model/tool invocation, frontend surface, or
default Chat integration. The W79 inventory marks the manual editor guard as
present, but `manual_lifemodel_editor` remains a high-risk legacy direct-write
blocker and must not be treated as proposal-first converged.

## W81 Builder Legacy Direct Apply Dev-Gate / No-Signal Guard

W81 adds a backend-only dev/migration gate to
`src-tauri/src/commands/builder.rs::builder_apply_signals`. The command now
fails closed by default and only enters the remaining legacy direct write path
when an explicit `BuilderLegacyDirectApplyOverride` with dev/migration purpose
is supplied. This preserves the normal product path through
`builder_create_proposals` and Review Center.

W81 also changes the no-signal completion branch in `builder_step_with_state`
to perform session-only cleanup without persisting durable LifeModel truth. The
legacy direct apply response is metadata-safe and no longer returns raw model
payloads, snapshots, feedback audit details, or run ids. Because an override can
still write durable LifeModel truth, Builder legacy direct apply remains a
high-risk legacy direct-write blocker until removed or converted to
proposal-first.

## W82 Calibration Direct Apply Legacy Gate / Proposal-First Default

W82 adds a backend dev/migration gate to
`src-tauri/src/commands/calibration.rs::apply_calibration(mode="direct")` and
`run_micro_evolution`. Both commands now fail closed by default and only enter
legacy direct persistence when an explicit
`CalibrationLegacyDirectApplyDevMigrationOverride` with dev/migration purpose
is supplied. Normal Calibration product flow remains
`calibration_create_proposals` / proposal mode, and Dashboard's micro-evolution
button now creates Calibration proposals rather than invoking direct
micro-evolution persistence.

The legacy direct/evolution responses are metadata-safe: they return counts,
snapshot ids, warnings, and bounded signal counts only, and do not return raw
LifeModel payloads, raw calibration changes/reasons, or raw evolution payloads.
Because an override can still write durable LifeModel truth, Calibration
direct/evolution remains a high-risk legacy direct-write blocker until removed
or converted to proposal-first.

## W83 Feedback Evolution Legacy Direct Apply Gate / Proposal-First Candidate Path

W83 adds a backend dev/migration gate to
`src-tauri/src/commands/feedback.rs::apply_feedback_evolution`. The command now
fails closed by default and only enters legacy direct persistence when an
explicit `FeedbackEvolutionLegacyDirectApplyOverride` with dev/migration
purpose is supplied. The legacy response is metadata-safe: it returns
counts/status/warnings only and does not return raw feedback text, raw
conversation inference, raw LifeModel, or raw evolution rule payloads.

`generate_evolution_report` is now read-only. It returns metadata-safe
counts/status and does not write LifeModel or `evolution_rules` truth. Feedback
signals remain low-risk source data, not accepted truth. Because an override can
still write durable LifeModel truth, Feedback evolution direct apply remains a
high-risk legacy direct-write blocker until removed or converted to
proposal/evidence-first.

## W84 Snapshot Restore / Data Import Legacy Direct Write Gate

W84 adds backend dev/migration/manual-restore gates to
`src-tauri/src/commands/version.rs::restore_snapshot` and
`src-tauri/src/commands/settings.rs::import_all_data`. Both commands now fail
closed by default and only enter legacy direct persistence when an explicit
`SnapshotRestoreLegacyDirectApplyOverride` or
`DataImportLegacyDirectApplyOverride` with a narrow migration/manual restore
purpose is supplied.

The legacy responses are metadata-safe. Snapshot restore returns snapshot ids,
status, and whether a pre-restore snapshot was created; it does not return the
restored LifeModel or snapshot YAML. Data import returns imported
message/vector counts and status; it does not return raw LifeModel, messages,
vectors, or the imported payload. Export and read-only snapshot inspection
paths are unchanged. Because the overrides can still replace durable LifeModel,
Memory, and vector truth, restore/import remain high-risk legacy direct-write
blockers until removed or converted to governed rollback/migration flows.

## W85 State / Daily Goal Source Data Boundary Proof

W85 adds a backend-only/internal source-data boundary proof in
`src-tauri/src/legacy_write_convergence.rs` for
`state_daily_goal_direct_writes`. The new
`StateSourceDataBoundaryReport`, `evaluate_state_source_data_boundary`, and
`ensure_state_source_data_boundary` return only metadata-safe path ids,
source-data / low-risk transient classification, default/ordinary Chat
unchanged booleans, compatibility_lifemodel_materialized_write=true,
writes_current_lifemodel_compatibility_view=true,
accepted_durable_hs_truth_write=false, active_hs_lifemodel_patch=false,
proposal_required_for_hs_truth_promotion=true, and blocking reason codes.

The proof intentionally acknowledges the current compatibility materialization:
State / Daily Goal writes the current LifeModel compatibility view / YAML
through `persist_life_model`. W85 classifies that write as source-data
compatibility materialized state, not accepted durable LifeModel-HS truth.

This is not a proposal-first conversion and not a fully-converged marker. It
does not change `record_state`, `add_daily_goal`, `update_daily_goal`,
`delete_daily_goal`, `toggle_daily_goal`, or `try_auto_checkin_daily_goals`
behavior; it does not add a Tauri command or frontend surface; it does not
create ProposalStore, EvidenceStore, AgentRun, runtime/model/tool, or default
Chat integration. State history and Daily Goal entries remain source data /
low-risk transient compatibility state, not accepted durable LifeModel-HS
truth. Any future promotion from state source data into durable LifeModel-HS
truth must be a separate proposal-first slice.

## W86 LifeModel Compatibility Materializer Caller Matrix

W86 adds a backend-only/internal caller matrix in
`src-tauri/src/legacy_write_convergence.rs` for the current LifeModel
compatibility materializer and direct save boundary. It defines
`LifeModelMaterializerCallerKind`,
`LifeModelMaterializerCallerRisk`,
`LifeModelMaterializerCallerGovernanceState`,
`LifeModelMaterializerCallerMatrixEntry`,
`LifeModelMaterializerCallerMatrixReport`,
`lifemodel_materializer_caller_matrix`,
`evaluate_lifemodel_materializer_caller_matrix`, and
`ensure_lifemodel_materializer_caller_matrix`.

The matrix classifies all current production entries found for this slice: 16
`persist_life_model` callsites plus 3 production `LifeModelManager::save`
related entries. It explicitly distinguishes the materializer root, ordinary
Chat daily-goal auto-checkin source-data compatibility writes, State/Daily Goal
source-data compatibility materialization, accepted proposal apply, audited
manual override, Builder/Calibration/Feedback guarded legacy dev-migration
override paths, and Snapshot restore/Data import gated override paths.

W86 is metadata-safe and reports no raw LifeModel, memory, chat, tool, prompt,
assistant output, or daily-goal payload. It does not add a command/frontend
surface, does not run runtime/model/tool, does not write Chat/AgentRun/Evidence
or external records, does not change default Chat, does not change the
`persist_life_model` signature, does not retire any legacy path, and does not
grant migration permission or runtime authority. It is not convergence
completion; `proposal_first_convergence_complete=false` remains explicit. W86
is the preparation layer for W87 LifeModel materializer caller restriction.

## W87 LifeModel Materializer Caller Restriction

W87 adds the backend-only/internal caller restriction layer on top of the W86
matrix. It defines `LifeModelMaterializerCallerPurpose`,
`LifeModelMaterializerCallerContext`,
`LifeModelMaterializerCallerRestrictionReport`,
`evaluate_lifemodel_materializer_caller_restriction`,
`ensure_lifemodel_materializer_caller_allowed`, and
`ensure_lifemodel_materializer_caller_restriction` in
`src-tauri/src/legacy_write_convergence.rs`.

`src-tauri/src/lib.rs::persist_life_model` now requires a typed caller context.
All 16 production `persist_life_model` callsites pass their W86 stable id,
kind, and governance purpose explicitly. The restriction evaluator fails closed
for unknown stable ids, kind/purpose mismatches, metadata-unsafe entries, raw
content flags, migration permission, runtime authority grants, source-data
callers marked as accepted durable LifeModel-HS truth, manual editor paths
marked proposal-first or fully converged, and restore/import/dev migration
override paths marked fully converged.

`src-tauri/src/commands/version.rs::restore_snapshot_direct_apply_after_gate`
now checks the `snapshot_restore_legacy_direct_apply` context before its direct
`LifeModelManager::save(&restored_model)`. This guard is placed after the
existing W84 restore override and before the actual save, preserving restore
snapshot save semantics while adding caller restriction.

W87 is metadata-safe and does not expose raw LifeModel, memory, chat,
daily-goal, tool, prompt, or assistant payloads. It does not add a Tauri
command, frontend/Settings surface, runtime/model/tool execution, or
Chat/AgentRun/Evidence/Proposal/Memory/MCP audit/external write. It does not
change default Chat routing, does not retire legacy paths, and does not mark any
legacy blocker fully converged. At W87, accepted proposal apply still had
`proposal_first_convergence_complete=false` because source-specific proposal
patch mapping and audit/readiness were still pending.

## W88 Proposal Application Source-Specific Patch Mapping

W88 adds a backend-only/internal PatchSource mapping report/ensure/resolver in
`src-tauri/src/commands/proposal.rs` for accepted LifeModel proposal apply.
`apply_proposal_to_state` no longer hardcodes `PatchSource::BuilderReview`
when creating the `LifeModelPatch` for PatchStore/audit persistence.

The W88-era explicit mapping was BuilderReview -> BuilderReview, CalibrationRun
-> Calibration, FeedbackEvolution -> Evolution, and Manual -> Manual, with
temporary metadata-safe Manual fallback for ChatConversation, ProactiveAgent,
SkillRuntime, Plugin, and MemoryGovernance. W95 supersedes that temporary state
by adding dedicated PatchSource variants for every ProposalSource.

The W88 mapping report contains only source enum names, mapped PatchSource,
fixed booleans, blocker codes, and follow-up text; it must not contain raw
proposal payloads, raw LifeModel patch values, memory text, chat text, or tool
payloads. W88 adds no Tauri command, frontend/Settings surface,
runtime/model/tool execution, default Chat routing change, or legacy path
retirement. W95 later closes the fallback source policy and marks
`proposal_first_convergence_complete=true`.

## W89 Proposal Application Source-Specific Patch Audit / Readiness

W89 adds a backend-only/internal readiness entry/report/evaluator/ensure in
`src-tauri/src/commands/proposal.rs` for the W88 ProposalSource -> PatchSource
mapping. It is audit/readiness proof only: no Tauri command, frontend/Settings
surface, runtime/model/tool execution, product behavior expansion, legacy path
retirement, or default Chat routing change.

The W89 readiness report proves `apply_proposal_to_state` still calls
`ensure_lifemodel_proposal_patch_source_mapping`, still passes
`resolve_lifemodel_patch_source_for_proposal(proposal)` into
`LifeModelPatch::from_proposal`, and does not reintroduce a hardcoded
`PatchSource::BuilderReview` in the apply path. It also proves ordinary
`send_message` / `start_stream_message` do not call the W88/W89 proposal
PatchSource mapping or readiness helpers.

W89's exact mappings were BuilderReview -> BuilderReview, CalibrationRun ->
Calibration, FeedbackEvolution -> Evolution, and Manual -> Manual. Its
temporary metadata-safe fallback mappings were ChatConversation, ProactiveAgent,
SkillRuntime, Plugin, and MemoryGovernance -> Manual. W95 closes that follow-up:
PatchSource now has dedicated variants for every ProposalSource, fallback count
is zero, and `proposal_first_convergence_complete=true`. The report remains
metadata-safe and contains no raw proposal payload, raw LifeModel patch value,
memory text, chat text, or tool payload.

## W90-W97 Final Legacy Direct-Write Convergence

W90 retires Builder legacy direct apply. `builder_apply_signals` fails closed
with a retired-path error and writes no LifeModel; the normal product path is
`builder_create_proposals`.

W91 retires Calibration direct/evolution durable LifeModel writes.
`apply_calibration(mode="direct")` and `run_micro_evolution` fail closed for
durable writes; normal Calibration flow remains `calibration_create_proposals`
or proposal mode.

W92 retires Feedback evolution durable LifeModel and `evolution_rules` writes.
`apply_feedback_evolution` fails closed for writes and report paths remain
metadata-safe/read-only.

W93 converts Snapshot restore and Data import to governed operations. They
require explicit governed requests, pre-change snapshots, request/payload
validation, materializer caller restriction, and metadata-safe audit/count/hash
results. Legacy restore/import override types are removed.

W94 converts manual LifeModel editor save to a governed manual override. It
requires explicit user intent, risk acknowledgement, a pre-change snapshot,
typed materializer context, and metadata-safe audit; the frontend wrapper sends
the required governed request.

W95 adds dedicated PatchSource variants for ChatConversation, ProactiveAgent,
SkillRuntime, Plugin, and MemoryGovernance, so every ProposalSource maps exactly
and no Manual fallback blocker remains.

W96 keeps State/Daily Goal writes classified as source-data compatibility
materialization only. They are not accepted durable LifeModel-HS truth and any
truth promotion still requires a future proposal-first path.

W97 updates the machine-readable inventory and materializer matrix. The final
report is metadata-safe/raw-payload-free with `overall_converged=true`,
`all_direct_writes_converged=true`, `high_risk_legacy_direct_write_count=0`,
and `proposal_first_convergence_complete=true`. Default Chat remains
`legacy_stream`, and ordinary `send_message` / `start_stream_message` do not
call W79-W97 helpers.

## Current Write Paths

| Area | Path / entry points | Risk class | Current guard | Future action |
| --- | --- | --- | --- | --- |
| LifeModel save primitive / materializer caller restriction | `openlife-core/src/life_model.rs::LifeModelManager::save`; `src-tauri/src/lib.rs::persist_life_model`; `lifemodel_materializer_caller_matrix`; `LifeModelMaterializerCallerContext` / restriction report | read-only/materialized compatibility primitive with governed caller restriction | W97 classifies the materializer root, source-data compatibility callers, governed manual override, governed restore/import, and accepted proposal apply. Unknown/mismatched callers fail closed; reports are metadata-safe; no migration/runtime authority is granted. | Keep the typed restriction in force for every new materializer caller. |
| Manual LifeModel editor | `src-tauri/src/commands/life_model.rs::save_life_model`; `frontend/src/pages/LifeModelEditor.tsx` | governed manual override | W94 requires a governed request with explicit user intent, risk acknowledgement, and pre-change snapshot. Save uses typed materializer context and returns metadata-safe audit without raw model content. | Consider future patch/proposal UX for higher assurance, but the legacy direct-write blocker is closed. |
| Review Center apply/edit | `src-tauri/src/commands/proposal.rs::accept_proposal_with_state`, `edit_proposal_with_state`, `apply_proposal_to_state`; `openlife-core/src/life_model/patch_store.rs` | already proposal-first | Safe Mode blocks apply/edit; proposal must be pending/postponed; payload validation runs before apply; LifeModel proposals create before/after snapshots and PatchStore records; W95 maps every ProposalSource to a dedicated PatchSource with no fallback blocker; MemoryWrite checks duplicate content; ExternalWriteAction re-validates safe paths, hash, UTF-8, and size. | Keep this as the convergence target for durable HS and risky LifeModel mutations. |
| Low-risk batch proposal apply | `src-tauri/src/commands/proposal.rs::batch_accept_low_risk_proposals` | already proposal-first | Safe Mode guard; accepts only pending low-risk proposals. | Keep limited to low risk; do not extend to high-risk identity, values, mission, long-term goals, sensitive relationships, or privacy boundaries. |
| Proposal storage | `openlife-core/src/agent/proposal_store.rs::{create_proposal,update_proposal}` | already proposal-first | Proposals are persisted with type, source, risk, status, run id, and before/after payloads; status transitions are explicit through Review Center commands. | Continue linking generated HS proposals to run/evidence ids. |
| Builder normal flow | `src-tauri/src/commands/builder.rs::builder_create_proposals`; `frontend/src/pages/BuilderPage.test.tsx` | already proposal-first | Finished sessions with pending signals are sent to ProposalStore; frontend tests assert `builder_apply_signals` is not called in the normal review flow. | Keep this as the only product Builder write path. |
| Builder legacy direct flow | `src-tauri/src/commands/builder.rs::builder_apply_signals`; no-signal completion branch in `builder_step_with_state` | retired no-write compatibility path | W90 retires direct apply. `builder_apply_signals` fails closed and writes no LifeModel; no-signal completion remains session-only/no durable LifeModel write. | Keep normal writes on `builder_create_proposals`. |
| Builder session persistence | `openlife-core/src/builder/store.rs`; `src-tauri/src/commands/builder.rs::{builder_start,builder_step,builder_delete_session}` | low-risk transient state | Stores unfinished/review sessions separately from accepted LifeModel truth. | Keep as transient workflow state; delete or expire stale sessions. |
| Calibration proposal flow | `src-tauri/src/commands/calibration.rs::calibration_create_proposals`; `apply_calibration(mode = "proposal")`; `frontend/src/pages/CalibrationPage.tsx`; `frontend/src/pages/DashboardPage.tsx` | already proposal-first | Change risk is assessed before proposal creation; identity values, long-term goals, and similar paths are high risk; proposals link to AgentRun; normal Calibration/Dashboard product flow writes ProposalStore rather than durable LifeModel truth. | Keep this as the product default and route all calibration changes through Review Center. |
| Calibration direct/evolution apply | `src-tauri/src/commands/calibration.rs::{run_micro_evolution,apply_calibration(mode = "direct")}` | retired no-write compatibility path | W91 retires durable LifeModel writes from both paths; they fail closed and write no LifeModel. | Keep normal flow on `calibration_create_proposals` / proposal mode. |
| Feedback signals | `openlife-core/src/feedback.rs::{save_feedback,log_event,save_conversation_inference,fetch_evolution_signals}`; `src-tauri/src/commands/feedback.rs::{save_feedback,log_analytics_event}`; `src-tauri/src/lib.rs::capture_conversation_signals` | low-risk transient state | Append-only local feedback, analytics, and inference rows; they are source data signals, not accepted LifeModel truth. | Promote useful signals into EvidenceStore records and proposals; add retention/deletion policy per ADR 0013. |
| Feedback evolution direct apply | `src-tauri/src/commands/feedback.rs::apply_feedback_evolution` | retired no-write compatibility path | W92 retires durable LifeModel and `evolution_rules` writes; command fails closed for writes and returns only metadata-safe candidate counts/status. | Future feedback-derived changes should create reviewable Proposal/Evidence records. |
| Feedback evolution report | `src-tauri/src/commands/feedback.rs::generate_evolution_report`; `openlife-core/src/feedback.rs::generate_evolution_report` | read-only low-risk report | W83 read-only report: command returns metadata-safe counts/status only and does not write LifeModel or `evolution_rules` truth. | Keep report read-only; future candidate creation may write only reviewable Proposal/Evidence records, not active rules or LifeModel truth. |
| State history and current state | `openlife-core/src/memory.rs::record_state_entry`; `src-tauri/src/commands/state.rs::record_state` | low-risk transient source data | W85 boundary proof present: user-initiated state samples append to `state_history` and currently materialize the LifeModel compatibility view / YAML through `persist_life_model`; current custom state dimension and `last_updated` are source data / low-risk transient compatibility state, not accepted durable LifeModel-HS truth, not an active HS LifeModel patch, and not automatically promoted. | Move to StateStore with TTL, source, confidence, and privacy metadata; any promotion to durable identity/preference/state truth must be a separate proposal-first slice. |
| Daily goals and chat check-in | `src-tauri/src/commands/state.rs::{add_daily_goal,update_daily_goal,delete_daily_goal,toggle_daily_goal}`; `src-tauri/src/lib.rs::try_auto_checkin_daily_goals` call sites | low-risk transient source data | W85 boundary proof present: daily goals/task completion currently materialize the LifeModel compatibility view / YAML through `persist_life_model` and remain source data / low-risk transient compatibility state; chat auto-check-in is keyword-triggered and does not edit long-term goal definitions or accepted LifeModel-HS truth. | Keep as short-lived task state or migrate to StateStore; any promotion to long-term goals or durable LifeModel-HS truth remains proposal-first future work. |
| Raw chat and memory records | `openlife-core/src/memory.rs::{save_message,save_memory_record}`; `src-tauri/src/lib.rs::persist_chat_message_if_needed`; `src-tauri/src/commands/memory.rs::index_memory_chunk` | low-risk transient state | Local raw/source records with privacy tags; user/manual indexing is explicit; raw memory is not accepted HS truth. | Preserve as raw life data with retention/deletion controls; generated durable memory claims should use MemoryWrite proposals or EvidenceStore. |
| Memory proposals | `openlife-core/src/agent/proposal_generators/chat.rs`; `openlife-core/src/agent/proposal_engine.rs::MemoryProposalGenerator`; `src-tauri/src/commands/proposal.rs::MemoryWrite` | already proposal-first | Generated memory writes land in ProposalStore and only write memory after accepted proposal application. | Link accepted memory writes to EvidenceStore evidence when HS evidence becomes canonical. |
| Vector memory maintenance | `openlife-core/src/vectors.rs::{run_tier_maintenance,archive_low_access_memories,restore_archived,set_importance}` | low-risk transient state | Changes retrieval tier/archive metadata, not LifeModel truth. | Keep automatic only for retrieval metadata; proposal-first if memory deletion/forgetting semantics affect accepted evidence. |
| Snapshots and restore | `openlife-core/src/versioning.rs::{snapshot,snapshot_for_patch}`; `src-tauri/src/commands/version.rs::{create_snapshot,restore_snapshot}` | read-only/materialized for snapshot creation; governed restore/import operation for restore | W93 requires explicit governed restore request, pre-change snapshot, typed materializer restriction, metadata-safe hashes/audit, and no raw LifeModel or snapshot YAML response. | Keep restore explicit and audited. |
| Data import/export | `src-tauri/src/commands/settings.rs::{export_all_data,import_all_data,apply_import_payload}` | governed restore/import operation for import | W93 requires explicit governed import request, target validation, pre-change snapshot, typed materializer restriction, metadata-safe counts/hashes/audit, and no raw imported payload response. Export remains read-only/materialized. | Keep import explicit and audited; do not treat imports as HS learning without re-materialization. |
| External writes and declarative tool stubs | `openlife-core/src/agent/action_executor/tool_executor.rs`; `openlife-core/src/agent/action_executor/execution_tools.rs`; `src-tauri/src/commands/proposal.rs::{ExternalWriteAction,ScheduledTask,DataExport}` | already proposal-first; disabled/declarative-only for propose-only tools | HS/tool policy creates ExternalWriteAction proposals instead of direct writes; calendar/email propose tools create declarative proposals; accepted proposals re-check safe paths and payload limits before side effects. | Keep direct external writes blocked until accepted; ensure future tool manifests cannot bypass proposal-first policy. |
| Compatibility YAML materializer | `openlife-core/src/life_model.rs::materialize_yaml_compatibility_view` | read-only/materialized | Produces a YAML string with source asset refs/digests and compact summaries; does not persist raw HS internals. | When stores become canonical, materializer should be the only YAML writer for HS-derived compatibility state. |

## New HS Write Paths From LMHS-1 Through LMHS-9

| HS path | Risk class | Current guard | Future action |
| --- | --- | --- | --- |
| `openlife-core/src/agent/evidence_store.rs::create_evidence` | low-risk transient state | New evidence starts as `candidate`; source refs store source ids/details and payload digests, not raw payloads; risk maps to privacy level. | Promotion from candidate evidence to durable active evidence should be governed by Evidence/Proposal rules. |
| `EvidenceStore::{weaken_evidence,archive_evidence,contradict_evidence,tombstone_evidence,link_proposal,link_agent_run,merge_run_metadata}` | low-risk transient state | Maintenance updates record status/links/metadata; reason text is digested for weaken/archive/contradiction/tombstone actions. | Impactful evidence weakening/deletion that changes user-facing behavior should become proposal-first unless strictly maintenance. |
| `openlife-core/src/proactive.rs::record_rejected_reminder_proposal` | low-risk transient state | Records only rejected `ProactiveAgent` reminder proposals; affected path is a proactive category; metadata stores proposal/run/action refs and digests; effect only weakens similar future reminder priority. | Keep the effect narrow; add user controls for deleting/forgetting learned negative evidence. |
| `openlife-core/src/agent/heuristic_store.rs::{create_heuristic,seed_mvp_heuristics,update_lifecycle,record_usage}` | low-risk transient state for seeded MVP assets and usage metadata; future accepted heuristics should be proposal-first | New heuristics start as `candidate`; active promotion requires accepted governance metadata or seeded built-in policy; archived/rejected heuristics cannot be promoted. | Non-built-in heuristic creation/promotion must come from accepted proposals with regression evidence. |
| `openlife-core/src/agent/policy_store.rs::mvp_builtin` | disabled/declarative-only | Built-in hard policies are in-memory declarations: sensitive topics LocalOnly and external writes proposal-first. No persisted policy mutation was added. | Persisted policy changes must be explicit high-risk proposals. |
| `openlife-core/src/agent/regression_suite.rs::mvp` | read-only/materialized | Deterministic checks read policy/heuristic stores and candidate drafts; no durable write path. | Store user-approved regression scenarios only after proposal/governor design exists. |
| `openlife-core/src/agent/hs_selector.rs` and runtime selection audit | read-only/materialized | Selects bounded runtime packets and metadata-safe audits; does not mutate stores. | Keep selectors read-only except for explicit usage telemetry with bounded metadata. |
| `openlife-core/src/agent/runtime.rs` HS prompt injection and `model_router.rs::route_with_hs_packet` | read-only/materialized | Adds selected guidance and enforces LocalOnly policy; no HS asset mutation. | Continue fail-closed privacy routing; never allow heuristics to relax hard policy. |
| `frontend/src/components/RunTracePanel.tsx` and proposal review trace fields | read-only/materialized | Displays selected collaboration rules, behavior checks, and concise evidence summaries; no write path. | Keep review surfaces metadata-safe and avoid raw HS dumps. |

## Conclusion

The MVP did not introduce a hidden high-risk direct-write path into identity,
values, mission, long-term goals, sensitive relationships, privacy boundaries,
or external side effects. Newly introduced HS writes are candidate evidence,
seeded built-in heuristics, metadata, or proposal creation paths, and runtime HS
selection/materialization remains bounded and read-only.

W97 completes the legacy direct-write convergence slice. Builder, Calibration,
and Feedback legacy direct-write override paths are retired/no-write. Manual
LifeModel editor save, Snapshot restore, and Data import are explicit governed
operations with validation, pre-change snapshots, typed materializer
restriction, and metadata-safe audit/count/hash results. Proposal application
has exact ProposalSource -> PatchSource mapping with no fallback blocker. State
and Daily Goal remain source-data compatibility materialization only, separate
from accepted durable LifeModel-HS truth. The final inventory reports
`overall_converged=true`, `all_direct_writes_converged=true`, and zero
high-risk legacy direct-write blockers.

## Convergence Backlog

1. Keep Review Center / accepted proposals as the normal durable LifeModel-HS
   mutation target.
2. Keep governed manual editor, restore, and import operations explicit,
   audited, snapshot-backed, and metadata-safe.
3. Split transient State/Daily Goal data into a future StateStore with
   TTL/source/confidence/privacy metadata; require proposals before durable
   preference or identity promotion.
4. Keep raw chat/memory/vector writes as local source data with retention,
   deletion, and forgetting controls; route generated durable memory claims
   through proposals/evidence.
