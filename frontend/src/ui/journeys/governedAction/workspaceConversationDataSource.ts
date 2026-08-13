import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  cancelChatTurn,
  cancelWorkTask,
  assignConversationProject,
  createChatSession,
  createProject,
  deactivateMarkdownMemoryFileProposal,
  deleteChatSession,
  detachResourceFromTurn,
  draftMarkdownMemoryFileProposal,
  getConversationViewModel,
  getMarkdownMemoryViewModel,
  listMainChatSkills,
  listMainChatToolCandidates,
  pickAndImportResources,
  renameChatSession,
  selectMarkdownMemoryRoot,
  selectMainChatSkill,
  clearMainChatSkill,
  startStreamMessage,
  submitMainChatTaskSteering,
  type ChatSession,
  type ConversationViewModel,
  type ChatLifeModelInfluenceSnapshot,
  type ResourceDetachReceipt,
  type ResourceImportSelectionResult,
  type MainChatMessageOptions,
  type ProjectRecord,
  type MainChatSelectedSkill,
  type MainChatSkillSummary,
  type MainChatToolCandidateList,
  type MarkdownMemoryProposalReceipt,
  type MarkdownMemoryRootSelection,
  type MarkdownMemoryScope,
  type MarkdownMemoryViewModel,
  type StreamMessageChunkPayload,
  type StreamMessageDonePayload,
  type StreamMessageStartPayload,
  type SubmitMainChatSteeringResponse,
} from "@/tauri";
import type { ChatMessage } from "@/types";

export type WorkspaceStreamEvents = {
  onStart(payload: StreamMessageStartPayload): void;
  onChunk(payload: StreamMessageChunkPayload): void;
};

export interface WorkspaceConversationDataSource {
  loadConversation?(conversationId?: string): Promise<ConversationViewModel>;
  /** Test/Work compatibility only; canonical Chat UI reads loadConversation. */
  listSessions?(): Promise<ChatSession[]>;
  loadHistory?(sessionId: string): Promise<ChatMessage[]>;
  loadLifeModelInfluence?(sessionId: string): Promise<ChatLifeModelInfluenceSnapshot | null>;
  createSession(sessionId: string, title: string): Promise<void>;
  createProject?(projectId: string, name: string): Promise<ProjectRecord>;
  assignProject?(conversationId: string, projectId: string | null): Promise<void>;
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
  cancelWorkTask?(taskId: string): Promise<unknown>;
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
  listToolCandidates?(taskSessionId?: string): Promise<MainChatToolCandidateList>;
  loadMarkdownMemory?(): Promise<MarkdownMemoryViewModel>;
  selectMarkdownMemoryRoot?(scope: MarkdownMemoryScope): Promise<MarkdownMemoryRootSelection>;
  draftMarkdownMemoryFileProposal?(request: {
    scope: MarkdownMemoryScope;
    relativePath: string;
    content: string;
    expectedCurrentDigest?: string;
  }): Promise<MarkdownMemoryProposalReceipt>;
  deactivateMarkdownMemoryFileProposal?(request: {
    scope: MarkdownMemoryScope;
    relativePath: string;
    expectedCurrentDigest: string;
  }): Promise<MarkdownMemoryProposalReceipt>;
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

export const tauriWorkspaceConversationDataSource: WorkspaceConversationDataSource = {
  loadConversation: getConversationViewModel,
  createSession: createChatSession,
  createProject,
  assignProject: assignConversationProject,
  renameSession: renameChatSession,
  deleteSession: deleteChatSession,
  pickResources: pickAndImportResources,
  detachResource: detachResourceFromTurn,
  streamTurn,
  cancelChatTurn,
  cancelWorkTask,
  steerTask: submitMainChatTaskSteering,
  listSkills: listMainChatSkills,
  selectSkill: selectMainChatSkill,
  clearSkill: clearMainChatSkill,
  listToolCandidates: listMainChatToolCandidates,
  loadMarkdownMemory: getMarkdownMemoryViewModel,
  selectMarkdownMemoryRoot,
  draftMarkdownMemoryFileProposal,
  deactivateMarkdownMemoryFileProposal,
};
