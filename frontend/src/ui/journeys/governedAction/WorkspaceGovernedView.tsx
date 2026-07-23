import { ArrowRight, Eye, RefreshCw } from "lucide-react";
import type { ReviewItem, TaskControl } from "@/tauri";
import {
  FoundationActionButton,
  FoundationDialog,
  FoundationNotice,
  FoundationStatusLabel,
} from "@/ui/foundation";
import { taskLifecyclePresentation } from "@/ui/journeys/readOnly/readOnlySpinePresentation";
import type { TaskResumeState } from "./governedActionContract";
import type { GovernedActionSnapshot } from "./governedActionDataSource";
import { findExactResumeControl } from "./governedActionPresentation";
import { WorkspaceConversationPanel } from "./WorkspaceConversationPanel";
import type { WorkspaceConversationController } from "./useWorkspaceConversation";

function resumeFeedback(
  state: TaskResumeState
): { title: string; body: string; tone: "protection" | "error" | "neutral" } | null {
  switch (state.phase) {
    case "idle":
      return null;
    case "blocked":
      return { title: "当前不能继续任务", body: state.reason, tone: "protection" };
    case "confirming":
      return { title: "等待确认", body: "确认后才会发送任务恢复请求。", tone: "neutral" };
    case "dispatching":
      return { title: "正在请求继续", body: "命令返回不代表任务已经继续。", tone: "neutral" };
    case "refreshing":
      return { title: "正在核对任务状态", body: "等待同一任务的后端读模型刷新。", tone: "neutral" };
    case "awaiting_projection":
      return {
        title: "任务仍未确认继续",
        body: "刷新结果仍在等待、阻断或缺少同一任务；当前继续保持暂停。",
        tone: "protection",
      };
    case "failed":
      return {
        title: state.stage === "dispatch" ? "继续请求失败" : "任务状态核对失败",
        body: state.errorCode,
        tone: "error",
      };
    case "resolved":
      if (state.refreshedTask.lifecycleStatus === "running") {
        return {
          title: "任务已继续，正在处理",
          body: "同一任务的刷新状态已经变为运行中；这还不是完成结论。",
          tone: "neutral",
        };
      }
      if (
        state.refreshedTask.lifecycleStatus === "completed" &&
        state.refreshedTask.finalDeliveryEvidencePresent &&
        state.refreshedTask.terminalDeliveryStatus === "delivered"
      ) {
        return {
          title: "任务已完成",
          body: "刷新后的任务状态和最终交付证据一致。",
          tone: "neutral",
        };
      }
      return {
        title: "任务状态已刷新",
        body: `同一任务当前为 ${state.refreshedTask.lifecycleStatus}；页面不会把恢复请求解释成完成。`,
        tone: "neutral",
      };
  }
}

function actionBusy(state: TaskResumeState): boolean {
  return ["dispatching", "refreshing"].includes(state.phase);
}

export function WorkspaceGovernedView({
  snapshot,
  refreshing,
  resumeState,
  onRefresh,
  onOpenReview,
  onResume,
  onConfirmResume,
  onCancelResume,
  onOpenInspector,
  conversation,
}: {
  snapshot: GovernedActionSnapshot | null;
  refreshing: boolean;
  resumeState: TaskResumeState;
  onRefresh: () => void;
  onOpenReview: (item: ReviewItem) => void;
  onResume: (control: TaskControl, taskId: string) => void;
  onConfirmResume: () => void;
  onCancelResume: () => void;
  onOpenInspector: () => void;
  conversation?: WorkspaceConversationController;
}) {
  const envelope = snapshot?.workspaceEnvelope;
  const model =
    envelope && (envelope.status === "ready" || envelope.status === "stale") ? envelope.data : null;
  const task = model?.activeTask;
  const permissionItem = model?.pendingReviewItems.find(item => item.type === "tool_permission");
  const resumeControl = snapshot ? findExactResumeControl(snapshot) : null;
  const lifecycle = task ? taskLifecyclePresentation(task) : null;
  const resumeDispatching = resumeState.phase === "dispatching";
  const resumeTarget = resumeState.phase === "idle" ? null : resumeState.control.targetTaskId;
  const feedback =
    !resumeTarget || !task || resumeTarget === task.canonicalTaskId
      ? resumeFeedback(resumeState)
      : null;
  const conversationDisabledReason = (() => {
    if (!conversation) return undefined;
    if (envelope?.status !== "ready") return "工作区读模型不是可用状态；请先重新读取。";
    if (!model) return "工作区读模型没有提供可用 payload。";
    const boundary = model.providerPrivacyBoundarySummary;
    if (boundary.blockedReason) return boundary.blockedReason;
    if (boundary.routeType === "unknown" || boundary.externalTransmission === "unknown") {
      return "模型与传输边界未知；完成核对前不能发送。";
    }
    if (boundary.localOnlyRequired && boundary.routeType !== "local") {
      return "当前要求仅本机处理，但后端没有确认本地路由。";
    }
    if (
      task?.lifecycleStatus === "waiting_permission" &&
      conversation.selectedSessionId !== null &&
      (!task.conversationId || task.conversationId === conversation.selectedSessionId)
    ) {
      return "当前对话正在等待权限决定；先处理请求，或开始一段新对话。";
    }
    return undefined;
  })();

  if (!snapshot || !envelope || envelope.status === "loading") {
    return (
      <div className="ol-governed-page ol-governed-page--centered" aria-busy="true">
        <FoundationNotice title="正在读取当前任务" tone="neutral">
          <p>读取完成前不开放权限决定或任务控制。</p>
        </FoundationNotice>
      </div>
    );
  }

  if (envelope.status === "error") {
    return (
      <div className="ol-governed-page ol-governed-page--centered">
        <FoundationNotice title="工作区状态暂时不可用" tone="error">
          <p>后端没有返回可确认的工作区状态；缺失数据不会被解释成没有任务。</p>
        </FoundationNotice>
        <FoundationActionButton
          label="重新读取"
          icon={<RefreshCw size={17} aria-hidden="true" />}
          loading={refreshing}
          loadingLabel="正在读取"
          disabled={resumeDispatching}
          disabledReason={resumeDispatching ? "任务恢复请求正在发送；请等待状态核对。" : undefined}
          onClick={onRefresh}
        />
      </div>
    );
  }

  if (!task) {
    return (
      <div className="ol-governed-page">
        <header className="ol-workspace-task-header">
          <div>
            <span className="ol-governed-kicker">当前执行</span>
            <h2>没有活动任务</h2>
          </div>
          <FoundationStatusLabel label="没有活动任务" status="neutral" />
        </header>
        <p className="ol-governed-muted">
          工作区不会把最近历史提升为当前执行。任务历史仍由“任务”页面负责。
        </p>
        <div className="ol-governed-inline-actions">
          <FoundationActionButton
            label="重新读取"
            icon={<RefreshCw size={17} aria-hidden="true" />}
            loading={refreshing}
            loadingLabel="正在读取"
            disabled={resumeDispatching}
            disabledReason={
              resumeDispatching ? "任务恢复请求正在发送；请等待状态核对。" : undefined
            }
            onClick={onRefresh}
          />
          <FoundationActionButton
            label="查看状态依据"
            icon={<Eye size={17} aria-hidden="true" />}
            variant="quiet"
            onClick={onOpenInspector}
          />
        </div>
        {conversation && (
          <WorkspaceConversationPanel
            controller={conversation}
            disabledReason={conversationDisabledReason}
          />
        )}
      </div>
    );
  }

  return (
    <article className="ol-governed-page" data-workspace-task-id={task.canonicalTaskId}>
      <header className="ol-workspace-task-header">
        <div>
          <span className="ol-governed-kicker">当前任务</span>
          <h2>{task.title}</h2>
        </div>
        {lifecycle && (
          <FoundationStatusLabel
            label={lifecycle.label}
            status={lifecycle.status}
            verified={lifecycle.verified}
          />
        )}
      </header>

      {envelope.status === "stale" && (
        <FoundationNotice title="工作区状态已陈旧" tone="protection" live>
          <p>刷新成功前，审核与任务恢复都保持关闭。</p>
        </FoundationNotice>
      )}

      <section className="ol-governed-section" aria-labelledby="workspace-current-state">
        <div className="ol-governed-section-heading">
          <span>当前状态</span>
          <h3 id="workspace-current-state">
            {task.lifecycleStatus === "waiting_permission"
              ? "任务暂停在一个动作之前"
              : task.lifecycleStatus === "running"
                ? "任务正在处理"
                : "任务状态需要核对"}
          </h3>
        </div>
        {task.latestResultPreview?.preview && <p>{task.latestResultPreview.preview}</p>}
        {!task.latestResultPreview?.preview && (
          <p>页面只呈现后端提供的任务状态，不根据标题或历史记录补写进度。</p>
        )}
      </section>

      {(permissionItem || task.pendingBlockers.length > 0) && (
        <section className="ol-workspace-blocker" aria-labelledby="workspace-blocker-title">
          <div>
            <span>当前阻塞</span>
            <h3 id="workspace-blocker-title">
              {permissionItem?.decisionContext.summary ?? "任务暂时不能继续"}
            </h3>
          </div>
          <p>
            {permissionItem?.decisionContext.permission?.purposeSummary ??
              task.pendingBlockers[0] ??
              "后端没有提供可读阻塞原因。"}
          </p>
        </section>
      )}

      <section className="ol-workspace-next" aria-labelledby="workspace-next-title">
        <div className="ol-governed-section-heading">
          <span>下一步</span>
          <h3 id="workspace-next-title">
            {permissionItem
              ? "先核对访问范围，再作决定"
              : resumeControl?.enabled
                ? "权限决定已刷新，可以请求继续"
                : "当前没有可执行控制"}
          </h3>
        </div>
        <div className="ol-governed-inline-actions">
          {permissionItem && (
            <FoundationActionButton
              label="查看权限请求"
              icon={<ArrowRight size={17} aria-hidden="true" />}
              variant="primary"
              disabled={envelope.status === "stale"}
              disabledReason={
                envelope.status === "stale" ? "工作区状态已陈旧；请先重新读取。" : undefined
              }
              onClick={() => onOpenReview(permissionItem)}
            />
          )}
          {!permissionItem && resumeControl && task.taskSessionId && (
            <FoundationActionButton
              label="继续任务"
              icon={<ArrowRight size={17} aria-hidden="true" />}
              variant="primary"
              data-action-category="task-control"
              data-action-id={resumeControl.id}
              data-action-kind={resumeControl.kind}
              data-action-effect={resumeControl.effect}
              data-action-enabled={String(resumeControl.enabled)}
              data-action-disabled-reason={resumeControl.disabledReason ?? ""}
              data-action-target-ref={resumeControl.targetTaskId}
              data-action-requires-confirmation={String(
                Boolean(resumeControl.requiresConfirmation)
              )}
              data-action-completion-proof-after-dispatch={String(
                Boolean(resumeControl.completionProofAfterDispatch)
              )}
              loading={actionBusy(resumeState)}
              loadingLabel={resumeState.phase === "refreshing" ? "正在核对" : "正在请求"}
              disabled={!resumeControl.enabled || envelope.status === "stale"}
              disabledReason={
                envelope.status === "stale"
                  ? "工作区状态已陈旧；请先重新读取。"
                  : resumeControl.enabled
                    ? undefined
                    : resumeControl.disabledReason || "后端未允许恢复当前任务。"
              }
              onClick={() => onResume(resumeControl, task.taskSessionId!)}
            />
          )}
          <FoundationActionButton
            label="状态依据"
            icon={<Eye size={17} aria-hidden="true" />}
            variant="quiet"
            onClick={onOpenInspector}
          />
          <FoundationActionButton
            label="重新读取"
            icon={<RefreshCw size={17} aria-hidden="true" />}
            variant="quiet"
            loading={refreshing}
            loadingLabel="正在读取"
            disabled={resumeDispatching}
            disabledReason={
              resumeDispatching ? "任务恢复请求正在发送；请等待状态核对。" : undefined
            }
            onClick={onRefresh}
          />
        </div>
      </section>

      {feedback && (
        <FoundationNotice title={feedback.title} tone={feedback.tone} live>
          <p>{feedback.body}</p>
        </FoundationNotice>
      )}

      {conversation && (
        <WorkspaceConversationPanel
          controller={conversation}
          disabledReason={conversationDisabledReason}
        />
      )}

      <details className="ol-workspace-activity">
        <summary id="workspace-activity-title">执行记录</summary>
        {model?.activity.length ? (
          <ol aria-labelledby="workspace-activity-title">
            {model.activity.map(item => (
              <li key={item.id} data-activity-status={item.status}>
                <span className="ol-workspace-activity__marker" aria-hidden="true" />
                <div>
                  <strong>{item.label}</strong>
                  <p>{item.summary}</p>
                </div>
              </li>
            ))}
          </ol>
        ) : (
          <p className="ol-governed-muted">当前没有后端提供的 metadata-only 活动记录。</p>
        )}
      </details>

      <FoundationDialog
        open={resumeState.phase === "confirming"}
        title="确认继续这项任务？"
        description="确认只发送任务恢复请求；运行和完成状态仍以后端刷新结果为准。"
        onClose={onCancelResume}
        footer={
          <>
            <FoundationActionButton label="取消" variant="quiet" onClick={onCancelResume} />
            <FoundationActionButton
              label="确认继续"
              variant="primary"
              icon={<ArrowRight size={17} aria-hidden="true" />}
              onClick={onConfirmResume}
            />
          </>
        }
      >
        <p>{task.title}</p>
      </FoundationDialog>
    </article>
  );
}
