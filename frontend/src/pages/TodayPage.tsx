import { useEffect, useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { ArrowRight, CalendarDays, CheckCircle2, Clock3, Inbox, ShieldCheck } from "lucide-react";
import type { DailyGoal } from "../types";
import {
  getDailyGoals,
  getPendingProposals,
  getSystemDiagnostics,
  type AgentProposal,
  type SystemDiagnostics,
} from "../tauri";
import { getSafeModeReason, isSafeMode } from "../utils/safeMode";

type TodayPageState = {
  diagnostics: SystemDiagnostics | null;
  dailyGoals: DailyGoal[];
  pendingProposals: AgentProposal[];
  loading: boolean;
  error: string;
};

const INITIAL_STATE: TodayPageState = {
  diagnostics: null,
  dailyGoals: [],
  pendingProposals: [],
  loading: true,
  error: "",
};

function StatusChip({ label, tone = "neutral" }: { label: string; tone?: "neutral" | "warn" }) {
  return (
    <span
      className={[
        "inline-flex h-7 items-center rounded-md border px-2.5 text-xs font-medium",
        tone === "warn"
          ? "border-amber-200 bg-amber-50 text-amber-900"
          : "border-stone-200 bg-white text-stone-700",
      ].join(" ")}
    >
      {label}
    </span>
  );
}

function choosePrimaryGoal(goals: DailyGoal[]): DailyGoal | null {
  return goals.find(goal => !goal.done) ?? goals[0] ?? null;
}

function formatTimeBlock(goal: DailyGoal | null): string {
  if (!goal?.time_block) return "未设置时间";
  return `${goal.time_block.start}-${goal.time_block.end}`;
}

function nextStepFor(goal: DailyGoal | null): string {
  if (!goal) return "还没有下一步。";
  if (goal.done) return `「${goal.name}」已完成，今天先保持当前节奏。`;
  return `从「${goal.name}」开始，先做 10 分钟。`;
}

export default function TodayPage() {
  const [state, setState] = useState<TodayPageState>(INITIAL_STATE);

  useEffect(() => {
    let cancelled = false;

    async function loadToday() {
      setState(current => ({ ...current, loading: true, error: "" }));
      try {
        const [diagnostics, dailyGoals, pendingProposals] = await Promise.all([
          getSystemDiagnostics().catch(() => null),
          getDailyGoals().catch(() => []),
          getPendingProposals(10).catch(() => []),
        ]);

        if (cancelled) return;
        setState({
          diagnostics,
          dailyGoals,
          pendingProposals,
          loading: false,
          error: "",
        });
      } catch (error) {
        if (cancelled) return;
        setState(current => ({
          ...current,
          loading: false,
          error: `今日状态读取失败：${String(error)}`,
        }));
      }
    }

    loadToday();

    return () => {
      cancelled = true;
    };
  }, []);

  const safeMode = isSafeMode(state.diagnostics);
  const safeModeReason = getSafeModeReason(state.diagnostics);
  const primaryGoal = useMemo(() => choosePrimaryGoal(state.dailyGoals), [state.dailyGoals]);
  const pendingCount =
    state.pendingProposals.length || state.diagnostics?.pending_proposal_count || 0;

  return (
    <div data-testid="today-page" className="h-full overflow-auto overflow-x-hidden bg-[#f5f6f2]">
      <div className="mx-auto flex w-full max-w-[1500px] flex-col gap-5 px-4 py-5 lg:px-6">
        <header className="flex flex-wrap items-start justify-between gap-4">
          <div className="min-w-0">
            <div className="flex items-center gap-2">
              <CalendarDays size={20} aria-hidden="true" className="text-stone-700" />
              <h1 className="text-xl font-semibold tracking-normal text-stone-950">今日</h1>
            </div>
            <div className="mt-3 flex flex-wrap gap-2">
              <StatusChip label={`待确认 ${pendingCount}`} />
              {safeMode && <StatusChip label="Safe Mode" tone="warn" />}
            </div>
          </div>
          <div className="flex flex-wrap gap-2">
            <Link
              to="/companion"
              className="inline-flex h-9 items-center justify-center gap-2 rounded-md bg-stone-900 px-3 text-sm font-semibold text-white hover:bg-stone-800"
            >
              和 OpenLife 说一下现在的状态
              <ArrowRight size={15} aria-hidden="true" />
            </Link>
            <Link
              to="/mailbox"
              className="inline-flex h-9 items-center justify-center gap-2 rounded-md border border-stone-300 bg-white px-3 text-sm font-semibold text-stone-800 hover:bg-stone-50"
            >
              查看邮箱
              <Inbox size={15} aria-hidden="true" />
            </Link>
          </div>
        </header>

        {safeMode && (
          <div className="flex flex-wrap items-start gap-3 rounded-lg border border-amber-200 bg-amber-50 px-4 py-3 text-sm text-amber-900">
            <ShieldCheck size={17} aria-hidden="true" className="mt-0.5 shrink-0" />
            <div>
              <div className="font-semibold">Safe Mode</div>
              <div className="mt-0.5 text-xs text-amber-800">{safeModeReason}</div>
            </div>
          </div>
        )}

        {state.error && (
          <div className="rounded-lg border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-800">
            {state.error}
          </div>
        )}

        {state.loading && (
          <div className="inline-flex items-center gap-2 text-xs font-medium text-stone-500">
            <Clock3 size={14} aria-hidden="true" />
            读取中
          </div>
        )}

        <section className="rounded-lg border border-stone-200 bg-white">
          <div className="border-b border-stone-100 px-4 py-3">
            <h2 className="text-sm font-semibold text-stone-950">今日目标</h2>
          </div>
          {primaryGoal ? (
            <div className="grid gap-4 px-4 py-5 sm:grid-cols-[1fr_auto] sm:items-center">
              <div>
                <div className="text-lg font-semibold text-stone-950">{primaryGoal.name}</div>
                <div className="mt-1 text-sm text-stone-500">{formatTimeBlock(primaryGoal)}</div>
              </div>
              <div className="inline-flex items-center gap-2 rounded-md border border-stone-200 px-3 py-2 text-sm text-stone-700">
                <CheckCircle2
                  size={16}
                  aria-hidden="true"
                  className={primaryGoal.done ? "text-emerald-700" : "text-stone-400"}
                />
                {primaryGoal.done ? "已完成" : "待推进"}
              </div>
            </div>
          ) : (
            <div className="px-4 py-8 text-center">
              <h2 className="text-sm font-semibold text-stone-950">今天还没有定下来</h2>
              <p className="mx-auto mt-1 max-w-md text-sm text-stone-600">
                可以先从一次对话开始，把今天最小的一步定下来。
              </p>
              <Link
                to="/companion"
                className="mt-4 inline-flex h-9 items-center justify-center gap-2 rounded-md bg-stone-900 px-3 text-sm font-semibold text-white hover:bg-stone-800"
              >
                和 OpenLife 说一下现在的状态
                <ArrowRight size={15} aria-hidden="true" />
              </Link>
            </div>
          )}
        </section>

        <section className="rounded-lg border border-stone-200 bg-white px-4 py-4">
          <div className="text-sm font-semibold text-stone-950">下一步</div>
          <div className="mt-2 text-base text-stone-800">{nextStepFor(primaryGoal)}</div>
          {!primaryGoal && (
            <div className="mt-3">
              <Link
                to="/companion"
                className="text-sm font-semibold text-stone-700 underline-offset-4 hover:underline"
              >
                和 OpenLife 说一下现在的状态
              </Link>
            </div>
          )}
        </section>

        <section className="rounded-lg border border-stone-200 bg-white px-4 py-4">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <div>
              <div className="text-sm font-semibold text-stone-950">待确认入口</div>
              <div className="mt-1 text-sm text-stone-600">
                {pendingCount > 0 ? `${pendingCount} 封信等你回复。` : "现在没有需要处理的信。"}
              </div>
            </div>
            <Link
              to="/mailbox"
              className="inline-flex h-9 items-center justify-center gap-2 rounded-md border border-stone-300 bg-white px-3 text-sm font-semibold text-stone-800 hover:bg-stone-50"
            >
              查看邮箱
              <Inbox size={15} aria-hidden="true" />
            </Link>
          </div>
        </section>
      </div>
    </div>
  );
}
