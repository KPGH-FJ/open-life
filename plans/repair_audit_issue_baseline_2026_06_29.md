# OpenLife Repair Audit Issue Baseline

Date: 2026-06-29

Status: tracked planning baseline. This file preserves the raw issue coverage used by the repair roadmap so development does not depend only on git-ignored local audit artifacts.

## Source Coverage

| Source | Raw ids | Count |
|---|---:|---:|
| 2026-06-28 initial product audit | `OL-001` to `OL-011` | 11 |
| 2026-06-28 v4 high-quality rerun | `V4-001` to `V4-016` | 16 |
| 2026-06-29 v5 real-data LifeModel audit | `V5-001` to `V5-024` | 24 |
| 2026-06-29 v6 cloud/provider contrast | `V6-001` to `V6-010` | 10 |
| Total |  | 61 |

## Severity Baseline

| Severity | Count | Planning meaning |
|---|---:|---|
| P0 | 3 | Blocks trust or safety until explicitly fixed and replayed. |
| P1 | 22 | Breaks a core loop: runtime truth, LifeModel, task output, recovery, or Review. |
| P2 | 32 | Significant product, governance, evidence, IA, or UX degradation. |
| P3 | 4 | Polish/accessibility/copy issues that still affect product quality. |

## Primary Repair Categories

Each raw issue appears in one primary category for implementation ownership. Some issues have secondary effects; do not duplicate them when counting backlog size.

| Category | Raw ids | Primary owner |
|---|---|---|
| Runtime Truth / Provider Route / Settings Readiness | `OL-001`, `OL-008`, `V4-001`, `V4-006`, `V4-007`, `V5-011`, `V6-001`, `V6-003`, `V6-004`, `V6-005`, `V6-009` | route facts, diagnostics, Settings, Companion/Runs disclosure |
| Runs / Trace / State Lifecycle / Recovery Evidence | `OL-007`, `V4-002`, `V4-011`, `V5-007`, `V5-008`, `V6-002`, `V6-007` | task sessions, AgentRun, transcript, Runs UI |
| LifeModel / Review / Memory / Evidence Closed Loop | `OL-002`, `OL-005`, `V4-003`, `V5-001`, `V5-002`, `V5-003`, `V5-004`, `V5-005`, `V5-013`, `V5-018`, `V5-020`, `V5-022` | proposal pipeline, patches/snapshots, LifeModel current view, Review |
| Agent Capability / PlanExecute / Blockers / MCP-Web Recovery | `OL-003`, `OL-006`, `V4-008`, `V4-013`, `V4-014`, `V4-015`, `V5-006`, `V5-010`, `V5-015`, `V5-021`, `V6-008` | Main Chat, PlanExecute, blocker recovery, capability readiness |
| Today / Daily Usefulness / Personalization Output Quality | `OL-004`, `V4-004`, `V5-009`, `V5-012`, `V5-023` | Today read model, output quality, preference application |
| Privacy / Safety / External Transmission / Data Governance | `OL-010`, `V4-005`, `V5-014`, `V6-006`, `V6-010` | Privacy page, provider transmission, danger-action preflight |
| Information Architecture / Navigation / Copy / Counts | `OL-009`, `V4-009`, `V4-012`, `V5-019`, `V5-024` | ProductShell, Review/Runs/Settings IA, badge/count taxonomy |
| Accessibility / Input Stability / Responsive Layout | `OL-011`, `V4-010`, `V4-016`, `V5-016`, `V5-017` | composer semantics, AX labels, IME, responsive layout |

## P0/P1 Development Gate

Before implementation claims any P0/P1 issue fixed, the evidence bundle must include:

1. The raw issue id.
2. The replay scenario from v4/v5/v6 or the original audit.
3. Screenshot or UI evidence when the issue is user-facing.
4. Runtime metadata, DB/trace evidence, or focused source/test proof for the backend claim.
5. A regression-map row saying `still_exists`, `fixed`, `improved`, `worse`, or `blocked`.

## Evidence Location Policy

- Local audit evidence remains under `frontend/test-results/product-audit-2026-06-29-openlife/`.
- That folder is intentionally git-ignored and may contain screenshots and local DB notes.
- This tracked baseline is the durable planning index.
- `make clean` preserves `frontend/test-results/product-audit-*`; `make clean-audit-results` is the explicit destructive cleanup target for local audit evidence.
