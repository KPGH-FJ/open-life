import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  addProjectReadRoot,
  archiveChatSession,
  archiveProject,
  bindProjectDirectory,
  cancelChatTurn,
  assignConversationProject,
  createChatSession,
  createProjectFromDirectory,
  deleteChatSession,
  deleteProject,
  detachResourceFromTurn,
  getConversationViewModel,
  listMainChatSkills,
  getMainChatSkillDetail,
  listMainChatToolCandidates,
  pickAndImportResources,
  renameChatSession,
  removeProjectReadRoot,
  restoreChatSession,
  restoreProject,
  selectMainChatSkill,
  selectNewConversationProject,
  setConversationMemoryMode,
  clearMainChatSkill,
  startStreamMessage,
  submitMainChatTaskSteering,
  updateProjectName,
} from "@/ipc/conversation";
import type {
  ConversationViewModel,
  ConversationMemoryMode,
  ResourceDetachReceipt,
  ResourceImportSelectionResult,
  MainChatMessageOptions,
  ProjectDirectoryCreationResult,
  ProjectRecord,
  MainChatSelectedSkill,
  MainChatSkillSummary,
  MainChatSkillDetail,
  MainChatToolCandidateList,
  StreamMessageChunkPayload,
  StreamMessageDonePayload,
  StreamMessageStartPayload,
  SubmitMainChatSteeringResponse,
} from "@/tauri";
import type { ChatMessage } from "@/types";

export type WorkspaceStreamEvents = {
  onStart(payload: StreamMessageStartPayload): void;
  onChunk(payload: StreamMessageChunkPayload): void;
};

export type NewConversationAdmission = {
  projectId: string | null;
  memoryMode: ConversationMemoryMode;
  selectedSkillId: string | null;
};

export interface ConversationDataSource {
  loadConversation(conversationId?: string): Promise<ConversationViewModel>;
  createSession(
    sessionId: string,
    title: string,
    admission: NewConversationAdmission
  ): Promise<void>;
  createProject?(projectId: string, name?: string): Promise<ProjectDirectoryCreationResult>;
  bindProjectDirectory?(
    projectId: string,
    expectedRevision: number
  ): Promise<ProjectDirectoryCreationResult>;
  addProjectReadRoot?(
    projectId: string,
    expectedRevision: number
  ): Promise<ProjectDirectoryCreationResult>;
  removeProjectReadRoot?(
    projectId: string,
    rootId: string,
    expectedRevision: number
  ): Promise<ProjectRecord>;
  updateProjectName?(
    projectId: string,
    name: string,
    expectedRevision: number
  ): Promise<ProjectRecord>;
  archiveProject?(projectId: string, expectedRevision: number): Promise<ProjectRecord>;
  restoreProject?(projectId: string, expectedRevision: number): Promise<ProjectRecord>;
  deleteProject?(projectId: string, expectedRevision: number): Promise<void>;
  assignProject?(conversationId: string, projectId: string | null): Promise<void>;
  selectProjectForNewConversation?(projectId: string | null): Promise<void>;
  setMemoryMode?(conversationId: string, mode: ConversationMemoryMode): Promise<void>;
  renameSession(sessionId: string, title: string): Promise<void>;
  archiveSession?(sessionId: string): Promise<void>;
  restoreSession?(sessionId: string): Promise<void>;
  deleteSession(sessionId: string): Promise<void>;
  pickResources(
    importOperationId: string,
    turnOperationId: string
  ): Promise<ResourceImportSelectionResult>;
  detachResource(
    operationId: string,
    turnOperationId: string,
    resourceId: string
  ): Promise<ResourceDetachReceipt>;
  streamTurn(
    sessionId: string,
    messages: ChatMessage[],
    options: MainChatMessageOptions,
    events: WorkspaceStreamEvents
  ): Promise<StreamMessageDonePayload>;
  cancelChatTurn?(conversationId: string, turnId: string): Promise<unknown>;
  steerTask?(request: {
    steeringId: string;
    taskId: string;
    runId: string;
    sessionId: string;
    content: string;
  }): Promise<SubmitMainChatSteeringResponse>;
  listSkills?(sessionId?: string): Promise<MainChatSkillSummary[]>;
  getSkillDetail?(skillId: string): Promise<MainChatSkillDetail>;
  selectSkill?(sessionId: string, skillId: string): Promise<MainChatSelectedSkill>;
  clearSkill?(sessionId: string): Promise<MainChatSelectedSkill>;
  listToolCandidates?(taskId?: string): Promise<MainChatToolCandidateList>;
}

function matchesActiveStream(
  payload: { session_id: string; operation_id: string },
  sessionId: string,
  operationId: string
): boolean {
  return payload.session_id === sessionId && payload.operation_id === operationId;
}

async function streamTurn(
  sessionId: string,
  messages: ChatMessage[],
  options: MainChatMessageOptions,
  events: WorkspaceStreamEvents
): Promise<StreamMessageDonePayload> {
  const unlisten: UnlistenFn[] = [];
  try {
    unlisten.push(
      await listen<StreamMessageStartPayload>("stream-message-start", event => {
        if (matchesActiveStream(event.payload, sessionId, options.operationId)) {
          events.onStart(event.payload);
        }
      })
    );
    unlisten.push(
      await listen<StreamMessageChunkPayload>("stream-message-chunk", event => {
        if (matchesActiveStream(event.payload, sessionId, options.operationId)) {
          events.onChunk(event.payload);
        }
      })
    );
    return await startStreamMessage(sessionId, messages, options);
  } finally {
    for (const stopListening of unlisten) stopListening();
  }
}

export const tauriConversationDataSource: ConversationDataSource = {
  loadConversation: getConversationViewModel,
  createSession: createChatSession,
  createProject: createProjectFromDirectory,
  bindProjectDirectory,
  addProjectReadRoot,
  removeProjectReadRoot,
  updateProjectName,
  archiveProject,
  restoreProject,
  deleteProject,
  assignProject: assignConversationProject,
  selectProjectForNewConversation: selectNewConversationProject,
  setMemoryMode: setConversationMemoryMode,
  renameSession: renameChatSession,
  archiveSession: archiveChatSession,
  restoreSession: restoreChatSession,
  deleteSession: deleteChatSession,
  pickResources: pickAndImportResources,
  detachResource: detachResourceFromTurn,
  streamTurn,
  cancelChatTurn,
  steerTask: submitMainChatTaskSteering,
  listSkills: listMainChatSkills,
  getSkillDetail: getMainChatSkillDetail,
  selectSkill: selectMainChatSkill,
  clearSkill: clearMainChatSkill,
  listToolCandidates: listMainChatToolCandidates,
};
