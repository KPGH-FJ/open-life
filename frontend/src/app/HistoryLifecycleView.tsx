import { RotateCcw, Trash2 } from "lucide-react";
import type { ConversationController } from "@/features/conversation/useConversationController";

function projectBlockerText(codes: string[]): string | null {
  if (codes.includes("project_delete_conversation_history_present")) {
    return "仍有对话历史引用，不能永久删除。";
  }
  if (codes.includes("project_delete_task_history_present")) {
    return "仍有任务或运行历史引用，不能永久删除。";
  }
  if (codes.includes("project_delete_task_history_unknown")) {
    return "任务历史当前不可核验，系统拒绝永久删除。";
  }
  if (codes.includes("project_delete_selected_for_new_conversation")) {
    return "它仍是新对话范围，不能永久删除。";
  }
  return null;
}

function conversationBlockerText(codes: string[] = []): string | null {
  if (codes.includes("conversation_delete_history_present")) {
    return "对话仍有消息或 Turn 历史，必须保留原始记录。";
  }
  if (codes.includes("conversation_delete_task_history_present")) {
    return "仍有 Task 历史引用，不能永久删除。";
  }
  if (codes.includes("conversation_task_history_unknown")) {
    return "Task 历史当前不可核验，系统拒绝改变生命周期。";
  }
  return null;
}

export function HistoryLifecycleView({
  controller,
  onOpenConversation,
}: {
  controller: ConversationController;
  onOpenConversation: (conversationId: string) => void;
}) {
  const archivedProjects = controller.projects.filter(project => project.status === "archived");
  const busy =
    controller.busy ||
    ["restoring", "deleting", "mutating_project"].includes(controller.sessionMutation.phase);

  if (archivedProjects.length === 0 && controller.archivedSessions.length === 0) return null;

  return (
    <section className="ol-history-lifecycle" aria-labelledby="ol-history-archive-title">
      <div className="ol-history-lifecycle__heading">
        <div>
          <span>保留与恢复</span>
          <h2 id="ol-history-archive-title">已归档</h2>
        </div>
        <p>归档内容保留原始记录。只有系统证明没有历史引用的空记录才能永久删除。</p>
      </div>

      {controller.archivedSessions.length > 0 && (
        <div className="ol-history-lifecycle__group">
          <h3>对话</h3>
          <ul>
            {controller.archivedSessions.map(session => {
              const blocker = conversationBlockerText(session.blockerCodes);
              return (
                <li key={session.session_id}>
                  <button
                    type="button"
                    className="ol-history-lifecycle__record"
                    onClick={() => onOpenConversation(session.session_id)}
                  >
                    <strong>{session.title}</strong>
                    <span>
                      {session.turnCount ?? 0} 个 Turn · {session.taskReferenceCount ?? "未知"} 个
                      Task 引用
                    </span>
                    {blocker && <small>{blocker}</small>}
                  </button>
                  <div className="ol-history-lifecycle__actions">
                    <button
                      type="button"
                      disabled={busy}
                      onClick={() => void controller.restoreArchived(session.session_id)}
                    >
                      <RotateCcw size={14} aria-hidden="true" />
                      恢复
                    </button>
                    <button
                      type="button"
                      className="ol-history-lifecycle__danger"
                      disabled={busy || !session.allowedControls?.includes("delete")}
                      title={blocker ?? undefined}
                      onClick={() => void controller.deleteArchived(session.session_id)}
                    >
                      <Trash2 size={14} aria-hidden="true" />
                      永久删除空记录
                    </button>
                  </div>
                </li>
              );
            })}
          </ul>
        </div>
      )}

      {archivedProjects.length > 0 && (
        <div className="ol-history-lifecycle__group">
          <h3>Projects</h3>
          <ul>
            {archivedProjects.map(project => {
              const blocker = projectBlockerText(project.blockerCodes);
              return (
                <li key={project.id}>
                  <div className="ol-history-lifecycle__record">
                    <strong>{project.name}</strong>
                    <span>
                      {project.totalConversationCount} 个对话引用 ·{" "}
                      {project.taskRunReferenceCount === null
                        ? "任务引用未知"
                        : `${project.taskRunReferenceCount} 个任务运行引用`}
                    </span>
                    {blocker && <small>{blocker}</small>}
                  </div>
                  <div className="ol-history-lifecycle__actions">
                    <button
                      type="button"
                      disabled={busy}
                      onClick={() => void controller.restoreProject(project.id, project.revision)}
                    >
                      <RotateCcw size={14} aria-hidden="true" />
                      恢复
                    </button>
                    <button
                      type="button"
                      className="ol-history-lifecycle__danger"
                      disabled={busy || !project.allowedControls.includes("delete")}
                      title={blocker ?? undefined}
                      onClick={() => void controller.deleteProject(project.id, project.revision)}
                    >
                      <Trash2 size={14} aria-hidden="true" />
                      永久删除记录
                    </button>
                  </div>
                </li>
              );
            })}
          </ul>
        </div>
      )}
    </section>
  );
}
