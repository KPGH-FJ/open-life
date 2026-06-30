# Sprint 1 Solution RFC: Trust Foundation

Date: 2026-06-29

Status: ready for thin-slice implementation after review.

## Scope

Raw issues: `OL-001`, `OL-008`, `V4-001`, `V4-006`, `V4-007`, `V5-011`, `V6-001`, `V6-003`, `V6-004`, `V6-005`, `V6-009`.

Primary source entrypoints:

- `src-tauri/src/provider_validation.rs`
- `src-tauri/src/commands/diagnostics.rs`
- `src-tauri/src/main_chat_runtime_facts/provider_route.rs`
- `frontend/src/pages/settings/tabs/OverviewTab.tsx`
- `frontend/src/pages/settings/tabs/ProviderTab.tsx`
- `frontend/src/utils/runtimeDisclosure.ts`
- `frontend/src/utils/runDisplaySummary.ts`

## Product Goal

Users must be able to trust what OpenLife actually used: provider, model, local/cloud route, fallback reason, and validation state. External-transmission history is represented only as instrumentation status in this sprint; definitive sent/not-sent audit history belongs to Sprint 5.

## Non-Goals

- Do not add new cloud providers.
- Do not run live provider tests without explicit user authorization.
- Do not store or display raw API keys.
- Do not redesign all Settings UI; only fix readiness truth and route disclosure.

## Data Contract

Create a metadata-safe `RuntimeRouteEvidence` read model.

| Field | Type | Meaning |
|---|---|---|
| `evidence_id` | string | Stable id for this evidence snapshot. |
| `generated_at` | ISO string | Backend time when evidence was assembled. |
| `conversation_id` | string? | Current conversation if scoped to chat. |
| `run_id` | string? | Actual run if available. |
| `task_session_id` | string? | Main Chat task session if available. |
| `answer_scope` | `current_turn` / `last_completed_turn` / `settings_readiness` / `planned_next_turn` / `unknown` | What the evidence answers. |
| `planned_route` | `RouteIdentity?` | Route intended before generation. |
| `actual_route` | `RouteIdentity?` | Route actually used by runtime. |
| `last_completed_route` | `RouteIdentity?` | Last completed run route. |
| `provider_readiness` | `ProviderReadiness` | Config and validation truth. |
| `fallback` | `FallbackEvidence?` | Fallback provider/model/reason if any. |
| `external_transmission` | `not_sent` / `sent` / `unknown` / `not_instrumented` | External transmission state when directly evidenced; otherwise instrumentation status. |
| `source_refs` | array | Metadata-safe source refs: diagnostics, AgentRun, task session, validation record. |
| `truth_confidence` | `verified` / `inferred` / `unknown` | Whether evidence is directly observed. |

`RouteIdentity`:

| Field | Type |
|---|---|
| `provider` | string |
| `model` | string |
| `route_type` | `local` / `cloud` / `agent_runtime` / `scripted` / `unknown` |
| `privacy_level` | string |
| `reason` | string |
| `provider_health_is_estimated` | boolean |

`ProviderReadiness`:

| Field | Meaning |
|---|---|
| `configured` | Provider/base/model/key config appears present. |
| `credential_present` | Key presence boolean only, never raw key. |
| `validated` | A live or accepted validation record is valid. |
| `validation_status` | `unconfigured`, `unvalidated`, `stale`, `validated`, `failed`, `scripted_dogfood`. |
| `preferred` | User/config preferred provider. |
| `actually_used` | Last actual runtime provider if available. |
| `stale` | Validation older than TTL or route evidence too old. |
| `failed` | Last validation or invocation failed. |
| `last_checked_at` | validation timestamp if present. |

External-transmission boundary:

- Sprint 1 may show `not_instrumented` or `unknown` when no durable transmission log exists.
- Sprint 1 may show `sent` only if existing runtime metadata positively proves an external provider invocation for the specific run.
- Sprint 1 may show `not_sent` only with positive local-route/no-provider-invocation evidence for the specific run. Missing provider logs alone are not enough.
- The UI copy must say "外发记录未接入" or "无法从当前证据判断是否外发" rather than "未外发" when the backend only has absence of logs.
- Persisted per-run sent/not-sent history, data categories, confirmation state, and retention links are explicitly deferred to Sprint 5.

## Settings Mapping

| Condition | Settings label | User meaning |
|---|---|---|
| not configured | Not configured | No cloud provider is ready. |
| configured, no validation | Configured, not validated | Key/config exists, live use not proven. |
| configured, stale validation | Validation stale | Recheck before treating as available. |
| validation failed | Failed validation | Cloud path is unavailable until fixed. |
| scripted dogfood only | Local test proof only | Developer proof, not external cloud readiness. |
| validated, not actually used | Validated, not used last run | Ready, but last task used another route. |
| actual route local | Used local | Current/last run stayed on local model. |
| actual route cloud | Used external provider | Current/last run sent to provider; see privacy log when Phase 5 lands. |

## Route-Truth Prompt Contract

Route-truth prompts must be answered by `RuntimeRouteEvidence` before model generation when they mention any strong signals:

- provider, model, route, routeType, fallback, actually used, current model
- DeepSeek, Ollama, OpenAI, OpenRouter, cloud, local, local-first
- "你现在用什么", "当前实际使用", "是否外发", "有没有调用云端"

Mixed task prompts should return both:

1. A short authoritative route statement.
2. If the user also asked a task, either continue with the task using that route or show a blocker.

## UI Contract

- Companion message header uses runtime-authored route chip, not assistant text.
- Runs list/detail use the same evidence fields.
- Settings Overview and Provider tab use the same readiness labels.
- If route is unknown, show "未验证" and a debug action, not a confident provider name.
- If external transmission is `unknown` or `not_instrumented`, show that state plainly and do not imply local-only privacy from missing telemetry.

## Tests

These are implementation-entry tests, not claims that the tests already exist. The sprint is not complete until new or updated focused tests cover the cases below and the command-level gates are recorded in the evidence bundle. Every command gate must record a non-zero matched/passed test count.

Backend focused tests:

- v6 C02/C03 mixed prompt classifies as route-truth.
- configured provider with no validation returns `validation_status=unvalidated`.
- scripted dogfood validation does not become external live-ready.
- local actual route plus cloud preference produces fallback/blocker evidence.
- missing transmission instrumentation renders `not_instrumented`, not `not_sent`.
- `not_sent` requires positive local/no-provider-invocation evidence.

Candidate command-level gates after adding the focused tests:

- `cargo test -p openlife-tauri provider_validation`
- `cargo test -p openlife-tauri provider_route`
- `cargo test -p openlife-tauri diagnostics`

Frontend tests:

- Settings Overview does not show green cloud readiness for configured-only provider.
- Runtime disclosure chip renders provider/model/route/fallback from evidence.
- Companion route answer does not display model self-claim as truth.

Candidate command-level frontend gates after adding/updating the focused tests:

- `cd frontend && corepack pnpm test -- SettingsPage.test.tsx`
- `cd frontend && corepack pnpm test -- runtimeDisclosure.test.ts`
- `cd frontend && corepack pnpm test -- ChatPage.test.tsx`

Replay:

- v6 C02/C03 prompt.
- Settings Overview vs Provider tab contradiction.
- Explicit cloud request when actual route remains local.

Replay evidence must include: prompt text, assistant route chip/header, run id, `AgentRun.modelRoute`, Settings readiness screenshot or diagnostics payload, and a short verdict stating whether model prose matched runtime evidence.

## Development Slices

1. Backend DTO and builder.
2. Route-truth classifier expansion.
3. Diagnostics command exposure.
4. Settings Overview mapping.
5. Companion/Runs route disclosure consumption.

Exit only when v6 route replay shows UI, Runs, and DB agreement.
