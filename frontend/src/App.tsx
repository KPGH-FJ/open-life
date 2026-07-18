import React, { Component, ReactNode, Suspense, useEffect, useState } from "react";
import { Navigate, Routes, Route, useLocation } from "react-router-dom";
import LoadingSpinner from "./components/LoadingSpinner";
import ProductShell from "./components/ProductShell";

// Lazy load page components for code splitting
const CompanionPage = React.lazy(() => import("./pages/CompanionPage"));
const VersionControl = React.lazy(() => import("./pages/VersionControl"));
const MemorySearch = React.lazy(() => import("./pages/MemorySearch"));
const A2APage = React.lazy(() => import("./pages/A2APage"));
const McpPage = React.lazy(() => import("./pages/McpPage"));
const BuilderPage = React.lazy(() => import("./pages/BuilderPage"));
const LifeModelPage = React.lazy(() => import("./pages/LifeModelPage"));
const TodayPage = React.lazy(() => import("./pages/TodayPage"));
const TodayV2PreviewPage = React.lazy(() => import("./pages/TodayV2PreviewPage"));
const SettingsPage = React.lazy(() => import("./pages/SettingsPage"));
const CalibrationPage = React.lazy(() => import("./pages/CalibrationPage"));
const MailboxPage = React.lazy(() => import("./pages/MailboxPage"));
const RunsPage = React.lazy(() => import("./pages/RunsPage"));
const AgentRunDetail = React.lazy(() => import("./pages/AgentRunDetail"));
const MetricsPage = React.lazy(() => import("./pages/MetricsPage"));
import { getSystemDiagnostics, type SystemDiagnostics } from "./tauri";
import {
  advancedRoutePath,
  LEGACY_PRODUCT_REDIRECTS,
  productRoutePath,
  runDetailRoutePattern,
  secondaryRoutePath,
} from "./productShellContract";
import { getSafeModeReason, isSafeMode } from "./utils/safeMode";
import { initPerformanceMonitoring } from "./utils/performance";

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
  handleDismiss = () => {
    this.setState({ hasError: false, error: "", errorInfo: "" });
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
                  重启应用
                </button>
                <button
                  onClick={this.handleDismiss}
                  className="flex-1 bg-white text-stone-600 border border-stone-300 px-4 py-2.5 rounded-lg text-sm font-medium hover:bg-stone-50 transition-colors"
                >
                  继续使用
                </button>
                <button
                  onClick={() => (window.location.href = "#/today")}
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

function RedirectWithState({ to }: { to: string }) {
  const location = useLocation();
  return (
    <Navigate
      to={{ pathname: to, search: location.search, hash: location.hash }}
      state={location.state}
      replace
    />
  );
}

export function DevelopmentExtensionRoute({
  diagnosticsResolved,
  enabled,
  children,
}: {
  diagnosticsResolved: boolean;
  enabled: boolean;
  children: ReactNode;
}) {
  if (!diagnosticsResolved) {
    return <LoadingSpinner text="正在确认扩展能力..." />;
  }
  if (!enabled) {
    return <Navigate to={productRoutePath("Settings")} replace />;
  }
  return children;
}

function App() {
  const [diagnostics, setDiagnostics] = useState<SystemDiagnostics | null>(null);
  const [diagnosticsResolved, setDiagnosticsResolved] = useState(false);

  useEffect(() => {
    getSystemDiagnostics()
      .then(setDiagnostics)
      .catch(() => setDiagnostics(null))
      .finally(() => setDiagnosticsResolved(true));
  }, []);

  useEffect(() => {
    initPerformanceMonitoring();
  }, []);

  const safeMode = isSafeMode(diagnostics);
  const safeModeReason = getSafeModeReason(diagnostics);
  const devExtensionsEnabled = diagnostics?.runtime_build_info?.devExtensionsEnabled === true;

  return (
    <>
      <ProductShell diagnostics={diagnostics} safeMode={safeMode} safeModeReason={safeModeReason}>
        <ErrorBoundary>
          <Suspense fallback={<LoadingSpinner text="加载中..." />}>
            <Routes>
              <Route path={productRoutePath("Today")} element={<TodayPage />} />
              <Route path="/today-v2-preview" element={<TodayV2PreviewPage />} />
              <Route path={productRoutePath("Companion")} element={<CompanionPage />} />
              <Route path={productRoutePath("Life Model")} element={<LifeModelPage />} />
              <Route path={productRoutePath("Mailbox")} element={<MailboxPage />} />
              <Route path={productRoutePath("Runs")} element={<RunsPage />} />
              <Route path={productRoutePath("Settings")} element={<SettingsPage />} />
              {LEGACY_PRODUCT_REDIRECTS.map(route => (
                <Route
                  key={route.from}
                  path={route.from}
                  element={<RedirectWithState to={route.to} />}
                />
              ))}
              <Route path={runDetailRoutePattern()} element={<AgentRunDetail />} />
              <Route path={secondaryRoutePath("LifeModelBuild")} element={<BuilderPage />} />
              <Route path={secondaryRoutePath("Memory")} element={<MemorySearch />} />
              <Route path={advancedRoutePath("Versions")} element={<VersionControl />} />
              <Route
                path={advancedRoutePath("McpTools")}
                element={
                  <DevelopmentExtensionRoute
                    diagnosticsResolved={diagnosticsResolved}
                    enabled={devExtensionsEnabled}
                  >
                    <McpPage />
                  </DevelopmentExtensionRoute>
                }
              />
              <Route
                path={advancedRoutePath("A2A")}
                element={
                  <DevelopmentExtensionRoute
                    diagnosticsResolved={diagnosticsResolved}
                    enabled={devExtensionsEnabled}
                  >
                    <A2APage />
                  </DevelopmentExtensionRoute>
                }
              />
              <Route path={advancedRoutePath("Calibration")} element={<CalibrationPage />} />
              <Route path={advancedRoutePath("Metrics")} element={<MetricsPage />} />
            </Routes>
          </Suspense>
        </ErrorBoundary>
      </ProductShell>
    </>
  );
}

export default App;
