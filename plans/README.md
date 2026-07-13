# OpenLife Plans Document Governance

> Last updated: 2026-07-07
> Status: active authority map for the single-system Phase7 cleanup pass

This file is the active plan index for Agents. Its purpose is to keep old
planning documents from steering new work.

## Current Precedence

1. `AGENTS.md`
2. `plans/README.md`
3. `plans/openlife_single_system_deletion_manifest.md`
4. `plans/openlife_single_system_development_preparation.md`
5. `plans/openlife_backend_remediation_v4.md` for the currently approved backend
   remediation work package, always subordinate to items 1-4.

All older Goal, Stage, Beta, dogfood, eval, adapter, and route-transition
documents are historical reference only unless a future user task explicitly
names one as input and keeps it subordinate to this single-system contract.

## Phase7 Contract

Phase7 is a deletion and product-trial pass. It is not another compatibility
adapter and not a documentation-only supersession.

The shipped product must have:

- no old runtime module in the product crate graph;
- no old command in the shipped Tauri handler;
- no old wrapper in `frontend/src/tauri.ts`;
- no product page/component importing dev-only bridges;
- no product page/component consuming old fallback/status fields;
- no active README or active plan index authorizing old routes;
- a Computer Use trial report that is green, or red with explicit fail-closed
  blockers and no Phase7 completion claim.

## Active Documents

- `plans/openlife_single_system_deletion_manifest.md`
- `plans/openlife_single_system_development_preparation.md`
- `plans/openlife_single_system_phase1_inventory.json`
- `plans/openlife_backend_remediation_v4.md`
- `plans/openlife_backend_remediation_v4_inventory.json`
- `plans/openlife_backend_remediation_v4_traceability.json`
- `plans/openlife_backend_remediation_v4_scenarios.json`
- `plans/openlife_backend_remediation_v4_scenario_waivers.json`
- `plans/openlife_backend_remediation_v4_phase0_evidence.md`
- `plans/openlife_backend_d068_authenticated_payload_red_matrix.md`
- `plans/openlife_backend_d057_pre_manifest_epoch_waiver.md`
- `plans/adr/0014-explicit-user-memory-write-lane.md`
- `plans/adr/0015-transient-state-command-lane.md`

These documents define the current cleanup scope, deletion manifest, and
acceptance gates. The backend remediation work package implements the Phase7
single-system contract; it does not supersede or create a second authority.
The deletion manifest is a contract artifact: objects marked
`not-done` are blockers, and objects marked `done` must be absent from product
build, product UI, product bridge, and active docs.

## Historical Documents

Historical documents may explain prior decisions and evidence. They do not set
current task order and do not authorize product-visible old routes. If a
historical document conflicts with the active documents above, the active
single-system contract wins.

Known historical references:

- `plans/main_chat_agent_kernel_rescue_goal_8_cleanup_final_gate.md`
- `plans/main_chat_stage2_preparation_index.md`
- `plans/main_chat_agent_stage2_internal_trial_goal_spec.md`
- `plans/main_chat_agent_migration_v1_goal_spec.md`

## Guard Rules

- no new system without deletion;
- no product-visible legacy/beta/stage/migration/cutover route;
- no direct durable write outside gateway authority;
- no frontend independent product readiness source;
- no readiness/completion claim without the required gates and trial evidence.
