# Sprint 4 Diagnosis Packet: Agent Task Productization

Date: 2026-06-29

Status: source-level preparation for Slice 4A. This packet is not a fixed-status claim.

## Scope

Raw audit issues: `OL-003`, `OL-006`, `V4-008`, `V4-013`, `V4-014`, `V4-015`, `V5-006`, `V5-010`, `V5-015`, `V5-021`, `V6-008`.

Primary user promise: a planning task should produce a usable plan artifact or an actionable blocker, with route/run evidence that does not depend on assistant prose.

## Verified Source Reality

Checked source entrypoints:

| Surface | Current reality | Risk |
|---|---|---|
| `src-tauri/src/commands/agent_runtime/plan_execute_product.rs` | Existing commands create, list, update, finalize, cancel, review, execute, and skip PlanExecute sessions. Session creation stores a draft and appends plan events. | The current product command still drafts from a fixed weekly-planning task text, so Slice 4A must not pretend arbitrary real-life prompts are already represented unless the Main Chat path passes them through. |
| `src-tauri/src/main_chat_agent_productization_tests.rs` | Existing tests prove Main Chat can expose PlanExecute agent state, draft controls, step controls, observation evidence, and final-delivery evidence. | These tests do not prove a user-facing plan artifact body with copy/continue affordances. |
| `frontend/src/tauri.ts` | `MainChatAgentStateSnapshot.plan` already carries plan id, session id, task/run ids, status, summary, revision, controls, and steps. | The type lacks a dedicated artifact body, assumptions, unknowns, and route/run evidence refs as a single product view. |
| `frontend/src/components/AgentControlPlane.tsx` | The UI shows a compact Plan panel, execution timeline, final delivery, and blocker list. | The plan surface reads like runtime/debug evidence, not a Claude-Artifact-like reusable deliverable. |
| `frontend/src/components/AgentControlPlane.tsx` blocker list | Existing blockers show title/detail/id/recoverability and some controls. | Missing typed recovery model for web/current-fact/provider/MCP blockers; do not solve this broadly in Slice 4A unless needed to avoid false success. |
| `frontend/src/components/RunTracePanel.tsx` / `frontend/src/pages/RunsPage.tsx` | Existing Runs tests cover `plan_execute_product` trace rendering. | Runs trace is evidence; it is not a substitute for the chat/task artifact users need. |

## Root-Cause Hypotheses

1. `V4-014` / `V5-006`: PlanExecute has backend session state but no canonical user-facing artifact view. The UI exposes summary, controls, and timeline, while the user asked for a plan body.
2. `OL-003`: Ambiguous planning can route into task machinery without a product-level clarification gate. This should be a later Slice 4C unless Slice 4A touches routing prompts.
3. `OL-006` / `V5-010`: Blockers exist as runtime events, but recovery actions are not normalized for the user. This should be Slice 4B after artifact body proof.
4. `V6-008`: Provider/model quality is not enough if the artifact/read-model contract is absent. Cloud output may improve wording but cannot fix missing artifact evidence.
5. `V5-021`: Plan state and run state can be technically present while the daily workflow still lacks copy, continue, and revision affordances.

## Industry Benchmark Applied

| Benchmark | Product bar for Slice 4A |
|---|---|
| Claude Artifacts | Substantial generated work should appear as a stable, reusable surface separate from transient chat prose. |
| Codex / Cursor background agents | Long-running or delegated work needs state, logs, controls, and deliverable evidence. |
| Notion AI workspace answers | The product should show source boundaries and should not imply current facts without a source. |

## Slice 4A Frozen Scope

Implement only: user-facing `PlanArtifactView` for existing PlanExecute/Main Chat plan state.

Required behavior:

- A PlanExecute planning response shows a visible artifact card/body in chat/task control surface.
- The artifact includes `plan_id`, `plan_session_id`, `task_session_id`, `run_id`, status, summary/title, generated body, steps, assumptions, unknowns, controls, route evidence ref, and run evidence ref where available.
- The body must come from backend/session/read-model data. The frontend may format it but must not invent plan content.
- If only summary/steps are available, backend must build a bounded plain-language body from those fields and label realtime unknowns as assumptions/unknowns.
- For realtime/current facts such as opening hours, weather, traffic, and date-sensitive availability, the artifact must either cite a source/tool observation or show `unknown`/offline assumption. Do not fabricate.
- Copy is allowed as a frontend control. Edit/continue/execute/retry controls must map to existing supported controls or render disabled with an explicit reason.
- Runs/trace must still show the same plan/run evidence; the artifact must not bypass Runs.

Non-goals for Slice 4A:

- No broad web browsing implementation.
- No new provider integration or key handling.
- No MCP/plugin readiness redesign.
- No ambiguous-request clarification gate unless needed as a tiny guard for the artifact path.
- No destructive writes, external actions, or direct LifeModel writes.

## Anti-Hallucination Checks

- Do not trust assistant text that says a plan was created; assert `agent_state.plan.planId` and `planSessionId` or the new artifact ids.
- Do not trust UI copy alone; verify backend read-model fields and frontend props/tests.
- Do not mark a plan successful if there is no visible artifact/body.
- Do not claim web/current facts; require tool/source evidence or explicit `unknown`.
- Do not claim cloud/provider improvement in this slice; route evidence is consumed only as already available truth.
- Do not treat generic blockers as recovered; recovery must name the blocker code and next action.

## Slice 4A Acceptance Tests

Backend focused tests to add or update:

- `cargo test -p openlife-tauri plan_execute_product` must include a non-zero matched test that builds `PlanArtifactView` from a draft session with body, ids, steps, assumptions, unknowns, and evidence refs.
- `cargo test -p openlife-tauri main_chat_agent_state_payload_exposes_plan_execute_controls_from_later_plan_transcript` or a nearby test must assert the artifact/read-model is surfaced through ordinary Main Chat PlanExecute state.
- `cargo test -p openlife-tauri main_chat_command_surface` must remain passing, proving no fallback regression.

Frontend focused tests to add or update:

- `cd frontend && corepack pnpm test -- AgentControlPlane.test.tsx` must assert the plan artifact body, plan id, copy control, continue/edit control semantics, and unknown/current-fact warning render.
- Add a focused util/component test if artifact formatting is extracted.
- `cd frontend && corepack pnpm typecheck` must pass.

Repository gates:

- `cargo fmt --check`
- `git diff --check`

## Replay Scenarios For Slice 4A

Manual replay can be app-based after code gates pass; automated tests must encode the same expectations with deterministic fixtures.

| Scenario | Expected Slice 4A evidence |
|---|---|
| `明天 2026-06-30 去四川博物馆，安排低压力半日行程；不要编造开放时间、天气、交通` | Artifact body appears; opening hours/weather/traffic are listed as unknown unless sourced; plan id and run evidence visible. |
| `800字项目总结、20分钟资料整理、15:00前回复合作消息，安排低压力计划` | Artifact body is Chinese, copyable, low-pressure, step-based, with plan id/session id. |
| `生成 Day 1-Day 7 低压力计划，不写入记忆` | Artifact body or structured sections appear; no memory write. |

## Rework Triggers

Slice 4A must be returned for rework if any of these happen:

- The chat says only "created governed draft" or similar without a visible plan body/artifact.
- The artifact body is hardcoded frontend demo text.
- Current facts are asserted without source/tool evidence.
- Route or run evidence is generated from assistant prose instead of existing runtime fields.
- Plan controls are shown but do not map to existing supported actions or disabled reasons.
- Existing command-surface tests lose legacy-fallback or silent-write guard coverage.
