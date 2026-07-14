# D065 Typed Audit Projection Contract Waiver v2

> Date: 2026-07-14
> Status: proposed; pending explicit human review
> Scope: BR4-D065 local IPC and product-projection contract only
> Proposes to supersede the rejected v1 array response shape for future D065
> implementation evidence only if explicitly accepted. It does not erase or
> rewrite any v1 result and has no authority while review is pending.

## Frozen v1 evidence retained

- Rejected v1 packet diff SHA-256:
  `f823f904ee191060f325aa0d20ce0aac75f5d57bde5b2b0fb4f7410181f53fb7`.
  This digest binds only the old packet reviewed as `REQUEST_CHANGES`; it must
  not be reused as the v2 digest.
- The v1 focused Rust run reported 34 of 34 selected D065 tests passing after
  implementation. The earlier RED baseline remains separately recorded as 34
  selected tests: 14 surrounding controls passing and 20 D065 target tests RED.
- The v1 focused UI run reported 18 passing tests plus 3 intentional BR4-D063
  RED tests. Those three D063 failures are not D065 failures and receive no
  D065 or D063 closure credit here.
- The independent source review decision for the v1 packet was
  `REQUEST_CHANGES`. Green mechanics did not override that review.

## Rejected v2 packet retained

- Rejected v2 packet SHA-256 (tracked diff plus this then-untracked waiver):
  `a01f4522eeb703fd4a1c1b31ea035962cd60c9f12bd657e2d4b80df0ce7f4c1e`.
  This digest remains bound to the reviewed packet and cannot be reused for a
  revised v2 packet.
- The independent review decision was `REQUEST_CHANGES`. A never-settling
  audit-list promise was joined in the same `Promise.all` as MCP server, tool,
  template, and privacy reads, so one hung audit read indefinitely withheld
  unrelated product facts and left the page loading. Recommendation fetching
  was coupled to that same core refresh boundary.
- The revision must prove that audit and recommendation settlement are
  independent of core MCP facts, that stale generations cannot overwrite a
  newer refresh, and that unmount cannot apply late results. This review
  finding is retained even if the revised mechanics become green.

## Rejected v2 revision packet retained

- Rejected revised-v2 packet SHA-256 (tracked diff plus this then-untracked
  waiver):
  `28123a47c9b6c996648f90cca7808e0268679d64ac50e4a776beb8a9d6e55efe`.
  This digest remains bound to the independently reviewed packet and cannot be
  reused for the next revision.
- The independent decision remained `REQUEST_CHANGES`. Its focused D065 Rust
  run reported 34 of 37 selected tests passing and three D065 failures. The
  single raw store read had made an implementation-count guard stale, while
  export gate errors had dropped the frozen outer
  `persistence_store_unavailable` authority marker.
- That packet also added a D065 expectation that the BR4-D063 legacy McpPage
  cleanup button remain present but disabled, contradicting the already-frozen
  D063 absence contract. D065 must not deepen or preserve that old route.
- Finally, the page still labeled minimized `payloadStored:false` receipts as
  original request parameters and execution results, and its default mock
  supplied raw path, content and result payloads. This violated product-truth
  consistency even though BR4-D068 authenticated-envelope work remains
  separately open.

## Rejected post-P1 revision packet retained

- Rejected tracked diff SHA-256:
  `fc0ed1db87d7eae2fa0ab20fa85cd2d8c95536d8b34c72a8c172db8f000b2b46`.
- Rejected tracked diff plus this then-untracked waiver SHA-256:
  `08871ae5a627c2caf3ea4a219986318bdd5eeab686de4eb9aec6f308d2ac807d`.
  Both digests remain bound to the independently reviewed packet and cannot
  be reused for the next revision.
- The independent decision was `REQUEST_CHANGES`. The packet correctly made
  recommendation settlement independent of core MCP facts, but collapsed a
  rejected `recommend_mcp_manifests` transport into `recommended=[]`. The page
  then displayed a trusted-empty message and invented missing goal/capability
  data as the cause. Unknown recommendation truth therefore became a false
  empty success.
- The next revision must distinguish checking, successful empty/non-empty, and
  unknown recommendation states. Older generation results or errors and all
  post-unmount settlements must not overwrite the current state. This finding
  remains in the audit trail even after those mechanics become green.

## Why a local contract revision is required

The v1 shipped `list_mcp_audit_logs` response was a bare array. A bare empty
array cannot distinguish a trusted empty canonical audit store from any of:

- the key-reference owner being unavailable;
- the audit database owner being unavailable;
- a query, decryption, or schema read being unknown;
- one or both canonical owners being verified but read-only degraded.

The v1 diagnostics fields also allowed contradictory combinations such as
`available` with null counts, or `unknown` with apparently exact zero counts.
Changing error prose or teaching the page to infer status would preserve the
same root defect.

## v2 contract change

The existing `list_mcp_audit_logs` command is retained as the only product
command. No second command, route, adapter fallback, or array compatibility
path is introduced.

The MCP audit read gateway owns one discriminated projection contract used by
both the list command and diagnostics:

- `available` carries exact bounded facts;
- `degraded` carries exact bounded facts plus a typed `reasonCode`;
- `unavailable` carries only a typed `reasonCode`;
- `unknown` carries only a typed `reasonCode`.

For the list command, successful facts contain `entries`. For diagnostics,
successful facts contain exact counts. The two failure variants cannot contain
entries or counts. WebView list limits are validated as `1..=200`; zero and
over-limit requests are explicit invalid requests and are never silently
truncated.

The existing Settings export remains a separate bounded product operation, not
a second audit-read authority. Its raw day input is validated as `1..=3650`
before the gateway, SQLite, or confirmation path. In one SQLite read snapshot,
the store records the canonical maximum row id, scans at most the newest 10,001
canonical rows by id, and checks whether older unscanned rows remain. Every
scanned `created_at` is parsed by one strict RFC3339 parser and compared to the
exact `DateTime` cutoff without microsecond truncation. No derived timestamp
column, schema-v4 claim, or second selector truth is introduced.

The serialized result carries inverse `complete` and `truncated` booleans plus
a typed `incomplete_reason`:

- `complete:true` is possible only when the snapshot is exhausted and at most
  10,000 scanned rows are eligible for the requested window;
- `scan_limit` means older canonical rows remain unscanned, even when fewer than
  10,000 entries were returned;
- `entry_limit` means the snapshot was exhausted but more than 10,000 scanned
  rows were eligible;
- `scan_and_entry_limit` means both limits apply.

The WebView accepts 201 through 10,000 valid export entries independently of
the list command's 200-row ceiling. It rejects over-ceiling responses,
entry-count mismatches, impossible completeness combinations, malformed
metadata, raw payloads, and structurally invalid minimized receipts. The saved
JSON retains all completeness fields. A partial result uses the explicit
`openlife-mcp-audit-incomplete.json` default name and warning; only a complete
result receives the canonical filename and complete wording.

SQLite query and decrypt work runs on the blocking pool, but admission is
serialized by one shared semaphore permit acquired before `spawn_blocking`.
The owned permit moves into the worker, so cancellation of the async caller
cannot admit another worker while the detached read is still running. Gate
closure, operation failure, and worker join failure remain fail-closed. This
prevents concurrent IPC from occupying multiple blocking-pool workers that
would only wait on the same canonical store mutex.

This strengthens the frozen semantics: trusted empty still means exact zero;
trusted non-empty remains exact and bounded; degraded stays readable; and an
untrusted or unobserved read cannot become an empty success. No expected safety
behavior is removed or weakened to obtain a green test.

## Evidence and approval boundary

- v1 source, digest, RED baseline, v1 mechanical results, and independent
  `REQUEST_CHANGES` decision remain part of the audit trail.
- v2 tests may update the IPC input/output shape only to assert the stronger
  discriminated semantics. Existing composite-owner, decrypt failure,
  unrelated-store, read-only, mutation-absence, and capability non-regression
  meanings must remain.
- The authority guard now expects exactly one raw audit-store `list_logs` call
  inside the gateway because diagnostics and list share `read_log_facts`.
  Command, diagnostics and Settings source scans still forbid raw store reads.
  This is stricter single-owner convergence, not a relaxed absence check.
- Mechanical counterexamples cover an exact 10,000-row complete export, an
  exhausted 10,001-row export with `entry_limit`, eligible rows hidden below a
  10,001-row scan ceiling with `scan_limit`, 201 valid WebView export entries,
  and a rejected 10,001-entry WebView response. The export test records
  statement counters separately for the snapshot `MAX`, bounded candidate
  scan, older-row `EXISTS`, and post-scan `MAX`; adding 20,000 unscanned rows
  does not create linear fullscan or VM work across the complete selector path.
  A WAL barrier inserts and commits a row between the candidate scan and
  completeness probe and proves the export remains bound to its original
  snapshot. Exact offset, sub-microsecond cutoff, malformed timestamp, and
  extreme legal-year cases prevent lossy or permissive timestamp claims.
  Concurrency tests also cover bounded worker entry, async-runtime heartbeat
  progress, caller cancellation without early permit release, a closed gate,
  and a panicking worker.
- This local waiver does not modify the frozen 40-scenario suite or the global
  human-approved scenario waiver registry.
- This document is proposed evidence, not accepted frozen-eval credit. Final
  v2 credit remains pending explicit human review.
- Former McpPage cleanup authority is absent: the page-local button, dialog,
  frontend bridge wrapper, and direct call were removed, and the page now links
  to governed Privacy settings. The backend cleanup command remains reachable
  only through that Settings preflight/confirmation workflow. The D063 absence
  guard is supporting evidence, but this D065 waiver itself grants zero D063
  closure credit; D063 status remains owned by its independent acceptance
  decision. In particular, the current cleanup SQL compares RFC3339 `TEXT`
  values lexically; a mechanical offset counterexample proves this can delete a
  chronologically newer row. That D063 root defect remains explicitly open.

## Explicitly unresolved dependencies

BR4-D057 must still provide an exact authenticated manifest-generation receipt,
and BR4-D064 must still bind the retained database identity/owner generation so
the read cannot cross an authority or SQLite identity transition. D065's
pre/post coordinator comparison is only an interim fail-closed check; it is not
that proof and earns zero D057/D064 closure credit.

D065 therefore remains `PARTIAL` until the exact generation and database
identity dependency is integrated and independently verified. This waiver
cannot be used to claim Phase7, backend remediation, or live product-trial
completion.

BR4-D068 also remains unresolved. This revision rejects a negative or
keyring-uncovered persisted `key_epoch` at product read time without rewriting
SQLite, but it does not introduce the authenticated payload envelope, schema
migration, legacy-payload cutover, or role/version binding required by D068.
The legacy migration decoder and schema-v3 ownership are intentionally
unchanged. D065 does not claim schema v4 and must compose with D068's future
authenticated-envelope migration. This work earns zero D068 closure credit.

The WebView decoder additionally treats the minimized receipt JSON as a
bounded, typed display contract: it checks exact fields and roles, rejects
stored-payload claims, and accepts only canonical SHA-256 standard-Base64-no-pad
syntax emitted by the Rust store. It does **not** recompute that digest from a
payload (the payload is deliberately absent), authenticate the receipt, bind a
row/version/role with AEAD additional data, or prove ciphertext integrity.
Those are D068 responsibilities, so this structural hardening earns zero AEAD,
authentication, migration, or D068 closure credit.
