# Main Chat Stage 1 Dogfood Gap Inventory

> Date: 2026-06-18
> Scope: classify Beta v1 evidence before Stage 1 implementation
> Status: preparation artifact

## 1. Purpose

Stage 1 must not duplicate Beta v1 or overclaim existing evidence. This
inventory classifies the current Beta v1 proof rows by how close they are to
real end-to-end dogfood.

Evidence source categories:

- `ordinary_command`: ordinary `send_message` / `start_stream_message` command
  path evidence exists.
- `runtime_gate`: a runtime/product maturity gate proves the capability, but
  not necessarily as a fresh user prompt.
- `task_control`: scenario is a control/read-model action against an existing
  task, plan, proposal, or memory object.
- `ui_mapping`: UI state is mapped to runtime data, but not yet verified by a
  browser-level E2E run for that scenario.
- `opt_in_live`: external live provider is required and not part of default
  deterministic readiness.

## 2. Current High-Level State

The repository currently has strong deterministic Beta readiness:

- 28 default real-task fixtures are marked passed.
- 2 external live fixtures are explicitly opt-in.
- command-surface coverage includes a 38-case matrix.
- `AgentControlPlane` renders the main task/action/observation/blocker/proposal
  states from typed runtime payloads.

The main Stage 1 gap is not missing core runtime objects. The gap is that many
scenarios are still proven through aggregate gates or state mappings instead of
full UI + command + runtime + final delivery dogfood.

## 3. Scenario Classification

| Id | Current classification | Stage 1 gap | Stage 1 decision |
| --- | --- | --- | --- |
| B1 DirectAnswer | `ordinary_command`, `ui_mapping` | Needs full Chat UI E2E with compact trace and no fake timeline. | Reuse as deterministic UI E2E. |
| B2 file read | `ordinary_command`, `ui_mapping` | Needs seeded real workspace file and source preview in UI. | Reuse, add seed file. |
| B3 session search | `ordinary_command`, `ui_mapping` | Needs seeded historical chat data and visible citation. | Reuse, add session seed. |
| B4 accepted memory context | `ordinary_command`, `runtime_gate` | Needs visible active memory/context source state. | Reuse, add accepted memory seed. |
| B5 fixture web read | `ordinary_command`, `ui_mapping` | Fixture-backed web is not live web. Needs deterministic fixture plus opt-in live separation. | Reuse as fixture; add live companion. |
| B6 selected skill | `ordinary_command`, `runtime_gate` | Needs UI selection flow, loaded digest, and unselected-skill exclusion. | Reuse, add UI E2E. |
| B7 MCP read candidate | `ordinary_command`, `runtime_gate` | Needs visible candidate/selection reason and policy proof. | Reuse, add candidate UI check. |
| B8 plan draft/first safe step | `ordinary_command`, `runtime_gate` | Current proof is not enough for full plan edit/execute/review UI loop. | Extend into multi-step UI E2E. |
| B9 skip plan step | `task_control`, `runtime_gate` | Not a fresh Chat prompt; requires existing plan session seed and UI control click. | Keep as task-control E2E. |
| B10 memory proposal | `ordinary_command`, `runtime_gate` | Needs Review Center / proposal controls visible from Chat. | Reuse, add accept/reject branch. |
| B11 memory accept | `task_control`, `runtime_gate` | Needs pending proposal seed and UI accept action. | Keep as seeded task-control E2E. |
| B12 memory rollback | `task_control`, `runtime_gate` | Needs accepted memory seed and proof active context excludes rolled-back memory. | Keep as seeded task-control E2E. |
| B13 task resume | `task_control`, `runtime_gate` | Needs blocked/paused task seed and resume UI action. | Keep as seeded continuity E2E. |
| B14 retry failed read | `task_control`, `runtime_gate` | Needs failed action seed and retry observation proof. | Keep as seeded recovery E2E. |
| B15 cancel task | `task_control`, `runtime_gate` | Needs non-terminal task seed and queued-action stop proof. | Keep as seeded recovery E2E. |
| B16 permission request | `ordinary_command`, `runtime_gate` | Needs exact action/tool/target/scope visible in UI. | Reuse, add approval/deny paths. |
| B17 tool selection explanation | `ordinary_command`, `runtime_gate` | Needs user-visible selection reason and policy evidence. | Reuse, add UI assertion. |
| B18 unselected skill blocked | `ordinary_command`, `runtime_gate` | Needs selected/cleared skill state and blocked final delivery. | Reuse, add UI assertion. |
| B19 final delivery inspection | `task_control`, `runtime_gate` | Needs terminal task seed and section-by-section UI check. | Keep as final delivery E2E. |
| B20 event replay | `task_control`, `runtime_gate` | Needs browser reconnect/replay behavior, not only event command. | Add Playwright/Tauri E2E. |
| B21 memory conflict compare | `ordinary_command`, `runtime_gate` | Needs conflict explanation visible without silent overwrite. | Reuse, add conflict UI check. |
| B22 multi-read AgentLoop | `ordinary_command`, `runtime_gate` | Needs two visible actions and two observations in task timeline. | Reuse, add UI E2E. |
| B23 web blocked by policy | `ordinary_command`, `runtime_gate` | Needs named blocker and next action visible in UI. | Reuse, add UI E2E. |
| B24 missing MCP blocker | `ordinary_command`, `runtime_gate` | Needs named blocker and no fake MCP observation. | Reuse, add UI E2E. |
| B25 external live DirectAnswer | `opt_in_live` | Not run by default; needs live provider preflight and no local/mock credit. | Keep opt-in. |
| B26 external live web/MCP | `opt_in_live` | Not run by default; needs live ReAct trace and provider identity audit. | Keep opt-in. |
| B27 knowledge asset inspection | `ordinary_command`, `runtime_gate` | Needs user-visible loaded/skipped asset inventory and policy boundary. | Reuse, add UI E2E. |
| B28 knowledge asset edit proposal | `ordinary_command`, `runtime_gate` | Needs proposal diff, no direct file write, accept/reject UI path. | Reuse, add UI E2E. |
| B29 stale resume blocker | `task_control`, `runtime_gate` | Needs stale context seed and refresh path visible. | Keep as seeded recovery E2E. |
| B30 durable change inventory | `task_control`, `runtime_gate` | Needs terminal task seed and final delivery audit. | Keep as final delivery E2E. |

## 4. Product Gaps To Close

1. Full browser-level E2E is missing for the most important scenarios.
2. Some real-task rows are aggregate gate proofs, not independent task runs.
3. Seed data is implicit in tests and must become a reusable dogfood fixture.
4. External live provider paths remain opt-in and not counted in default
   readiness.
5. Knowledge asset management is still a Beta slice, not a complete product.
6. UI evidence currently proves mapping, but not enough user journey quality.

## 5. Stage 1 Rule

Stage 1 may reuse all verified Beta foundations, but it cannot mark a scenario
dogfood-ready unless the report contains:

- user input;
- scenario prompt id and bounded prompt preview;
- route/strategy;
- task session id;
- runtime event evidence;
- action/observation/proposal/blocker evidence where applicable;
- visible UI state assertion;
- final delivery section assertion;
- silent-write and legacy-fallback counts;
- whether live provider was attempted.
