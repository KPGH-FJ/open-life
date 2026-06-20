# Main Chat Agent Stage 2 Internal Trial Goal Spec

> Date: 2026-06-19
> Status: prepared for CLI goal mode
> Depends on: Stage 1 automated engineering dogfood and Stage 2 preparation docs

## 1. Objective

Implement **Main Chat Agent Stage 2: Internal Trial Readiness**.

The target output is a truthful readiness report:

```text
ready_for_limited_internal_trial
```

or a fail-closed report with named blockers.

Stage 2 must not rebuild existing runtime systems. Reuse current:

- AgentIngress / StrategyRouter;
- AgentTaskSession and ActionQueue;
- ExecutionTranscript and event stream;
- AgentLoop / ActionExecutor / ExecutionPolicy;
- PlanExecute runtime;
- ProposalStore / ToolPermissionStore;
- Memory lifecycle / EvidenceStore;
- bounded context loader for knowledge assets and selected `SKILL.md`;
- AgentControlPlane and ChatPage.

## 2. Required Reading

Read before editing code:

- `AGENTS.md`
- `plans/README.md`
- `plans/main_chat_stage2_preparation_index.md`
- `plans/main_chat_stage2_industry_best_practices.md`
- `plans/main_chat_stage2_objective_and_scope.md`
- `plans/main_chat_stage2_internal_trial_acceptance_matrix.md`
- `plans/main_chat_stage2_manual_dogfood_task_set.md`
- `plans/main_chat_stage2_live_provider_eval_plan.md`
- `plans/main_chat_stage2_agent_control_plane_product_requirements.md`
- `plans/main_chat_stage2_memory_proposal_trial_flow.md`
- `plans/main_chat_stage2_failure_recovery_requirements.md`
- `plans/main_chat_stage2_readiness_gate_contract.md`
- `plans/main_chat_stage1_manual_dogfood_report.md`
- `plans/main_chat_agent_beta_v1_foundation_inventory.md`
- `plans/main_chat_agent_beta_v1_release_notes.md`

## 3. Non-goals

- Do not implement broad background autonomy.
- Do not implement public Skills Hub or marketplace.
- Do not enable arbitrary external writes.
- Do not create a new memory OS or parallel proposal format.
- Do not make external live provider evidence part of deterministic CI.
- Do not claim public beta readiness.
- Do not lower Stage 1/Beta gates to make Stage 2 pass.

## 4. Required Implementation Areas

### Phase 0: Stage 2 readiness report skeleton

Create or extend `run_main_chat_agent_stage2_readiness_gate` according to
`plans/main_chat_stage2_readiness_gate_contract.md`. The report must aggregate:

- deterministic Stage 1/Beta readiness;
- manual dogfood evidence status;
- live provider P0 status;
- AgentControlPlane P0 state coverage;
- memory proposal trial coverage;
- failure recovery coverage;
- no silent write count;
- no legacy fallback count;
- fake/live/browser evidence rejection counts;
- recommendation and blockers.

### Phase 1: manual dogfood evidence flow

Implement a structured manual dogfood report path for the S2-D task set.

The report must record reviewer id, commit, scenario id, prompt, task/run ids,
result, severity, notes, and blockers. It must fail closed when required P0
manual evidence is missing. Use
`plans/main_chat_stage2_manual_dogfood_report.md` for the reviewer-facing
summary and `frontend/test-results/main-chat-stage2-manual-dogfood-report.json`
for the machine-readable artifact consumed by the readiness gate.

### Phase 2: live provider P0 eval integration

Run or wire the L2 live-provider scenarios from
`main_chat_stage2_live_provider_eval_plan.md`.

Rules:

- real provider/model only;
- no local/mock/scripted/fixture credit;
- no keys in repo;
- missing credentials produce blocked reports;
- model failures produce scenario blockers, not relaxed gates.
- scenario-scoped setup is required when live scenarios intentionally need
  different policy state, such as web disabled for L2-L03 and web enabled for
  L2-L04.

### Phase 3: AgentControlPlane P0 product states

Implement missing UI/runtime mapping for:

- task header;
- plan/action timeline;
- observations;
- blockers;
- permission/proposal controls;
- retry/resume/cancel;
- final delivery;
- reviewer trace export.

All UI claims must be backed by typed runtime evidence.

### Phase 4: memory proposal trial flow

Close gaps for M2 scenarios:

- candidate with source evidence;
- conflict handling;
- accept/reject/edit/defer;
- materialization provenance;
- successful rollback for accepted/materialized memory;
- proposal-first knowledge file edits.

No memory or knowledge update may silently become durable truth.

### Phase 5: failure recovery coverage

Close gaps for R2 scenarios:

- missing source;
- network/web policy blocker;
- missing MCP target;
- disallowed tool;
- permission denied/accepted;
- retry;
- cancel;
- stale resume;
- plan step failure.

Each failure needs a user-facing next action or terminal explanation.

### Phase 6: final Stage 2 gate

The final report can return `ready_for_limited_internal_trial` only when:

- all P0 acceptance matrix rows pass;
- manual dogfood P0 evidence exists;
- required live provider P0 evidence exists;
- no hidden legacy fallback;
- no silent durable writes;
- final delivery is honest for P0 tasks;
- blocker list is empty or contains only approved non-P0 residual risks.

If implementation is complete but real manual dogfood or live provider evidence
is missing, the implementation may report
`implementation_complete_for_stage2_mechanism`, but the readiness recommendation
must remain `not_ready_for_limited_internal_trial`.

## 5. Test Plan

Run at minimum:

```bash
git diff --check
cargo test -p openlife-tauri main_chat_agent_stage1_dogfood -- --nocapture
cargo test -p openlife-tauri main_chat_agent_beta_v1_readiness -- --nocapture
cargo test -p openlife-tauri main_chat_product_maturity_v2 -- --nocapture
cargo test -p openlife-tauri main_chat_command_surface -- --nocapture
cargo test -p openlife-tauri main_chat_final_acceptance -- --nocapture
cargo test -p openlife-tauri main_chat_agent_stage2_readiness -- --nocapture
cargo test -p openlife-tauri main_chat_stage2_manual_dogfood -- --nocapture
cargo test -p openlife-tauri main_chat_stage2_live_provider -- --nocapture
cargo test -p openlife-tauri main_chat_stage2_failure_recovery -- --nocapture
pnpm --dir frontend typecheck
pnpm --dir frontend test -- src/pages/ChatPage.test.tsx src/components/AgentControlPlane.test.tsx src/tauri.test.ts
pnpm --dir frontend test:e2e -- main-chat-stage1-dogfood.spec.ts --reporter=line
```

Run the Stage 2 live P0 evals when real provider credentials are available. If
credentials are unavailable, the implementation can still be complete, but the
Stage 2 readiness recommendation must remain `not_ready_for_limited_internal_trial`.
If the current host cannot run real Tauri WebDriver, rely on Linux CI artifact
for real Tauri browser proof and keep local macOS fail-closed behavior.

## 6. Required Final Report

Final implementation report must include:

- completed phases;
- readiness recommendation;
- manual dogfood status;
- live provider status;
- scenario counts;
- tests run;
- changed files;
- unresolved blockers;
- residual risks;
- explicit statement that internal trial readiness is not public beta readiness.
