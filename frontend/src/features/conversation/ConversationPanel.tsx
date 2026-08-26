import {
  Archive,
  FilePlus2,
  Pencil,
  RefreshCw,
  RotateCcw,
  Send,
  Sparkles,
  Square,
  Wrench,
  X,
} from "lucide-react";
import { useState, type ReactNode } from "react";
import { FoundationActionButton, FoundationDialog, FoundationNotice } from "@/ui/foundation";
import { productErrorCode, productErrorMessage } from "@/shared/productError";
import type {
  MainChatBlockedTool,
  MainChatToolCandidate,
  ReasoningEffort,
  TaskControl,
} from "@/tauri";
import type { ConversationController } from "./useConversationController";
import { MessageContent } from "./MessageContent";

function projectBlockerText(codes: string[]): string | null {
  if (codes.includes("project_archive_active_conversations_present")) {
    return "先把活动对话移出这个 Project，才能归档。";
  }
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
  if (codes.includes("conversation_archive_active_task_present")) {
    return "当前仍有运行中或等待决定的 Work，不能归档。";
  }
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

function reasoningEffortLabel(effort: ReasoningEffort): string {
  return {
    none: "不推理（最快）",
    minimal: "极低",
    low: "低",
    medium: "中（均衡）",
    high: "高",
    xhigh: "很高",
    max: "最高（最慢）",
  }[effort];
}

function toolAdmissionLabel(candidate: MainChatToolCandidate): string {
  if (candidate.policyDecision === "allow" && !candidate.requiresPermission) {
    return "本轮可用 · 无需逐次授权";
  }
  if (candidate.requiresPermission) return "使用时会请求授权";
  return "当前不可执行";
}

function toolSelectionReasonLabel(reason: string): string {
  if (reason === "manifest_default_order") return "已注册并满足只读工具契约";
  if (reason === "capability_or_name_match") return "能力与本轮需要相符";
  return "由后端工具清单准入";
}

function blockedToolReasonLabel(tool: MainChatBlockedTool): string {
  if (tool.reasonCode === "write_like_tool_blocked") return "包含写入或外部副作用";
  if (tool.reasonCode === "high_risk_tool_blocked") return "风险等级需要单独决定";
  if (tool.reasonCode === "permission_required") return "执行前需要授权";
  if (tool.reasonCode === "declarative_only_tool_blocked") return "只有声明，没有可执行实现";
  if (tool.reasonCode === "tool_unavailable") return "当前不可用";
  return "未通过当前运行策略";
}

function recoveryControlLabel(control: TaskControl): string {
  return control.kind === "retry" ? "重试并创建新运行" : "继续并创建新运行";
}

function turnFeedback(
  controller: ConversationController,
  recoveryControl?: TaskControl,
  onRequestRecovery?: (control: TaskControl, expectedTaskId: string) => void,
  onOpenProviderSettings?: () => void
) {
  const state = controller.turnState;
  if (state.phase === "failed") {
    const providerRecoveryAvailable =
      onOpenProviderSettings &&
      /provider|model.*unavailable|credential/i.test(productErrorCode(state.reason));
    return (
      <FoundationNotice title="会话状态未确认" tone="error" live>
        <p>{productErrorMessage(state.reason, "本轮没有完成。你可以重试，或查看工作详情。")}</p>
        {providerRecoveryAvailable && (
          <FoundationActionButton
            label="检查模型连接"
            variant="secondary"
            onClick={onOpenProviderSettings}
          />
        )}
      </FoundationNotice>
    );
  }
  if (state.phase === "streaming" && state.cancelError) {
    return (
      <FoundationNotice title="取消请求失败" tone="error" live>
        <p>{productErrorMessage(state.cancelError)}</p>
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
      body: state.blockers[0] ?? "系统没有提供可展示的阻断原因。",
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
      {recoveryControl &&
        onRequestRecovery &&
        state.taskId === recoveryControl.targetTaskId &&
        recoveryControl.enabled && (
          <FoundationActionButton
            label={recoveryControlLabel(recoveryControl)}
            icon={<RotateCcw size={16} aria-hidden="true" />}
            variant="secondary"
            onClick={() => onRequestRecovery(recoveryControl, state.taskId!)}
          />
        )}
    </FoundationNotice>
  ) : null;
}

function lifeModelInfluenceFeedback(
  controller: ConversationController,
  onOpenLifeModel: (itemRef: string) => void
) {
  const state = controller.turnState;
  if (state.phase !== "resolved" || !state.lifeModelInfluence) return null;
  const receipt = state.lifeModelInfluence;
  if (receipt.permissionGranted || receipt.durableWriteAuthorized) {
    return (
      <FoundationNotice title="Life Model 内容不能作为授权" tone="error" live>
        <p>Life Model 只能帮助理解你的偏好，不能增加工具、权限或写入能力。</p>
      </FoundationNotice>
    );
  }
  if (receipt.selectedItems.length === 0 && receipt.status !== "current_instruction_override") {
    return null;
  }
  const applied = receipt.appliedSurfaces.length > 0;
  return (
    <FoundationNotice
      title={applied ? "本轮参考了你的 Life Model" : "本轮以当前指令为准"}
      tone="neutral"
    >
      <p>
        {applied
          ? `影响范围：${receipt.appliedSurfaces.join("、")}。它没有增加工具、权限或写入能力。`
          : "相关长期信息未覆盖你在本轮给出的明确要求。"}
      </p>
      {receipt.selectedItems.length > 0 && (
        <details>
          <summary>查看使用依据</summary>
          <ul>
            {receipt.selectedItems.map(item => (
              <li key={item.itemRef}>
                <strong>{item.statement}</strong>
                <br />
                <span>确认于 {item.confirmedAt}</span>
                <br />
                <FoundationActionButton
                  label={`在个人智能中查看：${item.statement}`}
                  variant="quiet"
                  onClick={() => onOpenLifeModel(item.itemRef)}
                />
              </li>
            ))}
          </ul>
          <p>当前要求始终优先；Life Model 不会扩大权限。</p>
        </details>
      )}
    </FoundationNotice>
  );
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
    return "系统返回的文件凭据与当前回合不一致；为避免引用错文件，本轮没有接受该结果。";
  }
  if (code.includes("cancel")) return "文件读取已取消，没有把它显示为成功。";
  return productErrorMessage(code, "文件处理没有完成。请检查文件后重试。");
}

export function ConversationPanel({
  controller,
  onOpenLifeModel,
  disabledReason,
  readOnlyReason,
  inlineCheckpoint,
  recoveryControl,
  onRequestRecovery,
  onOpenProviderSettings,
}: {
  controller: ConversationController;
  onOpenLifeModel: (itemRef: string) => void;
  disabledReason?: string;
  readOnlyReason?: string;
  inlineCheckpoint?: ReactNode;
  recoveryControl?: TaskControl;
  onRequestRecovery?: (control: TaskControl, expectedTaskId: string) => void;
  onOpenProviderSettings?: () => void;
}) {
  const [sessionDialog, setSessionDialog] = useState<"rename" | "delete" | null>(null);
  const [renameDraft, setRenameDraft] = useState("");
  const [modelPickerOpen, setModelPickerOpen] = useState(false);
  const [modelQuery, setModelQuery] = useState("");
  const [projectRenameTargetId, setProjectRenameTargetId] = useState<string | null>(null);
  const [projectRenameDraft, setProjectRenameDraft] = useState("");
  const action = controller.sendAction(disabledReason);
  const composerLocked = Boolean(readOnlyReason);
  const visibleMessages = controller.messages.filter(message => message.role !== "system");
  const selectedSession = [...controller.sessions, ...controller.archivedSessions].find(
    session => session.session_id === controller.selectedSessionId
  );
  const selectedProject = controller.projects.find(
    project => project.id === controller.selectedProjectId
  );
  const projectRenameTarget = controller.projects.find(
    project => project.id === projectRenameTargetId
  );
  const selectedProvider = controller.provider.profiles.find(
    profile => profile.profileId === controller.provider.selectedProfileId
  );
  const normalizedModelQuery = modelQuery.trim().toLocaleLowerCase("en-US");
  const matchingProviderProfiles = controller.provider.profiles
    .filter(profile => {
      if (!normalizedModelQuery) return true;
      return [profile.displayName, profile.modelId, profile.providerId]
        .filter(Boolean)
        .some(value => value!.toLocaleLowerCase("en-US").includes(normalizedModelQuery));
    })
    .sort((left, right) => {
      const leftSelected = left.profileId === controller.provider.selectedProfileId ? 0 : 1;
      const rightSelected = right.profileId === controller.provider.selectedProfileId ? 0 : 1;
      if (leftSelected !== rightSelected) return leftSelected - rightSelected;
      const leftAvailability = left.availability === "ready" ? 0 : 1;
      const rightAvailability = right.availability === "ready" ? 0 : 1;
      if (leftAvailability !== rightAvailability) return leftAvailability - rightAvailability;
      return (left.displayName ?? left.modelId).localeCompare(
        right.displayName ?? right.modelId,
        "zh-CN"
      );
    })
    .slice(0, 60);
  const selectedWorkspaceName = selectedProject?.workspaceRoot
    ?.split(/[\\/]/)
    .filter(Boolean)
    .pop();
  const sessionMutationBusy = [
    "renaming",
    "archiving",
    "restoring",
    "deleting",
    "creating_project",
    "assigning_project",
    "mutating_project",
  ].includes(controller.sessionMutation.phase);
  const resourceMutationBusy = ["importing", "detaching"].includes(
    controller.resourceMutation.phase
  );
  const pendingResourceCount = controller.pendingResources.length;
  const conversationSettingsLocked = controller.busy || pendingResourceCount > 0;
  const sessionMutationDisabledReason = sessionMutationBusy
    ? "会话操作正在等待系统保存并重新读取。"
    : undefined;

  return (
    <section className="ol-workspace-conversation" aria-label="当前对话">
      {(selectedSession || selectedProject) && (
        <details className="ol-workspace-conversation__manage">
          <summary>对话设置</summary>
          <div className="ol-workspace-conversation__tools">
            {selectedProject && (
              <span className="ol-workspace-conversation__project-scope">
                <span role="status">
                  {selectedWorkspaceName
                    ? `文件夹范围：${selectedWorkspaceName}`
                    : "此 Project 尚未绑定文件夹"}
                </span>
                <button
                  type="button"
                  className="ol-workspace-conversation__scope-action"
                  disabled={controller.busy || sessionMutationBusy}
                  onClick={() =>
                    void controller.bindProjectDirectory(
                      selectedProject.id,
                      selectedProject.revision
                    )
                  }
                >
                  {selectedWorkspaceName ? "更换文件夹" : "绑定文件夹"}
                </button>
                <button
                  type="button"
                  className="ol-workspace-conversation__scope-action"
                  disabled={controller.busy || sessionMutationBusy}
                  onClick={() =>
                    void controller.addProjectReadRoot(selectedProject.id, selectedProject.revision)
                  }
                >
                  添加读取文件夹
                </button>
                {selectedProject.additionalReadRoots.map(root => (
                  <span key={root.id} className="ol-workspace-conversation__read-root">
                    只读：{root.name}
                    <button
                      type="button"
                      className="ol-workspace-conversation__scope-action"
                      aria-label={`移除读取范围 ${root.name}`}
                      disabled={controller.busy || sessionMutationBusy}
                      onClick={() =>
                        void controller.removeProjectReadRoot(
                          selectedProject.id,
                          root.id,
                          selectedProject.revision
                        )
                      }
                    >
                      移除
                    </button>
                  </span>
                ))}
                <button
                  type="button"
                  className="ol-workspace-conversation__scope-action"
                  disabled={controller.busy || sessionMutationBusy}
                  onClick={() => {
                    setProjectRenameDraft(selectedProject.name);
                    setProjectRenameTargetId(selectedProject.id);
                  }}
                >
                  重命名 Project
                </button>
                <button
                  type="button"
                  className="ol-workspace-conversation__scope-action"
                  disabled={
                    controller.busy ||
                    sessionMutationBusy ||
                    !selectedProject.allowedControls.includes("archive")
                  }
                  title={projectBlockerText(selectedProject.blockerCodes) ?? undefined}
                  onClick={() =>
                    void controller.archiveProject(selectedProject.id, selectedProject.revision)
                  }
                >
                  <Archive size={14} aria-hidden="true" />
                  归档
                </button>
              </span>
            )}
            {selectedSession?.status === "archived" ? (
              <button
                type="button"
                className="ol-workspace-conversation__new"
                disabled={conversationSettingsLocked}
                onClick={() => void controller.restoreArchived(selectedSession.session_id)}
              >
                <RotateCcw size={15} aria-hidden="true" />
                恢复对话
              </button>
            ) : selectedSession ? (
              <>
                <button
                  type="button"
                  className="ol-workspace-conversation__new"
                  disabled={conversationSettingsLocked}
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
                  className="ol-workspace-conversation__new"
                  disabled={
                    conversationSettingsLocked ||
                    !(selectedSession.allowedControls?.includes("archive") ?? true)
                  }
                  title={conversationBlockerText(selectedSession.blockerCodes) ?? undefined}
                  onClick={() => void controller.archiveSelected()}
                >
                  <Archive size={15} aria-hidden="true" />
                  归档
                </button>
              </>
            ) : null}
          </div>
        </details>
      )}

      {controller.loadStatus === "loading" ? (
        <div className="ol-workspace-conversation__empty" aria-busy="true">
          正在读取会话记录
        </div>
      ) : controller.loadStatus === "error" ? (
        <div className="ol-workspace-conversation__load-error">
          <FoundationNotice title="会话记录暂时不可用" tone="error" live>
            <p>{productErrorMessage(controller.loadError, "暂时无法读取对话记录，请重试。")}</p>
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
              <div className="ol-workspace-transcript__body">
                <MessageContent
                  content={message.content}
                  allowBackendSources={message.role === "assistant"}
                />
                {message.role === "user" && message.attachmentsStatus === "unavailable" && (
                  <p className="ol-workspace-transcript__attachment-status" role="status">
                    这一轮的附件记录暂时无法读取，不能确认是否使用过文件。
                  </p>
                )}
                {(message.attachments?.length ?? 0) > 0 && (
                  <ul
                    className="ol-workspace-transcript__attachments"
                    aria-label="这一轮使用的文件"
                  >
                    {message.attachments?.map(attachment => (
                      <li key={attachment.resourceId} title={`SHA-256 ${attachment.digest}`}>
                        <span>{attachment.filename}</span>
                        <small>
                          {attachment.format.toUpperCase()} ·{" "}
                          {Math.max(1, Math.ceil(attachment.byteCount / 1024))} KB
                        </small>
                      </li>
                    ))}
                  </ul>
                )}
              </div>
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
        <div className="ol-workspace-conversation__empty" data-testid="workbench-onboarding">
          <strong>
            {controller.selectedSessionId ? "这段对话还没有消息" : "把一件真实工作交给 OpenLife"}
          </strong>
          <p>
            {controller.selectedSessionId
              ? "输入消息继续这段对话。"
              : "简单问题选择 Chat；需要查资料、读取文件或交付结果时选择 Work。发送前不会创建任务或写入任何长期状态。"}
          </p>
          {!controller.selectedSessionId && (
            <ol className="ol-workspace-onboarding-steps">
              <li>说清楚想要的结果和完成标准</li>
              <li>需要时添加文件、Project 或 Skill</li>
              <li>执行中可追加指令，关键边界才会请你决定</li>
            </ol>
          )}
        </div>
      )}

      {turnFeedback(controller, recoveryControl, onRequestRecovery, onOpenProviderSettings)}
      {inlineCheckpoint}
      {lifeModelInfluenceFeedback(controller, onOpenLifeModel)}

      {controller.sessionMutation.phase === "failed" && (
        <FoundationNotice title="会话操作未完成" tone="error" live>
          <p>{productErrorMessage(controller.sessionMutation.reason)}</p>
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
        <fieldset className="ol-workspace-mode" disabled={controller.busy || composerLocked}>
          <legend>运行方式</legend>
          <label>
            <input
              type="radio"
              name="workspace-mode"
              value="chat"
              checked={controller.mode === "chat"}
              onChange={() => controller.setMode("chat")}
            />
            Chat
          </label>
          <label>
            <input
              type="radio"
              name="workspace-mode"
              value="work"
              checked={controller.mode === "work"}
              disabled={controller.workStatus !== "available"}
              onChange={() => controller.setMode("work")}
            />
            Work
          </label>
          <small>
            {controller.mode === "chat"
              ? "直接对话，不创建任务。"
              : "可使用文件、工具与受治理动作完成任务。"}
            {controller.workStatus === "unavailable" && " Work 当前不可用；不会回退到旧执行路径。"}
          </small>
        </fieldset>
        <div className="ol-workspace-provider" aria-live="polite">
          <label>
            <span>模型</span>
            <button
              type="button"
              className="ol-workspace-model-trigger"
              disabled={
                controller.busy || composerLocked || controller.provider.profiles.length === 0
              }
              onClick={() => {
                setModelQuery("");
                setModelPickerOpen(true);
              }}
            >
              {selectedProvider?.displayName ?? selectedProvider?.modelId ?? "选择模型"}
            </button>
          </label>
          {controller.provider.status === "unavailable" && (
            <span>当前选择不可用；请选择上方可用模型，或前往设置完成配置。</span>
          )}
          {controller.provider.status === "unknown" && <span>正在核对可用模型。</span>}
          {selectedProvider && (
            <small>
              {selectedProvider.endpointClass === "local" ? "本地" : selectedProvider.providerId}
              {" · "}
              {selectedProvider.supportedReasoningEfforts.length > 0
                ? (controller.provider.selectedReasoningEffort ??
                  selectedProvider.defaultReasoningEffort)
                  ? `本轮推理：${reasoningEffortLabel(
                      (controller.provider.selectedReasoningEffort ??
                        selectedProvider.defaultReasoningEffort)!
                    )}`
                  : "本轮推理：模型默认"
                : "使用模型默认推理"}
            </small>
          )}
        </div>
        <details className="ol-workspace-composer-context ol-workspace-composer-options">
          <summary>
            <span>本轮选项</span>
            <small>
              {controller.mode === "work"
                ? controller.executionMode === "observe_only"
                  ? "只读研究"
                  : "标准执行"
                : "Chat"}
              {" · "}
              {controller.memoryMode === "off"
                ? "记忆关闭"
                : controller.memoryMode === "use_only"
                  ? "仅使用记忆"
                  : "使用记忆"}
            </small>
          </summary>
          <div className="ol-workspace-composer-options__body">
            {selectedProvider && selectedProvider.supportedReasoningEfforts.length > 0 && (
              <label className="ol-workspace-memory-mode" htmlFor="ol-workspace-reasoning-effort">
                <span>推理强度</span>
                <select
                  id="ol-workspace-reasoning-effort"
                  value={controller.provider.selectedReasoningEffort ?? ""}
                  disabled={controller.busy || composerLocked}
                  onChange={event => {
                    const value = event.target.value;
                    controller.selectReasoningEffort(value ? (value as ReasoningEffort) : null);
                  }}
                >
                  <option value="">
                    {selectedProvider.defaultReasoningEffort
                      ? `模型默认（${reasoningEffortLabel(selectedProvider.defaultReasoningEffort)}）`
                      : "模型默认"}
                  </option>
                  {selectedProvider.supportedReasoningEfforts.map(effort => (
                    <option key={effort} value={effort}>
                      {reasoningEffortLabel(effort)}
                    </option>
                  ))}
                </select>
                <small>只影响当前对话的新消息；不会自动切换模型。</small>
              </label>
            )}
            {controller.mode === "work" && (
              <label
                className="ol-workspace-memory-mode ol-workspace-execution-mode"
                htmlFor="ol-workspace-execution-mode"
              >
                <span>执行方式</span>
                <select
                  id="ol-workspace-execution-mode"
                  value={controller.executionMode}
                  disabled={controller.busy || composerLocked}
                  onChange={event =>
                    controller.setExecutionMode(
                      event.target.value as "scoped_agent" | "observe_only"
                    )
                  }
                >
                  <option value="scoped_agent">标准执行</option>
                  <option value="observe_only">只读研究</option>
                </select>
                <small>
                  {controller.executionMode === "observe_only"
                    ? "只允许分析与读取；不会创建 Artifact 或写入个人长期状态。"
                    : "已选范围内的低风险工作可自动执行；扩大范围或产生重要外部影响时再请你决定。"}
                </small>
              </label>
            )}
            <label className="ol-workspace-memory-mode" htmlFor="ol-workspace-memory-mode">
              <span>记忆</span>
              <select
                id="ol-workspace-memory-mode"
                value={controller.memoryMode}
                disabled={controller.busy || composerLocked || !controller.globalMemoryEnabled}
                onChange={event =>
                  void controller.setMemoryMode(
                    event.target.value as "use_and_learn" | "use_only" | "off"
                  )
                }
              >
                <option value="use_and_learn">使用并学习</option>
                <option value="use_only">仅使用</option>
                <option value="off">关闭</option>
              </select>
              <small>
                {controller.globalMemoryEnabled
                  ? "只影响当前对话；明确要求记住或忘记仍由你发起。"
                  : "Agent 记忆已在设置中关闭。"}
              </small>
            </label>
          </div>
        </details>
        {controller.mode === "work" && (
          <details className="ol-workspace-composer-context">
            <summary>
              <span>文件、技能与工具</span>
              <small>
                {pendingResourceCount > 0
                  ? `${pendingResourceCount} 个文件`
                  : controller.selectedSkillId
                    ? "已选择技能"
                    : "按需添加"}
              </small>
            </summary>
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
                    composerLocked ||
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
                        disabled={controller.busy || composerLocked || resourceMutationBusy}
                        onClick={() => void controller.detachResource(resource.resourceId)}
                      >
                        <X size={15} aria-hidden="true" />
                      </button>
                    </li>
                  ))}
                </ul>
              ) : (
                <p>只读取你明确选择的文件；内容按当前模型与隐私设置处理，未允许的外传不会执行。</p>
              )}
            </div>
            <div
              className="ol-workspace-capabilities"
              aria-labelledby="workspace-capabilities-title"
            >
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
                      controller.busy ||
                      composerLocked ||
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
                      : "发送第一条消息时原子绑定；技能不会扩大模型、网络、工具或写入权限。"}
                  </small>
                  {controller.selectedSkillDetail && (
                    <details className="ol-workspace-capabilities__skill-boundary">
                      <summary>查看技能能力边界</summary>
                      <dl>
                        <div>
                          <dt>允许工具</dt>
                          <dd>
                            {controller.selectedSkillDetail.allowedTools.join(" · ") ||
                              "不额外允许工具"}
                          </dd>
                        </div>
                        <div>
                          <dt>明确禁止</dt>
                          <dd>
                            {controller.selectedSkillDetail.disallowedTools.join(" · ") ||
                              "无额外声明；仍受全局策略约束"}
                          </dd>
                        </div>
                        <div>
                          <dt>所需授权</dt>
                          <dd>
                            {controller.selectedSkillDetail.requiredPermissions.join(" · ") ||
                              "技能本身不授予权限"}
                          </dd>
                        </div>
                      </dl>
                    </details>
                  )}
                </label>
                <div className="ol-workspace-capabilities__tools">
                  <span>
                    <Wrench size={15} aria-hidden="true" />
                    已注册只读工具
                  </span>
                  {controller.toolCandidates?.candidates.length ? (
                    <ul aria-label="本轮已准入的只读工具">
                      {controller.toolCandidates.candidates.slice(0, 4).map(candidate => (
                        <li key={candidate.candidateId}>
                          <span>
                            <strong>{candidate.toolName}</strong>
                            <small>{candidate.capabilityLabels.join(" · ") || "read"}</small>
                          </span>
                          <small>{toolAdmissionLabel(candidate)}</small>
                          <small>{toolSelectionReasonLabel(candidate.selectionReason)}</small>
                        </li>
                      ))}
                    </ul>
                  ) : (
                    <small>当前没有可用的只读工具。</small>
                  )}
                  {Boolean(controller.toolCandidates?.blockedTools.length) && (
                    <details className="ol-workspace-capabilities__blocked-tools">
                      <summary>
                        {controller.toolCandidates!.blockedTools.length} 个工具未准入本轮
                      </summary>
                      <ul aria-label="本轮未准入的工具">
                        {controller.toolCandidates!.blockedTools.slice(0, 8).map(tool => (
                          <li key={`${tool.toolName}:${tool.blockerId ?? tool.reasonCode}`}>
                            <strong>{tool.toolName}</strong>
                            <small>{blockedToolReasonLabel(tool)}</small>
                            <small>
                              {tool.requiresPermission
                                ? "需要在具体动作发生时确认"
                                : "当前模式不会开放"}
                            </small>
                          </li>
                        ))}
                      </ul>
                    </details>
                  )}
                  {controller.toolCandidates?.failureRecovery && (
                    <small role="status">
                      上次工具执行未完成；请从任务结果发起受控重试，系统不会自动改用其他工具。
                    </small>
                  )}
                </div>
              </div>
              {controller.capabilityState.phase === "failed" && (
                <small role="status">
                  技能或工具状态不可用：{controller.capabilityState.reason}
                </small>
              )}
            </div>
          </details>
        )}
        <label htmlFor="workspace-composer-input">消息</label>
        <textarea
          id="workspace-composer-input"
          value={controller.draft}
          rows={3}
          placeholder="告诉 OpenLife 你现在要处理什么"
          disabled={
            controller.loadStatus !== "ready" ||
            composerLocked ||
            (controller.busy && controller.turnState.phase !== "streaming")
          }
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
              ? controller.mode === "work"
                ? "任务正在执行；可追加指令，或切换对话让它在后台继续"
                : "正在生成回复；需要改变方向时可先取消，再发送新消息"
              : controller.turnState.phase === "cancelling"
                ? "正在确认取消结果"
                : action.enabled
                  ? "Enter 发送，Shift + Enter 换行"
                  : (action.disabledReason ?? "当前不能发送")}
          </span>
          <div className="ol-workspace-composer__actions">
            {(controller.turnState.phase === "streaming" ||
              controller.turnState.phase === "cancelling") && (
              <>
                {controller.mode === "work" && controller.turnState.phase === "streaming" && (
                  <FoundationActionButton
                    label="追加指令"
                    icon={<Send size={16} aria-hidden="true" />}
                    disabled={!controller.draft.trim() || !controller.activeTaskId}
                    disabledReason={
                      !controller.activeTaskId
                        ? "当前工作尚未准备好接收追加指令。"
                        : !controller.draft.trim()
                          ? "先输入要追加的指令。"
                          : undefined
                    }
                    data-action-category="product"
                    data-action-id={`workspace.steer:${controller.activeTaskId ?? "unknown"}`}
                    data-action-kind="continue"
                    data-action-enabled={String(
                      Boolean(controller.draft.trim() && controller.activeTaskId)
                    )}
                    data-action-target-ref={controller.activeTaskId ?? "unknown"}
                    type="button"
                    onClick={() => void controller.steer()}
                  />
                )}
                <FoundationActionButton
                  label="停止回复"
                  icon={<Square size={16} aria-hidden="true" />}
                  loading={controller.turnState.phase === "cancelling"}
                  loadingLabel="正在停止"
                  disabled={controller.turnState.phase === "cancelling"}
                  disabledReason={
                    controller.turnState.phase === "cancelling"
                      ? "停止请求已发送；正在等待真实终态。"
                      : undefined
                  }
                  data-action-category="product"
                  data-action-id={`workspace.cancel:${controller.turnState.turnId}`}
                  data-action-kind="cancel"
                  data-action-enabled={String(controller.turnState.phase === "streaming")}
                  data-action-target-ref={controller.turnState.turnId}
                  type="button"
                  onClick={() => void controller.cancel()}
                />
              </>
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
        open={modelPickerOpen}
        title="选择模型"
        description="当前对话的新消息将使用这个模型；不会自动切换 Provider。"
        onClose={() => setModelPickerOpen(false)}
        footer={
          <FoundationActionButton
            label="关闭"
            variant="quiet"
            onClick={() => setModelPickerOpen(false)}
          />
        }
      >
        <div className="ol-workspace-model-picker">
          <label>
            <span>搜索模型</span>
            <input
              value={modelQuery}
              autoComplete="off"
              placeholder="输入模型名称，例如 ox-alpha"
              onChange={event => setModelQuery(event.target.value)}
            />
          </label>
          <div className="ol-workspace-model-picker__list" role="listbox" aria-label="可用模型">
            {matchingProviderProfiles.map(profile => {
              const available = profile.availability === "ready";
              const selected = profile.profileId === controller.provider.selectedProfileId;
              return (
                <button
                  type="button"
                  role="option"
                  key={profile.profileId}
                  disabled={!available}
                  aria-selected={selected}
                  aria-label={`${profile.displayName ?? profile.modelId}，${profile.providerId}，${profile.modelId}${selected ? "，当前模型" : available ? "" : "，不可用"}`}
                  onClick={async () => {
                    if (await controller.selectProviderProfile(profile.profileId)) {
                      setModelPickerOpen(false);
                    }
                  }}
                >
                  <span>
                    <strong>{profile.displayName ?? profile.modelId}</strong>
                    <small>
                      {profile.providerId} · {profile.modelId}
                    </small>
                  </span>
                  <span>{selected ? "当前" : available ? "" : "不可用"}</span>
                </button>
              );
            })}
          </div>
          {normalizedModelQuery && matchingProviderProfiles.length === 0 && (
            <p>没有找到可执行的匹配模型。</p>
          )}
        </div>
      </FoundationDialog>

      <FoundationDialog
        open={Boolean(projectRenameTarget)}
        title="重命名 Project"
        description="名称与范围修订会一起进入新的 Project revision；文件夹内容不会改变。"
        busy={sessionMutationBusy}
        onClose={() => setProjectRenameTargetId(null)}
        footer={
          <>
            <FoundationActionButton
              label="取消"
              variant="quiet"
              disabled={sessionMutationBusy}
              disabledReason={sessionMutationDisabledReason}
              onClick={() => setProjectRenameTargetId(null)}
            />
            <FoundationActionButton
              label="保存名称"
              variant="primary"
              loading={
                controller.sessionMutation.phase === "mutating_project" &&
                controller.sessionMutation.action === "update"
              }
              loadingLabel="正在保存"
              disabled={!projectRenameDraft.trim() || sessionMutationBusy}
              disabledReason={
                !projectRenameDraft.trim()
                  ? "Project 名称不能为空。"
                  : sessionMutationDisabledReason
              }
              onClick={() => {
                if (!projectRenameTarget) return;
                void controller
                  .updateProjectName(
                    projectRenameTarget.id,
                    projectRenameDraft,
                    projectRenameTarget.revision
                  )
                  .then(saved => {
                    if (saved) setProjectRenameTargetId(null);
                  });
              }}
            />
          </>
        }
      >
        <label className="ol-workspace-session-dialog__field">
          <span>Project 名称</span>
          <input
            value={projectRenameDraft}
            maxLength={120}
            disabled={sessionMutationBusy}
            onChange={event => setProjectRenameDraft(event.target.value)}
          />
        </label>
      </FoundationDialog>

      <FoundationDialog
        open={sessionDialog === "rename"}
        title="重命名这段对话"
        description="保存成功后，这段对话会使用新名称。"
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
