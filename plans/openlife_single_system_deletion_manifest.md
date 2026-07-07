# OpenLife Single-System Deletion Manifest

> Date: 2026-07-07
> Status: Phase7 rerun deletion manifest
> Authority: active Phase7 contract evidence only. Historical stage, beta,
> migration, cutover, productization, maturity, step6, multi-strategy, react
> beta, and legacy-write artifacts do not define the current product path.

This manifest is intentionally conservative: `done` means the object is absent
from the shipped product module graph, shipped command handler, product frontend
page/component surface, product bridge, and active docs. Objects that still
exist only under test/archive/dev paths are classified explicitly and are not
counted as product completion evidence.

## Disposition Vocabulary

| Disposition | Meaning |
| --- | --- |
| `done` | Removed from product build, product UI, product bridge, shipped command handler, and active docs. |
| `product-valid-rename` | Useful logic was moved into a semantically current module name; the old shell was deleted. |
| `test-only-archive` | May remain only under test/archive/dev paths and cannot be imported by product pages or product crate modules. |
| `historical-doc-only` | May remain only as non-authoritative history outside the active docs index. |
| `product-valid` | Current single-system product authority; not an old Phase7 object. |
| `red-until-trial-green` | Product trial blocker remains; Phase7 must not be called complete. |

## Phase7 Object Disposition

| Object | Classification | Disposition | Current evidence |
| --- | --- | --- | --- |
| `src-tauri/src/main_chat_agent_beta_v1_*` | `delete-now` | `done` | Old beta modules and focused tests were deleted from `src-tauri/src`; no product crate module declaration remains. |
| `src-tauri/src/main_chat_agent_stage1_dogfood.rs` | `delete-now` | `done` | Old Stage1 dogfood backend module and tests were deleted from the product crate. |
| `src-tauri/src/main_chat_agent_stage2_readiness.rs` | `delete-now` | `done` | Old Stage2 readiness backend module and tests were deleted from the product crate. |
| `src-tauri/src/main_chat_stage3_execution_ux.rs` | `delete-now` | `done` | Old Stage3 execution UX backend module and tests were deleted from the product crate. |
| `src-tauri/src/main_chat_stage4_memory_knowledge.rs` | `product-valid-rename` | `done` | Product-valid pending memory proposal edit logic moved to `src-tauri/src/main_chat_memory_proposals.rs`; old Stage4 inventory/report shell was deleted. |
| `src-tauri/src/main_chat_stage5_release_debug.rs` | `delete-now` | `done` | Old Stage5 debug/report backend module and tests were deleted from the product crate. |
| `src-tauri/src/main_chat_step6_product_acceptance.rs` | `delete-now` | `done` | Old Step6 backend acceptance module and tests were deleted from the product crate. |
| `src-tauri/src/main_chat_agent_productization_eval.rs` | `delete-now` | `done` | Old productization eval module and tests were deleted from the product crate. |
| `src-tauri/src/main_chat_live_productization_eval.rs` | `delete-now` | `done` | Old live productization eval module was deleted from the product crate. |
| `src-tauri/src/main_chat_product_maturity_v2_final_readiness.rs` | `delete-now` | `done` | Old maturity readiness module was deleted from the product crate. |
| `src-tauri/src/main_chat_memory_lifecycle_eval.rs` | `delete-now` | `done` | Old eval shell was deleted from the product crate. |
| `src-tauri/src/main_chat_plan_interaction_eval.rs` | `delete-now` | `done` | Old eval shell was deleted from the product crate. |
| `src-tauri/src/main_chat_task_continuity_eval.rs` | `delete-now` | `done` | Old eval shell was deleted from the product crate. |
| `src-tauri/src/main_chat_event_stream_tests.rs` Stage/maturity assertions | `test-only-archive` | `done` | Old product-maturity event gate tests were removed instead of keeping a compiled product module alive. |
| `src-tauri/src/commands/agent_runtime/migration_ladder.rs` | `delete-now` | `done` | Old controlled pilot, migration, and cutover command module was deleted; `commands::agent_runtime` exports only current PlanExecute and Main Chat skill/tool commands. |
| `src-tauri/src/lib.rs` shipped handler old commands | `delete-now` | `done` | Stage, beta, migration, cutover, dogfood, productization, maturity, step6, legacy debug, and old eval commands are absent from `tauri::generate_handler!`. |
| `openlife-core/src/agent/multi_strategy_runtime.rs` | `delete-now` | `done` | Old multi-strategy runtime module and tests were deleted from core. |
| `openlife-core/src/agent/react_beta.rs` | `product-valid-rename` | `done` | Metadata-safe digest/preview helpers moved to `openlife-core/src/agent/metadata_safe.rs`; old beta module and tests were deleted. |
| `openlife-core/src/agent/runtime_migration_gate.rs` | `delete-now` | `done` | Old migration gate module and tests were deleted from core. |
| `openlife-core/src/agent/main_chat_agent_productization_v1.rs` | `product-valid-rename` | `done` | Product-valid state snapshot contract moved to `openlife-core/src/agent/main_chat_runtime_contract.rs`; old productization shell was deleted. |
| `src-tauri/src/legacy_write_convergence.rs` | `product-valid-rename` | `done` | Product guard moved to `src-tauri/src/life_model_materializer_guard.rs`; old legacy-write convergence shell and tests were deleted. |
| `frontend/src/pages/settings/MultiStrategyPreviewSection.tsx` | `delete-now` | `done` | Old product settings preview UI was deleted. |
| `frontend/src/pages/settings/multiStrategy/shared.tsx` | `delete-now` | `done` | Old product settings preview helper was deleted. |
| `frontend/src/pages/ChatPage.tsx` legacy fallback state | `delete-now` | `done` | Product Chat no longer owns `legacyFallbackUsed` state, no longer consumes `legacy_fallback_used`, and no longer renders the legacy fallback notice. |
| `frontend/src/tauri.ts` old product bridge wrappers/types | `delete-now` | `done` | Old migration/cutover/beta/stage/productization/maturity/step6 command wrappers and product types were removed from the product bridge. |
| `frontend/src/tauriDev.ts` old wrapper aliases | `test-only-archive` | `done` | Retained as dev/test-only compatibility surface. Product pages/components are guarded from importing it. |
| `frontend/src/types.ts` old route types | `delete-now` | `done` | Old migration/cutover/beta route types were removed from product-facing shared types. |
| `frontend/src/stage1BrowserEvidence.ts` | `test-only-archive` | `done` | Moved to `frontend/src/test/archive/stage1BrowserEvidence.ts`; not a product import. |
| `frontend/src/stage1DogfoodScenarios.ts` | `test-only-archive` | `done` | Moved to `frontend/src/test/archive/stage1DogfoodScenarios.ts`; not a product import. |
| `frontend/src/step6ProductAcceptance.ts` | `test-only-archive` | `done` | Moved to `frontend/src/test/archive/step6ProductAcceptance.ts`; not a product import. |
| Active README Stage/Beta/legacy route narrative | `delete-now` | `done` | Root `README.md` now describes only the current single-system path, current blockers, and trial entry. |
| Active plans index old route recommendations | `delete-now` | `done` | `plans/README.md` keeps single-system authority rules and active files only; old route docs are not active index recommendations. |
| Historical stage/beta/migration/cutover/step docs under `plans/` | `historical-doc-only` | `done` | They may exist only outside the active index and do not carry current development authority. |

## Product-Valid Current Authorities

These are not old Phase7 objects and must not be deleted as part of the old
route cleanup.

| Current object | Authority boundary |
| --- | --- |
| `src-tauri/src/main_chat_kernel.rs` | Current Main Chat product runtime authority used by send/stream. |
| `src-tauri/src/main_chat_turn_runtime.rs` | Single turn terminal/read-model wrapper for product command transport. |
| `src-tauri/src/main_chat_send.rs` and `src-tauri/src/main_chat_streaming.rs` | Product command executors for ordinary send and stream. |
| `src-tauri/src/main_chat_task_controls.rs` | Product task resume/cancel/retry/replay controls. |
| `src-tauri/src/main_chat_runtime_status.rs` | Product runtime/readiness evidence; frontend product pages must not use legacy fallback UI state. |
| `src-tauri/src/main_chat_memory_proposals.rs` | Product-valid pending memory proposal edit helper. |
| `src-tauri/src/life_model_materializer_guard.rs` | Product guard for LifeModel materialization callers. |
| `openlife-core/src/agent/main_chat_runtime_contract.rs` | Product runtime state snapshot contract. |
| `openlife-core/src/agent/metadata_safe.rs` | Shared metadata-safe digest/preview helper. |
| `frontend/src/tauri.ts` | Product bridge only. |
| `frontend/src/pages/ChatPage.tsx` | Product Chat UI, now consuming current task/session/read-model evidence instead of legacy fallback state. |

## Shipped Command Surface Result

All old Phase7 command families are absent from the shipped handler:

- multi-strategy preview
- runtime migration gate
- controlled pilot, migration, and cutover
- beta readiness
- stage1/stage2/stage3/stage4/stage5 reports and setup commands
- step6 product acceptance
- productization and maturity eval/readiness gates
- old debug bundle/internal issue report command family

The only broad-token command names still allowed in the handler are the current
product Builder commands `goal_capability_gap_analysis` and
`goal_capability_gap_report`.

### Product Allowlist Commands

These commands are the product allowlist for broad-token handler scans. They are
not legacy/development routes.

| Product allowlist command | Reason |
| --- | --- |
| `goal_capability_gap_analysis` | Read-only Builder product analysis; not a stage, beta, migration, cutover, eval, maturity, or productization route. |
| `goal_capability_gap_report` | Read-only Builder product report; not a stage, beta, migration, cutover, eval, maturity, or productization route. |

### Retired Legacy/Development Command Inventory

Every command below is classified as `done`: it is absent from the shipped
Tauri handler and cannot be called through the product bridge.

| Retired command | Disposition |
| --- | --- |
| `run_multi_strategy_agent_preview` | `done` |
| `run_main_chat_agent_execution_v1_eval_gate` | `done` |
| `run_main_chat_capability_eval_gate` | `done` |
| `run_main_chat_agent_productization_v1_gate` | `done` |
| `run_main_chat_external_live_productization_gate` | `done` |
| `run_main_chat_agent_product_maturity_v2_event_gate` | `done` |
| `run_main_chat_agent_product_maturity_v2_plan_gate` | `done` |
| `run_main_chat_agent_product_maturity_v2_skills_gate` | `done` |
| `run_main_chat_agent_product_maturity_v2_final_readiness_gate` | `done` |
| `run_main_chat_agent_beta_v1_readiness_gate` | `done` |
| `run_main_chat_agent_stage1_dogfood_gate` | `done` |
| `run_main_chat_agent_stage2_readiness_gate` | `done` |
| `run_main_chat_agent_step6_product_acceptance_gate` | `done` |
| `prepare_main_chat_step6_live_provider_eval_state` | `done` |
| `run_main_chat_stage3_execution_ux_report` | `done` |
| `validate_main_chat_agent_stage2_manual_dogfood_artifact` | `done` |
| `prepare_main_chat_agent_stage1_browser_dogfood_state` | `done` |
| `set_main_chat_agent_stage1_browser_network_policy` | `done` |
| `set_main_chat_agent_stage1_browser_scripted_response` | `done` |
| `set_main_chat_agent_stage1_browser_web_fixture_output` | `done` |
| `run_main_chat_agent_execution_v1_final_acceptance_gate` | `done` |
| `get_runtime_strategy_registry_status` | `done` |
| `get_react_beta_execution_status` | `done` |
| `check_runtime_migration_gate` | `done` |
| `check_controlled_chat_pilot_eligibility` | `done` |
| `check_controlled_pilot_promotion_readiness` | `done` |
| `draft_controlled_chat_migration_plan` | `done` |
| `record_controlled_chat_migration_review_decision` | `done` |
| `get_controlled_chat_migration_review_decision_summary` | `done` |
| `check_controlled_chat_migration_implementation_gate` | `done` |
| `run_controlled_chat_migration_shadow_run` | `done` |
| `record_controlled_chat_migration_shadow_review_decision` | `done` |
| `get_controlled_chat_migration_shadow_review_summary` | `done` |
| `check_controlled_chat_cutover_readiness` | `done` |
| `run_controlled_chat_cutover_candidate` | `done` |
| `record_controlled_chat_cutover_candidate_review_decision` | `done` |
| `get_controlled_chat_cutover_candidate_review_summary` | `done` |
| `check_controlled_chat_cutover_candidate_promotion_readiness` | `done` |
| `record_controlled_pilot_promotion_evidence` | `done` |
| `get_controlled_pilot_promotion_evidence_summary` | `done` |
| `list_stage4_knowledge_asset_inventory` | `done` |
| `run_main_chat_stage4_memory_knowledge_report` | `done` |
| `evaluate_main_chat_stage5_release_debug_preflight` | `done` |
| `export_main_chat_agent_debug_bundle` | `done` |
| `list_main_chat_debug_bundles` | `done` |
| `get_main_chat_debug_bundle` | `done` |
| `delete_main_chat_debug_bundle` | `done` |
| `create_main_chat_internal_issue_report` | `done` |
| `list_main_chat_internal_issue_reports` | `done` |
| `get_main_chat_internal_issue_report` | `done` |
| `delete_main_chat_internal_issue_report` | `done` |
| `run_main_chat_stage5_release_debug_report` | `done` |

## Guard Coverage

Phase7 hard-delete guards now require:

- old route markers are zero in product source, with only explicit
  test/archive/dev allowlist paths permitted;
- shipped handler old command count is zero;
- product crate module graph old stage/beta/productization/migration/cutover
  module count is zero;
- product frontend pages/components do not import `tauriDev.ts`;
- product frontend pages do not consume `legacy_fallback_used` or
  `legacyFallbackUsed`;
- active docs do not authorize Stage/Beta/migration/cutover/legacy route
  development.

## Computer Use Trial Status

Trial report path:
`frontend/test-results/phase7-computer-use-trial/trial-report.md`.

Status: `red-until-trial-green`.

The prior trial found real product blockers around external fact requests,
proposal resolution/task state, first LifeModel quick-build next steps, and
cross-page state consistency. Phase7 is not complete until the rerun either
turns green or remains red with fail-closed behavior that is explicit,
auditable, and consistent across Companion/Runs/Mailbox/Today.
