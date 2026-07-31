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
