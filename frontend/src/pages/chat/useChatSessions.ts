import { useCallback, useState } from "react";
import {
  listChatSessions,
  createChatSession,
  renameChatSession,
  deleteChatSession,
  type ChatSession,
} from "../../tauri";
import { logError } from "../../utils/logger";

function generateSessionId() {
  return "sess_" + Math.random().toString(36).slice(2) + Date.now().toString(36);
}

export function useChatSessions() {
  const [sessions, setSessions] = useState<ChatSession[]>([]);
  const [currentSessionId, setCurrentSessionId] = useState<string>("default");
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editingTitle, setEditingTitle] = useState("");

  const loadSessions = useCallback(async (activeSessionId?: string) => {
    try {
      const list = await listChatSessions();
      setSessions(list);
      if (list.length > 0) {
        const sid = activeSessionId ?? currentSessionId;
        setCurrentSessionId(list.find(s => s.session_id === sid) ? sid : list[0].session_id);
      }
    } catch (e) {
      logError("加载会话列表失败", e);
    }
  }, []);

  const handleNewSession = useCallback(async () => {
    const id = generateSessionId();
    try {
      await createChatSession(id, "新会话");
      await loadSessions();
      setCurrentSessionId(id);
    } catch (e) {
      logError("创建会话失败", e);
    }
  }, [loadSessions]);

  const handleDeleteSession = useCallback(
    async (id: string) => {
      try {
        await deleteChatSession(id);
        setSessions(prev => {
          const list = prev.filter(s => s.session_id !== id);
          if (list.length === 0) {
            setCurrentSessionId("default");
          } else if (!list.find(s => s.session_id === currentSessionId)) {
            // use currentSessionId from closure - this works because we read the latest
            // via the setter callback pattern below
          }
          return list;
        });
        // Re-fetch to ensure consistency
        const list = await listChatSessions();
        setSessions(list);
        if (list.length > 0 && !list.find(s => s.session_id === currentSessionId)) {
          setCurrentSessionId(list[0].session_id);
        }
      } catch (e) {
        logError("删除会话失败", e);
      }
    },
    [currentSessionId]
  );

  const startEditTitle = useCallback((s: ChatSession) => {
    setEditingId(s.session_id);
    setEditingTitle(s.title);
  }, []);

  const commitEditTitle = useCallback(async () => {
    if (!editingId) return;
    try {
      await renameChatSession(editingId, editingTitle.trim() || "未命名");
      await loadSessions();
    } catch (e) {
      logError("重命名失败", e);
    } finally {
      setEditingId(null);
      setEditingTitle("");
    }
  }, [editingId, editingTitle, loadSessions]);

  const cancelEditTitle = useCallback(() => {
    setEditingId(null);
    setEditingTitle("");
  }, []);

  return {
    sessions,
    currentSessionId,
    setCurrentSessionId,
    editingId,
    editingTitle,
    setEditingTitle,
    loadSessions,
    handleNewSession,
    handleDeleteSession,
    startEditTitle,
    commitEditTitle,
    cancelEditTitle,
  };
}
