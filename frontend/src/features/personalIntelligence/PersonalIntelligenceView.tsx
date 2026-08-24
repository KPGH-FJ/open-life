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
  LifeModelDocumentV2,
  LifeModelLearningCandidate,
  LifeModelLongTermGoalV2,
  LifeModelNamedItemV2,
  LifeModelRelationshipV2,
  LifeModelSectionV2,
  LifeModelStatementV2,
  ReviewItem,
} from "@/tauri";
import { FoundationActionButton, FoundationNotice, FoundationStatusLabel } from "@/ui/foundation";
import type { PersonalIntelligenceSnapshot } from "./personalIntelligenceDataSource";
import {
  personalIntelligenceLifecyclePresentation,
  personalIntelligenceReviewItems,
} from "./personalIntelligencePresentation";
import { LifeModelBuilderPanel } from "./LifeModelBuilderPanel";
import { LegacyLifeModelMigrationPanel } from "./LegacyLifeModelMigrationPanel";
import { LifeModelV2ControlsPanel } from "./LifeModelV2ControlsPanel";

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

function memoryScopeLabel(scope: string): string {
  if (scope === "project") return "Project";
  if (scope === "conversation") return "当前对话";
  if (scope === "workspace") return "Workspace";
  return "个人";
}

type CanonicalLifeModelSectionKey =
  | "identity"
  | "values"
  | "longTermGoals"
  | "stablePreferences"
  | "personalBoundaries"
  | "importantRelationships"
  | "capabilities"
  | "resources"
  | "decisionPrinciples"
  | "collaborationPreferences";

type CanonicalLifeModelItem =
  | LifeModelStatementV2
  | LifeModelLongTermGoalV2
  | LifeModelRelationshipV2
  | LifeModelNamedItemV2;

const canonicalSectionKey: Record<LifeModelSectionV2, CanonicalLifeModelSectionKey> = {
  identity: "identity",
  values: "values",
  long_term_goals: "longTermGoals",
  stable_preferences: "stablePreferences",
  personal_boundaries: "personalBoundaries",
  important_relationships: "importantRelationships",
  capabilities: "capabilities",
  resources: "resources",
  decision_principles: "decisionPrinciples",
  collaboration_preferences: "collaborationPreferences",
};

function findCanonicalLifeModelItem(document: LifeModelDocumentV2, itemRef: string) {
  const separator = itemRef.indexOf(":");
  if (separator <= 0) return null;
  const section = itemRef.slice(0, separator) as LifeModelSectionV2;
  const itemId = itemRef.slice(separator + 1);
  const key = canonicalSectionKey[section];
  if (!key || !itemId) return null;
  const items = document[key] as CanonicalLifeModelItem[];
  const item = items.find(candidate => candidate.id === itemId);
  if (!item) return null;
  const statement =
    "statement" in item
      ? item.statement
      : "direction" in item
        ? `${item.direction}：${item.meaning}`
        : "personLabel" in item
          ? `${item.personLabel}：${item.relationship}（${item.significance}）`
          : `${item.name}：${item.description}`;
  return { itemRef, statement, sourceRefs: item.sourceRefs, confirmedAt: item.confirmedAt };
}

function applyDisabledReason(reason?: string): string {
  if (!reason || /应用命令|typed apply|frontend|backend/i.test(reason)) {
    return "这项建议已经批准，但当前还不能应用。";
  }
  return reason;
}

export function PersonalIntelligenceView({
  snapshot,
  focusedLifeModelItemRef,
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
  onRestoreMemory,
  onRollbackMemory,
  onPrivacyEraseMemory,
  lifeModelAction,
  onDraftLifeModelChange,
  onDraftLegacyLifeModelMigration,
  onDraftLifeModelRollback,
  onDraftLifeModelExport,
  learningAction,
  onConfirmLearningCandidate,
  onStageLearningCandidate,
  onDeleteLearningCandidate,
  onRejectLearningCandidate,
  onPauseLearningSuggestionClass,
}: {
  snapshot: PersonalIntelligenceSnapshot | null;
  focusedLifeModelItemRef?: string | null;
  selectedItem: ReviewItem | null;
  refreshing: boolean;
  onRefresh: () => void;
  onSelectItem: (item: ReviewItem) => void;
  onOpenReview: (item: ReviewItem) => void;
  onOpenInspector: () => void;
  onOpenReviewCenter?: () => void;
  memoryAction: {
    memoryId: string;
    action: "correct" | "archive" | "restore" | "rollback" | "erase";
    error?: string;
  } | null;
  onCorrectMemory: (memoryId: string, content: string) => Promise<boolean>;
  onArchiveMemory: (memoryId: string) => Promise<boolean>;
  onRestoreMemory: (memoryId: string) => Promise<boolean>;
  onRollbackMemory: (memoryId: string, reason: string) => Promise<boolean>;
  onPrivacyEraseMemory: (memoryId: string) => Promise<boolean>;
  lifeModelAction: {
    kind: "migration" | "change" | "rollback" | "export";
    status: "submitting" | "review_required" | "failed";
    proposalId?: string;
    error?: string;
  } | null;
  onDraftLegacyLifeModelMigration: Parameters<typeof LegacyLifeModelMigrationPanel>[0]["onDraft"];
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
      <div className="ol-intelligence-page ol-intelligence-page--centered" aria-busy="true">
        <FoundationNotice title="正在读取长期状态" tone="neutral">
          <p>LifeModel、Memory 与审核状态完成核对前，不展示应用结论。</p>
        </FoundationNotice>
      </div>
    );
  }

  const lifeModel = snapshot.lifeModelEnvelope.data;
  const memory = snapshot.memoryEnvelope.data;
  const durableItems = personalIntelligenceReviewItems(snapshot);
  const memoryItems = durableItems.filter(
    item =>
      (item.type === "memory_write" || item.type === "memory_archive") &&
      ["pending", "edited", "deferred"].includes(item.status)
  );
  const lifeModelItems = durableItems.filter(
    item => item.type !== "memory_write" && item.type !== "memory_archive"
  );
  const lifeModelItem =
    lifeModelItems.find(item => item.id === selectedItem?.id) ?? lifeModelItems[0] ?? null;
  const state = personalIntelligenceLifecyclePresentation(snapshot, lifeModelItem, "life_model");
  const memoryOwnerReady = snapshot.memoryEnvelope.status === "ready";

  if (snapshot.lifeModelEnvelope.status === "error" && snapshot.memoryEnvelope.status === "error") {
    return (
      <div className="ol-intelligence-page ol-intelligence-page--centered">
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
  const legacyMigration = lifeModel?.legacyMigrationInventory ?? null;
  const v2LifeModelAction: Parameters<typeof LifeModelV2ControlsPanel>[0]["action"] =
    lifeModelAction && lifeModelAction.kind !== "migration"
      ? {
          ...lifeModelAction,
          kind: lifeModelAction.kind,
        }
      : null;
  const focusedLifeModelItem =
    canonicalView && focusedLifeModelItemRef
      ? findCanonicalLifeModelItem(canonicalView.document, focusedLifeModelItemRef)
      : null;
  const applyAction = lifeModelItem?.allowedActions.find(action => action.kind === "apply");
  const hasEstablishedView = Boolean(canonical);
  const builderDisabledReason = (() => {
    const statuses = [snapshot.lifeModelEnvelope.status, snapshot.reviewEnvelope.status];
    if (statuses.includes("error")) return "LifeModel 或 Review Center 当前不可用。";
    if (statuses.includes("stale")) return "长期状态已陈旧；请先重新读取。";
    if (statuses.includes("loading")) {
      return "长期状态读模型尚不可用。";
    }
    if (legacyMigration) {
      return "检测到旧版 LifeModel 数据；完成逐项迁移审核前不能建立新的规范版本。";
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
    <div className="ol-intelligence-page" data-intelligence-lifecycle={state.lifecycle}>
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
          <p>每个区域只显示自己的系统读模型；不可用的来源不会借用另一侧数据补造结论。</p>
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
        {focusedLifeModelItemRef &&
          (focusedLifeModelItem ? (
            <FoundationNotice title="本次影响使用的长期信息" tone="neutral">
              <p data-lifemodel-item-ref={focusedLifeModelItem.itemRef}>
                <strong>{focusedLifeModelItem.statement}</strong>
                <br />
                确认于 {focusedLifeModelItem.confirmedAt}
              </p>
            </FoundationNotice>
          ) : (
            <FoundationNotice title="无法定位这条长期信息" tone="error" live>
              <p>这条长期信息已不存在或已更新。页面不会用旧记录代替当前内容。</p>
            </FoundationNotice>
          ))}
        {snapshot.lifeModelEnvelope.status === "error" ? (
          <FoundationNotice title="关于我暂时不可用" tone="error" live>
            <p>暂时无法读取 Life Model；Agent Memory 仍可独立查看。</p>
          </FoundationNotice>
        ) : (
          <>
            <section
              className="ol-intelligence-current"
              aria-labelledby="intelligence-current-title"
            >
              <header className="ol-intelligence-section-heading">
                <div>
                  <span>当前理解</span>
                  <h2 id="intelligence-current-title">
                    {canonicalView
                      ? canonicalView.title
                      : legacyMigration
                        ? "发现待迁移的旧版长期信息"
                        : "长期理解尚未建立"}
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
                  : legacyMigration
                    ? "旧数据仍在原位置保留，当前不会把它当作规范 LifeModel 使用，也不会静默删除或自动迁移。"
                    : "尚未建立 LifeModel。你可以从一条明确、可审核的长期信息开始。"}
              </p>
              {canonicalView ? (
                <small>
                  {canonicalView.versionLabel}
                  {canonicalView.lastMaterializedAt
                    ? ` · 确认于 ${canonicalView.lastMaterializedAt}`
                    : " · 确认时间未知"}
                </small>
              ) : null}
            </section>

            {legacyMigration ? (
              <FoundationNotice title="旧版 LifeModel 已安全保留" tone="protection" live>
                <p>
                  当前文件 {legacyMigration.currentSourceBytes} 字节；历史目录包含
                  {legacyMigration.historyYamlFileCount} 个 YAML 文件，索引记录
                  {legacyMigration.historyManifestEntryCount} 条。迁移预览识别出
                  {legacyMigration.preview?.reviewRequiredCount ?? 0} 项需要逐项确认、
                  {legacyMigration.preview?.externalOwnerCount ?? 0} 项属于其他数据域、
                  {legacyMigration.preview?.manualClassificationCount ?? 0} 项需要人工分类。
                </p>
                <p>本阶段只有只读盘点；迁移、备份、切换与删除均未执行。</p>
              </FoundationNotice>
            ) : null}

            {legacyMigration ? (
              <LegacyLifeModelMigrationPanel
                inventory={legacyMigration}
                action={lifeModelAction}
                onDraft={onDraftLegacyLifeModelMigration}
                onOpenReview={onOpenReviewCenter}
              />
            ) : null}

            <section className="ol-intelligence-current" aria-labelledby="lifemodel-learning-title">
              <header className="ol-intelligence-section-heading">
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
                          label="送去需处理"
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

            {!hasEstablishedView && !legacyMigration && onOpenReviewCenter && (
              <LifeModelBuilderPanel
                disabledReason={builderDisabledReason}
                action={v2LifeModelAction}
                onChange={onDraftLifeModelChange}
                onOpenReview={onOpenReviewCenter}
              />
            )}

            {canonicalView ? (
              <LifeModelV2ControlsPanel
                canonical={canonicalView}
                history={lifeModel?.versionHistory ?? []}
                disabledReason={builderDisabledReason}
                action={v2LifeModelAction}
                onChange={onDraftLifeModelChange}
                onRollback={onDraftLifeModelRollback}
                onExport={onDraftLifeModelExport}
                onOpenReview={onOpenReviewCenter}
              />
            ) : null}

            <section className="ol-intelligence-change" aria-labelledby="intelligence-change-title">
              <div className="ol-intelligence-section-heading">
                <div>
                  <span>建议与应用</span>
                  <h2 id="intelligence-change-title">
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
                <div
                  className="ol-intelligence-change-list"
                  role="list"
                  aria-label="LifeModel 变更"
                >
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
                  <p className="ol-intelligence-change-summary">
                    {lifeModelItem.decisionContext.summary}
                  </p>
                  <div className="ol-intelligence-diff" aria-label="当前值与建议值">
                    <div>
                      <small>当前</small>
                      <strong>
                        {lifeModelItem.decisionContext.before?.summary ?? "系统未提供当前值"}
                      </strong>
                    </div>
                    <ArrowRight size={18} aria-hidden="true" />
                    <div>
                      <small>建议</small>
                      <strong>{lifeModelItem.decisionContext.after.summary}</strong>
                    </div>
                  </div>
                  <p className="ol-intelligence-state-conclusion">{state.detail}</p>
                  <ol className="ol-intelligence-lifecycle" aria-label="变更进度">
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
                  <div className="ol-intelligence-actions">
                    {["pending", "edited", "deferred"].includes(lifeModelItem.status) && (
                      <FoundationActionButton
                        label="查看并决定"
                        icon={<ArrowRight size={17} aria-hidden="true" />}
                        variant="primary"
                        data-action-category="product"
                        data-action-id={`personal-intelligence.open-review:${lifeModelItem.id}`}
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
                        disabledReason={applyDisabledReason(applyAction.disabledReason)}
                      />
                    )}
                    <FoundationActionButton
                      label="查看详情"
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
                <div className="ol-intelligence-empty">
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
            <p>暂时无法读取 Agent Memory；Life Model 仍可独立查看。</p>
          </FoundationNotice>
        ) : (
          <section className="ol-intelligence-memory" aria-labelledby="intelligence-memory-title">
            <div className="ol-intelligence-section-heading">
              <div>
                <span>Memory</span>
                <h2 id="intelligence-memory-title">记忆</h2>
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
                <dl className="ol-intelligence-memory-summary" aria-label="记忆概览">
                  <div>
                    <dt>当前记忆</dt>
                    <dd>{memory.summary.activeMemoryCount}</dd>
                  </div>
                </dl>
                {memoryItems.length > 0 ? (
                  <section
                    className="ol-memory-suggestions"
                    aria-labelledby="memory-suggestions-title"
                  >
                    <div className="ol-memory-assets__heading">
                      <div>
                        <h3 id="memory-suggestions-title">待处理的记忆建议</h3>
                        <p>这些内容尚未写入 Agent Memory；你可以先查看来源和影响，再决定。</p>
                      </div>
                    </div>
                    <ol>
                      {memoryItems.map(item => {
                        const memoryState = personalIntelligenceLifecyclePresentation(
                          snapshot,
                          item,
                          "memory"
                        );
                        const awaitingDecision = ["pending", "edited", "deferred"].includes(
                          item.status
                        );
                        return (
                          <li key={item.id}>
                            <div>
                              <FoundationStatusLabel
                                label={memoryState.label}
                                status={memoryState.status}
                                verified={memoryState.verified}
                              />
                              <strong>
                                {item.type === "memory_archive"
                                  ? "建议忘记这条记忆"
                                  : "OpenLife 建议记住"}
                              </strong>
                            </div>
                            <p>{item.decisionContext.after.summary}</p>
                            <small>{item.decisionContext.reasonSummary}</small>
                            {awaitingDecision ? (
                              <FoundationActionButton
                                label="查看并决定"
                                icon={<ArrowRight size={17} aria-hidden="true" />}
                                variant="primary"
                                data-action-category="product"
                                data-action-id={`memory.open-review:${item.id}`}
                                data-action-kind="open"
                                data-action-enabled="true"
                                data-action-disabled-reason=""
                                data-action-target-ref={item.id}
                                onClick={() => onOpenReview(item)}
                              />
                            ) : null}
                          </li>
                        );
                      })}
                    </ol>
                  </section>
                ) : null}
                <div className="ol-memory-assets" aria-label="可管理的长期记忆">
                  <div className="ol-memory-assets__heading">
                    <div>
                      <strong>OpenLife 记住的内容</strong>
                      <p>你可以随时编辑、忘记或删除；这些内容不会改变 LifeModel。</p>
                    </div>
                  </div>
                  {memory.items.length > 0 ? (
                    memory.items.map(item => {
                      const busy = memoryAction?.memoryId === item.memoryId;
                      const editing = editingMemoryId === item.memoryId;
                      const directActionDisabled = busy || !memoryOwnerReady;
                      const directActionDisabledReason = busy
                        ? "同一条 Memory 的操作正在处理。"
                        : "Memory 状态不是最新的可用读模型；请先重新读取。";
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
                            <span>{memoryScopeLabel(item.scope)}</span>
                          </div>
                          <p className="ol-memory-asset__content">
                            {item.content ?? "该记忆的正文和来源已经永久擦除。"}
                          </p>
                          <details className="ol-memory-asset__details">
                            <summary>详情</summary>
                            <small>记住原因：{item.whyRemembered}</small>
                            <small>使用方式：{item.recallExplanation}</small>
                            {item.sourceRefs.length > 0 ? (
                              <small>
                                来源：
                                {item.sourceRefs
                                  .slice(0, 3)
                                  .map(ref => ref.label)
                                  .join(" · ")}
                              </small>
                            ) : null}
                            {item.acceptedAt ? <small>保存时间：{item.acceptedAt}</small> : null}
                          </details>
                          {editing ? (
                            <div className="ol-memory-asset__editor">
                              <label htmlFor={`memory-correction-${item.memoryId}`}>编辑记忆</label>
                              <textarea
                                id={`memory-correction-${item.memoryId}`}
                                value={memoryDraft}
                                onChange={event => setMemoryDraft(event.target.value)}
                                disabled={busy}
                              />
                              <div>
                                <FoundationActionButton
                                  label="保存"
                                  loading={busy && memoryAction?.action === "correct"}
                                  loadingLabel="正在提交"
                                  disabled={
                                    directActionDisabled ||
                                    !memoryDraft.trim() ||
                                    memoryDraft.trim() === item.content
                                  }
                                  disabledReason={
                                    directActionDisabled
                                      ? directActionDisabledReason
                                      : !memoryDraft.trim()
                                        ? "请输入纠正后的完整内容。"
                                        : "内容没有变化；无需保存。"
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
                                label="编辑"
                                icon={<SquarePen size={16} aria-hidden="true" />}
                                variant="quiet"
                                disabled={directActionDisabled}
                                disabledReason={directActionDisabledReason}
                                onClick={() => {
                                  setEditingMemoryId(item.memoryId);
                                  setMemoryDraft(item.content ?? "");
                                }}
                              />
                            ) : null}
                            {item.canArchive ? (
                              <FoundationActionButton
                                label="忘记"
                                icon={<Archive size={16} aria-hidden="true" />}
                                variant="quiet"
                                loading={busy && memoryAction?.action === "archive"}
                                loadingLabel="正在提交"
                                disabled={directActionDisabled}
                                disabledReason={directActionDisabledReason}
                                onClick={() => void onArchiveMemory(item.memoryId)}
                              />
                            ) : null}
                            {item.canRestore ? (
                              <FoundationActionButton
                                label="撤销忘记"
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
                                label="撤销最近修改"
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
                                label="永久删除"
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
                    <p className="ol-intelligence-muted">还没有保存的记忆。</p>
                  )}
                </div>
              </>
            ) : (
              <p className="ol-intelligence-muted">系统没有提供可展示的记忆概览。</p>
            )}
          </section>
        )}
      </div>
    </div>
  );
}
