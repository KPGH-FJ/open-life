import { useMemo, useState } from "react";
import {
  Download,
  FileSearch,
  FolderOpen,
  PencilLine,
  Play,
  RefreshCw,
  RotateCcw,
  Search,
  XCircle,
} from "lucide-react";
import type {
  CanonicalTaskItemKind,
  CanonicalTaskItemStatus,
  ProductAction,
  TaskControl,
  TaskViewModelItem,
  TasksViewModel,
  ViewModelEnvelope,
} from "@/tauri";
import {
  FoundationActionButton,
  FoundationDialog,
  FoundationNotice,
  FoundationStatusLabel,
} from "@/ui/foundation";
import {
  isExecutableTaskControl,
  type TaskControlDispatchState,
} from "@/features/work/taskControlContract";
import { productErrorMessage } from "@/shared/productError";
import {
  formatBackendTime,
  taskNeedsAttention,
  taskLifecyclePresentation,
  taskPrimaryQuestion,
} from "@/features/work/taskPresentation";

type TaskFilter = "all" | "attention" | "active" | "terminal";
type ArtifactActionFailure = {
  artifactId: string;
  action: "open" | "export" | "undo" | "revision";
  error: unknown;
};

function taskMatchesFilter(item: TaskViewModelItem, filter: TaskFilter): boolean {
  if (filter === "all") return true;
  if (filter === "active") {
    return ["running", "waiting_review", "waiting_permission", "blocked"].includes(
      item.lifecycleStatus
    );
  }
  if (filter === "attention") {
    return taskNeedsAttention(item);
  }
  return [
    "completed",
    "failed",
    "remote_unknown",
    "cancelled",
    "completed_needs_evidence",
  ].includes(item.lifecycleStatus);
}

function taskSearchText(item: TaskViewModelItem): string {
  return [
    item.title,
    item.lifecycleStatus,
    item.pendingBlockers.join(" "),
    (item.attentionReasonCodes ?? []).join(" "),
    item.pendingReviewItemRefs.map(ref => ref.label).join(" "),
    item.latestResultPreview?.label ?? "",
    item.latestResultPreview?.preview ?? "",
    item.workPlan?.steps.map(step => step.kind).join(" ") ?? "",
    item.latestRunProvenance
      ? `${item.latestRunProvenance.providerId} ${item.latestRunProvenance.modelId} ${item.latestRunProvenance.projectName ?? ""}`
      : "",
    item.artifacts.map(artifact => artifact.materializedReference ?? artifact.mediaType).join(" "),
  ]
    .join(" ")
    .toLocaleLowerCase("zh-CN");
}

function provenanceLabel(
  provenance: NonNullable<TaskViewModelItem["latestRunProvenance"]>
): string {
  const project = provenance.projectName
    ? ` · Project ${provenance.projectName} r${provenance.projectRevision ?? "?"}`
    : "";
  const reasoning = provenance.reasoningEffort
    ? ` · 推理 ${provenance.reasoningEffort}`
    : " · 模型默认推理";
  const executionMode = provenance.executionMode === "observe_only" ? " · 只读研究" : " · 标准执行";
  return `${provenance.providerId} · ${provenance.modelId}${reasoning}${executionMode}${project}`;
}

function runProvenanceLabel(item: TaskViewModelItem): string | null {
  return item.latestRunProvenance ? provenanceLabel(item.latestRunProvenance) : null;
}

function artifactFailureTitle(action: ArtifactActionFailure["action"]): string {
  if (action === "open") return "文件没有打开";
  if (action === "export") return "文件没有另存";
  if (action === "revision") return "聚焦修订没有开始";
  return "撤销申请没有创建";
}

function artifactFailureFallback(action: ArtifactActionFailure["action"]): string {
  if (action === "open") return "文件核验或打开没有完成；原文件没有被修改。";
  if (action === "export") return "文件另存或写后核验没有完成；请重新选择位置后重试。";
  if (action === "revision") return "新修订运行没有创建；当前版本仍保持原状态。";
  return "撤销申请没有进入审核；当前产物仍保持原状态。";
}

function statusDetail(item: TaskViewModelItem): string {
  if (item.latestRunProvenance?.turnErrorCode) {
    return `模型执行未完成：${reasonLabel(item.latestRunProvenance.turnErrorCode)}`;
  }
  if (item.needsAttention && item.attentionReasonCodes?.[0]) {
    return `需要处理：${reasonLabel(item.attentionReasonCodes[0])}`;
  }
  if (item.lifecycleStatus === "waiting_review")
    return "当前运行已停在一个审核节点；完成决定后，系统只会继续该节点绑定的动作。";
  if (item.pendingReviewItemRefs.length > 0) return "有事项等待决定，任务尚未完成。";
  if (item.pendingBlockers.length > 0) return reasonLabel(item.pendingBlockers[0]);
  if (item.latestResultPreview?.preview) return item.latestResultPreview.preview;
  if (item.lifecycleStatus === "running") return "任务仍在执行。";
  if (item.lifecycleStatus === "remote_unknown") return "远端执行结果未知，不能标记为完成。";
  if (item.lifecycleStatus === "interrupted")
    return "任务因应用中断而停止，可以从保留的任务记录重试。";
  return "查看依据可核对当前生命周期与交付状态。";
}

function reasonLabel(reason: string): string {
  const labels: Record<string, string> = {
    read_tool_blocked: "所需资料当前不可访问",
    tool_permission_required: "当前工具动作正在等待你的授权",
    tool_permission_rejected: "你已拒绝当前工具动作；本轮工作已停止",
    tool_review_live_continuation_unavailable:
      "授权已记录，但原运行进程已中断；可以从保留的任务创建新的继续运行",
    web_search_challenge_detected: "搜索服务要求人机验证，请稍后重试或改用已配置的搜索服务",
    web_search_no_structured_results: "搜索服务没有返回可核验的结果",
    web_artifact_source_validation_failed: "当前读取的来源不足以支持这份结果",
    web_artifact_citation_missing: "生成的结果没有绑定到本轮已读取的来源",
    web_artifact_citation_not_in_body: "来源标记没有放在它所支持的正文陈述附近",
    web_artifact_citation_unknown: "生成的结果引用了本轮没有签发的来源标记",
    web_artifact_citation_run_mismatch: "来源标记不属于当前这轮工作",
    web_artifact_url_not_observed: "生成的结果写入了本轮没有实际读取的来源链接",
    web_fetch_distinct_search_result_missing: "搜索结果中没有更多可读取的不同来源",
    artifact_effect_unknown: "结果写入状态需要人工核对",
    artifact_delivery_failed: "结果交付失败",
    artifact_waiting_materialization: "结果尚未写入目标位置",
    artifact_content_digest_drift: "目标内容与已核验版本不一致",
    artifact_target_precondition_changed: "审核后目标文件已变化；OpenLife 没有覆盖用户的新内容",
    artifact_materialized_reference_missing: "结果位置缺少确认依据",
    artifact_preview_source_unavailable: "结果预览来源不可用",
    artifact_undone: "该版本已按用户请求撤销；这里保留撤销前已核验的历史记录",
    artifact_undo_confirmation_incomplete: "撤销记录缺少完整的执行回执，需要人工核对",
    artifact_undo_prior_verification_missing: "撤销前版本缺少完整的物化核验依据",
    artifact_undo_pending_or_failed: "撤销仍在等待决定或执行失败",
    artifact_undo_unavailable_without_original_bytes: "缺少可恢复的原始内容",
    artifact_undo_requires_verified_materialization: "结果核验后才能撤销",
    artifact_undo_unavailable: "当前结果不可撤销",
    artifact_revision_requires_completed_task: "任务完成后才能继续修改这个版本",
    artifact_revision_requires_verified_current_version: "当前版本通过完整性核验后才能继续修改",
    artifact_revision_conflicts_with_undo: "当前产物已有撤销记录，不能同时开始修订",
    work_provider_binding_stale: "Provider 或模型已经变化，请作为新的工作重新提交",
    work_project_assignment_stale: "对话所属 Project 已经变化，请作为新的工作重新提交",
    work_project_scope_stale: "Project 范围已经变化，请核对后作为新的工作重新提交",
    work_skill_binding_stale: "所选技能已经变化，请作为新的工作重新提交",
    canonical_work_observe_only_write_forbidden:
      "本轮采用只读研究模式，系统已阻止创建文件或写入个人长期状态",
    provider_quota_exhausted: "当前模型额度不足，额度恢复后可以重试",
    agent_step_artifact_content_type_invalid:
      "所选模型返回了不符合 Agent 执行契约的结果；请选择已验证支持 Work 的模型后重试",
    work_semantic_verification_needs_more_evidence: "现有来源不足以支持交付要求",
    work_semantic_verification_stalled: "现有来源仍不足，任务已停止，没有交付未经支持的结果",
  };
  if (labels[reason]) return labels[reason];
  return /^[a-z0-9_.:-]+$/i.test(reason) ? "系统要求核对这项状态" : reason;
}

function artifactTypeLabel(mediaType: string): string {
  const normalized = mediaType.split(";")[0]?.trim().toLocaleLowerCase();
  if (normalized === "text/markdown") return "Markdown 结果";
  if (normalized === "text/html") return "HTML 结果";
  if (normalized === "application/pdf") return "PDF 结果";
  if (normalized === "application/vnd.openxmlformats-officedocument.wordprocessingml.document")
    return "Word 文档";
  if (normalized === "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet")
    return "Excel 表格";
  if (normalized === "application/vnd.openxmlformats-officedocument.presentationml.presentation")
    return "PowerPoint 演示文稿";
  if (normalized === "text/csv") return "CSV 表格";
  if (normalized === "application/json") return "JSON 结果";
  if (normalized === "text/plain") return "文本结果";
  return normalized || "任务产物";
}

function artifactPreviewIsExtractedText(mediaType: string): boolean {
  const normalized = mediaType.split(";")[0]?.trim().toLocaleLowerCase();
  return [
    "application/pdf",
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    "application/vnd.openxmlformats-officedocument.presentationml.presentation",
  ].includes(normalized);
}

function artifactPreviewLabel(mediaType: string, historical: boolean): string {
  if (artifactPreviewIsExtractedText(mediaType)) {
    return historical ? "历史版本提取内容" : "提取内容";
  }
  return historical ? "历史版本预览" : "预览";
}

function artifactStatusLabel(status: TaskViewModelItem["artifacts"][number]["status"]): string {
  if (status === "materialized") return "已物化";
  if (status === "waiting_review") return "等待审核";
  if (status === "effect_unknown") return "结果未知";
  if (status === "failed") return "交付失败";
  return "草稿";
}

function artifactVerificationLabel(
  status: TaskViewModelItem["artifacts"][number]["verification"]["status"]
): string {
  if (status === "verified") return "文件完整性已核验";
  if (status === "pending") return "等待文件完整性核验";
  if (status === "failed") return "文件完整性核验失败";
  return "文件完整性未知";
}

function artifactChangeLabel(
  kind: TaskViewModelItem["artifacts"][number]["change"]["kind"]
): string {
  if (kind === "create") return "创建文件";
  if (kind === "replace") return "替换文件";
  if (kind === "rename") return "重命名文件";
  return "变更范围未知";
}

function taskItemKindLabel(kind: CanonicalTaskItemKind): string {
  if (kind === "instruction") return "任务输入";
  if (kind === "plan") return "执行计划";
  if (kind === "steering") return "追加指令";
  if (kind === "tool_call") return "工具调用";
  if (kind === "observation") return "工具结果";
  if (kind === "provider_generation") return "模型生成";
  if (kind === "artifact_draft") return "结果草稿";
  if (kind === "review_checkpoint") return "审核节点";
  if (kind === "artifact_materialized") return "结果写入";
  if (kind === "verification") return "结果核验";
  return "最终结果";
}

function taskItemStatusLabel(status: CanonicalTaskItemStatus): string {
  if (status === "waiting") return "等待";
  if (status === "running") return "执行中";
  if (status === "completed") return "已完成";
  if (status === "blocked") return "已阻塞";
  if (status === "failed") return "失败";
  if (status === "cancelled") return "已取消";
  if (status === "interrupted") return "已中断";
  return "结果未知";
}

function steeringStatusLabel(
  status: NonNullable<TaskViewModelItem["steerings"]>[number]["status"]
): string {
  if (status === "pending") return "等待安全检查点";
  if (status === "applied") return "已应用";
  if (status === "blocked") return "范围扩大已阻断";
  return "未应用";
}

function taskItemStatusTone(
  status: CanonicalTaskItemStatus
): "waiting" | "success" | "error" | "unknown" {
  if (status === "completed") return "success";
  if (["blocked", "failed", "cancelled", "interrupted"].includes(status)) return "error";
  if (status === "effect_unknown") return "unknown";
  return "waiting";
}

function workPlanStepLabel(
  kind: NonNullable<TaskViewModelItem["workPlan"]>["steps"][number]["kind"]
): string {
  const labels: Record<typeof kind, string> = {
    analyze: "分析任务",
    read_imported_document: "读取导入文档",
    read_workspace_file: "读取工作区文件",
    web_search: "搜索 Web",
    web_fetch: "读取网页",
    use_selected_skill: "应用所选 Skill",
    read_mcp: "读取已连接工具",
    draft_artifact: "起草结果",
    verify: "核验结果",
    deliver_result: "交付结果",
  };
  return labels[kind];
}

function completionContractLabel(plan: NonNullable<TaskViewModelItem["workPlan"]>): string {
  const result = plan.completion.resultKind === "artifact" ? "交付可审阅产物" : "交付最终回答";
  return plan.completion.requiresVerification ? `${result}，并完成结果核验` : result;
}

function CompletionLimitations({ task }: { task: TaskViewModelItem }) {
  if (task.completionDisposition !== "complete_with_disclosed_limitations") return null;
  return (
    <section className="ol-task-result-card__section" aria-label="已说明限制">
      <span>已说明限制</span>
      {task.completionLimitations.length > 0 ? (
        <ul>
          {task.completionLimitations.map(limitation => (
            <li key={limitation.requirementId}>
              <strong>{limitation.description}</strong>
              <small>要求：{limitation.requirementId} · 这是限制披露，不是来源支持。</small>
            </li>
          ))}
        </ul>
      ) : (
        <p>旧结果只保留了“含限制”的结论，具体限制条目不可用；不会补猜。</p>
      )}
    </section>
  );
}

function taskItemSummary(summaryCode: string): string {
  const [, tool] = summaryCode.split(":", 2);
  const toolLabel =
    tool === "document.read"
      ? "本地文档"
      : tool === "folder.list"
        ? "Project 目录"
        : tool === "file.search"
          ? "Project 文件搜索"
          : tool === "file.read"
            ? "Project 文件"
            : tool === "web.search"
              ? "Web 搜索"
              : tool === "web.fetch"
                ? "网页读取"
                : tool === "mcp.read_only"
                  ? "MCP 只读工具"
                  : tool;
  if (summaryCode.startsWith("work_tool_call:") && toolLabel) return `调用 ${toolLabel}`;
  if (summaryCode.startsWith("work_tool_observation:") && toolLabel)
    return `已取得 ${toolLabel} 的结果`;
  if (summaryCode === "legacy_work_personal_intelligence_unproven")
    return "历史 Work 个人智能路径已停用，不能作为任务完成证据";
  if (summaryCode === "work_selected_skill_context_applied") return "已应用所选 Skill 的指令";
  if (summaryCode === "work_provider_generation") return "模型正在生成结果";
  if (summaryCode === "work_provider_generation_completed") return "模型结果已经生成";
  if (summaryCode === "work_completed") return "任务结果已经交付";
  return "执行记录已更新";
}

function actionAttributes(action: ProductAction) {
  return {
    "data-action-category": "product",
    "data-action-id": action.id,
    "data-action-kind": action.kind,
    "data-action-enabled": String(action.enabled),
    "data-action-disabled-reason": action.disabledReason ?? "",
    "data-action-target-ref": action.targetRef ?? "",
  } as const;
}

function taskControlLabel(control: TaskControl): string {
  if (control.kind === "resume") return "继续并创建新运行";
  if (control.kind === "retry") return "重试并创建新运行";
  if (control.kind === "stop_run") return "停止当前运行";
  return control.label;
}

function taskControlIcon(control: TaskControl) {
  if (control.kind === "resume") return <Play size={17} aria-hidden="true" />;
  if (control.kind === "retry") return <RotateCcw size={17} aria-hidden="true" />;
  if (control.kind === "stop_run") return <XCircle size={17} aria-hidden="true" />;
  return <RefreshCw size={17} aria-hidden="true" />;
}

function taskControlFeedback(state: TaskControlDispatchState) {
  if (state.phase === "idle" || state.phase === "confirming") return null;
  if (state.phase === "blocked") {
    return {
      title: "当前不能执行这项动作",
      body: productErrorMessage(state.reason, "当前任务状态不允许执行这个动作。"),
      tone: "protection" as const,
    };
  }
  if (state.phase === "dispatching") {
    return {
      title: "正在发送任务请求",
      body: "命令返回不代表任务已经改变。",
      tone: "neutral" as const,
    };
  }
  if (state.phase === "refreshing") {
    return {
      title: "正在核对任务状态",
      body: "正在刷新同一项工作的状态。",
      tone: "neutral" as const,
    };
  }
  if (state.phase === "awaiting_projection") {
    return {
      title: "任务变化尚未确认",
      body: "请求已发送，但刷新后的同一任务还没有证明目标变化。",
      tone: "protection" as const,
    };
  }
  if (state.phase === "failed") {
    return {
      title: state.stage === "dispatch" ? "任务请求失败" : "任务状态核对失败",
      body: productErrorMessage(state.errorCode, "任务操作没有完成，请重试。"),
      tone: "error" as const,
    };
  }
  if (
    state.refreshedTask.lifecycleStatus === "completed" &&
    state.refreshedTask.terminalDeliveryStatus === "delivered" &&
    state.refreshedTask.finalDeliveryEvidencePresent
  ) {
    return null;
  }
  return {
    title: state.control.kind === "stop_run" ? "当前运行已停止" : "任务状态已更新",
    body:
      state.control.kind === "stop_run"
        ? "刷新后的同一任务已确认当前运行终止；继续会创建新运行。"
        : `刷新后的同一任务当前为 ${state.refreshedTask.lifecycleStatus}，且已创建新运行；这不是完成证明。`,
    tone: "neutral" as const,
  };
}

export function ResultsView({
  envelope,
  refreshing,
  selectedTaskId,
  onRefresh,
  onSelectTask,
  onOpenInspector,
  onAnnounce,
  taskControlState,
  onRequestTaskControl,
  onConfirmTaskControl,
  onCancelTaskControlConfirmation,
  onRequestArtifactUndo,
  onRequestTaskArtifactUndo,
  onReviseArtifact,
  onOpenArtifact,
  onExportArtifact,
  fixedFilter,
  scopedItems,
  embedded = false,
  detailOnly = false,
}: {
  envelope: ViewModelEnvelope<TasksViewModel>;
  refreshing: boolean;
  selectedTaskId: string | null;
  onRefresh: () => void;
  onSelectTask: (task: TaskViewModelItem) => void;
  onOpenInspector: () => void;
  onAnnounce: (message: string) => void;
  taskControlState: TaskControlDispatchState;
  onRequestTaskControl: (control: TaskControl, expectedTaskId: string) => void;
  onConfirmTaskControl: () => void;
  onCancelTaskControlConfirmation: () => void;
  onRequestArtifactUndo: (artifactId: string) => Promise<void>;
  onRequestTaskArtifactUndo: (taskId: string) => Promise<void>;
  onReviseArtifact: (
    taskId: string,
    artifactId: string,
    baseVersion: number,
    instruction: string
  ) => Promise<void>;
  onOpenArtifact: (artifactId: string, version: number) => Promise<void>;
  onExportArtifact: (artifactId: string, version: number) => Promise<void>;
  fixedFilter?: TaskFilter;
  scopedItems?: readonly TaskViewModelItem[];
  embedded?: boolean;
  detailOnly?: boolean;
}) {
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<TaskFilter>("all");
  const [undoingArtifactId, setUndoingArtifactId] = useState<string | null>(null);
  const [undoingTaskId, setUndoingTaskId] = useState<string | null>(null);
  const [taskUndoFailure, setTaskUndoFailure] = useState<unknown>(null);
  const [revisingArtifactId, setRevisingArtifactId] = useState<string | null>(null);
  const [revisionDraft, setRevisionDraft] = useState<{
    artifactId: string;
    instruction: string;
  } | null>(null);
  const [openingArtifactId, setOpeningArtifactId] = useState<string | null>(null);
  const [exportingArtifactId, setExportingArtifactId] = useState<string | null>(null);
  const [artifactActionFailure, setArtifactActionFailure] = useState<ArtifactActionFailure | null>(
    null
  );
  const items = scopedItems ? [...scopedItems] : (envelope.data?.items ?? []);
  const listAvailable = envelope.data !== null && !["error", "loading"].includes(envelope.status);
  const normalizedQuery = query.trim().toLocaleLowerCase("zh-CN");
  const activeFilter = fixedFilter ?? filter;
  const visibleItems = useMemo(
    () =>
      items.filter(
        item =>
          taskMatchesFilter(item, activeFilter) &&
          (!normalizedQuery || taskSearchText(item).includes(normalizedQuery))
      ),
    [activeFilter, items, normalizedQuery]
  );
  const selectedTask = items.find(item => item.canonicalTaskId === selectedTaskId) ?? null;
  const executableControls = selectedTask?.allowedControls.filter(isExecutableTaskControl) ?? [];
  const controlFeedback = taskControlFeedback(taskControlState);
  const taskControlBusy = ["dispatching", "refreshing"].includes(taskControlState.phase);
  const refreshAction = {
    id: "tasks.refresh",
    label: "重新读取工作状态",
    kind: "refresh",
    enabled: !refreshing,
    disabledReason: refreshing ? "任务列表正在刷新。" : undefined,
    targetRef: "TasksViewModel",
  } satisfies ProductAction;
  const inspectAction = {
    id: "tasks.inspect_evidence",
    label: "查看工作依据",
    kind: "inspect",
    enabled: true,
    targetRef: selectedTaskId ? `task:${selectedTaskId}` : "TasksViewModel.sourceRefs",
  } satisfies ProductAction;

  function announceVisible(nextQuery: string, nextFilter: TaskFilter): void {
    const normalized = nextQuery.trim().toLocaleLowerCase("zh-CN");
    const count = items.filter(
      item =>
        taskMatchesFilter(item, nextFilter) &&
        (!normalized || taskSearchText(item).includes(normalized))
    ).length;
    onAnnounce(`任务列表已更新，当前显示 ${count} 项。`);
  }

  return (
    <article
      className={`ol-workbench-result-page${embedded ? " ol-workbench-result-page--embedded" : ""}${detailOnly ? " ol-workbench-result-page--detail-only" : ""}`}
      data-testid="tasks-product-view"
    >
      {!detailOnly && (
        <header className="ol-workbench-result-page-heading ol-workbench-result-page-heading--with-actions">
          <div>
            <span>{embedded ? "当前对话" : "哪些工作需要我或可以继续"}</span>
            <h2>{embedded ? "进度与结果" : taskPrimaryQuestion(envelope)}</h2>
            <p>
              {embedded
                ? "计划、重要进度、待决定事项和最终结果会在这里持续更新。"
                : "这里显示 OpenLife 已确认的进度、阻塞、结果和可用动作。"}
            </p>
          </div>
          {!fixedFilter && (
            <FoundationActionButton
              label="重新读取"
              variant="quiet"
              icon={<RefreshCw size={18} strokeWidth={1.75} aria-hidden="true" />}
              loading={refreshing}
              loadingLabel="正在读取"
              {...actionAttributes(refreshAction)}
              onClick={onRefresh}
            />
          )}
        </header>
      )}

      {envelope.status === "loading" && (
        <FoundationNotice title="正在读取任务列表" tone="neutral" live>
          读取完成前不推断任务状态或恢复能力。
        </FoundationNotice>
      )}

      {envelope.status === "error" && (
        <FoundationNotice title="任务状态读取失败" tone="error">
          系统没有返回可确认的任务状态；当前数量未知，不会把读取失败显示为零项。原始错误可在检查器中核对。
        </FoundationNotice>
      )}

      {envelope.status === "stale" && (
        <FoundationNotice title="任务列表已陈旧，只用于查看" tone="protection">
          当前不会开放恢复、重试、取消或其他可能改变任务状态的动作。
        </FoundationNotice>
      )}

      {listAvailable && !detailOnly && (
        <section className="ol-workbench-result-section" aria-labelledby="tasks-list-title">
          <div className="ol-workbench-result-section-heading ol-workbench-result-task-tools-heading">
            <div>
              <span>工作</span>
              <h3 id="tasks-list-title">最近工作</h3>
            </div>
            {items.length > 5 && (
              <div className="ol-workbench-result-task-tools">
                <label className="ol-workbench-result-search">
                  <span className="ol-sr-only">搜索任务</span>
                  <Search size={17} strokeWidth={1.75} aria-hidden="true" />
                  <input
                    type="search"
                    value={query}
                    placeholder="搜索任务"
                    onChange={event => {
                      setQuery(event.target.value);
                      announceVisible(event.target.value, activeFilter);
                    }}
                  />
                </label>
                {!fixedFilter && (
                  <label className="ol-workbench-result-filter">
                    <span className="ol-sr-only">筛选任务</span>
                    <select
                      value={filter}
                      onChange={event => {
                        const nextFilter = event.target.value as TaskFilter;
                        setFilter(nextFilter);
                        announceVisible(query, nextFilter);
                      }}
                    >
                      <option value="all">全部</option>
                      <option value="attention">需要处理</option>
                      <option value="active">进行中</option>
                      <option value="terminal">最近结果</option>
                    </select>
                  </label>
                )}
              </div>
            )}
          </div>

          {items.length > 1 && (
            <p className="ol-workbench-result-list-count">
              共 {items.length} 项，当前显示 {visibleItems.length} 项
            </p>
          )}

          {visibleItems.length > 0 ? (
            <div className="ol-workbench-result-task-list">
              {visibleItems.map(item => {
                const lifecycle = taskLifecyclePresentation(item);
                const updatedAt = formatBackendTime(item.updatedAt);
                const provenance = runProvenanceLabel(item);
                return (
                  <button
                    key={item.canonicalTaskId}
                    type="button"
                    className="ol-workbench-result-task-row"
                    data-selected={selectedTaskId === item.canonicalTaskId ? "true" : "false"}
                    aria-pressed={selectedTaskId === item.canonicalTaskId}
                    onClick={() => onSelectTask(item)}
                  >
                    <span className="ol-workbench-result-task-row__copy">
                      <strong>{item.title}</strong>
                      <span>{statusDetail(item)}</span>
                      {provenance && <small>{provenance}</small>}
                      {updatedAt && <small>最近更新 {updatedAt}</small>}
                    </span>
                    <FoundationStatusLabel
                      label={lifecycle.label}
                      status={lifecycle.status}
                      verified={lifecycle.verified}
                    />
                  </button>
                );
              })}
            </div>
          ) : (
            <p className="ol-workbench-result-empty-list">
              {items.length === 0 ? "当前没有可展示的任务。" : "当前搜索和筛选下没有任务。"}
            </p>
          )}
        </section>
      )}

      {listAvailable && (
        <details
          className="ol-workbench-result-section ol-workbench-result-disclosure"
          aria-labelledby="tasks-plan-title"
          open={!embedded ? true : undefined}
        >
          <summary className="ol-workbench-result-section-heading">
            <div>
              <span>完成标准</span>
              <h3 id="tasks-plan-title">{selectedTask ? "计划与完成标准" : "选择任务查看计划"}</h3>
            </div>
          </summary>
          {selectedTask?.workPlan ? (
            <div className="ol-work-contract" data-testid="canonical-work-contract">
              <div className="ol-work-contract__completion">
                <span>完成标准</span>
                <strong>{completionContractLabel(selectedTask.workPlan)}</strong>
                <small>计划版本 {selectedTask.workPlan.revision}</small>
              </div>
              <ol className="ol-work-contract__steps">
                {selectedTask.workPlan.steps.map((step, index) => (
                  <li key={step.id}>
                    <span>{index + 1}</span>
                    <div>
                      <strong>{workPlanStepLabel(step.kind)}</strong>
                      <small>{step.required ? "完成所必需" : "按需执行"}</small>
                    </div>
                  </li>
                ))}
              </ol>
              <details className="ol-work-contract__budget">
                <summary>查看本轮执行边界</summary>
                <p>
                  最多 {selectedTask.workPlan.budgetPolicy.maxTotalItems} 个执行项、
                  {selectedTask.workPlan.budgetPolicy.maxToolAttempts} 次工具尝试和
                  {selectedTask.workPlan.budgetPolicy.maxProviderAttempts}{" "}
                  次模型尝试；达到边界后任务会停止并如实报告。
                </p>
              </details>
            </div>
          ) : (
            <p className="ol-workbench-result-empty-list">
              {selectedTask
                ? "这项工作没有可显示的结构化计划；OpenLife 不会猜测完成标准。"
                : "选择一项工作查看计划与完成标准。"}
            </p>
          )}
        </details>
      )}

      {listAvailable && (
        <details
          className="ol-workbench-result-section ol-workbench-result-disclosure"
          aria-labelledby="tasks-progress-title"
          open={
            !embedded ||
            selectedTask?.lifecycleStatus === "running" ||
            selectedTask?.lifecycleStatus === "waiting_review" ||
            selectedTask?.lifecycleStatus === "waiting_permission"
              ? true
              : undefined
          }
        >
          <summary className="ol-workbench-result-section-heading">
            <div>
              <span>进度</span>
              <h3 id="tasks-progress-title">
                {selectedTask ? selectedTask.title : "选择任务查看执行过程"}
              </h3>
            </div>
          </summary>
          {selectedTask && (selectedTask.steerings?.length ?? 0) > 0 && (
            <ol className="ol-task-item-timeline" data-testid="canonical-task-steerings">
              {selectedTask.steerings?.map(steering => (
                <li key={steering.steeringId} data-steering-status={steering.status}>
                  <div>
                    <strong>运行中调整</strong>
                    <p>
                      {steeringStatusLabel(steering.status)}
                      {steering.appliedPlanRevision
                        ? ` · 计划版本 ${steering.basePlanRevision} → ${steering.appliedPlanRevision}`
                        : ` · 基于计划版本 ${steering.basePlanRevision}`}
                    </p>
                  </div>
                  <FoundationStatusLabel
                    label={steeringStatusLabel(steering.status)}
                    status={
                      steering.status === "applied"
                        ? "success"
                        : steering.status === "pending"
                          ? "waiting"
                          : "blocked"
                    }
                    verified={steering.status === "applied"}
                  />
                </li>
              ))}
            </ol>
          )}
          {selectedTask && selectedTask.items.length > 0 ? (
            <ol className="ol-task-item-timeline" data-testid="canonical-task-items">
              {selectedTask.items.map(item => (
                <li key={item.id} data-item-status={item.status}>
                  <div>
                    <strong>{taskItemKindLabel(item.kind)}</strong>
                    <p>{taskItemSummary(item.summaryCode)}</p>
                  </div>
                  <FoundationStatusLabel
                    label={taskItemStatusLabel(item.status)}
                    status={taskItemStatusTone(item.status)}
                    verified={item.status === "completed"}
                  />
                </li>
              ))}
            </ol>
          ) : (
            <p className="ol-workbench-result-empty-list">
              {selectedTask ? "这项工作还没有可显示的执行记录。" : "选择一项工作查看执行进度。"}
            </p>
          )}
        </details>
      )}

      {listAvailable && (
        <section className="ol-workbench-result-section" aria-labelledby="tasks-results-title">
          <div className="ol-workbench-result-section-heading">
            <div>
              <span>结果</span>
              <h3 id="tasks-results-title">
                {selectedTask ? selectedTask.title : "选择任务查看产物"}
              </h3>
            </div>
            {selectedTask &&
              selectedTask.artifacts.filter(artifact => artifact.undo.available).length > 1 && (
                <FoundationActionButton
                  onClick={() => {
                    setTaskUndoFailure(null);
                    setUndoingTaskId(selectedTask.canonicalTaskId);
                    void onRequestTaskArtifactUndo(selectedTask.canonicalTaskId)
                      .catch(error => {
                        setTaskUndoFailure(error);
                        onAnnounce("批量撤销申请没有完整创建；现有产物状态已重新读取。");
                        onRefresh();
                      })
                      .finally(() => setUndoingTaskId(null));
                  }}
                  label="撤销全部修改"
                  icon={<RotateCcw aria-hidden="true" />}
                  loading={undoingTaskId === selectedTask.canonicalTaskId}
                  loadingLabel="正在创建逐文件撤销审核…"
                />
              )}
          </div>
          {selectedTask && <CompletionLimitations task={selectedTask} />}
          {taskUndoFailure !== null && (
            <FoundationNotice title="批量撤销申请未完成" tone="error" live>
              <p>
                {productErrorMessage(taskUndoFailure, "现有产物保持原状态，请逐项检查撤销入口。")}
              </p>
            </FoundationNotice>
          )}
          {selectedTask && selectedTask.artifacts.length > 0 ? (
            <div className="ol-task-result-grid" data-testid="canonical-task-artifacts">
              {selectedTask.artifacts.map(artifact => (
                <article className="ol-task-result-card" key={artifact.artifactId}>
                  <header className="ol-task-result-card__header">
                    <div>
                      <span>结果</span>
                      <h4>
                        {artifactTypeLabel(artifact.mediaType)} · v{artifact.version}
                      </h4>
                    </div>
                    <FoundationStatusLabel
                      label={
                        artifact.undo.status === "undone"
                          ? "已撤销"
                          : artifactStatusLabel(artifact.status)
                      }
                      status={
                        artifact.undo.status === "undone"
                          ? "success"
                          : artifact.status === "materialized" &&
                              artifact.verification.status === "verified"
                            ? "success"
                            : artifact.status === "failed" ||
                                artifact.verification.status === "failed"
                              ? "error"
                              : artifact.status === "effect_unknown" ||
                                  artifact.verification.status === "unknown"
                                ? "unknown"
                                : "waiting"
                      }
                      verified={
                        artifact.undo.status === "undone" ||
                        (artifact.status === "materialized" &&
                          artifact.verification.status === "verified")
                      }
                    />
                  </header>

                  {artifactActionFailure?.artifactId === artifact.artifactId && (
                    <FoundationNotice
                      title={artifactFailureTitle(artifactActionFailure.action)}
                      tone="error"
                      live
                    >
                      <p>
                        {productErrorMessage(
                          artifactActionFailure.error,
                          artifactFailureFallback(artifactActionFailure.action)
                        )}
                      </p>
                    </FoundationNotice>
                  )}
                  <p className="ol-task-result-card__location">
                    {artifact.undo.status === "undone" ? "原写入位置：" : ""}
                    {artifact.change.targetReference ?? "结果位置尚未确认。"}
                    {artifact.undo.status === "undone" ? "（已撤销）" : ""}
                  </p>

                  <section className="ol-task-result-card__section" aria-label="Preview">
                    <span>
                      {artifactPreviewLabel(artifact.mediaType, artifact.undo.status === "undone")}
                    </span>
                    {artifact.preview.content ? (
                      <pre>{artifact.preview.content}</pre>
                    ) : (
                      <p>
                        {artifact.undo.status === "undone"
                          ? artifactPreviewIsExtractedText(artifact.mediaType)
                            ? "历史版本内容提取不可用："
                            : "历史版本预览不可用："
                          : artifactPreviewIsExtractedText(artifact.mediaType)
                            ? "内容提取不可用："
                            : "预览不可用："}
                        {reasonLabel(artifact.preview.reasonCode ?? "来源未确认")}
                      </p>
                    )}
                    {artifact.preview.status === "truncated" && <small>这里只显示部分预览。</small>}
                    {artifact.undo.status === "undone" && (
                      <small>这是撤销前已核验版本的历史预览，不代表目标位置的当前内容。</small>
                    )}
                  </section>

                  <section className="ol-task-result-card__section" aria-label="File">
                    <span>文件</span>
                    {artifact.undo.status === "undone" ? (
                      <p>
                        {artifact.undo.operation === "restore_replaced"
                          ? "原产物已撤销；目标位置已恢复为替换前的内容。"
                          : artifact.undo.operation === "restore_moved"
                            ? "原产物已撤销；文件已恢复为重命名前的名称。"
                            : "原产物已撤销；OpenLife 新建的文件已移入安全回收位置。"}
                      </p>
                    ) : artifact.status === "materialized" &&
                      artifact.verification.status === "verified" ? (
                      <div className="ol-task-result-card__actions">
                        <FoundationActionButton
                          onClick={() => {
                            setArtifactActionFailure(null);
                            setOpeningArtifactId(artifact.artifactId);
                            void onOpenArtifact(artifact.artifactId, artifact.version)
                              .catch(error => {
                                setArtifactActionFailure({
                                  artifactId: artifact.artifactId,
                                  action: "open",
                                  error,
                                });
                                onAnnounce("文件没有打开；产物仍保持原状态。");
                              })
                              .finally(() => setOpeningArtifactId(null));
                          }}
                          label="打开文件"
                          icon={<FolderOpen aria-hidden="true" />}
                          loading={openingArtifactId === artifact.artifactId}
                          loadingLabel="正在核验并打开…"
                        />
                        <FoundationActionButton
                          onClick={() => {
                            setArtifactActionFailure(null);
                            setExportingArtifactId(artifact.artifactId);
                            void onExportArtifact(artifact.artifactId, artifact.version)
                              .catch(error => {
                                setArtifactActionFailure({
                                  artifactId: artifact.artifactId,
                                  action: "export",
                                  error,
                                });
                                onAnnounce("文件没有另存；原产物仍保持原状态。");
                              })
                              .finally(() => setExportingArtifactId(null));
                          }}
                          label="另存为…"
                          icon={<Download aria-hidden="true" />}
                          loading={exportingArtifactId === artifact.artifactId}
                          loadingLabel="正在另存并核验…"
                        />
                      </div>
                    ) : (
                      <p>文件通过物化与摘要核验后才可打开。</p>
                    )}
                  </section>
                  <section className="ol-task-result-card__section" aria-label="Focused revision">
                    <span>继续修改</span>
                    {artifact.revision.available ? (
                      revisionDraft?.artifactId === artifact.artifactId ? (
                        <form
                          className="ol-task-result-card__revision"
                          onSubmit={event => {
                            event.preventDefault();
                            const instruction = revisionDraft.instruction.trim();
                            if (!instruction || !selectedTask) return;
                            setArtifactActionFailure(null);
                            setRevisingArtifactId(artifact.artifactId);
                            void onReviseArtifact(
                              selectedTask.canonicalTaskId,
                              artifact.artifactId,
                              artifact.version,
                              instruction
                            )
                              .then(() => setRevisionDraft(null))
                              .catch(error => {
                                setArtifactActionFailure({
                                  artifactId: artifact.artifactId,
                                  action: "revision",
                                  error,
                                });
                                onAnnounce("聚焦修订没有开始；当前版本仍保持原状态。");
                              })
                              .finally(() => setRevisingArtifactId(null));
                          }}
                        >
                          <label htmlFor={`artifact-revision-${artifact.artifactId}`}>
                            只说明要改动的部分
                          </label>
                          <textarea
                            id={`artifact-revision-${artifact.artifactId}`}
                            value={revisionDraft.instruction}
                            onChange={event =>
                              setRevisionDraft({
                                artifactId: artifact.artifactId,
                                instruction: event.target.value,
                              })
                            }
                            placeholder="例如：把结论压缩为三点，其他章节保持不变。"
                            maxLength={10_000}
                            rows={4}
                            autoFocus
                          />
                          <div className="ol-task-result-card__actions">
                            <FoundationActionButton
                              type="submit"
                              label="开始新修订"
                              icon={<PencilLine aria-hidden="true" />}
                              disabled={!revisionDraft.instruction.trim()}
                              disabledReason={
                                !revisionDraft.instruction.trim()
                                  ? "先说明这个版本需要修改什么。"
                                  : undefined
                              }
                              loading={revisingArtifactId === artifact.artifactId}
                              loadingLabel="正在修订并核验…"
                            />
                            <FoundationActionButton
                              type="button"
                              variant="quiet"
                              label="取消"
                              disabled={revisingArtifactId === artifact.artifactId}
                              disabledReason={
                                revisingArtifactId === artifact.artifactId
                                  ? "修订请求正在创建，完成前不能关闭表单。"
                                  : undefined
                              }
                              onClick={() => setRevisionDraft(null)}
                            />
                          </div>
                        </form>
                      ) : (
                        <FoundationActionButton
                          onClick={() =>
                            setRevisionDraft({ artifactId: artifact.artifactId, instruction: "" })
                          }
                          label="聚焦修订此版本"
                          icon={<PencilLine aria-hidden="true" />}
                        />
                      )
                    ) : (
                      <p>
                        暂不可修订：
                        {reasonLabel(
                          artifact.revision.reasonCode ?? "缺少可验证的当前版本或任务尚未完成"
                        )}
                      </p>
                    )}
                    <small>新指令会创建绑定当前版本的新 Run；原版本和历史结果不会被删除。</small>
                  </section>
                  <details className="ol-task-result-card__details">
                    <summary>来源、完整性与恢复</summary>
                    <div className="ol-task-result-card__details-body">
                      <section className="ol-task-result-card__section" aria-label="Provenance">
                        <span>来源与版本</span>
                        <strong>
                          当前 v{artifact.version}
                          {artifact.previousVersion
                            ? ` · 基于 v${artifact.previousVersion}`
                            : " · 初始版本"}
                        </strong>
                        {artifact.sourceRunProvenance ? (
                          <p>{provenanceLabel(artifact.sourceRunProvenance)}</p>
                        ) : (
                          <p>本版本的 Run / 模型来源当前不可用，不使用当前设置代替。</p>
                        )}
                        {artifact.sourceResourceRefs.length > 0 ? (
                          <ul aria-label="本版本绑定的本地资源">
                            {artifact.sourceResourceRefs.map(resource => (
                              <li key={resource.id}>{resource.label}</li>
                            ))}
                          </ul>
                        ) : (
                          <small>本版本没有投影已绑定的本地资源。</small>
                        )}
                      </section>

                      <section className="ol-task-result-card__section" aria-label="Changes">
                        <span>变更</span>
                        <strong>{artifactChangeLabel(artifact.change.kind)}</strong>
                        {artifact.change.expectedPriorDigest && (
                          <small>替换基线：{artifact.change.expectedPriorDigest}</small>
                        )}
                      </section>

                      <section className="ol-task-result-card__section" aria-label="Verification">
                        <span>
                          {artifact.undo.status === "undone" ? "历史物化与撤销" : "文件完整性"}
                        </span>
                        <FoundationStatusLabel
                          label={
                            artifact.undo.status === "undone"
                              ? "撤销已核验"
                              : artifactVerificationLabel(artifact.verification.status)
                          }
                          status={
                            artifact.undo.status === "undone"
                              ? "success"
                              : artifact.verification.status === "verified"
                                ? "success"
                                : artifact.verification.status === "failed"
                                  ? "error"
                                  : artifact.verification.status === "unknown"
                                    ? "unknown"
                                    : "waiting"
                          }
                          verified={
                            artifact.undo.status === "undone" ||
                            artifact.verification.status === "verified"
                          }
                        />
                        <dl>
                          <div>
                            <dt>
                              {artifact.undo.status === "undone" ? "撤销前产物摘要" : "期望摘要"}
                            </dt>
                            <dd>{artifact.verification.expectedContentDigest}</dd>
                          </div>
                          <div>
                            <dt>
                              {artifact.undo.status === "undone" ? "撤销前实测摘要" : "实测摘要"}
                            </dt>
                            <dd>{artifact.verification.observedContentDigest ?? "尚未观测"}</dd>
                          </div>
                        </dl>
                        {artifact.verification.reasonCode && (
                          <small>{reasonLabel(artifact.verification.reasonCode)}</small>
                        )}
                      </section>

                      <section className="ol-task-result-card__section" aria-label="Undo">
                        <span>撤销</span>
                        {artifact.undo.available ? (
                          <FoundationActionButton
                            onClick={() => {
                              setArtifactActionFailure(null);
                              setUndoingArtifactId(artifact.artifactId);
                              void onRequestArtifactUndo(artifact.artifactId)
                                .catch(error => {
                                  setArtifactActionFailure({
                                    artifactId: artifact.artifactId,
                                    action: "undo",
                                    error,
                                  });
                                  onAnnounce("撤销申请没有创建；产物仍保持原状态。");
                                })
                                .finally(() => setUndoingArtifactId(null));
                            }}
                            label="申请撤销此产物"
                            icon={<RotateCcw aria-hidden="true" />}
                            loading={undoingArtifactId === artifact.artifactId}
                            loadingLabel="正在创建撤销审核…"
                          />
                        ) : artifact.undo.proposalRef ? (
                          <p>
                            {artifact.undo.status === "undone"
                              ? artifact.undo.operation === "restore_replaced"
                                ? "已撤销，替换前的原始内容已恢复并核验。"
                                : artifact.undo.operation === "restore_moved"
                                  ? "已撤销，重命名前的原名称已恢复并核验。"
                                  : "已撤销，OpenLife 新建的文件已移入安全回收位置。"
                              : "撤销正在等待审核或 reconciliation。"}
                          </p>
                        ) : (
                          <p>
                            不可撤销：
                            {reasonLabel(artifact.undo.reasonCode ?? "缺少可验证恢复依据")}
                          </p>
                        )}
                      </section>
                    </div>
                  </details>
                </article>
              ))}
            </div>
          ) : selectedTask?.latestResultPreview?.preview ? (
            <article className="ol-task-result-card" data-testid="canonical-task-answer">
              <header className="ol-task-result-card__header">
                <div>
                  <span>{selectedTask.finalDeliveryEvidencePresent ? "最终回答" : "当前结果"}</span>
                  <h4>
                    {selectedTask.finalDeliveryEvidencePresent
                      ? selectedTask.completionDisposition === "complete_with_disclosed_limitations"
                        ? "最终回答已交付，含已说明限制"
                        : "最终回答已交付"
                      : taskLifecyclePresentation(selectedTask).label}
                  </h4>
                </div>
                <FoundationStatusLabel
                  label={
                    selectedTask.finalDeliveryEvidencePresent
                      ? selectedTask.completionDisposition === "complete_with_disclosed_limitations"
                        ? "已交付，含限制"
                        : "已交付"
                      : "交付尚未确认"
                  }
                  status={
                    selectedTask.finalDeliveryEvidencePresent
                      ? selectedTask.completionDisposition === "complete_with_disclosed_limitations"
                        ? "waiting"
                        : "success"
                      : "unknown"
                  }
                  verified={
                    selectedTask.finalDeliveryEvidencePresent &&
                    selectedTask.completionDisposition !== "complete_with_disclosed_limitations"
                  }
                />
              </header>
              <section className="ol-task-result-card__section" aria-label="Answer">
                <span>回答</span>
                <p className="ol-task-result-card__answer">
                  {selectedTask.latestResultPreview.preview}
                </p>
              </section>
            </article>
          ) : (
            <p className="ol-workbench-result-empty-list">
              {selectedTask ? "这项工作还没有可交付的结果。" : "选择一项工作查看结果。"}
            </p>
          )}
        </section>
      )}

      {listAvailable && (
        <section className="ol-workbench-result-action-area" aria-labelledby="tasks-controls-title">
          <div>
            <span>可用动作</span>
            <h3 id="tasks-controls-title">
              {selectedTask ? selectedTask.title : "选择一个任务查看可用动作"}
            </h3>
          </div>
          {selectedTask && executableControls.length > 0 ? (
            <div className="ol-workbench-result-task-controls">
              {executableControls.map(control => {
                const busyForControl =
                  taskControlBusy &&
                  taskControlState.phase !== "idle" &&
                  taskControlState.control.id === control.id;
                const staleReason =
                  envelope.status === "stale" ? "任务读模型已陈旧；请先重新读取。" : undefined;
                const disabled = !control.enabled || Boolean(staleReason) || taskControlBusy;
                const disabledReason =
                  staleReason ||
                  control.disabledReason ||
                  (!control.enabled ? "此动作当前不可用。" : undefined) ||
                  (taskControlBusy ? "另一项任务动作正在核对。" : undefined);
                return (
                  <FoundationActionButton
                    key={control.id}
                    label={taskControlLabel(control)}
                    icon={taskControlIcon(control)}
                    variant={control.kind === "stop_run" ? "danger" : "secondary"}
                    loading={busyForControl}
                    loadingLabel={taskControlState.phase === "refreshing" ? "正在核对" : "正在请求"}
                    disabled={disabled}
                    disabledReason={disabledReason}
                    data-action-category="task-control"
                    data-action-id={control.id}
                    data-action-kind={control.kind}
                    data-action-effect={control.effect}
                    data-action-enabled={String(control.enabled)}
                    data-action-disabled-reason={
                      control.disabledReason ?? (!control.enabled ? "此动作当前不可用。" : "")
                    }
                    data-action-target-ref={control.targetTaskId}
                    data-action-target-action-id={control.targetActionId ?? ""}
                    data-action-requires-confirmation={String(
                      Boolean(control.requiresConfirmation)
                    )}
                    data-action-completion-proof-after-dispatch={String(
                      Boolean(control.completionProofAfterDispatch)
                    )}
                    onClick={() => onRequestTaskControl(control, selectedTask.canonicalTaskId)}
                  />
                );
              })}
            </div>
          ) : (
            <p className="ol-workbench-result-task-control-empty">
              {selectedTask
                ? "这项工作当前没有可用动作。"
                : "选择任务只改变当前检查对象，不会自动发送命令。"}
            </p>
          )}
          {controlFeedback && (
            <FoundationNotice title={controlFeedback.title} tone={controlFeedback.tone} live>
              <p>{controlFeedback.body}</p>
            </FoundationNotice>
          )}
        </section>
      )}

      <section className="ol-workbench-result-action-area" aria-labelledby="tasks-evidence-title">
        <div>
          <span>技术详情</span>
          <h3 id="tasks-evidence-title">
            {selectedTaskId ? "查看这项工作的依据与限制" : "查看工作列表的依据与限制"}
          </h3>
        </div>
        <FoundationActionButton
          label="查看详情"
          variant="secondary"
          icon={<FileSearch size={18} strokeWidth={1.75} aria-hidden="true" />}
          {...actionAttributes(inspectAction)}
          onClick={onOpenInspector}
        />
      </section>

      <FoundationDialog
        open={taskControlState.phase === "confirming"}
        title="确认执行这项任务动作？"
        description="确认只发送一次精确任务命令；最终状态仍以系统刷新结果为准。"
        onClose={onCancelTaskControlConfirmation}
        footer={
          <>
            <FoundationActionButton
              label="返回"
              variant="quiet"
              onClick={onCancelTaskControlConfirmation}
            />
            <FoundationActionButton
              label={
                taskControlState.phase === "confirming"
                  ? taskControlLabel(taskControlState.control)
                  : "确认"
              }
              variant="primary"
              onClick={onConfirmTaskControl}
            />
          </>
        }
      >
        <p>{selectedTask?.title ?? "当前任务"}</p>
      </FoundationDialog>
    </article>
  );
}
