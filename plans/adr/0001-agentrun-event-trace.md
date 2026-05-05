# ADR 0001: AgentRunEvent Append-Only Trace Model

Date: 2026-05-06
Status: accepted

## Context

OpenLife already has `AgentRun`, status updates, actions, observations, proposals, and audit stores. However, vNext needs a stronger durable trace that can explain every meaningful runtime transition across chat, streaming, fallback, tool execution, proposal creation/application, replay, future PlanMode, and future sub-agents.

Status updates are useful for UI, but they are not enough as the source of truth. vNext needs append-only events.

## Decision

Introduce `AgentRunEvent` as the durable append-only trace record for formal agent behavior.

Every event belongs to an `AgentRun`. Child runs, sub-agent runs, and replay runs must link back to their parent where applicable.

## Proposed Schema

```rust
pub struct AgentRunEvent {
    pub id: String,
    pub run_id: String,
    pub parent_event_id: Option<String>,
    pub event_type: AgentRunEventType,
    pub phase: Option<String>,
    pub actor: AgentEventActor,
    pub summary: String,
    pub payload: serde_json::Value,
    pub redaction: Option<RedactionSummary>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub enum AgentEventActor {
    User,
    Agent,
    SubAgent(String),
    Tool(String),
    Runtime,
    System,
}
```

Minimum P0 event types:

- `run.created`
- `context.assembled`
- `model.route_selected`
- `model.call_started`
- `model.call_completed`
- `model.call_failed`
- `tool.call_started`
- `tool.call_blocked`
- `tool.call_completed`
- `tool.call_failed`
- `observation.created`
- `proposal.created`
- `fallback.started`
- `fallback.completed`
- `json_repair.started`
- `json_repair.completed`
- `run.completed`
- `run.failed`

Later event types:

- `prompt.assembled`
- `plan.created`
- `plan.confirmation_requested`
- `proposal.accepted`
- `proposal.rejected`
- `proposal.applied`
- `proposal.apply_failed`
- `subagent.started`
- `subagent.completed`
- `compaction.created`
- `sandbox.blocked`

## Options Considered

### Option A: Store events inside AgentRun JSON

Pros:

- Simple to load with the run.
- Easy first implementation.

Cons:

- Harder to query by event type.
- Large runs become heavy.
- Append/update conflict risk.

### Option B: Separate `agent_run_events` SQLite table

Pros:

- Queryable.
- Append-friendly.
- Better for long-running and child runs.
- Easier to stream in UI.

Cons:

- Requires new store and migration.
- More joins for full run display.

### Option C: Use existing audit logs only

Pros:

- Minimal new storage.

Cons:

- Audit is not equivalent to runtime trace.
- Does not cover model, prompt, planning, fallback, or sub-agent transitions cleanly.

## Recommendation

Use Option B: a separate `agent_run_events` SQLite table, colocated with or linked to the AgentRun store.

## Consequences

Positive:

- Runtime behavior becomes inspectable.
- Fallback and repair paths become visible.
- Future PlanMode and SubAgentRuntime get a trace substrate.
- UI can show a timeline without reconstructing state from many stores.

Tradeoffs:

- More write events.
- More schema design work.
- Need payload redaction policy.

## Implementation Guardrails

- Events are append-only.
- Do not store raw secrets in payload.
- Event creation failures should not crash user chat unless persistence is essential for a side-effecting operation.
- Side effects must not be considered complete until the relevant event/audit/proposal linkage is attempted.
- UI status updates should be derived from events where possible, not used as the durable source.

## Verification

P0 tests should prove:

- Creating an AgentRun creates `run.created`.
- A normal model response creates model route/call/completion events.
- A tool call creates started/completed or started/blocked events.
- A fallback creates fallback events.
- JSON repair creates repair events.
- Event payload redaction strips configured sensitive fields.

## Open Questions

1. Should event persistence be best-effort for low-risk read-only runs?
   → **Resolved (P0-5):** Yes for P0. `try_record_event` silently drops on failure. Side-effecting ops should require persistence post-P1.

2. Should event IDs be UUIDs or deterministic monotonic IDs per run?
   → **Resolved (P0-1):** UUIDs (`Uuid::new_v4()`) for P0. Monotonic IDs can be added later.

3. Which payload fields are stored raw, summarized, or redacted?
   → **Resolved (P0-5):** Raw by default for P0. Redaction is opt-in via `AgentRunEvent.with_redaction()`. Full redaction policy governed by ADR 0006.

4. Should existing AgentRun status updates be migrated or merely bridged going forward?
   → **Resolved (P0-5):** Bridge only future runs. No migration. See `plans/openlife_vnext_agentrun_event_data_bridge.md` for full analysis.
