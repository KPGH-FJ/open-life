import { useEffect, useState } from "react";
import { ArrowLeft, Eye, RefreshCw, ShieldCheck } from "lucide-react";
import type { ReviewAction, ReviewItem } from "@/tauri";
import {
  FoundationActionButton,
  FoundationDialog,
  FoundationNotice,
  FoundationStatusLabel,
} from "@/ui/foundation";
import type { ReviewDispatchState } from "@/contracts/reviewDispatchContract";
import type { GovernedActionSnapshot } from "./governedActionDataSource";

const MAX_VISIBLE_LIFEMODEL_LEARNING_REVIEWS = 5;

export type ReviewQueueSection = {
  id: "lifemodel_learning" | "other";
  label: string;
  items: ReviewItem[];
  totalCount: number;
  hiddenCount: number;
};

export function reviewQueueSections(items: ReviewItem[]): ReviewQueueSection[] {
  const learning = items.filter(item => item.decisionContext.lifeModelLearning);
  const learningAwaitingDecision = learning.filter(item =>
    ["pending", "edited", "deferred"].includes(item.status)
  );
  const learningHistory = learning.filter(
    item => !["pending", "edited", "deferred"].includes(item.status)
  );
  const orderedLearning = [...learningAwaitingDecision, ...learningHistory];
  const other = items.filter(item => !item.decisionContext.lifeModelLearning);
  const sections: ReviewQueueSection[] = [];
  if (learning.length > 0) {
    sections.push({
      id: "lifemodel_learning",
      label: "LifeModel 学习建议",
      items: orderedLearning.slice(0, MAX_VISIBLE_LIFEMODEL_LEARNING_REVIEWS),
      totalCount: learning.length,
      hiddenCount: Math.max(0, learning.length - MAX_VISIBLE_LIFEMODEL_LEARNING_REVIEWS),
    });
  }
  if (other.length > 0) {
    sections.push({
      id: "other",
      label: "其他建议与权限",
      items: other,
      totalCount: other.length,
      hiddenCount: 0,
    });
  }
  return sections;
}

function actionOperation(item: ReviewItem): string | undefined {
  return item.decisionContext.actionContract?.operation;
}

function approvedActionLabel(item: ReviewItem): string {
  switch (actionOperation(item)) {
    case "move":
      return "批准并移动文件";
    case "trash":
      return "批准并移入 OpenLife 恢复区";
    case "restore":
      return "批准并恢复文件";
    case "create_local_calendar_projection":
      return "批准并创建本地任务及日历投影";
    case "create_scheduled_task":
      return "批准并创建定时任务";
    case "open_email_draft":
      return "批准并打开邮件草稿";
    case "open_browser_url":
      return "批准并打开网址";
    case "run_local_utility":
      return "批准并运行本地工具";
    case "export_data":
      return "批准并导出";
    default:
      return item.type === "external_write_action" ? "批准并写入文件" : "批准变更";
  }
}

function confirmedActionStatusLabel(item: ReviewItem): string {
  switch (actionOperation(item)) {
    case "move":
      return "文件移动已核验";
    case "trash":
      return "已移入恢复区";
    case "restore":
      return "文件恢复已核验";
    case "create_local_calendar_projection":
      return "本地任务及日历投影已处理";
    case "create_scheduled_task":
      return "定时任务已创建";
    case "open_email_draft":
      return "邮件草稿交接已记录";
    case "open_browser_url":
      return "浏览器交接已记录";
    case "run_local_utility":
      return "本地工具已完成";
    default:
      return item.type === "external_write_action" ? "文件写入已核验" : "已应用";
  }
}

export function reviewItemStatus(item: ReviewItem): {
  label: string;
  status: "neutral" | "waiting" | "unknown" | "error" | "success";
  verified?: boolean;
} {
  if (item.type === "external_write_action" && item.artifactEvidence) {
    if (
      item.artifactEvidence.state === "confirmed" &&
      item.artifactEvidence.observedContentDigest === item.artifactEvidence.contentDigest
    ) {
      return { label: confirmedActionStatusLabel(item), status: "success", verified: true };
    }
    if (item.artifactEvidence.state === "failed_before_effect") {
      return { label: "执行前失败", status: "error" };
    }
    if (item.artifactEvidence.state === "unknown") {
      return { label: "文件状态未知", status: "unknown" };
    }
    if (item.artifactEvidence.state === "confirmed") {
      return { label: "文件状态未知", status: "unknown" };
    }
    return { label: "正在核对文件", status: "waiting" };
  }
  if (["pending", "edited", "deferred"].includes(item.status)) {
    return { label: item.status === "deferred" ? "稍后处理" : "等待决定", status: "waiting" };
  }
  if (item.status === "approved") {
    if (item.type === "tool_permission") {
      return { label: "已允许一次", status: "neutral" };
    }
    if (item.materializationStatus === "applying") return { label: "正在应用", status: "waiting" };
    if (item.materializationStatus === "applied")
      return { label: confirmedActionStatusLabel(item), status: "success", verified: true };
    if (item.materializationStatus === "failed") return { label: "应用失败", status: "error" };
    if (item.materializationStatus === "rolled_back") return { label: "已回滚", status: "waiting" };
    if (item.materializationStatus === "unknown")
      return { label: "应用状态未知", status: "unknown" };
    return { label: "已批准，尚未应用", status: "neutral" };
  }
  if (item.status === "rejected") return { label: "已拒绝", status: "neutral" };
  return { label: "状态未知", status: "unknown" };
}

function actionLabel(action: ReviewAction, item: ReviewItem): string {
  if (action.kind === "approve") {
    return item.type === "tool_permission" ? "仅允许本次" : approvedActionLabel(item);
  }
  const labels: Partial<Record<ReviewAction["kind"], string>> = {
    reject: "拒绝",
    later: "稍后处理",
    edit: "修改",
    apply: "应用变更",
    revoke: "撤销",
    view_evidence: item.type === "tool_permission" ? "查看访问范围" : "查看依据",
  };
  return labels[action.kind] ?? action.label;
}

export function reviewDecisionFeedback(
  state: ReviewDispatchState,
  item: ReviewItem
): { title: string; body: string; tone: "protection" | "error" | "neutral" } | null {
  switch (state.phase) {
    case "idle":
      return null;
    case "blocked":
      return { title: "当前不能记录决定", body: state.reason, tone: "protection" };
    case "confirming":
      return null;
    case "dispatching":
      return { title: "正在记录决定", body: "命令返回后仍需刷新同一个审核项。", tone: "neutral" };
    case "refreshing":
      return {
        title: "正在核对决定",
        body: "正在重新读取审核项与关联任务。",
        tone: "neutral",
      };
    case "awaiting_projection":
      return {
        title: "决定尚未被读模型确认",
        body: "刷新后的同一审核项仍未确认请求的决定；任务继续保持暂停。",
        tone: "protection",
      };
    case "failed":
      return {
        title: state.stage === "dispatch" ? "决定记录失败" : "决定状态核对失败",
        body: state.errorCode,
        tone: "error",
      };
    case "resolved":
      if (state.action.kind === "approve" && item.type === "tool_permission") {
        return {
          title: "决定已记录，尚未继续任务",
          body: "下一步必须返回工作区，核对同一任务是否出现有效的恢复控制。",
          tone: "neutral",
        };
      }
      if (state.action.kind === "approve") {
        if (item.type === "external_write_action") {
          const evidence = item.artifactEvidence;
          return evidence?.state === "confirmed" &&
            evidence.observedContentDigest === evidence.contentDigest
            ? {
                title: "文件写入已核验",
                body: "刷新后的同一审核项包含目标、内容和落盘观察摘要；这不是从按钮回调推断的完成。",
                tone: "neutral",
              }
            : {
                title: "文件结果尚未确认",
                body: "批准请求已经返回，但后端读模型没有提供匹配的确认收据；当前保持未完成或未知。",
                tone: "protection",
              };
        }
        if (item.decisionContext.actionContract) {
          return state.refreshed.materializationStatus === "applied"
            ? {
                title: confirmedActionStatusLabel(item),
                body: item.decisionContext.actionContract.terminalEvidenceSummary,
                tone: "neutral",
              }
            : {
                title: "动作结果尚未确认",
                body: "批准已返回，但刷新后的后端读模型尚未证明这一精确动作完成。",
                tone: "protection",
              };
        }
        return state.refreshed.materializationStatus === "applied"
          ? {
              title: "变更已应用",
              body: "应用结论来自刷新后的同一审核项，不是批准命令的返回值。",
              tone: "neutral",
            }
          : {
              title: "已批准，尚未应用",
              body: "批准只记录决定；应用与完成仍需后端读模型提供独立结果。",
              tone: "neutral",
            };
      }
      return {
        title:
          state.action.kind === "reject"
            ? "已拒绝，不会执行该动作"
            : state.action.kind === "later"
              ? "已设为稍后处理，任务仍暂停"
              : "决定已由刷新后的读模型确认",
        body: "页面没有从命令回调推断额外结果。",
        tone: "neutral",
      };
  }
}

export function ReviewGovernedView({
  snapshot,
  selectedItem,
  refreshing,
  dispatchState,
  onRefresh,
  onSelectItem,
  onRequestAction,
  onConfirmAction,
  onCancelConfirmation,
  onEditLifeModelLearning,
  onBackWorkspace,
  backLabel = "返回工作区",
  onOpenInspector,
  visibleItems,
  embedded = false,
}: {
  snapshot: GovernedActionSnapshot | null;
  selectedItem: ReviewItem | null;
  refreshing: boolean;
  dispatchState: ReviewDispatchState;
  onRefresh: () => void;
  onSelectItem: (item: ReviewItem) => void;
  onRequestAction: (action: ReviewAction) => void;
  onConfirmAction: () => void;
  onCancelConfirmation: () => void;
  onEditLifeModelLearning: (statement: string) => Promise<boolean>;
  onBackWorkspace: () => void;
  backLabel?: string;
  onOpenInspector: () => void;
  visibleItems?: readonly ReviewItem[];
  embedded?: boolean;
}) {
  const [learningDraft, setLearningDraft] = useState("");
  const [learningEditBusy, setLearningEditBusy] = useState(false);
  const envelope = snapshot?.reviewEnvelope;
  const items = visibleItems
    ? [...visibleItems]
    : envelope && (envelope.status === "ready" || envelope.status === "stale")
      ? (envelope.data?.items ?? [])
      : [];
  const queueSections = reviewQueueSections(items);
  const permission = selectedItem?.decisionContext.permission;
  const dispatchAction = dispatchState.phase === "idle" ? null : dispatchState.action;
  const feedback =
    selectedItem && dispatchAction?.targetReviewItemId === selectedItem.id
      ? reviewDecisionFeedback(dispatchState, selectedItem)
      : null;
  const confirmingAction = dispatchState.phase === "confirming" ? dispatchState.action : null;
  const decisionBusy =
    dispatchState.phase === "dispatching" ||
    dispatchState.phase === "refreshing" ||
    learningEditBusy;
  const supportedActions =
    selectedItem?.allowedActions.filter(action =>
      ["approve", "reject", "later", "apply", "view_evidence"].includes(action.kind)
    ) ?? [];
  const hasEvidenceAction = supportedActions.some(action => action.kind === "view_evidence");
  const learningContext = selectedItem?.decisionContext.lifeModelLearning;
  const learningEditAction = selectedItem?.allowedActions.find(action => action.kind === "edit");

  useEffect(() => {
    setLearningDraft(learningContext?.proposedStatement ?? "");
    setLearningEditBusy(false);
  }, [learningContext?.candidateSnapshotDigest, learningContext?.proposedStatement]);

  if (!snapshot || !envelope || envelope.status === "loading") {
    return (
      <div className="ol-governed-page ol-governed-page--centered" aria-busy="true">
        <FoundationNotice title="正在读取需处理事项" tone="neutral">
          <p>读取完成前不开放决定动作。</p>
        </FoundationNotice>
      </div>
    );
  }

  if (envelope.status === "error") {
    return (
      <div className="ol-governed-page ol-governed-page--centered">
        <FoundationNotice title="审核状态暂时不可用" tone="error">
          <p>后端没有返回可确认的审核状态；当前不会从旧建议列表拼出决定页面。</p>
        </FoundationNotice>
        <FoundationActionButton
          label="重新读取"
          icon={<RefreshCw size={17} aria-hidden="true" />}
          loading={refreshing}
          loadingLabel="正在读取"
          onClick={onRefresh}
        />
      </div>
    );
  }

  if (items.length === 0) {
    return (
      <div className="ol-governed-page ol-governed-page--centered">
        <div className="ol-governed-empty">
          <span>建议与权限</span>
          <h2>暂无审核项</h2>
          <p>空列表只表示当前没有待决定项，不代表所有长期变更都已经应用。</p>
        </div>
        <div className="ol-governed-inline-actions">
          <FoundationActionButton
            label={backLabel}
            icon={<ArrowLeft size={17} aria-hidden="true" />}
            onClick={onBackWorkspace}
          />
          <FoundationActionButton
            label="重新读取"
            icon={<RefreshCw size={17} aria-hidden="true" />}
            variant="quiet"
            loading={refreshing}
            loadingLabel="正在读取"
            onClick={onRefresh}
          />
        </div>
      </div>
    );
  }

  return (
    <div className={`ol-review-layout${embedded ? " ol-review-layout--embedded" : ""}`}>
      <aside className="ol-review-queue" aria-label="审核项列表">
        <header>
          <span>需要逐项决定</span>
          <h2>建议与权限</h2>
        </header>
        <div className="ol-review-queue__items">
          {queueSections.map(section => (
            <section
              key={section.id}
              className="ol-review-queue-section"
              aria-labelledby={`review-queue-${section.id}`}
            >
              <div className="ol-review-queue-section__heading">
                <span id={`review-queue-${section.id}`}>{section.label}</span>
                <small>{section.totalCount} 项 · 逐项决定</small>
              </div>
              {section.items.map(item => {
                const status = reviewItemStatus(item);
                return (
                  <button
                    key={item.id}
                    type="button"
                    className="ol-review-queue-item"
                    data-current={selectedItem?.id === item.id ? "true" : "false"}
                    aria-current={selectedItem?.id === item.id ? "true" : undefined}
                    disabled={decisionBusy}
                    onClick={() => onSelectItem(item)}
                  >
                    <span>{item.decisionContext.title}</span>
                    <small>{status.label}</small>
                  </button>
                );
              })}
              {section.hiddenCount > 0 && (
                <p className="ol-review-queue-section__overflow">
                  其余 {section.hiddenCount} 项 LifeModel 学习记录暂不在当前队列展示。
                </p>
              )}
            </section>
          ))}
        </div>
      </aside>

      <article className="ol-review-detail" data-review-item-id={selectedItem?.id ?? "none"}>
        {!selectedItem ? (
          <div className="ol-governed-empty">
            <span>审核详情</span>
            <h2>选择一项查看</h2>
            <p>打开审核项只改变页面上下文，不会批准、拒绝或应用任何变更。</p>
          </div>
        ) : (
          <>
            <header className="ol-review-detail__header">
              <div>
                <span className="ol-governed-kicker">
                  {selectedItem.type === "tool_permission"
                    ? "一次性权限"
                    : learningContext
                      ? "LifeModel 学习建议"
                      : "变更建议"}
                </span>
                <h2>{selectedItem.decisionContext.title}</h2>
                <p>{selectedItem.decisionContext.summary}</p>
              </div>
              {(() => {
                const status = reviewItemStatus(selectedItem);
                return (
                  <FoundationStatusLabel
                    label={status.label}
                    status={status.status}
                    verified={status.verified}
                  />
                );
              })()}
            </header>

            {envelope.status === "stale" && (
              <FoundationNotice title="审核状态已陈旧" tone="protection" live>
                <p>刷新成功前，所有决定动作保持关闭。</p>
              </FoundationNotice>
            )}

            {permission ? (
              <section className="ol-review-diff" aria-labelledby="permission-change-title">
                <span>这项决定会改变什么</span>
                <h3 id="permission-change-title">未授权 → 仅允许一次精确动作</h3>
                <div className="ol-review-diff__columns">
                  <div>
                    <small>当前</small>
                    <strong>
                      {selectedItem.decisionContext.before?.summary ?? "请求尚未发送，动作尚未执行"}
                    </strong>
                  </div>
                  <div>
                    <small>批准后</small>
                    <strong>{selectedItem.decisionContext.after.summary}</strong>
                  </div>
                </div>
              </section>
            ) : (
              <section className="ol-review-diff" aria-labelledby="review-change-title">
                <span>建议变更</span>
                <h3 id="review-change-title">当前 → 建议</h3>
                <div className="ol-review-diff__columns">
                  <div>
                    <small>当前</small>
                    <strong>
                      {selectedItem.decisionContext.before?.summary ?? "后端未提供当前值"}
                    </strong>
                  </div>
                  <div>
                    <small>建议</small>
                    <strong>{selectedItem.decisionContext.after.summary}</strong>
                  </div>
                </div>
                {selectedItem.decisionContext.after.detail && (
                  <details>
                    <summary>查看精确变更</summary>
                    <pre>{selectedItem.decisionContext.after.detail}</pre>
                  </details>
                )}
              </section>
            )}

            {selectedItem.decisionContext.actionContract && (
              <section className="ol-review-rationale" aria-labelledby="action-contract-title">
                <div className="ol-governed-section-heading">
                  <span>动作合同</span>
                  <h3 id="action-contract-title">这次批准精确允许什么</h3>
                </div>
                <dl>
                  <div>
                    <dt>能力</dt>
                    <dd>{selectedItem.decisionContext.actionContract.capabilityId}</dd>
                  </div>
                  <div>
                    <dt>操作</dt>
                    <dd>{selectedItem.decisionContext.actionContract.operation}</dd>
                  </div>
                  <div>
                    <dt>确认范围</dt>
                    <dd>{selectedItem.decisionContext.actionContract.confirmationSummary}</dd>
                  </div>
                  <div>
                    <dt>副作用边界</dt>
                    <dd>{selectedItem.decisionContext.actionContract.effectBoundary}</dd>
                  </div>
                  <div>
                    <dt>完成证据</dt>
                    <dd>{selectedItem.decisionContext.actionContract.terminalEvidenceSummary}</dd>
                  </div>
                </dl>
                {selectedItem.decisionContext.after.detail && (
                  <details>
                    <summary>查看已审核的精确参数</summary>
                    <pre>{selectedItem.decisionContext.after.detail}</pre>
                  </details>
                )}
              </section>
            )}

            {learningContext && (
              <section
                className="ol-review-rationale"
                aria-labelledby="lifemodel-learning-review-title"
              >
                <div className="ol-governed-section-heading">
                  <span>长期信息证据</span>
                  <h3 id="lifemodel-learning-review-title">逐条核对后再决定</h3>
                </div>
                <dl>
                  <div>
                    <dt>位置</dt>
                    <dd>{learningContext.section}</dd>
                  </div>
                  <div>
                    <dt>证据</dt>
                    <dd>
                      {learningContext.supportCount} 条，来自{" "}
                      {learningContext.independentSupportCount} 个独立来源
                    </dd>
                  </div>
                  <div>
                    <dt>稳定性</dt>
                    <dd>{learningContext.stability}</dd>
                  </div>
                  <div>
                    <dt>确认时间</dt>
                    <dd>{learningContext.confirmedAt}</dd>
                  </div>
                  <div>
                    <dt>来源类型</dt>
                    <dd>{learningContext.sourceKinds.join("、") || "未知"}</dd>
                  </div>
                  <div>
                    <dt>来源记录</dt>
                    <dd>{learningContext.sourceRefs.join("、") || "未知"}</dd>
                  </div>
                  <div>
                    <dt>冲突</dt>
                    <dd>
                      {learningContext.conflictStatus === "none"
                        ? "未发现冲突"
                        : learningContext.conflictStatus}
                    </dd>
                  </div>
                </dl>
                <label>
                  审核后的表述
                  <textarea
                    value={learningDraft}
                    maxLength={500}
                    disabled={
                      learningEditBusy ||
                      envelope.status === "stale" ||
                      !learningEditAction?.enabled
                    }
                    onChange={event => setLearningDraft(event.target.value)}
                  />
                </label>
                <FoundationActionButton
                  label="保存修改，继续审核"
                  variant="secondary"
                  loading={learningEditBusy}
                  loadingLabel="正在核对"
                  disabled={
                    !learningEditAction?.enabled ||
                    envelope.status === "stale" ||
                    learningDraft.trim() === learningContext.proposedStatement
                  }
                  disabledReason={
                    (envelope.status === "stale"
                      ? "审核状态已陈旧；请先重新读取。"
                      : learningEditAction?.disabledReason) ??
                    (learningDraft.trim() === learningContext.proposedStatement
                      ? "内容尚未改变。"
                      : undefined)
                  }
                  onClick={() => {
                    setLearningEditBusy(true);
                    void onEditLifeModelLearning(learningDraft).finally(() =>
                      setLearningEditBusy(false)
                    );
                  }}
                />
              </section>
            )}

            {permission && (
              <section className="ol-permission-summary" aria-labelledby="permission-scope-title">
                <div className="ol-governed-section-heading">
                  <span>访问范围</span>
                  <h3 id="permission-scope-title">本次请求具体允许什么</h3>
                </div>
                <dl>
                  <div>
                    <dt>工具</dt>
                    <dd>{permission.toolLabel}</dd>
                  </div>
                  <div>
                    <dt>请求目标</dt>
                    <dd>{permission.requestedTargetLabel ?? "目标未知"}</dd>
                  </div>
                  {permission.resolvedTargetLabel && (
                    <div>
                      <dt>解析目标</dt>
                      <dd>{permission.resolvedTargetLabel}</dd>
                    </div>
                  )}
                  <div>
                    <dt>用途</dt>
                    <dd>{permission.purposeSummary}</dd>
                  </div>
                  <div>
                    <dt>传输边界</dt>
                    <dd>{permission.transmissionBoundary.summary}</dd>
                  </div>
                  <div>
                    <dt>有效方式</dt>
                    <dd>
                      {permission.policy === "allow_once" ? "一次精确匹配，使用后失效" : "策略未知"}
                    </dd>
                  </div>
                  <div>
                    <dt>撤销与失效</dt>
                    <dd>{permission.revocationSummary}</dd>
                  </div>
                </dl>
                {permission.status === "incomplete" && (
                  <FoundationNotice title="访问范围不完整" tone="protection" live>
                    <p>缺少 {permission.missingFields.join("、") || "必要字段"}；批准保持禁用。</p>
                  </FoundationNotice>
                )}
              </section>
            )}

            <section className="ol-review-rationale" aria-labelledby="review-rationale-title">
              <div className="ol-governed-section-heading">
                <span>原因与影响</span>
                <h3 id="review-rationale-title">为什么需要这项决定</h3>
              </div>
              <dl>
                <div>
                  <dt>原因</dt>
                  <dd>{selectedItem.decisionContext.reasonSummary}</dd>
                </div>
                <div>
                  <dt>影响</dt>
                  <dd>{selectedItem.decisionContext.impactSummary}</dd>
                </div>
                <div>
                  <dt>对象</dt>
                  <dd>
                    {selectedItem.decisionContext.affectedObjectLabels.join("、") || "未提供"}
                  </dd>
                </div>
                <div>
                  <dt>来源</dt>
                  <dd>{selectedItem.decisionContext.sourceSummary}</dd>
                </div>
                <div>
                  <dt>到期</dt>
                  <dd>{selectedItem.expiresAt ?? "后端未提供到期时间"}</dd>
                </div>
              </dl>
            </section>

            {selectedItem.type === "external_write_action" && (
              <section className="ol-review-rationale" aria-labelledby="artifact-evidence-title">
                <div className="ol-governed-section-heading">
                  <span>本地文件动作</span>
                  <h3 id="artifact-evidence-title">后端终态证据</h3>
                </div>
                {selectedItem.artifactEvidence ? (
                  <dl>
                    <div>
                      <dt>状态</dt>
                      <dd>{selectedItem.artifactEvidence.state}</dd>
                    </div>
                    <div>
                      <dt>大小</dt>
                      <dd>{selectedItem.artifactEvidence.byteSize} bytes</dd>
                    </div>
                    <div>
                      <dt>类型</dt>
                      <dd>{selectedItem.artifactEvidence.mediaType}</dd>
                    </div>
                    <div>
                      <dt>内容摘要</dt>
                      <dd>{selectedItem.artifactEvidence.contentDigest}</dd>
                    </div>
                    <div>
                      <dt>落盘观察</dt>
                      <dd>{selectedItem.artifactEvidence.observedContentDigest ?? "尚未确认"}</dd>
                    </div>
                    <div>
                      <dt>目标摘要</dt>
                      <dd>{selectedItem.artifactEvidence.targetReferenceDigest}</dd>
                    </div>
                  </dl>
                ) : (
                  <FoundationNotice title="尚无文件动作收据" tone="protection" live>
                    <p>
                      批准前这是正常状态；批准后若仍无收据，页面不会显示创建、移动、回收或恢复已完成。
                    </p>
                  </FoundationNotice>
                )}
              </section>
            )}

            {feedback && (
              <FoundationNotice title={feedback.title} tone={feedback.tone} live>
                <p>{feedback.body}</p>
              </FoundationNotice>
            )}

            <footer className="ol-review-actions" aria-label="审核决定">
              <div className="ol-review-actions__secondary">
                <FoundationActionButton
                  label={backLabel}
                  icon={<ArrowLeft size={17} aria-hidden="true" />}
                  variant="quiet"
                  onClick={onBackWorkspace}
                />
                {!hasEvidenceAction && (
                  <FoundationActionButton
                    label="状态依据"
                    icon={<Eye size={17} aria-hidden="true" />}
                    variant="quiet"
                    onClick={onOpenInspector}
                  />
                )}
                <FoundationActionButton
                  label="重新读取"
                  icon={<RefreshCw size={17} aria-hidden="true" />}
                  variant="quiet"
                  loading={refreshing}
                  loadingLabel="正在读取"
                  disabled={decisionBusy}
                  disabledReason={
                    decisionBusy ? "正在记录或核对决定；请等待当前读取完成。" : undefined
                  }
                  onClick={onRefresh}
                />
              </div>
              <div className="ol-review-actions__decisions">
                {supportedActions.map(action => {
                  const evidenceOnly = action.kind === "view_evidence";
                  const unsupportedDispatch = action.kind === "apply";
                  const disabledByEnvelope = envelope.status === "stale" && !evidenceOnly;
                  const disabledByBusy = decisionBusy && !evidenceOnly;
                  return (
                    <FoundationActionButton
                      key={action.id}
                      label={actionLabel(action, selectedItem)}
                      data-action-category="review"
                      data-action-id={action.id}
                      data-action-kind={action.kind}
                      data-action-effect={action.effect}
                      data-action-enabled={String(action.enabled)}
                      data-action-disabled-reason={action.disabledReason ?? ""}
                      data-action-target-ref={action.targetReviewItemId}
                      data-action-requires-confirmation={String(
                        Boolean(action.requiresConfirmation)
                      )}
                      data-action-expected-materialization-status={
                        action.expectedMaterializationStatusAfterDispatch ?? "unknown"
                      }
                      data-action-completion-proof-after-dispatch={String(
                        action.completionProofAfterDispatch
                      )}
                      icon={
                        action.kind === "approve" ? (
                          <ShieldCheck size={17} aria-hidden="true" />
                        ) : action.kind === "view_evidence" ? (
                          <Eye size={17} aria-hidden="true" />
                        ) : action.kind === "apply" ? (
                          <RefreshCw size={17} aria-hidden="true" />
                        ) : undefined
                      }
                      variant={
                        action.kind === "approve"
                          ? "primary"
                          : action.kind === "reject"
                            ? "danger"
                            : "secondary"
                      }
                      loading={
                        (dispatchState.phase === "dispatching" ||
                          dispatchState.phase === "refreshing") &&
                        dispatchState.action.id === action.id
                      }
                      loadingLabel={dispatchState.phase === "refreshing" ? "正在核对" : "正在记录"}
                      disabled={
                        !action.enabled ||
                        disabledByEnvelope ||
                        disabledByBusy ||
                        unsupportedDispatch
                      }
                      disabledReason={
                        disabledByEnvelope
                          ? "审核状态已陈旧；请先重新读取。"
                          : disabledByBusy
                            ? "正在核对上一项决定。"
                            : unsupportedDispatch
                              ? action.disabledReason ||
                                "当前前端没有可调用的 typed apply command；保持只读。"
                              : action.enabled
                                ? undefined
                                : action.disabledReason || "后端未允许该决定。"
                      }
                      onClick={() => (evidenceOnly ? onOpenInspector() : onRequestAction(action))}
                    />
                  );
                })}
              </div>
            </footer>
          </>
        )}
      </article>

      <FoundationDialog
        open={Boolean(confirmingAction && selectedItem)}
        title={
          selectedItem?.type === "tool_permission"
            ? "仅允许这一次？"
            : selectedItem?.decisionContext.actionContract
              ? `${approvedActionLabel(selectedItem)}？`
              : "确认批准变更？"
        }
        description={
          selectedItem?.decisionContext.actionContract
            ? `${selectedItem.decisionContext.actionContract.confirmationSummary} ${selectedItem.decisionContext.actionContract.terminalEvidenceSummary}`
            : "确认只记录审核决定；任务恢复、应用结果和完成状态都需要后续刷新证明。"
        }
        busy={false}
        onClose={onCancelConfirmation}
        footer={
          <>
            <FoundationActionButton label="取消" variant="quiet" onClick={onCancelConfirmation} />
            <FoundationActionButton
              label={
                selectedItem?.type === "tool_permission"
                  ? "确认仅允许本次"
                  : selectedItem?.decisionContext.actionContract
                    ? approvedActionLabel(selectedItem)
                    : "确认批准"
              }
              variant="primary"
              icon={<ShieldCheck size={17} aria-hidden="true" />}
              onClick={onConfirmAction}
            />
          </>
        }
      >
        {permission && (
          <dl className="ol-permission-confirmation">
            <div>
              <dt>工具</dt>
              <dd>{permission.toolLabel}</dd>
            </div>
            <div>
              <dt>目标</dt>
              <dd>
                {permission.resolvedTargetLabel ?? permission.requestedTargetLabel ?? "目标未知"}
              </dd>
            </div>
            <div>
              <dt>用途</dt>
              <dd>{permission.purposeSummary}</dd>
            </div>
            <div>
              <dt>传输</dt>
              <dd>{permission.transmissionBoundary.summary}</dd>
            </div>
            <div>
              <dt>有效期</dt>
              <dd>{permission.expiresAt ?? "一次精确匹配或后端到期"}</dd>
            </div>
          </dl>
        )}
      </FoundationDialog>
    </div>
  );
}
