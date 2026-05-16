import { describe, it, expect } from "vitest";
import {
  getTypedRunEventViewModel,
  getTypedActionViewModel,
  getTypedToolCallViewModel,
  getTypedProposalHint,
  getTypedRunHints,
  typedStatusSeverity,
  extractTypedActionOutcome,
  getBlockReasonDisplay,
  getProposalReasonDisplay,
  getFailureKindDisplay,
  getTypedReasonBadgesFromEvent,
  getTypedReasonBadgesFromAction,
  getTypedReasonBadgesFromToolCall,
  getTypedOutcomeLabels,
  getTypedEventDetailViewModel,
} from "@/utils/typedContract";
import type { AgentRunEvent, AgentRunEventType } from "@/types";
import type { AgentAction, ToolCallResult, AgentProposal } from "@/tauri";

function makeEvent(
  overrides: Partial<AgentRunEvent> & { eventType: AgentRunEventType }
): AgentRunEvent {
  return {
    id: "evt-1",
    runId: "run-1",
    actor: "runtime",
    summary: "test event",
    payload: {},
    createdAt: new Date().toISOString(),
    ...overrides,
  };
}

// =====================================================================
// getTypedRunEventViewModel
// =====================================================================

describe("getTypedRunEventViewModel", () => {
  it("parses replay.failed with block_reason", () => {
    const event = makeEvent({
      eventType: "replay.failed",
      payload: {
        status: "failed",
        run_id: "run-1",
        action_id: "a1",
        replay_of_action_id: "orig-1",
        block_reason: "replay_spec_missing",
        human_message: "noise text with replay_spec_missing keyword in error",
      },
    });
    const vm = getTypedRunEventViewModel(event);
    expect(vm.typedKind).toBe("replay_failed");
    expect(vm.blockReasonLabel).toBe("缺少重放规格");
    expect(vm.status).toBe("failed");
    expect(vm.severity).toBe("error");
  });

  it("replay.failed without block_reason or failure_kind degrades to unknown", () => {
    const event = makeEvent({
      eventType: "replay.failed",
      summary: "Replay failed: replay_spec_missing in summary text",
      payload: {
        status: "failed",
        run_id: "run-1",
        action_id: "a1",
        replay_of_action_id: "orig-1",
        human_message: "Fallback: replay_spec_missing error",
        // NO block_reason, NO failure_kind typed fields
      },
    });
    const vm = getTypedRunEventViewModel(event);
    // Without valid block_reason or failure_kind, payload fails structural validation
    expect(vm.typedKind).toBe("unknown");
    expect(vm.blockReasonLabel).toBeNull();
    expect(vm.failureKindLabel).toBeNull();
  });

  it("parses tool.call_blocked with block_reason and agent_spec_id", () => {
    const event = makeEvent({
      eventType: "tool.call_blocked",
      payload: {
        status: "blocked",
        tool_name: "web.search",
        source: "builtin",
        block_reason: "agent_spec_denied",
        agent_spec_id: "main.default",
      },
    });
    const vm = getTypedRunEventViewModel(event);
    expect(vm.typedKind).toBe("tool_call_blocked");
    expect(vm.blockReasonLabel).toBe("AgentSpec 拒绝");
    expect(vm.agentSpecId).toBe("main.default");
    expect(vm.toolName).toBe("web.search");
  });

  it("parses tool.call_blocked with proposal_reason (needs_confirmation)", () => {
    const event = makeEvent({
      eventType: "tool.call_blocked",
      payload: {
        status: "needs_confirmation",
        tool_name: "web.search",
        source: "builtin",
        proposal_reason: "network_policy_ask",
        proposal_id: "proposal-1",
      },
    });
    const vm = getTypedRunEventViewModel(event);
    expect(vm.proposalReasonLabel).toBe("网络策略询问");
    expect(vm.proposalId).toBe("proposal-1");
    expect(vm.severity).toBe("warning");
  });

  it("malformed tool.call_blocked with non-string block_reason degrades to unknown", () => {
    const event = makeEvent({
      eventType: "tool.call_blocked",
      payload: {
        status: "blocked",
        tool_name: "test",
        source: "builtin",
        block_reason: 999,
      },
    });
    const vm = getTypedRunEventViewModel(event);
    // block_reason is a number → not a valid reason → fails structural validation
    expect(vm.typedKind).toBe("unknown");
    expect(vm.blockReasonLabel).toBeNull();
  });

  it("handles unknown event type gracefully", () => {
    const event = makeEvent({
      eventType: "unknown" as AgentRunEventType,
      payload: { foo: "bar" },
    });
    const vm = getTypedRunEventViewModel(event);
    expect(vm.typedKind).toBe("unknown");
    expect(vm.blockReasonLabel).toBeNull();
    expect(vm.severity).toBe("info");
  });

  it("replay.completed with block_reason shows typed outcome", () => {
    const event = makeEvent({
      eventType: "replay.completed",
      payload: {
        status: "blocked",
        run_id: "run-1",
        action_id: "a1",
        replay_of_action_id: "orig",
        agent_spec_id: "main.default",
        tool_name: "remote_tool",
        source: "mcp:server",
        block_reason: "replay_spec_missing",
        failure_kind: null,
      },
    });
    const vm = getTypedRunEventViewModel(event);
    expect(vm.typedKind).toBe("replay_completed");
    expect(vm.blockReasonLabel).toBe("缺少重放规格");
    expect(vm.status).toBe("blocked");
  });

  // ── Structural validation: known eventType, empty/malformed payload → unknown ──

  it("replay.failed with empty payload → kind unknown", () => {
    const event = makeEvent({ eventType: "replay.failed", payload: {} });
    const vm = getTypedRunEventViewModel(event);
    expect(vm.typedKind).toBe("unknown");
    expect(vm.blockReasonLabel).toBeNull();
  });

  it("replay.failed with only human_message containing replay_spec_missing → kind unknown", () => {
    const event = makeEvent({
      eventType: "replay.failed",
      payload: {
        status: "failed",
        run_id: "run-1",
        action_id: "a1",
        replay_of_action_id: "orig-1",
        human_message: "Replay failed: replay_spec_missing",
        // NO block_reason, NO failure_kind
      },
    });
    const vm = getTypedRunEventViewModel(event);
    // human_message does not count as typed reason → structural validation fails
    expect(vm.typedKind).toBe("unknown");
    expect(vm.blockReasonLabel).toBeNull();
  });

  it("replay.failed with block_reason = 'unknown_random_string' → kind unknown", () => {
    const event = makeEvent({
      eventType: "replay.failed",
      payload: {
        status: "failed",
        run_id: "run-1",
        action_id: "a1",
        replay_of_action_id: "orig-1",
        block_reason: "unknown_random_string",
      },
    });
    const vm = getTypedRunEventViewModel(event);
    // unknown_random_string is not in VALID_BLOCK_REASONS → fails validation
    expect(vm.typedKind).toBe("unknown");
  });

  it("replay.completed status blocked without block_reason → kind unknown", () => {
    const event = makeEvent({
      eventType: "replay.completed",
      payload: {
        status: "blocked",
        run_id: "run-1",
        action_id: "a1",
        replay_of_action_id: "orig",
        agent_spec_id: "main.default",
        tool_name: "t",
        source: "builtin",
        // NO block_reason — but status is "blocked"
      },
    });
    const vm = getTypedRunEventViewModel(event);
    expect(vm.typedKind).toBe("unknown");
  });

  it("replay.completed status needs_confirmation without proposal_reason → kind unknown", () => {
    const event = makeEvent({
      eventType: "replay.completed",
      payload: {
        status: "needs_confirmation",
        run_id: "run-1",
        action_id: "a1",
        replay_of_action_id: "orig",
        agent_spec_id: "main.default",
        tool_name: "t",
        source: "builtin",
        // NO proposal_reason — but status is "needs_confirmation"
      },
    });
    const vm = getTypedRunEventViewModel(event);
    expect(vm.typedKind).toBe("unknown");
  });

  it("tool.call_blocked status blocked without block_reason → kind unknown", () => {
    const event = makeEvent({
      eventType: "tool.call_blocked",
      payload: {
        status: "blocked",
        tool_name: "web.search",
        source: "builtin",
        // NO block_reason — but status is "blocked"
      },
    });
    const vm = getTypedRunEventViewModel(event);
    expect(vm.typedKind).toBe("unknown");
  });

  it("tool.call_blocked status needs_confirmation without proposal_reason → kind unknown", () => {
    const event = makeEvent({
      eventType: "tool.call_blocked",
      payload: {
        status: "needs_confirmation",
        tool_name: "web.search",
        source: "builtin",
        // NO proposal_reason — but status is "needs_confirmation"
      },
    });
    const vm = getTypedRunEventViewModel(event);
    expect(vm.typedKind).toBe("unknown");
  });

  it("valid replay.failed with block_reason still parses normally", () => {
    const event = makeEvent({
      eventType: "replay.failed",
      payload: {
        status: "failed",
        run_id: "run-1",
        action_id: "a1",
        replay_of_action_id: "orig-1",
        block_reason: "replay_spec_missing",
        human_message: "noise: this is text, ignore",
      },
    });
    const vm = getTypedRunEventViewModel(event);
    expect(vm.typedKind).toBe("replay_failed");
    expect(vm.blockReasonLabel).toBe("缺少重放规格");
  });

  it("valid tool.call_blocked still parses normally", () => {
    const event = makeEvent({
      eventType: "tool.call_blocked",
      payload: {
        status: "blocked",
        tool_name: "web.search",
        source: "builtin",
        block_reason: "network_policy_denied",
      },
    });
    const vm = getTypedRunEventViewModel(event);
    expect(vm.typedKind).toBe("tool_call_blocked");
    expect(vm.blockReasonLabel).toBe("网络策略拒绝");
  });
});

// =====================================================================
// getTypedActionViewModel
// =====================================================================

function makeAction(overrides: Partial<AgentAction> = {}): AgentAction {
  return {
    id: "action-1",
    actionType: "tool_call",
    input: {},
    status: "blocked",
    timestamp: new Date().toISOString(),
    ...overrides,
  };
}

describe("getTypedActionViewModel", () => {
  it("extracts block_reason from structured output", () => {
    const action = makeAction({
      status: "blocked",
      error: "DO NOT USE THIS TEXT: replay_spec_missing occurred",
      output: {
        block_reason: "replay_spec_missing",
        agent_spec_id: "main.default",
      },
    });
    const vm = getTypedActionViewModel(action);
    expect(vm.blockReasonLabel).toBe("缺少重放规格");
    expect(vm.agentSpecId).toBe("main.default");
    expect(vm.typedReasonAvailable).toBe(true);
    expect(vm.isBlocked).toBe(true);
  });

  it("does not infer reason from error text", () => {
    const action = makeAction({
      status: "blocked",
      error: "Error: network_policy_denied during execution",
      // NO output with typed fields
    });
    const vm = getTypedActionViewModel(action);
    expect(vm.blockReasonLabel).toBeNull();
    expect(vm.typedReasonAvailable).toBe(false);
  });

  it("extracts proposal_reason from output", () => {
    const action = makeAction({
      status: "needs_confirmation",
      output: {
        proposal_reason: "network_policy_ask",
        proposal_id: "prop-123",
      },
    });
    const vm = getTypedActionViewModel(action);
    expect(vm.proposalReasonLabel).toBe("网络策略询问");
    expect(vm.proposalId).toBe("prop-123");
    expect(vm.needsConfirmation).toBe(true);
  });

  it("extracts failure_kind", () => {
    const action = makeAction({
      status: "failed",
      output: {
        failure_kind: "mcp_client_error",
      },
    });
    const vm = getTypedActionViewModel(action);
    expect(vm.failureKindLabel).toBe("MCP 客户端错误");
  });

  it("detects declarative_only tool", () => {
    const action = makeAction({
      toolScope: {
        toolId: "email.read",
        toolName: "email.read",
        source: "builtin",
        riskLevel: "low",
        capabilities: ["declarative_only"],
        actionType: "read",
      },
    });
    const vm = getTypedActionViewModel(action);
    expect(vm.isDeclarativeOnly).toBe(true);
  });

  it("typed reason trumps error text conflict", () => {
    // error says "network_policy_denied" but output says "agent_spec_denied"
    const action = makeAction({
      status: "blocked",
      error: "network_policy_denied: blocked by network",
      output: {
        block_reason: "agent_spec_denied",
      },
    });
    const vm = getTypedActionViewModel(action);
    // typed field wins
    expect(vm.blockReasonLabel).toBe("AgentSpec 拒绝");
  });

  // ── Hardened: invalid typed reasons are not accepted ──

  it("output.block_reason = 'unknown_random_string' → blockReasonLabel null, typedReasonAvailable false", () => {
    const action = makeAction({
      status: "blocked",
      output: { block_reason: "unknown_random_string" },
    });
    const vm = getTypedActionViewModel(action);
    expect(vm.blockReasonLabel).toBeNull();
    expect(vm.typedReasonAvailable).toBe(false);
  });

  it("output.block_reason = 123 → blockReasonLabel null", () => {
    const action = makeAction({
      status: "blocked",
      output: { block_reason: 123 },
    });
    const vm = getTypedActionViewModel(action);
    expect(vm.blockReasonLabel).toBeNull();
  });

  it("output.proposal_reason = 'bad_reason' → proposalReasonLabel null", () => {
    const action = makeAction({
      status: "needs_confirmation",
      output: { proposal_reason: "bad_reason" },
    });
    const vm = getTypedActionViewModel(action);
    expect(vm.proposalReasonLabel).toBeNull();
  });

  it("output.failure_kind = 'bad_failure' → failureKindLabel null", () => {
    const action = makeAction({
      status: "failed",
      output: { failure_kind: "bad_failure" },
    });
    const vm = getTypedActionViewModel(action);
    expect(vm.failureKindLabel).toBeNull();
  });

  it("action.error contains network_policy_denied but output has no typed fields → no inference", () => {
    const action = makeAction({
      status: "blocked",
      error: "network_policy_denied: blocked by policy",
      output: { text: "some generic message" },
    });
    const vm = getTypedActionViewModel(action);
    expect(vm.blockReasonLabel).toBeNull();
    expect(vm.typedReasonAvailable).toBe(false);
  });

  it("structured_result valid block_reason preferred over top-level valid block_reason", () => {
    const action = makeAction({
      status: "blocked",
      output: {
        block_reason: "agent_spec_denied",
        structured_result: {
          block_reason: "network_policy_denied",
        },
      },
    });
    const vm = getTypedActionViewModel(action);
    expect(vm.blockReasonLabel).toBe("网络策略拒绝");
  });

  it("structured_result invalid block_reason, top-level valid → falls back to top-level", () => {
    const action = makeAction({
      status: "blocked",
      output: {
        block_reason: "agent_spec_denied",
        structured_result: {
          block_reason: "bad_reason",
        },
      },
    });
    const vm = getTypedActionViewModel(action);
    expect(vm.blockReasonLabel).toBe("AgentSpec 拒绝");
  });

  it("only agent_spec_id/proposal_id without valid reason → typedReasonAvailable false", () => {
    const action = makeAction({
      status: "blocked",
      output: {
        agent_spec_id: "main.default",
        proposal_id: "prop-1",
      },
    });
    const vm = getTypedActionViewModel(action);
    expect(vm.typedReasonAvailable).toBe(false);
    expect(vm.agentSpecId).toBe("main.default");
    expect(vm.proposalId).toBe("prop-1");
  });

  it("block_reason = empty string → typedReasonAvailable false", () => {
    const action = makeAction({
      status: "blocked",
      output: { block_reason: "" },
    });
    const vm = getTypedActionViewModel(action);
    expect(vm.blockReasonLabel).toBeNull();
    expect(vm.typedReasonAvailable).toBe(false);
  });

  it("block_reason = null → typedReasonAvailable false", () => {
    const action = makeAction({
      status: "blocked",
      output: { block_reason: null },
    });
    const vm = getTypedActionViewModel(action);
    expect(vm.blockReasonLabel).toBeNull();
    expect(vm.typedReasonAvailable).toBe(false);
  });
});

// =====================================================================
// getTypedToolCallViewModel
// =====================================================================

function makeCall(overrides: Partial<ToolCallResult> = {}): ToolCallResult {
  return {
    name: "web.search",
    arguments: { query: "test" },
    success: false,
    ...overrides,
  };
}

describe("getTypedToolCallViewModel", () => {
  it("extracts block reason from output object", () => {
    const call = makeCall({
      status: "blocked",
      output: {
        block_reason: "agent_spec_denied",
        agent_spec_id: "main.default",
      },
    });
    const vm = getTypedToolCallViewModel(call);
    expect(vm.blockReasonLabel).toBe("AgentSpec 拒绝");
    expect(vm.agentSpecId).toBe("main.default");
    expect(vm.typedReasonAvailable).toBe(true);
  });

  it("returns nulls when output is a string", () => {
    const call = makeCall({
      output: "network_policy_denied: this is just text",
    });
    const vm = getTypedToolCallViewModel(call);
    expect(vm.blockReasonLabel).toBeNull();
    expect(vm.typedReasonAvailable).toBe(false);
  });

  it("returns nulls when no typed fields in output object", () => {
    const call = makeCall({
      output: { text: "some message", error_desc: "network issue" },
    });
    const vm = getTypedToolCallViewModel(call);
    expect(vm.blockReasonLabel).toBeNull();
    expect(vm.proposalReasonLabel).toBeNull();
    expect(vm.typedReasonAvailable).toBe(false);
  });

  // ── Hardened: invalid typed reasons are not accepted ──

  it("call.output.block_reason = 'unknown_random_string' → not accepted as valid reason", () => {
    const call = makeCall({
      output: { block_reason: "unknown_random_string" },
    });
    const vm = getTypedToolCallViewModel(call);
    expect(vm.blockReasonLabel).toBeNull();
    expect(vm.typedReasonAvailable).toBe(false);
  });

  it("call.output is string containing 'replay_spec_missing' → no typed inference", () => {
    const call = makeCall({
      output: "Error: replay_spec_missing occurred",
    });
    const vm = getTypedToolCallViewModel(call);
    expect(vm.blockReasonLabel).toBeNull();
    expect(vm.typedReasonAvailable).toBe(false);
  });

  it("call.error contains network_policy_denied, output no typed fields → no inference", () => {
    const call = makeCall({
      error: "network_policy_denied: blocked",
      output: { text: "generic" },
    });
    const vm = getTypedToolCallViewModel(call);
    expect(vm.blockReasonLabel).toBeNull();
  });

  it("structured_result valid reason preferred over top-level valid reason", () => {
    const call = makeCall({
      output: {
        block_reason: "agent_spec_denied",
        structured_result: {
          block_reason: "network_policy_denied",
        },
      },
    });
    const vm = getTypedToolCallViewModel(call);
    expect(vm.blockReasonLabel).toBe("网络策略拒绝");
  });

  it("structured_result invalid reason, top-level valid → fallback to top-level", () => {
    const call = makeCall({
      output: {
        block_reason: "agent_spec_denied",
        structured_result: {
          block_reason: "bad_reason",
        },
      },
    });
    const vm = getTypedToolCallViewModel(call);
    expect(vm.blockReasonLabel).toBe("AgentSpec 拒绝");
  });

  it("block_reason = number 999 → not accepted", () => {
    const call = makeCall({
      output: { block_reason: 999 },
    });
    const vm = getTypedToolCallViewModel(call);
    expect(vm.blockReasonLabel).toBeNull();
  });

  it("block_reason = boolean true → not accepted", () => {
    const call = makeCall({
      output: { block_reason: true },
    });
    const vm = getTypedToolCallViewModel(call);
    expect(vm.blockReasonLabel).toBeNull();
  });
});

// =====================================================================
// extractTypedActionOutcome — hardened
// =====================================================================

function makeOutcomeAction(
  overrides: Partial<{
    id: string;
    actionType: string;
    input: Record<string, unknown>;
    status: string;
    permissionDecision?: string;
    startedAt?: string;
    finishedAt?: string;
    error?: string;
    timestamp: string;
    toolScope?: {
      toolId: string;
      toolName: string;
      source: string;
      riskLevel: string;
      capabilities: string[];
      actionType: string;
      requiresConfirmation?: boolean;
      allowed?: boolean;
    };
    output?: string | Record<string, unknown>;
  }> = {}
): any {
  return {
    id: "action-oc-1",
    actionType: "tool_call",
    input: {},
    status: "blocked",
    timestamp: new Date().toISOString(),
    ...overrides,
  };
}

describe("extractTypedActionOutcome", () => {
  it("extracts valid block_reason from output", () => {
    const action = makeOutcomeAction({
      status: "blocked",
      output: { block_reason: "replay_spec_missing", agent_spec_id: "main.default" },
    });
    const outcome = extractTypedActionOutcome(action);
    expect(outcome.blockReason).toBe("replay_spec_missing");
    expect(outcome.agentSpecId).toBe("main.default");
    expect(outcome.typedReasonAvailable).toBe(true);
  });

  it("invalid block_reason → not returned", () => {
    const action = makeOutcomeAction({
      status: "blocked",
      output: { block_reason: "unknown_random_string" },
    });
    const outcome = extractTypedActionOutcome(action);
    expect(outcome.blockReason).toBeNull();
    expect(outcome.typedReasonAvailable).toBe(false);
  });

  it("invalid proposal_reason → not returned", () => {
    const action = makeOutcomeAction({
      status: "needs_confirmation",
      output: { proposal_reason: "bad_proposal_reason" },
    });
    const outcome = extractTypedActionOutcome(action);
    expect(outcome.proposalReason).toBeNull();
    expect(outcome.typedReasonAvailable).toBe(false);
  });

  it("invalid failure_kind → not returned", () => {
    const action = makeOutcomeAction({
      status: "failed",
      output: { failure_kind: "bad_failure_kind" },
    });
    const outcome = extractTypedActionOutcome(action);
    expect(outcome.failureKind).toBeNull();
    expect(outcome.typedReasonAvailable).toBe(false);
  });

  it("structured_result valid reason preferred over top-level valid reason", () => {
    const action = makeOutcomeAction({
      status: "blocked",
      output: {
        block_reason: "agent_spec_denied",
        structured_result: {
          block_reason: "network_policy_denied",
        },
      },
    });
    const outcome = extractTypedActionOutcome(action);
    expect(outcome.blockReason).toBe("network_policy_denied");
    expect(outcome.typedReasonAvailable).toBe(true);
  });

  it("structured_result invalid reason, top-level valid → falls back to top-level", () => {
    const action = makeOutcomeAction({
      status: "blocked",
      output: {
        block_reason: "agent_spec_denied",
        structured_result: {
          block_reason: "bad_reason",
        },
      },
    });
    const outcome = extractTypedActionOutcome(action);
    expect(outcome.blockReason).toBe("agent_spec_denied");
    expect(outcome.typedReasonAvailable).toBe(true);
  });

  it("only proposal_id and agent_spec_id → typedReasonAvailable false", () => {
    const action = makeOutcomeAction({
      status: "blocked",
      output: {
        agent_spec_id: "main.default",
        proposal_id: "prop-1",
      },
    });
    const outcome = extractTypedActionOutcome(action);
    expect(outcome.typedReasonAvailable).toBe(false);
    expect(outcome.agentSpecId).toBe("main.default");
    expect(outcome.proposalId).toBe("prop-1");
  });

  it("output is string → no typed reason (string output not parsed)", () => {
    const action = makeOutcomeAction({
      status: "blocked",
      output: '{"block_reason": "replay_spec_missing"}',
    });
    const outcome = extractTypedActionOutcome(action);
    expect(outcome.blockReason).toBeNull();
    expect(outcome.typedReasonAvailable).toBe(false);
  });

  it("output is undefined → no typed reason", () => {
    const action = makeOutcomeAction({
      status: "blocked",
    });
    const outcome = extractTypedActionOutcome(action);
    expect(outcome.blockReason).toBeNull();
    expect(outcome.typedReasonAvailable).toBe(false);
  });

  it("structured_result valid proposal_reason preferred over top-level", () => {
    const action = makeOutcomeAction({
      status: "needs_confirmation",
      output: {
        proposal_reason: "tool_permission_ask",
        structured_result: {
          proposal_reason: "network_policy_ask",
        },
      },
    });
    const outcome = extractTypedActionOutcome(action);
    expect(outcome.proposalReason).toBe("network_policy_ask");
    expect(outcome.typedReasonAvailable).toBe(true);
  });

  it("structured_result valid failure_kind preferred over top-level", () => {
    const action = makeOutcomeAction({
      status: "failed",
      output: {
        failure_kind: "mcp_client_error",
        structured_result: {
          failure_kind: "internal_error",
        },
      },
    });
    const outcome = extractTypedActionOutcome(action);
    expect(outcome.failureKind).toBe("internal_error");
    expect(outcome.typedReasonAvailable).toBe(true);
  });

  it("number block_reason → not accepted", () => {
    const action = makeOutcomeAction({
      status: "blocked",
      output: { block_reason: 42 },
    });
    const outcome = extractTypedActionOutcome(action);
    expect(outcome.blockReason).toBeNull();
    expect(outcome.typedReasonAvailable).toBe(false);
  });

  it("boolean block_reason → not accepted", () => {
    const action = makeOutcomeAction({
      status: "blocked",
      output: { block_reason: false },
    });
    const outcome = extractTypedActionOutcome(action);
    expect(outcome.blockReason).toBeNull();
    expect(outcome.typedReasonAvailable).toBe(false);
  });

  it("combined: block + proposal + failure all valid → all returned", () => {
    const action = makeOutcomeAction({
      status: "blocked",
      output: {
        block_reason: "replay_spec_missing",
        proposal_reason: "network_policy_ask",
        failure_kind: "internal_error",
      },
    });
    const outcome = extractTypedActionOutcome(action);
    expect(outcome.blockReason).toBe("replay_spec_missing");
    expect(outcome.proposalReason).toBe("network_policy_ask");
    expect(outcome.failureKind).toBe("internal_error");
    expect(outcome.typedReasonAvailable).toBe(true);
  });
});

// =====================================================================
// getTypedProposalHint
// =====================================================================

function makeProposal(overrides: Partial<AgentProposal> = {}): AgentProposal {
  return {
    id: "prop-1",
    runId: "run-1",
    proposalType: "tool_permission",
    source: "manual",
    affectedPath: "tool_permission.test",
    after: {},
    reason: "text reason",
    confidence: 0.8,
    riskLevel: "low",
    status: "pending",
    createdAt: new Date().toISOString(),
    ...overrides,
  };
}

describe("getTypedProposalHint", () => {
  it("detects network_policy_ask via typed boolean field", () => {
    const proposal = makeProposal({
      after: {
        network_policy_ask: true,
        tool_name: "web.search",
        reason: "needs_confirmation:network_policy", // noise text - should be ignored
      },
    });
    const hint = getTypedProposalHint(proposal);
    expect(hint.isNetworkPolicyAsk).toBe(true);
    expect(hint.toolName).toBe("web.search");
  });

  it("does not infer network_policy_ask from reason text", () => {
    const proposal = makeProposal({
      after: {
        tool_name: "some_tool",
        reason: "needs_confirmation:network_policy", // only text, no boolean field
        // NO network_policy_ask: true
      },
    });
    const hint = getTypedProposalHint(proposal);
    expect(hint.isNetworkPolicyAsk).toBe(false);
  });

  it("returns false when after is empty", () => {
    const proposal = makeProposal({ after: {} });
    const hint = getTypedProposalHint(proposal);
    expect(hint.isNetworkPolicyAsk).toBe(false);
    expect(hint.toolName).toBeNull();
  });

  it("extracts typed proposal_reason field", () => {
    const proposal = makeProposal({
      after: {
        proposal_reason: "network_policy_ask",
        tool_name: "net_tool",
      },
    });
    const hint = getTypedProposalHint(proposal);
    expect(hint.proposalReason).toBe("network_policy_ask");
  });

  it("ignores invalid proposal_reason values", () => {
    const proposal = makeProposal({
      after: {
        proposal_reason: "some_random_string",
      },
    });
    const hint = getTypedProposalHint(proposal);
    expect(hint.proposalReason).toBeNull();
  });
});

// =====================================================================
// getTypedRunHints
// =====================================================================

describe("getTypedRunHints", () => {
  it("produces replay_failed hint with block reason", () => {
    const events: AgentRunEvent[] = [
      makeEvent({
        eventType: "replay.failed",
        payload: {
          status: "failed",
          run_id: "run-1",
          action_id: "a1",
          replay_of_action_id: "orig",
          block_reason: "replay_spec_missing",
        },
      }),
    ];
    const hints = getTypedRunHints(events);
    expect(hints).toHaveLength(1);
    expect(hints[0].text).toBe("重放失败：缺少重放规格");
    expect(hints[0].severity).toBe("error");
  });

  it("produces tool blocked hint with block reason", () => {
    const events: AgentRunEvent[] = [
      makeEvent({
        eventType: "tool.call_blocked",
        payload: {
          status: "blocked",
          tool_name: "web.search",
          source: "builtin",
          block_reason: "network_policy_denied",
        },
      }),
    ];
    const hints = getTypedRunHints(events);
    expect(hints).toHaveLength(1);
    expect(hints[0].text).toBe("工具被阻断：网络策略拒绝");
    expect(hints[0].severity).toBe("error");
  });

  it("produces needs confirmation hint with proposal reason", () => {
    const events: AgentRunEvent[] = [
      makeEvent({
        eventType: "tool.call_blocked",
        payload: {
          status: "needs_confirmation",
          tool_name: "web.search",
          source: "builtin",
          proposal_reason: "network_policy_ask",
        },
      }),
    ];
    const hints = getTypedRunHints(events);
    expect(hints).toHaveLength(1);
    expect(hints[0].text).toBe("待确认：网络策略询问");
    expect(hints[0].severity).toBe("warning");
  });

  it("produces no typed hint for replay.failed without valid typed reason", () => {
    const events: AgentRunEvent[] = [
      makeEvent({
        eventType: "replay.failed",
        summary: "Replay failed with replay_spec_missing error",
        payload: {
          status: "failed",
          run_id: "run-1",
          action_id: "a1",
          replay_of_action_id: "orig-1",
          human_message: "Error: replay_spec_missing",
          // NO block_reason, NO failure_kind — fails structural validation
        },
      }),
    ];
    const hints = getTypedRunHints(events);
    // Payload fails validation → parsed as unknown → no typed hint
    expect(hints).toHaveLength(0);
  });

  it("returns empty array for unrelated events", () => {
    const events: AgentRunEvent[] = [
      makeEvent({ eventType: "run.created", payload: {} }),
      makeEvent({ eventType: "run.completed", payload: {} }),
    ];
    const hints = getTypedRunHints(events);
    expect(hints).toHaveLength(0);
  });

  it("deduplicates same typed reason", () => {
    const events: AgentRunEvent[] = [
      makeEvent({
        eventType: "replay.failed",
        payload: {
          status: "failed",
          run_id: "run-1",
          action_id: "a1",
          replay_of_action_id: "orig1",
          block_reason: "replay_spec_missing",
        },
      }),
      makeEvent({
        eventType: "replay.failed",
        payload: {
          status: "failed",
          run_id: "run-1",
          action_id: "a2",
          replay_of_action_id: "orig2",
          block_reason: "replay_spec_missing",
        },
      }),
    ];
    const hints = getTypedRunHints(events);
    // Only one hint even though there are two replay failed events with same reason
    expect(hints).toHaveLength(1);
  });
});

// =====================================================================
// typedStatusSeverity
// =====================================================================

describe("typedStatusSeverity", () => {
  it("returns error for blocked/failed/deny", () => {
    expect(typedStatusSeverity("blocked")).toBe("error");
    expect(typedStatusSeverity("failed")).toBe("error");
    expect(typedStatusSeverity("deny")).toBe("error");
  });
  it("returns warning for needs_confirmation/ask_every_time/pending", () => {
    expect(typedStatusSeverity("needs_confirmation")).toBe("warning");
    expect(typedStatusSeverity("ask_every_time")).toBe("warning");
    expect(typedStatusSeverity("pending")).toBe("warning");
  });
  it("returns success for completed/succeeded", () => {
    expect(typedStatusSeverity("completed")).toBe("success");
    expect(typedStatusSeverity("succeeded")).toBe("success");
  });
  it("returns info for unknown statuses", () => {
    expect(typedStatusSeverity("custom_status")).toBe("info");
  });
});

// =====================================================================
// getBlockReasonDisplay / getProposalReasonDisplay / getFailureKindDisplay
// =====================================================================

describe("getBlockReasonDisplay", () => {
  it("returns badge with correct label and severity for valid reason", () => {
    const badge = getBlockReasonDisplay("agent_spec_denied");
    expect(badge).not.toBeNull();
    expect(badge!.kind).toBe("block_reason");
    expect(badge!.label).toBe("AgentSpec 拒绝");
    expect(badge!.severity).toBe("error");
    expect(badge!.rawReason).toBe("agent_spec_denied");
  });

  it("returns null for invalid reason", () => {
    expect(getBlockReasonDisplay("invalid_reason")).toBeNull();
    expect(getBlockReasonDisplay(123)).toBeNull();
    expect(getBlockReasonDisplay("")).toBeNull();
    expect(getBlockReasonDisplay(null)).toBeNull();
    expect(getBlockReasonDisplay(undefined)).toBeNull();
  });
});

describe("getProposalReasonDisplay", () => {
  it("returns badge with correct label for valid reason", () => {
    const badge = getProposalReasonDisplay("network_policy_ask");
    expect(badge).not.toBeNull();
    expect(badge!.kind).toBe("proposal_reason");
    expect(badge!.label).toBe("网络策略询问");
    expect(badge!.severity).toBe("warning");
  });

  it("returns null for invalid reason", () => {
    expect(getProposalReasonDisplay("bad_reason")).toBeNull();
    expect(getProposalReasonDisplay(999)).toBeNull();
  });
});

describe("getFailureKindDisplay", () => {
  it("returns badge with correct label for valid failure kind", () => {
    const badge = getFailureKindDisplay("mcp_client_error");
    expect(badge).not.toBeNull();
    expect(badge!.kind).toBe("failure_kind");
    expect(badge!.label).toBe("MCP 客户端错误");
    expect(badge!.severity).toBe("error");
  });

  it("returns null for invalid failure kind", () => {
    expect(getFailureKindDisplay("bad_failure")).toBeNull();
    expect(getFailureKindDisplay(true)).toBeNull();
  });
});

// =====================================================================
// getTypedReasonBadgesFromEvent
// =====================================================================

describe("getTypedReasonBadgesFromEvent", () => {
  it("valid replay.failed block_reason → 1 block badge", () => {
    const event = makeEvent({
      eventType: "replay.failed",
      payload: {
        status: "failed",
        run_id: "run-1",
        action_id: "a1",
        replay_of_action_id: "orig-1",
        block_reason: "replay_spec_missing",
      },
    });
    const badges = getTypedReasonBadgesFromEvent(event);
    expect(badges).toHaveLength(1);
    expect(badges[0].kind).toBe("block_reason");
    expect(badges[0].label).toBe("缺少重放规格");
    expect(badges[0].severity).toBe("error");
  });

  it("replay.failed invalid block_reason → 0 badges", () => {
    const event = makeEvent({
      eventType: "replay.failed",
      payload: {
        status: "failed",
        run_id: "run-1",
        action_id: "a1",
        replay_of_action_id: "orig-1",
        block_reason: "invalid_reason",
      },
    });
    // invalid block_reason → fails structural validation → unknown → 0 badges
    const badges = getTypedReasonBadgesFromEvent(event);
    expect(badges).toHaveLength(0);
  });

  it("tool.call_blocked needs_confirmation proposal_reason → proposal badge severity warning", () => {
    const event = makeEvent({
      eventType: "tool.call_blocked",
      payload: {
        status: "needs_confirmation",
        tool_name: "web.search",
        source: "builtin",
        proposal_reason: "network_policy_ask",
      },
    });
    const badges = getTypedReasonBadgesFromEvent(event);
    expect(badges).toHaveLength(1);
    expect(badges[0].kind).toBe("proposal_reason");
    expect(badges[0].label).toBe("网络策略询问");
    expect(badges[0].severity).toBe("warning");
  });

  it("replay.completed with both block_reason and failure_kind → 2 badges", () => {
    const event = makeEvent({
      eventType: "replay.completed",
      payload: {
        status: "blocked",
        run_id: "run-1",
        action_id: "a1",
        replay_of_action_id: "orig",
        agent_spec_id: "main.default",
        tool_name: "t",
        source: "builtin",
        block_reason: "replay_spec_missing",
        failure_kind: "internal_error",
      },
    });
    const badges = getTypedReasonBadgesFromEvent(event);
    expect(badges).toHaveLength(2);
  });

  it("unknown event type → 0 badges", () => {
    const event = makeEvent({ eventType: "run.created", payload: {} });
    expect(getTypedReasonBadgesFromEvent(event)).toHaveLength(0);
  });

  it("event with summary containing reason but no typed payload → 0 badges", () => {
    const event = makeEvent({
      eventType: "replay.failed",
      summary: "replay_spec_missing in summary",
      payload: {
        status: "failed",
        run_id: "run-1",
        action_id: "a1",
        replay_of_action_id: "orig-1",
        human_message: "Error: replay_spec_missing",
        // no block_reason, no failure_kind
      },
    });
    const badges = getTypedReasonBadgesFromEvent(event);
    expect(badges).toHaveLength(0);
  });
});

// =====================================================================
// getTypedReasonBadgesFromAction
// =====================================================================

describe("getTypedReasonBadgesFromAction", () => {
  it("structured_result valid reason → badge label correct", () => {
    const action = makeAction({
      status: "blocked",
      output: {
        block_reason: "agent_spec_denied",
        structured_result: {
          block_reason: "network_policy_denied",
        },
      },
    });
    const badges = getTypedReasonBadgesFromAction(action);
    expect(badges).toHaveLength(1);
    expect(badges[0].label).toBe("网络策略拒绝");
    expect(badges[0].rawReason).toBe("network_policy_denied");
  });

  it("structured_result invalid, top-level valid → fallback to top-level badge", () => {
    const action = makeAction({
      status: "blocked",
      output: {
        block_reason: "agent_spec_denied",
        structured_result: {
          block_reason: "bad_reason",
        },
      },
    });
    const badges = getTypedReasonBadgesFromAction(action);
    expect(badges).toHaveLength(1);
    expect(badges[0].label).toBe("AgentSpec 拒绝");
  });

  it("only agent_spec_id/proposal_id → 0 typed reason badges", () => {
    const action = makeAction({
      status: "blocked",
      output: {
        agent_spec_id: "main.default",
        proposal_id: "prop-1",
      },
    });
    const badges = getTypedReasonBadgesFromAction(action);
    expect(badges).toHaveLength(0);
  });

  it("invalid block_reason → 0 badges", () => {
    const action = makeAction({
      status: "blocked",
      output: { block_reason: "unknown_random_string" },
    });
    expect(getTypedReasonBadgesFromAction(action)).toHaveLength(0);
  });

  it("action error contains reason but output no typed → 0 badges", () => {
    const action = makeAction({
      status: "blocked",
      error: "network_policy_denied: blocked",
      output: { text: "generic" },
    });
    expect(getTypedReasonBadgesFromAction(action)).toHaveLength(0);
  });
});

// =====================================================================
// getTypedReasonBadgesFromToolCall
// =====================================================================

describe("getTypedReasonBadgesFromToolCall", () => {
  it("invalid reason → 0 badges", () => {
    const badges = getTypedReasonBadgesFromToolCall({
      name: "web.search",
      output: { block_reason: "invalid_reason" },
    });
    expect(badges).toHaveLength(0);
  });

  it("valid failure_kind → failure badge", () => {
    const badges = getTypedReasonBadgesFromToolCall({
      name: "mcp.call_tool",
      output: { failure_kind: "mcp_client_error" },
    });
    expect(badges).toHaveLength(1);
    expect(badges[0].kind).toBe("failure_kind");
    expect(badges[0].label).toBe("MCP 客户端错误");
  });

  it("valid block_reason + proposal_reason → 2 badges", () => {
    const badges = getTypedReasonBadgesFromToolCall({
      name: "test",
      output: {
        block_reason: "agent_spec_denied",
        proposal_reason: "network_policy_ask",
      },
    });
    expect(badges).toHaveLength(2);
  });

  it("error text contains reason but no typed fields → 0 badges", () => {
    const badges = getTypedReasonBadgesFromToolCall({
      name: "test",
      error: "network_policy_denied: blocked",
      output: { text: "generic" },
    });
    expect(badges).toHaveLength(0);
  });

  it("number block_reason → 0 badges", () => {
    const badges = getTypedReasonBadgesFromToolCall({
      name: "test",
      output: { block_reason: 999 },
    });
    expect(badges).toHaveLength(0);
  });
});

// =====================================================================
// getTypedOutcomeLabels
// =====================================================================

describe("getTypedOutcomeLabels", () => {
  it("returns labels for valid typed outcome", () => {
    const action = makeOutcomeAction({
      status: "blocked",
      output: {
        block_reason: "replay_spec_missing",
        proposal_reason: "network_policy_ask",
        failure_kind: "internal_error",
      },
    });
    const outcome = extractTypedActionOutcome(action);
    const labels = getTypedOutcomeLabels(outcome);
    expect(labels.blockReasonLabel).toBe("缺少重放规格");
    expect(labels.proposalReasonLabel).toBe("网络策略询问");
    expect(labels.failureKindLabel).toBe("内部错误");
  });

  it("returns nulls for outcome without typed reasons", () => {
    const action = makeOutcomeAction({ status: "blocked" });
    const outcome = extractTypedActionOutcome(action);
    const labels = getTypedOutcomeLabels(outcome);
    expect(labels.blockReasonLabel).toBeNull();
    expect(labels.proposalReasonLabel).toBeNull();
    expect(labels.failureKindLabel).toBeNull();
  });

  it("invalid block_reason → null label", () => {
    const action = makeOutcomeAction({
      status: "blocked",
      output: { block_reason: "invalid_reason" },
    });
    const outcome = extractTypedActionOutcome(action);
    const labels = getTypedOutcomeLabels(outcome);
    expect(labels.blockReasonLabel).toBeNull();
  });
});

// =====================================================================
// getTypedEventDetailViewModel
// =====================================================================

describe("getTypedEventDetailViewModel", () => {
  it("tool.call_blocked valid block_reason → kind=tool_call_blocked, title/status/badges correct", () => {
    const event = makeEvent({
      eventType: "tool.call_blocked",
      payload: {
        status: "blocked",
        tool_name: "web.search",
        source: "builtin",
        block_reason: "agent_spec_denied",
        agent_spec_id: "main.default",
        proposal_id: "prop-1",
      },
    });
    const vm = getTypedEventDetailViewModel(event);
    expect(vm.kind).toBe("tool_call_blocked");
    expect(vm.title).toBe("阻断详情 (Typed Contract)");
    expect(vm.titleIconTone).toBe("error");
    expect(vm.statusLabel).toBe("已阻断");
    expect(vm.toolName).toBe("web.search");
    expect(vm.source).toBe("builtin");
    expect(vm.agentSpecId).toBe("main.default");
    expect(vm.proposalId).toBe("prop-1");
    expect(vm.badges).toHaveLength(1);
    expect(vm.badges[0].label).toBe("AgentSpec 拒绝");
  });

  it("tool.call_blocked needs_confirmation → title/status/badges correct", () => {
    const event = makeEvent({
      eventType: "tool.call_blocked",
      payload: {
        status: "needs_confirmation",
        tool_name: "web.search",
        source: "builtin",
        proposal_reason: "network_policy_ask",
        proposal_id: "prop-2",
      },
    });
    const vm = getTypedEventDetailViewModel(event);
    expect(vm.kind).toBe("tool_call_blocked");
    expect(vm.title).toBe("需确认详情 (Typed Contract)");
    expect(vm.statusLabel).toBe("需确认");
    expect(vm.statusTone).toBe("warning");
    expect(vm.badges).toHaveLength(1);
    expect(vm.badges[0].kind).toBe("proposal_reason");
    expect(vm.badges[0].label).toBe("网络策略询问");
    expect(vm.proposalId).toBe("prop-2");
  });

  it("tool.call_blocked with MCP wrapper → target fields present", () => {
    const event = makeEvent({
      eventType: "tool.call_blocked",
      payload: {
        status: "blocked",
        tool_name: "mcp.call_tool",
        source: "builtin",
        block_reason: "tool_permission_denied",
        target_tool_name: "remote_search",
        target_source: "mcp:my-server",
        wrapper_tool_name: "mcp.call_tool",
      },
    });
    const vm = getTypedEventDetailViewModel(event);
    expect(vm.targetToolName).toBe("remote_search");
    expect(vm.targetSource).toBe("mcp:my-server");
    expect(vm.wrapperToolName).toBe("mcp.call_tool");
  });

  it("replay.started → kind/title/status/metadata correct", () => {
    const event = makeEvent({
      eventType: "replay.started",
      payload: {
        status: "started",
        run_id: "run-1",
        action_id: "action-replay-1",
        replay_of_action_id: "orig-action-1",
        agent_spec_id: "main.default",
        tool_name: "web.search",
        source: "builtin",
      },
    });
    const vm = getTypedEventDetailViewModel(event);
    expect(vm.kind).toBe("replay_started");
    expect(vm.title).toBe("重放开始 (Typed Contract)");
    expect(vm.statusLabel).toBe("started");
    expect(vm.replayOfActionId).toBe("orig-action-1");
    expect(vm.toolName).toBe("web.search");
    expect(vm.source).toBe("builtin");
    expect(vm.agentSpecId).toBe("main.default");
    expect(vm.badges).toHaveLength(0);
  });

  it("replay.completed completed → statusLabel=成功, badges empty", () => {
    const event = makeEvent({
      eventType: "replay.completed",
      payload: {
        status: "completed",
        run_id: "run-1",
        action_id: "a1",
        replay_of_action_id: "orig-1",
        agent_spec_id: "main.default",
        tool_name: "t",
        source: "builtin",
        block_reason: null,
        proposal_reason: null,
        failure_kind: null,
      },
    });
    const vm = getTypedEventDetailViewModel(event);
    expect(vm.kind).toBe("replay_completed");
    expect(vm.statusLabel).toBe("成功");
    expect(vm.statusTone).toBe("success");
    expect(vm.titleIconTone).toBe("success");
    expect(vm.badges).toHaveLength(0);
  });

  it("replay.completed blocked → statusLabel=已阻断, badges include block reason", () => {
    const event = makeEvent({
      eventType: "replay.completed",
      payload: {
        status: "blocked",
        run_id: "run-1",
        action_id: "a1",
        replay_of_action_id: "orig-1",
        agent_spec_id: "main.default",
        tool_name: "t",
        source: "builtin",
        block_reason: "replay_spec_missing",
      },
    });
    const vm = getTypedEventDetailViewModel(event);
    expect(vm.statusLabel).toBe("已阻断");
    expect(vm.statusTone).toBe("error");
    expect(vm.titleIconTone).toBe("error");
    expect(vm.badges).toHaveLength(1);
    expect(vm.badges[0].label).toBe("缺少重放规格");
  });

  it("replay.completed needs_confirmation → statusLabel=需确认, badges include proposal reason", () => {
    const event = makeEvent({
      eventType: "replay.completed",
      payload: {
        status: "needs_confirmation",
        run_id: "run-1",
        action_id: "a1",
        replay_of_action_id: "orig-1",
        agent_spec_id: "main.default",
        tool_name: "t",
        source: "builtin",
        proposal_reason: "network_policy_ask",
      },
    });
    const vm = getTypedEventDetailViewModel(event);
    expect(vm.statusLabel).toBe("需确认");
    expect(vm.statusTone).toBe("warning");
    expect(vm.titleIconTone).toBe("warning");
    expect(vm.badges).toHaveLength(1);
    expect(vm.badges[0].kind).toBe("proposal_reason");
  });

  it("replay.failed → kind/title/status correct, badges include block/failure", () => {
    const event = makeEvent({
      eventType: "replay.failed",
      payload: {
        status: "failed",
        run_id: "run-1",
        action_id: "a1",
        replay_of_action_id: "orig-1",
        block_reason: "replay_spec_missing",
        failure_kind: "internal_error",
        human_message: "Replay failed details",
        tool_name: "remote_tool",
        source: "mcp:srv",
        agent_spec_id: "main.default",
      },
    });
    const vm = getTypedEventDetailViewModel(event);
    expect(vm.kind).toBe("replay_failed");
    expect(vm.title).toBe("重放失败 (Typed Contract)");
    expect(vm.titleIconTone).toBe("error");
    expect(vm.statusLabel).toBe("failed");
    expect(vm.statusTone).toBe("error");
    expect(vm.badges).toHaveLength(2);
    expect(vm.humanMessage).toBe("Replay failed details");
    expect(vm.toolName).toBe("remote_tool");
    expect(vm.source).toBe("mcp:srv");
    expect(vm.agentSpecId).toBe("main.default");
  });

  it("malformed replay.failed → kind=unknown, no badges", () => {
    const event = makeEvent({
      eventType: "replay.failed",
      payload: {
        status: "failed",
        run_id: "run-1",
        action_id: "a1",
        replay_of_action_id: "orig-1",
        block_reason: "invalid_reason",
        // invalid block_reason + no failure_kind → structural validation fails
      },
    });
    const vm = getTypedEventDetailViewModel(event);
    expect(vm.kind).toBe("unknown");
    expect(vm.title).toBe("");
    expect(vm.badges).toHaveLength(0);
  });

  it("replay.failed with no typed reason but human_message contains reason → kind=unknown", () => {
    const event = makeEvent({
      eventType: "replay.failed",
      summary: "Replay failed: replay_spec_missing in summary",
      payload: {
        status: "failed",
        run_id: "run-1",
        action_id: "a1",
        replay_of_action_id: "orig-1",
        human_message: "Error: replay_spec_missing",
        // NO block_reason, NO failure_kind — must not infer from text
      },
    });
    const vm = getTypedEventDetailViewModel(event);
    expect(vm.kind).toBe("unknown");
    expect(vm.badges).toHaveLength(0);
  });

  it("unknown event type → kind=unknown, no badges", () => {
    const event = makeEvent({
      eventType: "run.created",
      payload: { session_id: "sess-1" },
    });
    const vm = getTypedEventDetailViewModel(event);
    expect(vm.kind).toBe("unknown");
    expect(vm.title).toBe("");
    expect(vm.badges).toHaveLength(0);
  });

  it("valid replay.failed with block_reason and failure_kind has 2 badges", () => {
    const event = makeEvent({
      eventType: "replay.failed",
      payload: {
        status: "failed",
        run_id: "run-1",
        action_id: "a1",
        replay_of_action_id: "orig-1",
        block_reason: "replay_spec_missing",
        failure_kind: "missing_mcp_server",
      },
    });
    const vm = getTypedEventDetailViewModel(event);
    expect(vm.badges).toHaveLength(2);
  });
});

// =====================================================================
// Trace Contract: Summary-Misleading Cases
// =====================================================================

describe("summary misleading but typed payload correct", () => {
  it("tool.call_blocked: summary says network_policy_denied but typed says agent_spec_denied → typed wins", () => {
    const event = makeEvent({
      eventType: "tool.call_blocked",
      summary: "web.search blocked by network_policy_denied — IGNORE THIS TEXT",
      payload: {
        status: "blocked",
        tool_name: "web.search",
        source: "builtin",
        block_reason: "agent_spec_denied",
        agent_spec_id: "main.default",
      },
    });
    const vm = getTypedRunEventViewModel(event);
    expect(vm.blockReasonLabel).toBe("AgentSpec 拒绝");
    expect(vm.blockReasonLabel).not.toBe("网络策略拒绝");
  });

  it("replay.failed: summary says internal_error but typed says replay_spec_missing → typed wins", () => {
    const event = makeEvent({
      eventType: "replay.failed",
      summary: "Replay failed: internal_error occurred — IGNORE",
      payload: {
        status: "failed",
        run_id: "run-1",
        action_id: "a1",
        replay_of_action_id: "orig-1",
        block_reason: "replay_spec_missing",
        human_message: "Error: internal_error (noise in human_message)",
      },
    });
    const vm = getTypedRunEventViewModel(event);
    expect(vm.blockReasonLabel).toBe("缺少重放规格");
    expect(vm.blockReasonLabel).not.toBe("内部错误");
  });

  it("tool.call_blocked: summary lacks reason keyword but typed payload has valid block_reason → typed still wins", () => {
    const event = makeEvent({
      eventType: "tool.call_blocked",
      summary: "Generic tool call blocked",
      payload: {
        status: "blocked",
        tool_name: "remote_tool",
        source: "mcp:server",
        block_reason: "missing_mcp_client",
      },
    });
    const vm = getTypedRunEventViewModel(event);
    expect(vm.blockReasonLabel).toBe("缺少 MCP 客户端");
    expect(vm.typedKind).toBe("tool_call_blocked");
  });
});

// =====================================================================
// Trace Contract: Non-Standard ToolCallBlocked Payloads
// =====================================================================

describe("non-standard tool.call_blocked payloads degrade gracefully", () => {
  it("budget exceeded event (tools.rs:54) → kind=unknown (no typed fields)", () => {
    const event = makeEvent({
      eventType: "tool.call_blocked",
      summary: "Tool call budget exceeded",
      payload: {
        max_tool_calls: 6,
        current_count: 6,
      },
    });
    const vm = getTypedRunEventViewModel(event);
    // No status/tool_name/source → structural validation fails
    expect(vm.typedKind).toBe("unknown");
    expect(vm.blockReasonLabel).toBeNull();
  });

  it("plan executor block event (plan_executor.rs:284) → kind=unknown", () => {
    const event = makeEvent({
      eventType: "tool.call_blocked",
      summary: "tool blocked by AgentSpec",
      payload: {
        tool_name: "life_model.read",
        agentspec_id: "plan-agent",
      },
    });
    const vm = getTypedRunEventViewModel(event);
    // No status, source → structural validation fails
    expect(vm.typedKind).toBe("unknown");
  });
});
