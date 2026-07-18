import { useEffect, useMemo, useState } from "react";
import { Link } from "react-router-dom";
import {
  Activity,
  AlertTriangle,
  ArrowRight,
  CalendarDays,
  CheckCircle2,
  Clock3,
  Inbox,
  RefreshCw,
  ShieldCheck,
  SlidersHorizontal,
} from "lucide-react";
import {
  getDailyGoals,
  getLifeStateProjection,
  getProviderPrivacyBoundarySummary,
  type LifeStateProjection,
  type ProviderPrivacyBoundarySummary,
} from "../tauri";
import type { DailyGoal } from "../types";
import { mailboxRoute, productRoutePath } from "../productShellContract";
import type {
  DebugAction,
  EvidenceRef,
  ProductAction,
  ViewModelStatus,
  ViewModelWarning,
} from "../viewmodels/shared/viewModelEnvelope";
import type { TodayViewModelEnvelope } from "../viewmodels/today/todayViewModel";
import { buildTodayViewModelEnvelope } from "../viewmodels/today/todayViewModelAdapter";

type TodayV2PreviewLoadState = {
  projection: LifeStateProjection | null;
  providerPrivacyBoundary: ProviderPrivacyBoundarySummary | null;
  dailyGoals: DailyGoal[];
  loading: boolean;
  error: string;
};

const INITIAL_STATE: TodayV2PreviewLoadState = {
  projection: null,
  providerPrivacyBoundary: null,
  dailyGoals: [],
  loading: true,
  error: "",
};

function statusTone(status: ViewModelStatus): "neutral" | "good" | "warn" | "danger" {
  switch (status) {
    case "ready":
      return "good";
    case "stale":
      return "warn";
    case "error":
      return "danger";
    default:
      return "neutral";
  }
}

function chipClass(tone: "neutral" | "good" | "warn" | "danger"): string {
  const base = "inline-flex h-7 items-center rounded-md border px-2.5 text-xs font-semibold";
  switch (tone) {
    case "good":
      return `${base} border-emerald-200 bg-emerald-50 text-emerald-800`;
    case "warn":
      return `${base} border-amber-200 bg-amber-50 text-amber-900`;
    case "danger":
      return `${base} border-red-200 bg-red-50 text-red-800`;
    default:
      return `${base} border-stone-200 bg-white text-stone-700`;
  }
}

function formatTimestamp(value: string | null): string {
  if (!value) return "unknown";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString("zh-CN", {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function actionLabel(action: ProductAction): string {
  switch (action.id) {
    case "today.refresh":
      return "刷新今日状态";
    case "today.open_current_workspace_route":
      return "打开当前工作入口";
    case "today.open_current_review_route":
      return "查看待确认入口";
    default:
      return action.label;
  }
}

function actionRoute(action: ProductAction): string | null {
  switch (action.targetRef) {
    case "route:companion":
      return productRoutePath("Companion");
    case "route:mailbox":
      return mailboxRoute();
    case "today":
      return "/today-v2-preview";
    default:
      return null;
  }
}

function PrimaryActionControl({ action }: { action: ProductAction }) {
  const label = actionLabel(action);
  const route = actionRoute(action);
  const className = [
    "inline-flex h-9 items-center justify-center gap-2 rounded-md px-3 text-sm font-semibold",
    action.enabled
      ? "border border-stone-300 bg-white text-stone-800 hover:bg-stone-50"
      : "cursor-not-allowed border border-stone-200 bg-stone-100 text-stone-400",
  ].join(" ");
  const icon =
    action.kind === "refresh" ? (
      <RefreshCw size={15} aria-hidden="true" />
    ) : action.targetRef === "route:mailbox" ? (
      <Inbox size={15} aria-hidden="true" />
    ) : (
      <ArrowRight size={15} aria-hidden="true" />
    );

  if (!action.enabled) {
    return (
      <button type="button" disabled title={action.disabledReason} className={className}>
        {label}
        {icon}
      </button>
    );
  }

  if (route) {
    return (
      <Link to={route} className={className}>
        {label}
        {icon}
      </Link>
    );
  }

  return (
    <button type="button" className={className}>
      {label}
      {icon}
    </button>
  );
}

function WarningList({ warnings }: { warnings: ViewModelWarning[] }) {
  if (warnings.length === 0) return null;
  return (
    <section className="rounded-lg border border-stone-200 bg-white px-4 py-4">
      <div className="flex items-center gap-2 text-sm font-semibold text-stone-950">
        <AlertTriangle size={16} aria-hidden="true" />
        限制与未知状态
      </div>
      <div className="mt-3 grid gap-2">
        {warnings.map(warning => (
          <div
            key={warning.code}
            className={[
              "rounded-md border px-3 py-2 text-sm",
              warning.severity === "error"
                ? "border-red-200 bg-red-50 text-red-800"
                : warning.severity === "warning"
                  ? "border-amber-200 bg-amber-50 text-amber-900"
                  : "border-stone-200 bg-stone-50 text-stone-700",
            ].join(" ")}
          >
            <div className="font-semibold">{warning.code}</div>
            <div className="mt-0.5">{warning.message}</div>
          </div>
        ))}
      </div>
    </section>
  );
}

function EvidenceList({ evidenceRefs }: { evidenceRefs: EvidenceRef[] }) {
  if (evidenceRefs.length === 0) {
    return <div className="text-sm text-stone-500">没有可展示的证据引用。</div>;
  }
  return (
    <ul className="grid gap-2">
      {evidenceRefs.map(ref => (
        <li key={ref.id} className="rounded-md border border-stone-200 bg-white px-3 py-2">
          <div className="text-sm font-medium text-stone-900">{ref.label}</div>
          <div className="mt-0.5 text-xs text-stone-500">
            {ref.source}
            {ref.sensitivity ? ` · ${ref.sensitivity}` : ""}
          </div>
        </li>
      ))}
    </ul>
  );
}

function DebugActionList({ actions }: { actions: DebugAction[] }) {
  if (actions.length === 0) {
    return <div className="text-sm text-stone-500">没有调试动作。</div>;
  }
  return (
    <ul className="grid gap-2">
      {actions.map(action => (
        <li key={action.id} className="rounded-md border border-stone-200 bg-white px-3 py-2">
          <div className="text-sm font-medium text-stone-900">{action.label}</div>
          <div className="mt-0.5 text-xs text-stone-500">
            {action.kind}
            {action.developerOnly ? " · developer only" : ""}
            {action.enabled ? "" : " · disabled"}
          </div>
        </li>
      ))}
    </ul>
  );
}

export function TodayV2PreviewSurface({ envelope }: { envelope: TodayViewModelEnvelope }) {
  const data = envelope.data;
  const warnings = envelope.warnings ?? [];
  const evidenceRefs = envelope.evidenceRefs ?? [];
  const debugActions = envelope.actions.debugOnly ?? [];

  return (
    <div
      data-testid="today-v2-preview-page"
      className="h-full overflow-auto overflow-x-hidden bg-[#f5f6f2]"
    >
      <div className="mx-auto flex w-full max-w-[1500px] flex-col gap-5 px-4 py-5 lg:px-6">
        <header className="flex flex-wrap items-start justify-between gap-4">
          <div className="min-w-0">
            <div className="flex items-center gap-2">
              <CalendarDays size={20} aria-hidden="true" className="text-stone-700" />
              <h1 className="text-xl font-semibold tracking-normal text-stone-950">今日</h1>
              <span className="rounded-md border border-stone-200 bg-white px-2 py-1 text-xs font-semibold text-stone-600">
                V2 预览
              </span>
            </div>
            <div className="mt-3 flex flex-wrap gap-2">
              <span className={chipClass(statusTone(envelope.status))}>{envelope.status}</span>
              <span className={chipClass("neutral")}>
                更新 {formatTimestamp(envelope.lastUpdatedAt)}
              </span>
              {data?.safeMode.active && <span className={chipClass("warn")}>Safe Mode</span>}
            </div>
          </div>
          <div data-testid="today-v2-primary-actions" className="flex flex-wrap justify-end gap-2">
            {envelope.actions.primary.map(action => (
              <PrimaryActionControl key={action.id} action={action} />
            ))}
          </div>
        </header>

        <section className="rounded-lg border border-stone-200 bg-white px-4 py-4">
          {data ? (
            <div className="grid gap-4 lg:grid-cols-[1fr_auto] lg:items-start">
              <div>
                <div className="text-sm font-semibold text-stone-500">今日摘要</div>
                <h2 className="mt-1 text-lg font-semibold text-stone-950">
                  {data.dailyStateSummary.headline}
                </h2>
                <p className="mt-2 max-w-3xl text-sm leading-6 text-stone-700">
                  {data.dailyStateSummary.summary}
                </p>
              </div>
              <div className="flex flex-wrap gap-2 lg:justify-end">
                <span className={chipClass("neutral")}>
                  readiness {data.dailyStateSummary.readiness}
                </span>
                <span className={chipClass(data.pendingReviewCount > 0 ? "warn" : "good")}>
                  待确认 {data.pendingReviewCount}
                </span>
              </div>
            </div>
          ) : (
            <div>
              <div className="text-sm font-semibold text-stone-500">今日摘要</div>
              <h2 className="mt-1 text-lg font-semibold text-stone-950">
                {envelope.status === "loading" ? "读取 TodayViewModel" : "TodayViewModel 不可用"}
              </h2>
              <p className="mt-2 text-sm leading-6 text-stone-700">
                {warnings[0]?.message ?? "当前没有可渲染的数据。"}
              </p>
            </div>
          )}
        </section>

        {envelope.status === "stale" && (
          <section className="rounded-lg border border-amber-200 bg-amber-50 px-4 py-3 text-sm text-amber-900">
            <div className="flex items-start gap-3">
              <Clock3 size={17} aria-hidden="true" className="mt-0.5 shrink-0" />
              <div>
                <div className="font-semibold">stale</div>
                <div className="mt-0.5 text-xs text-amber-800">
                  需要刷新后再使用对时效敏感的动作。
                </div>
              </div>
            </div>
          </section>
        )}

        {data?.safeMode.active && (
          <section className="rounded-lg border border-amber-200 bg-amber-50 px-4 py-3 text-sm text-amber-900">
            <div className="flex items-start gap-3">
              <ShieldCheck size={17} aria-hidden="true" className="mt-0.5 shrink-0" />
              <div>
                <div className="font-semibold">Safe Mode（安全模式）</div>
                <div className="mt-0.5 text-xs text-amber-800">
                  {data.safeMode.reason ?? "LifeStateProjection 报告 Safe Mode。"}
                </div>
                <div className="mt-1 text-xs text-amber-800">
                  外部动作 {data.safeMode.blocksExternalActions ? "已阻断" : "未阻断"}；长期写入{" "}
                  {data.safeMode.blocksDurableWrites ? "已阻断" : "未阻断"}。
                </div>
              </div>
            </div>
          </section>
        )}

        {data && (
          <div className="grid gap-5 lg:grid-cols-[minmax(0,1.35fr)_minmax(320px,0.65fr)]">
            <section className="rounded-lg border border-stone-200 bg-white">
              <div className="border-b border-stone-100 px-4 py-3">
                <h2 className="text-sm font-semibold text-stone-950">主要目标</h2>
              </div>
              {data.primaryDailyGoal ? (
                <div className="grid gap-4 px-4 py-5 sm:grid-cols-[1fr_auto] sm:items-center">
                  <div>
                    <div className="text-lg font-semibold text-stone-950">
                      {data.primaryDailyGoal.title}
                    </div>
                    <div className="mt-1 text-sm text-stone-500">
                      classification {data.primaryDailyGoal.backendClassification}
                    </div>
                  </div>
                  <div className="inline-flex items-center gap-2 rounded-md border border-stone-200 px-3 py-2 text-sm text-stone-700">
                    <CheckCircle2
                      size={16}
                      aria-hidden="true"
                      className={
                        data.primaryDailyGoal.status === "done"
                          ? "text-emerald-700"
                          : "text-stone-400"
                      }
                    />
                    {data.primaryDailyGoal.status}
                  </div>
                </div>
              ) : (
                <div className="px-4 py-8 text-center">
                  <h2 className="text-sm font-semibold text-stone-950">没有当前目标</h2>
                  <p className="mx-auto mt-1 max-w-md text-sm text-stone-600">
                    ViewModel 没有返回主要目标。
                  </p>
                </div>
              )}
            </section>

            <section className="rounded-lg border border-stone-200 bg-white px-4 py-4">
              <div className="flex items-center gap-2 text-sm font-semibold text-stone-950">
                <Activity size={16} aria-hidden="true" />
                任务压力
              </div>
              <dl className="mt-3 grid grid-cols-2 gap-2 text-sm">
                <div className="rounded-md border border-stone-200 bg-stone-50 px-3 py-2">
                  <dt className="text-xs text-stone-500">active</dt>
                  <dd className="mt-1 font-semibold text-stone-950">
                    {data.currentTaskPressure.activeCount}
                  </dd>
                </div>
                <div className="rounded-md border border-stone-200 bg-stone-50 px-3 py-2">
                  <dt className="text-xs text-stone-500">permission</dt>
                  <dd className="mt-1 font-semibold text-stone-950">
                    {data.currentTaskPressure.waitingPermissionCount}
                  </dd>
                </div>
                <div className="rounded-md border border-stone-200 bg-stone-50 px-3 py-2">
                  <dt className="text-xs text-stone-500">blocked</dt>
                  <dd className="mt-1 font-semibold text-stone-950">
                    {data.currentTaskPressure.blockedCount}
                  </dd>
                </div>
                <div className="rounded-md border border-stone-200 bg-stone-50 px-3 py-2">
                  <dt className="text-xs text-stone-500">risk</dt>
                  <dd className="mt-1 font-semibold text-stone-950">
                    {data.currentTaskPressure.highestRisk}
                  </dd>
                </div>
              </dl>
            </section>
          </div>
        )}

        {data && (
          <section className="rounded-lg border border-stone-200 bg-white px-4 py-4">
            <div className="text-sm font-semibold text-stone-950">下一步</div>
            <div className="mt-2 text-base text-stone-800">
              {data.nextRecommendedAction
                ? actionLabel(data.nextRecommendedAction)
                : "下一步暂未生成"}
            </div>
          </section>
        )}

        {data && data.blockers.length > 0 && (
          <section className="rounded-lg border border-amber-200 bg-amber-50 px-4 py-4">
            <div className="flex items-center gap-2 text-sm font-semibold text-amber-950">
              <AlertTriangle size={16} aria-hidden="true" />
              阻断
            </div>
            <div className="mt-3 grid gap-2">
              {data.blockers.map(blocker => (
                <div
                  key={blocker.id}
                  className="rounded-md border border-amber-200 bg-white px-3 py-2"
                >
                  <div className="text-sm font-medium text-amber-950">{blocker.title}</div>
                  <div className="mt-0.5 text-xs text-amber-800">{blocker.category}</div>
                </div>
              ))}
            </div>
          </section>
        )}

        <WarningList warnings={warnings} />

        <details
          data-testid="today-v2-advanced-lane"
          className="rounded-lg border border-stone-200 bg-white px-4 py-3"
        >
          <summary className="flex cursor-pointer list-none items-center gap-2 text-sm font-semibold text-stone-950">
            <SlidersHorizontal size={16} aria-hidden="true" />
            高级证据
          </summary>
          <div className="mt-4 grid gap-4 lg:grid-cols-2">
            <div>
              <div className="mb-2 text-xs font-semibold uppercase tracking-normal text-stone-500">
                Evidence
              </div>
              <EvidenceList evidenceRefs={evidenceRefs} />
            </div>
            <div>
              <div className="mb-2 text-xs font-semibold uppercase tracking-normal text-stone-500">
                Debug only
              </div>
              <DebugActionList actions={debugActions} />
            </div>
          </div>
        </details>
      </div>
    </div>
  );
}

export default function TodayV2PreviewPage() {
  const [state, setState] = useState<TodayV2PreviewLoadState>(INITIAL_STATE);

  useEffect(() => {
    let cancelled = false;

    async function load() {
      setState(current => ({ ...current, loading: true, error: "" }));

      const [projection, providerBoundaryEnvelope, dailyGoals] = await Promise.all([
        getLifeStateProjection().catch(() => null),
        getProviderPrivacyBoundarySummary().catch(() => null),
        getDailyGoals().catch(() => []),
      ]);

      if (cancelled) return;
      setState({
        projection,
        providerPrivacyBoundary: providerBoundaryEnvelope?.data ?? null,
        dailyGoals,
        loading: false,
        error: projection ? "" : "LifeStateProjection failed to load.",
      });
    }

    load();

    return () => {
      cancelled = true;
    };
  }, []);

  const envelope = useMemo(() => {
    if (state.loading) {
      return buildTodayViewModelEnvelope({
        projection: state.projection,
        dailyGoals: state.dailyGoals,
        providerPrivacyBoundary: state.providerPrivacyBoundary,
        status: "loading",
      });
    }

    if (state.error || !state.projection) {
      return buildTodayViewModelEnvelope({
        projection: state.projection,
        dailyGoals: state.dailyGoals,
        providerPrivacyBoundary: state.providerPrivacyBoundary,
        status: "error",
        errorMessage: state.error || "LifeStateProjection failed to load.",
      });
    }

    return buildTodayViewModelEnvelope({
      projection: state.projection,
      dailyGoals: state.dailyGoals,
      providerPrivacyBoundary: state.providerPrivacyBoundary,
    });
  }, [state]);

  return <TodayV2PreviewSurface envelope={envelope} />;
}
