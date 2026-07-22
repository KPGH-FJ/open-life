# OpenLife AI Coding Entrypoint

This file is the durable AI-agent entrypoint for the OpenLife repository. It is
not a roadmap, progress log, or replacement for the active Phase7 plan files.

## Current Authority

Read in this order when starting non-trivial OpenLife work:

1. `AGENTS.md` - repo-local coding rules and current non-negotiables.
2. `plans/README.md` - active Phase7 plan authority map.
3. `plans/openlife_single_system_deletion_manifest.md` - old-route deletion and
   expected-absent contract.
4. `plans/openlife_single_system_development_preparation.md` - single-system
   development boundaries and phase definitions.
5. Task-specific decision/preparation files named by the user.

Older Goal, Stage, Beta, migration, cutover, productization, maturity,
W-series, and roadmap documents are historical reference only unless the user
explicitly names one as input and keeps it subordinate to the Phase7 contract.

## Project Shape

- Product: local-first personal agent framework / personal AI operating system.
- Stack: Tauri 2.x desktop shell, Rust core, React 18, TypeScript, Tailwind CSS,
  SQLite.
- Product definition: OpenLife is not just chat and not a generic life-planning
  app. It should let a private LifeModel guide local or cloud models for
  conversation, planning, writing, review, tool use, and user-approved state
  updates.
- Core operating model: LifeModel-HS protocol layer, governed runtime, ReAct
  default strategy, tool/skill execution, memory/feedback/maturation loop.

## Phase7 Contract

Phase7 is a single-system deletion and product-trial pass. It is not another
compatibility adapter and not a docs-only supersession.

Current consequences:

- old product runtime, command, frontend bridge, product UI route, and active
  route-authorizing docs must stay absent once classified `done` in the deletion
  manifest;
- objects marked expected-absent are evidence of deletion, not files to
  recreate;
- product-valid replacements must use current semantic names and must not keep
  old shells alive;
- Phase7 remains blocked while the product trial is
  `red-until-trial-green`;
- Stage6E RED findings are product development TODOs, not repository cleanup
  blockers.

## Main Chat Source Map

Ordinary Main Chat has two parallel Tauri command entrypoints. They share the
same runtime owner after the transport wrapper, but `send` does not flow
through `stream` and `stream` does not flow through `send`.

```text
frontend/src/tauri.ts
  -> src-tauri/src/lib.rs send_message
  -> src-tauri/src/main_chat_send.rs
  -> OpenLifeTurnRuntime::run_buffered
      -> src-tauri/src/main_chat_turn_runtime.rs
      -> src-tauri/src/main_chat_kernel.rs
      -> openlife-core/src/agent/main_chat_agent_v1.rs

frontend/src/tauri.ts
  -> src-tauri/src/lib.rs start_stream_message
  -> src-tauri/src/main_chat_streaming.rs
  -> OpenLifeTurnRuntime::run_streaming
      -> src-tauri/src/main_chat_turn_runtime.rs
      -> src-tauri/src/main_chat_kernel.rs
      -> openlife-core/src/agent/main_chat_agent_v1.rs
```

Read these supporting areas with that path:

- `src-tauri/src/main_chat_context_loader.rs`
- `src-tauri/src/main_chat_hs_runtime.rs`
- `src-tauri/src/main_chat_react_*`
- `src-tauri/src/provider_network_consent.rs`
- `src-tauri/src/main_chat_runtime_support.rs`
- `src-tauri/src/main_chat_task_controls.rs`
- `src-tauri/src/main_chat_event_stream.rs`
- `src-tauri/src/main_chat_final_gate.rs`
- `openlife-core/src/agent/model_router.rs`

`src-tauri/src/lib.rs` owns the Tauri command functions and handler wiring, but
it is not the ordinary Main Chat turn runtime owner.

Legacy fallback may exist only as explicit, countable, non-default behavior.
Hidden fallback completion is not acceptable. Tool, ReAct, route, permission,
or policy failures must become structured blocker, HITL, proposal, or failure
state.

## Current Status Boundaries

- Main Chat Agent Execution v1 is still in remediation and must not be described
  as finished.
- External live-provider-backed generation, web AgentLoop, MCP AgentLoop, and
  provider/live proposal-permission evidence remain incomplete.
- Local HTTP OpenAI-compatible proof, scripted provider proof, mock IPC,
  command-surface evals, and fixture-backed web reads are local evidence only;
  they do not count as external live-provider credit.
- `finalCompletionReady` must remain fail-closed unless the current gates and
  credited live-provider reports prove otherwise.
- The retired final acceptance command
  `run_main_chat_agent_execution_v1_final_acceptance_gate` must not return to
  shipped command or product bridge surfaces.
- The deleted old final-acceptance test owner
  `src-tauri/src/main_chat_final_acceptance_tests.rs` must remain absent unless
  a future explicit decision changes the contract.

## Non-Negotiable Product Rules

- No silent durable writes to LifeModel-HS truth, long-term Memory, files,
  calendar, email, external providers, plugins, or dangerous shell.
- Explicit memory and LifeModel updates must go through Review Center proposal
  flow unless a documented low-risk lane says otherwise.
- External or sensitive writes require confirmation, proposal, or blocker;
  assistant prose is never write authorization.
- Creating a proposal is not the same as completing the durable change.
- Workspace `AGENTS.md`, `SOUL.md`, `USER.md`, `MEMORY.md`, and selected
  `SKILL.md` are bounded context surfaces only; they do not override privacy,
  model, tool, or durable-write policy.
- Product state covered by `LifeStateProjection` should be consumed from that
  projection, not rebuilt from raw diagnostics/proposals/config fragments.
- Tool execution credit must come from explicit capability/risk/action
  contracts, not inferred tool names.
- Product UI must not present readiness, pending work, or completion from a
  page-local interpretation when a backend read model exists.

## Development Rules

- Preserve the dirty-worktree boundary: do not revert user or prior-agent
  changes unless explicitly asked.
- Use `/Users/tw/Desktop/open-life` as the only writable development checkout.
  Do not create or register another Git worktree, roadshow checkout, D0xx
  checkout, or sibling `open-life-*` development directory unless the user
  explicitly authorizes that exact action in the current task. Short-lived Git
  branches must be switched and developed in this checkout.
- Treat retained historical/V4/D0xx branch refs as read-only evidence until
  they are explicitly classified. Do not recreate physical checkouts for them
  merely to inspect their contents; use Git object-level commands instead.
- For Phase7 or repository-knowledge work, read the named prep/decision files
  before editing.
- Start authority cleanup with a source map. Every current path listed in a
  durable doc should exist now, or be clearly labeled historical/expected-absent.
- Prefer file-level source areas in long-lived docs over fragile line-number
  anchors.
- Do not restore deleted old-route files to satisfy documentation links or
  scans.
- Do not edit Rust/Tauri/React/frontend behavior during docs-only cleanup
  stages.
- Do not move ADR 0013, create plan archive namespaces, or reorganize `plans/`
  unless that is the explicit task scope.

## Testing And Evidence

Pick the smallest gate that matches the change. Common checks:

```sh
git diff --check
cargo fmt --check
cargo test -p openlife-tauri single_system -- --nocapture
cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture
corepack pnpm --dir frontend typecheck
corepack pnpm --dir frontend format:check
```

Use task-specific absence and claim scans from the active plan or decision
record. Keep shipped handler, product command, product bridge, and product page
surfaces separate from historical docs, tests, guards, and dev-only bridges.

Raw `rg` output is only evidence input. Classify hits by surface before using
them to claim presence, absence, readiness, or completion.

## Documentation Map

- `README.md` is the compact public/user entry.
- `docs/ARCHITECTURE.md` is an index and historical pointer, not a second
  authority over Main Chat.
- `docs/architecture/agent-runtime.md`,
  `docs/architecture/life-model.md`, `docs/architecture/governance.md`, and
  `docs/architecture/memory.md` are source-backed explanatory docs.
- `docs/development/testing.md` explains current test/evidence distinctions.
- `docs/repository_document_governance.md` defines public/local/private
  document governance.
- `plans/adr/0013-lifemodel-hs-source-of-truth-governance.md` remains the
  canonical LifeModel-HS governance ADR unless a future reviewed slice moves it.

## When Unsure

Default to the active Phase7 contract, fail closed on completion/readiness
claims, and preserve proposal-first / no-silent-write behavior. If a historical
doc conflicts with the current authority stack, keep the historical doc as
background and follow Phase7.
