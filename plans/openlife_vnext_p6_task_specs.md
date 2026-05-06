# OpenLife vNext P6 Task Specifications

Date: 2026-05-06

Status: draft

Package:

```text
AgentSpec-Governed Runtime and Context Assembly
```

P6 turns the now-traceable plan execution path into an agent-governed runtime path. The core question becomes: which agent identity is executing, which context is allowed, which tools are allowed, which prompt blocks apply, and how those decisions are recorded.

P6 does **not** introduce Bash/Shell, parallel SubAgents, handoff, automatic rollback, or a full AgentSpec editor.

## Baseline

Before P6:

- AgentRunEvent is append-only and can represent plan execution, cancellation, retry, and replay.
- ActionExecutor / ToolRuntime governs tool execution and permission/replay.
- PromptStack exists as a first-class primitive from earlier vNext work.
- AgentPlan / PlanExecutor can execute confirmed plans through ActionExecutor.
- P5 plan operations are traceable and stable enough to layer AgentSpec policy over them.

## Global Rules

- Execute exactly one P6 task spec at a time.
- Do not introduce Bash/Shell.
- Do not implement SubAgent parallel or handoff.
- Do not rewrite ChatPage.
- Do not bypass ToolRuntime, ActionExecutor, Proposal, PromptStack, AgentRunEvent, ExecutionSandbox, or PlanExecutor.
- AgentSpec may constrain tools/context/prompts, but it must not grant authority beyond existing runtime policy.
- New behavior must have focused tests.
- Run the task-specific verification commands.
- Final report must include changed files, tests run, results, and residual risks.

## P6-0: Documentation And Entry Sync

Goal:

Make P6 discoverable and AI-coding-ready.

Expected behavior:

- `AGENTS.md` references P6 task specs.
- Migration plan references P6 as the next planning source after P5.
- Test matrix includes P6 acceptance and test gates.
- Agent coding prompts include P6 global prompt and P6 task prompts.

Allowed edit areas:

- `AGENTS.md`
- `plans/openlife_vnext_p6_task_specs.md`
- `plans/openlife_vnext_migration_plan.md`
- `plans/openlife_vnext_test_and_acceptance_matrix.md`
- `plans/openlife_vnext_agent_coding_prompts.md`

Constraints:

- Documentation only.
- Do not change Rust or TypeScript code.

Verification:

- `rg -n "openlife_vnext_p6_task_specs|P6-0|P6-1|P6-2|P6-3|P6-4|P6-5|P6-6|P6-7|AgentSpec-Governed Runtime" AGENTS.md plans`
- `git diff --name-only` contains documentation files only.

## P6-1: AgentSpec Core Contract

Goal:

Define a stable core `AgentSpec` contract for governed runtime identity.

Expected behavior:

- AgentSpec describes an agent as a governed runtime unit, not only a prompt.
- AgentSpec includes at minimum:
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
- Existing sub-agent-specific concepts must not become a second competing agent model.
- Serialization is camelCase for frontend-facing contracts where applicable.

Allowed edit areas:

- `openlife-core/src/agent/types.rs`
- `openlife-core/src/agent/mod.rs`
- relevant focused tests under `openlife-core/src/agent/`

Constraints:

- Do not wire AgentSpec into execution yet.
- Do not implement SubAgentRuntime changes.
- Do not add UI.

Verification:

- `cargo test -p openlife-core agent`
- `cargo check -q`

Required tests:

- AgentSpec serde round-trip.
- default/main AgentSpec can be constructed.
- tool allow/deny policy fields preserve order and values.
- output schema reference is optional.

## P6-2: AgentTask Contract

Goal:

Define `AgentTask` as the formal intent-and-constraints object before execution.

Expected behavior:

- AgentTask is separate from AgentRun trace.
- AgentTask includes at minimum:
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
- AgentTask must not contain raw LifeModel or raw memory payloads.

Allowed edit areas:

- `openlife-core/src/agent/types.rs`
- `openlife-core/src/agent/mod.rs`
- relevant focused tests under `openlife-core/src/agent/`

Constraints:

- Do not replace all runtime entrypoints in this task.
- Do not implement scheduling/proactive migration.
- Do not add UI.

Verification:

- `cargo test -p openlife-core agent`
- `cargo check -q`

Required tests:

- AgentTask serde round-trip.
- AgentTask can reference AgentSpec id.
- AgentTask privacy/workspace fields do not require raw context payloads.

## P6-3: ContextPolicy And ContextAssembler

Goal:

Introduce a minimal governed context assembly path.

Expected behavior:

- ContextPolicy determines which context categories are eligible:
  - LifeModel summary
  - goals
  - state
  - memory snippets
  - current session summary
  - tool observations
- ContextAssembler returns a structured result containing:
  - included context categories
  - excluded context categories
  - privacy/redaction notes
  - compact summary suitable for AgentRunEvent
- AgentSpec may request context, but ContextAssembler decides what is actually included under policy.
- Context assembly records or can produce traceable metadata without storing sensitive raw payloads in event summaries.

Allowed edit areas:

- `openlife-core/src/agent/context_assembler.rs` or equivalent new module
- `openlife-core/src/agent/types.rs`
- `openlife-core/src/agent/mod.rs`
- relevant focused tests under `openlife-core/src/agent/`

Constraints:

- Do not rewrite existing chat context assembly wholesale.
- Do not expose raw memory or full LifeModel in AgentRunEvent payloads.
- Do not call LLMs.

Verification:

- `cargo test -p openlife-core agent`
- `cargo check -q`

Required tests:

- LifeModel summary can be included when policy allows.
- memory snippets are excluded when policy denies memory.
- privacy note appears when sensitive context is omitted.
- event-safe summary does not include raw sensitive text.

## P6-4: AgentSpec Tool Policy Enforcement

Goal:

Apply AgentSpec tool policy before ActionExecutor executes a tool.

Expected behavior:

- A tool must satisfy both:
  - existing ToolRuntime / ActionExecutor / Permission policy
  - AgentSpec allowed/denied tool policy
- Denied-by-AgentSpec tool attempts are blocked and recorded as AgentRunEvent.
- AgentSpec cannot allow a tool that ToolRuntime would otherwise block.
- Declarative-only tools remain non-executable.

Allowed edit areas:

- `openlife-core/src/agent/action_executor/`
- `openlife-core/src/agent/types.rs`
- `openlife-core/src/agent/event_store.rs` only if new event mapping is needed
- relevant focused tests under `openlife-core/src/agent/`

Constraints:

- Do not bypass existing permission/proposal behavior.
- Do not add new tool executors.
- Do not add UI.

Verification:

- `cargo test -p openlife-core agent`
- `cargo check -q`

Required tests:

- AgentSpec-allowed read tool can execute if ToolRuntime allows it.
- AgentSpec-denied tool is blocked before execution.
- AgentSpec allow does not bypass permission for write/external side-effect tools.
- blocked attempt records an AgentRunEvent or event-ready outcome.

## P6-5: PromptStack Binding

Goal:

Bind AgentSpec prompt references to PromptStack assembly.

Expected behavior:

- AgentSpec references prompt blocks by id/version or equivalent stable identifiers.
- PromptStack assembly can consume AgentSpec prompt references.
- AgentRunEvent records prompt block ids/versions, not sensitive full prompt text.
- Cloud-disallowed prompt blocks remain filterable under privacy policy.

Allowed edit areas:

- `openlife-core/src/agent/prompt_stack.rs` or existing PromptStack module
- `openlife-core/src/agent/types.rs`
- `openlife-core/src/agent/event_store.rs` only if event payload helpers are needed
- relevant focused tests under `openlife-core/src/agent/`

Constraints:

- Do not introduce ad hoc prompt fragments.
- Do not call LLMs.
- Do not change frontend UI.

Verification:

- `cargo test -p openlife-core agent`
- `cargo check -q`

Required tests:

- AgentSpec prompt block ids are assembled through PromptStack.
- missing prompt block is reported as structured error.
- event metadata contains block ids/versions only.
- cloud-disallowed block is excluded or summarized according to policy.

## P6-6: PlanExecutor Uses AgentSpec

Goal:

Carry AgentSpec policy into confirmed plan execution.

Expected behavior:

- Plan execution context can include an AgentSpec or AgentSpec id.
- Plan step tool intent must satisfy AgentSpec tool policy.
- If the plan intent and AgentSpec policy disagree, execution blocks and records a traceable event.
- Existing plan deviation, cancellation, retry, continuation, and review behavior remains stable.

Allowed edit areas:

- `openlife-core/src/agent/plan_executor.rs`
- `openlife-core/src/agent/types.rs`
- `src-tauri/src/commands/plan.rs` only for minimal context wiring
- relevant focused tests

Constraints:

- Do not change PlanExecutor into parallel execution.
- Do not bypass ActionExecutor.
- Do not rewrite plan commands.
- Do not add UI.

Verification:

- `cargo test -p openlife-core agent::plan_executor`
- `cargo test -p openlife-tauri commands::plan`
- `cargo check -q`

Required tests:

- plan step with AgentSpec-allowed tool executes.
- plan step with AgentSpec-denied tool is blocked.
- AgentSpec block records event.
- cancellation/retry/review existing tests remain green.

## P6-7: Minimal Runtime Trace Exposure

Goal:

Expose AgentSpec governance decisions minimally in existing trace UI.

Expected behavior:

- Run trace can display AgentSpec id/role when available.
- Tool blocked by AgentSpec has readable trace summary.
- Empty AgentSpec metadata does not clutter UI.
- Existing ChatPage streaming, proposal banner, plan operations, and RunTracePanel behavior remain stable.

Allowed edit areas:

- `frontend/src/types.ts`
- `frontend/src/components/RunTracePanel.tsx`
- `frontend/src/pages/ChatPage.tsx` only for minimal integration
- `frontend/src/test/mocks/tauri.ts`
- focused frontend tests

Constraints:

- Do not build a full AgentSpec editor.
- Do not redesign ChatPage.
- Do not change backend API unless the event contract requires a small typed addition.

Verification:

- `pnpm --dir frontend typecheck`
- `pnpm --dir frontend test -- --run RunTracePanel ChatPage tauri`
- `cargo check -q` if backend event contract changes.

Required tests:

- trace renders AgentSpec id/role when present.
- AgentSpec-denied tool event renders readable summary.
- empty AgentSpec metadata does not render extra UI.
- existing ChatPage tests remain green.

## P6 Completion Gate

P6 is complete when:

- AgentSpec and AgentTask are stable contracts.
- ContextAssembler can produce event-safe context summaries.
- AgentSpec tool policy cannot bypass ToolRuntime or Permission.
- PromptStack binds AgentSpec prompt references without ad hoc prompts.
- PlanExecutor respects AgentSpec policy.
- Minimal trace UI can surface AgentSpec governance decisions.

Required final verification:

- `cargo test -p openlife-core agent`
- `cargo test -p openlife-tauri commands::plan`
- `pnpm --dir frontend typecheck`
- `pnpm --dir frontend test -- --run RunTracePanel ChatPage tauri`
- `cargo check -q`
