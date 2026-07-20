import { useMemo, useRef, useState } from "react";
import {
  ArrowRight,
  Check,
  Clock3,
  FileSearch,
  Pencil,
  RefreshCw,
  RotateCcw,
  ShieldAlert,
  XCircle,
} from "lucide-react";
import {
  FoundationActionButton,
  FoundationDialog,
  FoundationIconButton,
  FoundationNotice,
  FoundationStatusLabel,
  FoundationTextField,
  FoundationToggle,
} from "@/ui/foundation";
import {
  OpenLifeWorkbenchShell,
  type WorkbenchContextSummary,
  type WorkbenchEvidenceReference,
  type WorkbenchInspectorModel,
} from "@/ui/shell";
import {
  fixtureActions,
  phase4cScenarios,
  productNavigation,
  settingsBoundary,
  settingsCategoryCopy,
  settingsContext,
  settingsInspector,
  settingsNavigation,
  type FixtureActionContract,
  type Phase4cScenarioId,
} from "./phase4c-fixtures";

const HARNESS_MARKER = "OPENLIFE_PHASE4C_DESKTOP_SHELL_HARNESS";

const scenarioOrder: readonly Phase4cScenarioId[] = [
  "today-ready",
  "workspace-permission",
  "tasks-unavailable",
  "review-pending",
  "review-approved",
  "life-model-limited",
  "safe-mode",
  "settings",
];

const navigationScenario: Record<string, Exclude<Phase4cScenarioId, "settings">> = {
  today: "today-ready",
  workspace: "workspace-permission",
  tasks: "tasks-unavailable",
  review: "review-pending",
  "life-model": "life-model-limited",
};

function actionAttributes(contract: FixtureActionContract) {
  return {
    "data-action-id": contract.id,
    "data-action-kind": contract.kind,
    "data-action-enabled": String(contract.enabled),
    "data-action-disabled-reason": contract.disabledReason ?? "",
    "data-action-target-ref": contract.targetRef,
    "data-action-confirmation": contract.confirmation ?? "none",
    "data-action-materialization": contract.materialization ?? "none",
  } as const;
}

export function DesktopShellHarness() {
  const [scenarioId, setScenarioId] =
    useState<Exclude<Phase4cScenarioId, "settings">>("today-ready");
  const [mode, setMode] = useState<"product" | "settings">("product");
  const [activeSettingsId, setActiveSettingsId] = useState("model-provider");
  const [settingsQuery, setSettingsQuery] = useState("");
  const [inspectorOpen, setInspectorOpen] = useState(false);
  const [focusKey, setFocusKey] = useState("initial");
  const focusSequenceRef = useRef(0);
  const [feedback, setFeedback] = useState(
    "Phase 4C 桌面壳已载入；所有内容均为布局 fixture，不连接产品后端。"
  );
  const [selectedEvidence, setSelectedEvidence] = useState("");
  const [reviewEditing, setReviewEditing] = useState(false);
  const [reviewDraft, setReviewDraft] = useState("出差前优先预留一段不被会议占用的准备时间");
  const [decisionDialog, setDecisionDialog] = useState<"approve" | "reject" | null>(null);

  const scenario = phase4cScenarios[scenarioId];
  const settingsCopy = settingsCategoryCopy[activeSettingsId];
  const context: WorkbenchContextSummary =
    mode === "settings" ? { ...settingsContext, title: settingsCopy.title } : scenario.context;

  const inspector: WorkbenchInspectorModel = useMemo(() => {
    const base = mode === "settings" ? settingsInspector : scenario.inspector;
    return selectedEvidence
      ? {
          ...base,
          evidenceFeedback: `已选择证据 ${selectedEvidence}；这是结构样例，不是真实后端回执。`,
          technicalDetails: [
            ...(base.technicalDetails ?? []),
            { label: "selected", value: selectedEvidence },
          ],
        }
      : base;
  }, [mode, scenario.inspector, selectedEvidence]);

  function requestContextFocus(prefix: string): void {
    focusSequenceRef.current += 1;
    setFocusKey(`${prefix}:${focusSequenceRef.current}`);
  }

  function announce(message: string): void {
    setFeedback(message);
  }

  function selectScenario(nextId: Phase4cScenarioId): void {
    setInspectorOpen(false);
    setSelectedEvidence("");
    setReviewEditing(false);
    if (nextId === "settings") {
      setMode("settings");
      requestContextFocus("qa-settings");
      announce("已切换到设置上下文样例；没有读取或修改真实配置。");
      return;
    }
    setMode("product");
    setScenarioId(nextId);
    requestContextFocus(`qa-${nextId}`);
    announce(`已切换到“${phase4cScenarios[nextId].label}”布局样例。`);
  }

  function navigateProduct(id: string): void {
    const nextScenario = navigationScenario[id];
    if (!nextScenario) return;
    setMode("product");
    setScenarioId(nextScenario);
    setInspectorOpen(false);
    setSelectedEvidence("");
    setReviewEditing(false);
    requestContextFocus(`nav-${id}`);
    announce(
      id === "tasks"
        ? "任务入口尚未迁移；当前显示明确的不可用状态，没有重定向。"
        : `已进入${phase4cScenarios[nextScenario].context.title}布局样例。`
    );
  }

  function openSettings(): void {
    setMode("settings");
    setInspectorOpen(false);
    setSelectedEvidence("");
    requestContextFocus("settings-open");
    announce("已进入独立设置上下文；产品主导航已替换为设置分类。");
  }

  function backFromSettings(): void {
    setMode("product");
    setSettingsQuery("");
    setInspectorOpen(false);
    announce("已返回之前的产品工作区。");
  }

  function navigateSettings(id: string): void {
    setActiveSettingsId(id);
    setInspectorOpen(false);
    setSelectedEvidence("");
    requestContextFocus(`settings-${id}`);
    announce(`已切换到设置分类“${settingsCategoryCopy[id].title}”。`);
  }

  function changeSettingsQuery(query: string): void {
    setSettingsQuery(query);
    const normalized = query.trim().toLocaleLowerCase("zh-CN");
    const resultCount = normalized
      ? settingsNavigation.filter(item =>
          `${item.label} ${item.meta ?? ""}`.toLocaleLowerCase("zh-CN").includes(normalized)
        ).length
      : settingsNavigation.length;
    announce(query ? `设置搜索更新：${resultCount} 个匹配分类。` : "设置搜索已清除。");
  }

  function openInspector(message = "已打开证据检查器。此处均为结构化布局样例。"): void {
    setInspectorOpen(true);
    announce(message);
  }

  function closeInspector(): void {
    setInspectorOpen(false);
    announce("证据检查器已关闭，焦点返回打开按钮。");
  }

  function openEvidence(evidence: WorkbenchEvidenceReference): void {
    setSelectedEvidence(evidence.id);
    announce(`已选择“${evidence.label}”；来源 ${evidence.source}，不代表真实后端证据。`);
  }

  function resetHarness(): void {
    setScenarioId("today-ready");
    setMode("product");
    setActiveSettingsId("model-provider");
    setSettingsQuery("");
    setInspectorOpen(false);
    setSelectedEvidence("");
    setReviewEditing(false);
    setDecisionDialog(null);
    requestContextFocus("reset");
    announce("桌面壳样例已重置；没有写入任何产品数据。");
  }

  function confirmReviewDecision(): void {
    setSelectedEvidence("");
    if (decisionDialog === "approve") {
      setScenarioId("review-approved");
      requestContextFocus("review-approved");
      announce("样例批准决定已记录；尚未应用，也没有写入长期状态。");
    } else if (decisionDialog === "reject") {
      announce("样例建议已拒绝；没有发生应用、长期写入或外部传输。");
    }
    setDecisionDialog(null);
  }

  return (
    <div
      className="ol-foundation phase4c-harness"
      data-harness-marker={HARNESS_MARKER}
      data-foundation-dialog-background
    >
      <header className="phase4c-qa-toolbar" aria-label="Phase 4C QA 工具栏">
        <div className="phase4c-qa-identity">
          <strong>Phase 4C</strong>
          <span>桌面 Shell · DEV ONLY</span>
        </div>
        <label className="phase4c-fixture-select">
          <span>布局状态</span>
          <select
            value={mode === "settings" ? "settings" : scenarioId}
            onChange={event => selectScenario(event.target.value as Phase4cScenarioId)}
          >
            {scenarioOrder.map(id => (
              <option key={id} value={id}>
                {id === "settings" ? "设置：独立上下文" : phase4cScenarios[id].label}
              </option>
            ))}
          </select>
        </label>
        <div className="phase4c-qa-feedback" aria-hidden="true" title={feedback}>
          {feedback}
        </div>
        <div className="phase4c-qa-boundaries" aria-label="样例边界">
          <FoundationStatusLabel label="DESKTOP_TAURI" />
          <FoundationStatusLabel label="LAYOUT_FIXTURE" />
          <FoundationStatusLabel label="未连接后端" status="unknown" />
        </div>
        <FoundationIconButton
          label="重置桌面壳样例"
          icon={<RotateCcw size={18} strokeWidth={1.75} aria-hidden="true" />}
          onClick={resetHarness}
        />
      </header>

      <div className="phase4c-shell-stage">
        <OpenLifeWorkbenchShell
          mode={mode}
          activeNavigationId={scenario.activeNavigationId}
          navigationItems={productNavigation}
          onNavigate={navigateProduct}
          activeSettingsId={activeSettingsId}
          settingsItems={settingsNavigation}
          settingsQuery={settingsQuery}
          onSettingsQueryChange={changeSettingsQuery}
          onSettingsNavigate={navigateSettings}
          onOpenSettings={openSettings}
          onBackFromSettings={backFromSettings}
          boundary={mode === "settings" ? settingsBoundary : scenario.boundary}
          context={context}
          focusKey={focusKey}
          inspectorOpen={inspectorOpen}
          inspector={inspector}
          onOpenInspector={() => openInspector()}
          onCloseInspector={closeInspector}
          onOpenEvidence={openEvidence}
          announcement={feedback}
        >
          {mode === "settings" ? (
            <SettingsSurface
              activeSettingsId={activeSettingsId}
              onOpenInspector={() => openInspector("已打开配置与真实边界的证据检查器。")}
            />
          ) : (
            <ProductFixtureSurface
              scenarioId={scenarioId}
              reviewEditing={reviewEditing}
              reviewDraft={reviewDraft}
              onReviewDraftChange={setReviewDraft}
              onReviewEditingChange={setReviewEditing}
              onNavigate={navigateProduct}
              onOpenInspector={openInspector}
              onAnnounce={announce}
              onOpenDecision={setDecisionDialog}
            />
          )}
        </OpenLifeWorkbenchShell>
      </div>

      <FoundationDialog
        open={decisionDialog !== null}
        title={decisionDialog === "reject" ? "确认拒绝这条建议" : "确认批准这条建议"}
        description="这是 Phase 4C 交互 fixture；确认只改变当前页面样例。"
        onClose={() => setDecisionDialog(null)}
        footer={
          <>
            <FoundationActionButton
              label="取消"
              variant="secondary"
              onClick={() => setDecisionDialog(null)}
            />
            <FoundationActionButton
              label={decisionDialog === "reject" ? "确认拒绝" : "确认批准"}
              variant={decisionDialog === "reject" ? "danger" : "primary"}
              {...actionAttributes(
                decisionDialog === "reject"
                  ? fixtureActions.rejectReview
                  : fixtureActions.approveReview
              )}
              onClick={confirmReviewDecision}
            />
          </>
        }
      >
        <FoundationNotice title="决定不等于应用" tone="protection">
          批准只记录决定。只有后续应用命令完成，并且刷新后的读模型返回 applied，才可显示完成。
        </FoundationNotice>
      </FoundationDialog>
    </div>
  );
}

function ProductFixtureSurface({
  scenarioId,
  reviewEditing,
  reviewDraft,
  onReviewDraftChange,
  onReviewEditingChange,
  onNavigate,
  onOpenInspector,
  onAnnounce,
  onOpenDecision,
}: {
  scenarioId: Exclude<Phase4cScenarioId, "settings">;
  reviewEditing: boolean;
  reviewDraft: string;
  onReviewDraftChange: (value: string) => void;
  onReviewEditingChange: (editing: boolean) => void;
  onNavigate: (id: string) => void;
  onOpenInspector: (message?: string) => void;
  onAnnounce: (message: string) => void;
  onOpenDecision: (decision: "approve" | "reject") => void;
}) {
  switch (scenarioId) {
    case "today-ready":
      return (
        <FixturePage eyebrow="今天先完成什么" title="给下一季度留出清晰的起点">
          <p className="phase4c-lead">
            上午完成季度回顾，下午整理旅行报销材料。关于出差准备习惯的一条长期记忆建议仍等待你决定。
          </p>
          <section className="phase4c-section" aria-labelledby="today-focus-title">
            <div className="phase4c-section-heading">
              <div>
                <span>当前目标</span>
                <h3 id="today-focus-title">今日重点</h3>
              </div>
              <span>来自今日计划摘要</span>
            </div>
            <ol className="phase4c-focus-list">
              <li>
                <span>09:30</span>
                <div>
                  <strong>完成季度回顾初稿</strong>
                  <p>整理三项结果和下一季度的两个调整。</p>
                </div>
              </li>
              <li>
                <span>14:00</span>
                <div>
                  <strong>核对旅行报销材料</strong>
                  <p>任务可以继续本地整理，外部摘要仍等待确认。</p>
                </div>
              </li>
            </ol>
          </section>
          <section
            className="phase4c-section phase4c-attention"
            aria-labelledby="today-review-title"
          >
            <div>
              <span>需要你的决定</span>
              <h3 id="today-review-title">一条记忆建议尚未审核</h3>
              <p>查看只进入待决策页面，不会批准、应用或写入长期状态。</p>
            </div>
            <FoundationActionButton
              label="查看待审核建议"
              variant="primary"
              icon={<ArrowRight size={18} strokeWidth={1.75} aria-hidden="true" />}
              {...actionAttributes(fixtureActions.openPendingReview)}
              onClick={() => onNavigate("review")}
            />
          </section>
        </FixturePage>
      );

    case "workspace-permission":
      return (
        <FixturePage eyebrow="当前任务正在做什么" title="整理旅行报销材料">
          <p className="phase4c-lead">
            已在本地识别发票、行程单和付款记录。任务暂停在生成外部摘要之前，等待你确认访问范围。
          </p>
          <FoundationNotice title="任务暂停在一个明确动作之前" tone="protection">
            尚未发送摘要，也没有把权限等待描述成任务完成。先查看工具、目标、数据范围与有效期。
          </FoundationNotice>
          <section className="phase4c-section" aria-labelledby="workspace-progress-title">
            <div className="phase4c-section-heading">
              <div>
                <span>执行进度</span>
                <h3 id="workspace-progress-title">最近步骤</h3>
              </div>
              <span>原始事件与运行标识已移入证据检查器</span>
            </div>
            <ol className="phase4c-timeline">
              <li data-state="done">
                <Check size={17} strokeWidth={1.75} aria-hidden="true" />
                <div>
                  <strong>材料已归类</strong>
                  <span>6 份发票、2 份行程单、1 条付款记录</span>
                </div>
              </li>
              <li data-state="waiting">
                <Clock3 size={17} strokeWidth={1.75} aria-hidden="true" />
                <div>
                  <strong>等待确认外部摘要范围</strong>
                  <span>任务保持暂停；确认前不发送</span>
                </div>
              </li>
            </ol>
          </section>
          <section className="phase4c-product-actions" aria-label="当前可用动作">
            <div>
              <span>下一步</span>
              <strong>先确认本次访问范围</strong>
            </div>
            <div className="phase4c-action-row">
              <FoundationActionButton
                label="查看访问范围"
                variant="primary"
                icon={<FileSearch size={18} strokeWidth={1.75} aria-hidden="true" />}
                {...actionAttributes(fixtureActions.openPermissionScope)}
                onClick={() => onOpenInspector("已打开本次权限范围；没有记录授权决定。")}
              />
              <FoundationActionButton
                label="继续任务"
                disabled
                disabledReason={fixtureActions.continueWorkspace.disabledReason ?? undefined}
                {...actionAttributes(fixtureActions.continueWorkspace)}
              />
            </div>
          </section>
        </FixturePage>
      );

    case "tasks-unavailable":
      return (
        <FixturePage eyebrow="哪些任务可以继续" title="任务页面尚未迁移">
          <p className="phase4c-lead">
            任务仍是正式一级入口，但当前桌面版本尚未连接任务队列。这里不会伪造历史记录或恢复能力。
          </p>
          <FoundationNotice title="当前入口明确不可用" tone="neutral">
            任务队列、重试、取消和恢复需要后端读模型与真实操作结果；当前保持不可用。
          </FoundationNotice>
          <section className="phase4c-product-actions" aria-label="可用替代动作">
            <div>
              <span>可用替代</span>
              <strong>返回当前工作区</strong>
            </div>
            <FoundationActionButton
              label="返回工作区"
              variant="primary"
              {...actionAttributes(fixtureActions.returnToWorkspace)}
              onClick={() => onNavigate("workspace")}
            />
          </section>
        </FixturePage>
      );

    case "review-pending":
      return (
        <FixturePage eyebrow="建议改变什么" title="出差前保留准备时间">
          <p className="phase4c-lead">
            系统从最近三次行程复盘中发现，你在出发前保留一段无会议时间时更从容。它建议把这条偏好加入长期记忆。
          </p>
          <section className="phase4c-section" aria-labelledby="review-diff-title">
            <div className="phase4c-section-heading">
              <div>
                <span>当前 → 建议</span>
                <h3 id="review-diff-title">变更对比</h3>
              </div>
              <button
                type="button"
                className="phase4c-text-action"
                {...actionAttributes(fixtureActions.openReviewEvidence)}
                onClick={() => onOpenInspector("已打开建议来源与影响；决定状态没有改变。")}
              >
                查看来源与影响
              </button>
            </div>
            <div className="phase4c-diff">
              <div>
                <span>当前</span>
                <p>尚无关于出差准备时间的长期偏好。</p>
              </div>
              <div>
                <span>建议</span>
                <p>{reviewDraft}</p>
              </div>
            </div>
          </section>
          {reviewEditing && (
            <section className="phase4c-inline-editor" aria-label="修改建议">
              <FoundationTextField
                id="phase4c-review-draft"
                label="建议内容"
                value={reviewDraft}
                onChange={event => onReviewDraftChange(event.target.value)}
                description="只修改当前页面 fixture，不写入后端。"
              />
              <FoundationActionButton
                label="保存修改"
                variant="secondary"
                {...actionAttributes(fixtureActions.saveReviewEdit)}
                onClick={() => {
                  onReviewEditingChange(false);
                  onAnnounce("样例建议已修改；仍处于等待决定状态，没有写入长期记忆。");
                }}
              />
            </section>
          )}
          <section className="phase4c-decision-bar" aria-label="审核决定">
            <FoundationActionButton
              label="稍后处理"
              variant="quiet"
              icon={<Clock3 size={18} strokeWidth={1.75} aria-hidden="true" />}
              {...actionAttributes(fixtureActions.deferReview)}
              onClick={() => onAnnounce("已保留为待审核样例；当前决定仍未完成。")}
            />
            <FoundationActionButton
              label="修改建议"
              variant="secondary"
              icon={<Pencil size={18} strokeWidth={1.75} aria-hidden="true" />}
              {...actionAttributes(fixtureActions.editReview)}
              onClick={() => {
                onReviewEditingChange(true);
                onAnnounce("已打开建议修改区域；尚未作出审核决定。");
              }}
            />
            <FoundationActionButton
              label="拒绝"
              variant="danger"
              icon={<XCircle size={18} strokeWidth={1.75} aria-hidden="true" />}
              {...actionAttributes(fixtureActions.rejectReview)}
              onClick={() => onOpenDecision("reject")}
            />
            <FoundationActionButton
              label="批准变更"
              variant="primary"
              icon={<Check size={18} strokeWidth={1.75} aria-hidden="true" />}
              {...actionAttributes(fixtureActions.approveReview)}
              onClick={() => onOpenDecision("approve")}
            />
          </section>
        </FixturePage>
      );

    case "review-approved":
      return (
        <FixturePage eyebrow="决定已记录" title="已批准，尚未应用">
          <p className="phase4c-lead">
            这条偏好的审核决定已经记录，但当前没有应用回执，也没有刷新后的长期状态证据。
          </p>
          <FoundationNotice title="批准不等于已写入" tone="protection">
            只有后续应用完成，并由刷新后的长期状态读模型确认，页面才能显示完成。
          </FoundationNotice>
          <section className="phase4c-section" aria-labelledby="approved-state-title">
            <div className="phase4c-section-heading">
              <div>
                <span>状态对照</span>
                <h3 id="approved-state-title">决定与应用分离</h3>
              </div>
            </div>
            <dl className="phase4c-state-table">
              <div>
                <dt>审核决定</dt>
                <dd>已批准</dd>
              </div>
              <div>
                <dt>应用状态</dt>
                <dd>尚未应用</dd>
              </div>
              <div>
                <dt>长期状态</dt>
                <dd>未知，保持关闭</dd>
              </div>
            </dl>
          </section>
          <section className="phase4c-product-actions" aria-label="应用状态动作">
            <div>
              <span>下一步</span>
              <strong>等待应用并刷新读模型</strong>
            </div>
            <div className="phase4c-action-row">
              <FoundationActionButton
                label="刷新应用状态"
                variant="primary"
                icon={<RefreshCw size={18} strokeWidth={1.75} aria-hidden="true" />}
                {...actionAttributes(fixtureActions.refreshApplication)}
                onClick={() =>
                  onAnnounce("样例刷新完成：状态仍为已批准、尚未应用；没有伪造 applied。")
                }
              />
              <FoundationActionButton
                label="应用变更"
                disabled
                disabledReason={fixtureActions.applyChange.disabledReason ?? undefined}
                {...actionAttributes(fixtureActions.applyChange)}
              />
            </div>
          </section>
        </FixturePage>
      );

    case "life-model-limited":
      return (
        <FixturePage eyebrow="当前有来源的长期理解" title="LifeModel 当前兼容受限">
          <p className="phase4c-lead">
            当前只展示一组有来源的长期理解摘要，不能在本页直接改写长期事实。
          </p>
          <section className="phase4c-section" aria-labelledby="life-summary-title">
            <div className="phase4c-section-heading">
              <div>
                <span>有来源摘要</span>
                <h3 id="life-summary-title">工作与节奏</h3>
              </div>
              <FoundationStatusLabel label="兼容受限" status="unknown" />
            </div>
            <div className="phase4c-reading-block">
              <p>你倾向在上午完成需要连续专注的写作，并把行政整理安排在下午。</p>
              <span>来源、时间和应用状态应由后端长期状态摘要提供。</span>
            </div>
          </section>
          <section className="phase4c-product-actions" aria-label="LifeModel 可用动作">
            <div>
              <span>可用动作</span>
              <strong>查看来源与限制</strong>
            </div>
            <FoundationActionButton
              label="查看依据"
              variant="primary"
              {...actionAttributes(fixtureActions.openLifeModelEvidence)}
              onClick={() => onOpenInspector("已打开 LifeModel 来源与限制样例。")}
            />
          </section>
        </FixturePage>
      );

    case "safe-mode":
      return (
        <FixturePage eyebrow="保护状态" title="可以查看，外部动作保持关闭">
          <p className="phase4c-lead">
            供应商与隐私边界证据缺失或陈旧。安全模式允许继续本地查看和整理，但不会自动外传或写入长期状态。
          </p>
          <FoundationNotice title="安全模式正在保护当前状态" tone="protection">
            安全模式不是错误；只有具体错误或危险动作被阻断时才进入错误状态。
          </FoundationNotice>
          <section className="phase4c-product-actions" aria-label="安全模式动作">
            <div>
              <span>下一步</span>
              <strong>先查看缺失证据</strong>
            </div>
            <div className="phase4c-action-row">
              <FoundationActionButton
                label="查看边界证据"
                variant="primary"
                icon={<ShieldAlert size={18} strokeWidth={1.75} aria-hidden="true" />}
                {...actionAttributes(fixtureActions.openSafeModeEvidence)}
                onClick={() => onOpenInspector("已打开安全模式依据；外部动作仍保持关闭。")}
              />
              <FoundationActionButton
                label="执行外部动作"
                disabled
                disabledReason={fixtureActions.safeModeExternalAction.disabledReason ?? undefined}
                {...actionAttributes(fixtureActions.safeModeExternalAction)}
              />
            </div>
          </section>
        </FixturePage>
      );
  }
}

function SettingsSurface({
  activeSettingsId,
  onOpenInspector,
}: {
  activeSettingsId: string;
  onOpenInspector: () => void;
}) {
  const copy = settingsCategoryCopy[activeSettingsId];

  if (activeSettingsId !== "model-provider") {
    return (
      <FixturePage eyebrow="设置分类" title={copy.title}>
        <p className="phase4c-lead">{copy.description}</p>
        <FoundationNotice title="此分类当前不可用" tone="neutral">
          当前桌面版本尚未连接该分类的读取与保存能力，不会显示或修改真实配置。
        </FoundationNotice>
      </FixturePage>
    );
  }

  return (
    <FixturePage eyebrow="当前配置与真实边界分开" title="模型与供应商">
      <p className="phase4c-lead">
        配置控件描述用户想使用什么；真实路由和是否外传只能由后端边界摘要证明。
      </p>
      <FoundationNotice title="当前传输边界未知" tone="protection">
        当前缺少后端边界结果，因此不显示“本地处理”，测试和保存动作也保持关闭。
      </FoundationNotice>
      <section className="phase4c-settings-form" aria-labelledby="provider-config-title">
        <div className="phase4c-section-heading">
          <div>
            <span>配置草稿</span>
            <h3 id="provider-config-title">供应商连接</h3>
          </div>
          <button
            type="button"
            className="phase4c-text-action"
            {...actionAttributes(fixtureActions.openSettingsBoundary)}
            onClick={onOpenInspector}
          >
            查看真实边界说明
          </button>
        </div>
        <FoundationTextField
          id="phase4c-provider-name"
          label="供应商"
          defaultValue="未连接"
          disabled
          disabledReason="当前页面尚未接入设置编辑状态。"
        />
        <FoundationTextField
          id="phase4c-provider-endpoint"
          label="服务地址"
          defaultValue=""
          placeholder="尚未配置"
          disabled
          disabledReason="未连接真实配置，不能推导当前路由。"
        />
        <FoundationToggle
          label="当前外部传输状态"
          description="未知不能显示为关闭，也不能显示为本地。"
          state="unknown"
        />
      </section>
      <section className="phase4c-product-actions" aria-label="供应商设置动作">
        <div>
          <span>动作边界</span>
          <strong>测试、保存和边界刷新相互独立</strong>
        </div>
        <div className="phase4c-action-row">
          <FoundationActionButton
            label="测试连接"
            disabled
            disabledReason={fixtureActions.testProvider.disabledReason ?? undefined}
            {...actionAttributes(fixtureActions.testProvider)}
          />
          <FoundationActionButton
            label="保存设置"
            disabled
            disabledReason={fixtureActions.saveProvider.disabledReason ?? undefined}
            {...actionAttributes(fixtureActions.saveProvider)}
          />
        </div>
      </section>
    </FixturePage>
  );
}

function FixturePage({
  eyebrow,
  title,
  children,
}: {
  eyebrow: string;
  title: string;
  children: React.ReactNode;
}) {
  return (
    <article className="phase4c-page">
      <header className="phase4c-page-heading">
        <span>{eyebrow}</span>
        <h2>{title}</h2>
      </header>
      {children}
    </article>
  );
}
