import type { TaskViewModelItem, TasksViewModel, ViewModelEnvelope } from "@/tauri";
import type { FoundationStatus } from "@/ui/foundation";

export type ProductStatusPresentation = {
  label: string;
  status: FoundationStatus;
  verified?: boolean;
};

export function taskNeedsAttention(item: TaskViewModelItem): boolean {
  return (
    item.needsAttention === true ||
    [
      "waiting_permission",
      "waiting_review",
      "blocked",
      "failed",
      "remote_unknown",
      "interrupted",
      "completed_with_pending_review",
      "completed_needs_evidence",
      "unknown",
    ].includes(item.lifecycleStatus) ||
    item.pendingBlockers.length > 0 ||
    item.pendingReviewItemRefs.length > 0
  );
}

export function taskLifecyclePresentation(item: TaskViewModelItem): ProductStatusPresentation {
  switch (item.lifecycleStatus) {
    case "running":
      return { label: "运行中", status: "neutral" };
    case "waiting_review":
      return { label: "等待审核", status: "waiting" };
    case "waiting_permission":
      return { label: "等待确认", status: "waiting" };
    case "blocked":
      return { label: "已阻断", status: "blocked" };
    case "failed":
      return { label: "失败", status: "error" };
    case "remote_unknown":
      return { label: "远端结果未知", status: "unknown" };
    case "cancelled":
      return { label: "已取消", status: "neutral" };
    case "interrupted":
      return { label: "已中断，可重试", status: "blocked" };
    case "completed_with_pending_review":
      return { label: "待审核，未完成", status: "waiting" };
    case "completed_needs_evidence":
      return { label: "缺少完成证据", status: "blocked" };
    case "completed":
      if (item.finalDeliveryEvidencePresent && item.terminalDeliveryStatus === "delivered") {
        if (item.completionDisposition === "complete_with_disclosed_limitations") {
          return { label: "已完成，含已说明限制", status: "waiting" };
        }
        return { label: "已完成", status: "success", verified: true };
      }
      return { label: "完成证据不足", status: "blocked" };
    case "unknown":
      return { label: "状态未知", status: "unknown" };
  }
}

export function taskPrimaryQuestion(envelope: ViewModelEnvelope<TasksViewModel>): string {
  if (envelope.status === "error") return "任务状态暂时无法确认";
  if (envelope.status === "stale") return "当前任务列表已陈旧，只用于查看";
  if (envelope.status === "empty" || envelope.data?.items.length === 0) {
    return "当前没有可展示的任务";
  }
  if (envelope.data?.summary.waitingPermissionCount) return "哪些任务在等待你的确认";
  if (envelope.data?.summary.waitingReviewCount) return "哪些结果在等待你的审核";
  if (envelope.data?.summary.blockedCount) return "哪些任务需要解除阻塞";
  if (envelope.data?.summary.completedNeedsEvidenceCount) return "哪些结果仍缺少完成证据";
  return "哪些任务需要我或可以继续";
}

export function formatBackendTime(value?: string | null): string | null {
  if (!value) return null;
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return null;
  return new Intl.DateTimeFormat("zh-CN", {
    month: "numeric",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(date);
}
