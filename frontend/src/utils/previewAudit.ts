import type { AgentRun } from "../tauri";

export interface MultiStrategyPreviewAudit {
  runtimeStrategyTraceKind?: string;
  previewRuntime?: string;
  taskKind?: string;
  selectedStrategyKind?: string;
  strategyKind?: string;
  payloadKind?: string;
  strategyDescriptorId?: string;
  strategyCapabilityIds?: string[];
  governanceDecisionKind?: string;
  governancePolicyKind?: string;
  selectionReasonCode?: string;
  reasonCode?: string;
  riskLevel?: string;
  hasHsPacket?: boolean;
  registryReady?: boolean;
  defaultChatUnchanged?: boolean;
  sideEffectBudget?: Record<string, number>;
  warnings?: string[];
  proposalIds?: string[];
  planStepCount?: number;
  planStepStatuses?: string[];
  blocked?: boolean;
  metadataSafe?: boolean;
  innerRunId?: string | null;
  writeControl?: {
    declaredWriteStepCount?: number;
    proposalRequiredStepCount?: number;
    blockedStepCount?: number;
  };
}

export function getMultiStrategyPreviewAudit(
  run?: AgentRun | null
): MultiStrategyPreviewAudit | null {
  const trace = run?.reasoningTrace as any;
  const raw = trace?.strategy_result ?? trace?.strategyResult;
  const audit = raw?.multiStrategyPreview ?? raw;
  if (!audit || typeof audit !== "object") return null;
  if (audit.previewRuntime === "multi_strategy") {
    return audit as MultiStrategyPreviewAudit;
  }
  if (run?.reasoningStrategy === "multi_strategy_preview" && audit.strategyKind) {
    return audit as MultiStrategyPreviewAudit;
  }
  return null;
}

export function previewWarningLabel(count: number): string {
  return `${count} warning${count === 1 ? "" : "s"}`;
}
