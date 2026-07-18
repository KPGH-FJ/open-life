# Quality and Test Audit

## Test Inventory

Finding: The repo has broad Rust and frontend test assets.

Evidence:

- Source scan found 84 test/spec files across `openlife-core/src`,
  `src-tauri/src`, `frontend/src`, and `frontend/e2e`.
- Rust tests include core agent tests, runtime-module tests, command-surface
  tests, ReAct boundary/unit tests, runtime facts tests, and single-system
  guards.
- Frontend tests cover pages, components, utilities, Tauri bridge behavior, and
  E2E specs.

File location:

- `openlife-core/src/agent/tests/`
- `src-tauri/src/*_tests.rs`
- `frontend/src/**/*.test.tsx`
- `frontend/e2e/*.spec.ts`

Confidence: High.

Impact: Test coverage is substantial, but test existence is not the same as
product readiness.

## Commands Verified In This Audit

Finding: The two Rust guards most relevant to Phase7 authority and runtime
ownership passed in the current checkout.

Evidence:

- `cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture`
  passed 26 tests.
- `cargo test -p openlife-tauri single_system -- --nocapture` passed 17 tests.
- `git diff --check` passed before and after report creation.

File location:

- `src-tauri/src/main_chat_runtime_module_tests.rs`
- `src-tauri/src/single_system_authority_tests.rs`

Confidence: High.

Impact: Current source supports the Phase7 guard claims used in this audit.

## Frontend Verification Gap

Finding: Frontend typecheck was not verified in this audit.

Evidence:

- `corepack pnpm --dir frontend typecheck` failed with `tsc: command not found`
  and pnpm warning that `node_modules` is missing.

File location:

- `frontend/package.json`

Confidence: High.

Impact: Frontend compile health is `UNKNOWN` until dependencies are installed
and the typecheck/Vitest gates are rerun.

## E2E and Product Trial Gap

Finding: E2E specs and prior trial artifacts exist, but this audit did not run
a live desktop product trial.

Evidence:

- `frontend/e2e` includes smoke, stage1 dogfood, and step6 product acceptance
  specs.
- Phase7 deletion manifest keeps Computer Use trial status as
  `red-until-trial-green`.

File location:

- `frontend/e2e/`
- `plans/openlife_single_system_deletion_manifest.md`

Confidence: High.

Impact: The codebase has test scaffolding, but the product cannot be called
trial-green from this audit.

## Quality Verdict

The backend guard posture is strong. The frontend verification posture is
currently blocked by missing dependencies in this checkout. Product readiness
requires a separate real desktop trial with isolated data and evidence.
