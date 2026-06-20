# Main Chat Agent Stage 3 Execution UX Goal Spec

> Date: 2026-06-20
> Status: prepared for CLI goal mode
> Depends on: Stage 2 readiness mechanism commit `90f78ce`

## 1. Objective

Implement **Main Chat Agent Stage 3: Execution UX and Main Chat Internal Alpha
Candidate**.

The output should be a Main Chat experience where internal users can see and
control Agent execution without relying on backend logs:

- task identity and route;
- active plan/action state;
- observations;
- blockers and next actions;
- permission/proposal controls;
- retry/resume/cancel/plan controls;
- final delivery;
- reload recovery;
- reviewer trace export.

Stage 3 is not the final internal-trial readiness gate. It prepares the product
for later S2-D01 through S2-D24 manual dogfood after Stage 3, Stage 4, and
Stage 5 are complete.

## 2. Required Reading

Read before editing code:

- `AGENTS.md`
- `plans/main_chat_stage3_preparation_index.md`
- `plans/main_chat_stage3_execution_ux_best_practices.md`
- `plans/main_chat_stage3_current_gap_inventory.md`
- `plans/main_chat_stage3_execution_ux_product_contract.md`
- `plans/main_chat_stage2_implementation_report.md`
- `plans/main_chat_stage2_readiness_gate_contract.md`
- `plans/main_chat_stage2_agent_control_plane_product_requirements.md`
- `plans/main_chat_agent_control_plane_ui_contract_v1.md`

## 3. Non-goals

- Do not run, fill, or fabricate S2-D01 through S2-D24 manual dogfood rows.
- Do not claim `ready_for_limited_internal_trial`.
- Do not create a second task panel, event model, task runtime, memory model,
  proposal format, or readiness gate.
- Do not implement Stage 4 memory/knowledge asset management beyond displaying
  existing proposal/memory runtime state.
- Do not implement Stage 5 release/debug operations beyond reviewer trace
  export required for Stage 3.
- Do not enable arbitrary external writes.
- Do not lower Stage 1/Beta/Stage 2 gates.

## 4. Required Implementation Areas

### Phase 0: contract alignment

Audit existing Main Chat execution surfaces and document the final data path in
the implementation report:

```text
Main Chat send/stream
  -> AgentIngress / strategy route
  -> AgentTaskSession / ActionQueue / Transcript / Event stream
  -> MainChatAgentStateSnapshot
  -> AgentControlPlane
```

If a UI state cannot be derived from this path, show a diagnostic fallback
instead of fabricating the state.

### Phase 1: primary control plane

Make `AgentControlPlane` the primary execution surface in Main Chat.

Requirements:

- no duplicate authoritative task panel;
- compact fallback only when typed state is missing;
- active task shell visible as soon as task/session/run identity is available;
- active task shell must use existing `MainChatAgentStateSnapshot`, task detail,
  event stream, or AgentIngress/task ids. It must not introduce a parallel
  frontend-only task state model;
- stable dimensions and readable responsive layout.

### Phase 2: execution timeline

Upgrade actions, observations, blockers, proposals, plan steps, event stream,
and final delivery into a coherent execution timeline.

Requirements:

- current action is visually emphasized;
- action rows link to observation or blocker evidence;
- observation previews are bounded and source-labeled;
- event stream shows sequence/source without overwhelming default view.

### Phase 3: scoped controls

Make controls visibly scoped and runtime-backed:

- resume/retry/cancel task controls;
- approve once / deny / defer permission controls;
- accept/reject/edit/defer proposal controls;
- plan confirm/edit/execute/skip/cancel/review controls;
- rollback controls only when memory lifecycle supports them.

Each control must show or carry exact object identity. Disabled controls must
explain missing runtime support or invalid state.

### Phase 4: final delivery and reload recovery

Make final delivery the terminal contract and make task state recoverable after
navigation or refresh.

Requirements:

- final delivery sections stay separated;
- completed/proposed/blocked/skipped/pending cannot be collapsed into "done";
- current conversation reload can restore latest relevant task snapshot and
  event stream into `AgentControlPlane`;
- reload recovery must use existing task session store, event stream, or
  conversation-linked task metadata. It must not recover by fuzzy matching
  message text or creating a new task;
- stale/missing state shows diagnostic blocker, not fake completion.

### Phase 5: Stage 3 eval and report

Add deterministic coverage for `UX3-01` through `UX3-13` in
`plans/main_chat_stage3_execution_ux_product_contract.md`.

The implementation must add or extend a focused Stage 3 UX coverage test/report
surface, for example `main_chat_stage3_execution_ux`. It must list every UX3 row
as passed, failed, or blocked with named blockers. This is not a new readiness
gate and must not return `ready_for_limited_internal_trial`.

The execution-first claim is only valid if at least `UX3-02`, `UX3-03`,
`UX3-04`, `UX3-06`, `UX3-09`, `UX3-11`, and `UX3-12` pass.

Reviewer trace export must copy a bounded one-line JSON object with stable keys:
`schemaVersion`, `taskId`, `runId`, `status`, `route`, `blockers`, `provider`,
`model`, `finalDeliveryStatus`, and `timestamp`.

Create `plans/main_chat_stage3_implementation_report.md` with:

- completed phases;
- changed files;
- UX3 scenario results;
- tests run;
- remaining blockers;
- explicit statement that Stage 3 does not grant limited internal trial
  readiness.

## 5. Test Plan

Run at minimum:

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
pnpm --dir frontend typecheck
pnpm --dir frontend format:check
pnpm --dir frontend test -- src/pages/ChatPage.test.tsx src/components/AgentControlPlane.test.tsx src/tauri.test.ts
```

Run browser/e2e only when the local host can support it:

```bash
pnpm --dir frontend test:e2e -- main-chat-stage1-dogfood.spec.ts --reporter=line
```

Do not mark Stage 3 complete if:

- UI claims are derived from assistant prose instead of runtime state;
- duplicate task surfaces disagree;
- blockers lack next action or terminal explanation;
- final delivery claims blocked/proposed/skipped work is completed;
- tests require synthetic manual dogfood evidence;
- Stage 2 readiness semantics are weakened.

## 6. Acceptance

Stage 3 can be accepted when:

- `UX3-01` through `UX3-13` are covered;
- Main Chat default path feels execution-first for task-like prompts;
- ordinary direct answers remain compact and not over-instrumented;
- every visible action/observation/blocker/proposal/final-delivery claim is
  runtime-backed;
- reviewer trace export is useful for later manual dogfood;
- Stage 2 readiness remains fail-closed without real manual dogfood and
  current-commit live evidence.

## 7. Required Final Response

After implementation, report:

- whether Stage 3 is complete;
- what changed;
- which UX3 scenarios passed;
- tests run and any tests not run;
- whether Stage 2 readiness remains `not_ready_for_limited_internal_trial`;
- whether it is appropriate to proceed to Stage 4 preparation.
