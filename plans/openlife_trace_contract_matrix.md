# OpenLife Trace Contract Matrix

Date: 2026-05-16

Status: active

> This document defines the end-to-end typed event contract from backend emission through frontend parsing to UI display. Every event that carries typed governance payloads (block_reason, proposal_reason, failure_kind, agent_spec_id, proposal_id) is tracked here.

---

## 1. Event Type Coverage Matrix

### Legend

| Mark | Meaning |
|------|---------|
| ✅ | Fully supported end-to-end |
| ⚠️ | Supported but payload not yet uniformly typed (reason text fallback exists) |
| 📋 | Defined but never emitted (orphan) |
| 🔮 | Frontend only; backend uses generic event type |
| ❌ | Emitted but not yet parsed by typedContract |

### Scale: Tier 1 Governance Events (typed payload required)

These events carry governance payloads (`block_reason`, `proposal_reason`, `failure_kind`, `agent_spec_id`, `proposal_id`). The typedContract must validate and extract them.

| # | Event Type | Backend Emission | Frontend Parser | Frontend Display | Typed Payload Required | Fallback Behavior |
|---|-----------|-----------------|-----------------|------------------|----------------------|-------------------|
| 1 | `tool.call_blocked` | ✅ `tool_executor.rs` (5 sites), `tools.rs`, `plan_executor.rs` | ✅ `parseTypedEventPayload` → `tool_call_blocked` | ✅ `RunTracePanel` via `TypedEventDetailViewModel` | `status`, `tool_name`, `source`, `block_reason`\|`proposal_reason` | Malformed → `kind: "unknown"`, no typed badge |
| 2 | `replay.started` | ✅ `commands/agent.rs` | ✅ `parseTypedEventPayload` → `replay_started` | ✅ `RunTracePanel` via `TypedEventDetailViewModel` | `status`, `run_id`, `action_id`, `replay_of_action_id`, `agent_spec_id`, `tool_name`, `source` | Malformed → `kind: "unknown"` |
| 3 | `replay.completed` | ✅ `commands/agent.rs` | ✅ `parseTypedEventPayload` → `replay_completed` | ✅ `RunTracePanel` via `TypedEventDetailViewModel` | `status`, `run_id`, `action_id`, `replay_of_action_id`, `agent_spec_id`, `tool_name`, `source`, optional `block_reason`\|`proposal_reason`\|`failure_kind` | Malformed → `kind: "unknown"` |
| 4 | `replay.failed` | ✅ `commands/agent.rs` (7 paths) | ✅ `parseTypedEventPayload` → `replay_failed` | ✅ `RunTracePanel` via `TypedEventDetailViewModel` | At least one of `block_reason`\|`failure_kind`; `run_id`, `action_id`, `replay_of_action_id` required | No valid reason → `kind: "unknown"` |

### Scale: Tier 2 Informational Events (structured payload optional)

These events carry metadata for trace/debug. They don't drive governance decisions in the UI. The typedContract passes them through as informational.

| # | Event Type | Backend Emission | Frontend Parser | Frontend Display | Summary-Dependent | Fallback |
|---|-----------|-----------------|-----------------|------------------|-------------------|----------|
| 5 | `run.created` | ✅ `orchestrator.rs`, `sub_agent.rs` | ❌ Pass-through (unknown kind) | ✅ `RunTracePanel` generic event row | Yes | N/A |
| 6 | `run.completed` | ✅ `orchestrator.rs` | ❌ Pass-through | ✅ `RunTracePanel` generic event row | Yes | N/A |
| 7 | `run.failed` | ✅ `orchestrator.rs`, `chat_persistence.rs` | ❌ Pass-through | ✅ `RunTracePanel` generic event row | Yes | N/A |
| 8 | `model.call_started` | ✅ `generation.rs` | ❌ Pass-through | ✅ `RunTracePanel` generic event row | Yes | N/A |
| 9 | `model.call_completed` | ✅ `generation.rs` | ❌ Pass-through | ✅ `RunTracePanel` generic event row | Yes | N/A |
| 10 | `model.call_failed` | ✅ `generation.rs` | ❌ Pass-through | ✅ `RunTracePanel` generic event row | Yes | N/A |
| 11 | `model.failed` | ✅ `streaming.rs` | ❌ Pass-through | ✅ `RunTracePanel` generic event row | Yes | N/A |
| 12 | `model.route_selected` | ✅ `generation.rs` (2 paths) | ❌ Pass-through | ✅ `RunTracePanel` generic event row | Yes | N/A |
| 13 | `tool.call_started` | ✅ `tools.rs`, `tool_executor.rs` | ❌ Pass-through | ✅ `RunTracePanel` generic event row | Yes | N/A |
| 14 | `tool.call_completed` | ✅ `tools.rs`, `tool_executor.rs` | ❌ Pass-through | ✅ `RunTracePanel` generic event row | Yes | N/A |
| 15 | `tool.call_failed` | ✅ `tools.rs`, `tool_executor.rs` | ❌ Pass-through | ✅ `RunTracePanel` generic event row | Yes | N/A |
| 16 | `observation.created` | ✅ `sub_agent.rs`, `plan_executor.rs` | ❌ Pass-through | ✅ `RunTracePanel` generic event row | Yes | N/A |
| 17 | `fallback.started` | ✅ `lib.rs` | ❌ Pass-through | ✅ `RunTracePanel` generic event row | Yes | N/A |
| 18 | `fallback.completed` | ✅ `lib.rs` | ❌ Pass-through | ✅ `RunTracePanel` generic event row | Yes | N/A |
| 19 | `json_repair.started` | ✅ `generation.rs` | ❌ Pass-through | ✅ `RunTracePanel` generic event row | Yes | N/A |
| 20 | `json_repair.completed` | ✅ `generation.rs` | ❌ Pass-through | ✅ `RunTracePanel` generic event row | Yes | N/A |
| 21 | `compaction.created` | ✅ `compaction.rs` | ❌ Pass-through | ✅ `RunTracePanel` generic event row | Yes | N/A |

### Scale: Tier 3 Plan/Proposal Events (informational, no typed governance)

| # | Event Type | Backend Emission | Frontend Parser | Frontend Display |
|---|-----------|-----------------|-----------------|------------------|
| 22 | `plan.created` | ✅ `plan_mode.rs` | ❌ Pass-through | ✅ `RunTracePanel` generic event row |
| 23 | `plan.confirmation_requested` | ✅ `plan_mode.rs` | ❌ Pass-through | ✅ `RunTracePanel` generic event row |
| 24 | `plan.confirmation_resolved` | 📋 Never emitted | ❌ Pass-through | ✅ `RunTracePanel` generic event row |
| 25 | `plan.execution_started` | ✅ `plan_executor.rs` | ❌ Pass-through | ✅ `RunTracePanel` generic event row |
| 26 | `plan.step_started` | ✅ `plan_executor.rs` | ❌ Pass-through | ✅ `RunTracePanel` generic event row |
| 27 | `plan.step_completed` | ✅ `plan_executor.rs` | ❌ Pass-through | ✅ `RunTracePanel` generic event row |
| 28 | `plan.step_failed` | ✅ `plan_executor.rs` | ❌ Pass-through | ✅ `RunTracePanel` generic event row |
| 29 | `plan.deviation_recorded` | ✅ `plan_executor.rs` | ❌ Pass-through | ✅ `RunTracePanel` generic event row |
| 30 | `plan.execution_completed` | ✅ `plan_executor.rs` (4 paths) | ❌ Pass-through | ✅ `RunTracePanel` generic event row |
| 31 | `plan.execution_failed` | ✅ `plan_executor.rs` (3 paths) | ❌ Pass-through | ✅ `RunTracePanel` generic event row |
| 32 | `plan.cancel_requested` | ✅ `commands/plan.rs` | ❌ Pass-through | ✅ `RunTracePanel` generic event row |
| 33 | `plan.cancelled` | ✅ `commands/plan.rs` | ❌ Pass-through | ✅ `RunTracePanel` generic event row |
| 34 | `plan.retry_requested` | ✅ `commands/plan.rs` | ❌ Pass-through | ✅ `RunTracePanel` generic event row |
| 35 | `plan.retry_started` | ✅ `commands/plan.rs` | ❌ Pass-through | ✅ `RunTracePanel` generic event row |
| 36 | `plan.continuation_requested` | ✅ `commands/plan.rs` | ❌ Pass-through | ✅ `RunTracePanel` generic event row |
| 37 | `plan.action_replayed` | ✅ `commands/plan.rs` | ❌ Pass-through | ✅ `RunTracePanel` generic event row |
| 38 | `plan.action_replay_requested` | ✅ `commands/plan.rs` | ❌ Pass-through | ✅ `RunTracePanel` generic event row |

### Scale: Tier 4 Governance/Context Events (structured metadata)

| # | Event Type | Backend Emission | Frontend Parser | Frontend Display |
|---|-----------|-----------------|-----------------|------------------|
| 39 | `agent_spec.selected` | ✅ `orchestrator.rs`, `streaming.rs`, `execution.rs` | ❌ Pass-through | ✅ `RunTracePanel` generic event row |
| 40 | `prompt_stack.assembled` | ✅ `orchestrator.rs`, `streaming.rs`, `execution.rs` | ❌ Pass-through | ✅ `RunTracePanel` generic event row |
| 41 | `context_governance.applied` | ✅ `orchestrator.rs`, `streaming.rs`, `execution.rs` | ❌ Pass-through | ✅ `RunTracePanel` generic event row |
| 42 | `context.assembled` | 📋 Never emitted | ❌ Pass-through | ✅ `RunTracePanel` generic event row |
| 43 | `proposal.created` | 📋 Never emitted | ❌ Pass-through | ✅ `RunTracePanel` generic event row |

### Scale: Tier 5 Shell Events (frontend-only type labels)

| # | Event Type | Backend Emission | Frontend Parser | Frontend Display |
|---|-----------|-----------------|-----------------|------------------|
| 44 | `shell.blocked` | 🔮 Backend emits as `tool.call_blocked` with `actor: Tool("shell.run")` | ❌ Not parsed separately | ✅ `RunTracePanel` shell-specific block |
| 45 | `shell.completed` | 🔮 Backend emits as `tool.call_completed` with `actor: Tool("shell.run")` | ❌ Not parsed separately | ✅ `RunTracePanel` shell-specific block |

**Note:** `shell.blocked` and `shell.completed` are frontend-only event type strings. The backend Rust enum has no `ShellBlocked` or `ShellCompleted` variants — it uses generic `ToolCallBlocked`/`ToolCallCompleted` with `actor: Tool("shell.run")`. The frontend types.ts declares them as valid strings but they are never received from the backend. This is a **frontend-only forward declaration** — the RunTracePanel handles them as a future-compat path. If the backend ever upgrades to emit dedicated shell events, the frontend is ready.

### Scale: Orphan Events

| # | Event Type | Status |
|---|-----------|--------|
| 43 | `proposal.created` | Defined in Rust enum, defined in frontend types.ts, parsed by event_store parser, but **never emitted** by any production or test code |
| 42 | `context.assembled` | Same — fully defined but never emitted |
| 24 | `plan.confirmation_resolved` | Same — fully defined but never emitted |

---

## 2. Backend Payload Contract Per Event Type

### 2.1 `tool.call_blocked` — Tier 1 Governance

**Must-have typed fields:** `status`, `tool_name`, `source`, and either `block_reason` or `proposal_reason`.

**Emission sites with full typed payload:**

| File | Line | Has `status` | Has `tool_name` | Has `source` | Has `block_reason` | Has `proposal_reason` | Has `agent_spec_id` | Extra fields |
|------|------|-------------|-----------------|-------------|--------------------|------------------------|---------------------|-------------|
| `tool_executor.rs` | 163-178 | ✅ blocked | ✅ | ✅ | ✅ | ❌ null | ✅ | `reason` (text) |
| `tool_executor.rs` | 421-438 | ✅ blocked | ✅ | ✅ | ✅ | ❌ null | ✅ | `target_tool_name`, `target_source`, `wrapper_tool_name` |
| `tool_executor.rs` | 466-483 | ✅ blocked | ✅ | ✅ | ✅ | ❌ null | ✅ | `target_tool_name`, `target_source`, `wrapper_tool_name` |
| `tool_executor.rs` | 792-808 | ✅ blocked | ✅ | ✅ | ✅ | ❌ null | ✅ | `reason` (text) |
| `tool_executor.rs` | 995-1012 | ✅ needs_confirmation | ✅ | ✅ | ❌ null | ✅ | ✅ | `reason` (text), `proposal_id` |
| `tool_executor.rs` | 1599-1606 (shell.run) | ✅ | ✅ | ✅ (auto-injected) | ✅ | ❌ null (auto-injected) | ✅ (auto-injected) | 7 call sites; closure auto-injects `source`/`failure_kind`/`proposal_reason`/`agent_spec_id` |
| `tools.rs` | 54-63 | ✅ blocked | ✅ | ✅ (`"runtime"`) | ✅ (`"invalid_arguments"`) | ❌ null | ❌ null | `max_tool_calls`, `current_count` preserved |
| `plan_executor.rs` | 284-288 | ✅ blocked | ✅ | ✅ (`"plan_executor"`) | ✅ (`"agent_spec_denied"`) | ❌ null | ✅ (`agent_spec_id` — fixed from `agentspec_id`) | `agent_spec_id` key corrected |

**Risk:** None — all production `ToolCallBlocked` emitters now satisfy the standard typed payload contract. The `tools.rs:54` budget exceeded case and `plan_executor.rs:284` AgentSpec deny case were fixed in the Post-Beta Audit.

### 2.2 `replay.started` — Tier 1 Governance

**Must-have typed fields:** `status` ("started"), `run_id`, `action_id`, `replay_of_action_id`, `agent_spec_id`, `tool_name`, `source`.

**Emission site:** `commands/agent.rs:278-292` — single site, uniform payload.

### 2.3 `replay.completed` — Tier 1 Governance

**Must-have typed fields:** `status` ("completed"|"blocked"|"needs_confirmation"), `run_id`, `action_id`, `replay_of_action_id`, `agent_spec_id`, `tool_name`, `source`. Optional: `block_reason`, `proposal_reason`, `failure_kind`.

**Emission site:** `commands/agent.rs:415-433` — single site. Outcome-driven: if blocked, includes `block_reason`; if needs_confirmation, includes `proposal_reason`.

### 2.4 `replay.failed` — Tier 1 Governance

**Must-have typed fields:** At least one of `block_reason`\|`failure_kind`; also `status` ("failed"), `run_id`, `action_id`, `replay_of_action_id`.

**Emission sites:** `commands/agent.rs` — 7 distinct early-failure paths via `record_replay_failed` closure. All include at minimum `status`, `run_id`, `action_id`, `replay_of_action_id`, plus `human_message`. Some include `block_reason`, `failure_kind`, `tool_name`, `source`, `agent_spec_id`.

---

## 3. Frontend Parsing Contract

### 3.1 `parseTypedEventPayload(event) → TypedEventPayload`

**Location:** `frontend/src/utils/typedContract.ts`

Performs structural validation on each known event type. If required fields are missing or have invalid types, returns `{ kind: "unknown" }`.

**Validation rules per event type:**

| Event Type | Required Fields | Required Reason | Strict |
|-----------|----------------|-----------------|--------|
| `tool.call_blocked` | `status` ∈ {"blocked","needs_confirmation"}, `tool_name` (non-empty string), `source` (non-empty string) | If status="blocked" → valid `block_reason`; if status="needs_confirmation" → valid `proposal_reason` | ✅ |
| `replay.started` | `status`="started", `run_id`, `action_id`, `replay_of_action_id`, `agent_spec_id`, `tool_name`, `source` (all non-empty strings) | N/A | ✅ |
| `replay.completed` | `status` ∈ {"completed","blocked","needs_confirmation"}, `run_id`, `action_id`, `replay_of_action_id`, `agent_spec_id`, `tool_name`, `source` | If status="blocked" → valid `block_reason`; if status="needs_confirmation" → valid `proposal_reason` | ✅ |
| `replay.failed` | At least one valid `block_reason`\|`failure_kind`; `run_id`, `action_id`, `replay_of_action_id` | Must have at least one valid reason (not from summary/human_message) | ✅ |
| All others | N/A | N/A | → `{ kind: "unknown" }` |

### 3.2 Display Helpers (internal use only)

**`TypedBadge`** — structured badge view model:
```ts
{ kind: "block_reason"|"proposal_reason"|"failure_kind", label: string, severity: "error"|"warning"|"info", rawReason: string }
```

**Key functions:**
- `getBlockReasonDisplay(reason)` — null for invalid, badge for valid
- `getProposalReasonDisplay(reason)` — null for invalid
- `getFailureKindDisplay(kind)` — null for invalid
- `getTypedReasonBadgesFromEvent(event)` — extracts badges from an AgentRunEvent
- `getTypedOutcomeLabels(outcome)` — labels from extractTypedActionOutcome

### 3.3 View Models (for UI consumption)

**`TypedEventViewModel`** — `getTypedRunEventViewModel(event)` — used by `getTypedRunHints` for RunsPage preview.

**`TypedActionViewModel`** — `getTypedActionViewModel(action)` — used by AgentRunDetail/ToolObservationPanel.

**`TypedToolCallViewModel`** — `getTypedToolCallViewModel(call)` — used by ToolCallCard.

**`TypedEventDetailViewModel`** — `getTypedEventDetailViewModel(event)` — used by RunTracePanel for event detail blocks.

**`TypedProposalHint`** — `getTypedProposalHint(proposal)` — used by ProposalReviewPage for network_policy_ask detection.

---

## 4. UI Display Contract

### 4.1 RunTracePanel

- **Entry:** `getTypedEventDetailViewModel(evt)`
- **DOES NOT:** call `parseTypedEventPayload`, import typed payload types, switch on `typed.kind`
- **Renders:** `TypedEventDetailBlock` unified component
- **For unknown/malformed:** Returns null (no detail block)
- **Consumes:** `TypedBadge` from view model (label and severity via typedContract)

### 4.2 RunsPage

- **Entry:** `getTypedRunHints(events)`
- **Produces:** Typed hints (replay failed + tool blocked) for list preview badges
- **Source of truth:** Event payloads (not action status counts)

### 4.3 AgentRunDetail

- **Entry:** `getTypedActionViewModel` for action rows
- **Replay Trace:** Via `RunTracePanel` only (no separate rendering)

### 4.4 ProposalReviewPage

- **Replay result:** Via `extractTypedActionOutcome` + `getTypedOutcomeLabels`
- **Network policy detection:** Via `getTypedProposalHint` (typed boolean, not text inference)

### 4.5 ToolObservationPanel

- **Entry:** `getTypedActionViewModel` for tool action display
- **Typed reasons shown only when `typedReasonAvailable`**

### 4.6 ToolCallCard

- **Entry:** `getTypedToolCallViewModel` for typed reason display

---

## 5. Degradation Strategy

| Scenario | Behavior |
|----------|----------|
| Typed payload present and valid | Show typed labels + badges via view model |
| Typed payload missing required fields | `parseTypedEventPayload` → `kind: "unknown"` → UI treats as generic event |
| Typed reason is invalid string | `getBlockReasonDisplay` → `null` → badge not shown |
| Typed reason is wrong type (number, boolean, null) | Validator rejects → `null` |
| Event type not recognized by parser | `{ kind: "unknown" }` → generic event row |
| `summary` contains reason text but no typed field | Ignored — no inference |
| `human_message` contains reason text | Shown as auxiliary text only, never drives badges |

---

## 6. Test Coverage Per Contract

### 6.1 typedContract.test.ts

| Contract Point | Happy Path | Malformed | Summary Misleading | Structured Result Priority |
|---------------|-----------|-----------|-------------------|---------------------------|
| `tool.call_blocked` (blocked) | ✅ | ✅ (non-string reason) | ✅ (summary noise ignored) | ✅ |
| `tool.call_blocked` (needs_confirmation) | ✅ | ✅ | ✅ | ✅ |
| `replay.started` | ✅ | ✅ | N/A | N/A |
| `replay.completed` (completed) | ✅ | ✅ | ✅ | ✅ |
| `replay.completed` (blocked) | ✅ | ✅ | ✅ | ✅ |
| `replay.completed` (needs_confirmation) | ✅ | ✅ | ✅ | ✅ |
| `replay.failed` | ✅ | ✅ | ✅ | ✅ |
| Unknown event type | ✅ | — | N/A | N/A |
| `ToolCallBudgetExceeded` (tools.rs:54) | ✅ (now compliant, has typed payload) | — | N/A | N/A |
| `PlanExecutorBlocked` (plan_executor.rs:284) | ✅ (now compliant, has typed payload) | — | N/A | N/A |
| `shell.run` manifest missing (not registered) | ✅ | — | N/A | N/A |

### 6.2 RunTracePanel.test.tsx

| Display Point | Test Present |
|--------------|-------------|
| `tool.call_blocked` typed detail block | ✅ |
| `replay.started` typed detail block | ✅ |
| `replay.completed` typed detail block | ✅ |
| `replay.failed` typed detail block | ✅ |
| Legacy event (no typed payload) → no detail block | ✅ |
| Invalid reason → no raw string display | ✅ |
| MCP wrapper/target fields | ✅ |
| NetworkPolicy ask with proposal_id | ✅ |

### 6.3 Rust event_store.rs tests

| Contract Point | Test Present |
|---------------|-------------|
| Event append + list round-trip | ✅ |
| Events in order | ✅ |
| Different runs isolated | ✅ |
| Parent event linkage | ✅ |
| Payload preserves data | ✅ |
| Redaction round-trip | ✅ |
| Unknown event type preserves | ✅ |
| Multiple unknown events coexist | ✅ |
| Plan execution events round-trip | ✅ |
| Governance events exclude raw prompt | ✅ |
| Cloned store shares connection | ✅ |

### 6.4 Rust action_executor/tool_executor.rs tests

| Contract Point | Test Present |
|---------------|-------------|
| `tool.call_blocked` with agent_spec_denied payload | ✅ (test_agentspec_deny_records_tool_call_blocked_event) |
| `tool.call_blocked` with missing_mcp_server payload | ✅ |
| `tool.call_blocked` with network_policy_ask payload + proposal_id | ✅ |
| `tool.call_blocked` with tool_permission_denied payload | ✅ |
| `tool.call_blocked` with declarative_only payload | ✅ |
| Replay events round-trip end-to-end | ✅ |

---

## 7. Known Gaps & Risk Assessment

### 7.1b Gap: ReplayFailed Early Paths Now Typed (FIXED)

**Status:** ✅ FIXED in Post-Beta Audit.

**Fixed issues in `commands/agent.rs`:**
- Path "Run not found": added `block_reason: ReplaySpecMissing`
- Path "AgentRun store not available": added `failure_kind: "internal_error"`
- Path "Action not found": added `block_reason: ReplaySpecMissing`

All 11 replay.failed emission paths now carry at least one valid typed reason.

### 7.1c Gap: ToolCallBlocked Payload Shape Now Uniform (FIXED)

**Status:** ✅ FIXED in Post-Beta Audit.

**Fixed issues:**
- `tools.rs:54` budget exceeded: added `status: "blocked"`, `tool_name`, `source: "runtime"`, `block_reason: "invalid_arguments"`, `proposal_reason: null`, `failure_kind: null`, `agent_spec_id: null`. Preserved `max_tool_calls`/`current_count`.
- `plan_executor.rs:284` AgentSpec deny: added `status: "blocked"`, `source: "plan_executor"`, `block_reason: "agent_spec_denied"`, `proposal_reason: null`, `failure_kind: null`. Fixed field name `agentspec_id` → `agent_spec_id` (also fixed in PlanExecutionStarted event and 3 corresponding tests).
- `tool_executor.rs` shell.run `record_blocked` closure: now auto-injects `source: "builtin"`, `failure_kind: null`, `proposal_reason: null`, `agent_spec_id` (from ctx). All 7 call sites automatically satisfy the typed contract.

**Remaining:** No known production `ToolCallBlocked` emitters with non-compliant payload shapes.

### 7.2 Gap: shell.blocked / shell.completed Are Frontend-Only Types

**Issue:** Frontend defines `shell.blocked` and `shell.completed` but backend never emits these event type strings. Backend uses `tool.call_blocked`/`tool.call_completed` with `actor: Tool("shell.run")`.

**Risk:** Low — RunTracePanel treats them as future-compat. If backend ever upgrades to dedicated shell events, the frontend is already ready.

**Resolution:** No action needed. Documented.

### 7.3 Gap: Three Orphan Event Types

**Issue:** `proposal.created`, `context.assembled`, `plan.confirmation_resolved` are fully defined but never emitted.

**Risk:** None — they round-trip correctly through the event store and frontend as `kind: "unknown"`.

**Resolution:** Future work — implement emission when the corresponding features are activated.

### 7.4 Gap: Plan/Tool Events in plan_executor.rs Now Compliant (FIXED)

**Status:** ✅ FIXED in Post-Beta Audit.

`plan_executor.rs:284` now emits `ToolCallBlocked` with full typed contract fields: `status: "blocked"`, `tool_name`, `source: "plan_executor"`, `block_reason: "agent_spec_denied"`, `proposal_reason: null`, `failure_kind: null`, `agent_spec_id`. Field name `agentspec_id` corrected to `agent_spec_id` site-wide (including PlanExecutionStarted event and 4 test assertions).

---

## 8. Governance Boundary Summary

| Boundary | Events Involved | UI Guard |
|----------|----------------|----------|
| **AgentSpec** | `tool.call_blocked` with `agent_spec_denied`/`agent_spec_missing`, `agent_spec.selected` | `agentSpecId` in view model, purple badge |
| **Replay** | `replay.started`, `replay.completed`, `replay.failed` | `replayOfActionId` in view model, replay detail block |
| **Network Policy** | `tool.call_blocked` with `network_policy_denied`, `network_policy_ask` | `proposal_reason` badge, proposal_id link |
| **Tool Permission** | `tool.call_blocked` with `tool_permission_denied`, `tool_permission_ask` | `block_reason`/`proposal_reason` badge, amber badge |
| **Privacy** | `context_governance.applied`, redaction fields | Redaction badge in RunTracePanel |
| **Proposal** | `proposal.created` (orphan), NetworkPolicy ask proposals | `proposalId` in view model |
| **Execution Sandbox** | `tool.call_blocked` with `sandbox_denied`, `shell.blocked` (frontend-only) | Shell-specific block display |
| **Plan Governance** | `plan.*` events | Generic event rows only |

---

## 9. Update Log

| Date | Update |
|------|--------|
| 2026-05-16 | **Post-Beta Audit Fixes**: ReplayFailed early paths 1-3 now carry typed reasons; tools.rs budget exceeded now compliant; plan_executor.rs `agentspec_id` → `agent_spec_id` fix; shell.run closure auto-injects contract fields. All production ToolCallBlocked/ReplayFailed emitters now satisfy typed payload contract. |

*Related: `plans/frontend_typed_contract_notes.md`, `plans/openlife_post_beta_roadmap.md`, `plans/current_agent_runtime_audit.md`*
