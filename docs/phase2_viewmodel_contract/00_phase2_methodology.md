# Phase 2 Methodology

Status: Phase 2 contract documentation.
Scope: ViewModel / backend ReadModel contract only. No Frontend V2 implementation.

## Documents Read

`VERIFIED_FACT`: The active repository authority stack was read before writing this package:

- `AGENTS.md`
- `plans/README.md`
- `plans/openlife_single_system_deletion_manifest.md`
- `plans/openlife_single_system_development_preparation.md`
- `OpenLife_Phase2_ViewModel_ReadModel_Codex_Goal_v1.0.md`

`VERIFIED_FACT`: The required Phase 1, Phase 0.5, and Phase 0 evidence sources were present and read for contract evidence:

- `docs/phase1_ux_ia/01_v2_decision_record.md`
- `docs/phase1_ux_ia/03_v2_information_architecture.md`
- `docs/phase1_ux_ia/04_agent_workspace_model.md`
- `docs/phase1_ux_ia/05_review_center_model.md`
- `docs/phase1_ux_ia/06_lifemodel_memory_model.md`
- `docs/phase1_ux_ia/08_diagnostics_visibility_policy.md`
- `docs/phase1_ux_ia/09_view_model_contract_proposal.md`
- `docs/phase1_ux_ia/10_phase1_summary.md`
- `docs/phase0_5/03_chat_companion_workspace_mapping.md`
- `docs/phase0_5/04_diagnostics_visibility_inventory.md`
- `docs/phase0_5/06_view_model_gap_inventory.md`
- `docs/phase0_5/07_phase0_5_summary.md`
- `docs/openlife-phase0-audit/02_backend_capability_map.md`
- `docs/openlife-phase0-audit/03_agent_system_analysis.md`
- `docs/openlife-phase0-audit/04_domain_model_analysis.md`
- `docs/openlife-phase0-audit/05_backend_frontend_contract.md`
- `docs/openlife-phase0-audit/06_security_governance_audit.md`
- `docs/openlife-phase0-audit/13_audit_summary.md`

No required input document was missing.

## Source Areas Inspected

`EXISTING_CODE`: The following source areas were inspected to classify existing contracts versus proposed ViewModels:

- `src-tauri/src/life_state_projection.rs`
- `frontend/src/tauri.ts`
- `frontend/src/utils/lifeStateProjection.ts`
- `frontend/src/utils/runtimeDisclosure.ts`
- `frontend/src/utils/reviewDecision.ts`
- `frontend/src/utils/proposalDisplay.ts`
- `frontend/src/utils/runDisplaySummary.ts`
- `frontend/src/utils/lifeModelTrust.ts`
- `frontend/src/pages/TodayPage.tsx`
- `frontend/src/pages/ChatPage.tsx`
- `frontend/src/pages/MailboxPage.tsx`
- `frontend/src/pages/RunsPage.tsx`
- `frontend/src/pages/LifeModelPage.tsx`
- `frontend/src/pages/MemorySearch.tsx`
- `frontend/src/pages/SettingsPage.tsx`

## Commands Run

`VERIFIED_FACT`: Commands were read-only except for creating these Phase 2 documentation files.

```sh
git status --short
sed -n ... AGENTS.md
sed -n ... OpenLife_Phase2_ViewModel_ReadModel_Codex_Goal_v1.0.md
sed -n ... plans/README.md
sed -n ... plans/openlife_single_system_deletion_manifest.md
sed -n ... plans/openlife_single_system_development_preparation.md
rg --files docs/phase1_ux_ia docs/phase0_5 docs/openlife-phase0-audit
rg -n "ViewModel|read model|LifeStateProjection|ReviewItem|Memory|LifeModel|Workspace|Settings|Today|Tasks" docs/phase1_ux_ia docs/phase0_5 docs/openlife-phase0-audit
rg -n "getLifeStateProjection|listMainChat|listAgentRuns|listProposals|getLifeModel|searchMemory|useState|useEffect" frontend/src/pages
sed -n ... src-tauri/src/life_state_projection.rs
sed -n ... frontend/src/tauri.ts
sed -n ... frontend/src/utils/*.ts
```

## Evidence Standard

`DESIGN_DECISION`: This package uses the Phase 2 evidence taxonomy for major claims:

| Classification | Meaning |
| --- | --- |
| `VERIFIED_FACT` | Verified by Phase 0 / 0.5 / 1 docs or active Phase7 authority. |
| `EXISTING_CODE` | Verified by direct source inspection in this pass. |
| `DESIGN_DECISION` | Accepted Phase 1 or Phase 2 contract direction. |
| `DESIGN_ASSUMPTION` | Plausible contract assumption requiring validation. |
| `CANDIDATE` | Preserved capability whose final implementation shape is not approved. |
| `UNKNOWN` | Not verified by available evidence. |
| `PHASE_2_REQUIRED` | Required before Frontend V2 implementation or Phase 3 slice work. |

Owner status values:

| Owner status | Meaning |
| --- | --- |
| `EXISTING` | Current backend or bridge contract exists. |
| `PARTIAL` | Existing primitives support part of the contract but do not own the full ViewModel. |
| `PROPOSED` | Contract is designed here but not implemented. |
| `UNKNOWN` | Ownership or source of truth is unresolved. |
| `PHASE_2_REQUIRED` | Must be implemented, verified, or explicitly approved before UI implementation. |

## Known Limits

`VERIFIED_FACT`: This pass did not run backend, frontend, browser, desktop, live-provider, Web AgentLoop, or MCP AgentLoop gates because the task is documentation-only.

`VERIFIED_FACT`: Phase 0.5 reported frontend install, typecheck, unit tests, and format check as passing, but browser smoke was partial due the `127.0.0.1:5173` dev-server readiness issue.

`UNKNOWN`: Real desktop/Tauri product trial remains red or unproven by the evidence read for this package.

`UNKNOWN`: External live-provider-backed generation, Web AgentLoop, MCP AgentLoop, and provider/live proposal-permission evidence remain incomplete by active authority.

## Production-Code Modification Statement

`VERIFIED_FACT`: Phase 2 only generated documentation under `docs/phase2_viewmodel_contract/`.

No production source code was intentionally modified. Frontend V2 implementation was not started.
