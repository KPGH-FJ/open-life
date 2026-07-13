# D063 Frozen Cleanup Contract Adapter v1

> Date: 2026-07-13
> Status: active RED-contract compatibility note
> Scope: test-interface mechanics only; no scenario, expected behavior, risk,
> retention bound, or production completion waiver

## Frozen Behavior

The D063 rubric is unchanged:

- retention is valid only from 1 through
  `openlife_core::mcp_audit::MCP_AUDIT_RETENTION_MAX_DAYS` (3650), inclusive;
- invalid raw values return an error before SQL, never panic, and preserve the
  exact SQLite rows;
- a valid retention cutoff is `now - retention`, deletes only rows strictly
  before that cutoff, and preserves rows just inside it;
- validation precedes the persistence effects gate, native confirmation, and
  the one domain mutation;
- missing or invalid native authority and degraded persistence perform zero
  mutation;
- the old page-local command, bridge, and public domain mutation entry are
  absent when D063 becomes GREEN.

## Adapter Contract

`backend_remediation_d063_tests.rs` owns
`d063_cleanup_contract_adapter_v1`. It accepts the untrusted raw `i64` used by
the shipped command and lets Rust infer the argument expected by
`McpAuditStore::cleanup` through `TryInto`.

- With the current raw `i64` domain method, the conversion is the identity and
  invalid-value tests remain intentionally RED.
- With the target `McpAuditRetentionDays` method, the same adapter compiles via
  `TryFrom<i64>` and exercises the same frozen inputs and assertions.

No test expectation or scenario changes during that type migration. If a
future Rust interface makes a mechanical adapter change unavoidable, create a
new versioned note and retain this v1 result; do not edit the expected behavior
above to obtain a green run.

## Evidence Boundary

`orchestrate_mcp_audit_cleanup` is a production-called seam whose injected
ports permit dynamic order and zero-mutation tests. Those tests prove the seam
semantics only. The shipped command already binds the real
`PersistenceCoordinator`, Rust-owned confirmation, and audit store to this
private seam, but source binding is not dynamic product-command evidence. The
production target remains RED in five distinct areas:

1. both the core cleanup boundary and shipped validator still accept raw
   `i64`; the core lacks `McpAuditRetentionDays`, and the command still uses an
   identity validator instead of converting the raw value and mapping the core
   error into `AppError`;
2. the former `clear_mcp_audit_logs` command and shipped handler registration
   remain;
3. the frontend bridge and McpPage page-local cleanup route remain;
4. the public `McpAuditStore::clear_old_logs` second mutation entry remains;
5. no concrete Tauri IPC/native-confirmation/database test yet proves rejected
   product commands perform zero audit mutation.

Accordingly, injected-port tests receive orchestration-seam credit only. They
do not receive concrete Tauri IPC, native-dialog, or end-to-end
product-command rejection credit.

The static Rust guard is deliberately mechanical rather than a second name or
type resolver. Shipped MCP audit imports must remain direct and unaliased;
audit handler entries must remain bare identifiers without cfg alternates.
Rename, glob, module-alias, multi-segment handler, and conflicting cfg forms
fail closed, as do inline command/domain modules that would escape the flat
source contract. The bounded mutation guard identifies the real audit DELETE
SQL owner and its current one-hop inherent callers, including method syntax and
explicit `Self::method` or `McpAuditStore::method` UFCS. It claims no deeper
interprocedural, trait, deref, alias, or type-inference coverage; the real
SQLite boundary tests remain the behavioral authority.

The separate native-grant authority test proves exact scope and single-use
behavior inside the Rust-owned authority. It does not by itself prove audit
database non-mutation after a rejected product command.
