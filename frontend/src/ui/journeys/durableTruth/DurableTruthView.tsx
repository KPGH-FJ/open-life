import { useRef, useState, type KeyboardEvent } from "react";
import {
  Archive,
  ArrowRight,
  Brain,
  CircleCheck,
  CirclePause,
  Eye,
  RefreshCw,
  RotateCcw,
  ShieldX,
  SquarePen,
  Trash2,
  UserRound,
} from "lucide-react";
import type {
  LegacyLifeModelMigrationItemV2,
  LifeModelLearningCandidate,
  ReviewItem,
} from "@/tauri";
import { FoundationActionButton, FoundationNotice, FoundationStatusLabel } from "@/ui/foundation";
import type { DurableTruthSnapshot } from "./durableTruthDataSource";
import { durableLifecyclePresentation, durableReviewItems } from "./durableTruthPresentation";
import { LifeModelBuilderPanel } from "./LifeModelBuilderPanel";
import { LifeModelMigrationPanel } from "./LifeModelMigrationPanel";
import { LifeModelV2ControlsPanel } from "./LifeModelV2ControlsPanel";

const migrationDispositionLabel: Record<LegacyLifeModelMigrationItemV2["disposition"], string> = {
  review_required: "需要你审核",
  external_owner: "属于其他区域",
  manual_classification: "需要人工判断",
  not_migrated: "不会迁移",
  migration_metadata: "迁移元数据",
};

const migrationOwnerLabel: Record<LegacyLifeModelMigrationItemV2["targetOwner"], string> = {
  life_model_v2: "LifeModel v2",
  state_store: "当前状态",
  tasks: "任务",
  agent_memory: "Agent 记忆",
  agent_runtime: "Agent 工具能力",
  migration_metadata: "迁移记录",
  legacy_compatibility_projection: "旧兼容投影",
  unassigned: "尚未确定",
};

const learningSectionLabel: Partial<Record<LifeModelLearningCandidate["section"], string>> = {
  stable_preferences: "长期稳定偏好",
  collaboration_preferences: "长期协作偏好",
};

const learningStatusLabel: Record<LifeModelLearningCandidate["status"], string> = {
  accumulating: "正在累计证据",
  reviewable: "已具备审核条件",
  conflicted: "存在冲突，不能提案",
  proposed: "已进入审核",
  rejected: "已拒绝",
  materialized: "已写入确认版本",
  expired: "已过期",
};

const learningSourceLabel: Record<LifeModelLearningCandidate["sourceKinds"][number], string> = {
  explicit_user_message: "用户明确表达",
  task_outcome: "已完成任务",
  agent_reflection: "任务复盘",
  user_feedback: "用户反馈",
  user_correction: "用户纠正",
  model_extraction: "模型辅助提取",
};

export function DurableTruthView({
  snapshot,
  selectedItem,
  refreshing,
  onRefresh,
  onSelectItem,
  onOpenReview,
  onOpenInspector,
  onOpenReviewCenter,
  memoryAction,
  onCorrectMemory,
  onArchiveMemory,
  onStopRecall,
  onRestoreMemory,
  onRollbackMemory,
  onPrivacyEraseMemory,
  migrationAction,
  onDraftLegacyMigration,
  lifeModelAction,
  onDraftLifeModelChange,
  onDraftLifeModelRollback,
  onDraftLifeModelExport,
  learningAction,
  onConfirmLearningCandidate,
  onStageLearningCandidate,
  onDeleteLearningCandidate,
  onRejectLearningCandidate,
  onPauseLearningSuggestionClass,
}: {
  snapshot: DurableTruthSnapshot | null;
  selectedItem: ReviewItem | null;
  refreshing: boolean;
  onRefresh: () => void;
  onSelectItem: (item: ReviewItem) => void;
  onOpenReview: (item: ReviewItem) => void;
  onOpenInspector: () => void;
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
  migrationAction: {
    status: "submitting" | "review_required" | "failed";
    proposalId?: string;
    error?: string;
  } | null;
  onDraftLegacyMigration: Parameters<typeof LifeModelMigrationPanel>[0]["onSubmit"];
  lifeModelAction: Parameters<typeof LifeModelV2ControlsPanel>[0]["action"];
  onDraftLifeModelChange: Parameters<typeof LifeModelV2ControlsPanel>[0]["onChange"];
  onDraftLifeModelRollback: Parameters<typeof LifeModelV2ControlsPanel>[0]["onRollback"];
  onDraftLifeModelExport: Parameters<typeof LifeModelV2ControlsPanel>[0]["onExport"];
  learningAction: {
    candidateId: string;
    kind: "confirm" | "stage" | "delete" | "reject" | "pause_class";
    error?: string;
  } | null;
  onConfirmLearningCandidate: (candidateId: string) => Promise<boolean>;
  onStageLearningCandidate: (candidateId: string) => Promise<boolean>;
  onDeleteLearningCandidate: (candidateId: string) => Promise<boolean>;
  onRejectLearningCandidate: (candidateId: string) => Promise<boolean>;
  onPauseLearningSuggestionClass: (candidateId: string) => Promise<boolean>;
}) {
  const [activeDomain, setActiveDomain] = useState<"life_model" | "agent_memory">("life_model");
  const lifeModelTabRef = useRef<HTMLButtonElement>(null);
  const agentMemoryTabRef = useRef<HTMLButtonElement>(null);
  const [editingMemoryId, setEditingMemoryId] = useState<string | null>(null);
  const [memoryDraft, setMemoryDraft] = useState("");
  const moveDomainFocus = (event: KeyboardEvent<HTMLButtonElement>) => {
    if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
    event.preventDefault();
    const next = event.key === "ArrowLeft" || event.key === "Home" ? "life_model" : "agent_memory";
    setActiveDomain(next);
    (next === "life_model" ? lifeModelTabRef : agentMemoryTabRef).current?.focus();
  };
  if (!snapshot || snapshot.lifeModelEnvelope.status === "loading") {
    return (
      <div className="ol-durable-page ol-durable-page--centered" aria-busy="true">
        <FoundationNotice title="正在读取长期状态" tone="neutral">
          <p>LifeModel、Memory 与审核状态完成核对前，不展示应用结论。</p>
        </FoundationNotice>
      </div>
    );
  }

  const lifeModel = snapshot.lifeModelEnvelope.data;
  const memory = snapshot.memoryEnvelope.data;
  const durableItems = durableReviewItems(snapshot);
  const lifeModelItems = durableItems.filter(
    item => item.type !== "memory_write" && item.type !== "memory_archive"
  );
  const lifeModelItem =
    lifeModelItems.find(item => item.id === selectedItem?.id) ?? lifeModelItems[0] ?? null;
  const state = durableLifecyclePresentation(snapshot, lifeModelItem);
  const memoryOwnerReady = snapshot.memoryEnvelope.status === "ready";
  const reviewOwnerReady = snapshot.reviewEnvelope.status === "ready";

  if (snapshot.lifeModelEnvelope.status === "error" && snapshot.memoryEnvelope.status === "error") {
    return (
      <div className="ol-durable-page ol-durable-page--centered">
        <FoundationNotice title="个人智能暂时不可用" tone="error" live>
          <p>LifeModel 与 Agent Memory 均读取失败；页面没有从旧页面或原始存储拼出替代结论。</p>
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

  const canonical = lifeModel?.canonicalSummary;
  const canonicalView = lifeModel?.truthMode === "canonical" ? (canonical ?? null) : null;
  const migrationPreview = canonicalView ? null : lifeModel?.legacyMigrationPreview;
  const applyAction = lifeModelItem?.allowedActions.find(action => action.kind === "apply");
  const hasEstablishedView = Boolean(canonical || migrationPreview);
  const builderDisabledReason = (() => {
    const statuses = [snapshot.lifeModelEnvelope.status, snapshot.reviewEnvelope.status];
    if (statuses.includes("error")) return "LifeModel 或 Review Center 当前不可用。";
    if (statuses.includes("stale")) return "长期状态已陈旧；请先重新读取。";
    if (statuses.includes("loading")) {
      return "长期状态读模型尚不可用。";
    }
    if (canonical?.conflictStatus && canonical.conflictStatus !== "none") {
      return "当前规范版本存在冲突；请先重新读取并解决冲突。";
    }
    if (canonical?.freshnessStatus && canonical.freshnessStatus !== "current") {
      return "当前规范版本不是最新状态；请先重新读取。";
    }
    return undefined;
  })();

  return (
    <div className="ol-durable-page" data-durable-lifecycle={state.lifecycle}>
      <header className="ol-intelligence-header">
        <span>Personal Agent OS</span>
        <h2>个人智能的两个组成部分</h2>
        <p>
          LifeModel 描述长期稳定的你；Agent Memory 保存完成工作所需的经历、偏好与做事方式。
          两者互相协作，但各自拥有独立来源和控制边界。
        </p>
      </header>

      <div className="ol-intelligence-tabs" role="tablist" aria-label="个人智能区域">
        <button
          ref={lifeModelTabRef}
          type="button"
          id="intelligence-tab-life-model"
          role="tab"
          aria-selected={activeDomain === "life_model"}
          aria-controls="intelligence-panel-life-model"
          tabIndex={activeDomain === "life_model" ? 0 : -1}
          onKeyDown={moveDomainFocus}
          onClick={() => setActiveDomain("life_model")}
        >
          <UserRound size={18} aria-hidden="true" />
          <span>
            <strong>关于我</strong>
            <small>LifeModel · 长期个人模型</small>
          </span>
        </button>
        <button
          ref={agentMemoryTabRef}
          type="button"
          id="intelligence-tab-agent-memory"
          role="tab"
          aria-selected={activeDomain === "agent_memory"}
          aria-controls="intelligence-panel-agent-memory"
          tabIndex={activeDomain === "agent_memory" ? 0 : -1}
          onKeyDown={moveDomainFocus}
          onClick={() => setActiveDomain("agent_memory")}
        >
          <Brain size={18} aria-hidden="true" />
          <span>
            <strong>Agent 记忆</strong>
            <small>工作连续性 · 不等于 LifeModel</small>
          </span>
        </button>
      </div>

      {(snapshot.lifeModelEnvelope.status === "error" ||
        snapshot.memoryEnvelope.status === "error" ||
        snapshot.reviewEnvelope.status === "error") && (
        <FoundationNotice title="部分来源暂时不可用" tone="error" live>
          <p>每个区域只显示自己的后端读模型；不可用的来源不会借用另一侧数据补造结论。</p>
        </FoundationNotice>
      )}

      {(snapshot.lifeModelEnvelope.status === "stale" ||
        snapshot.memoryEnvelope.status === "stale" ||
        snapshot.reviewEnvelope.status === "stale") && (
        <FoundationNotice title="长期状态已陈旧" tone="protection" live>
          <p>
            陈旧来源对应的区域只允许查看；依赖该来源的决定和写入保持关闭，另一侧仍按自己的读模型状态工作。
          </p>
        </FoundationNotice>
      )}

      <div
        id="intelligence-panel-life-model"
        className="ol-intelligence-domain"
        role="tabpanel"
        aria-labelledby="intelligence-tab-life-model"
        hidden={activeDomain !== "life_model"}
      >
        {snapshot.lifeModelEnvelope.status === "error" ? (
          <FoundationNotice title="关于我暂时不可用" tone="error" live>
            <p>LifeModelViewModel 读取失败；Agent Memory 仍可在相邻区域独立查看。</p>
          </FoundationNotice>
        ) : (
          <>
            <section className="ol-durable-current" aria-labelledby="durable-current-title">
              <header className="ol-durable-section-heading">
                <div>
                  <span>当前理解</span>
                  <h2 id="durable-current-title">
                    {canonicalView ? canonicalView.title : "长期理解尚未建立"}
                  </h2>
                </div>
                <FoundationStatusLabel
                  label={lifeModel?.truthMode === "canonical" ? "规范状态" : "来源受限"}
                  status={lifeModel?.truthMode === "canonical" ? "neutral" : "unknown"}
                />
              </header>
              <p>
                {canonicalView
                  ? canonicalView.summary
                  : "尚未建立规范 LifeModel；旧 YAML 只会出现在下方迁移预览中。"}
              </p>
              {canonicalView ? (
                <>
                  <small>
                    {canonicalView.versionLabel}
                    {canonicalView.lastMaterializedAt
                      ? ` · 确认于 ${canonicalView.lastMaterializedAt}`
                      : " · 确认时间未知"}
                  </small>
                  <details className="ol-lifemodel-yaml">
                    <summary>查看 YAML 人类视图</summary>
                    <p>
                      这是由当前规范版本确定性生成的只读表达。SQLite 中的版本化 JSON
                      才是权威；此处不能直接编辑或保存。
                    </p>
                    <dl>
                      <div>
                        <dt>版本</dt>
                        <dd>{canonicalView.humanProjection.modelVersion}</dd>
                      </div>
                      <div>
                        <dt>内容数量</dt>
                        <dd>{canonicalView.humanProjection.itemCount}</dd>
                      </div>
                      <div>
                        <dt>父版本</dt>
                        <dd>{canonicalView.parentVersion ?? "首次版本"}</dd>
                      </div>
                      <div>
                        <dt>最小来源</dt>
                        <dd>{canonicalView.evidenceRefs.length} 条</dd>
                      </div>
                      <div>
                        <dt>文档摘要</dt>
                        <dd>{canonicalView.documentDigest}</dd>
                      </div>
                      <div>
                        <dt>投影摘要</dt>
                        <dd>{canonicalView.humanProjection.projectionDigest}</dd>
                      </div>
                    </dl>
                    <pre aria-label="LifeModel YAML 人类视图">
                      <code>{canonicalView.humanProjection.yaml}</code>
                    </pre>
                  </details>
                </>
              ) : null}
            </section>

            <section className="ol-durable-current" aria-labelledby="lifemodel-learning-title">
              <header className="ol-durable-section-heading">
                <div>
                  <span>学习缓冲区</span>
                  <h2 id="lifemodel-learning-title">待验证的长期信息</h2>
                </div>
                <FoundationStatusLabel
                  label={
                    lifeModel?.learning.available
                      ? `${lifeModel.learning.activeCount} 条候选`
                      : "暂时不可用"
                  }
                  status={lifeModel?.learning.available ? "neutral" : "unknown"}
                />
              </header>
              <p>
                这里只显示最近五条候选。确认“这条符合我”后，你可以逐条送去 Review Center；
                审核通过并成功应用前，它仍没有写入 LifeModel。
              </p>
              {!lifeModel?.learning.available ? (
                <FoundationNotice title="学习候选暂时不可用" tone="protection">
                  <p>这不会影响普通 Agent、Agent Memory 或当前 LifeModel 的读取和使用。</p>
                </FoundationNotice>
              ) : lifeModel.learning.candidates.length === 0 ? (
                <p>目前没有待验证的长期信息。</p>
              ) : (
                <ol className="ol-lifemodel-migration-details" aria-label="待验证长期信息">
                  {lifeModel.learning.candidates.map(candidate => (
                    <li key={candidate.id}>
                      <div>
                        <strong>{candidate.summary}</strong>
                        <span>{learningSectionLabel[candidate.section] ?? candidate.section}</span>
                      </div>
                      <small>
                        {learningStatusLabel[candidate.status]} · {candidate.supportCount} 条证据 /{" "}
                        {candidate.independentSupportCount} 个独立来源 · 到期时间：
                        {candidate.expiresAt}
                      </small>
                      <small>
                        来源：
                        {[...new Set(candidate.sourceKinds)]
                          .map(source => learningSourceLabel[source])
                          .join("、") || "未知"}
                        {candidate.oppositionCount > 0
                          ? ` · ${candidate.oppositionCount} 条反向证据`
                          : ""}
                      </small>
                      {candidate.status !== "conflicted" && !candidate.confirmedAt ? (
                        <FoundationActionButton
                          label="这条符合我"
                          icon={<CircleCheck size={16} aria-hidden="true" />}
                          loading={
                            learningAction?.candidateId === candidate.id &&
                            learningAction.kind === "confirm" &&
                            !learningAction.error
                          }
                          loadingLabel="正在记录"
                          onClick={() => void onConfirmLearningCandidate(candidate.id)}
                        />
                      ) : null}
                      {candidate.status === "reviewable" && candidate.confirmedAt ? (
                        <FoundationActionButton
                          label="送去审核中心"
                          icon={<ArrowRight size={16} aria-hidden="true" />}
                          loading={
                            learningAction?.candidateId === candidate.id &&
                            learningAction.kind === "stage" &&
                            !learningAction.error
                          }
                          loadingLabel="正在创建审核项"
                          onClick={() => void onStageLearningCandidate(candidate.id)}
                        />
                      ) : null}
                      <FoundationActionButton
                        label="删除这条候选"
                        icon={<Trash2 size={16} aria-hidden="true" />}
                        loading={
                          learningAction?.candidateId === candidate.id &&
                          learningAction.kind === "delete" &&
                          !learningAction.error
                        }
                        loadingLabel="正在删除"
                        onClick={() => void onDeleteLearningCandidate(candidate.id)}
                      />
                      <FoundationActionButton
                        label="拒绝并不再建议类似内容"
                        icon={<ShieldX size={16} aria-hidden="true" />}
                        loading={
                          learningAction?.candidateId === candidate.id &&
                          learningAction.kind === "reject" &&
                          !learningAction.error
                        }
                        loadingLabel="正在拒绝"
                        onClick={() => void onRejectLearningCandidate(candidate.id)}
                      />
                      <FoundationActionButton
                        label="暂停这类建议"
                        icon={<CirclePause size={16} aria-hidden="true" />}
                        loading={
                          learningAction?.candidateId === candidate.id &&
                          learningAction.kind === "pause_class" &&
                          !learningAction.error
                        }
                        loadingLabel="正在暂停"
                        onClick={() => void onPauseLearningSuggestionClass(candidate.id)}
                      />
                      {learningAction?.candidateId === candidate.id && learningAction.error ? (
                        <FoundationNotice
                          title={
                            learningAction.kind === "confirm"
                              ? "反馈未记录"
                              : learningAction.kind === "stage"
                                ? "未进入审核"
                                : learningAction.kind === "delete"
                                  ? "候选未删除"
                                  : learningAction.kind === "reject"
                                    ? "候选未拒绝"
                                    : "这类建议未暂停"
                          }
                          tone="error"
                          live
                        >
                          <p>{learningAction.error}</p>
                        </FoundationNotice>
                      ) : null}
                    </li>
                  ))}
                </ol>
              )}
            </section>

            {migrationPreview ? (
              <section
                className="ol-lifemodel-migration"
                aria-labelledby="lifemodel-migration-title"
              >
                <header className="ol-durable-section-heading">
                  <div>
                    <span>旧模型整理</span>
                    <h2 id="lifemodel-migration-title">迁移前预览</h2>
                  </div>
                  <FoundationStatusLabel label="只读 · 尚未迁移" status="unknown" />
                </header>
                <p>
                  这是旧 YAML 中实际存在字段的归属清单。可映射内容仍需你确认；这份预览没有创建
                  LifeModel v2、建议或任何持久化变更。
                </p>
                <dl className="ol-lifemodel-migration-counts" aria-label="迁移字段分类统计">
                  <div>
                    <dt>需要审核</dt>
                    <dd>{migrationPreview.reviewRequiredCount}</dd>
                  </div>
                  <div>
                    <dt>其他区域</dt>
                    <dd>{migrationPreview.externalOwnerCount}</dd>
                  </div>
                  <div>
                    <dt>人工判断</dt>
                    <dd>{migrationPreview.manualClassificationCount}</dd>
                  </div>
                  <div>
                    <dt>不会迁移</dt>
                    <dd>{migrationPreview.notMigratedCount}</dd>
                  </div>
                </dl>
                {migrationPreview.containsSensitiveItems ? (
                  <FoundationNotice title="包含敏感个人信息" tone="protection">
                    <p>关系、健康或个人边界等内容不会自动进入新模型，必须由你重新确认。</p>
                  </FoundationNotice>
                ) : null}
                <details className="ol-lifemodel-migration-details">
                  <summary>查看全部 {migrationPreview.items.length} 个来源字段</summary>
                  <ol>
                    {migrationPreview.items.map(item => (
                      <li key={item.sourcePath}>
                        <div>
                          <code>{item.sourcePath}</code>
                          {item.sensitive ? <span>敏感</span> : null}
                        </div>
                        <p>{item.valuePreview || "空值"}</p>
                        <small>
                          {migrationDispositionLabel[item.disposition]} · 目标：
                          {migrationOwnerLabel[item.targetOwner]}
                          {item.valueTruncated ? " · 仅显示摘要" : ""}
                        </small>
                      </li>
                    ))}
                  </ol>
                </details>
                <small className="ol-lifemodel-migration-digest">
                  来源摘要：{migrationPreview.sourceDigest}
                </small>
                <LifeModelMigrationPanel
                  preview={migrationPreview}
                  submitting={migrationAction?.status === "submitting"}
                  proposalId={migrationAction?.proposalId}
                  error={migrationAction?.error}
                  onSubmit={onDraftLegacyMigration}
                  onOpenReview={onOpenReviewCenter}
                />
              </section>
            ) : null}

            {!hasEstablishedView && onOpenReviewCenter && (
              <LifeModelBuilderPanel
                disabledReason={builderDisabledReason}
                action={lifeModelAction}
                onChange={onDraftLifeModelChange}
                onOpenReview={onOpenReviewCenter}
              />
            )}

            {canonicalView ? (
              <LifeModelV2ControlsPanel
                canonical={canonicalView}
                history={lifeModel?.versionHistory ?? []}
                disabledReason={builderDisabledReason}
                action={lifeModelAction}
                onChange={onDraftLifeModelChange}
                onRollback={onDraftLifeModelRollback}
                onExport={onDraftLifeModelExport}
                onOpenReview={onOpenReviewCenter}
              />
            ) : null}

            <section className="ol-durable-change" aria-labelledby="durable-change-title">
              <div className="ol-durable-section-heading">
                <div>
                  <span>建议与应用</span>
                  <h2 id="durable-change-title">
                    {lifeModelItem?.decisionContext.title ?? "当前没有 LifeModel 建议"}
                  </h2>
                </div>
                <FoundationStatusLabel
                  label={state.label}
                  status={state.status}
                  verified={state.verified}
                  live
                />
              </div>

              {lifeModelItems.length > 1 && (
                <div className="ol-durable-change-list" role="list" aria-label="LifeModel 变更">
                  {lifeModelItems.map(item => (
                    <button
                      key={item.id}
                      type="button"
                      role="listitem"
                      aria-pressed={lifeModelItem?.id === item.id}
                      onClick={() => onSelectItem(item)}
                    >
                      {item.decisionContext.title}
                    </button>
                  ))}
                </div>
              )}

              {lifeModelItem ? (
                <>
                  <p className="ol-durable-change-summary">
                    {lifeModelItem.decisionContext.summary}
                  </p>
                  <div className="ol-durable-diff" aria-label="当前值与建议值">
                    <div>
                      <small>当前</small>
                      <strong>
                        {lifeModelItem.decisionContext.before?.summary ?? "后端未提供当前值"}
                      </strong>
                    </div>
                    <ArrowRight size={18} aria-hidden="true" />
                    <div>
                      <small>建议</small>
                      <strong>{lifeModelItem.decisionContext.after.summary}</strong>
                    </div>
                  </div>
                  <p className="ol-durable-state-conclusion">{state.detail}</p>
                  <ol className="ol-durable-lifecycle" aria-label="变更进度">
                    <li data-state={lifeModelItem.status === "approved" ? "complete" : "current"}>
                      <span>1</span>
                      <div>
                        <strong>决定</strong>
                        <small>
                          {lifeModelItem.status === "approved"
                            ? "已批准"
                            : lifeModelItem.status === "rejected"
                              ? "已拒绝"
                              : lifeModelItem.status === "deferred"
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
                    {["pending", "edited", "deferred"].includes(lifeModelItem.status) && (
                      <FoundationActionButton
                        label="查看并决定"
                        icon={<ArrowRight size={17} aria-hidden="true" />}
                        variant="primary"
                        data-action-category="product"
                        data-action-id={`durable.open-review:${lifeModelItem.id}`}
                        data-action-kind="open"
                        data-action-enabled="true"
                        data-action-disabled-reason=""
                        data-action-target-ref={lifeModelItem.id}
                        onClick={() => onOpenReview(lifeModelItem)}
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
          </>
        )}
      </div>

      <div
        id="intelligence-panel-agent-memory"
        className="ol-intelligence-domain"
        role="tabpanel"
        aria-labelledby="intelligence-tab-agent-memory"
        hidden={activeDomain !== "agent_memory"}
      >
        {snapshot.memoryEnvelope.status === "error" ? (
          <FoundationNotice title="Agent 记忆暂时不可用" tone="error" live>
            <p>MemoryViewModel 读取失败；LifeModel 仍可在相邻区域独立查看。</p>
          </FoundationNotice>
        ) : (
          <section className="ol-durable-memory" aria-labelledby="durable-memory-title">
            <div className="ol-durable-section-heading">
              <div>
                <span>Memory</span>
                <h2 id="durable-memory-title">Agent 记忆</h2>
              </div>
              <FoundationStatusLabel
                label={
                  snapshot.memoryEnvelope.status === "ready"
                    ? "已读取"
                    : snapshot.memoryEnvelope.status === "empty"
                      ? "还没有记忆"
                      : "来源受限"
                }
                status={
                  snapshot.memoryEnvelope.status === "ready" ||
                  snapshot.memoryEnvelope.status === "empty"
                    ? "neutral"
                    : "unknown"
                }
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
                      <p>
                        这里管理 Agent 如何延续工作，不修改“关于我”。纠正、停止召回和归档先进入
                        Review；永久擦除需要原生确认。
                      </p>
                    </div>
                  </div>
                  {memory.items.length > 0 ? (
                    memory.items.map(item => {
                      const busy = memoryAction?.memoryId === item.memoryId;
                      const editing = editingMemoryId === item.memoryId;
                      const directActionDisabled = busy || !memoryOwnerReady;
                      const reviewedActionDisabled = directActionDisabled || !reviewOwnerReady;
                      const directActionDisabledReason = busy
                        ? "同一条 Memory 的操作正在处理。"
                        : "Memory 状态不是最新的可用读模型；请先重新读取。";
                      const reviewedActionDisabledReason = !memoryOwnerReady
                        ? directActionDisabledReason
                        : !reviewOwnerReady
                          ? "Review Center 状态不可用；不能创建无法核对的审核建议。"
                          : directActionDisabledReason;
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
                          <small>如何召回：{item.recallExplanation}</small>
                          {item.sourceRefs.length > 0 ? (
                            <small>
                              来源：
                              {item.sourceRefs
                                .slice(0, 3)
                                .map(ref => ref.label)
                                .join(" · ")}
                            </small>
                          ) : null}
                          {item.acceptedAt ? <small>用户确认时间：{item.acceptedAt}</small> : null}
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
                                  disabled={
                                    reviewedActionDisabled ||
                                    !memoryDraft.trim() ||
                                    memoryDraft.trim() === item.content
                                  }
                                  disabledReason={
                                    reviewedActionDisabled
                                      ? reviewedActionDisabledReason
                                      : !memoryDraft.trim()
                                        ? "请输入纠正后的完整内容。"
                                        : "内容没有变化；无需创建新的 Review。"
                                  }
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
                                  disabledReason="当前 Memory 操作完成前不能关闭编辑器。"
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
                                disabled={reviewedActionDisabled}
                                disabledReason={reviewedActionDisabledReason}
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
                                disabled={reviewedActionDisabled}
                                disabledReason={reviewedActionDisabledReason}
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
                                disabled={reviewedActionDisabled}
                                disabledReason={reviewedActionDisabledReason}
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
                                disabled={directActionDisabled}
                                disabledReason={directActionDisabledReason}
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
                                disabled={directActionDisabled}
                                disabledReason={directActionDisabledReason}
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
                                disabled={directActionDisabled}
                                disabledReason={directActionDisabledReason}
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
        )}
      </div>
    </div>
  );
}
