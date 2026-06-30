# OpenLife Repair Preparation Review

Date: 2026-06-29

Status: conditionally approved preparation review and hardening pass. This document does not mark product issues fixed; it records whether the preparation artifacts are safe enough to enter development.

## Review Verdict

The preparation set is usable for high-quality staged development after the hardening changes in this pass. This is a preparation approval, not an implementation approval. It permits Sprint 1 thin-slice development only; it does not permit claiming any audited product issue fixed, running broad parallel sprint work, or expanding cloud-provider support before route/transmission evidence is trustworthy.

Before this pass, it had three blockers:

1. audit evidence could be deleted by default cleanup,
2. some command gates could falsely pass with zero matched tests,
3. external-transmission truth was over-scoped in Sprint 1.

Those blockers are now converted into explicit planning constraints and documentation.

## Approval Addendum

Read-only approval checks performed on 2026-06-29:

| Check | Result | Boundary |
|---|---|---|
| Preparation files exist | Passed | All expected repair plan/RFC files exist under `plans/`. |
| Unresolved-marker scan | Passed | No unresolved draft markers were found in the reviewed plan set. |
| Source-anchor spot checks | Passed | Provider validation, route-fact handling, task-session status, PlanExecute commands, LifeModel preference path, Today guard, and Chat composer AX labels were checked against current source paths. |
| Future contract vs current capability | Passed with constraints | Sprint 2 `timed_out` and Sprint 5 `ProviderTransmissionLogEntry` are explicitly marked as target/proposed contracts, not existing implemented capabilities. |
| Evidence retention | Passed with delivery risk | `make clean` now preserves `frontend/test-results/product-audit-*`; however product-audit screenshots remain git-ignored local artifacts, so tracked planning baselines must remain the durable source. |
| Version-control readiness | Not complete | The preparation documents are currently untracked and `Makefile` / `README.md` are modified. This is acceptable for local preparation, but they must be intentionally staged/committed or otherwise preserved before relying on them as a team contract. |

No automated test suite was run during this preparation approval. Test commands listed in sprint RFCs remain candidate implementation-entry gates until the sprint adds/updates the named focused tests and records non-zero matched/passed counts.

Historical note: this review predates Slice 5A commit `083015d`. Rows about missing `provider_transmission` test symbols were true at preparation-review time but are superseded by the committed provider-transmission read model/tests.

## Findings And Corrections

| Severity | Finding | Evidence checked | Correction |
|---|---|---|---|
| P1 | Product-audit evidence was only under git-ignored `frontend/test-results/` and default `make clean` deleted that directory. | `.gitignore` ignores `frontend/test-results/`; local audit folder contains v5/v6 screenshots/reports; old clean target removed `test-results`. | Added tracked `plans/repair_audit_issue_baseline_2026_06_29.md`; changed `make clean` to preserve `frontend/test-results/product-audit-*`; added explicit `make clean-audit-results`. |
| P1 | Sprint 1 implied it could answer whether data left the machine before provider transmission logging exists. | At preparation-review time, no `ProviderTransmissionLogEntry` source/test symbol existed and Sprint 5 said the log was proposed. `083015d` later implemented AgentRun-derived provider-transmission history, not a dedicated log table. | Sprint 1 exposes external-transmission instrumentation status unless positive runtime evidence exists; definitive sent/not-sent history is now partly addressed by Slice 5A, while a dedicated log table remains optional. |
| P1 | Candidate command gates could pass without proving the intended tests ran. | At preparation-review time, `provider_transmission` had no source/test symbol; `AgentRunDetail.tsx` existed but no `AgentRunDetail.test.tsx`; cargo/frontend filters could match zero tests if not checked. `083015d` supersedes the provider-transmission gap with focused tests. | Readiness index and sprint RFCs now require exact test names/files and non-zero matched/passed counts; Sprint 2/Sprint 5 call out missing tests explicitly. |
| P2 | LifeModel path contract was ambiguous across dot-path and slash-path conventions. | Code contains `preferences.communication_style` in builder/proposal paths and `/preferences/communication_style` in maturation/proposal outcome tests. | Sprint 3 now defines canonical dot path, accepted aliases, dedupe key, and alias-normalization tests. |
| P2 | README over-described Today as showing recent AgentRun. | `TodayPage.tsx` loads diagnostics, daily goals, and pending proposals; AgentRun belongs to Activity/Runs. | README now says Today shows system status, suggestions, and pending Proposal only. |
| P2 | Industry references could be misused as implementation evidence. | Repair plan referenced ChatGPT/Claude/Notion/Granola/Codex patterns. | Added `repair_industry_benchmark_guardrails.md` separating product pattern, OpenLife contract, and anti-hallucination check. |

## Current Preparedness By Phase

| Phase | Current quality | Development entry decision |
|---|---|---|
| Sprint 1 Trust Foundation | Strong enough for first thin slice. Scope now excludes definitive provider transmission history. | Enter development with DTO/classifier/Settings/Runs route proof only. |
| Sprint 2 Runs / Trace / Recovery | Strong after Sprint 1 DTO exists. Missing `AgentRunDetail.test.tsx` is now explicit. | Enter after Sprint 1 route DTO; add missing focused detail test in the slice. |
| Sprint 3 LifeModel Closed Loop | Strong enough for design review and thin slice. Path normalization risk is now explicit. | Enter after Sprint 1/2 evidence surfaces are stable. |
| Sprint 4 Agent Task Productization | Good RFC baseline, but depends on route and Runs evidence. | Do not start first; use after Trust/Runs foundation. |
| Sprint 5 Privacy / Provider Governance | Good schema direction, not implementation-ready until store choice is frozen. | Run schema review after Sprint 1 proves route truth; add transmission storage decision before coding. |
| Sprint 6 Daily UX / IA / AX | Good UX contract, should be sliced after core truth chains. | Only small non-invasive fixes before Sprints 1-3; major IA after data contracts stabilize. |

## Approval Boundaries

- Approved: Sprint 1 Trust Foundation thin slice, starting with backend `RuntimeRouteEvidence`, route-truth prompt handling, Settings readiness mapping, and runtime-authored route disclosure.
- Conditionally approved: Sprint 2 and Sprint 3 design refinement, provided they consume Sprint 1 evidence instead of inventing parallel route truth.
- Not approved for broad implementation yet: Sprint 4, Sprint 5, and Sprint 6. Their RFCs are good planning baselines, but they depend on route, Runs, and LifeModel evidence chains becoming stable.
- Not approved: live external provider tests, API-key entry, key rotation, destructive import/export/delete flows, broad visual redesign, or cloud-provider expansion.
- Not approved: any "fixed" claim without implementation diff, replay evidence, source/test proof, and updated regression map.

## Anti-Hallucination Nodes To Preserve During Development

- Provider/model/route/fallback truth must come from runtime metadata or durable route evidence, never model prose.
- Settings readiness must keep configured, credential_present, validated, stale, preferred, actually_used, failed, and fallback separate.
- `not_sent` requires positive evidence; missing provider logs mean unknown/not_instrumented.
- LifeModel acceptance must line up across proposal, patch/snapshot/current view, and UI.
- Runs state must line up across AgentRun, task session, transcript, and UI controls.
- A command gate needs a non-zero matched/passed test count.
- Industry references define desired product quality, not OpenLife implementation proof.
- Source comments, README wording, Settings labels, and assistant-visible UI copy are never sufficient proof by themselves; each trust claim needs source/runtime/test evidence.
- A local/mock/fixture/provider-client proof must remain labeled local or fixture unless the live-provider harness records a real external provider invocation under explicit opt-in.
- If a proposed DTO, DB table, event type, or frontend view model is not found in source yet, documents must call it `proposed`, `target`, or `to be added`, never `existing`.

## Remaining Preparation Work Before Each Sprint

These are not blockers for the whole roadmap, but they are blockers for each sprint's implementation claim:

1. Sprint 1 must name the exact backend/frontend tests added for `RuntimeRouteEvidence`.
2. Sprint 2 must add or choose an exact Runs detail test file.
3. Sprint 3 must implement path normalization tests for dot/slash aliases before changing Review/Overview behavior.
4. Sprint 4 must add focused artifact/blocker UI tests; `ChatPage.test.tsx` alone is not enough.
5. Sprint 5 must freeze the storage choice for transmission history and add provider-transmission tests before running filtered gates.
6. Sprint 6 must add ProductShell and ChatInputArea focused tests before claiming IA/AX coverage.

## Sprint 1 Entry Checklist

Before editing Sprint 1 implementation code, confirm:

1. The untracked preparation docs and modified `Makefile` / `README.md` are intentionally preserved for this workstream.
2. The first code slice names one owner path for `RuntimeRouteEvidence` and one frontend consumer path; avoid touching all Settings/Runs/Chat surfaces at once.
3. The initial tests are concrete and non-zero-matchable before they are used as gates.
4. Replay cases v6 C02/C03 and Settings configured-but-unvalidated are prepared with expected metadata fields.
5. Sprint 1 evidence does not claim definitive provider-transmission history; it may only show direct route proof or `unknown/not_instrumented`.

## Final Readiness Statement

The project should enter development in this order: Sprint 1 thin slice, Sprint 2 lifecycle evidence, Sprint 3 LifeModel closed-loop slice, then Sprints 4-6. Do not start broad cloud-provider expansion or visual redesign before runtime truth, Runs recovery, and LifeModel evidence are trustworthy.
