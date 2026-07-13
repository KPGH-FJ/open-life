# BR4-D068 authenticated MCP audit payload RED matrix

> Status: executable RED contract; production implementation not started
> Scope: test-only fixture seams, real SQLite attack fixtures, and evidence commands
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
| Strict receipt schema table | RED | Wrong role/kind, `payloadStored`, value type, byte type/range, digest, missing and unknown fields all fail. |
| Corrupt current ciphertext table | RED for arguments and result | Authentication failure in either current-format column fails both list and export without a placeholder success. |
| Envelope role/version/column table | RED | Wrong authenticated format version, swapped columns, and column/envelope mismatch fail. |
| Corrupt second legacy row | RED | Authentication failure aborts the whole migration; the first row is not partially rewritten. |
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
