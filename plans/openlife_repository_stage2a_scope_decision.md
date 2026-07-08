# OpenLife Repository Stage 2A Scope Decision

> Decision date: 2026-07-07
> Status: Stage 2A docs-only boundary record. This is not active authority
> promotion and not a Stage 2B implementation approval.

## Decision

Stage 2A converts the Stage 1 active-authority stale claims and active-doc
broken links into an executable repair scope. It does not edit `AGENTS.md`,
`docs/ARCHITECTURE.md`, `docs/DEV_HANDOVER.md`, `CONTRIBUTING.md`,
`OpenLife_Final_PRD.md`, `.github/PULL_REQUEST_TEMPLATE.md`, Rust/Tauri/React
source, `README.md`, or `plans/README.md`.

The next executable cleanup slice is Stage 2B. Stage 2B may repair active docs
and publication links only inside the file lists below. It must keep
`main_chat_runtime_module` as an inherited blocker unless that guard is fixed or
formally scoped out by a reviewed Phase7 decision.

## Stage 2B Editable Files

Stage 2B may edit these files for the named reasons only:

| File | Allowed Stage 2B edits |
| --- | --- |
| `AGENTS.md` | Reconcile K8/Goal 8 wording with `plans/README.md`; remove or historical-label stale final-gate/test-owner and old module claims; keep incomplete live-provider evidence as a blocker, not completion. |
| `docs/ARCHITECTURE.md` | Replace old Chat flow and missing module list from current source-map; retarget/remove obsolete detailed-architecture/API-doc links. |
| `docs/DEV_HANDOVER.md` | Label old module and command-count guidance as historical snapshot content, or remove stale counts. |
| `CONTRIBUTING.md` | Retarget broken resource links; do not convert branch-name examples into local-doc links. |
| `OpenLife_Final_PRD.md` | Retarget absolute local desktop links to repo-relative targets or label the section historical-only. |
| `.github/PULL_REQUEST_TEMPLATE.md` | Included in PR/publication cleanup scope; edit only if adding an explicit local-link/source-authority evidence checkbox. No broken link retarget is required. |
| `plans/openlife_repository_active_claim_audit.md` | Update only as the Stage 2A/2B evidence register. |
| `plans/openlife_repository_stage2a_scope_decision.md` | Update only if Stage 2B scope changes are explicitly approved. |
| `plans/openlife_repository_knowledge_architecture_cleanup_preparation.md` | Append concise cleanup records only; do not rewrite the preparation architecture. |

## Stage 2B Non-Editable Files

Stage 2B must not edit these surfaces without a separate user instruction:

- Rust/Tauri/React source, including `openlife-core/**`, `src-tauri/**`, and `frontend/src/**`.
- `README.md` and `plans/README.md`.
- ADR locations, including `plans/adr/**` and `docs/decisions/**`.
- Historical plan documents under `plans/**`, except the three Stage 2A record files named above.
- Stage 1 baseline JSON files, unless the user explicitly asks to regenerate a new baseline artifact.
- `.github/workflows/**`, unless a separate CI/link-checker task is approved.
- Any file move, rename, archive move, or deletion.
- Any recovery of `run_main_chat_agent_execution_v1_final_acceptance_gate`.

## Stage 2B Required Actions

| Area | Required action |
| --- | --- |
| AGENTS K8/current authority conflict | `source_map_required` before rewrite. Stage 2B must choose whether Goal 8 wording remains only as historical context or is rephrased under Phase7 single-system authority. |
| AGENTS final-gate/test-owner claims | `source_map_required`. Do not write that the retired command or missing test-owner file is current. |
| AGENTS old router/module claims | `mark_historical`. Remove from active module tables or explicitly label as historical progress context. |
| AGENTS live-provider boundary | `defer_with_reason`. Preserve incomplete live-provider evidence as a blocker; do not create completion language. |
| `docs/ARCHITECTURE.md` old Chat flow | `fix_now`. Replace `IntentRouter` / `LayerRouter` flow with current source-mapped Main Chat path, or mark the doc not-current before publication. |
| `docs/ARCHITECTURE.md` missing architecture/API links | `retarget_link`. Retarget or remove the obsolete detailed-architecture/API-doc links. |
| `docs/ARCHITECTURE.md` historical Hermes listing | `fix_now`. Replace from current file inventory or mark the table historical. |
| `docs/DEV_HANDOVER.md` old module/count guidance | `mark_historical`. It may stay only as a handover snapshot and cannot be onboarding authority for current command counts. |
| `CONTRIBUTING.md` broken links | `retarget_link` or `source_map_required` where no canonical target exists. |
| `OpenLife_Final_PRD.md` absolute links | `retarget_link` or mark the linked section historical-only. |
| `.github/PULL_REQUEST_TEMPLATE.md` | Include in publication cleanup scope; no Stage 2B edit required unless adding link-baseline evidence wording. |

## Publication Scope

The PR/publication cleanup scope is included and bounded:

- Included: `.github/PULL_REQUEST_TEMPLATE.md` as the manual PR governance
  checklist for public Markdown/HTML and authority-sync checks.
- Included: `CONTRIBUTING.md` and `OpenLife_Final_PRD.md` because they contain
  active broken public links.
- Excluded from Stage 2B: `.github/workflows/**`; there is no dedicated Markdown
  link checker today, and adding one is a separate CI task.

## Claims Still Blocked From Current Truth

Stage 2B must not write these claims as current truth:

- Phase7 complete.
- Main Chat Agent Execution v1 complete.
- External live-provider evidence complete.
- The retired final acceptance command is shipped or should be restored.
- The deleted old final-acceptance test-owner file is the current test owner.
- `main_chat_runtime_module` is green because Stage 2A exists.
- Old `IntentRouter` / `LayerRouter` / `hermes.rs` modules are current runtime authority.
- The obsolete detailed-architecture/API-doc targets exist.
- `docs/DEV_HANDOVER.md` command counts are current.
- `.github/PULL_REQUEST_TEMPLATE.md` or CI proves local links automatically.

## Acceptance Boundary

Stage 2A is accepted only if:

- every Stage 1 stale-claim blocker has one of the approved actions;
- all 14 active broken Markdown links have a Stage 2A action;
- `.github/PULL_REQUEST_TEMPLATE.md` has been scanned and included/excluded by
  decision, not ignored;
- Stage 2B editable and non-editable file lists are explicit;
- validation commands pass or report the inherited blocker honestly.

## Stage 2C Successor Note

Stage 2C is recorded in
`plans/openlife_repository_stage2c_phase_c_readiness_decision.md`. It is the
successor decision for Phase C / Stage3 doc-build readiness and does not change
the Stage 2A or Stage 2B editable file scope.
