# D063 Frozen Cleanup Contract Adapter v1

> Date: 2026-07-13
> Status: frozen RED baseline retained; first implementation hash rejected;
> root-cause revision awaiting independent review; live native-dialog product
> evidence remains pending
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

- At the frozen baseline, the raw `i64` domain method made the conversion an
  identity and left invalid-value tests intentionally RED.
- In the implementation result below, the same adapter compiles against
  `McpAuditRetentionDays` via `TryFrom<i64>` and exercises the unchanged inputs
  and assertions.

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
production target at baseline commit `8702b70` was RED in five distinct areas:

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

## 2026-07-14 First Implementation Result (Rejected)

The implementation slice now removes baseline defects 1 through 4 without
changing the frozen behavior:

- `McpAuditRetentionDays::try_from(i64)` is the only route from untrusted
  command input to deletion and accepts exactly 1 through 3650;
- `McpAuditStore::cleanup(McpAuditRetentionDays)` owns the only public MCP
  audit deletion path, uses checked subtraction, and the old
  `clear_old_logs` entry is absent;
- the shipped Settings command preserves validation, persistence effects,
  Rust-owned confirmation, then mutation order;
- the old backend command, handler registration, frontend bridge, mock route,
  and page-local dialog are absent; McpPage links to the governed Settings
  workflow instead.

Mechanical evidence on this slice:

- `cargo test -p openlife-tauri d063_ -- --nocapture --test-threads=1`:
  16 passed, 0 failed;
- the D063 McpPage assertions pass; the sole remaining full-file McpPage
  failure belongs to the separately frozen D065 unavailable-projection
  expectation and receives no D063 credit;
- frontend typecheck, frontend format check, Rust format check, and
  `git diff --check` pass.

The full uncommitted package was frozen as
`4875481fb4db1247af0bdca00e954087b0186172fd14b66038805f3d3f96fd21`.
Independent review returned `REQUEST_CHANGES`; the earlier
"contract-green" description is therefore withdrawn. The rejected package had
three defects:

1. persistence effects were checked only before awaiting native confirmation,
   so a later degraded/read-only transition could still reach deletion;
2. cleanup preflight and native confirmation used `affected_count = 0` rather
   than a backend-owned candidate snapshot, while the final command could
   delete rows;
3. invalid retention was converted through the generic `anyhow -> AppError`
   path and surfaced as `Internal` instead of a stable validation/config error.

This rejected hash remains evidence of why the next revision was required. It
receives no acceptance credit.

## D063 Cleanup Contract Adapter v2 Amendment

The frozen scenarios and expected behavior above do not change. The v2 adapter
adds only the mechanics needed to test the newly identified race and truth
boundaries. It is named `d063_cleanup_contract_adapter_v2`; the v1 name and
result above remain historical evidence rather than being silently rewritten:

- orchestration now has a read-only `prepare` port which returns the
  backend-owned candidate count before native confirmation;
- the same prepared count is passed to confirmation and mutation;
- the effects port is callable before preparation and again after confirmation;
- the domain adapter obtains the candidate count for the exact typed cutoff and
  passes it to the single `cleanup` mutation.

New counterfactuals are additive. They prove that effects revoked during the
dialog stop mutation, retention/predicate/count changes invalidate the server
challenge, and candidate drift rolls back inside the delete transaction. No
existing input, expected deletion boundary, risk rule, or rejection behavior
was weakened.

## Root-Cause Revision Evidence (Pending Independent Acceptance)

The revision now:

- captures one exact UTC cutoff in `McpAuditRetentionDays` and uses it for both
  candidate counting and deletion;
- binds retention, predicate version, and backend candidate count into the
  server challenge scope, then binds the exact cutoff into the native prompt
  and single-use grant arguments;
- rechecks global effects after native confirmation and once more after the
  audit-store guard is acquired;
- performs final count comparison plus deletion in one SQLite `IMMEDIATE`
  transaction and rolls back on drift;
- maps missing/out-of-range retention to stable `AppError::Config` reason
  `invalid_mcp_audit_retention_days`;
- makes the Settings workflow request the actual 90-day preflight and display
  the backend candidate snapshot rather than a page-local zero.

Current mechanical evidence:

- `cargo test -p openlife-tauri d063_ -- --nocapture --test-threads=1`:
  21 passed, 0 failed;
- `cargo test -p openlife-core mcp_audit::tests:: -- --nocapture
  --test-threads=1`: 4 passed, 0 failed;
- Rust-owned danger-action authority tests: 8 passed, 0 failed;
- `cargo check -p openlife-tauri --tests`: passed;
- Settings page: 17 passed;
- McpPage: 8 passed and one D065 unavailable-projection test remained RED; it
  receives no D063 credit;
- frontend typecheck, frontend format check, Rust format check, and
  `git diff --check` pass.

The revision is not accepted until its new full diff hash is frozen and an
independent reviewer replays the counterfactuals. It is also not live
native-dialog credit. The shipped command uses a concrete
production `WebviewWindow`, while Tauri's deterministic IPC harness uses
`MockRuntime`; treating a test-only injected port as the shipped native dialog
would be false evidence. Exact packaged-app confirmation, rejection, and
SQLite non-mutation therefore remain a Phase7 product-trial item. Until that
evidence exists, this slice cannot be final product-trial GREEN.
