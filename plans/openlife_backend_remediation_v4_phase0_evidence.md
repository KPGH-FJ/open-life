# OpenLife Backend Remediation v4 Phase 0 Evidence

> Date: 2026-07-10
> Status: active, fail-closed; this document does not grant Phase 1-6 completion
> Baseline revision: `1ca7613bcd25167cf173fa0a21e3baa908f21d94`

This is the evidence companion to `openlife_backend_remediation_v4.md`. It
separates observed facts, planned proof, and unknown product metrics. A module,
test name, or source string is not completion evidence by itself.

## Evidence Status Vocabulary

| Status | Meaning |
| --- | --- |
| `OBSERVED` | Reproduced from a network edge, canonical store, durable event, fault injection, or executable gate. |
| `SOURCE-REPRODUCED` | The baseline source contains the broken route. This is valid architecture evidence but not runtime proof. |
| `PARTIAL` | Some required properties were observed; named proof is still missing. |
| `UNKNOWN` | Not measured or not observable. It fails exact-consistency and readiness gates. |
| `PLANNED` | Required future evidence. It earns no completion credit. |

The 35 finding rows, broken invariants, source references, proof references,
counterfactuals, non-regression scenarios, and current status are in
`openlife_backend_remediation_v4_traceability.json`.

## Current Product Source Map

### Ordinary buffered Main Chat

```text
frontend invoke send_message
  -> src-tauri/src/lib.rs::send_message
  -> src-tauri/src/main_chat_send.rs::send_message_with_state
  -> OpenLifeTurnRuntime::run_buffered
  -> OpenLifeTurnRuntime::run_with_emitter
  -> main_chat_runtime_support::start_main_chat_agent_turn
  -> main_chat_kernel::run_main_chat_kernel_direct_answer_with_state
  -> MainChatKernel::run_turn
  -> SchedulerMainChatModelClient::generate_direct_answer
  -> InferenceScheduler::prepare_chat_request_with_policy
  -> InferenceScheduler::execute_prepared_with_start_observer
  -> provider adapter HTTP/local model edge
```

`MainChatProviderAuthorization` now carries the `ProviderDataRoute` and the
actual policy request id from `AgentIngressDecision`; DirectAnswer cannot use
the scheduler's policy-allowed default as a substitute for PolicyRouter truth.
The ingress decision also tightens the data route against every message
selected for provider context, so an innocuous follow-up cannot send earlier
sensitive history to cloud. The final HTTP boundary reapplies one configured
PrivacyPolicy to every message role and bounded context block.
Controlled HTTP capture now also correlates the wire-level
`x-openlife-request-id` with durable provider start/completion facts and checks
provider/model parity while a raw LifeModel sentinel remains absent.

### Ordinary streaming Main Chat

```text
frontend invoke start_stream_message
  -> src-tauri/src/lib.rs::start_stream_message
  -> src-tauri/src/main_chat_streaming.rs::start_stream_message_with_state
  -> OpenLifeTurnRuntime::run_streaming
  -> the same run_with_emitter and MainChatKernel owner as buffered
  -> generate_prepared_stream
  -> MainChatKernelEvent::ProviderToken
  -> stream-message-chunk
  -> durable terminal materialization
  -> stream-message-done
```

The local SSE test holds the second provider chunk behind a two-way barrier and
observes the first chunk in the product callback before the turn can finish.
It proves incremental plumbing for one controlled provider; it does not prove
external-provider behavior or crash durability.

### ReAct/provider path

```text
OpenLifeTurnRuntime
  -> main_chat_kernel ReAct branch
  -> main_chat_react_runtime::try_run_main_chat_react_agent_loop
  -> model-based candidate ranking
  -> AgentRuntime / AgentLoop
  -> ToolGateway
```

Status: `PARTIAL / P0 OPEN`. The context assembler overwrite bug and the raw
candidate-ranking provider bypass are fixed; candidate ranking now uses the
same configured PrivacyPolicy and prepared provider seam. Candidate-ranking
and non-streaming AgentLoop adapter receipts now propagate through the ReAct
attempt into the durable event store, and `liveProviderInvoked` is derived from
those receipts. A formal cross-provider `ExecutionReceiptV1`, restart-level
atomicity proof, and streaming AgentLoop receipt contract are still missing;
these paths do not earn Phase 1 truth completion credit.

### Product tools

```text
Main Chat policy/read plan
  -> ToolGateway contract validation
  -> ActionExecutor adapter
  -> NetworkClient / MCP / local read adapter
```

The generic `execute_tool_call`, arbitrary MCP register/unregister commands,
and direct A2A commands are absent from the default release handler. They can
exist only in debug `dev-extensions`; compilation fails when that feature is
combined with a non-debug build. The former `web.fetch(summarize=true)` helper
no longer starts an untracked hard-coded Ollama request; it returns a bounded,
structured `untrusted_external_content` observation so synthesis stays in the
active TurnRuntime's authorized provider path. ToolGateway single-authority
proof across all product commands remains `PARTIAL` until the Phase 4
callsite/absence gate.

### Durable state

| Concern | Current intended owner | Current evidence boundary |
| --- | --- | --- |
| Conversation messages | conversation store | canonical content; AgentRun should reference it |
| Agent run facts | AgentRun store and `MainChatAgentEventStore` | AgentRun persistence now projects input/output/action/observation/error/status content to refs, categories, byte counts and digests, including a v1 legacy-row migration; cross-store raw-sentinel and restart proof remains pending |
| Reversible Memory | `MemoryGateway` | explicit lane and undo require scenario proof |
| Canonical LifeModel/YAML migration source | `LifeModelWriteGateway` | version CAS exists; cross-store transaction/outbox is not complete |
| Accepted HS assets | accepted HS stores by asset category | target owner; no whole-model cutover is authorized |
| Proposal/review state | `ReviewWorkflow` plus ProposalStore storage | dispatch claim exists; remote reconciliation remains incomplete |
| Scheduled work | durable task store | claim/lease work exists; crash/reconciliation proof remains incomplete |
| Product state | `LifeStateProjection` and later `ProductProjection` | projection failure must remain unknown |

## Threat Model And Control Mapping

| Boundary / asset | Attacker or failure | Required control | Evidence | Status |
| --- | --- | --- | --- | --- |
| Current user message vs quoted/untrusted content | prompt injection tries to authorize an effect | authenticated source kind; deterministic policy; untrusted sources cannot authorize | ADR 0014 negative cases and frozen MEM-05/MEM-06 | `PLANNED` |
| Remote provider request | raw LifeModel, workspace, memory, or sensitive text leaks | typed authorization, bounded context, ContextManifest, capture-to-receipt parity | Direct LocalOnly cloud-isolation test; capture test | `PARTIAL` |
| ReAct context composition | later assembler restores raw messages | field-contribution assembler; Privacy is sole message transformer | production-order and order-invariance tests | `OBSERVED` |
| WebView | malicious remote content or XSS invokes broad filesystem/network commands | strict CSP, bundled release URL, least-privilege capabilities, no recursive AppData access | release config/capability guard and release build | `PARTIAL` |
| Loopback IPC / A2A | unrelated local process calls private task endpoints | no release listener; pairing/auth before body parse and LifeModel read | release compile quarantine plus authenticated development implementation | `PARTIAL`; real binary, persisted-policy, lifecycle, and live capability evidence remain open |
| MCP child process | malicious/hung server, arbitrary interpreter arguments, oversized frame | immutable Rust manifest/digest, async bounded transport, auth for remote MCP | MCP timeout/frame tests; release registration quarantine | `PARTIAL` |
| Network redirects/DNS | SSRF, rebinding, suffix confusion, oversized body | per-hop validation, DNS pin, label matching, private-address deny, limits | `network_client` focused tests | `PARTIAL`; live redirect chain pending |
| Concurrent permission/proposal writes | double consume or effect dispatch | SQL compare-and-swap / atomic claim | focused 100-consumer and claim tests | `PARTIAL`; external unknown reconciliation pending |
| Provider/tool/database/projection divergence | product reports success from planned or stale state | durable minimal receipt; unknown on absent observation; backend projection authority | cancellation/provider tests and frozen PRV-04/ZH-04 | `PARTIAL` |
| Process crash / disk / SQLite busy | half commit, lost task, silent temp fallback | transaction plus outbox, busy policy, explicit degraded/read-only state | fault-injection suite | `PLANNED` |
| Migration failure | partial schema or ignored error | versioned transactional migrations and verified backup | migration fault injection and rollback rehearsal | `PLANNED` |
| Duplicate sensitive storage | content copied into conversation, AgentRun, event, audit, config | canonical content owner; refs/digests elsewhere; keychain secret refs | raw sentinel scan across stores | `PARTIAL` |

Non-goals remain OS administrator compromise, full current-user/keychain
compromise, maliciously re-signed binaries, and universal exactly-once delivery
to peers without idempotency or reconciliation.

## Frozen Baseline

The following repository facts were recorded before the remediation edits:

| Metric | Baseline | Evidence status |
| --- | ---: | --- |
| baseline HEAD | `1ca7613bcd25167cf173fa0a21e3baa908f21d94` | `OBSERVED` |
| openlife-core tests | 615 passed, 1 ignored | `OBSERVED` in original audit summary; raw log was not retained |
| focused single-system gate | 25 passed | `OBSERVED` in original audit summary; raw log was not retained |
| focused runtime-module gate | 26 passed | `OBSERVED` in original audit summary; raw log was not retained |
| full Tauri suite | 394 passed, 1 load-sensitive failure, 3 ignored | `OBSERVED` in original audit summary; raw log was not retained |
| isolated failed stream test | passed in 32.28 seconds | `OBSERVED` in original audit summary |
| strict workspace Clippy | failed | `OBSERVED`; exact warnings must be refreshed before Phase 6 |
| cargo audit | 20 allowed warnings | `OBSERVED`; advisory ownership must be refreshed |
| shipped handler entries | 161 | `SOURCE-REPRODUCED` |
| SQLite files assembled by bootstrap | 17 | `SOURCE-REPRODUCED` |
| ordinary-chat unexpected Proposal rate | `UNKNOWN` | no retained baseline product run |
| executable task success rate | `UNKNOWN` | frozen v1 suite had not been executed at baseline |
| helpfulness median | `UNKNOWN` | no blind pairwise baseline |
| end-to-end first-token / terminal latency | `UNKNOWN` | no retained product timing artifact |
| Receipt-to-network/tool/storage consistency | `UNKNOWN` | formal receipt parity harness did not exist |

Unknown baseline KPIs must not be replaced with fixture results. The first v1
scenario run establishes the candidate measurement and records that historical
baseline comparison is unavailable. The acceptance rule “no more than 0.5
below baseline” remains unevaluable until a blind baseline sample exists and
therefore cannot pass by default.

## Frozen Scenario Contract

- Suite: `openlife-backend-scenarios-v1@2026-07-10`
- Count: 40 unique scenarios in the approved 8/6/6/6/6/4/4 grouping.
- SHA-256: `e969e091777134c62d388c012149c056813ee0c4eb290307c47cf8b439802482`
- Waivers: `openlife_backend_remediation_v4_scenario_waivers.json` (empty at freeze).
- Every group defines seed state, execution steps, observations, cleanup, and
  evaluator. Barrier overrides make concurrency/cancellation cases repeatable.
- Modifying v1 requires a new suite id, retention of old results, and a
  human-approved waiver. Updating the expected digest in a repair commit without
  that waiver is a contract violation.
- The in-repository digest detects accidental drift, but it is not
  tamper-resistant by itself because an implementation author could change the
  suite, digest, and test together. Non-author approval and old-result retention
  must therefore be enforced by the protected review/merge gate, not inferred
  from JSON fields alone.

## Release Quarantine Evidence

Commands executed against the current worktree:

| Command | Result | Claim supported |
| --- | --- | --- |
| `cargo check -p openlife-tauri --release` | pass | default release compiles without dev extensions |
| `cargo check -p openlife-tauri --features dev-extensions` | pass | debug dev surface remains buildable |
| `cargo check -p openlife-tauri --release --features dev-extensions` | expected compile failure | high-risk dev surface cannot enter a non-debug build |
| `cargo test -p openlife-tauri backend_remediation_phase0 -- --nocapture` | pass, 7 tests | document/config/source guards plus backend build-capability projection; not a substitute for artifact proof |
| `env -u OPENLIFE_PROFILE -u OPENLIFE_ENABLE_DEV_A2A -u OPENLIFE_A2A_PAIRED_TOKEN -u OPENLIFE_DATA_DIR -u OPENLIFE_ALLOW_DEV_EXTENSIONS_WITH_CUSTOM_DATA_DIR cargo run -p openlife-a2a-server --bin openlife-a2a-server --features dev-extensions --locked` | expected exit 2 before bind; observed `requires OPENLIFE_PROFILE=dev` | the current standalone development tool fails closed unless the isolated dev profile, explicit enablement, and paired authentication token are present |

The release WebView capability now contains only core, dialog, and text
read/write commands whose paths must be supplied through dialog dynamic scope.
Shell, store, HTTP, and recursive AppData permissions are absent. A2A no longer
auto-starts by default and is no longer a binary target in the Tauri product
package. The separate `tools/openlife-a2a-server` workspace tool requires an
isolated `dev` profile, explicit enablement, a paired authentication token, and
an explicit override before any custom data directory can be used; there is no
unauthenticated opt-in. Release diagnostics return
`a2aStatus=disabled_by_build` without probing a local port and expose backend
build facts for dev extensions/MCP registration. These surfaces earn no
product capability credit.

## Phase Backout Runbook

Backout means reverting the complete bounded phase change, never enabling the
old route as a permanent fallback.

| Phase | Trigger | Backout action | Data compatibility / verification |
| --- | --- | --- | --- |
| 0 | default release exposes a quarantined command, remote URL, A2A listener, or recursive AppData permission | stop release; restore last known safe release artifact; keep unsafe capability disabled | inspect resolved release config, handler, bundle, and open ports before re-release |
| 1 | capture contains raw sentinel, LocalOnly touches cloud, key migration loses a credential, or receipt disagrees with wire | disable external provider route; restore reference-only config backup; revert the whole provider seam change | old canonical conversation data remains valid; verify keychain ref and config contain no plaintext |
| 2 | cancellation permits late commit, terminal state is missing, deadlock occurs, or send/stream diverge | stop new turns; enter explicit degraded/read-only mode; revert runtime phase as one unit | drain/mark active runs interrupted; reconcile any dispatched unknown effect before retry |
| 3 | deterministic policy is bypassed, explicit Memory mutates HS, CAS loses updates, or proposal fatigue breaches gate | disable direct Memory lane; preserve created receipts/proposals; revert policy/control-plane phase | undo reversible Memory commits; never auto-accept or auto-replay pending/unknown effects |
| 4 | SSRF/auth bypass, manifest mismatch, hung extension escape, or tool authority bypass | disable affected network/MCP/A2A capability at manifest authority; revoke grants/tokens | retain audit digest, rotate compromised secret, verify no child/listener remains |
| 5 | migration/parity mismatch, partial canonical commit, outbox loss, scheduler duplicate, or vector corruption | stop writes; restore verified pre-migration backup; reset only the affected asset-category authority flag | rehearse restore and digest parity before reopening; unknown external effects remain quarantined |
| 6 | projection overclaims, old route is reachable, frozen suite regresses, or live trial is red | withdraw completion claim and release candidate; keep last green projection/schema | product displays unknown/degraded, not synthetic success; no data downgrade is attempted |

Before every schema or authority shift, record backup identity, schema version,
asset-category digest, restore command, and rehearsal result. A rollback is not
complete until the focused positive test, counterfactual test, capability
non-regression case, and old-route absence guard all pass against the restored
state.

## Current Phase 0 Exit Decision

`NOT GREEN`.

The release build quarantine, ADR amendment, traceability shape, scenario
freeze, threat mapping, and backout runbook now exist. Remaining blockers are:

1. retained raw reproduction artifacts for every P0/P1 rather than source refs
   alone;
2. a resolved release artifact/port test proving no A2A sidecar is bundled or
   listening;
3. a real WebView negative test against canonical AppData;
4. first execution of the frozen scenario suite and product KPI capture;
5. authenticated A2A binary evidence with persisted privacy policy, bounded
   lifecycle ownership, and real paired capability proof before A2A can regain
   product capability credit.

This fail-closed decision is intentional. Existing Phase 1/2 implementation
work may be audited and repaired, but no later phase may be declared complete
from the current Phase 0 evidence.
