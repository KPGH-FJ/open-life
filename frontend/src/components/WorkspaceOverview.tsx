import { useEffect, useState, memo } from "react";
import { Link } from "react-router-dom";
import {
  Activity,
  AlertCircle,
  CheckCircle2,
  Clock,
  Cpu,
  MessageSquare,
  ShieldCheck,
  Zap,
} from "lucide-react";
import type { SystemDiagnostics } from "../tauri";
import {
  getSystemDiagnostics,
  listAgentRuns,
  listProposals,
  listSkills,
  runSkill,
  getFeedbackSummary,
  countMemoryChunks,
} from "../tauri";
import { isSafeMode } from "../utils/safeMode";
import { logError } from "../utils/logger";

interface WorkspaceStats {
  pendingProposals: number;
  totalRuns: number;
  recentRuns: number;
  systemStatus: "healthy" | "warning" | "critical";
  lastActivity: string;
  totalFeedbackUp: number;
  totalFeedbackDown: number;
  memoryChunks: number;
  chatSessions: number;
}

const WorkspaceOverview = memo(function WorkspaceOverview() {
  const [stats, setStats] = useState<WorkspaceStats>({
    pendingProposals: 0,
    totalRuns: 0,
    recentRuns: 0,
    systemStatus: "healthy",
    lastActivity: "-",
    totalFeedbackUp: 0,
    totalFeedbackDown: 0,
    memoryChunks: 0,
    chatSessions: 0,
  });
  const [diagnostics, setDiagnostics] = useState<SystemDiagnostics | null>(null);
  const [skills, setSkills] = useState<{ id: string; name: string; description: string }[]>([]);
  const [skillMessage, setSkillMessage] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    loadWorkspaceData();
    const interval = setInterval(loadWorkspaceData, 30000); // Refresh every 30s
    return () => clearInterval(interval);
  }, []);

  async function loadWorkspaceData() {
    try {
      setLoading(true);
      const [diag, proposals, runs, skillList, feedback, memoryCount] = await Promise.all([
        getSystemDiagnostics().catch(() => null),
        listProposals().catch(() => []),
        listAgentRuns(100, 0).catch(() => []),
        listSkills().catch(() => []),
        getFeedbackSummary().catch(() => ({ total_feedback_up: 0, total_feedback_down: 0 })),
        countMemoryChunks().catch(() => 0),
      ]);

      setDiagnostics(diag);
      setSkills(skillList.slice(0, 3));

      const pendingCount = proposals.filter((p: any) => p.status === "pending").length;

      const recentCount = runs.filter((r: any) => {
        const runTime = new Date(r.startedAt);
        const dayAgo = new Date(Date.now() - 24 * 60 * 60 * 1000);
        return runTime > dayAgo;
      }).length;

      let status: "healthy" | "warning" | "critical" = "healthy";
      if (diag) {
        if (isSafeMode(diag)) {
          status = "critical";
        } else if (!diag.beta_ready) {
          status = "warning";
        }
      }

      setStats({
        pendingProposals: pendingCount,
        totalRuns: runs.length,
        recentRuns: recentCount,
        systemStatus: status,
        lastActivity: runs.length > 0 ? new Date(runs[0].startedAt).toLocaleString("zh-CN") : "-",
        totalFeedbackUp: feedback.total_feedback_up,
        totalFeedbackDown: feedback.total_feedback_down,
        memoryChunks: memoryCount,
        chatSessions: diag?.chat_session_count || 0,
      });
    } catch (e) {
      logError("Failed to load workspace data:", e);
    } finally {
      setLoading(false);
    }
  }

  const statusConfig = {
    healthy: {
      icon: CheckCircle2,
      class: "bg-emerald-50 text-emerald-700 border-emerald-200",
      label: "系统正常",
    },
    warning: {
      icon: AlertCircle,
      class: "bg-amber-50 text-amber-700 border-amber-200",
      label: "需要关注",
    },
    critical: {
      icon: AlertCircle,
      class: "bg-red-50 text-red-700 border-red-200",
      label: "存在风险",
    },
  };

  const status = statusConfig[stats.systemStatus];
  const StatusIcon = status.icon;

  if (loading) {
    return (
      <div className="animate-pulse space-y-4">
        <div className="h-32 bg-gray-100 rounded-xl" />
        <div className="grid grid-cols-4 gap-4">
          {[1, 2, 3, 4].map(i => (
            <div key={i} className="h-24 bg-gray-100 rounded-xl" />
          ))}
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      {/* System Status Banner */}
      <div className={`rounded-xl border p-4 flex items-center justify-between ${status.class}`}>
        <div className="flex items-center gap-3">
          <StatusIcon size={20} />
          <div>
            <div className="font-semibold text-sm">{status.label}</div>
            <div className="text-xs opacity-75">上次活动: {stats.lastActivity}</div>
          </div>
        </div>
        {diagnostics && (
          <div className="text-xs opacity-75">
            {diagnostics.beta_ready ? "Beta 就绪" : "试用准备中"}
          </div>
        )}
      </div>

      {/* Quick Stats Grid */}
      <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
        <Link
          to="/review"
          className="rounded-xl border border-stone-200 bg-white p-4 hover:shadow-md transition-shadow"
        >
          <div className="flex items-center justify-between mb-2">
            <ShieldCheck size={18} className="text-indigo-600" />
            {stats.pendingProposals > 0 && (
              <span className="bg-red-500 text-white text-xs px-2 py-0.5 rounded-full">
                {stats.pendingProposals}
              </span>
            )}
          </div>
          <div className="text-2xl font-bold text-stone-900">{stats.pendingProposals}</div>
          <div className="text-xs text-stone-500">待处理 Proposal</div>
        </Link>

        <Link
          to="/runs"
          className="rounded-xl border border-stone-200 bg-white p-4 hover:shadow-md transition-shadow"
        >
          <div className="flex items-center justify-between mb-2">
            <Activity size={18} className="text-blue-600" />
          </div>
          <div className="text-2xl font-bold text-stone-900">{stats.recentRuns}</div>
          <div className="text-xs text-stone-500">今日 Agent Run</div>
        </Link>

        <Link
          to="/runs"
          className="rounded-xl border border-stone-200 bg-white p-4 hover:shadow-md transition-shadow"
        >
          <div className="flex items-center justify-between mb-2">
            <Cpu size={18} className="text-emerald-600" />
          </div>
          <div className="text-2xl font-bold text-stone-900">{stats.totalRuns}</div>
          <div className="text-xs text-stone-500">累计运行次数</div>
        </Link>

        <div className="rounded-xl border border-stone-200 bg-white p-4">
          <div className="flex items-center justify-between mb-2">
            <MessageSquare size={18} className="text-amber-600" />
          </div>
          <div className="text-2xl font-bold text-stone-900">{stats.chatSessions}</div>
          <div className="text-xs text-stone-500">会话数</div>
        </div>

        <div className="rounded-xl border border-stone-200 bg-white p-4">
          <div className="flex items-center justify-between mb-2">
            <Clock size={18} className="text-purple-600" />
          </div>
          <div className="text-2xl font-bold text-stone-900">{stats.memoryChunks}</div>
          <div className="text-xs text-stone-500">记忆块</div>
        </div>

        <div className="rounded-xl border border-stone-200 bg-white p-4">
          <div className="flex items-center justify-between mb-2">
            <Activity size={18} className="text-rose-600" />
          </div>
          <div className="text-2xl font-bold text-stone-900">
            <span className="text-green-600">{stats.totalFeedbackUp}</span>
            <span className="text-stone-400 mx-1">/</span>
            <span className="text-red-600">{stats.totalFeedbackDown}</span>
          </div>
          <div className="text-xs text-stone-500">反馈 👍/👎</div>
        </div>
      </div>

      {/* Built-in Skills */}
      <div className="rounded-xl border border-stone-200 bg-white p-4">
        <div className="mb-3 flex items-center justify-between">
          <div>
            <div className="text-sm font-semibold text-stone-900">内置 Skills</div>
            <div className="text-xs text-stone-500">
              运行后会创建 AgentRun，并把建议送入 Review Center。
            </div>
          </div>
        </div>
        <div className="grid gap-2 md:grid-cols-3">
          {skills.map(skill => (
            <button
              key={skill.id}
              onClick={async () => {
                setSkillMessage(null);
                const res = await runSkill(skill.id, { text: `Run ${skill.name} from Workspace` });
                setSkillMessage(`${skill.name} 已完成：${res.summary}`);
              }}
              className="rounded-lg border border-stone-200 px-3 py-3 text-left hover:bg-stone-50"
            >
              <div className="text-sm font-medium text-stone-900">{skill.name}</div>
              <div className="mt-1 line-clamp-2 text-xs text-stone-500">{skill.description}</div>
            </button>
          ))}
        </div>
        {skillMessage && (
          <div className="mt-3 rounded-lg bg-indigo-50 px-3 py-2 text-xs text-indigo-800">
            {skillMessage}
          </div>
        )}
      </div>

      {/* Quick Actions */}
      <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
        <Link
          to="/agent"
          className="flex items-center gap-2 px-4 py-3 bg-stone-900 text-amber-50 rounded-xl text-sm font-medium hover:bg-stone-800 transition"
        >
          <MessageSquare size={16} />
          开始对话
        </Link>
        <Link
          to="/builder"
          className="flex items-center gap-2 px-4 py-3 bg-white border border-stone-200 text-stone-700 rounded-xl text-sm font-medium hover:bg-stone-50 transition"
        >
          <Zap size={16} />
          构建 LifeModel
        </Link>
        <Link
          to="/review"
          className="flex items-center gap-2 px-4 py-3 bg-white border border-stone-200 text-stone-700 rounded-xl text-sm font-medium hover:bg-stone-50 transition"
        >
          <ShieldCheck size={16} />
          审查 Proposal
          {stats.pendingProposals > 0 && (
            <span className="ml-auto bg-red-500 text-white text-xs px-2 py-0.5 rounded-full">
              {stats.pendingProposals}
            </span>
          )}
        </Link>
        <Link
          to="/memory"
          className="flex items-center gap-2 px-4 py-3 bg-white border border-stone-200 text-stone-700 rounded-xl text-sm font-medium hover:bg-stone-50 transition"
        >
          <Clock size={16} />
          查看记忆
        </Link>
      </div>
    </div>
  );
});

export default WorkspaceOverview;
