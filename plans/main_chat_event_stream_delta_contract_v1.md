# Main Chat Event Stream Delta Contract v1

> Date: 2026-06-17
> Status: preparation artifact for Product Maturity v2
> Parent: `plans/main_chat_agent_product_maturity_v2_goal_spec.md`

## 1. Purpose

Productization v1 exposes snapshots and ordered events derived from snapshots.
That is enough for deterministic UI readiness, but it is not a real task delta
stream.

This contract defines the real event stream needed for Product Maturity v2.

## 2. Baseline

Current state:

- Main Chat sends `agent_state` snapshots in send/stream results.
- Snapshot events are ordered by sequence.
- UI renders only runtime-backed objects.
- The productization report explicitly says:
  `snapshot_derived_ordered_events_not_live_delta_stream`.

Missing:

- durable event ids,
- event append source,
- replay since sequence,
- reconnect recovery,
- event dedupe,
- partial streaming vs durable event separation,
- event retention/compaction policy.

## 3. Benchmark Lessons

### Codex-style lesson

Users trust agent work when they see incremental execution: command started,
output observed, failure recorded, retry selected, final result delivered.

### Hermes-style lesson

Task progress should be a live work stream, not a completed transcript rendered
after the fact.

### OpenLife constraint

OpenLife must keep metadata safety and no silent writes. Event payloads must be
bounded and must not leak raw sensitive context.

## 4. Event Model

### 4.1 Event identity

Every durable event must include:

- `eventId`
- `taskSessionId`
- `runId`
- `sequence`
- `eventType`
- `objectType`
- `objectId`
- `createdAt`
- `source`
- `payloadDigest`
- `payload`

`sequence` is monotonic per `taskSessionId`.

### 4.2 Event source

Allowed sources:

- `agent_ingress`
- `strategy_router`
- `context_compiler`
- `plan_runtime`
- `action_queue`
- `action_executor`
- `agent_loop`
- `proposal_store`
- `task_control`
- `finalizer`
- `diagnostic`

Frontend local state is not an event source.

## 5. Required Event Types

Minimum durable event types:

- `task.created`
- `task.updated`
- `route.selected`
- `context.selected`
- `provider.selected`
- `plan.created`
- `plan.updated`
- `plan.confirmed`
- `plan.reviewed`
- `step.created`
- `step.updated`
- `step.skipped`
- `action.queued`
- `action.started`
- `action.updated`
- `action.completed`
- `action.failed`
- `observation.created`
- `blocker.created`
- `blocker.resolved`
- `proposal.created`
- `proposal.updated`
- `proposal.accepted`
- `proposal.rejected`
- `proposal.deferred`
- `memory.materialized`
- `memory.rolled_back`
- `final_delivery.created`
- `diagnostic.created`

## 6. Snapshot And Delta Relationship

Snapshot remains the recovery source.

Flow:

1. UI requests latest snapshot.
2. UI subscribes to events after `snapshot.sequence`.
3. Runtime emits deltas as state changes.
4. If stream disconnects, UI requests events since last applied sequence.
5. If event replay is unavailable or truncated, UI requests fresh snapshot.

## 7. Stream Transport

Initial implementation may reuse Tauri event emission.

Required command/event surface:

- `get_main_chat_agent_state_snapshot(taskSessionId)`
- `list_main_chat_agent_events(taskSessionId, afterSequence, limit)`
- `main-chat-agent-event` Tauri event
- optional `subscribe_main_chat_agent_task(taskSessionId)` helper

Tauri event emission is transport only. It is not the source of truth. A test
must fail if events are emitted to the UI but cannot be replayed through
`list_main_chat_agent_events`.

## 8. Event Store And Transaction Rules

The implementation must provide one of:

- a dedicated durable `MainChatAgentEventStore`, or
- a replayable adapter over existing task session, action queue, transcript,
  proposal, memory lifecycle, and final delivery records.

Either form must expose the same event API.

Prefer a narrow Main Chat event store or adapter around existing Main Chat task
session, action queue, transcript, proposal, memory lifecycle, and final
delivery records. Do not refactor all runtime stores unless the narrow event
contract cannot be satisfied and the blocker is explicitly reported.

### 8.1 Sequence Source

Each `taskSessionId` must have one authoritative sequence source.

Preferred implementation:

- Store `last_event_sequence` with the task session or event store.
- Allocate the next sequence while appending the event.
- Persist the sequence before emitting the event to the UI.

Adapter implementation:

- If events are derived from existing stores, sequence must be generated from a
  stable ordered tuple, not from read order alone.
- The tuple must include source store, source object id, source object timestamp,
  and event type.
- A replay after restart must return the same sequence for the same event.

Live events and backfilled compatibility events must share a clear namespace.
Backfilled events may use negative sequence numbers, a separate `backfill=true`
flag, or another explicit marker, but they must not collide with live durable
events.

### 8.2 Event Id Generation

`eventId` must be stable across replay.

Recommended shape:

```text
mainchat_event:{taskSessionId}:{sequence}:{eventType}:{objectId}:{payloadDigest}
```

If a dedicated event store uses random ids, it must still persist those ids and
return the same ids through replay. UI tests must reject replayed events whose
ids or sequences differ from the emitted events.

Append rules:

- Event sequence is allocated monotonically per `taskSessionId`.
- Runtime state mutation and corresponding event append should be transactional
  when they share the same store.
- If a true cross-store transaction is unavailable, the event must carry
  enough object evidence to detect and report partial append/state mismatch.
- Failed runtime actions may append `action.failed` and `blocker.created`; they
  must not append `action.completed`.
- Event append failure must be visible as `diagnostic.created` or a command
  error; it cannot silently degrade to snapshot-only success.

Backfill rules:

- Existing transcript-derived snapshot events may be exposed as `backfilled`
  events only with `source=diagnostic` or `source=task_control`.
- Backfilled events cannot receive live delta credit.
- Product readiness must distinguish live durable delta events from backfilled
  compatibility events.

## 9. Idempotency And Ordering

UI applies an event only if:

- `taskSessionId` matches current task,
- `sequence > lastAppliedSequence`,
- `eventId` has not been applied,
- required object evidence exists in payload or can be loaded.

Out-of-order events:

- buffer briefly if gap is small,
- request replay if gap persists,
- request snapshot if replay fails.

Duplicate events:

- ignore by `eventId`,
- ignore lower or equal sequence for same task.

## 10. Streaming Text Separation

Streaming assistant text is transient.

It must not create:

- action events,
- observation events,
- proposal events,
- final delivery events.

Final delivery can reference streamed text only after finalizer emits
`final_delivery.created`.

## 11. Privacy And Metadata Safety

Event payloads must:

- be bounded,
- avoid raw full prompt unless already approved for transcript,
- use digests for large inputs,
- include source labels rather than unsafe raw manifest details,
- avoid API keys and provider secrets,
- preserve no silent write metadata.

## 12. Eval Scenarios

Minimum scenarios:

- direct answer emits route and final delivery events,
- read action emits queued/started/completed/observation/final delivery,
- failure emits action.failed and blocker.created,
- proposal emits proposal.created then proposal.accepted,
- rollback emits memory.rolled_back,
- reconnect replays missed events,
- duplicate event is ignored,
- event gap triggers snapshot refresh.

## 13. Acceptance

This contract is satisfied when:

- UI can render live task progress from deltas,
- snapshot recovery works after reconnect,
- event replay works by sequence,
- event replay returns the same ids/sequences that were emitted live,
- backfilled snapshot events cannot be counted as live delta proof,
- fake frontend events cannot unlock execution UI,
- Productization report no longer needs to say snapshot-derived events are the
  only event semantics.

## 14. Stop Conditions

Stop if:

- event emission cannot be tied to runtime state transitions,
- event stream would leak raw private context,
- event ordering cannot be guaranteed per task,
- UI must infer execution from streamed text.
