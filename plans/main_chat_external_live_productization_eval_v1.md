# Main Chat External Live Productization Eval v1

> Date: 2026-06-17
> Status: preparation artifact for Product Maturity v2
> Parent: `plans/main_chat_agent_product_maturity_v2_goal_spec.md`

## 1. Purpose

This document defines opt-in external live productization eval for Main Chat.

Existing live-provider acceptance provides strict opt-in harness and gate paths
for proving external provider execution. It does not make external live proof a
default deterministic capability: real external proof still requires explicit
env opt-in, network access, and a real provider API key. Product Maturity v2 must
add product-level proof that live execution appears correctly in the UI model:
task, action, observation, blocker/proposal, final delivery, and event deltas.

## 2. Baseline

OpenLife already has:

- opt-in external live DirectAnswer proof path,
- opt-in external live ReAct web/MCP proof path,
- external live final acceptance gate path,
- provider identity and model identity evidence requirements,
- no fallback/no silent write evidence requirements,
- opt-in env-gated execution.

These paths are not default readiness evidence unless the ignored/opt-in live
tests are run successfully with a real external provider.

Missing:

- product-level live UI assertions,
- live event delta assertions,
- live final delivery UI evidence,
- live ToolPermission proposal UI flow proof,
- product readiness report that clearly separates deterministic and opt-in live.

## 3. Rules

- External live eval is opt-in only.
- External live eval must not count toward default deterministic readiness.
- External live eval must not weaken final/live-provider gates.
- External live eval must never serialize API keys.
- Local/mock providers cannot receive external live credit.
- Live product eval failure should block "live product readiness", not default
  deterministic maturity.
- This work must run after deterministic memory lifecycle, event stream, plan
  interaction, task continuity, and skill/tool product gates are passing. Live
  product eval depends on those product evidence surfaces.

## 4. Required Scenarios

| ID | Scenario | Required product evidence |
| --- | --- | --- |
| LIVE-PROD-01 | DirectAnswer through external provider. | task/run, provider/model, final delivery, no tool timeline. |
| LIVE-PROD-02 | External web ReAct read. | action, web observation, source, final delivery, no fake source. |
| LIVE-PROD-03 | External MCP candidate selection. | candidate list, selected target, ranking trace, observation. |
| LIVE-PROD-04 | ToolPermission proposal. | pending proposal, exact action id, no overlapping read success. |
| LIVE-PROD-05 | Live blocker/failure recovery. | blocker, reason, retry/cancel if safe. |
| LIVE-PROD-06 | Live delta stream. | event sequence for route/action/observation/final delivery. |

## 5. Environment

Use the existing live eval convention:

- `OPENLIFE_MAIN_CHAT_LIVE_PROVIDER_EVAL=1`
- `OPENLIFE_LIVE_EVAL_PROVIDER`
- `OPENLIFE_LIVE_EVAL_BASE`
- `OPENLIFE_LIVE_EVAL_MODEL`
- `OPENLIFE_LIVE_EVAL_API_KEY`

Do not commit environment files or keys.

## 6. Product Assertions

Each live product report must include:

- scenario id,
- external provider identity,
- provider model identity,
- task session id,
- run id,
- action ids,
- observation ids,
- proposal ids,
- final delivery id,
- event sequence range,
- UI state assertions,
- blockers,
- direct writes flag,
- legacy fallback flag.

## 7. UI Assertions

Live UI test must prove:

- DirectAnswer does not show fake action timeline.
- Web/MCP read shows action and observation.
- ToolPermission proposal shows approve/deny/defer only for exact action.
- Final delivery separates executed/proposed/blocked/pending.
- External provider trace is visible but bounded.
- If live event stream is enabled, UI applies deltas without requiring full
  refresh for every step.

## 8. Acceptance

This eval is ready when:

- default deterministic gate can run without network,
- deterministic v2 product gates pass before live product scenarios run,
- external live product gate can run with explicit env,
- each live scenario produces product evidence,
- no scenario obtains live credit from local/mock/synthetic provider,
- failure report includes metadata-safe blockers.

## 9. Stop Conditions

Stop if:

- live product eval requires weakening existing final gate,
- provider identity cannot be audited,
- UI evidence cannot be tied to task/action/observation/proposal ids,
- live failures are hidden behind successful assistant text.
