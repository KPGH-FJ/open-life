# Main Chat Next 6 Steps Master Spec

> Date: 2026-06-25
> Status: preparation artifact before the next Main Chat Agent development cycle
> Parent: `plans/README.md`

## 1. Purpose

This spec controls the next six Main Chat development steps after
`00eabf0 Implement agent self-state runtime facts`.

It is not an implementation report and it is not a readiness claim. Its purpose
is to prevent the next cycle from mixing three different problem classes:

- unfinished work from the Runtime Facts / Agent Self-State slice;
- legacy migration debt from pre-kernel Main Chat paths;
- architecture and product-state boundaries that remain too broad.

Every later step must update this spec or its acceptance matrix when scope,
evidence, or blockers change.

## 2. Verified Baseline

Current branch and commit:

- branch: `rescue/main-chat-kernel-goal-8`;
- commit: `00eabf0b1604bde5971fd127b5ebc64d20df22e7`;
- commit title: `Implement agent self-state runtime facts`.

Verified current state:

- `src-tauri/src/main_chat_runtime_facts.rs` still sets
  `runtime_facts_ready=false` for slice reports.
- Slice D covers RF-16 through RF-19 and explicitly leaves RF-20 and RF-21 out
  of scope.
- Slice B, Slice C, and Slice D still carry stream deferred blockers.
- `src-tauri/src/main_chat_command_surface_eval.rs` still initializes external
  live-provider coverage to `0.0`.
- `src-tauri/src/main_chat_final_acceptance_tests.rs` still marks the external
  live-provider acceptance runner with `#[ignore]`; it must be selected with
  `--ignored` and still requires explicit live-provider credentials and network.
- `frontend/src/pages/ChatPage.tsx` still hides detailed runtime evidence behind
  `showMainChatDiagnostics`.
- Main Chat code remains split across many `main_chat*.rs` files, with major
  concentration points including `main_chat_kernel.rs`,
  `main_chat_runtime_facts.rs`, command-surface/final-gate modules, readiness
  modules, and related gate/test modules.

Before starting Step 1, rerun the baseline command set in
`plans/main_chat_next_6_steps_acceptance_matrix.md` and record the result in
that step's review. Do not maintain a shorter duplicate baseline list here.

## 3. Operating Rules

- One step may fix one problem layer only.
- Do not combine live-provider work with Runtime Facts refactoring.
- Do not combine UI productization with fallback containment.
- Do not lower a gate to make a step pass.
- Do not claim full Runtime Facts readiness while RF-20, RF-21, or required
  send/stream parity remains missing.
- Do not claim live-provider completion from scripted, local, fixture, loopback,
  synthetic, or local-test HTTP evidence.
- Do not treat assistant prose as task state evidence.
- Do not treat proposal delivery as durable Memory or LifeModel completion.
- Do not add a new broad helper or catch-all resolver unless the refactor
  boundary document has been updated first.

## 4. Step Order

| Step | Name | Primary problem class | Entry criteria | Exit criteria |
| --- | --- | --- | --- | --- |
| 1 | Runtime Facts Completion | unfinished current slice | current baseline verified | RF-20/RF-21 and B/C/D stream parity pass without model/tool/write fallback |
| 2 | External Live Provider Evidence | prior validation gap | Step 1 passed or explicitly deferred with named blockers | DirectAnswer, web AgentLoop, MCP AgentLoop, and ToolPermission proposal have external live credit |
| 3 | Legacy Fallback Containment | historical migration debt | Step 1 and live gate status known | unsupported Main Chat strategies do not silently succeed through legacy fallback |
| 4 | Runtime Facts / Kernel Boundary Refactor | architecture boundary debt | Steps 1-3 behavior protected by tests | Runtime Facts code is split by stable responsibility without behavior regression |
| 5 | Agent Status Product UI | product experience gap | state vocabulary and backend evidence stable | default UI communicates status/action/proposal/permission without requiring diagnostics |
| 6 | End-To-End Product Acceptance | final integration | Steps 1-5 passed or have explicit blockers | real user task journeys pass with no silent write, no hidden fallback, and auditable evidence |

## 5. Step 1 Scope: Runtime Facts Completion

Must include:

- RF-20 blocked task self-state:
  - task/session state is blocked;
  - blocker code is visible as bounded metadata;
  - answer says the task is not completed;
  - answer exposes a valid next control or says no safe automatic control exists.
- RF-21 pending permission self-state:
  - pending permission action is counted;
  - target label is bounded and policy-safe;
  - raw unsafe manifest details are not exposed;
  - answer says user confirmation is required.
- Slice B stream parity.
- Slice C stream parity.
- Slice D stream parity.

Must not include:

- live-provider execution;
- Runtime Facts file decomposition;
- UI product redesign;
- fallback policy changes outside runtime fact dispatch.

Primary files likely touched:

- `src-tauri/src/main_chat_runtime_facts.rs`;
- `src-tauri/src/main_chat_runtime_facts_tests.rs`;
- `src-tauri/src/main_chat_kernel.rs`;
- focused send/stream command-surface tests if parity requires them.

Required verification:

```bash
cargo fmt --check
cargo check -p openlife-tauri
cargo test -p openlife-tauri main_chat_runtime_facts -- --nocapture
cargo test -p openlife-tauri main_chat_command_surface_eval_gate_covers_send_stream_runtime_matrix -- --nocapture
git diff --check
```

## 6. Step 2 Scope: External Live Provider Evidence

Must include:

- external live DirectAnswer credit;
- provider-backed web AgentLoop credit;
- provider-backed registered MCP AgentLoop credit;
- provider-backed MCP ToolPermission proposal credit;
- explicit no-credit behavior for local, scripted, fixture, synthetic, loopback,
  and local-test HTTP evidence.

Must not include:

- prompt or model behavior tuning that masks missing harness evidence;
- product UI work;
- fallback containment outside live evidence audit.

Primary files likely touched:

- `src-tauri/src/main_chat_live_provider_harness.rs`;
- `src-tauri/src/main_chat_final_gate.rs`;
- `src-tauri/src/main_chat_final_acceptance_tests.rs`;
- `src-tauri/src/main_chat_command_surface_eval.rs`.

Required verification:

```bash
cargo fmt --check
cargo check -p openlife-tauri
cargo test -p openlife-tauri main_chat_live_provider -- --nocapture
cargo test -p openlife-tauri main_chat_final_acceptance -- --nocapture
OPENLIFE_MAIN_CHAT_LIVE_PROVIDER_EVAL=1 cargo test -p openlife-tauri main_chat_final_acceptance_gate_runner_accepts_external_live_provider_when_opted_in -- --ignored --nocapture
git diff --check
```

The opt-in command must only be run after the live-provider setup document is
completed for the local machine.

## 7. Step 3 Scope: Legacy Fallback Containment

Must include:

- inventory every Main Chat strategy that can still bypass kernel support;
- decide whether `ReviewMaturation` enters kernel, returns a governed blocker,
  or stays non-default and unreachable from ordinary Main Chat;
- preserve explicit fallback transcript evidence where fallback remains
  intentionally possible;
- ensure default command-surface gates still report legacy fallback count `0`.

Must not include:

- broad route rewriting;
- removal of legacy code without preserving migration diagnostics;
- live-provider work.

Primary files likely touched:

- `src-tauri/src/main_chat_kernel.rs`;
- `src-tauri/src/main_chat_send.rs`;
- `src-tauri/src/main_chat_streaming.rs`;
- `src-tauri/src/main_chat_legacy_fallback.rs`;
- `src-tauri/src/main_chat_command_surface_eval.rs`.

Required verification:

```bash
cargo fmt --check
cargo check -p openlife-core
cargo check -p openlife-tauri
cargo test -p openlife-tauri main_chat_kernel -- --nocapture
cargo test -p openlife-tauri main_chat_command_surface -- --nocapture
cargo test -p openlife-tauri main_chat_final_acceptance -- --nocapture
git diff --check
```

## 8. Step 4 Scope: Runtime Facts / Kernel Boundary Refactor

Must include:

- split Runtime Facts by responsibility, not by arbitrary file size;
- preserve every existing runtime fact key, metadata field, and gate outcome;
- keep kernel consuming a typed runtime fact answer, not fact-specific internals;
- add module-boundary tests that prevent future re-concentration.

Must not include:

- new runtime facts;
- live-provider changes;
- UI redesign;
- lower acceptance thresholds.

The detailed boundary is governed by
`plans/main_chat_runtime_facts_refactor_boundary.md`.

## 9. Step 5 Scope: Agent Status Product UI

Must include:

- default-visible status language for completed, waiting, restricted, blocked,
  proposal pending, permission pending, and trace gap;
- primary user actions for proposal review, permission review, retry, resume,
  cancel, and refresh context;
- expanded trace remains available for developer evidence;
- raw sensitive/internal data stays hidden by default.

Must not include:

- backend state invention;
- raw prompt, raw Memory/LifeModel, provider keys, raw MCP manifests, absolute
  workspace paths, or unbounded blocker payloads.

The UI contract is governed by
`plans/main_chat_agent_status_ui_contract.md`.

Required verification:

```bash
pnpm --dir frontend format:check
pnpm --dir frontend typecheck
pnpm --dir frontend test -- src/components/ReasoningTracePanel.test.tsx
pnpm --dir frontend test -- src/pages/ChatPage.test.tsx
git diff --check
```

## 10. Step 6 Scope: End-To-End Product Acceptance

Must include user-level journeys, not only unit reports:

- ask current date/time/weekday;
- ask current model route;
- ask tool/web/MCP availability;
- read a workspace file;
- complete a direct answer and ask whether it completed;
- create a proposal and ask whether durable change completed;
- hit a blocked task and ask next action;
- hit a pending permission and review/accept it;
- execute provider-backed web read;
- execute provider-backed MCP read;
- recover or explicitly stop a blocked task.

Each journey must assert:

- no silent durable write;
- no hidden legacy fallback;
- structured runtime evidence exists;
- UI status is correct;
- answer does not invent unavailable evidence.

## 11. Stop Conditions

Stop development and return to preparation if:

- a step needs to change another step's core scope to pass;
- a test passes only by weakening a blocker or readiness threshold;
- a readiness field becomes true while its evidence remains missing;
- external live evidence cannot be distinguished from local/synthetic evidence;
- UI requires parsing assistant prose to determine task state;
- Runtime Facts gains another catch-all natural-language resolver.
