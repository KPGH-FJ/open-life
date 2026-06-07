import { useEffect, useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { ArrowRight, CalendarDays, CheckCircle2, Clock3, Inbox, ShieldCheck } from "lucide-react";
import type { DailyGoal, StateAlert } from "../types";
import {
  countMemoryChunks,
  getDailyGoals,
  getPendingProposals,
  getStateAlerts,
  getSystemDiagnostics,
  type AgentProposal,
  type SystemDiagnostics,
} from "../tauri";
import { getSafeModeReason, isSafeMode } from "../utils/safeMode";

type TodayPageState = {
  diagnostics: SystemDiagnostics | null;
  dailyGoals: DailyGoal[];
  stateAlerts: StateAlert[];
  memoryCount: number | null;
  pendingProposals: AgentProposal[];
  loading: boolean;
  error: string;
};

const INITIAL_STATE: TodayPageState = {
  diagnostics: null,
  dailyGoals: [],
  stateAlerts: [],
  memoryCount: null,
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

function chatStatusLabel(diagnostics: SystemDiagnostics | null): string {
  if (!diagnostics) return "状态读取中";
  return diagnostics.chat_ready ? "对话就绪" : "对话待修复";
}

function modelRouteLabel(diagnostics: SystemDiagnostics | null): string {
  if (!diagnostics) return "状态读取中";
  if (diagnostics.prefer_local_model) return "本地优先";
  if (diagnostics.cloud_api_configured || diagnostics.cloud_api_validated) return "云端可用";
  return "对话待配置";
}

export default function TodayPage() {
  const [state, setState] = useState<TodayPageState>(INITIAL_STATE);

  useEffect(() => {
    let cancelled = false;

    async function loadToday() {
      setState(current => ({ ...current, loading: true, error: "" }));
      try {
        const [diagnostics, dailyGoals, stateAlerts, memoryCount, pendingProposals] =
          await Promise.all([
            getSystemDiagnostics().catch(() => null),
            getDailyGoals().catch(() => []),
            getStateAlerts().catch(() => []),
            countMemoryChunks().catch(() => null),
            getPendingProposals(10).catch(() => []),
          ]);

        if (cancelled) return;
        setState({
          diagnostics,
          dailyGoals,
          stateAlerts,
          memoryCount,
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
  const completedCount = state.dailyGoals.filter(goal => goal.done).length;
  const pendingCount =
    state.pendingProposals.length || state.diagnostics?.pending_proposal_count || 0;
  const memoryCount = state.memoryCount ?? state.diagnostics?.memory_chunk_count ?? 0;
  const mainAlert = state.stateAlerts[0] ?? null;

  return (
    <div data-testid="today-page" className="h-full overflow-auto bg-[#f5f6f2]">
      <div className="mx-auto flex max-w-4xl flex-col gap-5 px-4 py-5 lg:px-6">
        <header className="flex flex-wrap items-start justify-between gap-4">
          <div className="min-w-0">
            <div className="flex items-center gap-2">
              <CalendarDays size={20} aria-hidden="true" className="text-stone-700" />
              <h1 className="text-xl font-semibold tracking-normal text-stone-950">今日</h1>
            </div>
            <div className="mt-3 flex flex-wrap gap-2">
              <StatusChip label={modelRouteLabel(state.diagnostics)} />
              <StatusChip label={`待确认 ${pendingCount}`} />
              {safeMode ? (
                <StatusChip label="Safe Mode" tone="warn" />
              ) : (
                <StatusChip label={chatStatusLabel(state.diagnostics)} />
              )}
            </div>
          </div>
          <div className="flex flex-wrap gap-2">
            <Link
              to="/companion"
              className="inline-flex h-9 items-center justify-center gap-2 rounded-md bg-stone-900 px-3 text-sm font-semibold text-white hover:bg-stone-800"
            >
              去陪伴
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
              <h2 className="text-sm font-semibold text-stone-950">今天还没有目标</h2>
              <p className="mx-auto mt-1 max-w-md text-sm text-stone-600">
                可以先从一次对话开始，把今天最小的一步定下来。
              </p>
              <Link
                to="/companion"
                className="mt-4 inline-flex h-9 items-center justify-center gap-2 rounded-md bg-stone-900 px-3 text-sm font-semibold text-white hover:bg-stone-800"
              >
                去陪伴
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
                去陪伴
              </Link>
            </div>
          )}
        </section>

        <section className="rounded-lg border border-stone-200 bg-white">
          <div className="border-b border-stone-100 px-4 py-3">
            <h2 className="text-sm font-semibold text-stone-950">轻量状态</h2>
          </div>
          <div className="divide-y divide-stone-100">
            <div className="grid gap-1 px-4 py-3 sm:grid-cols-[180px_1fr]">
              <div className="text-xs font-medium text-stone-500">每日目标</div>
              <div className="text-sm text-stone-800">
                {completedCount}/{state.dailyGoals.length} 已完成
              </div>
            </div>
            <div className="grid gap-1 px-4 py-3 sm:grid-cols-[180px_1fr]">
              <div className="text-xs font-medium text-stone-500">待确认</div>
              <div className="flex flex-wrap items-center gap-2 text-sm text-stone-800">
                <span>{pendingCount} 项</span>
                {pendingCount > 0 && (
                  <Link
                    to="/mailbox"
                    className="font-semibold text-stone-700 underline-offset-4 hover:underline"
                  >
                    查看邮箱
                  </Link>
                )}
              </div>
            </div>
            <div className="grid gap-1 px-4 py-3 sm:grid-cols-[180px_1fr]">
              <div className="text-xs font-medium text-stone-500">Life Model</div>
              <div className="text-sm text-stone-800">
                {state.diagnostics?.life_model_ready ? "可用" : "待构建"}
              </div>
            </div>
            <div className="grid gap-1 px-4 py-3 sm:grid-cols-[180px_1fr]">
              <div className="text-xs font-medium text-stone-500">记忆</div>
              <div className="text-sm text-stone-800">{memoryCount} 条</div>
            </div>
            {mainAlert && (
              <div className="grid gap-1 px-4 py-3 sm:grid-cols-[180px_1fr]">
                <div className="text-xs font-medium text-stone-500">状态提醒</div>
                <div className="text-sm text-stone-800">{mainAlert.message}</div>
              </div>
            )}
          </div>
        </section>

        <div className="flex flex-wrap gap-2 pb-8">
          <Link
            to="/workspace"
            className="inline-flex h-9 items-center justify-center rounded-md border border-stone-300 bg-white px-3 text-sm font-semibold text-stone-800 hover:bg-stone-50"
          >
            旧工作台
          </Link>
          <Link
            to="/life-model"
            className="inline-flex h-9 items-center justify-center rounded-md border border-stone-300 bg-white px-3 text-sm font-semibold text-stone-800 hover:bg-stone-50"
          >
            Life Model
          </Link>
        </div>
      </div>
    </div>
  );
}
