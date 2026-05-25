# OpenLife ExecutionFacade Coverage Audit

Date: 2026-05-25

Status: Codex-level coverage audit / Skill-specific prompt migrated; Skill lifecycle remains outside ExecutionFacade

Scope: code-fact audit plus Skill runtime safety net. Chat / StreamChat, Direct Tool, Scheduled, Replay, and Plan wrappers remain as previously migrated. Skill-specific prompt has migrated to PromptStack. Builder and Calibration remain unmigrated; Skill runtime lifecycle, JSON parsing, proposal writes, and ExecutionFacade integration remain outside this migration.

PromptStack-specific coverage is tracked in [`openlife_prompt_stack_coverage_audit.md`](openlife_prompt_stack_coverage_audit.md). That matrix is the source of truth for whether an entrypoint is PromptStack-governed, intentionally legacy/ad hoc, or not applicable because no model prompt is assembled.

## Current State Summary

- **Chat**: full Tauri ExecutionFacade path. `send_message_with_agent_loop_inner` resolves the required `AgentSpec`, builds `PromptBlockRegistry` and governed `ActionContext`, then calls `run_tauri_agent_task`. Governance errors fail closed; Runtime errors keep the existing fallback branch.
- **StreamChat**: full Tauri ExecutionFacade path. `start_stream_message_with_agent_loop` calls `run_tauri_agent_task` with a streaming callback. Governance errors emit `stream-message-error` only; Runtime errors remain eligible for fallback.
- **Scheduled**: Scheduled-specific Tauri ExecutionFacade wrapper migrated and event creation validated. `scheduler_runner.rs` still owns task claim/complete/failed file merge semantics, but execution now calls `run_tauri_scheduled_execution`, which builds the Scheduled governed loop/context through facade assembly helpers and wraps the internal `AgentLoop::run` call. This is not the Chat facade and does not inherit Chat fallback. Successful `AgentLoopResult`s produce a persisted `AgentRun`, normal AgentRunEvents (`run.created`, `agent_spec.selected`, prompt/context/model events as reached, and `run.completed`), and scheduler task `completed` with `agent_run_id` plus `result_preview`. Runtime failures that return an `AgentLoopResult` persist the failed `AgentRun`, write normal failure events (`model.call_failed` and/or `run.failed` depending on failure point), and return a typed Runtime error with `run_id`; the scheduler writes task `failed`, a readable error, and `agent_run_id` without `completed_at` or success preview. Missing `AgentSpec` fails before run creation, so only scheduler task failure/status is available.
- **Proactive suggestions**: `commands/proactive.rs` / `openlife-core/src/proactive.rs` generate suggestions only. They do not execute AgentLoop, do not create AgentRun, do not write AgentRunEvent, and do not call Chat fallback. Accepted scheduled/proactive executions enter through `scheduler_runner.rs` and the Scheduled-specific wrapper above.
- **Replay**: Replay-specific Tauri ExecutionFacade wrapper migrated. `replay_action_internal` still owns run/action lookup, original `AgentSpec` restoration, pre-execution ToolPermission fail-closed checks, `ReplayStarted` / `ReplayCompleted` / `ReplayFailed` events, Proposal-derived continuation semantics, and `AgentRun` updates. The actual action execution now calls `run_tauri_replay_execution`, which verifies the restored `AgentSpec`, `ActionContext`, `NetworkPolicy`, and sandbox-governed context before executing the original action. This is not the Chat facade, does not call `run_tauri_agent_task`, and does not inherit Chat fallback. ExecutionFacade does not write Proposal status; Proposal / replay callers remain the source of truth.
- **Plan execution**: Plan-specific Tauri ExecutionFacade wrapper migrated. `execute_agent_plan` / `retry_agent_plan` call `run_tauri_plan_execution`, which resolves the plan-bound `AgentSpec` or stored default fail-closed, builds the governed `ActionContext` with `NetworkPolicy`, `ExecutionSandbox`, ToolPermission store, memory/proposal/run/event stores, and executes steps through core `PlanExecutor`. The wrapper preserves plan confirmation, retry reset ordering, review gate, deviation, status, and trace semantics. It is not the Chat facade and does not call Chat fallback.
- **Direct tool execution**: facade wrapper migrated. `execute_tool_call_inner` now resolves the required `AgentSpec`, builds governed Tauri-side runtime assembly/context, and calls `run_tauri_direct_tool_execution`. The wrapper preserves the existing `ToolCallResult` command shape and uses direct `ActionExecutor` internally without Chat fallback semantics.
- **Builder / Calibration**: not migrated. This phase added proposal-store/status/event safety tests only. `builder_create_proposals` creates Review Center proposals and `proposal.created` events for accepted/edited review decisions without writing LifeModel. `apply_calibration` now defaults to the Proposal-first path, and `calibration_create_proposals` creates patchable scalar LifeModel proposals plus `proposal.created` events; actual LifeModel mutation is locked to Proposal acceptance status. The Calibration frontend formal path now creates Review Center proposals and no longer exposes a default direct-apply button. These events carry proposal metadata only and exclude raw prompts, `before`/`after` values, and full LifeModel content. Legacy/direct apply paths still exist only as explicit, default-disabled compatibility/debug/test paths and are documented below.
- **Skill runtime**: Skill-specific prompt migrated; Skill lifecycle remains outside ExecutionFacade and Chat facade. `src-tauri/src/commands/execution.rs::run_skill` now delegates to `run_skill_with_state`; the Skill-specific prompt itself is PromptStack-governed through SkillManifest-derived contract PromptBlocks appended to an effective AgentSpec before `AgentRuntime::execute_task_with_spec`. Raw user input is a normal user message, so it cannot influence Skill contract block boundaries; SummaryOnly cloud routing keeps only the non-sensitive contract and filters raw user/context content. The remaining path still owns `InferenceScheduler::generate_governed` skill JSON generation, skill envelope parsing/validation, and Review Center proposal creation. It does not call `run_tauri_agent_task`, `handle_agent_loop_fallback`, Scheduled/Replay/Plan wrappers, or Chat fallback. The frontend direct caller is `frontend/src/tauri.ts::runSkill`; `WorkspaceOverview` expects success to return `{ runId, status, summary, generatedProposals }` and currently has no custom failure UI beyond the rejected invoke.

## Execution Entry Inventory

| Entry point | File | Current execution path | Uses ExecutionFacade? | Governance boundary present? | AgentSpec required? | NetworkPolicy required? | PromptStack required? | Fallback behavior | Migration recommendation | Risk level |
|---|---|---|---|---|---|---|---|---|---|---|
| `send_message_with_agent_loop_inner` | `src-tauri/src/lib.rs` | Builds governed loop/context, then calls `run_tauri_agent_task(Chat)` | yes | Typed Governance/Runtime errors; Governance fail-closed | yes | yes | yes | Runtime fallback only; Governance returns error | Keep locked with entry/source audit tests | Low |
| `start_stream_message_with_agent_loop` | `src-tauri/src/streaming.rs` | Builds governed loop/context, then calls `run_tauri_agent_task(StreamChat)` | yes | Typed Governance/Runtime errors; Governance emits error event only | yes | yes | yes | Runtime fallback can continue streaming; Governance does not emit chunk/done | Keep locked with source audit tests | Low |
| `execute_scheduled_task` | `src-tauri/src/scheduler_runner.rs` | Loads scheduler dependencies, resolves required `AgentSpec`, then calls `run_tauri_scheduled_execution` | yes, Scheduled-specific wrapper | Typed Governance/Runtime errors; Runtime may carry failed `run_id`; scheduler converts both to failed task errors | yes | yes | yes | No Chat fallback; scheduler records task failure and `agent_run_id` when runtime provides one | Keep locked; remaining candidates are Builder / Calibration / Skill runtime | Low |
| `get_proactive_suggestions` / `ProactiveEngine::generate_suggestions` | `src-tauri/src/commands/proactive.rs`, `openlife-core/src/proactive.rs` | Loads LifeModel/proposal counts and returns declarative suggestions | no execution | Suggestion-only; no AgentRun or AgentRunEvent | no | no | no | No Chat fallback; no AgentLoop execution | Keep suggestion-only source audit locked | Low |
| `replay_action_internal` / `replay_agent_action` | `src-tauri/src/commands/agent.rs` | Restores original run/action, then calls `run_tauri_replay_execution` | yes, Replay-specific wrapper | Original AgentSpec, ToolPermission peek, NetworkPolicy, ExecutionSandbox, and Replay typed events | yes, restored from `run.agent_spec_id` or plan-bound spec; missing spec fails closed | yes, from current governed config/context | no model PromptStack | No Chat fallback; emits `ReplayStarted` / `ReplayCompleted` / `ReplayFailed`; Proposal status remains caller-owned | Keep locked; do not move to Chat facade | Low |
| `execute_agent_plan` / `retry_agent_plan` / plan execution core | `src-tauri/src/commands/plan.rs`, `src-tauri/src/execution_facade.rs` | Tauri commands call `run_tauri_plan_execution`; wrapper builds governed context and uses core `PlanExecutor` internally | yes, Plan-specific wrapper | Plan confirmation/review/deviation/retry gates; missing spec fails closed before execution; retry resolves spec before resetting failed status | yes, plan-bound spec or stored default only | yes, from facade-built ActionContext | no model PromptStack | Plan status/review/failure semantics; no Chat fallback | Keep locked; do not move to Chat facade | Low |
| Proposal accept/replay helpers | `src-tauri/src/commands/proposal.rs` | `accept_proposal_with_state` applies Proposal state only, marks accepted after successful apply, and returns `continue_run_id` / `continue_action_id` for replay lookup; it does not execute Chat fallback | no | Proposal status remains source of truth; ToolPermission replay lookup matches pending action tool/source/risk/action_type/step | context-dependent; replay itself requires original/restored spec | context-dependent; replay itself enforces current NetworkPolicy | no | Proposal status remains canonical; failed replay does not invent success | Next migration may only call a Replay-specific wrapper after Proposal apply succeeds | High |
| `execute_tool_call_inner` | `src-tauri/src/lib.rs` | Tauri direct tool facade wrapper, internally direct `ActionExecutor` | yes | Required AgentSpec plus governed `ActionContext`; wrapper verifies AgentSpec and NetworkPolicy | yes | yes | no | Returns command error/result; no Chat fallback | Keep locked with direct-tool source and behavior tests | Low |
| `run_skill` / `run_skill_with_state` | `src-tauri/src/commands/execution.rs` | SkillManifest-derived Skill PromptBlocks, required AgentSpec resolution, `AgentRuntime::execute_task_with_spec`, scheduler `generate_governed`, skill JSON parsing, and ProposalStore writes | no | AgentSpec fail-closed, PromptStack assembly, SkillManifest allowed-tools checked against AgentSpec | yes | model route privacy governed; no direct ActionContext/tool executor path for every step | yes | Missing AgentSpec fails before run creation; PromptStack/model failures persist failed Skill AgentRun plus metadata-only `run.failed`; no Chat fallback | Keep outside Chat facade; later decide whether parse/proposal lifecycle deserves a Skill-specific facade | Medium |
| `build_runtime_assembly_config`, `build_governed_agent_loop`, `build_governed_action_context` | `src-tauri/src/execution_facade.rs`, `src-tauri/src/execution_deps.rs` | Shared construction helpers | assembly-only | No standalone boundary; callers decide | caller-provided | caller-provided | caller-provided | none | Keep as internal assembly surface | Low |
| `run_tauri_agent_task` | `src-tauri/src/execution_facade.rs` | Tauri facade entrypoint for migrated AgentLoop modes | yes | Enforces required `AgentSpec`, `NetworkPolicy`, and `PromptRegistry` for Chat/StreamChat | yes | yes | yes | Caller decides Runtime fallback; Governance is typed error | Extend only through deliberate migration | Medium |
| `ActionExecutor` core | `openlife-core/src/agent/action_executor/` | Core tool/action executor | no, core engine | Enforces permissions/sandbox/network through `ActionContext` | context-dependent | context-dependent | no | Returns tool/action result or block | Do not migrate; wrap Tauri entrypoints instead | Medium |
| `PlanExecutor` core | `openlife-core/src/agent/plan_executor.rs` | Core plan execution engine | no, core engine | Plan review and confirmation semantics | supplied by caller | supplied by action callback | no | Plan outcomes and status transitions | Do not treat as Tauri facade target itself | Medium |
| `SubAgentRuntime` | `openlife-core/src/agent/sub_agent.rs` | Core sub-agent call-as-tool runtime | no, core engine | SubAgentSpec validation and child run trace | via SubAgentSpec | no direct network boundary | no | Child run/observation semantics | Audit with SubAgent roadmap, not this phase | Medium |
| Builder / Calibration commands | `src-tauri/src/commands/builder.rs`, `src-tauri/src/commands/calibration.rs` | LifeModel/proposal-oriented flows with proposal-created events | no | Proposal review and LifeModel safeguards; source audits prove no Chat facade/fallback calls | mixed | no full ActionContext by default | mixed | No Chat fallback | Defer migration until PromptStack/LifeModel Evolution audit; keep proposal/status safety tests locked | High |

## Not Migrated In This Phase

- **Builder / Calibration**: these paths mutate or propose LifeModel changes through distinct proposal semantics. They should wait for the PromptStack and LifeModel Evolution end-to-end audit. Current remaining direct paths are:
  - `builder_apply_signals`: legacy direct LifeModel apply path, gated by `system.allow_legacy_builder_direct_apply` at the Tauri command boundary and default-disabled.
  - `builder_step`: existing finalization path can persist an `updated_model` only when the finished session has no pending review signals; finished review sessions with pending signals are kept for Review Center instead of being written as drafts.
  - `apply_calibration` direct mode: legacy compatibility/debug/test-only LifeModel apply path, gated by `system.allow_legacy_calibration_direct_apply` and default-disabled. Missing mode now defaults to `proposal`; empty/unknown modes fail closed instead of falling back to direct. The normal Calibration UI does not expose this path; users must accept the created proposal in Review Center before LifeModel is written.
- **Proposal accept/replay status**: proposal state is the source of truth. `accept_proposal_with_state` may expose continuation ids for a pending action, but it must not perform a fake replay success or mark replay outcome through ExecutionFacade. Replay now uses a Replay-specific wrapper for execution only; ExecutionFacade does not write Proposal status and these paths must not be folded into a generic Chat degradation path.
- **Skill runtime lifecycle**: this path remains deliberately outside ExecutionFacade and Chat facade. The Skill-specific prompt is now PromptStack-governed, but the path still mixes model generation, SkillRegistry parsing/validation, `AgentSpec`, `AgentRun`, and ProposalStore semantics. Current safety net proves missing `AgentSpec` fails closed without run/model/fallback; AgentSpec restricted toolsets block skills whose declared `allowed_tools` are not permitted; PromptStack and model-generation failures persist failed Skill runs with readable errors; success keeps the frontend response shape. It must not be claimed as an ExecutionFacade migration until a Skill-specific facade owns this mixed contract explicitly.
- **Core executors**: `ActionExecutor`, `PlanExecutor`, and `SubAgentRuntime` are core engines, not Tauri user-facing entrypoints. Migration should wrap Tauri entrypoints, not the engines themselves.

## Next Batch Migration Recommendations

### Completed Candidate: Direct Tool Execution Facade Wrapper

- **Status**: migrated to a Tauri-side direct tool facade wrapper.
- **Boundary**: this is not a Chat fallback path. `run_tauri_direct_tool_execution` creates/persists the direct tool audit run and maps the action result back to the existing `ToolCallResult` shape; permission, sandbox, and network denials remain blocked/error tool outcomes.
- **Locked tests**: missing `AgentSpec` fail-closed with no `AgentRun`; source audit proves `execute_tool_call_inner` calls the direct tool facade and no longer constructs/calls `ActionExecutor` directly; result-shape test covers `goal.read`; sandbox and network denials assert no fallback warning and no `FallbackStarted` / `FallbackCompleted` events.
- **Still out of scope**: Builder and Calibration remain not migrated.

### Completed Candidate: Scheduled Proactive Execution Facade Wrapper

- **Status**: migrated to `run_tauri_scheduled_execution`; event creation validation and failed-run observability are complete for the current Scheduled / Proactive execution path.
- **Boundary**: this is not Chat/StreamChat `run_tauri_agent_task`. The wrapper keeps Scheduled `Planner` role, write-disabled config, restricted toolset, governed `NetworkPolicy`, `ExecutionSandbox`, `AgentSpec`, and `PromptBlockRegistry`. It may call `AgentLoop::run` internally, but `scheduler_runner.rs` no longer calls it directly.
- **Scheduler ownership**: `scheduler_runner.rs` still owns scheduler lease, stale-running recovery, terminal-task non-reclaim, and task file outcome merge. The facade returns `run_id`, output, and `result_preview`; it does not write `scheduled_tasks.json`. Outcome merge now only updates tasks still in `running`, so a late completion/failure cannot overwrite a newer terminal task state written concurrently.
- **AgentRunEvent coverage**: successful and runtime-failed scheduled AgentLoop runs use the existing AgentLoop events (`run.created`, `agent_spec.selected`, `prompt_stack.assembled`, `context_governance.applied`, model events, `tool.call_blocked` when tools are governed, `run.completed` or `run.failed`). NetworkPolicy hard deny/ask and sandbox deny record `tool.call_blocked` with typed metadata-only payloads and no fallback events. Missing `AgentSpec` fails before run creation, so there is no AgentRunEvent; the scheduler task failure/status is the traceable record.
- **Failure semantics**: missing `AgentSpec`, missing/mismatched `NetworkPolicy`, and missing `PromptBlockRegistry` fail closed as typed Governance errors before run creation and do not write `agent_run_id`. Runtime failures that have an `AgentLoopResult` persist the failed `AgentRun` and return typed Runtime errors carrying `run_id`. The scheduler maps failures to task `failed` with a readable error, without `completed_at` or `result_preview`, and writes `agent_run_id` only when the facade error provides one. NetworkPolicy/Sandbox governance blocks reached during tool execution remain blocked runtime outcomes/events and do not become fake success.
- **Payload contract**: scheduler task outcomes store metadata only (`status`, timestamps, readable error, `agent_run_id`, and bounded `result_preview` on success). AgentRunEvent payloads for prompt/context/tool governance use existing typed builders and do not include raw prompt, raw LifeModel, raw tool output, or full sensitive user context.
- **Proactive suggestion status**: ProactiveEngine remains suggestion-only and does not create AgentRun/AgentRunEvent. Once a proactive suggestion is accepted into a scheduled task, the Scheduled wrapper and scheduler task failure/status semantics above apply.
- **Do not do**: do not migrate Builder or Calibration through Scheduled; do not use Chat fallback for scheduled failures.

### Completed Candidate: Replay-Specific Facade Wrapper

- **Status**: migrated to `run_tauri_replay_execution`.
- **Entrypoints audited**: `accept_proposal_with_state` in `src-tauri/src/commands/proposal.rs` applies Proposal state and returns replay continuation metadata; `replay_agent_action` / `replay_action_internal` in `src-tauri/src/commands/agent.rs` perform direct action replay.
- **Boundary**: this is a Replay-specific wrapper, not Chat/StreamChat `run_tauri_agent_task`. `replay_action_internal` no longer constructs `ActionExecutor` directly; it calls `run_tauri_replay_execution` after restoring the original AgentSpec and constructing a governed replay `ActionContext`.
- **Direct ActionExecutor use retained elsewhere**: Plan execution now uses a Plan-specific wrapper. Core/test paths and Proposal helpers still use `ActionExecutor` directly where they are not Tauri facade entrypoints.
- **Locked governance semantics**: missing original `AgentSpec` fails closed before execution; replay restores the original run/plan-bound `AgentSpec`; original tool source, target, risk, action type, and capabilities are preserved; ToolPermission deny records `ReplayFailed`; NetworkPolicy and ExecutionSandbox denials return blocked replay outcomes with typed reasons and no success status.
- **Typed events**: `ReplayStarted` / `ReplayCompleted` / `ReplayFailed` payloads now include stable `run_id`, `original_run_id`, `action_id`, `replay_of_action_id`, `proposal_id`, `agent_spec_id`, and typed reason fields where applicable. Tests assert no raw prompt/context leakage.
- **No Chat fallback**: source audit locks that Replay does not call `run_tauri_agent_task(Chat/StreamChat)` or `handle_agent_loop_fallback`, and behavior tests assert no `FallbackStarted` / `FallbackCompleted` events.
- **Proposal status**: ExecutionFacade does not write Proposal status. Proposal / replay callers remain the source of truth.

### Completed Candidate: Plan-Specific Facade Wrapper

- **Status**: migrated to `run_tauri_plan_execution`.
- **Boundary**: this is a Plan-specific wrapper, not Chat/StreamChat `run_tauri_agent_task`. `execute_agent_plan` and `retry_agent_plan` no longer assemble ad hoc `ActionContext` or construct `ActionExecutor` in the command layer. The wrapper builds the governed Tauri context, validates `AgentSpec` and `NetworkPolicy`, and then delegates plan-step orchestration to core `PlanExecutor`.
- **Locked governance semantics**: plan-bound `AgentSpec` is used for execution; unbound plans use the stored default spec; missing plan-bound spec fails closed before execution starts; AgentSpec deny blocks before tool execution; NetworkPolicy deny blocks web steps; ExecutionSandbox deny blocks shell steps; ToolPermission deny/ask remains failed/blocked plan-step semantics without fake success.
- **Retry semantics**: `Retry` resolves `AgentSpec` and builds context before mutating a failed plan back to `Confirmed`, so missing/invalid governance leaves the failed plan status intact.
- **No Chat fallback**: source audit locks that Plan commands call `run_tauri_plan_execution`, do not construct `ActionExecutor`, do not call `run_tauri_agent_task`, and do not call `handle_agent_loop_fallback`.

### Remaining Candidates: Builder / Calibration / Skill Runtime Lifecycle

- **Builder / Calibration**: should wait for PromptStack and LifeModel Evolution audit because they own LifeModel proposal semantics. They now have safety-net tests for ProposalStore routing, ProposalStatus-gated apply, typed `proposal.created` events, redaction, default-disabled legacy direct gates, and no Chat fallback/source calls.
- **Skill runtime lifecycle**: Skill-specific prompt has migrated to PromptStack, but the lifecycle remains outside ExecutionFacade. The audit locks no Chat fallback / no wrapper masquerade with `skill_runtime_stays_outside_chat_facade_no_fallback_source_audit`. Remaining prerequisites before any facade migration are: define a Skill-specific facade contract, decide whether skill model generation and envelope parsing are one runtime outcome or two phases, define failure/event semantics for parse warnings vs hard failures, and add deterministic model-output fixtures or an injectable scheduler seam before moving execution.

## Audit Tests

This phase adds lightweight source-audit tests to lock the completed Chat and StreamChat convergence:

- `execution_facade_chat_path_uses_facade_entrypoint`
- `execution_facade_stream_chat_path_uses_facade_entrypoint`
- `prompt_stack_coverage_audit_doc_lists_all_runtime_entrypoints`
- `prompt_stack_source_audit_classifies_governed_legacy_and_not_applicable_paths`
- `execution_facade_stream_chat_unknown_prompt_block_fails_closed`

These tests scan only the relevant production entrypoint bodies and assert that Chat/StreamChat contain `run_tauri_agent_task` while avoiding direct `AgentLoop::run` / `AgentLoop::run_streaming` calls in those entrypoint slices. The PromptStack audit tests additionally require a documented matrix for Chat, StreamChat, Scheduled, PlanMode / Plan execution, Replay, Skill runtime, Builder, Calibration, Proactive suggestions, and Direct tool execution; they lock Skill-specific prompt as PromptStack-governed, Builder / Calibration prompts as not complete, and Direct Tool / Replay / Plan action execution / Proactive suggestions as no-model or suggestion-only paths that must not emit fake PromptStack traces. They complement the existing entrance-level `send_message_with_agent_loop` fail-closed tests and the streaming Governance error event tests.

Scheduled migration adds scheduler/facade-focused tests whose names include `scheduled`, `scheduler`, or `execution_facade` for targeted filtering:

- `scheduler_lease_is_short_and_complete_merges_interleaved_pending_task`
- `scheduler_completed_and_failed_tasks_are_not_reclaimed_even_if_old`
- `scheduled_failure_observability_records_readable_error_without_completion`
- `scheduled_missing_agentspec_fails_closed_without_chat_fallback`
- `scheduled_missing_agentspec_records_scheduler_task_failure`
- `scheduled_successful_outcome_records_facade_preview_and_run_id`
- `scheduled_runtime_failure_records_scheduler_task_failure_with_run_id`
- `scheduled_completion_must_not_overwrite_newer_terminal_task_state`
- `scheduled_execution_uses_scheduled_facade_wrapper_without_chat_fallback`
- `execution_facade_scheduled_path_uses_scheduled_wrapper`
- `execution_facade_proactive_suggestions_do_not_execute_via_chat_fallback`
- `execution_facade_scheduled_assembly_carries_network_policy_and_sandbox`
- `execution_facade_scheduled_mode_is_not_migrated_to_chat_task_entrypoint`
- `scheduled_facade_preserves_restricted_toolset`
- `scheduled_facade_requires_agent_spec`
- `scheduled_facade_requires_network_policy`
- `scheduled_facade_failed_run_persistence_on_runtime_failure`
- `scheduled_facade_prompt_stack_runtime_error_does_not_fallback`
- `scheduled_network_policy_denial_records_governed_failure_without_chat_fallback`
- `scheduled_network_policy_ask_records_blocked_proposal_without_chat_fallback`
- `scheduled_sandbox_denial_records_governed_failure_without_chat_fallback`
- `scheduled_facade_governance_error_kind`
- `scheduled_facade_runtime_error_kind`

These tests prove migration without broadening semantics: `execute_scheduled_task` calls the Scheduled wrapper, does not directly call `.run(`, does not call `run_tauri_agent_task`, and contains no Chat fallback events. Proactive suggestions remain non-executing suggestion generation. Scheduler failures still remain scheduler task failures. PromptStack runtime-error coverage is named explicitly; NetworkPolicy and Sandbox denial tests now reach the governed ActionContext/tool gates and assert typed `tool.call_blocked` events with metadata-only payloads.

Proposal Replay / Replay hardening adds and strengthens the following tests:

- `replay_missing_agent_spec_fails_closed`: now also asserts no action execution, no action success rewrite, `ReplayFailed`, no fallback events, and Proposal status remains canonical.
- `replay_restores_original_agent_spec`
- `accepted_tool_permission_replay_still_checks_agent_spec`
- `replay_does_not_escalate_tool_scope`
- `replay_permission_not_authorized_records_typed_replay_failed_event`: now also asserts no fallback and no action rewrite.
- `replay_network_policy_denied_fails_closed_without_fallback`
- `replay_execution_sandbox_denied_fails_closed_without_success_outcome`
- `replay_typed_event_payload_contract_is_stable_and_redacted`
- `execution_facade_replay_path_uses_replay_wrapper`
- `replay_facade_preserves_action_result_shape`
- `replay_facade_rejects_agent_spec_mismatch`
- `replay_facade_requires_network_policy`
- `replay_facade_sandbox_denial_does_not_fallback`
- `replay_facade_network_denial_does_not_fallback`

Replay-specific wrapper migration added these tests before the Plan wrapper phase.

Plan-specific wrapper migration adds and strengthens the following tests:

- `execution_facade_plan_path_uses_plan_wrapper_not_chat_or_direct_executor`
- `plan_facade_uses_plan_bound_agent_spec_to_block_tool_before_execution`
- `plan_facade_missing_plan_bound_agent_spec_fails_closed_without_status_change`
- `plan_facade_unbound_plan_uses_stored_default_agent_spec`
- `plan_facade_network_policy_denial_blocks_web_step_without_fallback`
- `plan_facade_execution_sandbox_denial_blocks_shell_step_without_fallback`
- `plan_facade_tool_permission_deny_blocks_step_without_fallback`
- `plan_facade_tool_permission_ask_preserves_blocked_plan_semantics`
- `plan_facade_retry_resolves_agent_spec_before_resetting_failed_plan`

This batch migrated only the Plan-specific wrapper on top of the existing Replay wrapper. Builder and Calibration remain unmigrated; Skill-specific prompt is migrated, while Skill lifecycle remains outside ExecutionFacade.

Builder / Calibration proposal safety-net tests added after the Plan wrapper phase:

- `builder_create_proposals_only_accepts_accepted_or_edited_decisions_and_records_redacted_events`
- `builder_command_source_does_not_call_chat_facade_or_fallback`
- `calibration_create_proposals_preserves_review_metadata_and_records_redacted_events`
- `calibration_life_model_apply_requires_proposal_acceptance_status`
- `apply_calibration_without_mode_defaults_to_proposal_and_does_not_write_life_model`
- `apply_calibration_proposal_mode_creates_proposals_and_records_redacted_event`
- `apply_calibration_direct_mode_is_rejected_by_default_and_does_not_write_life_model`
- `apply_calibration_direct_mode_allows_legacy_test_only_when_config_set`
- `apply_calibration_unknown_mode_errors_and_does_not_write_life_model`
- `apply_calibration_empty_mode_errors_and_does_not_write_life_model`
- `CalibrationPage`: creates Review Center proposals from the main action, hides default direct apply copy, and shows pending proposal guidance
- `tauri.test`: `applyCalibration()` defaults to `mode: "proposal"` and never `mode: "direct"`
- `calibration_command_source_does_not_call_chat_facade_or_fallback`
- `skill_runtime_stays_outside_chat_facade_no_fallback_source_audit`
- `skill_runtime_missing_agentspec_fails_closed_without_run_or_chat_fallback`
- `skill_runtime_agent_spec_restricted_toolset_blocks_disallowed_skill_tools`
- `skill_runtime_prompt_stack_failure_persists_failed_run_with_safe_payload`
- `skill_runtime_model_generation_failure_persists_failed_run_not_success`
- `skill_runtime_success_response_shape_stays_frontend_compatible`

These tests prove that Builder review decisions go to `ProposalStore` instead of direct LifeModel writes, pending/rejected decisions are not silently treated as accepted, Calibration missing/proposal modes and the frontend formal path create Review Center proposals without writing LifeModel, legacy direct calibration is default-disabled and config-gated, unknown/empty calibration modes fail closed, Calibration proposals preserve review metadata and become patchable LifeModel proposals, terminal Proposal statuses cannot be accepted again, `proposal.created` payloads are metadata-only, and Builder / Calibration / Skill runtime have not been folded into Chat facade or Chat fallback. The Skill runtime tests additionally prove Skill-specific prompt uses PromptStack instead of legacy prompt builders, the lifecycle remains outside ExecutionFacade, missing AgentSpec fails closed before run/model/fallback, restricted AgentSpec toolsets gate skill-declared tools, PromptStack/model failures are observable failed runs rather than fake success, and the frontend success response contract remains camelCase-compatible.
