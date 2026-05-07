# OpenLife vNext P9 Task Specifications

Date: 2026-05-07

Status: draft

Package:

```text
ExecutionSandbox-Governed Shell Execution
```

P9 introduces a narrowly scoped, non-interactive shell capability only after the P0-P8 runtime guardrails are in place. The key goal is not "give the model a terminal"; it is to make any future command execution pass through `ExecutionSandbox`, `ToolRuntime`, `AgentSpec`, `Permission`, `Proposal`, and append-only `AgentRunEvent`.

P9 starts from the current repository fact that `openlife-core/src/agent/execution_sandbox.rs` already exists with policy types and validation tests. Therefore P9 should focus on hardening, wiring, default-off manifests, traceability, and a minimal command executor. It must not introduce interactive terminals, arbitrary `/bin/sh -c` execution, scheduled shell automation, sub-agent shell access, or direct write side effects.

## Baseline Review

Before P9:

- P8 compaction is ready to close: compaction policy, summary builder, `compaction.created`, AgentLoop hook, and minimal trace surface exist.
- `cargo test -p openlife-core agent::compaction --lib` passes.
- `cargo test -p openlife-core agent::agent_loop --lib` passes.
- `cargo test -p openlife-core agent::event_store --lib` passes.
- `pnpm --dir frontend test -- --run RunTracePanel tauri` passes.
- `cargo check -q` passes.
- `ExecutionSandbox` already defines:
  - `cwd`
  - `safe_paths`
  - `deny_read_patterns`
  - `deny_write_patterns`
  - `network_policy`
  - `write_policy`
  - `timeout_ms`
  - `max_output_bytes`
  - `env_allowlist`
  - `command_allowlist`
  - `dangerous_command_denylist`
  - `bash_enabled`
- `cargo test -p openlife-core agent::execution_sandbox --lib` passes.
- No shell tool manifest, executor, ActionExecutor branch, Tauri config wiring, or frontend setting should be assumed complete.

## Global Rules

- Execute exactly one P9 task spec at a time.
- Shell is default-off.
- Do not expose an interactive shell.
- Do not use `/bin/sh -c`, `cmd /C`, or arbitrary command strings in the first executor.
- Initial execution shape must be structured: `command`, `args`, `cwd`, `env`.
- Do not allow pipes, redirects, command substitution, glob expansion, chained commands, or shell metacharacter bypasses in the first executor.
- Do not enable shell for normal chat, scheduled/proactive tasks, or sub-agents by default.
- Shell must enter through ToolRuntime/ActionExecutor only.
- AgentSpec can further deny shell but must not grant authority beyond sandbox/runtime policy.
- Every shell attempt must record append-only events.
- Writes remain proposal-first. P9 must not directly write files through shell execution.
- No shell output should exceed configured limits.
- Environment variables must be allowlisted by name and passed explicitly.
- Secret paths are denied even inside safe paths.
- Tests must not depend on host-specific commands beyond a tiny cross-platform allowlist or platform-gated cases.
- Final reports must include changed files, tests run, results, and residual risks.

## P9-0: Documentation And Entry Sync

Goal:

Make P9 discoverable and AI-coding-ready.

Expected behavior:

- `AGENTS.md` and `README.md` state that the current phase is P9 Shell/Sandbox after P8 compaction closure.
- Migration plan and test matrix align with the conservative P9 scope.
- Agent coding prompts include P9 global and task prompts.
- P9 explicitly excludes interactive shell, arbitrary shell strings, scheduled shell, sub-agent shell, and direct writes.

Allowed edit areas:

- `AGENTS.md`
- `README.md`
- `plans/openlife_vnext_p9_task_specs.md`
- `plans/openlife_vnext_migration_plan.md`
- `plans/openlife_vnext_test_and_acceptance_matrix.md`
- `plans/openlife_vnext_agent_coding_prompts.md`

Constraints:

- Documentation only.
- Do not change Rust or TypeScript code.

Verification:

- `rg -n "openlife_vnext_p9_task_specs|P9-0|P9-1|P9-2|P9-3|P9-4|P9-5|P9-6|P9-7|ExecutionSandbox-Governed Shell" AGENTS.md README.md plans`
- `git diff --name-only` contains documentation files only for this task.

## P9-1: Sandbox Contract Hardening

Goal:

Promote `ExecutionSandbox` from existing skeleton to the stable P9 policy contract.

Expected behavior:

- Sandbox validation has one canonical path for read/write operand checks.
- Legacy prefix-only helpers are not used for new shell behavior.
- Deny patterns take priority over safe paths.
- Dangerous command denylist takes priority over allowlist.
- Shell metacharacters are rejected before execution.
- The default policy remains disabled.
- Structured validation errors are stable enough for ToolRuntime observations.

Allowed edit areas:

- `openlife-core/src/agent/execution_sandbox.rs`
- `openlife-core/src/agent/mod.rs`
- focused tests

Constraints:

- No shell executor.
- No manifest registration.
- No Tauri settings.

Verification:

- `cargo test -p openlife-core agent::execution_sandbox --lib`
- `cargo check -q`

Required tests:

- default sandbox remains shell-disabled.
- deny-read blocks `.env`, private keys, and credentials paths even under safe paths.
- dangerous command denylist wins over allowlist.
- unknown commands are blocked when allowlist is non-empty.
- shell metacharacters are rejected.
- cwd outside safe paths is rejected.
- relative parent traversal is rejected.

## P9-2: Shell Tool Manifest Default-Off

Goal:

Introduce a manifest contract for shell without making it executable.

Expected behavior:

- Add a single canonical tool name, recommended: `shell.run`.
- Manifest is high-risk, disabled or declarative-only by default.
- Manifest declares structured input schema:
  - `command`
  - `args`
  - `cwd`
  - `env`
  - optional `reason`
- `shell.run` is excluded from model-callable tools unless enabled by runtime policy and AgentSpec.
- PromptStack/PlanMode continue to forbid shell by default.

Allowed edit areas:

- `openlife-core/src/mcp.rs`
- `openlife-core/src/tool_manifest.rs`
- `openlife-core/src/agent/prompt_stack.rs`
- focused tests

Constraints:

- No executor.
- No ActionExecutor branch.
- No frontend shell UI.

Verification:

- `cargo test -p openlife-core mcp --lib`
- `cargo test -p openlife-core tool_manifest --lib`
- `cargo test -p openlife-core agent::prompt_stack --lib`
- `cargo check -q`

Required tests:

- `shell.run` is high-risk.
- default manifest is not model-callable.
- declarative-only or disabled shell cannot execute.
- planning prompt still forbids shell.
- tool prompt excludes shell unless explicitly enabled by policy.

## P9-3: AppState And Action Context Sandbox Wiring

Goal:

Make runtime paths able to carry an `ExecutionSandbox` policy without executing shell.

Expected behavior:

- App config can represent the sandbox policy or derive a default from existing `system.safe_paths`.
- Tauri AppState/bootstrap can provide sandbox policy to action execution paths.
- `ActionExecutionContext` can carry `ExecutionSandbox`.
- Chat, plan execution, retry/continue, scheduled/proactive, and direct tool paths either pass the policy or explicitly pass disabled sandbox.
- No path should silently construct an enabled sandbox.

Allowed edit areas:

- `openlife-core/src/config.rs`
- `openlife-core/src/agent/action_executor/mod.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/src/commands/agent.rs`
- `src-tauri/src/commands/plan.rs`
- `src-tauri/src/scheduler_runner.rs`
- focused tests

Constraints:

- No shell execution.
- No settings UI beyond type-safe config plumbing.
- Preserve existing safe_paths behavior for file tools.

Verification:

- `cargo test -p openlife-core agent::action_executor --lib`
- `cargo test -p openlife-tauri commands::plan --lib`
- `cargo check -q`

Required tests:

- missing config yields disabled sandbox.
- configured safe_paths feed sandbox safe paths.
- formal runtime paths do not enable shell by default.
- plan execution receives sandbox policy without changing existing non-shell tools.

## P9-4: Non-Interactive Command Executor Skeleton

Goal:

Add the minimal executor primitive for structured commands.

Expected behavior:

- Executor accepts structured command requests, not raw shell strings.
- It uses `std::process::Command` or equivalent direct process spawning.
- It validates command, cwd, args, env names, and path operands before spawn.
- It enforces timeout and output byte limits.
- It returns structured stdout/stderr/status metadata.
- It does not use a shell interpreter.
- It does not directly write files.

Suggested implementation shape:

- Add `openlife-core/src/agent/shell_executor.rs`.
- Add:
  - `ShellCommandRequest`
  - `ShellCommandOutput`
  - `ShellExecutor`
  - `ShellExecutionError`

Allowed edit areas:

- `openlife-core/src/agent/shell_executor.rs`
- `openlife-core/src/agent/execution_sandbox.rs`
- `openlife-core/src/agent/mod.rs`
- focused tests

Constraints:

- No ToolRuntime integration yet.
- No Tauri command.
- No interactive session.
- No network commands.

Verification:

- `cargo test -p openlife-core agent::shell_executor --lib`
- `cargo test -p openlife-core agent::execution_sandbox --lib`
- `cargo check -q`

Required tests:

- allowed command succeeds.
- disabled sandbox blocks before spawn.
- denied command blocks before spawn.
- disallowed env variable is omitted or rejected.
- timeout kills or rejects long-running command.
- output is truncated at `max_output_bytes`.
- command with shell metacharacters is rejected.

## P9-5: ToolRuntime Shell Integration And Trace

Goal:

Expose `shell.run` through ActionExecutor only when sandbox, manifest, permission, and AgentSpec all allow it.

Expected behavior:

- `shell.run` execution goes through ActionExecutor/ToolRuntime.
- Disabled sandbox blocks before process spawn.
- Manifest disabled/declarative-only blocks before process spawn.
- High-risk permission policy can return `NeedsConfirmation`.
- AgentSpec-denied shell blocks before execution.
- Events are recorded for started, blocked, completed, failed, and timeout paths.
- Output stored in observations is truncated and safe.

Allowed edit areas:

- `openlife-core/src/agent/action_executor/`
- `openlife-core/src/agent/shell_executor.rs`
- `openlife-core/src/agent/types.rs`
- `openlife-core/src/agent/event_store.rs`
- focused tests

Constraints:

- No frontend shell command box.
- No scheduled/proactive shell.
- No sub-agent shell.
- Do not bypass existing permission/proposal/replay paths.

Verification:

- `cargo test -p openlife-core agent::action_executor --lib`
- `cargo test -p openlife-core agent::event_store --lib`
- `cargo check -q`

Required tests:

- shell disabled by default records blocked event.
- manifest disabled/declarative-only blocks.
- allowlisted command with accepted permission succeeds.
- denied command records blocked event.
- timeout records failed/timeout event.
- output truncation is reflected in payload metadata.
- no event payload contains unbounded stdout/stderr.

## P9-6: Governed Runtime Entry Policy

Goal:

Prevent shell from leaking into broad agent behavior before explicit product design.

Expected behavior:

- Normal Chat AgentSpec denies shell by default.
- PlanMode planner forbids shell.
- Plan execution can only use shell when:
  - selected AgentSpec allows `shell.run`
  - sandbox is enabled
  - manifest is executable
  - permission policy allows or user confirms
  - task is user-triggered
- Scheduled/proactive tasks pass disabled sandbox.
- SubAgentRuntime denies shell unless a future ADR/task explicitly changes that.

Allowed edit areas:

- `openlife-core/src/agent/types.rs`
- `openlife-core/src/agent/agent_loop.rs`
- `openlife-core/src/agent/plan_mode.rs`
- `openlife-core/src/agent/plan_executor.rs`
- `openlife-core/src/agent/sub_agent.rs`
- `src-tauri/src/scheduler_runner.rs`
- focused tests

Constraints:

- No interactive shell.
- No broad ChatPage rewrite.
- No automatic enabling.

Verification:

- `cargo test -p openlife-core agent::agent_loop --lib`
- `cargo test -p openlife-core agent::plan_mode --lib`
- `cargo test -p openlife-core agent::plan_executor --lib`
- `cargo test -p openlife-core agent::sub_agent --lib`
- `cargo check -q`

Required tests:

- default main AgentSpec denies shell.
- planner prompt/tool set excludes shell.
- plan-bound AgentSpec denial blocks shell.
- scheduled/proactive shell attempt uses disabled sandbox.
- sub-agent shell attempt is blocked.

## P9-7: Minimal Settings And Trace Surface

Goal:

Expose shell governance state without building a terminal UI.

Expected behavior:

- Settings can display or edit the sandbox toggle and safe paths only if backend config is ready.
- UI copy makes shell default-off status visible.
- Run trace renders shell tool blocked/completed/failed events using existing tool event components.
- No command entry surface is added.

Allowed edit areas:

- `frontend/src/types.ts`
- `frontend/src/tauri.ts`
- `frontend/src/test/mocks/tauri.ts`
- `frontend/src/components/RunTracePanel.tsx`
- `frontend/src/components/RunTracePanel.test.tsx`
- minimal Settings tab files if config plumbing exists

Constraints:

- No terminal emulator.
- No shell command input box.
- No redesign.

Verification:

- `pnpm --dir frontend test -- --run RunTracePanel tauri`
- `pnpm --dir frontend typecheck`

Required tests:

- trace renders shell blocked event.
- trace renders shell completed event with truncated-output marker.
- existing trace events still render.
- settings mock includes disabled sandbox by default if settings contract changed.

## P9 Exit Criteria

P9 is complete when:

- P9 task specs and coding prompts are discoverable from `AGENTS.md` and `README.md`.
- `ExecutionSandbox` remains default-off and test-covered.
- `shell.run` exists as the only shell tool contract.
- Shell cannot become model-callable unless sandbox, manifest, permission, and AgentSpec all allow it.
- Command execution is non-interactive and structured; no raw shell string execution is introduced.
- Deny-read, safe_paths, cwd, env allowlist, timeout, and output limit are enforced.
- Every shell attempt records append-only trace.
- Scheduled/proactive tasks and sub-agents cannot use shell by default.
- There is no terminal UI or broad ChatPage rewrite.

Recommended final verification:

- `cargo test -p openlife-core agent::execution_sandbox --lib`
- `cargo test -p openlife-core agent::shell_executor --lib`
- `cargo test -p openlife-core agent::action_executor --lib`
- `cargo test -p openlife-core agent::agent_loop --lib`
- `cargo test -p openlife-core agent::plan_executor --lib`
- `cargo test -p openlife-core agent::sub_agent --lib`
- `cargo test -p openlife-core agent::event_store --lib`
- `cargo test -p openlife-tauri commands::plan --lib`
- `pnpm --dir frontend test -- --run RunTracePanel tauri`
- `pnpm --dir frontend typecheck`
- `cargo check -q`
