# Frontend Validation Report

## Environment

- Node version: `v22.22.3`
- Corepack version: `0.34.6`
- pnpm version: `9.1.0`
- OS / shell notes: macOS Darwin `24.6.0` on arm64, shell `zsh`.

## Package Scripts

Evidence:

- `frontend/package.json`

Relevant scripts:

| Script | Command | Phase 0.5 use |
| --- | --- | --- |
| `dev` | `vite` | Used for bounded E2E smoke feasibility diagnosis. |
| `build` | `tsc && vite build` | Not run; `typecheck` is the narrower frontend health gate requested here. |
| `test` | `vitest run` | Run. |
| `format:check` | `prettier --check "src/**/*.{ts,tsx}"` | Run because it is clearly defined. |
| `typecheck` | `tsc --noEmit` | Run. |
| `test:e2e` | `playwright test` | Broad suite inspected; only the narrow smoke spec was attempted. |
| `test:e2e:tauri` | `node scripts/stage1-tauri-webdriver.mjs` | Not run; script explicitly blocks on macOS. |
| `test:e2e:tauri:step6` | `node scripts/step6-tauri-webdriver.mjs` | Not run; script explicitly blocks on macOS. |

## Dependency Installation

Command:

```sh
corepack enable
corepack pnpm --dir frontend install --frozen-lockfile
```

Result: passed.

Notes:

- `corepack enable` completed without output.
- Frozen install reported `Lockfile is up to date`.
- Installed packages were placed under `frontend/node_modules`.
- `git status --short` remained unchanged after install: only the pre-existing untracked `docs/openlife-phase0-audit/` directory was visible.

Finding: `FRONTEND_HEALTH_VERIFIED` for dependency installation.
Evidence: command output above; `frontend/package.json`; `frontend/node_modules` exists after install.
File location: `frontend/package.json`.
Confidence: High.
Impact: Phase 0's missing-dependency validation gap is closed for local typecheck and Vitest.

## Typecheck

Command:

```sh
corepack pnpm --dir frontend typecheck
```

Result: passed.

Failure summary if failed: not applicable.

Likely cause: not applicable.

Next action: keep this as the narrow required gate for future docs-only frontend validation.

Finding: `FRONTEND_HEALTH_VERIFIED` for TypeScript compile health.
Evidence: `tsc --noEmit` exited `0`.
File location: `frontend/package.json`; `frontend/src/`.
Confidence: High.
Impact: The current frontend source is type-safe in this checkout after installing dependencies.

## Unit Tests

Command:

```sh
corepack pnpm --dir frontend test
```

Result: passed.

Observed output:

- Test files: `39 passed (39)`.
- Tests: `430 passed (430)`.
- Notable warnings: repeated React Router v7 future-flag warnings.
- Expected route guard output: `No routes matched location "/__stage1-dogfood-chat"` from the product-router test that keeps the retired Stage1 route out of product routing.

Failure summary if failed: not applicable.

Likely cause: not applicable.

Next action: treat React Router v7 warnings as future upgrade work, not Phase 0.5 blockers.

Finding: `FRONTEND_HEALTH_VERIFIED` for the current Vitest suite.
Evidence: command output above.
File location: `frontend/src/**/*.test.tsx`, `frontend/src/**/*.test.ts`.
Confidence: High.
Impact: Existing frontend tests pass locally; this does not prove desktop/Tauri product trial readiness.

## Format Check

Command:

```sh
corepack pnpm --dir frontend format:check
```

Result: passed.

Observed output:

- Prettier reported all matched files use Prettier code style.
- Warning: `jsxBracketSameLine is deprecated`.

Finding: `FRONTEND_HEALTH_VERIFIED` for current frontend formatting.
Evidence: command output above.
File location: `frontend/package.json`; `frontend/src/`.
Confidence: High.
Impact: No formatting cleanup is needed before human review.

## E2E / Smoke Feasibility

Scripts found:

- `test:e2e`: browser Playwright suite under `frontend/e2e/`.
- `test:e2e:tauri`: Stage1 Tauri WebDriver runner.
- `test:e2e:tauri:step6`: Step6 Tauri WebDriver runner.
- `test:e2e:tauri:step6:local`: Step6 runner with blocked-live allowance.

Attempted:

```sh
corepack pnpm --dir frontend exec playwright test e2e/smoke.spec.ts --reporter=line
```

Result: failed.

Failure summary:

- Playwright timed out waiting `120000ms` for `config.webServer`.

Bounded diagnosis:

```sh
corepack pnpm --dir frontend dev -- --host 127.0.0.1
curl -I --max-time 5 http://localhost:5173/
curl -I --max-time 5 http://127.0.0.1:5173/
```

Observed:

- The dev command printed `vite "--" "--host" "127.0.0.1"` and served `http://localhost:5173/`.
- `curl http://localhost:5173/` returned `HTTP/1.1 200 OK`.
- `curl http://127.0.0.1:5173/` failed to connect.
- `frontend/playwright.config.ts` waits on `http://127.0.0.1:5173`.

If not attempted, why:

- The broad `test:e2e` suite was not run because it includes retired/blocked Stage1 and Step6 product-trial evidence specs, not only the smoke spec.
- Tauri WebDriver scripts were not run because `frontend/scripts/stage1-tauri-webdriver.mjs` and `frontend/scripts/step6-tauri-webdriver.mjs` return a macOS blocker (`tauri_webdriver_macos_not_supported_by_tauri_driver`) in preflight.
- No full desktop product trial was forced in Phase 0.5.

Finding: `FRONTEND_HEALTH_PARTIAL` for E2E smoke.
Evidence: Playwright timeout, bounded dev-server check, `frontend/playwright.config.ts`, `frontend/e2e/smoke.spec.ts`, Tauri WebDriver scripts.
File location: `frontend/playwright.config.ts`; `frontend/e2e/`; `frontend/scripts/stage1-tauri-webdriver.mjs`; `frontend/scripts/step6-tauri-webdriver.mjs`.
Confidence: High.
Impact: Browser smoke validation is blocked by web-server host/config behavior, while TypeScript and Vitest are green. This is not evidence of desktop product readiness.

## Git Working Tree Impact

Before validation, `git status --short` showed:

```text
?? docs/openlife-phase0-audit/
```

After dependency install, `git status --short` still showed only:

```text
?? docs/openlife-phase0-audit/
```

Validation-created filesystem state:

- `frontend/node_modules` exists after install.
- No tracked package, lockfile, or source file changes were visible from dependency installation or validation commands.

Report-writing impact:

- This Phase 0.5 pass intentionally creates `docs/phase0_5/`.

Finding: Dependency installation did not modify tracked files.
Evidence: `git status --short` before and after install.
File location: repository root; `frontend/node_modules`.
Confidence: High.
Impact: Phase 0.5 reports can be reviewed separately from production code.

## Verdict

Classification: `FRONTEND_HEALTH_PARTIAL`.

Reason:

- Dependency installation, typecheck, unit tests, and format check passed.
- Browser smoke E2E was attempted and failed at Playwright web-server readiness, with a likely host mismatch between Vite serving `localhost` and Playwright waiting on `127.0.0.1`.
- Tauri/desktop product trial remains unverified and should remain outside Phase 0.5.

No production frontend source was modified.
