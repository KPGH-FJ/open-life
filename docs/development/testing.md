# Testing

## Status

Current Phase7 testing and evidence map for the restart-baseline cleanup.
Passing a command proves only the layer named below. It does not close a
finding, make Phase7 green, or convert local/scripted evidence into native
desktop or external-live credit.

## Authority

Authority remains with `AGENTS.md`, `plans/README.md`,
`plans/openlife_single_system_deletion_manifest.md`,
`plans/openlife_single_system_development_preparation.md`, and the
machine-readable restart facts in
`plans/openlife_restart_baseline_cleanup.json`. This page is a developer
explainer beneath that stack.

## Last Fact Sync

2026-07-23 during restart-baseline cleanup. Command results are not inherited
from this date: every claimed pass must identify the SHA and invocation on which
it was reproduced.

## Current Source Map

- Rust workspace and crates: `Cargo.toml`, `openlife-core/Cargo.toml`,
  `src-tauri/Cargo.toml`
- frontend scripts and lockfile: `frontend/package.json`,
  `frontend/pnpm-lock.yaml`
- current Vitest authority: `frontend/vitest.config.ts`,
  `frontend/scripts/current-test-selection.mjs`
- historical Vitest entry:
  `frontend/scripts/vitest.historical.config.ts`
- coverage artifact checker:
  `frontend/scripts/check-coverage-threshold.mjs`
- browser-shell runner: `frontend/playwright.config.ts`
- current browser-shell spec:
  `frontend/e2e/workbench-browser-shell.spec.ts`
- default CI: `.github/workflows/ci.yml`
- current single-system guards:
  `src-tauri/src/single_system_authority_tests.rs`
- current Main Chat owner-shape guards:
  `src-tauri/src/main_chat_runtime_module_tests.rs`
- local live-provider contract harness:
  `src-tauri/src/main_chat_live_provider_tests.rs`
- exact native Phase4F report:
  `docs/phase4f_desktop_product_acceptance/03_native_trial_report.md`

The retired Stage1, Step6, and generic smoke specs are expected absent from the
default Playwright collection:

- `frontend/e2e/main-chat-stage1-dogfood.spec.ts`
- `frontend/e2e/main-chat-step6-product-acceptance.spec.ts`
- `frontend/e2e/smoke.spec.ts`

Their absence is not a broken link to repair.

## Evidence Levels

### 1. Compile And Unit-Contract

The CI check named `Compile and Unit Contract Checks` and the corresponding
local commands may prove:

- Rust/frontend compilation and type correctness;
- unit and focused contract behavior;
- static single-system and expected-absent guards;
- package/build integrity.

They do not launch a browser or Tauri shell and do not prove route usability,
native process identity, durable application, or external provider behavior.

### 2. Workbench Browser Shell

The CI check named `Workbench Browser Shell Smoke` runs the current Workbench
through Vite and Chromium. Its contract is bounded to:

- non-empty Playwright collection;
- `/today`, `/workspace`, `/tasks`, `/review`, `/life-model`, and `/settings`
  render their expected route heading;
- a retired route renders explicit old-page-unavailable state;
- an unknown route renders explicit path-unavailable state;
- uncaught JavaScript page errors fail the run.

The spec must not skip, conditionally return, catch-and-ignore, or otherwise
turn blocked state into a pass. This layer uses no Tauri IPC mock to claim
native behavior, invokes no external provider, and performs no approved durable
write.

This is browser-shell evidence only. It is not Tauri, migration, provider,
permission/resume, or durable-application credit.

### 3. Native Tauri

Native credit requires all of the following:

- exact source SHA;
- exact executable/app artifact identity;
- profile and data-directory boundary;
- observed Tauri process and desktop route;
- explicit record of every interaction not performed.

The Phase4F report grants current credit only to its exact reviewed artifact:
Today opened fail-closed and Settings rendered/focused with unknown boundary,
no recovery action, and disabled provider test/save. The earlier broad
six-route walk and credential-recovery attempt are historical evidence from a
different artifact.

After restart cleanup merges, `/settings` must be rerun from the exact merged
`main`. Browser-shell and old native results cannot be inherited by that SHA.

### 4. External Live

External-live credit requires a dedicated, authorized live-provider report for
the exact scenario and artifact. Direct generation, web AgentLoop, registered
MCP AgentLoop, and proposal/permission behavior remain separate claims.

The following do not count as external-live evidence:

- local HTTP OpenAI-compatible servers;
- scripted provider output;
- mock IPC;
- fixture-backed web reads;
- command-surface tests;
- native shell launch without an authorized external request;
- passing unit, contract, browser-shell, or Tauri startup checks.

Missing trace, fallback, silent write, synthetic/local provider, malformed
evidence, or an unapproved sensitive action remains a blocker.

## Current Default Local Gates

Choose the smallest gate matching a bounded edit. The final restart baseline
must run the full set required by its cleanup plan.

Frontend:

```sh
corepack pnpm --dir frontend format:check
corepack pnpm --dir frontend typecheck
corepack pnpm --dir frontend test:coverage:checker
corepack pnpm --dir frontend test:selection:checker
corepack pnpm --dir frontend test
corepack pnpm --dir frontend test:historical
corepack pnpm --dir frontend test:coverage
corepack pnpm --dir frontend build
corepack pnpm --dir frontend verify:release-absence
corepack pnpm --dir frontend test:e2e
```

`test:e2e` must collect at least one test and execute only the current Workbench
browser-shell spec by default.

Default `test` and `test:coverage` credit only the frozen current-product
Vitest ID set. The selection checker fails on an empty collection, retired
Stage1/Step6/dev/archive paths, or an unexpected ID/count digest. Historical
Stage1, Step6, dev-harness, and `tauriDev` diagnostic tests remain runnable only
through `test:historical`; their pass result grants no current-product credit.
The default test, watch, and coverage runners force the current scope even when
the caller has a conflicting `OPENLIFE_VITEST_SCOPE` environment value.

Coverage uses Vitest's `json-summary` reporter and one Node checker. Missing,
malformed, zero-line, and below-60-percent artifacts fail with separate
machine-readable diagnostics. CI does not parse coverage with shell pipelines
or `bc`.

Focused Rust:

```sh
cargo fmt --check
cargo test -p openlife-tauri single_system -- --nocapture
cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture
```

Final Rust:

```sh
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --all --locked
```

Repository and fact integrity:

```sh
git diff --check
jq empty plans/openlife_restart_baseline_cleanup.json
test ! -f src-tauri/src/main_chat_final_acceptance_tests.rs
rg -n "run_main_chat_agent_execution_v1_final_acceptance_gate" \
  src-tauri/src/lib.rs src-tauri/src/commands frontend/src/tauri.ts
```

The retired-command scan is expected to have no shipped-surface match and
therefore return exit status 1. Its raw output must still be classified by
surface before any absence claim.

## Historical Entrypoints

`.github/workflows/stage1-tauri-dogfood.yml` and
`.github/workflows/step6-tauri-product-acceptance.yml` are manual
`workflow_dispatch` historical contract runners only. They are not default
push/pull-request gates and do not define current product acceptance.

Archived Stage1/Step6 frontend helpers or Vitest contract fixtures may remain
under explicit test/archive paths. Their presence does not restore a product
route; their pass result does not grant current browser-shell, native-Tauri, or
external-live credit.

Historical Stage3-A, Stage5A/5B, and older command records remain recoverable
through Git and their point-in-time reports. They are not commands to run as
the current default entry.

## Interpretation Rules

- `git diff --check` checks patch whitespace only.
- `cargo fmt --check` checks Rust formatting only.
- typecheck/build success does not prove runtime state coherence.
- a test pass does not independently close any of the 72 restart findings.
- proposal creation is not durable application.
- browser-shell pass is not native-Tauri pass.
- native-Tauri launch is not external-live provider pass.
- an `UNKNOWN` state remains `UNKNOWN` until the required current-SHA evidence
  exists.
- `frontend/test-results` and Phase4F screenshots are retained evidence; do not
  delete them as disposable build output.
- validation must not access external providers, approve a real durable write,
  delete Keychain material, or mutate release/dev/QA product data unless a
  later task explicitly authorizes that exact action.
