# OpenLife vNext Agent Coding Prompts

Date: 2026-05-06

Use these prompts to drive AI coding after P0/P1.

## Global Prompt

```text
You are working on OpenLife vNext Agent Framework.

Read first:
- AGENTS.md
- plans/openlife_vnext_migration_plan.md
- plans/openlife_vnext_p2_p3_task_specs.md
- plans/openlife_vnext_p4_task_specs.md
- plans/openlife_vnext_test_and_acceptance_matrix.md
- plans/openlife_ai_coding_governance.md
- relevant ADR files under plans/adr/

Rules:
- Execute exactly one task spec.
- Edit only allowed files.
- Do not implement non-goals.
- Do not introduce SubAgentRuntime before its task.
- Do not introduce Bash/Shell before ExecutionSandbox task.
- Do not bypass ToolRuntime, Proposal, PromptStack, or AgentRunEvent.
- Add tests for new behavior.
- Run verification commands.
- Report changed files, tests run, results, and residual risks.
```

## P1 Carry-Over Prompt

```text
Execute vNext P1 carry-over: Wire AgentRunEventStore Into Product Paths.

Use:
- plans/openlife_vnext_p2_p3_task_specs.md
- plans/adr/0001-agentrun-event-trace.md
- plans/openlife_vnext_execution_entrypoints.md

Goal:
- Add durable AgentRunEventStore ownership to Tauri AppState/bootstrap.
- Pass the event store into chat, streaming, replay, direct tool execution, scheduled/proactive, and facade paths that already support event recording.
- Ensure fallback records trace events or a clearly documented failure event.

Constraints:
- Do not change the AgentRunEvent schema.
- Do not add event timeline UI.
- Do not migrate historical runs.

Verification:
- Run cargo test -p openlife-core agent.
- Run cargo test -p openlife-tauri.
```

## P2-1 Prompt

```text
Execute vNext task P2-1: PlanMode Schema and Store Skeleton.

Use:
- plans/openlife_vnext_p2_p3_task_specs.md
- plans/adr/0007-planmode-confirmation-policy.md
- plans/openlife_vnext_test_and_acceptance_matrix.md

Constraints:
- Add AgentPlan types and store skeleton only.
- Do not change normal chat behavior.
- Do not execute plans.
- Do not add UI.

Verification:
- Run cargo test -p openlife-core agent.
```

## P2-2 Prompt

```text
Execute vNext task P2-2: PlanMode Planner PromptStack.

Use:
- plans/openlife_vnext_p2_p3_task_specs.md
- plans/adr/0002-promptstack-system-prompt.md
- plans/adr/0007-planmode-confirmation-policy.md

Constraints:
- Add PlanningPrompt and AgentPlan output schema through PromptStack.
- Do not implement plan execution.
- Do not create sub-agents.

Verification:
- Run cargo test -p openlife-core agent.
```

## P2-3 Prompt

```text
Execute vNext task P2-3: PlanMode Read-Only Exploration.

Goal:
- Planner may use read-only tools and emit AgentPlan.
- Write tools must be blocked.
- Plan creation must record AgentRunEvent.

Constraints:
- No LifeModel/Memory mutation.
- No frontend UI.
- No sub-agent.

Verification:
- Run cargo test -p openlife-core agent.
```

## P2-4 Prompt

```text
Execute vNext task P2-4: Plan Confirmation Protocol.

Goal:
- High-risk plans require confirmation.
- Low-risk read-only plans can proceed.
- Confirmation request records event.

Constraints:
- Minimal backend protocol only.
- No polished frontend plan editor.

Verification:
- Run cargo test -p openlife-core agent.
- Run cargo test -p openlife-tauri if Tauri commands changed.
```

## P2-5 Prompt

```text
Execute vNext task P2-5: CompactionSummary Skeleton.

Goal:
- Define context compaction summary schema.
- Preserve active proposals, unresolved tool observations, and sensitive redaction markers.

Constraints:
- No automatic compaction trigger.
- No UI.

Verification:
- Run cargo test -p openlife-core agent.
```

## P3-1 Prompt

```text
Execute vNext task P3-1: SubAgentSpec Skeleton.

Use:
- plans/adr/0008-subagent-permissions.md

Constraints:
- Add AgentSpec/SubAgentSpec schema only.
- No sub-agent execution.
- No handoff/parallel.

Verification:
- Run cargo test -p openlife-core agent.
```

## P3-2 Prompt

```text
Execute vNext task P3-2: SubAgent call_as_tool Runtime.

Constraints:
- Implement call_as_tool only.
- Child AgentRun must link to parent.
- Context isolation and tool policy must be enforced.
- No handoff, no parallel, no bash.

Verification:
- Run cargo test -p openlife-core agent.
```

## P3-3 Prompt

```text
Execute vNext task P3-3: ReviewAgent Mode.

Use:
- plans/openlife_vnext_p2_p3_task_specs.md
- plans/adr/0008-subagent-permissions.md

Constraints:
- Reviewer may inspect plan/output/patch and return structured review output.
- Reviewer must not mutate LifeModel, Memory, files, or external state.
- Reviewer cannot call write tools.
- Review result must appear in parent run trace.
- No handoff, no parallel workers, no bash.

Verification:
- Run cargo test -p openlife-core agent.
```

## P3-4 Prompt

```text
Execute vNext task P3-4: ExecutionSandbox Skeleton.

Use:
- plans/adr/0009-execution-sandbox-bash.md

Constraints:
- Add sandbox policy types and validators only.
- No shell execution.
- No BashExecutor.

Verification:
- Run cargo test -p openlife-core agent.
```

## P3-5 Prompt

```text
Execute vNext task P3-5: ChatPage AgentRunEvent Timeline Contract.

Use:
- plans/openlife_vnext_p2_p3_task_specs.md
- plans/adr/0010-chatpage-state-model.md

Constraints:
- Define frontend AgentRunEvent type and Tauri mock contract only.
- Static timeline rendering is allowed if scoped.
- Do not rewrite ChatPage.
- Do not redesign the chat UI.
- Do not reshape backend APIs unless a documented contract requires it.

Verification:
- Run frontend tests for the touched files.
- Run cargo test -p openlife-tauri if Tauri command mocks or backend contracts changed.
```

## P4 Global Prompt

```text
You are working on OpenLife vNext P4: Confirmed Plan Execution and Trace UI Integration.

Read first:
- AGENTS.md
- plans/openlife_vnext_p4_task_specs.md
- plans/openlife_vnext_migration_plan.md
- plans/openlife_vnext_test_and_acceptance_matrix.md
- plans/openlife_ai_coding_governance.md
- plans/adr/0001-agentrun-event-trace.md
- plans/adr/0003-toolruntime-metadata-policy.md
- plans/adr/0007-planmode-confirmation-policy.md
- plans/adr/0010-chatpage-state-model.md

Rules:
- Execute exactly one P4 task spec.
- Do not introduce Bash/Shell.
- Do not implement SubAgent parallel or handoff.
- Do not rewrite ChatPage.
- Do not bypass ToolRuntime, Proposal, PromptStack, AgentRunEvent, or ExecutionSandbox.
- Add focused tests for new behavior.
- Run the verification commands listed in the task spec.
- Report changed files, tests run, results, and residual risks.
```

## P4-1 Prompt

```text
Execute vNext task P4-1: Plan Execution Contract and Events.

Use:
- plans/openlife_vnext_p4_task_specs.md
- plans/adr/0001-agentrun-event-trace.md
- plans/adr/0007-planmode-confirmation-policy.md

Goal:
- Define minimal types and event kinds for executing confirmed AgentPlans.
- Add plan execution event types and AgentRunEventStore round-trip tests.
- Preserve Unknown(String) behavior for future event types.

Constraints:
- No tool execution yet.
- No frontend UI.
- No Bash/Shell.

Verification:
- cargo test -p openlife-core event_store
- cargo test -p openlife-core agent
- cargo check -q
```

## P4-2 Prompt

```text
Execute vNext task P4-2: Minimal Confirmed Plan Executor.

Use:
- plans/openlife_vnext_p4_task_specs.md
- plans/adr/0003-toolruntime-metadata-policy.md
- plans/adr/0007-planmode-confirmation-policy.md

Goal:
- Implement a minimal PlanExecutor for confirmed plans.
- Execute read-only plan steps through existing ActionExecutor/ToolRuntime.
- Reject unconfirmed high-risk plans.
- Record plan step events and deviations in AgentRunEvent.

Constraints:
- No Bash/Shell.
- No parallel plan execution.
- No sub-agent handoff execution.
- No polished frontend UI.
- Do not bypass existing permission/proposal policy for writes.

Verification:
- cargo test -p openlife-core agent
- cargo check -q
```

## P4-3 Prompt

```text
Execute vNext task P4-3: Tauri Plan Commands.

Use:
- plans/openlife_vnext_p4_task_specs.md
- plans/adr/0007-planmode-confirmation-policy.md

Goal:
- Expose minimal Tauri commands for plan get/list/confirm/reject/execute.
- Add PlanStore to AppState/bootstrap if needed.
- Add frontend tauri.ts wrappers and types.

Constraints:
- No full plan editor.
- No ChatPage rewrite.
- Commands must keep Plan execution governed by PlanExecutor/ToolRuntime.

Verification:
- cargo test -p openlife-tauri
- pnpm --dir frontend typecheck
- cargo check -q
```

## P4-4 Prompt

```text
Execute vNext task P4-4: Chat Trace UI Integration.

Use:
- plans/openlife_vnext_p4_task_specs.md
- plans/adr/0010-chatpage-state-model.md

Goal:
- Wire RunTracePanel into ChatPage in the smallest safe way.
- Fetch AgentRunEvents for the active/latest run after completion.
- Preserve streaming, proposal banner, tool trace behavior, and existing layout.

Constraints:
- No ChatPage rewrite.
- No visual redesign.
- No backend API reshaping unless required by the existing list_agent_run_events contract.

Verification:
- pnpm --dir frontend typecheck
- pnpm --dir frontend test -- --run RunTracePanel ChatPage tauri
- cargo test -p openlife-tauri if backend command contracts changed
```

## P4-5 Prompt

```text
Execute vNext task P4-5: Plan Execution Review Gate.

Use:
- plans/openlife_vnext_p4_task_specs.md
- plans/adr/0008-subagent-permissions.md
- plans/adr/0007-planmode-confirmation-policy.md

Goal:
- Add an optional ReviewAgent gate for medium/high-risk plan execution results.
- ReviewAgent remains read-only and returns structured review output.
- Critical review issues prevent plan completion unless a future explicit override policy is added.

Constraints:
- No parallel reviewers.
- No handoff.
- No UI for review editing.
- No Bash/Shell.

Verification:
- cargo test -p openlife-core agent
- cargo check -q
```

## P5 Global Prompt

```text
You are working on OpenLife vNext P5: Governed Plan Operations and Recovery.

Read first:
- AGENTS.md
- plans/openlife_vnext_p5_task_specs.md
- plans/openlife_vnext_migration_plan.md
- plans/openlife_vnext_test_and_acceptance_matrix.md
- plans/openlife_ai_coding_governance.md
- plans/adr/0001-agentrun-event-trace.md
- plans/adr/0003-toolruntime-metadata.md
- plans/adr/0007-planmode-confirmation-policy.md
- plans/adr/0008-subagent-permissions.md
- plans/adr/0011-plan-recovery-rollback-policy.md

Rules:
- Execute exactly one P5 task spec.
- Do not introduce Bash/Shell.
- Do not implement SubAgent parallel or handoff.
- Do not rewrite ChatPage.
- Do not implement automatic rollback before ADR 0011 is accepted.
- Do not bypass ToolRuntime, Proposal, PromptStack, AgentRunEvent, ExecutionSandbox, or PlanExecutor.
- Preserve append-only AgentRunEvent history.
- Add focused tests for new behavior.
- Run the task spec verification commands.
- Report changed files, tests run, results, and residual risks.
```

## P5-0 Prompt

```text
Execute vNext task P5-0: Closeout And Baseline.

Use:
- plans/openlife_vnext_p5_task_specs.md
- plans/openlife_vnext_test_and_acceptance_matrix.md

Goal:
- Clean up P4 closeout leftovers before P5 implementation.
- Remove test-only unused imports.
- Add wrapper-level test proving executeAgentPlan() normalizes backend snake_case to frontend camelCase.
- Run the P4 closeout verification.

Constraints:
- No new plan operations.
- No UI changes.
- No retry/cancel implementation.

Verification:
- pnpm --dir frontend test -- --run tauri
- cargo test -p openlife-core agent::plan_executor
- cargo check -q
- make ci

Report:
- changed files
- warnings fixed or intentionally left
- tests run
- residual risks
```

## P5-1 Prompt

```text
Execute vNext task P5-1: Stable Plan Operation Contract.

Use:
- plans/openlife_vnext_p5_task_specs.md
- plans/adr/0007-planmode-confirmation-policy.md

Goal:
- Define stable plan operation result types.
- Replace ad hoc plan command JSON responses with a stable contract.
- Keep frontend wrappers camelCase.

Allowed edit areas:
- openlife-core/src/agent/types.rs
- src-tauri/src/commands/plan.rs
- frontend/src/tauri.ts
- frontend/src/types.ts
- frontend/src/test/mocks/tauri.ts
- relevant tests

Constraints:
- Do not add cancel/retry/continue behavior in this task.
- Do not bypass PlanExecutor or ToolRuntime.
- Do not rewrite ChatPage.

Verification:
- cargo test -p openlife-tauri commands::plan
- pnpm --dir frontend test -- --run tauri
- pnpm --dir frontend typecheck
- cargo check -q
```

## P5-2 Prompt

```text
Execute vNext task P5-2: Cancel Plan.

Use:
- plans/openlife_vnext_p5_task_specs.md
- plans/adr/0011-plan-recovery-rollback-policy.md

Goal:
- Add governed cancel_agent_plan(plan_id).
- Allow cancellation only from published, confirmed, or executing states.
- Reject cancellation for completed and rejected plans.
- Record plan.cancel_requested and plan.cancelled events.

Allowed edit areas:
- openlife-core/src/agent/types.rs
- openlife-core/src/agent/plan_store.rs
- openlife-core/src/agent/plan_executor.rs only if lifecycle helper is needed
- src-tauri/src/commands/plan.rs
- frontend/src/tauri.ts
- frontend/src/types.ts
- frontend/src/test/mocks/tauri.ts
- focused tests

Constraints:
- Cancellation does not rollback side effects.
- No Bash/Shell.
- No direct LifeModel, Memory, file, or external mutation.
- Do not implement retry in this task.

Verification:
- cargo test -p openlife-core agent::plan_store
- cargo test -p openlife-tauri commands::plan
- pnpm --dir frontend typecheck
- cargo check -q
```

## P5-3 Prompt

```text
Execute vNext task P5-3: Retry Failed Plan.

Use:
- plans/openlife_vnext_p5_task_specs.md
- plans/adr/0011-plan-recovery-rollback-policy.md
- plans/adr/0007-planmode-confirmation-policy.md

Goal:
- Add retry_agent_plan(plan_id) for failed and failed_review plans.
- Retry the whole plan in the first implementation.
- Preserve append-only trace history and record retry attempt events.

Expected events:
- plan.retry_requested
- plan.retry_started
- normal plan execution events for the new attempt

Constraints:
- No from-step retry yet.
- No rollback.
- No deletion or mutation of historical events.
- Retry must still use PlanExecutor, ToolRuntime, Permission, Proposal, and ReviewGate.

Verification:
- cargo test -p openlife-core agent::plan_executor
- cargo test -p openlife-tauri commands::plan
- pnpm --dir frontend typecheck
- cargo check -q
```

## P5-4 Prompt

```text
Execute vNext task P5-4: Blocked Action Continuation.

Use:
- plans/openlife_vnext_p5_task_specs.md
- plans/adr/0003-toolruntime-metadata.md
- plans/adr/0011-plan-recovery-rollback-policy.md

Goal:
- Link blocked/needs-confirmation plan actions to plan id and step index.
- Continue or replay after user approval through existing Permission / Proposal / Replay.
- Record plan action replay events.

Expected command shape:
- continue_agent_plan(plan_id)
- or replay_plan_action(plan_id, action_id), if action-level replay is safer

Constraints:
- Do not bypass ToolRuntime.
- Do not auto-approve permissions.
- Do not implement rollback.
- Do not directly write LifeModel, Memory, files, or external systems.

Verification:
- cargo test -p openlife-core agent
- cargo test -p openlife-tauri commands::plan
- cargo check -q
```

## P5-5 Prompt

```text
Execute vNext task P5-5: Rollback Policy ADR.

Use:
- plans/openlife_vnext_p5_task_specs.md
- plans/openlife_ai_coding_governance.md
- plans/adr/0011-plan-recovery-rollback-policy.md
- plans/adr/README.md

Goal:
- Finalize ADR 0011 for plan recovery and rollback policy.
- Decide retry, cancellation, rollback-capable, irreversible, confirmation, and event rules.
- Update ADR README status and backlog.

Constraints:
- Documentation only.
- Do not implement rollback executor.
- Do not add code.

Verification:
- rg checks can find ADR 0011 and P5 rollback policy references.
- git diff contains documentation files only.
```

## P5-6 Prompt

```text
Execute vNext task P5-6: Real Read-Only ReviewAgent Integration.

Use:
- plans/openlife_vnext_p5_task_specs.md
- plans/adr/0008-subagent-permissions.md
- plans/adr/0007-planmode-confirmation-policy.md

Goal:
- Replace production deterministic review gate with governed read-only ReviewAgent path.
- ReviewAgent outputs structured ReviewAgentOutput.
- Critical review continues to prevent Completed status.
- Parent trace links review result.

Allowed edit areas:
- openlife-core/src/agent/plan_executor.rs
- openlife-core/src/agent/sub_agent.rs
- src-tauri/src/commands/plan.rs
- focused tests

Constraints:
- ReviewAgent cannot mutate LifeModel, Memory, files, or external systems.
- No parallel reviewers.
- No handoff.
- No UI for review editing.
- No Bash/Shell.

Verification:
- cargo test -p openlife-core agent
- cargo check -q
```

## P5-7 Prompt

```text
Execute vNext task P5-7: Minimal Plan Operations UI.

Use:
- plans/openlife_vnext_p5_task_specs.md
- plans/adr/0010-chatpage-state-model.md

Goal:
- Add minimal UI controls for legal plan operations.
- Show plan status, operation result, and trace linkage.
- Preserve ChatPage streaming, proposal banner, and trace behavior.

Allowed edit areas:
- frontend/src/components/
- frontend/src/pages/ChatPage.tsx only for minimal integration
- frontend/src/tauri.ts
- frontend/src/types.ts
- frontend/src/test/mocks/tauri.ts
- focused frontend tests

Constraints:
- Do not rewrite ChatPage.
- Do not add a full plan editor.
- Do not do visual redesign.
- Empty plan state should not clutter UI.

Verification:
- pnpm --dir frontend typecheck
- pnpm --dir frontend test -- --run ChatPage RunTracePanel tauri
```
