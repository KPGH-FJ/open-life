import { Component, ReactNode, useEffect, useState } from "react";
import { Routes, Route, NavLink } from "react-router-dom";
import {
  Brain,
  Hammer,
  LayoutDashboard,
  Settings,
  Sparkles,
  ShieldCheck,
  Bot,
  History,
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
  { hasError: boolean; error: string; errorInfo: string }
> {
  constructor(props: { children: ReactNode }) {
    super(props);
    this.state = { hasError: false, error: "", errorInfo: "" };
  }
  static getDerivedStateFromError(error: any) {
    return { hasError: true, error: String(error && error.stack ? error.stack : error) };
  }
  componentDidCatch(error: any, info: any) {
    // eslint-disable-next-line no-console
    console.error("App ErrorBoundary caught:", error, info);
    this.setState({ errorInfo: info.componentStack || "" });
  }
  handleRetry = () => {
    this.setState({ hasError: false, error: "", errorInfo: "" });
    window.location.reload();
  };
  handleCopyError = () => {
    const errorText = `Error: ${this.state.error}\n\nComponent Stack:\n${this.state.errorInfo}`;
    navigator.clipboard.writeText(errorText).catch(() => {
      // Fallback for environments without clipboard API
    });
  };
  render() {
    if (this.state.hasError) {
      return (
        <div className="min-h-screen flex items-center justify-center bg-gray-50 p-4">
          <div className="max-w-lg w-full bg-white rounded-xl shadow-lg border border-red-100 overflow-hidden">
            <div className="bg-red-50 px-6 py-4 border-b border-red-100">
              <div className="flex items-center gap-3">
                <div className="w-10 h-10 rounded-full bg-red-100 flex items-center justify-center text-red-600">
                  <svg className="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      strokeWidth={2}
                      d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"
                    />
                  </svg>
                </div>
                <div>
                  <h2 className="font-bold text-red-900">页面渲染出错</h2>
                  <p className="text-sm text-red-700">OpenLife 遇到了意外错误</p>
                </div>
              </div>
            </div>
            <div className="p-6 space-y-4">
              <div className="bg-gray-50 rounded-lg p-4 border border-gray-200">
                <div className="flex items-center justify-between mb-2">
                  <span className="text-xs font-medium text-gray-500 uppercase tracking-wider">
                    错误详情
                  </span>
                  <button
                    onClick={this.handleCopyError}
                    className="text-xs text-stone-600 hover:text-stone-900 flex items-center gap-1 px-2 py-1 rounded hover:bg-gray-200 transition-colors"
                  >
                    <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path
                        strokeLinecap="round"
                        strokeLinejoin="round"
                        strokeWidth={2}
                        d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z"
                      />
                    </svg>
                    复制错误信息
                  </button>
                </div>
                <pre className="whitespace-pre-wrap text-xs text-red-700 font-mono bg-red-50 p-3 rounded border border-red-100 max-h-40 overflow-auto">
                  {this.state.error}
                </pre>
              </div>
              <div className="flex gap-3">
                <button
                  onClick={this.handleRetry}
                  className="flex-1 bg-stone-900 text-white px-4 py-2.5 rounded-lg text-sm font-medium hover:bg-stone-800 transition-colors flex items-center justify-center gap-2"
                >
                  <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      strokeWidth={2}
                      d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"
                    />
                  </svg>
                  重试
                </button>
                <button
                  onClick={() => (window.location.href = "#/")}
                  className="flex-1 bg-white text-stone-700 border border-stone-300 px-4 py-2.5 rounded-lg text-sm font-medium hover:bg-stone-50 transition-colors"
                >
                  返回首页
                </button>
              </div>
              <p className="text-xs text-gray-500 text-center">
                如果问题持续存在，请尝试重启应用或联系支持团队
              </p>
            </div>
          </div>
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
          <NavLink to="/chat" className={navClass}>
            <Bot size={16} /> Chat
          </NavLink>
          <NavLink to="/review" className={navClass}>
            <ShieldCheck size={16} /> Review
          </NavLink>
          <NavLink to="/runs" className={navClass}>
            <History size={16} /> Runs
          </NavLink>
          <NavLink to="/settings" className={navClass}>
            <Settings size={16} /> Settings
          </NavLink>
          <NavLink to="/life" className={navClass}>
            <Hammer size={16} /> Life
          </NavLink>
          <NavLink to="/memory" className={navClass}>
            <Brain size={16} /> Memory
          </NavLink>
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
            <Route path="/" element={<DashboardPage />} />
            <Route path="/workspace" element={<DashboardPage />} />
            {/* Agent: Chat + Runs */}
            <Route path="/agent" element={<ChatPage />} />
            <Route path="/chat" element={<ChatPage />} />
            <Route path="/runs" element={<RunsPage />} />
            <Route path="/runs/:runId" element={<AgentRunDetail />} />
            {/* Life: Builder + LifeModel */}
            <Route path="/life" element={<BuilderPage />} />
            <Route path="/builder" element={<BuilderPage />} />
            <Route path="/map" element={<LifeMapPage />} />
            {/* Memory */}
            <Route path="/memory" element={<MemorySearch />} />
            {/* Review */}
            <Route path="/review" element={<ProposalReviewPage />} />
            {/* Settings + Experimental */}
            <Route path="/settings" element={<SettingsPage />} />
            <Route path="/versions" element={<VersionControl />} />
            <Route path="/mcp" element={<McpPage />} />
            <Route path="/a2a" element={<A2APage />} />
            <Route path="/calibration" element={<CalibrationPage />} />
            <Route path="/metrics" element={<MetricsPage />} />
          </Routes>
        </ErrorBoundary>
      </main>
      {wizardReady && showWizard && <OnboardingWizard onComplete={() => setShowWizard(false)} />}
    </div>
  );
}

export default App;
