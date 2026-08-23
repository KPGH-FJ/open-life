import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  cancelChatTurn,
  assignConversationProject,
  createChatSession,
  createProject,
  deleteChatSession,
  detachResourceFromTurn,
  getConversationViewModel,
  listMainChatSkills,
  listMainChatToolCandidates,
  pickAndImportResources,
  renameChatSession,
  selectMainChatSkill,
  setConversationMemoryMode,
  clearMainChatSkill,
  startStreamMessage,
  submitMainChatTaskSteering,
} from "@/ipc/conversation";
import type {
  ConversationViewModel,
  ConversationMemoryMode,
  ResourceDetachReceipt,
  ResourceImportSelectionResult,
  MainChatMessageOptions,
  ProjectRecord,
  MainChatSelectedSkill,
  MainChatSkillSummary,
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

export interface ConversationDataSource {
  loadConversation(conversationId?: string): Promise<ConversationViewModel>;
  createSession(sessionId: string, title: string): Promise<void>;
  createProject?(projectId: string, name: string): Promise<ProjectRecord>;
  assignProject?(conversationId: string, projectId: string | null): Promise<void>;
  setMemoryMode?(conversationId: string, mode: ConversationMemoryMode): Promise<void>;
  renameSession(sessionId: string, title: string): Promise<void>;
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
  createProject,
  assignProject: assignConversationProject,
  setMemoryMode: setConversationMemoryMode,
  renameSession: renameChatSession,
  deleteSession: deleteChatSession,
  pickResources: pickAndImportResources,
  detachResource: detachResourceFromTurn,
  streamTurn,
  cancelChatTurn,
  steerTask: submitMainChatTaskSteering,
  listSkills: listMainChatSkills,
  selectSkill: selectMainChatSkill,
  clearSkill: clearMainChatSkill,
  listToolCandidates: listMainChatToolCandidates,
};
