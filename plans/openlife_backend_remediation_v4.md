# OpenLife Backend Remediation v4

> Date: 2026-07-10
> Status: active implementation work package
> Authority: subordinate to the Phase7 contract and its deletion manifest

This work package implements the approved backend remediation plan. It is not a
new product route, compatibility adapter, readiness claim, or authority over
`AGENTS.md`, `plans/README.md`, the Phase7 deletion manifest, or the current
single-system development preparation.

## Objective

Resolve the 35 audited backend findings by removing their root causes and the
old product routes that express them. The target has one turn runtime, one
policy router, one provider router, one tool gateway, governed canonical writes,
minimal durable execution facts, and backend-owned product projections.

## Threat Model

This round must defend against prompt injection from user-provided untrusted
content, web/file/tool/MCP/A2A observations, malicious or hung extensions,
unauthorized local loopback clients, concurrent/replayed writes, ambiguous
network outcomes, crash and migration failures, and duplicate sensitive-data
copies.

It does not claim protection from an operating-system administrator, an
attacker who controls the current user account and keychain, a maliciously
re-signed OpenLife binary, or universal exactly-once delivery to a downstream
system that supplies neither idempotency nor reconciliation.

## Root-Cause Work Protocol

Each finding moves through the following evidence chain:

```text
symptom -> reproduction -> root-cause trace -> broken invariant -> hypothesis
-> failing test -> minimal root fix -> old-route deletion -> positive proof
-> counterfactual proof -> capability non-regression
```

Three unsuccessful fixes for the same root condition stop implementation and
trigger a fresh architecture review. A timeout, retry, proposal, adapter, or
renamed symbol is not a root fix when the blocking route remains alive.

## Frozen Scenario Suite

The Phase 0 contract freezes 40 scenarios before behavioral implementation:

| Group | Count |
| --- | ---: |
| ordinary chat, planning, and writing | 8 |
| explicit and inferred Memory | 6 |
| privacy and provider routing | 6 |
| web, file, and local reads | 6 |
| tool permission and external writes | 6 |
| cancellation, resume, and concurrency | 4 |
| realistic Chinese ambiguity and daily-life language | 4 |

Expected outcomes and the helpfulness rubric may change only through a new
versioned suite plus a human-authored waiver explaining why the old expectation
was invalid. Fixture-backed cases cannot earn external live-provider, live-web,
live-MCP, or live-A2A credit.

## Baseline Evidence

Repository baseline at plan approval:

- HEAD `1ca7613bcd25167cf173fa0a21e3baa908f21d94`;
- `cargo test -p openlife-core`: 615 passed, 1 ignored;
- focused `single_system`: 25 passed;
- focused `main_chat_runtime_module`: 26 passed;
- full Tauri suite: 394 passed, 1 stream-timeout failure, 3 ignored;
- isolated failed stream test rerun: passed in 32.28 seconds;
- `cargo fmt --check`: passed;
- strict workspace Clippy: failed;
- cargo audit: no unallowed failure, 20 allowed warnings;
- shipped handler count observed during audit: 161;
- separate SQLite files assembled by bootstrap: 17.

These values are evidence inputs, not completion credit. The machine-readable
finding inventory is `plans/openlife_backend_remediation_v4_inventory.json`.
The invariant/proof matrix is
`plans/openlife_backend_remediation_v4_traceability.json`; the executable
Phase 0 source map, threat/control mapping, baseline UNKNOWN boundaries, and
backout runbook are
`plans/openlife_backend_remediation_v4_phase0_evidence.md`.

## Phase Order

1. Phase 0: evidence freeze, ADR decision, threat model, release quarantine.
2. Phase 1: provider fact seam and privacy P0 without a router rewrite.
3. Phase 2: single TurnRuntime, streaming, cancellation, terminal state, locks.
4. Phase 3: single control plane, non-interrupting governance, CAS.
5. Phase 4: ToolGateway, network, MCP, A2A.
6. Phase 5: canonical/outbox recovery, scheduler, audit, vector, phased HS.
7. Phase 6: product projections, old-route deletion, quality and live trial.

An unresolved phase gate blocks promotion, release-quarantine removal, live
capability credit, and any claim that the phase or a later phase is complete.
It does not forbid a fail-closed implementation slice whose dependency on the
open gate is explicit and whose capability remains quarantined. Such a slice
earns no completion credit until every prerequisite gate is independently
verified.

## Rollback And Backout

- Every phase uses focused commits and a bounded deletion list.
- Product data migrations must create a verified backup before authority shifts.
- A new interface may be reverted only with its entire phase; the old product
  route is not kept live as an indefinite fallback.
- Release quarantine may be relaxed only by the Phase 4 MCP/A2A gates.
- LifeModel-HS authority moves by asset category after parity and rollback
  rehearsal; there is no whole-model cutover.
- Unknown external effects are reconciled, never blindly replayed.

## Phase 0 Exit Gate

- the 35-finding inventory validates;
- the traceability matrix maps all 35 findings to broken invariants,
  reproduction evidence, positive/counterfactual proof, and non-regression
  scenarios;
- ADR 0014 is accepted and subordinate to ADR 0013;
- the threat model, scenario freeze, baseline, and backout policy are present;
- release has an explicit CSP and no remote localhost capability;
- A2A autostart, arbitrary MCP registration, direct A2A commands, and generic
  product tool execution are absent from the release handler;
- dev-only access requires the explicit `dev-extensions` Cargo feature and
  development Tauri configuration;
- focused Phase 0 guards, formatting, single-system, and runtime-module tests
  pass.
