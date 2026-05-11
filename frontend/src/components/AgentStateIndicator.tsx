import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  Brain,
  Wrench,
  Loader2,
  Eye,
  ShieldCheck,
  FileText,
  CheckCircle2,
  XCircle,
} from "lucide-react";

export type AgentRunPhase =
  | "thinking"
  | "planning_tool"
  | "executing_tool"
  | "observing"
  | "waiting_permission"
  | "generating_final"
  | "completed"
  | "failed";

interface AgentStatusUpdate {
  session_id: string;
  run_id: string;
  phase: AgentRunPhase;
  message: string;
  step_index: number;
  tool_call_index?: number;
  timestamp: string;
}

interface Props {
  sessionId: string | null;
  runId?: string;
  isActive: boolean;
}

const phaseConfig: Record<
  AgentRunPhase,
  { icon: React.ReactNode; label: string; color: string; bgColor: string }
> = {
  thinking: {
    icon: <Brain size={14} />,
    label: "思考中",
    color: "text-blue-600",
    bgColor: "bg-blue-50",
  },
  planning_tool: {
    icon: <Wrench size={14} />,
    label: "规划工具",
    color: "text-purple-600",
    bgColor: "bg-purple-50",
  },
  executing_tool: {
    icon: <Loader2 size={14} className="animate-spin" />,
    label: "执行中",
    color: "text-amber-600",
    bgColor: "bg-amber-50",
  },
  observing: {
    icon: <Eye size={14} />,
    label: "观察结果",
    color: "text-teal-600",
    bgColor: "bg-teal-50",
  },
  waiting_permission: {
    icon: <ShieldCheck size={14} />,
    label: "等待确认",
    color: "text-orange-600",
    bgColor: "bg-orange-50",
  },
  generating_final: {
    icon: <FileText size={14} />,
    label: "生成回答",
    color: "text-indigo-600",
    bgColor: "bg-indigo-50",
  },
  completed: {
    icon: <CheckCircle2 size={14} />,
    label: "已完成",
    color: "text-green-600",
    bgColor: "bg-green-50",
  },
  failed: {
    icon: <XCircle size={14} />,
    label: "失败",
    color: "text-red-600",
    bgColor: "bg-red-50",
  },
};

export default function AgentStateIndicator({ sessionId, runId, isActive }: Props) {
  const [currentPhase, setCurrentPhase] = useState<AgentRunPhase>("thinking");
  const [phaseMessage, setPhaseMessage] = useState<string>("准备中...");
  const [stepIndex, setStepIndex] = useState<number>(0);

  useEffect(() => {
    if (!isActive) {
      setCurrentPhase("thinking");
      setPhaseMessage("准备中...");
      setStepIndex(0);
      return;
    }

    const unsubscribe = listen<AgentStatusUpdate>("agent-status-update", event => {
      const update = event.payload;
      if (update.session_id !== sessionId) return;
      if (runId && update.run_id !== runId) return;

      setCurrentPhase(update.phase);
      setPhaseMessage(update.message);
      setStepIndex(update.step_index);
    });

    return () => {
      unsubscribe.then(fn => fn());
    };
  }, [sessionId, runId, isActive]);

  const config = phaseConfig[currentPhase];

  return (
    <div className="flex items-center gap-2 py-1.5 px-3 rounded-lg border border-stone-200 bg-white shadow-sm">
      <div className={`${config.color} ${config.bgColor} p-1 rounded-md`}>{config.icon}</div>
      <div className="flex flex-col min-w-0">
        <div className="flex items-center gap-1.5">
          <span className={`text-xs font-medium ${config.color}`}>{config.label}</span>
          {stepIndex > 0 && <span className="text-[10px] text-stone-400">步骤 {stepIndex}</span>}
        </div>
        <span className="text-[11px] text-stone-500 truncate">{phaseMessage}</span>
      </div>
    </div>
  );
}

export type { AgentStatusUpdate };
