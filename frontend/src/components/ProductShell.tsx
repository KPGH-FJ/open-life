import type { KeyboardEvent, ReactNode } from "react";
import { useEffect, useRef, useState } from "react";
import { Link, NavLink, useLocation } from "react-router-dom";
import { CircleEllipsis } from "lucide-react";
import {
  ADVANCED_PRODUCT_ROUTE_GROUPS,
  diagnosticsUsageReadinessIssues,
  diagnosticsUsageReady,
  PRIMARY_PRODUCT_ROUTES,
  productRoutePath,
  secondaryRoutePath,
  type ProductRouteLabel,
} from "../productShellContract";
import type { SystemDiagnostics } from "../tauri";

type ProductShellProps = {
  children: ReactNode;
  diagnostics: SystemDiagnostics | null;
  safeMode: boolean;
  safeModeReason: string;
};

const PRODUCT_ROUTE_ALIASES: Record<ProductRouteLabel, readonly string[]> = {
  Today: [productRoutePath("Today")],
  Companion: [productRoutePath("Companion")],
  Mailbox: [productRoutePath("Mailbox")],
  "Life Model": [
    productRoutePath("Life Model"),
    secondaryRoutePath("LifeModelBuild"),
    secondaryRoutePath("Memory"),
  ],
  Runs: [productRoutePath("Runs")],
  Settings: [productRoutePath("Settings")],
};

function matchesRoute(pathname: string, routePath: string): boolean {
  if (routePath === "/") {
    return pathname === "/";
  }
  return pathname === routePath || pathname.startsWith(`${routePath}/`);
}

function isProductRouteActive(label: ProductRouteLabel, pathname: string): boolean {
  return PRODUCT_ROUTE_ALIASES[label].some(routePath => matchesRoute(pathname, routePath));
}

function shortSha(sha?: string): string {
  if (!sha || sha === "unknown") return "unknown";
  return sha.slice(0, 7);
}

function runtimeBadgeLabel(diagnostics: SystemDiagnostics | null): string | null {
  const info = diagnostics?.runtime_build_info;
  if (!info) return null;
  const shouldShow =
    info.profile !== "release" ||
    info.frontendMode === "dev_server" ||
    info.binaryKind === "debug_binary" ||
    info.binaryKind === "debug_bundle";
  if (!shouldShow) return null;

  const profileLabel =
    info.profile === "dev" ? "Dev" : info.profile === "qa" ? "QA" : info.profile || "Runtime";
  const source = info.frontendMode === "dev_server" ? info.devUrl || "" : "";
  const port = source.match(/:(\d+)(?:\/)?$/)?.[1] ?? "";
  const portLabel = port || (info.frontendMode === "dev_server" ? "dev server" : info.binaryKind);
  return `OpenLife ${profileLabel} · ${portLabel} · ${shortSha(info.gitSha)}`;
}

function usageReady(diagnostics: SystemDiagnostics): boolean {
  return diagnosticsUsageReady(diagnostics);
}

function usageReadinessIssues(diagnostics: SystemDiagnostics): string[] {
  return diagnosticsUsageReadinessIssues(diagnostics);
}

export function MainTabs() {
  const location = useLocation();

  return (
    <nav
      aria-label="Primary product navigation"
      className="flex min-w-0 items-center justify-center"
    >
      <div className="grid w-full min-w-0 max-w-[760px] grid-cols-3 rounded-lg border border-stone-200 bg-white p-1 shadow-sm sm:grid-cols-6">
        {PRIMARY_PRODUCT_ROUTES.map(route => {
          const active = isProductRouteActive(route.label, location.pathname);

          return (
            <Link
              key={route.path}
              to={route.path}
              aria-current={active ? "page" : undefined}
              className={[
                "inline-flex h-10 min-w-0 items-center justify-center gap-2 rounded-md px-2",
                "text-sm font-semibold transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-stone-900/20",
                active
                  ? "bg-stone-900 text-white shadow-sm"
                  : "text-stone-600 hover:bg-stone-100 hover:text-stone-950",
              ].join(" ")}
            >
              <span className="truncate">{route.label}</span>
            </Link>
          );
        })}
      </div>
    </nav>
  );
}

function SecondaryToolsMenu({ devExtensionsEnabled }: { devExtensionsEnabled: boolean }) {
  const [open, setOpen] = useState(false);
  const buttonRef = useRef<HTMLButtonElement | null>(null);
  const location = useLocation();

  const closeMenu = () => {
    setOpen(false);
    buttonRef.current?.focus();
  };

  const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key === "Escape" && open) {
      event.preventDefault();
      closeMenu();
    }
  };

  useEffect(() => {
    setOpen(false);
  }, [location.pathname]);

  const routeGroups = ADVANCED_PRODUCT_ROUTE_GROUPS.map(group => ({
    ...group,
    items: group.items.filter(
      item => devExtensionsEnabled || (item.path !== "/mcp" && item.path !== "/a2a")
    ),
  })).filter(group => group.items.length > 0);

  return (
    <div className="relative" onKeyDown={handleKeyDown}>
      <button
        ref={buttonRef}
        type="button"
        aria-expanded={open}
        aria-controls="secondary-tools-panel"
        onClick={() => setOpen(current => !current)}
        className="inline-flex h-9 items-center gap-1.5 rounded-md border border-stone-200 bg-white px-2.5 text-xs font-semibold text-stone-700 shadow-sm hover:bg-stone-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-stone-900/20"
      >
        <CircleEllipsis size={15} aria-hidden="true" />
        Advanced
      </button>
      {open && (
        <nav
          id="secondary-tools-panel"
          aria-label="Advanced technical navigation"
          className="absolute right-0 top-11 z-30 w-56 rounded-lg border border-stone-200 bg-white p-1.5 shadow-lg"
        >
          <div className="px-3 pb-1 pt-1 text-[11px] font-semibold text-stone-500">
            Technical surfaces
          </div>
          {routeGroups.map(group => (
            <div key={group.label} className="py-1">
              <div className="px-3 pb-1 pt-1 text-[11px] font-semibold text-stone-400">
                {group.label}
              </div>
              {group.items.map(item => (
                <NavLink
                  key={item.path}
                  to={item.path}
                  onClick={() => setOpen(false)}
                  className={({ isActive }) =>
                    [
                      "flex h-9 items-center rounded-md px-3 text-sm font-medium transition",
                      isActive ? "bg-stone-900 text-white" : "text-stone-700 hover:bg-stone-100",
                    ].join(" ")
                  }
                >
                  {item.label}
                </NavLink>
              ))}
            </div>
          ))}
        </nav>
      )}
    </div>
  );
}

export default function ProductShell({
  children,
  diagnostics,
  safeMode,
  safeModeReason,
}: ProductShellProps) {
  const badgeLabel = runtimeBadgeLabel(diagnostics);
  const devExtensionsEnabled = diagnostics?.runtime_build_info?.devExtensionsEnabled === true;

  return (
    <div className="h-screen min-h-0 overflow-hidden bg-[#f5f6f2] text-stone-950">
      <div className="flex h-full min-h-0 flex-col">
        <header className="shrink-0 border-b border-stone-200 bg-[#fcfcf8]/95 px-4 py-2">
          <div className="mx-auto grid max-w-[1500px] grid-cols-[minmax(0,1fr)_auto] items-center gap-2 sm:grid-cols-[1fr_auto_1fr] sm:gap-3">
            <div className="hidden min-w-0 sm:block">
              {badgeLabel && (
                <div className="inline-flex max-w-full items-center rounded-md border border-emerald-200 bg-emerald-50 px-2 py-1 text-[11px] font-semibold text-emerald-800">
                  <span className="truncate">{badgeLabel}</span>
                </div>
              )}
            </div>
            <MainTabs />
            <div className="flex min-w-0 justify-end gap-2">
              {badgeLabel && (
                <div className="inline-flex max-w-[160px] items-center rounded-md border border-emerald-200 bg-emerald-50 px-2 py-1 text-[11px] font-semibold text-emerald-800 sm:hidden">
                  <span className="truncate">{badgeLabel}</span>
                </div>
              )}
              <SecondaryToolsMenu devExtensionsEnabled={devExtensionsEnabled} />
            </div>
          </div>
        </header>
        {safeMode && diagnostics && (
          <div
            data-testid="safe-mode-banner"
            className="shrink-0 border-b border-amber-200 bg-amber-50 px-4 py-3"
          >
            <div className="mx-auto flex max-w-[1500px] flex-wrap items-start justify-between gap-3">
              <div>
                <div className="text-sm font-semibold text-amber-900">
                  Safe Mode：当前数据环境存在风险
                </div>
                <div className="mt-1 text-xs text-amber-800">{safeModeReason}</div>
                <div className="mt-1 text-xs text-amber-700">
                  建议先去设置页的“恢复控制台”导出备份，再继续使用。
                </div>
              </div>
              <div className="flex gap-2">
                <NavLink
                  to={productRoutePath("Settings")}
                  className="rounded-md bg-amber-900 px-3 py-1.5 text-xs font-medium text-amber-50 hover:bg-amber-950"
                >
                  打开恢复控制台
                </NavLink>
                <NavLink
                  to={secondaryRoutePath("Memory")}
                  className="rounded-md border border-amber-300 bg-white px-3 py-1.5 text-xs font-medium text-amber-900 hover:bg-amber-100"
                >
                  查看记忆状态
                </NavLink>
              </div>
            </div>
          </div>
        )}
        {!safeMode && diagnostics && !usageReady(diagnostics) && (
          <div
            data-testid="usage-readiness-banner"
            className="shrink-0 border-b border-blue-100 bg-blue-50/90 px-4 py-2.5"
          >
            <div className="mx-auto flex max-w-[1500px] flex-wrap items-center justify-between gap-3">
              <div className="text-xs text-blue-900">
                <span className="font-semibold">使用准备中：</span>
                {usageReadinessIssues(diagnostics)[0] ??
                  "继续完成设置、构建和首轮对话，就能形成完整使用闭环。"}
              </div>
              <NavLink
                to={productRoutePath("Settings")}
                className="rounded-md bg-blue-700 px-3 py-1 text-[11px] font-medium text-white hover:bg-blue-800"
              >
                查看准备状态
              </NavLink>
            </div>
          </div>
        )}
        <main className="min-h-0 flex-1 overflow-hidden">{children}</main>
      </div>
    </div>
  );
}
