# Main Chat Stage 5 Release Debug Eval Matrix

> Date: 2026-06-20
> Stage: Stage 5 - Internal Trial Release and Debug Operations
> Status: preparation matrix

## 1. Purpose

Stage 5 must prove release/debug mechanics. It does not prove that internal
dogfood passed.

The Stage 5 report should be named:

```text
main_chat_stage5_release_debug
```

The report must include:

- `reportKind`;
- `schemaVersion`;
- `scenarioCount`;
- `passedScenarioCount`;
- `blockedScenarioCount`;
- `notAReadinessGate=true`;
- `readinessClaim=false`;
- rows for DBG5-01 through DBG5-24;
- evidence ids;
- blockers;
- build info;
- preflight summary;
- bundle ids;
- issue artifact ids;
- artifact storage summary;
- redaction summary;
- Stage 2 readiness preservation flag.

## 2. DBG5 Scenarios

| ID | Scenario | Expected proof |
| --- | --- | --- |
| DBG5-01 | Build/version provenance is visible. | Commit, branch, app version, timestamp from deterministic build/app metadata, or named unavailable blocker. |
| DBG5-02 | Environment preflight runs without invoking external provider by default. | Provider/key/network/scheduler/workspace/MCP/database status and blockers, with non-provider setup blockers classified as `environment_preflight_failure`. |
| DBG5-03 | Missing provider key is reported as environment blocker. | No model invocation, no fake live credit, blocker label present. |
| DBG5-04 | DirectAnswer task exports debug bundle. | Task/run/route/provider/final delivery metadata, no fake action timeline. |
| DBG5-05 | File read task exports action/observation evidence. | Tool/action/target digest/observation/final synthesis included. |
| DBG5-06 | ReAct web policy blocker exports policy evidence. | Failure class `policy_blocker`, recovery guidance, no fallback claim. |
| DBG5-07 | Registered MCP read success exports candidate and selected tool evidence. | Candidate count, selected target/action, policy allow, observation. |
| DBG5-08 | Tool selection failure is classified. | Failure class `tool_selection_failure` and allowlist/selected mismatch evidence. |
| DBG5-09 | Provider failure is classified separately from Agent failure. | Provider preflight/invocation blocker and environment recovery guidance. |
| DBG5-10 | Memory proposal task exports proposal-first evidence. | Proposal id/status/source evidence, no silent durable write. |
| DBG5-11 | Accepted memory context exports active/excluded memory ids. | Active lifecycle memory ids and excluded rolled-back/rejected ids. |
| DBG5-12 | Managed `USER.md` write exports draft/confirm/audit evidence. | Target, digests, version id, audit id, context reload from isolated eval AppState and temporary workspace only. |
| DBG5-13 | Managed `MEMORY.md` rollback exports snapshot/reload evidence. | Rolled-back version, restored digest, context reload proof from isolated eval AppState and temporary workspace only. |
| DBG5-14 | Final delivery debug separates completed/proposed/blocked/skipped/pending work. | Final delivery fields preserved in bundle. |
| DBG5-15 | Retry/resume/cancel failure is classified. | Task control state, action queue state, failure class `recovery_failure`. |
| DBG5-16 | UI state mismatch can be reported. | Backend snapshot id plus expected/missing visible control labels. |
| DBG5-17 | Export redaction drops fake API keys and auth headers. | Redaction report and no secret string in artifact. |
| DBG5-18 | Export redaction blocks raw private memory by default. | Memory ids/digests only, no raw memory content. |
| DBG5-19 | Issue report includes scenario/reviewer/build/task/run/bundle ids when task-attached. | Task-attached reports include run id; preflight-only reports include named missing task/run blockers; app-data artifact id/storage alias/digest/byte-size and required fields are present. |
| DBG5-20 | Stale or unknown build evidence is rejected. | Artifact validator blocker. |
| DBG5-21 | Stage 5 report cannot claim readiness. | `notAReadinessGate=true`, `readinessClaim=false`. |
| DBG5-22 | Stage 2 readiness remains fail-closed without manual/live evidence. | Stage 2 not-ready blockers preserved. |
| DBG5-23 | Local/mock provider is not credited as external live evidence. | Existing live-provider blockers preserved. |
| DBG5-24 | Debug bundle can be reloaded/inspected after app refresh. | App-data bundle list/get returns same metadata-safe artifact and retention/delete behavior is explicit. |

## 3. Required Negative Assertions

- no raw API key in exported bundle;
- no full prompt/system prompt export by default;
- no full transcript export by default;
- no raw private memory export by default;
- no full knowledge file export by default;
- no hidden legacy fallback;
- no external live credit from local/mock/scripted provider;
- no Stage 2 readiness overclaim;
- no issue artifact without build/task/bundle ids;
- no task-attached issue artifact without run id;
- no preflight-only or environment-blocked issue artifact marked as task
  behavior pass;
- no debug or issue artifact silently written into the git workspace;
- no managed knowledge write/rollback eval writes to the real repository root or
  real user knowledge files;
- no redaction pass that drops required identity/evidence fields;
- no failure pass without trace-backed evidence.

## 4. Minimum Test Plan

Run at minimum after implementation:

```bash
git diff --check
cargo fmt --check
cargo test -p openlife-core main_chat_agent_v1 -- --nocapture
cargo test -p openlife-tauri main_chat_stage5_release_debug -- --nocapture
cargo test -p openlife-tauri main_chat_stage4_memory_knowledge -- --nocapture
cargo test -p openlife-tauri main_chat_stage3_execution_ux -- --nocapture
cargo test -p openlife-tauri main_chat_agent_stage2_readiness -- --nocapture
cargo test -p openlife-tauri main_chat_final_acceptance -- --nocapture
cargo test -p openlife-tauri main_chat_command_surface -- --nocapture
cargo test -p openlife-tauri main_chat_agent_productization -- --nocapture
cargo test -p openlife-tauri main_chat_product_maturity_v2 -- --nocapture
pnpm --dir frontend typecheck
pnpm --dir frontend format:check
pnpm --dir frontend test -- src/pages/ChatPage.test.tsx src/components/AgentControlPlane.test.tsx src/pages/ProposalReviewPage.test.tsx src/tauri.test.ts
```

Add focused frontend tests for any new preflight/debug/export UI.

## 5. Acceptance

Stage 5 can be accepted when:

- DBG5-01 through DBG5-24 are covered as passed or blocked with named blockers;
- at least one bundle each exists for DirectAnswer, read action, policy blocker,
  memory proposal, memory context, managed knowledge write, rollback, and final
  delivery;
- redaction tests prove fake keys/raw memory/raw prompt are not exported;
- task-attached issue report artifact includes required build/scenario/reviewer/
  task/run/bundle ids and can be reloaded;
- preflight-only or environment-blocked issue artifact includes named blockers
  and missing task/run reasons, and cannot claim task behavior pass;
- unsafe required identity/evidence fields block artifact creation instead of
  being dropped for a false pass;
- debug/issue artifacts are schema-versioned, atomically written, stored outside
  the git workspace by default, and have explicit retention/delete behavior;
- Stage 1/2/3/4/final gates are not weakened.
