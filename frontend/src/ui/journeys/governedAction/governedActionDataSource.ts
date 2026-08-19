import {
  acceptProposal,
  cancelWorkTask,
  editLifeModelLearningProposal,
  getWorkbenchViewModel,
  postponeProposal,
  rejectProposal,
  requestArtifactUndo,
  retryWorkTask,
  type ReviewAction,
  type ReviewCenterViewModel,
  type ConversationViewModel,
  type ProviderPrivacyBoundarySummary,
  type TaskControl,
  type TasksViewModel,
  type ViewModelEnvelope,
  type WorkspaceViewModel,
} from "@/tauri";
import { journeyErrorCode as errorText } from "@/ui/journeys/journeyError";
import { buildReadModelErrorEnvelope } from "@/ui/journeys/productWorkbench/productBoundaryDataSource";

export type GovernedActionDiagnostic = {
  id:
    | "conversation_view_model"
    | "workspace_view_model"
    | "review_center_view_model"
    | "tasks_view_model"
    | "provider_privacy_boundary";
  status: "loaded" | "failed";
  message?: string;
};

export type GovernedActionSnapshot = {
  capturedAt: string | null;
  conversationEnvelope: ViewModelEnvelope<ConversationViewModel>;
  workspaceEnvelope: ViewModelEnvelope<WorkspaceViewModel>;
  reviewEnvelope: ViewModelEnvelope<ReviewCenterViewModel>;
  tasksEnvelope: ViewModelEnvelope<TasksViewModel>;
  boundaryEnvelope: ViewModelEnvelope<ProviderPrivacyBoundarySummary>;
  diagnostics: GovernedActionDiagnostic[];
};

export interface GovernedActionDataSource {
  load(conversationId?: string | null): Promise<GovernedActionSnapshot>;
  dispatchReviewAction(action: ReviewAction): Promise<void>;
  editLifeModelLearningProposal(proposalId: string, statement: string): Promise<void>;
  dispatchTaskControl(control: TaskControl): Promise<void>;
  requestArtifactUndo(artifactId: string): Promise<void>;
}

export function buildGovernedActionErrorSnapshot(error: unknown): GovernedActionSnapshot {
  const message = errorText(error);
  return {
    capturedAt: null,
    conversationEnvelope: buildReadModelErrorEnvelope(
      "ConversationViewModel",
      "conversation_view_model.load_failed",
      `ConversationViewModel could not be loaded: ${message}`
    ),
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
    boundaryEnvelope: buildReadModelErrorEnvelope(
      "ProviderPrivacyBoundarySummary",
      "provider_privacy_boundary.load_failed",
      `ProviderPrivacyBoundarySummary could not be loaded: ${message}`
    ),
    diagnostics: [
      { id: "conversation_view_model", status: "failed", message },
      { id: "workspace_view_model", status: "failed", message },
      { id: "review_center_view_model", status: "failed", message },
      { id: "tasks_view_model", status: "failed", message },
      { id: "provider_privacy_boundary", status: "failed", message },
    ],
  };
}

async function loadGovernedActionSnapshot(
  conversationId?: string | null
): Promise<GovernedActionSnapshot> {
  const workbench = await getWorkbenchViewModel(conversationId);

  return {
    capturedAt: workbench.capturedAt,
    conversationEnvelope: workbench.conversation,
    workspaceEnvelope: workbench.workspace,
    reviewEnvelope: workbench.review,
    tasksEnvelope: workbench.tasks,
    boundaryEnvelope: workbench.providerBoundary,
    diagnostics: [
      laneDiagnostic("conversation_view_model", workbench.conversation),
      laneDiagnostic("workspace_view_model", workbench.workspace),
      laneDiagnostic("review_center_view_model", workbench.review),
      laneDiagnostic("tasks_view_model", workbench.tasks),
      laneDiagnostic("provider_privacy_boundary", workbench.providerBoundary),
    ],
  };
}

function laneDiagnostic(
  id: GovernedActionDiagnostic["id"],
  envelope: ViewModelEnvelope<unknown>
): GovernedActionDiagnostic {
  return envelope.status === "error"
    ? { id, status: "failed", message: envelope.warnings?.[0]?.message ?? "unknown" }
    : { id, status: "loaded" };
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
