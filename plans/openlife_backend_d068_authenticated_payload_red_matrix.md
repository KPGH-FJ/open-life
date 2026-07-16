# BR4-D068 authenticated MCP audit payload RED matrix

> Status: storage-integration candidate locally GREEN; first frozen review
> findings repaired; independent re-review and D064 identity integration pending
> Scope: frozen scenarios, real SQLite attack fixtures, production codec/storage
> integration, and evidence commands
> Authority: subordinate to `AGENTS.md`, Phase7, and backend remediation v4

## Root-cause boundary

D068 is not a second key-authority or trusted-read finding. D057 remains the
keyring/bootstrap authority owner and D065 remains the composite read/projection
owner. D068 asks a narrower question: can a row prove that each ciphertext is
the expected minimized receipt format and role even when independently mutable
SQLite metadata is changed?

The target uses a versioned authenticated envelope or equivalent AEAD AAD. The
database `payload_minimized_version` column is an index/migration hint, never
the sole format authority. Strict decoding happens before `McpLogEntry` or
`AuditExport` construction.

## Evidence matrix

| Oracle | RED expectation | Target fact |
| --- | --- | --- |
| Legal current product write | GREEN | Exact minimized argument/result receipts remain listable and exportable; raw sentinels remain absent. |
| Valid current fixture | GREEN | The test fixture is forced through the same current envelope encoder; a blanket reject cannot green the negative tests. |
| Source-backed version-zero legacy payload | GREEN | Real historical raw ciphertext in the existing migration format migrates transactionally and remains readable after restart. |
| Legacy 0 -> current 1 column flip | RED for arguments and export/result | Changing only plaintext metadata cannot make legacy ciphertext current or expose raw plaintext. |
| Strict receipt schema table | RED | Both roles reject every missing required field, every required-field type mismatch, wrong role/kind, `payloadStored`, value type, byte range, digest, and unknown fields. |
| Corrupt current ciphertext table | RED for arguments and result | A same-length, valid-Base64 bit flip in either current-format AEAD ciphertext fails both list and export without a placeholder success. |
| Envelope role/version/column table | RED | Independently wrong authenticated roles, wrong envelope or column versions, a matching unsupported envelope/column version, and swapped columns all fail. |
| Corrupt second legacy row | RED | A same-length, valid-Base64 authenticated-ciphertext bit flip aborts the whole migration; the first row is not partially rewritten. |
| SQLite family snapshot | RED on every invalid path | Main, WAL, SHM and rollback-journal presence, bytes and stable metadata remain exact. |
| Real product bootstrap legal current payload | GREEN | The shipped bootstrap keeps both key-reference and audit stores canonical, preserves minimized reads, and performs no credential mutation. |
| Real product bootstrap | RED | A version-flipped row leaves the valid key-reference store healthy and byte-exact, creates/deletes no secret, marks the audit store unavailable, and blocks provider/tool effects. |

The invalid tables collect every accepted case before asserting, so one early
failure cannot hide unexecuted counterfactuals. No source-string scan or test
fixture counts as cryptographic proof.

## Mechanical commands

Run serially in one worktree-local Cargo target:

```sh
cargo test -p openlife-core d068_ -- --nocapture --test-threads=1
cargo test -p openlife-core mcp_audit::tests -- --nocapture --test-threads=1
cargo check -p openlife-core --tests
cargo test -p openlife-tauri d068_ -- --nocapture --test-threads=1
cargo check -p openlife-tauri --tests
cargo fmt --check
git diff --check
```

Before GREEN, the D068 filter must list and execute all named tests. D057 and
D065 must still be reported separately; neither can earn D068 closure credit.

The frozen core distribution is nine tests: three legal controls GREEN and six
target attack groups RED. The Tauri distribution adds one legal bootstrap
control GREEN and one product-boundary attack RED; neither replaces a core
oracle.

## Candidate implementation evidence (2026-07-14)

The scenario inputs, expected outcomes, and nine-plus-two denominator are
unchanged. Fixture mechanics were necessarily revised because a real
authenticated envelope cannot be issued before the row UUID and immutable row
context exist. Current-format adversarial fixtures now use a test-only row
issuer that shares the production UUIDv4, key-epoch, store-binding, row-context,
role, and AAD construction; only the chosen envelope version or receipt bytes
are test-controlled. Legacy fixtures remain on the exact pre-envelope encoder.

The candidate currently proves locally:

- current product writes seal strict minimized receipts in a versioned envelope;
- canonical writes and reads enforce a 512-byte control-free tool identity and
  a bounded RFC3339 timestamp, preventing a 1 MiB MCP frame from becoming an
  unbounded 200-row product projection;
- record identity, tool name, timestamp, and current/legacy ciphertext ceilings
  are enforced by conditional SQLite projection before an
  attacker-controlled `TEXT` value can be materialized as a Rust `String`; the
  codec repeats the current-envelope ceiling as a second boundary;
- UUIDv4 record id, key epoch, immutable row context, role, format header, and a
  store-binding digest are authenticated;
- current rows require the stored record id to equal `Uuid::to_string()` before
  authentication; this canonical-text invariant closes UUID spelling aliases
  that SQLite's binary `TEXT` unique index alone cannot detect, while the full
  index and startup semantic duplicate detection retain their separate guards;
- product key hydration, read-only open, rotation, migration, and write
  boundaries reject epochs above SQLite's signed 64-bit representation before
  any path can silently wrap a `u64` epoch or mutate a database artifact;
- the SQLite payload-version column is compared with authenticated envelope
  truth and cannot promote legacy ciphertext;
- list and export construct product DTOs only after strict authentication and
  schema validation; the old `"[decrypt failed]"` success placeholder is gone;
- existing version-zero raw rows and pre-envelope minimized rows are validated
  before a single `BEGIN IMMEDIATE` migration transaction;
- legacy migration is a bounded two-pass stream: the read-only pass retains
  only row ids after authentication, then the transaction reloads, revalidates,
  and exact-source-CAS migrates one row at a time; it never retains the table's
  old and newly sealed payloads together in memory;
- an invalid current or legacy row fails during a read-only startup preflight,
  before schema recording or payload migration can alter the SQLite family;
- the original frozen core D068 distribution remains 9/9; three additive
  independent-review scenarios are also GREEN, making the candidate filter
  12/12. MCP audit compatibility/security tests
  are 13/13 (including pre-envelope minimized-v1 history, whole-row replay,
  rejection of a weak same-named partial uniqueness index, registry repair
  after payload preflight, fail-closed rejection of a newer schema, and bounded
  product/legacy row metadata plus pre-allocation identity/ciphertext
  projection), and the
  Tauri bootstrap distribution is 2/2.

This is not final D068 closure. The current isolated branch predates D064 and
therefore derives the codec's replaceable store-binding seam from the canonical
SQLite slot. Final integration must consume D064's authenticated random
`store_identity_digest` from the sole reference/database authority; the path
digest must then disappear. Pre-envelope AES-GCM did not bind row metadata,
column role, or cross-row association, so migration cannot retroactively prove
that historical metadata was never rearranged; final D065 projection integration
must surface that retained history as explicit legacy-unverified/degraded
provenance instead of treating the new envelope as historical attestation.
D057 key cutover, D065 trusted-read projection, packaged keychain behavior,
power-loss evidence, and independent frozen-hash review receive no credit from
the local green gates above.

## Independent frozen-review amendment R1 (2026-07-14)

The first frozen-hash review rejected the candidate after mechanically proving
two facts not represented in the original nine-scenario denominator:

1. `Uuid::parse_str` accepts the 32-character simple form, while SQLite's
   binary `TEXT` unique index treats it as distinct from the canonical
   hyphenated form. Copying a complete row and removing the record-id hyphens
   therefore produced two live list/export DTOs authenticated by the same UUID
   binding without dropping the index.
2. key epochs are `u64` in the authority contract but signed `INTEGER` in
   SQLite. An epoch above `i64::MAX` could previously pass hydration and then
   wrap at a write cast, creating an unreadable durable row.

Both failures were reproduced as executable RED before the repair. The
additive tests now prove:

- the noncanonical whole-row replay fails live list/export and restart
  preflight with byte-exact zero rewrite;
- two genuinely distinct canonical UUIDv4 rows remain readable live and after
  restart, preventing a blanket-reject implementation;
- oversized epochs fail before new-database creation, existing-database
  migration, read-only hydration, rotation, or write, and the legal existing
  row remains readable.

These additions do not rewrite the original frozen expectations and earn no
credit for D064 random store identity, historical provenance, D057's complete
keyring cutover, or D065 projection. A new frozen diff hash and independent
re-review are still required.
