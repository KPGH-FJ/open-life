# Sprint 0 Diagnosis: Runtime Truth, Provider Route, Settings Readiness

Date: 2026-06-29

Status: Diagnosis packet and RFC outline. Not implemented.

## Raw Issues

Primary issues: `OL-001`, `OL-008`, `V4-001`, `V4-006`, `V4-007`, `V5-011`, `V6-001`, `V6-003`, `V6-004`, `V6-005`, `V6-009`.

Highest severity:

- `OL-001` P0: runtime facts/date/model/tool claims conflict with actual route evidence.
- `V6-001` P0: assistant claimed DeepSeek/cloud while Runs/DB proved local Ollama.
- `V6-003` P1, `V6-004` P1, `V6-005` P1: Settings and cloud preference do not produce trustworthy route/readiness behavior.

## Observed Evidence

- v6 C02/C03 route prompts produced assistant text claiming DeepSeek/cloud, while DB route evidence for run `82fef9fe-c262-494c-93de-ea9b4dc5a225` showed provider `ollama`, model `llama3.1:latest`, route type `local`, and `liveProviderInvoked=false`.
- Prior timeout run `51ceb9cb-be9e-49dd-8bd1-0df6fa21b8f4` showed cloud/DeepSeek timeout in UI while durable state remained non-terminal.
- Settings Overview and Provider tab used different readiness framing: Overview summarizes broad booleans, Provider tab has richer validation status.

## Source Findings

| Area | Finding |
|---|---|
| `src-tauri/src/provider_validation.rs` | A typed validation model already exists: `unconfigured`, `unvalidated`, `stale`, `validated`, `failed`; `cloud_api_configured` only means provider/base/model/key presence. |
| `src-tauri/src/commands/diagnostics.rs` | Diagnostics exposes validation fields, but `chat_ready`/`beta_ready` collapse several states. There is also a scripted dogfood validation override that can make readiness look stronger than external live proof. |
| `src-tauri/src/main_chat_runtime_facts/provider_route.rs` | Runtime route fact answers exist, including current/last/planned/preflight route data. The classifier is too phrase-specific for mixed prompts such as "provider/model/routeType/fallbackReason" plus "use cloud". |
| `frontend/src/pages/settings/tabs/OverviewTab.tsx` | Overview consumes broad fields such as `cloud_api_configured`, `ollama_online`, `chat_ready`, and `beta_ready`, which can blur configured vs validated vs actually-used route. |
| `frontend/src/pages/settings/tabs/ProviderTab.tsx` | Provider tab already distinguishes validation status more clearly and is the better source for UI state semantics. |
| `frontend/src/utils/runtimeDisclosure.ts` | There is a reusable disclosure component, but it depends on run metadata being present and does not prevent assistant prose from contradicting route truth. |
| `frontend/src/utils/runDisplaySummary.ts` | Summary can show route labels, but outcome text still relies on AgentRun counters and does not unify provider/readiness/truth across product surfaces. |

## Root-Cause Hypothesis

1. Authoritative route facts exist, but the product does not force route-truth prompts through them. If the classifier misses the prompt, the model can self-report provider/model and hallucinate route identity.
2. Settings has richer provider validation state in the backend, but top-level readiness UI collapses `configured`, `validated`, `scripted`, `preferred`, and `actually_used`.
3. Runtime route, provider validation, fallback reason, and external invocation status are not represented as one user-facing contract.
4. There is no durable per-run external transmission fact yet, so the product cannot prove `sent_to_external_provider` vs `not_sent_externally`.

## Industry Comparison

- ChatGPT Memory emphasizes user control and visibility for persistent personalization; OpenLife needs the same visibility standard for model/provider state because provider route changes privacy and trust.
- Codex cloud positions background work as task-based and auditable; OpenLife route state must be visible in Runs as task evidence, not just Settings copy.
- Granola's privacy framing makes private/default and sharing boundaries explicit; OpenLife needs an equally explicit "local only" vs "sent externally" route boundary.

## Solution RFC Outline

### Target Behavior

- A single `RuntimeRouteEvidence` contract becomes the only route truth for Companion, Runs, Settings, Privacy, and audit reports.
- Route-truth prompts are answered before model generation when the user asks about provider/model/route/fallback/tool availability, even if the prompt is mixed Chinese/English.
- Settings separates:
  - `configured`
  - `credential_present`
  - `validated`
  - `last_checked_at`
  - `preferred`
  - `planned_for_next_turn`
  - `actually_used_last_turn`
  - `stale`
  - `failed`
  - `fallback`
  - `external_invocation`

### UI Contract

- Companion answer header shows a runtime-authored route chip: provider, model, route type, privacy boundary, fallback reason.
- Settings Overview reuses Provider tab's typed readiness state and never shows green readiness for configured-only cloud.
- Runs list/detail shows route evidence and external-transmission instrumentation status per run. Definitive sent/not-sent audit history lands only after the Sprint 5 provider-transmission contract is implemented.

### Backend Contract

`RuntimeRouteEvidence` should be assembled from:

- provider validation summary
- ModelRouter status
- current-turn planned route
- last completed run route
- actual run `modelRoute`
- fallback reason
- live-provider invocation proof
- external transmission entry when E6 lands

### Failure States

- Requested cloud but no validated credential: return blocker `cloud_route_not_verified` or `cloud_provider_unvalidated`.
- Requested cloud but routed local due policy/fallback: show local route and fallback reason.
- Provider validation stale: show stale, not ready.
- Route evidence unavailable: show unknown and a developer-debug action, not a confident provider claim.

## Replay Tests

| Test | Expected |
|---|---|
| Ask `V6_CLOUD_TEST: 当前 provider/model/routeType/fallbackReason 是什么？本轮请优先使用云端` | Runtime-authored answer matches Runs/DB; no model self-claim contradiction |
| Settings configured key but no validation | Overview says configured/unvalidated, not Beta-ready/cloud-ready |
| Explicit cloud request under local route | UI shows local route plus fallback/blocker reason |
| Provider validation stale | Settings says stale and offers validation action, not ready |
| Last run local Ollama | Companion route chip and Runs both show local Ollama |

## Anti-Hallucination Checks

- Do not accept assistant text as proof of provider/model.
- Compare UI route with `AgentRun.modelRoute` and provider validation summary.
- Treat missing live-provider invocation proof as `not_verified`, not cloud success.
- Any readiness text must be backed by typed diagnostics fields.

## Thin-Slice Implementation Proposal

1. Add a small `RuntimeRouteEvidence` DTO and builder in backend diagnostics/runtime facts.
2. Expand `provider_route_fact` classification for route/provenance/cloud/fallback prompts.
3. Expose the DTO through existing diagnostics or a focused command.
4. Update Settings Overview and route disclosure chip to consume the DTO.
5. Add focused tests for v6 C02/C03 prompt classification and configured-vs-validated UI state.

## Open Questions

- Should `scripted_dogfood` validation ever appear in user-facing Settings, or only in developer diagnostics?
- Should route truth prompt response be pure system text, or a normal assistant message with a locked route evidence header?
- Where should external transmission data live before E6 adds the full provider transmission log?
