export const PRIMARY_PRODUCT_ROUTES = [
  { label: "Today", path: "/today" },
  { label: "Companion", path: "/companion" },
  { label: "Mailbox", path: "/mailbox" },
  { label: "Life Model", path: "/life-model" },
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

export const RUN_DETAIL_ROUTE_PATTERN = "/runs/:runId" as const;

export function runDetailRoutePattern(): typeof RUN_DETAIL_ROUTE_PATTERN {
  return RUN_DETAIL_ROUTE_PATTERN;
}

export function runDetailRoute(runId: string): string {
  const normalized = runId.trim();
  if (!normalized) return productRoutePath("Runs");
  return `${productRoutePath("Runs")}/${encodeURIComponent(normalized)}`;
}

export type MailboxRouteOptions = {
  proposalId?: string;
};

export type MailboxRouteState = {
  mainChatTaskSessionId?: string;
  returnTo?: string;
};

export type MailboxLinkOptions = MailboxRouteOptions & MailboxRouteState;

export function mailboxRoute(options: MailboxRouteOptions = {}): string {
  const proposalId = options.proposalId?.trim();
  if (!proposalId) return productRoutePath("Mailbox");
  return `${productRoutePath("Mailbox")}?proposal=${encodeURIComponent(proposalId)}`;
}

export function mailboxRouteState(options: MailboxRouteState): MailboxRouteState {
  const state: MailboxRouteState = {};
  if (options.mainChatTaskSessionId?.trim()) {
    state.mainChatTaskSessionId = options.mainChatTaskSessionId.trim();
  }
  if (options.returnTo?.trim()) {
    state.returnTo = options.returnTo.trim();
  }
  return state;
}

export function mailboxLinkTarget(
  options: MailboxLinkOptions = {}
): { to: string; state?: MailboxRouteState } {
  const state = mailboxRouteState(options);
  if (Object.keys(state).length === 0) {
    return { to: mailboxRoute(options) };
  }
  return { to: mailboxRoute(options), state };
}

export function diagnosticsUsageReady(diagnostics: {
  usage_ready?: boolean;
}): boolean {
  return diagnostics.usage_ready ?? false;
}

export function diagnosticsUsageReadinessIssues(diagnostics: {
  usage_readiness_issues?: string[];
}): string[] {
  return diagnostics.usage_readiness_issues ?? [];
}

export const LEGACY_PRODUCT_REDIRECTS = [
  { from: "/", to: "/today" },
  { from: "/workspace", to: "/today" },
  { from: "/dashboard", to: "/today" },
  { from: "/chat", to: "/companion" },
  { from: "/agent", to: "/companion" },
  { from: "/review", to: "/mailbox" },
  { from: "/builder", to: "/life-model/build" },
  { from: "/life", to: "/life-model" },
  { from: "/map", to: "/life-model" },
] as const;

export type LegacyProductRoute = (typeof LEGACY_PRODUCT_REDIRECTS)[number]["from"];

export function legacyRedirectTarget(path: LegacyProductRoute): string {
  const route = LEGACY_PRODUCT_REDIRECTS.find(item => item.from === path);
  if (!route) {
    throw new Error(`Unknown legacy product route: ${path}`);
  }
  return route.to;
}

export const SECONDARY_PRODUCT_ROUTES = [
  { label: "Life Model Build", key: "LifeModelBuild", path: "/life-model/build" },
  { label: "Memory", key: "Memory", path: "/memory" },
] as const;

export type SecondaryProductRouteKey = (typeof SECONDARY_PRODUCT_ROUTES)[number]["key"];

export function secondaryRoutePath(key: SecondaryProductRouteKey): string {
  const route = SECONDARY_PRODUCT_ROUTES.find(item => item.key === key);
  if (!route) {
    throw new Error(`Unknown secondary product route: ${key}`);
  }
  return route.path;
}

export const RETAINED_LEGACY_ROUTES = LEGACY_PRODUCT_REDIRECTS.map(route => route.from);

export const ADVANCED_PRODUCT_ROUTES = [
  { label: "MCP / Tools", key: "McpTools", path: "/mcp" },
  { label: "A2A", key: "A2A", path: "/a2a" },
  { label: "Metrics", key: "Metrics", path: "/metrics" },
  { label: "Calibration", key: "Calibration", path: "/calibration" },
  { label: "Versions", key: "Versions", path: "/versions" },
] as const;

export type AdvancedProductRouteKey = (typeof ADVANCED_PRODUCT_ROUTES)[number]["key"];

export function advancedRoutePath(key: AdvancedProductRouteKey): string {
  const route = ADVANCED_PRODUCT_ROUTES.find(item => item.key === key);
  if (!route) {
    throw new Error(`Unknown advanced product route: ${key}`);
  }
  return route.path;
}

export const ADVANCED_PRODUCT_ROUTE_GROUPS = [
  {
    label: "Advanced connections",
    items: [
      { label: "MCP / Tools", path: advancedRoutePath("McpTools") },
      { label: "A2A", path: advancedRoutePath("A2A") },
    ],
  },
  {
    label: "Stage / debug / eval",
    items: [
      { label: "Metrics", path: advancedRoutePath("Metrics") },
      { label: "Calibration", path: advancedRoutePath("Calibration") },
      { label: "Versions", path: advancedRoutePath("Versions") },
    ],
  },
] as const;

// Future AgentStage bitmap assets should live under frontend/public/assets/agent-stage.
export const AGENT_STAGE_ASSET_ROOT = "/assets/agent-stage" as const;
