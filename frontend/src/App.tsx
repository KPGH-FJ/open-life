import { Component, type ErrorInfo, type ReactNode, useEffect } from "react";
import { AlertTriangle, Copy, Home, RefreshCw } from "lucide-react";
import { Navigate, useLocation, useNavigate } from "react-router-dom";
import { FoundationActionButton, FoundationNotice } from "@/ui/foundation";
import { tauriPersonalIntelligenceDataSource } from "@/features/personalIntelligence/personalIntelligenceDataSource";
import { tauriWorkbenchDataSource } from "@/app/workbenchDataSource";
import { tauriConversationDataSource } from "@/features/conversation/conversationDataSource";
import {
  ProductWorkbench,
  type PublicProductSurfaceId,
  type ProductWorkbenchRouteState,
} from "@/app/ProductWorkbench";
import { tauriSettingsDataSource } from "@/features/settings/settingsDataSource";
import { productErrorCode } from "@/shared/productError";
import {
  isRetiredProductPath,
  productPath,
  resolveProductionRoute,
  SETTINGS_ROUTE_PATH,
} from "@/app/routes";
import { initPerformanceMonitoring } from "@/utils/performance";

type ErrorBoundaryState = {
  hasError: boolean;
  error: string;
  errorInfo: string;
  copyStatus: "idle" | "copied" | "failed";
};

class ErrorBoundary extends Component<{ children: ReactNode }, ErrorBoundaryState> {
  state: ErrorBoundaryState = {
    hasError: false,
    error: "",
    errorInfo: "",
    copyStatus: "idle",
  };

  static getDerivedStateFromError(error: unknown): Partial<ErrorBoundaryState> {
    return {
      hasError: true,
      error: error instanceof Error ? (error.stack ?? error.message) : productErrorCode(error),
    };
  }

  componentDidCatch(error: unknown, info: ErrorInfo): void {
    console.error("App ErrorBoundary caught:", error, info);
    this.setState({ errorInfo: info.componentStack ?? "" });
  }

  private reload = (): void => {
    window.location.reload();
  };

  private goWorkbench = (): void => {
    window.location.hash = productPath("workspace");
    window.location.reload();
  };

  private copyError = async (): Promise<void> => {
    const value = `Error: ${this.state.error}\n\nComponent Stack:\n${this.state.errorInfo}`;
    try {
      await navigator.clipboard.writeText(value);
      this.setState({ copyStatus: "copied" });
    } catch {
      this.setState({ copyStatus: "failed" });
    }
  };

  render() {
    if (!this.state.hasError) return this.props.children;
    return (
      <main className="ol-foundation ol-app-failure" aria-labelledby="app-failure-title">
        <AlertTriangle size={28} strokeWidth={1.75} aria-hidden="true" />
        <FoundationNotice title="界面暂时无法继续" tone="error" live>
          <p id="app-failure-title">
            当前页面渲染失败。重新载入前不会把缺失状态解释为任务、审核或写入已完成。
          </p>
        </FoundationNotice>
        <div className="ol-app-failure__actions">
          <FoundationActionButton
            label="重新载入"
            variant="primary"
            icon={<RefreshCw size={17} aria-hidden="true" />}
            onClick={this.reload}
          />
          <FoundationActionButton
            label="返回工作台"
            variant="secondary"
            icon={<Home size={17} aria-hidden="true" />}
            onClick={this.goWorkbench}
          />
          <FoundationActionButton
            label="复制错误信息"
            variant="quiet"
            icon={<Copy size={17} aria-hidden="true" />}
            onClick={() => void this.copyError()}
          />
        </div>
        <p className="ol-app-failure__feedback" role="status" aria-live="polite">
          {this.state.copyStatus === "copied"
            ? "错误信息已复制。"
            : this.state.copyStatus === "failed"
              ? "无法访问剪贴板；可展开下方技术信息。"
              : ""}
        </p>
        <details className="ol-app-failure__details">
          <summary>技术信息</summary>
          <pre>{this.state.error}</pre>
          {this.state.errorInfo && <pre>{this.state.errorInfo}</pre>}
        </details>
      </main>
    );
  }
}

function validReturnSurface(value: unknown): PublicProductSurfaceId {
  return value === "workspace" || value === "life-model" ? value : "workspace";
}

function UnavailableRoute({ pathname }: { pathname: string }) {
  const navigate = useNavigate();
  const retired = isRetiredProductPath(pathname);
  return (
    <main className="ol-foundation ol-route-unavailable" aria-labelledby="route-unavailable-title">
      <span>{retired ? "入口已退役" : "路径不可用"}</span>
      <h1 id="route-unavailable-title">
        {retired ? "这个旧页面已从产品中移除" : "OpenLife 没有这个产品页面"}
      </h1>
      <p>
        {retired
          ? "当前不会跳回旧界面，也不会把旧路径重定向成另一项产品操作。"
          : "请从当前桌面工作台的主导航进入已支持区域。"}
      </p>
      <code>{pathname}</code>
      <FoundationActionButton
        label="返回工作台"
        variant="primary"
        icon={<Home size={17} aria-hidden="true" />}
        onClick={() => navigate(productPath("workspace"), { replace: true })}
      />
    </main>
  );
}

function ProductionWorkbenchRoute() {
  const location = useLocation();
  const navigate = useNavigate();

  if (location.pathname === "/") {
    return <Navigate to={productPath("workspace")} replace />;
  }

  const locationState = location.state as { returnSurface?: unknown } | null;
  const route = resolveProductionRoute(
    location.pathname,
    validReturnSurface(locationState?.returnSurface)
  );
  if (!route) return <UnavailableRoute pathname={location.pathname} />;

  function changeRoute(next: ProductWorkbenchRouteState): void {
    if (next.mode === "settings") {
      if (location.pathname === SETTINGS_ROUTE_PATH) return;
      navigate(SETTINGS_ROUTE_PATH, { state: { returnSurface: next.surface } });
      return;
    }
    const nextPath = productPath(next.surface);
    if (location.pathname !== nextPath) navigate(nextPath);
  }

  return (
    <ProductWorkbench
      workbenchDataSource={tauriWorkbenchDataSource}
      personalIntelligenceDataSource={tauriPersonalIntelligenceDataSource}
      settingsDataSource={tauriSettingsDataSource}
      conversationDataSource={tauriConversationDataSource}
      initialMode={route.mode}
      initialSurface={route.surface}
      onRouteChange={changeRoute}
    />
  );
}

export default function App() {
  useEffect(() => {
    initPerformanceMonitoring();
  }, []);

  return (
    <ErrorBoundary>
      <ProductionWorkbenchRoute />
    </ErrorBoundary>
  );
}
