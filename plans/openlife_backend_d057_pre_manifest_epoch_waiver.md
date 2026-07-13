# D057 pre-manifest epoch proof waiver v1

> Waiver ID: `BR4-D057-PERF-001`
> Status: proposed; accepted by independent engineering review
> Acceptance boundary: not separate human/product acceptance; Phase7 GREEN remains blocked
> Date: 2026-07-13
> Scope: D057 MCP audit key-to-epoch completeness and startup complexity only
> Frozen 40-scenario suite: unchanged

## Why this waiver exists

The experimental test
`d057_current_v3_preflight_is_once_per_database_identity_and_bounded_by_epochs`
required the first startup of an existing current-v3 `mcp_log` database to
prove its complete epoch set with work bounded by epoch cardinality rather than
row count. That expectation is not implementable for the pre-manifest schema.

Current-v3 has per-row `key_epoch` values and a sidecar key-reference list, but
it has no authenticated database-owned epoch manifest or trusted epoch index.
For any algorithm that observes fewer than all legacy row epoch values, choose
one unobserved row and change only its epoch to an uncovered value. The observed
database state and keyring are identical in the covered and uncovered cases, so
the algorithm returns the same answer for both and must be wrong for one. An
exact first proof therefore has an `O(N)` lower bound unless a previously
authenticated summary covering the row set already exists.

None of the following removes that lower bound:

- inode, file-owner, or writer-lease evidence proves identity/concurrency, not
  database contents;
- SQLite `quick_check` proves structural consistency, not key-to-epoch binding;
- one ciphertext sample per known epoch cannot discover an unobserved epoch;
- the sidecar keyring does not authenticate the epoch set stored in SQLite;
- `SELECT DISTINCT key_epoch` without a trusted index scans the table, while
  building the first exact index also requires `O(N)` work.

## Retained old expectation and result

This correction does not erase the old result.

- Rejected experimental lineage: `ff2c12b` through `f6827ca`.
- Command rerun on 2026-07-13:
  `cargo test -p openlife-tauri d057_current_v3_preflight_is_once_per_database_identity_and_bounded_by_epochs -- --nocapture --test-threads=1`.
- Original expectation: one key preflight, one inspection preflight, one
  read-only reader, zero `quick_check`, and comparable SQLite work for 32 and
  2,048 current-v3 rows.
- Original result: **RED**.
- 32-row observation: two key preflights, two inspection preflights, four
  readers, four `quick_check` calls, 155 full-scan steps, and 4,141 VM steps.
- 2,048-row observation: two key preflights, two inspection preflights, four
  readers, four `quick_check` calls, 10,235 full-scan steps, and 219,853 VM
  steps.

The old implementation was inefficient, but optimizing those duplicate reads
cannot make the stronger first-start `O(epoch)` claim true. Sampling or omitting
the complete epoch discovery would only manufacture a false GREEN.

The rejected production WIP is not part of this waiver branch and must not be
revived to satisfy the old expectation.

## Corrected contract

The corrected contract separates migration proof from steady-state proof.

1. **Pre-manifest current-v3 first open**
   - perform exactly one complete epoch-discovery pass over the legacy row set;
   - treat this as explicit one-time `O(N)` migration work, not ordinary startup;
   - reject missing key material, invalid epochs, concurrent replacement, or an
     interrupted/ambiguous install without activating writable product truth;
   - install authenticated store metadata and its epoch set in the same
     transaction as the migration marker; no sample-based completion credit.

2. **Post-manifest same-identity restart**
   - authenticate the installed metadata before trusting its epoch set;
   - require key-reference coverage for every authenticated epoch;
   - perform work bounded by epoch cardinality, with no `mcp_log` full scan or
     full-database `quick_check` used as key-authority proof;
   - a missing, corrupt, stale, identity-mismatched, or unsupported manifest is
     recovery-required/unknown, never silently healthy.

3. **Row-level payload truth remains separate**
   - D057 proves key-reference coverage for the authenticated legitimate-write
     epoch set; it does not claim the manifest eagerly authenticated every
     ciphertext or proves that every current row is unchanged;
   - D068 binds the expected payload role and format version into AEAD and
     strictly validates the minimized receipt after decryption;
   - D065/D068 product read gateways lazily fail closed when key selection,
     authenticated role/version, decryption, or receipt-schema truth is invalid;
   - `Proposal`, placeholder text, or a sampled row cannot substitute for a
     failed read.

## Required future evidence

GREEN requires two distinct executable proofs, both using runtime SQLite
counters rather than wall-clock timing:

- a pre-manifest fixture shows one complete `O(N)` epoch discovery and one
  atomic authenticated-metadata installation, with crash/fault injection and
  zero partial authority;
- the next same-identity restart shows authenticated metadata verification and
  key coverage bounded by epoch count, with zero `mcp_log` full-scan steps.

Counterfactuals must cover an uncovered legacy epoch, corrupt manifest MAC,
unsupported manifest version, database replacement, interrupted installation,
and a row-level AEAD failure after an otherwise valid manifest restart.

The mechanical lower-bound test
`d057_pre_manifest_epoch_discovery_has_linear_work_without_authenticated_metadata`
retains the pre-manifest fact. It is not product GREEN evidence for the future
manifest implementation.

## Approval and integration boundary

This document corrects an experimental unit/performance oracle; it does not
modify the frozen 40 user scenarios or their digest, so the scenario-waiver
registry remains unchanged. The correction was requested after independent
anti-hallucination review. Integration must retain this document and the old
RED result; silently restoring the first-start `O(epoch)` expectation is a
contract violation.
