import {
  FilePlus2,
  MessageSquarePlus,
  Pencil,
  RefreshCw,
  Send,
  Sparkles,
  Square,
  Trash2,
  Wrench,
  X,
} from "lucide-react";
import { useState } from "react";
import { FoundationActionButton, FoundationDialog, FoundationNotice } from "@/ui/foundation";
import type { WorkspaceConversationController } from "./useWorkspaceConversation";
import { WorkspaceMessageContent } from "./WorkspaceMessageContent";

function turnFeedback(controller: WorkspaceConversationController) {
  const state = controller.turnState;
  if (state.phase === "failed") {
    return (
      <FoundationNotice title="会话状态未确认" tone="error" live>
        <p>{state.reason}</p>
      </FoundationNotice>
    );
  }
  if (state.phase === "streaming" && state.cancelError) {
    return (
      <FoundationNotice title="取消请求失败" tone="error" live>
        <p>{state.cancelError}；任务仍按运行中处理，可以再次请求取消。</p>
      </FoundationNotice>
    );
  }
  if (state.phase !== "resolved" || state.status === "completed") return null;
  const copy = {
    completed_with_pending_items: {
      title: "回复已返回，仍有待决定事项",
      body: "待决定事项不会在工作区被解释成已批准、已应用或任务完成。",
      tone: "protection" as const,
    },
    blocked: {
      title: "本轮已阻断",
      body: state.blockers[0] ?? "后端没有提供可展示的阻断原因。",
      tone: "protection" as const,
    },
    failed: {
      title: "本轮失败",
      body: "重新发送前请先核对任务与传输边界。",
      tone: "error" as const,
    },
    remote_unknown: {
      title: "远端结果未知",
      body: "为避免重复外部动作，当前不会自动重试。",
      tone: "protection" as const,
    },
    cancelled: {
      title: "本轮已取消",
      body: "没有把取消状态解释成完成。",
      tone: "neutral" as const,
    },
    interrupted: {
      title: "本轮已中断",
      body: "当前回复可能不完整，请先核对任务状态。",
      tone: "protection" as const,
    },
  }[state.status];
  return copy ? (
    <FoundationNotice title={copy.title} tone={copy.tone} live>
      <p>{copy.body}</p>
    </FoundationNotice>
  ) : null;
}

function resourceFailureText(code: string): string {
  if (code.includes("file_count_exceeded")) return "本轮最多只能读取 5 个文件。";
  if (code.includes("bytes_exceeded")) return "所选文件超过了当前允许的大小。";
  if (code.includes("symlink") || code.includes("not_regular_file")) {
    return "只能读取你直接选择的普通文件，不能读取符号链接或目录。";
  }
  if (
    code.includes("unsupported") ||
    code.includes("mime") ||
    code.includes("corrupt") ||
    code.includes("extension")
  ) {
    return "文件格式不受支持、内容损坏，或内容与扩展名不一致。";
  }
  if (code.includes("identity_mismatch") || code.includes("duplicate_receipt")) {
    return "后端返回的文件凭据与当前回合不一致；为避免引用错文件，本轮没有接受该结果。";
  }
  if (code.includes("cancel")) return "文件读取已取消，没有把它显示为成功。";
  return `文件处理失败（${code}）。`;
}

export function WorkspaceConversationPanel({
  controller,
  disabledReason,
}: {
  controller: WorkspaceConversationController;
  disabledReason?: string;
}) {
  const [sessionDialog, setSessionDialog] = useState<"rename" | "delete" | null>(null);
  const [renameDraft, setRenameDraft] = useState("");
  const action = controller.sendAction(disabledReason);
  const visibleMessages = controller.messages.filter(message => message.role !== "system");
  const selectedSession = controller.sessions.find(
    session => session.session_id === controller.selectedSessionId
  );
  const sessionMutationBusy = ["renaming", "deleting"].includes(controller.sessionMutation.phase);
  const resourceMutationBusy = ["importing", "detaching"].includes(
    controller.resourceMutation.phase
  );
  const pendingResourceCount = controller.pendingResources.length;
  const conversationSwitchLocked = controller.busy || pendingResourceCount > 0;
  const sessionMutationDisabledReason = sessionMutationBusy
    ? "会话操作正在等待后端保存并重新读取。"
    : undefined;

  return (
    <section className="ol-workspace-conversation" aria-labelledby="workspace-conversation-title">
      <header className="ol-workspace-conversation__header">
        <div>
          <span>对话</span>
          <h3 id="workspace-conversation-title">继续当前工作</h3>
        </div>
        <div className="ol-workspace-conversation__tools">
          {controller.sessions.length > 0 && (
            <label>
              <span className="ol-visually-hidden">选择对话</span>
              <select
                value={controller.selectedSessionId ?? ""}
                disabled={conversationSwitchLocked}
                onChange={event => controller.selectSession(event.target.value)}
              >
                {!controller.selectedSessionId && <option value="">新对话</option>}
                {controller.sessions.map(session => (
                  <option key={session.session_id} value={session.session_id}>
                    {session.title}
                  </option>
                ))}
              </select>
            </label>
          )}
          <button
            type="button"
            className="ol-workspace-conversation__new"
            disabled={conversationSwitchLocked}
            onClick={controller.startNewConversation}
          >
            <MessageSquarePlus size={16} aria-hidden="true" />
            新对话
          </button>
          {selectedSession && (
            <>
              <button
                type="button"
                className="ol-workspace-conversation__new"
                disabled={conversationSwitchLocked}
                onClick={() => {
                  setRenameDraft(selectedSession.title);
                  setSessionDialog("rename");
                }}
              >
                <Pencil size={15} aria-hidden="true" />
                重命名
              </button>
              <button
                type="button"
                className="ol-workspace-conversation__new ol-workspace-conversation__delete"
                disabled={conversationSwitchLocked}
                onClick={() => setSessionDialog("delete")}
              >
                <Trash2 size={15} aria-hidden="true" />
                删除
              </button>
            </>
          )}
        </div>
      </header>

      {controller.loadStatus === "loading" ? (
        <div className="ol-workspace-conversation__empty" aria-busy="true">
          正在读取会话记录
        </div>
      ) : controller.loadStatus === "error" ? (
        <div className="ol-workspace-conversation__load-error">
          <FoundationNotice title="会话记录暂时不可用" tone="error" live>
            <p>{controller.loadError ?? "后端未返回可用会话记录。"}</p>
          </FoundationNotice>
          <FoundationActionButton
            label="重新读取会话"
            icon={<RefreshCw size={17} aria-hidden="true" />}
            onClick={() => void controller.reload()}
          />
        </div>
      ) : visibleMessages.length > 0 ? (
        <ol className="ol-workspace-transcript" aria-label="当前对话记录">
          {visibleMessages.map((message, index) => (
            <li key={`${message.role}:${index}`} data-role={message.role}>
              <span>{message.role === "user" ? "你" : "OpenLife"}</span>
              <WorkspaceMessageContent
                content={message.content}
                allowBackendSources={message.role === "assistant"}
              />
            </li>
          ))}
          {controller.streamingReply && (
            <li data-role="assistant" data-streaming="true" aria-live="polite">
              <span>OpenLife</span>
              <p>{controller.streamingReply}</p>
            </li>
          )}
        </ol>
      ) : (
        <div className="ol-workspace-conversation__empty">
          <strong>{controller.selectedSessionId ? "这段对话还没有消息" : "开始一段新对话"}</strong>
          <p>草稿只保留在当前页面；发送才会创建会话并进入后端治理流程。</p>
        </div>
      )}

      {turnFeedback(controller)}

      {controller.sessionMutation.phase === "failed" && (
        <FoundationNotice title="会话操作未完成" tone="error" live>
          <p>{controller.sessionMutation.reason}</p>
        </FoundationNotice>
      )}

      {controller.resourceMutation.phase === "failed" && (
        <FoundationNotice title="文件没有完成变更" tone="error" live>
          <p>{resourceFailureText(controller.resourceMutation.reason)}</p>
        </FoundationNotice>
      )}

      <form
        className="ol-workspace-composer"
        onSubmit={event => {
          event.preventDefault();
          void controller.send(disabledReason);
        }}
      >
        <div className="ol-workspace-resources" aria-labelledby="workspace-resources-title">
          <div className="ol-workspace-resources__header">
            <div>
              <strong id="workspace-resources-title">本轮文件</strong>
              <span>
                {pendingResourceCount > 0 ? `已添加 ${pendingResourceCount}/5` : "未添加"}
              </span>
            </div>
            <button
              type="button"
              className="ol-workspace-resources__add"
              disabled={
                controller.loadStatus !== "ready" ||
                controller.busy ||
                resourceMutationBusy ||
                pendingResourceCount >= 5
              }
              onClick={() => void controller.attachResources()}
            >
              <FilePlus2 size={16} aria-hidden="true" />
              {controller.resourceMutation.phase === "importing" ? "正在读取" : "添加文件"}
            </button>
          </div>
          {pendingResourceCount > 0 ? (
            <ul className="ol-workspace-resources__list" aria-label="下一次发送包含的文件">
              {controller.pendingResources.map(resource => (
                <li key={resource.resourceId}>
                  <div>
                    <strong>{resource.filename}</strong>
                    <span>{Math.max(1, Math.ceil(resource.byteCount / 1024))} KB</span>
                  </div>
                  <button
                    type="button"
                    aria-label={`移除 ${resource.filename}`}
                    disabled={controller.busy || resourceMutationBusy}
                    onClick={() => void controller.detachResource(resource.resourceId)}
                  >
                    <X size={15} aria-hidden="true" />
                  </button>
                </li>
              ))}
            </ul>
          ) : (
            <p>
              只读取你通过原生选择器明确选中的文件；内容按当前 Provider
              和已生效隐私许可处理，未授权外传不会执行。
            </p>
          )}
        </div>
        <div className="ol-workspace-capabilities" aria-labelledby="workspace-capabilities-title">
          <div className="ol-workspace-capabilities__header">
            <div>
              <strong id="workspace-capabilities-title">技能与只读工具</strong>
              <span>
                {controller.capabilityState.phase === "loading"
                  ? "正在核对"
                  : controller.selectedSkillId
                    ? "当前对话已选择技能"
                    : "未选择技能"}
              </span>
            </div>
          </div>
          <div className="ol-workspace-capabilities__grid">
            <label>
              <span>
                <Sparkles size={15} aria-hidden="true" />
                当前技能
              </span>
              <select
                value={controller.selectedSkillId ?? ""}
                disabled={
                  !controller.selectedSessionId ||
                  controller.busy ||
                  controller.capabilityState.phase === "loading" ||
                  controller.capabilityState.phase === "selecting"
                }
                onChange={event => void controller.selectSkill(event.target.value || null)}
              >
                <option value="">不使用技能</option>
                {controller.skills
                  .filter(skill => skill.available)
                  .map(skill => (
                    <option key={skill.skillId} value={skill.skillId}>
                      {skill.name}
                    </option>
                  ))}
              </select>
              <small>
                {controller.selectedSessionId
                  ? "技能只提供有界指令，不会扩大模型、网络、工具或写入权限。"
                  : "先发送第一条消息建立对话，再为后续回合选择技能。"}
              </small>
            </label>
            <div className="ol-workspace-capabilities__tools">
              <span>
                <Wrench size={15} aria-hidden="true" />
                已注册只读工具
              </span>
              {controller.toolCandidates?.candidates.length ? (
                <ul>
                  {controller.toolCandidates.candidates.slice(0, 4).map(candidate => (
                    <li key={candidate.candidateId}>
                      <strong>{candidate.toolName}</strong>
                      <small>{candidate.capabilityLabels.join(" · ") || "read"}</small>
                    </li>
                  ))}
                </ul>
              ) : (
                <small>当前没有后端确认可用的 MCP 只读工具。</small>
              )}
              {Boolean(controller.toolCandidates?.blockedTools.length) && (
                <small>
                  另有 {controller.toolCandidates!.blockedTools.length} 个工具因写入、风险或
                  manifest 状态被后端阻断。
                </small>
              )}
            </div>
          </div>
          {controller.capabilityState.phase === "failed" && (
            <small role="status">技能或工具状态不可用：{controller.capabilityState.reason}</small>
          )}
        </div>
        <label htmlFor="workspace-composer-input">消息</label>
        <textarea
          id="workspace-composer-input"
          value={controller.draft}
          rows={3}
          placeholder="告诉 OpenLife 你现在要处理什么"
          disabled={controller.loadStatus !== "ready" || controller.busy}
          aria-describedby={!action.enabled ? "workspace-composer-disabled-reason" : undefined}
          onChange={event => controller.setDraft(event.target.value)}
          onKeyDown={event => {
            if (event.key === "Enter" && !event.shiftKey) {
              event.preventDefault();
              void controller.send(disabledReason);
            }
          }}
        />
        <div className="ol-workspace-composer__footer">
          <span
            id="workspace-composer-disabled-reason"
            className="ol-workspace-composer__status"
            role="status"
          >
            {controller.turnState.phase === "streaming"
              ? "正在接收回复；可取消当前任务"
              : controller.turnState.phase === "cancelling"
                ? "正在等待后端确认取消终态"
                : action.enabled
                  ? "Enter 发送，Shift + Enter 换行"
                  : (action.disabledReason ?? "当前不能发送")}
          </span>
          <div className="ol-workspace-composer__actions">
            {(controller.turnState.phase === "streaming" ||
              controller.turnState.phase === "cancelling") && (
              <FoundationActionButton
                label="取消任务"
                icon={<Square size={16} aria-hidden="true" />}
                loading={controller.turnState.phase === "cancelling"}
                loadingLabel="正在取消"
                disabled={controller.turnState.phase === "cancelling"}
                disabledReason={
                  controller.turnState.phase === "cancelling"
                    ? "取消请求已发送；正在等待真实终态。"
                    : undefined
                }
                data-action-category="product"
                data-action-id={`workspace.cancel:${controller.activeTaskSessionId ?? "unknown"}`}
                data-action-kind="cancel"
                data-action-enabled={String(controller.turnState.phase === "streaming")}
                data-action-target-ref={controller.activeTaskSessionId ?? "unknown"}
                type="button"
                onClick={() => void controller.cancel()}
              />
            )}
            {controller.turnState.phase !== "streaming" &&
              controller.turnState.phase !== "cancelling" && (
                <FoundationActionButton
                  label={action.label}
                  icon={<Send size={17} aria-hidden="true" />}
                  variant="primary"
                  loading={controller.busy}
                  loadingLabel={
                    controller.turnState.phase === "refreshing" ? "正在核对" : "正在发送"
                  }
                  disabled={!action.enabled}
                  disabledReason={action.disabledReason}
                  data-action-category="product"
                  data-action-id={action.id}
                  data-action-kind={action.kind}
                  data-action-enabled={String(action.enabled)}
                  data-action-disabled-reason={action.disabledReason ?? ""}
                  data-action-target-ref={action.targetRef}
                  type="submit"
                />
              )}
          </div>
        </div>
      </form>

      <FoundationDialog
        open={sessionDialog === "rename"}
        title="重命名这段对话"
        description="新名称只有在后端保存并重新读取成功后才会显示为已确认。"
        busy={sessionMutationBusy}
        onClose={() => setSessionDialog(null)}
        footer={
          <>
            <FoundationActionButton
              label="取消"
              variant="quiet"
              disabled={sessionMutationBusy}
              disabledReason={sessionMutationDisabledReason}
              onClick={() => setSessionDialog(null)}
            />
            <FoundationActionButton
              label="保存名称"
              variant="primary"
              loading={controller.sessionMutation.phase === "renaming"}
              loadingLabel="正在保存"
              disabled={!renameDraft.trim() || sessionMutationBusy}
              disabledReason={
                !renameDraft.trim() ? "对话名称不能为空。" : sessionMutationDisabledReason
              }
              onClick={() => {
                void controller.renameSelected(renameDraft).then(saved => {
                  if (saved) setSessionDialog(null);
                });
              }}
            />
          </>
        }
      >
        <label className="ol-workspace-session-dialog__field">
          <span>对话名称</span>
          <input
            value={renameDraft}
            maxLength={120}
            disabled={sessionMutationBusy}
            onChange={event => setRenameDraft(event.target.value)}
          />
        </label>
      </FoundationDialog>

      <FoundationDialog
        open={sessionDialog === "delete"}
        title="删除这段对话？"
        description="这会删除当前会话并写入删除记录。只有点击下方确认按钮才会执行。"
        busy={sessionMutationBusy}
        onClose={() => setSessionDialog(null)}
        footer={
          <>
            <FoundationActionButton
              label="保留对话"
              variant="quiet"
              disabled={sessionMutationBusy}
              disabledReason={sessionMutationDisabledReason}
              onClick={() => setSessionDialog(null)}
            />
            <FoundationActionButton
              label="确认删除"
              loading={controller.sessionMutation.phase === "deleting"}
              loadingLabel="正在删除"
              disabled={sessionMutationBusy}
              disabledReason={sessionMutationDisabledReason}
              onClick={() => {
                void controller.deleteSelected().then(deleted => {
                  if (deleted) setSessionDialog(null);
                });
              }}
            />
          </>
        }
      >
        <p>{selectedSession?.title ?? "当前对话"}</p>
      </FoundationDialog>
    </section>
  );
}
