import type {
  AgentRunEvent,
  ExecutionBlockReason,
  ExecutionProposalReason,
  ExecutionFailureKind,
  TypedEventPayload,
} from "../types";
import type { AgentAction, AgentProposal, ToolCallResult } from "../tauri";

// =====================================================================
// Label Maps — single source of truth for all human-readable labels
// =====================================================================

export const BLOCK_REASON_LABELS: Record<ExecutionBlockReason, string> = {
  agent_spec_denied: "AgentSpec 拒绝",
  agent_spec_missing: "缺少 AgentSpec",
  tool_permission_denied: "工具权限拒绝",
  network_policy_denied: "网络策略拒绝",
  sandbox_denied: "沙箱拒绝",
  missing_mcp_client: "缺少 MCP 客户端",
  disabled_manifest: "清单已禁用",
  declarative_only: "仅声明式",
  invalid_arguments: "无效参数",
  replay_spec_missing: "缺少重放规格",
  path_not_safe: "路径不安全",
  domain_blocked: "域名被阻断",
  pii_detected: "检测到 PII",
  unknown: "未知原因",
};

export const PROPOSAL_REASON_LABELS: Record<ExecutionProposalReason, string> = {
  network_policy_ask: "网络策略询问",
  tool_permission_ask: "工具权限询问",
  high_risk_action: "高风险动作",
};

export const FAILURE_KIND_LABELS: Record<ExecutionFailureKind, string> = {
  tool_runtime_error: "工具运行时错误",
  mcp_client_error: "MCP 客户端错误",
  missing_mcp_server: "缺少 MCP 服务器",
  internal_error: "内部错误",
  serialization_error: "序列化错误",
};

// =====================================================================
// Severity — uniform across all UI
// =====================================================================

export type TypedSeverity = "info" | "warning" | "error" | "success";

export function typedStatusSeverity(status: string): TypedSeverity {
  switch (status) {
    case "blocked":
    case "failed":
    case "deny":
      return "error";
    case "needs_confirmation":
    case "ask_every_time":
    case "pending":
      return "warning";
    case "completed":
    case "succeeded":
    case "success":
    case "allow":
    case "allow_once":
    case "allow_until_revoked":
      return "success";
    default:
      return "info";
  }
}

export function blockReasonSeverity(reason: ExecutionBlockReason): "error" | "warning" | "info" {
  switch (reason) {
    case "agent_spec_denied":
    case "agent_spec_missing":
    case "tool_permission_denied":
    case "network_policy_denied":
    case "sandbox_denied":
    case "missing_mcp_client":
    case "path_not_safe":
    case "domain_blocked":
    case "pii_detected":
    case "replay_spec_missing":
      return "error";
    case "disabled_manifest":
    case "declarative_only":
    case "invalid_arguments":
      return "warning";
    default:
      return "info";
  }
}

// =====================================================================
// Payload field extraction (internal helpers — NOT exported)
// =====================================================================

function isValidNonEmptyString(v: unknown): v is string {
  return typeof v === "string" && v.length > 0;
}

// =====================================================================
// Payload structural validation helpers (NOT exported)
// =====================================================================

function isNonEmptyString(v: unknown): boolean {
  return typeof v === "string" && v.length > 0;
}

function isValidStatus(v: unknown, allowed: string[]): boolean {
  return typeof v === "string" && allowed.includes(v);
}

// =====================================================================
// parseTypedEventPayload — with structural validation
// =====================================================================

/**
 * Parse event payload into a TypedEventPayload discriminated union.
 *
 * Each known eventType branch performs structural validation.
 * If the payload is missing required structured fields, the event is
 * degraded to `{ kind: "unknown" }` — the eventType alone is never
 * sufficient to declare typed payload success.
 */
export function parseTypedEventPayload(event: AgentRunEvent): TypedEventPayload {
  const p = event.payload as Record<string, unknown>;

  // --- tool.call_blocked -------------------------------------------------
  if (event.eventType === "tool.call_blocked") {
    const status = p.status;
    const toolName = p.tool_name;
    const source = p.source;
    const blockReason = p.block_reason;
    const proposalReason = p.proposal_reason;

    if (!isValidStatus(status, ["blocked", "needs_confirmation"]))
      return { kind: "unknown", data: p };
    if (!isNonEmptyString(toolName)) return { kind: "unknown", data: p };
    if (!isNonEmptyString(source)) return { kind: "unknown", data: p };

    if (status === "blocked" && !isValidBlockReason(blockReason))
      return { kind: "unknown", data: p };
    if (status === "needs_confirmation" && !isValidProposalReason(proposalReason))
      return { kind: "unknown", data: p };

    return {
      kind: "tool_call_blocked",
      data: {
        status: status as "blocked" | "needs_confirmation",
        tool_name: toolName as string,
        source: source as string,
        block_reason: isValidBlockReason(blockReason) ? blockReason : null,
        proposal_reason: isValidProposalReason(proposalReason) ? proposalReason : null,
        failure_kind: null,
        agent_spec_id: isNonEmptyString(p.agent_spec_id) ? (p.agent_spec_id as string) : null,
        human_message: isNonEmptyString(p.human_message)
          ? (p.human_message as string)
          : event.summary,
        target_tool_name: isNonEmptyString(p.target_tool_name)
          ? (p.target_tool_name as string)
          : undefined,
        target_source: isNonEmptyString(p.target_source) ? (p.target_source as string) : undefined,
        wrapper_tool_name: isNonEmptyString(p.wrapper_tool_name)
          ? (p.wrapper_tool_name as string)
          : undefined,
        proposal_id: isNonEmptyString(p.proposal_id) ? (p.proposal_id as string) : undefined,
      },
    };
  }

  // --- replay.started ----------------------------------------------------
  if (event.eventType === "replay.started") {
    const status = p.status;
    if (!isValidStatus(status, ["started"])) return { kind: "unknown", data: p };
    if (!isNonEmptyString(p.run_id)) return { kind: "unknown", data: p };
    if (!isNonEmptyString(p.action_id)) return { kind: "unknown", data: p };
    if (!isNonEmptyString(p.replay_of_action_id)) return { kind: "unknown", data: p };
    if (!isNonEmptyString(p.agent_spec_id)) return { kind: "unknown", data: p };
    if (!isNonEmptyString(p.tool_name)) return { kind: "unknown", data: p };
    if (!isNonEmptyString(p.source)) return { kind: "unknown", data: p };

    return {
      kind: "replay_started",
      data: {
        status: "started",
        run_id: p.run_id as string,
        action_id: p.action_id as string,
        replay_of_action_id: p.replay_of_action_id as string,
        agent_spec_id: p.agent_spec_id as string,
        tool_name: p.tool_name as string,
        source: p.source as string,
      },
    };
  }

  // --- replay.completed --------------------------------------------------
  if (event.eventType === "replay.completed") {
    const status = p.status;
    if (!isValidStatus(status, ["completed", "blocked", "needs_confirmation"]))
      return { kind: "unknown", data: p };
    if (!isNonEmptyString(p.run_id)) return { kind: "unknown", data: p };
    if (!isNonEmptyString(p.action_id)) return { kind: "unknown", data: p };
    if (!isNonEmptyString(p.replay_of_action_id)) return { kind: "unknown", data: p };
    if (!isNonEmptyString(p.agent_spec_id)) return { kind: "unknown", data: p };
    if (!isNonEmptyString(p.tool_name)) return { kind: "unknown", data: p };
    if (!isNonEmptyString(p.source)) return { kind: "unknown", data: p };

    const blockReason = p.block_reason;
    const proposalReason = p.proposal_reason;

    if (status === "blocked" && !isValidBlockReason(blockReason))
      return { kind: "unknown", data: p };
    if (status === "needs_confirmation" && !isValidProposalReason(proposalReason))
      return { kind: "unknown", data: p };

    return {
      kind: "replay_completed",
      data: {
        status: status as "completed" | "blocked" | "needs_confirmation",
        run_id: p.run_id as string,
        action_id: p.action_id as string,
        replay_of_action_id: p.replay_of_action_id as string,
        agent_spec_id: p.agent_spec_id as string,
        tool_name: p.tool_name as string,
        source: p.source as string,
        block_reason: isValidBlockReason(blockReason) ? blockReason : null,
        proposal_reason: isValidProposalReason(proposalReason) ? proposalReason : null,
        failure_kind: isValidFailureKind(p.failure_kind) ? p.failure_kind : null,
      },
    };
  }

  // --- replay.failed -----------------------------------------------------
  if (event.eventType === "replay.failed") {
    const blockReason = p.block_reason;
    const failureKind = p.failure_kind;
    const hasValidBlockReason = isValidBlockReason(blockReason);
    const hasValidFailureKind = isValidFailureKind(failureKind);

    // Must have at least one typed reason — human_message / summary do NOT count
    if (!hasValidBlockReason && !hasValidFailureKind) return { kind: "unknown", data: p };

    if (!isNonEmptyString(p.run_id)) return { kind: "unknown", data: p };
    if (!isNonEmptyString(p.action_id)) return { kind: "unknown", data: p };
    if (!isNonEmptyString(p.replay_of_action_id)) return { kind: "unknown", data: p };

    return {
      kind: "replay_failed",
      data: {
        status: "failed",
        run_id: p.run_id as string,
        action_id: p.action_id as string,
        replay_of_action_id: p.replay_of_action_id as string,
        human_message: isNonEmptyString(p.human_message)
          ? (p.human_message as string)
          : event.summary,
        block_reason: hasValidBlockReason ? blockReason : null,
        failure_kind: hasValidFailureKind ? failureKind : null,
        tool_name: isNonEmptyString(p.tool_name) ? (p.tool_name as string) : null,
        source: isNonEmptyString(p.source) ? (p.source as string) : null,
        agent_spec_id: isNonEmptyString(p.agent_spec_id) ? (p.agent_spec_id as string) : null,
      },
    };
  }

  return { kind: "unknown", data: p };
}

// =====================================================================
// Typed field extraction with structured_result priority
// =====================================================================

/**
 * Extract a typed field value with per-field structured_result priority.
 * - If structured_result has a valid value for `field`, use it.
 * - Otherwise, fall back to output's valid value.
 * - Invalid values (unknown strings, numbers, booleans, etc.) are never returned.
 */
function firstValidTypedValue<T>(
  output: Record<string, unknown>,
  structuredResult: Record<string, unknown> | undefined,
  field: string,
  validator: (v: unknown) => v is T
): T | null {
  if (structuredResult) {
    const srVal = structuredResult[field];
    if (validator(srVal)) return srVal;
  }
  const outVal = output[field];
  if (validator(outVal)) return outVal;
  return null;
}

// =====================================================================
// extractTypedActionOutcome — hardened
// =====================================================================

/**
 * Extract typed execution outcome from a replayed AgentAction.
 * Does NOT parse the error string for reasons — only inspects
 * structured typed fields (block_reason, proposal_reason, failure_kind).
 *
 * Per-field structured_result priority:
 *   - structured_result valid value preferred over output top-level valid value.
 *   - Invalid values (unknown strings, numbers, booleans, null/undefined)
 *     are never returned as reasons.
 *   - typedReasonAvailable is true only if at least one valid reason exists.
 */
export function extractTypedActionOutcome(action: {
  id: string;
  status: string;
  error?: string;
  output?: string | Record<string, unknown>;
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
}): {
  status: string;
  blockReason: string | null;
  proposalReason: string | null;
  failureKind: string | null;
  agentSpecId: string | null;
  proposalId: string | null;
  toolName?: string;
  source?: string;
  riskLevel?: string;
  actionType?: string;
  typedReasonAvailable: boolean;
} {
  const output: Record<string, unknown> = {};
  if (typeof action.output === "object" && action.output !== null) {
    Object.assign(output, action.output as Record<string, unknown>);
  }

  let structuredResult: Record<string, unknown> | undefined = undefined;
  if (
    output.structured_result &&
    typeof output.structured_result === "object" &&
    output.structured_result !== null
  ) {
    structuredResult = output.structured_result as Record<string, unknown>;
  }

  const blockReason = firstValidTypedValue(
    output,
    structuredResult,
    "block_reason",
    isValidBlockReason
  );
  const proposalReason = firstValidTypedValue(
    output,
    structuredResult,
    "proposal_reason",
    isValidProposalReason
  );
  const failureKind = firstValidTypedValue(
    output,
    structuredResult,
    "failure_kind",
    isValidFailureKind
  );
  const agentSpecId = firstValidTypedValue(
    output,
    structuredResult,
    "agent_spec_id",
    isValidNonEmptyString
  );
  const proposalId = firstValidTypedValue(
    output,
    structuredResult,
    "proposal_id",
    isValidNonEmptyString
  );

  return {
    status: action.status,
    blockReason,
    proposalReason,
    failureKind,
    agentSpecId,
    proposalId,
    toolName: action.toolScope?.toolName,
    source: action.toolScope?.source,
    riskLevel: action.toolScope?.riskLevel,
    actionType: action.toolScope?.actionType,
    typedReasonAvailable: blockReason !== null || proposalReason !== null || failureKind !== null,
  };
}

// =====================================================================
// Validation helpers (ensure values are valid enum members, not numbers etc.)
// =====================================================================

const VALID_BLOCK_REASONS = new Set<string>(Object.keys(BLOCK_REASON_LABELS));
const VALID_PROPOSAL_REASONS = new Set<string>(Object.keys(PROPOSAL_REASON_LABELS));
const VALID_FAILURE_KINDS = new Set<string>(Object.keys(FAILURE_KIND_LABELS));

function isValidBlockReason(v: unknown): v is ExecutionBlockReason {
  return typeof v === "string" && VALID_BLOCK_REASONS.has(v);
}

function isValidProposalReason(v: unknown): v is ExecutionProposalReason {
  return typeof v === "string" && VALID_PROPOSAL_REASONS.has(v);
}

function isValidFailureKind(v: unknown): v is ExecutionFailureKind {
  return typeof v === "string" && VALID_FAILURE_KINDS.has(v);
}

// =====================================================================
// NEW: TypedEventViewModel — unified event view model
// =====================================================================

export interface TypedEventViewModel {
  eventType: string;
  typedKind: TypedEventPayload["kind"];
  label: string;
  severity: TypedSeverity;
  blockReasonLabel: string | null;
  proposalReasonLabel: string | null;
  failureKindLabel: string | null;
  toolName: string | null;
  source: string | null;
  agentSpecId: string | null;
  proposalId: string | null;
  targetToolName: string | null;
  wrapperToolName: string | null;
  humanMessage: string | null;
  status: string;
}

export function getTypedRunEventViewModel(event: AgentRunEvent): TypedEventViewModel {
  const parsed = parseTypedEventPayload(event);
  const severity = typedStatusSeverity(
    parsed.kind === "tool_call_blocked"
      ? parsed.data.status
      : parsed.kind === "replay_completed"
        ? parsed.data.status
        : parsed.kind === "replay_failed"
          ? "failed"
          : event.eventType.startsWith("run.failed") || event.eventType.startsWith("model.failed")
            ? "failed"
            : event.eventType.startsWith("run.completed") ||
                event.eventType.startsWith("model.call_completed")
              ? "completed"
              : "info"
  );

  let vm: TypedEventViewModel;

  switch (parsed.kind) {
    case "tool_call_blocked": {
      const d = parsed.data;
      vm = {
        eventType: event.eventType,
        typedKind: "tool_call_blocked",
        label: getEventLabel(event.eventType),
        severity,
        blockReasonLabel: isValidBlockReason(d.block_reason)
          ? BLOCK_REASON_LABELS[d.block_reason]
          : null,
        proposalReasonLabel: isValidProposalReason(d.proposal_reason)
          ? PROPOSAL_REASON_LABELS[d.proposal_reason]
          : null,
        failureKindLabel: null,
        toolName: d.tool_name || null,
        source: d.source || null,
        agentSpecId: d.agent_spec_id || null,
        proposalId: d.proposal_id || null,
        targetToolName: d.target_tool_name || null,
        wrapperToolName: d.wrapper_tool_name || null,
        humanMessage: d.human_message || null,
        status: d.status,
      };
      break;
    }
    case "replay_started": {
      const d = parsed.data;
      vm = {
        eventType: event.eventType,
        typedKind: "replay_started",
        label: getEventLabel(event.eventType),
        severity: "info",
        blockReasonLabel: null,
        proposalReasonLabel: null,
        failureKindLabel: null,
        toolName: d.tool_name || null,
        source: d.source || null,
        agentSpecId: d.agent_spec_id || null,
        proposalId: null,
        targetToolName: null,
        wrapperToolName: null,
        humanMessage: null,
        status: "started",
      };
      break;
    }
    case "replay_completed": {
      const d = parsed.data;
      vm = {
        eventType: event.eventType,
        typedKind: "replay_completed",
        label: getEventLabel(event.eventType),
        severity,
        blockReasonLabel: isValidBlockReason(d.block_reason)
          ? BLOCK_REASON_LABELS[d.block_reason]
          : null,
        proposalReasonLabel: isValidProposalReason(d.proposal_reason)
          ? PROPOSAL_REASON_LABELS[d.proposal_reason]
          : null,
        failureKindLabel: isValidFailureKind(d.failure_kind)
          ? FAILURE_KIND_LABELS[d.failure_kind]
          : null,
        toolName: d.tool_name || null,
        source: d.source || null,
        agentSpecId: d.agent_spec_id || null,
        proposalId: null,
        targetToolName: null,
        wrapperToolName: null,
        humanMessage: null,
        status: d.status,
      };
      break;
    }
    case "replay_failed": {
      const d = parsed.data;
      vm = {
        eventType: event.eventType,
        typedKind: "replay_failed",
        label: getEventLabel(event.eventType),
        severity,
        blockReasonLabel: isValidBlockReason(d.block_reason)
          ? BLOCK_REASON_LABELS[d.block_reason]
          : null,
        proposalReasonLabel: null,
        failureKindLabel: isValidFailureKind(d.failure_kind)
          ? FAILURE_KIND_LABELS[d.failure_kind]
          : null,
        toolName: d.tool_name || null,
        source: d.source || null,
        agentSpecId: d.agent_spec_id || null,
        proposalId: null,
        targetToolName: null,
        wrapperToolName: null,
        humanMessage: d.human_message || null,
        status: "failed",
      };
      break;
    }
    default:
      vm = {
        eventType: event.eventType,
        typedKind: "unknown",
        label: getEventLabel(event.eventType),
        severity,
        blockReasonLabel: null,
        proposalReasonLabel: null,
        failureKindLabel: null,
        toolName: null,
        source: null,
        agentSpecId: null,
        proposalId: null,
        targetToolName: null,
        wrapperToolName: null,
        humanMessage: null,
        status: event.eventType.startsWith("run.failed") ? "failed" : "unknown",
      };
      break;
  }

  return vm;
}

// =====================================================================
// NEW: TypedActionViewModel — unified action view model
// =====================================================================

export interface TypedActionViewModel {
  status: string;
  isBlocked: boolean;
  isFailed: boolean;
  isSuccess: boolean;
  needsConfirmation: boolean;
  blockReasonLabel: string | null;
  proposalReasonLabel: string | null;
  failureKindLabel: string | null;
  agentSpecId: string | null;
  proposalId: string | null;
  toolName: string | null;
  source: string | null;
  riskLevel: string | null;
  actionType: string | null;
  requiresConfirmation: boolean | undefined;
  isAllowed: boolean | undefined;
  isDeclarativeOnly: boolean;
  typedReasonAvailable: boolean;
}

export function getTypedActionViewModel(action: AgentAction): TypedActionViewModel {
  const output: Record<string, unknown> = {};
  if (typeof action.output === "object" && action.output !== null) {
    Object.assign(output, action.output as Record<string, unknown>);
  }

  let structuredResult: Record<string, unknown> | undefined = undefined;
  if (
    output.structured_result &&
    typeof output.structured_result === "object" &&
    output.structured_result !== null
  ) {
    structuredResult = output.structured_result as Record<string, unknown>;
  }

  const blockReason = firstValidTypedValue(
    output,
    structuredResult,
    "block_reason",
    isValidBlockReason
  );
  const proposalReason = firstValidTypedValue(
    output,
    structuredResult,
    "proposal_reason",
    isValidProposalReason
  );
  const failureKind = firstValidTypedValue(
    output,
    structuredResult,
    "failure_kind",
    isValidFailureKind
  );
  const agentSpecId = firstValidTypedValue(
    output,
    structuredResult,
    "agent_spec_id",
    isValidNonEmptyString
  );
  const proposalId = firstValidTypedValue(
    output,
    structuredResult,
    "proposal_id",
    isValidNonEmptyString
  );

  const isDeclarativeOnly = action.toolScope?.capabilities?.includes("declarative_only") ?? false;
  const typedReasonAvailable =
    blockReason !== null || proposalReason !== null || failureKind !== null;

  return {
    status: action.status,
    isBlocked:
      action.status === "blocked" ||
      action.status === "needs_confirmation" ||
      action.permissionDecision === "deny" ||
      action.permissionDecision === "ask_every_time",
    isFailed: action.status === "failed" || (action.error !== undefined && action.error !== null),
    isSuccess:
      action.status === "succeeded" || action.status === "completed" || action.status === "success",
    needsConfirmation:
      action.status === "needs_confirmation" || action.permissionDecision === "ask_every_time",
    blockReasonLabel: blockReason ? BLOCK_REASON_LABELS[blockReason] : null,
    proposalReasonLabel: proposalReason ? PROPOSAL_REASON_LABELS[proposalReason] : null,
    failureKindLabel: failureKind ? FAILURE_KIND_LABELS[failureKind] : null,
    agentSpecId,
    proposalId,
    toolName: action.toolScope?.toolName ?? null,
    source: action.toolScope?.source ?? null,
    riskLevel: action.toolScope?.riskLevel ?? null,
    actionType: action.toolScope?.actionType ?? action.actionType ?? null,
    requiresConfirmation: action.toolScope?.requiresConfirmation,
    isAllowed: action.toolScope?.allowed,
    isDeclarativeOnly,
    typedReasonAvailable,
  };
}

// =====================================================================
// NEW: TypedProposalHint — typed proposal information
// =====================================================================

export interface TypedProposalHint {
  /** True only when a typed network_policy_ask field is present in after */
  isNetworkPolicyAsk: boolean;
  /** Tool name extracted from after typed fields */
  toolName: string | null;
  /** The typed proposal_reason value if present */
  proposalReason: ExecutionProposalReason | null;
  /** source from after */
  source: string | null;
}

/**
 * Extract typed governance hints from a proposal's `after` payload.
 * Does NOT inspect `after.reason` (text field) or `proposal.reason` (text field).
 * Only inspects typed boolean/string fields.
 */
export function getTypedProposalHint(proposal: AgentProposal): TypedProposalHint {
  const after = (proposal.after as Record<string, unknown>) ?? {};

  // Only use boolean typed fields, never text inference
  const isNetworkPolicyAsk = after.network_policy_ask === true;

  const proposalReasonRaw = after.proposal_reason;
  let proposalReason: ExecutionProposalReason | null = null;
  if (
    typeof proposalReasonRaw === "string" &&
    (proposalReasonRaw === "network_policy_ask" ||
      proposalReasonRaw === "tool_permission_ask" ||
      proposalReasonRaw === "high_risk_action")
  ) {
    proposalReason = proposalReasonRaw as ExecutionProposalReason;
  }

  const toolName =
    (typeof after.tool_name === "string" ? after.tool_name : null) ??
    (typeof after.toolName === "string" ? after.toolName : null);
  const source = typeof after.source === "string" ? after.source : null;

  return {
    isNetworkPolicyAsk,
    toolName,
    proposalReason,
    source,
  };
}

// =====================================================================
// NEW: TypedToolCallViewModel — unified tool call view model
// =====================================================================

export interface TypedToolCallViewModel {
  blockReasonLabel: string | null;
  proposalReasonLabel: string | null;
  failureKindLabel: string | null;
  agentSpecId: string | null;
  proposalId: string | null;
  typedReasonAvailable: boolean;
}

export function getTypedToolCallViewModel(call: ToolCallResult): TypedToolCallViewModel {
  const output: Record<string, unknown> =
    typeof call.output === "object" && call.output !== null
      ? (call.output as Record<string, unknown>)
      : {};

  let structuredResult: Record<string, unknown> | undefined = undefined;
  if (
    output.structured_result &&
    typeof output.structured_result === "object" &&
    output.structured_result !== null
  ) {
    structuredResult = output.structured_result as Record<string, unknown>;
  }

  const blockReason = firstValidTypedValue(
    output,
    structuredResult,
    "block_reason",
    isValidBlockReason
  );
  const proposalReason = firstValidTypedValue(
    output,
    structuredResult,
    "proposal_reason",
    isValidProposalReason
  );
  const failureKind = firstValidTypedValue(
    output,
    structuredResult,
    "failure_kind",
    isValidFailureKind
  );
  const agentSpecId = firstValidTypedValue(
    output,
    structuredResult,
    "agent_spec_id",
    isValidNonEmptyString
  );
  const proposalId = firstValidTypedValue(
    output,
    structuredResult,
    "proposal_id",
    isValidNonEmptyString
  );

  return {
    blockReasonLabel: blockReason ? BLOCK_REASON_LABELS[blockReason] : null,
    proposalReasonLabel: proposalReason ? PROPOSAL_REASON_LABELS[proposalReason] : null,
    failureKindLabel: failureKind ? FAILURE_KIND_LABELS[failureKind] : null,
    agentSpecId,
    proposalId,
    typedReasonAvailable: blockReason !== null || proposalReason !== null || failureKind !== null,
  };
}

// =====================================================================
// NEW: TypedRunHint — short hint for RunsPage list preview
// =====================================================================

export interface TypedRunHint {
  /** Key for deduplication in React lists */
  key: string;
  /** Short Chinese label suitable for a badge */
  text: string;
  /** Severity drives color */
  severity: TypedSeverity;
}

export function getTypedRunHints(events: AgentRunEvent[]): TypedRunHint[] {
  const hints: TypedRunHint[] = [];
  const seen = new Set<string>();

  for (const event of events) {
    const vm = getTypedRunEventViewModel(event);

    if (vm.typedKind === "replay_failed") {
      const key = `replay-failed-${vm.blockReasonLabel ?? "unknown"}`;
      if (seen.has(key)) continue;
      seen.add(key);
      if (vm.blockReasonLabel) {
        hints.push({ key, text: `重放失败：${vm.blockReasonLabel}`, severity: "error" });
      } else {
        hints.push({ key, text: "重放失败", severity: "error" });
      }
      continue;
    }

    if (vm.typedKind === "tool_call_blocked") {
      if (vm.status === "needs_confirmation") {
        const key = `tool-needsconf-${vm.proposalReasonLabel ?? "unknown"}`;
        if (seen.has(key)) continue;
        seen.add(key);
        if (vm.proposalReasonLabel) {
          hints.push({
            key,
            text: `待确认：${vm.proposalReasonLabel}`,
            severity: "warning",
          });
        } else {
          hints.push({
            key,
            text: `待确认：${vm.toolName ?? "未知工具"}`,
            severity: "warning",
          });
        }
      } else {
        const key = `tool-blocked-${vm.blockReasonLabel ?? "unknown"}`;
        if (seen.has(key)) continue;
        seen.add(key);
        if (vm.blockReasonLabel) {
          hints.push({
            key,
            text: `工具被阻断：${vm.blockReasonLabel}`,
            severity: "error",
          });
        } else {
          hints.push({
            key,
            text: `工具被阻断：${vm.toolName ?? "未知工具"}`,
            severity: "error",
          });
        }
      }
    }
  }

  return hints;
}

// =====================================================================
// NEW: Event-level Explainability — TypedEventExplanationViewModel
// =====================================================================

export interface TypedEventExplanationViewModel {
  title: string;
  tone: "success" | "warning" | "error" | "info";
  whatHappened: string;
  why: string | null;
  impact: string | null;
  nextStep: string | null;
  debugFacts: Array<{ label: string; value: string }>;
}

/**
 * Produce a user/developer-facing explanation for a single AgentRunEvent.
 *
 * Built entirely from typed payload data (via parseTypedEventPayload).
 * NEVER inspects summary/human_message for reason inference.
 * Unknown/malformed events return a safe fallback.
 */
export function getTypedEventExplanation(event: AgentRunEvent): TypedEventExplanationViewModel {
  const parsed = parseTypedEventPayload(event);

  // ── Helper: tool call blocked ──────────────────────────────────────
  function toolCallBlockedExplanation(): TypedEventExplanationViewModel {
    const d = parsed.kind === "tool_call_blocked" ? parsed.data : null;
    if (!d) return unknownFallback();

    const toolLabel = d.tool_name || "未知工具";
    const sourceLabel = d.source || "未知来源";

    if (d.status === "needs_confirmation") {
      const reasonLabel = isValidProposalReason(d.proposal_reason)
        ? PROPOSAL_REASON_LABELS[d.proposal_reason]
        : null;
      const whyText = reasonLabel ? `因为：${reasonLabel}` : "需要用户确认才能继续执行";
      const impactText = `工具 "${toolLabel}" 的执行被暂停，等待你的决定`;
      const nextStepText = `请前往 Review Center 审查并确认相关提案`;
      const debugFacts: Array<{ label: string; value: string }> = [
        { label: "eventType", value: "tool.call_blocked" },
        { label: "status", value: "needs_confirmation" },
        { label: "toolName", value: toolLabel },
        { label: "source", value: sourceLabel },
      ];
      if (d.agent_spec_id) debugFacts.push({ label: "agentSpecId", value: d.agent_spec_id });
      if (d.proposal_id) debugFacts.push({ label: "proposalId", value: d.proposal_id });
      if (d.proposal_reason) debugFacts.push({ label: "proposalReason", value: d.proposal_reason });

      return {
        title: `需要确认：${toolLabel}`,
        tone: "warning",
        whatHappened: `工具 "${toolLabel}" 需要你的确认才能继续执行`,
        why: whyText,
        impact: impactText,
        nextStep: nextStepText,
        debugFacts,
      };
    }

    // status === "blocked"
    const blockLabel = isValidBlockReason(d.block_reason)
      ? BLOCK_REASON_LABELS[d.block_reason]
      : null;
    const whyText = blockLabel ? `因为：${blockLabel}` : "工具调用被系统安全策略阻断";

    let impactText: string;
    let nextStepText: string | null;
    const debugFacts: Array<{ label: string; value: string }> = [
      { label: "eventType", value: "tool.call_blocked" },
      { label: "status", value: "blocked" },
      { label: "toolName", value: toolLabel },
      { label: "source", value: sourceLabel },
    ];

    if (d.block_reason === "agent_spec_denied") {
      impactText = `AgentSpec 阻止了 "${toolLabel}" 的执行`;
      nextStepText = `请调整 AgentSpec 配置以允许该工具，或使用其他方式完成任务`;
      if (d.agent_spec_id) debugFacts.push({ label: "agentSpecId", value: d.agent_spec_id });
    } else if (d.block_reason === "network_policy_denied" || d.block_reason === "domain_blocked") {
      impactText = `网络策略拒绝了 "${toolLabel}" 的网络访问`;
      nextStepText = `请调整网络策略或手动授予访问权限`;
    } else if (d.block_reason === "tool_permission_denied") {
      impactText = `工具权限策略拒绝了 "${toolLabel}" 的执行`;
      nextStepText = `请授予 "${toolLabel}" 的权限`;
    } else if (d.block_reason === "missing_mcp_client") {
      impactText = `缺少 "${toolLabel}" 所需的 MCP 客户端连接`;
      nextStepText = `请检查 MCP 服务器是否已启动并重新注册`;
    } else if (d.block_reason === "disabled_manifest" || d.block_reason === "declarative_only") {
      impactText = `"${toolLabel}" 在当前环境中不可用（声明式或已禁用）`;
      nextStepText = null;
    } else {
      impactText = `工具 "${toolLabel}" 被阻断`;
      nextStepText = `请检查系统日志了解详情`;
    }

    if (d.block_reason) debugFacts.push({ label: "blockReason", value: d.block_reason });
    if (d.agent_spec_id) debugFacts.push({ label: "agentSpecId", value: d.agent_spec_id });
    if (d.target_tool_name) debugFacts.push({ label: "targetToolName", value: d.target_tool_name });
    if (d.target_source) debugFacts.push({ label: "targetSource", value: d.target_source });

    return {
      title: `工具被阻断：${toolLabel}`,
      tone: "error",
      whatHappened: `工具 "${toolLabel}" 被系统安全策略阻断`,
      why: whyText,
      impact: impactText,
      nextStep: nextStepText,
      debugFacts,
    };
  }

  // ── Helper: replay started ──────────────────────────────────────────
  function replayStartedExplanation(): TypedEventExplanationViewModel {
    const d = parsed.kind === "replay_started" ? parsed.data : null;
    if (!d) return unknownFallback();

    return {
      title: `重放开始：${d.tool_name || "未知工具"}`,
      tone: "info",
      whatHappened: `开始重放之前的动作，工具："${d.tool_name || "未知工具"}"`,
      why: "用户或系统触发了动作重放",
      impact: "正在重新执行之前被暂停或被阻断的动作",
      nextStep: "等待重放结果",
      debugFacts: [
        { label: "eventType", value: "replay.started" },
        { label: "status", value: "started" },
        { label: "toolName", value: d.tool_name },
        { label: "source", value: d.source },
        { label: "actionId", value: d.action_id },
        { label: "replayOfActionId", value: d.replay_of_action_id },
        { label: "agentSpecId", value: d.agent_spec_id },
      ],
    };
  }

  // ── Helper: replay completed ────────────────────────────────────────
  function replayCompletedExplanation(): TypedEventExplanationViewModel {
    const d = parsed.kind === "replay_completed" ? parsed.data : null;
    if (!d) return unknownFallback();

    const toolLabel = d.tool_name || "未知工具";
    const debugFacts: Array<{ label: string; value: string }> = [
      { label: "eventType", value: "replay.completed" },
      { label: "status", value: d.status },
      { label: "toolName", value: toolLabel },
      { label: "source", value: d.source },
      { label: "actionId", value: d.action_id },
      { label: "replayOfActionId", value: d.replay_of_action_id },
      { label: "agentSpecId", value: d.agent_spec_id },
    ];

    if (d.status === "completed") {
      return {
        title: `重放成功：${toolLabel}`,
        tone: "success",
        whatHappened: `重放动作 "${toolLabel}" 执行成功`,
        why: null,
        impact: "之前被暂停或被阻断的动作已成功执行",
        nextStep: null,
        debugFacts,
      };
    }

    if (d.status === "blocked") {
      const blockLabel = isValidBlockReason(d.block_reason)
        ? BLOCK_REASON_LABELS[d.block_reason]
        : null;
      const whyText = blockLabel ? `因为：${blockLabel}` : "重放被安全策略拒绝";
      if (d.block_reason) debugFacts.push({ label: "blockReason", value: d.block_reason });
      if (d.failure_kind) debugFacts.push({ label: "failureKind", value: d.failure_kind });

      return {
        title: `重放被阻断：${toolLabel}`,
        tone: "error",
        whatHappened: `重放动作 "${toolLabel}" 被阻断`,
        why: whyText,
        impact: "重放无法完成，请查看原因后调整配置",
        nextStep: "请检查 AgentSpec 配置或工具权限",
        debugFacts,
      };
    }

    // status === "needs_confirmation"
    const proposalLabel = isValidProposalReason(d.proposal_reason)
      ? PROPOSAL_REASON_LABELS[d.proposal_reason]
      : null;
    const whyText = proposalLabel ? `因为：${proposalLabel}` : "重放需要用户确认";
    if (d.proposal_reason) debugFacts.push({ label: "proposalReason", value: d.proposal_reason });

    return {
      title: `重放需确认：${toolLabel}`,
      tone: "warning",
      whatHappened: `重放动作 "${toolLabel}" 需要你的确认`,
      why: whyText,
      impact: "重放被暂停，等待你的决定",
      nextStep: "请前往 Review Center 确认相关提案",
      debugFacts,
    };
  }

  // ── Helper: replay failed ───────────────────────────────────────────
  function replayFailedExplanation(): TypedEventExplanationViewModel {
    const d = parsed.kind === "replay_failed" ? parsed.data : null;
    if (!d) return unknownFallback();

    const toolLabel = d.tool_name || "未知工具";
    const debugFacts: Array<{ label: string; value: string }> = [
      { label: "eventType", value: "replay.failed" },
      { label: "status", value: "failed" },
      { label: "actionId", value: d.action_id },
      { label: "replayOfActionId", value: d.replay_of_action_id },
    ];

    if (d.tool_name) debugFacts.push({ label: "toolName", value: d.tool_name });
    if (d.source) debugFacts.push({ label: "source", value: d.source });
    if (d.agent_spec_id) debugFacts.push({ label: "agentSpecId", value: d.agent_spec_id });

    const blockLabel = isValidBlockReason(d.block_reason)
      ? BLOCK_REASON_LABELS[d.block_reason]
      : null;
    const failureLabel = isValidFailureKind(d.failure_kind)
      ? FAILURE_KIND_LABELS[d.failure_kind]
      : null;
    let whyText: string | null = null;
    let nextStepText: string;

    if (blockLabel) {
      whyText = `因为：${blockLabel}`;
      debugFacts.push({ label: "blockReason", value: d.block_reason! });
      if (d.block_reason === "replay_spec_missing") {
        nextStepText = "该动作缺少可重放的规格信息，请尝试重新创建任务";
      } else if (d.block_reason === "agent_spec_denied") {
        nextStepText = "请调整 AgentSpec 配置以允许重放该工具";
      } else {
        nextStepText = "请检查系统配置并重试";
      }
    } else if (failureLabel) {
      whyText = `因为：${failureLabel}`;
      debugFacts.push({ label: "failureKind", value: d.failure_kind! });
      if (d.failure_kind === "missing_mcp_server") {
        nextStepText = "请检查 MCP 服务器是否启动并重新注册";
      } else if (d.failure_kind === "mcp_client_error") {
        nextStepText = "MCP 客户端出错，请检查服务器日志";
      } else {
        nextStepText = "请查看详细 trace 进行诊断";
      }
    } else {
      nextStepText = "请查看详细 trace 进行诊断";
    }

    const whatHappened = `重放动作${toolLabel ? ` "${toolLabel}"` : ""}失败`;

    return {
      title: `重放失败：${toolLabel}`,
      tone: "error",
      whatHappened,
      why: whyText,
      impact: "之前的操作未能被重放，可能需要手动处理",
      nextStep: nextStepText,
      debugFacts,
    };
  }

  // ── Helper: unknown/malformed fallback ──────────────────────────────
  function unknownFallback(): TypedEventExplanationViewModel {
    return {
      title: "系统事件",
      tone: "info",
      whatHappened: "这是一个未识别的运行事件",
      why: null,
      impact: null,
      nextStep: null,
      debugFacts: [
        { label: "eventType", value: event.eventType },
        { label: "summary", value: event.summary },
      ],
    };
  }

  // ── Dispatch per parsed kind ────────────────────────────────────────
  switch (parsed.kind) {
    case "tool_call_blocked":
      return toolCallBlockedExplanation();
    case "replay_started":
      return replayStartedExplanation();
    case "replay_completed":
      return replayCompletedExplanation();
    case "replay_failed":
      return replayFailedExplanation();
    default:
      return unknownFallback();
  }
}

// =====================================================================
// NEW: Run-level Explainability — TypedRunExplanationViewModel
// =====================================================================

export interface TypedRunExplanationViewModel {
  headline: string;
  outcomeTone: "success" | "warning" | "error" | "info";
  primaryReason: string | null;
  agentSpecId: string | null;
  /** Number of prompt blocks assembled (from prompt_blocks array in payload). null = event not present. */
  promptBlockCount: number | null;
  /** First few prompt block IDs for display. */
  promptBlockIds: string[];
  contextPolicy: string | null;
  toolSummary: {
    started: number;
    completed: number;
    blocked: number;
    failed: number;
    needsConfirmation: number;
  };
  replaySummary: {
    started: number;
    completed: number;
    failed: number;
    blocked: number;
    needsConfirmation: number;
  };
  /**
   * Suggested user actions derived from typed governance reasons.
   * Empty array = no user action needed (success / info path).
   * Non-empty only for error / warning outcomes.
   */
  nextActions: Array<{
    kind:
      | "review_proposal"
      | "grant_permission"
      | "adjust_agent_spec"
      | "retry_replay"
      | "inspect_trace";
    label: string;
    severity: "warning" | "error";
  }>;
  userFacingBullets: string[];
  developerBullets: string[];
}

/**
 * Build a run-level structured explanation from the event timeline.
 *
 * All governance information comes from typed payload fields only.
 * Never inspects summary/human_message/error text for state inference.
 *
 * @param events AgentRunEvent timeline for the run
 * @param run Optional AgentRun for run-level metadata (status, kind, etc.)
 */
/**
 * Event types that carry typed governance payloads (tool.call_blocked,
 * replay.*). When one of these fails structural validation, it is a
 * trace-contract warning — not a clean success.
 */
const KNOWN_TYPED_EVENT_TYPES = new Set([
  "tool.call_blocked",
  "replay.started",
  "replay.completed",
  "replay.failed",
]);

export function getTypedRunExplanation(
  events: AgentRunEvent[],
  run?: { status: string; kind: string; outputPreview?: string }
): TypedRunExplanationViewModel {
  const toolSummary = { started: 0, completed: 0, blocked: 0, failed: 0, needsConfirmation: 0 };
  const replaySummary = { started: 0, completed: 0, blocked: 0, failed: 0, needsConfirmation: 0 };
  const seenBlockReasons = new Set<ExecutionBlockReason>();
  const seenProposalReasons = new Set<ExecutionProposalReason>();
  const seenFailureKinds = new Set<ExecutionFailureKind>();
  let agentSpecId: string | null = null;
  let promptBlockCount: number | null = null;
  const promptBlockIds: string[] = [];
  let contextPolicy: string | null = null;
  let hasReplayFailed = false;
  let hasAgentSpecDenied = false;
  let hasNeedsConfirmation = false;
  let allSucceeded = true;
  let malformedKnownTyped = 0;
  let hasGenericFailure = false;

  for (const event of events) {
    const parsed = parseTypedEventPayload(event);

    // Detect malformed known typed events: eventType is a governance type
    // but the payload fails structural validation → parsed.kind === "unknown"
    if (KNOWN_TYPED_EVENT_TYPES.has(event.eventType) && parsed.kind === "unknown") {
      malformedKnownTyped++;
    }

    // Track run-level metadata from non-typed events
    // ⚠️ Backend emits snake_case payloads. We read snake_case first,
    //    then fall back to camelCase for legacy test data compatibility.
    if (event.eventType === "agent_spec.selected") {
      const p = event.payload as Record<string, unknown>;
      // agent_spec_id (only first event wins)
      if (!agentSpecId) {
        if (typeof p.agent_spec_id === "string") agentSpecId = p.agent_spec_id;
        else if (typeof p.agentSpecId === "string") agentSpecId = p.agentSpecId;
      }
      // privacy_policy as contextPolicy fallback (lower priority than context_governance.applied)
      if (!contextPolicy) {
        if (typeof p.privacy_policy === "string") contextPolicy = p.privacy_policy;
        else if (typeof p.privacyPolicy === "string") contextPolicy = p.privacyPolicy;
      }
    }
    if (event.eventType === "prompt_stack.assembled") {
      const p = event.payload as Record<string, unknown>;
      // agent_spec_id from prompt_stack.assembled
      if (!agentSpecId) {
        if (typeof p.agent_spec_id === "string") agentSpecId = p.agent_spec_id;
        // Fallback: camelCase (legacy test data)
        else if (typeof p.agentSpecId === "string") agentSpecId = p.agentSpecId;
      }
      // prompt_blocks array — extract count and IDs
      if (promptBlockCount === null) {
        const blocks = p.prompt_blocks ?? p.promptBlocks;
        if (Array.isArray(blocks)) {
          promptBlockCount = blocks.length;
          for (const block of blocks) {
            if (block && typeof block === "object" && typeof (block as any).id === "string") {
              promptBlockIds.push((block as any).id as string);
            }
          }
        }
      }
    }
    if (event.eventType === "context_governance.applied") {
      const p = event.payload as Record<string, unknown>;
      // context_governance.applied has highest priority for contextPolicy.
      // Override any fallback from agent_spec.selected.
      // Primary: snake_case (real backend). orchestrator.rs uses agent_spec_privacy_policy,
      // execution.rs + streaming.rs use privacy_policy. Accept both.
      if (typeof p.privacy_policy === "string") contextPolicy = p.privacy_policy;
      else if (typeof p.agent_spec_privacy_policy === "string")
        contextPolicy = p.agent_spec_privacy_policy;
      // Fallback: camelCase (legacy test data)
      else if (typeof p.privacyPolicy === "string") contextPolicy = p.privacyPolicy;
    }

    // Count tool calls from typed events
    if (event.eventType === "tool.call_started") {
      toolSummary.started++;
    }
    if (event.eventType === "tool.call_completed") {
      toolSummary.completed++;
    }
    if (event.eventType === "tool.call_failed") {
      toolSummary.failed++;
      allSucceeded = false;
      hasGenericFailure = true;
    }

    // Typed tool.call_blocked
    if (parsed.kind === "tool_call_blocked") {
      const d = parsed.data;
      if (d.status === "needs_confirmation") {
        toolSummary.needsConfirmation++;
        hasNeedsConfirmation = true;
        if (isValidProposalReason(d.proposal_reason)) seenProposalReasons.add(d.proposal_reason);
      } else {
        toolSummary.blocked++;
        if (isValidBlockReason(d.block_reason)) {
          seenBlockReasons.add(d.block_reason);
          if (d.block_reason === "agent_spec_denied" || d.block_reason === "agent_spec_missing")
            hasAgentSpecDenied = true;
        }
      }
      if (d.agent_spec_id && !agentSpecId) agentSpecId = d.agent_spec_id;
      allSucceeded = false;
    }

    // Typed replay events
    if (parsed.kind === "replay_started") {
      replaySummary.started++;
      if (parsed.data.agent_spec_id && !agentSpecId) agentSpecId = parsed.data.agent_spec_id;
    }
    if (parsed.kind === "replay_completed") {
      const d = parsed.data;
      if (d.status === "completed") replaySummary.completed++;
      else if (d.status === "blocked") {
        replaySummary.blocked++;
        if (isValidBlockReason(d.block_reason)) seenBlockReasons.add(d.block_reason);
        if (isValidFailureKind(d.failure_kind)) seenFailureKinds.add(d.failure_kind);
        allSucceeded = false;
      } else if (d.status === "needs_confirmation") {
        replaySummary.needsConfirmation++;
        if (isValidProposalReason(d.proposal_reason)) seenProposalReasons.add(d.proposal_reason);
        hasNeedsConfirmation = true;
        allSucceeded = false;
      }
    }
    if (parsed.kind === "replay_failed") {
      replaySummary.failed++;
      hasReplayFailed = true;
      if (isValidBlockReason(parsed.data.block_reason))
        seenBlockReasons.add(parsed.data.block_reason);
      if (isValidFailureKind(parsed.data.failure_kind))
        seenFailureKinds.add(parsed.data.failure_kind);
      allSucceeded = false;
    }

    // run.failed / model.failed / model.call_failed
    if (
      event.eventType === "run.failed" ||
      event.eventType === "model.failed" ||
      event.eventType === "model.call_failed"
    ) {
      allSucceeded = false;
      hasGenericFailure = true;
    }
  }

  // ── Build nextActions from typed reasons ────────────────────────────
  const nextActions: TypedRunExplanationViewModel["nextActions"] = [];

  // needs_confirmation → review/grant
  if (hasNeedsConfirmation) {
    nextActions.push({
      kind: "review_proposal",
      label: "前往 Review Center 审查待确认提案",
      severity: "warning",
    });
    nextActions.push({
      kind: "grant_permission",
      label: "授予所需工具权限",
      severity: "warning",
    });
  }

  // agent_spec_denied → adjust_agent_spec
  if (hasAgentSpecDenied) {
    nextActions.push({
      kind: "adjust_agent_spec",
      label: "调整 AgentSpec 配置以允许被拒绝的工具",
      severity: "error",
    });
  }

  // replay.failed → retry_replay or inspect_trace
  if (hasReplayFailed) {
    nextActions.push({
      kind: "retry_replay",
      label: "重放失败，检查原因后重试",
      severity: "error",
    });
    nextActions.push({
      kind: "inspect_trace",
      label: "查看详细 trace 进行诊断",
      severity: "error",
    });
  }

  // ═══ Fallback: error/warning with no typed-reason nextActions ═══════
  // Generic failures (tool.call_failed, run.failed, model.failed) and
  // malformed known typed events are not covered by the typed-reason
  // nextActions above.  When the outcome will be error/warning but
  // nextActions is empty, inject inspect_trace so users always have a
  // next step.
  //
  // This does NOT add actions for success/info runs.

  // ── Build outcome tone ──────────────────────────────────────────────
  let outcomeTone: TypedRunExplanationViewModel["outcomeTone"];
  if (hasReplayFailed || hasAgentSpecDenied || toolSummary.failed > 0 || hasGenericFailure) {
    outcomeTone = "error";
  } else if (hasNeedsConfirmation) {
    outcomeTone = "warning";
  } else if (malformedKnownTyped > 0) {
    // Known governance event types with unparseable payloads → trace
    // contract warning.  Does not crash; does not infer from summary.
    outcomeTone = "warning";
  } else if (allSucceeded && events.some(e => e.eventType === "run.completed")) {
    outcomeTone = "success";
  } else {
    outcomeTone = "info";
  }

  // ── Fallback inspect_trace for any non-success/no nextActions ───────
  if (nextActions.length === 0 && outcomeTone !== "success" && outcomeTone !== "info") {
    nextActions.push({
      kind: "inspect_trace",
      label: "查看详细 trace 进行诊断",
      severity: outcomeTone === "error" ? "error" : "warning",
    });
  }

  // ── Build primaryReason from typed reasons ──────────────────────────
  let primaryReason: string | null = null;
  if (hasAgentSpecDenied) {
    primaryReason = "AgentSpec 拒绝了工具执行";
  } else if (hasReplayFailed) {
    primaryReason = "重放动作失败";
  } else if (hasNeedsConfirmation) {
    primaryReason = "工具需要用户确认";
  } else if (seenBlockReasons.size > 0) {
    primaryReason = `工具被阻断：${Array.from(seenBlockReasons)
      .map(r => BLOCK_REASON_LABELS[r])
      .join("、")}`;
  } else if (hasGenericFailure) {
    primaryReason = "运行中出现未分类错误";
  } else if (malformedKnownTyped > 0) {
    primaryReason = "运行 trace 中存在无法解析的治理事件";
  }

  // ── Build headline ──────────────────────────────────────────────────
  const kindLabel = run?.kind ? run.kind : "AgentRun";
  let headline: string;
  if (outcomeTone === "success") {
    headline = `${kindLabel} 运行完成，所有工具执行成功`;
  } else if (outcomeTone === "error") {
    headline = `${kindLabel} 运行遇到问题`;
  } else if (outcomeTone === "warning") {
    headline = `${kindLabel} 运行需要你的确认`;
  } else {
    headline = `${kindLabel} 运行记录`;
  }

  // ── Build userFacingBullets ─────────────────────────────────────────
  const userFacingBullets: string[] = [];
  if (toolSummary.started > 0) userFacingBullets.push(`发起了 ${toolSummary.started} 次工具调用`);
  if (toolSummary.completed > 0) userFacingBullets.push(`${toolSummary.completed} 次工具调用成功`);
  if (toolSummary.blocked > 0) userFacingBullets.push(`${toolSummary.blocked} 次工具调用被阻断`);
  if (toolSummary.failed > 0) userFacingBullets.push(`${toolSummary.failed} 次工具调用失败`);
  if (toolSummary.needsConfirmation > 0)
    userFacingBullets.push(`${toolSummary.needsConfirmation} 次工具调用需要确认`);
  if (replaySummary.started > 0) userFacingBullets.push(`发起了 ${replaySummary.started} 次重放`);
  if (replaySummary.completed > 0) userFacingBullets.push(`${replaySummary.completed} 次重放成功`);
  if (replaySummary.failed > 0) userFacingBullets.push(`${replaySummary.failed} 次重放失败`);
  if (replaySummary.blocked > 0) userFacingBullets.push(`${replaySummary.blocked} 次重放被阻断`);
  if (replaySummary.needsConfirmation > 0)
    userFacingBullets.push(`${replaySummary.needsConfirmation} 次重放需确认`);

  if (userFacingBullets.length === 0) {
    userFacingBullets.push("运行已完成，无显著事件");
  }

  // ── Build developerBullets ──────────────────────────────────────────
  const developerBullets: string[] = [];
  if (agentSpecId) developerBullets.push(`AgentSpec: ${agentSpecId}`);
  if (promptBlockCount !== null) {
    const idsPreview =
      promptBlockIds.length > 0
        ? ` (${promptBlockIds.slice(0, 5).join(", ")}${promptBlockIds.length > 5 ? "…" : ""})`
        : "";
    developerBullets.push(`Prompt blocks: ${promptBlockCount} blocks${idsPreview}`);
  }
  if (contextPolicy) developerBullets.push(`隐私策略: ${contextPolicy}`);
  if (seenBlockReasons.size > 0)
    developerBullets.push(
      `阻断原因: ${Array.from(seenBlockReasons)
        .map(r => BLOCK_REASON_LABELS[r])
        .join("、")}`
    );
  if (seenProposalReasons.size > 0)
    developerBullets.push(
      `需确认原因: ${Array.from(seenProposalReasons)
        .map(r => PROPOSAL_REASON_LABELS[r])
        .join("、")}`
    );
  if (seenFailureKinds.size > 0)
    developerBullets.push(
      `失败类型: ${Array.from(seenFailureKinds)
        .map(f => FAILURE_KIND_LABELS[f])
        .join("、")}`
    );
  if (hasReplayFailed) developerBullets.push("存在重放失败事件");
  if (hasGenericFailure) developerBullets.push("存在通用失败事件 (run/model/call_failed)");
  if (malformedKnownTyped > 0) developerBullets.push(`无法解析的治理事件: ${malformedKnownTyped}`);
  developerBullets.push(`事件总数: ${events.length}`);

  return {
    headline,
    outcomeTone,
    primaryReason,
    agentSpecId,
    promptBlockCount,
    promptBlockIds,
    contextPolicy,
    toolSummary,
    replaySummary,
    nextActions,
    userFacingBullets,
    developerBullets,
  };
}

// =====================================================================
// Helpers
// =====================================================================

function getEventLabel(eventType: string): string {
  const labels: Record<string, string> = {
    "run.created": "Run 创建",
    "run.completed": "Run 完成",
    "run.failed": "Run 失败",
    "tool.call_started": "工具调用开始",
    "tool.call_blocked": "工具调用被阻断",
    "tool.call_completed": "工具调用完成",
    "tool.call_failed": "工具调用失败",
    "replay.started": "重放开始",
    "replay.completed": "重放完成",
    "replay.failed": "重放失败",
    "model.call_started": "模型调用开始",
    "model.call_completed": "模型调用完成",
    "model.call_failed": "模型调用失败",
    "model.failed": "模型失败",
    "proposal.created": "Proposal 创建",
    "shell.blocked": "Shell 被阻断",
    "shell.completed": "Shell 完成",
    "plan.created": "Plan 创建",
    "plan.execution_started": "Plan 执行开始",
    "plan.execution_completed": "Plan 执行完成",
    "plan.execution_failed": "Plan 执行失败",
    "compaction.created": "压缩完成",
  };
  return labels[eventType] ?? eventType;
}

/**
 * Return true if and only if the named typed field is present (not null) in the payload.
 * NOT based on error text, summary, or reason string matching.
 */
export function hasTypedField(payload: Record<string, unknown>, field: string): boolean {
  const v = payload[field];
  return v !== null && v !== undefined && (typeof v === "string" ? v.length > 0 : true);
}

// =====================================================================
// TypedBadge — structured badge view model for UI consumption
// =====================================================================

export interface TypedBadge {
  kind: "block_reason" | "proposal_reason" | "failure_kind";
  label: string;
  severity: "error" | "warning" | "info";
  rawReason: string;
}

// =====================================================================
// Display helpers — single source of truth for label/severity
// =====================================================================

export function getBlockReasonDisplay(reason: unknown): TypedBadge | null {
  if (!isValidBlockReason(reason)) return null;
  return {
    kind: "block_reason",
    label: BLOCK_REASON_LABELS[reason],
    severity: blockReasonSeverity(reason) as "error" | "warning" | "info",
    rawReason: reason,
  };
}

export function getProposalReasonDisplay(reason: unknown): TypedBadge | null {
  if (!isValidProposalReason(reason)) return null;
  return {
    kind: "proposal_reason",
    label: PROPOSAL_REASON_LABELS[reason],
    severity: "warning",
    rawReason: reason,
  };
}

export function getFailureKindDisplay(kind: unknown): TypedBadge | null {
  if (!isValidFailureKind(kind)) return null;
  return {
    kind: "failure_kind",
    label: FAILURE_KIND_LABELS[kind],
    severity: "error",
    rawReason: kind,
  };
}

/**
 * Extract TypedBadge list from an AgentRunEvent.
 * Returns [] for events with no valid typed reasons.
 * Does NOT inspect summary/human_message/error text.
 */
export function getTypedReasonBadgesFromEvent(event: AgentRunEvent): TypedBadge[] {
  const parsed = parseTypedEventPayload(event);
  const badges: TypedBadge[] = [];

  if (parsed.kind === "tool_call_blocked") {
    const d = parsed.data;
    const block = getBlockReasonDisplay(d.block_reason);
    if (block) badges.push(block);
    const proposal = getProposalReasonDisplay(d.proposal_reason);
    if (proposal) badges.push(proposal);
  } else if (parsed.kind === "replay_completed") {
    const d = parsed.data;
    const block = getBlockReasonDisplay(d.block_reason);
    if (block) badges.push(block);
    const proposal = getProposalReasonDisplay(d.proposal_reason);
    if (proposal) badges.push(proposal);
    const failure = getFailureKindDisplay(d.failure_kind);
    if (failure) badges.push(failure);
  } else if (parsed.kind === "replay_failed") {
    const d = parsed.data;
    const block = getBlockReasonDisplay(d.block_reason);
    if (block) badges.push(block);
    const failure = getFailureKindDisplay(d.failure_kind);
    if (failure) badges.push(failure);
  }

  return badges;
}

/**
 * Extract TypedBadge list from an AgentAction.
 * Uses firstValidTypedValue with structured_result priority.
 * Does NOT inspect error text.
 */
export function getTypedReasonBadgesFromAction(action: {
  id: string;
  status: string;
  error?: string;
  output?: string | Record<string, unknown>;
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
}): TypedBadge[] {
  const vm = getTypedActionViewModel(action as AgentAction);
  const badges: TypedBadge[] = [];

  if (vm.blockReasonLabel) {
    const output: Record<string, unknown> =
      typeof action.output === "object" && action.output !== null
        ? (action.output as Record<string, unknown>)
        : {};
    let structuredResult: Record<string, unknown> | undefined = undefined;
    if (
      output.structured_result &&
      typeof output.structured_result === "object" &&
      output.structured_result !== null
    ) {
      structuredResult = output.structured_result as Record<string, unknown>;
    }
    const rawBlockReason = firstValidTypedValue(
      output,
      structuredResult,
      "block_reason",
      isValidBlockReason
    );
    const block = getBlockReasonDisplay(rawBlockReason);
    if (block) badges.push(block);
  }

  if (vm.proposalReasonLabel) {
    const output: Record<string, unknown> =
      typeof action.output === "object" && action.output !== null
        ? (action.output as Record<string, unknown>)
        : {};
    let structuredResult: Record<string, unknown> | undefined = undefined;
    if (
      output.structured_result &&
      typeof output.structured_result === "object" &&
      output.structured_result !== null
    ) {
      structuredResult = output.structured_result as Record<string, unknown>;
    }
    const rawProposalReason = firstValidTypedValue(
      output,
      structuredResult,
      "proposal_reason",
      isValidProposalReason
    );
    const proposal = getProposalReasonDisplay(rawProposalReason);
    if (proposal) badges.push(proposal);
  }

  if (vm.failureKindLabel) {
    const output: Record<string, unknown> =
      typeof action.output === "object" && action.output !== null
        ? (action.output as Record<string, unknown>)
        : {};
    let structuredResult: Record<string, unknown> | undefined = undefined;
    if (
      output.structured_result &&
      typeof output.structured_result === "object" &&
      output.structured_result !== null
    ) {
      structuredResult = output.structured_result as Record<string, unknown>;
    }
    const rawFailureKind = firstValidTypedValue(
      output,
      structuredResult,
      "failure_kind",
      isValidFailureKind
    );
    const failure = getFailureKindDisplay(rawFailureKind);
    if (failure) badges.push(failure);
  }

  return badges;
}

/**
 * Extract TypedBadge list from a ToolCallResult.
 * Uses firstValidTypedValue with structured_result priority.
 * Does NOT inspect error text.
 */
export function getTypedReasonBadgesFromToolCall(call: {
  name: string;
  arguments?: Record<string, unknown>;
  success?: boolean;
  status?: string;
  error?: string;
  output?: string | Record<string, unknown>;
  permission_decision?: string;
  permission_level?: string;
}): TypedBadge[] {
  const vm = getTypedToolCallViewModel(call as any);
  const badges: TypedBadge[] = [];

  if (vm.blockReasonLabel) {
    const output: Record<string, unknown> =
      typeof call.output === "object" && call.output !== null
        ? (call.output as Record<string, unknown>)
        : {};
    let structuredResult: Record<string, unknown> | undefined = undefined;
    if (
      output.structured_result &&
      typeof output.structured_result === "object" &&
      output.structured_result !== null
    ) {
      structuredResult = output.structured_result as Record<string, unknown>;
    }
    const rawBlockReason = firstValidTypedValue(
      output,
      structuredResult,
      "block_reason",
      isValidBlockReason
    );
    const block = getBlockReasonDisplay(rawBlockReason);
    if (block) badges.push(block);
  }

  if (vm.proposalReasonLabel) {
    const output: Record<string, unknown> =
      typeof call.output === "object" && call.output !== null
        ? (call.output as Record<string, unknown>)
        : {};
    let structuredResult: Record<string, unknown> | undefined = undefined;
    if (
      output.structured_result &&
      typeof output.structured_result === "object" &&
      output.structured_result !== null
    ) {
      structuredResult = output.structured_result as Record<string, unknown>;
    }
    const rawProposalReason = firstValidTypedValue(
      output,
      structuredResult,
      "proposal_reason",
      isValidProposalReason
    );
    const proposal = getProposalReasonDisplay(rawProposalReason);
    if (proposal) badges.push(proposal);
  }

  if (vm.failureKindLabel) {
    const output: Record<string, unknown> =
      typeof call.output === "object" && call.output !== null
        ? (call.output as Record<string, unknown>)
        : {};
    let structuredResult: Record<string, unknown> | undefined = undefined;
    if (
      output.structured_result &&
      typeof output.structured_result === "object" &&
      output.structured_result !== null
    ) {
      structuredResult = output.structured_result as Record<string, unknown>;
    }
    const rawFailureKind = firstValidTypedValue(
      output,
      structuredResult,
      "failure_kind",
      isValidFailureKind
    );
    const failure = getFailureKindDisplay(rawFailureKind);
    if (failure) badges.push(failure);
  }

  return badges;
}

/**
 * Construct typed outcome labels from an extractTypedActionOutcome result.
 * Returns only the labels from typedContract — never inspects raw reason strings.
 */
export function getTypedOutcomeLabels(outcome: ReturnType<typeof extractTypedActionOutcome>): {
  blockReasonLabel: string | null;
  proposalReasonLabel: string | null;
  failureKindLabel: string | null;
} {
  const blockDisplay = getBlockReasonDisplay(outcome.blockReason);
  const proposalDisplay = getProposalReasonDisplay(outcome.proposalReason);
  const failureDisplay = getFailureKindDisplay(outcome.failureKind);
  return {
    blockReasonLabel: blockDisplay?.label ?? null,
    proposalReasonLabel: proposalDisplay?.label ?? null,
    failureKindLabel: failureDisplay?.label ?? null,
  };
}

// =====================================================================
// TypedEventDetailViewModel — unified event detail for RunTracePanel
// =====================================================================

export interface TypedEventDetailViewModel {
  kind: "tool_call_blocked" | "replay_started" | "replay_completed" | "replay_failed" | "unknown";
  title: string;
  titleIconTone: "info" | "success" | "warning" | "error";
  statusLabel: string | null;
  statusTone: "info" | "success" | "warning" | "error" | null;
  toolName: string | null;
  source: string | null;
  agentSpecId: string | null;
  proposalId: string | null;
  actionId: string | null;
  replayOfActionId: string | null;
  targetToolName: string | null;
  targetSource: string | null;
  wrapperToolName: string | null;
  humanMessage: string | null;
  badges: TypedBadge[];
}

/**
 * Produce a unified event detail view model from an AgentRunEvent.
 *
 * This is the single function RunTracePanel should use for typed event detail
 * rendering. It calls parseTypedEventPayload internally and assembles all
 * display fields (titles, status labels, badges, meta fields).
 *
 * RunTracePanel MUST NOT call parseTypedEventPayload directly.
 * RunTracePanel MUST NOT access reason fields from typed payloads.
 */
export function getTypedEventDetailViewModel(event: AgentRunEvent): TypedEventDetailViewModel {
  const parsed = parseTypedEventPayload(event);
  const badges = getTypedReasonBadgesFromEvent(event);

  function blockVm(): TypedEventDetailViewModel {
    const d = parsed.kind === "tool_call_blocked" ? parsed.data : null;
    if (!d) return unknownVm();
    const isConfirmation = d.status === "needs_confirmation";
    return {
      kind: "tool_call_blocked",
      title: isConfirmation ? "需确认详情 (Typed Contract)" : "阻断详情 (Typed Contract)",
      titleIconTone: isConfirmation ? "warning" : "error",
      statusLabel: isConfirmation ? "需确认" : "已阻断",
      statusTone: isConfirmation ? "warning" : "error",
      toolName: d.tool_name ?? null,
      source: d.source ?? null,
      agentSpecId: d.agent_spec_id ?? null,
      proposalId: d.proposal_id ?? null,
      actionId: null,
      replayOfActionId: null,
      targetToolName: d.target_tool_name ?? null,
      targetSource: d.target_source ?? null,
      wrapperToolName: d.wrapper_tool_name ?? null,
      humanMessage: d.human_message ?? null,
      badges,
    };
  }

  function replayStartVm(): TypedEventDetailViewModel {
    const d = parsed.kind === "replay_started" ? parsed.data : null;
    if (!d) return unknownVm();
    return {
      kind: "replay_started",
      title: "重放开始 (Typed Contract)",
      titleIconTone: "info",
      statusLabel: "started",
      statusTone: "info",
      toolName: d.tool_name ?? null,
      source: d.source ?? null,
      agentSpecId: d.agent_spec_id ?? null,
      proposalId: null,
      actionId: d.action_id ?? null,
      replayOfActionId: d.replay_of_action_id ?? null,
      targetToolName: null,
      targetSource: null,
      wrapperToolName: null,
      humanMessage: null,
      badges,
    };
  }

  function replayCompleteVm(): TypedEventDetailViewModel {
    const d = parsed.kind === "replay_completed" ? parsed.data : null;
    if (!d) return unknownVm();
    const status = d.status;
    return {
      kind: "replay_completed",
      title: "重放完成 (Typed Contract)",
      titleIconTone:
        status === "completed" ? "success" : status === "blocked" ? "error" : "warning",
      statusLabel: status === "completed" ? "成功" : status === "blocked" ? "已阻断" : "需确认",
      statusTone: status === "completed" ? "success" : status === "blocked" ? "error" : "warning",
      toolName: d.tool_name ?? null,
      source: d.source ?? null,
      agentSpecId: d.agent_spec_id ?? null,
      proposalId: null,
      actionId: d.action_id ?? null,
      replayOfActionId: d.replay_of_action_id ?? null,
      targetToolName: null,
      targetSource: null,
      wrapperToolName: null,
      humanMessage: null,
      badges,
    };
  }

  function replayFailVm(): TypedEventDetailViewModel {
    const d = parsed.kind === "replay_failed" ? parsed.data : null;
    if (!d) return unknownVm();
    return {
      kind: "replay_failed",
      title: "重放失败 (Typed Contract)",
      titleIconTone: "error",
      statusLabel: "failed",
      statusTone: "error",
      toolName: d.tool_name ?? null,
      source: d.source ?? null,
      agentSpecId: d.agent_spec_id ?? null,
      proposalId: null,
      actionId: d.action_id ?? null,
      replayOfActionId: d.replay_of_action_id ?? null,
      targetToolName: null,
      targetSource: null,
      wrapperToolName: null,
      humanMessage: d.human_message ?? null,
      badges,
    };
  }

  function unknownVm(): TypedEventDetailViewModel {
    return {
      kind: "unknown",
      title: "",
      titleIconTone: "info",
      statusLabel: null,
      statusTone: null,
      toolName: null,
      source: null,
      agentSpecId: null,
      proposalId: null,
      actionId: null,
      replayOfActionId: null,
      targetToolName: null,
      targetSource: null,
      wrapperToolName: null,
      humanMessage: null,
      badges: [],
    };
  }

  switch (parsed.kind) {
    case "tool_call_blocked":
      return blockVm();
    case "replay_started":
      return replayStartVm();
    case "replay_completed":
      return replayCompleteVm();
    case "replay_failed":
      return replayFailVm();
    default:
      return unknownVm();
  }
}
