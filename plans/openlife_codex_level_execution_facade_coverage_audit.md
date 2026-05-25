# OpenLife ExecutionFacade Coverage Audit

Date: 2026-05-25

Status: Codex-level coverage audit / migration boundary

Scope: code-fact audit only. This phase does not migrate Scheduled, Replay, Plan execution, Builder, Calibration, or Proposal replay paths.

## Current State Summary

- **Chat**: full Tauri ExecutionFacade path. `send_message_with_agent_loop_inner` resolves the required `AgentSpec`, builds `PromptBlockRegistry` and governed `ActionContext`, then calls `run_tauri_agent_task`. Governance errors fail closed; Runtime errors keep the existing fallback branch.
- **StreamChat**: full Tauri ExecutionFacade path. `start_stream_message_with_agent_loop` calls `run_tauri_agent_task` with a streaming callback. Governance errors emit `stream-message-error` only; Runtime errors remain eligible for fallback.
- **Scheduled**: assembly-only. `scheduler_runner.rs` reuses facade assembly helpers for loop config, `AgentSpec`, `NetworkPolicy`, `PromptStack`, sandbox, and `ActionContext`, but still calls `AgentLoop::run` directly.
- **Replay**: not migrated. Replay preserves original action/proposal permission semantics through `replay_action_internal` and direct `ActionExecutor` use.
- **Plan execution**: not migrated. Plan commands use `PlanExecutor` plus direct `ActionExecutor` because plan confirmation, review gates, deviation handling, and retry semantics are separate from Chat fallback.
- **Direct tool execution**: not migrated. `execute_tool_call_inner` uses direct `ActionExecutor` with required `AgentSpec`, `NetworkPolicy`, and sandbox context. This is a good small wrapper candidate, but not migrated in this phase.
- **Builder / Calibration**: not migrated. These paths have LifeModel proposal semantics and should wait for PromptStack and LifeModel Evolution end-to-end audit.

## Execution Entry Inventory

| Entry point | File | Current execution path | Uses ExecutionFacade? | Governance boundary present? | AgentSpec required? | NetworkPolicy required? | PromptStack required? | Fallback behavior | Migration recommendation | Risk level |
|---|---|---|---|---|---|---|---|---|---|---|
| `send_message_with_agent_loop_inner` | `src-tauri/src/lib.rs` | Builds governed loop/context, then calls `run_tauri_agent_task(Chat)` | yes | Typed Governance/Runtime errors; Governance fail-closed | yes | yes | yes | Runtime fallback only; Governance returns error | Keep locked with entry/source audit tests | Low |
| `start_stream_message_with_agent_loop` | `src-tauri/src/streaming.rs` | Builds governed loop/context, then calls `run_tauri_agent_task(StreamChat)` | yes | Typed Governance/Runtime errors; Governance emits error event only | yes | yes | yes | Runtime fallback can continue streaming; Governance does not emit chunk/done | Keep locked with source audit tests | Low |
| `execute_scheduled_task` | `src-tauri/src/scheduler_runner.rs` | Reuses assembly helpers, then calls `AgentLoop::run` directly | assembly-only | AgentSpec/PromptStack/ActionContext fail-closed before run | yes | yes | yes | No Chat fallback; scheduler records task failure | Candidate after scheduler lease/stale recovery tests | Medium |
| `replay_action_internal` / `replay_agent_action` | `src-tauri/src/commands/agent.rs` | Restores original run/action and calls direct `ActionExecutor` | no | Original AgentSpec, permission replay, and Replay events | yes, restored or resolved | yes | no model PromptStack | No Chat fallback; emits replay status/events | Do not migrate in next batch | High |
| `execute_agent_plan` / `retry_agent_plan` / plan execution core | `src-tauri/src/commands/plan.rs` | Uses `PlanExecutor` plus direct `ActionExecutor` | no | Plan confirmation/review/deviation gates | yes | yes through action context | no model PromptStack | Plan status/review/failure semantics; no Chat fallback | Consider later wrapper only after plan-specific tests | High |
| Proposal accept/replay helpers | `src-tauri/src/commands/proposal.rs` | Proposal status changes and direct replay/action closures | no | Proposal confirmation and permission replay are source of truth | context-dependent | context-dependent | no | Proposal status remains canonical; no Chat fallback | Do not migrate now | High |
| `execute_tool_call_inner` | `src-tauri/src/lib.rs` | Direct `ActionExecutor` command path | no | Required AgentSpec plus governed `ActionContext` | yes | yes | no | Returns command error; no fallback | Good next minimal wrapper candidate | Medium |
| `run_skill` | `src-tauri/src/commands/execution.rs` | Skill runtime plus scheduler-governed generation | no | AgentSpec fail-closed and PromptStack assembly | yes | model route governed; no direct ActionContext for every step | yes | Skill parse/generation error handling, no Chat fallback | Audit separately; not next by default | Medium |
| `build_runtime_assembly_config`, `build_governed_agent_loop`, `build_governed_action_context` | `src-tauri/src/execution_facade.rs`, `src-tauri/src/execution_deps.rs` | Shared construction helpers | assembly-only | No standalone boundary; callers decide | caller-provided | caller-provided | caller-provided | none | Keep as internal assembly surface | Low |
| `run_tauri_agent_task` | `src-tauri/src/execution_facade.rs` | Tauri facade entrypoint for migrated AgentLoop modes | yes | Enforces required `AgentSpec`, `NetworkPolicy`, and `PromptRegistry` for Chat/StreamChat | yes | yes | yes | Caller decides Runtime fallback; Governance is typed error | Extend only through deliberate migration | Medium |
| `ActionExecutor` core | `openlife-core/src/agent/action_executor/` | Core tool/action executor | no, core engine | Enforces permissions/sandbox/network through `ActionContext` | context-dependent | context-dependent | no | Returns tool/action result or block | Do not migrate; wrap Tauri entrypoints instead | Medium |
| `PlanExecutor` core | `openlife-core/src/agent/plan_executor.rs` | Core plan execution engine | no, core engine | Plan review and confirmation semantics | supplied by caller | supplied by action callback | no | Plan outcomes and status transitions | Do not treat as Tauri facade target itself | Medium |
| `SubAgentRuntime` | `openlife-core/src/agent/sub_agent.rs` | Core sub-agent call-as-tool runtime | no, core engine | SubAgentSpec validation and child run trace | via SubAgentSpec | no direct network boundary | no | Child run/observation semantics | Audit with SubAgent roadmap, not this phase | Medium |
| Builder / Calibration commands | `src-tauri/src/commands/builder.rs`, `src-tauri/src/commands/calibration.rs` | LifeModel/proposal-oriented flows | no | Proposal review and LifeModel safeguards | mixed | no full ActionContext by default | mixed | No Chat fallback | Defer until PromptStack/LifeModel Evolution audit | High |

## Not Migrated In This Phase

- **Replay**: replay has independent proposal confirmation, permission replay, and original-action restoration semantics. Sending this through Chat fallback would hide whether the replayed action was allowed, denied, or blocked.
- **Scheduled**: scheduled execution already reuses assembly helpers, but it also needs scheduler-specific lease handling, stale-running recovery, and background failure observability. It should not inherit Chat fallback semantics.
- **Plan execution**: plan execution owns confirmation, review, deviation, retry, and step-result semantics. A facade wrapper must preserve those contracts before any migration.
- **Builder / Calibration**: these paths mutate or propose LifeModel changes through distinct proposal semantics. They should wait for the PromptStack and LifeModel Evolution end-to-end audit.
- **Proposal accept/replay**: proposal state is the source of truth. These paths must not be folded into a generic Chat degradation path.
- **Core executors**: `ActionExecutor`, `PlanExecutor`, and `SubAgentRuntime` are core engines, not Tauri user-facing entrypoints. Migration should wrap Tauri entrypoints, not the engines themselves.

## Next Batch Migration Recommendations

### Candidate 1: Direct Tool Execution Facade Wrapper

- **Why prioritize**: it is narrow, non-streaming, and already has explicit `AgentSpec`, `NetworkPolicy`, and `ExecutionSandbox` context. It does not need model prompt assembly or Chat fallback.
- **Risk**: the frontend command result shape and tool audit behavior must remain unchanged. Permission denial and sandbox denial must not be softened into Runtime fallback.
- **Required tests first**: missing `AgentSpec` fail-closed, NetworkPolicy required, sandbox denial remains blocked, no fallback run/warning, command response schema unchanged.
- **Do not do**: do not add Chat fallback, do not change `ToolCallResult`, and do not route Proposal replay through the wrapper.

### Candidate 2: Scheduled Proactive Execution Facade Hardening

- **Why prioritize**: scheduled execution is already assembly-only and therefore close to the facade boundary. Hardening it would remove one direct AgentLoop call without touching proposal replay or plan semantics.
- **Risk**: scheduler lease lifecycle, stale-running recovery, and background observability are more important than Chat-style degradation.
- **Required tests first**: lease is not held during model execution, stale running task recovery still works, missing `AgentSpec` fails closed, and scheduled failures record clear task/event state.
- **Do not do**: do not migrate Replay, Plan, Builder, or Calibration at the same time; do not use Chat fallback for scheduled failures.

## Audit Tests

This phase adds lightweight source-audit tests to lock the completed Chat and StreamChat convergence:

- `execution_facade_chat_path_uses_facade_entrypoint`
- `execution_facade_stream_chat_path_uses_facade_entrypoint`

These tests scan only the relevant production entrypoint bodies and assert that Chat/StreamChat contain `run_tauri_agent_task` while avoiding direct `AgentLoop::run` / `AgentLoop::run_streaming` calls in those entrypoint slices. They complement the existing entrance-level `send_message_with_agent_loop` fail-closed tests and the streaming Governance error event tests.
