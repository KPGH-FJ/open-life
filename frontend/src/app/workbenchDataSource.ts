import {
  type ReviewAction,
  type ReviewItem,
  type ReviewCenterViewModel,
  type ProviderPrivacyBoundarySummary,
  type TaskControl,
  type TaskViewModelItem,
  type TasksViewModel,
  type ViewModelEnvelope,
  type WorkspaceViewModel,
} from "@/tauri";
import { editLifeModelLearningProposal } from "@/ipc/personalIntelligence";
import { acceptProposal, postponeProposal, rejectProposal } from "@/ipc/review";
import {
  cancelWorkTask,
  exportArtifactResult,
  getWorkbenchViewModel,
  openArtifactResult,
  requestArtifactUndo,
  retryWorkTask,
} from "@/ipc/work";
import { productErrorCode as errorText } from "@/shared/productError";
import { buildReadModelErrorEnvelope } from "@/shared/readModelEnvelope";

export type WorkbenchDiagnostic = {
  id:
    | "workspace_view_model"
    | "review_center_view_model"
    | "tasks_view_model"
    | "provider_privacy_boundary";
  status: "loaded" | "failed";
  message?: string;
};

export type WorkbenchSnapshot = {
  capturedAt: string | null;
  workspaceEnvelope: ViewModelEnvelope<WorkspaceViewModel>;
  reviewEnvelope: ViewModelEnvelope<ReviewCenterViewModel>;
  tasksEnvelope: ViewModelEnvelope<TasksViewModel>;
  boundaryEnvelope: ViewModelEnvelope<ProviderPrivacyBoundarySummary>;
  diagnostics: WorkbenchDiagnostic[];
};

export interface WorkbenchDataSource {
  load(conversationId?: string | null): Promise<WorkbenchSnapshot>;
  dispatchReviewAction(action: ReviewAction): Promise<void>;
  editLifeModelLearningProposal(proposalId: string, statement: string): Promise<void>;
  dispatchTaskControl(control: TaskControl): Promise<void>;
  cancelTask(taskId: string): Promise<void>;
  requestArtifactUndo(artifactId: string): Promise<void>;
  openArtifactResult(artifactId: string, version: number): Promise<void>;
  exportArtifactResult(artifactId: string, version: number): Promise<string | null>;
}

export function scopedTasks(snapshot: WorkbenchSnapshot | null): TaskViewModelItem[] {
  if (!snapshot || !["ready", "stale"].includes(snapshot.tasksEnvelope.status)) return [];
  const conversationId = snapshot.workspaceEnvelope.data?.selectedConversationId ?? null;
  return (snapshot.tasksEnvelope.data?.items ?? []).filter(
    task => !conversationId || task.conversationId === conversationId
  );
}

export function activeScopedTask(snapshot: WorkbenchSnapshot | null): TaskViewModelItem | null {
  const conversationId = snapshot?.workspaceEnvelope.data?.selectedConversationId?.trim();
  if (!conversationId) return null;
  return (
    scopedTasks(snapshot).find(task =>
      ["running", "waiting_review", "waiting_permission", "blocked"].includes(task.lifecycleStatus)
    ) ?? null
  );
}

export function scopedReviewItems(snapshot: WorkbenchSnapshot | null): ReviewItem[] {
  const items = allReviewItems(snapshot);
  const reviewIds = new Set(
    scopedTasks(snapshot).flatMap(task => task.pendingReviewItemRefs.map(ref => ref.id))
  );
  return items.filter(item => reviewIds.has(item.id));
}

export function allReviewItems(snapshot: WorkbenchSnapshot | null): ReviewItem[] {
  if (!snapshot || !["ready", "stale"].includes(snapshot.reviewEnvelope.status)) return [];
  return snapshot.reviewEnvelope.data?.items ?? [];
}

export function buildWorkbenchErrorSnapshot(error: unknown): WorkbenchSnapshot {
  const message = errorText(error);
  return {
    capturedAt: null,
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
      { id: "workspace_view_model", status: "failed", message },
      { id: "review_center_view_model", status: "failed", message },
      { id: "tasks_view_model", status: "failed", message },
      { id: "provider_privacy_boundary", status: "failed", message },
    ],
  };
}

async function loadWorkbenchSnapshot(conversationId?: string | null): Promise<WorkbenchSnapshot> {
  const workbench = await getWorkbenchViewModel(conversationId);

  return {
    capturedAt: workbench.capturedAt,
    workspaceEnvelope: workbench.workspace,
    reviewEnvelope: workbench.review,
    tasksEnvelope: workbench.tasks,
    boundaryEnvelope: workbench.providerBoundary,
    diagnostics: [
      laneDiagnostic("workspace_view_model", workbench.workspace),
      laneDiagnostic("review_center_view_model", workbench.review),
      laneDiagnostic("tasks_view_model", workbench.tasks),
      laneDiagnostic("provider_privacy_boundary", workbench.providerBoundary),
    ],
  };
}

function laneDiagnostic(
  id: WorkbenchDiagnostic["id"],
  envelope: ViewModelEnvelope<unknown>
): WorkbenchDiagnostic {
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

async function cancelTask(taskId: string): Promise<void> {
  await cancelWorkTask(taskId);
}

export const tauriWorkbenchDataSource: WorkbenchDataSource = {
  load: loadWorkbenchSnapshot,
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
  cancelTask,
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
  async openArtifactResult(artifactId, version) {
    await openArtifactResult(artifactId, version);
  },
  async exportArtifactResult(artifactId, version) {
    const receipt = await exportArtifactResult(artifactId, version);
    if (receipt.cancelled) return null;
    if (!receipt.savedPath || !receipt.contentDigest?.startsWith("sha256:")) {
      throw new Error("artifact_export_receipt_unverified");
    }
    return receipt.savedPath;
  },
};
