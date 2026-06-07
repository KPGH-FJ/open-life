export const PRIMARY_PRODUCT_ROUTES = [
  { label: "陪伴", path: "/companion", legacyAlias: "/chat" },
  { label: "今日", path: "/today", legacyAlias: "/" },
  { label: "Life Model", path: "/life-model", legacyAlias: "/builder" },
  { label: "邮箱", path: "/mailbox", legacyAlias: "/review" },
] as const;

export type ProductRouteLabel = (typeof PRIMARY_PRODUCT_ROUTES)[number]["label"];

export function productRoutePath(label: ProductRouteLabel): string {
  const route = PRIMARY_PRODUCT_ROUTES.find(item => item.label === label);
  if (!route) {
    throw new Error(`Unknown product route label: ${label}`);
  }
  return route.path;
}

export const RETAINED_LEGACY_ROUTES = [
  "/chat",
  "/agent",
  "/review",
  "/builder",
  "/life",
  "/map",
  "/memory",
  "/runs",
  "/settings",
  "/mcp",
  "/a2a",
  "/metrics",
  "/versions",
  "/calibration",
] as const;

// Future AgentStage bitmap assets should live under frontend/public/assets/agent-stage.
export const AGENT_STAGE_ASSET_ROOT = "/assets/agent-stage" as const;
