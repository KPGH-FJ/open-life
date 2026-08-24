import type {
  CanonicalWorkControlResult,
  ExportArtifactResult,
  SendMessageResult,
  WorkbenchViewModel,
} from "../tauri";
import { safeInvoke } from "./invoke";

export async function openExternalHttpsSource(url: string): Promise<void> {
  return safeInvoke("open_external_https_source", { url });
}

export async function openArtifactResult(artifactId: string, version: number): Promise<void> {
  return safeInvoke("open_artifact_result", { artifactId, version });
}

export async function exportArtifactResult(
  artifactId: string,
  version: number
): Promise<ExportArtifactResult> {
  return safeInvoke("export_artifact_result", { artifactId, version });
}

export async function stopWorkRun(
  taskId: string,
  runId: string
): Promise<CanonicalWorkControlResult> {
  return safeInvoke<CanonicalWorkControlResult>("stop_work_run", { taskId, runId });
}

async function restartWorkTask(
  command: "retry_work_task" | "resume_work_task",
  taskId: string,
  priorRunId: string
): Promise<SendMessageResult> {
  return safeInvoke<SendMessageResult>(command, {
    taskId,
    priorRunId,
    newRunId: crypto.randomUUID(),
    newTurnId: crypto.randomUUID(),
  });
}

export async function retryWorkTask(
  taskId: string,
  priorRunId: string
): Promise<SendMessageResult> {
  return restartWorkTask("retry_work_task", taskId, priorRunId);
}

export async function resumeWorkTask(
  taskId: string,
  priorRunId: string
): Promise<SendMessageResult> {
  return restartWorkTask("resume_work_task", taskId, priorRunId);
}

export async function reviseWorkArtifact(
  taskId: string,
  artifactId: string,
  baseVersion: number,
  instruction: string
): Promise<SendMessageResult> {
  const newRunId = crypto.randomUUID();
  const newTurnId = crypto.randomUUID();
  const receipt = await safeInvoke<SendMessageResult>("revise_work_artifact", {
    taskId,
    artifactId,
    baseVersion,
    instruction,
    newRunId,
    newTurnId,
  });
  if (receipt.run_id !== newRunId) {
    throw new Error("artifact_revision_run_identity_mismatch");
  }
  return receipt;
}

export async function getWorkbenchViewModel(
  conversationId?: string | null
): Promise<WorkbenchViewModel> {
  return safeInvoke<WorkbenchViewModel>("get_workbench_view_model", {
    ...(conversationId == null || conversationId === "" ? {} : { conversationId }),
  });
}

export async function requestArtifactUndo(artifactId: string): Promise<{
  artifactId: string;
  proposalId: string;
  status: "waiting_review";
}> {
  return safeInvoke("request_artifact_undo", { artifactId });
}
