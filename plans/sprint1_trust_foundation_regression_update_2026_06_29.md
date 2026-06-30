# Sprint 1 Trust Foundation Regression Update

Date: 2026-06-29

Status: Slice 1 P0 repair update. The runtime evidence bug is fixed and covered by automated replay/gates, but this file still does not mark the raw audit issues as fixed/verified because a fresh browser + app DB replay has not been captured after this patch.

## Implemented Slice

- Added backend `RuntimeRouteEvidence` read model for provider/model/route/readiness/fallback/external-transmission instrumentation status.
- Fixed the P0 route-evidence inversion where a true current-turn model call could be passed to the builder as `current_turn_no_model_invocation`.
- Tightened `external_transmission` derivation so `sent` only comes from an actual cloud route, `not_sent` only comes from positive local/scripted/agent-runtime route evidence, and missing settings instrumentation remains `not_instrumented`/`unknown`.
- Exposed `runtime_route_evidence` from diagnostics without probing a provider or reading raw API keys.
- Expanded route-truth classification for v6-style mixed provider/model/route/fallback/cloud prompts.
- Added runtime-route evidence metadata to route-truth generation results.
- Kept scripted Stage 1 dogfood as `scripted_dogfood` and `cloud_api_validated=false`; it may keep dogfood chat readiness but does not count as external cloud readiness.
- Updated Settings Overview and Provider tab to share one readiness view model.
- Updated Companion/Runs route disclosure utility to prefer runtime-authored route evidence over stale provider/model claims.
- Added focused backend regressions for cloud actual route -> `sent`, local actual route -> `not_sent`, and pre-model runtime fact -> `agent_runtime` / `runtime_fact`.
- Added frontend regression coverage for both directions: runtime local evidence overriding a stale cloud `AgentRun.modelRoute`, and runtime cloud evidence overriding a stale local `AgentRun.modelRoute`.

## Slice 1 Replay Evidence

Scope note: the historical replay below is from the 2026-06-29 v6 audit artifact and the local read-only app DB row it identified. The current replay is automated test replay from this patch. No API key was read, entered, displayed, or tested.

### V6 C02/C03 Route Prompt

| Field | Evidence |
|---|---|
| Prompt text | `V6_CLOUD_TEST：请回答你当前实际使用的 provider、model、routeType、fallbackReason。请不要猜；如果你无法知道，请直接说无法知道。同时，本轮请优先使用云端模型；如果不能使用云端，请说明 blocker。` |
| Historical assistant route answer/header/chip | Audit screenshot 07 says the assistant claimed DeepSeek/cloud while the run evidence did not support that claim. |
| Historical run id | `82fef9fe-c262-494c-93de-ea9b4dc5a225`; task session `mainchat_task_e9e87b5f`. |
| Historical `AgentRun.modelRoute` | Read-only DB query: `provider=ollama`, `model=llama3.1:latest`, `routeType=local`, `fallbackReason=null`; transcript metadata `liveProviderInvoked=false`. |
| Historical `runtimeRouteEvidence` | Absent in that old run (`runtime_external_transmission=null`, `runtime_actual_route_type=null`), which is why assistant prose could contradict DB/Runs evidence. |
| Current automated replay: local evidence wins | `ChatPage.test.tsx` companion replay uses stale `AgentRun.modelRoute=DeepSeek/deepseek-chat/cloud` plus `runtimeRouteEvidence.actual_route=Ollama/llama3:latest/local`, `external_transmission=not_sent`; UI assertions require `最近实际路线 · 本地 · Ollama · llama3:latest`, `本地路线 · Ollama`, `运行证据：未外发`, and no `云端路线 · DeepSeek`. |
| Current automated replay: cloud evidence wins | `runtimeDisclosure.test.ts` adds the reverse case: stale `AgentRun.modelRoute=ollama/local`, `runtimeRouteEvidence.actual_route=DeepSeek/deepseek-chat/cloud`, `external_transmission=sent`; UI assertions require `云端路线`, `DeepSeek`, and `运行证据：已外发`. |
| UI/Runs/DB verdict | Historical browser/Runs/DB were inconsistent and lacked `runtimeRouteEvidence`. Current automated UI and run-object evidence are consistent with `runtimeRouteEvidence`; fresh browser + app DB replay remains pending before marking the raw V6 issue fixed/verified. |

### Settings Readiness Contradiction

| Field | Evidence |
|---|---|
| Diagnostics payload shape | `runtime_route_evidence.provider_readiness` separates `configured`, `credential_present`, `validated`, `validation_status`, `preferred`, `actually_used`, and `last_checked_at`; diagnostics command adds `runtime_route_evidence` without probing a provider. |
| Configured / credential / validated / status split | `OverviewTab.test.tsx` fixture: `configured=true`, `credential_present=true`, `validated=false`, `validation_status=unvalidated`, `external_transmission=not_instrumented`; UI asserts `Configured, not validated` and `不能当作 cloud-ready`. |
| Scripted/dogfood split | `ProviderTab.test.tsx` fixture: `configured=true`, `credential_present=true`, `validated=false`, `validation_status=scripted_dogfood`; UI asserts `Local test proof only`, `不是 external cloud ready`, and `外发记录未接入`. |
| Readiness verdict | Current automated replay proves configured/key-present/scripted are not displayed as external cloud ready. Fresh Settings screenshot against the running app is still pending. |

### Explicit Cloud Request With Actual Local/Fallback

| Field | Evidence |
|---|---|
| Prompt text | Same `V6_CLOUD_TEST` prompt above explicitly asks to prefer cloud and report blocker if cloud cannot be used. |
| Planned route | Backend focused fixture config in `provider_route_runtime_route_evidence_reports_local_fallback_and_transmission_boundary`: `deepseek / deepseek-chat / cloud` with no API key and network policy enabled. |
| Actual route | Backend fixture completed run: `ollama / llama3 / local`; current evidence reports local route and `external_transmission=not_sent`. |
| Fallback reason | Backend evidence asserts `fallback.reason=provider_api_key_missing`. |
| UI route/fallback display | `ChatPage.test.tsx` companion replay now includes fallback evidence and asserts actual local chips plus `Fallback：provider_api_key_missing`, while rejecting stale `云端路线 · DeepSeek`. |
| Verdict | Current automated replay shows actual local/fallback evidence overrides cloud claim. Historical DB run had `fallbackReason=null`, proving why this slice was needed; fresh app DB replay after this patch is still pending. |

## Regression Map

| Raw issue | Status | Evidence in this change | Remaining blocker |
|---|---|---|---|
| `OL-001` | improved | Route-truth classifier and `runtimeRouteEvidence` metadata now prevent provider/model truth from relying on assistant prose. `ChatPage.test.tsx` covers runtime evidence overriding a stale DeepSeek/cloud claim and showing fallback evidence. | Needs fresh browser replay with prompt, route chip/header, run id, `AgentRun.modelRoute`, and DB/Runs comparison before marking fixed. |
| `OL-008` | improved | Settings Overview cloud checklist now uses shared provider readiness instead of `cloud_api_configured`. | Needs Settings screenshot/diagnostics payload replay. |
| `V4-001` | improved | Overview no longer treats configured-only provider as cloud-ready; focused Overview test covers configured/unvalidated. | Needs v4 Settings replay screenshot. |
| `V4-006` | improved | Diagnostics exposes provider readiness fields and source refs through `RuntimeRouteEvidence`. | Full UI surfacing beyond the thin slice remains pending. |
| `V4-007` | improved | Runtime disclosure uses evidence route and unknown/not-instrumented states instead of confident fallback labels. | Runs/detail replay still pending. |
| `V5-011` | improved | Runtime disclosure chip renders provider/model/route/fallback from evidence, not model text. | Needs original v5 replay path checked against current UI. |
| `V6-001` | improved | v6-style route prompt classifier tests added; ChatPage test proves DeepSeek/cloud stale claim is not rendered as route truth when evidence says Ollama/local; runtimeDisclosure test proves cloud evidence still renders as cloud/sent. | Needs fresh C02/C03 replay against UI, Runs, and DB before fixed. |
| `V6-003` | improved | Settings Overview and Provider tab consume the same readiness utility. | Needs side-by-side Settings replay. |
| `V6-004` | improved | `scripted_dogfood` is separated from validated external cloud readiness; configured/unvalidated remains warning, not ready. | Needs manual Settings health screenshot. |
| `V6-005` | improved | Backend evidence builder records local actual route plus cloud planned/preflight blocker as fallback evidence; focused Rust test covers positive local no-send evidence; ChatPage test shows actual local plus fallback reason instead of cloud claim. | Needs fresh explicit cloud-request browser/DB replay with persisted runtime evidence before fixed. |
| `V6-009` | blocked | Not directly replayed in this slice. Unknown/not_instrumented display rules were added where route evidence is rendered. | Needs the original V6-009 scenario identified and replayed before status can move beyond blocked. |

## Gates Run

- `cargo test -p openlife-tauri provider_route -- --nocapture` -> 9 passed.
- `cargo test -p openlife-tauri diagnostics -- --nocapture` -> 1 passed.
- `cargo test -p openlife-tauri provider_validation -- --nocapture` -> 6 passed.
- `cd frontend && corepack pnpm test -- runtimeDisclosure.test.ts OverviewTab.test.tsx ProviderTab.test.tsx ChatPage.test.tsx` -> 4 files passed, 80 tests passed.
- `cd frontend && corepack pnpm typecheck` -> passed.
- `git diff --check` -> passed.

## Deferred

- No real live external provider test was run.
- No API key was read, entered, or modified.
- No full provider transmission log was implemented.
- No old run was classified as `not_sent` without positive local/no-provider-invocation route evidence.
- No broad Settings redesign or full Runs lifecycle work was done.
