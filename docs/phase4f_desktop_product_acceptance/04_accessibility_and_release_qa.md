# Phase 4F Accessibility And Release QA

Status: `PASS_WITH_MANUAL_VOICEOVER_BLOCKED`
Date: 2026-07-21

## Native Keyboard And Focus

| Check | Result |
| --- | --- |
| Route navigation moves focus to the new `h1` | `PASS` |
| Current navigation exposes `aria-current="page"` | `PASS` by DOM regression tests |
| Inspector open focuses its heading | `PASS` native |
| Inspector close restores trigger focus | `PASS` native |
| Recovery dialog opens on its heading | `PASS` native |
| Dialog Tab order reaches close and footer controls | `PASS` native |
| Escape cancels without dispatch and restores keyboard opener | `PASS` native |
| Dynamic route and cancellation messages update the live region | `PASS` native |

The macOS click path does not always move keyboard focus to a button. Focus
restoration was therefore rechecked through a keyboard-opened dialog, where it
returned to `检查系统凭据` as required.

## Accessibility Tree

The native WebKit accessibility tree exposed labelled navigation, headings,
buttons, search field, switches, secure credential input, dialog, expanded and
collapsed Inspector state, and disabled controls with adjacent reasons. The
credential value was not exposed because none was entered.

## VoiceOver Boundary

System Settings could momentarily toggle VoiceOver on, but the automation
session could not keep it enabled after returning to OpenLife. No reliable
spoken transcript or VoiceOver cursor traversal was captured. The AX tree and
keyboard results above are valid evidence, but they are not substituted for a
manual VoiceOver pass.

`MANUAL_VOICEOVER_RUN=BLOCKED_BY_AUTOMATION_ENVIRONMENT`

## Contrast

Computed semantic-token pairs:

| Pair | Ratio | Requirement | Result |
| --- | ---: | ---: | --- |
| ink / canvas | 18.88:1 | 4.5:1 | pass |
| secondary / canvas | 8.19:1 | 4.5:1 | pass |
| muted / canvas | 5.74:1 | 4.5:1 | pass |
| muted / sidebar | 5.27:1 | 4.5:1 | pass |
| amber / amber-soft | 5.90:1 | 4.5:1 | pass |
| red / red-soft | 6.35:1 | 4.5:1 | pass |
| green / green-soft | 4.83:1 | 4.5:1 | pass |
| focus / canvas | 5.17:1 | 3:1 | pass |
| control boundary / canvas | 3.69:1 | 3:1 | pass |

## Desktop Layout

The real packaged window was captured at `1228x768`. Sidebar, context bar,
work surface, Inspector, Settings navigation, recovery dialog, error notices,
and disabled-action reasons remained readable without overlap or horizontal
overflow. Mobile is intentionally outside this phase and receives no credit.

## Release Gates

The production build passed its absence guard and excluded old shells/pages,
compatibility redirects, and Phase 4 development harnesses. The Tauri bundle
completed with the existing warning that bundle identifier `ai.openlife.app`
ends in `.app`; this is recorded as packaging cleanup, not a frontend truth or
runtime blocker.

| Gate | Result |
| --- | --- |
| `corepack pnpm --dir frontend format:check` | `PASS` |
| `corepack pnpm --dir frontend typecheck` | `PASS` |
| `corepack pnpm --dir frontend test` | `PASS`, 37 files / 273 tests |
| `corepack pnpm --dir frontend build` | `PASS` |
| `corepack pnpm --dir frontend verify:release-absence` | `PASS` |
| `cargo fmt --check` | `PASS` |
| `cargo test -p openlife-tauri single_system -- --nocapture` | `PASS`, 44 tests |
| `cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture` | `PASS`, 30 tests |
| `git diff --check` | `PASS` |

The first single-system run correctly detected that the new bounded Settings
read of `LifeStateProjection.safeMode` was missing from the Phase 1 source
inventory. The inventory now records that direct backend-owned read; the scan
was not weakened and the full authority gate passed on rerun.

CodeRabbit CLI was unavailable in the local environment. A local changed-file
review was completed instead. It found and repaired the missing Product Action
Contract for credential recovery, including mutual exclusion with settings
test/save operations; the final full gates above include that repair.
