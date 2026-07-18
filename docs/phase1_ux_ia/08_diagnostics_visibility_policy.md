# Diagnostics Visibility Policy

Status: Phase 1 visibility policy proposal.
Scope: Product/default/advanced/developer classification only.

## Classification Legend

- `VERIFIED_FACT`
- `DESIGN_DECISION`
- `DESIGN_ASSUMPTION`
- `CANDIDATE`
- `UNKNOWN`
- `PHASE_2_REQUIRED`

## Visibility Levels

```text
DEFAULT_PRODUCT
EXPANDABLE_DETAILS
ADVANCED_INSPECTOR
DEVELOPER_ONLY
REMOVE_OR_ARCHIVE
NEEDS_HUMAN_DECISION
```

`DESIGN_DECISION`: Hide by default does not mean delete. Evidence must remain reachable through the right layer.

## Classification Matrix

| Surface | V2 visibility | Reason | Risk if too visible | Risk if too hidden |
| --- | --- | --- | --- | --- |
| Safe Mode | DEFAULT_PRODUCT | Safety state affects user action and trust. | Low; can feel alarming if copy is poor. | User misses why writes/actions are blocked. |
| usage readiness | DEFAULT_PRODUCT | Setup/readiness affects first-use success. | May overclaim product readiness if not projection-backed. | User cannot recover setup issues. |
| current task status | DEFAULT_PRODUCT | User must know running/waiting/blocked/failed/completed. | Low if concise. | Pending/failure states look like success. |
| pending review count | DEFAULT_PRODUCT | Drives Review Center action. | Can become noisy if ungrouped. | User misses required confirmations. |
| tool permission summary | DEFAULT_PRODUCT | Explains why tool work waits or is blocked. | Tool concepts may feel technical. | User cannot safely authorize/retry. |
| runtime disclosure | DEFAULT_PRODUCT / EXPANDABLE_DETAILS | Shows boundary, outcome, tools, proposals, blockers. | Internal labels leak. | User cannot tell what happened. |
| tool call details | EXPANDABLE_DETAILS | Useful for trust and debugging. | Arguments/status can overwhelm. | Failure reasons become opaque. |
| reasoning trace | ADVANCED_INSPECTOR | Debug/evidence value, high cognitive load. | Product feels like developer console. | Support/debug loses evidence. |
| run trace | ADVANCED_INSPECTOR | Useful for task/run evidence. | Technical "run" framing dominates. | User/support cannot inspect failures. |
| kernel events | ADVANCED_INSPECTOR | Low-level runtime events. | Leaks architecture. | Deep diagnosis harder. |
| durable events | ADVANCED_INSPECTOR | Audit/replay value. | Stream details overwhelm. | Task evidence incomplete. |
| raw transcript | ADVANCED_INSPECTOR | Full auditable record. | Privacy/cognitive load. | User cannot audit exact sequence. |
| provider health | ADVANCED_INSPECTOR | Trust/support for local/cloud route. | Provider internals dominate Settings. | External/local boundary unclear. |
| PolicyRouter | DEVELOPER_ONLY | Internal authority chain. | Makes product feel unfinished. | Engineering support loses route proof. |
| ModelRouter | ADVANCED_INSPECTOR / DEVELOPER_ONLY | Provider diagnostics may support trust. | Too technical by default. | Provider failures unclear. |
| MCP/A2A | DEVELOPER_ONLY / NEEDS_HUMAN_DECISION | External connection strategy not product-proven. | Overclaims readiness. | Advanced users cannot manage connections. |
| metrics | DEVELOPER_ONLY | Operational data. | Dashboard/developer console drift. | Engineering support loses signal. |
| calibration | NEEDS_HUMAN_DECISION | Could be trust feature or advanced tool. | Confuses learning/governance. | User may lack control over product learning. |
| versions | NEEDS_HUMAN_DECISION | Snapshot/rollback has trust value. | Maintenance UI may dominate. | Recovery path hidden. |
| tauriDev/test/historical surfaces | DEVELOPER_ONLY / REMOVE_OR_ARCHIVE | Not product authority. | Restores deleted old-route mental model. | Test evidence may be harder to find. |

## Phase 2 Visibility Stop Rules

1. `PHASE_2_REQUIRED`: Do not expose MCP/A2A, calibration, versions, or metrics in ordinary navigation until humans classify them as product, advanced, or developer-only.
2. `PHASE_2_REQUIRED`: Do not hide blocker, failed, waiting-permission, safe-mode, pending-review, or provider/privacy boundary states behind advanced-only surfaces.
3. `PHASE_2_REQUIRED`: Do not expose raw trace, kernel events, durable events, raw transcript, provider health, PolicyRouter, ModelRouter, or tauriDev/test surfaces as default product UI.
4. `PHASE_2_REQUIRED`: Do not remove evidence access when hiding diagnostics by default; preserve evidence refs and advanced inspection paths.

## ProductAction vs DebugAction

`DESIGN_DECISION`: Default product surfaces may expose `ProductAction`, such as continue, retry, cancel, review, inspect evidence, or open settings.

`DESIGN_DECISION`: Review Center exposes `ReviewAction`, such as approve, reject, later, modify, and view evidence.

`DESIGN_DECISION`: Advanced inspector exposes `DebugAction`, such as raw trace, export JSON, provider health, route evidence, or raw transcript. Debug actions must not appear as default product actions.

## Evidence Preservation Rule

`VERIFIED_FACT`: Existing evidence surfaces include reasoning trace, run trace, tool cards, kernel events, durable events, final delivery, proposal display, review decisions, and runtime disclosure. Source: `docs/openlife-phase0-audit/08_frontend_current_state_audit.md`, `docs/phase0_5/04_diagnostics_visibility_inventory.md`.

`DESIGN_DECISION`: V2 should preserve evidence refs even when default UI shows only a plain-language summary.

## Default Product Layer

Default product layer includes:

- safe mode and recovery guidance;
- usage readiness only when projection-backed;
- current task state;
- pending review count;
- tool permission summary when relevant;
- blocker/failure reason;
- final result with completed, pending, and blocked sections separated;
- privacy/provider boundary summary.

`PHASE_2_REQUIRED`: Define backend-owned summary fields so pages do not recompute these states locally.

## Expandable Details

Expandable details include:

- concise tool action details;
- evidence summaries;
- before/after review summaries;
- task timeline details;
- provider/privacy explanation;
- safe-path/danger-action summary.

`DESIGN_DECISION`: These details are for motivated users, not raw developer output.

## Advanced Inspector

Advanced inspector includes:

- reasoning trace;
- run trace;
- kernel events;
- durable event stream/replay state;
- full execution transcript;
- sanitized tool arguments and output hashes;
- runtime route evidence;
- provider health rows;
- raw audit refs;
- export/debug actions.

`DESIGN_DECISION`: Advanced inspector must be reachable from task/evidence contexts, but not prominent in default navigation.

## Developer-only

Developer-only includes:

- `tauriDev.ts` surfaces;
- historical Stage/Beta/migration/cutover artifacts;
- internal debug toggles;
- PolicyRouter authority internals;
- metrics internals;
- test/archive reports;
- raw command wrappers.

`DESIGN_DECISION`: These are not product surfaces and must not reauthorize old routes.

## Remove Or Archive

`DESIGN_DECISION`: Old route labels and wrappers classified as Phase7 done/test-only/historical must not return to product IA. Keep them only where the active deletion manifest allows evidence, tests, or history.

## Human Decisions Needed

1. Whether `MCP/A2A` is developer-only or an advanced product capability.
2. Whether `校准` is user-facing governance or developer-only maintenance.
3. Whether `版本` is a product recovery surface or advanced maintenance.
4. Which support mode exposes ModelRouter/Provider health.
5. What default evidence must always appear after external/tool actions.
