# Main Chat Stage 3 Execution UX Implementation Report

> Date: 2026-06-20
> Scope: Stage 3 Execution UX only
> Status: implementation complete; not a limited-internal-trial readiness claim

## Data Path

Stage 3 uses the existing governed Main Chat execution path:

```text
Main Chat send/stream
  -> AgentIngress / strategy route
  -> AgentTaskSession / ActionQueue / ExecutionTranscript / Main Chat event stream
  -> MainChatAgentStateSnapshot
  -> AgentControlPlane
```

When the typed `MainChatAgentStateSnapshot` is unavailable, ChatPage now shows a
compact diagnostic shell derived only from `AgentIngress` and task detail. That
shell is explicitly marked as diagnostic and is not a second authoritative task
panel.

## Completed Phases

- Phase 0, contract alignment: documented the final data path and kept all UI
  state tied to existing runtime snapshots, task state, transcript, or event
  stream data.
- Phase 1, primary control plane: kept `AgentControlPlane` as the primary
  execution surface and relabeled the typed-state-missing ChatPage fallback as a
  diagnostic task shell.
- Phase 2, execution timeline: added a unified timeline for plan steps, actions,
  observations, blockers, proposals, event stream markers, and final delivery.
- Phase 3, scoped controls: preserved existing runtime-backed resume, retry,
  cancel, permission, proposal, plan, and rollback controls, while exposing exact
  action/blocker/proposal identities in the timeline and detail rows.
- Phase 4, final delivery and reload recovery: kept final delivery sections
  separate and added `skippedWork` from existing plan evidence; reload recovery
  remains tied to task snapshot/event-stream mechanisms already covered by
  productization and event gate evidence.
- Phase 5, Stage 3 eval/report: added `main_chat_stage3_execution_ux` coverage
  with UX3-01 through UX3-13 and a Tauri command wrapper. This is not a
  readiness gate.

## Changed Files

- `frontend/src/components/AgentControlPlane.tsx`: reviewer trace one-line JSON,
  execution timeline, linked action evidence, event sequence/source chips, and
  separated skipped final-delivery work.
- `frontend/src/pages/ChatPage.tsx`: compact diagnostic shell when typed agent
  state is missing.
- `frontend/src/tauri.ts`: Stage 3 report types and command wrapper; added
  `finalDelivery.skippedWork`.
- `frontend/src/test/mocks/tauri.ts`, `frontend/src/tauri.test.ts`: deterministic
  mock and wrapper coverage for the Stage 3 report.
- `frontend/src/components/AgentControlPlane.test.tsx`,
  `frontend/src/pages/ChatPage.test.tsx`: focused UI coverage for timeline,
  skipped work, reviewer trace JSON, diagnostic fallback, and updated runtime
  evidence selectors.
- `openlife-core/src/agent/main_chat_agent_productization_v1.rs`: derives
  `skippedWork` final-delivery evidence from existing skipped plan steps.
- `src-tauri/src/main_chat_stage3_execution_ux.rs`: deterministic Stage 3 UX
  report surface.
- `src-tauri/src/main_chat_stage3_execution_ux_tests.rs`: UX3 report coverage
  test.
- `src-tauri/src/commands/agent_runtime/mod.rs`, `src-tauri/src/lib.rs`: command
  registration and module wiring.
- `src-tauri/src/main_chat_event_stream.rs`: fixture update for the new
  final-delivery field.

## UX3 Scenario Results

| ID | Result | Evidence summary |
| --- | --- | --- |
| UX3-01 | passed | Direct answer uses compact AgentControlPlane state with no fake action observations. |
| UX3-02 | passed | File read action and observation evidence are visible. |
| UX3-03 | passed | Missing file blocker is visible with source/reason evidence. |
| UX3-04 | passed | Web/network policy blocker is visible without fake web observation. |
| UX3-05 | passed | Registered MCP selected target and observation evidence are visible. |
| UX3-06 | passed | ToolPermission proposal controls are scoped to target/action evidence. |
| UX3-07 | passed | PlanExecute draft controls remain visible through existing plan session/revision state. |
| UX3-08 | passed | Memory proposal remains pending/reviewable with no materialized-memory claim. |
| UX3-09 | passed | Retry evidence is scoped to failed action and follow-up observation/blocker state. |
| UX3-10 | passed | Cancelled terminal task state is distinct from continued queued work. |
| UX3-11 | passed | Final delivery separates completed, proposed, blocked, skipped, pending, durable changes, and next steps. |
| UX3-12 | passed | Reload recovery evidence uses durable event stream replay and conversation-linked task snapshot recovery. |
| UX3-13 | passed | Reviewer trace export is bounded one-line JSON with stable keys. |

Execution-first claim subset passed: `UX3-02`, `UX3-03`, `UX3-04`,
`UX3-06`, `UX3-09`, `UX3-11`, and `UX3-12`.

## Tests Run

```bash
git diff --check
cargo fmt --check
cargo test -p openlife-core main_chat_agent_v1 -- --nocapture
cargo test -p openlife-tauri main_chat_agent_stage1_dogfood -- --nocapture
cargo test -p openlife-tauri main_chat_agent_beta_v1_readiness -- --nocapture
cargo test -p openlife-tauri main_chat_product_maturity_v2 -- --nocapture
cargo test -p openlife-tauri main_chat_command_surface -- --nocapture
cargo test -p openlife-tauri main_chat_final_acceptance -- --nocapture
cargo test -p openlife-tauri main_chat_agent_stage2_readiness -- --nocapture
cargo test -p openlife-tauri main_chat_stage3_execution_ux -- --nocapture
corepack pnpm --dir frontend typecheck
corepack pnpm --dir frontend format:check
corepack pnpm --dir frontend test -- src/pages/ChatPage.test.tsx src/components/AgentControlPlane.test.tsx src/tauri.test.ts
corepack pnpm --dir frontend test:e2e -- main-chat-stage1-dogfood.spec.ts --reporter=line
```

Observed results:

- `openlife-core main_chat_agent_v1`: 31 passed.
- `main_chat_agent_stage1_dogfood`: 22 passed.
- `main_chat_agent_beta_v1_readiness`: 3 passed.
- `main_chat_product_maturity_v2`: 9 passed.
- `main_chat_command_surface`: 24 passed.
- `main_chat_final_acceptance`: 86 passed, 1 ignored external-provider opt-in
  test.
- `main_chat_agent_stage2_readiness`: 56 passed, 1 ignored external-provider
  opt-in test.
- `main_chat_stage3_execution_ux`: 1 passed.
- Frontend specified bundle: 127 passed.
- Playwright browser command: 1 passed. This host ran the non-Tauri browser
  blocked-report path; it did not produce real Tauri D01-D36 manual dogfood
  evidence.

## Remaining Blockers

- Stage 2 readiness remains
  `not_ready_for_limited_internal_trial` without real S2-D01 through S2-D24
  manual dogfood and current-commit live provider evidence.
- Real external live-provider Stage 2/Final Acceptance opt-in tests were not
  executed in this environment because they require explicit opt-in, network, and
  a real external provider API key.
- The Playwright browser run did not observe real Tauri Chat UI D01-D36 journeys;
  it only verified the configured non-Tauri blocked evidence path.
- Stage 4 memory/knowledge asset management and Stage 5 release/debug operations
  remain out of scope.

## Readiness Statement

Stage 3 is an execution UX milestone and internal alpha candidate surface. It
does not grant `ready_for_limited_internal_trial`, does not run or fill S2-D01
through S2-D24 manual dogfood rows, and does not replace the Stage 2 readiness
gate. Stage 2 remains fail-closed until real manual dogfood and current-commit
live evidence exist.
