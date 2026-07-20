import { useMemo, useState } from "react";
import { FileSearch, RefreshCw, Search } from "lucide-react";
import type { ProductAction, TaskViewModelItem, TasksViewModel, ViewModelEnvelope } from "@/tauri";
import { FoundationActionButton, FoundationNotice, FoundationStatusLabel } from "@/ui/foundation";
import {
  formatBackendTime,
  taskLifecyclePresentation,
  taskPrimaryQuestion,
} from "./readOnlySpinePresentation";

type TaskFilter = "all" | "attention" | "active" | "terminal";

function taskMatchesFilter(item: TaskViewModelItem, filter: TaskFilter): boolean {
  if (filter === "all") return true;
  if (filter === "active") {
    return ["running", "waiting_permission", "blocked"].includes(item.lifecycleStatus);
  }
  if (filter === "attention") {
    return (
      [
        "waiting_permission",
        "blocked",
        "failed",
        "completed_with_pending_review",
        "completed_needs_evidence",
        "unknown",
      ].includes(item.lifecycleStatus) ||
      item.pendingBlockers.length > 0 ||
      item.pendingReviewItemRefs.length > 0
    );
  }
  return ["completed", "failed", "cancelled", "completed_needs_evidence"].includes(
    item.lifecycleStatus
  );
}

function taskSearchText(item: TaskViewModelItem): string {
  return [
    item.title,
    item.lifecycleStatus,
    item.pendingBlockers.join(" "),
    item.pendingReviewItemRefs.map(ref => ref.label).join(" "),
    item.latestResultPreview?.label ?? "",
    item.latestResultPreview?.preview ?? "",
  ]
    .join(" ")
    .toLocaleLowerCase("zh-CN");
}

function statusDetail(item: TaskViewModelItem): string {
  if (item.pendingReviewItemRefs.length > 0) return "有事项等待决定，任务尚未完成。";
  if (item.pendingBlockers.length > 0) return item.pendingBlockers[0];
  if (item.latestResultPreview?.preview) return item.latestResultPreview.preview;
  if (item.lifecycleStatus === "running") return "任务仍在执行。";
  return "查看依据可核对当前生命周期与交付状态。";
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

export function TasksReadOnlyView({
  envelope,
  refreshing,
  selectedTaskId,
  onRefresh,
  onSelectTask,
  onOpenInspector,
  onAnnounce,
}: {
  envelope: ViewModelEnvelope<TasksViewModel>;
  refreshing: boolean;
  selectedTaskId: string | null;
  onRefresh: () => void;
  onSelectTask: (task: TaskViewModelItem) => void;
  onOpenInspector: () => void;
  onAnnounce: (message: string) => void;
}) {
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<TaskFilter>("all");
  const items = envelope.data?.items ?? [];
  const listAvailable = envelope.data !== null && !["error", "loading"].includes(envelope.status);
  const normalizedQuery = query.trim().toLocaleLowerCase("zh-CN");
  const visibleItems = useMemo(
    () =>
      items.filter(
        item =>
          taskMatchesFilter(item, filter) &&
          (!normalizedQuery || taskSearchText(item).includes(normalizedQuery))
      ),
    [filter, items, normalizedQuery]
  );
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
    <article className="ol-readonly-page" data-testid="phase4d-tasks-view">
      <header className="ol-readonly-page-heading ol-readonly-page-heading--with-actions">
        <div>
          <span>哪些任务需要我或可以继续</span>
          <h2>{taskPrimaryQuestion(envelope)}</h2>
          <p>生命周期、阻塞和结果只来自后端任务读模型；当前页面不执行任务控制。</p>
        </div>
        <FoundationActionButton
          label="重新读取"
          variant="quiet"
          icon={<RefreshCw size={18} strokeWidth={1.75} aria-hidden="true" />}
          loading={refreshing}
          loadingLabel="正在读取"
          {...actionAttributes(refreshAction)}
          onClick={onRefresh}
        />
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
        <section className="ol-readonly-section" aria-labelledby="tasks-list-title">
          <div className="ol-readonly-section-heading ol-readonly-task-tools-heading">
            <div>
              <span>任务列表</span>
              <h3 id="tasks-list-title">最近工作</h3>
            </div>
            <div className="ol-readonly-task-tools">
              <label className="ol-readonly-search">
                <span className="ol-sr-only">搜索任务</span>
                <Search size={17} strokeWidth={1.75} aria-hidden="true" />
                <input
                  type="search"
                  value={query}
                  placeholder="搜索任务"
                  onChange={event => {
                    setQuery(event.target.value);
                    announceVisible(event.target.value, filter);
                  }}
                />
              </label>
              <label className="ol-readonly-filter">
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
            </div>
          </div>

          <p className="ol-readonly-list-count">
            共 {items.length} 项，当前显示 {visibleItems.length} 项
          </p>

          {visibleItems.length > 0 ? (
            <div className="ol-readonly-task-list">
              {visibleItems.map(item => {
                const lifecycle = taskLifecyclePresentation(item);
                const updatedAt = formatBackendTime(item.updatedAt);
                return (
                  <button
                    key={item.canonicalTaskId}
                    type="button"
                    className="ol-readonly-task-row"
                    data-selected={selectedTaskId === item.canonicalTaskId ? "true" : "false"}
                    aria-pressed={selectedTaskId === item.canonicalTaskId}
                    onClick={() => onSelectTask(item)}
                  >
                    <span className="ol-readonly-task-row__copy">
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
            <p className="ol-readonly-empty-list">
              {items.length === 0 ? "当前没有可展示的任务。" : "当前搜索和筛选下没有任务。"}
            </p>
          )}
        </section>
      )}

      <section className="ol-readonly-action-area" aria-labelledby="tasks-evidence-title">
        <div>
          <span>证据入口</span>
          <h3 id="tasks-evidence-title">
            {selectedTaskId ? "核对所选任务的来源" : "核对任务列表的来源与限制"}
          </h3>
        </div>
        <FoundationActionButton
          label="查看依据"
          variant="secondary"
          icon={<FileSearch size={18} strokeWidth={1.75} aria-hidden="true" />}
          {...actionAttributes(inspectAction)}
          onClick={onOpenInspector}
        />
      </section>
    </article>
  );
}
