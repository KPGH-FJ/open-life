import type { AgentRun } from "../tauri";

export interface PlanExecuteProductTrace {
  runtimeStrategyTraceKind?: string;
  scenarioId?: string;
  planSessionId?: string;
  strategyKind?: string;
  selectedStrategyKind?: string;
  payloadKind?: string;
  strategyDescriptorId?: string;
  strategyCapabilityIds?: string[];
  selectionReasonCode?: string;
  governanceDecisionKind?: string;
  registryReady?: boolean;
  defaultChatUnchanged?: boolean;
  sideEffectBudget?: Record<string, number>;
  status?: string;
  sourceAgentRunId?: string | null;
  sourceChatSessionId?: string | null;
  stepCount?: number;
  stepStatusCounts?: {
    planned?: number;
    executed?: number;
    requiresProposal?: number;
    blocked?: number;
  };
  generatedProposalIds?: string[];
  generatedProposalCount?: number;
  governanceDecisionCounts?: {
    allow?: number;
    requireProposal?: number;
    block?: number;
  };
  warningCount?: number;
  metadataSafe?: boolean;
  directLifeModelWrites?: boolean;
  externalWritesExecuted?: boolean;
}

function isRecord(value: unknown): value is Record<string, any> {
  return !!value && typeof value === "object" && !Array.isArray(value);
}

function stringValue(value: unknown): string | undefined {
  return typeof value === "string" && value.trim() ? value : undefined;
}

function nullableStringValue(value: unknown): string | null | undefined {
  if (value === null) return null;
  return stringValue(value);
}

function numberValue(value: unknown): number | undefined {
  return typeof value === "number" && Number.isFinite(value) ? value : undefined;
}

function booleanValue(value: unknown): boolean | undefined {
  return typeof value === "boolean" ? value : undefined;
}

function stringArray(value: unknown): string[] {
  return Array.isArray(value)
    ? value.filter((item): item is string => typeof item === "string")
    : [];
}

function rawStrategyResult(run?: AgentRun | null): Record<string, any> | null {
  const trace = run?.reasoningTrace as any;
  const raw = trace?.strategy_result ?? trace?.strategyResult;
  if (!isRecord(raw)) return null;
  const nested = raw.planExecuteProduct ?? raw.plan_execute_product;
  return isRecord(nested) ? nested : raw;
}

export function getPlanExecuteProductTrace(run?: AgentRun | null): PlanExecuteProductTrace | null {
  const raw = rawStrategyResult(run);
  if (!raw) return null;

  const isProductTrace =
    raw.planExecuteProductVertical === true ||
    raw.plan_execute_product_vertical === true ||
    (run?.reasoningStrategy === "plan_execute_product" &&
      (raw.strategyKind === "plan_execute" ||
        !!raw.planSessionId ||
        raw.scenarioId === "weekly_planning"));
  if (!isProductTrace) return null;

  const stepStatusCounts = isRecord(raw.stepStatusCounts) ? raw.stepStatusCounts : {};
  const governanceDecisionCounts = isRecord(raw.governanceDecisionCounts)
    ? raw.governanceDecisionCounts
    : {};
  const proposalIds = stringArray(raw.generatedProposalIds ?? raw.generated_proposal_ids);

  return {
    scenarioId: stringValue(raw.scenarioId ?? raw.scenario_id),
    planSessionId: stringValue(raw.planSessionId ?? raw.plan_session_id),
    runtimeStrategyTraceKind: stringValue(
      raw.runtimeStrategyTraceKind ?? raw.runtime_strategy_trace_kind
    ),
    strategyKind: stringValue(raw.strategyKind ?? raw.strategy_kind),
    selectedStrategyKind: stringValue(raw.selectedStrategyKind ?? raw.selected_strategy_kind),
    payloadKind: stringValue(raw.payloadKind ?? raw.payload_kind),
    strategyDescriptorId: stringValue(raw.strategyDescriptorId ?? raw.strategy_descriptor_id),
    strategyCapabilityIds: stringArray(raw.strategyCapabilityIds ?? raw.strategy_capability_ids),
    selectionReasonCode: stringValue(raw.selectionReasonCode ?? raw.selection_reason_code),
    governanceDecisionKind: stringValue(raw.governanceDecisionKind ?? raw.governance_decision_kind),
    registryReady: booleanValue(raw.registryReady ?? raw.registry_ready),
    defaultChatUnchanged: booleanValue(raw.defaultChatUnchanged ?? raw.default_chat_unchanged),
    sideEffectBudget: isRecord(raw.sideEffectBudget ?? raw.side_effect_budget)
      ? (raw.sideEffectBudget ?? raw.side_effect_budget)
      : undefined,
    status: stringValue(raw.status),
    sourceAgentRunId: nullableStringValue(raw.sourceAgentRunId ?? raw.source_agent_run_id),
    sourceChatSessionId: nullableStringValue(raw.sourceChatSessionId ?? raw.source_chat_session_id),
    stepCount: numberValue(raw.stepCount ?? raw.step_count),
    stepStatusCounts: {
      planned: numberValue(stepStatusCounts.planned),
      executed: numberValue(stepStatusCounts.executed),
      requiresProposal: numberValue(
        stepStatusCounts.requiresProposal ?? stepStatusCounts.requires_proposal
      ),
      blocked: numberValue(stepStatusCounts.blocked),
    },
    generatedProposalIds: proposalIds,
    generatedProposalCount:
      numberValue(raw.generatedProposalCount ?? raw.generated_proposal_count) ?? proposalIds.length,
    governanceDecisionCounts: {
      allow: numberValue(governanceDecisionCounts.allow),
      requireProposal: numberValue(
        governanceDecisionCounts.requireProposal ?? governanceDecisionCounts.require_proposal
      ),
      block: numberValue(governanceDecisionCounts.block),
    },
    warningCount: numberValue(raw.warningCount ?? raw.warning_count),
    metadataSafe: booleanValue(raw.metadataSafe ?? raw.metadata_safe),
    directLifeModelWrites: booleanValue(raw.directLifeModelWrites ?? raw.direct_life_model_writes),
    externalWritesExecuted: booleanValue(
      raw.externalWritesExecuted ?? raw.external_writes_executed
    ),
  };
}

function countLabel(count: number | undefined, label: string): string | null {
  if (count === undefined) return null;
  return `${count} ${label}`;
}

export function planExecuteProductSubtitle(trace: PlanExecuteProductTrace): string {
  return [
    trace.scenarioId,
    countLabel(trace.stepCount, "步"),
    trace.generatedProposalCount === undefined ? null : `待确认 ${trace.generatedProposalCount}`,
  ]
    .filter(Boolean)
    .join(" · ");
}

export function planExecuteProductSearchText(trace: PlanExecuteProductTrace): string {
  return [
    "plan_execute_product",
    "plan execute product",
    "weekly planning",
    trace.scenarioId,
    trace.planSessionId,
    trace.runtimeStrategyTraceKind,
    trace.strategyKind,
    trace.selectedStrategyKind,
    trace.payloadKind,
    trace.strategyDescriptorId,
    trace.selectionReasonCode,
    trace.governanceDecisionKind,
    ...(trace.strategyCapabilityIds ?? []),
    trace.status,
    trace.sourceAgentRunId,
    trace.sourceChatSessionId,
    ...(trace.generatedProposalIds ?? []),
  ]
    .filter(Boolean)
    .join(" ");
}
