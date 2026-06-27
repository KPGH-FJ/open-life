# Main Chat Agent Status UI Contract

> Date: 2026-06-25
> Status: preparation artifact before Agent status product UI work
> Parent: `plans/main_chat_next_6_steps_master_spec.md`

## 1. Purpose

This contract defines how Main Chat should expose Agent status to users after
the runtime evidence is stable. The UI must help users answer:

- Did the task finish?
- Is it waiting for me?
- Is it blocked?
- What did it actually do?
- What can I safely do next?

The UI must not require users to inspect developer diagnostics to understand
basic task state.

## 2. Current Baseline

Current product surface:

- `ReasoningTracePanel` can render structured runtime evidence from
  `generation_result`.
- `ReasoningTracePanel` is currently displayed only when
  `showMainChatDiagnostics` is enabled.
- `ChatPage` has a Task continuity surface that can list task status, blockers,
  actions, proposals, and final delivery.
- These surfaces are valuable, but they still read as diagnostic/control-plane
  UI rather than a simple default user-facing status system.

## 3. Status Vocabulary

Default-visible statuses:

| Status | Meaning | Primary source |
| --- | --- | --- |
| `completed` | The answer or task delivery is complete according to task/run/final-delivery evidence. | task session, agent run, final delivery, transcript |
| `waiting_for_user` | The Agent needs user confirmation, proposal review, or permission approval. | action queue, proposal store, task session |
| `restricted` | Policy or safety prevented execution. | blocker codes, tool policy, provider preflight |
| `blocked` | The task cannot progress without a specific recovery/control action. | task session, action queue, transcript |
| `trace_gap` | Required evidence is missing, so the Agent cannot make a confident state claim. | runtime fact trace gap |
| `proposal_pending` | A response may be delivered, but durable Memory/LifeModel/file change is still pending review. | proposal store, task session |
| `permission_pending` | A tool/action is pending explicit permission. | action queue, ToolPermission proposal, task session |
| `running` | A task is actively executing or streaming. | current event stream/task state |

Internal statuses such as raw strategy names, digest labels, exact run IDs, and
candidate IDs should stay in expanded trace unless the user explicitly opens
developer diagnostics.

## 4. Default UI Requirements

Default UI should show:

- one primary status chip;
- one source chip when the answer uses runtime facts, tool observation, model
  generation, proposal state, or blocker state;
- one short next-action label when a user action exists;
- proposal and permission affordances when waiting for user;
- concise blocked reason using bounded labels;
- trace-gap copy that says evidence is missing instead of inventing history.

Default UI should not show:

- raw system prompt;
- raw LifeModel or Memory content;
- provider key, endpoint secret, or credential value;
- raw MCP manifest id/description;
- absolute workspace path;
- full internal digest list;
- unbounded transcript metadata;
- unsupported action buttons.

## 5. Expanded Trace Requirements

Expanded trace may show:

- `runtimeFactKeys`;
- `runtimeFactSource`;
- source/authority/freshness/privacy;
- provider route distinctions;
- tool availability distinctions;
- task/run/delivery status;
- pending counts;
- bounded blocker labels;
- bounded action and observation summaries;
- trace gap code;
- selected skill and context source summary when bounded.

Expanded trace must read structured fields only. It must not parse assistant
prose to infer task completion, pending permission, proposal state, or tool
availability.

## 6. UI Mapping

| Backend evidence | Default chip | Next action | Expanded trace |
| --- | --- | --- | --- |
| `completedResponse=true`, no pending proposal/permission | Completed | None or follow-up suggestion | task/run/final-delivery evidence |
| `pendingProposalCount>0` | Proposal pending | Review proposal | proposal count, durable status |
| `pendingPermissionCount>0` | Permission pending | Review permission | permission count, bounded target |
| `taskStatus=blocked` | Blocked | Retry, refresh, cancel, or none | blocker code and control eligibility |
| `runtimeFactTraceGap=true` | Unknown | Refresh context or inspect task | trace gap code |
| web policy blocked | Restricted | Configure/enable policy if appropriate | policy blocker |
| MCP no safe read candidate | Restricted | Configure MCP if appropriate | safe read candidate count |
| model-generated answer | Model response | None | actual provider/model route |
| runtime clock answer | Local runtime | None | clock key/source/timezone |

## 7. Product Acceptance Cases

| ID | Scenario | Expected UI |
| --- | --- | --- |
| UI-S1 | User asks "今天星期几". | Default answer shows local runtime source; expanded trace shows clock keys. |
| UI-S2 | User asks "你现在用什么模型". | Default answer distinguishes current/last/configured/planned route where relevant. |
| UI-S3 | User asks whether a completed task finished. | Default status says completed with no proposal/permission pending. |
| UI-S4 | User asks after a pending proposal. | Default status says proposal pending, not durable complete. |
| UI-S5 | User asks after pending permission. | Default status says permission pending and offers review action. |
| UI-S6 | User asks after blocked task. | Default status says blocked/restricted and offers safe next control. |
| UI-S7 | User asks last action with missing evidence. | Default status says unknown/trace gap and does not invent history. |
| UI-S8 | Developer expands trace. | Structured evidence appears without raw prompt, raw memory, keys, or raw manifest. |

## 8. Stop Conditions

Stop UI work if:

- backend evidence required for a default chip does not exist;
- UI code needs to parse assistant prose to know state;
- UI would expose sensitive raw metadata by default;
- a control button can call an action not allowed by backend task controls;
- product copy claims durable completion while proposal or permission is pending.
