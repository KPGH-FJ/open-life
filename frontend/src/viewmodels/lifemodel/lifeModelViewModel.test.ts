import { readdirSync, readFileSync } from "node:fs";
import { basename, join } from "node:path";
import { describe, expect, it } from "vitest";
import { buildLifeModelViewModelEnvelope } from "./lifeModelViewModelAdapter";
import {
  emptyLifeModelViewModelInput,
  errorLifeModelViewModelInput,
  makeLifeModelCurrentView,
  makeLifeModelProposal,
  makeTierStats,
  readyLifeModelViewModelInput,
  safeModeLifeModelViewModelInput,
  staleLifeModelViewModelInput,
} from "./lifeModelViewModel.fixtures";

describe("lifeModelViewModelAdapter", () => {
  it("represents an empty LifeModel without a fake canonical summary", () => {
    const envelope = buildLifeModelViewModelEnvelope(emptyLifeModelViewModelInput);

    expect(envelope.status).toBe("empty");
    expect(envelope.data?.truthMode).toBe("unknown");
    expect(envelope.data?.canonicalSummary).toBeNull();
    expect(envelope.data?.dimensionSummaries).toEqual([]);
    expect(envelope.warnings?.map(warning => warning.code)).toContain("lifemodel.empty_limited");
  });

  it("labels current compatibility view as current compatibility instead of canonical", () => {
    const envelope = buildLifeModelViewModelEnvelope(readyLifeModelViewModelInput);

    expect(envelope.status).toBe("ready");
    expect(envelope.data?.truthMode).toBe("current_compatibility");
    expect(envelope.data?.canonicalSummary).toBeNull();
    expect(envelope.data?.currentViewSummary).toMatchObject({
      compatibilityMode: true,
      label: "Short-term goal",
      ownerStatus: "PARTIAL",
    });
  });

  it("keeps dimension summaries limited with confidence and provenance labels", () => {
    const envelope = buildLifeModelViewModelEnvelope(readyLifeModelViewModelInput);
    const goals = envelope.data?.dimensionSummaries.find(dimension => dimension.id === "goals");

    expect(goals).toMatchObject({
      label: "Goals",
      confidence: "medium",
      provenance: "limited",
      ownerStatus: "PHASE_2_REQUIRED",
    });
    expect(goals?.pendingReviewItemRefs).toHaveLength(1);
    expect(goals?.evidenceRefs.map(ref => ref.label)).toContain("Model4DCompletion");
  });

  it("maps pending LifeModel proposals to candidate changes and pending counts only", () => {
    const envelope = buildLifeModelViewModelEnvelope(readyLifeModelViewModelInput);

    expect(envelope.data?.pendingUpdateCounts).toMatchObject({
      candidate: 1,
      pendingReview: 1,
      approvedNotApplied: 0,
      failedMaterialization: 0,
    });
    expect(envelope.data?.candidateChanges).toHaveLength(1);
    expect(envelope.data?.candidateChanges[0]).toMatchObject({
      decisionStatus: "pending",
      affectedDimensionIds: ["goals"],
    });
    expect(envelope.data?.materializedChanges).toEqual([]);
  });

  it("counts explicit life_model_update proposals even when the path is not dimension-prefixed", () => {
    const envelope = buildLifeModelViewModelEnvelope({
      ...readyLifeModelViewModelInput,
      pendingProposals: [
        makeLifeModelProposal({
          id: "proposal-lifemodel-overview-1",
          proposalType: "life_model_update",
          affectedPath: "profile.overview",
        }),
      ],
    });

    expect(envelope.data?.pendingUpdateCounts.pendingReview).toBe(1);
    expect(envelope.data?.candidateChanges).toHaveLength(1);
    expect(envelope.data?.candidateChanges[0]).toMatchObject({
      decisionStatus: "pending",
      affectedDimensionIds: ["unknown"],
    });
  });

  it("does not turn accepted proposal or current-view evidence into applied materialization", () => {
    const acceptedProposal = makeLifeModelProposal({
      id: "proposal-accepted-1",
      status: "accepted",
      resolvedAt: "2026-07-09T01:00:00.000Z",
    });
    const currentView = makeLifeModelCurrentView({
      change: {
        path: "goals.short_term[0]",
        proposalId: acceptedProposal.id,
        proposalStatus: "accepted",
        proposalSource: "chat_conversation",
        confidence: 0.8,
        riskLevel: "medium",
        before: null,
        after: {
          name: "Ship the limited LifeModel slice",
        },
        snapshotVersions: [],
        currentMatchesAcceptedAfter: true,
      },
    });

    const envelope = buildLifeModelViewModelEnvelope({
      ...readyLifeModelViewModelInput,
      currentView,
      pendingProposals: [acceptedProposal],
    });

    expect(envelope.data?.pendingUpdateCounts.approvedNotApplied).toBe(1);
    expect(envelope.data?.candidateChanges).toEqual([]);
    expect(envelope.data?.materializedChanges).toEqual([]);
    expect(envelope.warnings?.map(warning => warning.code)).toContain(
      "lifemodel.materialization_owner_required"
    );
  });

  it("disables risky actions when stale", () => {
    const envelope = buildLifeModelViewModelEnvelope(staleLifeModelViewModelInput);
    const updateAction = envelope.actions.primary.find(
      action => action.id === "lifemodel.request_update"
    );

    expect(envelope.status).toBe("stale");
    expect(updateAction).toMatchObject({
      enabled: false,
      disabledReason: "Refresh LifeModel state before using this action.",
    });
  });

  it("disables risky actions when Safe Mode is active", () => {
    const envelope = buildLifeModelViewModelEnvelope(safeModeLifeModelViewModelInput);
    const updateAction = envelope.actions.primary.find(
      action => action.id === "lifemodel.request_update"
    );

    expect(envelope.status).toBe("ready");
    expect(updateAction?.enabled).toBe(false);
    expect(updateAction?.disabledReason).toMatch(/Safe Mode/);
    expect(envelope.warnings?.map(warning => warning.code)).toContain("lifemodel.safe_mode");
  });

  it("keeps error envelopes null instead of falling back to raw LifeModel data", () => {
    const envelope = buildLifeModelViewModelEnvelope(errorLifeModelViewModelInput);

    expect(envelope.status).toBe("error");
    expect(envelope.data).toBeNull();
    expect(envelope.evidenceRefs).toEqual([]);
    expect(envelope.actions.primary).toEqual([
      {
        id: "lifemodel.refresh",
        label: "Refresh LifeModel state",
        kind: "refresh",
        enabled: true,
        targetRef: "lifemodel",
      },
    ]);
  });

  it("keeps Memory linkage partial or unknown when only count and tier stats exist", () => {
    const partial = buildLifeModelViewModelEnvelope({
      ...readyLifeModelViewModelInput,
      memoryCount: 14,
      tierStats: makeTierStats(),
    });
    const unknown = buildLifeModelViewModelEnvelope({
      ...readyLifeModelViewModelInput,
      memoryCount: null,
      tierStats: null,
    });

    expect(partial.data?.memoryLinkage).toMatchObject({
      linkedMemoryCount: 14,
      materializedMemoryCount: 0,
      linkageStatus: "partial",
      ownerStatus: "PHASE_2_REQUIRED",
    });
    expect(partial.data?.memoryLinkage.tierSummary).toMatchObject({
      total: 14,
      tier1: 5,
    });
    expect(unknown.data?.memoryLinkage.linkageStatus).toBe("unknown");
  });

  it("keeps debug-only actions out of primary actions", () => {
    const envelope = buildLifeModelViewModelEnvelope(readyLifeModelViewModelInput);
    const primaryIds = new Set(envelope.actions.primary.map(action => action.id));

    expect(envelope.actions.debugOnly).toEqual([
      {
        id: "lifemodel.inspect_limited_input_refs",
        label: "Inspect limited LifeModel input refs",
        kind: "raw_json",
        enabled: true,
        developerOnly: true,
        targetRef: "LifeModelViewModel.limitedInputRefs",
      },
    ]);
    for (const action of envelope.actions.debugOnly ?? []) {
      expect(primaryIds.has(action.id)).toBe(false);
    }
  });

  it("keeps forbidden bridge and write symbols out of the limited ViewModel package", () => {
    const forbiddenSymbols = [
      ["get", "System", "Diagnostics"].join(""),
      ["save", "Life", "Model"].join(""),
      ["accept", "Proposal"].join(""),
      ["batch", "Accept", "Low", "Risk", "Proposals"].join(""),
      ["edit", "Proposal"].join(""),
      ["reject", "Proposal"].join(""),
      ["postpone", "Proposal"].join(""),
      ["tauri", "Dev"].join(""),
      ["safe", "Invoke"].join(""),
      ["invoke", "("].join(""),
    ];
    const packageDir = join(process.cwd(), "src/viewmodels/lifemodel");
    const sources = readdirSync(packageDir)
      .filter(fileName => fileName.endsWith(".ts"))
      .map(fileName => ({
        fileName: basename(fileName),
        source: readFileSync(join(packageDir, fileName), "utf8"),
      }));

    for (const { fileName, source } of sources) {
      for (const symbol of forbiddenSymbols) {
        expect(source, `${fileName} should not contain ${symbol}`).not.toContain(symbol);
      }
    }
  });
});
