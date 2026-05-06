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
