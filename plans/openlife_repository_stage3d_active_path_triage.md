# OpenLife Repository Stage3D Active Path Triage

> Date: 2026-07-07
> Status: triage artifact only; no path repair implementation
> Scope: classify active missing path records from the Stage3B link baseline
> Authority: subordinate to `AGENTS.md`, `plans/README.md`,
> `plans/openlife_single_system_deletion_manifest.md`, and
> `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md`

Stage3D does not repair links, move ADR files, create product docs, edit runtime
source, or promote repository cleanup into runtime authority.

## Input Baseline

Input file:

```text
plans/openlife_repository_document_link_baseline.json
```

The baseline was used as an input artifact only. It was not modified by this
Stage3D pass.

Extraction rule:

```text
missing_local_paths where broken_link_type == active_doc_broken_path_mention
```

Count check:

| Check | Value |
| --- | ---: |
| `summary.active_doc_missing_records` | 171 |
| Extracted active records | 171 |
| Group key | `source_path`, `raw_target`, `resolved_path` |
| Unique groups | 114 |

## Triage Categories

| Category | Meaning |
| --- | --- |
| `retarget_now_candidate` | The intended existing target is clear, such as `.github/*` or a repo-root relative path. A later repair slice can retarget without creating new architecture. |
| `remove_or_reword_candidate` | The target is a stale/example/non-path phrase. Do not create a file only to satisfy it. |
| `future_path_reference_keep_blocked` | The path describes a planned namespace or future document. Keep blocked until an approved creation or move slice. |
| `historical_should_not_be_active` | The path is retired, deleted, or historical implementation evidence. It may remain only as a quoted historical/deletion label or be reworded out of active path space. |
| `adr_consolidation_blocker` | The missing target is tied to ADR index or ADR 0013 canonical-path consolidation. Do not resolve it outside an ADR slice. |
| `needs_user_decision` | The text is ambiguous shorthand or conceptual path text. The user should decide whether to make it real, enumerate existing files, or reword it. |

Category totals:

| Category | Records | Groups |
| --- | ---: | ---: |
| `retarget_now_candidate` | 33 | 19 |
| `remove_or_reword_candidate` | 14 | 10 |
| `future_path_reference_keep_blocked` | 29 | 19 |
| `historical_should_not_be_active` | 83 | 59 |
| `adr_consolidation_blocker` | 8 | 5 |
| `needs_user_decision` | 4 | 2 |
| **Total** | **171** | **114** |

## Source File Counts

| Source file | Active records |
| --- | ---: |
| `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md` | 47 |
| `plans/openlife_repository_active_claim_audit.md` | 31 |
| `plans/openlife_single_system_deletion_manifest.md` | 24 |
| `plans/openlife_single_system_development_preparation.md` | 20 |
| `AGENTS.md` | 10 |
| `plans/openlife_repository_stage2a_scope_decision.md` | 10 |
| `docs/repository_document_governance.md` | 9 |
| `docs/github_repository_governance.md` | 8 |
| `plans/openlife_repository_stage2c_phase_c_readiness_decision.md` | 5 |
| `docs/DEV_HANDOVER.md` | 3 |
| `CONTRIBUTING.md` | 2 |
| `docs/decisions/0002-proposal-unified.md` | 1 |
| `docs/decisions/0003-agent-run-tracking.md` | 1 |
| **Total** | **171** |

## ADR Missing Targets

These are the ADR-related active missing targets that block ADR consolidation
until a dedicated ADR slice exists.

| Missing target | Records | Sources and lines | Stage3D classification |
| --- | ---: | --- | --- |
| `docs/decisions/README.md` | 5 | `docs/repository_document_governance.md:150`; `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md:356,367,774,817` | `adr_consolidation_blocker` |
| `docs/decisions/0013` | 1 | `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md:203` | `adr_consolidation_blocker` |
| `docs/decisions/0013-lifemodel-hs-source-of-truth-governance.md` | 1 | `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md:360` | `adr_consolidation_blocker` |
| `plans/adr/0013` | 1 | `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md:203` | `adr_consolidation_blocker` |

Adjacent ADR shorthand:

| Missing target | Records | Sources and lines | Stage3D classification |
| --- | ---: | --- | --- |
| `docs/decisions/0001-0003` | 3 | `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md:71,195,357` | `needs_user_decision` |

Stage3D decision: ADR consolidation is still blocked. ADR 0013 remains at its
current canonical path until a later same-slice ADR implementation updates the
decision index, ownership, templates, active docs, and regenerated baselines.

## Grouped Triage

| Category | Records | Source | Lines | Raw target | Resolved path | Triage note |
| --- | ---: | --- | --- | --- | --- | --- |
| `historical_should_not_be_active` | 2 | `AGENTS.md` | `72,78` | `docs/index` | `docs/index` | Historical progress/doc-index text; do not create a current doc target from it. |
| `historical_should_not_be_active` | 1 | `AGENTS.md` | `35` | `docs/progress` | `docs/progress` | Historical progress text; keep only as historical wording or reword. |
| `historical_should_not_be_active` | 1 | `AGENTS.md` | `35` | `docs/progress/verification` | `docs/progress/verification` | Historical verification text; keep only as historical wording or reword. |
| `historical_should_not_be_active` | 1 | `AGENTS.md` | `228` | `frontend/src/utils/previewAudit.ts` | `frontend/src/utils/previewAudit.ts` | Retired preview/audit surface; do not retarget as product source. |
| `historical_should_not_be_active` | 4 | `AGENTS.md` | `869,875,876,877` | `src-tauri/src/legacy_write_convergence.rs` | `src-tauri/src/legacy_write_convergence.rs` | Historical W79-W87 path; current product guard is a different file and should not be implied silently. |
| `historical_should_not_be_active` | 1 | `AGENTS.md` | `27` | `src-tauri/src/main_chat_final_acceptance_tests.rs` | `src-tauri/src/main_chat_final_acceptance_tests.rs` | Missing old test-owner path; do not restore it through docs cleanup. |
| `remove_or_reword_candidate` | 1 | `CONTRIBUTING.md` | `92` | `docs/architecture-update` | `docs/architecture-update` | Branch-name example, not a local documentation target. |
| `historical_should_not_be_active` | 1 | `CONTRIBUTING.md` | `71` | `openlife-core/src/hermes.rs` | `openlife-core/src/hermes.rs` | Historical concept note; keep as non-current wording only. |
| `retarget_now_candidate` | 1 | `docs/DEV_HANDOVER.md` | `255` | `/AGENTS.md` | `/AGENTS.md` | Remove leading slash and use `AGENTS.md`. |
| `retarget_now_candidate` | 1 | `docs/DEV_HANDOVER.md` | `257` | `/README.md` | `/README.md` | Remove leading slash and use `README.md`. |
| `retarget_now_candidate` | 1 | `docs/DEV_HANDOVER.md` | `256` | `/plans/openlife_development_plan.md` | `/plans/openlife_development_plan.md` | Remove leading slash and use `plans/openlife_development_plan.md`. |
| `historical_should_not_be_active` | 1 | `docs/decisions/0002-proposal-unified.md` | `104` | `frontend/src/pages/ProposalReviewPage.tsx` | `frontend/src/pages/ProposalReviewPage.tsx` | Historical implementation reference inside ADR text. |
| `historical_should_not_be_active` | 1 | `docs/decisions/0003-agent-run-tracking.md` | `94` | `frontend/src/components/HermesTracePanel.tsx` | `frontend/src/components/HermesTracePanel.tsx` | Historical implementation reference inside ADR text. |
| `retarget_now_candidate` | 1 | `docs/github_repository_governance.md` | `148` | `github/CODEOWNERS` | `github/CODEOWNERS` | Retarget to `.github/CODEOWNERS`. |
| `retarget_now_candidate` | 1 | `docs/github_repository_governance.md` | `142` | `github/ISSUE_TEMPLATE/00_0_quick_task.yml` | `github/ISSUE_TEMPLATE/00_0_quick_task.yml` | Retarget to `.github/ISSUE_TEMPLATE/00_0_quick_task.yml`. |
| `retarget_now_candidate` | 1 | `docs/github_repository_governance.md` | `143` | `github/ISSUE_TEMPLATE/03_bug_report.yml` | `github/ISSUE_TEMPLATE/03_bug_report.yml` | Retarget to `.github/ISSUE_TEMPLATE/03_bug_report.yml`. |
| `retarget_now_candidate` | 1 | `docs/github_repository_governance.md` | `144` | `github/ISSUE_TEMPLATE/04_adr_proposal.yml` | `github/ISSUE_TEMPLATE/04_adr_proposal.yml` | Retarget to `.github/ISSUE_TEMPLATE/04_adr_proposal.yml`. |
| `retarget_now_candidate` | 1 | `docs/github_repository_governance.md` | `146` | `github/PULL_REQUEST_TEMPLATE.md` | `github/PULL_REQUEST_TEMPLATE.md` | Retarget to `.github/PULL_REQUEST_TEMPLATE.md`. |
| `retarget_now_candidate` | 1 | `docs/github_repository_governance.md` | `150` | `github/dependabot.yml` | `github/dependabot.yml` | Retarget to `.github/dependabot.yml`. |
| `retarget_now_candidate` | 1 | `docs/github_repository_governance.md` | `147` | `github/labels.yml` | `github/labels.yml` | Retarget to `.github/labels.yml`. |
| `retarget_now_candidate` | 1 | `docs/github_repository_governance.md` | `149` | `github/workflows/ci.yml` | `github/workflows/ci.yml` | Retarget to `.github/workflows/ci.yml`. |
| `adr_consolidation_blocker` | 1 | `docs/repository_document_governance.md` | `150` | `docs/decisions/README.md` | `docs/decisions/README.md` | ADR index target; keep blocked until ADR slice. |
| `future_path_reference_keep_blocked` | 1 | `docs/repository_document_governance.md` | `36` | `docs/local/` | `docs/local` | Future/private namespace policy; do not create empty directory. |
| `future_path_reference_keep_blocked` | 1 | `docs/repository_document_governance.md` | `35` | `docs/private/` | `docs/private` | Future/private namespace policy; do not create empty directory. |
| `future_path_reference_keep_blocked` | 1 | `docs/repository_document_governance.md` | `125` | `docs/product/` | `docs/product` | Public product-doc namespace remains blocked until approved content exists. |
| `future_path_reference_keep_blocked` | 2 | `docs/repository_document_governance.md` | `129,151` | `plans/archive/` | `plans/archive` | Plans archive remains blocked until link impact is approved. |
| `future_path_reference_keep_blocked` | 1 | `docs/repository_document_governance.md` | `39` | `plans/drafts/` | `plans/drafts` | Future/local namespace policy; do not create empty directory. |
| `future_path_reference_keep_blocked` | 1 | `docs/repository_document_governance.md` | `38` | `plans/local/` | `plans/local` | Future/local namespace policy; do not create empty directory. |
| `future_path_reference_keep_blocked` | 1 | `docs/repository_document_governance.md` | `37` | `plans/private/` | `plans/private` | Future/private namespace policy; do not create empty directory. |
| `retarget_now_candidate` | 1 | `plans/openlife_repository_active_claim_audit.md` | `66` | `/plans/openlife_development_plan.md` | `/plans/openlife_development_plan.md` | Remove leading slash and use `plans/openlife_development_plan.md`. |
| `remove_or_reword_candidate` | 3 | `plans/openlife_repository_active_claim_audit.md` | `36,49,110` | `docs/ARCHITECTUREDETAILED.md` | `docs/ARCHITECTUREDETAILED.md` | Stale missing architecture link; do not create the old target. |
| `remove_or_reword_candidate` | 3 | `plans/openlife_repository_active_claim_audit.md` | `36,49,110` | `docs/api/` | `docs/api` | Stale missing API-doc link; do not create the old target. |
| `remove_or_reword_candidate` | 1 | `plans/openlife_repository_active_claim_audit.md` | `88` | `docs/architecture-update` | `docs/architecture-update` | Branch-name example, not a local documentation target. |
| `historical_should_not_be_active` | 1 | `plans/openlife_repository_active_claim_audit.md` | `86` | `docs/index` | `docs/index` | Historical active-claim audit target; reword as missing historical reference. |
| `historical_should_not_be_active` | 1 | `plans/openlife_repository_active_claim_audit.md` | `86` | `docs/progress` | `docs/progress` | Historical active-claim audit target; reword as missing historical reference. |
| `historical_should_not_be_active` | 1 | `plans/openlife_repository_active_claim_audit.md` | `86` | `docs/progress/verification` | `docs/progress/verification` | Historical active-claim audit target; reword as missing historical reference. |
| `historical_should_not_be_active` | 1 | `plans/openlife_repository_active_claim_audit.md` | `62` | `frontend/src/utils/previewAudit.ts` | `frontend/src/utils/previewAudit.ts` | Retired preview/audit reference. |
| `retarget_now_candidate` | 2 | `plans/openlife_repository_active_claim_audit.md` | `37,92` | `github/PULL_REQUEST_TEMPLATE.md` | `github/PULL_REQUEST_TEMPLATE.md` | Retarget to `.github/PULL_REQUEST_TEMPLATE.md`. |
| `historical_should_not_be_active` | 3 | `plans/openlife_repository_active_claim_audit.md` | `34,61,85` | `openlife-core/src/agent/multi_strategy_runtime.rs` | `openlife-core/src/agent/multi_strategy_runtime.rs` | Deleted runtime path; keep only as historical/deleted label. |
| `historical_should_not_be_active` | 3 | `plans/openlife_repository_active_claim_audit.md` | `34,63,85` | `openlife-core/src/agent/runtime_migration_gate.rs` | `openlife-core/src/agent/runtime_migration_gate.rs` | Deleted migration-gate path; keep only as historical/deleted label. |
| `historical_should_not_be_active` | 2 | `plans/openlife_repository_active_claim_audit.md` | `33,50` | `openlife-core/src/hermes.rs` | `openlife-core/src/hermes.rs` | Deleted historical concept path. |
| `historical_should_not_be_active` | 1 | `plans/openlife_repository_active_claim_audit.md` | `33` | `openlife-core/src/layer_router.rs` | `openlife-core/src/layer_router.rs` | Deleted historical router path. |
| `historical_should_not_be_active` | 1 | `plans/openlife_repository_active_claim_audit.md` | `33` | `openlife-core/src/router.rs` | `openlife-core/src/router.rs` | Deleted historical router path. |
| `historical_should_not_be_active` | 1 | `plans/openlife_repository_active_claim_audit.md` | `87` | `src-tauri/src/legacy_write_convergence.rs` | `src-tauri/src/legacy_write_convergence.rs` | Historical legacy-write path; do not retarget silently. |
| `historical_should_not_be_active` | 3 | `plans/openlife_repository_active_claim_audit.md` | `29,83,106` | `src-tauri/src/main_chat_final_acceptance_tests.rs` | `src-tauri/src/main_chat_final_acceptance_tests.rs` | Missing old test-owner path; do not restore through docs cleanup. |
| `historical_should_not_be_active` | 1 | `plans/openlife_repository_active_claim_audit.md` | `84` | `src-tauri/src/main_chat_legacy_agent_loop.rs` | `src-tauri/src/main_chat_legacy_agent_loop.rs` | Deleted legacy-loop path. |
| `historical_should_not_be_active` | 1 | `plans/openlife_repository_active_claim_audit.md` | `84` | `src-tauri/src/main_chat_legacy_fallback.rs` | `src-tauri/src/main_chat_legacy_fallback.rs` | Deleted legacy-fallback path. |
| `historical_should_not_be_active` | 1 | `plans/openlife_repository_active_claim_audit.md` | `84` | `src-tauri/src/main_chat_strategy.rs` | `src-tauri/src/main_chat_strategy.rs` | Deleted historical strategy path. |
| `remove_or_reword_candidate` | 1 | `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md` | `167` | `docs/ARCHITECTUREDETAILED.md` | `docs/ARCHITECTUREDETAILED.md` | Stale architecture target; do not create the old file. |
| `remove_or_reword_candidate` | 1 | `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md` | `424` | `docs/api` | `docs/api` | Stale scan target text; reword or quote as non-path. |
| `remove_or_reword_candidate` | 1 | `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md` | `168` | `docs/api/` | `docs/api` | Stale architecture target; do not create the old directory. |
| `needs_user_decision` | 3 | `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md` | `71,195,357` | `docs/decisions/0001-0003` | `docs/decisions/0001-0003` | Decide whether to enumerate existing ADR files or wait for an ADR index. |
| `adr_consolidation_blocker` | 1 | `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md` | `203` | `docs/decisions/0013` | `docs/decisions/0013` | ADR canonical-path target; blocked. |
| `adr_consolidation_blocker` | 1 | `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md` | `360` | `docs/decisions/0013-lifemodel-hs-source-of-truth-governance.md` | `docs/decisions/0013-lifemodel-hs-source-of-truth-governance.md` | ADR canonical-path target; blocked. |
| `adr_consolidation_blocker` | 4 | `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md` | `356,367,774,817` | `docs/decisions/README.md` | `docs/decisions/README.md` | ADR index target; blocked. |
| `future_path_reference_keep_blocked` | 2 | `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md` | `65,80` | `docs/product` | `docs/product` | Future product-doc namespace; blocked. |
| `future_path_reference_keep_blocked` | 5 | `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md` | `214,679,722,775,818` | `docs/product/` | `docs/product` | Future product-doc namespace; blocked. |
| `future_path_reference_keep_blocked` | 1 | `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md` | `336` | `docs/product/scenarios.md` | `docs/product/scenarios.md` | Future product doc; blocked until approved. |
| `future_path_reference_keep_blocked` | 1 | `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md` | `336` | `docs/product/vision.md` | `docs/product/vision.md` | Future product doc; blocked until approved. |
| `needs_user_decision` | 1 | `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md` | `472` | `docs/types` | `docs/types` | Ambiguous phrase; decide whether it means docs and types or a real path. |
| `retarget_now_candidate` | 4 | `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md` | `72,248,315,363` | `github/CODEOWNERS` | `github/CODEOWNERS` | Retarget to `.github/CODEOWNERS`. |
| `retarget_now_candidate` | 2 | `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md` | `72,364` | `github/ISSUE_TEMPLATE/04_adr_proposal.yml` | `github/ISSUE_TEMPLATE/04_adr_proposal.yml` | Retarget to `.github/ISSUE_TEMPLATE/04_adr_proposal.yml`. |
| `retarget_now_candidate` | 1 | `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md` | `365` | `github/ISSUE_TEMPLATE/config.yml` | `github/ISSUE_TEMPLATE/config.yml` | Retarget to `.github/ISSUE_TEMPLATE/config.yml`. |
| `retarget_now_candidate` | 5 | `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md` | `72,247,314,512,588` | `github/PULL_REQUEST_TEMPLATE.md` | `github/PULL_REQUEST_TEMPLATE.md` | Retarget to `.github/PULL_REQUEST_TEMPLATE.md`. |
| `future_path_reference_keep_blocked` | 1 | `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md` | `69` | `plans/active` | `plans/active` | Future plan namespace; blocked. |
| `future_path_reference_keep_blocked` | 1 | `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md` | `84` | `plans/active/` | `plans/active` | Future plan namespace; blocked. |
| `adr_consolidation_blocker` | 1 | `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md` | `203` | `plans/adr/0013` | `plans/adr/0013` | ADR canonical-path shorthand; blocked. |
| `future_path_reference_keep_blocked` | 1 | `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md` | `84` | `plans/archive` | `plans/archive` | Future archive namespace; blocked. |
| `future_path_reference_keep_blocked` | 4 | `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md` | `246,385,776,818` | `plans/archive/` | `plans/archive` | Future archive namespace; blocked. |
| `future_path_reference_keep_blocked` | 1 | `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md` | `323` | `plans/archive/index` | `plans/archive/index` | Future archive index; blocked. |
| `retarget_now_candidate` | 1 | `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md` | `280` | `plans/openlife_repository_document_inventory.md` | `plans/openlife_repository_document_inventory.md` | Retarget to existing JSON baseline or reword as `.json` artifact. |
| `historical_should_not_be_active` | 3 | `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md` | `125,405,449` | `src-tauri/src/main_chat_final_acceptance_tests.rs` | `src-tauri/src/main_chat_final_acceptance_tests.rs` | Missing old test-owner path; do not restore through docs cleanup. |
| `remove_or_reword_candidate` | 1 | `plans/openlife_repository_stage2a_scope_decision.md` | `58` | `docs/API` | `docs/API` | Stale case variant of missing API docs; do not create target. |
| `remove_or_reword_candidate` | 1 | `plans/openlife_repository_stage2a_scope_decision.md` | `87` | `docs/ARCHITECTUREDETAILED.md` | `docs/ARCHITECTUREDETAILED.md` | Stale architecture target; do not create the old file. |
| `remove_or_reword_candidate` | 1 | `plans/openlife_repository_stage2a_scope_decision.md` | `87` | `docs/api/` | `docs/api` | Stale API-doc target; do not create the old directory. |
| `retarget_now_candidate` | 6 | `plans/openlife_repository_stage2a_scope_decision.md` | `12,31,63,69,89,97` | `github/PULL_REQUEST_TEMPLATE.md` | `github/PULL_REQUEST_TEMPLATE.md` | Retarget to `.github/PULL_REQUEST_TEMPLATE.md`. |
| `historical_should_not_be_active` | 1 | `plans/openlife_repository_stage2a_scope_decision.md` | `84` | `src-tauri/src/main_chat_final_acceptance_tests.rs` | `src-tauri/src/main_chat_final_acceptance_tests.rs` | Missing old test-owner path; do not restore through docs cleanup. |
| `future_path_reference_keep_blocked` | 2 | `plans/openlife_repository_stage2c_phase_c_readiness_decision.md` | `59,204` | `docs/product/` | `docs/product` | Future product-doc namespace; blocked. |
| `future_path_reference_keep_blocked` | 1 | `plans/openlife_repository_stage2c_phase_c_readiness_decision.md` | `82` | `docs/product/scenarios.md` | `docs/product/scenarios.md` | Future product doc; blocked until approved. |
| `future_path_reference_keep_blocked` | 1 | `plans/openlife_repository_stage2c_phase_c_readiness_decision.md` | `81` | `docs/product/vision.md` | `docs/product/vision.md` | Future product doc; blocked until approved. |
| `historical_should_not_be_active` | 1 | `plans/openlife_repository_stage2c_phase_c_readiness_decision.md` | `197` | `src-tauri/src/main_chat_final_acceptance_tests.rs` | `src-tauri/src/main_chat_final_acceptance_tests.rs` | Missing old test-owner path; do not restore through docs cleanup. |
| `historical_should_not_be_active` | 1 | `plans/openlife_single_system_deletion_manifest.md` | `51` | `frontend/src/pages/settings/MultiStrategyPreviewSection.tsx` | `frontend/src/pages/settings/MultiStrategyPreviewSection.tsx` | Retired product UI path named as deletion-manifest object. |
| `historical_should_not_be_active` | 1 | `plans/openlife_single_system_deletion_manifest.md` | `52` | `frontend/src/pages/settings/multiStrategy/shared.tsx` | `frontend/src/pages/settings/multiStrategy/shared.tsx` | Retired product UI helper path named as deletion-manifest object. |
| `historical_should_not_be_active` | 1 | `plans/openlife_single_system_deletion_manifest.md` | `57` | `frontend/src/stage1BrowserEvidence.ts` | `frontend/src/stage1BrowserEvidence.ts` | Retired old test path; moved archive path is separate evidence. |
| `historical_should_not_be_active` | 1 | `plans/openlife_single_system_deletion_manifest.md` | `58` | `frontend/src/stage1DogfoodScenarios.ts` | `frontend/src/stage1DogfoodScenarios.ts` | Retired old test path; moved archive path is separate evidence. |
| `historical_should_not_be_active` | 1 | `plans/openlife_single_system_deletion_manifest.md` | `59` | `frontend/src/step6ProductAcceptance.ts` | `frontend/src/step6ProductAcceptance.ts` | Retired old test path; moved archive path is separate evidence. |
| `historical_should_not_be_active` | 1 | `plans/openlife_single_system_deletion_manifest.md` | `49` | `openlife-core/src/agent/main_chat_agent_productization_v1.rs` | `openlife-core/src/agent/main_chat_agent_productization_v1.rs` | Retired source path named as deletion-manifest object. |
| `historical_should_not_be_active` | 1 | `plans/openlife_single_system_deletion_manifest.md` | `46` | `openlife-core/src/agent/multi_strategy_runtime.rs` | `openlife-core/src/agent/multi_strategy_runtime.rs` | Retired source path named as deletion-manifest object. |
| `historical_should_not_be_active` | 1 | `plans/openlife_single_system_deletion_manifest.md` | `47` | `openlife-core/src/agent/react_beta.rs` | `openlife-core/src/agent/react_beta.rs` | Retired source path named as deletion-manifest object. |
| `historical_should_not_be_active` | 1 | `plans/openlife_single_system_deletion_manifest.md` | `48` | `openlife-core/src/agent/runtime_migration_gate.rs` | `openlife-core/src/agent/runtime_migration_gate.rs` | Retired source path named as deletion-manifest object. |
| `historical_should_not_be_active` | 1 | `plans/openlife_single_system_deletion_manifest.md` | `44` | `src-tauri/src/commands/agent_runtime/migration_ladder.rs` | `src-tauri/src/commands/agent_runtime/migration_ladder.rs` | Retired source path named as deletion-manifest object. |
| `historical_should_not_be_active` | 1 | `plans/openlife_single_system_deletion_manifest.md` | `50` | `src-tauri/src/legacy_write_convergence.rs` | `src-tauri/src/legacy_write_convergence.rs` | Retired source path named as deletion-manifest object. |
| `historical_should_not_be_active` | 1 | `plans/openlife_single_system_deletion_manifest.md` | `37` | `src-tauri/src/main_chat_agent_productization_eval.rs` | `src-tauri/src/main_chat_agent_productization_eval.rs` | Retired source path named as deletion-manifest object. |
| `historical_should_not_be_active` | 1 | `plans/openlife_single_system_deletion_manifest.md` | `31` | `src-tauri/src/main_chat_agent_stage1_dogfood.rs` | `src-tauri/src/main_chat_agent_stage1_dogfood.rs` | Retired source path named as deletion-manifest object. |
| `historical_should_not_be_active` | 1 | `plans/openlife_single_system_deletion_manifest.md` | `32` | `src-tauri/src/main_chat_agent_stage2_readiness.rs` | `src-tauri/src/main_chat_agent_stage2_readiness.rs` | Retired source path named as deletion-manifest object. |
| `historical_should_not_be_active` | 1 | `plans/openlife_single_system_deletion_manifest.md` | `43` | `src-tauri/src/main_chat_event_stream_tests.rs` | `src-tauri/src/main_chat_event_stream_tests.rs` | Retired test path named as deletion-manifest object. |
| `historical_should_not_be_active` | 1 | `plans/openlife_single_system_deletion_manifest.md` | `38` | `src-tauri/src/main_chat_live_productization_eval.rs` | `src-tauri/src/main_chat_live_productization_eval.rs` | Retired source path named as deletion-manifest object. |
| `historical_should_not_be_active` | 1 | `plans/openlife_single_system_deletion_manifest.md` | `40` | `src-tauri/src/main_chat_memory_lifecycle_eval.rs` | `src-tauri/src/main_chat_memory_lifecycle_eval.rs` | Retired source path named as deletion-manifest object. |
| `historical_should_not_be_active` | 1 | `plans/openlife_single_system_deletion_manifest.md` | `41` | `src-tauri/src/main_chat_plan_interaction_eval.rs` | `src-tauri/src/main_chat_plan_interaction_eval.rs` | Retired source path named as deletion-manifest object. |
| `historical_should_not_be_active` | 1 | `plans/openlife_single_system_deletion_manifest.md` | `39` | `src-tauri/src/main_chat_product_maturity_v2_final_readiness.rs` | `src-tauri/src/main_chat_product_maturity_v2_final_readiness.rs` | Retired source path named as deletion-manifest object. |
| `historical_should_not_be_active` | 1 | `plans/openlife_single_system_deletion_manifest.md` | `33` | `src-tauri/src/main_chat_stage3_execution_ux.rs` | `src-tauri/src/main_chat_stage3_execution_ux.rs` | Retired source path named as deletion-manifest object. |
| `historical_should_not_be_active` | 1 | `plans/openlife_single_system_deletion_manifest.md` | `34` | `src-tauri/src/main_chat_stage4_memory_knowledge.rs` | `src-tauri/src/main_chat_stage4_memory_knowledge.rs` | Retired source path named as deletion-manifest object. |
| `historical_should_not_be_active` | 1 | `plans/openlife_single_system_deletion_manifest.md` | `35` | `src-tauri/src/main_chat_stage5_release_debug.rs` | `src-tauri/src/main_chat_stage5_release_debug.rs` | Retired source path named as deletion-manifest object. |
| `historical_should_not_be_active` | 1 | `plans/openlife_single_system_deletion_manifest.md` | `36` | `src-tauri/src/main_chat_step6_product_acceptance.rs` | `src-tauri/src/main_chat_step6_product_acceptance.rs` | Retired source path named as deletion-manifest object. |
| `historical_should_not_be_active` | 1 | `plans/openlife_single_system_deletion_manifest.md` | `42` | `src-tauri/src/main_chat_task_continuity_eval.rs` | `src-tauri/src/main_chat_task_continuity_eval.rs` | Retired source path named as deletion-manifest object. |
| `historical_should_not_be_active` | 1 | `plans/openlife_single_system_development_preparation.md` | `152` | `frontend/src/pages/settings/MultiStrategyPreviewSection.tsx` | `frontend/src/pages/settings/MultiStrategyPreviewSection.tsx` | Stale pre-Phase7 observed object; reword before using as current source map. |
| `historical_should_not_be_active` | 3 | `plans/openlife_single_system_development_preparation.md` | `61,73,380` | `openlife-core/src/agent/strategy.rs` | `openlife-core/src/agent/strategy.rs` | Stale pre-Phase7 observed object; reword before using as current source map. |
| `historical_should_not_be_active` | 2 | `plans/openlife_single_system_development_preparation.md` | `75,382` | `openlife-core/src/layer_router.rs` | `openlife-core/src/layer_router.rs` | Stale pre-Phase7 observed object; reword before using as current source map. |
| `historical_should_not_be_active` | 2 | `plans/openlife_single_system_development_preparation.md` | `74,381` | `openlife-core/src/router.rs` | `openlife-core/src/router.rs` | Stale pre-Phase7 observed object; reword before using as current source map. |
| `historical_should_not_be_active` | 1 | `plans/openlife_single_system_development_preparation.md` | `276` | `src-tauri/src/legacy_write_convergence.rs` | `src-tauri/src/legacy_write_convergence.rs` | Stale pre-Phase7 observed object; reword before using as current source map. |
| `historical_should_not_be_active` | 2 | `plans/openlife_single_system_development_preparation.md` | `58,328` | `src-tauri/src/main_chat_legacy_agent_loop.rs` | `src-tauri/src/main_chat_legacy_agent_loop.rs` | Stale pre-Phase7 observed object; reword before using as current source map. |
| `historical_should_not_be_active` | 3 | `plans/openlife_single_system_development_preparation.md` | `59,76,385` | `src-tauri/src/main_chat_route_preview.rs` | `src-tauri/src/main_chat_route_preview.rs` | Stale pre-Phase7 observed object; reword before using as current source map. |
| `historical_should_not_be_active` | 2 | `plans/openlife_single_system_development_preparation.md` | `56,326` | `src-tauri/src/main_chat_strategy.rs` | `src-tauri/src/main_chat_strategy.rs` | Stale pre-Phase7 observed object; reword before using as current source map. |
| `historical_should_not_be_active` | 4 | `plans/openlife_single_system_development_preparation.md` | `57,133,327,558` | `src-tauri/src/main_chat_tool_loop.rs` | `src-tauri/src/main_chat_tool_loop.rs` | Stale pre-Phase7 observed object; reword before using as current source map. |

## Next Implementation Scope

Stage3D reaches the standard for a bounded active path repair implementation
only. That future slice should be non-ADR, docs-only, and should not attempt to
make the active missing baseline zero in one pass.

Minimal first repair file list:

- `docs/DEV_HANDOVER.md`
- `docs/github_repository_governance.md`
- `CONTRIBUTING.md`
- `plans/openlife_repository_active_claim_audit.md`
- `plans/openlife_repository_stage2a_scope_decision.md`
- `plans/openlife_repository_stage2c_phase_c_readiness_decision.md`
- `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md`

The first repair slice should:

- retarget clear `.github/*` and repo-root relative paths;
- reword stale/example path text that should not become a file;
- keep future namespaces blocked rather than creating empty directories;
- leave ADR-related paths untouched;
- regenerate `plans/openlife_repository_document_link_baseline.json` only in
  the repair slice after edits are reviewed.

Full active-path closure would need a broader approved file list:

- `AGENTS.md`
- `docs/repository_document_governance.md`
- `docs/decisions/0002-proposal-unified.md`
- `docs/decisions/0003-agent-run-tracking.md`
- `plans/openlife_single_system_deletion_manifest.md`
- `plans/openlife_single_system_development_preparation.md`
- `plans/openlife_repository_document_link_baseline.json`

Those files are not part of Stage3D edits. Some of them are active authority or
historical decision records, so the next implementer should separate ordinary
retargeting from historical-source wording and ADR consolidation.

## Stage3D Readiness Verdict

| Question | Verdict |
| --- | --- |
| Is active missing path triage complete? | Yes: 171 active records were extracted and grouped into 114 source/raw/resolved groups. |
| Can a bounded active path repair implementation start next? | Yes, for deterministic non-ADR retarget/reword work only. |
| Can ADR consolidation start directly from this artifact? | No. ADR-related missing records remain blockers and require a dedicated same-slice ADR implementation plan. |
| Did Stage3D modify source code or baseline JSON? | No. |
| Did Stage3D repair links? | No. |

The runtime-module guard remains an inherited blocker unless a separate current
test run proves otherwise. Stage3D does not treat that blocker as resolved.
