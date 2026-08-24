import { Activity, ChevronRight } from "lucide-react";
import type { TaskViewModelItem } from "@/tauri";
import { FoundationStatusLabel } from "@/ui/foundation";
import {
  formatBackendTime,
  taskLifecyclePresentation,
  taskNeedsAttention,
} from "./taskPresentation";

function activitySummary(item: TaskViewModelItem): string {
  if (item.pendingReviewItemRefs.length > 0) {
    return `${item.pendingReviewItemRefs.length} 个决定节点等待处理`;
  }
  if (item.needsAttention && item.attentionReasonCodes?.[0]) {
    return `需要处理：${item.attentionReasonCodes[0].replace(/_/g, " ")}`;
  }
  if (item.allowedControls.some(control => control.enabled && control.kind === "resume")) {
    return "原运行已终止，可以创建新运行继续";
  }
  if (item.allowedControls.some(control => control.enabled && control.kind === "retry")) {
    return "失败边界已记录，可以创建新运行重试";
  }
  return item.latestResultPreview?.label ?? "打开查看已记录的运行事实";
}

export function GlobalActivityView({
  items,
  selectedTaskId,
  onOpenTask,
}: {
  items: readonly TaskViewModelItem[];
  selectedTaskId: string | null;
  onOpenTask: (task: TaskViewModelItem) => void;
}) {
  if (items.length === 0) return null;

  const attentionCount = items.filter(taskNeedsAttention).length;
  const activeCount = items.filter(item =>
    ["running", "waiting_review", "waiting_permission"].includes(item.lifecycleStatus)
  ).length;

  return (
    <details className="ol-global-activity" open={attentionCount > 0}>
      <summary>
        <span className="ol-global-activity__title">
          <Activity size={17} aria-hidden="true" />
          全部活动
        </span>
        <span className="ol-global-activity__counts">
          {activeCount > 0 && `${activeCount} 项进行中`}
          {activeCount > 0 && attentionCount > 0 && " · "}
          {attentionCount > 0 ? `${attentionCount} 项需要处理` : `${items.length} 项最近工作`}
        </span>
      </summary>
      <ol aria-label="全部任务活动">
        {items.slice(0, 12).map(item => {
          const lifecycle = taskLifecyclePresentation(item);
          const updatedAt = formatBackendTime(item.updatedAt);
          return (
            <li key={item.canonicalTaskId}>
              <button
                type="button"
                aria-current={selectedTaskId === item.canonicalTaskId ? "true" : undefined}
                onClick={() => onOpenTask(item)}
              >
                <span className="ol-global-activity__copy">
                  <strong>{item.title}</strong>
                  <small>
                    {activitySummary(item)}
                    {updatedAt ? ` · ${updatedAt}` : ""}
                  </small>
                </span>
                <FoundationStatusLabel
                  label={lifecycle.label}
                  status={lifecycle.status}
                  verified={lifecycle.verified}
                />
                <ChevronRight size={16} aria-hidden="true" />
              </button>
            </li>
          );
        })}
      </ol>
      {items.length > 12 && <p>显示最近 12 项；其余工作仍保留在任务历史中。</p>}
    </details>
  );
}
