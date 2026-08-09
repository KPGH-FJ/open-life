import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ReviewAction, TaskControl } from "@/tauri";

const tauriMocks = vi.hoisted(() => ({
  acceptProposal: vi.fn(),
  cancelMainChatAgentTask: vi.fn(),
  editLifeModelLearningProposal: vi.fn(),
  getReviewCenterViewModel: vi.fn(),
  getTasksViewModel: vi.fn(),
  getWorkspaceViewModel: vi.fn(),
  postponeProposal: vi.fn(),
  refreshMainChatAgentTaskContext: vi.fn(),
  rejectProposal: vi.fn(),
  resumeMainChatAgentTask: vi.fn(),
  retryMainChatAgentAction: vi.fn(),
}));

vi.mock("@/tauri", () => tauriMocks);

import { tauriGovernedActionDataSource } from "./governedActionDataSource";

function action(kind: ReviewAction["kind"]): ReviewAction {
  const effect =
    kind === "apply"
      ? "materialization_request"
      : kind === "resume"
        ? "task_resume_request"
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

describe("governed action Tauri data source", () => {
  beforeEach(() => vi.clearAllMocks());

  it("loads Workspace, Review, and Tasks as separate backend owners", async () => {
    const envelope = {
      data: null,
      status: "empty",
      lastUpdatedAt: "2026-07-20T00:00:00Z",
      source: "backend-readmodel",
      evidenceRefs: [],
      warnings: [],
      actions: { primary: [], review: [], debugOnly: [] },
    };
    tauriMocks.getWorkspaceViewModel.mockResolvedValue(envelope);
    tauriMocks.getReviewCenterViewModel.mockResolvedValue(envelope);
    tauriMocks.getTasksViewModel.mockResolvedValue(envelope);

    const snapshot = await tauriGovernedActionDataSource.load();

    expect(tauriMocks.getWorkspaceViewModel).toHaveBeenCalledOnce();
    expect(tauriMocks.getReviewCenterViewModel).toHaveBeenCalledOnce();
    expect(tauriMocks.getTasksViewModel).toHaveBeenCalledOnce();
    expect(snapshot.diagnostics.every(item => item.status === "loaded")).toBe(true);
  });

  it("preserves partial command failure as an error envelope", async () => {
    const envelope = {
      data: null,
      status: "empty",
      lastUpdatedAt: null,
      source: "backend-readmodel",
      evidenceRefs: [],
      warnings: [],
      actions: { primary: [], review: [], debugOnly: [] },
    };
    tauriMocks.getWorkspaceViewModel.mockResolvedValue(envelope);
    tauriMocks.getReviewCenterViewModel.mockRejectedValue(new Error("review unavailable"));
    tauriMocks.getTasksViewModel.mockResolvedValue(envelope);

    const snapshot = await tauriGovernedActionDataSource.load();

    expect(snapshot.reviewEnvelope.status).toBe("error");
    expect(snapshot.workspaceEnvelope.status).toBe("empty");
    expect(snapshot.diagnostics).toContainEqual({
      id: "review_center_view_model",
      status: "failed",
      message: "review unavailable",
    });
  });

  it("maps only supported review decisions and task resume to product commands", async () => {
    tauriMocks.acceptProposal.mockResolvedValue(undefined);
    tauriMocks.rejectProposal.mockResolvedValue(undefined);
    tauriMocks.postponeProposal.mockResolvedValue(undefined);
    tauriMocks.resumeMainChatAgentTask.mockResolvedValue(undefined);
    await tauriGovernedActionDataSource.dispatchReviewAction(action("approve"));
    await tauriGovernedActionDataSource.dispatchReviewAction(action("reject"));
    await tauriGovernedActionDataSource.dispatchReviewAction(action("later"));
    await tauriGovernedActionDataSource.resumeTask({
      id: "task-1:resume",
      label: "Resume",
      kind: "resume",
      effect: "task_resume_request",
      enabled: true,
      targetTaskId: "task-1",
      completionProofAfterDispatch: false,
    } satisfies TaskControl);

    expect(tauriMocks.acceptProposal).toHaveBeenCalledWith("review-1");
    expect(tauriMocks.rejectProposal).toHaveBeenCalledWith("review-1");
    expect(tauriMocks.postponeProposal).toHaveBeenCalledWith("review-1");
    expect(tauriMocks.resumeMainChatAgentTask).toHaveBeenCalledWith("task-1");
  });

  it("uses the schema-aware LifeModel learning editor and verifies its receipt", async () => {
    tauriMocks.editLifeModelLearningProposal.mockResolvedValueOnce({
      proposalId: "review-1",
      status: "edited_pending_review",
      resultDocumentDigest: "sha256:result",
      durableWriteExecuted: false,
    });

    await tauriGovernedActionDataSource.editLifeModelLearningProposal(
      "review-1",
      "先给结论，再补充依据"
    );

    expect(tauriMocks.editLifeModelLearningProposal).toHaveBeenCalledWith(
      "review-1",
      "先给结论，再补充依据"
    );
  });

  it("dispatches only exact executable TaskControl contracts", async () => {
    const controls: TaskControl[] = [
      {
        id: "task-1:resume",
        label: "Resume",
        kind: "resume",
        effect: "task_resume_request",
        enabled: true,
        targetTaskId: "task-1",
        completionProofAfterDispatch: false,
      },
      {
        id: "task-1:retry",
        label: "Retry",
        kind: "retry",
        effect: "task_retry_request",
        enabled: true,
        targetTaskId: "task-1",
        targetActionId: "action-2",
        completionProofAfterDispatch: false,
      },
      {
        id: "task-1:cancel",
        label: "Cancel",
        kind: "cancel",
        effect: "task_cancel_request",
        enabled: true,
        requiresConfirmation: true,
        targetTaskId: "task-1",
        completionProofAfterDispatch: false,
      },
      {
        id: "task-1:refresh",
        label: "Refresh context",
        kind: "refresh_context",
        effect: "task_refresh_request",
        enabled: true,
        targetTaskId: "task-1",
        completionProofAfterDispatch: false,
      },
    ];

    for (const control of controls) {
      await tauriGovernedActionDataSource.dispatchTaskControl(control);
    }

    expect(tauriMocks.resumeMainChatAgentTask).toHaveBeenCalledWith("task-1");
    expect(tauriMocks.retryMainChatAgentAction).toHaveBeenCalledWith("task-1", "action-2");
    expect(tauriMocks.cancelMainChatAgentTask).toHaveBeenCalledWith("task-1");
    expect(tauriMocks.refreshMainChatAgentTaskContext).toHaveBeenCalledWith("task-1");
  });

  it("fails closed instead of inventing edit, apply, revoke, resume, or evidence dispatch", async () => {
    for (const kind of ["edit", "apply", "revoke", "resume", "view_evidence"] as const) {
      await expect(
        tauriGovernedActionDataSource.dispatchReviewAction(action(kind))
      ).rejects.toThrow();
    }
    expect(tauriMocks.acceptProposal).not.toHaveBeenCalled();
    expect(tauriMocks.rejectProposal).not.toHaveBeenCalled();
    expect(tauriMocks.postponeProposal).not.toHaveBeenCalled();
  });
});
