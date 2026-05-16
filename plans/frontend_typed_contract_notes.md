# Frontend Typed Contract Notes

Date: 2026-05-16

Status: active

> This document defines the frontend typed contract rules established during the "Typed Contract Hardening" phase. All future UI changes to Agent trace, replay, proposal, and tool execution display must follow these rules.

---

## 1. Core Principle

**Typed payload is the single source of truth for UI business semantics.**

- `summary`, `error`, `message`, `reason` (text) fields may appear in UI as **auxiliary debug information** but must **never** drive state judgment, label selection, color assignment, or operation suggestions.
- All business-state decisions (blocked/needs_confirmation/failed/success) must come from structured typed fields: `block_reason`, `proposal_reason`, `failure_kind`, `status`, etc.
- Unknown, missing, or malformed typed payloads must degrade gracefully to `unknown`, never crash the page.

---

## 2. Typed Contract Layer

Location: `frontend/src/utils/typedContract.ts`

### 2.1 Core APIs

| API | Purpose | Returns |
|-----|---------|---------|
| `parseTypedEventPayload(event)` | Extract typed payload from AgentRunEvent | `TypedEventPayload` (discriminated union) |
| `getTypedRunEventViewModel(event)` | Full view model for any AgentRunEvent | `TypedEventViewModel` |
| `getTypedActionViewModel(action)` | View model for AgentAction | `TypedActionViewModel` |
| `getTypedToolCallViewModel(call)` | View model for ToolCallResult | `TypedToolCallViewModel` |
| `getTypedProposalHint(proposal)` | Governance hints from AgentProposal | `TypedProposalHint` |
| `getTypedRunHints(events)` | Short hints for RunsPage list preview | `TypedRunHint[]` |
| `extractTypedActionOutcome(action)` | Typed outcome from replayed AgentAction | outcome object |
| `typedStatusSeverity(status)` | Severity from status string | `TypedSeverity` |

### 2.2 Label Maps (single source of truth — internal only)

- `BLOCK_REASON_LABELS` — 14 block reasons → Chinese labels
- `PROPOSAL_REASON_LABELS` — 3 proposal reasons → Chinese labels
- `FAILURE_KIND_LABELS` — 5 failure kinds → Chinese labels

These label maps are internal to `typedContract.ts`. **UI components must never import them directly.**

Instead, UI components consume typed display helpers:

| Helper | Returns | Purpose |
|--------|---------|---------|
| `getBlockReasonDisplay(reason)` | `TypedBadge \| null` | Label + severity for a block reason |
| `getProposalReasonDisplay(reason)` | `TypedBadge \| null` | Label + severity for a proposal reason |
| `getFailureKindDisplay(kind)` | `TypedBadge \| null` | Label + severity for a failure kind |
| `getTypedReasonBadgesFromEvent(event)` | `TypedBadge[]` | All typed badges from an AgentRunEvent |
| `getTypedReasonBadgesFromAction(action)` | `TypedBadge[]` | All typed badges from an AgentAction |
| `getTypedReasonBadgesFromToolCall(call)` | `TypedBadge[]` | All typed badges from a ToolCallResult |
| `getTypedOutcomeLabels(outcome)` | outcome labels | Labels from extractTypedActionOutcome |

The `TypedBadge` view model:

```ts
interface TypedBadge {
  kind: "block_reason" | "proposal_reason" | "failure_kind";
  label: string;        // Chinese label from label map
  severity: "error" | "warning" | "info";
  rawReason: string;    // enum key for per-reason visual styling
}
```

### 2.3 Severity Rules

| Status | Severity |
|--------|----------|
| `blocked`, `failed`, `deny` | `error` |
| `needs_confirmation`, `ask_every_time`, `pending` | `warning` |
| `completed`, `succeeded`, `success`, `allow*` | `success` |
| everything else | `info` |

---

## 3. Component Convergence

All target components now use typedContract APIs exclusively:

| Component | API Used | Previous Anti-Pattern |
|-----------|----------|----------------------|
| `ProposalReviewPage` | `getTypedProposalHint`, `extractTypedActionOutcome`, `getTypedOutcomeLabels` | Direct `BLOCK_REASON_LABELS[x] ?? x`, `PROPOSAL_REASON_LABELS[x] ?? x` |
| `AgentRunDetail` | `getTypedActionViewModel` | Direct label map lookups; separate Replay Trace rendering (removed) |
| `RunsPage` | `getTypedRunHints` | Action status counts only; now typed hints from event payloads take priority |
| `RunTracePanel` | `getTypedEventDetailViewModel` | Direct `parseTypedEventPayload` + typed payload imports + per-kind detail components |
| `ToolObservationPanel` | `getTypedActionViewModel` | Generic "blocked by policy" messages |
| `ToolCallCard` | `getTypedToolCallViewModel` | Inline `extractTypedBlockInfo` helper |

### UI Consumption Rules

1. **UI MUST NOT import label maps directly.** `BLOCK_REASON_LABELS`, `PROPOSAL_REASON_LABELS`, `FAILURE_KIND_LABELS` are internal to `typedContract.ts`.
2. **UI MUST use typedContract view model / badge helpers** to get labels and severity.
3. **UI MUST NOT judge reason validity.** Validity is determined by `getBlockReasonDisplay` / `getProposalReasonDisplay` / `getFailureKindDisplay` returning `null`.
4. **UI MUST NOT fallback to raw reason strings.** If a helper returns `null`, the reason is invalid and should not be displayed.
5. **New reasons only require changes in `typedContract.ts` and its tests.** UI components pick up new labels automatically via the badge helpers.
6. **RunTracePanel MUST NOT call `parseTypedEventPayload` directly.** It must use `getTypedEventDetailViewModel(event)` to render typed event details.
7. **RunTracePanel MUST NOT import typed payload types** (`ToolCallBlockedPayload`, `ReplayStartedPayload`, etc.). All event detail fields come from `TypedEventDetailViewModel`.

### AgentRunDetail / RunTracePanel Single Trace Principle

- **AgentRunDetail must NOT duplicate Replay Trace rendering.** The separate "Replay Events Trace" section that manually rendered replay.started/completed/failed events with inline badge lookups has been removed.
- **RunTracePanel is the single event timeline / replay trace display entry point.** All events (tool calls, replays, model calls, etc.) are displayed through RunTracePanel's expandable event rows with typed contract detail components.
- **RunTracePanel consumes `getTypedEventDetailViewModel(event)` exclusively** — it does NOT call `parseTypedEventPayload`, does NOT import typed payload types, and does NOT switch on `typed.kind` to pick detail components.
- **`getTypedEventDetailViewModel(event)`** is the single function that assembles all display fields (title, status label, badges, meta fields) from a typed event. It returns `TypedEventDetailViewModel`:
  - `kind`: discriminated union of known event detail kinds
  - `title`, `titleIconTone`: heading text and icon color
  - `statusLabel`, `statusTone`: status display text and color
  - `toolName`, `source`, `agentSpecId`, `proposalId`: common meta fields
  - `actionId`, `replayOfActionId`: replay-specific identifiers
  - `targetToolName`, `targetSource`, `wrapperToolName`: MCP wrapper info
  - `humanMessage`: auxiliary text (never used for reason inference)
  - `badges: TypedBadge[]`: all typed reason badges
- Summary view models for replay outcomes are provided through `getTypedReasonBadgesFromEvent()` and `getTypedOutcomeLabels()`, not through duplicated parser/label/badge logic.

### Typed Event Detail Extension Points

When adding a new typed event detail kind:

1. Add the new kind to `TypedEventDetailViewModel["kind"]` in `typedContract.ts`.
2. Add a new branch in `getTypedEventDetailViewModel` to assemble the view model.
3. In `RunTracePanel.tsx`, extend `detailIcon()` and any kind-specific rendering conditionals.
4. Do NOT add a new detail component in RunTracePanel — reuse `TypedEventDetailBlock`.
5. Add tests in `typedContract.test.ts` for `getTypedEventDetailViewModel`.

---

## 4. Extension Points

When adding new event types, new block reasons, new proposal reasons, or new failure kinds:

### 4.1 New Event Type

1. Add the event type string to `AgentRunEventType` in `types.ts`.
2. If the event carries typed payload:
   - Add the payload struct to `types.ts` (e.g., `MyNewPayload`).
   - Add a new variant to `TypedEventPayload` discriminated union in `types.ts`.
   - Add a new `if` branch in `parseTypedEventPayload` in `typedContract.ts`.
   - Add a new case in `getTypedRunEventViewModel` in `typedContract.ts`.
   - If appropriate, add a new branch in `getTypedRunHints`.
3. If the event has a new event type string label, add it to `getEventLabel` in `typedContract.ts`.

### 4.2 New Block / Proposal / Failure Kind

1. Add the string literal to the type definition in `types.ts` (e.g., extend `ExecutionBlockReason`).
2. Add the Chinese label to the corresponding label map in `typedContract.ts`.
3. Optionally add severity classification in `blockReasonSeverity` in `typedContract.ts`.

### 4.3 New Component Using Typed Contract

1. Import only the needed view model functions from `typedContract.ts`.
2. Call the view model function once per entity.
3. Use `vm.blockReasonLabel`, `vm.proposalReasonLabel`, etc. — never go back to `payload["block_reason"]`.
4. Never call `includes(...)`, `match(...)`, or `search(...)` on `error`/`summary`/`reason` text fields for business decisions.
5. Add tests with noise text and conflict text to prove the component does not string-infer.

---

## 5. Test Requirements

For any new typed contract integration:

1. **Happy path**: typed field present → correct label shown.
2. **Noise text**: typed field present + misleading error text → typed field wins.
3. **Missing typed**: no typed field → no typed inference, show generic if needed.
4. **Conflict text**: typed field says X, error text says Y → typed field X shown.
5. **Malformed**: typed field has invalid value (number, boolean) → safe fallback to null/unknown.
6. **Unknown event type**: `TypedEventPayload["kind"] === "unknown"` → no crash.

---

## 6. Degradation Strategy

| Scenario | Behavior |
|----------|----------|
| Typed field present and valid | Show typed label |
| Typed field absent | Show generic status only (no text-inferred reason) |
| Typed field invalid type (e.g., number instead of string) | `null` — treated as missing |
| Payload is empty or missing | `unknown` kind |
| Event type not recognized by parser | `unknown` kind, raw payload preserved for debug |

---

## 7. Residual Risks

1. **Old events may lack typed payloads**: Events created before this upgrade may have `summary` text but no typed fields. The UI will show generic status without typed reasons. This is by design — not a regression.
2. **Backend must produce typed payloads**: This phase only hardens the frontend contract. Future backend work should ensure all new events carry typed payloads (`block_reason`, `proposal_reason`, `failure_kind`).
3. **ToolObservationPanel fallback**: When no typed reason exists, the component falls back to generic messages like "该工具调用被权限策略或沙盒规则阻断". This is acceptable for legacy events but should be replaced with backend typed payload generation.

---

*This document should be updated whenever new event types, block reasons, proposal reasons, or failure kinds are added.*

*Related: `plans/openlife_post_beta_roadmap.md`, `plans/current_agent_runtime_audit.md`, `plans/openlife_vnext_core_primitives_and_boundaries.md`*
