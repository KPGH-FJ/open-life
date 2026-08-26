import {
  Archive,
  Folder,
  FolderOpen,
  History,
  MessageSquare,
  MessageSquarePlus,
  UserRound,
} from "lucide-react";
import type { ConversationController } from "@/features/conversation/useConversationController";

export type ProductSidebarView = "conversation" | "history" | "personal-intelligence";

export function ProductSidebar({
  controller,
  activeView,
  attentionCount,
  onNewConversation,
  onOpenFolder,
  onSelectProject,
  onSelectConversation,
  onOpenHistory,
  onOpenPersonalIntelligence,
}: {
  controller: ConversationController;
  activeView: ProductSidebarView;
  attentionCount: number;
  onNewConversation: () => void;
  onOpenFolder: () => void;
  onSelectProject: (projectId: string) => void;
  onSelectConversation: (conversationId: string) => void;
  onOpenHistory: () => void;
  onOpenPersonalIntelligence: () => void;
}) {
  const activeProjects = controller.projects.filter(project => project.status === "active");
  const activeSessions = controller.sessions.filter(session => session.status !== "archived");
  const backgroundWorkCanDetach =
    controller.mode === "work" && controller.turnState.phase === "streaming";
  const navigationLocked =
    controller.pendingResources.length > 0 || (controller.busy && !backgroundWorkCanDetach);

  return (
    <nav className="ol-product-sidebar" aria-label="工作导航">
      <div className="ol-product-sidebar__primary-actions">
        <button
          type="button"
          aria-label="新对话"
          disabled={navigationLocked}
          onClick={onNewConversation}
        >
          <MessageSquarePlus size={18} aria-hidden="true" />
          <span>新对话</span>
        </button>
        <button
          type="button"
          aria-label="打开文件夹"
          disabled={navigationLocked}
          onClick={onOpenFolder}
        >
          <FolderOpen size={18} aria-hidden="true" />
          <span>打开文件夹</span>
        </button>
      </div>

      <section className="ol-product-sidebar__section" aria-labelledby="ol-sidebar-projects-title">
        <div className="ol-product-sidebar__section-heading">
          <h2 id="ol-sidebar-projects-title">Projects</h2>
        </div>
        {activeProjects.length > 0 ? (
          <ul>
            {activeProjects.slice(0, 8).map(project => (
              <li key={project.id}>
                <button
                  type="button"
                  aria-label={project.name}
                  data-current={
                    activeView === "conversation" && controller.selectedProjectId === project.id
                      ? "true"
                      : "false"
                  }
                  disabled={navigationLocked}
                  onClick={() => onSelectProject(project.id)}
                >
                  <Folder size={16} aria-hidden="true" />
                  <span>{project.name}</span>
                </button>
              </li>
            ))}
          </ul>
        ) : (
          <p>打开一个文件夹开始 Work。</p>
        )}
      </section>

      <section
        className="ol-product-sidebar__section ol-product-sidebar__section--conversations"
        aria-labelledby="ol-sidebar-conversations-title"
      >
        <div className="ol-product-sidebar__section-heading">
          <h2 id="ol-sidebar-conversations-title">最近对话</h2>
        </div>
        {activeSessions.length > 0 ? (
          <ul>
            {activeSessions.slice(0, 12).map(session => (
              <li key={session.session_id}>
                <button
                  type="button"
                  aria-label={session.title}
                  data-current={
                    activeView === "conversation" &&
                    controller.selectedSessionId === session.session_id
                      ? "true"
                      : "false"
                  }
                  disabled={navigationLocked}
                  onClick={() => onSelectConversation(session.session_id)}
                >
                  <MessageSquare size={16} aria-hidden="true" />
                  <span>{session.title}</span>
                </button>
              </li>
            ))}
          </ul>
        ) : (
          <p>还没有保存的对话。</p>
        )}
      </section>

      <div className="ol-product-sidebar__secondary-actions">
        <button
          type="button"
          aria-label={attentionCount > 0 ? `历史，${attentionCount} 项需要处理` : "历史"}
          data-current={activeView === "history" ? "true" : "false"}
          disabled={navigationLocked}
          onClick={onOpenHistory}
        >
          <History size={18} aria-hidden="true" />
          <span>历史</span>
          {attentionCount > 0 && (
            <b aria-label={`${attentionCount} 项需要处理`}>{attentionCount}</b>
          )}
        </button>
        {controller.archivedSessions.length > 0 && (
          <span className="ol-product-sidebar__archive-count">
            <Archive size={14} aria-hidden="true" />
            {controller.archivedSessions.length} 个已归档对话
          </span>
        )}
        <button
          type="button"
          aria-label="个人智能"
          data-current={activeView === "personal-intelligence" ? "true" : "false"}
          disabled={navigationLocked}
          onClick={onOpenPersonalIntelligence}
        >
          <UserRound size={18} aria-hidden="true" />
          <span>个人智能</span>
        </button>
      </div>
    </nav>
  );
}
