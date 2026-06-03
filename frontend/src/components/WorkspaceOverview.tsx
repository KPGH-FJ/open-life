import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import {
  Activity,
  AlertCircle,
  CalendarDays,
  CheckCircle2,
  Clock,
  Cpu,
  ExternalLink,
  ListChecks,
  MessageSquare,
  Play,
  Save,
  ShieldCheck,
  Zap,
} from "lucide-react";
import type { SystemDiagnostics } from "../tauri";
import {
  createPlanExecuteSession,
  executePlanExecuteStep,
  finalizePlanExecuteSession,
  getSystemDiagnostics,
  listPlanExecuteSessions,
  listAgentRuns,
  listProposals,
  listSkills,
  runSkill,
  getFeedbackSummary,
  countMemoryChunks,
  updatePlanExecuteSessionDraft,
} from "../tauri";
import type { PlanExecuteSession, PlanExecuteStepRecord } from "../types";
import { isSafeMode } from "../utils/safeMode";

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

export default function WorkspaceOverview() {
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
  const [planSession, setPlanSession] = useState<PlanExecuteSession | null>(null);
  const [planMessage, setPlanMessage] = useState<string | null>(null);
  const [planError, setPlanError] = useState<string | null>(null);
  const [planBusy, setPlanBusy] = useState(false);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    loadWorkspaceData();
    const interval = setInterval(loadWorkspaceData, 30000); // Refresh every 30s
    return () => clearInterval(interval);
  }, []);

  async function loadWorkspaceData() {
    try {
      setLoading(true);
      const [diag, proposals, runs, skillList, feedback, memoryCount, planSessions] =
        await Promise.all([
          getSystemDiagnostics().catch(() => null),
          listProposals().catch(() => []),
          listAgentRuns(100, 0).catch(() => []),
          listSkills().catch(() => []),
          getFeedbackSummary().catch(() => ({ total_feedback_up: 0, total_feedback_down: 0 })),
          countMemoryChunks().catch(() => 0),
          listPlanExecuteSessions(3).catch(() => []),
        ]);

      setDiagnostics(diag);
      setSkills(skillList.slice(0, 3));
      setPlanSession(current => current ?? planSessions[0] ?? null);

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
      console.error("Failed to load workspace data:", e);
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

  function setPlanStepTitle(stepId: string, title: string) {
    setPlanSession(current => {
      if (!current) return current;
      return {
        ...current,
        steps: current.steps.map(step => (step.stepId === stepId ? { ...step, title } : step)),
      };
    });
  }

  async function startWeeklyPlan() {
    setPlanBusy(true);
    setPlanError(null);
    setPlanMessage(null);
    try {
      const session = await createPlanExecuteSession({
        scenarioId: "weekly_planning",
        sourceChatSessionId: "workspace_weekly_planning",
        maxSteps: 5,
      });
      setPlanSession(session);
      setPlanMessage("本周计划已生成草稿");
    } catch (e: any) {
      setPlanError(e?.message ?? String(e));
    } finally {
      setPlanBusy(false);
    }
  }

  async function savePlanDraft() {
    if (!planSession) return;
    setPlanBusy(true);
    setPlanError(null);
    try {
      const session = await updatePlanExecuteSessionDraft({
        sessionId: planSession.sessionId,
        steps: planSession.steps.map(step => ({
          stepId: step.stepId,
          title: step.title,
          intent: step.intent,
          actionKind: step.actionKind,
          toolName: step.toolName ?? undefined,
          declaredWrite: step.declaredWrite,
          riskLevel: step.riskLevel,
        })),
      });
      setPlanSession(session);
      setPlanMessage("草稿已保存");
    } catch (e: any) {
      setPlanError(e?.message ?? String(e));
    } finally {
      setPlanBusy(false);
    }
  }

  async function confirmPlan() {
    if (!planSession) return;
    setPlanBusy(true);
    setPlanError(null);
    try {
      const session = await finalizePlanExecuteSession(planSession.sessionId);
      setPlanSession(session);
      setPlanMessage("计划已确认");
    } catch (e: any) {
      setPlanError(e?.message ?? String(e));
    } finally {
      setPlanBusy(false);
    }
  }

  async function executePlanStep(stepId: string) {
    if (!planSession) return;
    setPlanBusy(true);
    setPlanError(null);
    try {
      const output = await executePlanExecuteStep({
        sessionId: planSession.sessionId,
        stepId,
      });
      setPlanSession(output.session);
      setPlanMessage("步骤已更新");
    } catch (e: any) {
      setPlanError(e?.message ?? String(e));
    } finally {
      setPlanBusy(false);
    }
  }

  function stepBadge(step: PlanExecuteStepRecord) {
    if (step.status === "executed") return "bg-emerald-50 text-emerald-700 border-emerald-100";
    if (step.status === "requires_proposal") return "bg-amber-50 text-amber-700 border-amber-100";
    if (step.status === "blocked") return "bg-red-50 text-red-700 border-red-100";
    return "bg-stone-50 text-stone-600 border-stone-200";
  }

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

      <div className="rounded-xl border border-stone-200 bg-white p-4">
        <div className="mb-3 flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
          <div className="flex items-center gap-3">
            <div className="flex h-9 w-9 items-center justify-center rounded-lg bg-stone-900 text-amber-50">
              <CalendarDays size={18} />
            </div>
            <div>
              <div className="text-sm font-semibold text-stone-900">本周计划</div>
              <div className="text-xs text-stone-500">
                {planSession
                  ? `${planSession.stepCount} 个步骤 · ${planSession.status}`
                  : "从 LifeModel 生成本周步骤"}
              </div>
            </div>
          </div>
          <div className="flex flex-wrap gap-2">
            <button
              type="button"
              onClick={startWeeklyPlan}
              disabled={planBusy}
              className="inline-flex items-center gap-2 rounded-lg bg-stone-900 px-3 py-2 text-sm font-medium text-amber-50 hover:bg-stone-800 disabled:cursor-not-allowed disabled:opacity-60"
            >
              <ListChecks size={16} />
              开始本周规划
            </button>
            {planSession?.sourceAgentRunId && (
              <Link
                to={`/runs/${planSession.sourceAgentRunId}`}
                className="inline-flex items-center gap-2 rounded-lg border border-stone-200 px-3 py-2 text-sm font-medium text-stone-700 hover:bg-stone-50"
              >
                <ExternalLink size={15} />
                Run
              </Link>
            )}
          </div>
        </div>

        {planSession ? (
          <div className="space-y-2">
            {planSession.steps.map(step => (
              <div
                key={step.stepId}
                className="grid gap-2 rounded-lg border border-stone-200 p-3 md:grid-cols-[minmax(0,1fr)_auto]"
              >
                <div className="min-w-0">
                  <div className="mb-2 flex flex-wrap items-center gap-2">
                    <span className="text-xs font-medium text-stone-500">{step.stepId}</span>
                    <span
                      className={`rounded-full border px-2 py-0.5 text-xs font-medium ${stepBadge(step)}`}
                    >
                      {step.status}
                    </span>
                    {step.declaredWrite && (
                      <span className="rounded-full border border-amber-100 bg-amber-50 px-2 py-0.5 text-xs font-medium text-amber-700">
                        Proposal
                      </span>
                    )}
                  </div>
                  {planSession.status === "draft" ? (
                    <input
                      value={step.title}
                      onChange={event => setPlanStepTitle(step.stepId, event.target.value)}
                      className="w-full rounded-md border border-stone-200 px-3 py-2 text-sm text-stone-900 outline-none focus:border-stone-400"
                    />
                  ) : (
                    <div className="text-sm font-medium text-stone-900">{step.title}</div>
                  )}
                  {step.observationSummary && (
                    <div className="mt-2 rounded-md bg-emerald-50 px-3 py-2 text-xs text-emerald-800">
                      {step.observationSummary}
                    </div>
                  )}
                  {step.linkedProposalId && (
                    <Link
                      to="/review"
                      className="mt-2 inline-flex items-center gap-1 rounded-md bg-amber-50 px-2 py-1 text-xs font-medium text-amber-800 hover:bg-amber-100"
                    >
                      <ShieldCheck size={13} />
                      {step.linkedProposalId}
                    </Link>
                  )}
                </div>
                <div className="flex items-center gap-2 md:justify-end">
                  {planSession.status === "draft" ? null : (
                    <button
                      type="button"
                      onClick={() => executePlanStep(step.stepId)}
                      disabled={
                        planBusy ||
                        step.status === "executed" ||
                        step.status === "requires_proposal" ||
                        planSession.status === "cancelled"
                      }
                      className="inline-flex items-center gap-2 rounded-lg border border-stone-200 px-3 py-2 text-sm font-medium text-stone-700 hover:bg-stone-50 disabled:cursor-not-allowed disabled:opacity-50"
                    >
                      <Play size={15} />
                      执行 {step.stepId}
                    </button>
                  )}
                </div>
              </div>
            ))}
            <div className="flex flex-wrap items-center gap-2 pt-1">
              {planSession.status === "draft" && (
                <>
                  <button
                    type="button"
                    onClick={savePlanDraft}
                    disabled={planBusy}
                    className="inline-flex items-center gap-2 rounded-lg border border-stone-200 px-3 py-2 text-sm font-medium text-stone-700 hover:bg-stone-50 disabled:cursor-not-allowed disabled:opacity-60"
                  >
                    <Save size={15} />
                    保存草稿
                  </button>
                  <button
                    type="button"
                    onClick={confirmPlan}
                    disabled={planBusy}
                    className="inline-flex items-center gap-2 rounded-lg bg-emerald-700 px-3 py-2 text-sm font-medium text-white hover:bg-emerald-800 disabled:cursor-not-allowed disabled:opacity-60"
                  >
                    <CheckCircle2 size={15} />
                    确认计划
                  </button>
                </>
              )}
              {planMessage && <span className="text-xs text-stone-500">{planMessage}</span>}
              {planError && <span className="text-xs text-red-600">{planError}</span>}
            </div>
          </div>
        ) : (
          <div className="rounded-lg border border-dashed border-stone-200 px-3 py-6 text-center text-sm text-stone-500">
            暂无本周计划
          </div>
        )}
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
}
