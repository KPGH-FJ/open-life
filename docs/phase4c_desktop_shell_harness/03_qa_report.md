# Phase 4C QA Report

Status: `PASS_FOR_HUMAN_REVIEW`
Date: 2026-07-20

## Automated Browser QA

Command:

```sh
corepack pnpm -C frontend qa:phase4c
```

Result: `PASS`, 110 assertions, zero browser errors/warnings.

Verified:

- canonical harness path and rejected alternate paths;
- `1440x900` and `1280x800` with zero horizontal overflow;
- sidebar `232px`, context bar `56px`, Inspector `344px`;
- no sidebar/main/Inspector overlap;
- body `14px`, metadata `12px`;
- skip-link reachability and focus transfer to the main work surface;
- real forward-Tab reachability, keyboard-visible focus rings, and
  navigation-to-heading focus without programmatic QA focus injection;
- navigation while Inspector is open closes the old Inspector and keeps focus
  on the new page heading;
- explicit unavailable Tasks page;
- Settings context, search, Back, and focus restoration;
- pending review view does not approve;
- approve confirmation produces approved-not-applied only;
- approval clears evidence selected from the previous pending state;
- Inspector heading focus, structured evidence, close, and focus restoration;
- Safe Mode fail-closed with no verified-success green;
- complete fixture Action Contract attributes;
- exactly one polite live region;
- no mobile navigation/drawer/sheet controls;
- WCAG AA text and 3:1 non-text contrast pairs.

Machine-readable evidence:

- `artifacts/phase4c-browser-qa.json`.

## Screenshots

- `artifacts/phase4c_1440x900_today.png`;
- `artifacts/phase4c_1440x900_today_inspector.png`;
- `artifacts/phase4c_1280x800_today.png`;
- `artifacts/phase4c_1280x800_today_inspector.png`;
- `artifacts/phase4c_1440x900_settings.png`;
- `artifacts/phase4c_1440x900_review_approved.png`;
- `artifacts/phase4c_1440x900_safe_mode.png`.

## Visual Review

- 1280 Inspector-open layout remains readable and does not collapse into a
  dense dashboard;
- the work surface uses one primary conclusion and linear sections;
- Settings reads as a separate utility context rather than a sixth dashboard;
- white/gray surfaces, black text, restrained lines, low radius, and Lucide
  icons remain consistent with the approved Codex/Cursor direction;
- QA and engineering feedback is outside the product shell;
- raw DTO names remain in Inspector source/technical detail, not primary copy.

## Real Tauri Desktop Startup

The dev-only overlay was started with Tauri CLI `2.11.0`, an isolated
`OPENLIFE_DATA_DIR`, and A2A autostart disabled. Both supported invocation
shapes were exercised:

- repository root: `frontend/node_modules/.bin/tauri dev --config src-tauri/tauri.phase4c.conf.json`;
- `src-tauri/`: `../frontend/node_modules/.bin/tauri dev --config tauri.phase4c.conf.json`.

Result: `PASS`.

- Vite bound to `127.0.0.1:4185`;
- the structured hook resolved its explicit working directory to
  `frontend/` from both invocation shapes;
- Rust dev build completed and launched `target/debug/openlife-tauri`;
- canonical `/dev/phase4c/` returned `200`;
- `/`, `/index.html`, and `/phase4c/` returned `404`;
- the Tauri process and listener stopped cleanly;
- the temporary product-data directory was removed.

The Phase 4C package command was also invoked and rejected by the explicit
development-only `beforeBuildCommand`, as required.

## Independent Review

CodeRabbit CLI was not installed, so no CodeRabbit result is claimed. A
read-only native Codex review using `gpt-5.5` independently reran the relevant
gates and found three actionable issues:

1. invocation-dependent Tauri hook cwd;
2. navigation focus overwritten by Inspector close focus restoration;
3. stale selected evidence surviving a review-state transition.

All three were reproduced, fixed, and covered by tests. The local review also
identified that `vite.phase4c.config.ts` was missing from `tsconfig.node.json`;
the config is now included in TypeScript checking and guarded by a contract
test.

A second independent pass found three additional guard-quality/accessibility
issues:

1. the skip-link target was not programmatically focusable;
2. browser QA forced focus after pressing Tab and could falsely pass an
   unreachable control;
3. the release scan relied on a component export name that minification may
   rewrite.

The main target now has `tabIndex=-1`; QA reaches every tested control only by
forward Tab traversal; and the release scan includes the stable
`ol-workbench-shell` marker. Target tests, browser QA, and the production build
all pass after these repairs.

A third read-only Codex review examined the repaired staged, unstaged, and
untracked patch and reported no remaining discrete correctness issue. This is
an independent local review result, not a CodeRabbit result.

This proves desktop WebView startup and entry isolation. It does not prove a
backend business journey or external-provider behavior.

## Repository Gates

| Command | Result |
| --- | --- |
| `git diff --check` | PASS |
| `cargo fmt --check` | PASS |
| `cargo clippy --all --locked -- -D warnings` | PASS |
| `cargo test -p openlife-core --lib --quiet` | PASS: 1486 passed, 3 ignored |
| `cargo test -p openlife-tauri single_system -- --nocapture` | PASS: 43 passed |
| `cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture` | PASS: 30 passed |
| `corepack pnpm -C frontend typecheck` | PASS |
| `corepack pnpm -C frontend format:check` | PASS |
| scoped Prettier check for scripts/configs | PASS |
| `corepack pnpm -C frontend test` | PASS: 50 files, 555 tests |
| `corepack pnpm -C frontend test:coverage` | PASS: 78.23% statements/lines; Shell 98.29% |
| `corepack pnpm -C frontend build` | PASS: 1673 modules; release absence PASS |
| `corepack pnpm -C frontend build:phase4c` | PASS: 1609 modules |
| `corepack pnpm -C frontend qa:phase4c` | PASS: 110 assertions |
| Phase 4C Tauri package command | PASS: rejected by the required dev-only guard |

The frontend suite emits existing React Router v7 future warnings and expected
stderr from tests that exercise failure paths. They do not represent test
failures or browser-console errors in the Phase 4C harness.

## Contrast Results

| Pair | Ratio |
| --- | ---: |
| ink on canvas | 18.88:1 |
| secondary on sidebar | 7.51:1 |
| muted on canvas | 5.74:1 |
| amber on amber soft | 5.90:1 |
| red on red soft | 6.35:1 |
| green on green soft | 4.83:1 |
| control boundary | 3.69:1 |
| focus on canvas | 5.17:1 |
| focus on amber | 4.97:1 |

## Manual Limits

- no mobile QA was run because mobile is outside current product scope;
- automated accessibility checks do not replace a later VoiceOver pass in the
  production Tauri WebView;
- fixtures do not prove backend commands, refresh ordering, or real page data;
- real journey dogfood begins only after Phase 4D connects read models/actions.
