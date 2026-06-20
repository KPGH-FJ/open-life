# Main Chat Stage 2 Failure Recovery Requirements

> Date: 2026-06-19
> Stage: Main Chat Agent Stage 2 - Internal Trial Readiness
> Status: preparation requirements

## 1. Purpose

Internal trial will fail if users cannot tell what went wrong or what to do
next. Stage 2 must make failure a first-class product state.

## 2. Failure Taxonomy

| Failure | Example | Required state | Allowed controls | Final delivery requirement |
| --- | --- | --- | --- | --- |
| Intent ambiguity | "Help me with my plan." | `ask_user` or bounded draft. | clarify, cancel. | State what information is needed. |
| Missing source | File/session/MCP target absent. | `blocked_missing_source`. | change target, retry, cancel. | Do not hallucinate source contents. |
| Policy blocker | Web/network/MCP disabled. | `blocked_by_policy`. | request permission if supported, cancel. | Explain policy and next safe option. |
| Permission needed | Write-like or sensitive action. | `waiting_for_permission`. | approve exact scope, deny, defer. | Mark work pending, not done. |
| Permission denied | User denies action. | `permission_denied`. | revise plan, cancel. | No execution after denial. |
| Provider malformed output | Bad JSON/action envelope. | `model_output_invalid`. | retry with guidance, cancel. | Show failure, not confident answer. |
| Disallowed tool | Model selects unknown/unsafe target. | `model_selected_disallowed_tool`. | retry, ask user, cancel. | No fallback unless explicitly labeled. |
| Tool execution failure | Read error/network timeout. | `tool_execution_failed`. | safe retry, change source, cancel. | Preserve error and observation status. |
| Stale task | Resume target changed or task too old. | `stale_context_warning`. | refresh context, restart task, cancel. | Do not replay stale action silently. |
| Terminal task | Cancelled/completed task resumed. | `terminal_state_blocker`. | start new task. | Explain terminal state. |
| Memory conflict | New preference conflicts with accepted memory. | `memory_conflict_detected`. | clarify, create proposal, reject. | No silent overwrite. |
| Plan step failure | Step blocked/skipped/fails. | `step_blocked` or `step_failed`. | retry step, skip step, edit plan, cancel. | Review separates failed/skipped/completed. |

## 3. Runtime Requirements

Every failure must have:

- task id;
- action id when applicable;
- blocker reason code;
- user-facing explanation;
- retryability flag;
- safety/risk level when permission is involved;
- final delivery section;
- event stream entry;
- transcript entry or explicit missing-transcript blocker.

## 4. UI Requirements

Every failure must show:

- what failed;
- why it failed;
- whether anything was changed;
- what the user can do next;
- whether retry is safe;
- whether permission is needed;
- link or id for reviewer trace.

No failure may disappear into a generic assistant apology.

## 5. P0 Recovery Scenarios

| ID | Scenario | Required outcome |
| --- | --- | --- |
| R2-01 | Missing workspace file. | Missing-source blocker, no hallucinated file summary. |
| R2-02 | Web read while network disabled. | Policy blocker, no live/web success claim. |
| R2-03 | MCP target missing. | Missing MCP target blocker. |
| R2-04 | Model selects disallowed tool. | `model_selected_disallowed_tool`, no fallback execution. |
| R2-05 | Permission proposal denied. | Action remains unexecuted. |
| R2-06 | Permission proposal accepted. | Exact pending action resumes. |
| R2-07 | Safe read fails then retries. | Retry creates linked attempt and final result/blocker. |
| R2-08 | Cancel queued task. | No later queued actions execute. |
| R2-09 | Resume stale task. | Stale blocker or refresh path. |
| R2-10 | Plan step failure. | Step marked failed/blocked, review reflects it. |

## 6. Acceptance

Stage 2 recovery is ready only when:

- R2-01 through R2-10 pass in automated or manual dogfood evidence;
- each failure has visible user controls or terminal explanation;
- final delivery does not claim blocked/skipped/proposed work as completed;
- retry/resume/cancel behavior is linked to original task/action ids;
- no unsafe replay occurs after denial or stale state.
