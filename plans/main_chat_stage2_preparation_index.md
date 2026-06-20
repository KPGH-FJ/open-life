# Main Chat Stage 2 Preparation Index

> Date: 2026-06-19
> Stage: Main Chat Agent Stage 2 - Internal Trial Readiness
> Status: implementation mechanism report available; readiness blocked on manual evidence

## 1. Stage 2 Direction

Stage 2 should begin now that Stage 1 automated engineering dogfood has passed.
The next target is limited internal trial readiness, not another deterministic
engineering-only gate and not public beta.

Stage 2 must prove:

- real internal users can start from Main Chat;
- the Agent visibly executes, blocks, asks, proposes, or recovers;
- live provider behavior is tested separately from deterministic readiness;
- memory and knowledge updates stay proposal-first;
- final delivery is honest about completed, proposed, blocked, skipped, and
  pending work;
- manual dogfood produces trace-backed product feedback.

## 2. Preparation Documents

| Document | Purpose |
| --- | --- |
| `plans/main_chat_stage2_industry_best_practices.md` | Source-backed practices from OpenAI, Anthropic, Codex, Claude Code, and MCP security guidance. |
| `plans/main_chat_stage2_objective_and_scope.md` | Defines Stage 2 target, non-goals, workstreams, and exit criteria. |
| `plans/main_chat_stage2_internal_trial_acceptance_matrix.md` | Product acceptance matrix for limited internal trial. |
| `plans/main_chat_stage2_manual_dogfood_task_set.md` | Real user task set for manual dogfood and reviewer reporting. |
| `plans/main_chat_stage2_manual_dogfood_reviewer_worksheet.md` | Non-evidence worksheet for reviewers to collect S2-D01 through S2-D24 trace-backed rows before writing the machine-readable artifact. |
| `plans/main_chat_stage2_manual_dogfood_artifact_template.json` | Non-evidence JSON template for all S2-D01 through S2-D24 rows; it intentionally fails validation until real reviewer/runtime evidence replaces placeholders. |
| `plans/main_chat_stage2_live_provider_eval_plan.md` | External live-provider scenarios, environment contract, and fail-closed rules. |
| `plans/main_chat_stage2_agent_control_plane_product_requirements.md` | Product requirements for the execution-first task panel. |
| `plans/main_chat_stage2_memory_proposal_trial_flow.md` | Memory/proposal/knowledge flow for internal trial. |
| `plans/main_chat_stage2_failure_recovery_requirements.md` | Failure taxonomy, runtime/UI requirements, and recovery scenarios. |
| `plans/main_chat_stage2_readiness_gate_contract.md` | Typed readiness command/report contract and fail-closed rules. |
| `plans/main_chat_agent_stage2_internal_trial_goal_spec.md` | CLI goal-mode implementation entrypoint. |
| `plans/main_chat_stage2_implementation_report.md` | Current Stage 2 implementation evidence, verification run, blockers, and residual risks. |

## 3. Recommended Development Order

1. Build Stage 2 readiness report skeleton from `main_chat_stage2_readiness_gate_contract.md`.
2. Add/extend manual dogfood report schema and task runner support.
3. Productize AgentControlPlane P0 states and trace export/reporting.
4. Implement memory proposal trial flow gaps exposed by M2 tasks.
5. Implement failure recovery gaps exposed by R2 tasks.
6. Run external live provider P0 scenarios and keep failures trace-backed.
7. Run manual dogfood and convert failures into deterministic regressions.
8. Return `ready_for_limited_internal_trial` only when P0 acceptance passes.

## 4. CLI Goal Prompt

Use this short prompt for CLI goal mode after review:

```text
Implement Main Chat Agent Stage 2 Internal Trial Readiness.
Read plans/main_chat_agent_stage2_internal_trial_goal_spec.md and every
required document listed there. Keep scope to Stage 2. Reuse existing Beta v1
and Stage 1 runtime objects; do not create parallel task, event, memory, plan,
proposal, skill, or tool systems. Build the internal-trial readiness report,
manual dogfood evidence flow, live-provider P0 eval integration,
AgentControlPlane P0 product states, memory proposal trial flow, and failure
recovery coverage needed to return ready_for_limited_internal_trial or named
blockers. Do not claim internal trial readiness without manual dogfood and
real live-provider P0 evidence. If live-provider evidence is unavailable, return
not_ready with named blockers instead of deferring it inside this stage.
It is acceptable to finish implementation_complete_for_stage2_mechanism while
the readiness gate still returns not_ready because real manual reviewer
evidence is missing, or because the final gate is run without known build
provenance.
```

## 5. Readiness To Start Stage 2 Development

Stage 2 development can start after:

- these preparation documents are reviewed;
- working tree is clean or intentionally staged;
- provider keys are configured outside git;
- Stage 1 Tauri Dogfood and current CI remain green;
- the developer accepts that manual dogfood is part of the exit criteria;
- the developer accepts that live provider failures cannot be hidden by lowering
  final gate standards.

## 6. Non-negotiable Invariants

- No silent durable LifeModel, memory, file, external, plugin, or provider
  writes.
- No hidden legacy fallback.
- No fake browser/live evidence.
- No local/mock/scripted provider credited as external live.
- No knowledge file can override runtime privacy/tool/model policy.
- No "done" final delivery for proposed, blocked, skipped, or unexecuted work.
