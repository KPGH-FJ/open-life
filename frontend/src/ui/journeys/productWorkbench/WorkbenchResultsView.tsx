import { useMemo, useState } from "react";
import { FileSearch, Play, RefreshCw, RotateCcw, Search, XCircle } from "lucide-react";
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
} from "@/ui/journeys/governedAction/taskControlContract";
import {
  formatBackendTime,
  taskNeedsAttention,
  taskLifecyclePresentation,
  taskPrimaryQuestion,
} from "./workbenchPresentation";

type TaskFilter = "all" | "attention" | "active" | "terminal";

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
    item.artifacts.map(artifact => artifact.materializedReference ?? artifact.mediaType).join(" "),
  ]
    .join(" ")
    .toLocaleLowerCase("zh-CN");
}

function statusDetail(item: TaskViewModelItem): string {
  if (item.needsAttention && item.attentionReasonCodes?.[0]) {
    return `需要处理：${reasonLabel(item.attentionReasonCodes[0])}`;
  }
  if (item.lifecycleStatus === "waiting_review") return "报告产物正在等待你的审核，任务尚未完成。";
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
    artifact_effect_unknown: "结果写入状态需要人工核对",
    artifact_delivery_failed: "结果交付失败",
    artifact_waiting_materialization: "结果尚未写入目标位置",
    artifact_content_digest_drift: "目标内容与已核验版本不一致",
    artifact_materialized_reference_missing: "结果位置缺少确认依据",
    artifact_preview_source_unavailable: "结果预览来源不可用",
    artifact_undo_pending_or_failed: "撤销仍在等待决定或执行失败",
    artifact_undo_unavailable_without_original_bytes: "缺少可恢复的原始内容",
    artifact_undo_requires_verified_materialization: "结果核验后才能撤销",
    artifact_undo_unavailable: "当前结果不可撤销",
    work_provider_binding_stale: "Provider 或模型已经变化，请作为新的工作重新提交",
    work_project_assignment_stale: "对话所属 Project 已经变化，请作为新的工作重新提交",
    work_project_scope_stale: "Project 范围已经变化，请核对后作为新的工作重新提交",
    work_skill_binding_stale: "所选技能已经变化，请作为新的工作重新提交",
  };
  if (labels[reason]) return labels[reason];
  return /^[a-z0-9_.:-]+$/i.test(reason) ? "后端要求核对这项状态" : reason;
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
  if (status === "verified") return "内容已核验";
  if (status === "pending") return "等待物化核验";
  if (status === "failed") return "核验失败";
  return "核验结果未知";
}

function artifactChangeLabel(
  kind: TaskViewModelItem["artifacts"][number]["change"]["kind"]
): string {
  if (kind === "create") return "创建文件";
  if (kind === "replace") return "替换文件";
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

function taskItemSummary(summaryCode: string): string {
  const [, tool] = summaryCode.split(":", 2);
  const toolLabel =
    tool === "document.read"
      ? "本地文档"
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
  if (control.kind === "resume") return "继续任务";
  if (control.kind === "retry") return "重试失败步骤";
  if (control.kind === "cancel") return "取消任务";
  if (control.kind === "refresh_context") return "刷新上下文";
  return control.label;
}

function taskControlIcon(control: TaskControl) {
  if (control.kind === "resume") return <Play size={17} aria-hidden="true" />;
  if (control.kind === "retry") return <RotateCcw size={17} aria-hidden="true" />;
  if (control.kind === "cancel") return <XCircle size={17} aria-hidden="true" />;
  return <RefreshCw size={17} aria-hidden="true" />;
}

function taskControlFeedback(state: TaskControlDispatchState) {
  if (state.phase === "idle" || state.phase === "confirming") return null;
  if (state.phase === "blocked") {
    return { title: "当前不能执行这项动作", body: state.reason, tone: "protection" as const };
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
      body: state.errorCode,
      tone: "error" as const,
    };
  }
  return {
    title:
      state.control.kind === "cancel"
        ? "任务已取消"
        : state.control.kind === "refresh_context"
          ? "上下文已刷新"
          : "任务状态已更新",
    body:
      state.control.kind === "cancel"
        ? "刷新后的同一任务已确认取消。"
        : `刷新后的同一任务当前为 ${state.refreshedTask.lifecycleStatus}；这不是完成证明。`,
    tone: "neutral" as const,
  };
}

export function WorkbenchResultsView({
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
  fixedFilter,
  scopedItems,
  embedded = false,
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
  fixedFilter?: TaskFilter;
  scopedItems?: readonly TaskViewModelItem[];
  embedded?: boolean;
}) {
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<TaskFilter>("all");
  const [undoingArtifactId, setUndoingArtifactId] = useState<string | null>(null);
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
    label: "Refresh TasksViewModel",
    kind: "refresh",
    enabled: !refreshing,
    disabledReason: refreshing ? "任务列表正在刷新。" : undefined,
    targetRef: "TasksViewModel",
  } satisfies ProductAction;
  const inspectAction = {
    id: "tasks.inspect_evidence",
    label: "Inspect Tasks evidence",
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
      className={`ol-workbench-result-page${embedded ? " ol-workbench-result-page--embedded" : ""}`}
      data-testid="tasks-product-view"
    >
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

      {envelope.status === "loading" && (
        <FoundationNotice title="正在读取任务列表" tone="neutral" live>
          读取完成前不推断任务状态或恢复能力。
        </FoundationNotice>
      )}

      {envelope.status === "error" && (
        <FoundationNotice title="任务状态读取失败" tone="error">
          后端没有返回可确认的任务状态；当前数量未知，不会把读取失败显示为零项。原始错误可在检查器中核对。
        </FoundationNotice>
      )}

      {envelope.status === "stale" && (
        <FoundationNotice title="任务列表已陈旧，只用于查看" tone="protection">
          当前不会开放恢复、重试、取消或其他可能改变任务状态的动作。
        </FoundationNotice>
      )}

      {listAvailable && (
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
        <section className="ol-workbench-result-section" aria-labelledby="tasks-plan-title">
          <div className="ol-workbench-result-section-heading">
            <div>
              <span>完成标准</span>
              <h3 id="tasks-plan-title">{selectedTask ? "计划与完成标准" : "选择任务查看计划"}</h3>
            </div>
          </div>
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
        </section>
      )}

      {listAvailable && (
        <section className="ol-workbench-result-section" aria-labelledby="tasks-progress-title">
          <div className="ol-workbench-result-section-heading">
            <div>
              <span>进度</span>
              <h3 id="tasks-progress-title">
                {selectedTask ? selectedTask.title : "选择任务查看执行过程"}
              </h3>
            </div>
          </div>
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
        </section>
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
          </div>
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
                      label={artifactStatusLabel(artifact.status)}
                      status={
                        artifact.status === "materialized"
                          ? "success"
                          : artifact.status === "failed"
                            ? "error"
                            : artifact.status === "effect_unknown"
                              ? "unknown"
                              : "waiting"
                      }
                      verified={artifact.verification.status === "verified"}
                    />
                  </header>

                  <section className="ol-task-result-card__section" aria-label="Changes">
                    <span>变更</span>
                    <strong>{artifactChangeLabel(artifact.change.kind)}</strong>
                    <p>{artifact.change.targetReference ?? "结果位置尚未确认。"}</p>
                    {artifact.change.expectedPriorDigest && (
                      <small>替换基线：{artifact.change.expectedPriorDigest}</small>
                    )}
                  </section>

                  <section className="ol-task-result-card__section" aria-label="Preview">
                    <span>预览</span>
                    {artifact.preview.content ? (
                      <pre>{artifact.preview.content}</pre>
                    ) : (
                      <p>预览不可用：{reasonLabel(artifact.preview.reasonCode ?? "来源未确认")}</p>
                    )}
                    {artifact.preview.status === "truncated" && <small>这里只显示部分预览。</small>}
                  </section>

                  <section className="ol-task-result-card__section" aria-label="Verification">
                    <span>核验</span>
                    <FoundationStatusLabel
                      label={artifactVerificationLabel(artifact.verification.status)}
                      status={
                        artifact.verification.status === "verified"
                          ? "success"
                          : artifact.verification.status === "failed"
                            ? "error"
                            : artifact.verification.status === "unknown"
                              ? "unknown"
                              : "waiting"
                      }
                      verified={artifact.verification.status === "verified"}
                    />
                    <details className="ol-task-result-card__technical">
                      <summary>查看技术核验信息</summary>
                      <dl>
                        <div>
                          <dt>期望摘要</dt>
                          <dd>{artifact.verification.expectedContentDigest}</dd>
                        </div>
                        <div>
                          <dt>实测摘要</dt>
                          <dd>{artifact.verification.observedContentDigest ?? "尚未观测"}</dd>
                        </div>
                      </dl>
                    </details>
                    {artifact.verification.reasonCode && (
                      <small>{reasonLabel(artifact.verification.reasonCode)}</small>
                    )}
                  </section>
                  <section className="ol-task-result-card__section" aria-label="Undo">
                    <span>撤销</span>
                    {artifact.undo.available ? (
                      <FoundationActionButton
                        onClick={() => {
                          setUndoingArtifactId(artifact.artifactId);
                          void onRequestArtifactUndo(artifact.artifactId).finally(() =>
                            setUndoingArtifactId(null)
                          );
                        }}
                        label="申请撤销此产物"
                        icon={<RotateCcw aria-hidden="true" />}
                        loading={undoingArtifactId === artifact.artifactId}
                        loadingLabel="正在创建撤销审核…"
                      />
                    ) : artifact.undo.proposalRef ? (
                      <p>
                        {artifact.undo.status === "undone"
                          ? "已撤销，原文件已移入 OpenLife 安全回收位置。"
                          : "撤销正在等待审核或 reconciliation。"}
                      </p>
                    ) : (
                      <p>
                        不可撤销：{reasonLabel(artifact.undo.reasonCode ?? "缺少可验证恢复依据")}
                      </p>
                    )}
                  </section>
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
                      ? "最终回答已交付"
                      : selectedTask.latestResultPreview.label}
                  </h4>
                </div>
                <FoundationStatusLabel
                  label={selectedTask.finalDeliveryEvidencePresent ? "已交付" : "交付尚未确认"}
                  status={selectedTask.finalDeliveryEvidencePresent ? "success" : "unknown"}
                  verified={selectedTask.finalDeliveryEvidencePresent}
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
                    variant={control.kind === "cancel" ? "danger" : "secondary"}
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
        title={
          taskControlState.phase === "confirming" && taskControlState.control.kind === "cancel"
            ? "确认取消这项任务？"
            : "确认执行这项任务动作？"
        }
        description="确认只发送一次精确任务命令；最终状态仍以后端刷新结果为准。"
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
              variant={
                taskControlState.phase === "confirming" &&
                taskControlState.control.kind === "cancel"
                  ? "danger"
                  : "primary"
              }
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
