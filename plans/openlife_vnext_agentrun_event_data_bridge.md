# OpenLife vNext AgentRunEvent Data Bridge Audit

Date: 2026-05-06  
Status: P0-5 deliverable (spec only, no code changes)  
Related: `plans/adr/0001-agentrun-event-trace.md`, `plans/openlife_vnext_p0_p1_task_specs.md`

---

## 1. Inventory of Existing AgentRun Persisted Fields

Source: `openlife-core/src/agent/types.rs::AgentRun` (line 280) and `openlife-core/src/agent/store.rs::AgentRunStore` (line 7).

### 1.1 AgentRun Struct Fields

| Field | Type | Persisted As | Notes |
|-------|------|-------------|-------|
| `id` | `String` (UUID) | `TEXT PRIMARY KEY` | The run's primary identifier |
| `task_id` | `String` (UUID) | `TEXT NOT NULL` | Link to AgentTask (currently same UUID as run.id in most constructors) |
| `session_id` | `Option<String>` | `TEXT` | Chat session link |
| `status` | `AgentRunStatus` | `TEXT` (running/waiting_permission/completed/failed/cancelled) | Last-write-wins status |
| `kind` | `AgentTaskKind` | `TEXT` (conversation/builder/calibration/...) | What originated this run |
| `user_input` | `Option<String>` | `TEXT` | The raw user message |
| `context_summary` | `Option<ContextSummary>` | `JSON TEXT` | Context assembly metadata |
| `model_route` | `Option<ModelRouteTrace>` | `JSON TEXT` | Which model was used and why |
| `output_preview` | `Option<String>` | `TEXT` | First 200 chars of output |
| `error` | `Option<AgentRunError>` | `JSON TEXT` | Error message + phase + recoverable flag |
| `generated_proposals` | `Vec<String>` | `JSON TEXT DEFAULT '[]'` | List of proposal IDs from this run |
| `actions` | `Vec<AgentAction>` | `JSON TEXT DEFAULT '[]'` | All tool/memory/LifeModel actions |
| `observations` | `Vec<AgentObservation>` | `JSON TEXT DEFAULT '[]'` | All tool/memory/agent observations |
| `reasoning_strategy` | `Option<String>` | `TEXT` | "layered" or "direct" |
| `reasoning_trace` | `Option<ReasoningTrace>` | `JSON TEXT` | LayeredReasoner full trace |
| `warnings` | `Vec<String>` | **NOT PERSISTED** | Runtime warnings (parse issues, repair flags) |
| `status_updates` | `Vec<AgentLoopStatusUpdate>` | `JSON TEXT DEFAULT '[]'` | Phase-level UI status updates |
| `step_count` | `u32` | `INTEGER DEFAULT 0` | Total loop iterations |
| `tool_call_count` | `u32` | `INTEGER DEFAULT 0` | Total tool invocations |
| `deleted_at` / `delete_reason` | `Option<DateTime>` / `Option<String>` | `TEXT` | Soft delete |
| `started_at` / `finished_at` | `DateTime` / `Option<DateTime>` | `TEXT` | Run timing |

### 1.2 Related Persistent Stores

| Store | Database | Records |
|-------|----------|---------|
| `AgentRunStore` | `agent_runs.db` (SQLite) | AgentRun records (full snapshot, one row per run) |
| `ProposalStore` | `proposals.db` (SQLite) | AgentProposal records with `run_id` backlink |
| `McpAuditStore` | `mcp_audit.db` (SQLite) | Tool call audit logs (separate from runtime trace) |

---

## 2. Mapping Current Fields to AgentRunEvent Types

### 2.1 Direct Field-to-Event Mappings

| Current Field | Maps to AgentRunEvent | Mapping Strategy |
|---|---|---|---|
| `status: Running` at creation | `run.created` | **Emit once** when AgentRun is first created |
| `status: Completed` | `run.completed` | **Emit once** when run finishes successfully |
| `status: Failed` | `run.failed` | **Emit once** when run fails |
| `status: WaitingPermission` | `tool.call_blocked` + `proposal.created` | **Derived from** blocked action |
| `context_summary` | `context.assembled` | **Emit once** when context assembly completes |
| `model_route` | `model.route_selected` | **Emit once** when ModelRouter chooses a path |
| `actions[]` (each item) | `tool.call_started/completed/failed/blocked` | **One event per action** (status determines type) |
| `observations[]` (each item) | `observation.created` | **One event per observation** |
| `generated_proposals[]` (each item) | `proposal.created` | **One event per proposal** |
| `error: AgentRunError` | `model.call_failed` or `run.failed` | **Embedded in** appropriate failure event |
| `status_updates[]` | NOT mapped (UI projection, not trace) | **Derived from** events for UI, not primary |
| `step_count` / `tool_call_count` | `run.completed` payload | **Embedded** in completion event payload |
| `warnings[]` | Scattered across relevant events | **In event summary/payload** |
| `reasoning_strategy` / `reasoning_trace` | `model.call_completed` payload | **Embedded** in model completion event |
| `started_at` / `finished_at` | Event timestamps | **Each event** has its own `created_at` |

### 2.2 Gaps (Current fields NOT representable as events)

| Gap | Severity | Notes |
|-----|----------|-------|
| `fallback` path does not record events | High | `handle_agent_loop_fallback` creates a new AgentRun with no event trace |
| `L1 reflex` has no `context.assembled` or `model.route_selected` event | Medium | Creates AgentRun but skips full execution path |
| `Builder/Calibration` paths have no AgentLoop events | Medium | Creates AgentRun but executes outside AgentLoop |
| `warnings` field is **not persisted** in store.rs | Medium | Lost on reload; event store should preserve them |
| `replay_agent_action` modifies existing run without events | Medium | Event timeline breaks for replayed actions |
| Scheduled task AgentRun has no `proposal.created` events | Low | No proposal generation in scheduler runner |

---

## 3. Bridge Strategy Comparison

### Option A: Bridge Only Future Runs

**How it works:** All `AgentRun` records created after P0-1 deployment also get `AgentRunEvent` records. Old runs (pre-P0-1) are left as-is.

| Pros | Cons |
|------|------|
| Zero migration risk | Old runs appear empty in event timeline |
| Simple to implement | User sees different UX for old vs new runs |
| No data corruption risk | Audit trail is split |
| Fast rollout | |

### Option B: Synthesize Events for Old Runs on Read

**How it works:** When a run is loaded that has no events, synthesize `AgentRunEvent` records in-memory from the existing AgentRun JSON fields.

| Pros | Cons |
|------|------|
| Uniform UX for all runs | Synthetic events are not append-only truth |
| No database migration | Event timeline may be incomplete or imprecise |
| | `warnings` are not persisted — lost data |
| | Adds read-time complexity |
| | Events synthesized from snapshot, not actual runtime trace |

### Option C: One-Time Migration

**How it works:** A migration script scans all existing AgentRun records and creates synthetic `AgentRunEvent` records in the event store.

| Pros | Cons |
|------|------|
| Events are persistent | Synthetic events look like real runtime trace |
| Queryable via standard event API | One-time migration is not reversible |
| | Existing data has less fidelity (no warnings, no repair events) |
| | Migration may fail silently on malformed data |

### Option D: Ignore Old Runs for Event Timeline

**How it works:** Old runs are displayed with existing fields (status, actions, model_route, output). Event timeline UI only shows for runs with events.

| Pros | Cons |
|------|------|
| Honest about data fidelity | Split UX |
| No fake events | Old runs miss timeline feature |
| Minimal implementation | |

---

## 4. P0 Recommendation

**Recommendation: Option A (Bridge Only Future Runs).**

Rationale:

1. **P0 scope is minimal.** The task spec says "No database migration. No code changes." This rules out Options B and C for P0.

2. **Honest data fidelity.** Synthetic events (Options B/C) would be misleading — they claim "append-only trace" but are reconstructed from snapshots that lack warnings, repair events, per-action timestamps, and intermediate model route details.

3. **ADRs support append-only truth.** ADR 0001 states: "Events are append-only. UI status updates should be derived from events where possible, not used as the durable source." Synthesizing from old snapshots violates this principle.

4. **Option D is the honest fallback if we want to avoid even bridge complexity.** But Option A is simpler: just record events for new runs and treat old runs as pre-event-era records.

### Implementation Plan for P0

1. Future `AgentRun` creation paths call `try_record_event(run.created)`.
2. Future `AgentLoop` execution paths record events (P0-2 already done).
3. Old runs (pre-P0-1) are loaded normally — no event bridge attempted.
4. UI displays old runs with existing fields; new runs get event timeline.
5. `AgentRunStore::get_run()` returns `AgentRun` as-is; event timeline is a separate query.

### Migration Path for P1+

When P1-7 (Execution Facade) is implemented, every new run will naturally get events because the facade calls AgentLoop. Builder/Calibration paths will be refactored to go through the facade, gaining events at that time.

### Risks of Option A

| Risk | Mitigation |
|------|-----------|
| Split UX (old runs vs new) | Accept as temporary; old runs age out naturally |
| Migration planning pressure | Document that synthetic bridging is a post-P1 concern |
| AgentRunEvent store may have no data for old runs | This is by design; old runs use existing fields |

---

## 5. ADR 0001 Open Questions Update

Original ADR 0001 open questions and resolution:

| # | Question | P0-5 Resolution |
|---|----------|----------------|
| 1 | Should event persistence be best-effort for low-risk read-only runs? | **Yes.** P0-1 already uses `try_record_event` which silently drops on store failure. This is acceptable for P0. Post-P1, side-effecting operations should *require* event persistence before commit. |
| 2 | Should event IDs be UUIDs or deterministic monotonic IDs per run? | **UUIDs for P0.** P0-1 uses `Uuid::new_v4()`. Monotonic IDs can be added later as a secondary index. |
| 3 | Which payload fields are stored raw, summarized, or redacted? | **Raw by default for P0.** P0-1 stores payload as `serde_json::Value`. Redaction is opt-in via `AgentRunEvent.with_redaction()`. Full redaction policy is a P2 concern (ADR 0006 governs cloud privacy). |
| 4 | Should existing AgentRun status updates be migrated or merely bridged going forward? | **Bridge going forward only. No migration.** See Option A recommendation above. Status updates (`status_updates` field) remain as UI projection; events become the durable source for new runs. |

These questions are now **resolved** for P0 implementation. The ADR 0001 document can be updated to reflect these decisions.

---

## 6. Appendix: Current AgentRun Table Schema

```sql
CREATE TABLE IF NOT EXISTS agent_runs (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL,
    session_id TEXT,
    status TEXT NOT NULL,
    kind TEXT NOT NULL,
    user_input TEXT,
    context_summary_json TEXT,
    model_route_json TEXT,
    output_preview TEXT,
    error_json TEXT,
    generated_proposals_json TEXT DEFAULT '[]',
    actions_json TEXT DEFAULT '[]',
    observations_json TEXT DEFAULT '[]',
    reasoning_strategy TEXT,
    reasoning_trace_json TEXT,
    status_updates_json TEXT NOT NULL DEFAULT '[]',
    step_count INTEGER NOT NULL DEFAULT 0,
    tool_call_count INTEGER NOT NULL DEFAULT 0,
    deleted_at TEXT,
    delete_reason TEXT,
    started_at TEXT NOT NULL,
    finished_at TEXT
);
```

---

*End of P0-5 deliverable.*
