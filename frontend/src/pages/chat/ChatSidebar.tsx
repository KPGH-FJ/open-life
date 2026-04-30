import { MessageSquare, Plus, Trash2, Edit2 } from "lucide-react";
import type { ChatSession } from "../../tauri";
import EmptyState from "../../components/EmptyState";

interface ChatSidebarProps {
  sessions: ChatSession[];
  currentSessionId: string;
  editingId: string | null;
  editingTitle: string;
  onSelectSession: (sessionId: string) => void;
  onNewSession: () => void;
  onStartEditTitle: (session: ChatSession) => void;
  onCommitEditTitle: () => void;
  onCancelEditTitle: () => void;
  onEditTitleChange: (title: string) => void;
  onDeleteSession: (sessionId: string) => void;
}

export default function ChatSidebar({
  sessions,
  currentSessionId,
  editingId,
  editingTitle,
  onSelectSession,
  onNewSession,
  onStartEditTitle,
  onCommitEditTitle,
  onCancelEditTitle,
  onEditTitleChange,
  onDeleteSession,
}: ChatSidebarProps) {
  return (
    <div className="w-64 border-r bg-gray-50 flex flex-col">
      <div className="px-4 py-3 border-b flex items-center justify-between">
        <span className="text-sm font-semibold text-gray-700">会话</span>
        <button
          onClick={onNewSession}
          className="p-1.5 rounded-md hover:bg-gray-200 text-gray-600"
          title="新建会话"
        >
          <Plus size={16} />
        </button>
      </div>
      <div className="flex-1 overflow-auto py-2 space-y-1">
        {sessions.map(s => (
          <div
            key={s.session_id}
            className={`mx-2 px-3 py-2 rounded-md flex items-center gap-2 cursor-pointer group ${
              s.session_id === currentSessionId
                ? "bg-indigo-100 text-indigo-900"
                : "hover:bg-gray-200 text-gray-700"
            }`}
            onClick={() => onSelectSession(s.session_id)}
          >
            <MessageSquare size={16} className="shrink-0" />
            {editingId === s.session_id ? (
              <input
                autoFocus
                className="flex-1 min-w-0 text-sm bg-white border rounded px-1"
                value={editingTitle}
                onChange={e => onEditTitleChange(e.target.value)}
                onKeyDown={e => {
                  if (e.key === "Enter") onCommitEditTitle();
                  if (e.key === "Escape") onCancelEditTitle();
                }}
                onBlur={onCommitEditTitle}
                onClick={e => e.stopPropagation()}
              />
            ) : (
              <span className="flex-1 min-w-0 truncate text-sm">{s.title}</span>
            )}
            {editingId !== s.session_id && (
              <div className="hidden group-hover:flex items-center gap-1">
                <button
                  onClick={e => {
                    e.stopPropagation();
                    onStartEditTitle(s);
                  }}
                  className="p-1 rounded hover:bg-gray-300 text-gray-500"
                  title="重命名"
                >
                  <Edit2 size={12} />
                </button>
                <button
                  onClick={e => {
                    e.stopPropagation();
                    onDeleteSession(s.session_id);
                  }}
                  className="p-1 rounded hover:bg-red-100 text-gray-500 hover:text-red-600"
                  title="删除"
                >
                  <Trash2 size={12} />
                </button>
              </div>
            )}
          </div>
        ))}
        {sessions.length === 0 && (
          <EmptyState title="暂无会话" description="点击 + 新建一个会话" className="py-6" />
        )}
      </div>
    </div>
  );
}
