# Sprint 5 Diagnosis Packet: Privacy and Provider Governance

Date: 2026-06-29

Status: source-level preparation for bounded Slice 5A. This packet is not a fixed-status claim.

## Scope

Raw audit issues: `OL-010`, `V4-005`, `V5-014`, `V6-006`, `V6-010`.

Primary user promise: a user can tell whether a run stayed local, was sent externally, was blocked before external transmission, or is unknown because older instrumentation is missing.

## Verified Source Reality

Checked source entrypoints:

| Surface | Current reality | Risk |
|---|---|---|
| `src-tauri/src/main_chat_runtime_facts/provider_route.rs` | `RuntimeRouteEvidence` already includes `external_transmission`, route identity, provider readiness, fallback, and source refs. | This is route/transmission evidence, not a full provider payload ledger. Do not infer exact sent payloads or provider retention from it. |
| `src-tauri/src/main_chat_runtime_facts_tests.rs` | Tests already cover cloud -> `sent`, local/runtime fact -> `not_sent`, fallback local -> `not_sent`, and missing settings instrumentation -> `not_instrumented`. | These tests prove route boundary semantics, not Privacy page history. |
| `frontend/src/pages/RunsPage.tsx` / `frontend/src/pages/AgentRunDetail.tsx` | Runs surfaces can display route evidence and external transmission for individual run/detail contexts. | Privacy page does not yet provide an aggregated per-run transmission history. |
| `frontend/src/pages/settings/tabs/PrivacyTab.tsx` | Privacy currently shows hot memory summary, local MCP audit counts, audit export/cleanup/key rotation actions, and PII policy. | It lacks sent/not-sent history and danger-action preflight UX. Some buttons still look like direct actions rather than preflight-first governance. |
| `src-tauri/src/main_chat_live_provider_harness.rs` and agent runtime final gate | Live provider harness can prove opt-in model invocation/no-invocation with blockers. | This is eval harness evidence, not user-facing transmission history. Do not run live providers in Slice 5A. |
| Search result for `ProviderTransmission` | No dedicated ProviderTransmission source/test symbol exists yet. | A new table is optional later; Slice 5A should avoid broad storage migration unless required. |

## Root-Cause Hypotheses

1. `V6-006` / `V6-010`: external provider transmission is visible in scattered runtime/eval surfaces but not as a user-facing Privacy history.
2. `OL-010`: danger actions are grouped in Privacy but do not yet have a unified typed preflight model with scope, backup status, safe-mode blocking, and final confirmation semantics.
3. `V4-005` / `V5-014`: existing UI can imply safety from absence of evidence unless `unknown` / `not_instrumented` is explicitly shown.
4. A new provider-transmission table is not the right first slice until the read-model semantics are proven from current AgentRun and RuntimeRouteEvidence.

## Industry Benchmark Applied

| Benchmark | Product bar for Slice 5A |
|---|---|
| Granola sharing/privacy pattern | Visibility and sharing state must be explicit in the product surface, not implied by missing controls. |
| Codex / Cursor background agents | Agent runs need auditable state, logs, and proof of what executed or did not execute. |
| ChatGPT / Claude data controls as product pattern | User-facing data-control surfaces should separate local, external, and unknown states. This is a product pattern only; do not make provider-retention claims without direct provider docs and OpenLife runtime evidence. |

## Slice 5A Frozen Scope

Implement only: read-only provider transmission history derived from existing AgentRun / RuntimeRouteEvidence semantics, surfaced in Privacy.

Required behavior:

- Add a backend `ProviderTransmissionView` or equivalent read model derived from durable AgentRun route metadata and `RuntimeRouteEvidence` semantics.
- Supported statuses for Slice 5A: `sent`, `not_sent`, `blocked`, `unknown`, `not_instrumented`.
- `not_sent` requires positive local / agent_runtime / scripted route evidence or explicit no-provider-invocation evidence.
- `sent` requires cloud route or live-provider invocation evidence from runtime metadata.
- `blocked` requires provider preflight/live-provider blocker evidence and no model invocation.
- Old runs without route/transmission instrumentation must render `not_instrumented` or `unknown`, never `not_sent`.
- Privacy tab renders recent per-run rows with run id, task id when available, status, provider/model/route type, reason, evidence id, confidence, and a safe data-category label.
- API keys must never be serialized into the view, test fixtures, or frontend diagnostics.

Non-goals for Slice 5A:

- No new live cloud/provider calls.
- No provider-key editing, viewing, or rotation behavior change.
- No import overwrite, deletion, rollback, cleanup, or key rotation final action.
- No new SQLite table unless the implementation proves AgentRun-derived projection is insufficient.
- No provider retention-policy claims beyond `unknown` / configured static docs placeholder.

## Storage Decision For Slice 5A

Use AgentRun-derived projection as the durable source for Slice 5A:

- `AgentRun.model_route` / route trace remains the durable execution record.
- `RuntimeRouteEvidence` remains the route/transmission semantics source.
- The new command should be a read model over recent runs, not a parallel truth table.
- Backfill behavior: old runs missing route evidence return `not_instrumented`.
- Migration behavior: no storage migration in Slice 5A.

Future Slice 5B may introduce a dedicated `ProviderTransmissionLogEntry` store only if payload categories, confirmation state, and provider-retention refs cannot be represented from AgentRun metadata.

## Anti-Hallucination Checks

- Do not infer `not_sent` from missing provider log.
- Do not infer `sent` from Settings configured/preferred provider.
- Do not trust assistant text that says cloud/local; use route evidence and live-provider metadata.
- Do not expose or serialize API key material; tests should use fake keys only when checking redaction.
- Do not mark old runs safe; use `unknown` or `not_instrumented`.
- Do not claim retention behavior unless backed by a direct provider policy reference and explicit runtime provider identity.

## Slice 5A Acceptance Tests

Backend focused tests to add or update:

- `provider_transmission_view_records_local_not_sent_with_positive_route_evidence`
- `provider_transmission_view_records_cloud_sent_with_cloud_route_evidence`
- `provider_transmission_view_marks_missing_route_old_run_not_instrumented`
- `provider_transmission_view_rejects_missing_log_as_not_sent_evidence`
- `provider_transmission_view_never_serializes_key_material`
- `provider_transmission_view_records_blocked_preflight_without_model_invocation`

Suggested backend gates:

- `cargo test -p openlife-tauri provider_transmission`
- `cargo test -p openlife-tauri provider_route_runtime_route_evidence`
- `cargo test -p openlife-tauri live_provider`

Frontend focused tests to add or update:

- `PrivacyTab` renders `sent`, `not_sent`, `blocked`, `unknown`, and `not_instrumented` rows distinctly.
- `PrivacyTab` shows no-key-leak diagnostics and does not display raw prompts or API keys.
- Empty history explains that instrumentation is missing instead of implying no external transmission.

Suggested frontend gates:

- `cd frontend && corepack pnpm test -- PrivacyTab.test.tsx`
- `cd frontend && corepack pnpm test -- SettingsPage.test.tsx`
- `cd frontend && corepack pnpm typecheck`

Repository gates:

- `cargo fmt --check`
- `git diff --check`

## Replay Scenarios For Slice 5A

| Scenario | Expected Slice 5A evidence |
|---|---|
| Local Main Chat run | Privacy shows `not_sent`, local/agent_runtime route evidence id, provider/model if available. |
| Cloud/live provider blocked preflight | Privacy shows `blocked`, blocker code, no model invocation. |
| Historical run without model route | Privacy shows `not_instrumented` or `unknown`, never `not_sent`. |
| Settings configured provider but no invocation | Privacy does not show `sent`; Settings configured state is not invocation proof. |

## Rework Triggers

Slice 5A must be returned for rework if any of these happen:

- `not_sent` is produced solely because no transmission log exists.
- Settings configured/preferred provider is treated as `sent` or live-ready.
- API key or secret-like material appears in command output, frontend state, tests, or traces.
- Privacy page hides unknown/not-instrumented rows.
- A new storage table duplicates route truth without consuming `RuntimeRouteEvidence`.
- Live provider calls are executed by default.
