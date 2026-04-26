import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import {
  Map,
  User,
  Target,
  Zap,
  Activity,
  ArrowRight,
  Heart,
  Compass,
  Star,
  Clock,
  TrendingUp,
  AlertCircle,
  CheckCircle2,
  Circle,
} from "lucide-react";
import {
  getLifeModel,
  getModel4DCompletion,
  getDailyGoals,
  getStateAlerts,
  goalCapabilityGapReport,
  identityGoalAlignmentReport,
} from "../tauri";
import type { LifeModel, DailyGoal, StateAlert } from "../types";
import type { Model4DCompletion, CapabilityGap, AlignmentIssue } from "../tauri";
import EmptyState from "../components/EmptyState";

interface DimensionCardProps {
  title: string;
  icon: React.ReactNode;
  color: string;
  bgColor: string;
  completion: number;
  children: React.ReactNode;
  linkTo: string;
  linkLabel: string;
}

function DimensionCard({
  title,
  icon,
  color,
  bgColor,
  completion,
  children,
  linkTo,
  linkLabel,
}: DimensionCardProps) {
  return (
    <div className={`rounded-2xl border ${bgColor} p-5`}>
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center gap-2">
          <div className={`p-1.5 rounded-lg ${color} text-white`}>{icon}</div>
          <h3 className="font-semibold text-gray-900">{title}</h3>
        </div>
        <div className="flex items-center gap-2">
          <div className="text-xs text-gray-500">完成度 {Math.round(completion)}%</div>
          <div className="w-16 h-1.5 bg-gray-200 rounded-full overflow-hidden">
            <div
              className="h-full rounded-full bg-emerald-500"
              style={{ width: `${Math.max(0, Math.min(100, completion))}%` }}
            />
          </div>
        </div>
      </div>
      <div className="space-y-3">{children}</div>
      <Link
        to={linkTo}
        className="mt-4 inline-flex items-center gap-1 text-sm text-indigo-600 hover:text-indigo-700"
      >
        {linkLabel} <ArrowRight size={14} />
      </Link>
    </div>
  );
}

function GoalItem({ goal }: { goal: DailyGoal }) {
  return (
    <div className="flex items-center gap-2 text-sm">
      {goal.done ? (
        <CheckCircle2 size={14} className="text-emerald-500 shrink-0" />
      ) : (
        <Circle size={14} className="text-gray-300 shrink-0" />
      )}
      <span className={goal.done ? "text-gray-400 line-through" : "text-gray-800"}>
        {goal.name}
      </span>
    </div>
  );
}

export default function LifeMapPage() {
  const [model, setModel] = useState<LifeModel | null>(null);
  const [completion, setCompletion] = useState<Model4DCompletion | null>(null);
  const [dailyGoals, setDailyGoals] = useState<DailyGoal[]>([]);
  const [stateAlerts, setStateAlerts] = useState<StateAlert[]>([]);
  const [gaps, setGaps] = useState<CapabilityGap[]>([]);
  const [alignments, setAlignments] = useState<AlignmentIssue[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    loadAll();
  }, []);

  const loadAll = async () => {
    setLoading(true);
    try {
      const [m, c, goals, alerts, g, a] = await Promise.all([
        getLifeModel(),
        getModel4DCompletion(),
        getDailyGoals(),
        getStateAlerts(),
        goalCapabilityGapReport(),
        identityGoalAlignmentReport(),
      ]);
      setModel(m);
      setCompletion(c);
      setDailyGoals(goals);
      setStateAlerts(alerts);
      setGaps(g);
      setAlignments(a);
    } catch (e) {
      console.error("LifeMap load failed:", e);
    } finally {
      setLoading(false);
    }
  };

  if (loading) {
    return (
      <div className="h-full flex items-center justify-center">
        <div className="text-gray-500">加载人生地图...</div>
      </div>
    );
  }

  const identityComp = completion?.identity ?? 0;
  const goalsComp = completion?.goals ?? 0;
  const capsComp = completion?.capabilities ?? 0;
  const stateComp = completion?.state ?? 0;
  const overallComp = completion?.overall ?? 0;

  const topValues = model?.identity?.values?.slice(0, 3) ?? [];
  const topGoals = [
    ...(model?.goals?.short_term ?? []),
    ...(model?.goals?.medium_term ?? []),
    ...(model?.goals?.long_term ?? []),
  ].slice(0, 3);
  const topSkills = model?.capabilities?.skills?.slice(0, 3) ?? [];
  const topResources = model?.capabilities?.resources?.slice(0, 3) ?? [];

  return (
    <div className="h-full overflow-auto bg-[#f4efe7] p-4 sm:p-6">
      <div className="max-w-6xl mx-auto space-y-5">
        {/* Header */}
        <div className="flex items-center justify-between">
          <div>
            <h2 className="text-2xl font-bold text-stone-950 flex items-center gap-2">
              <Map className="text-stone-700" size={24} />
              人生地图
              <span className="sr-only">LifeMap</span>
            </h2>
            <p className="mt-1 text-sm text-stone-500">
              四维模型的可视化总览。你在这里看到的一切，都是 OpenLife 理解你的基础。
            </p>
          </div>
          <div className="flex items-center gap-3">
            <div className="text-right">
              <div className="text-2xl font-bold text-stone-900">{Math.round(overallComp)}%</div>
              <div className="text-xs text-stone-500">整体完成度</div>
            </div>
            <div className="w-12 h-12 rounded-full border-4 border-emerald-200 flex items-center justify-center">
              <div
                className="w-10 h-10 rounded-full bg-emerald-500 flex items-center justify-center text-white text-xs font-bold"
                style={{ opacity: Math.max(0.3, overallComp / 100) }}
              >
                {Math.round(overallComp)}
              </div>
            </div>
          </div>
        </div>

        {/* Four Dimensions Grid */}
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
          {/* Identity */}
          <DimensionCard
            title="Identity · 我是谁"
            icon={<User size={16} />}
            color="bg-purple-600"
            bgColor="bg-white border border-purple-100"
            completion={identityComp}
            linkTo="/builder"
            linkLabel="完善身份"
          >
            {model?.identity?.name && (
              <div className="text-sm font-medium text-gray-900">{model.identity.name}</div>
            )}
            {topValues.length > 0 ? (
              <div className="flex flex-wrap gap-2">
                {topValues.map((v: { name: string }) => (
                  <span
                    key={v.name}
                    className="inline-flex items-center gap-1 rounded-full bg-purple-50 px-2.5 py-1 text-xs text-purple-700 border border-purple-100"
                  >
                    <Heart size={10} />
                    {v.name}
                  </span>
                ))}
              </div>
            ) : (
              <EmptyState
                title="价值观待补充"
                description="添加核心价值观，让建议更贴合你的内在标准。"
                className="py-2"
              />
            )}
            {model?.identity?.mission_statement && (
              <div className="text-xs text-gray-600 italic border-l-2 border-purple-200 pl-3">
                {model.identity.mission_statement}
              </div>
            )}
          </DimensionCard>

          {/* Goals */}
          <DimensionCard
            title="Goals · 我要去哪"
            icon={<Target size={16} />}
            color="bg-indigo-600"
            bgColor="bg-white border border-indigo-100"
            completion={goalsComp}
            linkTo="/builder"
            linkLabel="完善目标"
          >
            {topGoals.length > 0 ? (
              <div className="space-y-2">
                {topGoals.map((g: { name: string; progress?: number }, i: number) => (
                  <div key={i} className="flex items-center justify-between text-sm">
                    <div className="flex items-center gap-2">
                      <Compass size={14} className="text-indigo-400" />
                      <span className="text-gray-800">{g.name}</span>
                    </div>
                    <div className="text-xs text-gray-500">
                      {g.progress !== undefined ? `${Math.round(g.progress)}%` : ""}
                    </div>
                  </div>
                ))}
              </div>
            ) : (
              <EmptyState
                title="目标待设定"
                description="设定短中长期目标，OpenLife 才能帮你拆解行动。"
                className="py-2"
              />
            )}
            {dailyGoals.length > 0 && (
              <div className="border-t pt-2">
                <div className="text-[11px] font-medium text-gray-500 mb-1">今日目标</div>
                {dailyGoals.slice(0, 3).map((g, i) => (
                  <GoalItem key={i} goal={g} />
                ))}
              </div>
            )}
          </DimensionCard>

          {/* Capabilities */}
          <DimensionCard
            title="Capabilities · 我有什么"
            icon={<Zap size={16} />}
            color="bg-amber-600"
            bgColor="bg-white border border-amber-100"
            completion={capsComp}
            linkTo="/builder"
            linkLabel="完善能力"
          >
            {topSkills.length > 0 ? (
              <div className="flex flex-wrap gap-2">
                {topSkills.map((s: { name: string; level?: number }) => (
                  <span
                    key={s.name}
                    className="inline-flex items-center gap-1 rounded-full bg-amber-50 px-2.5 py-1 text-xs text-amber-700 border border-amber-100"
                  >
                    <Star size={10} />
                    {s.name} {s.level !== undefined ? `· L${s.level}` : ""}
                  </span>
                ))}
              </div>
            ) : (
              <EmptyState
                title="技能待记录"
                description="记录你的核心技能与资源，发现能力缺口。"
                className="py-2"
              />
            )}
            {topResources.length > 0 && (
              <div className="text-xs text-gray-600">
                <span className="font-medium">资源：</span>
                {topResources.map((r: { name: string }) => r.name).join("、")}
              </div>
            )}
            {gaps.length > 0 && (
              <div className="border-t pt-2">
                <div className="text-[11px] font-medium text-amber-700 mb-1">
                  <AlertCircle size={10} className="inline mr-1" />
                  能力缺口 ({gaps.length})
                </div>
                {gaps.slice(0, 2).map((g: CapabilityGap, i: number) => (
                  <div key={i} className="text-xs text-gray-600">
                    {g.goal_name} 需要 {g.skill_name}
                  </div>
                ))}
              </div>
            )}
          </DimensionCard>

          {/* State */}
          <DimensionCard
            title="State · 我现在怎样"
            icon={<Activity size={16} />}
            color="bg-emerald-600"
            bgColor="bg-white border border-emerald-100"
            completion={stateComp}
            linkTo="/dashboard"
            linkLabel="记录状态"
          >
            {model?.state?.emotional_state?.current_mood && (
              <div className="flex items-center gap-2 text-sm">
                <Heart size={14} className="text-rose-400" />
                <span className="text-gray-800">
                  情绪：{model.state.emotional_state.current_mood}
                </span>
              </div>
            )}
            {model?.state?.current_focus && (
              <div className="flex items-center gap-2 text-sm">
                <Target size={14} className="text-indigo-400" />
                <span className="text-gray-800">当前专注：{model.state.current_focus}</span>
              </div>
            )}
            {model?.state?.health_status?.physical && (
              <div className="flex items-center gap-2 text-sm">
                <Activity size={14} className="text-emerald-400" />
                <span className="text-gray-800">健康：{model.state.health_status.physical}</span>
              </div>
            )}
            {model?.state?.custom_dimensions && model.state.custom_dimensions.length > 0 ? (
              <div className="flex flex-wrap gap-2">
                {model.state.custom_dimensions
                  .slice(0, 3)
                  .map((dim: { name: string; current_value: number; unit: string }) => (
                    <span
                      key={dim.name}
                      className="inline-flex items-center gap-1 rounded-full bg-emerald-50 px-2.5 py-1 text-xs text-emerald-700 border border-emerald-100"
                    >
                      <TrendingUp size={10} />
                      {dim.name}: {dim.current_value.toFixed(1)}
                      {dim.unit}
                    </span>
                  ))}
              </div>
            ) : (
              <EmptyState
                title="自定义维度待添加"
                description="记录睡眠、运动、专注度等维度，追踪长期趋势。"
                className="py-2"
              />
            )}
            {stateAlerts.length > 0 && (
              <div className="border-t pt-2">
                <div className="text-[11px] font-medium text-amber-700 mb-1">
                  <AlertCircle size={10} className="inline mr-1" />
                  状态预警 ({stateAlerts.length})
                </div>
                {stateAlerts.slice(0, 2).map((alert: StateAlert, i: number) => (
                  <div key={i} className="text-xs text-gray-600">
                    {alert.dimension_name}: {alert.message}
                  </div>
                ))}
              </div>
            )}
          </DimensionCard>
        </div>

        {/* Alignment Check */}
        {alignments.length > 0 && (
          <div className="bg-white border border-amber-100 rounded-2xl p-5">
            <div className="flex items-center gap-2 mb-3">
              <AlertCircle size={18} className="text-amber-600" />
              <h3 className="font-semibold text-gray-900">价值观-目标一致性提醒</h3>
            </div>
            <div className="grid gap-3 md:grid-cols-2">
              {alignments.slice(0, 4).map((issue: AlignmentIssue, i: number) => (
                <div key={i} className="bg-amber-50 rounded-lg px-3 py-2 text-sm text-gray-800">
                  {issue.goal_name}
                </div>
              ))}
            </div>
          </div>
        )}

        {/* Bottom: Quick Actions */}
        <div className="bg-white border rounded-2xl p-5">
          <h3 className="font-semibold text-gray-900 mb-3">快速行动</h3>
          <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
            <Link
              to="/chat"
              className="flex items-center gap-3 rounded-xl bg-indigo-50 border border-indigo-100 p-4 hover:bg-indigo-100 transition"
            >
              <div className="p-2 bg-indigo-600 text-white rounded-lg">
                <Target size={16} />
              </div>
              <div>
                <div className="text-sm font-medium text-gray-900">今日规划</div>
                <div className="text-xs text-gray-500">让 OpenLife 帮你切分今天</div>
              </div>
            </Link>
            <Link
              to="/builder"
              className="flex items-center gap-3 rounded-xl bg-purple-50 border border-purple-100 p-4 hover:bg-purple-100 transition"
            >
              <div className="p-2 bg-purple-600 text-white rounded-lg">
                <User size={16} />
              </div>
              <div>
                <div className="text-sm font-medium text-gray-900">完善模型</div>
                <div className="text-xs text-gray-500">补充缺失的维度</div>
              </div>
            </Link>
            <Link
              to="/dashboard"
              className="flex items-center gap-3 rounded-xl bg-emerald-50 border border-emerald-100 p-4 hover:bg-emerald-100 transition"
            >
              <div className="p-2 bg-emerald-600 text-white rounded-lg">
                <Activity size={16} />
              </div>
              <div>
                <div className="text-sm font-medium text-gray-900">记录状态</div>
                <div className="text-xs text-gray-500">追踪长期趋势</div>
              </div>
            </Link>
            <Link
              to="/calibration"
              className="flex items-center gap-3 rounded-xl bg-amber-50 border border-amber-100 p-4 hover:bg-amber-100 transition"
            >
              <div className="p-2 bg-amber-600 text-white rounded-lg">
                <Clock size={16} />
              </div>
              <div>
                <div className="text-sm font-medium text-gray-900">周期校准</div>
                <div className="text-xs text-gray-500">回顾与微调</div>
              </div>
            </Link>
          </div>
        </div>
      </div>
    </div>
  );
}
