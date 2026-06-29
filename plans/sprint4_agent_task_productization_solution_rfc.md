# Sprint 4 Solution RFC: Agent Task Productization

Date: 2026-06-29

Status: prepared for RFC review; implement after Sprint 1-3 foundations.

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
- Realtime fact request without web returns blocker/offline assumption, not fabricated fact.
- MCP missing route returns typed blocker.
- Ambiguous prompt returns clarification within bounded time.

Candidate command-level backend gates after adding/updating focused tests:

- `cargo test -p openlife-tauri plan_execute_product`
- `cargo test -p openlife-tauri main_chat_command_surface`
- `cargo test -p openlife-tauri main_chat_react`

Frontend:

- Artifact card renders body, plan id, copy/edit/continue controls.
- Blocker card renders recovery actions.
- Revision updates or links to existing plan.

Candidate command-level frontend gates after adding/updating focused tests:

- `cd frontend && corepack pnpm test -- ChatPage.test.tsx`
- add or update focused artifact/blocker component tests before claiming PlanArtifactView or BlockerRecoveryView coverage.

Replay:

- v5 Sichuan Museum half-day plan.
- v5 work-plan tasks: 800字总结, 20分钟整理, 15:00回复.
- v4 Day 1-Day 7 low-pressure plan.
- v5 "帮我安排一下".

## Development Slices

1. Plan artifact card for existing PlanExecute session data.
2. Typed blocker card for missing web/current facts.
3. Ambiguous request clarification gate.
4. MCP/plugin capability readiness view.
5. Replay real-life task scenarios.

Exit only when planning produces usable output or actionable blocker.
