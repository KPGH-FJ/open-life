# ADR 0014: Explicit User Memory Write Lane

Date: 2026-07-10
Status: accepted
Relationship: amends ADR 0013 without replacing its LifeModel-HS governance

## Context

ADR 0013 makes risky HS mutation proposal-first and allows automatic acceptance
only for bounded transient state. That remains correct for inferred learning,
canonical HS assets, identity, values, long-term goals, sensitive relationships,
and privacy policy.

The product also needs to distinguish inferred learning from a current user who
explicitly asks OpenLife to remember an exact, reversible fact. Treating that
request as another interrupting proposal creates review fatigue and makes the
assistant less capable even though the user already supplied the instruction.

## Decision

OpenLife may provide a non-interrupting explicit Memory write lane only when all
of the following are true:

- the authority source is `current_authenticated_user_message`;
- the request explicitly asks to remember, save, or update an exact fact;
- the destination is reversible Memory, not a canonical HS asset;
- the classified risk is low or medium;
- the write records the source message id, fact key, sensitivity, and audit
  digest;
- the product returns an inspectable receipt and a working undo action;
- the content did not originate from a tool, web page, file, MCP server, A2A
  peer, assistant message, historical transcript, or quoted instruction.

This lane must not directly mutate canonical HS assets. It must not update
identity, values, mission, long-term goals, stable policy, privacy policy, or
sensitive relationship definitions. High-sensitivity memory remains governed
by ReviewWorkflow.

## Inferred Memory

Model- or heuristic-inferred durable facts never use this lane. They are
deduplicated into a deferred ReviewBatch. ReviewBatch creation must not block
the current turn and must not be described as a completed durable change.

## Relationship To ADR 0013

ADR 0013 remains authoritative for LifeModel-HS source of truth, phased
migration, transient state auto-acceptance, and risky canonical mutation. This
ADR narrows the treatment of an exact user-directed, reversible Memory write;
it does not relax HS policy or allow silent learning.

## Required Evidence

- deterministic tests that only the current user message can authorize the
  lane;
- negative prompt-injection tests for web, file, MCP, A2A, tool, and quoted
  content;
- concurrent write and undo tests;
- product trials showing explicit memory succeeds without proposal fatigue;
- receipts that distinguish committed Memory from staged ReviewBatch and
  canonical HS changes.
