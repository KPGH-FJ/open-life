# OpenLife Roadshow V2 Web Evidence

Status: scoped implementation, generic live fetch, governed DeepSeek live
search, and the RC04 external Resource + Web + Provider backend chain are
verified on current code. Native product trial and independent read-only review
remain pending. This file does not make a roadshow release or global
backend-remediation completion claim.

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

## 2026-07-18 current-code addendum

Commit `930ce33e76583371e6fe067940042d89fb9c2f59` adds a governed
DeepSeek server-side Web Search adapter without adding a second runtime,
router, ToolGateway, observation schema, or credential owner:

- the adapter uses one bounded POST through the existing `NetworkClient` with
  the exact `web.search` capability, redirect rejection, DNS/SSRF validation,
  response limits, timeout, and no automatic POST retry;
- the response edge accepts only structured `web_search_result` title and
  canonical HTTPS URL pairs;
- provider thinking, signatures, encrypted content, error bodies, and prose
  without an exact structured-result URL are discarded;
- a bounded provider-synthesized line may become that result's snippet only
  when the same line contains its exact URL, and the observation explicitly
  states that it is untrusted, not independently verified, and not guaranteed
  to be entailed by the page;
- output remains the existing strict
  `openlife_web_search_observation_v1` contract; an attempted parallel set of
  extra provider-summary fields was caught by the live gate and removed rather
  than weakening `deny_unknown_fields`;
- `SearchProviderConfig` remains per-execution and the existing SystemConfig +
  Keychain-backed search credential remains the one configuration owner;
- the frontend change is type-contract only. A visual/product control for
  choosing the search provider belongs to the later frontend handoff and is
  not claimed by this backend freeze.

Commit `81d972635933a6908842090ecb63d74bfcd92141` closes the
stability gap found by the second live RC04 run. [DeepSeek's official
integration guide](https://api-docs.deepseek.com/quick_start/agent_integrations/claude_code)
says the model decides whether the query requires Web Search; one real
run therefore returned only thinking/text and correctly failed as
`web_search_no_structured_results`. Because PolicyRouter had already authorized
and required external evidence, the adapter now sends `tool_choice=web_search`
and asks for exactly one verbatim-query search. This is one request, not a
failure retry. DeepSeek may still perform an unobservable number of internal
search operations despite `max_uses=1`, so OpenLife claims one ToolGateway/HTTP
dispatch and does not claim an exact remote internal search count.

Current mechanical and live evidence:

| Gate | Current result | Credit boundary |
| --- | --- | --- |
| Core full suite | 1477 passed, 0 failed, 2 ignored | current code, including typed DeepSeek, forced policy-required tool choice, and exact network-policy counterfactuals |
| Tauri full suite | 1172 passed, 0 failed, 13 ignored; parser binary 2/2 | ignored live gates do not receive credit from this run |
| workspace all-target Clippy with `-D warnings` | passed | warning-free current workspace |
| frontend typecheck and format | passed | backend config contract only; no UI journey credit |
| real DeepSeek search + captured local Provider | 1/1 passed after stability fix | actual external search, one dispatch, typed observation, bound Web citation, durable Provider lifecycle |
| RC04 real Resource + DeepSeek search + external Provider | 2/2 consecutive passed after stability fix | each run has one frozen Resource, one external Web action, one external Provider, both citation classes, zero Proposals |

The current macOS proxy/fake-IP path is handled by a domain-bound egress
exception for the fixed official DeepSeek endpoint. It does not weaken private,
reserved, mixed-DNS, redirect, or caller-controlled URL checks. DuckDuckGo's
current challenge response still fails closed as `web_search_challenge_detected`;
the DeepSeek success does not relabel that route as successful.

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

## Historical bounded red at the original V2 checkpoint

The following results describe the original `a4060557` checkpoint. They remain
historical failure evidence and are not rewritten as if they had passed at that
time; the dated addendum above records which items were later repaired:

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

- native desktop product trial covering search success, challenge/blocker,
  citation display, retry/replay, and no-Proposal ordinary reads;
- independent read-only source and evidence review.

Until those finish, V2 is
`implementation_and_external_live_verified_native_trial_and_independent_review_pending`,
not globally complete, and the roadshow release remains NO-GO.
