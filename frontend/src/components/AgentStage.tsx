import { useId } from "react";
import type { LucideIcon } from "lucide-react";
import {
  AlertCircle,
  Brain,
  CalendarDays,
  Inbox,
  ListChecks,
  MessageSquare,
  ShieldCheck,
  Sparkles,
} from "lucide-react";

export const AGENT_STAGE_STATES = [
  "idle",
  "listening",
  "sorting",
  "memory",
  "planning",
  "review",
  "privacy",
  "error",
] as const;

export type AgentStageState = (typeof AGENT_STAGE_STATES)[number];

export type AgentStageProps = {
  state: AgentStageState;
  title?: string;
  status?: string;
  compact?: boolean;
};

type AgentStageConfig = {
  title: string;
  status: string;
  Icon: LucideIcon;
  shell: string;
  horizon: string;
  floor: string;
  actor: string;
  signal: string;
  rail: string;
  badge: string;
  motion: string;
};

const REDUCED_MOTION_CLASSES = "motion-reduce:animate-none motion-reduce:transition-none";

const STATE_CONFIG: Record<AgentStageState, AgentStageConfig> = {
  idle: {
    title: "在场",
    status: "安静待命，保持陪伴。",
    Icon: Sparkles,
    shell: "border-stone-200 bg-[#f7f6ef] text-stone-950",
    horizon: "bg-stone-300/70",
    floor: "border-stone-300/80 bg-stone-100/60",
    actor: "border-stone-800 bg-stone-950 text-white",
    signal: "bg-emerald-500",
    rail: "border-stone-300",
    badge: "bg-stone-900 text-white",
    motion: "",
  },
  listening: {
    title: "正在听",
    status: "接收你的当前想法，先不急着判断。",
    Icon: MessageSquare,
    shell: "border-emerald-200 bg-[#f3faf6] text-stone-950",
    horizon: "bg-emerald-300/70",
    floor: "border-emerald-300/80 bg-emerald-50/80",
    actor: "border-emerald-900 bg-emerald-950 text-emerald-50",
    signal: "bg-cyan-500",
    rail: "border-emerald-300",
    badge: "bg-emerald-900 text-emerald-50",
    motion: "motion-safe:animate-pulse",
  },
  sorting: {
    title: "整理中",
    status: "把线索排成可处理的顺序。",
    Icon: ListChecks,
    shell: "border-amber-200 bg-[#fbf7ed] text-stone-950",
    horizon: "bg-amber-300/70",
    floor: "border-amber-300/80 bg-amber-50/80",
    actor: "border-amber-800 bg-amber-500 text-stone-950",
    signal: "bg-stone-800",
    rail: "border-amber-300",
    badge: "bg-amber-600 text-stone-950",
    motion: "motion-safe:animate-pulse",
  },
  memory: {
    title: "查看依据",
    status: "检查记忆和依据的索引，不展开原始内容。",
    Icon: Brain,
    shell: "border-teal-200 bg-[#f2faf7] text-stone-950",
    horizon: "bg-teal-300/70",
    floor: "border-teal-300/80 bg-teal-50/80",
    actor: "border-teal-900 bg-teal-900 text-teal-50",
    signal: "bg-indigo-500",
    rail: "border-teal-300",
    badge: "bg-teal-900 text-teal-50",
    motion: "",
  },
  planning: {
    title: "形成下一步",
    status: "压缩成一小步可执行的路径。",
    Icon: CalendarDays,
    shell: "border-lime-200 bg-[#f8faef] text-stone-950",
    horizon: "bg-lime-300/70",
    floor: "border-lime-300/80 bg-lime-50/80",
    actor: "border-lime-800 bg-lime-700 text-white",
    signal: "bg-amber-500",
    rail: "border-lime-300",
    badge: "bg-lime-800 text-white",
    motion: "motion-safe:animate-pulse",
  },
  review: {
    title: "等待确认",
    status: "有内容需要你过目后再继续。",
    Icon: Inbox,
    shell: "border-orange-200 bg-[#fff7ed] text-stone-950",
    horizon: "bg-orange-300/70",
    floor: "border-orange-300/80 bg-orange-50/80",
    actor: "border-orange-900 bg-orange-700 text-white",
    signal: "bg-rose-500",
    rail: "border-orange-300",
    badge: "bg-orange-800 text-white",
    motion: "",
  },
  privacy: {
    title: "边界保护",
    status: "隐私、权限或本地优先边界已开启。",
    Icon: ShieldCheck,
    shell: "border-sky-200 bg-[#f1f8f8] text-stone-950",
    horizon: "bg-sky-300/70",
    floor: "border-sky-300/80 bg-sky-50/80",
    actor: "border-slate-900 bg-slate-900 text-sky-50",
    signal: "bg-sky-500",
    rail: "border-sky-300",
    badge: "bg-slate-900 text-sky-50",
    motion: "",
  },
  error: {
    title: "需要修复",
    status: "当前路径出错，需要重新选择或重试。",
    Icon: AlertCircle,
    shell: "border-red-200 bg-[#fff4f2] text-stone-950",
    horizon: "bg-red-300/70",
    floor: "border-red-300/80 bg-red-50/80",
    actor: "border-red-900 bg-red-700 text-white",
    signal: "bg-red-500",
    rail: "border-red-300",
    badge: "bg-red-800 text-white",
    motion: "",
  },
};

function cx(...classes: Array<string | false | null | undefined>): string {
  return classes.filter(Boolean).join(" ");
}

export default function AgentStage({ state, title, status, compact = false }: AgentStageProps) {
  const titleId = useId();
  const statusId = useId();
  const config = STATE_CONFIG[state];
  const displayTitle = title ?? config.title;
  const displayStatus = status ?? config.status;
  const Icon = config.Icon;

  return (
    <section
      data-testid="agent-stage"
      data-state={state}
      aria-labelledby={titleId}
      aria-describedby={statusId}
      className={cx(
        "relative isolate overflow-hidden rounded-lg border shadow-sm",
        "transition-colors duration-300 motion-reduce:transition-none",
        compact ? "min-h-[180px] p-4" : "min-h-[260px] p-5 sm:p-6",
        config.shell
      )}
    >
      <div aria-hidden="true" className="absolute inset-0 bg-white/20" />
      <div
        aria-hidden="true"
        className={cx("absolute left-6 right-6 top-7 h-px", config.horizon)}
      />
      <div
        aria-hidden="true"
        className={cx(
          "absolute bottom-7 left-7 right-7 h-16 border border-t-0",
          "[transform:perspective(520px)_rotateX(58deg)]",
          config.floor
        )}
      />
      <div
        aria-hidden="true"
        className="absolute left-5 top-1/2 grid -translate-y-1/2 gap-2"
      >
        {[0, 1, 2].map(index => (
          <span
            key={index}
            data-testid="agent-stage-motion"
            className={cx(
              "block h-1.5 rounded-sm transition-all duration-300",
              index === 0 ? "w-7" : index === 1 ? "w-11" : "w-5",
              index === 1 && config.motion,
              REDUCED_MOTION_CLASSES,
              config.signal
            )}
          />
        ))}
      </div>
      <div
        aria-hidden="true"
        className="absolute right-5 top-1/2 grid -translate-y-1/2 gap-2"
      >
        {[0, 1, 2].map(index => (
          <span
            key={index}
            data-testid="agent-stage-motion"
            className={cx(
              "block h-1.5 rounded-sm transition-all duration-300",
              index === 0 ? "w-5" : index === 1 ? "w-11" : "w-7",
              index === 1 && config.motion,
              REDUCED_MOTION_CLASSES,
              config.signal
            )}
          />
        ))}
      </div>

      <div className="relative z-10 flex h-full min-h-[inherit] flex-col items-center justify-center gap-5">
        <div aria-hidden="true" className="relative grid place-items-center">
          <span
            data-testid="agent-stage-motion"
            className={cx(
              "absolute h-24 w-24 rounded-lg border transition-all duration-300",
              "motion-reduce:scale-100",
              config.motion,
              REDUCED_MOTION_CLASSES,
              config.rail
            )}
          />
          <span
            className={cx(
              "absolute h-16 w-28 rounded-md border-b-2 border-l border-r opacity-70",
              config.rail
            )}
          />
          <div
            className={cx(
              "relative grid h-20 w-20 place-items-center rounded-md border-2 shadow-sm",
              config.actor
            )}
          >
            <Icon size={28} strokeWidth={1.8} aria-hidden="true" />
          </div>
        </div>

        <div
          id={statusId}
          role="status"
          aria-label="OpenLife Agent 状态"
          aria-live="polite"
          aria-atomic="true"
          data-testid="agent-stage-status"
          className="flex max-w-[260px] flex-col items-center gap-1 text-center"
        >
          <span
            id={titleId}
            className={cx(
              "inline-flex min-h-7 items-center rounded-md px-3 text-sm font-semibold",
              config.badge
            )}
          >
            {displayTitle}
          </span>
          <span className="text-xs leading-5 text-stone-700">{displayStatus}</span>
        </div>
      </div>
    </section>
  );
}
