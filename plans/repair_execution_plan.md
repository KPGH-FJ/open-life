# OpenLife High Quality Repair Execution Plan

Date: 2026-06-29

Status: conditionally approved Sprint 0 diagnosis and solution-freeze artifact. No issue in this document is marked fixed.

## Purpose

This plan turns the 61 product-audit findings into an executable repair backlog. The repair order is intentionally trust-first:

1. Runtime truth, provider route, Settings readiness.
2. Runs, trace, task lifecycle, timeout recovery.
3. LifeModel, Review, Memory closed loop.
4. Agent task productization and blocker recovery.
5. Privacy/provider governance.
6. Daily UX, IA, accessibility, and responsive polish.

The rule for all later implementation is: diagnose first, write the behavior contract second, then implement a thin slice with replay evidence.

Preparation approval boundary:

- This document is approved as a repair execution contract, not as evidence that any product issue is fixed.
- Sprint 1 may start as the first thin slice after its entry checklist is satisfied.
- Sprint 2 and Sprint 3 may continue design refinement, but should not implement competing route/run/LifeModel truth models.
- Sprint 4-6 must not become broad implementation work until the trust foundation, Runs lifecycle, and LifeModel closed loop are backed by source/test/replay evidence.

## Inputs

- `plans/repair_audit_issue_baseline_2026_06_29.md` is the tracked baseline for raw issue coverage, category ownership, and severity counts.
- `frontend/test-results/product-audit-2026-06-29-openlife/repair_backlog_by_issue_attribute.md`
- `frontend/test-results/product-audit-2026-06-29-openlife/combined_issue_classification.md`
- v4/v5/v6 audit folders under `frontend/test-results/product-audit-2026-06-29-openlife/`
- Evidence retention rule: `frontend/test-results/` is git-ignored local evidence. It may contain screenshots and DB notes, but it must not be the only source of truth for development planning. `make clean` preserves `frontend/test-results/product-audit-*`; use `make clean-audit-results` only when intentionally discarding local audit evidence.
- Delivery-state rule: planning docs under `plans/` plus supporting `Makefile` / `README.md` changes must be intentionally preserved before implementation depends on them. Untracked preparation files are acceptable during local drafting, but they are not a durable team contract until tracked, staged for review, committed, or explicitly archived.
- Source paths inspected during Sprint 0:
  - `src-tauri/src/provider_validation.rs`
  - `src-tauri/src/commands/diagnostics.rs`
  - `src-tauri/src/main_chat_runtime_facts/provider_route.rs`
  - `src-tauri/src/main_chat_task_controls.rs`
  - `src-tauri/src/main_chat_generation_support.rs`
  - `src-tauri/src/main_chat_strategy.rs`
  - `frontend/src/pages/settings/tabs/OverviewTab.tsx`
  - `frontend/src/pages/settings/tabs/ProviderTab.tsx`
  - `frontend/src/pages/RunsPage.tsx`
  - `frontend/src/pages/AgentRunDetail.tsx`
  - `frontend/src/utils/runtimeDisclosure.ts`
  - `frontend/src/utils/runDisplaySummary.ts`
  - `frontend/src/pages/LifeModelPage.tsx`
  - `frontend/src/pages/MailboxPage.tsx`
  - `frontend/src/utils/proposalDisplay.ts`
  - `frontend/src/utils/reviewDecision.ts`
  - `frontend/src/utils/lifeModelTrust.ts`
  - `frontend/src/utils/lifeModelQuality.ts`

## Sprint 0 Output

| Artifact | Purpose |
|---|---|
| `plans/repair_execution_plan.md` | Master repair backlog, issue-to-epic mapping, P0/P1 root hypotheses, replay entrypoints |
| `plans/repair_phase_readiness_index.md` | Phase readiness index, development entry gates, shared replay pack |
| `plans/sprint0_diagnosis_runtime_provider_settings.md` | Diagnosis packet and RFC outline for runtime route/provider/Settings truth |
| `plans/sprint0_diagnosis_runs_lifecycle_recovery.md` | Diagnosis packet and RFC outline for Runs/task lifecycle/timeout recovery |
| `plans/sprint0_diagnosis_lifemodel_review_memory.md` | Diagnosis packet and RFC outline for LifeModel/Review/Memory closed loop |
| `plans/sprint1_trust_foundation_solution_rfc.md` | Implementation-ready RFC for route/provider/Settings trust foundation |
| `plans/sprint2_runs_trace_recovery_solution_rfc.md` | Implementation-ready RFC for Runs lifecycle and recovery evidence |
| `plans/sprint3_lifemodel_closed_loop_solution_rfc.md` | Design-ready RFC for LifeModel/Review/Memory closed loop |
| `plans/sprint4_agent_task_productization_solution_rfc.md` | RFC for planning artifacts, blockers, and capability readiness |
| `plans/sprint5_privacy_provider_governance_solution_rfc.md` | RFC for provider transmission log and danger-action preflight |
| `plans/sprint6_daily_ux_ia_ax_solution_rfc.md` | RFC for Today taxonomy, IA, copy, accessibility, and responsive UX |
| `plans/repair_audit_issue_baseline_2026_06_29.md` | Tracked raw issue baseline so ignored screenshot/report folders are not the only planning source |
| `plans/repair_industry_benchmark_guardrails.md` | Industry reference guardrails and anti-hallucination boundaries |
| `plans/repair_preparation_review_2026_06_29.md` | Development-preparation review findings and readiness corrections |

## Current Approval Snapshot

| Area | Approval state | Notes |
|---|---|---|
| Diagnosis coverage | Approved for planning | The tracked baseline covers 61 raw audit issues and maps them to repair epics. |
| Anti-hallucination safeguards | Approved | Route truth, provider readiness, LifeModel write evidence, Runs state, external transmission, and realtime facts all have explicit "cannot infer from prose/UI" rules. |
| Sprint 1 implementation readiness | Approved with checklist | Start with a narrow `RuntimeRouteEvidence`/route-disclosure/Settings readiness slice. |
| Sprint 2 implementation readiness | Conditional | Depends on Sprint 1 route DTO or an identical stub contract; timeout representation must be frozen before code changes. |
| Sprint 3 implementation readiness | Conditional | Start only with canonical `preferences.communication_style` path normalization and visible accepted fact trace. |
| Sprint 4-6 implementation readiness | Not approved for broad work | Use RFCs as design constraints only until earlier trust chains are stable. |
| Existing product fixes | Not approved | No audited issue is fixed by these preparation docs alone. |
| Test evidence | Not yet run | Candidate test commands require non-zero matched/passed counts during implementation evidence collection. |

## Industry Reference Snapshot

The repair direction borrows product principles from current public references, not one-to-one UI cloning:

| Product | Public reference | Practice to borrow | OpenLife implication |
|---|---|---|---|
| ChatGPT Memory / Projects | https://help.openai.com/articles/8590148-memory-faq and https://help.openai.com/en/articles/10169521-projects-in-chatgpt | Saved memory and project context are user-visible and user-controllable. | LifeModel and Memory writes must be inspectable, editable, rejectable, and reversible. |
| Claude Artifacts | https://support.anthropic.com/en/articles/9487310-what-are-artifacts-and-how-do-i-use-them | Substantial generated work is separated from chat into a dedicated, reusable/editable surface. | PlanExecute must return visible plan/artifact bodies, not only hidden governed-draft status text. |
| Notion AI | https://www.notion.com/help/notion-ai-faqs and https://www.notion.com/help/notion-ai-connectors | AI work happens inside the user's workspace and can connect to knowledge sources. | File/session/web/MCP answers need source boundary and source feel, not unsupported claims. |
| Granola | https://docs.granola.ai/help-center/getting-more-from-your-notes/recipes | Sharing/visibility is a product-level control with explicit private/workspace/link modes. | Use this only as a product-pattern analogy for explicit privacy boundaries; it is not direct evidence for LLM provider-transmission telemetry. |
| Codex cloud | https://developers.openai.com/codex/cloud and https://developers.openai.com/codex/cli/features | Background tasks require task status, resumability, and deliverable evidence. | Runs must be the product's trustworthy task log, not a low-information debug table. |

Industry reference guard: these references define product-quality patterns, not implementation proof. Provider-transmission logging in Sprint 5 still needs OpenLife-specific runtime evidence and, before implementation, a more direct provider/privacy reference set if the design starts making retention or third-party processing claims.

## Repair Epics

| Epic | Primary issues | Owner surface | First thin slice | Exit criteria |
|---|---|---|---|---|
| E1 Runtime Truth / Provider Route / Settings Readiness | `OL-001`, `OL-008`, `V4-001`, `V4-006`, `V4-007`, `V5-011`, `V6-001`, `V6-003`, `V6-004`, `V6-005`, `V6-009` | backend diagnostics, route facts, Settings, Companion, Runs | Unified `RuntimeRouteEvidence` read model and route-truth prompt handling | Companion answer, Runs, and DB agree on provider/model/route/fallback |
| E2 Runs / Trace / State Lifecycle / Recovery | `OL-007`, `V4-002`, `V4-011`, `V5-007`, `V5-008`, `V6-002`, `V6-007` | task sessions, AgentRun store, Runs UI | Timeout/cancel/retry state finalization and `RunEvidenceView` aggregation | Timeout no longer leaves durable `running`; Runs has replayable evidence |
| E3 LifeModel / Review / Memory Closed Loop | `OL-002`, `OL-005`, `V4-003`, `V5-001`, `V5-002`, `V5-003`, `V5-004`, `V5-005`, `V5-013`, `V5-018`, `V5-020`, `V5-022` | proposal pipeline, LifeModel read model, Review, Builder, Versions | Accepted preference visible in LifeModel with source/proposal trace | Accepted fact appears in Overview and can be traced to proposal/patch/snapshot |
| E4 Agent Task Productization | `OL-003`, `OL-006`, `V4-008`, `V4-013`, `V4-014`, `V4-015`, `V5-006`, `V5-010`, `V5-015`, `V5-021`, `V6-008` | Main Chat, PlanExecute, blockers, MCP/plugin readiness | Planning returns artifact/body with recovery controls | Real-life planning tasks produce usable output or actionable blocker |
| E5 Today / Daily Usefulness / Personalization | `OL-004`, `V4-004`, `V5-009`, `V5-012`, `V5-023` | Today, output guardrails, preference application | Typed Today cards: signal/state/goal/task/suggestion | State metrics no longer become goals; accepted preference affects output or is explicitly unused |
| E6 Privacy / Provider Governance | `OL-010`, `V4-005`, `V5-014`, `V6-006`, `V6-010` | Privacy page, provider telemetry, danger zone actions | Provider transmission log entry per run | User can see sent/not-sent externally, provider/model, data class, confirmation state |
| E7 IA / Navigation / Copy / Counts | `OL-009`, `V4-009`, `V4-012`, `V5-019`, `V5-024` | navigation, Review naming, counts, copy | Rename/governance nav and count taxonomy | Review/Runs/Settings/LifeModel are findable and counts are consistent |
| E8 Accessibility / Input / Responsive | `OL-011`, `V4-010`, `V4-016`, `V5-016`, `V5-017` | composer, buttons, narrow layouts | Semantic composer and named controls | AX names stable; Chinese input and 560/720 widths are usable |

## P0/P1 Root Hypothesis Ledger

Every P0/P1 issue has a current root-cause hypothesis and a replay entrypoint. These are not final root causes; they are the next engineering investigation contract.

| Issue | Severity | Root-cause hypothesis | Replay entrypoint |
|---|---|---|---|
| `OL-001` | P0 | Route/date/tool truth can be answered by model prose instead of runtime-authored facts. `provider_route` classifier is too narrow for mixed real prompts. | Ask current date/provider/model/tool/fallback; compare assistant text with `AgentRun.modelRoute` and route facts. |
| `V4-001` | P1 | Settings Overview consumes broad readiness booleans while Provider tab has richer validation states. | Open Settings Overview and Model tab with configured-but-unvalidated provider; compare labels. |
| `V6-001` | P0 | Explicit cloud/provider prompt fell through to generation; model claimed DeepSeek while DB route was local Ollama. | Replay v6 C02/C03 prompt; verify run `modelRoute.provider/model/routeType` in DB/Runs. |
| `V6-003` | P1 | `configured`, `validated`, `preferred`, and `actually_used` are rendered by separate components with different state contracts. | Screenshot Settings Overview and Provider tab after same diagnostics load. |
| `V6-004` | P1 | Provider health UI treats enabled/configured availability as readiness even when health/validation is gray or unchecked. | Open provider health/readiness cards; compare `cloud_api_validation_status` and visible state. |
| `V6-005` | P1 | Cloud preference does not produce an authoritative blocker/fallback reason when actual route remains local. | Prompt "use cloud or explain why not"; verify route chip, fallback reason, and run metadata. |
| `V4-002` | P1 | Blocker paths do not always persist observation/blocker events into AgentRun and task transcript. | Trigger file/web/MCP blocker; open Runs detail and count observations/blockers. |
| `V5-007` | P1 | Runs UI aggregates AgentRun counters separately from task-session transcript, hiding useful events. | Open Builder/web/PlanExecute run detail; compare task transcript with visible counters. |
| `V5-008` | P1 | Ambiguous request can remain non-terminal because timeout/cancel/recovery state is not durably finalized. | Send "帮我安排一下"; wait; verify task status, controls, and DB state. |
| `V6-002` | P1 | Provider timeout UI state is not synchronized back to AgentRun/task session terminal state. | Replay DeepSeek timeout case; verify run/session is `timed_out` or `failed`, not `running`. |
| `OL-002` | P0 | Permission classification is overbroad; read-only builtin operations can be shown as high-risk/write. | Trigger read-only builtin proposal/permission path; inspect proposal type/risk/action. |
| `OL-005` | P1 | LifeModel fields are rendered as low-source summary without canonical source/proposal/patch projection. | Open LifeModel Overview; trace any claim to source/proposal/snapshot. |
| `V4-003` | P1 | Memory intent dedupe and schema normalization are weak; one request can create duplicate proposals. | Ask to remember one low-risk preference; inspect Review proposal count/types. |
| `V5-001` | P1 | Builder candidate state is not surfaced as the same pending-change object in LifeModel/Review. | Complete Builder candidate; navigate Builder, LifeModel, Review. |
| `V5-002` | P1 | Builder/LM UI can present update language before proposal acceptance/write confirmation. | Submit Builder update; observe copy before accepting Review proposal. |
| `V5-003` | P1 | Accepted patch persists but Overview reads limited LifeModel fields and does not project accepted preference path. | Accept `preferences.communication_style`; reopen Overview and source trace. |
| `V5-004` | P1 | Extraction lacks field-boundary/source-span validation and quarantine for schema-mixed facts. | Enter multi-field persona data; inspect generated proposal paths and values. |
| `V5-005` | P1 | Proposal title/detail derives from broad proposal type/source, not enough from affected path/source excerpt. | Open Review detail for Builder proposals; check title, diff, source, affected path. |
| `OL-003` | P1 | Ambiguous planning defaults to assumptions and capability claims without clarification gate. | Ask vague planning prompt; verify clarification vs unsupported assumptions. |
| `OL-006` | P1 | Blockers are raw runtime outcomes without product recovery model and setup links. | Trigger missing web/MCP/safe-path; inspect blocker CTA and Runs event. |
| `V4-014` | P1 | PlanExecute creates governed draft state but chat lacks visible artifact/body projection. | Ask for Day 1-Day 7 plan; verify visible plan body/artifact and plan_id. |
| `V5-006` | P1 | Specific planning path routes to governance/debug copy rather than user-facing plan artifact. | Ask 800字总结/资料整理/15:00回复计划; check output body language. |
| `OL-004` | P1 | Today mixes state metrics, pending proposals, and goals without typed card model. | Open Today; compare pending counts and `qapressure` style state display. |
| `V4-004` | P1 | Same as `OL-004`, confirmed after rerun; Today count source differs from Review/Mailbox. | Open Today and Review after seeded state metric; compare count taxonomy. |
| `V5-009` | P1 | Real-life planning lacks realtime-fact guard and leaks governance/internal copy. | Ask Sichuan Museum plan with no web; verify no fabricated hours/weather/traffic. |

## Sprint Execution Contract

Each Sprint must produce five artifacts before being considered complete:

1. Problem Diagnosis Packet with raw IDs, user path, evidence, source-path hypothesis, industry comparison, and unresolved uncertainty.
2. Solution RFC with target behavior, state model, UI expression, API/DB contract, failure states, and acceptance checks.
3. Thin-slice implementation with minimal blast radius.
4. Regression map update against v4/v5/v6 scenarios.
5. Evidence bundle: screenshots plus DB/trace/runtime metadata or focused tests.

## Development Control Rules

Use these rules during implementation to keep the repair work from becoming another layer of untrusted UI:

1. One canonical truth model per domain. If Sprint 1 introduces `RuntimeRouteEvidence`, later Settings, Runs, Privacy, and Companion surfaces must consume it or a direct derivative rather than rebuilding route truth from labels.
2. Thin slice before broad surface updates. A sprint should prove one backend read/write path and one user-visible surface before expanding to all pages.
3. No prose-only proof. Assistant messages, button labels, README text, and screenshots can support evidence, but cannot replace runtime metadata, DB/trace records, or focused tests.
4. Candidate tests are not gates until they run non-zero matched cases. If a test file is missing, create it in the sprint or remove the command from the gate.
5. Local/mock/fixture evidence must remain labeled local/mock/fixture. It cannot satisfy external live-provider, cloud route, provider transmission, or third-party privacy claims.
6. Unknown is an acceptable state. Prefer `unknown`, `not_instrumented`, or an explicit blocker over a confident but unsupported readiness/privacy claim.
7. Update the regression map in the same change set as the fix. A code diff without replay status is not complete for this repair program.

## Source-Anchor Verification Checklist

Before coding each high-risk slice, verify these anchors are still current:

| Slice | Source anchor to re-check | Why |
|---|---|---|
| Provider readiness | `src-tauri/src/provider_validation.rs` | Prevent confusing configured/key-present with validated/live-ready. |
| Route fact prompts | `src-tauri/src/main_chat_runtime_facts/provider_route.rs` | Ensure route-truth prompts are intercepted before model prose. |
| Task lifecycle | `openlife-core/src/agent/main_chat_agent_v1.rs` | Confirm native session statuses before adding/mapping timeout. |
| Runs UI | `frontend/src/pages/RunsPage.tsx` and `frontend/src/pages/AgentRunDetail.tsx` | Prevent UI counters from diverging from transcript/task evidence. |
| LifeModel preference | `src-tauri/src/commands/proposal.rs`, `src-tauri/src/commands/builder.rs`, `frontend/src/pages/LifeModelEditor.tsx` | Keep proposal, patch/snapshot, and current view aligned. |
| PlanExecute | `src-tauri/src/commands/agent_runtime/plan_execute_product.rs` | Reuse existing session/draft/blocker events instead of inventing a parallel planner. |
| Today / AX | `frontend/src/utils/dailyGoalDisplayGuard.ts`, `frontend/src/pages/chat/ChatInputArea.tsx` | Preserve state-vs-goal guards and named controls while polishing UI. |

## Anti-Hallucination Gates

- Provider/model/route/fallback truth must come from runtime metadata, not assistant prose.
- Settings must not treat `configured` as `validated`.
- LifeModel write success must line up across proposal, patch/snapshot/current model, and Overview.
- Runs state must be checked against task session, transcript, and AgentRun, not only UI status.
- External provider calls must be represented from runtime/transmission evidence. Absence of evidence is not cloud proof, and absence of a provider log is not enough to claim `not_sent`.
- Realtime facts such as date, weather, opening hours, and traffic need sources or explicit blocker/offline assumption.

## Sprint 1 Thin-Slice Recommendation

Start with E1 first, then the minimum E2 hook needed to show route/run consistency. Keep the code changes thin:

1. Create a backend `RuntimeRouteEvidence` read model from current/last/planned route, provider validation, preflight, fallback, and external transmission instrumentation status.
2. Expand route-truth prompt classification so v6 C02/C03 style prompts are answered by facts before model generation.
3. Update Settings Overview to consume the same typed provider readiness state as Provider tab.
4. Add only the Runs route evidence projection needed to prove Companion/Runs/DB agreement.
5. Defer timeout-finalization and full `RunEvidenceView` aggregation to Sprint 2 unless they are required to make the route evidence durable.

Do not start cloud-provider expansion until E1 has route proof and E6 has transmission logging.

## Replay Matrix

| Replay | Blocks release if | Evidence required |
|---|---|---|
| v6 C02/C03 route prompts | UI answer, Runs, and DB disagree | screenshot, run id, provider/model/route/fallback metadata |
| v6 timeout run | durable state remains `running` after timeout | task session row, AgentRun row, Runs screenshot |
| v5 accepted preference | accepted value is not visible in LifeModel Overview | proposal id, patch/snapshot path, Overview screenshot |
| v5 Sichuan Museum plan | output invents realtime facts or lacks offline assumptions | assistant output, route evidence, no-web blocker or source |
| v4 Day 1-Day 7 plan | no visible body/artifact appears | plan id, artifact/body screenshot, Runs trace |
| Today state metric | state metric appears as goal/task | Today screenshot, source/pending-count comparison |
| Review duplicate memory | one intent creates duplicate schema-inconsistent proposals | Review screenshot, proposal ids |
| Settings provider readiness | configured/unvalidated appears as live-ready | diagnostics payload or Provider tab screenshot |

## Not Fixed Yet

This Sprint 0 artifact does not change runtime behavior. It only freezes the diagnosis and repair order. Any later claim of "fixed" must cite the implementation diff, focused tests, and replay evidence.
