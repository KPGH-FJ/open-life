# Main Chat Stage 1 Preparation Index

> Date: 2026-06-19
> Stage: Main Chat Agent Stage 1 - Real End-to-End Dogfood
> Status: implementation complete for automated engineering dogfood; manual/internal-trial review remains separate

## 1. Why Stage 1 Exists

Beta v1 proved deterministic readiness through runtime, command-surface, UI
mapping, and structured reports. Stage 1 must prove that the product works as a
real dogfood experience:

- user starts from Chat;
- work-like request creates an observable task;
- actions and observations are visible;
- failures are recoverable or clearly blocked;
- memory/knowledge changes are proposal-first;
- final delivery is accurate.

## 1.1 Current Evidence

Stage 1 automated engineering dogfood passed on the supported Linux Tauri
WebDriver path in GitHub Actions:

- Workflow: `Stage 1 Tauri Dogfood`
- Run id: `27807633105`
- Evidence source: `tauri_command_surface_browser_observed`
- Observed scenarios: `36`
- Passed journeys: `36`
- Failed journeys: `0`
- Blockers: `[]`
- Browser report digest:
  `bytes:25422 hash:sha256:b53415fe64b623298be32b93fe55d3c45b7941c65d94e1ce6f3c716db8ade678`

This is a `ready_for_engineering_dogfood` result for default deterministic
Stage 1 readiness. It is not a `ready_for_internal_trial` result; manual dogfood
and external live-provider evidence remain separate scopes.

## 2. Preparation Documents

| Document | Purpose |
| --- | --- |
| `plans/main_chat_stage1_industry_best_practices.md` | Source-backed lessons from Codex, Claude, Hermes, OpenClaw, and Anthropic eval/tool guidance. |
| `plans/main_chat_stage1_dogfood_gap_inventory.md` | Classifies Beta v1 evidence and identifies what must become real E2E dogfood. |
| `plans/main_chat_stage1_dogfood_scenarios.md` | Defines deterministic and opt-in live scenario matrix. |
| `plans/main_chat_stage1_seed_data_contract.md` | Defines reusable isolated seed data for realistic dogfood. |
| `plans/main_chat_stage1_e2e_harness_contract.md` | Defines Rust/Tauri, frontend, and browser E2E harness requirements. |
| `plans/main_chat_stage1_user_visible_acceptance.md` | Defines user-visible states, controls, final delivery, and non-fake rules. |
| `plans/main_chat_stage1_live_provider_preflight.md` | Defines opt-in live provider environment and fail-closed evidence rules. |
| `plans/main_chat_stage1_manual_dogfood_protocol.md` | Defines human review process and blocker criteria. |
| `plans/main_chat_stage1_readiness_gate_contract.md` | Defines final Stage 1 readiness command/report shape. |
| `plans/main_chat_agent_stage1_dogfood_goal_spec.md` | CLI goal-mode entrypoint for implementation. |

## 3. Historical Implementation Entry Point

The Stage 1 implementation goal is complete. This was the concise instruction
used for CLI goal mode and is retained as audit trail:

```text
Implement Main Chat Agent Stage 1 Real End-to-End Dogfood.
Read plans/main_chat_agent_stage1_dogfood_goal_spec.md and all required reading
listed there. Keep scope to Stage 1. Reuse existing Beta v1 task/event/memory/
plan/proposal/skill/tool objects. Do not create parallel runtime systems. Build
the deterministic seed data, dogfood scenario harness, UI evidence checks, and
run_main_chat_agent_stage1_dogfood_gate report. External live remains opt-in and
separate. Stop and report blockers instead of overclaiming readiness.
```

This CLI prompt is intentionally short. The stricter acceptance rules live in
`plans/main_chat_agent_stage1_dogfood_goal_spec.md` and especially require the
browser-level smoke slice before Stage 1 can be marked complete.

## 4. Readiness To Start Development

Stage 1 implementation work started only after:

- Stage 1 preparation documents are tracked or intentionally staged, and the
  remaining working tree is clean;
- Beta v1 readiness still passes;
- all preparation documents above are present;
- provider keys, if used, are configured outside git;
- developer agrees not to count local/mock provider as external live evidence.
