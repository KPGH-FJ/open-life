import { Component, ReactNode, useEffect, useState } from "react";
import { Routes, Route, NavLink } from "react-router-dom";
import {
  Map,
  MessageSquare,
  GitBranch,
  Brain,
  Network,
  Hammer,
  LayoutDashboard,
  Settings,
  Wrench,
  Sparkles,
  Activity,
  ShieldCheck,
} from "lucide-react";
import LifeMapPage from "./pages/LifeMapPage";
import ChatPage from "./pages/ChatPage";
import VersionControl from "./pages/VersionControl";
import MemorySearch from "./pages/MemorySearch";
import A2APage from "./pages/A2APage";
import McpPage from "./pages/McpPage";
import BuilderPage from "./pages/BuilderPage";
import DashboardPage from "./pages/DashboardPage";
import SettingsPage from "./pages/SettingsPage";
import CalibrationPage from "./pages/CalibrationPage";
import ProposalReviewPage from "./pages/ProposalReviewPage";
import RunsPage from "./pages/RunsPage";
import AgentRunDetail from "./pages/AgentRunDetail";
import MetricsPage from "./pages/MetricsPage";
import OnboardingWizard from "./components/OnboardingWizard";
import { getSystemDiagnostics, hasCompletedOnboarding, type SystemDiagnostics } from "./tauri";
import { getSafeModeReason, isSafeMode } from "./utils/safeMode";

class ErrorBoundary extends Component<
  { children: ReactNode },
  { hasError: boolean; error: string }
> {
  constructor(props: { children: ReactNode }) {
    super(props);
    this.state = { hasError: false, error: "" };
  }
  static getDerivedStateFromError(error: any) {
    return { hasError: true, error: String(error && error.stack ? error.stack : error) };
  }
  componentDidCatch(error: any, info: any) {
    // eslint-disable-next-line no-console
    console.error("App ErrorBoundary caught:", error, info);
  }
  render() {
    if (this.state.hasError) {
      return (
        <div className="p-6 text-red-700 bg-red-50">
          <h2 className="font-bold mb-2">页面渲染出错</h2>
          <pre className="whitespace-pre-wrap text-sm">{this.state.error}</pre>
        </div>
      );
    }
    return this.props.children;
  }
}

function App() {
  const [showWizard, setShowWizard] = useState(false);
  const [wizardReady, setWizardReady] = useState(false);
  const [diagnostics, setDiagnostics] = useState<SystemDiagnostics | null>(null);

  useEffect(() => {
    hasCompletedOnboarding()
      .then(done => {
        setShowWizard(!done);
        setWizardReady(true);
      })
      .catch(() => {
        setShowWizard(true);
        setWizardReady(true);
      });
  }, []);

  useEffect(() => {
    getSystemDiagnostics()
      .then(setDiagnostics)
      .catch(() => setDiagnostics(null));
  }, []);

  const safeMode = isSafeMode(diagnostics);
  const safeModeReason = getSafeModeReason(diagnostics);

  const navClass = ({ isActive }: { isActive: boolean }) =>
    `flex items-center gap-1.5 px-3 py-1.5 rounded-full text-sm font-medium transition ${
      isActive ? "bg-stone-900 text-amber-50 shadow-sm" : "text-stone-600 hover:bg-stone-100"
    }`;

  return (
    <div className="h-screen flex flex-col bg-[#f4efe7] text-stone-900">
      <header className="bg-[#fbf7ef]/95 border-b border-stone-200 px-4 py-3 flex items-center justify-between gap-4">
        <div className="flex items-center gap-3 shrink-0">
          <div className="h-9 w-9 rounded-2xl bg-stone-900 text-amber-100 flex items-center justify-center shadow-sm">
            <Sparkles size={18} />
          </div>
          <div>
            <h1 className="text-lg font-bold tracking-tight text-stone-950">OpenLife</h1>
            <div className="text-[11px] text-stone-500">你的成长驾驶舱</div>
          </div>
          {diagnostics && (
            <NavLink
              to="/settings"
              className={`ml-2 hidden rounded-full px-3 py-1 text-[11px] font-medium md:inline-flex ${
                diagnostics.beta_ready
                  ? "bg-emerald-100 text-emerald-800"
                  : diagnostics.chat_ready
                    ? "bg-blue-100 text-blue-800"
                    : "bg-amber-100 text-amber-800"
              }`}
            >
              {diagnostics.beta_ready
                ? "Beta 可试用"
                : diagnostics.chat_ready
                  ? "核心链路已通"
                  : "试用待修复"}
            </NavLink>
          )}
        </div>
        <nav className="flex flex-wrap justify-end gap-2">
          <NavLink to="/" end className={navClass}>
            <LayoutDashboard size={16} /> Workspace
          </NavLink>
          <NavLink to="/agent" className={navClass}>
            <MessageSquare size={16} /> Chat
          </NavLink>
          <NavLink to="/builder" className={navClass}>
            <Hammer size={16} /> LifeModel
          </NavLink>
          <NavLink to="/memory" className={navClass}>
            <Brain size={16} /> Memory
          </NavLink>
          <NavLink to="/runs" className={navClass}>
            <Activity size={16} /> Runs
          </NavLink>
          <NavLink to="/review" className={navClass}>
            <ShieldCheck size={16} /> Review
          </NavLink>
          <NavLink to="/settings" className={navClass}>
            <Settings size={16} /> 设置
          </NavLink>
          <details className="relative group">
            <summary className="list-none flex items-center gap-1.5 px-3 py-1.5 rounded-full text-sm font-medium text-stone-600 hover:bg-stone-100 cursor-pointer">
              <Wrench size={16} /> 高级
            </summary>
            <div className="absolute right-0 z-20 mt-2 w-44 rounded-xl border border-stone-200 bg-white p-2 shadow-lg">
              <NavLink to="/map" className={navClass}>
                <Map size={16} /> 人生地图
              </NavLink>
              <NavLink to="/versions" className={navClass}>
                <GitBranch size={16} /> 版本控制
              </NavLink>
              <NavLink to="/mcp" className={navClass}>
                <Wrench size={16} /> MCP
              </NavLink>
              <NavLink to="/a2a" className={navClass}>
                <Network size={16} /> A2A
              </NavLink>
              <NavLink to="/metrics" className={navClass}>
                <Activity size={16} /> 监控
              </NavLink>
              <NavLink to="/calibration" className={navClass}>
                <Settings size={16} /> 校准
              </NavLink>
            </div>
          </details>
        </nav>
      </header>
      {safeMode && diagnostics && (
        <div className="border-b border-amber-200 bg-amber-50 px-4 py-3">
          <div className="mx-auto flex max-w-7xl flex-wrap items-start justify-between gap-3">
            <div>
              <div className="text-sm font-semibold text-amber-900">
                Safe Mode：当前数据环境存在风险
              </div>
              <div className="mt-1 text-xs text-amber-800">{safeModeReason}</div>
              <div className="mt-1 text-xs text-amber-700">
                建议先去设置页的“恢复控制台”导出备份，再继续试用。
              </div>
            </div>
            <div className="flex gap-2">
              <NavLink
                to="/settings"
                className="rounded-full bg-amber-900 px-3 py-1.5 text-xs font-medium text-amber-50 hover:bg-amber-950"
              >
                打开恢复控制台
              </NavLink>
              <NavLink
                to="/memory"
                className="rounded-full border border-amber-300 bg-white px-3 py-1.5 text-xs font-medium text-amber-900 hover:bg-amber-100"
              >
                查看记忆状态
              </NavLink>
            </div>
          </div>
        </div>
      )}
      {!safeMode && diagnostics && !diagnostics.beta_ready && (
        <div className="border-b border-blue-100 bg-blue-50/80 px-4 py-2.5">
          <div className="mx-auto flex max-w-7xl flex-wrap items-center justify-between gap-3">
            <div className="text-xs text-blue-900">
              <span className="font-semibold">Beta 试用准备中：</span>
              {diagnostics.beta_readiness_issues?.[0] ??
                "继续完成设置、构建和首轮对话，就能形成完整试用闭环。"}
            </div>
            <NavLink
              to="/settings"
              className="rounded-full bg-blue-700 px-3 py-1 text-[11px] font-medium text-white hover:bg-blue-800"
            >
              查看试用完成度
            </NavLink>
          </div>
        </div>
      )}
      <main className="flex-1 overflow-hidden">
        <ErrorBoundary>
          <Routes>
            <Route path="/workspace" element={<DashboardPage />} />
            <Route path="/" element={<DashboardPage />} />
            <Route path="/map" element={<LifeMapPage />} />
            <Route path="/agent" element={<ChatPage />} />
            <Route path="/chat" element={<ChatPage />} />
            <Route path="/versions" element={<VersionControl />} />
            <Route path="/memory" element={<MemorySearch />} />
            <Route path="/a2a" element={<A2APage />} />
            <Route path="/mcp" element={<McpPage />} />
            <Route path="/builder" element={<BuilderPage />} />
            <Route path="/calibration" element={<CalibrationPage />} />
            <Route path="/runs" element={<RunsPage />} />
            <Route path="/runs/:runId" element={<AgentRunDetail />} />
            <Route path="/review" element={<ProposalReviewPage />} />
            <Route path="/metrics" element={<MetricsPage />} />
            <Route path="/settings" element={<SettingsPage />} />
          </Routes>
        </ErrorBoundary>
      </main>
      {wizardReady && showWizard && <OnboardingWizard onComplete={() => setShowWizard(false)} />}
    </div>
  );
}

export default App;
