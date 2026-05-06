# OpenLife vNext P7 Task Specifications

Date: 2026-05-06

Status: draft

Package:

```text
AgentSpec Store, Runtime Selection, and Governed Agent Entry Points
```

P7 turns the P6 AgentSpec helpers into real runtime selection infrastructure. The core question becomes: which AgentSpec is selected for this task or plan, where is that spec stored, how does it drive PromptStack and ContextPolicy, and how is that decision visible in trace.

P7 does **not** introduce Bash/Shell, SubAgent parallel execution, handoff execution, a full AgentSpec marketplace, or a ChatPage rewrite.

## Baseline

Before P7:

- AgentSpec and AgentTask exist as core contracts.
- ContextPolicy can filter context and is used by AgentRuntime default paths.
- PromptStack can assemble AgentSpec prompt block ids through a registry-facing API.
- PlanExecutor can receive an AgentSpec and pre-block denied tools.
- Tauri plan execution currently falls back to the default main AgentSpec behavior.

## Global Rules

- Execute exactly one P7 task spec at a time.
- Do not introduce Bash/Shell.
- Do not implement SubAgent parallel or handoff.
- Do not rewrite ChatPage.
- Do not bypass ToolRuntime, ActionExecutor, Proposal, PromptStack, AgentRunEvent, ExecutionSandbox, PlanExecutor, or ContextPolicy.
- AgentSpec may constrain tools/context/prompts, but it must not grant authority beyond existing runtime policy.
- Persisted AgentSpec selection must be deterministic and traceable.
- New behavior must have focused tests, including denial/error tests.
- Run the task-specific verification commands.
- Final report must include changed files, tests run, results, and residual risks.

## P7-0: Documentation And ADR Sync

Goal:

Make P7 discoverable and AI-coding-ready.

Expected behavior:

- `AGENTS.md` references P7 task specs.
- Migration plan references P7 after P6.
- Test matrix includes P7 acceptance and test gates.
- Agent coding prompts include P7 global prompt and P7 task prompts.
- ADR 0012 records AgentSpec store and runtime selection guardrails.

Allowed edit areas:

- `AGENTS.md`
- `plans/openlife_vnext_p7_task_specs.md`
- `plans/openlife_vnext_migration_plan.md`
- `plans/openlife_vnext_test_and_acceptance_matrix.md`
- `plans/openlife_vnext_agent_coding_prompts.md`
- `plans/adr/README.md`
- `plans/adr/0012-agentspec-store-runtime-selection.md`

Constraints:

- Documentation only.
- Do not change Rust or TypeScript code.

Verification:

- `rg -n "openlife_vnext_p7_task_specs|P7-0|P7-1|P7-2|P7-3|P7-4|P7-5|AgentSpec Store|ADR 0012" AGENTS.md plans`
- `git diff --name-only` contains documentation files only.

## P7-1: AgentSpecStore

Goal:

Add durable storage for AgentSpec definitions.

Expected behavior:

- `AgentSpecStore` persists AgentSpec records using the existing store style.
- The default main AgentSpec is bootstrapped with a stable id, for example `main.default`.
- Store supports:
  - create spec
  - get spec
  - list specs
  - update spec
  - activate/deactivate spec
  - ensure default main spec
- Stored specs round-trip all policy fields:
  - prompt block ids
  - allowed tools
  - denied tools
  - privacy policy
  - context permissions
  - active status
- Unknown spec ids return a structured error.

Allowed edit areas:

- `openlife-core/src/agent/types.rs`
- `openlife-core/src/agent/agent_spec_store.rs`
- `openlife-core/src/agent/mod.rs`
- relevant focused tests under `openlife-core/src/agent/`

Constraints:

- Do not wire Tauri commands in this task.
- Do not add UI.
- Do not change PlanExecutor behavior.
- Do not implement specialist agent marketplace semantics.

Verification:

- `cargo test -p openlife-core agent`
- `cargo check -q`

Required tests:

- default main spec is bootstrapped.
- AgentSpec round-trips through store.
- inactive specs are not selected as default.
- unknown spec id returns structured error.

## P7-2: Tauri AgentSpec Commands And AppState Wiring

Goal:

Expose minimal AgentSpec lifecycle commands and wire the store into bootstrap.

Expected behavior:

- AppState/bootstrap owns an `AgentSpecStore`.
- Bootstrap calls `ensure_default_main_spec`.
- Tauri commands expose a stable frontend contract:
  - `get_agent_spec(spec_id)`
  - `list_agent_specs()`
  - `get_default_agent_spec()`
  - `update_agent_spec(spec)`
  - `set_default_agent_spec(spec_id)`
- Frontend `tauri.ts`, `types.ts`, and mocks include matching wrappers.

Allowed edit areas:

- `src-tauri/src/state.rs`
- `src-tauri/src/bootstrap.rs`
- `src-tauri/src/lib.rs`
- `src-tauri/src/commands/agent.rs` or a new `src-tauri/src/commands/agent_spec.rs`
- `src-tauri/src/commands/mod.rs`
- `src-tauri/src/test_utils.rs`
- `frontend/src/tauri.ts`
- `frontend/src/types.ts`
- `frontend/src/test/mocks/tauri.ts`
- relevant tests

Constraints:

- Do not build a polished AgentSpec editor.
- Do not rewrite Settings or ChatPage.
- Do not change normal chat behavior beyond bootstrap wiring.

Verification:

- `cargo test -p openlife-tauri`
- `pnpm --dir frontend typecheck`
- `pnpm --dir frontend test -- --run tauri`
- `cargo check -q`

Required tests:

- default spec available after bootstrap.
- list returns the default main spec.
- update preserves stable fields.
- frontend wrappers typecheck.

## P7-3: Runtime AgentSpec Selection

Goal:

Make AgentRuntime execute with a resolved AgentSpec rather than only default helpers.

Expected behavior:

- Add a governed runtime entry, such as:
  - `execute_task_with_spec`
  - `generate_direct_with_spec`
  - or `AgentRuntimeExecutionContext`
- Runtime resolves or receives an AgentSpec before context/prompt assembly.
- Runtime uses AgentSpec prompt block ids through `PromptStack::try_from_agentspec`.
- Runtime derives ContextPolicy from AgentSpec fields, including:
  - `privacy_policy`
  - `can_access_lifemodel`
  - `can_access_memory_evidence`
  - future workspace scope
- Unknown prompt block ids fail before reasoning/model calls.
- Trace-ready metadata contains AgentSpec id and PromptBlock id/version, not raw prompt text.

Allowed edit areas:

- `openlife-core/src/agent/runtime.rs`
- `openlife-core/src/agent/context_assembler.rs`
- `openlife-core/src/agent/prompt_stack.rs`
- `openlife-core/src/agent/types.rs`
- relevant focused tests under `openlife-core/src/agent/`

Constraints:

- Do not call LLMs in new unit tests.
- Do not bypass PromptStack or ContextPolicy.
- Do not change ActionExecutor or PlanExecutor in this task.

Verification:

- `cargo test -p openlife-core agent`
- `cargo check -q`

Required tests:

- `execute_task_with_spec` uses AgentSpec prompt block ids.
- unknown prompt block id fails before reasoning.
- spec without memory access excludes memory.
- spec without LifeModel access excludes LifeModel summary.
- default main spec preserves current behavior.

## P7-4: Plan Execution Uses Stored AgentSpec

Goal:

Stop hardcoding default AgentSpec in plan execution and resolve the stored governing spec.

Expected behavior:

- `execute_agent_plan` and `retry_agent_plan` resolve AgentSpec before constructing PlanExecutor.
- Spec resolution order is deterministic:
  1. plan-bound spec id if the model has one
  2. run/task-bound spec id if available
  3. stored default main spec
- Missing explicit spec id returns structured error unless an explicit fallback policy allows defaulting.
- Resolved `agentspec_id` appears in plan execution trace payloads.
- AgentSpec-denied tools remain blocked before ActionExecutor/ToolRuntime execution.

Allowed edit areas:

- `openlife-core/src/agent/types.rs`
- `openlife-core/src/agent/plan_store.rs`
- `openlife-core/src/agent/plan_executor.rs`
- `src-tauri/src/commands/plan.rs`
- relevant tests

Constraints:

- Do not introduce parallel plan execution.
- Do not bypass PlanExecutor.
- Do not change permission/proposal/replay policy.
- Do not add plan editor UI.

Verification:

- `cargo test -p openlife-core agent::plan_executor`
- `cargo test -p openlife-tauri commands::plan`
- `cargo check -q`

Required tests:

- execute plan uses stored default AgentSpec.
- plan-bound AgentSpec deny blocks tool before execution.
- missing explicit spec id produces structured error or documented fallback.
- trace includes `agentspec_id`.

## P7-5: Minimal Frontend Contract Surface

Goal:

Expose AgentSpec contract to frontend code without building a large editor.

Expected behavior:

- Frontend types match backend AgentSpec contract.
- Tauri wrappers exist for P7-2 commands.
- Mocks return realistic AgentSpec shapes.
- Optional minimal Settings/dev surface may list the current default AgentSpec, but no full editor is required.

Allowed edit areas:

- `frontend/src/types.ts`
- `frontend/src/tauri.ts`
- `frontend/src/test/mocks/tauri.ts`
- optionally a small Settings tab or dev-only surface
- focused frontend tests

Constraints:

- Do not rewrite Settings.
- Do not rewrite ChatPage.
- Do not add a full AgentSpec marketplace/editor.
- Do not change trace UI except for small typed event metadata if needed.

Verification:

- `pnpm --dir frontend typecheck`
- `pnpm --dir frontend test -- --run tauri`
- if Settings changed: `pnpm --dir frontend test -- --run Settings tauri`

Required tests:

- wrappers typecheck.
- mock returns AgentSpec shape.
- default AgentSpec can be read from frontend wrapper.

## P7 Exit Criteria

P7 is complete when:

- There is a durable AgentSpecStore.
- Default main AgentSpec is bootstrapped and selectable.
- Runtime entrypoints can execute with a resolved AgentSpec.
- PromptStack and ContextPolicy are driven by the selected AgentSpec.
- Plan execution resolves stored AgentSpec instead of hardcoded defaults.
- Frontend/backend contracts can read the current AgentSpec.
- Tests prove denied context, denied tools, missing prompt blocks, and missing specs are handled without model/tool side effects.
