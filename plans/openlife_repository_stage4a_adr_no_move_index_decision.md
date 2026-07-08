# OpenLife Repository Stage4A ADR No-Move Index Decision

> Date: 2026-07-07
> Status: documentation decision record only
> Authority: subordinate to `plans/README.md`,
> `plans/openlife_single_system_deletion_manifest.md`,
> `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md`, and
> `plans/openlife_repository_stage3f_active_missing_actionability_decision.md`.

Stage4A resolves the Stage3F/Stage3H `active_adr_blocked_records=16` set by
creating an ADR index and selecting a no-move canonical pointer for ADR 0013.
It does not move ADR 0013 and does not create a duplicate ADR 0013 decision file
under `docs/decisions/`.

## Files Touched

- `docs/decisions/README.md`
- `docs/repository_document_governance.md`
- `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md`
- `plans/openlife_repository_stage3f_active_missing_actionability_decision.md`
- `plans/openlife_repository_document_link_baseline.json`
- `plans/openlife_repository_document_inventory.json`
- `plans/openlife_repository_stage4a_adr_no_move_index_decision.md`

Files and paths intentionally not touched:

- Rust/Tauri/React/frontend source
- `AGENTS.md`
- `README.md`
- `plans/README.md`
- `plans/adr/0013-lifemodel-hs-source-of-truth-governance.md`
- duplicate ADR 0013 file under `docs/decisions/`
- `docs/product/`
- `plans/archive/`
- `plans/active/`

## Decision

Stage4A creates `docs/decisions/README.md` as the ADR decision-log index. The
index lists ADR 0001, ADR 0002, ADR 0003, and ADR 0013 with status, canonical
path, and authority impact.

ADR 0013 remains canonical at:

```text
plans/adr/0013-lifemodel-hs-source-of-truth-governance.md
```

The ADR 0013 index row must point to that existing file and explicitly record
the Stage4A no-move canonical pointer. Any later ADR 0013 move requires a
separate reviewed slice that updates active docs, GitHub governance surfaces,
and JSON baselines together.

## Regenerated JSON Summary

| Category | Before Stage4A | After Stage4A | Decision |
| --- | ---: | ---: | --- |
| `active_doc_missing_records` | 92 | 76 | ADR blocked active records resolved by index creation and no-move canonical pointer text. |
| `active_actionable_repair_records` | 0 | 0 | No new actionable repair records introduced. |
| `active_expected_absent_records` | 37 | 37 | Preserved as Phase7 deletion evidence. |
| `active_future_blocked_records` | 39 | 39 | Preserved; no future namespaces created. |
| `active_adr_blocked_records` | 16 | 0 | ADR blocker resolved without moving ADR 0013. |

## Explicit Non-Actions

- no source code was changed;
- ADR 0013 was not moved;
- no duplicate ADR 0013 file was created under `docs/decisions/`;
- no `docs/product/`, `plans/archive/`, or `plans/active/` namespace was
  created;
- no authority promotion was performed;
- no Phase7 completion claim was made;
- no Main Chat Agent Execution v1 completion claim was made;
- no live-provider evidence completion claim was made.

## Validation

The expected `main_chat_runtime_module` result remains the inherited
two-failure blocker unless a current run proves the same failure set. Stage4A
does not attempt to repair or reclassify that guard.

| Command | Result |
| --- | --- |
| `git diff --check` | Passed. |
| `cargo fmt --check` | Passed. |
| `python3 -m json.tool plans/openlife_repository_document_link_baseline.json >/tmp/openlife_repository_document_link_baseline_stage4a_verify.json` | Passed. |
| `python3 -m json.tool plans/openlife_repository_document_inventory.json >/tmp/openlife_repository_document_inventory_stage4a_verify.json` | Passed. |
| `cargo test -p openlife-tauri single_system -- --nocapture` | Passed, 17 tests. |
| `cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture` | Failed as inherited blocker with the same failure set: 24 passed, 2 failed. Failures remain `main_chat_final_gate_aggregation_is_not_hidden_in_test_module` and `main_chat_live_provider_completed_report_builder_is_not_hidden_in_test_module`. |
