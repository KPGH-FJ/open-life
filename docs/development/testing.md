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

## Historical report behavior matrix

Run the controlled S6 report matrix with:

```sh
scripts/s6-report-matrix.zsh
```

This historical script proves bounded report contracts that remain useful as
migration evidence. It does not prove the reconstructed general Agent, a native
product path, or an external-live path. It is not an R4 acceptance gate; R4
uses canonical Work generation, approval, materialization, restart recovery,
verification, and Undo product tests.

The required external-live report case is gated separately:

```sh
scripts/live-eval.zsh cargo test -p openlife-tauri --locked \
  roadshow_cc01_external_live_resource_web_report_waits_for_review_then_materializes_once \
  -- --ignored --nocapture
```

It uses the configured provider and real Web access. Never paste provider
payloads, credentials, resource bodies, or generated report content into plans
or test summaries. A failed or unavailable live adapter remains blocked.

Native review must use an exact current Tauri bundle and a purpose-specific
data profile. If that profile requires macOS Keychain recovery, stop for
explicit user confirmation rather than silently substituting fixture
credentials or rotating an existing credential.

## macOS exact-native identity

R0 and later native evidence must use an explicit local signing identity rather
than a linker-generated ad-hoc executable identity:

```sh
OPENLIFE_CODESIGN_IDENTITY="OpenLife Local Code Signing" \
  scripts/macos-exact-native.zsh
```

The verifier requires the signed bundle identity `ai.openlife.desktop`; the
legacy `ai.openlife.app` name remains only as the explicit pre-reconstruction
data-directory migration source and is not a valid macOS bundle identity.

The script builds the exact source and verifies the configured bundle
identifier, the selected signing authority, and the strict deep resource seal.
It never reads Keychain values. A release build uses the fixed product Keychain
service, so R0 first-run evidence used the fresh reconstruction profile after an
explicit, bounded reset of OpenLife-owned internal keys. Provider and search
credentials were not touched. Development-only isolated Keychain services may
be used for diagnostics, but cannot be presented as release-path evidence.

For local self-signed development, Keychain restart credit is bound to the
exact built application. Rebuilding may cause macOS to require explicit ACL
recovery even when the certificate subject is unchanged; cross-build access is
not credited until a stable Developer ID/distribution identity is available.
R0's measured exact-binary baseline on 2026-08-13 was 216.7 ms from launch to
all protected execution stores open, 70,592 KiB RSS at that boundary, and zero
observed network sockets. These are comparison measurements, not a release SLA.
