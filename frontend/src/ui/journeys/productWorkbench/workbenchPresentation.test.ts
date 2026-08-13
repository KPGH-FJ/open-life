import { describe, expect, it } from "vitest";
import type {
  EvidenceRef,
  ProviderPrivacyBoundarySummary,
  TaskViewModelItem,
  ViewModelEnvelope,
} from "@/tauri";
import { boundaryPresentation, taskLifecyclePresentation } from "./workbenchPresentation";

const routeEvidence: EvidenceRef = {
  id: "provider-route:test",
  label: "Provider route",
  source: "provider",
  sensitivity: "local_private",
};

function boundaryEnvelope(
  data: ProviderPrivacyBoundarySummary | null,
  status: ViewModelEnvelope<ProviderPrivacyBoundarySummary>["status"] = "ready",
  evidenceRefs: EvidenceRef[] = data?.evidenceRefs ?? []
): ViewModelEnvelope<ProviderPrivacyBoundarySummary> {
  return {
    data,
    status,
    lastUpdatedAt: "2026-07-18T08:30:00.000Z",
    source: "backend-readmodel",
    evidenceRefs,
    actions: { primary: [], review: [], debugOnly: [] },
  };
}

function localBoundary(
  overrides: Partial<ProviderPrivacyBoundarySummary> = {}
): ProviderPrivacyBoundarySummary {
  return {
    routeType: "local",
    externalTransmission: "not_sent",
    providerLabel: "Ollama",
    modelLabel: "qwen2.5:14b",
    privacyLabel: "local only",
    risk: "none",
    localOnlyRequired: true,
    evidenceRefs: [routeEvidence],
    ...overrides,
  };
}

function task(overrides: Partial<TaskViewModelItem> = {}): TaskViewModelItem {
  return {
    canonicalTaskId: "task-test",
    relatedRunIds: [],
    title: "Test task",
    strategy: "react",
    lifecycleStatus: "completed",
    terminalDeliveryStatus: "delivered",
    finalDeliveryEvidencePresent: true,
    items: [],
    artifacts: [],
    pendingBlockers: [],
    pendingReviewItemRefs: [],
    allowedControls: [],
    nextRecommendedControl: "none",
    evidenceRefs: [],
    ...overrides,
  };
}

describe("Workbench read-only presentation invariants", () => {
  it("renders verified local only when route, transmission, risk, and evidence are all known", () => {
    expect(boundaryPresentation(boundaryEnvelope(localBoundary()))).toMatchObject({
      status: "success",
      verified: true,
    });

    const failClosed = [
      boundaryEnvelope(localBoundary({ routeType: "unknown" })),
      boundaryEnvelope(localBoundary({ externalTransmission: "unknown" })),
      boundaryEnvelope(localBoundary({ externalTransmission: "possible" })),
      boundaryEnvelope(localBoundary({ risk: "unknown" })),
      boundaryEnvelope(localBoundary({ evidenceRefs: [] }), "ready", []),
      boundaryEnvelope(localBoundary(), "stale"),
      boundaryEnvelope(null, "error", []),
    ];

    for (const envelope of failClosed) {
      expect(boundaryPresentation(envelope).status).not.toBe("success");
      expect(boundaryPresentation(envelope).verified).not.toBe(true);
    }
  });

  it("requires delivered final evidence before a completed task can render green", () => {
    expect(taskLifecyclePresentation(task())).toMatchObject({
      label: "已完成",
      status: "success",
      verified: true,
    });
    expect(taskLifecyclePresentation(task({ finalDeliveryEvidencePresent: false }))).toMatchObject({
      label: "完成证据不足",
      status: "blocked",
    });
    expect(taskLifecyclePresentation(task({ terminalDeliveryStatus: "unknown" }))).toMatchObject({
      label: "完成证据不足",
      status: "blocked",
    });
  });

  it("keeps pending review separate from completion", () => {
    expect(
      taskLifecyclePresentation(
        task({
          lifecycleStatus: "completed_with_pending_review",
          terminalDeliveryStatus: "completed_with_pending_review",
          finalDeliveryEvidencePresent: false,
        })
      )
    ).toMatchObject({ label: "待审核，未完成", status: "waiting" });
  });

  it("keeps a remote-unknown task unverified and non-green", () => {
    expect(
      taskLifecyclePresentation(
        task({
          lifecycleStatus: "remote_unknown",
          terminalDeliveryStatus: "unknown",
          finalDeliveryEvidencePresent: false,
        })
      )
    ).toEqual({ label: "远端结果未知", status: "unknown" });
  });
});
