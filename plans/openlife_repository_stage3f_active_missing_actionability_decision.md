# OpenLife Repository Stage3F Active Missing Actionability Decision

> Date: 2026-07-07
> Status: documentation classification record only
> Authority: subordinate to `plans/README.md`,
> `plans/openlife_single_system_deletion_manifest.md`, and
> `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md`.

Stage3F classifies the current `active_doc_missing_records=143` records from
`plans/openlife_repository_document_link_baseline.json`. It does not reuse the
older Stage3D count of 171.

Stage3F is not closure. It does not authorize ADR changes, active authority
promotion, runtime/source edits, future namespace creation, or any product
behavior change.

## Files Touched

- `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md`
- `plans/openlife_repository_document_link_baseline.json`
- `plans/openlife_repository_document_inventory.json`
- `plans/openlife_repository_stage3f_active_missing_actionability_decision.md`

Files and paths intentionally not touched:

- Rust/Tauri/React/frontend source
- `AGENTS.md`
- `README.md`
- `plans/README.md`
- `docs/decisions/*`
- `plans/adr/*`
- `docs/product/`
- `plans/archive/`

## Classification Rules

Classification precedence:

1. ADR blocker records remain blocked when the target is the ADR index, a
   duplicate ADR 0013 decision target under `docs/decisions/`, or a shorthand
   ADR 0013 path that does not name the existing canonical file.
2. Future namespace records remain blocked when the target is under
   `docs/product`, `plans/archive`, `plans/active`, or local/private/draft
   namespaces.
3. Expected-absent records are targets listed as `done` deletion,
   test-archive, or product-valid-rename objects in
   `plans/openlife_single_system_deletion_manifest.md`.
4. Remaining records are actionable repair candidates for a future authorized
   documentation slice; that means retarget, reword, source-map, or checker
   refinement, not creating the missing target by default.

## Stage3F Counts

| Category | Records | Stage3F decision |
| --- | ---: | --- |
| `active_actionable_repair_records` | 51 | Future authorized doc slice must retarget, reword, source-map, or refine checker handling. |
| `active_expected_absent_records` | 37 | Preserve absence as Phase7 deletion evidence; do not restore files. |
| `active_future_blocked_records` | 39 | Keep future namespaces blocked; do not create empty directories or placeholders. |
| `active_adr_blocked_records` | 16 | Keep ADR consolidation blocked in Stage3F; do not create the ADR index or move ADR 0013. |
| **Total classified active records** | **143** | Matches current Stage3E-after baseline. |

## Expected-Absent Decision

The old objects recorded in
`plans/openlife_single_system_deletion_manifest.md` as deleted, test-only
archive, or product-valid rename targets are expected to be absent. Stage3F
therefore records them as `active_expected_absent_records`.

This includes missing path mentions from the deletion manifest itself and other
active/preparation records that still name the same deleted targets. These are
not instructions to recreate:

- old Main Chat stage/beta/productization/maturity/eval modules;
- old migration/cutover command modules;
- old `multi_strategy_runtime`, `runtime_migration_gate`, and `react_beta`
  module files;
- old legacy-write convergence shell path;
- old Settings preview and frontend test source paths that have been deleted or
  moved to test/archive/dev-only surfaces.

## ADR Blocker Retained

Stage3F keeps the ADR blocker unchanged:

- The ADR index must not be created in Stage3F.
- ADR 0013 remains at
  `plans/adr/0013-lifemodel-hs-source-of-truth-governance.md`.
- A duplicate ADR 0013 decision file under `docs/decisions/` must not be
  created as part of Stage3F.
- No ADR file is moved.

## Future Namespace Blocker Retained

Stage3F keeps future namespace creation blocked:

- no `docs/product/`;
- no `plans/archive/`;
- no `plans/active/`;
- no `docs/private/` or `docs/local/`;
- no `plans/private/`, `plans/local/`, or `plans/drafts/`.

The Stage3F classification may name these paths as blocked namespaces, but it
does not create empty directories or placeholder documents.

## Validation

The expected `main_chat_runtime_module` result remains the inherited
two-failure blocker, not a passing Stage3F signal.

| Command | Result |
| --- | --- |
| `git diff --check` | Passed. |
| `cargo fmt --check` | Passed. |
| `python3 -m json.tool plans/openlife_repository_document_link_baseline.json >/tmp/openlife_repository_document_link_baseline_stage3f_pretty.json` | Passed. |
| `python3 -m json.tool plans/openlife_repository_document_inventory.json >/tmp/openlife_repository_document_inventory_stage3f_pretty.json` | Passed. |
| Future directory absence checks | Passed for `docs/product`, `plans/archive`, `plans/active`, the ADR index target, and local/private/draft namespace paths. |
| Stage3F actionability count check | Passed with `143 = 51 + 37 + 39 + 16`. |
| `cargo test -p openlife-tauri single_system -- --nocapture` | Passed, 17 tests. |
| `cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture` | Failed as inherited blocker: 24 passed, 2 failed. Failures remain `main_chat_final_gate_aggregation_is_not_hidden_in_test_module` and `main_chat_live_provider_completed_report_builder_is_not_hidden_in_test_module`. |

## Stage3G First Actionable Repair Record

Date: 2026-07-07

Stage3G executes the first bounded repair pass for the Stage3F
`active_actionable_repair_records=51` set only. It does not attempt active
missing cleanup, ADR consolidation, future namespace creation, expected-absent
resolution, runtime/source repair, or authority promotion.

Stage3G repaired deterministic records in allowed files by:

- rewriting old source paths as historical/deleted/source-map residue instead
  of current file targets;
- replacing wrong root package-target wording with existing package evidence
  such as `frontend/package.json`;
- keeping Phase7 deleted/source-map semantics explicit without creating or
  restoring missing files.

Files touched by Stage3G:

- `CONTRIBUTING.md`
- `plans/openlife_repository_active_claim_audit.md`
- `plans/openlife_repository_stage2a_scope_decision.md`
- `plans/openlife_repository_stage2c_phase_c_readiness_decision.md`
- `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md`
- `plans/openlife_single_system_development_preparation.md`
- `plans/openlife_repository_stage3f_active_missing_actionability_decision.md`
- `plans/openlife_repository_document_link_baseline.json`
- `plans/openlife_repository_document_inventory.json`

Stage3G regenerated JSON summary:

| Category | Before Stage3G | After Stage3G | Decision |
| --- | ---: | ---: | --- |
| `active_doc_missing_records` | 143 | 96 | Declined; not a cleanup-to-zero pass. |
| `active_actionable_repair_records` | 51 | 4 | Remaining records are from forbidden files only. |
| `active_expected_absent_records` | 37 | 37 | Preserved as Phase7 deletion evidence. |
| `active_future_blocked_records` | 39 | 39 | Preserved; no future namespaces created. |
| `active_adr_blocked_records` | 16 | 16 | Preserved; no ADR files moved or created. |

Remaining Stage3F actionable records skipped by Stage3G:

| Source | Records | Skip reason |
| --- | ---: | --- |
| `AGENTS.md` | 2 | Forbidden by Stage3G scope; root AI authority file was not edited. |
| `docs/decisions/0002-proposal-unified.md` | 1 | Forbidden by Stage3G scope; `docs/decisions/*` was not edited. |
| `docs/decisions/0003-agent-run-tracking.md` | 1 | Forbidden by Stage3G scope; `docs/decisions/*` was not edited. |

Stage3G validation results:

| Command | Result |
| --- | --- |
| `git diff --check` | Passed. |
| `cargo fmt --check` | Passed. |
| `python3 -m json.tool plans/openlife_repository_document_link_baseline.json >/tmp/openlife_repository_document_link_baseline_stage3g_pretty.json` | Passed. |
| `python3 -m json.tool plans/openlife_repository_document_inventory.json >/tmp/openlife_repository_document_inventory_stage3g_pretty.json` | Passed. |
| `cargo test -p openlife-tauri single_system -- --nocapture` | Passed, 17 tests. |
| `cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture` | Failed as inherited blocker: 24 passed, 2 failed. Failures remain `main_chat_final_gate_aggregation_is_not_hidden_in_test_module` and `main_chat_live_provider_completed_report_builder_is_not_hidden_in_test_module`. |

## Stage3H Residual Forbidden-File Actionable Repair Record

Date: 2026-07-07

Stage3H clears only the four Stage3G residual
`active_actionable_repair_records`. It does not attempt general active missing
cleanup, ADR consolidation, authority promotion, future namespace creation,
runtime/source cleanup, Main Chat readiness closure, or live-provider completion.

Stage3H repaired the residual records by:

- rewording the root current-authority source-map caveat so the deleted
  final-acceptance test owner is not presented as a current file target;
- rewording the historical preview-audit utility residue so the current
  source-map remains `src-tauri/src/commands/agent_runtime/`;
- retargeting ADR 0002's related frontend surface to
  `frontend/src/pages/ChatPage.tsx`;
- retargeting ADR 0003's related trace surface to
  `frontend/src/components/RunTracePanel.tsx`.

Files touched by Stage3H:

- `AGENTS.md`
- `docs/decisions/0002-proposal-unified.md`
- `docs/decisions/0003-agent-run-tracking.md`
- `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md`
- `plans/openlife_repository_stage3f_active_missing_actionability_decision.md`
- `plans/openlife_repository_document_link_baseline.json`
- `plans/openlife_repository_document_inventory.json`

Stage3H regenerated JSON summary:

| Category | Before Stage3H | After Stage3H | Decision |
| --- | ---: | ---: | --- |
| `active_doc_missing_records` | 96 | 92 | Only four residual actionable records were cleared. |
| `active_actionable_repair_records` | 4 | 0 | Residual forbidden-file actionable references were reworded or retargeted. |
| `active_expected_absent_records` | 37 | 37 | Preserved as Phase7 deletion evidence. |
| `active_future_blocked_records` | 39 | 39 | Preserved; no future namespaces created. |
| `active_adr_blocked_records` | 16 | 16 | Preserved; no ADR files moved or created. |

Stage3H explicit non-actions:

- no Rust/Tauri/React/frontend source code was changed;
- no future namespace was created;
- no ADR consolidation was performed;
- no ADR index file was created;
- no ADR status was changed;
- no authority promotion was performed;
- no Main Chat complete or live-provider complete claim was made.

Stage3H validation results:

| Command | Result |
| --- | --- |
| `git diff --check` | Passed. |
| `cargo fmt --check` | Passed. |
| `python3 -m json.tool plans/openlife_repository_document_link_baseline.json >/tmp/openlife_repository_document_link_baseline_stage3h_verify.json` | Passed. |
| `python3 -m json.tool plans/openlife_repository_document_inventory.json >/tmp/openlife_repository_document_inventory_stage3h_verify.json` | Passed. |
| `cargo test -p openlife-tauri single_system -- --nocapture` | Passed, 17 tests. |
| `cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture` | Failed as inherited blocker: 24 passed, 2 failed. Failures remain `main_chat_final_gate_aggregation_is_not_hidden_in_test_module` and `main_chat_live_provider_completed_report_builder_is_not_hidden_in_test_module`. |

## Stage4A ADR No-Move Index Consolidation Record

Date: 2026-07-07

Stage4A resolves the Stage3H residual `active_adr_blocked_records=16` set by
creating the ADR index and choosing a no-move canonical pointer for ADR 0013.
It does not move ADR 0013 and does not create a duplicate ADR 0013 file under
`docs/decisions/`.

Stage4A created:

- `docs/decisions/README.md`
- `plans/openlife_repository_stage4a_adr_no_move_index_decision.md`

Stage4A updated:

- `docs/repository_document_governance.md`
- `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md`
- `plans/openlife_repository_stage3f_active_missing_actionability_decision.md`
- `plans/openlife_repository_document_link_baseline.json`
- `plans/openlife_repository_document_inventory.json`

Stage4A regenerated JSON summary:

| Category | Before Stage4A | After Stage4A | Decision |
| --- | ---: | ---: | --- |
| `active_doc_missing_records` | 92 | 76 | ADR blocked active records were resolved by index creation and no-move canonical pointer text. |
| `active_actionable_repair_records` | 0 | 0 | No new actionable repair records introduced. |
| `active_expected_absent_records` | 37 | 37 | Preserved as Phase7 deletion evidence. |
| `active_future_blocked_records` | 39 | 39 | Preserved; no future namespaces created. |
| `active_adr_blocked_records` | 16 | 0 | ADR blocker resolved without moving ADR 0013. |

Stage4A explicit non-actions:

- no Rust/Tauri/React/frontend source code was changed;
- ADR 0013 was not moved;
- no duplicate ADR 0013 file was created under `docs/decisions/`;
- no future namespace was created;
- no authority promotion was performed;
- no Phase7 completion claim was made;
- no Main Chat Agent Execution v1 or live-provider completion claim was made.

Stage4A validation results:

| Command | Result |
| --- | --- |
| `git diff --check` | Passed. |
| `cargo fmt --check` | Passed. |
| `python3 -m json.tool plans/openlife_repository_document_link_baseline.json >/tmp/openlife_repository_document_link_baseline_stage4a_verify.json` | Passed. |
| `python3 -m json.tool plans/openlife_repository_document_inventory.json >/tmp/openlife_repository_document_inventory_stage4a_verify.json` | Passed. |
| `cargo test -p openlife-tauri single_system -- --nocapture` | Passed, 17 tests. |
| `cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture` | Failed as inherited blocker with the same failure set: 24 passed, 2 failed. Failures remain `main_chat_final_gate_aggregation_is_not_hidden_in_test_module` and `main_chat_live_provider_completed_report_builder_is_not_hidden_in_test_module`. |
