# OpenLife Repository Stage4B Future Namespace Decision

> Decision date: 2026-07-07
> Status: Stage4B docs-only future namespace reference rewording.
> Authority: subordinate to `AGENTS.md`, `plans/README.md`,
> `plans/openlife_single_system_deletion_manifest.md`, and
> `plans/openlife_single_system_development_preparation.md`.

Stage4B resolves the remaining active future namespace blocker records by
rewording active documentation references. It does not create future
directories, placeholder files, or moved plan trees.

## Decision

Future repository namespace labels remain blocked until a later reviewed slice
creates real approved content in the same patch. Active docs should describe
those labels as governance decisions, not as concrete local path targets.

## Input Records

Stage4B starts from the Stage4A baseline:

| Category | Before Stage4B |
| --- | ---: |
| `active_doc_missing_records` | 76 |
| `active_actionable_repair_records` | 0 |
| `active_expected_absent_records` | 37 |
| `active_future_blocked_records` | 39 |
| `active_adr_blocked_records` | 0 |

The 39 future namespace records came from:

| Source | Records |
| --- | ---: |
| `docs/repository_document_governance.md` | 8 |
| `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md` | 27 |
| `plans/openlife_repository_stage2c_phase_c_readiness_decision.md` | 4 |

## Stage4B Actions

- Reworded local/private/draft governance from missing paths into reserved
  namespace labels and existing local-only note roots.
- Reworded product-doc, plan-archive, and active-plan references so active docs
  no longer imply those targets should exist now.
- Preserved expected-absent Phase7 deletion evidence without restoring removed
  files.
- Preserved the Stage4A ADR no-move decision and did not move ADR 0013.

## Output Records

After Stage4B, the baseline target is:

| Category | After Stage4B |
| --- | ---: |
| `active_doc_missing_records` | 37 |
| `active_actionable_repair_records` | 0 |
| `active_expected_absent_records` | 37 |
| `active_future_blocked_records` | 0 |
| `active_adr_blocked_records` | 0 |

## Boundaries

Stage4B is docs and baseline metadata only:

- no Rust, Tauri, React, or frontend source edits;
- no future namespace creation;
- no placeholder file creation;
- no broad plan move;
- no ADR 0013 move;
- no Phase7 completion claim;
- no Main Chat Agent Execution v1 completion claim;
- no live-provider evidence completion claim;
- no runtime-module green claim.

The inherited runtime-module blocker remains acceptable only if the failure set
and count stay at 24 passed / 2 failed with the same two named failures.

## Validation Results

| Command | Result |
| --- | --- |
| `git diff --check` | Passed. |
| `cargo fmt --check` | Passed. |
| `python3 -m json.tool plans/openlife_repository_document_link_baseline.json >/tmp/openlife_repository_document_link_baseline_stage4b_verify.json` | Passed. |
| `python3 -m json.tool plans/openlife_repository_document_inventory.json >/tmp/openlife_repository_document_inventory_stage4b_verify.json` | Passed. |
| Future namespace absence shell check | Passed. |
| `cargo test -p openlife-tauri single_system -- --nocapture` | Passed, 17 tests. |
| `cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture` | Failed as the accepted inherited blocker: 24 passed, 2 failed. Failures remain `main_chat_final_gate_aggregation_is_not_hidden_in_test_module` and `main_chat_live_provider_completed_report_builder_is_not_hidden_in_test_module`. |
