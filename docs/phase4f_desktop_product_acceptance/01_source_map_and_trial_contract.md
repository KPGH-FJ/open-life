# Phase 4F Source Map And Trial Contract

Status: `ACTIVE`
Date: 2026-07-21

## Production Frontend Chain

```text
frontend/src/main.tsx
  -> frontend/src/App.tsx
  -> frontend/src/ui/journeys/readOnly/ReadOnlySpineJourney.tsx
  -> frontend/src/ui/shell/OpenLifeWorkbenchShell.tsx
  -> frontend/src/ui/journeys/**
  -> frontend/src/tauri.ts
  -> shipped Tauri command handlers
  -> backend read models / governed runtime owners
```

Canonical desktop routes are `/today`, `/workspace`, `/tasks`, `/review`,
`/life-model`, and `/settings`. Root redirects only to Today. Retired routes
remain explicitly unavailable; they are not trial fallbacks.

## Settings And Safe Mode Field Sources

| Product fact or action | Exact owner | Frontend use | Fail-closed rule |
| --- | --- | --- | --- |
| Safe Mode active/reason/source refs | `get_life_state_projection` -> `LifeStateProjection.safeMode` | `SettingsPrivacySnapshot.safeMode` | Missing projection does not claim normal operation; generic Safe Mode does not authorize credential recovery |
| Provider/transmission boundary | `get_provider_privacy_boundary_summary` | independent envelope in Settings | Unknown/error never becomes local, private, or safe |
| Editable settings | sanitized `get_config` | local draft only | Missing config never becomes a default Provider or inferred privacy conclusion |
| Credential recovery eligibility | no typed backend product field exists at this SHA | no shipped Settings action | Generic `safeMode.active`, free-text reason, and generic startup-warning refs are insufficient authorization |
| Credential recovery command | shipped `recover_required_credential_access` | typed bridge remains unused by the current Settings journey | A future reviewed slice must expose exact backend eligibility before the product can call it |

The backend command still checks four integrity credential purposes, but the
current product journey does not call it. A rejected Phase 4F attempt exposed
the command for every Safe Mode cause; independent review found that vector
corruption, degraded database state, and unrelated startup warnings can also
activate Safe Mode. The action therefore stays unavailable until the backend
owns a typed eligibility/cause contract. The frontend must not parse the
free-text reason to recreate that authority.

`AppConfig.llm.openai_key` is a write-only submission field at this boundary:
Rust marks it `skip_serializing`, so `get_config` omits it. The frontend may use
the non-secret optional `openai_key_ref` only as stored-credential presence
metadata; it must clear that metadata in a draft when the provider identity
changes and must never display the reference as credential material.

## Journey Credit Rules

| Journey | Required credit | Forbidden substitute |
| --- | --- | --- |
| Routes and shell | packaged production Tauri UI | Vite/browser-only screenshot |
| Permission and resume | exact pending ReviewItem, governed decision, refreshed task, explicit resume | fixture, guessed review target, command callback as completion |
| Durable truth | exact proposal and refreshed backend materialization state | approved treated as applied, local page inference |
| Settings | real command result plus refreshed boundary | draft, test receipt, or save callback alone |
| Recovery | typed backend eligibility, app confirmation, native confirmation, metadata report, restart, refreshed Safe Mode | generic Safe Mode or a report alone treated as authorization/recovery |
| Accessibility | actual keyboard/focus and VoiceOver observations | component semantics alone |

External live-provider evidence is optional for bounded frontend repair but
remains required for any broader backend/live readiness claim. Its absence is
`UNKNOWN`, not a pass.

## Security Boundary During Trial

- use a fresh `OPENLIFE_PROFILE=qa` and isolated `OPENLIFE_DATA_DIR`;
- recognize that `OPENLIFE_DATA_DIR` does not namespace the macOS Keychain
  service `com.openlife.desktop`; an existing entry can retain ACL state from a
  different binary identity;
- do not enter or expose real provider credentials;
- do not contact an external provider without action-time user confirmation;
- do not execute credential recovery while the current product read model lacks
  typed recovery eligibility;
- remove isolated runtime data after evidence capture, but retain scrubbed
  reports and screenshots under this phase directory.
