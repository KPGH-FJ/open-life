# Sprint 4 Solution RFC: Agent Task Productization

Date: 2026-06-29

Status: ready for bounded Slice 4A implementation after source-level diagnosis in `plans/sprint4_agent_task_productization_diagnosis_packet.md`. This is not approval for broad agent-task redesign.

## Scope

Raw issues: `OL-003`, `OL-006`, `V4-008`, `V4-013`, `V4-014`, `V4-015`, `V5-006`, `V5-010`, `V5-015`, `V5-021`, `V6-008`.

Primary source entrypoints:

- `src-tauri/src/main_chat_route_preview.rs`
- `src-tauri/src/commands/agent_runtime/plan_execute_product.rs`
- PlanExecute commands in `frontend/src/tauri.ts`
- `frontend/src/pages/ChatPage.tsx`
- `frontend/src/components/AgentControlPlane.tsx`
- `frontend/src/components/RunTracePanel.tsx`
- `frontend/src/pages/RunsPage.tsx`
- MCP/plugin pages and capability status utilities

## Product Goal

Agent tasks should produce usable work, visible artifacts, and clear recovery paths. A user asking for a plan should receive a plan body or an artifact, not a hidden governed-draft message.

## Source Reality Freeze

Slice 4A must start from the current PlanExecute and Main Chat evidence surfaces:

- Backend PlanExecute already has session lifecycle commands, step records, revision checks, blocker event emission, and AgentRun trace updates in `src-tauri/src/commands/agent_runtime/plan_execute_product.rs`.
- Main Chat already exposes plan evidence and draft controls in `agent_state.plan`; existing tests prove plan/session/control/evidence presence but not a user-facing artifact body.
- Frontend `AgentControlPlane` currently renders plan summary, step timeline, final delivery, and blockers. It does not yet present a reusable plan artifact comparable to Claude Artifacts.
- Runs/trace evidence already exists and must be consumed, not replaced by a parallel planner-specific truth model.

## Non-Goals

- Do not enable unsafe writes.
- Do not implement broad web browsing if governed web route is unavailable.
- Do not add provider-specific hacks.
- Do not hide blockers behind generic "failed" messages.

## View Models

`PlanArtifactView`:

| Field | Meaning |
|---|---|
| `plan_id` | Stable plan id. |
| `task_session_id` | Main Chat task session. |
| `run_id` | AgentRun id. |
| `status` | draft, ready, executing, blocked, cancelled, completed. |
| `title` | user objective. |
| `body` | user-facing plan text or structured sections. |
| `steps` | step id, label, status, optional due/time block. |
| `assumptions` | explicit assumptions, especially realtime unknowns. |
| `unknowns` | facts not verified. |
| `controls` | copy, edit, continue, execute step, skip, cancel, retry. |
| `route_evidence` | Sprint 1 route evidence. |
| `run_evidence` | Sprint 2 evidence refs. |

Slice 4A required minimum:

| Field | Minimum acceptance |
|---|---|
| `body` | Bounded backend-derived plan text assembled from existing session summary/steps/objective. Frontend may format but may not invent content. |
| `assumptions` | At least explicit offline/realtime limitations when current facts are requested without source evidence. |
| `unknowns` | Date-sensitive facts, web facts, weather, opening hours, traffic, or provider capability gaps that are not verified. |
| `controls` | Copy plus only supported plan controls; unsupported continue/edit actions must be hidden or disabled with a reason. |
| `route_evidence` / `run_evidence` | Existing Sprint 1/Sprint 2 refs where available; unknown is allowed, fabricated evidence is not. |

`BlockerRecoveryView`:

| Field | Meaning |
|---|---|
| `blocker_code` | web_unavailable, mcp_missing, safe_path_required, provider_unvalidated, permission_required, realtime_fact_unavailable. |
| `plain_summary` | user-facing explanation. |
| `why_it_happened` | short factual reason. |
| `recovery_actions` | setup, retry, continue offline, choose file, open settings, request permission. |
| `can_continue_offline` | boolean. |
| `evidence_ref` | run/task/transcript id. |

`CapabilityReadinessView`:

| Field | Values |
|---|---|
| `capability` | web, workspace_file, mcp, plugin, provider, plan_execute |
| `installed` | boolean |
| `registered` | boolean |
| `executable` | boolean |
| `permissioned` | boolean |
| `tested` | boolean |
| `last_error` | safe string |

## Behavior Contracts

Planning:

- Specific planning prompts return a plan body/artifact in the same user language.
- Long plans use artifact card plus summary.
- Revisions preserve context and update the same plan when possible.
- Ambiguous prompts ask a clarifying question unless a current task context is explicit.

Realtime facts:

- Museum opening hours, weather, traffic, dates, and current facts require source evidence.
- Without web/tool route, return offline plan plus explicit unknowns.

Blockers:

- Missing web/MCP/provider/safe path produces `BlockerRecoveryView`.
- Blockers appear in chat and Runs timeline.
- Recovery action links to the relevant Settings/Tools page when available.

## Tests

Every command gate must record a non-zero matched/passed test count. These gates are implementation-entry targets, not proof that the current repo already has all coverage.

Backend:

- Planning intent creates PlanArtifactView with body and plan id.
- PlanArtifactView is built from durable PlanExecute/Main Chat state, not from frontend demo text.
- Realtime fact request without web returns blocker/offline assumption, not fabricated fact.
- MCP missing route returns typed blocker.
- Ambiguous prompt returns clarification within bounded time.

Candidate command-level backend gates after adding/updating focused tests:

- `cargo test -p openlife-tauri plan_execute_product`
- `cargo test -p openlife-tauri main_chat_command_surface`
- `cargo test -p openlife-tauri main_chat_react`
- `cargo test -p openlife-tauri main_chat_agent_state_payload_exposes_plan_execute_controls_from_later_plan_transcript`

Frontend:

- Artifact card renders body, plan id, copy/edit/continue controls.
- Blocker card renders recovery actions.
- Revision updates or links to existing plan.

Candidate command-level frontend gates after adding/updating focused tests:

- `cd frontend && corepack pnpm test -- ChatPage.test.tsx`
- `cd frontend && corepack pnpm test -- AgentControlPlane.test.tsx`
- add or update focused artifact/blocker component tests before claiming PlanArtifactView or BlockerRecoveryView coverage.

Replay:

- v5 Sichuan Museum half-day plan.
- v5 work-plan tasks: 800字总结, 20分钟整理, 15:00回复.
- v4 Day 1-Day 7 low-pressure plan.
- v5 "帮我安排一下".

## Development Slices

1. Slice 4A: Plan artifact read model and card for existing PlanExecute/Main Chat plan state.
2. Typed blocker card for missing web/current facts.
3. Ambiguous request clarification gate.
4. MCP/plugin capability readiness view.
5. Replay real-life task scenarios.

Exit only when planning produces usable output or actionable blocker.

## Slice 4A Implementation Contract

Goal: make existing PlanExecute work visible as a reusable product artifact without changing provider, MCP, web, or LifeModel write behavior.

Required implementation:

1. Add a backend `PlanArtifactView` builder near the PlanExecute product command or Main Chat agent-state projection. It must carry plan/session/task/run ids, status, title/summary, body, steps, assumptions, unknowns, controls, route evidence, and run evidence.
2. Surface the artifact through the existing ordinary Main Chat PlanExecute path and/or `agent_state.plan`; do not create a separate planner truth store.
3. Render a first-class plan artifact section/card in `AgentControlPlane`, with copy support and existing supported controls.
4. Preserve existing final-delivery, timeline, Runs, route, and blocker surfaces.
5. Encode the three replay prompts above as deterministic fixture/unit expectations where live app replay is not part of the automated gate.

Blocked from this slice:

- Direct web/current-fact implementation.
- External provider expansion or key handling.
- MCP/plugin readiness redesign.
- Memory/LifeModel writes.
- UI-only artifact body not backed by backend/read-model evidence.
