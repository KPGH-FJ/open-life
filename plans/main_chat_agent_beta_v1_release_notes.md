# Main Chat Agent Beta v1 Release Notes

> Date: 2026-06-18
> Scope: deterministic local Beta readiness evidence

## What Works By Default

- Main Chat work-like prompts route through governed task sessions instead of a
  hidden legacy chat completion path.
- Direct answers stay lightweight but carry task/session and provider trace
  evidence.
- Read tasks cover workspace files, session search, accepted memory context,
  selected `SKILL.md` context, web policy blockers or fixture-backed reads, MCP
  read candidates, multi-read AgentLoop paths, and knowledge asset inspection.
- Plan-Execute flows expose plan draft/edit/confirm/skip/execute/review evidence
  through the existing plan runtime.
- Recovery flows cover retry, cancel, resume, stale guards, exact permission
  replay, and terminal blockers.
- Final delivery distinguishes completed work, observations used, proposals,
  blocked work, skipped work, pending user action, and external-live status.

## Proposal-First Behavior

- Memory changes create proposal/confirmation records before becoming accepted
  context.
- The deterministic Beta scope proves a proposal-first `AGENTS.md` knowledge
  asset edit slice. Broader edit/rollback/conflict management for all knowledge
  asset types remains outside this minimum readiness claim.
- Durable truth changes are not inferred from assistant text alone.
- Risky or external writes require explicit permission or return a named
  blocker.

## Blocked Or Unsupported Behavior

- External live provider evidence is not part of default deterministic
  readiness. It requires explicit opt-in, real credentials, network access, and
  auditable provider traces.
- Local, mock, fixture, loopback, scripted, or synthetic provider responses do
  not count as external-live credit.
- Broad background autonomy, arbitrary external writes, marketplace-scale plugin
  hardening, enterprise sync, and full public knowledge-manager workflows remain
  outside this Beta scope.

## Inspecting Task Evidence

- The runtime evidence surfaces are task sessions, action queue entries,
  execution transcript entries, durable task events, proposal records, memory
  lifecycle records, plan records, permission records, and final-delivery
  sections.
- In the UI, `AgentControlPlane` and `ChatPage` render task status, timeline
  events, actions, observations, blockers, proposals, event replay status,
  controls, and final delivery from typed runtime payloads.
- The deterministic command-surface matrix covers ordinary `send_message` and
  `start_stream_message` paths and reports legacy fallback and silent durable
  write counts.

## Inspecting Knowledge Assets

- Bounded context loading covers scoped `AGENTS.md`, `SOUL.md`, `USER.md`,
  `MEMORY.md`, selected `SKILL.md`, and configured knowledge roots.
- Scenario B27 proves loaded knowledge asset inventory, scope, digest, and
  policy-boundary evidence through ordinary Main Chat command surfaces.
- Scenario B28 proves knowledge asset edit proposals create Review Center style
  proposal evidence and do not directly write the underlying file.
- Knowledge files remain context surfaces. They cannot override privacy, model
  routing, tool, memory, or execution policy.

## Running Readiness Gates

Default deterministic gates referenced by this Beta evidence bundle:

```bash
git diff --check
cargo test -p openlife-core main_chat_agent -- --nocapture
cargo test -p openlife-tauri main_chat -- --nocapture
cargo test -p openlife-tauri main_chat_agent_productization -- --nocapture
cargo test -p openlife-tauri main_chat_command_surface -- --nocapture
cargo test -p openlife-tauri main_chat_final_acceptance -- --nocapture
cargo test -p openlife-tauri main_chat_agent_execution_v1 -- --nocapture
cargo test -p openlife-tauri main_chat_agent_beta_v1_readiness -- --nocapture
cargo test -p openlife-tauri main_chat_product_maturity_v2 -- --nocapture
corepack pnpm --dir frontend typecheck
corepack pnpm --dir frontend test -- src/pages/ChatPage.test.tsx src/components/AgentControlPlane.test.tsx src/tauri.test.ts
```

The foundation inventory records the exact command results used for the current
readiness evidence. Broader `main_chat` filters remain recommended for release
verification when time permits.

The Tauri readiness command is:

```text
run_main_chat_agent_beta_v1_readiness_gate
```

It returns `MainChatAgentBetaV1ReadinessReport` from an isolated eval state,
keeps external live evidence separate, and fails closed when required
deterministic dimensions are missing.

## External Live Evidence

External live tests were not run for this release evidence bundle.

The opt-in live gate remains separate:

```bash
OPENLIFE_MAIN_CHAT_LIVE_PROVIDER_EVAL=1 \
OPENLIFE_LIVE_EVAL_PROVIDER=deepseek \
OPENLIFE_LIVE_EVAL_BASE=https://api.deepseek.com \
OPENLIFE_LIVE_EVAL_MODEL=deepseek-v4-flash \
cargo test -p openlife-tauri main_chat_live_provider -- --ignored --nocapture
```

No API keys or provider secrets should be written into repository files.
