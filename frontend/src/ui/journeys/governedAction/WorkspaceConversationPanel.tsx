import {
  FilePlus2,
  FolderOpen,
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
import { useEffect, useState } from "react";
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
        <p>{state.cancelError}；当前轮次仍按运行中处理，可以再次请求取消。</p>
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

function lifeModelInfluenceFeedback(
  controller: WorkspaceConversationController,
  onOpenLifeModel: (itemRef: string) => void
) {
  const state = controller.turnState;
  if (state.phase !== "resolved" || !state.lifeModelInfluence) return null;
  const receipt = state.lifeModelInfluence;
  if (receipt.permissionGranted || receipt.durableWriteAuthorized) {
    return (
      <FoundationNotice title="Life Model 影响凭据无效" tone="error" live>
        <p>Life Model 不能授予权限或批准持久写入；本轮不会把该凭据解释为有效授权。</p>
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
                <code>{item.itemRef}</code> · 确认于 {item.confirmedAt} · {item.reasonCode}
                {item.sourceRefs.length > 0 && (
                  <>
                    <br />
                    <span>来源：{item.sourceRefs.join("、")}</span>
                  </>
                )}
                <br />
                <FoundationActionButton
                  label={`在个人智能中查看：${item.statement}`}
                  variant="quiet"
                  onClick={() => onOpenLifeModel(item.itemRef)}
                />
              </li>
            ))}
          </ul>
          <p>
            Life Model v{receipt.modelVersion ?? "未知"} · 当前指令优先：
            {receipt.currentInstructionPriorityPreserved ? "是" : "未确认"} · 安全策略优先：
            {receipt.policyPriorityPreserved ? "是" : "未确认"}
          </p>
        </details>
      )}
    </FoundationNotice>
  );
}

function sourceBoundBasisFeedback(controller: WorkspaceConversationController) {
  const state = controller.turnState;
  if (state.phase !== "resolved" || !state.sourceBoundBasis) return null;
  const basis = state.sourceBoundBasis;
  const sourceLabels: Record<string, string> = {
    current_message: "本轮消息",
    agent_memory: "Agent Memory",
    markdown_memory: "Markdown 工作记忆",
    document_or_resource: "选中文档或资源",
  };
  const answerWasBound =
    basis.checkStatus === "semantic_support_passed" ||
    basis.checkStatus === "deterministic_rendered";
  return (
    <FoundationNotice
      title={answerWasBound ? "本轮按限定资料回答" : "本轮限定资料边界"}
      tone="neutral"
    >
      <p>
        {answerWasBound
          ? "OpenLife 只允许使用你指定的资料，回答在展示前已完成边界核对。"
          : "OpenLife 没有展示无法在你指定资料范围内核对的回答。"}
      </p>
      <details>
        <summary>查看回答依据</summary>
        <ul>
          <li>
            采用资料：
            {basis.sourceTypes.map(type => sourceLabels[type] ?? "其他获准资料").join("、")}
          </li>
          <li>本轮事实块：{basis.factCount} 条</li>
          <li>
            核对状态：
            {basis.checkStatus === "semantic_support_passed"
              ? "逐句支持检查通过"
              : basis.checkStatus === "deterministic_rendered"
                ? "程序已按原文确定性输出"
                : basis.checkStatus === "no_evidence"
                  ? "限定范围内没有可用事实"
                  : basis.checkStatus === "failed_closed"
                    ? "核对未通过，已停止生成"
                    : "状态未知"}
          </li>
        </ul>
        <p>Life Model 如有参与，只影响表达方式，不作为事实来源。</p>
      </details>
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
    return "后端返回的文件凭据与当前回合不一致；为避免引用错文件，本轮没有接受该结果。";
  }
  if (code.includes("cancel")) return "文件读取已取消，没有把它显示为成功。";
  return `文件处理失败（${code}）。`;
}

function MarkdownMemoryPanel({ controller }: { controller: WorkspaceConversationController }) {
  const memory = controller.markdownMemory;
  const model = memory.model;
  const [scope, setScope] = useState<"workspace" | "project">("project");
  const [relativePath, setRelativePath] = useState("MEMORY.md");
  const [newFile, setNewFile] = useState(false);
  const selected = newFile
    ? undefined
    : model?.files.find(file => file.scope === scope && file.relativePath === relativePath);
  const [content, setContent] = useState("");
  const selectedKey = selected
    ? `${selected.scope}:${selected.relativePath}:${selected.contentDigest}`
    : "";

  useEffect(() => {
    setContent(selected?.content ?? "");
  }, [selected?.content, selectedKey]);

  const root = model?.roots.find(item => item.scope === scope);
  const busy = memory.phase === "submitting" || memory.phase === "selecting_root";

  return (
    <section className="ol-workspace-markdown-memory" aria-labelledby="workspace-memory-title">
      <div className="ol-workspace-resources__header">
        <div>
          <strong id="workspace-memory-title">Markdown 工作记忆</strong>
          <span>按 Workspace / Project 隔离；不是 Life Model</span>
        </div>
        <button
          type="button"
          className="ol-workspace-resources__add"
          disabled={busy}
          onClick={() => void controller.reloadMarkdownMemory()}
        >
          <RefreshCw size={15} aria-hidden="true" />
          重新读取
        </button>
      </div>

      {memory.phase === "failed" && (
        <FoundationNotice title="Markdown Memory 状态未确认" tone="error" live>
          <p>{memory.reason}</p>
        </FoundationNotice>
      )}
      {model?.truncated && (
        <FoundationNotice title="部分 Markdown Memory 未加载" tone="protection">
          <p>至少一个文件超出数量、大小或安全边界；当前不会把这次读取解释为完整。</p>
        </FoundationNotice>
      )}

      <div className="ol-workspace-markdown-memory__roots">
        {(["workspace", "project"] as const).map(candidateScope => {
          const candidate = model?.roots.find(item => item.scope === candidateScope);
          return (
            <button
              key={candidateScope}
              type="button"
              className={scope === candidateScope ? "is-active" : undefined}
              disabled={busy}
              onClick={() => setScope(candidateScope)}
            >
              <strong>{candidateScope === "workspace" ? "Workspace" : "Project"}</strong>
              <span>{candidate?.status === "ready" ? "已绑定" : "未绑定"}</span>
            </button>
          );
        })}
        <button
          type="button"
          disabled={busy}
          onClick={() => void controller.selectMarkdownMemoryRoot(scope)}
        >
          <FolderOpen size={15} aria-hidden="true" />
          {root?.configured ? "更换文件夹" : "选择文件夹"}
        </button>
      </div>

      {root?.status === "ready" ? (
        <div className="ol-workspace-markdown-memory__editor">
          <label>
            <span>文件</span>
            <select
              value={selected ? `${selected.scope}:${selected.relativePath}` : "new"}
              disabled={busy}
              onChange={event => {
                const value = event.target.value;
                if (value === "new") {
                  setNewFile(true);
                  setRelativePath("MEMORY.md");
                  setContent("");
                  return;
                }
                const [, ...pathParts] = value.split(":");
                setNewFile(false);
                setRelativePath(pathParts.join(":"));
              }}
            >
              <option value="new">新建或替换 MEMORY.md</option>
              {model?.files
                .filter(file => file.scope === scope)
                .map(file => (
                  <option
                    key={`${file.scope}:${file.relativePath}`}
                    value={`${file.scope}:${file.relativePath}`}
                  >
                    {file.relativePath}
                  </option>
                ))}
            </select>
          </label>
          <label>
            <span>相对路径</span>
            <input
              value={relativePath}
              disabled={Boolean(selected) || busy}
              onChange={event => setRelativePath(event.target.value)}
              placeholder="MEMORY.md 或 memories/topic.md"
            />
          </label>
          <label>
            <span>Markdown 内容</span>
            <textarea
              rows={7}
              value={content}
              disabled={busy}
              onChange={event => setContent(event.target.value)}
            />
          </label>
          <div className="ol-workspace-markdown-memory__actions">
            <FoundationActionButton
              label="提交到 Review"
              icon={<Pencil size={15} aria-hidden="true" />}
              loading={memory.phase === "submitting" && memory.operation === "write"}
              disabled={busy || !relativePath.trim() || !content.trim()}
              disabledReason={
                busy
                  ? "变更正在提交到 Review；文件仍未修改。"
                  : !relativePath.trim()
                    ? "先填写相对路径。"
                    : !content.trim()
                      ? "先填写 Markdown 内容。"
                      : undefined
              }
              onClick={() =>
                void controller.proposeMarkdownMemoryWrite({
                  scope,
                  relativePath,
                  content,
                  ...(selected ? { expectedCurrentDigest: selected.contentDigest } : {}),
                })
              }
            />
            {selected && (
              <FoundationActionButton
                label="停用"
                icon={<Trash2 size={15} aria-hidden="true" />}
                variant="quiet"
                loading={memory.phase === "submitting" && memory.operation === "deactivate"}
                disabled={busy}
                disabledReason={
                  busy ? "另一项 Memory 变更正在提交；当前文件仍保持启用。" : undefined
                }
                onClick={() =>
                  void controller.proposeMarkdownMemoryDeactivation({
                    scope,
                    relativePath: selected.relativePath,
                    expectedCurrentDigest: selected.contentDigest,
                  })
                }
              />
            )}
          </div>
          <small>
            提交只创建 Review
            项；批准并确认物化前，文件不会改变。运行时只选择与当前任务相关的有界段落。
          </small>
        </div>
      ) : (
        <p>先为当前作用域选择一个真实文件夹；其他文件夹中的 Memory 不会被召回。</p>
      )}

      {memory.phase === "ready" && memory.lastProposal && (
        <FoundationNotice title="等待 Review" tone="protection" live>
          <p>
            {memory.lastProposal.relativePath} 的
            {memory.lastProposal.operation === "write" ? "写入" : "停用"}
            仍待审核；当前不显示为已应用。
          </p>
        </FoundationNotice>
      )}
    </section>
  );
}

export function WorkspaceConversationPanel({
  controller,
  onOpenLifeModel,
  disabledReason,
}: {
  controller: WorkspaceConversationController;
  onOpenLifeModel: (itemRef: string) => void;
  disabledReason?: string;
}) {
  const [sessionDialog, setSessionDialog] = useState<"rename" | "delete" | null>(null);
  const [renameDraft, setRenameDraft] = useState("");
  const [projectNameDraft, setProjectNameDraft] = useState("");
  const action = controller.sendAction(disabledReason);
  const visibleMessages = controller.messages.filter(message => message.role !== "system");
  const selectedSession = controller.sessions.find(
    session => session.session_id === controller.selectedSessionId
  );
  const sessionMutationBusy = [
    "renaming",
    "deleting",
    "creating_project",
    "assigning_project",
  ].includes(controller.sessionMutation.phase);
  const resourceMutationBusy = ["importing", "detaching"].includes(
    controller.resourceMutation.phase
  );
  const pendingResourceCount = controller.pendingResources.length;
  const backgroundWorkCanDetach =
    controller.mode === "work" && controller.turnState.phase === "streaming";
  const conversationSwitchLocked =
    (controller.busy && !backgroundWorkCanDetach) || pendingResourceCount > 0;
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
          <label>
            <span className="ol-visually-hidden">选择 Project</span>
            <select
              value={controller.selectedProjectId ?? ""}
              disabled={!controller.selectedSessionId || controller.busy || sessionMutationBusy}
              onChange={event => void controller.assignProject(event.target.value || null)}
            >
              <option value="">不属于 Project</option>
              {controller.projects.map(project => (
                <option key={project.id} value={project.id}>
                  {project.name}
                </option>
              ))}
            </select>
          </label>
          <label>
            <span className="ol-visually-hidden">新 Project 名称</span>
            <input
              value={projectNameDraft}
              placeholder="新 Project"
              disabled={controller.busy || sessionMutationBusy}
              onChange={event => setProjectNameDraft(event.target.value)}
            />
          </label>
          <button
            type="button"
            className="ol-workspace-conversation__new"
            disabled={!projectNameDraft.trim() || controller.busy || sessionMutationBusy}
            onClick={() => {
              const name = projectNameDraft;
              void controller.createProject(name).then(created => {
                if (created) setProjectNameDraft("");
              });
            }}
          >
            <FolderOpen size={15} aria-hidden="true" />新 Project
          </button>
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

      {turnFeedback(controller)}
      {sourceBoundBasisFeedback(controller)}
      {lifeModelInfluenceFeedback(controller, onOpenLifeModel)}

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

      {controller.mode === "work" && controller.workStatus === "ready" && (
        <MarkdownMemoryPanel controller={controller} />
      )}

      <form
        className="ol-workspace-composer"
        onSubmit={event => {
          event.preventDefault();
          void controller.send(disabledReason);
        }}
      >
        <fieldset className="ol-workspace-mode" disabled={controller.busy}>
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
              disabled={controller.workStatus !== "ready"}
              onChange={() => controller.setMode("work")}
            />
            Work
          </label>
          <small>
            {controller.mode === "chat"
              ? "直接对话，不创建任务。"
              : "可使用文件、工具与受治理动作完成任务。"}
            {controller.workStatus === "reconstructing" &&
              " Work 当前不可用；不会回退到旧执行路径。"}
          </small>
        </fieldset>
        {controller.mode === "chat" && (
          <div className="ol-workspace-provider" aria-live="polite">
            {controller.provider.status === "ready" ? (
              <span>
                当前模型：
                {controller.provider.profiles.find(profile => profile.selected)?.providerId ??
                  "未知供应商"}
                {" · "}
                {controller.provider.profiles.find(profile => profile.selected)?.modelId ??
                  "未知模型"}
              </span>
            ) : controller.provider.status === "unavailable" ? (
              <span>当前模型不可用；请在设置中选择并配置模型。</span>
            ) : (
              <span>正在核对当前模型。</span>
            )}
          </div>
        )}
        {controller.mode === "work" && (
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
        )}
        {controller.mode === "work" && (
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
        )}
        <label htmlFor="workspace-composer-input">消息</label>
        <textarea
          id="workspace-composer-input"
          value={controller.draft}
          rows={3}
          placeholder="告诉 OpenLife 你现在要处理什么"
          disabled={
            controller.loadStatus !== "ready" ||
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
                ? "正在等待后端确认取消终态"
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
                    disabled={!controller.draft.trim() || !controller.activeTaskSessionId}
                    disabledReason={
                      !controller.activeTaskSessionId
                        ? "后端尚未返回当前 Work Task 身份。"
                        : !controller.draft.trim()
                          ? "先输入要追加的指令。"
                          : undefined
                    }
                    data-action-category="product"
                    data-action-id={`workspace.steer:${controller.activeTaskSessionId ?? "unknown"}`}
                    data-action-kind="continue"
                    data-action-enabled={String(
                      Boolean(controller.draft.trim() && controller.activeTaskSessionId)
                    )}
                    data-action-target-ref={controller.activeTaskSessionId ?? "unknown"}
                    type="button"
                    onClick={() => void controller.steer()}
                  />
                )}
                <FoundationActionButton
                  label="停止回复"
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
