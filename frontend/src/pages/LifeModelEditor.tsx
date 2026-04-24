import { useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import {
  Save, Plus, Trash2, ChevronDown, ChevronRight, Map, Edit3,
  User, Target, Zap, Heart, ArrowRight, ShieldCheck, Sparkles
} from "lucide-react";
import type { LifeModel, GoalItem, Milestone, Skill, Resource, ToolCapability, KnowledgeDomain, Relationship } from "../types";
import { getLifeModel, saveLifeModel, createSnapshot, getSystemDiagnostics, type SystemDiagnostics } from "../tauri";
import LoadingSpinner from "../components/LoadingSpinner";
import { getSafeModeReason, isSafeMode } from "../utils/safeMode";
import { buildRuntimeActionError, buildSafeModeBlockedMessage } from "../utils/runtimeMessages";

function emptyModel(): LifeModel {
  const now = new Date().toISOString();
  return {
    metadata: { version: "1.0.0", created_at: now, updated_at: now, author: "" },
    identity: {
      name: "",
      values: [],
      personality_traits: [],
      life_philosophy: "",
      mission_statement: "",
      role_definition: { primary_role: "", secondary_roles: [], responsibilities: [], boundaries: [] },
      voice_style: {
        formality: "neutral",
        tone_descriptors: [],
        vocabulary_preference: "",
        emoji_usage: "sparingly",
      },
    },
    goals: {
      short_term: [],
      medium_term: [],
      long_term: [],
      life_goals: [],
      daily: [],
      progress: 0,
      related_memories: [],
    },
    capabilities: { skills: [], resources: [], networks: [], tools: [], knowledge_domains: [] },
    state: {
      current_focus: "",
      health_status: { physical: "", mental: "", energy_level: 5 },
      emotional_state: { current_mood: "", stress_level: 3, fulfillment_score: 5 },
      recent_reflections: [],
      open_questions: [],
      focus_areas: [],
      recent_events: [],
      habit_streaks: [],
      custom_dimensions: [],
      alerts: [],
    },
    relationships: { inner_circle: [], mentors: [], collaborators: [] },
    preferences: {
      work_hours: { preferred_start: "09:00", preferred_end: "17:00", timezone: "UTC" },
      peak_energy_time: "",
      communication_style: "",
      learning_style: "",
      decision_making_style: "",
    },
    evolution_rules: [],
  };
}

function emptyGoal(): GoalItem {
  return { name: "", priority: 5, status: "pending", milestones: [], description: "", progress: 0, related_memories: [] };
}

function emptyMilestone(): Milestone {
  return { name: "", status: "pending", description: "" };
}

function emptySkill(): Skill {
  return { name: "", proficiency: 5, description: "" };
}

function emptyResource(): Resource {
  return { name: "", resource_type: "other", description: "", availability: "" };
}

function emptyToolCapability(): ToolCapability {
  return { name: "", proficiency: 5, description: "" };
}

function emptyKnowledgeDomain(): KnowledgeDomain {
  return { domain: "", level: 5, description: "" };
}

function emptyRelationship(): Relationship {
  return { name: "", relationship_type: "", importance: 5, notes: "" };
}

export default function LifeModelEditor() {
  const [model, setModel] = useState<LifeModel | null>(null);
  const [diagnostics, setDiagnostics] = useState<SystemDiagnostics | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [notice, setNotice] = useState("");
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
  const [loadError, setLoadError] = useState<string | null>(null);
  const [dirty, setDirty] = useState(false);
  const [viewMode, setViewMode] = useState<"map" | "edit">("map");
  const autoSaveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const loadModel = () => {
    setLoading(true);
    setLoadError(null);
    getLifeModel()
      .then((m) => {
        setModel(m);
        setDirty(false);
      })
      .catch((e) => {
        setModel(null);
        setLoadError(e?.message || String(e));
      })
      .finally(() => setLoading(false));
  };

  useEffect(() => {
    loadModel();
    getSystemDiagnostics().then(setDiagnostics).catch(() => null);
  }, []);

  const safeMode = isSafeMode(diagnostics);
  const safeModeReason = getSafeModeReason(diagnostics);

  // 自动保存：模型变更后 2 秒 debounce
  useEffect(() => {
    if (!model || loading || !dirty || safeMode) return;
    if (autoSaveTimerRef.current) clearTimeout(autoSaveTimerRef.current);
    autoSaveTimerRef.current = setTimeout(() => {
      saveLifeModel(model)
        .then(() => {
          setDirty(false);
          setNotice("已自动保存");
          setTimeout(() => setNotice((n) => (n === "已自动保存" ? "" : n)), 1500);
        })
        .catch(() => {
          setNotice("自动保存失败");
        });
    }, 2000);
    return () => {
      if (autoSaveTimerRef.current) clearTimeout(autoSaveTimerRef.current);
    };
  }, [model, loading, dirty, safeMode]);

  const update = (fn: (d: LifeModel) => void) => {
    if (!model) return;
    if (safeMode) {
      setNotice(buildSafeModeBlockedMessage("人生模型编辑", diagnostics));
      return;
    }
    const next = { ...model };
    fn(next);
    next.metadata.updated_at = new Date().toISOString();
    setModel(next);
    setDirty(true);
  };

  const toggleSection = (key: string) => {
    setCollapsed((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  };

  const handleSave = async () => {
    if (!model) return;
    if (safeMode) {
      setNotice(buildSafeModeBlockedMessage("人生模型保存", diagnostics));
      return;
    }
    setSaving(true);
    try {
      await saveLifeModel(model);
      await createSnapshot("auto-save", "手动保存时创建的快照");
      setDirty(false);
      setNotice("保存成功并已自动快照");
      setTimeout(() => setNotice(""), 2000);
    } catch (e) {
      setNotice(buildRuntimeActionError("保存人生模型", e, "data"));
    } finally {
      setSaving(false);
    }
  };

  function SectionHeader({ title, sectionKey }: { title: string; sectionKey: string }) {
    const isCollapsed = collapsed.has(sectionKey);
    return (
      <button
        onClick={() => toggleSection(sectionKey)}
        className="flex items-center gap-2 text-lg font-semibold text-gray-700 hover:text-indigo-700 transition-colors w-full text-left"
      >
        {isCollapsed ? <ChevronRight size={18} /> : <ChevronDown size={18} />}
        {title}
      </button>
    );
  }

  if (loading || !model) {
    if (loadError) {
      return (
        <div className="h-full overflow-auto bg-white p-6">
          <div className="max-w-3xl mx-auto rounded-xl border border-rose-100 bg-rose-50 p-6 space-y-4">
            <h2 className="text-lg font-semibold text-rose-800">人生模型读取失败</h2>
            <p className="text-sm text-rose-700">{loadError}</p>
            <div className="flex flex-wrap gap-3">
              <button onClick={loadModel} className="px-4 py-2 rounded-md bg-white border text-sm hover:bg-rose-50">
                重试读取
              </button>
              <button
                onClick={() => {
                  setModel(emptyModel());
                  setDirty(false);
                  setLoadError(null);
                  setViewMode("edit");
                }}
                className="px-4 py-2 rounded-md bg-rose-600 text-white text-sm hover:bg-rose-700"
              >
                新建空模型
              </button>
              <a href="#/settings" className="px-4 py-2 rounded-md bg-white border text-sm hover:bg-rose-50">
                去 Settings 查看数据目录
              </a>
            </div>
          </div>
        </div>
      );
    }
    return (
      <div className="h-full flex items-center justify-center">
        <LoadingSpinner text="正在加载人生模型..." />
      </div>
    );
  }

  const allGoals = [
    ...model.goals.short_term,
    ...model.goals.medium_term,
    ...model.goals.long_term,
    ...model.goals.life_goals,
  ];
  const activeGoals = allGoals.filter((goal) => goal.status !== "completed");
  const topValues = [...model.identity.values].sort((a, b) => b.weight - a.weight).slice(0, 5);
  const topSkills = [...model.capabilities.skills].sort((a, b) => b.proficiency - a.proficiency).slice(0, 5);
  const stateSignals = [
    model.state.current_focus && `当前重心：${model.state.current_focus}`,
    model.state.emotional_state.current_mood && `心情：${model.state.emotional_state.current_mood}`,
    model.state.health_status.energy_level !== undefined && `精力：${model.state.health_status.energy_level}/10`,
    model.state.emotional_state.stress_level !== undefined && `压力：${model.state.emotional_state.stress_level}/10`,
  ].filter(Boolean) as string[];
  const completion = {
    identity: Math.round((
      Number(Boolean(model.identity.name)) +
      Number(model.identity.values.length > 0) +
      Number(Boolean(model.identity.mission_statement || model.identity.life_philosophy)) +
      Number(Boolean(model.identity.role_definition.primary_role))
    ) / 4 * 100),
    goals: Math.round((
      Number(allGoals.length > 0) +
      Number(model.goals.daily.length > 0) +
      Number(activeGoals.length > 0) +
      Number(allGoals.some((goal) => goal.milestones.length > 0))
    ) / 4 * 100),
    capabilities: Math.round((
      Number(model.capabilities.skills.length > 0) +
      Number(model.capabilities.resources.length > 0) +
      Number(model.capabilities.knowledge_domains.length > 0) +
      Number(model.capabilities.tools.length > 0)
    ) / 4 * 100),
    state: Math.round((
      Number(Boolean(model.state.current_focus)) +
      Number(model.state.focus_areas.length > 0) +
      Number(model.state.custom_dimensions.length > 0) +
      Number(model.state.habit_streaks.length > 0 || stateSignals.length > 0)
    ) / 4 * 100),
  };
  const overallCompletion = Math.round((completion.identity + completion.goals + completion.capabilities + completion.state) / 4);

  const MapCard = ({
    title,
    subtitle,
    icon,
    score,
    children,
  }: {
    title: string;
    subtitle: string;
    icon: ReactNode;
    score: number;
    children: ReactNode;
  }) => (
    <section className="rounded-3xl border border-stone-200 bg-white/80 p-5 shadow-sm">
      <div className="flex items-start justify-between gap-3">
        <div className="flex items-center gap-3">
          <div className="flex h-11 w-11 items-center justify-center rounded-2xl bg-stone-900 text-amber-50">
            {icon}
          </div>
          <div>
            <h3 className="font-semibold text-stone-950">{title}</h3>
            <p className="text-xs text-stone-500">{subtitle}</p>
          </div>
        </div>
        <span className="rounded-full bg-stone-100 px-2.5 py-1 text-xs font-medium text-stone-700">{score}%</span>
      </div>
      <div className="mt-4 h-2 rounded-full bg-stone-100">
        <div className="h-2 rounded-full bg-emerald-600" style={{ width: `${Math.max(0, Math.min(100, score))}%` }} />
      </div>
      <div className="mt-4">{children}</div>
    </section>
  );

  if (viewMode === "map") {
    return (
      <div className="h-full overflow-auto bg-[#f4efe7] p-6">
        <div className="mx-auto max-w-6xl space-y-6">
          {safeMode && (
            <div className="rounded-3xl border border-amber-200 bg-amber-50 p-5">
              <div className="flex flex-col gap-3 md:flex-row md:items-start md:justify-between">
                <div>
                  <div className="text-sm font-semibold text-amber-900">Safe Mode：人生模型当前只读</div>
                  <div className="mt-1 text-sm leading-6 text-amber-800">
                    {safeModeReason} 为了避免覆盖现有数据，编辑和保存已暂时关闭。你仍然可以查看当前人生地图。
                  </div>
                </div>
                <a
                  href="#/settings"
                  className="inline-flex items-center gap-2 rounded-full border border-amber-300 bg-white px-4 py-2 text-sm font-medium text-amber-900 hover:bg-amber-100"
                >
                  去恢复控制台 <ArrowRight size={15} />
                </a>
              </div>
            </div>
          )}
          <div className="relative overflow-hidden rounded-3xl border border-stone-200 bg-[#fbf7ef] p-6 shadow-sm">
            <div className="absolute -right-12 -top-16 h-56 w-56 rounded-full bg-amber-200/40 blur-2xl" />
            <div className="relative flex flex-col gap-5 md:flex-row md:items-start md:justify-between">
              <div>
                <div className="inline-flex items-center gap-2 rounded-full bg-stone-900 px-3 py-1 text-xs font-medium text-amber-50">
                  <Map size={14} />
                  人生地图
                </div>
                <h2 className="mt-4 text-3xl font-semibold tracking-tight text-stone-950">
                  {model.identity.name ? `${model.identity.name} 的人生模型` : "OpenLife 眼中的你"}
                </h2>
                <p className="mt-3 max-w-3xl text-sm leading-7 text-stone-600">
                  这里不是配置表，而是 OpenLife 当前对你的理解。你可以先阅读这张地图，再决定是否进入细节编辑。
                </p>
              </div>
              <div className="flex flex-wrap gap-2">
                <button
                  onClick={() => setViewMode("edit")}
                  className="inline-flex items-center gap-2 rounded-full bg-stone-900 px-4 py-2 text-sm font-medium text-amber-50 hover:bg-stone-800"
                >
                  <Edit3 size={15} />
                  {safeMode ? "只读查看" : "编辑模型"}
                </button>
                <a href="#/builder" className="inline-flex items-center gap-2 rounded-full border border-stone-200 bg-white/80 px-4 py-2 text-sm font-medium text-stone-700 hover:bg-white">
                  继续构建 <ArrowRight size={15} />
                </a>
              </div>
            </div>
            <div className="relative mt-6 grid gap-3 md:grid-cols-4">
              {[
                ["Identity", completion.identity],
                ["Goals", completion.goals],
                ["Capabilities", completion.capabilities],
                ["State", completion.state],
              ].map(([label, score]) => (
                <div key={label} className="rounded-2xl border border-white bg-white/75 p-4">
                  <div className="text-xs text-stone-500">{label}</div>
                  <div className="mt-1 text-2xl font-semibold text-stone-950">{score}%</div>
                </div>
              ))}
            </div>
            <div className="relative mt-4 rounded-2xl border border-white bg-white/70 p-4">
              <div className="flex items-center justify-between text-sm">
                <span className="font-medium text-stone-800">整体完整度</span>
                <span className="text-stone-500">{overallCompletion}%</span>
              </div>
              <div className="mt-2 h-2 rounded-full bg-stone-200">
                <div className="h-2 rounded-full bg-stone-900" style={{ width: `${overallCompletion}%` }} />
              </div>
            </div>
          </div>

          <div className="grid gap-5 lg:grid-cols-2">
            <MapCard title="Identity 我是谁" subtitle="身份、价值观、角色和表达风格" icon={<User size={20} />} score={completion.identity}>
              <div className="space-y-4">
                <div>
                  <div className="text-xs font-medium text-stone-500">使命 / 哲学</div>
                  <p className="mt-1 text-sm leading-6 text-stone-700">
                    {model.identity.mission_statement || model.identity.life_philosophy || "还没有写下明确的人生叙事。建议从 Builder 的 Identity 维度开始。"}
                  </p>
                </div>
                <div>
                  <div className="text-xs font-medium text-stone-500">核心价值观</div>
                  <div className="mt-2 flex flex-wrap gap-2">
                    {topValues.length > 0 ? topValues.map((value) => (
                      <span key={value.name} className="rounded-full bg-amber-50 px-3 py-1 text-xs text-amber-800 border border-amber-100">
                        {value.name} · {value.weight}
                      </span>
                    )) : <span className="text-sm text-stone-500">暂无价值观</span>}
                  </div>
                </div>
                <div className="rounded-2xl bg-stone-50 p-3 text-sm text-stone-600">
                  当前角色：{model.identity.role_definition.primary_role || "尚未定义"}
                </div>
              </div>
            </MapCard>

            <MapCard title="Goals 我要去哪里" subtitle="长期方向、中期项目、今日行动" icon={<Target size={20} />} score={completion.goals}>
              <div className="space-y-3">
                {activeGoals.slice(0, 4).map((goal) => (
                  <div key={goal.name} className="rounded-2xl border border-stone-100 bg-stone-50 p-3">
                    <div className="text-sm font-medium text-stone-800">{goal.name || "未命名目标"}</div>
                    <div className="mt-1 text-xs text-stone-500">优先级 {goal.priority} · {goal.status}</div>
                    {goal.description && <div className="mt-2 text-xs leading-5 text-stone-600">{goal.description}</div>}
                  </div>
                ))}
                {activeGoals.length === 0 && <div className="text-sm text-stone-500">暂无进行中的长期或短期目标。</div>}
                <div className="text-xs text-stone-500">今日目标：{model.goals.daily.length} 个</div>
              </div>
            </MapCard>

            <MapCard title="Capabilities 我有什么" subtitle="技能、资源、工具和知识域" icon={<Zap size={20} />} score={completion.capabilities}>
              <div className="space-y-4">
                <div>
                  <div className="text-xs font-medium text-stone-500">能力资产</div>
                  <div className="mt-2 grid gap-2">
                    {topSkills.length > 0 ? topSkills.map((skill) => (
                      <div key={skill.name} className="flex items-center justify-between rounded-xl bg-stone-50 px-3 py-2 text-sm">
                        <span className="text-stone-700">{skill.name}</span>
                        <span className="text-xs text-stone-500">{skill.proficiency}/10</span>
                      </div>
                    )) : <span className="text-sm text-stone-500">暂无技能记录</span>}
                  </div>
                </div>
                <div className="flex flex-wrap gap-2 text-xs">
                  {model.capabilities.resources.slice(0, 4).map((resource) => (
                    <span key={resource.name} className="rounded-full bg-emerald-50 px-3 py-1 text-emerald-800 border border-emerald-100">{resource.name}</span>
                  ))}
                  {model.capabilities.knowledge_domains.slice(0, 4).map((domain) => (
                    <span key={domain.domain} className="rounded-full bg-blue-50 px-3 py-1 text-blue-800 border border-blue-100">{domain.domain}</span>
                  ))}
                </div>
              </div>
            </MapCard>

            <MapCard title="State 我现在怎么样" subtitle="当前重心、状态、习惯和预警" icon={<Heart size={20} />} score={completion.state}>
              <div className="space-y-4">
                <div className="grid gap-2">
                  {stateSignals.length > 0 ? stateSignals.map((signal) => (
                    <div key={signal} className="rounded-xl bg-stone-50 px-3 py-2 text-sm text-stone-700">{signal}</div>
                  )) : <span className="text-sm text-stone-500">暂无当前状态记录</span>}
                </div>
                <div>
                  <div className="text-xs font-medium text-stone-500">关注领域</div>
                  <div className="mt-2 flex flex-wrap gap-2">
                    {model.state.focus_areas.length > 0 ? model.state.focus_areas.map((area) => (
                      <span key={area} className="rounded-full bg-rose-50 px-3 py-1 text-xs text-rose-800 border border-rose-100">{area}</span>
                    )) : <span className="text-sm text-stone-500">暂无关注领域</span>}
                  </div>
                </div>
              </div>
            </MapCard>
          </div>

          <div className="grid gap-4 md:grid-cols-3">
            <a href="#/chat" className="rounded-2xl border border-stone-200 bg-white/80 p-4 transition hover:-translate-y-0.5 hover:shadow-sm">
              <Sparkles size={18} className="text-stone-700" />
              <div className="mt-3 text-sm font-medium text-stone-900">用这张地图开始对话</div>
              <div className="mt-1 text-xs leading-5 text-stone-500">让 Chat 基于当前模型做规划、复盘或决策陪跑。</div>
            </a>
            <a href="#/builder" className="rounded-2xl border border-stone-200 bg-white/80 p-4 transition hover:-translate-y-0.5 hover:shadow-sm">
              <ShieldCheck size={18} className="text-stone-700" />
              <div className="mt-3 text-sm font-medium text-stone-900">补全薄弱维度</div>
              <div className="mt-1 text-xs leading-5 text-stone-500">继续构建 Identity、Goals、Capabilities 或 State。</div>
            </a>
            <a href="#/versions" className="rounded-2xl border border-stone-200 bg-white/80 p-4 transition hover:-translate-y-0.5 hover:shadow-sm">
              <Map size={18} className="text-stone-700" />
              <div className="mt-3 text-sm font-medium text-stone-900">查看历史变化</div>
              <div className="mt-1 text-xs leading-5 text-stone-500">每次重要修改都可以通过版本控制回看和回滚。</div>
            </a>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="h-full overflow-auto bg-[#f4efe7] p-6">
      <div className="max-w-4xl mx-auto bg-white rounded-xl shadow p-6 space-y-8">
        <div className="flex items-center justify-between">
          <div>
            <h2 className="text-2xl font-bold text-gray-800">人生模型编辑器</h2>
            <p className="mt-1 text-sm text-gray-500">细节编辑会自动保存；高风险改动建议手动保存并创建快照。</p>
          </div>
          <div className="flex gap-2">
            <button
              onClick={() => setViewMode("map")}
              className="flex items-center gap-2 rounded-md border border-gray-200 px-4 py-2 text-sm text-gray-700 hover:bg-gray-50"
            >
              <Map size={18} /> 返回地图
            </button>
            <button
              onClick={handleSave}
              disabled={saving || safeMode}
              className="flex items-center gap-2 bg-indigo-600 text-white px-4 py-2 rounded-md hover:bg-indigo-700 disabled:opacity-50"
            >
              <Save size={18} /> {saving ? "保存中..." : "保存"}
            </button>
          </div>
        </div>
        {safeMode && (
          <div className="rounded-xl border border-amber-200 bg-amber-50 px-4 py-3 text-sm text-amber-900">
            <div className="font-medium">Safe Mode：编辑已切换为只读</div>
            <div className="mt-1 text-amber-800">
              {safeModeReason} 你可以继续检查当前模型，但所有字段写入、自动保存和手动保存都已暂停。
            </div>
            <a href="#/settings" className="mt-2 inline-flex items-center gap-1 font-medium text-amber-900 underline">
              去 Settings 的恢复控制台 <ArrowRight size={14} />
            </a>
          </div>
        )}
        {notice && <div className="text-sm text-green-600">{notice}</div>}
        <fieldset disabled={safeMode} className={safeMode ? "opacity-70" : ""}>
        {/* 基本信息 */}
        <section className="space-y-3">
          <SectionHeader title="基本信息" sectionKey="basic" />
          {!collapsed.has("basic") && (<>
          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-sm text-gray-500">姓名 / 代号</label>
              <input
                value={model.identity.name}
                onChange={(e) => update((m) => (m.identity.name = e.target.value))}
                className="w-full border rounded-md px-3 py-2"
              />
            </div>
            <div>
              <label className="block text-sm text-gray-500">人生哲学</label>
              <input
                value={model.identity.life_philosophy}
                onChange={(e) => update((m) => (m.identity.life_philosophy = e.target.value))}
                className="w-full border rounded-md px-3 py-2"
              />
            </div>
          </div>
          <div>
            <label className="block text-sm text-gray-500">使命陈述</label>
            <textarea
              aria-label="使命陈述"
              value={model.identity.mission_statement}
              onChange={(e) => update((m) => (m.identity.mission_statement = e.target.value))}
              className="w-full border rounded-md px-3 py-2 min-h-[5rem]"
            />
          </div>
          </>)}
        </section>

        {/* 价值观 */}
        <section className="space-y-3">
          <SectionHeader title="价值观" sectionKey="values" />
          {!collapsed.has("values") && (<>
          {model.identity.values.map((v, idx) => (
            <div key={idx} className="flex gap-3 items-center">
              <input
                placeholder="名称"
                value={v.name}
                onChange={(e) => update((m) => (m.identity.values[idx].name = e.target.value))}
                className="flex-1 border rounded-md px-3 py-2"
              />
              <input
                type="number"
                min={1}
                max={10}
                value={v.weight}
                onChange={(e) => update((m) => (m.identity.values[idx].weight = Number(e.target.value)))}
                className="w-20 border rounded-md px-3 py-2"
              />
              <input
                placeholder="描述"
                value={v.description}
                onChange={(e) => update((m) => (m.identity.values[idx].description = e.target.value))}
                className="flex-[2] border rounded-md px-3 py-2"
              />
              <button onClick={() => update((m) => m.identity.values.splice(idx, 1))} className="text-red-500 hover:text-red-700">
                <Trash2 size={18} />
              </button>
            </div>
          ))}
          <button
            onClick={() => update((m) => m.identity.values.push({ name: "", weight: 5, description: "" }))}
            className="flex items-center gap-1 text-indigo-600 text-sm font-medium"
          >
            <Plus size={16} /> 添加价值观
          </button>
          </>)}
        </section>

        {/* 性格特质 */}
        <section className="space-y-3">
          <SectionHeader title="性格特质" sectionKey="personality" />
          {!collapsed.has("personality") && (<>
          {model.identity.personality_traits.map((t, idx) => (
            <div key={idx} className="flex gap-3 items-center">
              <input
                placeholder="特质"
                value={t.trait_name}
                onChange={(e) => update((m) => (m.identity.personality_traits[idx].trait_name = e.target.value))}
                className="flex-1 border rounded-md px-3 py-2"
              />
              <input
                type="number"
                min={1}
                max={10}
                value={t.score}
                onChange={(e) => update((m) => (m.identity.personality_traits[idx].score = Number(e.target.value)))}
                className="w-20 border rounded-md px-3 py-2"
              />
              <button onClick={() => update((m) => m.identity.personality_traits.splice(idx, 1))} className="text-red-500 hover:text-red-700">
                <Trash2 size={18} />
              </button>
            </div>
          ))}
          <button
            onClick={() => update((m) => m.identity.personality_traits.push({ trait_name: "", score: 5 }))}
            className="flex items-center gap-1 text-indigo-600 text-sm font-medium"
          >
            <Plus size={16} /> 添加性格特质
          </button>
          </>)}
        </section>

        <section className="space-y-4">
          <SectionHeader title="角色与表达风格" sectionKey="role" />
          {!collapsed.has("role") && (<>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <div>
              <label className="block text-sm text-gray-500">主要角色</label>
              <input
                aria-label="主要角色"
                value={model.identity.role_definition.primary_role}
                onChange={(e) => update((m) => (m.identity.role_definition.primary_role = e.target.value))}
                className="w-full border rounded-md px-3 py-2"
              />
            </div>
            <div>
              <label className="block text-sm text-gray-500">正式程度</label>
              <select
                value={model.identity.voice_style.formality}
                onChange={(e) => update((m) => (m.identity.voice_style.formality = e.target.value as typeof model.identity.voice_style.formality))}
                className="w-full border rounded-md px-3 py-2"
              >
                <option value="casual">轻松</option>
                <option value="neutral">中性</option>
                <option value="formal">正式</option>
              </select>
            </div>
            <div>
              <label className="block text-sm text-gray-500">词汇偏好</label>
              <input
                value={model.identity.voice_style.vocabulary_preference}
                onChange={(e) => update((m) => (m.identity.voice_style.vocabulary_preference = e.target.value))}
                className="w-full border rounded-md px-3 py-2"
              />
            </div>
            <div>
              <label className="block text-sm text-gray-500">表情使用频率</label>
              <select
                value={model.identity.voice_style.emoji_usage}
                onChange={(e) => update((m) => (m.identity.voice_style.emoji_usage = e.target.value as typeof model.identity.voice_style.emoji_usage))}
                className="w-full border rounded-md px-3 py-2"
              >
                <option value="never">从不</option>
                <option value="sparingly">偶尔</option>
                <option value="often">经常</option>
              </select>
            </div>
          </div>

          <div className="space-y-3">
            <div>
              <label className="block text-sm text-gray-500 mb-1">次要角色</label>
              <div className="flex flex-wrap gap-2">
                {model.identity.role_definition.secondary_roles.map((role, idx) => (
                  <span key={idx} className="inline-flex items-center gap-1 bg-gray-100 text-gray-700 px-2 py-1 rounded-md text-sm">
                    {role}
                    <button onClick={() => update((m) => m.identity.role_definition.secondary_roles.splice(idx, 1))} className="text-gray-400 hover:text-gray-600">×</button>
                  </span>
                ))}
              </div>
              <input
                placeholder="按回车添加"
                className="mt-2 w-full border rounded-md px-3 py-2 text-sm"
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    const val = (e.target as HTMLInputElement).value.trim();
                    if (val) {
                      update((m) => m.identity.role_definition.secondary_roles.push(val));
                      (e.target as HTMLInputElement).value = "";
                    }
                  }
                }}
              />
            </div>

            {([
              { key: "responsibilities", label: "职责" },
              { key: "boundaries", label: "边界" },
            ] as const).map((item) => (
              <div key={item.label}>
                <label className="block text-sm text-gray-500 mb-1">{item.label}</label>
                <div className="flex flex-wrap gap-2">
                  {model.identity.role_definition[item.key].map((value: string, idx: number) => (
                    <span key={idx} className="inline-flex items-center gap-1 bg-indigo-50 text-indigo-700 px-2 py-1 rounded-md text-sm border border-indigo-100">
                      {value}
                      <button
                        onClick={() => update((m) => m.identity.role_definition[item.key].splice(idx, 1))}
                        className="text-indigo-400 hover:text-indigo-600"
                      >
                        ×
                      </button>
                    </span>
                  ))}
                </div>
                <input
                  placeholder="按回车添加"
                  className="mt-2 w-full border rounded-md px-3 py-2 text-sm"
                  onKeyDown={(e) => {
                    if (e.key === "Enter") {
                      const val = (e.target as HTMLInputElement).value.trim();
                      if (val) {
                        update((m) => m.identity.role_definition[item.key].push(val));
                        (e.target as HTMLInputElement).value = "";
                      }
                    }
                  }}
                />
              </div>
            ))}

            <div>
              <label className="block text-sm text-gray-500 mb-1">语气描述词</label>
              <div className="flex flex-wrap gap-2">
                {model.identity.voice_style.tone_descriptors.map((value: string, idx: number) => (
                  <span key={idx} className="inline-flex items-center gap-1 bg-indigo-50 text-indigo-700 px-2 py-1 rounded-md text-sm border border-indigo-100">
                    {value}
                    <button
                      onClick={() => update((m) => m.identity.voice_style.tone_descriptors.splice(idx, 1))}
                      className="text-indigo-400 hover:text-indigo-600"
                    >
                      ×
                    </button>
                  </span>
                ))}
              </div>
              <input
                placeholder="按回车添加"
                className="mt-2 w-full border rounded-md px-3 py-2 text-sm"
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    const val = (e.target as HTMLInputElement).value.trim();
                    if (val) {
                      update((m) => m.identity.voice_style.tone_descriptors.push(val));
                      (e.target as HTMLInputElement).value = "";
                    }
                  }
                }}
              />
            </div>
          </div>
          </>)}
        </section>

        {/* 目标 */}
        <section className="space-y-6">
          <SectionHeader title="目标" sectionKey="goals" />
          {!collapsed.has("goals") && (<>
          {([
            { key: "short_term", label: "短期目标" },
            { key: "medium_term", label: "中期目标" },
            { key: "long_term", label: "长期目标" },
            { key: "life_goals", label: "人生目标" },
          ] as const).map(({ key, label }) => (
            <div key={key} className="border rounded-lg p-4 space-y-3 bg-gray-50">
              <div className="font-medium text-gray-800">{label}</div>
              {model.goals[key].map((g, idx) => (
                <div key={idx} className="bg-white border rounded-md p-3 space-y-2">
                  <div className="flex gap-3 items-center">
                    <input
                      placeholder="目标名称"
                      value={g.name}
                      onChange={(e) => update((m) => (m.goals[key][idx].name = e.target.value))}
                      className="flex-1 border rounded-md px-3 py-2"
                    />
                    <select
                      value={g.status}
                      onChange={(e) => update((m) => (m.goals[key][idx].status = e.target.value))}
                      className="border rounded-md px-3 py-2"
                    >
                      <option value="pending">待开始</option>
                      <option value="in_progress">进行中</option>
                      <option value="completed">已完成</option>
                    </select>
                    <input
                      type="number"
                      min={1}
                      max={10}
                      value={g.priority}
                      onChange={(e) => update((m) => (m.goals[key][idx].priority = Number(e.target.value)))}
                      className="w-20 border rounded-md px-3 py-2"
                      placeholder="优先级"
                    />
                    <button onClick={() => update((m) => m.goals[key].splice(idx, 1))} className="text-red-500 hover:text-red-700">
                      <Trash2 size={18} />
                    </button>
                  </div>
                  <input
                    placeholder="描述"
                    value={g.description}
                    onChange={(e) => update((m) => (m.goals[key][idx].description = e.target.value))}
                    className="w-full border rounded-md px-3 py-2"
                  />
                  <div className="pl-4 border-l-2 border-indigo-100 space-y-2">
                    <div className="text-sm text-gray-500">里程碑</div>
                    {g.milestones.map((ms, midx) => (
                      <div key={midx} className="flex gap-2 items-center">
                        <input
                          placeholder="里程碑名称"
                          value={ms.name}
                          onChange={(e) => update((m) => (m.goals[key][idx].milestones[midx].name = e.target.value))}
                          className="flex-1 border rounded-md px-3 py-2 text-sm"
                        />
                        <input
                          type="date"
                          value={ms.target_date?.slice(0, 10) || ""}
                          onChange={(e) =>
                            update((m) => {
                              const d = e.target.value;
                              m.goals[key][idx].milestones[midx].target_date = d ? new Date(d).toISOString() : undefined;
                            })
                          }
                          className="border rounded-md px-3 py-2 text-sm"
                        />
                        <select
                          value={ms.status}
                          onChange={(e) => update((m) => (m.goals[key][idx].milestones[midx].status = e.target.value))}
                          className="border rounded-md px-3 py-2 text-sm"
                        >
                          <option value="pending">待开始</option>
                          <option value="in_progress">进行中</option>
                          <option value="completed">已完成</option>
                        </select>
                        <button
                          onClick={() => update((m) => m.goals[key][idx].milestones.splice(midx, 1))}
                          className="text-red-500 hover:text-red-700"
                        >
                          <Trash2 size={16} />
                        </button>
                      </div>
                    ))}
                    <button
                      onClick={() => update((m) => m.goals[key][idx].milestones.push(emptyMilestone()))}
                      className="flex items-center gap-1 text-indigo-600 text-xs font-medium"
                    >
                      <Plus size={14} /> 添加里程碑
                    </button>
                  </div>
                </div>
              ))}
              <button
                onClick={() => update((m) => m.goals[key].push(emptyGoal()))}
                className="flex items-center gap-1 text-indigo-600 text-sm font-medium"
              >
                <Plus size={16} /> 添加{label}
              </button>
            </div>
          ))}
          </>)}
        </section>

        {/* 能力 */}
        <section className="space-y-6">
          <SectionHeader title="能力" sectionKey="capabilities" />
          {!collapsed.has("capabilities") && (<>

          <div className="border rounded-lg p-4 space-y-3 bg-gray-50">
            <div className="font-medium text-gray-800">技能</div>
            {model.capabilities.skills.map((s, idx) => (
              <div key={idx} className="bg-white border rounded-md p-3 space-y-2">
                <div className="flex gap-3 items-center">
                  <input
                    placeholder="技能名称"
                    value={s.name}
                    onChange={(e) => update((m) => (m.capabilities.skills[idx].name = e.target.value))}
                    className="flex-1 border rounded-md px-3 py-2"
                  />
                  <input
                    type="number"
                    min={1}
                    max={10}
                    value={s.proficiency}
                    onChange={(e) => update((m) => (m.capabilities.skills[idx].proficiency = Number(e.target.value)))}
                    className="w-24 border rounded-md px-3 py-2"
                    placeholder="熟练度"
                  />
                  <button onClick={() => update((m) => m.capabilities.skills.splice(idx, 1))} className="text-red-500 hover:text-red-700">
                    <Trash2 size={18} />
                  </button>
                </div>
                <input
                  placeholder="描述"
                  value={s.description}
                  onChange={(e) => update((m) => (m.capabilities.skills[idx].description = e.target.value))}
                  className="w-full border rounded-md px-3 py-2"
                />
              </div>
            ))}
            <button
              onClick={() => update((m) => m.capabilities.skills.push(emptySkill()))}
              className="flex items-center gap-1 text-indigo-600 text-sm font-medium"
            >
              <Plus size={16} /> 添加技能
            </button>
          </div>

          <div className="border rounded-lg p-4 space-y-3 bg-gray-50">
            <div className="font-medium text-gray-800">资源</div>
            {model.capabilities.resources.map((r, idx) => (
              <div key={idx} className="bg-white border rounded-md p-3 space-y-2">
                <div className="flex gap-3 items-center">
                  <input
                    placeholder="资源名称"
                    value={r.name}
                    onChange={(e) => update((m) => (m.capabilities.resources[idx].name = e.target.value))}
                    className="flex-1 border rounded-md px-3 py-2"
                  />
                  <select
                    value={r.resource_type}
                    onChange={(e) => update((m) => (m.capabilities.resources[idx].resource_type = e.target.value))}
                    className="border rounded-md px-3 py-2"
                  >
                    <option value="time">时间</option>
                    <option value="money">金钱</option>
                    <option value="network">人脉</option>
                    <option value="knowledge">知识</option>
                    <option value="other">其他</option>
                  </select>
                  <button onClick={() => update((m) => m.capabilities.resources.splice(idx, 1))} className="text-red-500 hover:text-red-700">
                    <Trash2 size={18} />
                  </button>
                </div>
                <input
                  placeholder="描述"
                  value={r.description}
                  onChange={(e) => update((m) => (m.capabilities.resources[idx].description = e.target.value))}
                  className="w-full border rounded-md px-3 py-2"
                />
                <input
                  placeholder="可用性 / 获取方式"
                  value={r.availability}
                  onChange={(e) => update((m) => (m.capabilities.resources[idx].availability = e.target.value))}
                  className="w-full border rounded-md px-3 py-2"
                />
              </div>
            ))}
            <button
              onClick={() => update((m) => m.capabilities.resources.push(emptyResource()))}
              className="flex items-center gap-1 text-indigo-600 text-sm font-medium"
            >
              <Plus size={16} /> 添加资源
            </button>
          </div>

          <div className="border rounded-lg p-4 space-y-3 bg-gray-50">
            <div className="font-medium text-gray-800">人脉网络</div>
            {model.capabilities.networks.map((n, idx) => (
              <div key={idx} className="flex gap-3 items-center">
                <input
                  placeholder="人脉 / 组织名称"
                  value={n}
                  onChange={(e) => update((m) => (m.capabilities.networks[idx] = e.target.value))}
                  className="flex-1 border rounded-md px-3 py-2"
                />
                <button onClick={() => update((m) => m.capabilities.networks.splice(idx, 1))} className="text-red-500 hover:text-red-700">
                  <Trash2 size={18} />
                </button>
              </div>
            ))}
            <button
              onClick={() => update((m) => m.capabilities.networks.push(""))}
              className="flex items-center gap-1 text-indigo-600 text-sm font-medium"
            >
              <Plus size={16} /> 添加人脉
            </button>
          </div>

          <div className="border rounded-lg p-4 space-y-3 bg-gray-50">
            <div className="font-medium text-gray-800">工具能力</div>
            {model.capabilities.tools.map((tool, idx) => (
              <div key={idx} className="bg-white border rounded-md p-3 space-y-2">
                <div className="flex gap-3 items-center">
                  <input
                    placeholder="工具名称"
                    value={tool.name}
                    onChange={(e) => update((m) => (m.capabilities.tools[idx].name = e.target.value))}
                    className="flex-1 border rounded-md px-3 py-2"
                  />
                  <input
                    type="number"
                    min={1}
                    max={10}
                    value={tool.proficiency}
                    onChange={(e) => update((m) => (m.capabilities.tools[idx].proficiency = Number(e.target.value)))}
                    className="w-24 border rounded-md px-3 py-2"
                  />
                  <button onClick={() => update((m) => m.capabilities.tools.splice(idx, 1))} className="text-red-500 hover:text-red-700">
                    <Trash2 size={18} />
                  </button>
                </div>
                <input
                  placeholder="使用说明 / 擅长场景"
                  value={tool.description}
                  onChange={(e) => update((m) => (m.capabilities.tools[idx].description = e.target.value))}
                  className="w-full border rounded-md px-3 py-2"
                />
              </div>
            ))}
            <button onClick={() => update((m) => m.capabilities.tools.push(emptyToolCapability()))} className="flex items-center gap-1 text-indigo-600 text-sm font-medium">
              <Plus size={16} /> 添加工具能力
            </button>
          </div>

          <div className="border rounded-lg p-4 space-y-3 bg-gray-50">
            <div className="font-medium text-gray-800">知识领域</div>
            {model.capabilities.knowledge_domains.map((domain, idx) => (
              <div key={idx} className="bg-white border rounded-md p-3 space-y-2">
                <div className="flex gap-3 items-center">
                  <input
                    placeholder="领域名称"
                    value={domain.domain}
                    onChange={(e) => update((m) => (m.capabilities.knowledge_domains[idx].domain = e.target.value))}
                    className="flex-1 border rounded-md px-3 py-2"
                  />
                  <input
                    type="number"
                    min={1}
                    max={10}
                    value={domain.level}
                    onChange={(e) => update((m) => (m.capabilities.knowledge_domains[idx].level = Number(e.target.value)))}
                    className="w-24 border rounded-md px-3 py-2"
                  />
                  <button onClick={() => update((m) => m.capabilities.knowledge_domains.splice(idx, 1))} className="text-red-500 hover:text-red-700">
                    <Trash2 size={18} />
                  </button>
                </div>
                <input
                  placeholder="描述"
                  value={domain.description}
                  onChange={(e) => update((m) => (m.capabilities.knowledge_domains[idx].description = e.target.value))}
                  className="w-full border rounded-md px-3 py-2"
                />
              </div>
            ))}
            <button onClick={() => update((m) => m.capabilities.knowledge_domains.push(emptyKnowledgeDomain()))} className="flex items-center gap-1 text-indigo-600 text-sm font-medium">
              <Plus size={16} /> 添加知识领域
            </button>
          </div>
          </>)}
        </section>

        {/* 当前状态 */}
        <section className="space-y-4">
          <SectionHeader title="当前状态" sectionKey="state" />
          {!collapsed.has("state") && (<>
          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-sm text-gray-500">当前重心</label>
              <input
                value={model.state.current_focus}
                onChange={(e) => update((m) => (m.state.current_focus = e.target.value))}
                className="w-full border rounded-md px-3 py-2"
              />
            </div>
            <div>
              <label className="block text-sm text-gray-500">当前心情</label>
              <input
                value={model.state.emotional_state.current_mood}
                onChange={(e) => update((m) => (m.state.emotional_state.current_mood = e.target.value))}
                className="w-full border rounded-md px-3 py-2"
              />
            </div>
            <div>
              <label className="block text-sm text-gray-500">身体健康</label>
              <input
                value={model.state.health_status.physical}
                onChange={(e) => update((m) => (m.state.health_status.physical = e.target.value))}
                className="w-full border rounded-md px-3 py-2"
              />
            </div>
            <div>
              <label className="block text-sm text-gray-500">心理健康</label>
              <input
                value={model.state.health_status.mental}
                onChange={(e) => update((m) => (m.state.health_status.mental = e.target.value))}
                className="w-full border rounded-md px-3 py-2"
              />
            </div>
            <div>
              <label className="block text-sm text-gray-500">精力水平 (1-10)</label>
              <input
                type="number"
                min={1}
                max={10}
                value={model.state.health_status.energy_level}
                onChange={(e) => update((m) => (m.state.health_status.energy_level = Number(e.target.value)))}
                className="w-full border rounded-md px-3 py-2"
              />
            </div>
            <div>
              <label className="block text-sm text-gray-500">压力水平 (1-10)</label>
              <input
                type="number"
                min={1}
                max={10}
                value={model.state.emotional_state.stress_level}
                onChange={(e) => update((m) => (m.state.emotional_state.stress_level = Number(e.target.value)))}
                className="w-full border rounded-md px-3 py-2"
              />
            </div>
            <div>
              <label className="block text-sm text-gray-500">满足度 (1-10)</label>
              <input
                type="number"
                min={1}
                max={10}
                value={model.state.emotional_state.fulfillment_score}
                onChange={(e) => update((m) => (m.state.emotional_state.fulfillment_score = Number(e.target.value)))}
                className="w-full border rounded-md px-3 py-2"
              />
            </div>
          </div>

          <div className="space-y-3 pt-2">
            <div>
              <label className="block text-sm text-gray-500 mb-1">关注领域</label>
              <div className="flex flex-wrap gap-2">
                {model.state.focus_areas.map((area, idx) => (
                  <span key={idx} className="inline-flex items-center gap-1 bg-indigo-50 text-indigo-700 px-2 py-1 rounded-md text-sm border border-indigo-100">
                    {area}
                    <button onClick={() => update((m) => m.state.focus_areas.splice(idx, 1))} className="text-indigo-400 hover:text-indigo-600">×</button>
                  </span>
                ))}
              </div>
              <input
                placeholder="按回车添加"
                className="mt-2 w-full border rounded-md px-3 py-2 text-sm"
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    const val = (e.target as HTMLInputElement).value.trim();
                    if (val) {
                      update((m) => m.state.focus_areas.push(val));
                      (e.target as HTMLInputElement).value = "";
                    }
                  }
                }}
              />
            </div>

            <div>
              <label className="block text-sm text-gray-500 mb-1">近期关键事件</label>
              <div className="space-y-2">
                {model.state.recent_events.map((ev, idx) => (
                  <div key={idx} className="flex gap-2 items-center">
                    <input
                      value={ev}
                      onChange={(e) => update((m) => (m.state.recent_events[idx] = e.target.value))}
                      className="flex-1 border rounded-md px-3 py-2 text-sm"
                    />
                    <button onClick={() => update((m) => m.state.recent_events.splice(idx, 1))} className="text-red-500 hover:text-red-700"><Trash2 size={18} /></button>
                  </div>
                ))}
                <button onClick={() => update((m) => m.state.recent_events.push(""))} className="flex items-center gap-1 text-indigo-600 text-sm font-medium"><Plus size={16} /> 添加事件</button>
              </div>
            </div>

            <div>
              <label className="block text-sm text-gray-500 mb-1">习惯连续天数</label>
              <div className="space-y-2">
                {model.state.habit_streaks.map((h, idx) => (
                  <div key={idx} className="flex gap-2 items-center">
                    <input
                      placeholder="习惯名称"
                      value={h.name}
                      onChange={(e) => update((m) => (m.state.habit_streaks[idx].name = e.target.value))}
                      className="flex-1 border rounded-md px-3 py-2 text-sm"
                    />
                    <input
                      type="number"
                      placeholder="天数"
                      min={0}
                      value={h.streak_days}
                      onChange={(e) => update((m) => (m.state.habit_streaks[idx].streak_days = Number(e.target.value)))}
                      className="w-24 border rounded-md px-3 py-2 text-sm"
                    />
                    <button onClick={() => update((m) => m.state.habit_streaks.splice(idx, 1))} className="text-red-500 hover:text-red-700"><Trash2 size={18} /></button>
                  </div>
                ))}
                <button onClick={() => update((m) => m.state.habit_streaks.push({ name: "", streak_days: 0 }))} className="flex items-center gap-1 text-indigo-600 text-sm font-medium"><Plus size={16} /> 添加习惯</button>
              </div>
            </div>
          </div>
          </>)}
        </section>

        <section className="space-y-6">
          <SectionHeader title="关系与偏好" sectionKey="relationships" />
          {!collapsed.has("relationships") && (<>

          {([
            { key: "inner_circle", label: "核心关系" },
            { key: "mentors", label: "导师 / 引路人" },
            { key: "collaborators", label: "协作者" },
          ] as const).map(({ key, label }) => (
            <div key={key} className="border rounded-lg p-4 space-y-3 bg-gray-50">
              <div className="font-medium text-gray-800">{label}</div>
              {model.relationships[key].map((rel, idx) => (
                <div key={idx} className="bg-white border rounded-md p-3 space-y-2">
                  <div className="flex gap-3 items-center">
                    <input
                      placeholder="姓名 / 称呼"
                      value={rel.name}
                      onChange={(e) => update((m) => (m.relationships[key][idx].name = e.target.value))}
                      className="flex-1 border rounded-md px-3 py-2"
                    />
                    <input
                      placeholder="关系类型"
                      value={rel.relationship_type}
                      onChange={(e) => update((m) => (m.relationships[key][idx].relationship_type = e.target.value))}
                      className="flex-1 border rounded-md px-3 py-2"
                    />
                    <input
                      type="number"
                      min={1}
                      max={10}
                      value={rel.importance}
                      onChange={(e) => update((m) => (m.relationships[key][idx].importance = Number(e.target.value)))}
                      className="w-24 border rounded-md px-3 py-2"
                    />
                    <button onClick={() => update((m) => m.relationships[key].splice(idx, 1))} className="text-red-500 hover:text-red-700">
                      <Trash2 size={18} />
                    </button>
                  </div>
                  <textarea
                    placeholder="备注"
                    value={rel.notes}
                    onChange={(e) => update((m) => (m.relationships[key][idx].notes = e.target.value))}
                    className="w-full border rounded-md px-3 py-2 min-h-[4rem]"
                  />
                </div>
              ))}
              <button onClick={() => update((m) => m.relationships[key].push(emptyRelationship()))} className="flex items-center gap-1 text-indigo-600 text-sm font-medium">
                <Plus size={16} /> 添加{label}
              </button>
            </div>
          ))}

          <div className="border rounded-lg p-4 bg-gray-50 space-y-4">
            <div className="font-medium text-gray-800">工作与决策偏好</div>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
              <div>
                <label className="block text-sm text-gray-500">偏好开始时间</label>
                <input
                  value={model.preferences.work_hours.preferred_start}
                  onChange={(e) => update((m) => (m.preferences.work_hours.preferred_start = e.target.value))}
                  className="w-full border rounded-md px-3 py-2"
                />
              </div>
              <div>
                <label className="block text-sm text-gray-500">偏好结束时间</label>
                <input
                  value={model.preferences.work_hours.preferred_end}
                  onChange={(e) => update((m) => (m.preferences.work_hours.preferred_end = e.target.value))}
                  className="w-full border rounded-md px-3 py-2"
                />
              </div>
              <div>
                <label className="block text-sm text-gray-500">时区</label>
                <input
                  value={model.preferences.work_hours.timezone}
                  onChange={(e) => update((m) => (m.preferences.work_hours.timezone = e.target.value))}
                  className="w-full border rounded-md px-3 py-2"
                />
              </div>
              <div>
                <label className="block text-sm text-gray-500">高能时段</label>
                <input
                  aria-label="高能时段"
                  value={model.preferences.peak_energy_time}
                  onChange={(e) => update((m) => (m.preferences.peak_energy_time = e.target.value))}
                  className="w-full border rounded-md px-3 py-2"
                />
              </div>
              <div>
                <label className="block text-sm text-gray-500">沟通风格</label>
                <input
                  value={model.preferences.communication_style}
                  onChange={(e) => update((m) => (m.preferences.communication_style = e.target.value))}
                  className="w-full border rounded-md px-3 py-2"
                />
              </div>
              <div>
                <label className="block text-sm text-gray-500">学习风格</label>
                <input
                  value={model.preferences.learning_style}
                  onChange={(e) => update((m) => (m.preferences.learning_style = e.target.value))}
                  className="w-full border rounded-md px-3 py-2"
                />
              </div>
              <div className="md:col-span-2">
                <label className="block text-sm text-gray-500">决策风格</label>
                <input
                  value={model.preferences.decision_making_style}
                  onChange={(e) => update((m) => (m.preferences.decision_making_style = e.target.value))}
                  className="w-full border rounded-md px-3 py-2"
                />
              </div>
            </div>
          </div>
          </>)}
        </section>

        <section className="space-y-4 border-t pt-4">
          <div className="flex items-center justify-between">
            <h3 className="text-base font-semibold text-gray-800">自动进化规则</h3>
            <span className="text-xs text-gray-400">由系统根据反馈数据自动生成</span>
          </div>
          <div className="space-y-2">
            {model.evolution_rules.map((rule, idx) => (
              <div key={idx} className="flex gap-2 items-start">
                <textarea
                  value={rule}
                  onChange={(e) => update((m) => (m.evolution_rules[idx] = e.target.value))}
                  className="flex-1 border rounded-md px-3 py-2 text-sm min-h-[3rem]"
                  placeholder="例如：用户近期偏好简洁回答，优先给出要点。"
                />
                <button onClick={() => update((m) => m.evolution_rules.splice(idx, 1))} className="text-red-500 hover:text-red-700 mt-2"><Trash2 size={18} /></button>
              </div>
            ))}
            <button onClick={() => update((m) => m.evolution_rules.push(""))} className="flex items-center gap-1 text-indigo-600 text-sm font-medium"><Plus size={16} /> 添加规则</button>
          </div>
        </section>
        </fieldset>
      </div>
    </div>
  );
}
