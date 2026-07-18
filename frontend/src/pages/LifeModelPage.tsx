import { useEffect, useMemo, useState } from "react";
import { Link } from "react-router-dom";
import {
  ArrowRight,
  Brain,
  CheckCircle2,
  Clock3,
  Database,
  Inbox,
  ShieldCheck,
} from "lucide-react";
import {
  builderListUnfinished,
  getLifeStateProjection,
  getLifeModelViewModel,
  type LifeStateProjection,
  type LifeModelCurrentViewSummary,
  type LifeModelDimensionSummary,
  type LifeModelViewModel,
  type UnfinishedBuilderSession,
  type ViewModelEnvelope,
} from "../tauri";
import {
  StatusChip as ProductStatusChip,
  TechnicalDetails,
  TrustDrawer,
} from "../components/product/ProductPrimitives";
import { mailboxRoute, secondaryRoutePath } from "../productShellContract";

type LifeModelSection = "build" | "overview" | "evidence";

type SectionConfig = {
  id: LifeModelSection;
  label: string;
};

type LifeModelPageState = {
  viewModelEnvelope: ViewModelEnvelope<LifeModelViewModel> | null;
  projection: LifeStateProjection | null;
  unfinishedSessions: UnfinishedBuilderSession[];
  loading: boolean;
  error: string;
};

const SECTIONS: SectionConfig[] = [
  { id: "build", label: "构建" },
  { id: "overview", label: "概览" },
  { id: "evidence", label: "依据" },
];

const INITIAL_STATE: LifeModelPageState = {
  viewModelEnvelope: null,
  projection: null,
  unfinishedSessions: [],
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

function formatUpdatedAt(value: string | undefined | null): string {
  if (!value) return "最近更新 未记录";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "最近更新 未记录";
  return `最近更新 ${date.toLocaleDateString("zh-CN")}`;
}

function dataFromEnvelope(
  envelope: ViewModelEnvelope<LifeModelViewModel> | null
): LifeModelViewModel | null {
  return envelope?.data ?? null;
}

function readinessFromViewModel(viewModel: LifeModelViewModel | null): number | null {
  return normalizePercent(viewModel?.trustQualityState.completionScore ?? null);
}

function topStatusLabel(envelope: ViewModelEnvelope<LifeModelViewModel> | null): string {
  if (!envelope) return "状态未读取";
  if (envelope.status === "empty") return "待构建";
  if (envelope.status === "error") return "状态读取失败";
  if (envelope.status === "stale") return "状态需刷新";
  const mode = envelope.data?.truthMode;
  if (mode === "current_compatibility") return "Life Model 本地可读";
  if (mode === "canonical") return "Life Model 已物化";
  return "状态已读取";
}

function readinessStateLabel(value: LifeModelViewModel["trustQualityState"]["readiness"]): string {
  const labels: Record<LifeModelViewModel["trustQualityState"]["readiness"], string> = {
    not_built: "待构建",
    limited: "受限可用",
    usable_with_limits: "基本可用",
    ready: "就绪",
    stale: "需刷新",
    unknown: "未知",
  };
  return labels[value] ?? "未知";
}

function confidenceLabel(value: LifeModelDimensionSummary["confidence"]): string {
  const labels: Record<LifeModelDimensionSummary["confidence"], string> = {
    high: "高",
    medium: "中",
    low: "低",
    unknown: "未知",
  };
  return labels[value] ?? "未知";
}

function ownerStatusLabel(value: string): string {
  if (value === "PARTIAL") return "后端部分拥有";
  if (value === "PHASE_2_REQUIRED") return "后续切片补全";
  return "未知";
}

function summaryItems(summary: string): string[] {
  return uniqueShortItems(summary.split(/\s+\/\s+|\n/), 3);
}

function formatProjectionCount(value: number | null): string {
  return value == null ? "状态未读取" : String(value);
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
  viewModel,
  projection,
  unfinishedSessions,
}: {
  viewModel: LifeModelViewModel | null;
  projection: LifeStateProjection | null;
  unfinishedSessions: UnfinishedBuilderSession[];
}) {
  const overall = readinessFromViewModel(viewModel);
  const builderReviewCount = viewModel?.pendingUpdateCounts.pendingReview ?? null;
  const unfinishedCount = projection?.readiness.unfinishedBuilderSessions ?? null;
  const reviewReadyCount =
    projection == null
      ? null
      : unfinishedSessions.filter(
          session => session.waiting_for_review && session.pending_signal_count > 0
        ).length;

  return (
    <section id="life-model-build" role="tabpanel" className="space-y-5">
      <div className="grid gap-3 md:grid-cols-[1fr_auto] md:items-start">
        <div>
          <h2 className="text-sm font-semibold text-stone-950">构建状态</h2>
          <p className="mt-1 text-sm text-stone-600">
            构建产生候选，Mailbox 确认后才会更新 Life Model。
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
            to={secondaryRoutePath("LifeModelBuild")}
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
            to={secondaryRoutePath("LifeModelBuild")}
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
            <div className="mt-1 text-lg font-semibold text-stone-950">
              {formatProjectionCount(unfinishedCount)}
            </div>
            <div className="mt-1 text-xs text-stone-500">
              {reviewReadyCount == null
                ? "状态暂不可用"
                : reviewReadyCount > 0
                  ? `${reviewReadyCount} 个已可确认`
                  : "可继续构建"}
            </div>
          </div>
          <div className="p-4">
            <div className="text-xs font-medium text-stone-500">待确认更新</div>
            <div className="mt-1 text-lg font-semibold text-stone-950">
              {formatProjectionCount(builderReviewCount)}
            </div>
            <div className="mt-1 text-xs text-stone-500">通过 Mailbox 处理</div>
          </div>
        </div>
      </div>

      {builderReviewCount != null && builderReviewCount > 0 && (
        <div className="flex flex-wrap items-center justify-between gap-3 rounded-lg border border-amber-200 bg-amber-50 px-4 py-3">
          <div>
            <div className="text-sm font-semibold text-amber-950">有构建内容等待确认</div>
            <div className="mt-0.5 text-xs text-amber-800">
              这里不直接应用更新；请在 Mailbox 中逐项处理。
            </div>
          </div>
          <Link
            to={mailboxRoute()}
            className="inline-flex h-8 items-center gap-2 rounded-md bg-amber-900 px-3 text-xs font-semibold text-white hover:bg-amber-950"
          >
            打开 Mailbox
            <Inbox size={14} aria-hidden="true" />
          </Link>
        </div>
      )}
    </section>
  );
}

function CommunicationStyleCurrentView({
  currentView,
}: {
  currentView: LifeModelCurrentViewSummary | null;
}) {
  const value = currentView?.summary?.trim();
  if (!currentView || !value) return null;
  const view = currentView;

  return (
    <section
      data-testid="communication-style-current-view"
      className="rounded-lg border border-stone-200 bg-white p-4"
    >
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <div className="text-xs font-medium text-stone-500">Current view</div>
          <h3 className="mt-1 text-sm font-semibold text-stone-950">{view.label}</h3>
        </div>
        <div className="flex flex-wrap gap-1.5">
          <ProductStatusChip label="兼容视图" tone="ready" />
          <ProductStatusChip label={ownerStatusLabel(view.ownerStatus)} />
        </div>
      </div>
      <div className="mt-3 rounded-md border border-stone-200 bg-stone-50 px-3 py-2 text-sm leading-6 text-stone-900">
        {value}
      </div>
      <p className="mt-2 text-xs leading-5 text-stone-500">
        这条摘要来自后端 LifeModelViewModel；是否已物化由下方物化记录单独展示。
      </p>

      <div className="mt-3">
        <TechnicalDetails summary="查看后端依据">
          <div className="grid gap-2 md:grid-cols-2">
            <div>
              <span className="text-stone-400">位置：</span>
              <span className="break-all">{view.currentViewRef.id}</span>
            </div>
            <div>
              <span className="text-stone-400">发散状态：</span>
              <span>{view.divergenceFromCanonical}</span>
            </div>
            <div>
              <span className="text-stone-400">依据数：</span>
              <span>{view.evidenceRefs.length}</span>
            </div>
            <div>
              <span className="text-stone-400">所有者：</span>
              <span>{ownerStatusLabel(view.ownerStatus)}</span>
            </div>
          </div>
        </TechnicalDetails>
      </div>
    </section>
  );
}

function OverviewSection({ viewModel }: { viewModel: LifeModelViewModel | null }) {
  const dimensions = useMemo(() => viewModel?.dimensionSummaries ?? [], [viewModel]);
  const empty = !viewModel || (dimensions.length === 0 && !viewModel.currentViewSummary);

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
          to={secondaryRoutePath("LifeModelBuild")}
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
          只显示后端摘要；完整构建和确认仍在 Builder 与 Mailbox 中完成。
        </p>
      </div>
      <CommunicationStyleCurrentView currentView={viewModel.currentViewSummary} />
      <div className="rounded-lg border border-stone-200 bg-white">
        {dimensions.map((dimension, index) => {
          const items = summaryItems(dimension.summary);
          return (
            <div
              key={dimension.id}
              className={[
                "grid gap-3 px-4 py-4 sm:grid-cols-[170px_1fr]",
                index === 0 ? "" : "border-t border-stone-100",
              ].join(" ")}
            >
              <div>
                <div className="text-sm font-semibold text-stone-950">{dimension.label}</div>
                <div className="mt-0.5 text-xs text-stone-500">{items.length} 条摘要</div>
                <div className="mt-2 flex flex-wrap gap-1">
                  {dimension.pendingReviewItemRefs.length > 0 && (
                    <ProductStatusChip label="Mailbox 待确认" tone="warning" />
                  )}
                  {dimension.stale && <ProductStatusChip label="需刷新" tone="warning" />}
                  <ProductStatusChip label={ownerStatusLabel(dimension.ownerStatus)} />
                </div>
              </div>
              <div className="space-y-3">
                {items.length ? (
                  <ul className="grid gap-1.5 text-sm text-stone-700">
                    {items.map(item => (
                      <li key={item} className="flex items-center gap-2">
                        <span
                          className="h-1.5 w-1.5 rounded-full bg-stone-400"
                          aria-hidden="true"
                        />
                        <span>{item}</span>
                      </li>
                    ))}
                  </ul>
                ) : (
                  <div className="text-sm text-stone-500">暂无摘要</div>
                )}
                <TrustDrawer
                  title={`${dimension.label} 可信度`}
                  subtitle={`${readinessStateLabel(viewModel.trustQualityState.readiness)} · ${confidenceLabel(dimension.confidence)}`}
                >
                  <div className="grid gap-2 text-xs text-stone-600 md:grid-cols-2">
                    <div>来源：{dimension.provenance}</div>
                    <div>待确认：{dimension.pendingReviewItemRefs.length}</div>
                    <div>依据：{dimension.evidenceRefs.length}</div>
                    <div>所有者：{ownerStatusLabel(dimension.ownerStatus)}</div>
                  </div>
                  <div className="mt-3 flex flex-wrap gap-2">
                    <Link
                      to={mailboxRoute()}
                      className="rounded-md bg-stone-900 px-2.5 py-1 text-xs font-semibold text-white hover:bg-stone-800"
                    >
                      Open Mailbox
                    </Link>
                    <Link
                      to={secondaryRoutePath("LifeModelBuild")}
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
          );
        })}
      </div>
    </section>
  );
}

function EvidenceSection({ viewModel }: { viewModel: LifeModelViewModel | null }) {
  const memoryLinkage = viewModel?.memoryLinkage;
  const effectiveMemoryCount = memoryLinkage?.linkedMemoryCount ?? 0;
  const tierSummary = memoryLinkage?.tierSummary;
  const pendingCount = viewModel?.pendingUpdateCounts.pendingReview ?? null;
  const sourceLabels = uniqueShortItems(
    viewModel?.sourceRefs.map(ref => ref.label || ref.id) ?? [],
    3
  );
  const recentSources = sourceLabels.length ? sourceLabels.join(" / ") : "暂无待确认来源";
  const candidateChanges = viewModel?.candidateChanges ?? [];
  const materializedChanges = viewModel?.materializedChanges ?? [];

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
            to={secondaryRoutePath("Memory")}
            className="inline-flex h-9 items-center justify-center gap-2 rounded-md border border-stone-300 bg-white px-3 text-sm font-semibold text-stone-800 hover:bg-stone-50"
          >
            查看记忆
            <Database size={15} aria-hidden="true" />
          </Link>
          <Link
            to={mailboxRoute()}
            className="inline-flex h-9 items-center justify-center gap-2 rounded-md bg-stone-900 px-3 text-sm font-semibold text-white hover:bg-stone-800"
          >
            打开 Mailbox
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
              {tierSummary?.total != null
                ? `活跃 ${(tierSummary.tier1 ?? 0) + (tierSummary.tier2 ?? 0) + (tierSummary.tier3 ?? 0)}`
                : "只读统计"}
            </div>
          </div>
          <div className="p-4">
            <div className="text-xs font-medium text-stone-500">待确认更新</div>
            <div className="mt-1 text-lg font-semibold text-stone-950">
              {formatProjectionCount(pendingCount)}
            </div>
            <div className="mt-1 text-xs text-stone-500">
              {pendingCount == null ? "状态暂不可用" : "进入 Mailbox 确认"}
            </div>
          </div>
          <div className="p-4">
            <div className="text-xs font-medium text-stone-500">最近依据来源</div>
            <div className="mt-1 text-sm font-semibold text-stone-950">{recentSources}</div>
            <div className="mt-1 text-xs text-stone-500">不显示原始内容</div>
          </div>
        </div>
      </div>

      {candidateChanges.length > 0 && (
        <div className="rounded-lg border border-stone-200 bg-white">
          {candidateChanges.slice(0, 3).map((change, index) => (
            <div
              key={change.changeRef.id}
              className={[
                "flex flex-wrap items-center justify-between gap-3 px-4 py-3",
                index === 0 ? "" : "border-t border-stone-100",
              ].join(" ")}
            >
              <div>
                <div data-testid={`life-model-pending-proposal-primary-${change.changeRef.id}`}>
                  <div className="text-sm font-semibold text-stone-950">{change.title}</div>
                  <div className="mt-0.5 text-xs text-stone-500">
                    {change.changeKind} · {change.affectedDimensionIds.join(" / ")}
                  </div>
                  <div className="mt-1 text-xs text-stone-600">
                    OpenLife 发现一条候选更新，需要你在 Mailbox 中确认后才会写入。
                  </div>
                </div>
                <div className="mt-2 max-w-xl">
                  <TechnicalDetails summary="后端依据">
                    <div className="space-y-1">
                      <div className="min-w-0">
                        <span className="text-stone-400">确认记录：</span>
                        <span className="break-all">{change.changeRef.id}</span>
                      </div>
                      <div className="min-w-0">
                        <span className="text-stone-400">状态：</span>
                        <span>{change.decisionStatus}</span>
                      </div>
                      {change.evidenceRefs.map(ref => (
                        <div key={ref.id} className="min-w-0">
                          <span className="text-stone-400">{ref.label}：</span>
                          <span className="break-all">{ref.id}</span>
                        </div>
                      ))}
                    </div>
                  </TechnicalDetails>
                </div>
              </div>
              <Link
                to={mailboxRoute()}
                className="text-xs font-semibold text-stone-700 underline-offset-4 hover:underline"
              >
                去 Mailbox 处理
              </Link>
            </div>
          ))}
        </div>
      )}

      {materializedChanges.length > 0 && (
        <div className="rounded-lg border border-stone-200 bg-white">
          {materializedChanges.slice(0, 3).map((change, index) => (
            <div
              key={change.changeRef.id}
              className={[
                "flex flex-wrap items-center justify-between gap-3 px-4 py-3",
                index === 0 ? "" : "border-t border-stone-100",
              ].join(" ")}
            >
              <div>
                <div className="text-sm font-semibold text-stone-950">{change.title}</div>
                <div className="mt-0.5 text-xs text-stone-500">
                  {change.materializationStatus} ·{" "}
                  {change.materializedAt ? formatUpdatedAt(change.materializedAt) : "时间未记录"}
                </div>
              </div>
              <ProductStatusChip label="后端物化证据" tone="ready" />
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
        const [viewModelEnvelope, projection, unfinishedSessions] = await Promise.all([
          getLifeModelViewModel().catch(() => null),
          getLifeStateProjection().catch(() => null),
          builderListUnfinished().catch(() => []),
        ]);

        if (cancelled) return;
        setState({
          viewModelEnvelope,
          projection,
          unfinishedSessions,
          loading: false,
          error: viewModelEnvelope ? "" : "Life Model 状态读取失败：后端读模型不可用",
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

  const viewModel = dataFromEnvelope(state.viewModelEnvelope);
  const overall = readinessFromViewModel(viewModel);
  const safeMode = state.projection?.safeMode.active ?? false;
  const safeModeReason = state.projection?.safeMode.reason ?? "系统当前处于 Safe Mode。";
  const pendingCount = viewModel?.pendingUpdateCounts.pendingReview ?? null;
  const topStatus = topStatusLabel(state.viewModelEnvelope);

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
              <StatusChip
                label={pendingCount == null ? "待确认状态读取中" : `待确认 ${pendingCount}`}
              />
              <StatusChip label={formatUpdatedAt(state.viewModelEnvelope?.lastUpdatedAt)} />
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
              viewModel={viewModel}
              projection={state.projection}
              unfinishedSessions={state.unfinishedSessions}
            />
          )}
          {activeSection === "overview" && <OverviewSection viewModel={viewModel} />}
          {activeSection === "evidence" && <EvidenceSection viewModel={viewModel} />}
        </div>
      </div>
    </div>
  );
}
