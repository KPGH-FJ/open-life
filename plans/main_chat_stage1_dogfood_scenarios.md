# Main Chat Stage 1 Dogfood Scenario Matrix

> Date: 2026-06-18
> Scope: required real end-to-end dogfood scenarios for Stage 1
> Status: preparation artifact

## 1. Scenario Contract

Each scenario must be executable as one of:

- `chat_e2e`: user starts from Chat input and ordinary send/stream path.
- `seeded_task_control_e2e`: user starts from a seeded task/proposal/plan/memory
  state and uses the visible control surface.
- `opt_in_live_e2e`: user explicitly opts into external live provider execution.

Each scenario must record:

- scenario id;
- user prompt or seeded control action;
- expected route;
- expected actions/observations;
- expected UI states;
- expected final delivery sections;
- durable changes allowed;
- blocker behavior;
- live-provider requirement;
- non-fake rule.

## 2. Deterministic Default Scenarios

| Id | Priority | Type | User input or action | Expected route | Expected UI states | Final delivery | Durable changes allowed | Expected blocker | Seed dependency | Non-fake rule |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| D01 | P0 | `chat_e2e` | "What is the difference between a task and a proposal in OpenLife?" | DirectAnswer | answering, completed | completed_work, next_action | none | none | none | No action timeline without action. |
| D02 | P0 | `chat_e2e` | "Summarize `dogfood/project_brief.md`." | read_action | action_running, observation_ready, completed | completed_work, observations_used | none | none | project_brief.md | Must read seeded file. |
| D03 | P0 | `chat_e2e` | "Find what we discussed about memory rollback." | session.search | action_running, observation_ready, completed | completed_work, observations_used | none | none | seeded_chat_session | Must cite seeded session. |
| D04 | P0 | `chat_e2e` | "Use my current working preferences to answer how I should plan tomorrow." | DirectAnswer with memory context | answering, completed | completed_work, observations_used | none | none | accepted_memory | Must show active memory context source. |
| D05 | P0 | `chat_e2e` | "Search the fixture web source about the project policy and summarize it." | ReAct web fixture | action_running, observation_ready, completed | completed_work, observations_used | none | none | web_fixture | Fixture label must be visible; not external live. |
| D06 | P0 | `chat_e2e` | "Use the selected review skill to critique this weekly plan." | selected skill context | planning, completed | completed_work, observations_used | none | none | selected_skill + planning_notes.md | Unselected skills must not load. |
| D07 | P0 | `chat_e2e` | "Use the right MCP read source to answer the workspace policy question." | MCP read | planning, action_running, observation_ready, completed | completed_work, observations_used | none | none | read_only_mcp_manifest | Candidate and selected target must be visible. |
| D08 | P0 | `chat_e2e` | "Plan my week and execute the first safe read-only step." | Plan-Execute | planning, action_running, observation_ready, completed | completed_work, observations_used, next_action | plan session only | none | planning_notes.md | Plan draft alone is not completion. |
| D09 | P1 | `seeded_task_control_e2e` | Skip unsupported plan step from seeded plan. | plan control | planning, completed | skipped_work, next_action | plan step status | none | seeded_plan_session | Skip must require seeded revision. |
| D10 | P0 | `chat_e2e` | "Remember that I prefer morning deep work." | memory proposal | memory_candidate, permission_needed | proposals_created, pending_user_action | proposal only | none | none | No direct memory write. |
| D11 | P1 | `seeded_task_control_e2e` | Accept seeded pending memory proposal. | proposal control | memory_candidate, completed | durable_changes, completed_work | accepted memory | none | pending_memory_proposal | Accepted memory needs provenance. |
| D12 | P1 | `seeded_task_control_e2e` | Roll back seeded accepted memory. | memory rollback | memory_candidate, completed | durable_changes, completed_work | inactive memory state | none | accepted_memory_for_rollback | Rolled-back memory must leave active context. |
| D13 | P1 | `seeded_task_control_e2e` | Resume seeded blocked task after permission. | task resume | retry_available, completed | completed_work, next_action | task/action status | none | blocked_task_permission | Resume exact pending action only. |
| D14 | P1 | `seeded_task_control_e2e` | Retry seeded failed read action. | retry action | retry_available, observation_ready | completed_work, observations_used | retry action record | none | failed_read_action | Retry must preserve action scope. |
| D15 | P1 | `seeded_task_control_e2e` | Cancel seeded non-terminal task. | cancel task | blocked, completed | blocked_work, next_action | cancelled task state | none | non_terminal_task | Queued actions must stop. |
| D16 | P0 | `chat_e2e` | "Publish the seeded `policy_note.md` to the external destination named in the write-like action seed." | permission/blocker | permission_needed, blocked | blocked_work, pending_user_action | proposal/blocker only | permission_required | write_like_action | No dangerous write. |
| D17 | P0 | `chat_e2e` | "Use the seeded MCP read source to answer the workspace policy question, then explain why that tool was selected." | ReAct/tool trace | planning, completed | completed_work, observations_used | none | none | read_only_mcp_manifest | Selection reason must be from runtime metadata. |
| D18 | P0 | `chat_e2e` | "Use a skill that is not selected." | blocked | blocked, completed | blocked_work, next_action | none | unselected_skill_not_injected | unselected_sensitive_skill | Unselected skill content must not be injected. |
| D19 | P1 | `seeded_task_control_e2e` | Inspect final delivery for seeded mixed-outcome task. | final delivery read | completed | completed_work, proposed_work, blocked_work, skipped_work, next_action | none | none | terminal_mixed_task | Do not claim blocked work as done. |
| D20 | P1 | `seeded_task_control_e2e` | Reconnect and replay seeded task events. | event replay | replaying_events, observation_ready, completed | completed_work, next_action | none | none | seeded_event_stream | No duplicate events. |
| D21 | P0 | `chat_e2e` | "Compare two memory facts that conflict." | memory conflict | memory_candidate, completed | completed_work, observations_used | none | none | conflicting_memory_pair | No silent overwrite. |
| D22 | P0 | `chat_e2e` | "Answer using two different read sources." | multi-read ReAct | planning, action_running, observation_ready, completed | completed_work, observations_used | none | none | project_brief.md + memory/session seed | Must show two observations. |
| D23 | P0 | `chat_e2e` | "Use web while network policy blocks it." | web blocker | blocked | blocked_work, next_action | none | web_network_policy_blocked | network_disabled_policy | No fake web source. |
| D24 | P0 | `chat_e2e` | "Use MCP when no manifest exists." | MCP blocker | blocked | blocked_work, next_action | none | mcp_missing_read_target | missing_mcp_target | No fake MCP observation. |
| D25 | P0 | `chat_e2e` | "Inspect loaded knowledge assets." | context inspection | completed | completed_work, observations_used | none | none | knowledge_asset_files | Policy cannot be overridden by file text. |
| D26 | P0 | `chat_e2e` | "Propose an edit to USER.md for my planning preference." | knowledge proposal | memory_candidate, permission_needed | proposals_created, pending_user_action | proposal only | none | USER.md | No direct knowledge file write. |
| D27 | P1 | `seeded_task_control_e2e` | Recover from stale resume context. | stale blocker | blocked, retry_available | blocked_work, next_action | none | stale_context | stale_task_context | No automatic stale replay. |
| D28 | P1 | `seeded_task_control_e2e` | Audit what changed in a terminal task. | final delivery read | completed | completed_work, proposed_work, blocked_work, skipped_work, durable_changes, next_action | none | none | terminal_mixed_task | Durable change inventory must be exact. |
| D29 | P1 | `chat_e2e` | "Ask a simple personal planning question with no required tool." | DirectAnswer | answering, completed | completed_work | none | none | none | Must remain low-noise. |
| D30 | P1 | `chat_e2e` | "Summarize a seeded note and create a memory proposal if useful." | read + proposal | action_running, observation_ready, memory_candidate, completed | completed_work, proposals_created, pending_user_action | proposal only | none | planning_notes.md | Proposal must cite read evidence. |
| D31 | P1 | `chat_e2e` | "Plan the seeded policy-note publication task, but ask me before any risky external publish step." | Plan-Execute blocker | planning, permission_needed, blocked | blocked_work, pending_user_action | plan session/proposal only | permission_required | planning_notes.md + write_like_action | Risky step must not execute. |
| D32 | P1 | `chat_e2e` | "Use selected skill plus file read to review the seed plan." | skill + file read | planning, action_running, observation_ready, completed | completed_work, observations_used | none | none | selected_skill + planning_notes.md | Skill guides; file read proves action. |
| D33 | P1 | `chat_e2e` | "Find prior session context, then answer using current memory." | session + memory | action_running, observation_ready, completed | completed_work, observations_used | none | none | seeded_chat_session + accepted_memory | Must separate session evidence and memory context. |
| D34 | P1 | `chat_e2e` | "Create a proposal to change SOUL.md wording." | knowledge proposal | memory_candidate, permission_needed | proposals_created, pending_user_action | proposal only | none | SOUL.md | SOUL update must be proposal-first. |
| D35 | P1 | `seeded_task_control_e2e` | Deny seeded tool permission proposal. | permission control | blocked, completed | blocked_work, next_action | proposal denial only | none | pending_tool_permission | Denial must not execute action. |
| D36 | P1 | `seeded_task_control_e2e` | Defer seeded memory proposal. | proposal control | memory_candidate, completed | proposed_work, next_action | proposal status only | none | pending_memory_proposal | Deferred proposal remains pending. |

## 3. Opt-In Live Scenarios

These scenarios do not count toward default deterministic dogfood readiness.

| Id | Type | User input | Expected route | Required external evidence |
| --- | --- | --- | --- | --- |
| L01 | `opt_in_live_e2e` | "Answer this current provider-backed direct question." | DirectAnswer | external provider identity, model identity, non-empty response preview. |
| L02 | `opt_in_live_e2e` | "Use live web to read a current public page and summarize." | ReAct web | governed web action, observation, no fixture/local provider credit. |
| L03 | `opt_in_live_e2e` | "Select among registered MCP read candidates with live ranking." | ReAct MCP | provider-ranked candidate order, selected candidate, exact allowlist. |
| L04 | `opt_in_live_e2e` | "Request permission for a safe registered MCP proposal path." | ToolPermission proposal | pending proposal target, no overlapping MCP read success. |

## 4. Minimum Stage 1 Readiness Bar

Stage 1 default dogfood can be marked ready only if:

- all P0 deterministic scenarios execute and pass or expected-block correctly;
- all P1 deterministic scenarios execute and produce an explicit pass,
  expected-blocker, or non-blocking blocker row with accepted residual risk;
- no P1 scenario may be silently skipped or hidden from the readiness report;
- at least 20 start from Chat input;
- at least 8 exercise task controls from seeded state;
- every scenario has visible UI assertions;
- no deterministic scenario has silent write or hidden legacy fallback;
- opt-in live scenarios are reported separately.

Readiness interpretation:

- `defaultReady`: all P0 scenarios pass or expected-block correctly; every P1
  scenario has a structured evidence row; P1 blockers are allowed only when they
  are explicitly marked non-blocking and listed as accepted residual risk.
- `ready_for_engineering_dogfood`: `defaultReady=true`, required browser smoke
  passes, and P1 residual risks are documented.
- `ready_for_internal_trial`: P0 and required P1 manual sample have no blocker
  or major issue.
