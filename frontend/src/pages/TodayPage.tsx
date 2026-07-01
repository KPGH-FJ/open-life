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
  Lightbulb,
  ShieldCheck,
} from "lucide-react";
import type { DailyGoal } from "../types";
import {
  getDailyGoals,
  getSystemDiagnostics,
  listProposals,
  type AgentProposal,
  type SystemDiagnostics,
} from "../tauri";
import {
  inspectDailyGoalName,
  type DailyGoalDisplayGuard,
  type TodayCardType,
} from "../utils/dailyGoalDisplayGuard";
import {
  countPendingReviewProposals,
  REVIEW_PENDING_PROPOSAL_LIMIT,
} from "../utils/reviewPendingCount";
import { getSafeModeReason, isSafeMode } from "../utils/safeMode";
import { mailboxRoute, productRoutePath } from "../productShellContract";

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

type TodayClassifiedCardType = Extract<TodayCardType, "state_signal" | "suggestion" | "blocker">;

type TodayGoalCardView = {
  type: "goal";
  id: string;
  title: string;
  goal: DailyGoal;
  originalIndex: number;
};

type TodayClassifiedCardView = {
  type: TodayClassifiedCardType;
  id: string;
  title: string;
  detail: string;
  guard: DailyGoalDisplayGuard;
  originalIndex: number;
};

type TodayPendingProposalCardView = {
  type: "pending_proposal";
  id: string;
  title: string;
  count: number;
  href: string;
};

type TodayCardView = TodayGoalCardView | TodayClassifiedCardView | TodayPendingProposalCardView;

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

function todayCardId(type: TodayCardView["type"], originalIndex: number, title: string): string {
  return `${type}-${originalIndex}-${title}`;
}

function buildDailyGoalCards(goals: DailyGoal[]): TodayCardView[] {
  return goals.map((goal, originalIndex) => {
    const guard = inspectDailyGoalName(goal.name);
    if (guard.valid) {
      return {
        type: "goal",
        id: todayCardId("goal", originalIndex, goal.name),
        title: goal.name,
        goal,
        originalIndex,
      };
    }
    const cardType: Extract<TodayCardType, "state_signal" | "suggestion" | "blocker"> =
      guard.cardType === "state_signal" ||
      guard.cardType === "suggestion" ||
      guard.cardType === "blocker"
        ? guard.cardType
        : "blocker";

    return {
      type: cardType,
      id: todayCardId(cardType, originalIndex, goal.name),
      title: goal.name,
      detail: `${guard.reason ?? "这条内容不会作为今日目标。"} ${
        guard.recoveryAction ?? ""
      }`.trim(),
      guard,
      originalIndex,
    };
  });
}

function goalCards(cards: TodayCardView[]): TodayGoalCardView[] {
  return cards.filter((card): card is TodayGoalCardView => {
    return card.type === "goal";
  });
}

function classifiedCardsByType<T extends TodayClassifiedCardType>(
  cards: TodayCardView[],
  type: T
): Array<TodayClassifiedCardView & { type: T }> {
  return cards.filter((card): card is TodayClassifiedCardView & { type: T } => {
    return card.type === type;
  });
}

function choosePrimaryGoal(cards: TodayGoalCardView[]): DailyGoal | null {
  const selected = cards.find(card => !card.goal.done) ?? cards[0] ?? null;
  return selected?.goal ?? null;
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
          listProposals("pending", undefined, undefined, REVIEW_PENDING_PROPOSAL_LIMIT).catch(
            () => []
          ),
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
  const dailyGoalCards = useMemo(() => buildDailyGoalCards(state.dailyGoals), [state.dailyGoals]);
  const primaryGoal = useMemo(() => choosePrimaryGoal(goalCards(dailyGoalCards)), [dailyGoalCards]);
  const stateSignalCards = useMemo(
    () => classifiedCardsByType(dailyGoalCards, "state_signal"),
    [dailyGoalCards]
  );
  const suggestionCards = useMemo(
    () => classifiedCardsByType(dailyGoalCards, "suggestion"),
    [dailyGoalCards]
  );
  const blockerCards = useMemo(
    () => classifiedCardsByType(dailyGoalCards, "blocker"),
    [dailyGoalCards]
  );
  const pendingCount = countPendingReviewProposals(state.pendingProposals);
  const pendingReviewCard: TodayCardView = {
    type: "pending_proposal",
    id: "pending-review-proposals",
    title: "待确认入口",
    count: pendingCount,
    href: mailboxRoute(),
  };

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
              to={productRoutePath("Companion")}
              className="inline-flex h-9 items-center justify-center gap-2 rounded-md bg-stone-900 px-3 text-sm font-semibold text-white hover:bg-stone-800"
            >
              和 OpenLife 说一下现在的状态
              <ArrowRight size={15} aria-hidden="true" />
            </Link>
            <Link
              to={mailboxRoute()}
              className="inline-flex h-9 items-center justify-center gap-2 rounded-md border border-stone-300 bg-white px-3 text-sm font-semibold text-stone-800 hover:bg-stone-50"
            >
              查看待确认项
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

        <section
          data-testid="today-goal-section"
          className="rounded-lg border border-stone-200 bg-white"
        >
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
                to={productRoutePath("Companion")}
                className="mt-4 inline-flex h-9 items-center justify-center gap-2 rounded-md bg-stone-900 px-3 text-sm font-semibold text-white hover:bg-stone-800"
              >
                和 OpenLife 说一下现在的状态
                <ArrowRight size={15} aria-hidden="true" />
              </Link>
            </div>
          )}
        </section>

        {stateSignalCards.length > 0 && (
          <section
            aria-label="状态信号"
            data-testid="today-state-signals"
            className="rounded-lg border border-sky-200 bg-sky-50 px-4 py-4"
          >
            <div className="flex items-center gap-2 text-sm font-semibold text-sky-950">
              <Activity size={16} aria-hidden="true" />
              状态信号
            </div>
            <div className="mt-1 text-sm text-sky-800">
              这些是压力、精力、情绪或置信度等状态，不会生成目标、任务或下一步行动。
            </div>
            <div className="mt-3 grid gap-2">
              {stateSignalCards.slice(0, 3).map(card => (
                <div
                  key={card.id}
                  data-card-type="state_signal"
                  data-testid="today-card-state-signal"
                  className="rounded-md border border-sky-200 bg-white/80 px-3 py-2"
                >
                  <div className="text-sm font-medium text-sky-950">{card.title}</div>
                  <div className="mt-0.5 text-xs text-sky-800">{card.detail}</div>
                </div>
              ))}
            </div>
          </section>
        )}

        {suggestionCards.length > 0 && (
          <section
            aria-label="建议"
            data-testid="today-suggestions"
            className="rounded-lg border border-stone-200 bg-white px-4 py-4"
          >
            <div className="flex items-center gap-2 text-sm font-semibold text-stone-950">
              <Lightbulb size={16} aria-hidden="true" />
              建议
            </div>
            <div className="mt-1 text-sm text-stone-600">
              这些内容只是建议；确认前不会显示为今日目标。
            </div>
            <div className="mt-3 grid gap-2">
              {suggestionCards.slice(0, 3).map(card => (
                <div
                  key={card.id}
                  data-card-type="suggestion"
                  data-testid="today-card-suggestion"
                  className="rounded-md border border-stone-200 bg-stone-50 px-3 py-2"
                >
                  <div className="text-sm font-medium text-stone-950">{card.title}</div>
                  <div className="mt-0.5 text-xs text-stone-600">{card.detail}</div>
                </div>
              ))}
            </div>
          </section>
        )}

        {blockerCards.length > 0 && (
          <section
            aria-label="阻断"
            data-testid="today-blockers"
            className="rounded-lg border border-amber-200 bg-amber-50 px-4 py-4"
          >
            <div className="flex items-center gap-2 text-sm font-semibold text-amber-950">
              <AlertTriangle size={16} aria-hidden="true" />
              需要处理的阻断
            </div>
            <div className="mt-1 text-sm text-amber-800">
              这些内容不是用户目标；处理后再单独确认今天要推进的目标。
            </div>
            <div className="mt-3 grid gap-2">
              {blockerCards.slice(0, 3).map(card => (
                <div
                  key={card.id}
                  data-card-type="blocker"
                  data-testid="today-card-blocker"
                  className="rounded-md border border-amber-200 bg-white/75 px-3 py-2"
                >
                  <div className="text-sm font-medium text-amber-950">{card.title}</div>
                  <div className="mt-0.5 text-xs text-amber-800">{card.detail}</div>
                </div>
              ))}
            </div>
          </section>
        )}

        <section
          data-testid="today-next-step"
          className="rounded-lg border border-stone-200 bg-white px-4 py-4"
        >
          <div className="text-sm font-semibold text-stone-950">下一步</div>
          <div className="mt-2 text-base text-stone-800">{nextStepFor(primaryGoal)}</div>
          {!primaryGoal && (
            <div className="mt-3">
              <Link
                to={productRoutePath("Companion")}
                className="text-sm font-semibold text-stone-700 underline-offset-4 hover:underline"
              >
                和 OpenLife 说一下现在的状态
              </Link>
            </div>
          )}
        </section>

        <section
          data-card-type="pending_proposal"
          data-testid="today-card-pending-proposal"
          className="rounded-lg border border-stone-200 bg-white px-4 py-4"
        >
          <div className="flex flex-wrap items-center justify-between gap-3">
            <div>
              <div className="text-sm font-semibold text-stone-950">{pendingReviewCard.title}</div>
              <div className="mt-1 text-sm text-stone-600">
                {pendingReviewCard.count > 0
                  ? `${pendingReviewCard.count} 个待确认项需要你处理。`
                  : "现在没有需要处理的待确认项。"}
              </div>
            </div>
            <Link
              to={pendingReviewCard.href}
              className="inline-flex h-9 items-center justify-center gap-2 rounded-md border border-stone-300 bg-white px-3 text-sm font-semibold text-stone-800 hover:bg-stone-50"
            >
              查看待确认项
              <Inbox size={15} aria-hidden="true" />
            </Link>
          </div>
        </section>
      </div>
    </div>
  );
}
