import type { RuntimeMigrationGateReport } from "../../../types";

const SAFE_SUMMARY_KEYS = [
  "runtimeBoundary",
  "defaultChatAdapterRouting",
  "contractHarness",
  "adapterDryRun",
  "dryRunReview",
  "implementationReadiness",
  "adapterPreview",
  "controlledPreviewReview",
  "controlledPreviewApprovalReadiness",
  "cutoverImplementationPlan",
  "cutoverPlanReview",
  "cutoverPlanApprovalReadiness",
  "narrowImplementationDiscussionGate",
  "narrowImplementationPlan",
  "narrowImplementationPlanReview",
  "narrowImplementationPlanApprovalReadiness",
  "activationPlan",
  "activationReview",
  "activationImplementationGate",
  "cutoverReadinessGate",
  "promotionReadinessGate",
  "taskKind",
  "reasonCode",
  "riskLevel",
  "hasHsPacket",
  "policyReasonCode",
  "descriptorKind",
  "candidateAdapter",
  "contractShape",
  "candidateReady",
  "previewReady",
  "nonDefault",
  "allowWrites",
  "maxToolCalls",
  "metadataSafe",
  "planningOnly",
  "requiredEvidenceReady",
  "defaultChatUnchanged",
  "defaultChatPathUnchanged",
  "readOnly",
  "eligible",
  "cutoverPlanApprovalReady",
  "ordinaryEntryPreflightStatusReady",
  "discussionGateEligible",
  "ordinaryEntryPreflight",
  "statusReady",
  "sendPreflightReady",
  "streamPreflightReady",
  "sendSideEffectLockEngaged",
  "streamSideEffectLockEngaged",
  "contractHarnessReady",
  "dryRunReady",
  "blocked",
  "adapterPath",
  "adapterDisabled",
  "chatMessageSaved",
  "agentRunRecorded",
  "reviewerNoteStorage",
  "humanReviewOnly",
  "draftReady",
  "manualReviewRequired",
  "notAutomaticMigration",
  "requiresSeparateImplementation",
  "requiresSeparateCutoverReview",
  "requiresSeparateCutoverImplementation",
  "candidatePromotionReady",
  "currentMode",
  "routingMode",
  "adapterScaffoldPresent",
  "controlledAdapterEnabled",
  "defaultSendPath",
  "startStreamPath",
  "controlledCandidateAvailable",
  "activationImplementationGateEligible",
  "candidatePromotionReadinessRequired",
  "automaticMigrationEnabled",
  "ready",
  "requiredApprovedPreviews",
  "approvedPreviewCount",
  "verifiedPreviewRunCount",
  "latestPreviewRunId",
  "inputMessageLength",
  "inputMessageHash",
  "planSectionCount",
  "implementationReadinessReady",
  "previewReviewApproved",
  "previewDigestMatched",
  "cutoverReadinessEligible",
  "requiredApprovedCandidates",
  "approvedCandidateCount",
  "verifiedCandidateCount",
  "latestDecisionKind",
  "blockingReasonCount",
  "activationSectionCount",
  "preconditionSectionCount",
  "adapterContractCheckCount",
  "fallbackPlanCount",
  "rollbackPlanCount",
  "observabilityPlanCount",
  "testPlanCount",
  "latestDecisionPresent",
  "implementationGateEligible",
  "activationPlanDigestMatched",
  "approvedCount",
  "rejectedCount",
  "requestReworkCount",
  "rejectOrReworkCount",
  "implementationEligible",
  "latestShadowReviewDecisionKind",
  "shadowRunReady",
  "blockedBeforeRuntime",
  "userOutputPresent",
  "outputDigestPresent",
  "contentStorage",
  "toolStorage",
  "chatHistoryStorage",
  "proposalStorage",
  "lifeModelPatchStorage",
  "memoryStorage",
  "evidenceStorage",
  "mcpAuditStorage",
  "transcriptStorage",
  "agentRunStorage",
  "runtimeCallStorage",
  "modelCallStorage",
  "externalWriteStorage",
];
export const GATE_FIELDS: Array<keyof Omit<RuntimeMigrationGateReport, "blockingReasons">> = [
  "defaultChatUnchanged",
  "previewPathHealthy",
  "metadataSafeTraceReady",
  "fallbackAvailable",
  "noExternalWrites",
  "proposalFirstPreserved",
];

export function classNames(...classes: (string | false | undefined)[]) {
  return classes.filter(Boolean).join(" ");
}

export function readableError(error: unknown): string {
  if (typeof error === "string") return error;
  if (error && typeof error === "object") {
    if ("message" in error && typeof (error as any).message === "string") {
      return (error as any).message;
    }
    if ("error" in error && typeof (error as any).error === "string") {
      return (error as any).error;
    }
  }
  return String(error);
}

export function safeSummaryEntries(summary: Record<string, unknown>): Array<[string, string]> {
  return SAFE_SUMMARY_KEYS.flatMap(key => {
    const value = summary[key];
    if (value === undefined || value === null) return [];
    if (!["string", "number", "boolean"].includes(typeof value)) return [];
    return [[key, String(value)]];
  });
}

export function PlanList({ title, items }: { title: string; items: string[] }) {
  return (
    <div>
      <div className="text-xs font-medium text-stone-700">{title}</div>
      <div className="mt-1 space-y-1">
        {items.map(item => (
          <div
            key={item}
            className="rounded-md border border-stone-100 bg-stone-50 px-2 py-1 text-xs leading-5 text-stone-700"
          >
            {item}
          </div>
        ))}
      </div>
    </div>
  );
}
