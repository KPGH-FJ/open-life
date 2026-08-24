import type { ChatMessage } from "../types";
import type {
  CancelChatTurnResult,
  ConversationMemoryMode,
  ConversationViewModel,
  MainChatMessageOptions,
  MainChatSelectedSkill,
  MainChatSkillDetail,
  MainChatSkillSummary,
  MainChatToolCandidateList,
  ProjectDirectoryCreationResult,
  ProjectRecord,
  ResourceDetachReceipt,
  ResourceImportSelectionResult,
  StreamMessageDonePayload,
  SubmitMainChatSteeringResponse,
} from "../tauri";
import { safeInvoke } from "./invoke";

function sessionArgs(sessionId: string): { sessionId: string } {
  return { sessionId };
}

function selectedSkillArgs(selectedSkillId?: string): { selectedSkillId: string } | undefined {
  const trimmed = selectedSkillId?.trim();
  return trimmed ? { selectedSkillId: trimmed } : undefined;
}

export async function listMainChatSkills(sessionId?: string): Promise<MainChatSkillSummary[]> {
  return safeInvoke<MainChatSkillSummary[]>("list_main_chat_skills", {
    ...(sessionId === undefined ? {} : { sessionId }),
  });
}

export async function getMainChatSkillDetail(skillId: string): Promise<MainChatSkillDetail> {
  return safeInvoke<MainChatSkillDetail>("get_main_chat_skill_detail", { skillId });
}

export async function selectMainChatSkill(
  sessionId: string,
  skillId: string
): Promise<MainChatSelectedSkill> {
  return safeInvoke<MainChatSelectedSkill>("select_main_chat_skill", { sessionId, skillId });
}

export async function clearMainChatSkill(sessionId: string): Promise<MainChatSelectedSkill> {
  return safeInvoke<MainChatSelectedSkill>("clear_main_chat_skill", { sessionId });
}

export async function listMainChatToolCandidates(
  taskId?: string
): Promise<MainChatToolCandidateList> {
  return safeInvoke<MainChatToolCandidateList>("list_main_chat_tool_candidates", { taskId });
}

export async function cancelChatTurn(
  conversationId: string,
  turnId: string
): Promise<CancelChatTurnResult> {
  return safeInvoke<CancelChatTurnResult>("cancel_chat_turn", { conversationId, turnId });
}

export async function submitMainChatTaskSteering(request: {
  steeringId: string;
  taskId: string;
  runId: string;
  sessionId: string;
  content: string;
}): Promise<SubmitMainChatSteeringResponse> {
  return safeInvoke<SubmitMainChatSteeringResponse>("submit_main_chat_task_steering", request);
}

export async function startStreamMessage(
  sessionId: string,
  messages: ChatMessage[],
  options: MainChatMessageOptions
): Promise<StreamMessageDonePayload> {
  return safeInvoke<StreamMessageDonePayload>("start_stream_message", {
    args: {
      operationId: options.operationId,
      ...sessionArgs(sessionId),
      messages,
      mode: options.mode ?? "chat",
      taskId: options.taskId,
      runId: options.runId,
      providerProfileId: options.providerProfileId,
      reasoningEffort: options.reasoningEffort,
      executionMode: options.executionMode,
      ...selectedSkillArgs(options.selectedSkillId),
    },
  });
}

export async function pickAndImportResources(
  importOperationId: string,
  turnOperationId: string
): Promise<ResourceImportSelectionResult> {
  return safeInvoke<ResourceImportSelectionResult>("pick_and_import_resources", {
    importOperationId,
    turnOperationId,
  });
}

export async function detachResourceFromTurn(
  operationId: string,
  turnOperationId: string,
  resourceId: string
): Promise<ResourceDetachReceipt> {
  return safeInvoke<ResourceDetachReceipt>("detach_resource_from_turn", {
    operationId,
    turnOperationId,
    resourceId,
  });
}

export async function getConversationViewModel(
  conversationId?: string
): Promise<ConversationViewModel> {
  return safeInvoke<ConversationViewModel>("get_conversation_view_model", { conversationId });
}

export async function createChatSession(
  sessionId: string,
  title: string,
  admission: {
    projectId: string | null;
    memoryMode: ConversationMemoryMode;
    selectedSkillId: string | null;
  }
): Promise<void> {
  return safeInvoke("create_chat_session", {
    ...sessionArgs(sessionId),
    title,
    projectId: admission.projectId,
    selectedSkillId: admission.selectedSkillId,
    memoryMode: admission.memoryMode,
  });
}

export async function createProjectFromDirectory(
  projectId: string,
  name?: string
): Promise<ProjectDirectoryCreationResult> {
  return safeInvoke<ProjectDirectoryCreationResult>("create_project_from_directory", {
    projectId,
    name: name?.trim() || null,
  });
}

export async function bindProjectDirectory(
  projectId: string,
  expectedRevision: number
): Promise<ProjectDirectoryCreationResult> {
  return safeInvoke<ProjectDirectoryCreationResult>("bind_project_directory", {
    projectId,
    expectedRevision,
  });
}

export async function addProjectReadRoot(
  projectId: string,
  expectedRevision: number
): Promise<ProjectDirectoryCreationResult> {
  return safeInvoke<ProjectDirectoryCreationResult>("add_project_read_root", {
    projectId,
    expectedRevision,
  });
}

export async function removeProjectReadRoot(
  projectId: string,
  rootId: string,
  expectedRevision: number
): Promise<ProjectRecord> {
  return safeInvoke<ProjectRecord>("remove_project_read_root", {
    projectId,
    rootId,
    expectedRevision,
  });
}

export async function updateProjectName(
  projectId: string,
  name: string,
  expectedRevision: number
): Promise<ProjectRecord> {
  return safeInvoke<ProjectRecord>("update_project_name", { projectId, name, expectedRevision });
}

export async function archiveProject(
  projectId: string,
  expectedRevision: number
): Promise<ProjectRecord> {
  return safeInvoke<ProjectRecord>("archive_project", { projectId, expectedRevision });
}

export async function restoreProject(
  projectId: string,
  expectedRevision: number
): Promise<ProjectRecord> {
  return safeInvoke<ProjectRecord>("restore_project", { projectId, expectedRevision });
}

export async function deleteProject(projectId: string, expectedRevision: number): Promise<void> {
  return safeInvoke("delete_project", { projectId, expectedRevision });
}

export async function assignConversationProject(
  conversationId: string,
  projectId: string | null
): Promise<void> {
  return safeInvoke("assign_conversation_project", { conversationId, projectId });
}

export async function selectNewConversationProject(projectId: string | null): Promise<void> {
  return safeInvoke("select_new_conversation_project", { projectId });
}

export async function setConversationMemoryMode(
  conversationId: string,
  mode: ConversationMemoryMode
): Promise<void> {
  return safeInvoke("set_conversation_memory_mode", { conversationId, mode });
}

export async function renameChatSession(sessionId: string, title: string): Promise<void> {
  return safeInvoke("rename_chat_session", { ...sessionArgs(sessionId), title });
}

export async function archiveChatSession(sessionId: string): Promise<void> {
  return safeInvoke("archive_chat_session", sessionArgs(sessionId));
}

export async function restoreChatSession(sessionId: string): Promise<void> {
  return safeInvoke("restore_chat_session", sessionArgs(sessionId));
}

export async function deleteChatSession(sessionId: string): Promise<void> {
  return safeInvoke("delete_chat_session", sessionArgs(sessionId));
}
