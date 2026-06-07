import type { KeyboardEvent, ReactNode } from "react";
import { useRef, useState } from "react";
import { NavLink, useLocation } from "react-router-dom";
import type { LucideIcon } from "lucide-react";
import {
  Activity,
  Brain,
  CalendarDays,
  CircleEllipsis,
  GitBranch,
  HeartHandshake,
  History,
  Inbox,
  Network,
  Settings,
  SlidersHorizontal,
  Sparkles,
  Wrench,
} from "lucide-react";
import { PRIMARY_PRODUCT_ROUTES, type ProductRouteLabel } from "../productShellContract";
import type { SystemDiagnostics } from "../tauri";

type ProductShellProps = {
  children: ReactNode;
  diagnostics: SystemDiagnostics | null;
  safeMode: boolean;
  safeModeReason: string;
};

const PRODUCT_TAB_ICONS: Record<ProductRouteLabel, LucideIcon> = {
  陪伴: HeartHandshake,
  今日: CalendarDays,
  "Life Model": Brain,
  邮箱: Inbox,
};

const PRODUCT_ROUTE_ALIASES: Record<ProductRouteLabel, readonly string[]> = {
  陪伴: ["/companion", "/chat", "/agent"],
  今日: ["/today", "/", "/workspace"],
  "Life Model": ["/life-model", "/builder", "/life", "/map", "/memory"],
  邮箱: ["/mailbox", "/review"],
};

const SECONDARY_DIRECT_TOOLS = [
  { label: "Runs", path: "/runs", icon: History },
  { label: "Settings", path: "/settings", icon: Settings },
] as const;

const SECONDARY_MENU_TOOLS = [
  { label: "MCP", path: "/mcp", icon: Wrench },
  { label: "A2A", path: "/a2a", icon: Network },
  { label: "Metrics", path: "/metrics", icon: Activity },
  { label: "Versions", path: "/versions", icon: GitBranch },
  { label: "Calibration", path: "/calibration", icon: SlidersHorizontal },
] as const;

function matchesRoute(pathname: string, routePath: string): boolean {
  if (routePath === "/") {
    return pathname === "/";
  }
  return pathname === routePath || pathname.startsWith(`${routePath}/`);
}

function isProductRouteActive(label: ProductRouteLabel, pathname: string): boolean {
  return PRODUCT_ROUTE_ALIASES[label].some(routePath => matchesRoute(pathname, routePath));
}

function secondaryLinkClass({ isActive }: { isActive: boolean }): string {
  return [
    "inline-flex h-9 items-center gap-2 rounded-md px-3 text-sm font-medium transition",
    "border border-transparent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-stone-900/20",
    isActive
      ? "bg-stone-900 text-white shadow-sm"
      : "text-stone-700 hover:border-stone-200 hover:bg-white",
  ].join(" ");
}

export function MainTabs() {
  const location = useLocation();

  return (
    <nav
      aria-label="Primary product navigation"
      className="flex min-w-0 flex-1 items-center justify-center"
    >
      <div className="grid w-full max-w-[520px] grid-cols-4 rounded-lg border border-stone-200 bg-white p-1 shadow-sm">
        {PRIMARY_PRODUCT_ROUTES.map(route => {
          const Icon = PRODUCT_TAB_ICONS[route.label];
          const active = isProductRouteActive(route.label, location.pathname);

          return (
            <NavLink
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
              <Icon size={16} aria-hidden="true" className="shrink-0" />
              <span className="truncate">{route.label}</span>
            </NavLink>
          );
        })}
      </div>
    </nav>
  );
}

export function SecondaryToolsMenu() {
  const [open, setOpen] = useState(false);
  const buttonRef = useRef<HTMLButtonElement | null>(null);
  const firstMenuItemRef = useRef<HTMLAnchorElement | null>(null);

  const closeMenu = (restoreFocus = false) => {
    setOpen(false);
    if (restoreFocus) {
      window.setTimeout(() => buttonRef.current?.focus(), 0);
    }
  };

  const focusFirstMenuItem = () => {
    setOpen(true);
    window.setTimeout(() => firstMenuItemRef.current?.focus(), 0);
  };

  const handleButtonKeyDown = (event: KeyboardEvent<HTMLButtonElement>) => {
    if (event.key === "Escape" && open) {
      event.preventDefault();
      closeMenu(true);
      return;
    }
    if (event.key === "ArrowDown") {
      event.preventDefault();
      focusFirstMenuItem();
    }
  };

  const handleMenuKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key === "Escape") {
      event.preventDefault();
      closeMenu(true);
      return;
    }
    if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;

    const items = Array.from(event.currentTarget.querySelectorAll<HTMLAnchorElement>("a[href]"));
    if (!items.length) return;
    event.preventDefault();
    const currentIndex = items.indexOf(document.activeElement as HTMLAnchorElement);
    const fallbackIndex = event.key === "ArrowDown" ? 0 : items.length - 1;
    const nextIndex =
      currentIndex === -1
        ? fallbackIndex
        : (currentIndex + (event.key === "ArrowDown" ? 1 : -1) + items.length) % items.length;
    items[nextIndex]?.focus();
  };

  return (
    <nav aria-label="Secondary tools" className="relative flex items-center gap-1.5">
      {SECONDARY_DIRECT_TOOLS.map(item => {
        const Icon = item.icon;
        return (
          <NavLink key={item.path} to={item.path} className={secondaryLinkClass}>
            <Icon size={15} aria-hidden="true" />
            <span>{item.label}</span>
          </NavLink>
        );
      })}
      <button
        ref={buttonRef}
        type="button"
        aria-label="二级入口"
        aria-expanded={open}
        aria-controls="secondary-tools-menu"
        title="二级入口"
        onClick={() => setOpen(value => !value)}
        onKeyDown={handleButtonKeyDown}
        className={[
          "inline-flex h-9 w-9 items-center justify-center rounded-md border border-transparent",
          "text-stone-700 transition hover:border-stone-200 hover:bg-white",
          "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-stone-900/20",
          open ? "bg-white shadow-sm" : "",
        ].join(" ")}
      >
        <CircleEllipsis size={18} aria-hidden="true" />
      </button>
      {open && (
        <div
          id="secondary-tools-menu"
          onKeyDown={handleMenuKeyDown}
          className="absolute right-0 top-11 z-30 min-w-[190px] rounded-lg border border-stone-200 bg-white p-1.5 shadow-xl"
        >
          {SECONDARY_MENU_TOOLS.map((item, index) => {
            const Icon = item.icon;
            return (
              <NavLink
                key={item.path}
                ref={index === 0 ? firstMenuItemRef : undefined}
                to={item.path}
                onClick={() => setOpen(false)}
                className={({ isActive }) =>
                  [
                    "flex items-center gap-2 rounded-md px-3 py-2 text-sm font-medium transition",
                    "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-stone-900/20",
                    isActive
                      ? "bg-stone-900 text-white"
                      : "text-stone-700 hover:bg-stone-100 hover:text-stone-950",
                  ].join(" ")
                }
              >
                <Icon size={15} aria-hidden="true" />
                <span>{item.label}</span>
              </NavLink>
            );
          })}
        </div>
      )}
    </nav>
  );
}

export default function ProductShell({
  children,
  diagnostics,
  safeMode,
  safeModeReason,
}: ProductShellProps) {
  return (
    <div className="h-screen min-h-0 overflow-hidden bg-[#f5f6f2] text-stone-950">
      <div className="flex h-full min-h-0 flex-col">
        <header className="shrink-0 border-b border-stone-200 bg-[#fcfcf8]/95 px-4 py-3">
          <div className="mx-auto flex max-w-[1500px] flex-wrap items-center gap-3 lg:flex-nowrap">
            <div className="flex min-w-[170px] flex-1 items-center gap-3 lg:flex-none">
              <div className="flex h-9 w-9 items-center justify-center rounded-lg bg-stone-950 text-white shadow-sm">
                <Sparkles size={18} aria-hidden="true" />
              </div>
              <div className="min-w-0">
                <h1 className="truncate text-base font-bold tracking-normal text-stone-950">
                  OpenLife
                </h1>
              </div>
              {diagnostics && (
                <NavLink
                  to="/settings"
                  className={`hidden rounded-md px-2.5 py-1 text-[11px] font-semibold md:inline-flex ${
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
            <div className="order-3 w-full lg:order-none lg:flex-1">
              <MainTabs />
            </div>
            <SecondaryToolsMenu />
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
                  建议先去设置页的“恢复控制台”导出备份，再继续试用。
                </div>
              </div>
              <div className="flex gap-2">
                <NavLink
                  to="/settings"
                  className="rounded-md bg-amber-900 px-3 py-1.5 text-xs font-medium text-amber-50 hover:bg-amber-950"
                >
                  打开恢复控制台
                </NavLink>
                <NavLink
                  to="/memory"
                  className="rounded-md border border-amber-300 bg-white px-3 py-1.5 text-xs font-medium text-amber-900 hover:bg-amber-100"
                >
                  查看记忆状态
                </NavLink>
              </div>
            </div>
          </div>
        )}
        {!safeMode && diagnostics && !diagnostics.beta_ready && (
          <div
            data-testid="beta-readiness-banner"
            className="shrink-0 border-b border-blue-100 bg-blue-50/90 px-4 py-2.5"
          >
            <div className="mx-auto flex max-w-[1500px] flex-wrap items-center justify-between gap-3">
              <div className="text-xs text-blue-900">
                <span className="font-semibold">Beta 试用准备中：</span>
                {diagnostics.beta_readiness_issues?.[0] ??
                  "继续完成设置、构建和首轮对话，就能形成完整试用闭环。"}
              </div>
              <NavLink
                to="/settings"
                className="rounded-md bg-blue-700 px-3 py-1 text-[11px] font-medium text-white hover:bg-blue-800"
              >
                查看试用完成度
              </NavLink>
            </div>
          </div>
        )}
        <main className="min-h-0 flex-1 overflow-hidden">{children}</main>
      </div>
    </div>
  );
}
