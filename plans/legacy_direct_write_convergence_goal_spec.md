# Legacy Direct-Write Convergence Goal Spec

> Last updated: 2026-06-04
> Status: Completed CLI Goal-mode implementation spec / audit trail for W90-W97

This document is the completed CLI Goal-mode spec and audit trail for the
Legacy Direct-Write Convergence block.

The pre-W90 baseline below describes the historical state at Goal start. Do not
treat it as the current repository state; current status is governed by
`AGENTS.md`, `plans/README.md`, and
`plans/lifemodel_governed_runtime_progress.md`.

## 1. Goal-Start Baseline

The goal-start authoritative baseline was **W89 Proposal Application
Source-Specific Patch Audit / Readiness complete**.

The implementation Agent was required to read these files before editing code:

1. `AGENTS.md`
2. `plans/README.md`
3. `plans/lifemodel_governed_runtime_progress.md`
4. `plans/lifemodel_hs_legacy_write_path_audit.md`
5. `plans/adr/0013-lifemodel-hs-source-of-truth-governance.md`

Goal-start completed preparation:

- W79 created the machine-readable legacy direct-write inventory guard.
- W80 added metadata-safe manual LifeModel editor override audit.
- W81 gated Builder legacy direct apply and made no-signal completion no-write.
- W82 gated Calibration direct apply and micro-evolution legacy persistence.
- W83 gated Feedback evolution direct apply and made evolution report read-only.
- W84 gated Snapshot restore and Data import legacy direct write paths.
- W85 proved State / Daily Goal writes are transient source data, not accepted
  durable LifeModel-HS truth.
- W86 created the LifeModel compatibility materializer caller matrix.
- W87 restricted `persist_life_model` callers with typed source-data context.
- W88 mapped accepted LifeModel proposal `PatchSource` by `ProposalSource`.
- W89 audited that mapping and proved apply-path resolver usage.

Goal-start blockers resolved by W90-W97:

- Builder legacy direct apply override still exists.
- Calibration direct/evolution override capability still exists.
- Feedback evolution direct apply override capability still exists.
- Snapshot restore and Data import remain gated legacy direct writes.
- Manual LifeModel editor direct save remains a high-risk manual direct write.
- Proposal `PatchSource` fallback policy remains unresolved.
- State / Daily Goal source-data remains coupled to the LifeModel compatibility
  materialized view.
- `all_direct_writes_converged=false`.
- `proposal_first_convergence_complete=false`.

## 2. Goal Objective

Complete Legacy Direct-Write Convergence from W90 through W97.

The final state must satisfy all of the following:

- No high-risk hidden or legacy direct-write capability remains for durable
  LifeModel-HS truth.
- Normal durable LifeModel-HS mutation is Proposal/Governor accepted before
  apply.
- Manual or restore/import exceptions are explicit governed operations, with
  metadata-safe audit, pre-change snapshot where applicable, and no raw payload
  leakage in reports/debug/audit output.
- State / Daily Goal remains source data or is split into a source-data store;
  it is not silently promoted into durable identity, values, preferences,
  relationships, health, finance, or long-term goals.
- Accepted proposal application uses source-specific `PatchSource` without
  misleading fallback labels.
- `legacy_write_convergence` reports:
  - `all_direct_writes_converged=true`
  - `proposal_first_convergence_complete=true`
  - no high-risk blocker entries
  - no unsupported or unclassified production materializer caller
- Default Chat remains `legacy_stream`.
- Ordinary `send_message` and `start_stream_message` do not call W79-W97
  helper, gate, report, readiness, override, or convergence APIs.
- `AGENTS.md`, `plans/README.md`,
  `plans/lifemodel_governed_runtime_progress.md`, and
  `plans/lifemodel_hs_legacy_write_path_audit.md` are synchronized to the final
  W97 state.

## 3. Non-Negotiable Invariants

Do not change these invariants:

- Do not migrate default Chat.
- Do not replace ordinary `send_message` or `start_stream_message`.
- Do not route ordinary Chat through maturation, legacy convergence, proposal
  readiness, default adapter proof, or runtime migration surfaces.
- Do not run model/runtime/tool execution just to converge legacy write paths.
- Do not write raw LifeModel content, raw memory, raw vector payload, raw
  imported data, raw prompt, raw assistant output, raw tool payload, or raw
  proposal patch value into metadata reports, debug dumps, or audit summaries.
- Do not mark a path converged because it has a fail-closed guard. Convergence
  requires actual retirement, proposal-first conversion, or a governed operation
  whose authority and audit are explicitly represented.
- Do not mark `proposal_first_convergence_complete=true` while any accepted
  LifeModel proposal source still has an unconfirmed or misleading fallback
  `PatchSource` policy.
- Do not hide remaining risk by removing inventory entries. Retired paths should
  be represented as retired/converged where useful, not silently erased.
- Do not commit or push unless the user explicitly asks after review.

## 4. Acceptable Final Write Categories

At W97, every write touching LifeModel-HS adjacent state must fit one of these
categories:

| Category | Allowed | Requirements |
| --- | --- | --- |
| Accepted proposal apply | Yes | Pending/postponed Proposal accepted or edited by user; payload validated; snapshot/audit present; `PatchSource` source-specific |
| Governed manual override | Yes, only if explicit | User intent explicit; high-risk warning or typed override; pre-change snapshot; metadata-safe audit; not represented as automated learning |
| Governed restore/import | Yes, only if explicit | Restore/import purpose explicit; pre-change snapshot or migration audit; metadata-safe counts/hashes/ids; not treated as HS learning |
| Source-data append/update | Yes | Stored as source data with source/confidence/time/privacy metadata; no silent durable truth promotion |
| Compatibility materialized view | Yes, only as compatibility | Must be classified as materialized/source-data view, not durable accepted HS truth |
| Runtime/model/tool generated claim | No direct apply | Must become Evidence/Proposal first |
| Legacy dev/migration override direct write | No | Must be retired, converted, or reclassified as governed operation with explicit authority |

## 5. Implementation Strategy

The Agent should complete the whole block in one Goal run, but implement in this
exact internal order:

1. W90 Builder legacy direct apply retirement
2. W91 Calibration direct/evolution retirement
3. W92 Feedback evolution direct apply retirement
4. W93 Snapshot restore / Data import governed operation conversion
5. W94 Manual LifeModel editor convergence
6. W95 Proposal `PatchSource` fallback policy closure
7. W96 State / Daily Goal source-data split or convergence classification
8. W97 Final inventory, authority sync, and convergence verification

Run targeted tests after each major code area when practical. Run the full
verification matrix only at the end.

If the Agent discovers that a direct path cannot be removed without breaking a
public command surface, keep the command surface compatible but remove the
direct write capability behind it. A command may become a deprecated
fail-closed compatibility surface or may create proposals instead. It must not
retain an override that can directly mutate durable LifeModel-HS truth.

## 6. W90 Spec: Builder Legacy Direct Apply Retirement

### Scope

Primary files:

- `src-tauri/src/commands/builder.rs`
- `src-tauri/src/legacy_write_convergence.rs`
- `src-tauri/src/legacy_write_convergence_tests.rs`
- `frontend/src/pages/BuilderPage.test.tsx` if frontend expectations need sync
- Authority docs listed in Section 2

### Required Behavior

- Normal Builder write flow remains `builder_create_proposals`.
- `builder_apply_signals` must no longer be able to directly write durable
  LifeModel truth through a dev/migration override.
- The W81 no-signal completion branch must remain session-only and must not
  persist LifeModel truth.
- If `builder_apply_signals` command remains registered, it must either:
  - return a clear deprecated/fail-closed response with no durable write, or
  - convert its decisions into ProposalStore entries without applying them.
- No raw Builder model, run, feedback audit, prompt, or LifeModel payload may
  appear in output/debug/audit.

### Required Tests

- Default `builder_apply_signals` cannot write durable LifeModel truth.
- Any previous dev/migration override path cannot write durable LifeModel truth.
- Normal Builder proposal path still creates proposals.
- No-signal completion remains session-only/no durable write.
- W79 inventory no longer marks Builder as high-risk legacy direct write.
- Default Chat entrypoints do not call Builder convergence helpers.

### W90 Done Criteria

- Builder blocker is retired or proposal-first converted.
- Other blockers remain accurately represented.
- Do not set global convergence true yet.

## 7. W91 Spec: Calibration Direct/Evolution Retirement

### Scope

Primary files:

- `src-tauri/src/commands/calibration.rs`
- `src-tauri/src/legacy_write_convergence.rs`
- `src-tauri/src/legacy_write_convergence_tests.rs`
- `frontend/src/pages/CalibrationPage.tsx` and related tests if required
- `frontend/src/pages/DashboardPage.tsx` and related tests if required

### Required Behavior

- `calibration_create_proposals` and proposal mode remain the normal write path.
- `apply_calibration(mode="direct")` must no longer directly persist durable
  LifeModel-HS truth.
- `run_micro_evolution` must no longer directly persist durable LifeModel-HS
  truth.
- If direct/evolution commands remain for compatibility, they must fail closed
  or create proposals/evidence only.
- No raw LifeModel, calibration change reason, or evolution payload may appear
  in metadata output.

### Required Tests

- Direct calibration mode cannot write durable LifeModel truth, including with
  old dev/migration override style inputs.
- Micro-evolution cannot write durable LifeModel truth.
- Proposal mode remains functional.
- Inventory marks Calibration direct/evolution as converged or retired, not as
  high-risk.
- Default Chat entrypoints remain isolated.

### W91 Done Criteria

- Calibration direct/evolution blocker removed or proposal-first converted.
- Builder remains converged from W90.
- Remaining blockers still explicit.

## 8. W92 Spec: Feedback Evolution Direct Apply Retirement

### Scope

Primary files:

- `src-tauri/src/commands/feedback.rs`
- `openlife-core/src/feedback.rs`
- `src-tauri/src/legacy_write_convergence.rs`
- `src-tauri/src/legacy_write_convergence_tests.rs`
- Settings UI copy/tests if required

### Required Behavior

- `generate_evolution_report` remains read-only unless it creates only
  reviewable Evidence/Proposal records.
- `apply_feedback_evolution` must no longer directly write LifeModel or
  `evolution_rules` truth.
- Future feedback-driven changes must become evidence/proposals/candidates.
- If the public command remains, it must fail closed or return metadata-safe
  proposal/evidence creation results only.

### Required Tests

- Feedback evolution direct apply cannot write durable LifeModel truth.
- Feedback evolution direct apply cannot write active evolution rules.
- Read-only report remains read-only.
- Inventory marks Feedback evolution blocker converged or retired.
- Metadata output remains raw-payload-free.

### W92 Done Criteria

- Feedback evolution direct apply blocker removed or evidence/proposal-first
  converted.
- Builder and Calibration remain converged.

## 9. W93 Spec: Snapshot Restore / Data Import Governed Conversion

### Scope

Primary files:

- `src-tauri/src/commands/version.rs`
- `src-tauri/src/commands/settings.rs`
- `openlife-core/src/versioning.rs`
- `src-tauri/src/legacy_write_convergence.rs`
- `src-tauri/src/legacy_write_convergence_tests.rs`

### Required Behavior

Snapshot restore and data import are not ordinary LifeModel learning. They may
remain write-capable only as explicit governed operations.

Required governed restore/import properties:

- Explicit user/request purpose.
- Pre-change snapshot where the operation can mutate current LifeModel, memory,
  vector, or settings state.
- Metadata-safe audit containing ids, counts, hashes, status, source kind, and
  operation purpose only.
- No raw restored LifeModel, snapshot YAML, raw imported LifeModel, raw messages,
  raw vectors, raw memory, or raw payload in responses/debug/audit.
- Not represented as runtime authority, migration permission, or HS learning.
- Fail closed on missing purpose, missing pre-change snapshot where required,
  invalid payload, unsafe payload, or unsupported import target.

The previous dev/migration/manual restore override concept must be retired or
renamed/reclassified into an explicit governed restore/import request type.

### Required Tests

- Restore without governed restore request fails closed.
- Restore with valid governed request succeeds with pre-change snapshot and
  metadata-safe audit.
- Import without governed import request fails closed.
- Import with valid governed request returns only counts/status/hash metadata.
- Invalid/unsafe import payload fails closed.
- Inventory no longer classifies restore/import as high-risk legacy direct-write
  blockers, while still classifying them as governed high-risk operations.

### W93 Done Criteria

- Restore/import are governed operations or retired.
- They are not marked proposal-first HS learning.
- They are not hidden direct writes.

## 10. W94 Spec: Manual LifeModel Editor Convergence

### Scope

Primary files:

- `src-tauri/src/commands/life_model.rs`
- `frontend/src/pages/LifeModelEditor.tsx`
- `frontend/src/tauri.ts`
- `src-tauri/src/legacy_write_convergence.rs`
- `src-tauri/src/legacy_write_convergence_tests.rs`

### Required Behavior

The manual editor cannot remain a silent high-risk direct write.

Preferred path:

- Convert normal manual editor save into a LifeModel patch/proposal review flow.
- The editor submits a proposal or patch draft.
- Actual durable apply occurs only through Review Center accept/edit.

Acceptable fallback if the current UX cannot be fully converted in one run:

- Keep a deliberately named explicit manual override command/path.
- Require explicit user intent and high-risk manual override metadata.
- Create pre-change snapshot.
- Record metadata-safe audit.
- Ensure the inventory classifies this as governed manual override, not legacy
  direct write.

In both cases:

- No raw LifeModel content in audit/debug/report.
- No runtime/model/tool authority.
- No default Chat impact.

### Required Tests

- Normal manual editor save path no longer silently writes durable LifeModel
  truth.
- Proposal path or governed manual override path works as specified.
- Manual override audit remains metadata-safe.
- Inventory no longer marks manual editor as legacy direct-write blocker.

### W94 Done Criteria

- Manual editor is proposal-first or governed manual override.
- It is not hidden legacy direct write.

## 11. W95 Spec: Proposal PatchSource Fallback Policy Closure

### Scope

Primary files:

- `openlife-core/src/life_model/patch.rs`
- `openlife-core/src/life_model/patch_store.rs`
- `src-tauri/src/commands/proposal.rs`
- `src-tauri/src/legacy_write_convergence.rs`
- `src-tauri/src/legacy_write_convergence_tests.rs`

### Required Behavior

The W89 fallback blocker must be closed.

Preferred path:

- Add dedicated `PatchSource` variants for proposal sources that currently
  fallback to `Manual`, including:
  - `ChatConversation`
  - `ProactiveAgent`
  - `SkillRuntime`
  - `Plugin`
  - `MemoryGovernance`
- Update display/from-string/persistence mapping.
- Update W88/W89 mapping/readiness logic.
- Ensure no source except `ProposalSource::Manual` is mislabeled as Manual.
- Ensure `BuilderReview` is used only for Builder-origin proposals.

Acceptable alternative:

- Formally define an accepted Manual fallback policy with typed metadata that
  preserves origin separately and cannot be misread as a human manual edit.
- This alternative must be explicit in code and tests, not only docs.

### Required Tests

- Every `ProposalSource` maps to a non-misleading `PatchSource` or a formally
  accepted typed fallback.
- W89 readiness can pass without fallback blockers.
- `proposal_first_convergence_complete=true` only after all fallback blockers
  are resolved.
- PatchStore parsing remains backward-compatible for existing stored strings.

### W95 Done Criteria

- Proposal apply source semantics are complete.
- No misleading BuilderReview or Manual fallback remains.

## 12. W96 Spec: State / Daily Goal Source-Data Split

### Scope

Primary files:

- `src-tauri/src/commands/state.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/src/legacy_write_convergence.rs`
- `src-tauri/src/legacy_write_convergence_tests.rs`
- `openlife-core/src/memory.rs`
- `openlife-core/src/life_model.rs` if compatibility materialization changes

### Required Behavior

State and Daily Goal data must be represented as source data, not silently
accepted durable LifeModel-HS truth.

Preferred path:

- Store state/daily-goal updates in source-data structures with source,
  confidence, timestamp, retention/TTL where applicable, and privacy metadata.
- Keep LifeModel compatibility materialization read-only or clearly marked as a
  compatibility view.
- Do not promote state samples, task completions, or chat auto-check-in results
  into durable identity/preferences/goals without proposal.

Acceptable bounded path:

- If a full StateStore split is too large, retain compatibility materialization
  but add stronger typed classification and tests proving:
  - source-data write only
  - no accepted durable HS truth write
  - no active LifeModel patch
  - promotion requires proposal
  - inventory treats it as converged low-risk source-data compatibility, not a
    high-risk blocker

### Required Tests

- State updates do not create durable accepted HS truth.
- Daily goal updates and chat auto-check-in do not modify long-term goals or
  accepted LifeModel-HS truth.
- Any promotion path requires Proposal/Governor acceptance.
- Inventory marks State / Daily Goal as converged source-data boundary.

### W96 Done Criteria

- State / Daily Goal no longer prevents final direct-write convergence.
- Any remaining compatibility materialization is explicit and tested.

## 13. W97 Spec: Final Convergence Inventory And Authority Sync

### Scope

Primary files:

- `src-tauri/src/legacy_write_convergence.rs`
- `src-tauri/src/legacy_write_convergence_tests.rs`
- `src-tauri/src/lib.rs` default Chat isolation tests
- `AGENTS.md`
- `plans/README.md`
- `plans/lifemodel_governed_runtime_progress.md`
- `plans/lifemodel_hs_legacy_write_path_audit.md`

### Required Behavior

- Final inventory reports no high-risk legacy direct-write blockers.
- `all_direct_writes_converged=true`.
- `proposal_first_convergence_complete=true`.
- Remaining write-capable paths are classified as:
  - accepted proposal apply
  - governed manual override
  - governed restore/import operation
  - low-risk source data
  - materialized compatibility/audit/snapshot output
- W79-W97 helpers remain outside ordinary default Chat.
- Docs update from W89 current status to W97 final convergence complete.
- Docs must not claim default Chat migration, runtime authority, model/tool
  execution, or automatic HS learning.

### Required Tests

- Legacy convergence inventory final report passes.
- No high-risk blocker entries remain.
- No unsupported or unclassified production materializer caller remains.
- Default Chat ordinary entrypoint isolation passes.
- Metadata reports remain raw-payload-free.

### W97 Done Criteria

- The big Legacy Direct-Write Convergence block is complete.
- The next development block can start from a governed LifeModel-HS write
  surface rather than legacy direct write cleanup.

## 14. Final Verification Matrix

Run all applicable targeted tests, then full CI.

Minimum required commands:

```bash
cargo test -p openlife-tauri builder -- --nocapture
cargo test -p openlife-tauri calibration -- --nocapture
cargo test -p openlife-tauri feedback -- --nocapture
cargo test -p openlife-tauri version -- --nocapture
cargo test -p openlife-tauri settings -- --nocapture
cargo test -p openlife-tauri proposal -- --nocapture
cargo test -p openlife-tauri legacy_write_convergence -- --nocapture
cargo test -p openlife-tauri default_chat_entrypoints_do_not_call_w19_w60_command_surfaces -- --nocapture
git diff --check
make ci
```

Required search checks:

```bash
rg -n "with_state_for_dev_migration|direct_apply_after_gate|LegacyDirectApplyOverride|manual_restore_override|dev_migration_override" src-tauri/src openlife-core/src
rg -n "send_message|start_stream_message" src-tauri/src/lib.rs
rg -n "all_direct_writes_converged|proposal_first_convergence_complete|high-risk|blocker" src-tauri/src/legacy_write_convergence.rs src-tauri/src/legacy_write_convergence_tests.rs plans AGENTS.md
```

The first `rg` may still find historical test names or retired compatibility
text, but final code must not retain an executable legacy direct-write override
that can mutate durable LifeModel-HS truth.

## 15. CLI Goal Prompt

Paste this prompt into Codex CLI from the repository root:

```text
PLEASE IMPLEMENT THIS GOAL:

Read and follow `plans/legacy_direct_write_convergence_goal_spec.md`.

You are completing the full Legacy Direct-Write Convergence block from W90
through W97 in one sustained Goal-mode run.

Current baseline:
- W89 Proposal Application Source-Specific Patch Audit / Readiness is complete.
- The repository should be clean before you start.
- Default Chat must remain `legacy_stream`.
- Ordinary `send_message` and `start_stream_message` must not call W79-W97
  helper/gate/report/readiness/override/convergence APIs.

Goal:
- Retire or governance-convert remaining legacy direct-write capability across
  Builder, Calibration, Feedback evolution, Snapshot restore/Data import,
  Manual LifeModel editor, Proposal PatchSource fallback policy, and
  State/Daily Goal source-data coupling.
- Finish with no high-risk legacy direct-write blockers.
- Finish with `all_direct_writes_converged=true`.
- Finish with `proposal_first_convergence_complete=true`.
- Synchronize `AGENTS.md`, `plans/README.md`,
  `plans/lifemodel_governed_runtime_progress.md`, and
  `plans/lifemodel_hs_legacy_write_path_audit.md` to W97.

Implementation order:
1. W90 Builder legacy direct apply retirement.
2. W91 Calibration direct/evolution retirement.
3. W92 Feedback evolution direct apply retirement.
4. W93 Snapshot restore / Data import governed operation conversion.
5. W94 Manual LifeModel editor convergence.
6. W95 Proposal PatchSource fallback policy closure.
7. W96 State / Daily Goal source-data split or convergence classification.
8. W97 final convergence inventory and authority sync.

Hard constraints:
- Do not migrate default Chat.
- Do not run runtime/model/tool execution for this block.
- Do not write raw LifeModel, memory, vector, prompt, assistant output, tool
  payload, import payload, or proposal patch values into metadata reports,
  debug output, or audit summaries.
- Do not mark a path converged merely because it is fail-closed.
- Do not hide remaining blockers by deleting inventory entries without a code
  change that retires or governance-converts the path.
- Do not commit or push. Stop after implementation and verification with a
  summary.

Required final verification:
- cargo test -p openlife-tauri builder -- --nocapture
- cargo test -p openlife-tauri calibration -- --nocapture
- cargo test -p openlife-tauri feedback -- --nocapture
- cargo test -p openlife-tauri version -- --nocapture
- cargo test -p openlife-tauri settings -- --nocapture
- cargo test -p openlife-tauri proposal -- --nocapture
- cargo test -p openlife-tauri legacy_write_convergence -- --nocapture
- cargo test -p openlife-tauri default_chat_entrypoints_do_not_call_w19_w60_command_surfaces -- --nocapture
- git diff --check
- make ci

Final output must include:
- W90-W97 completion summary.
- Exact final convergence state.
- Remaining risks, if any.
- Test results.
- Files changed.
- Confirmation that no commit or push was performed.
```

## 16. Reviewer Acceptance Checklist

Use this checklist before accepting the Agent output:

- Builder legacy direct apply cannot write durable LifeModel truth.
- Calibration direct/evolution cannot write durable LifeModel truth.
- Feedback evolution cannot write durable LifeModel or active rule truth.
- Restore/import are governed operations or retired, not legacy hidden writes.
- Manual editor is proposal-first or governed manual override, not silent direct
  save.
- Proposal `PatchSource` mapping has no misleading fallback blocker.
- State / Daily Goal is source data or explicitly compatible materialized view,
  not accepted durable HS truth.
- `all_direct_writes_converged=true`.
- `proposal_first_convergence_complete=true`.
- No ordinary default Chat route change.
- No raw payload leakage in reports/debug/audit.
- Docs are synchronized to W97.
- `make ci` passes.
