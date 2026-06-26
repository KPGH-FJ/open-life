# Main Chat Next 6 Steps Acceptance Matrix

> Date: 2026-06-26
> Status: active acceptance artifact for the next Main Chat Agent development cycle
> Parent: `plans/main_chat_next_6_steps_master_spec.md`

## 1. Purpose

This matrix turns the next six steps into auditable acceptance rows. A row is
not complete unless the expected evidence is present in code, tests, and the
reported gate output.

## 2. Status Labels

- `baseline_passed`: current baseline already proves the row.
- `not_started`: row has not been implemented.
- `partial`: some evidence exists but the row cannot be credited.
- `blocked`: implementation cannot proceed without an external dependency.
- `complete`: future implementation has passed all evidence requirements.

## 3. Matrix

| ID | Step | Scenario | Current status | Required evidence | Negative assertions | Required commands |
| --- | --- | --- | --- | --- | --- | --- |
| S1-RF20 | 1 | User asks whether a blocked task completed. | complete | `taskStatus=blocked`, bounded `blockerCodes`, `uiStatus=restricted`, next control metadata. | Must not say completed; must not call model; must not parse assistant prose. | `cargo test -p openlife-tauri main_chat_runtime_facts -- --nocapture` |
| S1-RF21 | 1 | User asks status while a permission action is pending. | complete | `taskStatus=waiting_permission`, `pendingPermissionCount>0`, bounded permission target label, `uiStatus=waiting_for_user`. | Must not expose raw unsafe manifest; must not execute pending action; must not claim durable completion. | `cargo test -p openlife-tauri main_chat_runtime_facts -- --nocapture` |
| S1-B-STREAM | 1 | Provider route runtime facts work through stream. | complete | Slice B stream command-surface proof exists and no `slice_b_provider_route_stream_out_of_scope`. | Must not call model when answering pre-model blocked route facts. | Runtime facts test plus command-surface matrix |
| S1-C-STREAM | 1 | Tool availability runtime facts work through stream. | complete | Slice C stream command-surface proof exists and no `slice_c_tool_availability_stream_out_of_scope`. | Must not run active reachability probe; must not expose raw MCP manifest. | Runtime facts test plus command-surface matrix |
| S1-D-STREAM | 1 | Agent self-state runtime facts work through stream. | complete | Slice D stream command-surface proof exists and no `slice_d_agent_self_state_stream_out_of_scope`. | Must not use current self-state question task as the target task; must not infer from assistant prose. | Runtime facts test plus command-surface matrix |
| S1-READY | 1 | Runtime Facts full-layer readiness. | partial | Full report covers required RF rows or names blockers; `runtimeFactsReady` may become true only when full contract passes. | Must not flip `runtime_facts_ready` from slice-only success. | Runtime facts full report command when implemented |
| S2-DIRECT | 2 | External live DirectAnswer. | complete | Credited direct external live report with provider/model/run/task trace and non-empty normalized response preview. | Must reject scripted, local, fixture, loopback, synthetic, local-test HTTP credit. | Opt-in live final acceptance command |
| S2-WEB | 2 | Provider-backed web AgentLoop. | complete | Credited web AgentLoop report with governed `web.*` target, action status succeeded, no single-step fallback. | Must not overlap MCP success or ToolPermission proposal trace. | Opt-in live final acceptance command |
| S2-MCP | 2 | Provider-backed registered MCP AgentLoop. | complete | Credited MCP report with multi-candidate registered MCP set, provider-ranked selection, safe labels, and successful governed action. | Must not accept deterministic-only or local-ranked selection as provider-backed credit. | Opt-in live final acceptance command |
| S2-PERM | 2 | Provider-backed MCP ToolPermission proposal. | complete | Credited proposal-permission report with selected MCP candidate and pending permission proposal target match. | Must not also claim MCP read success; must not execute write. | Opt-in live final acceptance command |
| S2-READY | 2 | Live provider gate ready. | complete | `live_provider_ready_count=4`, live provider coverage booleans true, acceptance live gate ready. | Must fail closed without opt-in, key, network, or external provider. | `cargo test -p openlife-tauri main_chat_final_acceptance -- --nocapture` and opt-in live command |
| S3-INV | 3 | Legacy fallback strategy inventory. | complete | `main_chat_kernel_support_disposition` covers all ordinary strategies; ReAct with no specific kernel target gets a bounded kernel memory read, and no current ordinary strategy needs hidden legacy success. | Must not silently route unsupported ordinary turns to legacy success. | `cargo test -p openlife-tauri main_chat_kernel -- --nocapture`; focused ReviewMaturation send/stream command-surface test |
| S3-REVIEW | 3 | `ReviewMaturation` disposition. | complete | `ReviewMaturation` enters MainChatKernel as `kernelSupportDisposition=governed_blocker` with `review_maturation_kernel_executor_unavailable`, blocked task state, transcript metadata, no actions, no model call, and no legacy fallback. | Must not disappear into legacy generation. | `cargo test -p openlife-tauri main_chat_kernel -- --nocapture`; `cargo test -p openlife-tauri main_chat_command_surface -- --nocapture` |
| S3-ZERO | 3 | Default command surface legacy count remains zero. | complete | Command-surface report legacy fallback count remains zero and all matrix cases remain kernel-backed. | Must not hide fallback by omitting metadata. | command-surface matrix and final acceptance tests |
| S4-SPLIT | 4 | Runtime Facts module split. | complete | `main_chat_runtime_facts.rs` is now a facade over focused `contract`, `registry`, `resolver`, `clock`, `provider_route`, `tool_availability`, `agent_self_state`, and `eval` modules; `main_chat_runtime_facts_responsibilities_are_split_into_focused_modules` guards the split. | Must not create new catch-all file or duplicate fact definitions. | `cargo test -p openlife-tauri main_chat_runtime_facts -- --nocapture` |
| S4-KERNEL | 4 | Kernel consumes typed boundary only. | complete | Kernel uses `MainChatRuntimeFactPreModelRequest` / `MainChatRuntimeFactPostModelRequest` and typed resolver entry points; `main_chat_kernel_consumes_runtime_facts_through_typed_boundary_only` rejects fact-specific resolver/classifier or eval imports. | Must not move fact-specific rules into kernel. | `cargo test -p openlife-tauri main_chat_runtime_facts -- --nocapture` |
| S4-REGRESS | 4 | Refactor behavior preserved. | complete | Runtime Facts focused tests still cover RF-01 through RF-21, command-surface matrix still passes send/stream runtime facts paths, and `runtime_facts_ready=false` semantics are unchanged. | Must not change readiness semantics during pure refactor. | runtime facts test plus command-surface matrix |
| S5-DEFAULT | 5 | Default UI shows task status without diagnostics. | complete | `ChatPage` renders default `main-chat-agent-status` from structured `generation_result`, task state, run/delivery, blocker, and proposal/permission evidence; product vocabulary covers completed, running, waiting, restricted, blocked, trace gap, proposal pending, and permission pending. | Does not require `showMainChatDiagnostics`; helper tests assert status mapping without assistant prose parsing. | `pnpm --dir frontend test -- src/pages/ChatPage.test.tsx` |
| S5-ACTION | 5 | Default UI exposes safe next action. | complete | Proposal review, permission review, retry, resume, cancel, and refresh context actions render only from structured pending counts, task `can*` flags, task controls, or `safeNextControls`; command assertions cover retry/resume/cancel/refresh context wiring. Default status shortcuts link to Review Center; task-continuity detail may execute an explicit ToolPermission accept+resume only when structured proposal/action evidence belongs to that task. | Must not show unsafe or impossible controls; memory/durable/write-like proposal acceptance stays out of task-continuity direct controls. | `pnpm --dir frontend test -- src/pages/ChatPage.test.tsx` |
| S5-TRACE | 5 | Developer trace remains bounded. | complete | Default status surface can open the expanded `ReasoningTracePanel`; trace rows still read structured `generation_result` fields, hide raw input, strip control characters, and redact absolute workspace paths. | Must not parse assistant prose; must not expose raw prompts, keys, manifests, or absolute paths. | `pnpm --dir frontend test -- src/components/ReasoningTracePanel.test.tsx` |
| S6-E2E | 6 | Real task suite. | partial | Step 6 now has an 11-journey matrix, machine-readable report at `frontend/test-results/main-chat-step6-product-acceptance-report.json`, focused report credit tests, Rust final-gate ingestion, a Playwright spec that emits explicit blockers when real Tauri UI/live evidence is unavailable, and dedicated WebDriver entries. Strict `test:e2e:tauri:step6` requires full local+external acceptance; local-only `test:e2e:tauri:step6:local` is for supported Linux CI and can pass only when real Tauri UI proves all 9 deterministic local journeys while `S6-LIVE-WEB` / `S6-LIVE-MCP` remain explicitly blocked and `acceptanceReady=false`. Step 6 reports must carry `schemaVersion=step6-product-acceptance-v1`, exact readiness semantics, `smokePassed=true`, a fresh bounded RFC3339 `generatedAt`, a recomputable `reportDigest`, exact observed 11-journey count/order, distinct non-synthetic task/run ids for every non-blocked journey, expected journey kind, real-Tauri observation path, per-journey answer/runtime/UI evidence labels, explicit `uiStatusEvidence` labels for every observed journey, blocked live rows with structured `blocked_live_evidence` UI status, local rows free of live-provider status/provider metadata, safe UI/trace labels, and an approved evidence source: ready reports require `tauri_command_surface_step6_browser_observed`, while blocked reports require `tauri_command_surface_unavailable`; stale, tampered, reordered/truncated, synthetic runtime-id, screenshot-like, visible-state-only UI claim, unsafe-label, local-live-metadata, missing blocked-live UI status, or incomplete browser reports fail with typed blockers. Step 6 `overallReady` now also requires the nested Main Chat final acceptance gate to be ready; incomplete final acceptance produces `step6_final_acceptance_not_ready` and preserves `finalAcceptanceBlockers`. `.github/workflows/step6-tauri-product-acceptance.yml` installs Linux WebDriver dependencies, builds the debug Tauri app, runs the local-only Step 6 WebDriver path, asserts no fake live credit, and uploads the report. The same workflow has a manual `workflow_dispatch` `run_external_live` path that uses only `OPENLIFE_MAIN_CHAT_LIVE_PROVIDER_EVAL` plus dedicated `OPENLIFE_LIVE_EVAL_PROVIDER` / `OPENLIFE_LIVE_EVAL_BASE` / `OPENLIFE_LIVE_EVAL_MODEL` / `OPENLIFE_LIVE_EVAL_API_KEY` variables, prepares a non-persisted in-memory live provider scheduler before the external-live rows, and then requires all 11 journeys with `acceptanceReady=true`; missing key/route data stays blocked and cannot mint live credit. Both Playwright and WebDriver paths now reject local/synthetic/loopback provider labels instead of crediting any cloud route as external live evidence. `S6-PERMISSION` uses a seeded pending ToolPermission task, clicks the Chat task-continuity `Accept proposal` control, requires accepted proposal evidence, automatic resume replay, and final delivery. Current evidence on this machine remains blocked, not credited: default Playwright produces `e2eEnvironmentReady=false`, no local journey pass, and blocked live rows; the strict Step 6 Tauri WebDriver entry fail-closes on macOS with `tauri_webdriver_macos_not_supported_by_tauri_driver`. | Must not accept screenshots alone; must not accept stale report schemas, stale/tampered or old report traces/digests, reordered/truncated observed journey arrays, reused or synthetic runtime identities, wrong journey kinds or observation paths, wrong journey evidence labels, unsafe UI/trace labels, blocked live rows without structured blocked UI status, visible-state-only or boolean-only UI status claims, local rows carrying live-provider status/provider metadata, unapproved evidence sources, or reports without smoke proof; must not mark local fixture as live external proof; must not weaken or bypass the nested final acceptance gate; must not credit default Vite/browser, unsupported Tauri WebDriver, generic provider env vars, persisted settings writes, or local/loopback provider evidence as external live proof. | `pnpm --dir frontend test -- src/step6ProductAcceptance.test.ts`; `pnpm --dir frontend test -- src/stage1BrowserEvidence.test.ts`; `pnpm --dir frontend test:e2e`; `pnpm --dir frontend test:e2e:tauri:step6` currently fail-closes on macOS; `pnpm --dir frontend test:e2e:tauri:step6:local` is expected to pass only on supported Linux WebDriver runners with live rows blocked; manual GitHub Actions strict live dispatch requires dedicated `OPENLIFE_LIVE_EVAL_*` variables and secret; `cargo test -p openlife-tauri main_chat_final_acceptance -- --nocapture` |

S6 report-level readiness/smoke/count/live-blocker fields and safety summary claims (`noSilentDurableWrite`, `noHiddenLegacyFallback`, `noLocalEvidenceCreditedAsExternalLive`, `noInventedUnavailableEvidence`, and `uiStatusFromStructuredEvidence`) are digest-covered. Safety claims must also match row-derived evidence; claim mismatches fail with typed `step6_browser_*_claim_mismatch` blockers.

S6 observed journey rows must also carry digest-covered `entryPoint` and `routeStrategy` evidence. Chat-prompt journeys require `ordinary_main_chat_input`, seeded permission/recovery controls require `task_continuity_control`, blocked live rows require `blocked_live_evidence_report`, and any unsafe, missing, mismatched, legacy, or fallback route evidence fails the browser report builder, WebDriver validator, and Rust Step 6 gate.

S6 local deterministic rows must not carry live-provider metadata. The browser report builder, WebDriver validator, and Rust Step 6 gate reject local rows with live status or provider-kind fields instead of allowing local evidence to look provider-backed.

S6 final delivery credit must come from structured `finalDeliverySections`; each non-blocked journey must include at least one matrix-approved final delivery section for that journey. Trace rows, generic unrelated final sections, or boolean-only `finalDeliveryObserved` claims are not accepted as final delivery evidence.

S6 blocked live rows still must satisfy every per-row safety invariant. A blocked live report that carries `legacyFallbackUsed`, `silentDurableWriteDetected`, invented unavailable evidence, or local-fixture live credit fails with row-specific typed blockers instead of relying only on summary booleans.

S6 WebDriver `--validate-journeys-only` must validate and emit the full static journey contract, including id/order, kind, execution mode, prompts, answer/runtime/UI/final-delivery evidence labels, seeded prep task ids, and control labels; frontend tests compare that emitted contract against the TypeScript Step6 matrix.

S6 current evidence on 2026-06-26: GitHub Actions Linux PR run
`28244710624` at commit `d523d75cf4b5b577a068b8f8f9ee5d29adef6f78`
completed the local-only Step 6 path. The uploaded report artifact
`7908263816`
(`sha256:911d8c1a0805e64aadfa736e2d18cedb8b74c73733840c6e6ba02a6c3b8d0756`)
proves `e2eEnvironmentReady=true`, `localDeterministicReady=true`,
`externalLiveReady=false`, `acceptanceReady=false`, all 9 deterministic
local journeys passed, `S6-LIVE-WEB` and `S6-LIVE-MCP` were explicit
blocked-live rows, no local evidence was credited as external live, and the
nested final gate stayed fail-closed on missing external live credit. Manual
workflow_dispatch run `28243975872` with dedicated `OPENLIFE_LIVE_EVAL_*`
configuration also completed the local-only stage, proved the provider
preflight was configured and ready for external provider `deepseek`, then
failed the strict external-live stage at `S6-LIVE-WEB` with
`webdriver_control_plane_delivery_timeout`; the uploaded report artifact
`7908037640`
(`sha256:9166c046defd661da9180b540666ceaffbe1e8f4f3980488b44f940f276a7c72`)
does not provide external live final-gate credit. Therefore Step 6 remains
`partial`: Linux real Tauri local evidence is complete, external live final
gate credit is still blocked, and Main Chat Agent Execution v1 must not be
declared complete.

## 4. Baseline Commands

These commands should be run before starting Step 1 and after each step unless a
step-specific command set supersedes them:

```bash
cargo fmt --check
cargo check -p openlife-core
cargo check -p openlife-tauri
cargo test -p openlife-tauri main_chat_runtime_facts -- --nocapture
cargo test -p openlife-tauri main_chat_command_surface_eval_gate_covers_send_stream_runtime_matrix -- --nocapture
cargo test -p openlife-tauri main_chat_final_acceptance -- --nocapture
pnpm --dir frontend format:check
pnpm --dir frontend typecheck
git diff --check
```

## 5. Hallucination Checks

Before marking any row complete, verify:

- the expected field exists in code or serialized report output;
- the test asserts the field, not only a prose answer;
- a negative assertion exists for the main failure mode;
- no out-of-scope row was silently removed;
- no ignored live test is counted as completed;
- no fixture, local, scripted, or synthetic path is credited as external live
  provider evidence.
