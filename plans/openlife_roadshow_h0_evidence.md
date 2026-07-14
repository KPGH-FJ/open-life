# OpenLife Roadshow H0 Facts And Freeze Evidence

> Date: 2026-07-14
> Baseline: `6704d6d1d56c26600b47f1bff68fe4ac743dbd96`
> Plan commit: `a4a0a49347f9d8fcac98e7f874550aa499cfe900`
> Decision: development may proceed; the roadshow candidate is not ready
> Completion boundary: H0 fact discovery only, not H1, Phase7, or Backend
> Remediation v4 completion

## 1. H0 Decision

H0 exits with a **development GO** and a **roadshow release NO-GO**.

Every required H0 question now has an observed outcome. Several outcomes are
failures rather than passes:

- external DirectAnswer and exact-prompt provider token streaming work;
- the live Web path is not provider-backed and can misclassify a challenge page
  as successful tool completion;
- ordinary frozen-scenario execution fails at `ORD-03` on both the original
  baseline and the roadshow worktree;
- ResourceStore/ResourceGateway and StateStore/StateAsset do not exist in
  product code;
- roadshow quarantine is incomplete because Scheduler, vector maintenance, and
  MCP read/product surfaces remain reachable in a default release build;
- the active user configuration still contains a legacy plaintext provider
  credential and has not completed the real keychain migration smoke;
- safe document libraries are viable, but an async timeout cannot terminate a
  blocking parser. Production parsing requires an isolated, killable worker.

The earlier 34-36 hour estimate is therefore rejected. The revised estimate
for the complete hard-gate and reliability contract is P50 40-48 hours and P80
56-72 hours. A narrower demo may appear earlier, but it receives no roadshow
GREEN credit until the frozen gates pass.

No product code change made while investigating H0 receives H0 completion
credit. Provider/network/event changes remain uncommitted H1/V2 candidates and
must be reviewed, tested, and committed separately.

## 2. Current Source Map

| Concern | Current product path and owner | H0 finding |
| --- | --- | --- |
| buffered send | `frontend/src/tauri.ts` -> `src-tauri/src/lib.rs` -> `main_chat_send.rs::send_message_with_operation_state` -> `OpenLifeTurnRuntime::run_buffered` | shared runtime owner exists |
| streaming send | `frontend/src/tauri.ts` -> `src-tauri/src/lib.rs` -> `main_chat_streaming.rs::start_stream_message_with_operation_state` -> `OpenLifeTurnRuntime::run_streaming` | shared runtime owner exists; exact external DirectAnswer produced provider-bound chunks |
| turn runtime | `src-tauri/src/main_chat_turn_runtime.rs::OpenLifeTurnRuntime` -> `main_chat_kernel.rs` -> `openlife-core/src/agent/main_chat_agent_v1.rs` | ordinary frozen run still fails at `ORD-03` |
| provider preparation | `main_chat_kernel.rs` -> `openlife-core/src/scheduler.rs` -> `PreparedProviderRequest` / `ContextManifest` in `openlife-core/src/llm.rs` | bounded prepared seam exists; `ChatMessage` remains string-only |
| provider truth | HTTP adapter start observer -> `MainChatKernelEvent` -> `main_chat_turn_runtime.rs` -> `MainChatAgentEventStore` | external DirectAnswer durable start/completion parity passed after the system-proxy root fix |
| tool execution | Main Chat ReAct selection/runtime -> `openlife_core::agent::ToolGateway` -> typed action executor | one product ToolGateway exists; live Web synthesis bypasses provider generation |
| current file read | Main Chat context loader and builtin `file.read`, limited to configured safe/workspace paths and UTF-8 text | not an upload/import or canonical resource system |
| imported resources | no `ResourceGateway`, `ResourceStore`, resource IPC contract, attachment DTO, or canonical resource schema | V1 not started |
| transient state | React `ChatPage` quick-command parsing -> `add_daily_goal` / `toggle_daily_goal` -> YAML/LifeModel write gateway | frontend still owns mutation intent and success flow; ADR 0015 not implemented |
| StateStore | only ADR 0015 target contract | V3 not started |
| artifact proposal | `file.write_proposal` -> durable `ExternalWriteAction` Proposal -> `accept_proposal_with_state` -> `safe_write_utf8` | acceptance-to-materialization works for bounded UTF-8 content inside safe paths |
| artifact lifecycle truth | Proposal store plus file bytes; no complete artifact effect/receipt/reconciliation projection | V4 partial and BR4-D020 remains open |

`src-tauri/src/lib.rs` owns command wiring, not Main Chat execution.

## 3. Provider, Web, And Streaming Probes

### External provider

`main_chat_live_provider_eval_harness_invokes_external_direct_answer_when_opted_in`
passed against the configured external provider. Same-task durable events
contained ordered `provider.started` and `provider.completed`, and the harness
credits invocation from those events rather than transcript prose.

The observed pre-dispatch failure was caused by the macOS system proxy's
RFC2544 fake-IP route. The proposed compatibility fix is constrained to a
hostname whose entire resolution set is fake-IP space plus an explicitly
configured loopback HTTP proxy; literal private targets, mixed DNS answers, and
non-loopback proxies remain blocked. This is an H1 candidate, not H0 closure.

The current user configuration still holds a legacy plaintext credential.
Evidence and commands deliberately record presence only; no credential value is
stored in this package. The existing `secret_store` migration code and unit
tests do not substitute for a real OS-keychain migration/restart smoke. The
credential should be rotated after migration because it was previously present
on disk.

### True streaming

`main_chat_live_provider_stream_command_surface_emits_external_provider_tokens_when_opted_in`
passed with the exact DirectAnswer prompt:

- 41 `stream-message-chunk` events carried a non-empty provider `request_id`;
- durable `provider.started` preceded durable `provider.completed`;
- `stream-message-done` was the final event;
- observed command time was 1.706 seconds;
- status was `completed`, `model_invoked` was true, and blockers were empty.

A different prompt completed in 193 ms with only one compatibility chunk and no
provider request id. That run is not token-streaming evidence. The gate credits
only chunks bound to the provider request.

### Live Web

`main_chat_live_provider_stream_command_surface_invokes_external_step6_web_when_opted_in`
is a hard failure:

- `web.search` executed through ToolGateway;
- selected generation path was local read-tool synthesis;
- scheduler/provider generation was not called;
- the HTTP response was a DuckDuckGo challenge page;
- no structured results were parsed;
- action and turn were nevertheless projected as completed.

The current test name and intended claim exceed the implementation. The test
expectation remains frozen. V2 must add provider-backed selection/synthesis,
challenge/no-result detection, typed failure, and source attribution.

## 4. Document Parser Spike

The spike used only MIT-compatible dependencies:

| Crate | Version | License | Decision |
| --- | --- | --- | --- |
| `lopdf` | 0.44.0 | MIT | admit for bounded text PDF extraction |
| `calamine` | 0.36.0 | MIT | admit for bounded XLSX extraction |
| `zip` | 8.6.0 | MIT | admit with entry, expanded-byte, and ratio limits |
| `quick-xml` | 0.41.0 | MIT | admit with DTD/entity and external-relationship rejection |
| `csv` | 1.4.0 | Unlicense/MIT | admit for bounded CSV extraction |
| `infer` | 0.19.0 | MIT | admit as a magic-byte input, not the only format validator |

Positive sentinels proved:

- PDF page 1 and page 2 remain distinguishable;
- DOCX paragraph ordinals remain distinguishable;
- CSV range `A3:C3` remains distinguishable;
- XLSX sheet `roadshow_metrics` and sentinel cell `R3C1` remain distinguishable.

Negative probes rejected wrong MIME, corrupt OOXML, external DOCX
relationships, excessive ZIP expansion, and encrypted PDF input. On the small
frozen fixture set, the parser process used 0.02 seconds wall time and
7,815,168 bytes maximum resident set size.

The spike also disproved the proposed timeout implementation: wrapping blocking
parsing in `tokio::time::timeout` cannot preempt the blocking code. V1 must run
parsers in a child process or equivalent killable isolation with a global
concurrency cap, wall-time kill, input/expanded-byte caps, and OS resource
limits. A detached `spawn_blocking` task is insufficient because timed-out work
continues consuming resources.

### Conditional gate decisions

- image understanding: **NO-GO**; `ChatMessage` and frontend message contracts
  contain only string content and no typed multimodal/privacy path;
- OCR/scanned PDF: **NO-GO**; no bounded, killable OCR worker or live evidence;
- PDF output: **NO-GO**; no governed materializer plus render verification.

These capabilities must remain absent from the roadshow claim and UI.

## 5. Frozen Fixtures

The suite is now `frozen_h0_2026_07_14`. Its files live under
`plans/fixtures/openlife_roadshow_core`, and
`scripts/generate-roadshow-core-fixtures.py` regenerates identical bytes using
fixed ZIP metadata. A double generation followed by SHA-256 validation passed.

The authoritative filenames and digests are stored beside every scenario in
`plans/openlife_roadshow_core_capability_scenarios.json`. The parser spike was
rerun against the frozen PDF, DOCX, CSV, and XLSX files and passed all
provenance sentinels.

After this freeze, expected outcomes or fixture bytes require a versioned
waiver that retains the previous result.

## 6. Artifact Materialization Probe

The following focused tests passed:

- `accept_external_write_action_writes_file_to_safe_path`;
- `accept_external_write_action_blocks_outside_safe_paths`;
- `proposal_accepts_hs_external_write_payload_and_verifies_hash`.

Observed behavior is narrower than V4 completion:

- accepted UTF-8 content is written inside an existing safe parent;
- the implementation uses a temporary file, `sync_all`, and rename;
- target and parent symlink defenses exist;
- an outside-safe-path proposal does not write a file;
- Proposal accepted status is observable.

Still missing for the roadshow artifact lane are canonical effect status,
operation/payload replay binding, crash reconciliation, final digest parity,
projection/UI truth, restart recovery, and confirmed/failed/unknown separation.

## 7. StateStore Dependency Map

There is no reusable StateStore implementation. V3 must introduce one canonical
SQLite owner with this dependency order:

```text
current authenticated message + UUIDv4 operation
  -> IntentFrame / PolicyDecision
  -> StateGateway validation
  -> StateStore transaction
       state_asset + version/CAS
       operation_id + payload_digest uniqueness
       minimal execution receipt
       outbox row
  -> idempotent Task/LifeState projection
  -> YAML compatibility materialization after canonical commit
  -> backend read model
  -> Chat/Today UI
```

Required migration inputs are the existing YAML daily-goal/state shapes and
their direct Tauri commands. Required deletions are React mutating quick-command
authority, frontend-authored success prose, unkeyed mutations, and independent
YAML product writes. Scheduler is not a dependency of this lane.

## 8. ResourceStore Dependency Map

There is no attachment or imported-resource product seam. V1 requires:

```text
dialog selection / import IPC
  -> ResourceGateway
  -> killable parser worker
  -> canonical SQLite BLOB + metadata + digest + provenance + tombstone
  -> deterministic bounded selector (no VectorStore)
  -> BoundedContextBlock + ContextManifest
  -> PreparedProviderRequest
  -> validated ResourceCitation projection
```

`PreparedProviderRequest`, `BoundedContextBlock`, and `ContextManifest` can be
reused. `ChatMessage`, the frontend DTO, ResourceGateway/Store, parser process,
attachment IPC, citations, cleanup, and lifecycle persistence must be added.

## 9. Quarantine Proof

| Surface | Default release observation | Roadshow decision |
| --- | --- | --- |
| arbitrary `execute_tool_call` | compiled and wired only with `dev-extensions`; default features are empty | quarantined |
| arbitrary MCP registration | compiled and wired only with `dev-extensions` | quarantined |
| A2A commands and autostart | compiled/wired only with `dev-extensions`; autostart also needs explicit authenticated opt-in | quarantined |
| MCP list/audit/recommendation surfaces | still shipped in the default handler and frontend bridge | not quarantined |
| Scheduler runner and settings | runner starts unconditionally and settings commands are shipped | not quarantined |
| vector maintenance/product surfaces | startup maintenance and memory/vector commands remain shipped | not quarantined |
| attachment VectorStore use | no attachment route exists | absent, but not positive capability evidence |

H0 therefore records quarantine as failed. H1/V1 integration must remove or
compile-gate the remaining roadshow product surfaces while preserving required
Memory behavior.

## 10. Baseline And Build Evidence

- `cargo fetch --locked`: passed in approximately 1.11 seconds;
- first Tauri `--no-run` warmup in the fixed target directory: approximately
  5 minutes 11 seconds;
- subsequent focused builds reuse
  `/Users/tw/Desktop/open-life-roadshow/target`;
- two file-materialization tests passed in 0.10 seconds after build;
- HS payload materialization passed in 0.07 seconds after build;
- external DirectAnswer streaming passed in 1.91 seconds test time and 1.706
  seconds observed command time.

The eight ordinary frozen scenarios are not a green capability baseline.
`ORD-01` and `ORD-02` reached their zero-Proposal assertions, then `ORD-03`
failed with `turn_final_tool_receipt_owner_missing`. The same failure occurred
at the untouched baseline commit, so it is not caused by the roadshow
provider/network changes. Current measurable Proposal fatigue is therefore
zero for 2 observed ordinary scenarios; the full 8-scenario rate is unknown,
and ordinary capability completion is 0/8 at the suite level.

## 11. H1/V1/V2/V3/V4 Entry Blockers

1. repair `ORD-03` by tracing the missing canonical tool-receipt owner;
2. review and commit provider/system-proxy truth without broadening SSRF access;
3. keep the retry-safe first immutable `tool.started` event and prove terminal
   receipt parity;
4. make live Web provider-backed and treat challenge/no-result as typed failure;
5. compile-gate or remove remaining roadshow MCP/Scheduler/vector surfaces;
6. implement the killable resource parser process and ResourceStore lane;
7. implement StateStore before deleting frontend quick-command mutation;
8. complete artifact effect/receipt/reconciliation truth;
9. execute real keychain migration/restart verification and rotate the legacy
   credential;
10. rerun cumulative scenarios and live product trials without changing the
    frozen expectations.

H0's development GO authorizes these slices in the approved order. It does not
authorize a readiness claim.
