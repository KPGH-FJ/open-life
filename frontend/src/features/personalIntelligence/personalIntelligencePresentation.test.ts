import { describe, expect, it } from "vitest";
import {
  buildPersonalIntelligenceFixtureSnapshot,
  durableReviewItem,
} from "@/test/fixtures/workbench/personalIntelligence";
import { personalIntelligenceLifecyclePresentation } from "./personalIntelligencePresentation";

describe("Personal Intelligence lifecycle presentation", () => {
  it("keeps pending, approved, and applying distinct from applied", () => {
    expect(
      personalIntelligenceLifecyclePresentation(
        buildPersonalIntelligenceFixtureSnapshot("fixture-ready", "pending"),
        durableReviewItem("pending")
      )
    ).toMatchObject({ lifecycle: "pending_review", status: "waiting" });

    expect(
      personalIntelligenceLifecyclePresentation(
        buildPersonalIntelligenceFixtureSnapshot(
          "fixture-personal-intelligence-approved",
          "approved_not_applied"
        ),
        durableReviewItem("approved_not_applied")
      )
    ).toMatchObject({ lifecycle: "approved_not_applied", status: "neutral" });

    expect(
      personalIntelligenceLifecyclePresentation(
        buildPersonalIntelligenceFixtureSnapshot(
          "fixture-personal-intelligence-applying",
          "applying"
        ),
        durableReviewItem("applying")
      )
    ).toMatchObject({ lifecycle: "applying", status: "waiting" });
  });

  it("uses green only when the exact proposal materialization proof exists", () => {
    const snapshot = buildPersonalIntelligenceFixtureSnapshot(
      "fixture-personal-intelligence-applied",
      "applied"
    );
    const item = durableReviewItem("applied");
    expect(personalIntelligenceLifecyclePresentation(snapshot, item)).toMatchObject({
      lifecycle: "applied",
      status: "success",
      verified: true,
    });

    const withoutExactProof = {
      ...snapshot,
      lifeModelEnvelope: {
        ...snapshot.lifeModelEnvelope,
        data: snapshot.lifeModelEnvelope.data
          ? { ...snapshot.lifeModelEnvelope.data, materializedChanges: [] }
          : null,
      },
    };
    expect(personalIntelligenceLifecyclePresentation(withoutExactProof, item)).toMatchObject({
      lifecycle: "unknown",
      status: "unknown",
      label: "应用证明不完整",
    });
  });

  it("keeps stale, failed, and rolled-back outcomes explicit and non-green", () => {
    expect(
      personalIntelligenceLifecyclePresentation(
        buildPersonalIntelligenceFixtureSnapshot("fixture-stale", "applied"),
        durableReviewItem("applied")
      )
    ).toMatchObject({ lifecycle: "unknown", status: "stale" });
    expect(
      personalIntelligenceLifecyclePresentation(
        buildPersonalIntelligenceFixtureSnapshot("fixture-personal-intelligence-failed", "failed"),
        durableReviewItem("failed")
      )
    ).toMatchObject({ lifecycle: "failed", status: "error" });
    expect(
      personalIntelligenceLifecyclePresentation(
        buildPersonalIntelligenceFixtureSnapshot(
          "fixture-personal-intelligence-rolled-back",
          "rolled_back"
        ),
        durableReviewItem("rolled_back")
      )
    ).toMatchObject({ lifecycle: "rolled_back", status: "waiting" });
  });

  it("fails closed when Personal Intelligence owners report pending refs but ReviewItem is missing", () => {
    const snapshot = buildPersonalIntelligenceFixtureSnapshot("fixture-ready", "pending");
    const withoutReviewItem = {
      ...snapshot,
      reviewEnvelope: {
        ...snapshot.reviewEnvelope,
        status: "empty" as const,
        data: snapshot.reviewEnvelope.data
          ? { ...snapshot.reviewEnvelope.data, items: [], batches: [] }
          : null,
      },
    };

    expect(personalIntelligenceLifecyclePresentation(withoutReviewItem, null)).toMatchObject({
      lifecycle: "unknown",
      label: "变更状态不完整",
      status: "unknown",
    });
  });

  it("does not present historical Agent Memory refs as a missing LifeModel review", () => {
    const snapshot = buildPersonalIntelligenceFixtureSnapshot("fixture-ready", "pending");
    if (!snapshot.lifeModelEnvelope.data) throw new Error("fixture LifeModel missing");
    snapshot.lifeModelEnvelope.data = {
      ...snapshot.lifeModelEnvelope.data,
      pendingUpdateCounts: {
        ...snapshot.lifeModelEnvelope.data.pendingUpdateCounts,
        candidate: 0,
        pendingReview: 0,
        approvedNotApplied: 0,
        failedMaterialization: 0,
      },
      candidateChanges: [],
      relatedReviewItemRefs: [],
    };
    snapshot.reviewEnvelope = {
      ...snapshot.reviewEnvelope,
      status: "empty",
      data: snapshot.reviewEnvelope.data
        ? { ...snapshot.reviewEnvelope.data, items: [], batches: [] }
        : null,
    };

    expect(personalIntelligenceLifecyclePresentation(snapshot, null, "life_model")).toMatchObject({
      lifecycle: "none",
      label: "没有待核对变更",
      status: "neutral",
    });
  });

  it("does not let one peer owner failure poison the other owner's review truth", () => {
    const snapshot = buildPersonalIntelligenceFixtureSnapshot("fixture-ready", "pending");
    snapshot.memoryEnvelope = {
      ...snapshot.memoryEnvelope,
      data: null,
      status: "error",
      evidenceRefs: [],
    };

    expect(
      personalIntelligenceLifecyclePresentation(snapshot, durableReviewItem("pending"))
    ).toMatchObject({
      lifecycle: "pending_review",
      status: "waiting",
    });
  });
});
