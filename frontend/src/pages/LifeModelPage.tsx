import { useEffect, useMemo, useState } from "react";
import { Link } from "react-router-dom";
import {
  AlertTriangle,
  ArrowRight,
  Brain,
  CheckCircle2,
  Clock3,
  Database,
  Inbox,
  ShieldCheck,
} from "lucide-react";
import type { LifeModel } from "../types";
import {
  builderListUnfinished,
  countMemoryChunks,
  getLifeModel,
  getMemoryTierStats,
  getModel4DCompletion,
  getSystemDiagnostics,
  listProposals,
  type AgentProposal,
  type Model4DCompletion,
  type SystemDiagnostics,
  type TierStats,
  type UnfinishedBuilderSession,
} from "../tauri";
import {
  getLifeModelQualityIssues,
  issuesForLifeModelDimension,
  type LifeModelQualityIssue,
} from "../utils/lifeModelQuality";
import {
  buildLifeModelTrustViews,
  splitLifeModelItemsByDisplayQuality,
  type LifeModelDimensionKey,
  type LifeModelDisplayQualityIssue,
} from "../utils/lifeModelTrust";
import { getSafeModeReason, isSafeMode } from "../utils/safeMode";
import {
  StatusChip as ProductStatusChip,
  TrustDrawer,
} from "../components/product/ProductPrimitives";

type LifeModelSection = "build" | "overview" | "evidence";

type SectionConfig = {
  id: LifeModelSection;
  label: string;
};

type ModelDimension = {
  key: LifeModelDimensionKey;
  title: string;
  items: string[];
  suppressedIssues: LifeModelDisplayQualityIssue[];
};

type LifeModelPageState = {
  lifeModel: LifeModel | null;
  diagnostics: SystemDiagnostics | null;
  completion: Model4DCompletion | null;
  unfinishedSessions: UnfinishedBuilderSession[];
  memoryCount: number | null;
  tierStats: TierStats | null;
  pendingProposals: AgentProposal[];
  loading: boolean;
  error: string;
};

const SECTIONS: SectionConfig[] = [
  { id: "build", label: "构建" },
  { id: "overview", label: "概览" },
  { id: "evidence", label: "依据" },
];

const INITIAL_STATE: LifeModelPageState = {
  lifeModel: null,
  diagnostics: null,
  completion: null,
  unfinishedSessions: [],
  memoryCount: null,
  tierStats: null,
  pendingProposals: [],
  loading: true,
  error: "",
};

function normalizePercent(value: number | undefined | null): number | null {
  if (typeof value !== "number" || Number.isNaN(value)) return null;
  const normalized = value > 0 && value <= 1 ? value * 100 : value;
  return Math.max(0, Math.min(100, normalized));
}

function formatPercent(value: number | undefined | null): string {
  const normalized = normalizePercent(value);
  return normalized == null ? "未读取" : `约 ${Math.round(normalized)}%`;
}

function readinessLabel(value: number | undefined | null): string {
  const normalized = normalizePercent(value);
  if (normalized == null) return "状态未读取";
  if (normalized < 40) return "待补全";
  if (normalized < 70) return "正在形成";
  if (normalized < 85) return "基本可用";
  return "较完整";
}

function compactText(value: string | undefined | null, maxLength = 46): string | null {
  const normalized = value?.trim();
  if (!normalized) return null;
  if (normalized.length <= maxLength) return normalized;
  return `${normalized.slice(0, maxLength - 1)}…`;
}

function uniqueShortItems(items: Array<string | null | undefined>, limit = 3): string[] {
  const seen = new Set<string>();
  const output: string[] = [];
  for (const item of items) {
    const text = compactText(item);
    if (!text || seen.has(text)) continue;
    seen.add(text);
    output.push(text);
    if (output.length >= limit) break;
  }
  return output;
}

function formatUpdatedAt(value: string | undefined): string {
  if (!value) return "最近更新 未记录";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "最近更新 未记录";
  return `最近更新 ${date.toLocaleDateString("zh-CN")}`;
}

function sourceLabel(source: string): string {
  const labels: Record<string, string> = {
    builder_review: "构建",
    calibration_run: "校准",
    feedback_evolution: "反馈",
    memory_governance: "记忆治理",
    skill_runtime: "技能候选",
    plugin: "插件确认",
    manual: "手动调整",
    chat_conversation: "对话",
    proactive_agent: "OpenLife 主动提醒",
    planning_session: "规划",
  };
  return labels[source] ?? "待确认";
}

function proposalTypeLabel(type: string): string {
  const labels: Record<string, string> = {
    life_model_update: "Life Model 更新",
    memory_update: "记忆更新",
    scheduled_task: "任务",
    external_write_action: "外部写入",
    data_export: "数据导出",
  };
  return labels[type] ?? "待确认项";
}

function completionOverall(
  diagnostics: SystemDiagnostics | null,
  completion: Model4DCompletion | null
): number | null {
  const diagnosticOverall = normalizePercent(diagnostics?.builder_completion?.overall);
  if (diagnosticOverall != null) return diagnosticOverall;
  const explicitOverall = normalizePercent(completion?.overall);
  if (explicitOverall != null) return explicitOverall;
  const values = [
    normalizePercent(completion?.identity),
    normalizePercent(completion?.goals),
    normalizePercent(completion?.capabilities),
    normalizePercent(completion?.state),
  ].filter((value): value is number => value != null);
  if (!values.length) return null;
  return values.reduce((sum, value) => sum + value, 0) / values.length;
}

function isModelEmpty(model: LifeModel | null, diagnostics: SystemDiagnostics | null): boolean {
  if (!model) return true;
  if (diagnostics?.model_empty) return true;
  const dimensions = buildDimensions(model);
  return dimensions.every(dimension => dimension.items.length === 0);
}

function buildDimensions(model: LifeModel): ModelDimension[] {
  function dimension(
    key: LifeModelDimensionKey,
    title: string,
    rawItems: Array<string | null | undefined>
  ): ModelDimension {
    const quality = splitLifeModelItemsByDisplayQuality(uniqueShortItems(rawItems, 8));
    return {
      key,
      title,
      items: quality.displayable.slice(0, 3),
      suppressedIssues: quality.suppressed,
    };
  }

  return [
    dimension("identity", "Identity", [
      model.identity.name,
      model.identity.role_definition.primary_role,
      ...model.identity.values.map(value => value.name),
      model.identity.mission_statement,
    ]),
    dimension("goals", "Goals", [
      ...model.goals.daily.map(goal => goal.name),
      ...model.goals.short_term.map(goal => goal.name),
      ...model.goals.medium_term.map(goal => goal.name),
      ...model.goals.long_term.map(goal => goal.name),
      ...model.goals.life_goals.map(goal => goal.name),
    ]),
    dimension("capabilities", "Capabilities", [
      ...model.capabilities.skills.map(skill => skill.name),
      ...model.capabilities.knowledge_domains.map(domain => domain.domain),
      ...model.capabilities.resources.map(resource => resource.name),
    ]),
    dimension("state", "State", [
      model.state.current_focus ? `当前专注：${model.state.current_focus}` : null,
      ...model.state.focus_areas,
      model.state.health_status.energy_level
        ? `能量：${model.state.health_status.energy_level}/10`
        : null,
    ]),
  ];
}

function countBuilderReviewItems(
  diagnostics: SystemDiagnostics | null,
  pendingProposals: AgentProposal[]
): number {
  const diagnosticCount = diagnostics?.pending_builder_review_sessions ?? 0;
  const proposalCount = pendingProposals.filter(
    proposal => proposal.source === "builder_review"
  ).length;
  return Math.max(diagnosticCount, proposalCount);
}

function StatusChip({ label }: { label: string }) {
  return (
    <span className="inline-flex h-7 items-center rounded-md border border-stone-200 bg-white px-2.5 text-xs font-medium text-stone-700">
      {label}
    </span>
  );
}

function SectionTabs({
  active,
  onChange,
}: {
  active: LifeModelSection;
  onChange: (section: LifeModelSection) => void;
}) {
  return (
    <div
      role="tablist"
      aria-label="Life Model sections"
      className="inline-grid grid-cols-3 rounded-lg border border-stone-200 bg-white p-1"
    >
      {SECTIONS.map(section => {
        const selected = active === section.id;
        return (
          <button
            key={section.id}
            type="button"
            role="tab"
            aria-selected={selected}
            aria-controls={`life-model-${section.id}`}
            onClick={() => onChange(section.id)}
            className={[
              "h-8 rounded-md px-4 text-sm font-semibold transition",
              selected ? "bg-stone-900 text-white" : "text-stone-600 hover:bg-stone-100",
            ].join(" ")}
          >
            {section.label}
          </button>
        );
      })}
    </div>
  );
}

function BuildSection({
  diagnostics,
  completion,
  unfinishedSessions,
  pendingProposals,
}: {
  diagnostics: SystemDiagnostics | null;
  completion: Model4DCompletion | null;
  unfinishedSessions: UnfinishedBuilderSession[];
  pendingProposals: AgentProposal[];
}) {
  const overall = completionOverall(diagnostics, completion);
  const builderReviewCount = countBuilderReviewItems(diagnostics, pendingProposals);
  const unfinishedCount = Math.max(
    diagnostics?.unfinished_builder_sessions ?? 0,
    unfinishedSessions.filter(session => !session.finished).length
  );
  const reviewReadyCount = unfinishedSessions.filter(
    session => session.finished && (session.pending_signals?.length ?? 0) > 0
  ).length;

  return (
    <section id="life-model-build" role="tabpanel" className="space-y-5">
      <div className="grid gap-3 md:grid-cols-[1fr_auto] md:items-start">
        <div>
          <h2 className="text-sm font-semibold text-stone-950">构建状态</h2>
          <p className="mt-1 text-sm text-stone-600">
            构建产生候选，Review 确认后才会更新 Life Model。
          </p>
        </div>
      </div>

      <div className="grid gap-3 lg:grid-cols-3">
        <div className="rounded-lg border border-stone-200 bg-white p-4">
          <div className="text-base font-semibold text-stone-950">快速构建</div>
          <p className="mt-2 min-h-10 text-sm leading-5 text-stone-600">
            少量问题，先形成可用轮廓。
          </p>
          <Link
            to="/builder"
            className="mt-4 inline-flex h-9 items-center justify-center gap-2 rounded-md bg-stone-900 px-3 text-sm font-semibold text-white hover:bg-stone-800"
          >
            开始快速构建
            <ArrowRight size={15} aria-hidden="true" />
          </Link>
        </div>
        <div className="rounded-lg border border-stone-200 bg-white p-4">
          <div className="text-base font-semibold text-stone-950">对话构建</div>
          <p className="mt-2 min-h-10 text-sm leading-5 text-stone-600">像聊天一样慢慢补全。</p>
          <Link
            to="/builder"
            className="mt-4 inline-flex h-9 items-center justify-center gap-2 rounded-md border border-stone-300 bg-white px-3 text-sm font-semibold text-stone-800 hover:bg-stone-50"
          >
            开始对话构建
            <ArrowRight size={15} aria-hidden="true" />
          </Link>
        </div>
        <div className="rounded-lg border border-stone-200 bg-stone-50 p-4">
          <div className="text-base font-semibold text-stone-950">从已有内容整理</div>
          <p className="mt-2 min-h-10 text-sm leading-5 text-stone-600">
            从记忆、历史或文本整理候选项。
          </p>
          <button
            type="button"
            disabled
            className="mt-4 inline-flex h-9 items-center justify-center rounded-md border border-stone-200 bg-white px-3 text-sm font-semibold text-stone-400"
          >
            暂不可用
          </button>
        </div>
      </div>

      <div className="rounded-lg border border-stone-200 bg-white">
        <div className="grid gap-0 divide-y divide-stone-100 md:grid-cols-3 md:divide-x md:divide-y-0">
          <div className="p-4">
            <div className="text-xs font-medium text-stone-500">构建状态</div>
            <div className="mt-1 text-lg font-semibold text-stone-950">
              {readinessLabel(overall)}
            </div>
            <div className="mt-1 text-xs text-stone-500">{formatPercent(overall)}</div>
          </div>
          <div className="p-4">
            <div className="text-xs font-medium text-stone-500">未完成会话</div>
            <div className="mt-1 text-lg font-semibold text-stone-950">{unfinishedCount}</div>
            <div className="mt-1 text-xs text-stone-500">
              {reviewReadyCount > 0 ? `${reviewReadyCount} 个已可确认` : "可继续构建"}
            </div>
          </div>
          <div className="p-4">
            <div className="text-xs font-medium text-stone-500">待确认更新</div>
            <div className="mt-1 text-lg font-semibold text-stone-950">{builderReviewCount}</div>
            <div className="mt-1 text-xs text-stone-500">通过 Review 处理</div>
          </div>
        </div>
      </div>

      {builderReviewCount > 0 && (
        <div className="flex flex-wrap items-center justify-between gap-3 rounded-lg border border-amber-200 bg-amber-50 px-4 py-3">
          <div>
            <div className="text-sm font-semibold text-amber-950">有构建内容等待确认</div>
            <div className="mt-0.5 text-xs text-amber-800">
              这里不直接应用更新；请在 Review 中逐项处理。
            </div>
          </div>
          <Link
            to="/mailbox"
            className="inline-flex h-8 items-center gap-2 rounded-md bg-amber-900 px-3 text-xs font-semibold text-white hover:bg-amber-950"
          >
            打开 Review
            <Inbox size={14} aria-hidden="true" />
          </Link>
        </div>
      )}
    </section>
  );
}

function OverviewSection({
  lifeModel,
  diagnostics,
  completion,
  pendingProposals,
}: {
  lifeModel: LifeModel | null;
  diagnostics: SystemDiagnostics | null;
  completion: Model4DCompletion | null;
  pendingProposals: AgentProposal[];
}) {
  const [ignoredIssueIds, setIgnoredIssueIds] = useState<Set<string>>(new Set());
  const [deferredIssueIds, setDeferredIssueIds] = useState<Set<string>>(new Set());
  const empty = isModelEmpty(lifeModel, diagnostics);
  const dimensions = useMemo(() => (lifeModel ? buildDimensions(lifeModel) : []), [lifeModel]);
  const suppressedByDimension = useMemo(
    () =>
      dimensions.reduce(
        (acc, dimension) => {
          acc[dimension.key] = dimension.suppressedIssues;
          return acc;
        },
        {
          identity: [],
          goals: [],
          capabilities: [],
          state: [],
        } as Record<LifeModelDimensionKey, LifeModelDisplayQualityIssue[]>
      ),
    [dimensions]
  );
  const trustViews = useMemo(
    () =>
      buildLifeModelTrustViews({
        lifeModel,
        diagnostics,
        completion,
        pendingProposals,
        suppressedByDimension,
      }),
    [completion, diagnostics, lifeModel, pendingProposals, suppressedByDimension]
  );
  const qualityIssues = useMemo(() => getLifeModelQualityIssues(lifeModel), [lifeModel]);
  const visibleQualityIssues = useMemo(
    () => qualityIssues.filter(issue => !ignoredIssueIds.has(issue.id)),
    [ignoredIssueIds, qualityIssues]
  );

  if (empty) {
    return (
      <section
        id="life-model-overview"
        role="tabpanel"
        className="rounded-lg border border-dashed border-stone-300 bg-white px-5 py-8 text-center"
      >
        <Brain size={22} aria-hidden="true" className="mx-auto text-stone-500" />
        <h2 className="mt-3 text-sm font-semibold text-stone-950">模型还没有形成稳定摘要</h2>
        <p className="mx-auto mt-1 max-w-md text-sm text-stone-600">
          先用 Builder 形成首轮结构，再回到这里查看四维摘要。
        </p>
        <Link
          to="/builder"
          className="mt-4 inline-flex h-9 items-center justify-center gap-2 rounded-md bg-stone-900 px-3 text-sm font-semibold text-white hover:bg-stone-800"
        >
          去构建
          <ArrowRight size={15} aria-hidden="true" />
        </Link>
      </section>
    );
  }

  return (
    <section id="life-model-overview" role="tabpanel" className="space-y-4">
      <div>
        <h2 className="text-sm font-semibold text-stone-950">四维摘要</h2>
        <p className="mt-1 text-sm text-stone-600">
          只显示短摘要；完整构建和确认仍在 Builder 与 Review 中完成。
        </p>
      </div>
      {visibleQualityIssues.length > 0 && (
        <QualityIssuePanel
          issues={visibleQualityIssues}
          deferredIssueIds={deferredIssueIds}
          onIgnore={issueId => {
            setIgnoredIssueIds(current => {
              const next = new Set(current);
              next.add(issueId);
              return next;
            });
          }}
          onDefer={issueId => {
            setDeferredIssueIds(current => {
              const next = new Set(current);
              next.add(issueId);
              return next;
            });
          }}
        />
      )}
      <div className="rounded-lg border border-stone-200 bg-white">
        {dimensions.map((dimension, index) => (
          <div
            key={dimension.key}
            className={[
              "grid gap-3 px-4 py-4 sm:grid-cols-[170px_1fr]",
              index === 0 ? "" : "border-t border-stone-100",
            ].join(" ")}
          >
            <div>
              <div className="text-sm font-semibold text-stone-950">{dimension.title}</div>
              <div className="mt-0.5 text-xs text-stone-500">{dimension.items.length} 条摘要</div>
              <div className="mt-2 flex flex-wrap gap-1">
                {trustViews[dimension.key].pendingProposalCount > 0 && (
                  <ProductStatusChip label="Review 待确认" tone="warning" />
                )}
                {dimension.suppressedIssues.length > 0 && (
                  <ProductStatusChip label="原始抽取已降级" tone="warning" />
                )}
                {issuesForLifeModelDimension(visibleQualityIssues, dimension.key).length > 0 && (
                  <ProductStatusChip label="需要修正" tone="warning" />
                )}
              </div>
            </div>
            <div className="space-y-3">
              {dimension.items.length ? (
                <ul className="grid gap-1.5 text-sm text-stone-700">
                  {dimension.items.slice(0, 3).map(item => (
                    <li key={item} className="flex items-center gap-2">
                      <span className="h-1.5 w-1.5 rounded-full bg-stone-400" aria-hidden="true" />
                      <span>{item}</span>
                    </li>
                  ))}
                </ul>
              ) : (
                <div className="text-sm text-stone-500">
                  {dimension.suppressedIssues.length > 0
                    ? "暂无已确认摘要；低质量抽取已放入依据。"
                    : "暂无摘要"}
                </div>
              )}
              <TrustDrawer
                title={`${dimension.title} 可信度`}
                subtitle={`${trustViews[dimension.key].statusLabel} · ${trustViews[dimension.key].confidenceLabel}`}
              >
                <div className="grid gap-2 text-xs text-stone-600 md:grid-cols-2">
                  <div>来源：{trustViews[dimension.key].sourceSummary}</div>
                  <div>{trustViews[dimension.key].updatedAtLabel}</div>
                  <div>待确认：{trustViews[dimension.key].pendingProposalCount}</div>
                  <div>依据：{trustViews[dimension.key].evidenceSummary}</div>
                </div>
                {dimension.suppressedIssues.length > 0 && (
                  <div className="mt-3 rounded-md border border-amber-200 bg-amber-50 px-3 py-2">
                    <div className="text-xs font-semibold text-amber-950">
                      不进入正式摘要的原始抽取
                    </div>
                    <div className="mt-2 space-y-2">
                      {dimension.suppressedIssues.slice(0, 3).map(issue => (
                        <div key={issue.value} className="text-xs text-amber-900">
                          <div className="font-medium">{issue.value}</div>
                          <div className="mt-0.5">
                            {issue.reason} {issue.recoveryAction}
                          </div>
                        </div>
                      ))}
                    </div>
                  </div>
                )}
                <div className="mt-3 flex flex-wrap gap-2">
                  <Link
                    to="/mailbox"
                    className="rounded-md bg-stone-900 px-2.5 py-1 text-xs font-semibold text-white hover:bg-stone-800"
                  >
                    Open Review
                  </Link>
                  <Link
                    to="/builder"
                    className="rounded-md border border-stone-200 bg-white px-2.5 py-1 text-xs font-semibold text-stone-700 hover:bg-stone-50"
                  >
                    Correct
                  </Link>
                  <button
                    type="button"
                    disabled
                    className="rounded-md border border-stone-200 bg-stone-50 px-2.5 py-1 text-xs font-semibold text-stone-400"
                  >
                    Forget
                  </button>
                </div>
              </TrustDrawer>
            </div>
          </div>
        ))}
      </div>
    </section>
  );
}

function QualityIssuePanel({
  issues,
  deferredIssueIds,
  onIgnore,
  onDefer,
}: {
  issues: LifeModelQualityIssue[];
  deferredIssueIds: Set<string>;
  onIgnore: (issueId: string) => void;
  onDefer: (issueId: string) => void;
}) {
  return (
    <div className="rounded-lg border border-amber-200 bg-amber-50 px-4 py-3">
      <div className="flex items-start gap-2">
        <AlertTriangle size={16} aria-hidden="true" className="mt-0.5 shrink-0 text-amber-800" />
        <div>
          <div className="text-sm font-semibold text-amber-950">发现可能影响画像可信度的字段</div>
          <div className="mt-0.5 text-xs text-amber-800">
            本次视图处理，不会改写 Life Model；正式更新仍需 Review 确认。
          </div>
        </div>
      </div>
      <div className="mt-3 grid gap-2">
        {issues.map((issue, index) => (
          <div
            key={`${issue.dimension}-${issue.label}-${index}`}
            className="flex flex-wrap items-center justify-between gap-2 rounded-md border border-amber-200 bg-white/80 px-3 py-2"
          >
            <div>
              <div className="text-sm font-medium text-amber-950">{issue.label}</div>
              <div className="mt-0.5 text-xs text-amber-800">{issue.detail}</div>
              {deferredIssueIds.has(issue.id) && (
                <div className="mt-1 text-xs font-medium text-stone-600">已标记稍后处理</div>
              )}
            </div>
            <div className="flex flex-wrap items-center gap-2">
              <Link
                to={issue.route}
                className="rounded-md bg-amber-900 px-2.5 py-1 text-xs font-semibold text-white hover:bg-amber-950"
              >
                修正
              </Link>
              <button
                type="button"
                onClick={() => onIgnore(issue.id)}
                className="rounded-md border border-amber-200 bg-white px-2.5 py-1 text-xs font-semibold text-amber-900 hover:bg-amber-50"
              >
                不采用
              </button>
              <button
                type="button"
                onClick={() => onDefer(issue.id)}
                className="rounded-md border border-stone-200 bg-white px-2.5 py-1 text-xs font-semibold text-stone-700 hover:bg-stone-50"
              >
                稍后处理
              </button>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

function EvidenceSection({
  diagnostics,
  memoryCount,
  tierStats,
  pendingProposals,
}: {
  diagnostics: SystemDiagnostics | null;
  memoryCount: number | null;
  tierStats: TierStats | null;
  pendingProposals: AgentProposal[];
}) {
  const effectiveMemoryCount = memoryCount ?? diagnostics?.memory_chunk_count ?? 0;
  const pendingCount = pendingProposals.length || diagnostics?.pending_proposal_count || 0;
  const sourceLabels = uniqueShortItems(
    pendingProposals.map(proposal => sourceLabel(proposal.source)),
    3
  );
  const recentSources = sourceLabels.length ? sourceLabels.join(" / ") : "暂无待确认来源";

  return (
    <section id="life-model-evidence" role="tabpanel" className="space-y-5">
      <div className="grid gap-3 md:grid-cols-[1fr_auto] md:items-start">
        <div>
          <h2 className="text-sm font-semibold text-stone-950">依据层</h2>
          <p className="mt-1 text-sm text-stone-600">
            记忆和待确认内容只在这里作为低信息量摘要出现。
          </p>
        </div>
        <div className="flex flex-wrap gap-2">
          <Link
            to="/memory"
            className="inline-flex h-9 items-center justify-center gap-2 rounded-md border border-stone-300 bg-white px-3 text-sm font-semibold text-stone-800 hover:bg-stone-50"
          >
            查看记忆
            <Database size={15} aria-hidden="true" />
          </Link>
          <Link
            to="/mailbox"
            className="inline-flex h-9 items-center justify-center gap-2 rounded-md bg-stone-900 px-3 text-sm font-semibold text-white hover:bg-stone-800"
          >
            打开 Review
            <Inbox size={15} aria-hidden="true" />
          </Link>
        </div>
      </div>

      <div className="rounded-lg border border-stone-200 bg-white">
        <div className="grid gap-0 divide-y divide-stone-100 md:grid-cols-3 md:divide-x md:divide-y-0">
          <div className="p-4">
            <div className="text-xs font-medium text-stone-500">记忆条数</div>
            <div className="mt-1 text-lg font-semibold text-stone-950">{effectiveMemoryCount}</div>
            <div className="mt-1 text-xs text-stone-500">
              {tierStats
                ? `活跃 ${tierStats.tier1 + tierStats.tier2 + tierStats.tier3}`
                : "只读统计"}
            </div>
          </div>
          <div className="p-4">
            <div className="text-xs font-medium text-stone-500">待确认更新</div>
            <div className="mt-1 text-lg font-semibold text-stone-950">{pendingCount}</div>
            <div className="mt-1 text-xs text-stone-500">进入 Review 确认</div>
          </div>
          <div className="p-4">
            <div className="text-xs font-medium text-stone-500">最近依据来源</div>
            <div className="mt-1 text-sm font-semibold text-stone-950">{recentSources}</div>
            <div className="mt-1 text-xs text-stone-500">不显示原始内容</div>
          </div>
        </div>
      </div>

      {pendingProposals.length > 0 && (
        <div className="rounded-lg border border-stone-200 bg-white">
          {pendingProposals.slice(0, 3).map((proposal, index) => (
            <div
              key={proposal.id}
              className={[
                "flex flex-wrap items-center justify-between gap-3 px-4 py-3",
                index === 0 ? "" : "border-t border-stone-100",
              ].join(" ")}
            >
              <div>
                <div className="text-sm font-semibold text-stone-950">
                  {proposalTypeLabel(proposal.proposalType)}
                </div>
                <div className="mt-0.5 text-xs text-stone-500">
                  {sourceLabel(proposal.source)} · 影响 {proposal.riskLevel}
                </div>
                <div className="mt-1 text-xs text-stone-600">
                  {proposal.whyOpenLifeThinksThis?.trim() ||
                    (proposal.evidenceSummaries?.length
                      ? `${proposal.evidenceSummaries.length} 条依据摘要待你确认`
                      : "暂无足够依据摘要")}
                </div>
              </div>
              <Link
                to="/mailbox"
                className="text-xs font-semibold text-stone-700 underline-offset-4 hover:underline"
              >
                去 Review 处理
              </Link>
            </div>
          ))}
        </div>
      )}
    </section>
  );
}

export default function LifeModelPage() {
  const [activeSection, setActiveSection] = useState<LifeModelSection>("build");
  const [state, setState] = useState<LifeModelPageState>(INITIAL_STATE);

  useEffect(() => {
    let cancelled = false;

    async function loadLifeModelSurface() {
      setState(current => ({ ...current, loading: true, error: "" }));
      try {
        const [
          lifeModel,
          diagnostics,
          completion,
          unfinishedSessions,
          memoryCount,
          tierStats,
          pendingProposals,
        ] = await Promise.all([
          getLifeModel().catch(() => null),
          getSystemDiagnostics().catch(() => null),
          getModel4DCompletion().catch(() => null),
          builderListUnfinished().catch(() => []),
          countMemoryChunks().catch(() => null),
          getMemoryTierStats().catch(() => null),
          listProposals("pending").catch(() => []),
        ]);

        if (cancelled) return;
        setState({
          lifeModel,
          diagnostics,
          completion,
          unfinishedSessions,
          memoryCount,
          tierStats,
          pendingProposals,
          loading: false,
          error: "",
        });
      } catch (error) {
        if (cancelled) return;
        setState(current => ({
          ...current,
          loading: false,
          error: `Life Model 状态读取失败：${String(error)}`,
        }));
      }
    }

    loadLifeModelSurface();

    return () => {
      cancelled = true;
    };
  }, []);

  const overall = completionOverall(state.diagnostics, state.completion);
  const safeMode = isSafeMode(state.diagnostics);
  const safeModeReason = getSafeModeReason(state.diagnostics);
  const pendingCount =
    state.pendingProposals.length || state.diagnostics?.pending_proposal_count || 0;
  const topStatus =
    state.diagnostics?.life_model_ready && !state.diagnostics?.model_empty
      ? "Life Model 本地可读"
      : "待构建";

  return (
    <div
      data-testid="life-model-page"
      className="h-full overflow-auto overflow-x-hidden bg-[#f5f6f2]"
    >
      <div className="mx-auto flex w-full max-w-[1500px] flex-col gap-5 px-4 py-5 lg:px-6">
        <header className="flex flex-wrap items-start justify-between gap-4">
          <div className="min-w-0">
            <div className="flex items-center gap-2">
              <Brain size={20} aria-hidden="true" className="text-stone-700" />
              <h1 className="text-xl font-semibold tracking-normal text-stone-950">Life Model</h1>
            </div>
            <div className="mt-3 flex flex-wrap gap-2">
              <StatusChip label={topStatus} />
              <StatusChip label={`待确认 ${pendingCount}`} />
              <StatusChip label={formatUpdatedAt(state.lifeModel?.metadata.updated_at)} />
            </div>
          </div>
          <div className="flex items-center gap-2 rounded-lg border border-stone-200 bg-white px-3 py-2">
            <CheckCircle2 size={16} aria-hidden="true" className="text-emerald-700" />
            <div>
              <div className="text-xs font-medium text-stone-500">状态</div>
              <div className="text-sm font-semibold text-stone-950">{readinessLabel(overall)}</div>
            </div>
          </div>
        </header>

        {safeMode && (
          <div className="flex flex-wrap items-start gap-3 rounded-lg border border-amber-200 bg-amber-50 px-4 py-3 text-sm text-amber-900">
            <ShieldCheck size={17} aria-hidden="true" className="mt-0.5 shrink-0" />
            <div>
              <div className="font-semibold">Safe Mode：Life Model 只读</div>
              <div className="mt-0.5 text-xs text-amber-800">{safeModeReason}</div>
            </div>
          </div>
        )}

        {state.error && (
          <div className="rounded-lg border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-800">
            {state.error}
          </div>
        )}

        <div className="flex flex-wrap items-center justify-between gap-3">
          <SectionTabs active={activeSection} onChange={setActiveSection} />
          {state.loading && (
            <div className="inline-flex items-center gap-2 text-xs font-medium text-stone-500">
              <Clock3 size={14} aria-hidden="true" />
              读取中
            </div>
          )}
        </div>

        <div className="pb-8">
          {activeSection === "build" && (
            <BuildSection
              diagnostics={state.diagnostics}
              completion={state.completion}
              unfinishedSessions={state.unfinishedSessions}
              pendingProposals={state.pendingProposals}
            />
          )}
          {activeSection === "overview" && (
            <OverviewSection
              lifeModel={state.lifeModel}
              diagnostics={state.diagnostics}
              completion={state.completion}
              pendingProposals={state.pendingProposals}
            />
          )}
          {activeSection === "evidence" && (
            <EvidenceSection
              diagnostics={state.diagnostics}
              memoryCount={state.memoryCount}
              tierStats={state.tierStats}
              pendingProposals={state.pendingProposals}
            />
          )}
        </div>
      </div>
    </div>
  );
}
