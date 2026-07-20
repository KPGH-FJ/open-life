import {
  Bot,
  Brain,
  DatabaseBackup,
  Eye,
  KeyRound,
  LayoutList,
  LockKeyhole,
  MessageSquareText,
  Palette,
  Settings2,
  ShieldCheck,
  SlidersHorizontal,
  Sparkles,
  SquareCheckBig,
  Wrench,
} from "lucide-react";
import type {
  WorkbenchBoundarySummary,
  WorkbenchContextSummary,
  WorkbenchInspectorModel,
  WorkbenchNavigationItem,
} from "@/ui/shell";

export type Phase4cScenarioId =
  | "today-ready"
  | "workspace-permission"
  | "tasks-unavailable"
  | "review-pending"
  | "review-approved"
  | "life-model-limited"
  | "safe-mode"
  | "settings";

export interface Phase4cScenario {
  id: Exclude<Phase4cScenarioId, "settings">;
  label: string;
  activeNavigationId: string;
  context: WorkbenchContextSummary;
  boundary: WorkbenchBoundarySummary;
  inspector: WorkbenchInspectorModel;
}

export type FixtureActionKind = "product" | "review" | "debug";

export interface FixtureActionContract {
  id: string;
  kind: FixtureActionKind;
  enabled: boolean;
  disabledReason: string | null;
  targetRef: string;
  confirmation?: "none" | "required";
  materialization?: "none" | "decision_only_refresh_required";
}

export const productNavigation: readonly WorkbenchNavigationItem[] = [
  { id: "today", label: "今日", meta: "每日关注", icon: Sparkles },
  { id: "workspace", label: "工作区", meta: "当前执行", icon: MessageSquareText },
  { id: "tasks", label: "任务", meta: "队列与连续性", icon: LayoutList },
  { id: "review", label: "审核中心", meta: "建议与权限", icon: SquareCheckBig, badge: "1" },
  { id: "life-model", label: "LifeModel", meta: "长期理解", icon: Brain },
] as const;

export const settingsNavigation: readonly WorkbenchNavigationItem[] = [
  { id: "model-provider", label: "模型与供应商", meta: "路由配置", icon: Bot },
  { id: "privacy-network", label: "隐私与网络", meta: "传输边界", icon: ShieldCheck },
  { id: "tools-permissions", label: "工具与权限", meta: "能力范围", icon: KeyRound },
  { id: "data-recovery", label: "数据与恢复", meta: "导出与快照", icon: DatabaseBackup },
  { id: "life-memory", label: "LifeModel 与记忆", meta: "长期状态", icon: Brain },
  { id: "appearance", label: "外观", meta: "桌面显示", icon: Palette },
  { id: "advanced-support", label: "高级与支持", meta: "诊断信息", icon: Wrench },
] as const;

const unknownBoundary: WorkbenchBoundarySummary = {
  label: "传输边界待确认",
  detail: "当前没有来自后端的确定传输边界结论。",
  status: "unknown",
};

export const phase4cScenarios: Record<Exclude<Phase4cScenarioId, "settings">, Phase4cScenario> = {
  "today-ready": {
    id: "today-ready",
    label: "今日：可用 + 待审核",
    activeNavigationId: "today",
    context: {
      eyebrow: "今天",
      title: "今日工作台",
      status: { label: "1 项等待决定", status: "waiting" },
    },
    boundary: unknownBoundary,
    inspector: {
      title: "今日计划依据",
      conclusion: "今天的计划可以查看和整理，其中一条记忆建议仍等待你的决定。",
      risk: "该建议尚未批准，也没有写入 LifeModel 或长期记忆。",
      nextAction: "先查看建议差异，再决定批准、修改、拒绝或稍后处理。",
      evidence: [
        {
          id: "evidence_today_focus_fixture",
          label: "今日关注点样例",
          source: "layout_fixture.today.focusList",
          sensitivity: "local_private",
        },
        {
          id: "evidence_pending_review_fixture",
          label: "待审核建议样例",
          source: "TodayViewModel.pendingReviewCount + reviewCenterLink",
          sensitivity: "personal_context",
        },
      ],
      technicalDetails: [
        { label: "fixture", value: "phase4c.today.ready_pending_review" },
        { label: "backend", value: "not_connected" },
      ],
    },
  },
  "workspace-permission": {
    id: "workspace-permission",
    label: "工作区：等待授权",
    activeNavigationId: "workspace",
    context: {
      eyebrow: "当前执行",
      title: "整理旅行报销材料",
      status: { label: "等待你的确认", status: "waiting" },
    },
    boundary: {
      label: "可能发生外部传输",
      detail: "是否发送、发送到哪里以及数据范围仍需证据确认。",
      status: "unknown",
    },
    inspector: {
      title: "本次访问范围",
      conclusion: "任务暂停在发送请求之前；当前只完成了本地材料整理。",
      risk: "继续可能把报销摘要发送到外部模型，目标与字段范围必须先确认。",
      nextAction: "核对工具、目标、数据范围和有效期，再决定是否仅允许本次。",
      evidence: [
        {
          id: "evidence_permission_scope_fixture",
          label: "权限范围样例",
          source: "WorkspaceViewModel.pendingReviewItems[].decisionContext.permission",
          sensitivity: "financial_private",
        },
        {
          id: "evidence_task_checkpoint_fixture",
          label: "任务检查点样例",
          source: "WorkspaceViewModel.activeTask.evidenceRefs",
          sensitivity: "local_metadata",
        },
      ],
      technicalDetails: [
        { label: "targetRef", value: "task:fixture-travel-expense" },
        { label: "scope", value: "fixture.permission.scope.unverified" },
      ],
    },
  },
  "tasks-unavailable": {
    id: "tasks-unavailable",
    label: "任务：未迁移",
    activeNavigationId: "tasks",
    context: {
      eyebrow: "任务连续性",
      title: "任务",
      status: { label: "新页面尚未接入", status: "neutral" },
    },
    boundary: unknownBoundary,
    inspector: {
      title: "任务入口状态",
      conclusion: "任务仍保留为一级信息架构入口，但当前桌面壳尚未接入任务页面。",
      risk: "当前不能从这个开发壳读取、恢复、重试或取消真实任务。",
      nextAction: "返回工作区；连接真实任务读模型后再开放队列与恢复能力。",
      evidence: [
        {
          id: "evidence_tasks_unavailable_fixture",
          label: "未迁移状态说明",
          source: "layout_fixture.routeAvailability",
          sensitivity: "non_product_fixture",
        },
      ],
      technicalDetails: [{ label: "backend", value: "not_connected" }],
    },
  },
  "review-pending": {
    id: "review-pending",
    label: "审核：等待决定",
    activeNavigationId: "review",
    context: {
      eyebrow: "建议审核",
      title: "审核中心",
      status: { label: "等待你的决定", status: "waiting" },
    },
    boundary: unknownBoundary,
    inspector: {
      title: "建议来源与影响",
      conclusion: "系统建议把一条稳定偏好加入长期记忆，目前仍是待决策建议。",
      risk: "批准会记录决定，但只有后续应用并刷新读模型后，长期状态才可能改变。",
      nextAction: "比较当前与建议内容，确认来源和影响后再作决定。",
      evidence: [
        {
          id: "evidence_review_proposal_fixture",
          label: "建议来源样例",
          source: "ReviewItem.proposalSummary",
          sensitivity: "personal_context",
        },
        {
          id: "evidence_review_target_fixture",
          label: "影响对象样例",
          source: "ReviewItem.targetRefs",
          sensitivity: "local_metadata",
        },
      ],
      technicalDetails: [
        { label: "reviewId", value: "review:fixture-preference-001" },
        { label: "state", value: "pending_decision" },
      ],
    },
  },
  "review-approved": {
    id: "review-approved",
    label: "审核：已批准未应用",
    activeNavigationId: "review",
    context: {
      eyebrow: "建议审核",
      title: "审核中心",
      status: { label: "已批准，尚未应用", status: "waiting" },
    },
    boundary: unknownBoundary,
    inspector: {
      title: "批准与应用状态",
      conclusion: "批准决定已经记录，但没有证据证明长期状态已经应用。",
      risk: "把 approved 显示成 applied 会误导用户认为 LifeModel 已更新。",
      nextAction: "等待应用命令与刷新后的读模型；在此之前保持“尚未应用”。",
      evidence: [
        {
          id: "evidence_approval_fixture",
          label: "批准决定样例",
          source: "ReviewItem.status",
          sensitivity: "local_metadata",
        },
        {
          id: "evidence_application_fixture",
          label: "应用状态样例",
          source: "ReviewItem.materializationStatus",
          sensitivity: "local_metadata",
        },
      ],
      technicalDetails: [
        { label: "decision", value: "approved" },
        { label: "application", value: "not_materialized" },
      ],
    },
  },
  "life-model-limited": {
    id: "life-model-limited",
    label: "LifeModel：兼容受限",
    activeNavigationId: "life-model",
    context: {
      eyebrow: "长期理解",
      title: "LifeModel",
      status: { label: "当前兼容受限", status: "unknown" },
    },
    boundary: unknownBoundary,
    inspector: {
      title: "LifeModel 可见范围",
      conclusion: "当前视图只能表达有来源的摘要与限制，不能代表完整长期状态。",
      risk: "缺少来源、陈旧时间或应用状态时，不应把候选内容显示为长期事实。",
      nextAction: "只查看有来源的摘要；写入与回滚能力保持不可用。",
      evidence: [
        {
          id: "evidence_life_model_fixture",
          label: "长期理解摘要样例",
          source: "LifeModelViewModel.currentViewSummary + provenanceRefs",
          sensitivity: "highly_personal",
        },
      ],
      technicalDetails: [{ label: "compatibility", value: "limited_fixture_only" }],
    },
  },
  "safe-mode": {
    id: "safe-mode",
    label: "安全模式：未知关闭",
    activeNavigationId: "workspace",
    context: {
      eyebrow: "保护状态",
      title: "工作区",
      status: { label: "安全模式", status: "waiting" },
    },
    boundary: {
      label: "传输边界未知",
      detail: "缺少新鲜证据；外部动作与长期写入保持关闭。",
      status: "unknown",
    },
    inspector: {
      title: "安全模式依据",
      conclusion: "系统仍允许查看和本地整理，但不会自动执行外部动作或长期写入。",
      risk: "风险来自缺失或陈旧的边界证据，而不是安全模式本身。",
      nextAction: "刷新后端边界摘要；确认前保持关闭。",
      evidence: [
        {
          id: "evidence_boundary_missing_fixture",
          label: "边界证据缺失样例",
          source: "ProviderPrivacyBoundarySummary",
          sensitivity: "privacy_metadata",
        },
      ],
      technicalDetails: [
        { label: "summary", value: "missing_or_stale" },
        { label: "failClosed", value: "true" },
      ],
    },
  },
};

export const settingsContext: WorkbenchContextSummary = {
  eyebrow: "设置",
  title: "模型与供应商",
  status: { label: "边界待后端刷新", status: "unknown" },
};

export const settingsBoundary: WorkbenchBoundarySummary = {
  label: "当前传输边界未知",
  detail: "配置值不能替代后端返回的真实传输边界。",
  status: "unknown",
};

export const settingsInspector: WorkbenchInspectorModel = {
  title: "配置与真实边界",
  conclusion: "这里展示的是设置布局；当前供应商、路由和传输结果均未连接后端。",
  risk: "保存配置或测试连接都不能单独证明请求保持本地，也不能证明没有外传。",
  nextAction: "React 迁移时按测试、保存、边界刷新三个独立步骤接入真实命令。",
  evidence: [
    {
      id: "evidence_provider_boundary_fixture",
      label: "供应商隐私边界样例",
      source: "ProviderPrivacyBoundarySummary",
      sensitivity: "privacy_metadata",
    },
  ],
  technicalDetails: [
    { label: "config", value: "layout_fixture" },
    { label: "routeType", value: "unknown" },
  ],
};

export const fixtureActions = {
  openPendingReview: {
    id: "fixture.today.open_pending_review",
    kind: "product",
    enabled: true,
    disabledReason: null,
    targetRef: "review:fixture-preference-001",
    confirmation: "none",
    materialization: "none",
  },
  openReviewEvidence: {
    id: "fixture.review.open_evidence",
    kind: "product",
    enabled: true,
    disabledReason: null,
    targetRef: "review:fixture-preference-001",
    confirmation: "none",
    materialization: "none",
  },
  openPermissionScope: {
    id: "fixture.workspace.open_permission_scope",
    kind: "product",
    enabled: true,
    disabledReason: null,
    targetRef: "permission:fixture-external-summary",
    confirmation: "none",
    materialization: "none",
  },
  returnToWorkspace: {
    id: "fixture.tasks.return_workspace",
    kind: "product",
    enabled: true,
    disabledReason: null,
    targetRef: "workspace:fixture-current",
    confirmation: "none",
    materialization: "none",
  },
  continueWorkspace: {
    id: "fixture.workspace.continue",
    kind: "product",
    enabled: false,
    disabledReason: "等待权限决定；当前尚不能恢复任务。",
    targetRef: "task:fixture-travel-expense",
    confirmation: "none",
    materialization: "none",
  },
  deferReview: {
    id: "fixture.review.defer",
    kind: "review",
    enabled: true,
    disabledReason: null,
    targetRef: "review:fixture-preference-001",
    confirmation: "none",
    materialization: "decision_only_refresh_required",
  },
  editReview: {
    id: "fixture.review.edit",
    kind: "review",
    enabled: true,
    disabledReason: null,
    targetRef: "review:fixture-preference-001",
    confirmation: "none",
    materialization: "decision_only_refresh_required",
  },
  rejectReview: {
    id: "fixture.review.reject",
    kind: "review",
    enabled: true,
    disabledReason: null,
    targetRef: "review:fixture-preference-001",
    confirmation: "required",
    materialization: "decision_only_refresh_required",
  },
  approveReview: {
    id: "fixture.review.approve",
    kind: "review",
    enabled: true,
    disabledReason: null,
    targetRef: "review:fixture-preference-001",
    confirmation: "required",
    materialization: "decision_only_refresh_required",
  },
  saveReviewEdit: {
    id: "fixture.review.save_edit",
    kind: "review",
    enabled: true,
    disabledReason: null,
    targetRef: "review:fixture-preference-001",
    confirmation: "none",
    materialization: "decision_only_refresh_required",
  },
  refreshApplication: {
    id: "fixture.review.refresh_application",
    kind: "product",
    enabled: true,
    disabledReason: null,
    targetRef: "review:fixture-preference-001",
    confirmation: "none",
    materialization: "none",
  },
  applyChange: {
    id: "fixture.review.apply_change",
    kind: "product",
    enabled: false,
    disabledReason: "当前没有可用的应用命令；批准不能直接显示为完成。",
    targetRef: "review:fixture-preference-001",
    confirmation: "required",
    materialization: "decision_only_refresh_required",
  },
  safeModeExternalAction: {
    id: "fixture.safe_mode.external_action",
    kind: "product",
    enabled: false,
    disabledReason: "传输边界未知；安全模式保持外部动作关闭。",
    targetRef: "privacy-boundary:fixture-current",
    confirmation: "required",
    materialization: "none",
  },
  openLifeModelEvidence: {
    id: "fixture.life_model.open_evidence",
    kind: "product",
    enabled: true,
    disabledReason: null,
    targetRef: "life-state-projection:fixture-current",
    confirmation: "none",
    materialization: "none",
  },
  openSafeModeEvidence: {
    id: "fixture.safe_mode.open_evidence",
    kind: "product",
    enabled: true,
    disabledReason: null,
    targetRef: "privacy-boundary:fixture-current",
    confirmation: "none",
    materialization: "none",
  },
  testProvider: {
    id: "fixture.settings.test_provider",
    kind: "product",
    enabled: false,
    disabledReason: "当前未接入外部请求确认与测试命令。",
    targetRef: "provider-config:fixture-draft",
    confirmation: "required",
    materialization: "none",
  },
  openSettingsBoundary: {
    id: "fixture.settings.open_boundary",
    kind: "product",
    enabled: true,
    disabledReason: null,
    targetRef: "provider-privacy-boundary:fixture-current",
    confirmation: "none",
    materialization: "none",
  },
  saveProvider: {
    id: "fixture.settings.save_provider",
    kind: "product",
    enabled: false,
    disabledReason: "当前未接入保存与边界刷新命令。",
    targetRef: "provider-config:fixture-draft",
    confirmation: "none",
    materialization: "none",
  },
} satisfies Record<string, FixtureActionContract>;

export const settingsCategoryCopy: Record<string, { title: string; description: string }> = {
  "model-provider": {
    title: "模型与供应商",
    description: "配置选择与真实路由结论分开呈现。",
  },
  "privacy-network": {
    title: "隐私与网络",
    description: "解释传输边界、网络策略和缺失证据。",
  },
  "tools-permissions": {
    title: "工具与权限",
    description: "按用户任务查看能力范围和可撤销权限。",
  },
  "data-recovery": {
    title: "数据与恢复",
    description: "导出、快照和恢复操作将在真实契约接入后开放。",
  },
  "life-memory": {
    title: "LifeModel 与记忆",
    description: "长期状态仍由后端读模型和审核流程拥有。",
  },
  appearance: {
    title: "外观",
    description: "当前阶段只冻结桌面白色工作台视觉基础。",
  },
  "advanced-support": {
    title: "高级与支持",
    description: "诊断信息属于设置上下文，不是一级产品入口。",
  },
};

export const settingsCategoryIcons = {
  settings: Settings2,
  privacy: LockKeyhole,
  appearance: Eye,
  controls: SlidersHorizontal,
};
