import {
  createChatSession,
  getChatHistory,
  listChatSessions,
  sendMessageV2,
  type ChatSession,
  type MainChatMessageOptions,
  type SendMessageResult,
} from "@/tauri";
import type { ChatMessage } from "@/types";

export interface WorkspaceConversationDataSource {
  listSessions(): Promise<ChatSession[]>;
  loadHistory(sessionId: string): Promise<ChatMessage[]>;
  createSession(sessionId: string, title: string): Promise<void>;
  sendTurn(
    sessionId: string,
    messages: ChatMessage[],
    options: MainChatMessageOptions
  ): Promise<SendMessageResult>;
}

export const tauriWorkspaceConversationDataSource: WorkspaceConversationDataSource = {
  listSessions: listChatSessions,
  loadHistory: getChatHistory,
  createSession: createChatSession,
  sendTurn: sendMessageV2,
};
