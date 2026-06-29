# OpenLife Repair Phase Readiness Index

Date: 2026-06-29

Status: conditionally approved development-preparation baseline for all repair phases. This file does not mark any issue fixed.

## Readiness Standard

Each phase may enter implementation only after these are explicit:

1. Problem scope: raw issue ids, user journey, evidence, source entrypoints.
2. Product contract: target behavior, user-facing state, failure state, recovery path.
3. Engineering contract: backend read/write model, frontend view model, ownership boundaries.
4. Anti-hallucination contract: what cannot be inferred from model text or UI copy.
5. Regression contract: v4/v5/v6 replay scenarios plus focused automated tests.
6. Non-goals: what must not be changed in the phase.

Preparation approval is not release approval. A phase can move from planning to implementation only when its entry gate below is satisfied, and it can move from implementation to "fixed" only with implementation diff, replay evidence, focused tests, and regression-map update.

## Phase Readiness Summary

| Phase | Prepared artifact | Current readiness | May implement? | Reason |
|---|---|---|---|---|
| 1 Trust Foundation | `plans/sprint1_trust_foundation_solution_rfc.md` | Ready for thin-slice implementation after evidence-retention and external-transmission boundary review | Yes | Route/readiness fields, UI semantics, tests, and non-goals are specified; definitive sent/not-sent transmission history is deferred to Phase 5. |
| 2 Runs / Trace / Recovery | `plans/sprint2_runs_trace_recovery_solution_rfc.md` | Ready for Slice 2A after Phase 1 route DTO is committed | Conditional | Lifecycle and RunEvidenceView are specified; timeout storage is frozen as `Failed + failure_kind=timeout`; implementation must consume Phase 1 route evidence. |
| 3 LifeModel Closed Loop | `plans/sprint3_lifemodel_closed_loop_solution_rfc.md` | Ready for design-review, then thin slice | Conditional | Change/current view models are specified; implementation should start with one low-risk preference path. |
| 4 Agent Task Productization | `plans/sprint4_agent_task_productization_solution_rfc.md` | Ready for RFC review, not first implementation | Later | Depends on Phase 1 route truth and Phase 2 Runs evidence. |
| 5 Privacy / Provider Governance | `plans/sprint5_privacy_provider_governance_solution_rfc.md` | Ready for schema review, not first implementation | Later | Depends on Phase 1 actual route proof and transmission truth integration. |
| 6 Daily UX / IA / AX | `plans/sprint6_daily_ux_ia_ax_solution_rfc.md` | Ready for targeted small fixes; major IA later | Later | Depends on stable facts, tasks, and LifeModel contracts. |

## Entry Gate By Phase

| Phase | Required before implementation | Must not start with |
|---|---|---|
| 1 Trust Foundation | One backend owner for `RuntimeRouteEvidence`, one frontend route-disclosure consumer, exact focused tests with non-zero-match plan, v6 C02/C03 replay setup. | Cloud-provider expansion, API-key changes, or definitive provider-transmission history. |
| 2 Runs / Trace / Recovery | Phase 1 route evidence DTO committed; timeout representation frozen as `Failed + failure_kind=timeout`; focused RunEvidenceView tests named before completion. | Independent route/fallback model, native `TimedOut` enum migration in this slice, or UI-only timeout label. |
| 3 LifeModel Closed Loop | Canonical path normalization rule and focused tests for `preferences.communication_style`; source/proposal/patch/current-view trace shape agreed. | Broad schema rewrite, bulk proposal migration, or accepting facts without current-view projection. |
| 4 Agent Task Productization | Route evidence and Runs evidence consumable by chat/task artifact surfaces; blocker taxonomy confirmed. | New planner UX that hides blockers or bypasses Runs trace. |
| 5 Privacy / Provider Governance | Storage choice for transmission log frozen; local/unknown/not_instrumented semantics tested; key redaction tests named. | Any claim that old runs were `not_sent` because no provider log exists. |
| 6 Daily UX / IA / AX | Stable typed data contracts for Today, Review counts, Runs, and route disclosure; focused AX/responsive tests named. | Large IA redesign that changes data semantics before trust chains stabilize. |

## Delivery-State Gate

These preparation artifacts are only durable if they are intentionally preserved:

1. `plans/repair_*` and `plans/sprint*_*.md` must be tracked, staged for review, or explicitly archived before implementation depends on them.
2. Modified support docs such as `Makefile` and `README.md` must be reviewed with the same change set if they are part of the preparation contract.
3. Git-ignored audit folders under `frontend/test-results/product-audit-*` remain local evidence, not the canonical planning source.
4. `make clean` may be used during development because it preserves product-audit folders; `make clean-audit-results` is destructive for audit evidence and must be intentional.

## Recommended Development Order

1. Implement Phase 1 slice A: `RuntimeRouteEvidence` backend DTO and route-truth prompt path.
2. Implement Phase 1 slice B: Settings Overview readiness mapping and runtime-authored route chip.
3. Implement Phase 2 slice A: durable terminal state for timeout/cancel/error.
4. Implement Phase 2 slice B: `RunEvidenceView` list/detail aggregation.
5. Replay v6 route and timeout cases.
6. Implement Phase 3 slice A: `preferences.communication_style` accepted-write visibility.
7. Replay v5 LifeModel write and Review cases.
8. Only then start Phase 4 planning artifact/blocker productization.

Do not run broad parallel development across phases. Phase 4-6 RFCs should influence design choices, but their implementation should wait until the earlier evidence chains are stable enough to prevent duplicated route, run, LifeModel, or privacy truth models.

## Cross-Phase Invariants

- No route truth can come from assistant prose.
- No readiness state can treat configured-only provider as validated/live-ready.
- No task can remain indefinitely `running` after timeout/cancel/error.
- No LifeModel fact can be considered user-visible until it appears in current view with source/change trace.
- No external provider invocation can be claimed unless runtime evidence says sent externally.
- No `not_sent` external-transmission claim can be made from missing logs alone; until Phase 5 lands, unknown/not_instrumented is acceptable and more honest.
- No realtime fact can be invented; it needs source evidence or an explicit blocker/offline assumption.
- No candidate test command can be treated as a gate unless the evidence bundle records non-zero matching tests or a named newly added test file.
- Product-audit evidence must have a tracked baseline in `plans/` because raw screenshots/reports under `frontend/test-results/` are git-ignored local artifacts.

## Shared Replay Pack

| Replay | Primary phase | Required evidence |
|---|---|---|
| v6 C02/C03 route prompt | Phase 1 | assistant header, Runs route, DB route, fallback/external instrumentation status |
| v6 timeout run | Phase 2 | AgentRun terminal state, task session terminal state, transcript timeout event |
| v5 accepted preference | Phase 3 | proposal id, accepted patch/snapshot/current view, Overview screenshot |
| v5 Sichuan Museum plan | Phase 4 | plan body/artifact, no fabricated realtime facts, route evidence |
| v4 Day 1-Day 7 plan | Phase 4 | artifact/body, plan id, copy/continue controls |
| v6 provider transmission | Phase 5 | sent/not-sent log, provider/model, data class, no key leakage |
| Today qapressure state | Phase 6 | typed card, not treated as goal/task, count consistency |
| 560/720 width composer | Phase 6 | no overlap, named input/send/cancel controls |

## Command Gate Rule

Focused command gates in sprint RFCs are development-entry contracts, not current proof. A phase cannot mark a gate passed unless the evidence bundle includes:

1. The exact command.
2. The exact test files or test names expected to match.
3. A non-zero matched/passed test count.
4. A note if a listed test file does not exist yet and must be created in that phase.

This prevents `cargo test <filter>` or frontend test filters from passing with zero matched tests.

If a candidate test file does not exist, the sprint must first create the focused test file or replace the command with an existing exact test target. A broad command such as `cargo test -p openlife-tauri provider_route` is useful only when the evidence bundle shows the intended tests were actually discovered and executed.
