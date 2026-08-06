import { useState } from "react";
import { Archive, ArrowRight, Eye, RefreshCw, RotateCcw, ShieldX, SquarePen } from "lucide-react";
import type { ReviewItem } from "@/tauri";
import { FoundationActionButton, FoundationNotice, FoundationStatusLabel } from "@/ui/foundation";
import type { DurableTruthSnapshot } from "./durableTruthDataSource";
import { durableLifecyclePresentation, durableReviewItems } from "./durableTruthPresentation";
import { LifeModelBuilderPanel } from "./LifeModelBuilderPanel";
import type { LifeModelBuilderController } from "./useLifeModelBuilder";

const dimensionOrder = ["identity", "goals", "capabilities", "state"];

export function DurableTruthView({
  snapshot,
  selectedItem,
  refreshing,
  onRefresh,
  onSelectItem,
  onOpenReview,
  onOpenInspector,
  builder,
  onOpenReviewCenter,
  memoryAction,
  onCorrectMemory,
  onArchiveMemory,
  onStopRecall,
  onRestoreMemory,
  onRollbackMemory,
  onPrivacyEraseMemory,
}: {
  snapshot: DurableTruthSnapshot | null;
  selectedItem: ReviewItem | null;
  refreshing: boolean;
  onRefresh: () => void;
  onSelectItem: (item: ReviewItem) => void;
  onOpenReview: (item: ReviewItem) => void;
  onOpenInspector: () => void;
  builder?: LifeModelBuilderController;
  onOpenReviewCenter?: () => void;
  memoryAction: {
    memoryId: string;
    action: "correct" | "stop_recall" | "archive" | "restore" | "rollback" | "erase";
    error?: string;
  } | null;
  onCorrectMemory: (memoryId: string, content: string) => Promise<boolean>;
  onArchiveMemory: (memoryId: string) => Promise<boolean>;
  onStopRecall: (memoryId: string) => Promise<boolean>;
  onRestoreMemory: (memoryId: string) => Promise<boolean>;
  onRollbackMemory: (memoryId: string, reason: string) => Promise<boolean>;
  onPrivacyEraseMemory: (memoryId: string) => Promise<boolean>;
}) {
  const [editingMemoryId, setEditingMemoryId] = useState<string | null>(null);
  const [memoryDraft, setMemoryDraft] = useState("");
  if (!snapshot || snapshot.lifeModelEnvelope.status === "loading") {
    return (
      <div className="ol-durable-page ol-durable-page--centered" aria-busy="true">
        <FoundationNotice title="正在读取长期状态" tone="neutral">
          <p>LifeModel、Memory 与审核状态完成核对前，不展示应用结论。</p>
        </FoundationNotice>
      </div>
    );
  }

  const state = durableLifecyclePresentation(snapshot, selectedItem);
  const lifeModel =
    snapshot.lifeModelEnvelope.status === "ready" || snapshot.lifeModelEnvelope.status === "stale"
      ? snapshot.lifeModelEnvelope.data
      : null;
  const memory =
    snapshot.memoryEnvelope.status === "ready" || snapshot.memoryEnvelope.status === "stale"
      ? snapshot.memoryEnvelope.data
      : null;
  const durableItems = durableReviewItems(snapshot);

  if (
    snapshot.lifeModelEnvelope.status === "error" ||
    snapshot.memoryEnvelope.status === "error" ||
    snapshot.reviewEnvelope.status === "error"
  ) {
    return (
      <div className="ol-durable-page ol-durable-page--centered">
        <FoundationNotice title="长期状态暂时不可用" tone="error" live>
          <p>至少一个后端读模型读取失败；页面没有从旧页面或原始存储拼出替代结论。</p>
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

  const currentView = lifeModel?.currentViewSummary;
  const canonical = lifeModel?.canonicalSummary;
  const sortedDimensions = [...(lifeModel?.dimensionSummaries ?? [])].sort(
    (left, right) => dimensionOrder.indexOf(left.id) - dimensionOrder.indexOf(right.id)
  );
  const applyAction = selectedItem?.allowedActions.find(action => action.kind === "apply");
  const hasEstablishedView = Boolean(
    currentView || canonical || (lifeModel?.dimensionSummaries.length ?? 0) > 0
  );
  const builderDisabledReason = (() => {
    const statuses = [
      snapshot.lifeModelEnvelope.status,
      snapshot.memoryEnvelope.status,
      snapshot.reviewEnvelope.status,
    ];
    if (statuses.includes("stale")) return "长期状态已陈旧；请先重新读取。";
    if (statuses.includes("loading")) {
      return "长期状态读模型尚不可用。";
    }
    return undefined;
  })();

  return (
    <div className="ol-durable-page" data-durable-lifecycle={state.lifecycle}>
      {(snapshot.lifeModelEnvelope.status === "stale" ||
        snapshot.memoryEnvelope.status === "stale" ||
        snapshot.reviewEnvelope.status === "stale") && (
        <FoundationNotice title="长期状态已陈旧" tone="protection" live>
          <p>刷新成功前只允许查看来源；审核决定、应用和回滚结论全部保持关闭。</p>
        </FoundationNotice>
      )}

      <section className="ol-durable-current" aria-labelledby="durable-current-title">
        <header className="ol-durable-section-heading">
          <div>
            <span>当前理解</span>
            <h2 id="durable-current-title">
              {currentView?.label ?? canonical?.title ?? "长期理解尚未建立"}
            </h2>
          </div>
          <FoundationStatusLabel
            label={
              lifeModel?.truthMode === "canonical"
                ? "规范状态"
                : lifeModel?.truthMode === "current_compatibility"
                  ? "兼容视图"
                  : "来源受限"
            }
            status={lifeModel?.truthMode === "canonical" ? "neutral" : "unknown"}
          />
        </header>
        <p>
          {currentView?.summary ??
            canonical?.summary ??
            "后端没有提供可展示的当前或规范摘要；页面不会从旧 LifeModel 对象补造内容。"}
        </p>
        {sortedDimensions.length > 0 && (
          <dl className="ol-durable-dimensions">
            {sortedDimensions.map(dimension => (
              <div key={dimension.id} data-stale={String(dimension.stale)}>
                <dt>{dimension.label}</dt>
                <dd>{dimension.summary}</dd>
                <small>
                  {dimension.stale ? "已陈旧" : `可信度 ${dimension.confidence}`} · 来源
                  {dimension.provenance === "limited" ? "受限" : "未知"}
                </small>
              </div>
            ))}
          </dl>
        )}
      </section>

      {!hasEstablishedView && builder && onOpenReviewCenter && (
        <LifeModelBuilderPanel
          controller={builder}
          disabledReason={builderDisabledReason}
          onOpenReview={onOpenReviewCenter}
        />
      )}

      <section className="ol-durable-change" aria-labelledby="durable-change-title">
        <div className="ol-durable-section-heading">
          <div>
            <span>建议与应用</span>
            <h2 id="durable-change-title">
              {selectedItem?.decisionContext.title ?? "当前没有长期状态建议"}
            </h2>
          </div>
          <FoundationStatusLabel
            label={state.label}
            status={state.status}
            verified={state.verified}
            live
          />
        </div>

        {durableItems.length > 1 && (
          <div className="ol-durable-change-list" role="list" aria-label="长期状态变更">
            {durableItems.map(item => (
              <button
                key={item.id}
                type="button"
                role="listitem"
                aria-pressed={selectedItem?.id === item.id}
                onClick={() => onSelectItem(item)}
              >
                {item.decisionContext.title}
              </button>
            ))}
          </div>
        )}

        {selectedItem ? (
          <>
            <p className="ol-durable-change-summary">{selectedItem.decisionContext.summary}</p>
            <div className="ol-durable-diff" aria-label="当前值与建议值">
              <div>
                <small>当前</small>
                <strong>
                  {selectedItem.decisionContext.before?.summary ?? "后端未提供当前值"}
                </strong>
              </div>
              <ArrowRight size={18} aria-hidden="true" />
              <div>
                <small>建议</small>
                <strong>{selectedItem.decisionContext.after.summary}</strong>
              </div>
            </div>
            <p className="ol-durable-state-conclusion">{state.detail}</p>
            <ol className="ol-durable-lifecycle" aria-label="变更进度">
              <li data-state={selectedItem.status === "approved" ? "complete" : "current"}>
                <span>1</span>
                <div>
                  <strong>决定</strong>
                  <small>
                    {selectedItem.status === "approved"
                      ? "已批准"
                      : selectedItem.status === "rejected"
                        ? "已拒绝"
                        : selectedItem.status === "deferred"
                          ? "稍后处理"
                          : "等待决定"}
                  </small>
                </div>
              </li>
              <li
                data-state={
                  state.lifecycle === "applying"
                    ? "current"
                    : state.lifecycle === "applied" || state.lifecycle === "rolled_back"
                      ? "complete"
                      : state.lifecycle === "failed"
                        ? "failed"
                        : "pending"
                }
              >
                <span>2</span>
                <div>
                  <strong>应用</strong>
                  <small>{state.label}</small>
                </div>
              </li>
              <li
                data-state={
                  state.lifecycle === "applied"
                    ? "complete"
                    : state.lifecycle === "rolled_back"
                      ? "current"
                      : "pending"
                }
              >
                <span>3</span>
                <div>
                  <strong>长期状态</strong>
                  <small>
                    {state.lifecycle === "applied"
                      ? "读模型已确认"
                      : state.lifecycle === "rolled_back"
                        ? "已恢复此前状态"
                        : "尚未确认变更"}
                  </small>
                </div>
              </li>
            </ol>
            <div className="ol-durable-actions">
              {["pending", "edited", "deferred"].includes(selectedItem.status) && (
                <FoundationActionButton
                  label="查看并决定"
                  icon={<ArrowRight size={17} aria-hidden="true" />}
                  variant="primary"
                  data-action-category="product"
                  data-action-id={`durable.open-review:${selectedItem.id}`}
                  data-action-kind="open"
                  data-action-enabled="true"
                  data-action-disabled-reason=""
                  data-action-target-ref={selectedItem.id}
                  onClick={() => onOpenReview(selectedItem)}
                />
              )}
              {applyAction && (
                <FoundationActionButton
                  label="应用变更"
                  data-action-category="review"
                  data-action-id={applyAction.id}
                  data-action-kind={applyAction.kind}
                  data-action-effect={applyAction.effect}
                  data-action-enabled={String(applyAction.enabled)}
                  data-action-disabled-reason={applyAction.disabledReason ?? ""}
                  data-action-target-ref={applyAction.targetReviewItemId}
                  disabled
                  disabledReason={
                    applyAction.disabledReason ||
                    "当前前端没有可调用的 typed apply command；保持只读。"
                  }
                />
              )}
              <FoundationActionButton
                label="查看状态依据"
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
                onClick={onRefresh}
              />
            </div>
          </>
        ) : (
          <div className="ol-durable-empty">
            <p>{state.detail}</p>
            <FoundationActionButton
              label="重新读取"
              icon={<RefreshCw size={17} aria-hidden="true" />}
              variant="quiet"
              loading={refreshing}
              loadingLabel="正在读取"
              onClick={onRefresh}
            />
          </div>
        )}
      </section>

      <section className="ol-durable-memory" aria-labelledby="durable-memory-title">
        <div className="ol-durable-section-heading">
          <div>
            <span>Memory</span>
            <h2 id="durable-memory-title">长期记忆概览</h2>
          </div>
          <FoundationStatusLabel
            label={snapshot.memoryEnvelope.status === "ready" ? "已读取" : "来源受限"}
            status={snapshot.memoryEnvelope.status === "ready" ? "neutral" : "unknown"}
          />
        </div>
        {memory ? (
          <>
            <dl className="ol-durable-memory-summary">
              <div>
                <dt>当前记忆</dt>
                <dd>{memory.summary.activeMemoryCount}</dd>
              </div>
              <div>
                <dt>待决定</dt>
                <dd>{memory.summary.reviewRequiredCount}</dd>
              </div>
              <div>
                <dt>待应用</dt>
                <dd>{memory.summary.pendingMaterializationCount}</dd>
              </div>
              <div>
                <dt>应用失败</dt>
                <dd>{memory.summary.failedMaterializationCount}</dd>
              </div>
            </dl>
            <div className="ol-durable-lanes" role="list" aria-label="记忆分层">
              {memory.laneSummaries.map(lane => (
                <div key={lane.lane} role="listitem">
                  <div>
                    <strong>{lane.label}</strong>
                    <small>{lane.activeCount} 条当前记录</small>
                  </div>
                  <span>
                    {lane.pendingReviewCount > 0
                      ? `${lane.pendingReviewCount} 待决定`
                      : lane.rolledBackCount > 0
                        ? `${lane.rolledBackCount} 已回滚`
                        : `${lane.materializedCount} 已应用`}
                  </span>
                </div>
              ))}
            </div>
            <div className="ol-memory-assets" aria-label="可管理的长期记忆">
              <div className="ol-memory-assets__heading">
                <div>
                  <strong>记忆内容与控制</strong>
                  <p>纠正和停止召回先进入 Review；恢复立即生效；永久擦除需要原生确认。</p>
                </div>
              </div>
              {memory.items.length > 0 ? (
                memory.items.map(item => {
                  const busy = memoryAction?.memoryId === item.memoryId;
                  const editing = editingMemoryId === item.memoryId;
                  return (
                    <article className="ol-memory-asset" key={item.memoryId}>
                      <div className="ol-memory-asset__meta">
                        <FoundationStatusLabel
                          label={
                            item.recallState === "active"
                              ? "正在召回"
                              : item.recallState === "paused"
                                ? "已停止召回"
                                : item.recallState === "archived"
                                  ? "已归档"
                                  : item.recallState === "erased"
                                    ? "正文已擦除"
                                    : "历史记录"
                          }
                          status={item.recallState === "active" ? "neutral" : "unknown"}
                        />
                        <span>{item.scope}</span>
                        <span>{item.category}</span>
                      </div>
                      <p className="ol-memory-asset__content">
                        {item.content ?? "该记忆的正文和来源已经永久擦除。"}
                      </p>
                      <small>为什么记住：{item.whyRemembered}</small>
                      {item.evidenceIds.length > 0 ? (
                        <small>来源：{item.evidenceIds.slice(0, 3).join(" · ")}</small>
                      ) : null}
                      {editing ? (
                        <div className="ol-memory-asset__editor">
                          <label htmlFor={`memory-correction-${item.memoryId}`}>
                            纠正后的完整内容
                          </label>
                          <textarea
                            id={`memory-correction-${item.memoryId}`}
                            value={memoryDraft}
                            onChange={event => setMemoryDraft(event.target.value)}
                            disabled={busy}
                          />
                          <div>
                            <FoundationActionButton
                              label="提交 Review"
                              loading={busy && memoryAction?.action === "correct"}
                              loadingLabel="正在提交"
                              disabled={!memoryDraft.trim() || memoryDraft.trim() === item.content}
                              onClick={() =>
                                void onCorrectMemory(item.memoryId, memoryDraft).then(ok => {
                                  if (ok) setEditingMemoryId(null);
                                })
                              }
                            />
                            <FoundationActionButton
                              label="取消"
                              variant="quiet"
                              disabled={busy}
                              onClick={() => setEditingMemoryId(null)}
                            />
                          </div>
                        </div>
                      ) : null}
                      {memoryAction?.memoryId === item.memoryId && memoryAction.error ? (
                        <FoundationNotice title="Memory 操作未完成" tone="error" live>
                          <p>{memoryAction.error}</p>
                        </FoundationNotice>
                      ) : null}
                      <div className="ol-memory-asset__actions">
                        {item.canCorrect ? (
                          <FoundationActionButton
                            label="纠正"
                            icon={<SquarePen size={16} aria-hidden="true" />}
                            variant="quiet"
                            disabled={busy}
                            onClick={() => {
                              setEditingMemoryId(item.memoryId);
                              setMemoryDraft(item.content ?? "");
                            }}
                          />
                        ) : null}
                        {item.canStopRecall ? (
                          <FoundationActionButton
                            label="停止召回"
                            icon={<Archive size={16} aria-hidden="true" />}
                            variant="quiet"
                            loading={busy && memoryAction?.action === "stop_recall"}
                            loadingLabel="正在提交"
                            disabled={busy}
                            onClick={() => void onStopRecall(item.memoryId)}
                          />
                        ) : null}
                        {item.canArchive ? (
                          <FoundationActionButton
                            label="归档"
                            icon={<Archive size={16} aria-hidden="true" />}
                            variant="quiet"
                            loading={busy && memoryAction?.action === "archive"}
                            loadingLabel="正在提交"
                            disabled={busy}
                            onClick={() => void onArchiveMemory(item.memoryId)}
                          />
                        ) : null}
                        {item.canRestore ? (
                          <FoundationActionButton
                            label="恢复召回"
                            icon={<RotateCcw size={16} aria-hidden="true" />}
                            variant="quiet"
                            loading={busy && memoryAction?.action === "restore"}
                            loadingLabel="正在恢复"
                            disabled={busy}
                            onClick={() => void onRestoreMemory(item.memoryId)}
                          />
                        ) : null}
                        {item.canRollback ? (
                          <FoundationActionButton
                            label="回滚这次变更"
                            icon={<RotateCcw size={16} aria-hidden="true" />}
                            variant="quiet"
                            loading={busy && memoryAction?.action === "rollback"}
                            loadingLabel="正在回滚"
                            disabled={busy}
                            onClick={() =>
                              void onRollbackMemory(
                                item.memoryId,
                                "user_requested_product_memory_rollback"
                              )
                            }
                          />
                        ) : null}
                        {item.canPrivacyErase ? (
                          <FoundationActionButton
                            label="永久擦除"
                            icon={<ShieldX size={16} aria-hidden="true" />}
                            variant="quiet"
                            loading={busy && memoryAction?.action === "erase"}
                            loadingLabel="等待确认"
                            disabled={busy}
                            onClick={() => void onPrivacyEraseMemory(item.memoryId)}
                          />
                        ) : null}
                      </div>
                    </article>
                  );
                })
              ) : (
                <p className="ol-durable-muted">还没有可管理的跨会话 Memory。</p>
              )}
            </div>
          </>
        ) : (
          <p className="ol-durable-muted">后端没有提供可展示的记忆概览。</p>
        )}
      </section>
    </div>
  );
}
