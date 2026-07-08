# OpenLife Repository Stage6A Phase7 Trial Readiness Decision

> Date: 2026-07-08 Asia/Shanghai
> Evidence run: 2026-07-07T16:51:41Z
> Commit checked: `3a33c4b`
> Branch: `codex/openlife-product-core-baseline`
> Status: Stage6A trial/readiness decision, not Phase7 completion

## Decision

Stage6A is **red-fail-closed for product trial readiness** and **green for the
required local executable gate set**.

This does not mark Phase7 as finished. It does not mark Main Chat Agent
Execution v1 as finished. It does not mark external live-provider evidence as
finished. No local HTTP, mocked, fixture, ignored, or browser-fallback evidence
is counted as external live evidence.

No ADR move, plan archive move, source authority promotion, retired final
acceptance command restoration, or retired final acceptance test-owner
restoration was performed.

## Status Vocabulary

| Status | Meaning in Stage6A |
| --- | --- |
| `green` | The command or check ran and passed with durable evidence. |
| `red-fail-closed` | The command or check ran and refused readiness with explicit blockers and no green credit. |
| `blocked` | The evidence could not be collected in this environment; no readiness credit is granted. |

## Computer Use Trial Report

Current report:
`frontend/test-results/phase7-computer-use-trial/trial-report.md`.

Status: `RED` / `red-fail-closed`.

The report records that local executable gates are green, while the native
Computer Use/Tauri/WebDriver product trial was not executed in this environment.
It keeps product trial readiness red-fail-closed with explicit blockers and no
external live-provider credit.

| Old blocker from prior report | Current Stage6A status |
| --- | --- |
| Trial isolation failed because the controlled UI process did not inherit `OPENLIFE_DATA_DIR`. | Still `blocked` for product credit. The current Tauri WebDriver runner does not run on this macOS environment, so a clean isolated native UI process was not re-proven. |
| External live fact request completed through local `memory.search` instead of fail-closing when no web/weather evidence existed. | Not reproducible at command-surface level: `cargo test -p openlife-tauri main_chat_command_surface -- --nocapture` passed, including English and Chinese weather tests that require `web.search` evidence and `network_policy_blocked` when network is unavailable. Still not re-proven through isolated native UI. |
| First LifeModel build was not cleanly verifiable because the UI was not using isolated data. | Still `blocked`; no isolated native UI trial was executed. |
| Proposal accept/edit/reject and permission resume were not completed in Computer Use. | Still `blocked`; Step6 journey matrix includes permission/recovery scenarios, but WebDriver could not observe them in this environment. |
| Cross-page consistency was not accepted as green. | Still `blocked`; no clean native UI cross-page run was executed. |

## Required Gate Results

All required local executable gates were rerun after the Playwright cleanup
side effect so their logs exist under
`frontend/test-results/phase7-computer-use-trial/terminal-logs/`.

| Command | Status | Durable evidence |
| --- | --- | --- |
| `git diff --check` | `green` | `stage6a-git-diff-check.log` |
| `cargo fmt --check` | `green` | `stage6a-cargo-fmt-check.log` |
| `cargo test -p openlife-tauri single_system -- --nocapture` | `green`, 17 passed | `stage6a-cargo-test-single-system.log` |
| `cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture` | `green`, 26 passed | `stage6a-cargo-test-main-chat-runtime-module.log` |
| `cargo test -p openlife-tauri main_chat_command_surface -- --nocapture` | `green`, 61 passed | `stage6a-cargo-test-main-chat-command-surface.log` |
| `corepack pnpm --dir frontend typecheck` | `green` | `stage6a-frontend-typecheck.log` |
| `corepack pnpm --dir frontend format:check` | `green` | `stage6a-frontend-format-check.log` |
| `corepack pnpm --dir frontend test -- App.test.tsx ChatPage.test.tsx tauri.test.ts` | `green`, 128 passed across 3 files | `stage6a-frontend-vitest-selected.log` |

Warnings from Rust dead-code/unused checks and React Router future flags were
observed, but they did not fail the requested gates.

## Product Trial And Browser Evidence

`corepack pnpm --dir frontend exec node scripts/step6-tauri-webdriver.mjs
--validate-journeys-only` is journey contract validation only, with no execution
credit. It validates 11 Step6 journey definitions, including 9 local
deterministic journey definitions and 2 external-live journey definitions.

`corepack pnpm --dir frontend exec node scripts/step6-tauri-webdriver.mjs
--validate-observed-rules-only` validates observed-rule fixtures only, with no
native product-trial execution credit.

`corepack pnpm --dir frontend test:e2e:tauri:step6:local` is
`red-fail-closed`. It did not run a product browser session. The explicit
blockers are:

- `step6_product_acceptance_e2e_blocked`
- `real_tauri_browser_command_surface_unavailable`
- `tauri_webdriver_environment_not_ready`
- `tauri_webdriver_macos_not_supported_by_tauri_driver`

The generated Step6 product acceptance report at
`frontend/test-results/main-chat-step6-product-acceptance-report.json` records:

- `e2eEnvironmentReady=false`
- `localDeterministicReady=false`
- `externalLiveReady=false`
- `acceptanceReady=false`
- `passedJourneys=[]`
- `blockedLiveJourneys=["S6-LIVE-WEB","S6-LIVE-MCP"]`
- no hidden legacy fallback
- no silent durable write
- no local evidence credited as external live

The live-provider/Tauri preflight log records:

- `OPENLIFE_MAIN_CHAT_LIVE_PROVIDER_EVAL_present=false`
- `OPENLIFE_PROVIDER_API_KEY_present=false`
- `OPENAI_API_KEY_present=false`
- `OPENLIFE_MAIN_CHAT_NETWORK_ENABLED_present=false`
- `platform=darwin`
- `tauri_driver_available=false`

An optional browser-only Playwright smoke attempt was made and timed out waiting
120000 ms for the configured web server. It is not counted as product trial
credit, native Tauri evidence, or external live evidence.

## Stage6A Readiness Classification

| Area | Classification | Reason |
| --- | --- | --- |
| Phase7 local deletion/readiness gates | `green` | Required local commands passed. |
| External weather/fact fail-closed at command surface | `green` for command surface only | Current command-surface tests passed the weather/network blocker regressions. |
| Clean isolated native Computer Use/Tauri product trial | `blocked` | Current environment cannot run the Tauri WebDriver product session. |
| Step6/Tauri WebDriver product trial command | `red-fail-closed` | It emitted explicit environment blockers and did not claim readiness. |
| External live-provider evidence | `blocked` | Explicit live opt-in, provider key, network flag, and external provider readiness are absent. |
| First LifeModel build through isolated UI | `blocked` | No isolated native UI run occurred. |
| Proposal accept/edit/reject through isolated UI | `blocked` | No isolated native UI run occurred. |
| Permission resume through isolated UI | `blocked` | No isolated native UI run occurred. |
| Cross-page Companion/Runs/Mailbox/Today consistency | `blocked` | No isolated native UI run occurred. |

## Required Next Evidence Before Green Trial

Phase7 product trial cannot turn green until a clean native UI trial captures:

- the actual UI process inheriting an isolated `OPENLIFE_DATA_DIR`;
- first LifeModel build/readiness from that isolated state;
- external fact request failing closed or using governed read-only evidence;
- proposal accept/edit/reject through the visible UI;
- ToolPermission accept/resume through the visible UI;
- Companion, Runs, Mailbox, and Today showing consistent structured state;
- external live-provider evidence only through the dedicated live harness with
  explicit opt-in, real external provider endpoint, key present, network
  enabled, and no local/mock/fixture credit.
