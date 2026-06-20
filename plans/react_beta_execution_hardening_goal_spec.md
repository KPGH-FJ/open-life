# ReAct Beta Execution Hardening Goal Spec

> Status: Completed CLI Goal-mode implementation spec / audit trail for W114-W123
> Date: 2026-06-04
> Scope: harden the ReAct execution spine toward Beta without migrating default Chat

## 1. Summary

This Goal prepared the implementation block after W113 RuntimeStrategy /
Multi-Strategy Runtime Maturity. References to W113 as "current" below describe
the historical baseline at Goal start.

The current code already has important Alpha+ pieces:

- `AgentLoop` can generate model output, parse JSON `actions` / `tool_calls`,
  call `ActionExecutor`, append observations, enforce step/tool budgets, and
  stop on permission.
- `ActionExecutor` is split into core OS tools, execution tools, declarative
  stubs, and helper logic.
- `McpRegistry` exposes `ToolManifest` metadata and P1/P2 tool taxonomy.
- Tool permission proposals and replay exist through Review Center and
  `replay_agent_action`.
- W90-W97 removed high-risk legacy direct writes.
- W98-W105 created a non-default Plan-Execute product vertical.
- W106-W113 made ReAct and PlanExecute descriptor/registry-ready strategies,
  with metadata-safe selection/execution reports and trace vocabulary.

At Goal start, the W113 code was not yet ReAct Beta-ready. W114-W123 hardened
the execution spine so ReAct action planning, tool registry authority, action /
observation traces, permission/replay, and proposal-first writes became a more
stable contract.

This Goal is explicitly not a default Chat route cutover. It may harden existing
ReAct/AgentLoop internals and non-default preview/status surfaces, but it must
not replace ordinary `send_message` / `start_stream_message` or treat any
readiness report as migration permission.

## 2. Objective

Complete W114-W123: ReAct Beta Execution Hardening.

At the end of the Goal:

- ReAct has a machine-readable Beta readiness report over execution core,
  tool registry, permission/replay, proposal-first writes, trace visibility, and
  default Chat isolation.
- AgentLoop action parsing has a stable schema and fail-soft repair path.
- Tool registry readiness proves executable, proposal-only, disabled, and
  declarative-only tools are correctly classified.
- ActionExecutor uses manifest authority consistently and blocks unknown,
  disabled, declarative-only, unsafe, or over-budget tools.
- AgentRun action/observation timeline has a stable metadata-safe envelope.
- ToolPermission proposals and replay preserve canonical blocked action scope.
- Internal writes and external side effects remain proposal-first.
- Runs/Trace surfaces can inspect ReAct action lifecycle without raw payload
  leakage.
- Docs and progress index are synchronized to W123.

## 3. Non-Negotiable Constraints

- Do not migrate default Chat.
- Do not replace ordinary `send_message` or `start_stream_message`.
- Do not use W114-W123 readiness/status as migration permission.
- Do not add automatic Chat route cutover, automatic controlled adapter
  attachment, or automatic default Chat RuntimeStrategy routing.
- Do not weaken W49-W72 default Chat adapter guard stack.
- Do not directly write durable LifeModel-HS truth from ReAct.
- Do not silently write Memory, files, calendar, email, external providers, MCP,
  A2A, plugin state, or tool permission state outside explicit governed paths.
- Do not mark plugin tools executable unless a real local executor exists.
- Do not let broad tools prompt text imply write/external side-effect intent.
- Do not store raw prompt, raw assistant output, raw tool payload, raw memory
  context, raw LifeModel text, raw file contents, raw web contents, raw email
  body, or PII in readiness/status/debug reports.
- Do not commit or push from the implementation Agent.

## 4. Current Baseline Facts To Preserve

Current latest commit before this Goal:

```text
7057934 Complete runtime strategy maturity
```

Current completed blocks:

- W90-W97 Legacy Direct-Write Convergence complete.
- W98-W105 Plan-Execute Product Vertical complete.
- W106-W113 RuntimeStrategy / Multi-Strategy Runtime Maturity complete.

Current routing boundary:

- default Chat remains `legacy_stream`.
- Ordinary `send_message` / `start_stream_message` must not call non-default
  readiness/status/migration surfaces.

Current relevant code:

- `openlife-core/src/agent/agent_loop.rs`
- `openlife-core/src/agent/action_executor/`
- `openlife-core/src/agent/runtime.rs`
- `openlife-core/src/agent/runtime_contract.rs`
- `openlife-core/src/agent/strategy_runtime.rs`
- `openlife-core/src/mcp.rs`
- `openlife-core/src/tool_manifest.rs`
- `openlife-core/src/tool_permissions.rs`
- `src-tauri/src/commands/agent.rs`
- `src-tauri/src/commands/proposal.rs`
- `frontend/src/pages/AgentRunDetail.tsx`
- `frontend/src/components/ToolCallCard.tsx`

## 5. Stage Order

Implement W114-W123 in one sustained Goal run, keeping this internal order:

1. W114 ReAct Beta readiness contract.
2. W115 AgentLoop action schema and parser hardening.
3. W116 Tool registry Beta taxonomy and readiness.
4. W117 ActionExecutor manifest-authority hardening.
5. W118 AgentRun action/observation trace envelope.
6. W119 Permission proposal and replay hardening.
7. W120 Proposal-first internal/external write hardening.
8. W121 Non-default ReAct Beta execution/status harness.
9. W122 Runs/Trace UI hardening for action lifecycle.
10. W123 Docs/progress/verification sync.

## 6. W114 Spec: ReAct Beta Readiness Contract

### Scope

Primary files:

- `openlife-core/src/agent/agent_loop.rs`
- `openlife-core/src/agent/action_executor/`
- `openlife-core/src/agent/tests/`
- optionally a new focused core module if local patterns support it

### Required Behavior

Add a pure, metadata-safe ReAct Beta readiness report/evaluator. Suggested
names:

- `ReactBetaExecutionReadinessReport`
- `evaluate_react_beta_execution_readiness`
- `ensure_react_beta_execution_readiness`

The report must cover:

- `react_loop_present`
- `action_schema_ready`
- `tool_registry_ready`
- `action_executor_manifest_authority_ready`
- `agent_run_trace_ready`
- `permission_replay_ready`
- `proposal_first_writes_ready`
- `runs_trace_surface_ready`
- `default_chat_unchanged`
- `migration_permission=false`
- `runtime_strategy_ready=true` when W113 descriptors remain ready
- `blocking_reasons`
- metadata-safe summary

The evaluator must be pure. It must not:

- run model/runtime/tool calls
- create AgentRun/Proposal/Evidence/Memory/LifeModel/MCP audit/Chat records
- inspect raw current user input
- require network/file/provider availability

### Required Tests

- Clean current code reports readiness or clear blockers.
- Missing action schema readiness fails closed.
- Missing registry readiness fails closed.
- Missing permission/replay readiness fails closed.
- Report serialization excludes raw prompt/output/tool/memory/LifeModel/file
  data and PII.
- Readiness result is not migration permission.

## 7. W115 Spec: AgentLoop Action Schema And Parser Hardening

### Scope

Primary files:

- `openlife-core/src/agent/agent_loop.rs`
- `openlife-core/src/agent/tests/integration.rs`
- `openlife-core/src/agent/tests/runtime_convergence.rs`
- focused `react_beta_*` tests

### Required Behavior

Introduce or formalize a typed action request schema that maps model JSON into
`AgentActionRequest` without ad hoc ambiguity.

Accepted model shapes:

```json
{
  "final": "optional user-facing text before/after actions",
  "actions": [
    {
      "name": "memory.search",
      "arguments": { "query": "metadata-safe query" },
      "action_type": "mcp_tool"
    }
  ],
  "warnings": ["optional metadata-safe warning"],
  "thought_summary": "metadata-safe summary only"
}
```

Legacy `tool_calls` may remain supported, but it must normalize into the same
typed request shape.

Parser hardening requirements:

- Empty or no-action model output remains final-only.
- Invalid JSON records parse warnings and triggers existing one-shot repair.
- Missing tool name fails soft with metadata-safe error, not panic.
- Invalid arguments default to `{}` only when safe and recorded as warning.
- `action_type` defaults to `mcp_tool` only for registered tool-like actions.
- Tool names must be normalized by ActionExecutor or registry, not by raw string
  hacks in multiple places.
- `step_index` and `tool_call_count` remain deterministic.
- Raw model reply must not be copied into trace/readiness reports.

### Required Tests

- New `actions` schema parses to `AgentActionRequest`.
- Legacy `tool_calls` schema still parses.
- Invalid JSON attempts repair/fail-soft and records warning.
- Missing name is metadata-safe and does not panic.
- Broad tools prompt alone does not create actions.
- Raw prompt/assistant output/PII are absent from parser reports/debug dumps.

## 8. W116 Spec: Tool Registry Beta Taxonomy And Readiness

### Scope

Primary files:

- `openlife-core/src/mcp.rs`
- `openlife-core/src/tool_manifest.rs`
- `openlife-core/src/agent/action_executor/`
- `openlife-core/src/agent/tests/runtime_integration.rs`
- docs taxonomy references as needed

### Required Behavior

Add a machine-readable tool registry Beta readiness report. Suggested names:

- `ToolRegistryBetaReadinessReport`
- `evaluate_tool_registry_beta_readiness`
- `ensure_tool_registry_beta_readiness`

The report must classify the Beta tool set:

- executable read tools
- proposal-only governed executors
- permission-gated executors
- disabled/declarative-only stubs
- unsupported/missing tools

Required taxonomy states:

| Tool | Required state |
| --- | --- |
| `life_model.read` | executable low-risk read |
| `life_model.propose_patch` | proposal-first only |
| `goal.read` | executable low-risk read |
| `memory.search` | executable low-risk read |
| `memory.propose_write` | proposal-first only |
| `memory.propose_archive` | proposal-first only |
| `proposal.list` | executable low-risk read |
| `agent_run.lookup` | executable low-risk read |
| `permission.check` | executable low-risk read |
| `permission.request` | proposal-first permission request |
| `permission.replay_action` | permission-gated replay helper |
| `mcp.call_tool` | permission/manifest-gated wrapper |
| `a2a.call_agent` | permission/manifest-gated or blocked by provider policy |
| `file.read` | executable only inside safe paths |
| `file.write_proposal` | ExternalWriteAction proposal-only |
| `web.search` | network policy gated read |
| `web.fetch` | network policy gated read |
| `calendar.read` | read-only configured ICS/scope |
| `calendar.propose_event` | ScheduledTask proposal-only |
| `email.read` | disabled/declarative-only unless real executor exists |
| `email.propose_draft` | DataExport proposal-only |
| `task.create_proposal` | governed task/proposal path, no silent external write |
| plugin-declared tools | disabled/declarative-only unless real local executor exists |

Readiness must fail closed when:

- a required P1 tool is missing
- a proposal-only tool appears as direct external write executor
- a disabled/declarative-only tool appears executable
- an unknown tool is treated as confirmation-needed instead of blocked
- a plugin tool appears executable without executor evidence
- calendar/email proposal tools create `ExternalWriteAction` fallback

### Required Tests

- Tool taxonomy report covers all required tool ids.
- `calendar.propose_event` and `email.propose_draft` remain proposal-only.
- `email.read` remains declarative-only unless a real executor exists.
- Unknown/disabled/declarative-only tools block.
- Plugin tools without executor are not executable.
- Report is metadata-safe.

## 9. W117 Spec: ActionExecutor Manifest Authority Hardening

### Scope

Primary files:

- `openlife-core/src/agent/action_executor/`
- `openlife-core/src/mcp.rs`
- `openlife-core/src/tool_manifest.rs`
- `openlife-core/src/agent/tests/runtime_integration.rs`

### Required Behavior

ActionExecutor must treat `ToolManifest` as the authority for execution.

Requirements:

- Every tool execution path resolves a manifest before execution.
- Unknown tools are blocked, not converted into confirmation requests.
- Disabled/declarative-only tools are blocked or proposal-only as explicitly
  governed.
- Built-in execution tools call the local execution handler through the same
  manifest-governed path.
- MCP/A2A/plugin routing does not bypass permission checks.
- Safe path checks remain before file reads/writes.
- Network policy checks remain before web/A2A/network calls.
- `allow_writes=false` blocks direct writes and allows only proposal creation
  where the proposal path is explicitly safe.
- `max_tool_calls` and budget stops are traceable.
- Error messages do not echo raw arguments or PII.

### Required Tests

- Low-risk read tool executes and creates action/observation.
- Unknown tool blocks without ToolPermission proposal.
- Declarative-only tool blocks or creates explicit governed proposal where
  allowed.
- File read outside safe paths blocks.
- Web fetch private/local URL blocks.
- Direct external write becomes proposal-first under HS policy.
- `allow_writes=false` blocks direct write-like execution.
- All action/observation records use metadata-safe output previews.

## 10. W118 Spec: AgentRun Action/Observation Trace Envelope

### Scope

Primary files:

- `openlife-core/src/agent/types.rs`
- `openlife-core/src/agent/agent_loop.rs`
- `openlife-core/src/agent/store.rs`
- `frontend/src/pages/AgentRunDetail.tsx`
- `frontend/src/components/ToolCallCard.tsx`

### Required Behavior

Define a stable metadata-safe ReAct trace envelope for each action and
observation.

The envelope should expose:

- run id
- action id
- step index
- tool call index
- action type
- tool id/name/source
- action type category: read/write/network/external_side_effect/proposal
- risk level
- permission decision
- status: succeeded/failed/blocked/needs_confirmation
- proposal id if created
- observation id
- observation status
- output preview/hash/counts, not raw payload
- timing metadata

Do not remove existing `AgentAction` / `AgentObservation` compatibility fields
unless all frontend and tests are updated.

### Required Tests

- Successful tool call creates trace envelope.
- Blocked permission creates trace envelope.
- Proposal-created tool call links proposal id.
- Failed tool call records recoverable metadata-safe error.
- Serialized run trace omits raw prompt, raw tool payload, raw file content,
  raw web content, memory context, and PII.

## 11. W119 Spec: Permission Proposal And Replay Hardening

### Scope

Primary files:

- `openlife-core/src/agent/action_executor/tool_executor.rs`
- `openlife-core/src/tool_permissions.rs`
- `src-tauri/src/commands/agent.rs`
- `src-tauri/src/commands/proposal.rs`
- frontend replay surfaces/tests as needed

### Required Behavior

Permission/replay must be canonical and replay-safe.

Requirements:

- Blocked high-risk/permission-needed actions create exactly one relevant
  ToolPermission proposal per canonical blocked action when possible.
- Proposal payload includes canonical tool scope:
  - tool name/id
  - source
  - risk level
  - action type
  - capabilities
  - blocked action id/run id
  - input hash/length/metadata-safe preview, not raw payload when risky
- Accepting ToolPermission stores canonical policy.
- Replay uses the original action identity and canonical scope.
- Replay does not consume `allow_once` during pre-check; actual execution
  policy behavior must be explicit and tested.
- Replay cannot run unknown/disabled/declarative-only tools.
- Replay updates the original action and observation in the original run.
- Replay status cannot mark the run completed if other actions remain pending.

### Required Tests

- High-risk tool creates ToolPermission proposal.
- Accept proposal then replay succeeds for allowed scope.
- Replay before approval fails.
- Replay after deny fails.
- Replay of unknown/declarative-only tool fails.
- Replay does not duplicate actions.
- Replay does not leak raw input or PII.

## 12. W120 Spec: Proposal-First Internal And External Write Hardening

### Scope

Primary files:

- `openlife-core/src/agent/action_executor/core_os_tools.rs`
- `openlife-core/src/agent/action_executor/execution_tools.rs`
- `openlife-core/src/agent/action_executor/declarative_stubs.rs`
- `src-tauri/src/commands/proposal.rs`
- existing proposal tests

### Required Behavior

ReAct write-like actions must converge to proposal-first semantics.

Requirements:

- `life_model.propose_patch` creates typed LifeModel proposal only.
- `memory.propose_write` creates MemoryWrite proposal only.
- `memory.propose_archive` creates MemoryArchive proposal only.
- `file.write_proposal` creates ExternalWriteAction proposal only and enforces
  pre-insert size limit and payload minimization.
- `calendar.propose_event` creates ScheduledTask proposal only.
- `email.propose_draft` creates DataExport proposal only.
- `task.create_proposal` follows governed task/proposal semantics and does not
  mutate external systems.
- Proposal application paths remain explicit Review Center actions.
- LifeModel proposal acceptance still creates required snapshots and PatchStore
  source mappings.

### Required Tests

- Each proposal-first tool creates the expected proposal type.
- No proposal-first tool performs external provider/file/calendar/email writes
  directly.
- ExternalWriteAction size limit is enforced before proposal insert.
- Proposal payloads are minimized where required.
- Accepted proposals still use source-specific patch/source mapping.

## 13. W121 Spec: Non-Default ReAct Beta Execution/Status Harness

### Scope

Primary files:

- `src-tauri/src/commands/agent_runtime/`
- `src-tauri/src/lib.rs`
- `frontend/src/tauri.ts`
- optional Settings diagnostic panel only if low-risk

### Required Behavior

Add explicit non-default ReAct Beta diagnostic/status command(s) only if useful.
Suggested command:

```text
get_react_beta_execution_status
```

The command must:

- return W114 readiness plus tool registry readiness and default Chat isolation
- be read-only
- run no runtime/model/tool calls
- write no AgentRun/Proposal/Evidence/Memory/LifeModel/MCP audit/Chat records
- be absent from ordinary Chat send/stream paths
- be clearly not migration permission

If adding a non-default execution harness, it must be explicit, write-disabled
by default, and must not replace ordinary Chat.

### Required Tests

- Status command read-only side-effect counts unchanged.
- Status command output metadata-safe.
- Ordinary `send_message` / `start_stream_message` do not call W114-W123
  commands/helpers.
- Frontend wrapper does not run from Chat main send path.

## 14. W122 Spec: Runs/Trace UI Hardening

### Scope

Primary files:

- `frontend/src/pages/AgentRunDetail.tsx`
- `frontend/src/components/ToolCallCard.tsx`
- `frontend/src/components/RunTracePanel.tsx`
- `frontend/src/pages/RunsPage.tsx`
- frontend tests/mocks

### Required Behavior

Runs UI should make ReAct action lifecycle understandable without pretending
unfinished tools are production-ready.

Requirements:

- Action timeline shows status, tool source, risk, permission decision,
  proposal link, replay availability, and observation summary.
- Replay button appears only when action is replayable.
- Disabled/declarative-only/unknown tool states are displayed as blocked.
- Proposal-created actions link to Review Center when possible.
- Safe Mode or blocked states must not be hidden.
- UI must not render raw tool payload, raw prompt, raw memory context, raw file
  content, raw web content, raw email body, or PII.

### Required Tests

- Tool action card renders succeeded, blocked, needs_confirmation, and failed
  states.
- Proposal links render for proposal-created actions.
- Replay affordance only appears for replayable actions.
- Runs search can find tool/source/status metadata.
- Raw payload/PII does not render.

## 15. W123 Spec: Docs, Progress Index, And Verification Sync

### Scope

Primary files:

- `AGENTS.md`
- `plans/README.md`
- `plans/lifemodel_governed_runtime_progress.md`
- `plans/openlife_lifemodel_governed_agent_runtime.md`
- `plans/openlife_development_plan.md`
- `plans/openlife_react_beta_roadmap.md`
- this file

### Required Behavior

Update docs from W113 baseline to W123 ReAct Beta Execution Hardening complete.

State explicitly:

- W114-W123 is ReAct Beta execution hardening.
- W114-W123 is not default Chat migration.
- default Chat remains `legacy_stream`.
- ReAct action/tool execution is harder and more inspectable, but full Beta may
  still require Skill Runtime, ModelRouter/Privacy, and product golden path.
- Tool taxonomy matches code state.
- Readiness/status reports are not migration permission.

### Required Tests

- `rg` checks confirm docs mention W123 consistently.
- `rg` checks confirm old W113-only current status is scoped as historical.
- `rg` checks confirm no stale P1/P2 tool taxonomy contradictions.
- `git diff --check` passes.
- `make ci` passes.

## 16. Final Verification Matrix

Run targeted tests first, then full CI.

Minimum required commands:

```bash
cargo test -p openlife-core react_beta -- --nocapture
cargo test -p openlife-core agent_loop -- --nocapture
cargo test -p openlife-core action_executor -- --nocapture
cargo test -p openlife-core runtime_integration -- --nocapture
cargo test -p openlife-core runtime_convergence -- --nocapture
cargo test -p openlife-tauri agent -- --nocapture
cargo test -p openlife-tauri proposal -- --nocapture
cargo test -p openlife-tauri react_beta -- --nocapture
cargo test -p openlife-tauri default_chat_entrypoints_do_not_call_w19_w60_command_surfaces -- --nocapture
cd frontend && pnpm test -- --run
git diff --check
make ci
```

If no `react_beta` test filter exists at the start of the Goal, create focused
tests whose names include `react_beta` so the command is valid.

Before handoff, also run focused search checks:

```bash
rg -n "react_beta|ReactBeta|W123|ReAct Beta Execution Hardening" AGENTS.md plans openlife-core/src src-tauri/src frontend/src
rg -n "calendar.propose_event|email.propose_draft|ExternalWriteAction|declarative_only|plugin" AGENTS.md plans openlife-core/src src-tauri/src
rg -n "send_message|start_stream_message|get_react_beta_execution_status|ReactBeta" src-tauri/src/lib.rs
rg -n "default Chat.*migration permission|readiness.*migration permission|status.*migration permission" AGENTS.md plans
```

Use the search results to prove docs are synchronized, tool taxonomy matches
code, and ordinary Chat entrypoints do not invoke W114-W123 helpers or commands.

## 17. Handoff Output Requirements

When the implementation Agent finishes, it must output:

- W114-W123 change summary by W-slice
- new core interfaces/reports
- new commands or frontend wrappers/surfaces
- tests run and results
- skipped tests with reason
- residual risks
- whether W123 is complete
- whether the next big block can start

The implementation Agent must not commit or push.

## 18. Historical Copyable CLI Goal Prompt

```text
You are implementing W114-W123: ReAct Beta Execution Hardening.

Read and strictly follow:
- AGENTS.md
- plans/README.md
- plans/openlife_lifemodel_governed_agent_runtime.md
- plans/openlife_react_beta_roadmap.md
- plans/react_beta_execution_hardening_goal_spec.md

Current baseline at Goal start:
- Latest completed block is W113 RuntimeStrategy / Multi-Strategy Runtime Maturity.
- Latest pushed commit before this Goal is 7057934 Complete runtime strategy maturity.
- default Chat remains legacy_stream.
- Ordinary send_message/start_stream_message must not be replaced or migrated.
- W114-W123 is not default Chat migration and not migration permission.

Implement W114-W123 in one sustained Goal run, keeping the internal order:
1. W114 ReAct Beta readiness contract.
2. W115 AgentLoop action schema and parser hardening.
3. W116 Tool registry Beta taxonomy and readiness.
4. W117 ActionExecutor manifest-authority hardening.
5. W118 AgentRun action/observation trace envelope.
6. W119 Permission proposal and replay hardening.
7. W120 Proposal-first internal/external write hardening.
8. W121 Non-default ReAct Beta execution/status harness.
9. W122 Runs/Trace UI hardening for action lifecycle.
10. W123 docs/progress/verification sync.

Hard constraints:
- Do not migrate default Chat.
- Do not replace ordinary send_message or start_stream_message.
- Do not grant migration permission.
- Do not silently write LifeModel/Memory/files/calendar/email/external providers/plugin state.
- Do not make plugin tools executable without a real local executor.
- Keep calendar.propose_event and email.propose_draft proposal-only governed executors.
- Enforce ExternalWriteAction pre-insert size limit and payload minimization.
- Keep readiness/status outputs metadata-safe and raw-content-free.
- Add/keep tests that prove ordinary Chat does not call W114-W123 status/readiness helpers.

Minimum verification:
- cargo test -p openlife-core react_beta -- --nocapture
- cargo test -p openlife-core agent_loop -- --nocapture
- cargo test -p openlife-core action_executor -- --nocapture
- cargo test -p openlife-core runtime_integration -- --nocapture
- cargo test -p openlife-core runtime_convergence -- --nocapture
- cargo test -p openlife-tauri agent -- --nocapture
- cargo test -p openlife-tauri proposal -- --nocapture
- cargo test -p openlife-tauri react_beta -- --nocapture
- cargo test -p openlife-tauri default_chat_entrypoints_do_not_call_w19_w60_command_surfaces -- --nocapture
- cd frontend && pnpm test -- --run
- git diff --check
- make ci

If a react_beta filter does not exist yet, create focused tests with react_beta
in the test names.

Finish by outputting:
- W114-W123 change summary
- new interfaces/reports/commands
- tests run and results
- residual risks
- whether W123 is complete
- whether the next big block can start

Do not commit. Do not push.
```
