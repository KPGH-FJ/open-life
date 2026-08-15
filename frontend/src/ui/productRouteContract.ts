import type {
  PublicProductSurfaceId,
  ProductWorkbenchRouteState,
} from "@/ui/journeys/productWorkbench";

export const PRODUCT_ROUTE_PATHS: Readonly<Record<PublicProductSurfaceId, string>> = {
  workspace: "/workspace",
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
  "/calibration",
  "/metrics",
  "/today",
  "/tasks",
  "/review",
]);

export type ProductionRouteResolution = ProductWorkbenchRouteState & {
  path: string;
};

export function productPath(surface: PublicProductSurfaceId): string {
  return PRODUCT_ROUTE_PATHS[surface];
}

export function resolveProductionRoute(
  pathname: string,
  settingsReturnSurface: PublicProductSurfaceId = "workspace"
): ProductionRouteResolution | null {
  if (pathname === SETTINGS_ROUTE_PATH) {
    return { mode: "settings", surface: settingsReturnSurface, path: SETTINGS_ROUTE_PATH };
  }
  const entry = Object.entries(PRODUCT_ROUTE_PATHS).find(([, path]) => path === pathname);
  if (!entry) return null;
  return {
    mode: "product",
    surface: entry[0] as PublicProductSurfaceId,
    path: entry[1],
  };
}

export function isRetiredProductPath(pathname: string): boolean {
  return RETIRED_PRODUCT_PATHS.has(pathname) || pathname.startsWith("/runs/");
}
