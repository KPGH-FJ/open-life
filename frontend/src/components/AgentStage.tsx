import { useId } from "react";
import catYarnImage from "../assets/agent-stage/cat-yarn.png";

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
  signal: string;
  card: string;
};

const STATE_CONFIG: Record<AgentStageState, AgentStageConfig> = {
  idle: {
    title: "安静待命",
    status: "安静待命，保持陪伴。",
    signal: "bg-stone-300",
    card: "bg-[#fffefa]",
  },
  listening: {
    title: "正在听",
    status: "接收你的当前想法，先不急着判断。",
    signal: "bg-emerald-300",
    card: "bg-[#fbfffc]",
  },
  sorting: {
    title: "整理中",
    status: "把线索排成可处理的顺序。",
    signal: "bg-amber-300",
    card: "bg-[#fffdf8]",
  },
  memory: {
    title: "翻看记忆",
    status: "检查记忆和依据的索引，不展开原始内容。",
    signal: "bg-sky-300",
    card: "bg-[#fbfdff]",
  },
  planning: {
    title: "规划下一步",
    status: "压缩成一小步可执行的路径。",
    signal: "bg-lime-300",
    card: "bg-[#fefff8]",
  },
  review: {
    title: "有信等你回",
    status: "有内容需要你过目后再继续。",
    signal: "bg-rose-300",
    card: "bg-[#fffefa]",
  },
  privacy: {
    title: "边界开启",
    status: "隐私、权限或本地优先边界已开启。",
    signal: "bg-cyan-300",
    card: "bg-[#fbfeff]",
  },
  error: {
    title: "需要修复",
    status: "当前路径出错，需要重新选择或重试。",
    signal: "bg-red-300",
    card: "bg-[#fffafa]",
  },
};

function cx(...classes: Array<string | false | null | undefined>): string {
  return classes.filter(Boolean).join(" ");
}

function StageCat() {
  return (
    <img
      aria-hidden="true"
      alt=""
      data-testid="agent-stage-figure"
      draggable={false}
      src={catYarnImage}
      className={cx(
        "h-full max-h-[300px] w-full max-w-[560px] scale-100 select-none object-contain mix-blend-multiply sm:max-h-[560px] sm:scale-[1.08]",
        "drop-shadow-[0_22px_34px_rgba(17,24,39,0.08)]"
      )}
    />
  );
}

export default function AgentStage({ state, title, status, compact = false }: AgentStageProps) {
  const titleId = useId();
  const statusId = useId();
  const config = STATE_CONFIG[state];
  const displayTitle = title ?? config.title;
  const displayStatus = status ?? config.status;

  return (
    <section
      data-testid="agent-stage"
      data-state={state}
      aria-labelledby={titleId}
      aria-describedby={statusId}
      className={cx(
        "relative flex h-full min-h-0 flex-col overflow-hidden rounded-xl border border-stone-200 shadow-sm",
        compact ? "min-h-[360px] sm:min-h-[420px]" : "min-h-[560px]",
        config.card
      )}
    >
      <div
        id={statusId}
        role="status"
        aria-label="OpenLife Agent 状态"
        aria-live="polite"
        aria-atomic="true"
        data-testid="agent-stage-status"
        className="flex h-16 shrink-0 items-center justify-between border-b border-stone-200 px-7"
      >
        <span className="text-xs font-semibold tracking-normal text-stone-500">状态</span>
        <span id={titleId} className="text-base font-semibold tracking-normal text-stone-950">
          {displayTitle}
        </span>
        <span className="sr-only">{displayStatus}</span>
      </div>

      <div className="relative flex min-h-0 flex-1 items-center justify-center overflow-hidden px-5 py-7">
        <div
          aria-hidden="true"
          className={cx(
            "absolute right-9 top-9 h-2.5 w-2.5 rounded-full",
            "transition-colors duration-300",
            config.signal
          )}
        />
        <StageCat />
      </div>
    </section>
  );
}
