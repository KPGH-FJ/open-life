# Phase 4B Dev Harness And Release Absence Contract

Status: `TECHNICAL_EXIT_PASS`
Date: 2026-07-19

## 1. Dev-Only Topology

```text
frontend/dev/phase4b/index.html
  -> frontend/src/dev/phase4b/main.tsx
  -> FoundationHarness
  -> frontend/src/ui/foundation

frontend/vite.phase4b.config.ts
  -> __OPENLIFE_PHASE4B_HARNESS__ = true
  -> dist-phase4b

frontend/vite.config.ts
  -> __OPENLIFE_PHASE4B_HARNESS__ = false
  -> normal production dist
```

Local browser entry:

```sh
corepack pnpm --dir frontend dev:phase4b --host 127.0.0.1
```

Open `http://127.0.0.1:4184/dev/phase4b/`.

`qa:phase4b` is self-contained: it reuses an already reachable harness server,
or starts Vite on the configured local URL, waits for readiness, runs the
browser checks, and terminates only the process it started.

## 2. Harness Semantics

The top QA bar is outside any proposed product shell and labels the surface
`DEV ONLY`, `LAYOUT_FIXTURE`, and `not connected to product backend`.

| Harness value | Source classification |
| --- | --- |
| approval state and feedback | `LAYOUT_FIXTURE`; local React state only |
| provider URL and model validation | `LAYOUT_FIXTURE`; no backend call |
| network-policy toggle | `LAYOUT_FIXTURE`; not authorization |
| unknown privacy boundary | semantic fail-closed state sample |
| navigation/evidence rows | component interaction sample only |
| Evidence id/label/source/sensitivity | layout examples, not real `EvidenceRef` |
| technical entry/fixture/backend fields | dev diagnostics for harness isolation |

No fixture metric, decision, or provider state is product truth. The harness
does not import `tauri.ts`, dispatch Review actions, grant permission, or write
LifeModel/Memory state.

## 3. No Fake Controls

- Approval opens a confirmation dialog and produces approved-not-applied
  feedback.
- View, later, reject, reset, navigation, evidence, and toggle controls produce
  visible local feedback.
- Unsupported apply is disabled with a visible reason.
- Loading is a labeled component-state sample and cannot be clicked.
- Unavailable navigation says the page is not migrated; it does not redirect.

## 4. Tauri Dev Overlay

`src-tauri/tauri.phase4b.conf.json` points Tauri dev mode at the independent
harness and uses a distinct development identifier. Bundling is disabled. Its
`beforeBuildCommand` always rejects release/package use through
`frontend/scripts/reject-phase4b-tauri-build.mjs`.

This overlay is for real desktop-shell layout and keyboard dogfood only. It
does not create a shipped command, route, or second product application.

## 5. Release Absence Guards

The normal frontend build runs
`frontend/scripts/verify-production-absence.mjs`, which proves:

- `TodayV2PreviewPage.tsx` remains absent;
- `/today-v2-preview` is absent from `App.tsx` and the release bundle;
- harness imports and fixture markers are absent from `App.tsx` and
  `ProductShell.tsx`;
- harness/fixture markers are absent from normal `dist` output.

Rust single-system authority tests independently inspect source/configuration
boundaries. `frontend/src/App.test.tsx` also scans production source while
explicitly excluding the dev-only directory from production ownership.

The guard proves release absence. It does not prove that Shell V2 or any V2
business journey exists.
