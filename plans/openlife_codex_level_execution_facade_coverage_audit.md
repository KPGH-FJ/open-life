# OpenLife ExecutionFacade Coverage Audit

Date: 2026-05-25

Status: Codex-level coverage audit / Proposal Replay-Replay hardening preparation complete

Scope: code-fact audit plus Proposal Replay / Replay safety-net hardening. Replay, Plan execution, Builder, Calibration, and Skill runtime paths remain unmigrated.

## Current State Summary

- **Chat**: full Tauri ExecutionFacade path. `send_message_with_agent_loop_inner` resolves the required `AgentSpec`, builds `PromptBlockRegistry` and governed `ActionContext`, then calls `run_tauri_agent_task`. Governance errors fail closed; Runtime errors keep the existing fallback branch.
- **StreamChat**: full Tauri ExecutionFacade path. `start_stream_message_with_agent_loop` calls `run_tauri_agent_task` with a streaming callback. Governance errors emit `stream-message-error` only; Runtime errors remain eligible for fallback.
- **Scheduled**: Scheduled-specific Tauri ExecutionFacade wrapper migrated. `scheduler_runner.rs` still owns task claim/complete/failed file merge semantics, but execution now calls `run_tauri_scheduled_execution`, which builds the Scheduled governed loop/context through facade assembly helpers and wraps the internal `AgentLoop::run` call. This is not the Chat facade and does not inherit Chat fallback. Runtime failures that return an `AgentLoopResult` now persist the failed `AgentRun` before returning a typed Runtime error with `run_id`.
- **Replay**: hardening preparation complete, not migrated. Replay preserves original action/proposal permission semantics through `replay_action_internal` and direct `ActionExecutor` use. Safety-net tests now cover missing `AgentSpec`, original `AgentSpec` restoration, no tool-scope escalation, ToolPermission deny, NetworkPolicy hard deny, ExecutionSandbox hard deny, typed replay payload contract, and no Chat fallback.
- **Plan execution**: not migrated. Plan commands use `PlanExecutor` plus direct `ActionExecutor` because plan confirmation, review gates, deviation handling, and retry semantics are separate from Chat fallback.
- **Direct tool execution**: facade wrapper migrated. `execute_tool_call_inner` now resolves the required `AgentSpec`, builds governed Tauri-side runtime assembly/context, and calls `run_tauri_direct_tool_execution`. The wrapper preserves the existing `ToolCallResult` command shape and uses direct `ActionExecutor` internally without Chat fallback semantics.
- **Builder / Calibration**: not migrated. These paths have LifeModel proposal semantics and should wait for PromptStack and LifeModel Evolution end-to-end audit.

## Execution Entry Inventory

| Entry point | File | Current execution path | Uses ExecutionFacade? | Governance boundary present? | AgentSpec required? | NetworkPolicy required? | PromptStack required? | Fallback behavior | Migration recommendation | Risk level |
|---|---|---|---|---|---|---|---|---|---|---|
| `send_message_with_agent_loop_inner` | `src-tauri/src/lib.rs` | Builds governed loop/context, then calls `run_tauri_agent_task(Chat)` | yes | Typed Governance/Runtime errors; Governance fail-closed | yes | yes | yes | Runtime fallback only; Governance returns error | Keep locked with entry/source audit tests | Low |
| `start_stream_message_with_agent_loop` | `src-tauri/src/streaming.rs` | Builds governed loop/context, then calls `run_tauri_agent_task(StreamChat)` | yes | Typed Governance/Runtime errors; Governance emits error event only | yes | yes | yes | Runtime fallback can continue streaming; Governance does not emit chunk/done | Keep locked with source audit tests | Low |
| `execute_scheduled_task` | `src-tauri/src/scheduler_runner.rs` | Loads scheduler dependencies, resolves required `AgentSpec`, then calls `run_tauri_scheduled_execution` | yes, Scheduled-specific wrapper | Typed Governance/Runtime errors; Runtime may carry failed `run_id`; scheduler converts both to failed task errors | yes | yes | yes | No Chat fallback; scheduler records task failure and `agent_run_id` when runtime provides one | Keep locked; next candidate should be Proposal replay / Replay / Plan / Builder / Calibration | Low |
| `replay_action_internal` / `replay_agent_action` | `src-tauri/src/commands/agent.rs` | Restores original run/action and calls direct `ActionExecutor` | no | Original AgentSpec, ToolPermission peek, NetworkPolicy, ExecutionSandbox, and Replay typed events | yes, restored from `run.agent_spec_id` or plan-bound spec; missing spec fails closed | yes, from current governed config/context | no model PromptStack | No Chat fallback; emits `ReplayStarted` / `ReplayCompleted` / `ReplayFailed` | Next migration may only be a Replay-specific wrapper; must not use Chat facade | High |
| `execute_agent_plan` / `retry_agent_plan` / plan execution core | `src-tauri/src/commands/plan.rs` | Uses `PlanExecutor` plus direct `ActionExecutor` | no | Plan confirmation/review/deviation gates | yes | yes through action context | no model PromptStack | Plan status/review/failure semantics; no Chat fallback | Consider later wrapper only after plan-specific tests | High |
| Proposal accept/replay helpers | `src-tauri/src/commands/proposal.rs` | `accept_proposal_with_state` applies Proposal state only, marks accepted after successful apply, and returns `continue_run_id` / `continue_action_id` for replay lookup; it does not execute Chat fallback | no | Proposal status remains source of truth; ToolPermission replay lookup matches pending action tool/source/risk/action_type/step | context-dependent; replay itself requires original/restored spec | context-dependent; replay itself enforces current NetworkPolicy | no | Proposal status remains canonical; failed replay does not invent success | Next migration may only call a Replay-specific wrapper after Proposal apply succeeds | High |
| `execute_tool_call_inner` | `src-tauri/src/lib.rs` | Tauri direct tool facade wrapper, internally direct `ActionExecutor` | yes | Required AgentSpec plus governed `ActionContext`; wrapper verifies AgentSpec and NetworkPolicy | yes | yes | no | Returns command error/result; no Chat fallback | Keep locked with direct-tool source and behavior tests | Low |
| `run_skill` | `src-tauri/src/commands/execution.rs` | Skill runtime plus scheduler-governed generation | no | AgentSpec fail-closed and PromptStack assembly | yes | model route governed; no direct ActionContext for every step | yes | Skill parse/generation error handling, no Chat fallback | Audit separately; not next by default | Medium |
| `build_runtime_assembly_config`, `build_governed_agent_loop`, `build_governed_action_context` | `src-tauri/src/execution_facade.rs`, `src-tauri/src/execution_deps.rs` | Shared construction helpers | assembly-only | No standalone boundary; callers decide | caller-provided | caller-provided | caller-provided | none | Keep as internal assembly surface | Low |
| `run_tauri_agent_task` | `src-tauri/src/execution_facade.rs` | Tauri facade entrypoint for migrated AgentLoop modes | yes | Enforces required `AgentSpec`, `NetworkPolicy`, and `PromptRegistry` for Chat/StreamChat | yes | yes | yes | Caller decides Runtime fallback; Governance is typed error | Extend only through deliberate migration | Medium |
| `ActionExecutor` core | `openlife-core/src/agent/action_executor/` | Core tool/action executor | no, core engine | Enforces permissions/sandbox/network through `ActionContext` | context-dependent | context-dependent | no | Returns tool/action result or block | Do not migrate; wrap Tauri entrypoints instead | Medium |
| `PlanExecutor` core | `openlife-core/src/agent/plan_executor.rs` | Core plan execution engine | no, core engine | Plan review and confirmation semantics | supplied by caller | supplied by action callback | no | Plan outcomes and status transitions | Do not treat as Tauri facade target itself | Medium |
| `SubAgentRuntime` | `openlife-core/src/agent/sub_agent.rs` | Core sub-agent call-as-tool runtime | no, core engine | SubAgentSpec validation and child run trace | via SubAgentSpec | no direct network boundary | no | Child run/observation semantics | Audit with SubAgent roadmap, not this phase | Medium |
| Builder / Calibration commands | `src-tauri/src/commands/builder.rs`, `src-tauri/src/commands/calibration.rs` | LifeModel/proposal-oriented flows | no | Proposal review and LifeModel safeguards | mixed | no full ActionContext by default | mixed | No Chat fallback | Defer until PromptStack/LifeModel Evolution audit | High |

## Not Migrated In This Phase

- **Replay**: replay has independent proposal confirmation, permission replay, original `AgentSpec` restoration, original tool source/target/scope preservation, and typed replay event semantics. Sending this through Chat fallback would hide whether the replayed action was allowed, denied, or blocked.
- **Plan execution**: plan execution owns confirmation, review, deviation, retry, and step-result semantics. A facade wrapper must preserve those contracts before any migration.
- **Builder / Calibration**: these paths mutate or propose LifeModel changes through distinct proposal semantics. They should wait for the PromptStack and LifeModel Evolution end-to-end audit.
- **Proposal accept/replay**: proposal state is the source of truth. `accept_proposal_with_state` may expose continuation ids for a pending action, but it must not perform a fake replay success or mark replay outcome through ExecutionFacade. These paths must not be folded into a generic Chat degradation path.
- **Core executors**: `ActionExecutor`, `PlanExecutor`, and `SubAgentRuntime` are core engines, not Tauri user-facing entrypoints. Migration should wrap Tauri entrypoints, not the engines themselves.

## Next Batch Migration Recommendations

### Completed Candidate: Direct Tool Execution Facade Wrapper

- **Status**: migrated to a Tauri-side direct tool facade wrapper.
- **Boundary**: this is not a Chat fallback path. `run_tauri_direct_tool_execution` creates/persists the direct tool audit run and maps the action result back to the existing `ToolCallResult` shape; permission, sandbox, and network denials remain blocked/error tool outcomes.
- **Locked tests**: missing `AgentSpec` fail-closed with no `AgentRun`; source audit proves `execute_tool_call_inner` calls the direct tool facade and no longer constructs/calls `ActionExecutor` directly; result-shape test covers `goal.read`; sandbox and network denials assert no fallback warning and no `FallbackStarted` / `FallbackCompleted` events.
- **Still out of scope**: Proposal replay remains not migrated. Plan, Builder, and Calibration remain not migrated.

### Completed Candidate: Scheduled Proactive Execution Facade Wrapper

- **Status**: migrated to `run_tauri_scheduled_execution`; failed-run observability fixed.
- **Boundary**: this is not Chat/StreamChat `run_tauri_agent_task`. The wrapper keeps Scheduled `Planner` role, write-disabled config, restricted toolset, governed `NetworkPolicy`, `ExecutionSandbox`, `AgentSpec`, and `PromptBlockRegistry`. It may call `AgentLoop::run` internally, but `scheduler_runner.rs` no longer calls it directly.
- **Scheduler ownership**: `scheduler_runner.rs` still owns scheduler lease, stale-running recovery, terminal-task non-reclaim, and task file outcome merge. The facade returns `run_id`, output, and `result_preview`; it does not write `scheduled_tasks.json`.
- **Failure semantics**: missing `AgentSpec`, missing/mismatched `NetworkPolicy`, and missing `PromptBlockRegistry` fail closed as typed Governance errors before run creation and do not write `agent_run_id`. Runtime failures that have an `AgentLoopResult` persist the failed `AgentRun` and return typed Runtime errors carrying `run_id`. The scheduler maps failures to task `failed` with a readable error, without `completed_at` or `result_preview`, and writes `agent_run_id` only when the facade error provides one.
- **Do not do**: do not migrate Replay, Plan, Builder, or Calibration at the same time; do not use Chat fallback for scheduled failures.

### Completed Preparation: Proposal Replay / Replay Hardening

- **Status**: safety-net hardening complete; formal migration not done.
- **Entrypoints audited**: `accept_proposal_with_state` in `src-tauri/src/commands/proposal.rs` applies Proposal state and returns replay continuation metadata; `replay_agent_action` / `replay_action_internal` in `src-tauri/src/commands/agent.rs` perform direct action replay.
- **Direct ActionExecutor use retained**: `replay_action_internal` still constructs `ActionExecutor` directly. Plan execution in `src-tauri/src/commands/plan.rs` and core/test paths still use `ActionExecutor` directly. This is intentional for this batch.
- **Locked governance semantics**: missing original `AgentSpec` fails closed before execution; replay restores the original run/plan-bound `AgentSpec`; original tool source, target, risk, action type, and capabilities are preserved; ToolPermission deny records `ReplayFailed`; NetworkPolicy and ExecutionSandbox denials return blocked replay outcomes with typed reasons and no success status.
- **Typed events**: `ReplayStarted` / `ReplayCompleted` / `ReplayFailed` payloads now include stable `run_id`, `original_run_id`, `action_id`, `replay_of_action_id`, `proposal_id`, `agent_spec_id`, and typed reason fields where applicable. Tests assert no raw prompt/context leakage.
- **No Chat fallback**: source audit locks that Replay does not call `run_tauri_agent_task(Chat/StreamChat)` or `handle_agent_loop_fallback`, and behavior tests assert no `FallbackStarted` / `FallbackCompleted` events.
- **Next migration constraint**: if migration proceeds, it must be a Replay-specific facade/wrapper that preserves Proposal status and replay governance. It must not use Chat facade, StreamChat facade, or Chat fallback.

### Remaining Candidates: Plan / Builder / Calibration / Skill Runtime

- **Proposal replay / Replay**: ready for a future Replay-specific wrapper migration after review.
- **Plan**: needs a plan-specific wrapper only after confirmation/review/deviation/retry tests are locked.
- **Builder / Calibration**: should wait for PromptStack and LifeModel Evolution audit because they own LifeModel proposal semantics.
- **Skill runtime**: still not migrated; audit separately because it mixes model generation, parsing, and skill execution semantics.

## Audit Tests

This phase adds lightweight source-audit tests to lock the completed Chat and StreamChat convergence:

- `execution_facade_chat_path_uses_facade_entrypoint`
- `execution_facade_stream_chat_path_uses_facade_entrypoint`

These tests scan only the relevant production entrypoint bodies and assert that Chat/StreamChat contain `run_tauri_agent_task` while avoiding direct `AgentLoop::run` / `AgentLoop::run_streaming` calls in those entrypoint slices. They complement the existing entrance-level `send_message_with_agent_loop` fail-closed tests and the streaming Governance error event tests.

Scheduled migration adds scheduler/facade-focused tests whose names include `scheduled`, `scheduler`, or `execution_facade` for targeted filtering:

- `scheduler_lease_is_short_and_complete_merges_interleaved_pending_task`
- `scheduler_completed_and_failed_tasks_are_not_reclaimed_even_if_old`
- `scheduled_failure_observability_records_readable_error_without_completion`
- `scheduled_missing_agentspec_fails_closed_without_chat_fallback`
- `scheduled_missing_agentspec_records_scheduler_task_failure`
- `scheduled_successful_outcome_records_facade_preview_and_run_id`
- `scheduled_runtime_failure_records_scheduler_task_failure_with_run_id`
- `scheduled_execution_uses_scheduled_facade_wrapper_without_chat_fallback`
- `execution_facade_scheduled_path_uses_scheduled_wrapper`
- `execution_facade_scheduled_assembly_carries_network_policy_and_sandbox`
- `execution_facade_scheduled_mode_is_not_migrated_to_chat_task_entrypoint`
- `scheduled_facade_preserves_restricted_toolset`
- `scheduled_facade_requires_agent_spec`
- `scheduled_facade_requires_network_policy`
- `scheduled_facade_failed_run_persistence_on_runtime_failure`
- `scheduled_facade_prompt_stack_runtime_error_does_not_fallback`
- `scheduled_facade_governance_error_kind`
- `scheduled_facade_runtime_error_kind`

These tests prove migration without broadening semantics: `execute_scheduled_task` calls the Scheduled wrapper, does not directly call `.run(`, does not call `run_tauri_agent_task`, and contains no Chat fallback events. Scheduler failures still remain scheduler task failures. PromptStack runtime-error coverage is now named explicitly; no test claims sandbox denial unless it reaches the sandbox gate.

Proposal Replay / Replay hardening adds and strengthens the following tests:

- `replay_missing_agent_spec_fails_closed`: now also asserts no action execution, no action success rewrite, `ReplayFailed`, no fallback events, and Proposal status remains canonical.
- `replay_restores_original_agent_spec`
- `accepted_tool_permission_replay_still_checks_agent_spec`
- `replay_does_not_escalate_tool_scope`
- `replay_permission_not_authorized_records_typed_replay_failed_event`: now also asserts no fallback and no action rewrite.
- `replay_network_policy_denied_fails_closed_without_fallback`
- `replay_execution_sandbox_denied_fails_closed_without_success_outcome`
- `replay_typed_event_payload_contract_is_stable_and_redacted`
- `execution_facade_replay_path_is_not_migrated_to_chat_task_entrypoint`

This batch deliberately did not add a Replay facade wrapper and did not migrate Plan, Builder, Calibration, or Skill runtime.
