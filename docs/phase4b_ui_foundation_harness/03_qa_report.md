# Phase 4B UI Foundation QA Report

Status: `TECHNICAL_EXIT_PASS`
Date: 2026-07-19

## 1. Automated Browser QA

Command:

```sh
corepack pnpm --dir frontend qa:phase4b
```

Result: `PASS`, 90 assertions, zero browser console warnings/errors.

The command was also verified from a clean shell with port 4184 initially
closed. It started the harness server, waited for readiness, completed QA, and
left no listener behind.

| Viewport       | Overflow        | Body | Metadata | Top / bottom screenshot                                |
| -------------- | --------------- | ---- | -------- | ------------------------------------------------------ |
| 1440x900       | 0px             | 14px | 12px     | `phase4b_1440x900.png` / `phase4b_1440x900_bottom.png` |
| 1280x800       | 0px             | 14px | 12px     | `phase4b_1280x800.png` / `phase4b_1280x800_bottom.png` |
| 390x844        | 0px             | 14px | 12px     | `phase4b_390x844.png` / `phase4b_390x844_bottom.png`   |
| 390x844 dialog | bottom anchored | 14px | 12px     | `phase4b_390x844_dialog.png`                           |

The machine-readable report is
`artifacts/phase4b-browser-qa.json`.

## 2. Interaction And Accessibility Checks

Passed:

- keyboard activation opens approval confirmation;
- reset, approval, confirmation, evidence, unavailable navigation, and toggle
  paths expose visible focus and keyboard activation;
- dialog focus enters the heading, Escape closes, and focus returns to trigger;
- busy-state and inline-callback rerenders preserve the active dialog control;
- dialog background is inert and hidden from the accessibility tree;
- current navigation exposes `aria-current=page`;
- mobile interaction targets are at least 44px high;
- unavailable navigation and evidence controls produce visible feedback;
- disabled apply exposes a visible reason;
- disabled text fields require and link a visible reason;
- dynamic feedback has exactly one polite live region;
- approval remains `approved, not applied` and denies durable-write completion;
- unknown state does not use verified-success presentation;
- `/`, `/index.html`, and `/phase4b/` return 404 and cannot load `src/main.tsx`;
- all three viewports have no horizontal overflow.

## 3. Contrast

| Pair                 | Ratio   | Result |
| -------------------- | ------- | ------ |
| ink on canvas        | 18.88:1 | PASS   |
| secondary on sidebar | 7.51:1  | PASS   |
| muted on canvas      | 5.74:1  | PASS   |
| amber on amber-soft  | 5.90:1  | PASS   |
| red on red-soft      | 6.35:1  | PASS   |
| green on green-soft  | 4.83:1  | PASS   |

All measured text pairs meet or exceed 4.5:1.

| Non-text pair                    | Ratio  | Result |
| -------------------------------- | ------ | ------ |
| control boundary on canvas       | 3.69:1 | PASS   |
| control boundary on sunken hover | 3.30:1 | PASS   |
| focus on canvas                  | 5.17:1 | PASS   |
| focus on amber soft              | 4.97:1 | PASS   |
| focus on red soft                | 4.89:1 | PASS   |
| focus on green soft              | 4.95:1 | PASS   |

All measured control-boundary and focus pairs meet or exceed 3:1.

## 4. Manual Screenshot Review

The seven browser artifacts were inspected at original resolution. Top and
bottom captures cover the internally scrolling work surface. No text
overlap, incoherent clipping, horizontal overflow, nested-card composition, or
mobile dialog obstruction was observed. Desktop uses one continuous work
surface; mobile converts the QA state list to a compact horizontal navigation
row.

## 5. Validation Results

```text
git diff --check: PASS
cargo fmt --check: PASS
cargo clippy --all --locked -- -D warnings: PASS
cargo test -p openlife-core --lib: PASS, 1,486 passed / 3 ignored
cargo test -p openlife-tauri single_system: PASS, 42 passed
cargo test -p openlife-tauri main_chat_runtime_module: PASS, 30 passed
frontend typecheck: PASS
frontend format:check: PASS
frontend test: PASS, 48 files / 537 tests
frontend coverage: PASS, 48 files / 537 tests
coverage overall: 78.06% lines/statements, 77.47% branches, 70.10% functions
foundation coverage: 95.66% lines, 84.94% branches, 100% functions
normal production build and absence scan: PASS, 1,673 modules
dev-only Phase 4B build: PASS, 1,605 modules
Phase 4B browser QA: PASS, 90 assertions
dev-only Tauri build rejection guard: PASS from repo root and src-tauri
real Tauri dev startup with isolated data: PASS
```

The Tauri run used a temporary `OPENLIFE_DATA_DIR`, disabled live-provider
credit and A2A autostart, bound Vite to `127.0.0.1:4184`, compiled and launched
`target/debug/openlife-tauri`. The canonical harness URL returned 200 while the
three rejected HTML paths returned 404. All processes and temporary data were
then removed.

The Tauri result proves desktop-shell process startup and entry routing, not a
separate pixel capture. The Playwright captures of the same dev URL remain the
visual evidence. No backend journey or external-provider credit is claimed.

Reproducible coverage, dist, and Rust incremental caches were removed after
validation. This did not change source, dependencies, screenshots, or product
data.

Earlier independent-review and reclaimed-byte observations had no durable
artifact and are therefore not acceptance evidence. The machine-readable QA,
tracked screenshots, command results in this report, and remote CI are the
gating evidence.

## 6. Residual QA Boundary

The full local gate set passed. Remote CI remains required after the Phase 4B
review PR is opened. Human review remains required before merge or Phase 4C.
