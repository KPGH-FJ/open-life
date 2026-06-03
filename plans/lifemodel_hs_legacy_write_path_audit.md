# LifeModel-HS Legacy Write Path Audit

Date: May 28, 2026

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

## Current Write Paths

| Area | Path / entry points | Risk class | Current guard | Future action |
| --- | --- | --- | --- | --- |
| LifeModel save primitive | `openlife-core/src/life_model.rs::LifeModelManager::save`; `src-tauri/src/lib.rs::persist_life_model` | legacy direct write requiring future convergence | Central save prepares model metadata and can create a daily snapshot; callers decide governance. | Keep as the compatibility materializer writer only; route durable HS and risky LifeModel mutations through Proposal/Governor acceptance. |
| Manual LifeModel editor | `src-tauri/src/commands/life_model.rs::save_life_model`; `frontend/src/pages/LifeModelEditor.tsx` | legacy direct write requiring future convergence | W80 metadata-safe explicit manual override audit records source, before/after hashes, rough changed sections, risk class, timestamp, command/function name, and manualOverride/proposalFirst/stillLegacyDirectWrite flags after successful save. It does not record raw LifeModel content and does not make the path proposal-first. | Convert to patch/proposal review or stronger manual override UX with confirmation and richer governance. |
| Review Center apply/edit | `src-tauri/src/commands/proposal.rs::accept_proposal_with_state`, `edit_proposal_with_state`, `apply_proposal_to_state`; `openlife-core/src/life_model/patch_store.rs` | already proposal-first | Safe Mode blocks apply/edit; proposal must be pending/postponed; payload validation runs before apply; LifeModel proposals create before/after snapshots and PatchStore records; MemoryWrite checks duplicate content; ExternalWriteAction re-validates safe paths, hash, UTF-8, and size. | Make this the convergence target for legacy LifeModel, memory, tool, and HS mutations; preserve source-specific patch source instead of the current broad BuilderReview patch source. |
| Low-risk batch proposal apply | `src-tauri/src/commands/proposal.rs::batch_accept_low_risk_proposals` | already proposal-first | Safe Mode guard; accepts only pending low-risk proposals. | Keep limited to low risk; do not extend to high-risk identity, values, mission, long-term goals, sensitive relationships, or privacy boundaries. |
| Proposal storage | `openlife-core/src/agent/proposal_store.rs::{create_proposal,update_proposal}` | already proposal-first | Proposals are persisted with type, source, risk, status, run id, and before/after payloads; status transitions are explicit through Review Center commands. | Continue linking generated HS proposals to run/evidence ids. |
| Builder normal flow | `src-tauri/src/commands/builder.rs::builder_create_proposals`; `frontend/src/pages/BuilderPage.test.tsx` | already proposal-first | Finished sessions with pending signals are sent to ProposalStore; frontend tests assert `builder_apply_signals` is not called in the normal review flow. | Keep this as the only product Builder write path. |
| Builder legacy direct flow | `src-tauri/src/commands/builder.rs::builder_apply_signals`; no-signal completion branch in `builder_step_with_state` | legacy direct write requiring future convergence | W81 guard present: command fails closed by default and requires explicit dev/migration override for the remaining legacy direct apply path; response omits raw model/run/audit payloads; no-signal completion performs session-only cleanup and does not write durable LifeModel truth. | Remove `builder_apply_signals` or convert the remaining override path fully to proposal-first before treating Builder as converged. |
| Builder session persistence | `openlife-core/src/builder/store.rs`; `src-tauri/src/commands/builder.rs::{builder_start,builder_step,builder_delete_session}` | low-risk transient state | Stores unfinished/review sessions separately from accepted LifeModel truth. | Keep as transient workflow state; delete or expire stale sessions. |
| Calibration proposal flow | `src-tauri/src/commands/calibration.rs::calibration_create_proposals`; `apply_calibration(mode = "proposal")` | already proposal-first | Change risk is assessed before proposal creation; identity values, long-term goals, and similar paths are high risk; proposals link to AgentRun. | Make this the product default and route all calibration changes through Review Center. |
| Calibration direct/evolution apply | `src-tauri/src/commands/calibration.rs::{run_micro_evolution,apply_calibration(mode = "direct")}`; `frontend/src/pages/CalibrationPage.tsx` currently calls direct mode | legacy direct write requiring future convergence | Direct apply creates snapshots and feedback analytics; `apply_calibration` is user-triggered, but it can persist high-risk dimensions if passed in. | Change UI default to proposal mode; gate direct mode as legacy/debug/migration only; remove automatic `result.applied` persistence from product evolution. |
| Feedback signals | `openlife-core/src/feedback.rs::{save_feedback,log_event,save_conversation_inference,fetch_evolution_signals}`; `src-tauri/src/lib.rs::capture_conversation_signals` | low-risk transient state | Append-only local feedback, analytics, and inference rows; they are signals, not accepted LifeModel truth. | Promote useful signals into EvidenceStore records and proposals; add retention/deletion policy per ADR 0013. |
| Feedback evolution direct writes | `src-tauri/src/commands/feedback.rs::{apply_feedback_evolution,generate_evolution_report}` | legacy direct write requiring future convergence | User/settings command path; central save creates metadata/snapshot where requested. | Convert suggested rules and feedback-driven model changes into proposals or HS evidence/heuristic candidates. |
| State history and current state | `openlife-core/src/memory.rs::record_state_entry`; `src-tauri/src/commands/state.rs::record_state` | low-risk transient state | User-initiated state samples append to `state_history`; current custom state dimension and `last_updated` are updated directly, not identity/values. | Move to StateStore with TTL, source, confidence, and privacy metadata; require proposals before promoting to durable identity/preference. |
| Daily goals and chat check-in | `src-tauri/src/commands/state.rs::{add_daily_goal,update_daily_goal,delete_daily_goal,toggle_daily_goal}`; `src-tauri/src/lib.rs::try_auto_checkin_daily_goals` call sites | low-risk transient state | Directly mutates daily goals/task completion only; chat auto-check-in is keyword-triggered and does not edit long-term goal definitions. | Keep as short-lived task state or migrate to StateStore; any promotion to long-term goals remains proposal-first. |
| Raw chat and memory records | `openlife-core/src/memory.rs::{save_message,save_memory_record}`; `src-tauri/src/lib.rs::persist_chat_message_if_needed`; `src-tauri/src/commands/memory.rs::index_memory_chunk` | low-risk transient state | Local raw/source records with privacy tags; user/manual indexing is explicit; raw memory is not accepted HS truth. | Preserve as raw life data with retention/deletion controls; generated durable memory claims should use MemoryWrite proposals or EvidenceStore. |
| Memory proposals | `openlife-core/src/agent/proposal_generators/chat.rs`; `openlife-core/src/agent/proposal_engine.rs::MemoryProposalGenerator`; `src-tauri/src/commands/proposal.rs::MemoryWrite` | already proposal-first | Generated memory writes land in ProposalStore and only write memory after accepted proposal application. | Link accepted memory writes to EvidenceStore evidence when HS evidence becomes canonical. |
| Vector memory maintenance | `openlife-core/src/vectors.rs::{run_tier_maintenance,archive_low_access_memories,restore_archived,set_importance}` | low-risk transient state | Changes retrieval tier/archive metadata, not LifeModel truth. | Keep automatic only for retrieval metadata; proposal-first if memory deletion/forgetting semantics affect accepted evidence. |
| Snapshots and restore | `openlife-core/src/versioning.rs::{snapshot,snapshot_for_patch}`; `src-tauri/src/commands/version.rs::{create_snapshot,restore_snapshot}` | read-only/materialized for snapshot creation; legacy direct write requiring future convergence for restore | Snapshot creation writes materialized YAML history; restore creates a pre-restore snapshot before replacing current YAML. | Keep snapshot writes as materialized/audit outputs; make restore an explicit rollback/governor operation with clear user confirmation. |
| Data import/export | `src-tauri/src/commands/settings.rs::{export_all_data,import_all_data,apply_import_payload}` | legacy direct write requiring future convergence | Import parses payload, captures prior model/messages/vectors, and attempts rollback on failure; settings UI disables import in Safe Mode. | Keep as explicit migration/restore path with stronger audit; do not treat imports as HS learning or accepted truth without re-materialization. |
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

Legacy direct-write paths still exist and are not converged: manual LifeModel
editor save, Builder legacy apply/no-signal completion, Calibration direct
apply/evolution, feedback evolution, restore/import, and several low-risk
state/memory compatibility writes. These are known convergence items, not HS
MVP additions. As of W79, they are represented by a machine-readable inventory
guard; as of W80, manual LifeModel editor save also has a metadata-safe manual
override audit guard; as of W81, Builder legacy direct apply defaults fail
closed and no-signal completion is no-write/session-only. These guards are not
convergence completion.

## Convergence Backlog

1. Make Review Center application the only normal durable LifeModel write path.
2. Remove or fully proposal-first-convert `builder_apply_signals`; keep Builder
   no-signal completion session-only with no durable LifeModel write.
3. Change Calibration UI/defaults to proposal mode and gate direct calibration
   and micro-evolution persistence as legacy/debug/migration only.
4. Convert feedback evolution and `evolution_rules` updates into EvidenceStore
   records plus heuristic or LifeModel proposals.
5. Split transient state into StateStore with TTL/source/confidence/privacy
   metadata; require proposals before durable preference or identity promotion.
6. Keep raw chat/memory/vector writes as local source data with retention,
   deletion, and forgetting controls; route generated durable memory claims
   through proposals/evidence.
7. Restrict `LifeModelManager::save` to compatibility materialization,
   migration, explicit manual override, and accepted proposal application.
8. Add source-specific PatchStore mapping for Builder, Calibration, Feedback,
   Chat, Manual, and HS proposal sources.
