# Main Chat Agent Control Plane v2 UI Flow

> Date: 2026-06-17
> Status: preparation artifact for Product Maturity v2
> Parent: `plans/main_chat_agent_product_maturity_v2_goal_spec.md`

## 1. Purpose

This document defines the UI flow for the next Main Chat Agent Control Plane.

The goal is not visual polish first. The goal is state completeness: users must
see what the agent is doing, what is blocked, what can be changed, what was
accepted, what can be rolled back, and what is safe to continue.

## 2. Current Baseline

Productization v1 UI already renders:

- task status,
- route,
- context,
- provider,
- plan summary,
- actions,
- observations,
- blockers,
- proposals,
- final delivery,
- resume/retry/cancel,
- proposal controls,
- exact ToolPermission approve/deny/defer.

V2 UI must add richer surfaces for:

- memory lifecycle,
- real event stream state,
- editable plan steps,
- task continuity,
- skills/tools.

## 3. Layout

Main Chat v2 should keep chat as the control surface.

Recommended sections:

1. Compact task header.
2. Live event/status strip.
3. Plan panel.
4. Action/observation timeline.
5. Memory/proposal panel.
6. Task continuity panel.
7. Skill/tool panel.
8. Final delivery panel.

Panels should be collapsible but not hidden behind developer-only views.

Priority rule:

- Always visible: compact task header, current event/status strip, current
  plan/action/blocker/proposal control, and final delivery when present.
- Collapsible by default: full event log, memory lifecycle history, task
  continuity list, and skill/tool details.
- Review Center remains the deep governance surface for full provenance,
  conflict history, and rollback history.
- Main Chat must show enough state to act safely, but it should not become a
  dense database browser.

## 3.1 Product Quality Bar

The UI is ready only if it helps the user continue the task, not merely inspect
runtime internals.

Required quality bar:

- The first viewport must answer: what the agent is doing now, what needs the
  user, and what already changed.
- DirectAnswer must stay low-noise; do not show a large task dashboard unless
  the user expands task details.
- The current blocker or current available control must be more prominent than
  historical evidence.
- Deep provenance, event logs, memory history, and tool details start collapsed
  unless they explain the current blocker or required user decision.
- Destructive or durable controls must state the consequence before execution.
- Unsupported controls are hidden or clearly disabled with a stable reason.
- Narrow screens must preserve task status, current control, and final delivery
  before secondary evidence.
- Empty panels are hidden; a section should not render just to prove the schema
  exists.

## 4. Memory Proposal Card v2

States:

- candidate,
- pending review,
- edited pending review,
- accepted,
- materialized,
- rejected,
- deferred,
- superseded,
- rolled back.

Visible fields:

- proposed memory,
- scope,
- evidence,
- confidence,
- conflict state,
- provenance,
- active/materialized status,
- rollback history.

Controls:

- accept,
- reject,
- edit,
- defer,
- rollback only when implemented and allowed,
- open Review Center.

## 5. Plan Panel v2

Visible fields:

- plan goal,
- plan status,
- revision,
- step list,
- active step,
- linked action/observation/proposal,
- blocked/skipped reason,
- review summary.

Controls:

- confirm plan,
- edit plan,
- execute next read step,
- skip step,
- cancel remaining steps,
- review.

Render rule:

- assistant prose can be shown as text, but cannot become PlanView without
  plan evidence.

## 6. Event Stream State

UI states:

- `loading_snapshot`,
- `subscribed`,
- `receiving_event`,
- `replaying_events`,
- `event_gap_detected`,
- `snapshot_refresh_required`,
- `stream_disconnected`,
- `stream_recovered`.

Visible behavior:

- small status strip, not a large developer log by default,
- expandable event log for audit,
- clear reconnect/replay indicator.
- event status cannot show `subscribed` unless replay command and live event
  transport are both available.

## 7. Task Continuity Panel

Task list item:

- title,
- status,
- route,
- last updated,
- last observation,
- blocker/proposal count,
- next action.

Task detail:

- timeline,
- pending controls,
- stale warning,
- terminal state explanation,
- last safe resume point.

Controls:

- resume,
- retry,
- cancel,
- refresh context,
- open trace.

## 8. Skill/Tool Panel

Visible fields:

- selected skill,
- skill source,
- bounded instruction preview,
- tool candidates,
- selected tool,
- selection reason,
- risk/policy status,
- permission requirement.

Controls:

- select skill,
- clear skill,
- inspect skill,
- approve permission when exact action proposal exists,
- retry/switch tool if safe.

Skill/tool detail should start collapsed unless the current task is blocked on
tool choice or permission.

## 9. Final Delivery v2

Final delivery must separate:

- final answer,
- executed actions,
- observations used,
- plan steps completed,
- plan steps skipped,
- proposals created,
- memory changes accepted,
- memory changes rolled back,
- blocked items,
- pending user actions,
- next steps.

## 10. Fail-closed UI Rules

- No action card without action evidence.
- No observation card without observation evidence.
- No proposal card without ProposalStore/equivalent fixture evidence.
- No rollback button without rollback command.
- No plan edit button without plan command.
- No skill selection claim without selected skill id.
- No event stream status without stream/replay state.
- No task resume button for terminal/stale unsafe task.

## 11. Manual QA Checklist

Before calling v2 UI ready:

- simple answer remains low-noise,
- read task shows action/observation,
- multi-step task shows multiple observations,
- blocked task stays visible,
- memory proposal can be accepted/rejected/edited/deferred,
- accepted memory can be inspected,
- rollback appears only when real,
- plan step can be edited/skipped,
- stale task asks before continuing,
- skill selection is visible and bounded,
- final delivery distinguishes executed/proposed/blocked/pending.
