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
- plans/openlife_vnext_p8_task_specs.md
- plans/openlife_vnext_test_and_acceptance_matrix.md
- plans/openlife_ai_coding_governance.md
- relevant ADR files under plans/adr/

Current phase: P8 Compaction can close. P9 ExecutionSandbox-Governed Shell Execution is next.

Rules:
- Execute exactly one task spec.
- Edit only allowed files.
- Do not implement non-goals.
- Do not introduce SubAgentRuntime before its task.
- Do not introduce Bash/Shell before ExecutionSandbox task.
- Do not bypass ToolRuntime, Proposal, PromptStack, or AgentRunEvent.
- For P9: shell is default-off, no interactive terminal, no /bin/sh -c, no raw shell strings, structured command input only, no scheduled/sub-agent shell by default.
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

## P6 Global Prompt

```text
You are working on OpenLife vNext P6: AgentSpec-Governed Runtime and Context Assembly.

Read first:
- AGENTS.md
- plans/openlife_vnext_p6_task_specs.md
- plans/openlife_vnext_migration_plan.md
- plans/openlife_vnext_test_and_acceptance_matrix.md
- plans/openlife_vnext_core_primitives_and_boundaries.md
- plans/openlife_vnext_architecture_principles.md
- plans/openlife_ai_coding_governance.md
- plans/adr/0001-agentrun-event-trace.md
- plans/adr/0003-toolruntime-metadata.md
- plans/adr/0007-planmode-confirmation-policy.md
- plans/adr/0010-chatpage-state-model.md

Rules:
- Execute exactly one P6 task spec.
- Do not introduce Bash/Shell.
- Do not implement SubAgent parallel or handoff.
- Do not rewrite ChatPage.
- Do not implement automatic rollback.
- Do not build a full AgentSpec editor.
- Do not bypass ToolRuntime, ActionExecutor, Proposal, PromptStack, AgentRunEvent, ExecutionSandbox, or PlanExecutor.
- AgentSpec may constrain tools/context/prompts, but it must not grant authority beyond existing runtime policy.
- Preserve append-only AgentRunEvent history.
- Add focused tests for new behavior.
- Run the task spec verification commands.
- Report changed files, tests run, results, and residual risks.
```

## P6-0 Prompt

```text
Execute vNext task P6-0: Documentation And Entry Sync.

Use:
- AGENTS.md
- plans/openlife_vnext_p6_task_specs.md
- plans/openlife_vnext_migration_plan.md
- plans/openlife_vnext_test_and_acceptance_matrix.md
- plans/openlife_vnext_agent_coding_prompts.md

Goal:
- Make P6 discoverable and AI-coding-ready.
- Ensure document entrypoints reference P6 task specs.
- Ensure P6 task order and acceptance commands are clear.

Constraints:
- Documentation only.
- Do not change Rust or TypeScript code.

Verification:
- rg -n "openlife_vnext_p6_task_specs|P6-0|P6-1|P6-2|P6-3|P6-4|P6-5|P6-6|P6-7|AgentSpec-Governed Runtime" AGENTS.md plans
- git diff --name-only contains documentation files only

Report:
- changed files
- verification result
- residual risks
```

## P6-1 Prompt

```text
Execute vNext task P6-1: AgentSpec Core Contract.

Use:
- plans/openlife_vnext_p6_task_specs.md
- plans/openlife_vnext_core_primitives_and_boundaries.md
- plans/openlife_vnext_architecture_principles.md

Goal:
- Define a stable core AgentSpec contract for governed runtime identity.
- AgentSpec must describe an agent as a governed runtime unit, not only a prompt.

Expected fields:
- id
- name
- role
- base prompt id or equivalent prompt reference
- prompt block ids
- allowed tools
- denied tools or policy deny list
- context policy
- tool policy
- memory policy
- privacy policy
- optional output schema reference
- max steps

Allowed edit areas:
- openlife-core/src/agent/types.rs
- openlife-core/src/agent/mod.rs
- relevant focused tests under openlife-core/src/agent/

Constraints:
- Do not wire AgentSpec into execution yet.
- Do not implement SubAgentRuntime changes.
- Do not add UI.
- AgentSpec must not bypass ToolRuntime or Permission.

Verification:
- cargo test -p openlife-core agent
- cargo check -q

Required tests:
- AgentSpec serde round-trip
- default/main AgentSpec can be constructed
- tool allow/deny policy fields preserve values
- output schema reference is optional
```

## P6-2 Prompt

```text
Execute vNext task P6-2: AgentTask Contract.

Use:
- plans/openlife_vnext_p6_task_specs.md
- plans/openlife_vnext_core_primitives_and_boundaries.md

Goal:
- Define AgentTask as the formal intent-and-constraints object before execution.
- AgentTask must be separate from AgentRun trace.

Expected fields:
- id
- kind
- user intent
- session id
- workspace scope
- privacy level
- requires plan
- expected output
- initiator
- associated AgentSpec id

Allowed edit areas:
- openlife-core/src/agent/types.rs
- openlife-core/src/agent/mod.rs
- relevant focused tests under openlife-core/src/agent/

Constraints:
- Do not replace all runtime entrypoints in this task.
- Do not implement scheduling/proactive migration.
- Do not add UI.
- AgentTask must not contain raw LifeModel or raw memory payloads.

Verification:
- cargo test -p openlife-core agent
- cargo check -q

Required tests:
- AgentTask serde round-trip
- AgentTask can reference AgentSpec id
- AgentTask privacy/workspace fields do not require raw context payloads
```

## P6-3 Prompt

```text
Execute vNext task P6-3: ContextPolicy And ContextAssembler.

Use:
- plans/openlife_vnext_p6_task_specs.md
- plans/openlife_vnext_core_primitives_and_boundaries.md
- plans/adr/0001-agentrun-event-trace.md

Goal:
- Introduce a minimal governed context assembly path.
- AgentSpec may request context, but ContextAssembler decides what is actually included under policy.

Expected behavior:
- ContextPolicy determines eligible context categories:
  - LifeModel summary
  - goals
  - state
  - memory snippets
  - current session summary
  - tool observations
- ContextAssembler returns:
  - included context categories
  - excluded context categories
  - privacy/redaction notes
  - compact summary suitable for AgentRunEvent

Allowed edit areas:
- openlife-core/src/agent/context_assembler.rs or equivalent new module
- openlife-core/src/agent/types.rs
- openlife-core/src/agent/mod.rs
- relevant focused tests under openlife-core/src/agent/

Constraints:
- Do not rewrite existing chat context assembly wholesale.
- Do not expose raw memory or full LifeModel in AgentRunEvent payloads.
- Do not call LLMs.

Verification:
- cargo test -p openlife-core agent
- cargo check -q

Required tests:
- LifeModel summary can be included when policy allows
- memory snippets are excluded when policy denies memory
- privacy note appears when sensitive context is omitted
- event-safe summary does not include raw sensitive text
```

## P6-4 Prompt

```text
Execute vNext task P6-4: AgentSpec Tool Policy Enforcement.

Use:
- plans/openlife_vnext_p6_task_specs.md
- plans/adr/0003-toolruntime-metadata.md
- plans/openlife_vnext_core_primitives_and_boundaries.md

Goal:
- Apply AgentSpec tool policy before ActionExecutor executes a tool.

Expected behavior:
- A tool must satisfy both existing ToolRuntime/ActionExecutor/Permission policy and AgentSpec allowed/denied tool policy.
- Denied-by-AgentSpec tool attempts are blocked and recorded as AgentRunEvent or event-ready outcome.
- AgentSpec cannot allow a tool that ToolRuntime would otherwise block.
- Declarative-only tools remain non-executable.

Allowed edit areas:
- openlife-core/src/agent/action_executor/
- openlife-core/src/agent/types.rs
- openlife-core/src/agent/event_store.rs only if new event mapping is needed
- relevant focused tests under openlife-core/src/agent/

Constraints:
- Do not bypass existing permission/proposal behavior.
- Do not add new tool executors.
- Do not add UI.

Verification:
- cargo test -p openlife-core agent
- cargo check -q

Required tests:
- AgentSpec-allowed read tool can execute if ToolRuntime allows it
- AgentSpec-denied tool is blocked before execution
- AgentSpec allow does not bypass permission for write/external side-effect tools
- blocked attempt records an AgentRunEvent or event-ready outcome
```

## P6-5 Prompt

```text
Execute vNext task P6-5: PromptStack Binding.

Use:
- plans/openlife_vnext_p6_task_specs.md
- plans/openlife_vnext_core_primitives_and_boundaries.md
- plans/adr/0001-agentrun-event-trace.md

Goal:
- Bind AgentSpec prompt references to PromptStack assembly.

Expected behavior:
- AgentSpec references prompt blocks by id/version or equivalent stable identifiers.
- PromptStack assembly can consume AgentSpec prompt references.
- AgentRunEvent records prompt block ids/versions, not sensitive full prompt text.
- Cloud-disallowed prompt blocks remain filterable under privacy policy.

Allowed edit areas:
- openlife-core/src/agent/prompt_stack.rs or existing PromptStack module
- openlife-core/src/agent/types.rs
- openlife-core/src/agent/event_store.rs only if event payload helpers are needed
- relevant focused tests under openlife-core/src/agent/

Constraints:
- Do not introduce ad hoc prompt fragments.
- Do not call LLMs.
- Do not change frontend UI.

Verification:
- cargo test -p openlife-core agent
- cargo check -q

Required tests:
- AgentSpec prompt block ids are assembled through PromptStack
- missing prompt block is reported as structured error
- event metadata contains block ids/versions only
- cloud-disallowed block is excluded or summarized according to policy
```

## P6-6 Prompt

```text
Execute vNext task P6-6: PlanExecutor Uses AgentSpec.

Use:
- plans/openlife_vnext_p6_task_specs.md
- plans/adr/0003-toolruntime-metadata.md
- plans/adr/0007-planmode-confirmation-policy.md

Goal:
- Carry AgentSpec policy into confirmed plan execution.

Expected behavior:
- Plan execution context can include an AgentSpec or AgentSpec id.
- Plan step tool intent must satisfy AgentSpec tool policy.
- If plan intent and AgentSpec policy disagree, execution blocks and records a traceable event.
- Existing plan deviation, cancellation, retry, continuation, and review behavior remains stable.

Allowed edit areas:
- openlife-core/src/agent/plan_executor.rs
- openlife-core/src/agent/types.rs
- src-tauri/src/commands/plan.rs only for minimal context wiring
- relevant focused tests

Constraints:
- Do not change PlanExecutor into parallel execution.
- Do not bypass ActionExecutor.
- Do not rewrite plan commands.
- Do not add UI.

Verification:
- cargo test -p openlife-core agent::plan_executor
- cargo test -p openlife-tauri commands::plan
- cargo check -q

Required tests:
- plan step with AgentSpec-allowed tool executes
- plan step with AgentSpec-denied tool is blocked
- AgentSpec block records event
- cancellation/retry/review existing tests remain green
```

## P6-7 Prompt

```text
Execute vNext task P6-7: Minimal Runtime Trace Exposure.

Use:
- plans/openlife_vnext_p6_task_specs.md
- plans/adr/0010-chatpage-state-model.md

Goal:
- Expose AgentSpec governance decisions minimally in existing trace UI.

Expected behavior:
- Run trace can display AgentSpec id/role when available.
- Tool blocked by AgentSpec has readable trace summary.
- Empty AgentSpec metadata does not clutter UI.
- Existing ChatPage streaming, proposal banner, plan operations, and RunTracePanel behavior remain stable.

Allowed edit areas:
- frontend/src/types.ts
- frontend/src/components/RunTracePanel.tsx
- frontend/src/pages/ChatPage.tsx only for minimal integration
- frontend/src/test/mocks/tauri.ts
- focused frontend tests

Constraints:
- Do not build a full AgentSpec editor.
- Do not redesign ChatPage.
- Do not change backend API unless the event contract requires a small typed addition.

Verification:
- pnpm --dir frontend typecheck
- pnpm --dir frontend test -- --run RunTracePanel ChatPage tauri
- cargo check -q if backend event contract changes

Required tests:
- trace renders AgentSpec id/role when present
- AgentSpec-denied tool event renders readable summary
- empty AgentSpec metadata does not render extra UI
- existing ChatPage tests remain green
```

## P7 Global Prompt

```text
You are working on OpenLife vNext P7: AgentSpec Store, Runtime Selection, and Governed Agent Entry Points.

Read first:
- AGENTS.md
- plans/openlife_vnext_p7_task_specs.md
- plans/openlife_vnext_migration_plan.md
- plans/openlife_vnext_test_and_acceptance_matrix.md
- plans/openlife_ai_coding_governance.md
- plans/adr/0001-agentrun-event-trace.md
- plans/adr/0002-promptstack-system-prompt.md
- plans/adr/0003-toolruntime-metadata.md
- plans/adr/0006-cloud-privacy-modelrouter.md
- plans/adr/0007-planmode-confirmation-policy.md
- plans/adr/0012-agentspec-store-runtime-selection.md

Rules:
- Execute exactly one P7 task spec.
- Do not introduce Bash/Shell.
- Do not implement SubAgent parallel or handoff.
- Do not rewrite ChatPage.
- Do not build a full AgentSpec marketplace/editor.
- Do not bypass ToolRuntime, ActionExecutor, Proposal, PromptStack, ContextPolicy, AgentRunEvent, ExecutionSandbox, PlanExecutor, or AgentSpecStore.
- AgentSpec may constrain tools/context/prompts, but it must not grant authority beyond existing runtime policy.
- Persisted AgentSpec selection must be deterministic and traceable.
- Preserve append-only AgentRunEvent history.
- Add focused tests, including denial/error tests.
- Run the task spec verification commands.
- Report changed files, tests run, results, and residual risks.
```

## P7-0 Prompt

```text
Execute vNext task P7-0: Documentation And ADR Sync.

Use:
- AGENTS.md
- plans/openlife_vnext_p7_task_specs.md
- plans/openlife_vnext_migration_plan.md
- plans/openlife_vnext_test_and_acceptance_matrix.md
- plans/openlife_vnext_agent_coding_prompts.md
- plans/adr/README.md
- plans/adr/0012-agentspec-store-runtime-selection.md

Goal:
- Make P7 discoverable and AI-coding-ready.
- Ensure document entrypoints reference P7 task specs.
- Ensure P7 task order and acceptance commands are clear.
- Ensure ADR 0012 captures AgentSpecStore and runtime selection guardrails.

Constraints:
- Documentation only.
- Do not change Rust or TypeScript code.

Verification:
- rg -n "openlife_vnext_p7_task_specs|P7-0|P7-1|P7-2|P7-3|P7-4|P7-5|AgentSpec Store|ADR 0012" AGENTS.md plans
- git diff --name-only contains documentation files only

Report:
- changed files
- verification result
- residual risks
```

## P7-1 Prompt

```text
Execute vNext task P7-1: AgentSpecStore.

Use:
- plans/openlife_vnext_p7_task_specs.md
- plans/adr/0012-agentspec-store-runtime-selection.md
- plans/openlife_vnext_test_and_acceptance_matrix.md

Goal:
- Add durable AgentSpecStore for AgentSpec definitions.
- Bootstrap a default main AgentSpec with a stable id such as main.default.

Allowed edit areas:
- openlife-core/src/agent/types.rs
- openlife-core/src/agent/agent_spec_store.rs
- openlife-core/src/agent/mod.rs
- relevant focused tests under openlife-core/src/agent/

Constraints:
- Do not wire Tauri commands in this task.
- Do not add UI.
- Do not change PlanExecutor behavior.
- Do not implement specialist agent marketplace semantics.

Verification:
- cargo test -p openlife-core agent
- cargo check -q

Required tests:
- default main spec is bootstrapped
- AgentSpec round-trips through store
- inactive specs are not selected as default
- unknown spec id returns structured error
```

## P7-2 Prompt

```text
Execute vNext task P7-2: Tauri AgentSpec Commands And AppState Wiring.

Use:
- plans/openlife_vnext_p7_task_specs.md
- plans/adr/0012-agentspec-store-runtime-selection.md

Goal:
- Wire AgentSpecStore into AppState/bootstrap.
- Expose minimal AgentSpec lifecycle commands and frontend wrappers.

Expected commands:
- get_agent_spec(spec_id)
- list_agent_specs()
- get_default_agent_spec()
- update_agent_spec(spec)
- set_default_agent_spec(spec_id)

Allowed edit areas:
- src-tauri/src/state.rs
- src-tauri/src/bootstrap.rs
- src-tauri/src/lib.rs
- src-tauri/src/commands/agent.rs or src-tauri/src/commands/agent_spec.rs
- src-tauri/src/commands/mod.rs
- src-tauri/src/test_utils.rs
- frontend/src/tauri.ts
- frontend/src/types.ts
- frontend/src/test/mocks/tauri.ts
- relevant tests

Constraints:
- Do not build a polished AgentSpec editor.
- Do not rewrite Settings or ChatPage.
- Do not change normal chat behavior beyond bootstrap wiring.

Verification:
- cargo test -p openlife-tauri
- pnpm --dir frontend typecheck
- pnpm --dir frontend test -- --run tauri
- cargo check -q

Required tests:
- default spec available after bootstrap
- list returns the default main spec
- update preserves stable fields
- frontend wrappers typecheck
```

## P7-3 Prompt

```text
Execute vNext task P7-3: Runtime AgentSpec Selection.

Use:
- plans/openlife_vnext_p7_task_specs.md
- plans/adr/0012-agentspec-store-runtime-selection.md
- plans/adr/0002-promptstack-system-prompt.md
- plans/adr/0006-cloud-privacy-modelrouter.md

Goal:
- Make AgentRuntime execute with a resolved AgentSpec.
- Use selected AgentSpec to drive PromptStack and ContextPolicy.

Allowed edit areas:
- openlife-core/src/agent/runtime.rs
- openlife-core/src/agent/context_assembler.rs
- openlife-core/src/agent/prompt_stack.rs
- openlife-core/src/agent/types.rs
- relevant focused tests under openlife-core/src/agent/

Constraints:
- Do not call LLMs in new unit tests.
- Do not bypass PromptStack or ContextPolicy.
- Do not change ActionExecutor or PlanExecutor in this task.

Verification:
- cargo test -p openlife-core agent
- cargo check -q

Required tests:
- execute_task_with_spec uses AgentSpec prompt block ids
- unknown prompt block id fails before reasoning
- spec without memory access excludes memory
- spec without LifeModel access excludes LifeModel summary
- default main spec preserves current behavior
```

## P7-4 Prompt

```text
Execute vNext task P7-4: Plan Execution Uses Stored AgentSpec.

Use:
- plans/openlife_vnext_p7_task_specs.md
- plans/adr/0012-agentspec-store-runtime-selection.md
- plans/adr/0007-planmode-confirmation-policy.md

Goal:
- Stop hardcoding default AgentSpec in plan execution.
- Resolve stored AgentSpec for execute_agent_plan and retry_agent_plan.

Allowed edit areas:
- openlife-core/src/agent/types.rs
- openlife-core/src/agent/plan_store.rs
- openlife-core/src/agent/plan_executor.rs
- src-tauri/src/commands/plan.rs
- relevant tests

Constraints:
- Do not introduce parallel plan execution.
- Do not bypass PlanExecutor.
- Do not change permission/proposal/replay policy.
- Do not add plan editor UI.

Verification:
- cargo test -p openlife-core agent::plan_executor
- cargo test -p openlife-tauri commands::plan
- cargo check -q

Required tests:
- execute plan uses stored default AgentSpec
- plan-bound AgentSpec deny blocks tool before execution
- missing explicit spec id produces structured error or documented fallback
- trace includes agentspec_id
```

## P7-5 Prompt

```text
Execute vNext task P7-5: Minimal Frontend Contract Surface.

Use:
- plans/openlife_vnext_p7_task_specs.md

Goal:
- Expose AgentSpec contract to frontend code without building a large editor.

Allowed edit areas:
- frontend/src/types.ts
- frontend/src/tauri.ts
- frontend/src/test/mocks/tauri.ts
- optionally a small Settings tab or dev-only surface
- focused frontend tests

Constraints:
- Do not rewrite Settings.
- Do not rewrite ChatPage.
- Do not add a full AgentSpec marketplace/editor.
- Do not change trace UI except for small typed event metadata if needed.

Verification:
- pnpm --dir frontend typecheck
- pnpm --dir frontend test -- --run tauri
- if Settings changed: pnpm --dir frontend test -- --run Settings tauri

Required tests:
- wrappers typecheck
- mock returns AgentSpec shape
- default AgentSpec can be read from frontend wrapper
```

## P8 Global Prompt

```text
You are working on OpenLife vNext P8: Compaction, Long-Context Continuity, and Privacy-Governed Summary Trace.

Read first:
- AGENTS.md
- plans/openlife_vnext_p8_task_specs.md
- plans/openlife_vnext_migration_plan.md
- plans/openlife_vnext_test_and_acceptance_matrix.md
- plans/openlife_ai_coding_governance.md
- plans/adr/0001-agentrun-event-trace.md
- plans/adr/0002-promptstack-system-prompt.md
- plans/adr/0006-cloud-privacy-modelrouter.md
- plans/adr/0012-agentspec-store-runtime-selection.md

Rules:
- Execute exactly one P8 task spec.
- Do not introduce Bash/Shell.
- Do not implement SubAgent parallel or handoff.
- Do not rewrite ChatPage.
- Do not bypass AgentSpec, PromptStack, ContextPolicy, AgentRunEvent, PrivacyPolicy, ToolRuntime, ActionExecutor, Proposal, or PlanExecutor.
- Compaction must preserve active proposals, unresolved tool observations, important decisions, and pending user confirmations.
- Compaction summaries and event payloads must not contain raw sensitive user text, raw LifeModel identity fields, or raw memory snippets.
- SummaryOnly cloud paths must receive sanitized messages only.
- Prefer rule-based compaction first. Model summarization is optional and must be privacy-governed.
- Add focused tests and avoid network-backed unit tests.
- Run the task spec verification commands.
- Report changed files, tests run, results, and residual risks.
```

## P8-0 Prompt

```text
Execute vNext task P8-0: Documentation And Entry Sync.

Use:
- AGENTS.md
- plans/openlife_vnext_p8_task_specs.md
- plans/openlife_vnext_migration_plan.md
- plans/openlife_vnext_test_and_acceptance_matrix.md
- plans/openlife_vnext_agent_coding_prompts.md

Goal:
- Make P8 discoverable and AI-coding-ready.
- Ensure document entrypoints reference P8 task specs.
- Ensure P8 explicitly means Compaction, not Bash/Shell, SubAgent parallel, or ChatPage rewrite.

Constraints:
- Documentation only.
- Do not change Rust or TypeScript code.

Verification:
- rg -n "openlife_vnext_p8_task_specs|P8-0|P8-1|P8-2|P8-3|P8-4|P8-5|P8-6|CompactionSummary|compaction.created" AGENTS.md plans
- git diff --name-only contains documentation files only

Report:
- changed files
- verification result
- residual risks
```

## P8-1 Prompt

```text
Execute vNext task P8-1: Compaction Trigger And Policy.

Use:
- plans/openlife_vnext_p8_task_specs.md
- plans/openlife_vnext_test_and_acceptance_matrix.md

Goal:
- Define when an AgentLoop context should be compacted.
- Keep the decision deterministic and model-free.

Allowed edit areas:
- openlife-core/src/agent/compaction.rs
- openlife-core/src/agent/mod.rs
- relevant focused tests

Suggested implementation:
- CompactionConfig
- CompactionDecision
- estimate_message_tokens(messages)
- should_compact(messages, config)

Constraints:
- No LLM calls.
- No AgentLoop behavior changes in this task.
- No persistence changes in this task.

Verification:
- cargo test -p openlife-core agent::compaction --lib
- cargo check -q

Required tests:
- disabled config never compacts
- empty messages do not compact
- below thresholds does not compact
- token threshold triggers compaction
- message count threshold triggers compaction
- min_messages_before_compaction prevents premature compaction
```

## P8-2 Prompt

```text
Execute vNext task P8-2: CompactionSummary Builder.

Use:
- plans/openlife_vnext_p8_task_specs.md
- plans/adr/0001-agentrun-event-trace.md
- plans/adr/0006-cloud-privacy-modelrouter.md

Goal:
- Build a CompactionSummary from runtime context while preserving critical state and redacting sensitive content.

Allowed edit areas:
- openlife-core/src/agent/types.rs
- openlife-core/src/agent/compaction.rs
- relevant focused tests

Expected behavior:
- Preserve active proposal ids.
- Preserve unresolved tool observations.
- Preserve important decisions and pending tasks.
- Redact obvious PII, raw LifeModel identity fields, raw memory snippets, and raw sensitive user messages.
- Keep the first implementation rule-based and model-free.

Constraints:
- No model summarizer in this task.
- Do not store raw sensitive content in summary fields.
- Preserve serde compatibility where possible with serde defaults for new fields.

Verification:
- cargo test -p openlife-core agent::compaction --lib
- cargo test -p openlife-core agent::types::compaction_tests --lib
- cargo check -q

Required tests:
- active proposals are preserved
- unresolved observations are preserved
- decisions/pending tasks are preserved
- PII is redacted
- raw LifeModel/memory/user sensitive text is absent from cloud-safe summary
- summary round-trips through serde
```

## P8-3 Prompt

```text
Execute vNext task P8-3: Compaction AgentRunEvent.

Use:
- plans/openlife_vnext_p8_task_specs.md
- plans/adr/0001-agentrun-event-trace.md

Goal:
- Record compaction as append-only runtime trace.

Expected event:
- AgentRunEventType::CompactionCreated serialized as "compaction.created"

Allowed edit areas:
- openlife-core/src/agent/types.rs
- openlife-core/src/agent/event_store.rs
- frontend/src/types.ts
- frontend/src/components/RunTracePanel.tsx
- frontend/src/components/RunTracePanel.test.tsx
- frontend/src/test/mocks/tauri.ts
- relevant focused tests

Constraints:
- No large trace UI rewrite.
- Event payload must not expose raw prompt, memory, LifeModel, or sensitive user text.

Verification:
- cargo test -p openlife-core agent::event_store --lib
- pnpm --dir frontend test -- --run RunTracePanel tauri
- pnpm --dir frontend typecheck
- cargo check -q

Required tests:
- compaction.created serde round-trip
- event payload excludes raw sensitive content
- frontend type union includes compaction.created
- RunTracePanel renders a compaction event
```

## P8-4 Prompt

```text
Execute vNext task P8-4: AgentLoop Compaction Hook.

Use:
- plans/openlife_vnext_p8_task_specs.md
- plans/adr/0001-agentrun-event-trace.md
- plans/adr/0006-cloud-privacy-modelrouter.md

Goal:
- Use compacted context during long AgentLoop runs.

Allowed edit areas:
- openlife-core/src/agent/agent_loop.rs
- openlife-core/src/agent/compaction.rs
- openlife-core/src/agent/types.rs
- relevant focused tests

Expected behavior:
- AgentLoop checks compaction policy before model generation.
- If compaction triggers, build CompactionSummary and record compaction.created.
- Replace older messages with one compacted context message.
- Preserve latest user message, active proposals, unresolved observations, and pending decisions.
- Future generation uses compacted context.

Constraints:
- Do not lose the latest user message.
- Do not lose unresolved tool observations.
- Do not call a cloud model for compaction in this task.
- Missing event store must not panic.
- Keep changes localized to AgentLoop context preparation.

Verification:
- cargo test -p openlife-core agent::agent_loop --lib
- cargo test -p openlife-core agent::compaction --lib
- cargo check -q

Required tests:
- long message history triggers compaction
- compacted message count is smaller than original
- latest user message remains present
- active proposal ids appear in summary metadata
- unresolved observations appear in summary metadata
- compaction.created event is recorded
- no event store path remains safe
- SummaryOnly compaction summary excludes raw sensitive text
```

## P8-5 Prompt

```text
Execute vNext task P8-5: Optional Privacy-Governed Summarizer.

Use:
- plans/openlife_vnext_p8_task_specs.md
- plans/adr/0006-cloud-privacy-modelrouter.md

Goal:
- Optionally add a model-based compaction summarizer after the rule-based path is safe.

Allowed edit areas:
- openlife-core/src/agent/compaction.rs
- openlife-core/src/scheduler.rs
- relevant focused tests

Constraints:
- This task is optional for P8 completion unless explicitly requested.
- Do not require network-backed tests.
- Do not bypass P7 privacy governance.
- LocalOnly never falls back to cloud.
- SummaryOnly cloud payload must be sanitized.

Verification:
- cargo test -p openlife-core agent::compaction --lib
- cargo test -p openlife-core scheduler --lib
- cargo check -q

Required tests:
- LocalOnly without local model does not call cloud
- SummaryOnly cloud payload is sanitized
- summarizer error falls back to rule-based summary
```

## P8-6 Prompt

```text
Execute vNext task P8-6: Minimal Frontend Trace Surface.

Use:
- plans/openlife_vnext_p8_task_specs.md

Goal:
- Expose compaction in trace without building a large new UI.

Allowed edit areas:
- frontend/src/types.ts
- frontend/src/components/RunTracePanel.tsx
- frontend/src/components/RunTracePanel.test.tsx
- frontend/src/test/mocks/tauri.ts

Constraints:
- Minimal trace support only.
- Do not build a compaction editor or timeline redesign.
- Do not rewrite ChatPage.

Verification:
- pnpm --dir frontend test -- --run RunTracePanel tauri
- pnpm --dir frontend typecheck

Required tests:
- compaction event renders
- existing trace events still render
```

## P9 Global Prompt

```text
You are working on OpenLife vNext P9: ExecutionSandbox-Governed Shell Execution.

Read first:
- AGENTS.md
- plans/openlife_vnext_p9_task_specs.md
- plans/openlife_vnext_migration_plan.md
- plans/openlife_vnext_test_and_acceptance_matrix.md
- plans/openlife_vnext_core_primitives_and_boundaries.md
- plans/openlife_ai_coding_governance.md
- plans/adr/0009-execution-sandbox-bash.md

Rules:
- Execute exactly one P9 task spec.
- Shell is default-off.
- Do not expose an interactive shell.
- Do not use /bin/sh -c, cmd /C, or arbitrary command strings in the first executor.
- Use structured command input: command, args, cwd, env.
- Do not allow pipes, redirects, chained commands, command substitution, glob expansion, or shell metacharacter bypasses.
- Do not enable shell for normal chat, scheduled/proactive tasks, or sub-agents by default.
- Route shell through ToolRuntime/ActionExecutor only.
- Do not bypass AgentSpec, PromptStack, ContextPolicy, AgentRunEvent, Permission, Proposal, or ExecutionSandbox.
- Writes remain proposal-first.
- Add focused denial tests, not only success tests.
- Run the task spec verification commands.
- Report changed files, tests run, results, and residual risks.
```

## P9-0 Prompt

```text
Execute vNext task P9-0: Documentation And Entry Sync.

Use:
- AGENTS.md
- README.md
- plans/openlife_vnext_p9_task_specs.md
- plans/openlife_vnext_migration_plan.md
- plans/openlife_vnext_test_and_acceptance_matrix.md
- plans/openlife_vnext_agent_coding_prompts.md

Goal:
- Make P9 discoverable and AI-coding-ready.
- State that P8 can close and P9 is the next phase.
- Ensure P9 explicitly excludes interactive shell, arbitrary shell strings, scheduled shell, sub-agent shell, and direct writes.

Constraints:
- Documentation only.
- Do not change Rust or TypeScript code.

Verification:
- rg -n "openlife_vnext_p9_task_specs|P9-0|P9-1|P9-2|P9-3|P9-4|P9-5|P9-6|P9-7|ExecutionSandbox-Governed Shell" AGENTS.md README.md plans
- git diff --name-only contains documentation files only
```

## P9-1 Prompt

```text
Execute vNext task P9-1: Sandbox Contract Hardening.

Use:
- plans/openlife_vnext_p9_task_specs.md
- plans/adr/0009-execution-sandbox-bash.md

Goal:
- Promote ExecutionSandbox from existing skeleton to stable P9 policy contract.
- Keep shell default-off.

Allowed edit areas:
- openlife-core/src/agent/execution_sandbox.rs
- openlife-core/src/agent/mod.rs
- focused tests

Constraints:
- No shell executor.
- No manifest registration.
- No Tauri settings.

Verification:
- cargo test -p openlife-core agent::execution_sandbox --lib
- cargo check -q
```

## P9-2 Prompt

```text
Execute vNext task P9-2: Shell Tool Manifest Default-Off.

Use:
- plans/openlife_vnext_p9_task_specs.md
- plans/adr/0009-execution-sandbox-bash.md

Goal:
- Add the shell.run manifest contract without making it executable.
- Keep it high-risk and default-off.

Allowed edit areas:
- openlife-core/src/mcp.rs
- openlife-core/src/tool_manifest.rs
- openlife-core/src/agent/prompt_stack.rs
- focused tests

Constraints:
- No executor.
- No ActionExecutor branch.
- No frontend shell UI.

Verification:
- cargo test -p openlife-core mcp --lib
- cargo test -p openlife-core tool_manifest --lib
- cargo test -p openlife-core agent::prompt_stack --lib
- cargo check -q
```

## P9-3 Prompt

```text
Execute vNext task P9-3: AppState And Action Context Sandbox Wiring.

Goal:
- Carry ExecutionSandbox policy through config/AppState/action execution paths without executing shell.

Allowed edit areas:
- openlife-core/src/config.rs
- openlife-core/src/agent/action_executor/mod.rs
- src-tauri/src/lib.rs
- src-tauri/src/commands/agent.rs
- src-tauri/src/commands/plan.rs
- src-tauri/src/scheduler_runner.rs
- focused tests

Constraints:
- No shell execution.
- No settings UI beyond type-safe config plumbing.
- Preserve existing safe_paths behavior for file tools.

Verification:
- cargo test -p openlife-core agent::action_executor --lib
- cargo test -p openlife-tauri commands::plan --lib
- cargo check -q
```

## P9-4 Prompt

```text
Execute vNext task P9-4: Non-Interactive Command Executor Skeleton.

Goal:
- Add a structured command executor primitive that does not use shell interpreters.

Allowed edit areas:
- openlife-core/src/agent/shell_executor.rs
- openlife-core/src/agent/execution_sandbox.rs
- openlife-core/src/agent/mod.rs
- focused tests

Constraints:
- No ToolRuntime integration yet.
- No Tauri command.
- No interactive session.
- No network commands.

Verification:
- cargo test -p openlife-core agent::shell_executor --lib
- cargo test -p openlife-core agent::execution_sandbox --lib
- cargo check -q
```

## P9-5 Prompt

```text
Execute vNext task P9-5: ToolRuntime Shell Integration And Trace.

Goal:
- Expose shell.run through ActionExecutor only when sandbox, manifest, permission, and AgentSpec all allow it.

Allowed edit areas:
- openlife-core/src/agent/action_executor/
- openlife-core/src/agent/shell_executor.rs
- openlife-core/src/agent/types.rs
- openlife-core/src/agent/event_store.rs
- focused tests

Constraints:
- No frontend shell command box.
- No scheduled/proactive shell.
- No sub-agent shell.
- Do not bypass existing permission/proposal/replay paths.

Verification:
- cargo test -p openlife-core agent::action_executor --lib
- cargo test -p openlife-core agent::event_store --lib
- cargo check -q
```

## P9-6 Prompt

```text
Execute vNext task P9-6: Governed Runtime Entry Policy.

Goal:
- Prevent shell from leaking into broad agent behavior before explicit product design.

Allowed edit areas:
- openlife-core/src/agent/types.rs
- openlife-core/src/agent/agent_loop.rs
- openlife-core/src/agent/plan_mode.rs
- openlife-core/src/agent/plan_executor.rs
- openlife-core/src/agent/sub_agent.rs
- src-tauri/src/scheduler_runner.rs
- focused tests

Constraints:
- No interactive shell.
- No broad ChatPage rewrite.
- No automatic enabling.

Verification:
- cargo test -p openlife-core agent::agent_loop --lib
- cargo test -p openlife-core agent::plan_mode --lib
- cargo test -p openlife-core agent::plan_executor --lib
- cargo test -p openlife-core agent::sub_agent --lib
- cargo check -q
```

## P9-7 Prompt

```text
Execute vNext task P9-7: Minimal Settings And Trace Surface.

Goal:
- Expose shell governance state without building a terminal UI.

Allowed edit areas:
- frontend/src/types.ts
- frontend/src/tauri.ts
- frontend/src/test/mocks/tauri.ts
- frontend/src/components/RunTracePanel.tsx
- frontend/src/components/RunTracePanel.test.tsx
- minimal Settings tab files if config plumbing exists

Constraints:
- No terminal emulator.
- No shell command input box.
- No redesign.

Verification:
- pnpm --dir frontend test -- --run RunTracePanel tauri
- pnpm --dir frontend typecheck
```

## P9-0 Prompt

```text
Execute vNext task P9-0: Documentation And Entry Sync.

Goal:
- Make P9 discoverable and AI-coding-ready.
- State that P8 can close and P9 is the next phase.
- Ensure P9 explicitly excludes interactive shell, arbitrary shell strings,
  scheduled shell, sub-agent shell, and direct writes.

Allowed edit areas:
- AGENTS.md
- README.md
- plans/openlife_vnext_p9_task_specs.md
- plans/openlife_vnext_migration_plan.md
- plans/openlife_vnext_test_and_acceptance_matrix.md
- plans/openlife_vnext_agent_coding_prompts.md

Constraints:
- Documentation only.
- Do not change Rust or TypeScript code.

Verification:
- rg -n "openlife_vnext_p9_task_specs|P9-0|P9-1|P9-2|P9-3|P9-4|P9-5|P9-6|P9-7|ExecutionSandbox-Governed Shell" AGENTS.md README.md plans
- git diff --name-only contains documentation files only.
```

## P10 Global Prompt

```text
You are working on OpenLife vNext P10: Frontend Agent Workspace.

Read first:
- AGENTS.md
- plans/openlife_vnext_p10_task_specs.md
- plans/openlife_vnext_test_and_acceptance_matrix.md
- plans/openlife_vnext_architecture_principles.md

Global rules:
- Execute exactly one P10 task spec.
- Do not rewrite ChatPage wholesale.
- Preserve Chat streaming stability.
- Do not add terminal UI or shell command input.
- Do not expose shell.run in generic tools_prompt or normal Chat UI.
- Do not change backend runtime semantics unless the task explicitly allows a
  small read-only DTO/query addition.
- Keep all mutations proposal-first or routed through existing governed plan
  operations.
- Add focused frontend tests for new states and interactions.
```

## P10-0 Prompt

```text
Execute vNext task P10-0: Documentation And Entry Sync.

Goal:
- Make P10 discoverable and AI-coding-ready.
- State that P9 Shell/Sandbox core is closed and P10 Frontend Agent Workspace is current.
- Make P10 non-goals explicit: no ChatPage rewrite, no terminal UI, no shell enablement UX, no backend runtime migration.

Allowed edit areas:
- AGENTS.md
- README.md
- plans/openlife_vnext_p10_task_specs.md
- plans/openlife_vnext_migration_plan.md
- plans/openlife_vnext_test_and_acceptance_matrix.md
- plans/openlife_vnext_agent_coding_prompts.md

Constraints:
- Documentation only.
- Do not change Rust or TypeScript code.

Verification:
- rg -n "openlife_vnext_p10_task_specs|P10-0|P10-1|P10-2|P10-3|P10-4|P10-5|Frontend Agent Workspace" AGENTS.md README.md plans
- git diff --name-only contains documentation files only.
```

## P10-1 Prompt

```text
Execute vNext task P10-1: Agent Workspace Information Architecture.

Goal:
- Define the frontend workspace shell for recent runs, plans, tools, proposals, and next actions.

Allowed edit areas:
- frontend/src/App.tsx
- frontend/src/pages/DashboardPage.tsx
- frontend/src/pages/RunsPage.tsx
- frontend/src/pages/ChatPage.tsx only for links/embedding points
- shared frontend components and tests
- frontend/src/tauri.ts / mocks only for typed wrappers around existing commands

Constraints:
- No broad ChatPage rewrite.
- No terminal UI or shell command input.
- No backend mutation behavior.
- UI should be operational and dense, not a landing page.

Verification:
- pnpm --dir frontend test -- --run App Dashboard Runs tauri
- pnpm --dir frontend typecheck
```

## P10-2 Prompt

```text
Execute vNext task P10-2: Run Timeline And Event Detail Surface.

Goal:
- Upgrade runtime trace from compact rows into a useful run inspection experience.

Allowed edit areas:
- frontend/src/components/RunTracePanel.tsx
- frontend/src/components/RunTracePanel.test.tsx
- frontend/src/pages/RunsPage.tsx
- frontend/src/types.ts
- frontend/src/test/mocks/tauri.ts

Constraints:
- Do not add new backend event types unless truly required.
- Do not display unbounded stdout/stderr.
- Shell events render as governed tool events only; no command entry UI.

Verification:
- pnpm --dir frontend test -- --run RunTracePanel Runs tauri
- pnpm --dir frontend typecheck
```

## P10-3 Prompt

```text
Execute vNext task P10-3: Tool Observation Panel.

Goal:
- Make tool calls and observations explainable to users.

Allowed edit areas:
- frontend/src/pages/RunsPage.tsx
- frontend/src/components/ToolCallCard.tsx
- new small frontend components
- frontend/src/types.ts
- frontend/src/test/mocks/tauri.ts

Constraints:
- No new execution controls.
- No direct replay/retry button unless it calls an existing governed command and is covered by tests.
- Large outputs are collapsed and bounded.

Verification:
- pnpm --dir frontend test -- --run Runs ToolCallCard tauri
- pnpm --dir frontend typecheck
```

## P10-4 Prompt

```text
Execute vNext task P10-4: Proposal Evidence And Review Context.

Goal:
- Help users review proposals with source/evidence context.

Allowed edit areas:
- Review/Proposal frontend components
- frontend/src/pages/ChatPage.tsx for banner link/context only
- frontend/src/pages/RunsPage.tsx
- frontend/src/types.ts
- frontend/src/tauri.ts / mocks if existing proposal DTO wrappers need fields

Constraints:
- Do not add direct apply bypasses.
- Do not expose raw sensitive memory evidence.
- Backend changes should be read-only DTO additions only if existing data is unavailable.

Verification:
- pnpm --dir frontend test -- --run Proposal Chat Runs tauri
- pnpm --dir frontend typecheck
- Backend tests only if DTO commands change.
```

## P10-5 Prompt

```text
Execute vNext task P10-5: Plan Confirmation And Operations Surface.

Goal:
- Make confirmed plan execution usable from the frontend without changing plan runtime semantics.

Allowed edit areas:
- plan-related frontend components/pages
- frontend/src/tauri.ts
- frontend/src/types.ts
- frontend/src/test/mocks/tauri.ts
- minimal read-only backend DTO normalization only if required

Constraints:
- Do not implement rollback unless ADR 0011 is accepted and a separate task is created.
- Do not add shell plan execution UI.
- Do not make illegal terminal-state operations available.

Verification:
- pnpm --dir frontend test -- --run Plan Runs Chat tauri
- pnpm --dir frontend typecheck
- cargo test -p openlife-tauri commands::plan --lib if command DTOs change.
```
