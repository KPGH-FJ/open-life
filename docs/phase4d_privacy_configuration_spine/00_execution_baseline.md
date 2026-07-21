# Phase 4D Privacy And Configuration Execution Baseline

Status: `IMPLEMENTED`
Date: 2026-07-21

## Verified Starting Point

- Durable-truth PR `#61` is merged at
  `8c3de9fe4d3391d3c6ba0d806dcfe0fc73e65270`.
- Protected-main CI run `29799299686` completed successfully.
- Local typecheck, format check, production build, and production absence guard
  passed again on that merged main.
- This slice starts from that exact main state on
  `codex/phase4d-privacy-configuration-spine`.
- The candidate UI remains development-only at `/dev/phase4d/`.

## Slice Goal

Add the final Phase 4D desktop journey to the candidate Shell:

```text
sanitized AppConfig + ProviderPrivacyBoundarySummary
  -> edit a local draft
  -> test the exact provider target without saving
  -> resolve an exact ReviewItem when consent is required
  -> save explicitly
  -> re-read config and provider/privacy boundary
  -> known | unknown | failed
```

The journey must keep configuration preference, provider test evidence,
permission decision, config persistence, and current provider/privacy truth as
separate facts.

## Allowed Changes

- `frontend/src/ui/journeys/settingsPrivacy/**`;
- the existing dev-only Phase 4D composition, fixtures, and desktop QA;
- the Phase 4A settings orchestration reducer and its tests;
- Settings search inside the candidate desktop Shell;
- the shared candidate Review view where rich permission fields are rendered;
- release absence guards, the existing Rust source guard, documentation, and
  evidence artifacts.

## Forbidden Changes

- production `App.tsx`, `ProductShell.tsx`, route authority, or old Settings
  page owners;
- Rust/Tauri command behavior, provider authority, network policy, credential
  storage, or durable-write rules;
- inferring a local route from `preferLocalModel` or page-local config;
- treating a successful connection test as a save;
- treating a save callback as proof of the refreshed provider boundary;
- guessing a ReviewItem from provider, title, or route;
- displaying, searching, logging, or placing credentials in Inspector;
- mobile implementation or mobile acceptance criteria.

## Entry Gate

`PRIVACY_CONFIGURATION_IMPLEMENTATION_ALLOWED=YES` because the prior slice is
merged, main CI is green, main was reverified, and the user explicitly continued
the approved route.

`PRODUCTION_AUTHORITY_SWITCH_ALLOWED=NO` until Phase 4E.
