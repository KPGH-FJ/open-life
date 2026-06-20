# Main Chat Stage 5 Implementation Report

Date: 2026-06-20

## Scope Completed

- Added `main_chat_stage5_release_debug` backend module and Tauri commands for:
  - build/environment preflight;
  - metadata-safe debug bundle export;
  - internal issue report creation;
  - artifact list/get/delete;
  - Stage 5 DBG5-01 through DBG5-24 report aggregation;
  - failure taxonomy.
- Stored debug and issue artifacts under app data aliases such as `stage5/debug_bundles/<id>.json` and `stage5/issue_reports/<id>.json`.
- Added schema versions, artifact ids, aliases, digests, byte sizes, atomic temp-file rename writes, reload/list behavior, and delete behavior.
- Added recursive metadata-safety validation before artifact writes. Raw prompts, raw responses, raw private memory, API-key-like values, auth headers, control characters, and absolute private paths fail closed or are dropped from optional previews.
- Added task-attached and preflight-only issue report paths. Pass/fail task reports require task id, run id, and bundle id. Preflight-only blocked reports may omit task/run/bundle ids when they include an explicit missing-task reason and named blockers.
- Reused existing Main Chat runtime data:
  - `AgentTaskSession`
  - `AgentRun`
  - `ExecutionTranscript`
  - `ActionQueue`
  - `ProposalStore`
  - `MemoryLifecycleStore`
  - Stage 4 managed knowledge helpers
  - Stage 2 readiness and final/live-provider preflight concepts
- Added isolated managed `USER.md` / `MEMORY.md` DBG5 evidence using an isolated eval `AppState` and a UUID temp workspace. It does not write the real repository or real user knowledge files.
- Wired Stage 5 commands into the existing Tauri command handler.
- Extended existing frontend surfaces instead of creating a second control plane:
  - `frontend/src/tauri.ts` wrappers and types;
  - `AgentControlPlane` internal debug operations strip;
  - `ChatPage` handlers for preflight, bundle export, issue report, and artifact refresh;
  - frontend mocks and tests.

## DBG5 Results

Fresh artifact-store report rows:

| Row | Result | Evidence / blocker |
| --- | --- | --- |
| DBG5-01 | passed | Build/version provenance is visible; unknown fields become blockers. |
| DBG5-02 | passed | Default preflight does not invoke external providers or models. |
| DBG5-03 | passed | Missing provider key is reported as `provider_api_key_missing`. |
| DBG5-04 | blocked until matching artifact exists | `stage5_direct_answer_debug_bundle_missing`; DirectAnswer bundle export is covered by focused artifact test. |
| DBG5-05 | blocked until matching artifact exists | `stage5_read_action_debug_bundle_missing`. |
| DBG5-06 | blocked until matching artifact exists | `stage5_policy_blocker_debug_bundle_missing`. |
| DBG5-07 | blocked until matching artifact exists | `stage5_mcp_read_debug_bundle_missing`. |
| DBG5-08 | passed | `model_selected_disallowed_tool` maps to `tool_selection_failure`. |
| DBG5-09 | passed | Provider timeout maps to `provider_failure`. |
| DBG5-10 | blocked until matching artifact exists | `stage5_memory_proposal_debug_bundle_missing`. |
| DBG5-11 | blocked until matching artifact exists | `stage5_memory_context_debug_bundle_missing`. |
| DBG5-12 | passed | Isolated temp-workspace managed `USER.md` draft/confirm/audit proof. |
| DBG5-13 | passed | Isolated temp-workspace managed `MEMORY.md` rollback/reload proof. |
| DBG5-14 | blocked until matching artifact exists | `stage5_final_delivery_debug_bundle_missing`. |
| DBG5-15 | passed | Retry/resume/cancel failure taxonomy maps to `recovery_failure`. |
| DBG5-16 | passed | UI mismatch taxonomy maps to `ui_state_failure`. |
| DBG5-17 | passed | Fake API key/auth header previews are dropped or blocked. |
| DBG5-18 | passed | Raw private memory is blocked by default. |
| DBG5-19 | blocked until issue artifact exists | `stage5_issue_report_artifact_missing`; task-attached and preflight-only issue creation are covered by focused artifact test. |
| DBG5-20 | passed | Unknown build commit remains rejected as release evidence. |
| DBG5-21 | passed | Report has `notAReadinessGate=true`. |
| DBG5-22 | passed | Stage 2 readiness remains fail-closed. |
| DBG5-23 | passed | Local/mock provider evidence is not credited as external live evidence. |
| DBG5-24 | blocked until artifact exists | `stage5_debug_bundle_artifact_missing`; reload/list/delete behavior is covered by focused artifact test. |

Rows that need task-specific debug artifacts intentionally stay blocked in a fresh app-data store with named blockers. Bundle-backed rows require stored bundle content that actually proves the row category, not just a matching scenario id. The focused artifact test proves that a DirectAnswer bundle plus issue report credits `DBG5-04`, `DBG5-19`, and reload row `DBG5-24`, while read action, MCP, and memory rows remain blocked until their own matching artifacts exist.

## Tests Run

- `git diff --check`
- `cargo fmt --check`
- `cargo test -p openlife-core main_chat_agent_v1 -- --nocapture`
- `cargo test -p openlife-tauri main_chat_stage5_release_debug -- --nocapture`
- `cargo test -p openlife-tauri main_chat_stage4_memory_knowledge -- --nocapture`
- `cargo test -p openlife-tauri main_chat_stage3_execution_ux -- --nocapture`
- `cargo test -p openlife-tauri main_chat_agent_stage2_readiness -- --nocapture`
- `cargo test -p openlife-tauri main_chat_final_acceptance -- --nocapture`
- `cargo test -p openlife-tauri main_chat_command_surface -- --nocapture`
- `cargo test -p openlife-tauri main_chat_agent_productization -- --nocapture`
- `cargo test -p openlife-tauri main_chat_product_maturity_v2 -- --nocapture`
- `pnpm --dir frontend typecheck`
- `pnpm --dir frontend format:check`
- `pnpm --dir frontend test -- src/pages/ChatPage.test.tsx src/components/AgentControlPlane.test.tsx src/pages/ProposalReviewPage.test.tsx src/tauri.test.ts`

Expected ignored live-provider tests remained ignored unless external live opt-in, network, and real credentials are supplied.

## Remaining Blockers

- Stage 5 is an operational release/debug layer only. It does not provide live-provider completion evidence.
- Rows requiring concrete task artifacts remain blocked in a fresh artifact store until testers export the relevant metadata-safe debug bundle or issue report.
- Stage 2 manual dogfood rows were not run or filled.
- External live-provider DirectAnswer/web/MCP/proposal-permission evidence remains separate and incomplete.

## Readiness Statement

Stage 5 does not grant limited internal trial readiness.

`ready_for_limited_internal_trial=false`

`readinessClaim=false`

`notAReadinessGate=true`

Main Chat Agent Execution v1 remains not complete until the existing Stage 2/final/live-provider gates have complete auditable evidence.
