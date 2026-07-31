import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  cancelMainChatAgentTask,
  createChatSession,
  deleteChatSession,
  getChatHistory,
  listChatSessions,
  renameChatSession,
  startStreamMessage,
  type ChatSession,
  type MainChatAgentTaskState,
  type MainChatMessageOptions,
  type StreamMessageChunkPayload,
  type StreamMessageDonePayload,
  type StreamMessageStartPayload,
} from "@/tauri";
import type { ChatMessage } from "@/types";

export type WorkspaceStreamEvents = {
  onStart(payload: StreamMessageStartPayload): void;
  onChunk(payload: StreamMessageChunkPayload): void;
};

export interface WorkspaceConversationDataSource {
  listSessions(): Promise<ChatSession[]>;
  loadHistory(sessionId: string): Promise<ChatMessage[]>;
  createSession(sessionId: string, title: string): Promise<void>;
  renameSession(sessionId: string, title: string): Promise<void>;
  deleteSession(sessionId: string): Promise<void>;
  streamTurn(
    sessionId: string,
    messages: ChatMessage[],
    options: MainChatMessageOptions,
    events: WorkspaceStreamEvents
  ): Promise<StreamMessageDonePayload>;
  cancelTask(taskSessionId: string): Promise<MainChatAgentTaskState>;
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
  listSessions: listChatSessions,
  loadHistory: getChatHistory,
  createSession: createChatSession,
  renameSession: renameChatSession,
  deleteSession: deleteChatSession,
  streamTurn,
  cancelTask: cancelMainChatAgentTask,
};
