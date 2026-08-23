import type { ChatMessage } from "../types";
import type {
  CancelChatTurnResult,
  ConversationMemoryMode,
  ConversationViewModel,
  MainChatMessageOptions,
  MainChatSelectedSkill,
  MainChatSkillSummary,
  MainChatToolCandidateList,
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

export async function createChatSession(sessionId: string, title: string): Promise<void> {
  return safeInvoke("create_chat_session", { ...sessionArgs(sessionId), title });
}

export async function createProject(projectId: string, name: string): Promise<ProjectRecord> {
  return safeInvoke<ProjectRecord>("create_project", { projectId, name });
}

export async function assignConversationProject(
  conversationId: string,
  projectId: string | null
): Promise<void> {
  return safeInvoke("assign_conversation_project", { conversationId, projectId });
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

export async function deleteChatSession(sessionId: string): Promise<void> {
  return safeInvoke("delete_chat_session", sessionArgs(sessionId));
}
