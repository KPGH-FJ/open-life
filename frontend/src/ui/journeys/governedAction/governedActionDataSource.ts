import {
  acceptProposal,
  cancelWorkTask,
  editLifeModelLearningProposal,
  getReviewCenterViewModel,
  getTasksViewModel,
  getWorkspaceViewModel,
  postponeProposal,
  rejectProposal,
  requestArtifactUndo,
  retryWorkTask,
  type ReviewAction,
  type ReviewCenterViewModel,
  type TaskControl,
  type TasksViewModel,
  type ViewModelEnvelope,
  type WorkspaceViewModel,
} from "@/tauri";
import { journeyErrorCode as errorText } from "@/ui/journeys/journeyError";
import { buildReadModelErrorEnvelope } from "@/ui/journeys/readOnly/readOnlySpineDataSource";

export type GovernedActionDiagnostic = {
  id: "workspace_view_model" | "review_center_view_model" | "tasks_view_model";
  status: "loaded" | "failed";
  message?: string;
};

export type GovernedActionSnapshot = {
  workspaceEnvelope: ViewModelEnvelope<WorkspaceViewModel>;
  reviewEnvelope: ViewModelEnvelope<ReviewCenterViewModel>;
  tasksEnvelope: ViewModelEnvelope<TasksViewModel>;
  diagnostics: GovernedActionDiagnostic[];
};

export interface GovernedActionDataSource {
  load(): Promise<GovernedActionSnapshot>;
  dispatchReviewAction(action: ReviewAction): Promise<void>;
  editLifeModelLearningProposal(proposalId: string, statement: string): Promise<void>;
  resumeTask(control: TaskControl): Promise<void>;
  dispatchTaskControl(control: TaskControl): Promise<void>;
  requestArtifactUndo(artifactId: string): Promise<void>;
}

export function buildGovernedActionErrorSnapshot(error: unknown): GovernedActionSnapshot {
  const message = errorText(error);
  return {
    workspaceEnvelope: buildReadModelErrorEnvelope(
      "WorkspaceViewModel",
      "workspace_view_model.load_failed",
      `WorkspaceViewModel could not be loaded: ${message}`
    ),
    reviewEnvelope: buildReadModelErrorEnvelope(
      "ReviewCenterViewModel",
      "review_center_view_model.load_failed",
      `ReviewCenterViewModel could not be loaded: ${message}`
    ),
    tasksEnvelope: buildReadModelErrorEnvelope(
      "TasksViewModel",
      "tasks_view_model.load_failed",
      `TasksViewModel could not be loaded: ${message}`
    ),
    diagnostics: [
      { id: "workspace_view_model", status: "failed", message },
      { id: "review_center_view_model", status: "failed", message },
      { id: "tasks_view_model", status: "failed", message },
    ],
  };
}

function settledEnvelope<T>(
  result: PromiseSettledResult<ViewModelEnvelope<T>>,
  targetRef: string,
  code: string
): ViewModelEnvelope<T> {
  return result.status === "fulfilled"
    ? result.value
    : buildReadModelErrorEnvelope<T>(
        targetRef,
        code,
        `${targetRef} could not be loaded: ${errorText(result.reason)}`
      );
}

async function loadGovernedActionSnapshot(): Promise<GovernedActionSnapshot> {
  const [workspaceResult, reviewResult, tasksResult] = await Promise.allSettled([
    getWorkspaceViewModel(),
    getReviewCenterViewModel(),
    getTasksViewModel(),
  ]);

  return {
    workspaceEnvelope: settledEnvelope(
      workspaceResult,
      "WorkspaceViewModel",
      "workspace_view_model.load_failed"
    ),
    reviewEnvelope: settledEnvelope(
      reviewResult,
      "ReviewCenterViewModel",
      "review_center_view_model.load_failed"
    ),
    tasksEnvelope: settledEnvelope(tasksResult, "TasksViewModel", "tasks_view_model.load_failed"),
    diagnostics: [
      workspaceResult.status === "fulfilled"
        ? { id: "workspace_view_model", status: "loaded" }
        : {
            id: "workspace_view_model",
            status: "failed",
            message: errorText(workspaceResult.reason),
          },
      reviewResult.status === "fulfilled"
        ? { id: "review_center_view_model", status: "loaded" }
        : {
            id: "review_center_view_model",
            status: "failed",
            message: errorText(reviewResult.reason),
          },
      tasksResult.status === "fulfilled"
        ? { id: "tasks_view_model", status: "loaded" }
        : {
            id: "tasks_view_model",
            status: "failed",
            message: errorText(tasksResult.reason),
          },
    ],
  };
}

async function dispatchReviewAction(action: ReviewAction): Promise<void> {
  switch (action.kind) {
    case "approve":
      await acceptProposal(action.targetReviewItemId);
      return;
    case "reject":
      await rejectProposal(action.targetReviewItemId);
      return;
    case "later":
      await postponeProposal(action.targetReviewItemId);
      return;
    case "edit":
      throw new Error("review_edit_requires_typed_payload_contract");
    case "apply":
      throw new Error("review_apply_command_unavailable");
    case "revoke":
      throw new Error("review_revoke_command_unavailable");
    case "resume":
      throw new Error("review_resume_requires_task_control_contract");
    case "view_evidence":
      throw new Error("review_evidence_requires_navigation_handler");
  }
}

async function dispatchTaskControl(control: TaskControl): Promise<void> {
  switch (control.kind) {
    case "resume":
      throw new Error("canonical_task_resume_requires_retry_or_review_checkpoint");
    case "cancel":
      await cancelWorkTask(control.targetTaskId);
      return;
    case "retry":
      if (!control.targetActionId) throw new Error("task_retry_target_action_missing");
      await retryWorkTask(control.targetTaskId, control.targetActionId);
      return;
    case "refresh_context":
      throw new Error("canonical_task_refresh_context_unavailable");
    case "open_trace":
    case "open_run":
    case "open_review_item":
    case "view_evidence":
      throw new Error("task_control_requires_navigation_handler");
  }
}

export const tauriGovernedActionDataSource: GovernedActionDataSource = {
  load: loadGovernedActionSnapshot,
  dispatchReviewAction,
  async editLifeModelLearningProposal(proposalId, statement) {
    const receipt = await editLifeModelLearningProposal(proposalId, statement);
    if (
      receipt.proposalId !== proposalId ||
      receipt.status !== "edited_pending_review" ||
      !receipt.resultDocumentDigest ||
      receipt.durableWriteExecuted ||
      !receipt.learning ||
      !receipt.learning.candidateId ||
      receipt.learning.proposalId !== proposalId ||
      receipt.learning.status !== "proposed" ||
      receipt.learning.contentScrubbed ||
      !receipt.learning.correctionObservationId ||
      receipt.learning.canonicalLifeModelChanged
    ) {
      throw new Error("lifemodel_learning_edit_receipt_unverified");
    }
  },
  resumeTask: async control => {
    throw new Error(`canonical_task_resume_unavailable:${control.targetTaskId}`);
  },
  dispatchTaskControl,
  async requestArtifactUndo(artifactId) {
    const receipt = await requestArtifactUndo(artifactId);
    if (
      receipt.artifactId !== artifactId ||
      !receipt.proposalId ||
      receipt.status !== "waiting_review"
    ) {
      throw new Error("artifact_undo_receipt_unverified");
    }
  },
};
