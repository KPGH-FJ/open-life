import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ReviewAction, TaskControl } from "@/tauri";

const tauriMocks = vi.hoisted(() => ({
  acceptProposal: vi.fn(),
  editLifeModelLearningProposal: vi.fn(),
  postponeProposal: vi.fn(),
  rejectProposal: vi.fn(),
}));

const workIpcMocks = vi.hoisted(() => ({
  resumeWorkTask: vi.fn(),
  exportArtifactResult: vi.fn(),
  getWorkbenchViewModel: vi.fn(),
  openArtifactResult: vi.fn(),
  requestArtifactUndo: vi.fn(),
  requestTaskArtifactUndo: vi.fn(),
  reviseWorkArtifact: vi.fn(),
  retryWorkTask: vi.fn(),
  stopWorkRun: vi.fn(),
}));

vi.mock("@/ipc/personalIntelligence", () => ({
  editLifeModelLearningProposal: tauriMocks.editLifeModelLearningProposal,
}));
vi.mock("@/ipc/review", () => ({
  acceptProposal: tauriMocks.acceptProposal,
  postponeProposal: tauriMocks.postponeProposal,
  rejectProposal: tauriMocks.rejectProposal,
}));
vi.mock("@/ipc/work", () => workIpcMocks);

import { activeScopedTask, tauriWorkbenchDataSource } from "@/app/workbenchDataSource";
import { workbenchFixtureDataSource } from "@/test/fixtures/workbench/workbench";

function action(kind: ReviewAction["kind"]): ReviewAction {
  const effect =
    kind === "apply"
      ? "materialization_request"
      : kind === "view_evidence"
        ? "evidence_only"
        : "decision_only";
  return {
    id: `review-1:${kind}`,
    label: kind,
    kind,
    effect,
    enabled: true,
    requiresConfirmation: kind === "approve" || kind === "apply",
    targetReviewItemId: "review-1",
    completionProofAfterDispatch: false,
  } as ReviewAction;
}

describe("Workbench Tauri data source", () => {
  beforeEach(() => vi.clearAllMocks());

  it("loads all Workbench lanes from one backend-composed snapshot", async () => {
    const envelope = {
      data: null,
      status: "empty",
      lastUpdatedAt: "2026-07-20T00:00:00Z",
      source: "backend-readmodel",
      evidenceRefs: [],
      warnings: [],
      actions: { primary: [], review: [], debugOnly: [] },
    };
    workIpcMocks.getWorkbenchViewModel.mockResolvedValue({
      capturedAt: "2026-07-20T00:00:00Z",
      workspace: envelope,
      review: envelope,
      tasks: envelope,
      providerBoundary: envelope,
    });

    const snapshot = await tauriWorkbenchDataSource.load();

    expect(workIpcMocks.getWorkbenchViewModel).toHaveBeenCalledOnce();
    expect(snapshot.diagnostics.every(item => item.status === "loaded")).toBe(true);
  });

  it("does not present an unrelated historical task as active for an unpersisted new conversation", async () => {
    const fixture = workbenchFixtureDataSource("fixture-ready");
    const snapshot = await fixture.load();
    snapshot.workspaceEnvelope.data = {
      ...snapshot.workspaceEnvelope.data!,
      selectedConversationId: undefined,
    };

    expect(activeScopedTask(snapshot)).toBeNull();
  });

  it("does not let an older terminal blocker replace the latest task as current execution", async () => {
    const fixture = workbenchFixtureDataSource("fixture-ready");
    const snapshot = await fixture.load();
    const template = snapshot.tasksEnvelope.data!.items[0];
    snapshot.tasksEnvelope.data!.items = [
      {
        ...template,
        canonicalTaskId: "latest-completed",
        lifecycleStatus: "completed",
        terminalDeliveryStatus: "delivered",
        finalDeliveryEvidencePresent: true,
        pendingReviewItemRefs: [],
      },
      {
        ...template,
        canonicalTaskId: "older-blocked",
        lifecycleStatus: "blocked",
        terminalDeliveryStatus: "blocked",
        finalDeliveryEvidencePresent: false,
        pendingReviewItemRefs: [],
      },
    ];

    expect(activeScopedTask(snapshot)).toBeNull();
  });

  it("preserves an independently degraded lane from the aggregate snapshot", async () => {
    const envelope = {
      data: null,
      status: "empty",
      lastUpdatedAt: null,
      source: "backend-readmodel",
      evidenceRefs: [],
      warnings: [],
      actions: { primary: [], review: [], debugOnly: [] },
    };
    const reviewError = {
      ...envelope,
      status: "error",
      warnings: [
        {
          code: "review_center_view_model_unavailable",
          message: "review unavailable",
          severity: "error",
          evidenceRefs: [],
        },
      ],
    };
    workIpcMocks.getWorkbenchViewModel.mockResolvedValue({
      capturedAt: "2026-07-20T00:00:00Z",
      workspace: envelope,
      review: reviewError,
      tasks: envelope,
      providerBoundary: envelope,
    });

    const snapshot = await tauriWorkbenchDataSource.load();

    expect(snapshot.reviewEnvelope.status).toBe("error");
    expect(snapshot.workspaceEnvelope.status).toBe("empty");
    expect(snapshot.diagnostics).toContainEqual({
      id: "review_center_view_model",
      status: "failed",
      message: "review unavailable",
    });
  });

  it("maps only supported review decisions", async () => {
    tauriMocks.acceptProposal.mockResolvedValue(undefined);
    tauriMocks.rejectProposal.mockResolvedValue(undefined);
    tauriMocks.postponeProposal.mockResolvedValue(undefined);
    await tauriWorkbenchDataSource.dispatchReviewAction(action("approve"));
    await tauriWorkbenchDataSource.dispatchReviewAction(action("reject"));
    await tauriWorkbenchDataSource.dispatchReviewAction(action("later"));
    expect(tauriMocks.acceptProposal).toHaveBeenCalledWith("review-1");
    expect(tauriMocks.rejectProposal).toHaveBeenCalledWith("review-1");
    expect(tauriMocks.postponeProposal).toHaveBeenCalledWith("review-1");
  });

  it("uses the schema-aware LifeModel learning editor and verifies its receipt", async () => {
    tauriMocks.editLifeModelLearningProposal.mockResolvedValueOnce({
      proposalId: "review-1",
      status: "edited_pending_review",
      resultDocumentDigest: "sha256:result",
      durableWriteExecuted: false,
      learning: {
        candidateId: "candidate-1",
        proposalId: "review-1",
        changed: true,
        status: "proposed",
        contentScrubbed: false,
        correctionObservationId: "observation-edit-1",
        canonicalLifeModelChanged: false,
      },
    });

    await tauriWorkbenchDataSource.editLifeModelLearningProposal(
      "review-1",
      "先给结论，再补充依据"
    );

    expect(tauriMocks.editLifeModelLearningProposal).toHaveBeenCalledWith(
      "review-1",
      "先给结论，再补充依据"
    );
  });

  it("fails closed when the learning edit receipt lacks candidate evidence", async () => {
    tauriMocks.editLifeModelLearningProposal.mockResolvedValueOnce({
      proposalId: "review-1",
      status: "edited_pending_review",
      resultDocumentDigest: "sha256:result",
      durableWriteExecuted: false,
      learning: {
        candidateId: "",
        proposalId: "review-1",
        changed: true,
        status: "proposed",
        contentScrubbed: false,
        canonicalLifeModelChanged: false,
      },
    } as never);

    await expect(
      tauriWorkbenchDataSource.editLifeModelLearningProposal("review-1", "先给结论")
    ).rejects.toThrow("lifemodel_learning_edit_receipt_unverified");
  });

  it("dispatches only exact executable TaskControl contracts", async () => {
    const controls: TaskControl[] = [
      {
        id: "task-1:resume",
        label: "Continue in a new run",
        kind: "resume",
        effect: "task_resume_request",
        enabled: true,
        targetTaskId: "task-1",
        targetActionId: "run-1",
        completionProofAfterDispatch: false,
      },
      {
        id: "task-1:retry",
        label: "Retry",
        kind: "retry",
        effect: "task_retry_request",
        enabled: true,
        targetTaskId: "task-1",
        targetActionId: "run-2",
        completionProofAfterDispatch: false,
      },
      {
        id: "task-1:stop_run",
        label: "Stop current run",
        kind: "stop_run",
        effect: "task_stop_run_request",
        enabled: true,
        targetTaskId: "task-1",
        targetActionId: "run-3",
        completionProofAfterDispatch: false,
      },
    ];

    for (const control of controls) {
      await tauriWorkbenchDataSource.dispatchTaskControl(control);
    }

    expect(workIpcMocks.resumeWorkTask).toHaveBeenCalledWith("task-1", "run-1");
    expect(workIpcMocks.retryWorkTask).toHaveBeenCalledWith("task-1", "run-2");
    expect(workIpcMocks.stopWorkRun).toHaveBeenCalledWith("task-1", "run-3");
  });

  it("verifies an exact verified Artifact Undo review receipt", async () => {
    workIpcMocks.requestArtifactUndo.mockResolvedValue({
      artifactId: "artifact-1",
      proposalId: "proposal-undo-1",
      status: "waiting_review",
    });

    await tauriWorkbenchDataSource.requestArtifactUndo("artifact-1");

    expect(workIpcMocks.requestArtifactUndo).toHaveBeenCalledWith("artifact-1");
  });

  it("fails closed on an unbound Artifact Undo review receipt", async () => {
    workIpcMocks.requestArtifactUndo.mockResolvedValue({
      artifactId: "artifact-other",
      proposalId: "proposal-undo-1",
      status: "waiting_review",
    });

    await expect(tauriWorkbenchDataSource.requestArtifactUndo("artifact-1")).rejects.toThrow(
      "artifact_undo_receipt_unverified"
    );
  });

  it("verifies a grouped Artifact Undo receipt without hiding partial failures", async () => {
    workIpcMocks.requestTaskArtifactUndo.mockResolvedValue({
      taskId: "task-1",
      status: "partial_waiting_review",
      proposals: [
        {
          artifactId: "artifact-1",
          proposalId: "proposal-undo-1",
          status: "waiting_review",
        },
      ],
      failures: [{ artifactId: "artifact-2", reasonCode: "artifact_undo_request_failed" }],
    });

    await expect(tauriWorkbenchDataSource.requestTaskArtifactUndo("task-1")).resolves.toEqual({
      failures: [{ artifactId: "artifact-2", reasonCode: "artifact_undo_request_failed" }],
    });
    expect(workIpcMocks.requestTaskArtifactUndo).toHaveBeenCalledWith("task-1");
  });

  it("fails closed when a grouped Undo receipt claims success while carrying failures", async () => {
    workIpcMocks.requestTaskArtifactUndo.mockResolvedValue({
      taskId: "task-1",
      status: "waiting_review",
      proposals: [
        {
          artifactId: "artifact-1",
          proposalId: "proposal-undo-1",
          status: "waiting_review",
        },
      ],
      failures: [{ artifactId: "artifact-2", reasonCode: "artifact_undo_request_failed" }],
    });

    await expect(tauriWorkbenchDataSource.requestTaskArtifactUndo("task-1")).rejects.toThrow(
      "task_artifact_undo_receipt_unverified"
    );
  });

  it("starts a focused revision only through the exact Artifact version contract", async () => {
    workIpcMocks.reviseWorkArtifact.mockResolvedValue({
      reply: "等待审核",
      status: "completed",
      blockers: [],
      run_id: "11111111-1111-4111-8111-111111111111",
    });

    await tauriWorkbenchDataSource.reviseArtifact("task-1", "artifact:1", 3, "只缩短结论");

    expect(workIpcMocks.reviseWorkArtifact).toHaveBeenCalledWith(
      "task-1",
      "artifact:1",
      3,
      "只缩短结论"
    );
  });

  it("does not accept a non-terminal focused revision receipt", async () => {
    workIpcMocks.reviseWorkArtifact.mockResolvedValue({
      reply: "unexpected",
      status: "blocked",
      blockers: ["artifact_revision_base_changed"],
      run_id: "11111111-1111-4111-8111-111111111111",
    });

    await expect(
      tauriWorkbenchDataSource.reviseArtifact("task-1", "artifact:1", 3, "只缩短结论")
    ).rejects.toThrow("artifact_revision_receipt_unverified");
  });

  it("opens only the exact Artifact identity and version selected by the result view", async () => {
    workIpcMocks.openArtifactResult.mockResolvedValue(undefined);

    await tauriWorkbenchDataSource.openArtifactResult("artifact-1", 3);

    expect(workIpcMocks.openArtifactResult).toHaveBeenCalledWith("artifact-1", 3);
  });

  it("accepts only a digest-bound Artifact export receipt", async () => {
    workIpcMocks.exportArtifactResult.mockResolvedValue({
      cancelled: false,
      savedPath: "/tmp/result.md",
      contentDigest: "sha256:result",
    });

    await expect(tauriWorkbenchDataSource.exportArtifactResult("artifact-1", 3)).resolves.toBe(
      "/tmp/result.md"
    );
    expect(workIpcMocks.exportArtifactResult).toHaveBeenCalledWith("artifact-1", 3);
  });

  it("fails closed instead of inventing edit, apply, revoke, or evidence dispatch", async () => {
    for (const kind of ["edit", "apply", "revoke", "view_evidence"] as const) {
      await expect(tauriWorkbenchDataSource.dispatchReviewAction(action(kind))).rejects.toThrow();
    }
    expect(tauriMocks.acceptProposal).not.toHaveBeenCalled();
    expect(tauriMocks.rejectProposal).not.toHaveBeenCalled();
    expect(tauriMocks.postponeProposal).not.toHaveBeenCalled();
  });
});
