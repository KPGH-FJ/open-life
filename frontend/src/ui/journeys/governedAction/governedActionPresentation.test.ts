import { describe, expect, it } from "vitest";
import type {
  ProviderPrivacyBoundarySummary,
  ReviewItem,
  TaskViewModelItem,
  ViewModelEnvelope,
} from "@/tauri";
import type { GovernedActionSnapshot } from "./governedActionDataSource";
import {
  governedBoundaryEnvelope,
  reviewContext,
  workspaceContext,
} from "./governedActionPresentation";

const boundary: ProviderPrivacyBoundarySummary = {
  routeType: "local",
  externalTransmission: "not_sent",
  providerLabel: "Local",
  modelLabel: "Model",
  privacyLabel: "Local only",
  risk: "none",
  localOnlyRequired: true,
  evidenceRefs: [],
};

function envelope<T>(
  data: T,
  status: ViewModelEnvelope<T>["status"] = "ready"
): ViewModelEnvelope<T> {
  return {
    data,
    status,
    lastUpdatedAt: "2026-07-20T00:00:00Z",
    source: "backend-readmodel",
    evidenceRefs: [],
    warnings: [],
    actions: { primary: [], review: [], debugOnly: [] },
  };
}

function activeTask(): TaskViewModelItem {
  return {
    canonicalTaskId: "task-1",
    relatedRunIds: [],
    title: "Prepare interview synthesis",
    lifecycleStatus: "waiting_permission",
    terminalDeliveryStatus: "not_terminal",
    finalDeliveryEvidencePresent: false,
    items: [],
    artifacts: [],
    pendingBlockers: ["permission required"],
    pendingReviewItemRefs: [],
    allowedControls: [
      {
        id: "task-1:resume",
        label: "Resume",
        kind: "resume",
        effect: "task_resume_request",
        enabled: true,
        targetTaskId: "task-1",
        completionProofAfterDispatch: false,
      },
    ],
    nextRecommendedControl: "resume",
    evidenceRefs: [],
  };
}

function reviewItem(): ReviewItem {
  return {
    id: "review-1",
    type: "tool_permission",
    source: {
      kind: "proposal",
      proposalId: "review-1",
      proposalSource: "chat_conversation",
    },
    status: "approved",
    materializationStatus: "unknown",
    decisionContext: {
      reviewItemId: "review-1",
      title: "Allow once",
      summary: "Allow one exact action.",
      after: {
        kind: "redacted",
        summary: "See permission context.",
        sensitivity: "redacted",
        truncated: false,
      },
      reasonSummary: "Read one note.",
      sourceSummary: "Current conversation",
      impactSummary: "Decision only.",
      affectedObjectLabels: ["Note"],
      evidenceRefs: [],
    },
    allowedActions: [],
    risk: "medium",
    evidenceRefs: [],
    targetRefs: [],
  };
}

function snapshot(status: ViewModelEnvelope<unknown>["status"] = "ready"): GovernedActionSnapshot {
  return {
    capturedAt: "2026-07-20T00:00:00Z",
    conversationEnvelope: envelope(
      {
        status: "ready",
        conversations: [],
        projects: [],
        selectedProjectId: null,
        selectedConversationId: null,
        globalMemoryEnabled: true,
        selectedMemoryMode: "use_and_learn",
        messages: [],
        latestTurn: null,
        providerStatus: "ready",
        providerProfiles: [],
        selectedProviderProfileId: null,
        providerErrorCode: null,
        workStatus: "available",
      },
      status
    ),
    workspaceEnvelope: envelope(
      {
        tasks: [activeTask()],
        activeTask: activeTask(),
        recentTaskRefs: [],
        pendingReviewItems: [],
        activity: [],
        providerPrivacyBoundarySummary: boundary,
        activityRedactionState: "metadata_only",
        sourceRefs: [],
        contractLimitations: [],
      },
      status
    ),
    reviewEnvelope: envelope(
      {
        batches: [],
        items: [reviewItem()],
        summary: {
          total: 1,
          actionRequiredCount: 0,
          blockedActionCount: 0,
          byStatus: { approved: 1 },
          byRisk: { medium: 1 },
          byMaterializationStatus: { unknown: 1 },
        },
      },
      status
    ),
    tasksEnvelope: envelope(
      {
        items: [activeTask()],
        summary: {
          total: 1,
          needsAttentionCount: 0,
          activeCount: 1,
          waitingReviewCount: 0,
          waitingPermissionCount: 1,
          blockedCount: 0,
          pendingReviewCount: 0,
          completedCount: 0,
          completedNeedsEvidenceCount: 0,
          failedCount: 0,
          cancelledCount: 0,
          byLifecycleStatus: { waiting_permission: 1 },
        },
        sourceRefs: [],
        contractLimitations: [],
      },
      status
    ),
    boundaryEnvelope: envelope(boundary, status),
    diagnostics: [],
  };
}

describe("governed action presentation", () => {
  it("propagates stale and error Workspace status into the privacy boundary", () => {
    expect(governedBoundaryEnvelope(snapshot("stale")).status).toBe("stale");
    expect(governedBoundaryEnvelope(snapshot("error")).status).toBe("error");
  });

  it("keeps waiting permission as the single Workspace conclusion", () => {
    expect(workspaceContext(snapshot()).status).toMatchObject({
      label: "等待确认",
      status: "waiting",
    });
  });

  it("labels approved permission as not resumed, never completed", () => {
    expect(reviewContext(snapshot(), reviewItem()).status).toMatchObject({
      label: "已允许一次，尚未继续任务",
      status: "neutral",
    });
  });
});
