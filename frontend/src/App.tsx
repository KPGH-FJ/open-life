import { Component, ReactNode, useEffect, useState } from "react";
import { Routes, Route, NavLink } from "react-router-dom";
import { Map, MessageSquare, GitBranch, Brain, Network, Hammer, LayoutDashboard, Settings, Wrench, SlidersHorizontal, Sparkles } from "lucide-react";
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
import OnboardingWizard from "./components/OnboardingWizard";
import { hasCompletedOnboarding } from "./tauri";

class ErrorBoundary extends Component<{ children: ReactNode }, { hasError: boolean; error: string }> {
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

  useEffect(() => {
    hasCompletedOnboarding()
      .then((done) => {
        setShowWizard(!done);
        setWizardReady(true);
      })
      .catch(() => {
        setShowWizard(true);
        setWizardReady(true);
      });
  }, []);

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
        </div>
        <nav className="flex flex-wrap justify-end gap-2">
          <NavLink
            to="/dashboard"
            className={navClass}
          >
            <LayoutDashboard size={16} /> 今日
          </NavLink>
          <NavLink
            to="/builder"
            className={navClass}
          >
            <Hammer size={16} /> 构建我
          </NavLink>
          <NavLink
            to="/chat"
            className={navClass}
          >
            <MessageSquare size={16} /> 对话
          </NavLink>
          <NavLink
            to="/"
            end
            className={navClass}
          >
            <Map size={16} /> 人生地图
          </NavLink>
          <NavLink
            to="/memory"
            className={navClass}
          >
            <Brain size={16} /> 记忆
          </NavLink>
          <NavLink
            to="/calibration"
            className={navClass}
          >
            <SlidersHorizontal size={16} /> 校准
          </NavLink>
          <details className="relative group">
            <summary className="list-none flex items-center gap-1.5 px-3 py-1.5 rounded-full text-sm font-medium text-stone-600 hover:bg-stone-100 cursor-pointer">
              <Wrench size={16} /> 高级
            </summary>
            <div className="absolute right-0 z-20 mt-2 w-44 rounded-xl border border-stone-200 bg-white p-2 shadow-lg">
              <NavLink to="/versions" className={navClass}>
                <GitBranch size={16} /> 版本控制
              </NavLink>
              <NavLink to="/mcp" className={navClass}>
                <Wrench size={16} /> MCP
              </NavLink>
              <NavLink to="/a2a" className={navClass}>
                <Network size={16} /> A2A
              </NavLink>
            </div>
          </details>
          <NavLink
            to="/settings"
            className={navClass}
          >
            <Settings size={16} /> 设置
          </NavLink>
        </nav>
      </header>
      <main className="flex-1 overflow-hidden">
        <ErrorBoundary>
          <Routes>
            <Route path="/dashboard" element={<DashboardPage />} />
            <Route path="/" element={<LifeMapPage />} />
            <Route path="/chat" element={<ChatPage />} />
            <Route path="/versions" element={<VersionControl />} />
            <Route path="/memory" element={<MemorySearch />} />
            <Route path="/a2a" element={<A2APage />} />
            <Route path="/mcp" element={<McpPage />} />
            <Route path="/builder" element={<BuilderPage />} />
            <Route path="/calibration" element={<CalibrationPage />} />
            <Route path="/settings" element={<SettingsPage />} />
          </Routes>
        </ErrorBoundary>
      </main>
      {wizardReady && showWizard && (
        <OnboardingWizard onComplete={() => setShowWizard(false)} />
      )}
    </div>
  );
}

export default App;
