import { ArrowRight, FileSearch, RefreshCw } from "lucide-react";
import type { ProductAction } from "@/tauri";
import { FoundationActionButton, FoundationNotice, FoundationStatusLabel } from "@/ui/foundation";
import type {
  TodayBlockerCategory,
  TodayViewModelEnvelope,
} from "@/viewmodels/today/todayViewModel";

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

function blockerLabel(category: TodayBlockerCategory, fallback: string): string {
  switch (category) {
    case "safe_mode":
      return "安全模式正在保护外部动作与长期写入";
    case "waiting_review":
      return "有建议等待你的决定";
    case "waiting_permission":
      return "有任务等待访问确认";
    case "blocked_task":
      return "有任务处于阻断状态";
    case "provider_privacy":
      return "模型与隐私边界需要确认";
    case "missing_context":
      return "当前信息不足";
    case "unknown":
      return fallback;
  }
}

function visibleDisabledReason(action: ProductAction): string | undefined {
  if (action.enabled) return undefined;
  if (action.id === "today.open_current_workspace_route") return "请先重新读取今日状态。";
  if (action.id === "today.refresh") return "今日状态正在读取。";
  return "当前后端动作契约未开放这个入口。";
}

export function TodayReadOnlyView({
  envelope,
  refreshing,
  onRefresh,
  onNavigate,
  onOpenInspector,
}: {
  envelope: TodayViewModelEnvelope;
  refreshing: boolean;
  onRefresh: () => void;
  onNavigate: (surfaceId: string) => void;
  onOpenInspector: () => void;
}) {
  const data = ["error", "loading"].includes(envelope.status) ? null : envelope.data;
  const sourceRefreshAction =
    envelope.actions.primary.find(action => action.id === "today.refresh") ??
    ({
      id: "today.refresh",
      label: "Refresh Today state",
      kind: "refresh",
      enabled: !refreshing,
      disabledReason: refreshing ? "今日状态正在刷新。" : undefined,
      targetRef: "today",
    } satisfies ProductAction);
  const refreshAction = {
    ...sourceRefreshAction,
    enabled: sourceRefreshAction.enabled && !refreshing,
    disabledReason: refreshing ? "今日状态正在刷新。" : sourceRefreshAction.disabledReason,
  } satisfies ProductAction;
  const workspaceAction = data?.workspaceLink;
  const reviewAction = data?.reviewCenterLink;
  const inspectAction = {
    id: "today.inspect_evidence",
    label: "Inspect Today evidence",
    kind: "inspect",
    enabled: true,
    targetRef: "today.evidenceRefs",
  } satisfies ProductAction;
  const goal = data?.primaryDailyGoal ?? null;
  const visibleBlockers = data?.blockers.filter(blocker => blocker.category !== "safe_mode") ?? [];
  const hasAttention = Boolean(data && (visibleBlockers.length > 0 || data.pendingReviewCount > 0));

  return (
    <article className="ol-readonly-page" data-testid="today-product-view">
      <header className="ol-readonly-page-heading">
        <span>今天先完成什么</span>
        <h2>把注意力放在一个明确重点上</h2>
        <p>这里只整理后端已经提供的今日目标、阻塞和待决定事项。</p>
      </header>

      {envelope.status === "loading" && (
        <FoundationNotice title="正在读取今日状态" tone="neutral" live>
          读取完成前不生成新的建议，也不推断任务或长期状态。
        </FoundationNotice>
      )}

      {envelope.status === "error" && (
        <FoundationNotice title="今日状态读取失败" tone="error">
          后端没有返回可用的今日状态。当前只允许重新读取，原始错误可在检查器中核对。
        </FoundationNotice>
      )}

      {envelope.status === "stale" && !data?.safeMode.active && (
        <FoundationNotice title="当前计划已陈旧，只读且不执行" tone="protection">
          你仍可查看有来源的内容；刷新成功前不使用旧状态启动新动作。
        </FoundationNotice>
      )}

      {data?.safeMode.active && (
        <FoundationNotice title="安全模式正在保护当前工作" tone="protection">
          当前状态只读；外部动作与长期写入保持关闭。具体原因可在检查器中核对。
        </FoundationNotice>
      )}

      {data && (
        <>
          <section className="ol-readonly-section" aria-labelledby="today-current-goal">
            <div className="ol-readonly-section-heading">
              <div>
                <span>当前目标</span>
                <h3 id="today-current-goal">{goal ? goal.title : "今天还没有明确重点"}</h3>
              </div>
              {goal && (
                <FoundationStatusLabel
                  label={goal.status === "done" ? "已记录完成" : "待推进"}
                  status={
                    envelope.status === "stale"
                      ? "stale"
                      : goal.status === "done"
                        ? "success"
                        : "neutral"
                  }
                  verified={envelope.status !== "stale" && goal.status === "done"}
                />
              )}
            </div>
            <p className="ol-readonly-reading">
              {goal
                ? goal.status === "done"
                  ? "这个目标已由后端当前状态标记完成；本页不会再次写入或修改它。"
                  : "从这个目标开始；本页不会在没有后端动作契约时改变完成状态。"
                : "没有后端提供的目标时，本页不会用建议、状态指标或占位文案代替。"}
            </p>
          </section>

          {hasAttention && (
            <section className="ol-readonly-section" aria-labelledby="today-attention-title">
              <div className="ol-readonly-section-heading">
                <div>
                  <span>阻塞与风险</span>
                  <h3 id="today-attention-title">需要你留意</h3>
                </div>
              </div>
              <ul className="ol-readonly-attention-list">
                {visibleBlockers.map(blocker => (
                  <li key={blocker.id}>{blockerLabel(blocker.category, blocker.title)}</li>
                ))}
                {data.pendingReviewCount > 0 &&
                  !data.blockers.some(blocker => blocker.category === "waiting_review") && (
                    <li>{data.pendingReviewCount} 项建议等待决定，尚未批准或应用。</li>
                  )}
              </ul>
            </section>
          )}

          <section className="ol-readonly-action-area" aria-labelledby="today-next-step">
            <div>
              <span>下一步</span>
              <h3 id="today-next-step">
                {hasAttention ? "先查看需要处理的上下文" : "继续当前重点，必要时重新读取"}
              </h3>
            </div>
            <div className="ol-readonly-action-row">
              {workspaceAction && (
                <FoundationActionButton
                  label="打开工作区"
                  variant="primary"
                  icon={<ArrowRight size={18} strokeWidth={1.75} aria-hidden="true" />}
                  disabled={!workspaceAction.enabled}
                  disabledReason={visibleDisabledReason(workspaceAction)}
                  {...actionAttributes(workspaceAction)}
                  onClick={() => onNavigate("workspace")}
                />
              )}
              {reviewAction && data.pendingReviewCount > 0 && (
                <FoundationActionButton
                  label="查看待决定建议"
                  variant="secondary"
                  disabled={!reviewAction.enabled}
                  disabledReason={visibleDisabledReason(reviewAction)}
                  {...actionAttributes(reviewAction)}
                  onClick={() => onNavigate("review")}
                />
              )}
              <FoundationActionButton
                label="查看依据"
                variant="quiet"
                icon={<FileSearch size={18} strokeWidth={1.75} aria-hidden="true" />}
                {...actionAttributes(inspectAction)}
                onClick={onOpenInspector}
              />
              <FoundationActionButton
                label="重新读取"
                variant="quiet"
                icon={<RefreshCw size={18} strokeWidth={1.75} aria-hidden="true" />}
                loading={refreshing}
                loadingLabel="正在读取"
                disabled={!sourceRefreshAction.enabled && !refreshing}
                disabledReason={
                  !sourceRefreshAction.enabled && !refreshing
                    ? visibleDisabledReason(sourceRefreshAction)
                    : undefined
                }
                {...actionAttributes(refreshAction)}
                onClick={onRefresh}
              />
            </div>
          </section>
        </>
      )}

      {!data && (
        <section className="ol-readonly-action-area" aria-label="今日读取动作">
          <div>
            <span>下一步</span>
            <h3>重新读取后端状态</h3>
          </div>
          <FoundationActionButton
            label="重新读取"
            variant="primary"
            icon={<RefreshCw size={18} strokeWidth={1.75} aria-hidden="true" />}
            loading={refreshing}
            loadingLabel="正在读取"
            disabled={!sourceRefreshAction.enabled && !refreshing}
            disabledReason={
              !sourceRefreshAction.enabled && !refreshing
                ? visibleDisabledReason(sourceRefreshAction)
                : undefined
            }
            {...actionAttributes(refreshAction)}
            onClick={onRefresh}
          />
        </section>
      )}
    </article>
  );
}
