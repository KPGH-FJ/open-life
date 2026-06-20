# Main Chat Agent Stage 1 Dogfood Goal Spec

> Date: 2026-06-19
> Status: completed automated Stage 1 engineering dogfood goal; retained as acceptance audit trail
> Depends on: Beta v1 readiness and Stage 1 preparation documents

## 1. Objective

Implement **Main Chat Agent Stage 1: Real End-to-End Dogfood**.

Status update: this automated engineering dogfood goal passed in Linux CI run
`27807633105` with real `tauri_command_surface_browser_observed` evidence, 36
observed scenarios, 36 passed journeys, 0 failed journeys, and no blockers.
Manual dogfood / internal-trial approval and external live-provider proof remain
separate follow-up scopes.

The goal is to prove that real users can start from Main Chat, issue realistic
work-like requests, see the agent execute or block correctly, and receive a
final delivery grounded in runtime evidence.

Stage 1 must build on current Beta v1 foundations. It must not rebuild task,
event, memory, plan, skill, proposal, or tool systems under new names.

## 2. Required Reading

Read before editing code:

- `AGENTS.md`
- `plans/main_chat_agent_beta_v1_foundation_inventory.md`
- `plans/main_chat_agent_beta_v1_release_notes.md`
- `plans/main_chat_stage1_industry_best_practices.md`
- `plans/main_chat_stage1_dogfood_gap_inventory.md`
- `plans/main_chat_stage1_dogfood_scenarios.md`
- `plans/main_chat_stage1_seed_data_contract.md`
- `plans/main_chat_stage1_e2e_harness_contract.md`
- `plans/main_chat_stage1_user_visible_acceptance.md`
- `plans/main_chat_stage1_live_provider_preflight.md`
- `plans/main_chat_stage1_manual_dogfood_protocol.md`
- `plans/main_chat_stage1_readiness_gate_contract.md`

## 3. Non-Goals

- Do not implement broad background autonomy.
- Do not implement full public Skills Hub or marketplace.
- Do not add dangerous write automation.
- Do not count external live provider evidence in default deterministic
  readiness.
- Do not create a parallel runtime object model.
- Do not mark aggregate gate evidence as browser-level dogfood unless a UI E2E
  path actually ran.

## 4. Implementation Phases

All phases below are complete for automated deterministic Stage 1 engineering
dogfood.

### Phase 0: Stage 1 inventory and seed foundation

Create deterministic seed setup for dogfood workspace, memories, sessions,
tasks, proposals, plans, MCP manifests, and web fixture sources.

Artifact:

- seed manifest included in `MainChatAgentStage1DogfoodReport`.

### Phase 1: deterministic command E2E

Implement Stage 1 dogfood scenarios through ordinary `send_message`,
`start_stream_message`, and existing task/proposal/plan/memory controls.

Artifact:

- `main_chat_agent_stage1_dogfood` Rust/Tauri tests;
- per-scenario evidence rows.

### Phase 2: UI integration evidence

Extend frontend tests as needed so scenario payloads render in Chat and
`AgentControlPlane` with correct controls and final delivery sections.

Artifact:

- focused `ChatPage` / `AgentControlPlane` tests;
- no frontend-only fake state.

### Phase 3: browser-level E2E slice

Add Playwright or equivalent browser-level smoke dogfood for the required core
journeys:

- DirectAnswer;
- file read;
- Plan-Execute;
- memory proposal;
- permission blocker;
- multi-read ReAct;
- event replay or recovery.

Artifact:

- browser E2E report for all required core journeys.

The browser E2E command must be self-contained. Update
`frontend/playwright.config.ts` with `webServer` or provide an equivalent
checked-in runner that starts and tears down the dev server. Do not require a
human to manually pre-start `localhost:5173` for the minimum gate.

If the app-level E2E environment is not currently stable, the readiness report
must return `not_ready_browser_e2e_blocked`. Do not mark Stage 1 complete by
substituting command-level or component-level tests for this phase.

### Phase 4: readiness aggregation

Add `run_main_chat_agent_stage1_dogfood_gate` returning
`MainChatAgentStage1DogfoodReport`.

Artifact:

- structured default readiness and opt-in live status;
- internal trial recommendation.

### Phase 5: manual dogfood report

Produce `plans/main_chat_stage1_manual_dogfood_report.md` after manual runs, or
mark manual dogfood as not attempted and recommend engineering dogfood only.

## 5. Acceptance

Stage 1 automated engineering dogfood is complete only when:

- default deterministic dogfood runs all P0/P1 required scenarios from
  `plans/main_chat_stage1_dogfood_scenarios.md`;
- at least 20 scenarios start from Chat input;
- at least 8 seeded task-control scenarios pass;
- every passed scenario has runtime evidence, UI evidence, and final delivery
  evidence;
- the required browser-level E2E smoke slice passes;
- hidden legacy fallback count is zero;
- silent durable write count is zero;
- fake execution count is zero;
- memory/knowledge updates remain proposal-first;
- expected blockers are visible and named;
- opt-in live status is separate and fail-closed without credentials;
- readiness report returns at least `ready_for_engineering_dogfood` honestly;
- `git status --short` contains only intentional source/docs changes.

## 6. Required Final Report

The implementation final report must include:

- completed phases;
- scenario counts and failures;
- readiness recommendation;
- tests run;
- browser E2E status;
- external live status;
- manual dogfood status;
- files changed;
- blockers and residual risks.
