# OpenLife Roadshow V2 Web Evidence

Status: scoped implementation verified and real DuckDuckGo search path passed;
generic live fetch is blocked by the current macOS fake-IP/proxy environment,
while external cloud-provider proof, native product trial, and independent
read-only review remain pending. This file does not make a roadshow release or
global backend-remediation completion claim.

## Scope and commit

The V2 implementation is commit `a40605579cfed02b310cfaa0f7be2bbea82af596`
on `codex/roadshow-core-recovery`:

- `web.search` emits a bounded typed untrusted observation instead of raw page
  text or a false-success challenge body;
- DuckDuckGo challenge and empty/unparsed 2xx pages become typed failures, while
  ordinary result content discussing CAPTCHA or bot detection remains usable;
- PolicyRouter explicitly authorizes both the Web read and the same-turn
  provider synthesis capability;
- the existing `OpenLifeTurnRuntime` and `MainChatKernel` execute ToolGateway
  first, then pass bounded Web evidence through the existing prepared-provider
  boundary; no second runtime, router, or tool path was introduced;
- request-scoped `webref_<digest>` citations are bound to the canonical run and
  validated before any product-visible final answer;
- the backend, not model prose, renders the HTTPS source footer;
- the footer states `OpenLife 引用已绑定，内容未背书`, which describes citation
  binding without claiming that OpenLife fact-checked the external page;
- selected Web context persists only a strict metadata reference, not query,
  title, URL, snippet, message, or model-input copies;
- Web provider output is buffered until citation validation; ordinary direct
  answers retain their existing provider-token streaming path;
- empty, failed, and remote provider attempts preserve actual route/receipt
  identity and cannot be mislabeled as local success;
- exact operation replay reuses the durable final delivery and does not
  redispatch the Web tool or Provider;
- canonical AgentRun reasoning strategies now persist as the typed `direct`,
  `react`, or `memory_governance` values instead of degrading to opaque receipts.

## Mechanical evidence

Verified on 2026-07-15 in `/Users/tw/Desktop/open-life-roadshow`:

| Gate | Result | Credit boundary |
| --- | --- | --- |
| `cargo test -p openlife-core web_search::tests -- --nocapture` | 5/5 passed | typed observation parsing, HTTPS bounds, current-run citation, forged/cross-run rejection, metadata-only context refs |
| `cargo test -p openlife-core web_content_observation_tests -- --nocapture` | 4/4 passed | challenge/empty failure, CAPTCHA false-positive counterexample, bounded search and fetch observations |
| canonical reasoning-strategy round-trip test | passed | `direct`, `react`, and `memory_governance` remain typed after store restart boundary |
| `cargo test -p openlife-tauri web -- --nocapture` | 20 passed, 4 explicitly ignored live gates | Web policy, ToolGateway, citation, durable refs, send/stream, replay; ignored cases receive no credit |
| capability eval suite | 6/6 passed | local deterministic direct/file/Web/MCP evidence; scripted Web fixture is not live-provider credit |
| command-surface send/stream matrix | passed | operation-scoped citation fixture works through both ordinary product entrypoints |
| `cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture` | 29/29 passed | one ordinary TurnRuntime and deleted-route guards |
| real DuckDuckGo plus captured local HTTP Provider gate | 1/1 passed | actual Web network read, same-turn provider adapter, durable provider lifecycle, final-last delivery |
| same-operation Web replay | passed | one provider dispatch and one durable provider lifecycle across retry |
| `cargo check -p openlife-tauri --tests` | passed with two existing warning groups | no warning-free claim |
| `cargo fmt --check` and `git diff --check` | passed | formatting and patch hygiene |

The real search gate used DuckDuckGo over the current network and a captured
local HTTP OpenAI-compatible provider. It proves the network-read, prepared
provider, citation, Receipt, event, and terminal-projection chain. The local
provider is not external cloud-provider credit.

## Failure and counterfactual evidence

- missing, malformed, forged, or cross-run citation ids produce
  `web_citation_validation_failed` and no `FinalAnswer`;
- a DuckDuckGo challenge produces `web_search_challenge_detected`;
- an unparsed/empty 2xx page produces `web_search_no_structured_results`;
- a normal result about CAPTCHA remains a valid result instead of being
  misclassified as a challenge;
- invalid Web observation data never invokes the Provider;
- Provider empty/failure terminal facts retain Provider, model, request id, and
  typed lifecycle events while producing no final answer;
- missing canonical run identity remains a structured
  `canonical_run_identity_missing` blocker;
- durable provider events reject malformed Web refs and retain no raw Web body;
- replay with the same operation id performs no second Provider dispatch.

## Bounded red and environment evidence

The following results are intentionally not converted into green credit:

- the ignored generic `web.fetch https://example.com/` live gate returned
  `network_policy_blocked`;
- `scutil --proxy` showed the active HTTP/HTTPS proxy at `127.0.0.1:1082`;
- `example.com` resolved to `198.18.0.165` and `::ffff:0:c612:a5`, which are
  fake-IP/reserved ranges rejected by the SSRF/DNS-rebinding boundary;
- no generic caller-controlled URL was granted the fixed-official-endpoint
  proxy exception used by the bounded DuckDuckGo search adapter;
- all `OPENLIFE_MAIN_CHAT_LIVE_PROVIDER_EVAL` and
  `OPENLIFE_LIVE_EVAL_*` variables were unset, so no external cloud Provider
  gate ran;
- the full kernel set was 60/68 with the same eight pre-existing failures in
  MCP/queue, file-write route, memory-admission, and typed payload tests;
- the single-system set remained 24/32 with the same eight pre-existing
  inventory/gateway/frontend/read-model failures;
- `cargo clippy -p openlife-core --lib --no-deps -- -D warnings` remained red
  with 35 existing errors; no new error pointed to `web_search.rs` or the V2 Web
  implementation;
- an independent read-only source and evidence review was not run and remains
  pending.

## Remaining V2 evidence

- external cloud-provider Web answer with real credentials and the same
  receipt/citation assertions;
- generic live fetch on a network that exposes the destination's real public
  address, without weakening the SSRF/DNS-rebinding policy;
- native desktop product trial covering search success, challenge/blocker,
  citation display, retry/replay, and no-Proposal ordinary reads;
- independent read-only source and evidence review;
- cumulative concurrency, fault injection, and soak execution.

Until those finish, V2 is
`implementation_verified_live_search_passed_live_fetch_environment_blocked`,
not globally complete, and the roadshow release remains NO-GO.
