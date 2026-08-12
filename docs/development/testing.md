# Testing OpenLife

Use the smallest check that matches the change, then broaden before merging.

## Fast Checks

```sh
git diff --check
cargo fmt --check
corepack pnpm --dir frontend format:check
corepack pnpm --dir frontend typecheck
```

## Product Tests

```sh
cargo test -p openlife-core --locked
cargo test -p openlife-tauri --locked
corepack pnpm --dir frontend test
corepack pnpm --dir frontend build
corepack pnpm --dir frontend test:e2e
```

## Full Gate

```sh
cargo clippy --all --locked -- -D warnings
cargo test --all --locked
```

## Evidence Levels

- unit/contract tests prove only their code contract;
- browser-shell tests prove React routing and rendering, not native Tauri;
- native Tauri tests prove the exact local build and trial path;
- scripted or local HTTP providers are not external-live providers;
- external-live behavior requires an explicitly authorized live-provider run.

Tests must fail closed. A blocked prerequisite must not return success, and a
passing test must not be used as evidence for a broader layer.

Tests use synthetic resources under `test-fixtures/`. They must not read real
application data, Keychain contents, or private user files.

## Report behavior matrix

Run the controlled S6 report matrix with:

```sh
scripts/s6-report-matrix.zsh
```

This script proves deterministic Task/Run/Item/Artifact, tool, Review,
recovery, concurrency, and projection contracts. It deliberately does not
claim native or external-live evidence.

The required external-live report case is gated separately:

```sh
scripts/live-eval.zsh cargo test -p openlife-tauri --locked \
  roadshow_cc01_external_live_resource_web_report_waits_for_review_then_materializes_once \
  -- --ignored --nocapture
```

It uses the configured provider and real Web access. Never paste provider
payloads, credentials, resource bodies, or generated report content into plans
or test summaries. A failed or unavailable live adapter remains blocked.

Native review must use an exact current Tauri bundle and an isolated data
profile. If that profile requires macOS Keychain initialization or recovery,
stop for explicit user confirmation rather than touching the default profile or
silently substituting fixture credentials.
