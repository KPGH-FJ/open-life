export const PRIMARY_PRODUCT_ROUTES = [
  { label: "Today", path: "/today", legacyAlias: "/" },
  { label: "Companion", path: "/companion", legacyAlias: "/chat" },
  { label: "Review", path: "/mailbox", legacyAlias: "/review" },
  { label: "Life Model", path: "/life-model", legacyAlias: "/builder" },
  { label: "Runs", path: "/runs" },
  { label: "Settings", path: "/settings" },
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
  "/mcp",
  "/a2a",
  "/metrics",
  "/versions",
  "/calibration",
] as const;

export const ADVANCED_PRODUCT_ROUTE_GROUPS = [
  {
    label: "Advanced connections",
    items: [
      { label: "MCP / Tools", path: "/mcp" },
      { label: "A2A", path: "/a2a" },
    ],
  },
  {
    label: "Stage / debug / eval",
    items: [
      { label: "Metrics", path: "/metrics" },
      { label: "Calibration", path: "/calibration" },
      { label: "Versions", path: "/versions" },
    ],
  },
] as const;

// Future AgentStage bitmap assets should live under frontend/public/assets/agent-stage.
export const AGENT_STAGE_ASSET_ROOT = "/assets/agent-stage" as const;
