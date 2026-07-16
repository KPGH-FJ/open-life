# OpenLife Roadshow V1 Resource Evidence

Status: implementation verified; external live attachment round, native desktop
product trial, and independent read-only review pending. This file does not make
a roadshow release or global backend-remediation completion claim.

## Scope and commits

The V1 implementation is the contiguous resource slice from `dad4775` through
`2508068` on `codex/roadshow-core-recovery`:

- one SQLite `ResourceStore` canonical owner for bytes, metadata, chunks,
  message bindings, import/detach receipts, outbox facts, and tombstones;
- bounded PDF, DOCX, CSV, XLSX, text, Markdown, JSON, and source extraction with
  typed provenance;
- killable parser worker with timeout, cancellation, concurrency, input,
  expanded-output, and process-resource bounds;
- deterministic bounded selection with no vector/model retrieval route;
- request-scoped backend citation ids and canonical citation rendering;
- one Rust-owned native picker and `ResourceRuntime`, with no WebView file path
  input and no release fallback store;
- Main Chat binding through the existing turn operation id, PolicyAuthorization,
  PreparedProviderRequest, provider adapter, and TurnRuntime;
- attachment UI through one `useChatResources` adapter which consumes backend
  receipts and does not own resource completion truth.

## Truth and recovery checks

- Import and detach use UUIDv4 operation identities bound to canonical payloads.
- A lost import response is reconciled through `get_resource_import_status` and
  the canonical stored receipt before the UI displays an attachment.
- A lost detach response keeps the attachment visible and reuses the same
  detach operation identity on retry.
- Import cancellation is distinct from provider cancellation and prevents the
  resource commit guard from admitting a late commit.
- Attachment turns cannot execute the old frontend `/goal` or `/state` mutation
  path; the text and bound resources enter the ordinary TurnRuntime instead.
- Resource-backed streaming is buffered until citation validation. Ordinary
  turns retain their existing token streaming behavior.
- Missing, malformed, forged, or request-external resource citations fail the
  turn instead of producing a verified source footer.
- Removing one message binding preserves a resource still used by another
  message. Removing the last binding deletes bytes/chunks, writes a tombstone,
  and prevents restart resurrection.

## Mechanical evidence

Verified on 2026-07-15 in `/Users/tw/Desktop/open-life-roadshow`:

| Gate | Result | Credit boundary |
| --- | --- | --- |
| `cargo test -p openlife-core resource -- --nocapture` | 15/15 passed | parser, worker, canonical store, replay, cancel, selection, provenance, tombstone |
| `cargo test -p openlife-tauri resource_commands::tests -- --nocapture` | 4/4 passed | active owner/cancel, native-selected file reader, symlink rejection, no semantic-index route |
| resource provider negative test | passed | bound resource context rejects uncited model output |
| resource provider positive test | passed | local captured HTTP request uses issued citation and canonical footer |
| `cargo test -p openlife-tauri main_chat_runtime_module -- --nocapture` | 29/29 passed | one ordinary TurnRuntime and deleted-route guards |
| shipped handler allowlist guard | passed | new resource status command did not restore legacy/dev commands |
| focused Vitest suite | 114/114 passed | IPC aliases, composer, exact turn binding, attach-only send, detach replay, receipt reconciliation, slash-side-effect counterexample |
| frontend typecheck | passed | no TypeScript errors |
| frontend format check | passed | all frontend source matched Prettier |
| `cargo check -p openlife-tauri --tests` | passed with two pre-existing core warning groups | no warning-free claim |

The positive provider evidence uses a real local HTTP capture server. It proves
the request/receipt/citation boundary but is not external live-provider credit.
The frontend tests mock Tauri IPC and are not native-picker product-trial credit.

## Review boundary

The autonomous source/diff review found and fixed two counterexamples before
this evidence was recorded: attachment text could still reach the old slash
side-effect helper, and lost IPC responses could not be reconciled. A separate
independent read-only source and evidence review was not run and is not
credited.

## Remaining V1 evidence

- native desktop picker/import/remove/restart product trial with the frozen
  PDF+DOCX and CSV+XLSX fixtures;
- external live-provider attachment answer with validated citations;
- independent read-only source and evidence review;
- cumulative RC-02/RC-03 loops, concurrency/fault runs, and soak evidence.

Until those finish, V1 is `implementation_verified_live_trial_pending`, not
`slice_provisionally_green`, and the roadshow release remains NO-GO.
