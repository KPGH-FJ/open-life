# OpenLife Trace Contract Matrix

Date: 2026-05-18

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
| 1 | `tool.call_blocked` | ✅ `tool_executor.rs` (hard NetworkPolicy, ask, sandbox, AgentSpec, mcp target, policy sites), `tools.rs`, `plan_executor.rs` | ✅ `parseTypedEventPayload` → `tool_call_blocked` | ✅ `RunTracePanel` via `TypedEventDetailViewModel` | `status`, `tool_name`, `source`, `block_reason`\|`proposal_reason` | Malformed → `kind: "unknown"`, no typed badge |
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
| 19 | `fallback.failed` | ✅ `lib.rs` | ❌ Pass-through | ✅ `RunTracePanel` generic event row | Yes | N/A |
| 20 | `json_repair.started` | ✅ `generation.rs` | ❌ Pass-through | ✅ `RunTracePanel` generic event row | Yes | N/A |
| 21 | `json_repair.completed` | ✅ `generation.rs` | ❌ Pass-through | ✅ `RunTracePanel` generic event row | Yes | N/A |
| 22 | `compaction.created` | ✅ `compaction.rs` | ❌ Pass-through | ✅ `RunTracePanel` generic event row | Yes | N/A |

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
| 43 | `proposal.created` | ✅ `builder.rs`, `calibration.rs` | ❌ Pass-through | ✅ `RunTracePanel` generic event row |

### Scale: Tier 5 Shell Events (frontend-only type labels)

| # | Event Type | Backend Emission | Frontend Parser | Frontend Display |
|---|-----------|-----------------|-----------------|------------------|
| 44 | `shell.blocked` | 🔮 Backend emits as `tool.call_blocked` with `actor: Tool("shell.run")` | ❌ Not parsed separately | ✅ `RunTracePanel` shell-specific block |
| 45 | `shell.completed` | 🔮 Backend emits as `tool.call_completed` with `actor: Tool("shell.run")` | ❌ Not parsed separately | ✅ `RunTracePanel` shell-specific block |

**Note:** `shell.blocked` and `shell.completed` are frontend-only event type strings. The backend Rust enum has no `ShellBlocked` or `ShellCompleted` variants — it uses generic `ToolCallBlocked`/`ToolCallCompleted` with `actor: Tool("shell.run")`. The frontend types.ts declares them as valid strings but they are never received from the backend. This is a **frontend-only forward declaration** — the RunTracePanel handles them as a future-compat path. If the backend ever upgrades to emit dedicated shell events, the frontend is ready.

### Scale: Orphan Events

| # | Event Type | Status |
|---|-----------|--------|
| 42 | `context.assembled` | Same — fully defined but never emitted |
| 24 | `plan.confirmation_resolved` | Same — fully defined but never emitted |

---

## 2. Backend Payload Contract Per Event Type

### 2.1 `tool.call_blocked` — Tier 1 Governance

**Must-have typed fields:** `status`, `tool_name`, `source`, and either `block_reason` or `proposal_reason`.

**Emission sites — all converged to `build_tool_call_blocked_payload` builder:**

| File | Site | Has `status` | Has `tool_name` | Has `source` | Has `block_reason` | Has `proposal_reason` | Has `agent_spec_id` | Extra fields |
|------|------|-------------|-----------------|-------------|--------------------|------------------------|---------------------|-------------|
| `tool_executor.rs` | AgentSpec deny (Phase 1) | ✅ blocked | ✅ | ✅ | ✅ | ❌ null | ✅ Some(spec.id) | `reason` (text) |
| `tool_executor.rs` | hard NetworkPolicy block (deny / disabled / override / domain block) | ✅ blocked | ✅ | ✅ | ✅ | ❌ null | ✅ Some | `reason` (text) |
| `tool_executor.rs` | mcp.call_tool target AgentSpec deny | ✅ blocked | ✅ | ✅ (`"builtin"`) | ✅ | ❌ null | ✅ Some | `target_tool_name`, `target_source`, `wrapper_tool_name` |
| `tool_executor.rs` | mcp.call_tool target hard block | ✅ blocked | ✅ | ✅ (`"builtin"`) | ✅ | ❌ null | ✅ Some | `target_tool_name`, `target_source`, `wrapper_tool_name` |
| `tool_executor.rs` | handle_blocked (policy) | ✅ blocked/needs_confirmation | ✅ | ✅ | ✅ | ✅\|null | ✅ Some | `reason` (text) |
| `tool_executor.rs` | network_ask_proposal_ex | ✅ needs_confirmation | ✅ | ✅ | ❌ null | ✅ | ✅ Some | `reason` (text), `proposal_id` |
| `tool_executor.rs` | shell.run (7 paths) | ✅ blocked/needs_confirmation | ✅ (`"shell.run"`) | ✅ (`"builtin"`) | ✅ | ✅\|null | ✅ Some | `reason`, `bash_enabled`, `needs_confirmation`, `permission_decision` |
| `tools.rs` | budget exceeded | ✅ blocked | ✅ | ✅ (`"runtime"`) | ✅ (`"invalid_arguments"`) | ❌ null | ❌ **None** | `max_tool_calls`, `current_count` |
| `plan_executor.rs` | AgentSpec deny | ✅ blocked | ✅ | ✅ (`"plan_executor"`) | ✅ (`"agent_spec_denied"`) | ❌ null | ✅ Some | — |

**All sites use `trace_payloads::build_tool_call_blocked_payload` — zero hand-written tool_call_blocked payloads remain in production.

**Risk:** None — all production `ToolCallBlocked` emitters now use `trace_payloads::build_tool_call_blocked_payload`. Zero hand-written payloads remain. Hard NetworkPolicy denials now emit the same typed event as ask/sandbox/permission blocks, so Scheduled / Proactive governed tool paths can prove denial without falling back to Chat. Contract tests and production share the same builder, eliminating desynchronisation risk.

### 2.2 `replay.started` — Tier 1 Governance

**Must-have typed fields:** `status` ("started"), `run_id`, `action_id`, `replay_of_action_id`, `agent_spec_id`, `tool_name`, `source`.

**Emission site:** `commands/agent.rs:278-292` — single site, uniform payload.

### 2.3 `replay.completed` — Tier 1 Governance

**Must-have typed fields:** `status` ("completed"|"blocked"|"needs_confirmation"), `run_id`, `action_id`, `replay_of_action_id`, `agent_spec_id`, `tool_name`, `source`. Optional: `block_reason`, `proposal_reason`, `failure_kind`.

**Emission site:** `commands/agent.rs:415-433` — single site. Outcome-driven: if blocked, includes `block_reason`; if needs_confirmation, includes `proposal_reason`.

### 2.4 `replay.failed` — Tier 1 Governance

**Must-have typed fields:** At least one of `block_reason`|`failure_kind`; also `status` ("failed"), `run_id`, `action_id`, `replay_of_action_id`.

**Emission sites:** `commands/agent.rs` — 7 early-failure paths via `record_replay_failed` closure + 1 execution-outcome path when `exec_result.status == Failed`. All use `build_replay_failed_payload`. Early-failure paths include `human_message`; some include `block_reason`, `failure_kind`, `tool_name`, `source`, `agent_spec_id`. Outcome path: priority `block_reason` → `failure_kind` → fallback `internal_error`; `tool_name`/`source`/`agent_spec_id` via `extra`.

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

## 6.5 Backend Contract Tests (Trace Explainability)

> **Production payload builder:** `openlife-core/src/agent/trace_payloads.rs`
> **Contract helpers:** `openlife-core/src/agent/tests/contract_helpers.rs`
> **Contract tests:** `openlife-core/src/agent/event_store.rs`

The contract tests no longer use hand-written `serde_json::json!({...})` payloads.  Instead, they call the same `trace_payloads::build_*()` functions used by the real production emit sites:

| Emit site | Builder used |
|-----------|-------------|
| `src-tauri/src/streaming.rs` | `build_agent_spec_selected_payload`, `build_prompt_stack_assembled_payload`, `build_context_governance_applied_payload` (StreamingExecution) |
| `src-tauri/src/commands/execution.rs` | `build_agent_spec_selected_payload`, `build_prompt_stack_assembled_payload`, `build_context_governance_applied_payload` (StreamingExecution) |
| `openlife-core/src/agent/agent_loop/orchestrator.rs` | `build_agent_spec_selected_payload`, `build_prompt_stack_assembled_payload`, `build_context_governance_applied_payload` (Orchestrator) |
| `src-tauri/src/commands/agent.rs` | `build_replay_started_payload`, `build_replay_completed_payload`, `build_replay_failed_payload` |
| `openlife-core/src/agent/action_executor/tool_executor.rs` | `build_tool_call_blocked_payload` (7 production sites: AgentSpec deny, mcp target block x2, policy block, NetworkPolicy ask, shell.run x7) |
| `openlife-core/src/agent/agent_loop/tools.rs` | `build_tool_call_blocked_payload` (1 site: budget exceeded) |
| `openlife-core/src/agent/plan_executor.rs` | `build_tool_call_blocked_payload` (1 site: AgentSpec deny) |

A change to any builder function is **immediately reflected** in both production and test payloads.  Hand-written JSON round-trip tests in `event_store.rs` are no longer the sole contract proof.

### 6.5.1 Coverage Matrix

| Contract Test | Event Types Covered | Production Builder Used |
|--------------|-------------------|------------------------|
| `test_agent_spec_selected_payload_contract` | `agent_spec.selected` | `build_agent_spec_selected_payload` |
| `test_prompt_stack_assembled_payload_contract` | `prompt_stack.assembled` | `build_prompt_stack_assembled_payload` |
| `test_context_governance_applied_payload_contract` | `context_governance.applied` | `build_context_governance_applied_payload` (both emitter variants) |
| `test_tool_call_blocked_typed_payload_contract` | `tool.call_blocked` (4 variants) | `build_tool_call_blocked_payload` — **now builder-driven** (AgentSpec deny, NetworkPolicy ask, MCP target block, budget exceeded with `agent_spec_id: null`) |
| `test_replay_failed_events_have_typed_reason` | `replay.failed` (4 variants) | `build_replay_failed_payload` — **now builder-driven** (replay_spec_missing x2, internal_error x2) |
| `test_generic_failure_events_round_trip` | `model.failed`, `model.call_failed`, `tool.call_failed`, `run.failed` | `build_model_failed_payload`, `build_model_call_failed_payload`, `build_tool_call_failed_payload`, `build_run_failed_payload` |
| `test_tool_call_blocked_rejects_invalid_enum_reason` | `tool.call_blocked` (invalid reason) | `build_tool_call_blocked_payload` with `"not_a_real_enum_variant"` — rejected by `assert_no_typed_reason` |
| `test_replay_failed_rejects_invalid_enum_reason` | `replay.failed` (invalid reason) | `build_replay_failed_payload` with `"not_a_real_enum_variant"` — rejected by `assert_no_typed_reason` |
| `test_tool_call_blocked_rejects_null_reasons` | `tool.call_blocked` (null reasons) | `build_tool_call_blocked_payload` with `None` reasons — rejected |
| `test_replay_failed_rejects_null_reasons` | `replay.failed` (null reasons) | `build_replay_failed_payload` with `None` reasons — rejected |
| `test_tool_call_blocked_with_valid_block_reason_passes` | `tool.call_blocked` (valid reason) | `build_tool_call_blocked_payload` with `"agent_spec_denied"` — passes `assert_has_typed_reason` |
| `test_replay_failed_with_valid_reason_passes` | `replay.failed` (valid reason) | `build_replay_failed_payload` with `"replay_spec_missing"` — passes `assert_has_typed_reason` |

### 6.5.2 Typed Reason Enum Validation

The contract helpers (`assert_has_typed_reason`, `assert_no_typed_reason`) now validate against the **production enum variant strings** defined in `trace_payloads.rs`:

| Field | Valid Values |
|-------|-------------|
| `block_reason` | `agent_spec_denied`, `agent_spec_missing`, `network_policy_denied`, `domain_blocked`, `tool_permission_denied`, `missing_mcp_client`, `disabled_manifest`, `declarative_only`, `sandbox_denied`, `path_not_safe`, `invalid_arguments`, `replay_spec_missing`, `pii_detected`, `unknown` |
| `proposal_reason` | `network_policy_ask`, `tool_permission_ask`, `high_risk_action` |
| `failure_kind` | `tool_runtime_error`, `mcp_client_error`, `missing_mcp_server`, `internal_error`, `serialization_error` |

Values like `"not_a_real_enum_variant"`, empty strings, and `"null"` are **rejected** — matching the frontend `typedContract.ts` parser behaviour.  This prevents desynchronisation between backend and frontend validation.

### 6.5.3 Contract Helpers

**File:** `openlife-core/src/agent/tests/contract_helpers.rs`

| Helper | Signature | Purpose |
|--------|-----------|---------|
| `assert_has_string` | `(payload, field)` | Assert non-empty string field present |
| `assert_has_optional_string_or_null` | `(payload, field)` | Assert field exists and value is non-empty string or `null` (e.g. `agent_spec_id` in `tool.call_blocked` when no AgentSpec in scope) |
| `assert_has_array` | `(payload, field)` | Assert non-empty array field present |
| `assert_has_array_allow_empty` | `(payload, field)` | Assert array field present (may be empty) |
| `assert_array_items_have_field` | `(payload, array_field, item_field)` | Assert each array element has sub-field |
| `assert_has_typed_reason` | `(payload, candidates)` | Assert at least one valid typed reason **with recognised enum value** |
| `assert_no_typed_reason` | `(payload, candidates)` | Assert no valid typed reason (for malformed payload verification) |
| `assert_field_absent` | `(payload, field)` | Assert field does NOT exist in payload |

### 6.5.4 Cross-Layer Alignment

Field names are anchored in **backend Rust snake_case** as the authoritative source:

| Backend Rust | Frontend Fixture | Frontend camelCase Fallback |
|-------------|-----------------|---------------------------|
| `agent_spec_id` | `agent_spec_id` | `agentSpecId` (legacy) |
| `prompt_blocks` | `prompt_blocks` | — |
| `privacy_policy` | `privacy_policy` | `privacyPolicy` (legacy) |
| `agent_spec_privacy_policy` | `agent_spec_privacy_policy` | — |
| `block_reason` | `block_reason` | — |
| `proposal_reason` | `proposal_reason` | — |
| `failure_kind` | `failure_kind` | — |

**Anti-patterns prevented:**
- No `prompt_stack_id` / `promptStackId` field.
- No inference of typed reasons from `summary` / `human_message` text.
- Invalid enum variants like `"not_a_real_enum_variant"` are rejected by both backend helper and frontend parser.

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

### 7.1d Gap: ToolCallBlocked Production Builder Convergence (FIXED — this round)

**Status:** ✅ FIXED in Post-Beta Stabilization — 2026-05-18.

**Background:** The `tool.call_blocked` payloads were the last remaining production emission sites that used hand-written `serde_json::json!({...})` instead of delegating to `trace_payloads::build_tool_call_blocked_payload`. While all sites already satisfied the typed contract, the payload construction logic was duplicated across 3 files and 15 call sites. This created risk of desynchronisation between production and contract test shapes.

**Changes in this round:**
- `trace_payloads::build_tool_call_blocked_payload` signature updated: `agent_spec_id` from `impl Into<String>` → `Option<impl Into<String>>`. When `None`, serialised as `serde_json::Value::Null` — matching the budget-exceeded path where no AgentSpec is in scope.
- **7 production emit sites in `tool_executor.rs`** migrated to the builder:
  - AgentSpec deny (Phase 1)
  - mcp.call_tool target AgentSpec deny
  - mcp.call_tool target hard block (ToolPermissionDenied, PiiDetected, etc.)
  - `handle_blocked` general policy block (DeclarativeOnly, DisabledManifest, ToolPermissionDenied, PiiDetected, Unknown)
  - `network_ask_proposal_ex` NetworkPolicy ask + proposal_id
  - `execute_shell_run` `record_blocked` closure replaced with `emit_blocked` helper that delegates to builder. All 7 shell.run block sites (missing manifest, disabled, declarative-only, sandbox deny, AgentSpec deny, AgentSpec missing, permission deny/ask) now use the builder.
- **1 site in `agent_loop/tools.rs`**: budget exceeded now uses builder with `agent_spec_id: None`.
- **1 site in `plan_executor.rs`**: AgentSpec deny now uses builder with `agent_spec_id: Some(spec.id)`.
- **0 remaining production emission sites** emit `tool.call_blocked` with hand-written payloads.

**`agent_spec_id` contract semantics:**
| Value | Serialised As | When Used |
|-------|-------------|-----------|
| `Some(id)` | `"agent_spec_id": "<id>"` (string) | Normal governed paths: AgentSpec deny, permission deny, sandbox deny, etc. |
| `None` | `"agent_spec_id": null` | No AgentSpec in scope: budget exceeded (AgentLoop runtime), may also appear in shell.run AgentSpec-missing gate |
| Frontend parser | Accepts `string \| null` | `null` treated as absent agent spec; no badge generated |

**`extra` field merge semantics:**
The builder uses `BTreeMap::entry(k).or_insert(v)` when merging `extra` — core fields (`status`, `tool_name`, `source`, `agent_spec_id`, `block_reason`, `proposal_reason`, `failure_kind`) are never overwritten. Extra-only fields (`proposal_id`, `reason`, `target_tool_name`, `wrapper_tool_name`, `max_tool_calls`, `needs_confirmation`, `permission_decision`, `bash_enabled`) are safely injected.

**Tests added (9 new):**
- `builder_tool_call_blocked_none_agent_spec_id_passes_contract` — `agent_spec_id: None` passes contract_helpers
- `test_tool_call_blocked_none_agent_spec_id_passes` — event_store round-trip with null agent_spec_id
- `test_tool_call_blocked_some_agent_spec_id_passes` — event_store round-trip with string agent_spec_id
- `tool_call_blocked_event_payload_has_contract_fields` updated to validate through contract_helpers
- `network_policy_ask_event_payload_has_contract_fields` updated to validate through contract_helpers
- `shell_blocked_event_payload_has_contract_fields` updated to validate through contract_helpers
- `shell_run_manifest_missing_records_typed_tool_call_blocked` updated to validate through contract_helpers
- Existing `test_allowed_block_reasons_known` / `test_rejects_invalid_enum_variant` / `test_rejects_null_and_empty` remain intact
- All contract_helpers tests (`valid_typed_reasons_pass` / `invalid_block_reason_fails` / etc.) remain intact

### 7.2 Gap: shell.blocked / shell.completed Are Frontend-Only Types

**Issue:** Frontend defines `shell.blocked` and `shell.completed` but backend never emits these event type strings. Backend uses `tool.call_blocked`/`tool.call_completed` with `actor: Tool("shell.run")`.

**Risk:** Low — RunTracePanel treats them as future-compat. If backend ever upgrades to dedicated shell events, the frontend is already ready.

**Resolution:** No action needed. Documented.

### 7.3 Gap: Two Orphan Event Types

**Issue:** `context.assembled` and `plan.confirmation_resolved` are fully defined but never emitted. `proposal.created` is no longer orphaned: Builder proposal creation and Calibration Proposal-first creation emit metadata-only events.

**Risk:** None — they round-trip correctly through the event store and frontend as `kind: "unknown"`.

**Resolution:** Future work — implement emission for the remaining orphan events when the corresponding features are activated.

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
| **Proposal** | `proposal.created`, NetworkPolicy ask proposals | `proposalId` in view model |
| **Execution Sandbox** | `tool.call_blocked` with `sandbox_denied`, `shell.blocked` (frontend-only) | Shell-specific block display |
| **Plan Governance** | `plan.*` events | Generic event rows only |

---

## 9. Trace Contract Drift Audit

### 9.1 Purpose

The trace contract drift audit is a **CI-enforceable source-code scanning test** that prevents developers from bypassing `trace_payloads::build_*_payload` builders when emitting typed governance events. It is a lightweight, deterministic, *no-runtime* test — it reads source files at test time and performs text-level scanning, similar to a linter targeted at the typed payload contract.

**This audit does NOT:**
- Replace runtime contract tests (`event_store.rs`, `contract_helpers.rs`)
- Scan generic events (`RunCreated`, `ToolCallStarted`, etc.)
- Block normal `serde_json::json!` usage outside governance events
- Parse Rust AST — it uses simple text-window scanning on sanitised source

**This audit DOES:**
- Flag any production emission of the audited governance event types if their payload is not constructed via the required builder
- Run in CI as part of `cargo test -p openlife-core trace_contract_audit`
- Include both positive and negative unit tests on synthetic snippets
- **Mask Rust comments and string literals** before scanning, preventing tokens hidden inside comments/strings from creating false passes or false-positive emission counts

### 9.2 Implementation

**File:** `openlife-core/src/agent/tests/trace_contract_audit.rs`

**Test name:** `all_production_files_use_required_builders`

**Integration point:** `openlife-core/src/agent/tests/mod.rs`

**Scanning algorithm:**
1. Read each target production file relative to workspace root.
2. Split at the last `#[cfg(test)]` marker (detected on **original** source) — everything after is excluded.
3. **Sanitise** the production source: Rust line comments (`// …`), block comments (`/* … */`), regular string literals (`"…"` with escape handling), and raw string literals (`r"…"`, `r#"…"#`, …) are masked with spaces (newlines preserved).
4. Locate every occurrence of the event enum token in the **sanitised** source only.
5. Build a symmetric window around each occurrence on the sanitised source.
6. Check that at least one required builder name appears in the window (on sanitised source).
7. If no builder found → **violation** (test fails).
8. If `expected_emissions` is set and count ≠ expected → **violation**.
9. If `expected_emissions` is `None` and count < `min_emissions` → **warning**.

**Source sanitisation details:**
- `// …` — line comment content replaced with spaces, trailing newline preserved.
- `/* … */` — block comment content replaced with spaces (newlines inside preserved). Non-nested; unterminated block comments masked through end-of-input.
- `"…"` — regular string literal content replaced with spaces. Escape sequences (`\\`, `\"`, `\n`, etc.) consume two positions.
- `r"…"` / `r#"…"#` / `r##"…"##` — raw string literal content replaced with spaces. Closing delimiter detected by matching hash count.
- Snippet output for violations uses the **original** (unsanitised) source for developer readability.

### 9.3 Audited Events & Builders

| Event | Required Builder(s) | Production Files Scanned | Exact Count | Window |
|-------|-------------------|------------------------|-------------|--------|
| `AgentRunEventType::ToolCallBlocked` | `build_tool_call_blocked_payload` | `tool_executor.rs` | 6 | 1200 |
| `AgentRunEventType::ToolCallBlocked` | `build_tool_call_blocked_payload` | `tools.rs` | 1 | 900 |
| `AgentRunEventType::ToolCallBlocked` | `build_tool_call_blocked_payload` | `plan_executor.rs` | 1 | 900 |
| `AgentRunEventType::ReplayFailed` | `build_replay_failed_payload` | `src-tauri/src/commands/agent.rs` | 2 | 1500 |
| `AgentRunEventType::ReplayStarted` | `build_replay_started_payload` | `src-tauri/src/commands/agent.rs` | 1 | 900 |
| `AgentRunEventType::ReplayCompleted` | `build_replay_completed_payload` | `src-tauri/src/commands/agent.rs` | 1 | 1800 |
| `AgentRunEventType::AgentSpecSelected` | `build_agent_spec_selected_payload` | `streaming.rs`, `execution.rs`, `orchestrator.rs` | 1 each | 900 |
| `AgentRunEventType::PromptStackAssembled` | `build_prompt_stack_assembled_payload` | `streaming.rs`, `execution.rs`, `orchestrator.rs` | 1 each | 900 |
| `AgentRunEventType::ContextGovernanceApplied` | `build_context_governance_applied_payload` | `streaming.rs`, `execution.rs`, `orchestrator.rs` | 1 each | 900 |

**Window policy (May 2026 revision):**
- **Default window: ±900 chars** (previously ±1500). In practice builder calls are 1–10 lines from the event enum token; 900 is conservative.
- **tool_executor.rs: ±1200** — larger file (~5.4k lines) with deeply-nested helper closures.
- **agent.rs ReplayFailed: ±1500** — event token in early `let event_type = if …` expression, builder ~21 lines later inside nested `if let Some(ref event_store)` block (~1365 chars).
- **agent.rs ReplayCompleted: ±1800** — longest span: event token at L428 in `let event_type = if …`, builder at L457 inside deeply nested payload branch (~1740 chars).

**Exact emission count (`expected_emissions`):**
- Every audit rule now specifies the exact number of production-code event occurrences.
- If the count changes (additions or removals), the audit **fails** — forcing a deliberate rule update and preventing silent drift.
- `min_emissions` is retained as a fallback for future rules whose count is legitimately variable (not used by any current rule).

### 9.4 Negative / Unit Tests

The audit module includes 26 tests (10 original synthetic tests + 10 new sanitisation negative tests + 5 original + 1 count-mismatch test):

**Positive/negative on compliance (original):**

| Test | Verifies |
|------|----------|
| `negative_tool_call_blocked_without_builder_fails` | Event token present, builder absent → violation |
| `positive_tool_call_blocked_with_builder_passes` | Both present → no violation |
| `negative_replay_failed_without_builder_fails` | ReplayFailed without builder → violation |
| `negative_replay_failed_hand_written_serialized_payload_fails` | Hand-written `json!` without builder → violation |
| `negative_replay_failed_using_completed_builder_is_violation` | ReplayFailed + `build_replay_completed_payload` = violation |
| `positive_replay_failed_with_correct_builder_passes` | ReplayFailed + `build_replay_failed_payload` = passes |
| `positive_replay_completed_with_builder_passes` | ReplayCompleted + builder = passes |
| `positive_context_governance_applied_with_builder_passes` | ContextGovernanceApplied + builder = passes |
| `negative_agent_spec_selected_without_builder_fails` | AgentSpecSelected without builder → violation |
| `builder_too_far_outside_window_is_violation` | Builder outside ±window → violation |
| `test_region_content_is_excluded` | Test code after `#[cfg(test)]` is correctly excluded |
| `audit_does_not_flag_unrelated_serde_json_invocation` | Normal `json!` usage not flagged |
| `expected_emissions_fails_on_count_mismatch` | Exact count mismatch → violation |

**Source sanitisation negative tests (new in May 2026 revision):**

| Test | What false pass / false positive it prevents |
|------|---------------------------------------------|
| `event_token_in_line_comment_is_ignored` | Event token `// AgentRunEventType::ToolCallBlocked` → not counted, no violation |
| `event_token_in_block_comment_is_ignored` | Event token `/* … AgentRunEventType::ToolCallBlocked … */` → not counted |
| `event_token_in_string_literal_is_ignored` | Event token `"AgentRunEventType::ToolCallBlocked"` in string → not counted |
| `builder_in_line_comment_does_not_pass_real_event` | Builder `// build_tool_call_blocked_payload` + json! event → still violates |
| `builder_in_block_comment_does_not_pass_real_event` | Builder `/* build_tool_call_blocked_payload */` + json! event → still violates |
| `builder_in_string_literal_does_not_pass_real_event` | Builder `"build_tool_call_blocked_payload"` in string + json! event → still violates |
| `builder_in_raw_string_does_not_pass_real_event` | Builder `r#"build_tool_call_blocked_payload"#` in raw string + json! event → still violates |
| `builder_in_raw_string_double_hash_does_not_pass` | Builder `r##"build_tool_call_blocked_payload"##` → still violates |
| `string_with_escaped_quote_is_fully_masked` | `"foo \" bar"` escape handling — token after `\"` not leaked |
| `test_region_detection_ignores_cfg_test_in_comment` | `// #[cfg(test)]` comment → not mistaken for real test split |
| `adjacent_event_with_different_builder_not_masked` | Adjacent AgentSpecSelected builder + ToolCallBlocked json! → ToolCallBlocked audit still fails |
| `min_emissions_warning_when_too_few_production_emissions` | `min_emissions` fallback works (when `expected_emissions` is `None`) |

### 9.5 How to Add a New Event to the Audit

When a new typed governance event type is introduced and must use a specific builder:

1. Add a new `AuditRule::new(...)` entry in `audit_rules()` in `trace_contract_audit.rs`.
2. Set `expected_emissions: Some(N)` to the exact number of production emission sites. For files whose count is legitimately variable, use `expected_emissions: None` with `min_emissions` — add a comment explaining why.
3. Set `required_builders: &["..."].` The slice supports multiple builders if an event has genuinely distinct valid construction paths, but in practice each event should map to a single canonical builder. Multi-builder rules require explicit justification.
4. Choose `window_chars`: start with 900 (default). Only increase if the builder is far from the event token — document the measured distance in a code comment.
5. Run `cargo test -p openlife-core trace_contract_audit` to verify.
6. Run `make ci` for full regression.

### 9.6 Failure Messages

When a violation is found, the test assertion produces a readable message:

```
VIOLATION in src-tauri/src/commands/agent.rs:426 — `AgentRunEventType::ReplayFailed`
  emitted but none of ["build_replay_failed_payload"]
  found within ±1500 chars (sanitised source).
  near: ...[code snippet around the violation]...
```

Count mismatches produce:

```
COUNT MISMATCH in openlife-core/src/agent/action_executor/tool_executor.rs — expected
exactly 6 production emissions of `AgentRunEventType::ToolCallBlocked`, found 7.
The audit rule may be stale or emissions were added/removed without updating the rule.
```

Both are sufficient for a developer to locate and fix the drift.

---

## 9.5 AgentRunEvent Contract Coverage

### 9.5.1 Purpose

The **event contract coverage manifest** (`event_contract_manifest()` in `trace_contract_audit.rs`) is the single source of truth that classifies every `AgentRunEventType` variant.  Its goals:

1. **No orphan variants** — every enum variant must have an explicit classification and a reason.
2. **No drift between manifest and audit rules** — every `ProductionAudited` entry must have a matching `AuditRule`; every `AuditRule` must have a matching `ProductionAudited` manifest entry.
3. **No silent additions** — if a developer adds a new `AgentRunEventType` variant without updating the manifest, `all_enum_variants_have_manifest_entry` fails.
4. **No empty reasons** — `IntentionallyExcluded` events must document *why*.

### 9.5.2 Classification Tiers

| Tier | `EventContractStatus` | Meaning | What's required |
|------|-----------------------|---------|-----------------|
| 1 | **ProductionAudited** | Event has a typed payload builder in `trace_payloads.rs` **and** at least one `AuditRule`. | `source_file`, `builder`, `expected_emissions`, `window_chars` per audit rule. |
| 2 | **IntentionallyExcluded** | Event has a typed builder but is not (yet) in the audit. | Non-empty `reason` explaining exclusion. No `AuditRule` required. |
| 3 | **LegacyInternalOnly** | Runtime lifecycle / infrastructure event; no typed governance payload. | Non-empty `reason`. No `AuditRule` required. |
| 4 | **TypeOnlyNoDirectEmission** | Enum variant never directly emitted by production code. | Non-empty `reason`. No `AuditRule` required. |

### 9.5.3 Current Event Classification Summary

| Tier | Count | Events |
|------|-------|--------|
| ProductionAudited | 7 | ToolCallBlocked, ReplayFailed, ReplayStarted, ReplayCompleted, AgentSpecSelected, PromptStackAssembled, ContextGovernanceApplied |
| IntentionallyExcluded | 8 | ModelFailed, RunFailed, ToolCallFailed, ModelCallFailed, ProposalCreated, FallbackStarted, FallbackCompleted, FallbackFailed |
| LegacyInternalOnly | 29 | RunCreated, ContextAssembled, ModelRouteSelected, ModelCallStarted, ModelCallCompleted, ToolCallStarted, ToolCallCompleted, ObservationCreated, JsonRepairStarted, JsonRepairCompleted, RunCompleted, CompactionCreated, PlanCreated, PlanConfirmationRequested, PlanConfirmationResolved, PlanExecutionStarted, PlanStepStarted, PlanStepCompleted, PlanStepFailed, PlanDeviationRecorded, PlanExecutionCompleted, PlanExecutionFailed, PlanCancelRequested, PlanCancelled, PlanRetryRequested, PlanRetryStarted, PlanContinuationRequested, PlanActionReplayed, PlanActionReplayRequested |
| TypeOnlyNoDirectEmission | 1 | Unknown |
| **Total** | **45** | |

### 9.5.4 Admission Rules for Each Tier

**ProductionAudited:**
- Must have a typed payload builder in `trace_payloads.rs`.
- Must have at least one `AuditRule` in `audit_rules()` with `event_token`, `file_rel_path`, `required_builders`, `expected_emissions`, `window_chars`.
- Must have `production_rule_tokens` in the manifest matching the audit rule tokens.
- Change in production emission count → CI fails (exact count mismatch).

**IntentionallyExcluded:**
- Must have a non-empty `reason` in the manifest explaining why the event is excluded from the audit.
- A typed builder is optional but typical for these events.

**LegacyInternalOnly:**
- Must have a non-empty `reason` in the manifest.
- No typed builder expected.
- No audit rule expected.

**TypeOnlyNoDirectEmission:**
- Must have a non-empty `reason` in the manifest.
- No typed builder expected.
- No audit rule expected.
- Examples: `Unknown(String)` variant for forward compatibility.

### 9.5.5 Developer Workflow for New Events

1. Add the new `AgentRunEventType` variant to `types/mod.rs`.
2. **If it needs a typed payload:** create a `build_*_payload` function in `trace_payloads.rs`.
3. **If it's a governance event (Tier 1):**
   - Add one or more `AuditRule` entries in `audit_rules()`.
   - Add an `EventContractEntry::ProductionAudited` in `event_contract_manifest()` with the matching `production_rule_tokens`.
4. **If it's a generic event (Tier 2):** add an `IntentionallyExcluded` entry with a reason.
5. **If it's a lifecycle event (Tier 3):** add a `LegacyInternalOnly` entry with a reason.
6. **If it's type-only (Tier 4):** add a `TypeOnlyNoDirectEmission` entry.
7. Add positive/negative contract tests in `event_store.rs` and/or `trace_contract_audit.rs`.
8. Run `cargo test -p openlife-core trace_contract_audit` to verify.
9. Update this document (Section 9.5.3 count and Section 10 changelog).
10. Run `make ci`.

### 9.5.6 Coverage Enforcement Tests

All event contract tests are prefixed `event_contract_` and live in `trace_contract_audit.rs`. Run them with:

```
cargo test -p openlife-core event_contract -- --nocapture
```

**Positive tests (call validators on real data):**

| Test | Verifies |
|------|----------|
| `event_contract_all_enum_variants_have_manifest_entry` | Every `AgentRunEventType` variant has a manifest entry; no stale entries |
| `event_contract_production_audited_events_have_audit_rules` | Every `ProductionAudited` entry has matching `AuditRule`(s); every audit rule has a `ProductionAudited` manifest entry; counts match |
| `event_contract_intentionally_excluded_have_reason` | No `IntentionallyExcluded` entry has an empty reason |
| `event_contract_no_duplicate_events` | No duplicate event names in manifest |
| `event_contract_document_matches_manifest` | Parses `trace_contract_matrix.md` Section 9.5.3 table and verifies: tier events match manifest, counts match, Total row matches, no forbidden stale strings |
| `event_contract_parse_enum_finds_45_variants` | Enum parser finds exactly 45 variants |
| `event_contract_parse_enum_sanitised_no_paren_in_names` | Parser strips tuple data (no parenthesised names) |

**Negative tests (construct bad input, call validators, assert failure):**

| Test | What it proves |
|------|---------------|
| `event_contract_missing_manifest_entry_fails` | Enum variant without manifest entry → `validate_manifest_against_enum` fails |
| `event_contract_stale_manifest_entry_fails` | Manifest entry for removed variant → fails with "stale" |
| `event_contract_production_audited_without_rule_fails` | `ProductionAudited` with non-existent rule token → `validate_manifest_against_audit_rules` fails |
| `event_contract_audit_rule_without_production_manifest_fails` | Audit rule for event demoted from `ProductionAudited` → fails |
| `event_contract_intentionally_excluded_empty_reason_fails` | `IntentionallyExcluded` with empty reason → fails with "empty reason" |
| `event_contract_duplicate_in_manifest_fails` | Duplicate event name in manifest → `validate_no_duplicate_events` fails |
| `event_contract_document_missing_summary_table_fails` | No 9.5.3 table in doc → `validate_document_against_manifest` fails with "classification summary" |
| `event_contract_document_missing_total_row_fails` | Table has tier rows but no Total row → fails with "Total row" |
| `event_contract_document_total_mismatch_fails` | All tier rows correct but Total row is 99 → fails with "99" |
| `event_contract_document_duplicate_event_fails` | Same event twice in one tier → fails with "duplicate" |
| `event_contract_document_production_list_mismatch_fails` | Doc `ProductionAudited` count 99 but manifest has 7 → fails |
| `event_contract_document_forbidden_string_fails` | Document contains the stale string "all 45 variants" → fails with "forbidden" |

> **Note:** `cargo test -p openlife-core trace_contract_audit` runs all tests — builder-audit (26), event contract (19), and payload builder contract (12). `cargo test -p openlife-core event_contract` runs only the 19 event contract tests. `cargo test -p openlife-core payload_builder_contract` runs only the 12 payload builder contract tests.

---

## 9.6 Typed Payload Builder Contract Coverage

### 9.6.1 Purpose

While the **event contract manifest** (Section 9.5) classifies every `AgentRunEventType` variant into tiers, and the **AuditRule** (Section 9.3) enforces that production emit sites use typed builders, neither of these layers verifies that the **builder functions themselves** are correctly tracked. The builder contract coverage audit closes this gap.

**Why builder coverage is needed:**

| Layer | What it guarantees | File |
|-------|-------------------|------|
| **Event manifest** (`event_contract_manifest`) | Every `AgentRunEventType` variant is classified; no orphan variants | `trace_contract_audit.rs` |
| **AuditRule** (`audit_rules`) | Production emit sites use typed builders; no hand-written payloads | `trace_contract_audit.rs` |
| **Builder coverage** (`payload_builder_contract_manifest`) | Every `build_*_payload` in `trace_payloads.rs` is manifested, mapped to the correct event, has the correct status, and references real contract tests | `trace_contract_audit.rs` |

Without builder coverage, a developer could:
1. Add a new `build_*_payload` function without updating the manifest — contract tests would pass but the builder would be untracked.
2. Declare a stale builder in the manifest that no longer exists — the manifest would lie about what is tracked.
3. Map a builder to the wrong event status — `ProductionAudited` vs `IntentionallyExcluded` contract would desync.
4. Reference nonexistent contract tests — required tests might silently disappear after a refactor.

### 9.6.2 Builder Classification Tiers

| Status | Meaning | Events |
|--------|---------|--------|
| **ProductionAudited** | Governance event — builder in production source audit (has `AuditRule`) | ToolCallBlocked, ReplayFailed, ReplayStarted, ReplayCompleted, AgentSpecSelected, PromptStackAssembled, ContextGovernanceApplied |
| **IntentionallyExcludedGenericFailure** | Generic failure or metadata-only lifecycle event — typed builder exists but is excluded from production source audit | ModelFailed, RunFailed, ToolCallFailed, ModelCallFailed, ProposalCreated, FallbackStarted, FallbackCompleted, FallbackFailed |
| **LegacyNoTypedBuilder** | Lifecycle/internal event with no typed builder | (none currently declared in builder manifest) |
| **TypeOnlyNoBuilder** | Catch-all variant with no typed builder (e.g. `Unknown`) | (none currently declared in builder manifest) |

### 9.6.3 Current Builder Coverage Table

| Event | Builder | Status | Required Contract Tests |
|-------|---------|--------|------------------------|
| ToolCallBlocked | `build_tool_call_blocked_payload` | ProductionAudited | `test_tool_call_blocked_typed_payload_contract`, `test_builders_produce_snake_case_fields` |
| ReplayFailed | `build_replay_failed_payload` | ProductionAudited | `test_replay_failed_events_have_typed_reason`, `test_builders_produce_snake_case_fields` |
| ReplayStarted | `build_replay_started_payload` | ProductionAudited | `test_builders_produce_snake_case_fields` |
| ReplayCompleted | `build_replay_completed_payload` | ProductionAudited | `test_builders_produce_snake_case_fields` |
| AgentSpecSelected | `build_agent_spec_selected_payload` | ProductionAudited | `test_agent_spec_selected_payload_contract`, `test_builders_produce_snake_case_fields` |
| PromptStackAssembled | `build_prompt_stack_assembled_payload` | ProductionAudited | `test_prompt_stack_assembled_payload_contract`, `test_builders_produce_snake_case_fields` |
| ContextGovernanceApplied | `build_context_governance_applied_payload` | ProductionAudited | `test_context_governance_applied_payload_contract`, `test_builders_produce_snake_case_fields` |
| ModelFailed | `build_model_failed_payload` | IntentionallyExcludedGenericFailure | `test_generic_failure_events_round_trip`, `test_builders_produce_snake_case_fields` |
| RunFailed | `build_run_failed_payload` | IntentionallyExcludedGenericFailure | `test_generic_failure_events_round_trip`, `test_builders_produce_snake_case_fields` |
| ToolCallFailed | `build_tool_call_failed_payload` | IntentionallyExcludedGenericFailure | `test_generic_failure_events_round_trip`, `test_builders_produce_snake_case_fields` |
| ModelCallFailed | `build_model_call_failed_payload` | IntentionallyExcludedGenericFailure | `test_generic_failure_events_round_trip`, `test_builders_produce_snake_case_fields` |
| ProposalCreated | `build_proposal_created_payload` | IntentionallyExcludedGenericFailure | `test_builders_produce_snake_case_fields` |
| FallbackStarted | `build_fallback_started_payload` | IntentionallyExcludedGenericFailure | `test_fallback_payload_builders_are_metadata_only` |
| FallbackCompleted | `build_fallback_completed_payload` | IntentionallyExcludedGenericFailure | `test_fallback_payload_builders_are_metadata_only` |
| FallbackFailed | `build_fallback_failed_payload` | IntentionallyExcludedGenericFailure | `test_fallback_payload_builders_are_metadata_only` |

Skill runtime note (2026-05-25): `run_skill_with_state` emits generic `RunFailed` for PromptStack/runtime and model-generation failures only after a Skill `AgentRun` exists. The payload uses `build_run_failed_payload` with a bounded readable error string. It does not contain raw prompt text, raw LifeModel, full sensitive context, or full model output. `RunFailed` remains `IntentionallyExcludedGenericFailure`, so this does not add a production source-audit rule.

### 9.6.4 Enforcement Checks

The validator (`validate_payload_builders_against_manifest`) performs these checks at test time:

1. Every `build_*_payload` in `trace_payloads.rs` source must be in the builder manifest.
2. Every builder declared in the manifest must exist in `trace_payloads.rs` source.
3. Every event in the builder manifest must exist in `event_contract_manifest()`.
4. `ProductionAudited` builder entries must map to `EventContractStatus::ProductionAudited`.
5. `IntentionallyExcludedGenericFailure` builder entries must map to `EventContractStatus::IntentionallyExcluded`.
6. `LegacyNoTypedBuilder` / `TypeOnlyNoBuilder` entries must not declare a builder name.
7. Every builder entry must declare at least one `required_contract_tests`.
8. Every `required_contract_tests` name must be a real test function found in scanned test files (`event_store.rs` or `trace_payloads.rs`).
9. `reason` must not be empty.
10. No duplicate `event` or duplicate `builder` in the manifest.

### 9.6.5 Source Scanner

Builder names are extracted from `openlife-core/src/agent/trace_payloads.rs` by scanning the **sanitised** source (comments and string literals masked). Only `pub fn build_*_payload(...)` declarations are captured — comment-only or string-only builder names are invisible to the scanner. The scanner strips the test region (`#[cfg(test)]`) from results.

A parser unit test (`payload_builder_contract_comment_and_string_fake_builders_ignored`) verifies that fake builders in comments and strings do not pollute the discovered list.

### 9.6.6 Test Coverage

All payload builder contract tests are prefixed `payload_builder_contract_` and live in `trace_contract_audit.rs`.

**Positive tests (call validator on real data):**

| Test | Verifies |
|------|----------|
| `payload_builder_contract_all_builders_are_manifested` | Full validator pass — all checks (1-10) pass against real source/manifest |
| `payload_builder_contract_manifest_builders_exist_in_source` | Every manifest builder found in `trace_payloads.rs` |
| `payload_builder_contract_events_exist_in_event_manifest` | Every builder manifest event in `event_contract_manifest` |
| `payload_builder_contract_production_status_matches_event_manifest` | `ProductionAudited` / `IntentionallyExcludedGenericFailure` match event manifest status |
| `payload_builder_contract_required_tests_exist` | All required contract test names found in scanned test files |
| `payload_builder_contract_scanner_finds_real_builders` | Scanner discovers all known builder names |
| `payload_builder_contract_comment_and_string_fake_builders_ignored` | Scanner ignores fakes in comments/strings; exact expected set matched |

**Negative tests (construct bad input, call validator, assert Err):**

| Test | What it proves |
|------|---------------|
| `payload_builder_contract_missing_builder_manifest_entry_fails` | Source builder without manifest entry → validator fails |
| `payload_builder_contract_stale_builder_entry_fails` | Manifest builder not in source → validator fails |
| `payload_builder_contract_wrong_event_status_fails` | Builder `ProductionAudited` but event manifest says `IntentionallyExcluded` → fails |
| `payload_builder_contract_missing_required_test_fails` | Required test name not found in scanned files → fails |
| `payload_builder_contract_duplicate_builder_fails` | Same builder name appears twice in manifest → fails |

### 9.6.7 How to Add a New Typed Payload Builder

1. Create the `build_*_payload` function in `trace_payloads.rs`.
2. Add a `PayloadBuilderContractEntry` to `payload_builder_contract_manifest()` with:
   - Correct `event` (short name matching `AgentRunEventType` variant).
   - Correct `status` (`ProductionAudited` if governance event with `AuditRule`; `IntentionallyExcludedGenericFailure` if generic failure).
   - Non-empty `reason`.
   - At least one `required_contract_tests` entry naming a real test function.
3. If the event is new, also update `event_contract_manifest()` and `audit_rules()`.
4. Add or update contract tests in `event_store.rs` or `trace_payloads.rs`.
5. Run verification:
   ```
   cargo test -p openlife-core payload_builder_contract -- --nocapture
   cargo test -p openlife-core event_contract -- --nocapture
   cargo test -p openlife-core trace_contract_audit -- --nocapture
   ```
6. Update this document (Section 9.6.3 table and Section 10 changelog).
7. Run `make ci`.

### 9.6.8 Verification Commands

```bash
cargo test -p openlife-core payload_builder_contract -- --nocapture
cargo test -p openlife-core event_contract -- --nocapture
cargo test -p openlife-core trace_contract_audit -- --nocapture
make ci
```

---

## 9.7 Backend ↔ Frontend Typed Event Contract Parity

### 9.7.1 Purpose

While the **AuditRule** (Section 9.3) enforces that backend production emit sites use typed builders, and the **event contract manifest** (Section 9.5) classifies every `AgentRunEventType` variant, and the **builder coverage manifest** (Section 9.6) tracks every typed payload builder — **none of these layers verify that the frontend parser (`typedContract.ts`), fixtures (`agentRunEvents.ts`), and tests (`typedContract.test.ts`) stay in sync with the backend typed payload contract**.

The **Backend ↔ Frontend Typed Event Contract Parity** audit closes this gap.

**Why cross-end parity is needed:**

- Backend `trace_payloads::build_*_payload` functions output **snake_case** JSON fields (e.g. `agent_spec_id`, `prompt_blocks`, `block_reason`).
- Frontend `typedContract.ts` `parseTypedEventPayload()` parses these exact snake_case field names into structurally validated `TypedEventPayload` discriminated unions.
- Frontend `fixtures/agentRunEvents.ts` must mirror the real backend payload shape so explainability tests are meaningful.
- Frontend `typedContract.test.ts` must exercise the real field names so a backend field rename or removal causes a test failure.

Without cross-end parity, a developer could:

1. Rename a field in a backend builder (`build_*_payload`) — backend contract tests pass, but frontend silently degrades to `kind: "unknown"`.
2. Add a new required field to a builder — backend tests pass, but frontend parser doesn't check it.
3. Remove a fixture field — explainability tests still pass because they don't exercise that field directly, but the fixture no longer represents real payloads.
4. Add a new typed builder event — backend manifest is updated, but frontend has no corresponding parser/fixture/test coverage.

### 9.7.2 Implementation

**File:** `openlife-core/src/agent/tests/frontend_contract_parity.rs`

**Module:** registered in `openlife-core/src/agent/tests/mod.rs`

**Backend builder source:** The backend builder name list is **not** hand-written in `frontend_contract_parity.rs`. Instead, `trace_contract_audit.rs` exposes `pub(super) fn typed_payload_builder_refs() -> Vec<(&str, &str)>` which is derived from the real `payload_builder_contract_manifest()` — the same manifest used by `payload_builder_contract` tests. This guarantees zero drift between the backend builder manifest and the frontend parity audit.

**Core types:**

- `FrontendTypedEventContractEntry` — each entry defines: `event`, `backend_builder`, `frontend_event_type`, `required_payload_fields`, `optional_payload_fields`, `frontend_parser_tokens`, `fixture_tokens`, `test_tokens`, and `is_generic_failure: bool`. The `is_generic_failure` flag controls validator behavior: governance events (`false`) check required fields against `typedContract.ts` AND fixtures/tests; generic failure events (`true`) check required fields against fixtures/tests only (parser does not structurally parse them).

**Core validator:**

```
validate_frontend_typed_contract_parity(
    parity_manifest: &[FrontendTypedEventContractEntry],
    builder_refs: &[(&str, &str)],    // from typed_payload_builder_refs()
    typed_contract_source: &str,
    typed_contract_test_source: &str,
    fixtures_source: &str,
) -> Result<(), Vec<String>>
```

**Checks performed:**

| # | Check | Governance Events | Generic Failure Events |
|---|-------|-------------------|----------------------|
| 1 | Every backend builder has a frontend parity entry | ✅ | ✅ |
| 2 | No frontend parity entry references a non-existent backend builder | ✅ | ✅ |
| 3 | No duplicate `event` in parity manifest | ✅ | ✅ |
| 4 | No duplicate `frontend_event_type` in parity manifest | ✅ | ✅ |
| 5 | `frontend_event_type` string exists in `typedContract.ts` | ✅ | ✅ |
| 6 | Each `required_payload_fields` exists in `typedContract.ts` | ✅ | ❌ (not checked — parser does not structurally parse these events) |
| 7 | Each `required_payload_fields` exists in `agentRunEvents.ts` OR `typedContract.test.ts` | ✅ | ✅ (primary required-field check for generic failures) |
| 8 | Each `frontend_parser_tokens` exists in `typedContract.ts` | ✅ | ✅ |
| 9 | Each `fixture_tokens` exists in `agentRunEvents.ts` | ✅ | ✅ |
| 10 | Each `test_tokens` exists in `typedContract.test.ts` | ✅ | ✅ |

### 9.7.3 Covered Events

| Event | Backend Builder | Frontend Event Type | GF? | Required Fields | Fixture/Test Coverage Tokens |
|-------|----------------|--------------------|-----|----------------|-----------------------------|
| AgentSpecSelected | `build_agent_spec_selected_payload` | `agent_spec.selected` | — | `agent_spec_id`, `privacy_policy` | fixture: `agent_spec.selected`; test: `AgentRunEvent` |
| PromptStackAssembled | `build_prompt_stack_assembled_payload` | `prompt_stack.assembled` | — | `agent_spec_id`, `prompt_blocks[]` with `id`, `version`, `purpose`, `privacy_level`, `cloud_allowed`, `token_budget`, `applies_to`, `estimated_tokens` | fixture: `prompt_blocks`; test: `prompt_stack.assembled` |
| ContextGovernanceApplied | `build_context_governance_applied_payload` | `context_governance.applied` | — | `agent_spec_id` | fixture: `privacy_policy`; test: `context_governance.applied` |
| ToolCallBlocked | `build_tool_call_blocked_payload` | `tool.call_blocked` | — | `status`, `tool_name`, `source`, `agent_spec_id` | fixture: `block_reason`; test: `BlockReason` |
| ReplayStarted | `build_replay_started_payload` | `replay.started` | — | `status`, `run_id`, `action_id`, `replay_of_action_id`, `agent_spec_id`, `tool_name`, `source` | test: `agent_spec_id`, `tool_name`, `source` (no fixture) |
| ReplayCompleted | `build_replay_completed_payload` | `replay.completed` | — | `status`, `run_id`, `action_id`, `replay_of_action_id`, `agent_spec_id`, `tool_name`, `source` | test: `agent_spec_id`, `tool_name`, `source` (no fixture) |
| ReplayFailed | `build_replay_failed_payload` | `replay.failed` | — | `status`, `run_id`, `action_id`, `replay_of_action_id`, `human_message` | fixture: `block_reason`; test: `replay.failed` |
| FallbackStarted | `build_fallback_started_payload` | `fallback.started` | ✅ Metadata | `status`, `fallback_mode`, `generation_path`, `agent_spec_id`, `privacy_policy`, `original_error_summary` | test: `test_fallback_payload_builders_are_metadata_only` |
| FallbackCompleted | `build_fallback_completed_payload` | `fallback.completed` | ✅ Metadata | `status`, `fallback_mode`, `generation_path`, `agent_spec_id`, `privacy_policy`, `original_error_summary`, `response_length` | test: `test_fallback_payload_builders_are_metadata_only` |
| FallbackFailed | `build_fallback_failed_payload` | `fallback.failed` | ✅ Metadata | `status`, `fallback_mode`, `generation_path`, `agent_spec_id`, `privacy_policy`, `original_error_summary`, `fallback_error_summary` | test: `test_fallback_payload_builders_are_metadata_only` |
| ModelFailed | `build_model_failed_payload` | `model.failed` | ✅ GF | `agent_spec_id`, `error` | test: `error` (no fixture) |
| RunFailed | `build_run_failed_payload` | `run.failed` | ✅ GF | `error` | test: `error` (no fixture) |
| ToolCallFailed | `build_tool_call_failed_payload` | `tool.call_failed` | ✅ GF | `tool`, `error` | test: `error` (no fixture) |
| ModelCallFailed | `build_model_call_failed_payload` | `model.call_failed` | ✅ GF | `provider`, `model`, `error` | test: `provider`, `model`, `error` (no fixture) |

> **GF = Generic Failure**. **Metadata = metadata-only lifecycle event**. Both pass through `parseTypedEventPayload` as `kind: "unknown"`; required backend fields are locked by Rust builder tests rather than frontend typed-governance parsing.

### 9.7.4 Test Coverage

All tests are prefixed `frontend_typed_contract_` and live in `frontend_contract_parity.rs`.

**Positive tests (call validator on real data):**

| Test | Verifies |
|------|----------|
| `frontend_typed_contract_all_backend_builders_have_frontend_entry` | Cross-manifest coverage: every real backend builder (from `typed_payload_builder_refs()`) has a parity entry, no extraneous builders |
| `frontend_typed_contract_frontend_tokens_exist` | Full validator pass — all checks against real source files |
| `frontend_typed_contract_required_fields_exist` | Every `required_payload_fields` found in correct location (typedContract.ts for governance, fixtures/tests for generic failures) |
| `frontend_typed_contract_fixture_tokens_exist` | Every `fixture_tokens` found in `agentRunEvents.ts` |
| `frontend_typed_contract_test_tokens_exist` | Every `test_tokens` found in `typedContract.test.ts` |

**Negative tests (construct bad input, call validator, assert Err):**

| Test | What it proves |
|------|---------------|
| `frontend_typed_contract_missing_frontend_entry_fails` | Backend builder without parity entry → validator fails |
| `frontend_typed_contract_unknown_builder_fails` | Parity entry referencing non-existent backend builder → validator fails |
| `frontend_typed_contract_duplicate_event_type_fails` | Two entries with same `frontend_event_type` → validator fails |
| `frontend_typed_contract_missing_parser_token_fails` | Parser token removed from `typedContract.ts` → validator fails |
| `frontend_typed_contract_missing_required_field_fails` | Required governance field removed from `typedContract.ts` → validator fails |
| `frontend_typed_contract_missing_fixture_token_fails` | Fixture token removed from `agentRunEvents.ts` → validator fails |
| `frontend_typed_contract_missing_test_token_fails` | Test token removed from `typedContract.test.ts` → validator fails |
| `frontend_typed_contract_backend_builder_source_drift_fails` | New `build_synthetic_new_payload` added to `typed_payload_builder_refs()` but not in parity manifest → validator fails with builder name in error |
| `frontend_typed_contract_generic_failure_required_field_missing_fails` | Generic failure required field (`provider`) removed from `typedContract.test.ts` → validator fails with field name in error |

### 9.7.5 Verification Commands

```bash
cargo test -p openlife-core frontend_typed_contract -- --nocapture
cargo test -p openlife-core payload_builder_contract -- --nocapture
cargo test -p openlife-core event_contract -- --nocapture
cargo test -p openlife-core trace_contract_audit -- --nocapture
cd frontend && corepack pnpm test -- typedContract.test.ts
make ci
```

### 9.7.6 How to Add/Modify a Typed Event (Cross-End)

When a new typed event is introduced or an existing typed event payload shape changes:

1. **Backend:** Create or update the `build_*_payload` function in `trace_payloads.rs`.
2. **Backend manifest:** Update `payload_builder_contract_manifest()` in `trace_contract_audit.rs` (this automatically propagates to `typed_payload_builder_refs()` which the frontend parity audit consumes — no separate hand-written list to update).
3. **Backend audit:** Add or update `AuditRule` entries in `audit_rules()`.
4. **Event manifest:** Update `event_contract_manifest()` in `trace_contract_audit.rs`.
5. **Frontend parity manifest:** Add or update `FrontendTypedEventContractEntry` in `frontend_parity_manifest()` in `frontend_contract_parity.rs`. If this is a governance event, set `is_generic_failure: false`; if it is a generic failure (pass-through as `kind: "unknown"`), set `is_generic_failure: true`. Add correct `required_payload_fields`, `frontend_parser_tokens`, `fixture_tokens`, and `test_tokens`.
6. **Frontend parser:** Update `parseTypedEventPayload()` in `typedContract.ts` to parse the new/changed fields (governance events only; generic failures pass through as `kind: "unknown"`).
7. **Frontend fixtures:** Add or update event timelines in `agentRunEvents.ts` with the new/changed fields.
8. **Frontend tests:** Add or update tests in `typedContract.test.ts`.
9. **Document:** Update this document (Section 9.7.3 table and Section 10 changelog).
10. **Verify:**
    ```bash
    cargo test -p openlife-core frontend_typed_contract -- --nocapture
    cd frontend && corepack pnpm test -- typedContract.test.ts
    make ci
    ```
11. All parity checks must pass — both Rust and frontend tests.

### 9.7.7 Boundaries

**Prohibited:**
- Modifying frontend parser behavior just to make tokens match — tokens are assertions about what already exists.
- Modifying Rust payload builder output fields just to match parity entries — parity entries describe the actual contract.
- Introducing TS parser, AST, or external dependencies for token scanning.
- Using snapshot-based string comparisons — all checks are field/token-level.
- Weakening existing `payload_builder_contract`, `event_contract`, or `trace_contract_audit` tests.
- Modifying any UI component.
- Writing a hard-coded list of backend builder names in `frontend_contract_parity.rs` — the backend builder source must come from `trace_contract_audit`'s `typed_payload_builder_refs()`.

**Allowed:**
- Adding missing fixture fields if they genuinely reflect backend builder output and were simply absent from fixtures.
- Adding missing test tokens if the test file was genuinely missing coverage for existing parser behavior.
- Exposing `pub(super)` helpers from `trace_contract_audit.rs` for reuse by sibling test modules.
- All changes must be small, justified, and documented in the commit message.

---

## 10. Update Log

| Date | Update |
|------|--------|
| 2026-05-26 | **Runtime Fallback Metadata Contract**: Added `FallbackFailed` event type and builder-driven payloads for `fallback.started`, `fallback.completed`, and `fallback.failed`. Chat / StreamChat fallback remains a governed legacy compatibility retry for Runtime/model failures only; Governance failures fail closed. Fallback payloads carry metadata (`status`, `fallback_mode`, `generation_path`, `prompt_stack_source`, `agent_spec_id`, `privacy_policy`, sanitized error summaries, response length) and exclude raw prompt, raw user text, raw LifeModel, raw memory, and full model output. Event contract manifest updated to 45 variants; fallback events moved to IntentionallyExcluded metadata builder coverage. |
| 2026-05-25 | **Skill Runtime Pre-Migration Safety Net**: Skill runtime remains outside ExecutionFacade. Added failed-run observability for PromptStack/runtime and model-generation failures using generic `run.failed` payloads from `build_run_failed_payload`; payloads are bounded readable errors only and exclude raw prompt, raw LifeModel, sensitive context, and full model output. Added tests for missing AgentSpec fail-closed/no fallback, AgentSpec restricted toolset gating, PromptStack/model failure failed-run status, success response shape, and source audit proving no Chat facade/fallback or Scheduled/Replay/Plan wrapper masquerade. |
| 2026-05-18 | **Frontend Contract Parity Rework** (返工): (1) Removed hand-written `backend_builder_manifest()` from `frontend_contract_parity.rs`. Backend builder names now come from `trace_contract_audit::typed_payload_builder_refs()` which is derived from the real `payload_builder_contract_manifest()`. Added `pub(super)` exposure on `typed_payload_builder_refs()` and `parse_payload_builders_from_source()`. (2) Added `is_generic_failure: bool` flag to `FrontendTypedEventContractEntry`. Validator now applies different rules: governance events check required fields against `typedContract.ts` AND fixtures/tests; generic failures check required fields against fixtures/tests only (parser does not structurally parse them). (3) Generic failure entries now have proper `required_payload_fields` (ModelFailed: `agent_spec_id, error`; RunFailed: `error`; ToolCallFailed: `tool, error`; ModelCallFailed: `provider, model, error`). (4) Added 2 new negative tests: `frontend_typed_contract_backend_builder_source_drift_fails` (new builder without parity entry → fails) and `frontend_typed_contract_generic_failure_required_field_missing_fails` (generic failure field removed from test → fails). Total tests: 14 (5 positive + 9 negative). |
| 2026-05-18 | **Backend ↔ Frontend Typed Event Contract Parity**: Added `openlife-core/src/agent/tests/frontend_contract_parity.rs` — cross-end contract parity audit with 11-event `FrontendTypedEventContractEntry` manifest. Added `validate_frontend_typed_contract_parity()` validator — 10 consistency checks across backend builder manifest, frontend `typedContract.ts` parser, `agentRunEvents.ts` fixtures, and `typedContract.test.ts` tests. Added 12 tests (5 positive + 7 negative). Negative tests prove that removing a parser token, required field, fixture token, or test token — or adding an unknown builder or duplicate event type — causes validator failure. Registered module in `tests/mod.rs`. Updated document with Section 9.7. |
| 2026-05-18 | **Typed Payload Builder Contract Coverage**: Added `payload_builder_contract_manifest()` — maps all 11 typed payload builders to events, statuses (ProductionAudited x7, IntentionallyExcludedGenericFailure x4), and required contract tests. Added `parse_payload_builders_from_source()` — scans `trace_payloads.rs` for `pub fn build_*_payload` declarations (comments/strings masked via `sanitize_source`). Added `collect_known_test_function_names()` — scans `event_store.rs` and `trace_payloads.rs` for test function names. Added `validate_payload_builders_against_manifest()` — 10 consistency checks (source↔manifest↔event manifest↔tests). Added 12 tests (7 positive + 5 negative). Scanner unit test verifies comment/string fakes are ignored. All tests pass; `make ci` passes. |
| 2026-05-18 | **Event Contract Coverage Manifest**: Added `event_contract_manifest()` — a classification of all 44 `AgentRunEventType` variants into 4 tiers (ProductionAudited x7, IntentionallyExcluded x4, LegacyInternalOnly x32, TypeOnlyNoDirectEmission x1). Added `parse_agent_run_event_type_variants()` — extracts enum variants from `types/mod.rs` source. Added 11 coverage enforcement tests. Added validator functions (`validate_manifest_against_enum`, `validate_manifest_against_audit_rules`, `validate_intentionally_excluded_reasons`, `validate_document_against_manifest`). Document now verified against manifest at test time — mismatch fails CI. `make ci` passes. |
| 2026-05-18 | **Trace Contract Audit Hardening**: Source sanitisation — Rust comments (`//`/`/* */`) and string literals (`"…"`/`r#*"…"#*`) are now masked before scanning, preventing tokens in comments/strings from creating false passes or false-positive emission counts. Added `expected_emissions: Option<usize>` to `AuditRule` — all audit rules now use exact emission counts (previously only `min_emissions`). Default window narrowed from ±1500 to ±900 chars (exceptions documented with measured-distance justification). Added 10 new negative tests covering line comments, block comments, regular strings, raw strings, double-hash raw strings, escaped-quote strings, `#[cfg(test)]` in comments, and adjacent-event non-masking. `make ci` passes. No production code changes. |
| 2026-05-18 | **ReplayFailed Builder Separation**: Fixed `src-tauri/src/commands/agent.rs` replay outcome path — `ReplayFailed` now exclusively uses `build_replay_failed_payload` (was incorrectly unified with `build_replay_completed_payload`). Failed path: priority `block_reason` → `failure_kind` → fallback `internal_error`, with `tool_name`/`source`/`agent_spec_id` via `extra`. Succeeded/Blocked/NeedsConfirmation continue using `build_replay_completed_payload`. Audit rule tightened to only allow `build_replay_failed_payload` for `ReplayFailed`. Added `negative_replay_failed_using_completed_builder_is_violation` test. Replaced permissive `positive_replay_failed_with_either_builder_passes` with `positive_replay_failed_with_correct_builder_passes`. Restored Sections 11.3-11.10 (Explainability) that were truncated in previous edit. |
| 2026-05-18 | **Trace Contract Drift Audit**: Added `openlife-core/src/agent/tests/trace_contract_audit.rs` — a CI-enforceable source-code scanning test. Covers 7 event types across 7 production files. See Section 9. |
| 2026-05-18 | **Builder-Driven Test Cleanup**: Converted `test_tool_call_blocked_typed_payload_contract` (3 hand-written JSON cases → 4 builder-driven cases including `agent_spec_id: null` budget-exceeded) and `test_replay_failed_events_have_typed_reason` (4 hand-written JSON cases → 4 builder-driven cases) to use `trace_payloads::build_*` functions exclusively. Added `assert_has_optional_string_or_null` helper in `contract_helpers.rs` — `agent_spec_id` contract now requires field existence but allows `string | null`. Added 4 new helper tests (`optional_string_or_null_accepts_string`, `_accepts_null`, `_rejects_empty_string`, `_rejects_missing_field`). Assertions in both contract tests now use `contract_helpers::*` helpers exclusively (no more ad hoc `v.as_str().is_some_and(...)` logic). **All typed governance contract tests (`tool.call_blocked`, `replay.started`, `replay.completed`, `replay.failed`, `agent_spec.selected`, `prompt_stack.assembled`, `context_governance.applied`, generic failures) are now fully builder-driven — zero hand-written typed payloads remain in Rust contract tests.** |
| 2026-05-17 | **Production Payload Builder + Enum Validation**: Created `openlife-core/src/agent/trace_payloads.rs` with production payload builder functions. Refactored 7 emit sites across `streaming.rs`, `execution.rs`, `orchestrator.rs`, and `agent.rs` to delegate to the shared builders. Contract tests now call the same builders — hand-written JSON is no longer the sole contract proof. `assert_has_typed_reason` now validates against production enum variant strings (`allowed_block_reasons`, `allowed_proposal_reasons`, `allowed_failure_kinds`). Invalid enum values like `"not_a_real_enum_variant"` are rejected, matching frontend `typedContract.ts`. Added `assert_no_typed_reason` helper for malformed payload verification. Added 4 new contract tests (invalid enum rejection x2, valid enum regression x2). |
| 2026-05-16 | **Post-Beta Audit Fixes**: ReplayFailed early paths 1-3 now carry typed reasons; tools.rs budget exceeded now compliant; plan_executor.rs `agentspec_id` → `agent_spec_id` fix; shell.run closure auto-injects contract fields. All production ToolCallBlocked/ReplayFailed emitters now satisfy typed payload contract. |
| 2026-05-16 | **Explainability Layer**: Added `getTypedEventExplanation`, `getTypedRunExplanation`, `TypedRunExplanationViewModel`, `TypedEventExplanationViewModel` to `typedContract.ts`. Added `EventExplanationBlock` and `RunExplanationPanel` components. Integrated into `RunTracePanel`, `AgentRunDetail`, and `RunsPage`. See Section 11 (Explainability Layer). |
| 2026-05-16 | **Explainability Snake-Case Fix**: Fixed `getTypedRunExplanation` metadata extraction to read real backend snake_case fields (`agent_spec_id`, `privacy_policy`, `agent_spec_privacy_policy`, `prompt_blocks`). Removed fake `promptStackId` field — replaced with `promptBlockCount` / `promptBlockIds` extracted from `prompt_blocks` array (Scheme B). Added snake_case priority with camelCase backward-compat fallback. Updated `TypedRunExplanationViewModel` interface. Updated `RunExplanationPanel` developer display. Added 9 new tests covering real payload contracts. |
| 2026-05-16 | **Post-Beta Explainability Quality Hardening**: Added `frontend/src/test/fixtures/agentRunEvents.ts` with 5 real-world snake_case event timeline fixtures (successfulGovernedRun, agentSpecDeniedToolRun, needsConfirmationRun, replayFailedRun, malformedAndUnknownRun). Removed `kind: "none"` nextAction — success/info runs now have empty `nextActions` array. `RunExplanationPanel` hides "建议操作" section when `nextActions` is empty. `nextActions` severity narrowed to `"warning" | "error"` (no more `"info"` noise). Added 13 fixture-based end-to-end tests across typedContract, RunTracePanel, AgentRunDetail, and RunsPage. Updated sections 11.2, 11.6, 11.7, 11.8. |
| 2026-05-16 | **nextActions Fallback & Malformed Known Typed Event Fix**: Added `nextActions` fallback rule: when `outcomeTone` is error/warning and no typed-reason-driven nextActions exist, `inspect_trace` is auto-appended. Generic failures (`tool.call_failed`, `run.failed`, `model.failed`, `model.call_failed`) now all set `hasGenericFailure` and surface `primaryReason: "运行中出现未分类错误"`. Known typed event types with malformed payloads now produce `outcomeTone: "warning"`, `primaryReason: "运行 trace 中存在无法解析的治理事件"`, malformed count in developerBullets, and `inspect_trace` nextAction — no longer treated as clean success. Unknown event types (`custom.unknown_event`) do NOT trigger malformed warning. Added `KNOWED_TYPED_EVENT_TYPES` set and `malformedKnownTyped` counter. Added 5 new unit tests (generic failure x3, malformed semantics, regression guard). Updated sections 11.2, 11.7, 11.8, 11.9, 11.10. |

## 11. Explainability Layer

### 11.1 Boundary: Typed Contract vs. Explainability

| Layer | Responsible For | File |
|-------|----------------|------|
| **Typed Contract** | Parse payloads, validate typed fields, produce typed view models | `frontend/src/utils/typedContract.ts` |
| **Explainability** | Produce user/developer-facing explanations from typed view models | `frontend/src/utils/typedContract.ts` (same file) |
| **UI Rendering** | Display explanation views, badges, lists | `RunTracePanel`, `RunExplanationPanel`, `EventExplanationBlock`, `AgentRunDetail`, `RunsPage` |

**Hard rule:** `summary` / `human_message` / `error` text is never used for state inference in explanation helpers. Only typed payload fields (`block_reason`, `proposal_reason`, `failure_kind`, `agent_spec_id`, `proposal_id`, `status`) determine what the explanation says.

### 11.2 Run-Level Explanation (`getTypedRunExplanation`)

**Input:** `AgentRunEvent[]` timeline + optional `{status, kind}` from AgentRun.

**Output:** `TypedRunExplanationViewModel` with:
- `headline`, `outcomeTone` — run-level summary
- `primaryReason` — the most important typed reason (from block_reasons, proposal_reasons, failure_kinds)
- `agentSpecId` — extracted from `agent_spec_id` field in `agent_spec.selected` or `prompt_stack.assembled` or typed events (snake_case, real backend)
- `promptBlockCount` / `promptBlockIds` — extracted from `prompt_blocks` array in `prompt_stack.assembled` (snake_case, real backend). No `prompt_stack_id` field exists in backend payloads.
- `contextPolicy` — extracted from `privacy_policy` (execution/streaming path) or `agent_spec_privacy_policy` (orchestrator path) in `context_governance.applied`, or from `privacy_policy` in `agent_spec.selected` (snake_case, real backend)
- `toolSummary`, `replaySummary` — counters from typed events
- `nextActions` — actionable items derived from typed reasons (not from summary text)
- `userFacingBullets`, `developerBullets` — plain-language items

**Metadata field precedence:** All metadata fields use **snake_case priority** matching real backend payloads. camelCase is a backward-compat fallback for legacy test data only.

| Event Type | Real Backend Fields (snake_case) | camelCase Fallback |
|-----------|--------------------------------|-------------------|
| `agent_spec.selected` | `agent_spec_id`, `role`, `privacy_policy` | `agentSpecId` |
| `prompt_stack.assembled` | `agent_spec_id`, typed metadata-only `prompt_blocks[]` (no `prompt_stack_id`, no raw prompt content) | — |
| `context_governance.applied` | `privacy_policy` (exec/streaming), or `agent_spec_privacy_policy` (orchestrator) | `privacyPolicy` |

**PromptStack scheme:** Scheme B — frontend extracts block count and IDs from `prompt_blocks` array. No `prompt_stack_id` field exists in the backend; adding one would create a fake contract. The block trace captures real metadata (`id`, `version`, `purpose`, `privacy_level`, `cloud_allowed`, `token_budget`, `applies_to`, `estimated_tokens`) without leaking prompt content, raw LifeModel, raw memory, or raw user content.

**nextActions derivation:**

| Condition | nextAction.kind | Severity |
|-----------|----------------|----------|
| Any `tool.call_blocked` with `needs_confirmation` | `review_proposal`, `grant_permission` | warning |
| Any `tool.call_blocked` with `agent_spec_denied` | `adjust_agent_spec` | error |
| Any `replay.failed` with typed reason | `retry_replay`, `inspect_trace` | error |
| Generic failure (tool.call_failed, run.failed, model.failed, model.call_failed) without typed reasons | `inspect_trace` | error |
| Malformed known typed events without other errors | `inspect_trace` | warning |
| All clear (success / info / no governance issues) | **(empty array)** — no user action needed | — |

**nextActions semantics:**
- **Empty array** (`nextActions.length === 0`): the run completed without governance issues. No "建议操作" section is rendered in `RunExplanationPanel`.
- **Non-empty array**: only appears for error / warning outcomes. Every entry has a real action (`review_proposal`, `grant_permission`, `adjust_agent_spec`, `retry_replay`, `inspect_trace`) with `severity: "warning" | "error"`.
- **Fallback rule**: when `outcomeTone` is `"error"` or `"warning"` and no typed-reason-driven nextActions exist, `inspect_trace` is automatically appended so users always have a diagnostic next step. Generic failures (`tool.call_failed`, `run.failed`, `model.failed`) and malformed known typed events all trigger this fallback.
- The `kind: "none"` filler no longer exists. Success / info runs do not display misleading "查看运行 trace" or "查看详细 trace 进行审计" suggestions.
- `nextActions` is derived exclusively from typed payload fields (`block_reason`, `proposal_reason`, `failure_kind`) or the presence of error/warning outcome. It never uses `summary`, `human_message`, or `error` text as a state source.

**Fallback:** Empty events → `outcomeTone: "info"`, `headline: "运行记录"`, `userFacingBullets: ["无显著事件"]`.

### 11.3 Event-Level Explanation (`getTypedEventExplanation`)

**Input:** Single `AgentRunEvent`.

**Output:** `TypedEventExplanationViewModel` with:
- `title`, `tone` — event summary
- `whatHappened` — user-facing description
- `why` — typed reason label (null if none)
- `impact` — what this means for the user
- `nextStep` — actionable suggestion
- `debugFacts` — label/value pairs (eventType, toolName, source, agentSpecId, proposalId, actionId, replayOfActionId)

**Supported event types (typed explanation):**

| Event Type | Status Variants | Explanation Quality |
|-----------|----------------|-------------------|
| `tool.call_blocked` | blocked, needs_confirmation | Full (user-facing whatHappened/why/impact/nextStep) |
| `replay.started` | started | Full |
| `replay.completed` | completed, blocked, needs_confirmation | Full per-status |
| `replay.failed` | failed (with block_reason or failure_kind) | Full |

**Unknown/malformed fallback:**
- `whatHappened: "这是一个未识别的运行事件"`
- `tone: "info"`
- `why/impact/nextStep: null`
- `debugFacts` contains `eventType` and `summary`

### 11.4 UI Integration

| UI Component | What Shows |
|-------------|-----------|
| `RunTracePanel` | `EventExplanationBlock` first in expanded event, then TypedEventDetailBlock, then raw payload |
| `AgentRunDetail` | `RunExplanationPanel` above the trace timeline |
| `RunsPage` | `primaryReason` from run-level explanation displayed as hint badge |

### 11.5 Events With Only Generic Debug Display

These event types do not have user-facing explanation yet, and show only in the generic event row + raw payload:
- `run.created`, `run.completed`, `run.failed`
- `model.call_started`, `model.call_completed`, `model.call_failed`, `model.failed`
- `tool.call_started`, `tool.call_completed`, `tool.call_failed`
- `context.assembled`, `agent_spec.selected`, `prompt_stack.assembled`, `context_governance.applied`
- `proposal.created`, `fallback.*`, `json_repair.*`, `compaction.created`
- `plan.*` events
- `shell.blocked`, `shell.completed`

These are non-governance events that don't carry typed payloads with block/proposal/failure reasons. They display in `RunTracePanel` as generic event rows with raw payload.

### 11.6 Test Coverage

| Test File | What's Tested |
|-----------|--------------|
| `typedContract.test.ts` | `getTypedEventExplanation`: tool_call_blocked (agent_spec_denied, network_policy_ask), replay.completed (completed, blocked), replay.failed (block_reason, failure_kind), unknown fallback, malformed fallback. `getTypedRunExplanation`: all success, needs confirmation, AgentSpec denied, replay failed, mixed, summary misleading but typed correct, summary has reason but typed absent (now produces malformed warning), agentSpecId/promptBlockCount/promptBlockIds/contextPolicy extraction (snake_case real backend + camelCase compat fallback), contextPolicy fallback from agent_spec.selected, empty events. **Fixture-based**: successfulGovernedRun (success, empty nextActions), agentSpecDeniedToolRun (error, adjust_agent_spec), needsConfirmationRun (warning, review/grant), replayFailedRun (error, retry/inspect), malformedAndUnknownRun (warning, inspect_trace, malformed count). **Generic failure**: tool.call_failed, run.failed, model.failed, model.call_failed all produce error tone + inspect_trace. |
| `RunTracePanel.test.tsx` | Expanded typed event shows user-facing explanation before raw payload. Unknown/malformed event shows fallback. Event explanation does not infer from summary text. Raw payload remains in debug section. **Fixture-based**: agentSpecDeniedToolRun (typed detail visible, user-facing explanation), malformedAndUnknownRun (no crash, fallback). |
| `AgentRunDetail.test.tsx` | Run-level explanation panel above trace. AgentSpec denied → adjust_agent_spec. Needs confirmation → review/grant. Replay failed → retry/inspect. **Fixture-based**: successfulGovernedRun (no 建议操作, no misleading hint, developer info present), agentSpecDeniedToolRun, needsConfirmationRun, replayFailedRun. |
| `RunsPage.test.tsx` | Explanation hint from typed payload. Misleading summary doesn't create false hint. Pure typed events still produce preview hint. **Fixture-based**: successfulGovernedRun (no primaryReason, no misleading hint), agentSpecDeniedToolRun (primaryReason visible), replayFailedRun (primaryReason visible), malformedAndUnknownRun (no crash, no misleading hint). |

### 11.7 Real Event Fixtures

**File:** `frontend/src/test/fixtures/agentRunEvents.ts`

Five real-world event timeline fixtures for end-to-end explainability validation. All fixtures use **snake_case payloads** matching the real backend contract. No fixture uses `summary`, `human_message`, or `error` text as a state source.

| Fixture | Event Types Included | Purpose |
|---------|---------------------|---------|
| `successfulGovernedRun` | `agent_spec.selected`, `prompt_stack.assembled`, `context_governance.applied`, `tool.call_started`, `tool.call_completed`, `run.completed` | Validates success path: agentSpecId, prompt blocks, privacy policy extraction; empty nextActions. |
| `agentSpecDeniedToolRun` | `agent_spec.selected`, `prompt_stack.assembled`, `context_governance.applied`, `tool.call_blocked` (status: "blocked", block_reason: "agent_spec_denied"), `run.completed` | Validates AgentSpec deny: error tone, adjust_agent_spec nextAction, main.strict spec ID. |
| `needsConfirmationRun` | `agent_spec.selected`, `prompt_stack.assembled`, `context_governance.applied`, `tool.call_blocked` (status: "needs_confirmation", proposal_reason: "network_policy_ask", proposal_id: "prop-network-ask-001"), `run.completed` | Validates needs confirmation: warning tone, review/grant nextActions, proposal_id. |
| `replayFailedRun` | `agent_spec.selected`, `prompt_stack.assembled`, `replay.failed` (block_reason: "replay_spec_missing"), `run.completed` | Validates replay failure: error tone, retry_replay + inspect_trace nextActions, replaySummary.failed. |
| `malformedAndUnknownRun` | `tool.call_blocked` (invalid block_reason enum), `replay.failed` (null reasons), `custom.unknown_event`, `run.completed` | Validates soft-fail: no crash, warning tone (NOT success), malformed count in developerBullets, inspect_trace nextAction, no inference from summary text. |

**Import path:** `@/test/fixtures/agentRunEvents`

**Usage in tests:**
```typescript
import { successfulGovernedRun } from "@/test/fixtures/agentRunEvents";
const exp = getTypedRunExplanation(successfulGovernedRun, { status: "completed", kind: "conversation" });
```

### 11.8 nextActions Semantic Contract

| Run Outcome | `nextActions.length` | UI Behavior |
|------------|---------------------|-------------|
| success / info (no governance issues) | 0 (empty array) | `RunExplanationPanel` shows NO "建议操作" section |
| warning (needs_confirmation) | 1-2 | Shows "建议操作" with `review_proposal` + `grant_permission` |
| warning (malformed known typed events) | 1 | Shows "建议操作" with `inspect_trace` |
| error (agent_spec_denied, replay_failed) | 1-3 | Shows "建议操作" with `adjust_agent_spec` and/or `retry_replay` + `inspect_trace` |
| error (generic failure: tool.call_failed, run.failed, model.failed, model.call_failed) | 1 | Shows "建议操作" with `inspect_trace` |

**Anti-patterns prevented:**
- No `kind: "none"` filler action that creates noise under success runs.
- No duplicate "查看运行 trace" + "查看详细 trace 进行审计" on success paths.
- error/warning always has at least one nextAction (fallback `inspect_trace`).
- Developer info remains collapsible and out of the way.
- Success runs display only headline + summary bullets + collapsible developer section.

### 11.9 Malformed Known Typed Events

When a known governance event type (`tool.call_blocked`, `replay.started`, `replay.completed`, `replay.failed`) has a payload that fails structural validation:

- **outcomeTone**: `"warning"` (raised from `"info"`/`"success"` if no higher-priority error exists)
- **primaryReason**: `"运行 trace 中存在无法解析的治理事件"`
- **nextActions**: `inspect_trace` with `severity: "warning"` (via fallback rule)
- **developerBullets**: includes `"无法解析的治理事件: N"` count
- **Hard rule**: never infers specific `block_reason` / `proposal_reason` / `failure_kind` from `summary` or `human_message` text
- **Unknown event types** (`custom.unknown_event` etc.) are NOT counted as malformed — they do not affect outcomeTone

### 11.10 Generic Failure Fallback

When `tool.call_failed`, `run.failed`, `model.failed`, or `model.call_failed` events exist but no typed governance reasons are present:

- **outcomeTone**: `"error"`
- **primaryReason**: `"运行中出现未分类错误"`
- **nextActions**: `inspect_trace` with `severity: "error"` (via fallback rule)
- **developerBullets**: includes `"存在通用失败事件"`
