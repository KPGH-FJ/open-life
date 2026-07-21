import type { ReadOnlyProductSurfaceId, ReadOnlySpineRouteState } from "@/ui/journeys/readOnly";

export const PRODUCT_ROUTE_PATHS: Readonly<Record<ReadOnlyProductSurfaceId, string>> = {
  today: "/today",
  workspace: "/workspace",
  tasks: "/tasks",
  review: "/review",
  "life-model": "/life-model",
};

export const SETTINGS_ROUTE_PATH = "/settings";

const RETIRED_PRODUCT_PATHS = new Set([
  "/companion",
  "/mailbox",
  "/runs",
  "/memory",
  "/builder",
  "/versions",
  "/mcp",
  "/a2a",
  "/calibration",
  "/metrics",
]);

export type ProductionRouteResolution = ReadOnlySpineRouteState & {
  path: string;
};

export function productPath(surface: ReadOnlyProductSurfaceId): string {
  return PRODUCT_ROUTE_PATHS[surface];
}

export function resolveProductionRoute(
  pathname: string,
  settingsReturnSurface: ReadOnlyProductSurfaceId = "today"
): ProductionRouteResolution | null {
  if (pathname === SETTINGS_ROUTE_PATH) {
    return { mode: "settings", surface: settingsReturnSurface, path: SETTINGS_ROUTE_PATH };
  }
  const entry = Object.entries(PRODUCT_ROUTE_PATHS).find(([, path]) => path === pathname);
  if (!entry) return null;
  return {
    mode: "product",
    surface: entry[0] as ReadOnlyProductSurfaceId,
    path: entry[1],
  };
}

export function isRetiredProductPath(pathname: string): boolean {
  return RETIRED_PRODUCT_PATHS.has(pathname) || pathname.startsWith("/runs/");
}
