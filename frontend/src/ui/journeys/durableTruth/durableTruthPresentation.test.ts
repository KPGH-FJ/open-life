import { describe, expect, it } from "vitest";
import {
  buildDurableFixtureSnapshot,
  durableReviewItem,
} from "@/dev/phase4d/phase4d-durable-fixtures";
import { durableLifecyclePresentation } from "./durableTruthPresentation";

describe("durable truth lifecycle presentation", () => {
  it("keeps pending, approved, and applying distinct from applied", () => {
    expect(
      durableLifecyclePresentation(
        buildDurableFixtureSnapshot("fixture-ready", "pending"),
        durableReviewItem("pending")
      )
    ).toMatchObject({ lifecycle: "pending_review", status: "waiting" });

    expect(
      durableLifecyclePresentation(
        buildDurableFixtureSnapshot("fixture-durable-approved", "approved_not_applied"),
        durableReviewItem("approved_not_applied")
      )
    ).toMatchObject({ lifecycle: "approved_not_applied", status: "neutral" });

    expect(
      durableLifecyclePresentation(
        buildDurableFixtureSnapshot("fixture-durable-applying", "applying"),
        durableReviewItem("applying")
      )
    ).toMatchObject({ lifecycle: "applying", status: "waiting" });
  });

  it("uses green only when the exact proposal materialization proof exists", () => {
    const snapshot = buildDurableFixtureSnapshot("fixture-durable-applied", "applied");
    const item = durableReviewItem("applied");
    expect(durableLifecyclePresentation(snapshot, item)).toMatchObject({
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
    expect(durableLifecyclePresentation(withoutExactProof, item)).toMatchObject({
      lifecycle: "unknown",
      status: "unknown",
      label: "应用证明不完整",
    });
  });

  it("keeps stale, failed, and rolled-back outcomes explicit and non-green", () => {
    expect(
      durableLifecyclePresentation(
        buildDurableFixtureSnapshot("fixture-stale", "applied"),
        durableReviewItem("applied")
      )
    ).toMatchObject({ lifecycle: "unknown", status: "stale" });
    expect(
      durableLifecyclePresentation(
        buildDurableFixtureSnapshot("fixture-durable-failed", "failed"),
        durableReviewItem("failed")
      )
    ).toMatchObject({ lifecycle: "failed", status: "error" });
    expect(
      durableLifecyclePresentation(
        buildDurableFixtureSnapshot("fixture-durable-rolled-back", "rolled_back"),
        durableReviewItem("rolled_back")
      )
    ).toMatchObject({ lifecycle: "rolled_back", status: "waiting" });
  });

  it("fails closed when durable owners report pending refs but ReviewItem is missing", () => {
    const snapshot = buildDurableFixtureSnapshot("fixture-ready", "pending");
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

    expect(durableLifecyclePresentation(withoutReviewItem, null)).toMatchObject({
      lifecycle: "unknown",
      label: "变更状态不完整",
      status: "unknown",
    });
  });
});
