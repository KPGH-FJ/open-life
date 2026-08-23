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

export async function cancelWorkTask(taskId: string): Promise<CanonicalWorkControlResult> {
  return safeInvoke<CanonicalWorkControlResult>("cancel_work_task", { taskId });
}

export async function retryWorkTask(
  taskId: string,
  priorRunId: string
): Promise<SendMessageResult> {
  return safeInvoke<SendMessageResult>("retry_work_task", {
    taskId,
    priorRunId,
    newRunId: crypto.randomUUID(),
    newTurnId: crypto.randomUUID(),
  });
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
