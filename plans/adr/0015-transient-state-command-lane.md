# ADR 0015: TurnRuntime-owned transient state command lane

Date: 2026-07-12
Status: accepted
Relationship: preserved by ADR 0016 as the domain-owned transient StateStore
lane; does not relax ADR 0014 or LifeModel proposal requirements

## Context

The Chat product currently intercepts `/goal` and `/state` in React. The page
can mutate daily goals or state history before the ordinary message reaches
`OpenLifeTurnRuntime`, then generates success prose and persists conversation
messages separately. A lost response or message-write failure can therefore
leave one effect with no matching receipt, and a retry can duplicate it.

Moving only the operation UUID earlier would reduce duplicate writes but would
leave policy, execution, truth projection, and user-facing completion owned by
the frontend. That would preserve the architectural cause.

The original ADR 0013 permitted automatic acceptance only for bounded transient
`StateAsset` updates; ADR 0016 preserves that rule under domain ownership. The
existing YAML daily-goal and state-history shapes do
not meet that contract because they do not uniformly carry TTL, source,
confidence, privacy, operation identity, and an inspectable receipt.

## Decision

Ordinary Chat mutations, including slash commands, enter the same
`OpenLifeTurnRuntime` as every other Main Chat turn. React may format a backend
read model, but it may not parse a mutating command, invoke a durable command,
or author success prose.

`PolicyRouter` may authorize a direct transient-state lane only when all of the
following are true:

- authority is the current authenticated user message;
- the requested value and target are explicit and unambiguous;
- the asset is short-lived state or daily task state, not a long-term goal,
  identity, value, preference, policy, relationship, or accepted HS asset;
- risk is low and sensitivity is not high;
- the command has a UUIDv4 operation id bound to the exact canonical payload;
- the state record has source, confidence, privacy class, creation time, and a
  bounded expiry between 24 hours and 7 days;
- the product can return a typed execution receipt and an undo or explicit
  expiry path.

Medium/high-risk, sensitive, inferred, ambiguous, and long-term changes remain
proposal-first through `ReviewWorkflow`.

## Canonical ownership

`StateStore` is the target canonical owner for this lane. It stores typed
transient assets and daily task state with:

- stable asset id and version;
- operation id plus payload digest uniqueness;
- source message reference;
- state kind and bounded value;
- confidence and privacy class;
- `created_at`, `updated_at`, and `expires_at`;
- undo/expiry state;
- a minimal execution receipt and outbox event in the same transaction.

An exact operation replay returns the prior receipt. Reusing an operation id
with a different payload fails closed. Concurrent submissions have one winner.
Projection failure is `projection_degraded`, never canonical failure.

The YAML LifeModel remains a migration/compatibility view for this asset class
until the StateStore parity and rollback gates pass. During migration:

- existing YAML/state-history data may be imported or shadow-read;
- product writes have one owner and must not dual-write independently;
- materialization to YAML is an idempotent outbox projection;
- a failed projection cannot invite a second canonical state mutation.

Conversation messages remain owned by the Conversation gateway. The
TurnRuntime creates the user message and execution identity before the state
effect, persists the state receipt durably, and emits assistant/final delivery
from that receipt. If message projection fails, retry/reconciliation reuses the
same operation receipt and never repeats the effect.

## Deletions required for completion

- ChatPage mutating `tryHandleQuickCommand` execution;
- direct page calls to `add_daily_goal` and `record_state` as Chat authority;
- frontend-generated mutation success prose;
- unkeyed state/daily-task canonical mutation;
- YAML and state-history dual product-write authority for migrated assets.

Read-only shortcuts may remain only if they consume a backend read model and do
not synthesize lifecycle or readiness truth.

## Required evidence

- exact command, natural-language equivalent, quoted-injection, sensitive, and
  long-term-goal routing tests;
- effect committed followed by response/message loss, then same-operation retry;
- operation-id payload drift and concurrent-winner tests;
- cancellation before commit and cancellation after commit receipt tests;
- expiry and undo tests;
- StateStore transaction/outbox fault injection;
- YAML shadow parity and rollback rehearsal before source-of-truth cutover;
- product trial proving the command remains useful without Proposal fatigue and
  that UI completion matches canonical storage and receipt facts.
