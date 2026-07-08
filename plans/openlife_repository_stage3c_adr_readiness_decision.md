# OpenLife Repository Stage3C ADR Readiness Decision

> Date: 2026-07-07
> Status: Stage3C decision artifact only; no ADR consolidation implementation
> Scope: decide ADR consolidation readiness from the current checkout
> Authority: subordinate to `AGENTS.md`, `plans/README.md`,
> `plans/openlife_single_system_deletion_manifest.md`, and
> `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md`

Stage3C does not move ADR files, create an ADR index, create product docs, edit
runtime source, or promote repository cleanup into current runtime authority.

## Decision

Current decision: **not ready for ADR consolidation implementation**.

Stage3C is complete only as an ADR readiness decision. It does not authorize
moving ADR 0013 from `plans/adr/`, creating `docs/decisions/README.md`, or
rewriting active authority links.

The current canonical ADR 0013 location remains:

```text
plans/adr/0013-lifemodel-hs-source-of-truth-governance.md
```

## Verified Inputs

The following required paths were checked in the current checkout and exist:

| Path | Stage3C interpretation |
| --- | --- |
| `docs/decisions/0001-lifemodel-patch.md` | Existing historical ADR. |
| `docs/decisions/0002-proposal-unified.md` | Existing historical/stable ADR. |
| `docs/decisions/0003-agent-run-tracking.md` | Existing historical/stable ADR. |
| `plans/adr/0013-lifemodel-hs-source-of-truth-governance.md` | Current canonical accepted ADR 0013 file. |
| `.github/CODEOWNERS` | Governance owner file; currently owns ADR 0013 at `plans/adr/`. |
| `.github/ISSUE_TEMPLATE/04_adr_proposal.yml` | ADR proposal template exists; no Stage3C edit. |
| `.github/ISSUE_TEMPLATE/config.yml` | GitHub template config links ADR 0013 at `plans/adr/`. |
| `plans/openlife_repository_document_inventory.json` | Stage3B inventory baseline exists. |
| `plans/openlife_repository_document_link_baseline.json` | Stage3B link baseline exists. |

Stage3B baseline facts used for this decision:

| Evidence | Current value |
| --- | --- |
| Inventory schema | `openlife_repository_document_inventory.stage3b.v1` |
| Inventory document count | 198 Markdown/HTML documents |
| Inventory ADR decision | `ready_for_adr_consolidation = false` |
| Inventory authority decision | `ready_for_authority_promotion = false` |
| Link baseline schema | `openlife_repository_document_link_baseline.stage3b.v1` |
| Link baseline missing local path records | 365 |
| Link baseline active-doc missing records | 171 |
| Link baseline historical/private missing records | 194 |
| `docs/decisions/README.md` source check | `false` |
| ADR move in Stage3B | `not_performed` |
| Authority promotion in Stage3B | `not_performed` |

## Blockers

ADR consolidation implementation is blocked by the following current facts:

1. `docs/decisions/README.md` does not exist, and Stage3C is explicitly not
   allowed to create it.
2. Stage3B records `ready_for_adr_consolidation = false`.
3. Stage3B records 171 active-doc missing local path records. Stage3C does not
   resolve or formally scope out those records.
4. ADR-related active missing records remain in the link baseline:
   `docs/decisions/README.md`, `docs/decisions/0013`,
   `docs/decisions/0013-lifemodel-hs-source-of-truth-governance.md`, and the
   shorthand `plans/adr/0013`.
5. Active and governance references still point to the current
   `plans/adr/0013-lifemodel-hs-source-of-truth-governance.md` location.
6. Moving ADR 0013 without a same-slice update would break or stale
   `.github/CODEOWNERS`, `.github/ISSUE_TEMPLATE/config.yml`, active
   architecture docs, planning references, and Stage3B baseline artifacts.
7. Stage3B recorded `main_chat_runtime_module` as an inherited blocker. Stage3C
   does not repair runtime-module ownership and must not claim it is green
   unless the current acceptance command proves it.

## ADR Reference Impact Graph

This graph classifies every current hit family for:

```text
plans/adr/0013
docs/decisions/0013
docs/decisions/README
ADR 0013
lifemodel-hs-source-of-truth-governance
```

Classification meanings:

- `active`: current or scoped reference that can affect present work.
- `historical`: retained background, audit trail, disabled template, or
  superseded material.
- `template`: GitHub issue template/config surface.
- `governance`: ownership, inventory, baseline, preparation, or decision
  surface.

| Classification | Hit family | Current impact |
| --- | --- | --- |
| active | `AGENTS.md:59`, `AGENTS.md:89` | Active AI entry and LifeModel-HS hard-boundary references. A move must update these in the same slice. |
| active | `docs/architecture/life-model.md:17,29,62` | Current explanatory architecture references ADR 0013 by `plans/adr/` path. A move must update these. |
| active | `docs/architecture/governance.md:15,27` | Current governance explainer references ADR 0013 by `plans/adr/` path. A move must update these. |
| active | `plans/builder_life_model_design.md:5` | Scoped Builder reference uses ADR 0013 as current LifeModel-HS governance. A move must update or intentionally leave a canonical pointer. |
| active | `plans/lifemodel_hs_mvp_task_specs.md:14,28,48,100,115,125,626` | Current scoped task spec and validation text reference ADR 0013. A move must update commands and links. |
| active | `plans/openlife_lifemodel_governed_agent_runtime.md:145,748` | Future-governed runtime plan references the current ADR path. A move must update it. |
| active | `plans/main_chat_agent_migration_v1_goal_spec.md:186,746` | Active remediation/audit-trail reference, subordinate to Phase7 current authority. A move must update or preserve pointer semantics. |
| governance | `plans/adr/0013-lifemodel-hs-source-of-truth-governance.md:1` | Current canonical accepted ADR 0013 record. It remains here unless a later implementation slice moves it. |
| governance | `.github/CODEOWNERS:10` | Ownership is path-specific to `plans/adr/0013...`; must change in the same slice as any move. |
| governance | `docs/repository_document_governance.md:149-150` | Governance explicitly defers ADR consolidation and `docs/decisions/README.md`. |
| governance | `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md` ADR references | Preparation artifact defines Phase D ADR consolidation prerequisites and says ADR movement is not ready. |
| governance | `plans/openlife_repository_document_inventory.json` ADR references | Stage3B inventory records no ADR moves, existing ADR 0013 path, and `ready_for_adr_consolidation = false`. Regenerate if ADR location changes. |
| governance | `plans/openlife_repository_document_link_baseline.json` ADR references | Stage3B link baseline records `docs/decisions/README.md = false` and ADR-related active missing records. Regenerate if ADR location changes. |
| governance | `plans/openlife_repository_stage3c_adr_readiness_decision.md` | This Stage3C decision classifies ADR impact and keeps ADR 0013 in `plans/adr/`. |
| template | `.github/ISSUE_TEMPLATE/config.yml:9-10` | Contact link names ADR 0013 and links the current `plans/adr/` path. A move must update it. |
| template | `.github/ISSUE_TEMPLATE/04_adr_proposal.yml` | Required path exists. It has no current exact ADR 0013/path hit, but must be checked in the same slice if ADR governance wording changes. |
| template | `.github/ISSUE_TEMPLATE/90_lifemodel_hs_epic.yml.disabled:19,31,78` | Disabled historical issue template; update only if retained as a meaningful template, otherwise classify as historical. |
| template | `.github/ISSUE_TEMPLATE/91_lifemodel_hs_task.yml.disabled:62,192` | Disabled historical issue template; update only if retained as a meaningful template, otherwise classify as historical. |
| historical | `docs/decisions/0001-lifemodel-patch.md:4,13` | Historical ADR 0001 says parts are superseded by ADR 0013. Do not rewrite as current behavior without review. |
| historical | `docs/decisions/0002-proposal-unified.md:5` | Historical/stable ADR 0002 points to ADR 0013 as a newer governance constraint. |
| historical | `plans/lifemodel_hs_legacy_write_path_audit.md:8,333` | Historical audit references ADR 0013. Move only with preservation of audit meaning. |
| historical | `plans/skill_runtime_goal_spec.md:97,451` | Completed W150-W158 audit-trail references. |
| historical | `plans/plan_execute_product_vertical_goal_spec.md:26,672` | Completed W98-W105 audit-trail references. |
| historical | `plans/legacy_direct_write_convergence_goal_spec.md:25` | Completed W90-W97 audit-trail reference. |
| historical | `plans/lifemodel_hs_architecture_plan.md:7,1171` and `plans/lifemodel_hs_architecture_plan.zh.html:327,1460` | Design baseline references now governed by ADR 0013. Preserve historical/design context. |
| historical | `plans/openlife_repository_stage2c_phase_c_readiness_decision.md:121,139` | Earlier repository readiness decision reference. |
| historical | `plans/lifemodel_governed_backend_completion_goal_spec.md:581` | Completed backend completion audit-trail reference. |
| historical | `plans/runtime_strategy_maturity_goal_spec.md:27,627` | Completed W106-W113 audit-trail references. |

No `README.md` or `CONTRIBUTING.md` hit was found in the pre-edit impact scan.

## Required Next-Slice Surface If Moving ADR 0013

If a later implementation stage chooses to move ADR 0013 to
`docs/decisions/0013-lifemodel-hs-source-of-truth-governance.md`, it must update
all of the following in the same reviewed slice:

1. Move `plans/adr/0013-lifemodel-hs-source-of-truth-governance.md` to the new
   canonical file without duplicating ADR number 0013.
2. Create or update `docs/decisions/README.md` as the canonical decision index.
3. Update `.github/CODEOWNERS` for the new ADR canonical path.
4. Update `.github/ISSUE_TEMPLATE/config.yml` contact link.
5. Re-check `.github/ISSUE_TEMPLATE/04_adr_proposal.yml` and update any
   governance wording if needed.
6. Decide whether disabled issue templates stay historical or should receive
   path updates; record that decision explicitly.
7. Update active references in `AGENTS.md`, `docs/architecture/life-model.md`,
   `docs/architecture/governance.md`, `plans/builder_life_model_design.md`,
   `plans/lifemodel_hs_mvp_task_specs.md`,
   `plans/openlife_lifemodel_governed_agent_runtime.md`, and any current
   Phase7 authority map that names ADR 0013.
8. Update historical references only when the link target would otherwise
   break; preserve their historical status and do not turn historical claims
   into current authority.
9. Regenerate or update
   `plans/openlife_repository_document_inventory.json` and
   `plans/openlife_repository_document_link_baseline.json`.
10. Re-run the ADR reference impact scan and prove that old/new path references
    are either intentionally present, historical, or resolved.
11. Run the Stage3C or successor validation commands and record results.

If that complete surface is not in scope, ADR 0013 must remain in
`plans/adr/`, and any future `docs/decisions/README.md` must point to the
existing file rather than duplicating it.

## Acceptance Boundary

Stage3C may only conclude:

```text
ADR readiness decision complete.
```

It must not conclude any of the following:

- Phase7 complete.
- Main Chat Agent Execution v1 complete.
- Live-provider evidence complete.
- `main_chat_runtime_module` green unless proven by the current command output.
- ADR consolidation implementation ready.
- ADR 0013 moved.
- `docs/decisions/README.md` created.
- Active missing path records resolved.

CodeRabbit external review is not part of Stage3C local validation because the
local CodeRabbit auth status is not logged in.

## Stage3C Validation Snapshot

Validation run on 2026-07-07:

| Check | Result |
| --- | --- |
| `git diff --check` | Passed. |
| `cargo fmt --check` | Passed. |
| `python3 -m json.tool plans/openlife_repository_document_inventory.json >/tmp/openlife_repository_document_inventory_stage3c_pretty.json` | Passed. |
| `python3 -m json.tool plans/openlife_repository_document_link_baseline.json >/tmp/openlife_repository_document_link_baseline_stage3c_pretty.json` | Passed. |
| ADR impact scan for `plans/adr/0013`, `docs/decisions/0013`, `docs/decisions/README`, `ADR 0013`, and `lifemodel-hs-source-of-truth-governance` | Completed with expected hits; the impact graph above classifies the hit families as active, historical, template, or governance. |
| `rg -n "run_main_chat_agent_execution_v1_final_acceptance_gate" src-tauri/src/lib.rs src-tauri/src/commands frontend/src/tauri.ts` | No matches; `rg` exited 1 as expected for the absence guard. |
| `cargo test -p openlife-tauri single_system -- --nocapture` | Passed, 17 tests. |
| `cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture` | Failed as inherited blocker: 24 passed, 2 failed. Failures remain `main_chat_final_gate_aggregation_is_not_hidden_in_test_module` and `main_chat_live_provider_completed_report_builder_is_not_hidden_in_test_module`. |

The failed runtime-module guard is evidence against ADR consolidation readiness,
not a Stage3C permission to repair runtime source or restore retired commands.
