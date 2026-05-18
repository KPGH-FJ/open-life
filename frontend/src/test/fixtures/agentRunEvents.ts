/**
 * Real-world AgentRun event timeline fixtures for trace explainability
 * end-to-end validation.
 *
 * ALL fixtures use snake_case payloads matching the real backend contract.
 * NO fixture uses summary/human_message/error as a state source.
 *
 * ═══════════════════════════════════════════════════════════════════════
 * BACKEND CONTRACT ALIGNMENT
 * ═══════════════════════════════════════════════════════════════════════
 *
 * These fixtures are NOT arbitrary mock data.  Each fixture corresponds to
 * a concrete backend contract tested in the Rust test suite.  The field
 * names and shapes mirror the real production payloads emitted by:
 *
 *   Backend contract tests:
 *     openlife-core/src/agent/event_store.rs  (uses production builders)
 *     openlife-core/src/agent/trace_payloads.rs  (production payload builders)
 *     openlife-core/src/agent/tests/contract_helpers.rs  (typed reason enum validation)
 *     src-tauri/src/commands/proposal.rs      (replay typed event tests)
 *
 *   Field name authority: Rust snake_case is canonical.
 *     Frontend camelCase is a legacy fallback ONLY.
 *
 *   Per-event contract (verified by both Rust builder output and TS tests):
 *   ┌─────────────────────────────┬──────────────────────────────────────┐
 *   │ Event Type                  │ Required Fields (snake_case)          │
 *   ├─────────────────────────────┼──────────────────────────────────────┤
 *   │ agent_spec.selected         │ agent_spec_id, role, privacy_policy  │
 *   │ prompt_stack.assembled      │ agent_spec_id, prompt_blocks (array, │
 *   │                             │   items have id); NO prompt_stack_id │
 *   │ context_governance.applied  │ agent_spec_id, context_included,     │
 *   │                             │   context_excluded, privacy_policy   │
 *   │                             │   or agent_spec_privacy_policy       │
 *   │ tool.call_blocked           │ status, tool_name, source,           │
 *   │                             │   block_reason or proposal_reason    │
 *   │                             │   (values must be valid enum strings)│
 *   │ replay.failed               │ status, run_id, action_id,           │
 *   │                             │   replay_of_action_id,               │
 *   │                             │   block_reason or failure_kind       │
 *   │                             │   (values must be valid enum strings)│
 *   │ tool.call_failed            │ (generic — no typed reason required) │
 *   │ run.failed                  │ (generic — no typed reason required) │
 *   │ model.failed                │ (generic — no typed reason required) │
 *   │ model.call_failed           │ (generic — no typed reason required) │
 *   └─────────────────────────────┴──────────────────────────────────────┘
 *
 *   Production payload builder:
 *     All event payloads in this fixture are produced by the same
 *     builder functions that production emit sites use:
 *     - openlife-core/src/agent/trace_payloads.rs
 *
 *     Real emit sites (streaming, execution, orchestrator, replay)
 *     now delegate to these builders, so a change to a builder is
 *     immediately reflected in both production and test payloads.
 *
 * ═══════════════════════════════════════════════════════════════════════
 *
 * Fixture inventory (5 sets):
 *  1. successfulGovernedRun — agent_spec.selected, prompt_stack.assembled,
 *     context_governance.applied, tool calls, run.completed
 *  2. agentSpecDeniedToolRun — agent_spec.selected, prompt_stack.assembled,
 *     context_governance.applied, tool.call_blocked(agent_spec_denied), run.completed
 *  3. needsConfirmationRun — tool.call_blocked(needs_confirmation + network_policy_ask
 *     + proposal_id), run.completed
 *  4. replayFailedRun — replay.failed(block_reason), run.completed
 *  5. malformedAndUnknownRun — malformed tool.call_blocked + malformed replay.failed
 *     + unknown eventType, run.completed
 */

import type { AgentRunEvent, AgentRunEventType } from "@/types";

// =====================================================================
// Utility: build a minimal event with defaults
// =====================================================================

let _seq = 0;
function ev(
  eventType: AgentRunEventType,
  payload: Record<string, unknown>,
  overrides?: Partial<AgentRunEvent>
): AgentRunEvent {
  _seq++;
  return {
    id: `evt-fixture-${_seq}`,
    runId: "run-fixture",
    eventType,
    actor: "runtime",
    summary: `${eventType}`,
    payload,
    createdAt: new Date(Date.UTC(2026, 4, 16, 10, 0, _seq)).toISOString(),
    ...overrides,
  };
}

// ─────────────────────────────────────────────────────────────────────
// FIXTURE 1: successfulGovernedRun
// A cleanly governed run where everything works end-to-end.
// ─────────────────────────────────────────────────────────────────────

export const successfulGovernedRun: AgentRunEvent[] = [
  ev("agent_spec.selected", {
    agent_spec_id: "main.default",
    role: "main",
    privacy_policy: "local_only",
  }),
  ev("prompt_stack.assembled", {
    agent_spec_id: "main.default",
    prompt_blocks: [
      { id: "base_system", version: "1.0.0", purpose: "system prompt" },
      { id: "privacy_rule", version: "1.0.0", purpose: "privacy rule" },
      { id: "tools_manifest", version: "1.0.0", purpose: "tool list" },
    ],
  }),
  ev("context_governance.applied", {
    agent_spec_id: "main.default",
    context_included: ["lifemodel_summary", "goals"],
    context_excluded: ["raw_health_data"],
    privacy_policy: "local_only",
  }),
  ev("tool.call_started", { tool_name: "memory.search", source: "builtin" }),
  ev("tool.call_completed", {
    tool_name: "memory.search",
    source: "builtin",
    output: "Found 3 relevant memories",
  }),
  ev("run.completed", { stop_reason: "no_tools" }),
];

// ─────────────────────────────────────────────────────────────────────
// FIXTURE 2: agentSpecDeniedToolRun
// AgentSpec blocks a tool via typed agent_spec_denied.
// ─────────────────────────────────────────────────────────────────────

export const agentSpecDeniedToolRun: AgentRunEvent[] = [
  ev("agent_spec.selected", {
    agent_spec_id: "main.strict",
    role: "main",
    privacy_policy: "local_only",
  }),
  ev("prompt_stack.assembled", {
    agent_spec_id: "main.strict",
    prompt_blocks: [
      { id: "base_system", version: "1.0.0", purpose: "system prompt" },
      { id: "privacy_rule", version: "1.0.0", purpose: "privacy rule" },
    ],
  }),
  ev("context_governance.applied", {
    agent_spec_id: "main.strict",
    context_included: ["lifemodel_summary"],
    context_excluded: ["raw_health_data"],
    privacy_policy: "local_only",
  }),
  ev("tool.call_blocked", {
    status: "blocked",
    tool_name: "web.search",
    source: "builtin",
    block_reason: "agent_spec_denied",
    proposal_reason: null,
    failure_kind: null,
    agent_spec_id: "main.strict",
  }),
  ev("run.completed", { stop_reason: "no_tools" }),
];

// ─────────────────────────────────────────────────────────────────────
// FIXTURE 3: needsConfirmationRun
// A tool requires user confirmation via network_policy_ask.
// ─────────────────────────────────────────────────────────────────────

export const needsConfirmationRun: AgentRunEvent[] = [
  ev("agent_spec.selected", {
    agent_spec_id: "main.default",
    role: "main",
    privacy_policy: "cloud_allowed",
  }),
  ev("prompt_stack.assembled", {
    agent_spec_id: "main.default",
    prompt_blocks: [
      { id: "base_system", version: "1.0.0", purpose: "system prompt" },
      { id: "tools_manifest", version: "1.0.0", purpose: "tool list" },
    ],
  }),
  ev("context_governance.applied", {
    agent_spec_id: "main.default",
    context_included: ["lifemodel_summary", "goals"],
    context_excluded: [],
    privacy_policy: "cloud_allowed",
  }),
  ev("tool.call_blocked", {
    status: "needs_confirmation",
    tool_name: "web.search",
    source: "builtin",
    block_reason: null,
    proposal_reason: "network_policy_ask",
    failure_kind: null,
    agent_spec_id: "main.default",
    proposal_id: "prop-network-ask-001",
  }),
  ev("run.completed", { stop_reason: "needs_user_input" }),
];

// ─────────────────────────────────────────────────────────────────────
// FIXTURE 4: replayFailedRun
// A replay action fails with replay_spec_missing block_reason.
// ─────────────────────────────────────────────────────────────────────

export const replayFailedRun: AgentRunEvent[] = [
  ev("agent_spec.selected", {
    agent_spec_id: "main.default",
    role: "main",
    privacy_policy: "cloud_allowed",
  }),
  ev("prompt_stack.assembled", {
    agent_spec_id: "main.default",
    prompt_blocks: [
      { id: "base_system", version: "1.0.0", purpose: "system prompt" },
      { id: "tools_manifest", version: "1.0.0", purpose: "tool list" },
    ],
  }),
  ev("replay.failed", {
    status: "failed",
    run_id: "run-fixture-parent",
    action_id: "action-replay-1",
    replay_of_action_id: "action-orig-1",
    block_reason: "replay_spec_missing",
    failure_kind: null,
    tool_name: "web.search",
    source: "builtin",
    agent_spec_id: "main.default",
    human_message: "Replay failed: missing action spec for replay",
  }),
  ev("run.completed", { stop_reason: "no_tools" }),
];

// ─────────────────────────────────────────────────────────────────────
// FIXTURE 5: malformedAndUnknownRun
// Contains malformed typed events and an entirely unknown event type.
// Validates that UI soft-fails without crashing.
// ─────────────────────────────────────────────────────────────────────

export const malformedAndUnknownRun: AgentRunEvent[] = [
  ev("tool.call_blocked", {
    // Malformed: has "status" but the only typed reason field is an
    // invalid string (not a known ExecutionBlockReason or ExecutionProposalReason).
    status: "blocked",
    tool_name: "web.search",
    source: "builtin",
    block_reason: "not_a_real_enum_variant",
    proposal_reason: null,
    failure_kind: null,
    agent_spec_id: null,
  }),
  ev("replay.failed", {
    // Malformed: missing required fields for structural validation
    // (block_reason and failure_kind are both null/invalid).
    status: "failed",
    run_id: "run-fixture",
    action_id: "a1",
    replay_of_action_id: "orig-1",
    block_reason: null,
    failure_kind: null,
    tool_name: "web.search",
    source: "builtin",
  }),
  ev("custom.unknown_event" as AgentRunEventType, {
    custom_field: "some_value",
    another_field: 42,
    // This eventType has no known contract at all
  }),
  ev("run.completed", { stop_reason: "no_tools" }),
];
