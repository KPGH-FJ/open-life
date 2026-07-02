import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import App from "./App";
import { invoke } from "@tauri-apps/api/core";
import { mockInvoke } from "@/test/mocks/tauri";
import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import {
  ADVANCED_PRODUCT_ROUTE_GROUPS,
  ADVANCED_PRODUCT_ROUTES,
  AGENT_STAGE_ASSET_ROOT,
  LEGACY_PRODUCT_REDIRECTS,
  PRIMARY_PRODUCT_ROUTES,
  RETAINED_LEGACY_ROUTES,
  RUN_DETAIL_ROUTE_PATTERN,
  SECONDARY_PRODUCT_ROUTES,
  mailboxLinkTarget,
  mailboxRoute,
  mailboxRouteState,
  runDetailRoute,
  runDetailRoutePattern,
} from "./productShellContract";

function productionSourceFiles(dir = join(process.cwd(), "src")): string[] {
  const entries = readdirSync(dir);
  const files: string[] = [];
  for (const entry of entries) {
    const fullPath = join(dir, entry);
    const stat = statSync(fullPath);
    if (stat.isDirectory()) {
      if (entry === "test") continue;
      files.push(...productionSourceFiles(fullPath));
    } else if (/\.(ts|tsx)$/.test(entry) && !/\.test\.(ts|tsx)$/.test(entry)) {
      files.push(fullPath);
    }
  }
  return files;
}

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

describe("App product surface routing", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockImplementation(mockInvoke);
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("shows safe mode banner when diagnostics reports degraded storage", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_system_diagnostics") {
        return Promise.resolve({
          router: {
            onnx_available: false,
            onnx_disabled: false,
            active_backend: "regex",
            latency_threshold_us: 50000,
          },
          mcp_server_count: 1,
          mcp_tool_count: 2,
          mcp_recent_audit_count: 1,
          mcp_recent_pii_count: 0,
          memory_chunk_count: 42,
          vector_corrupt_embedding_count: 2,
          unfinished_builder_sessions: 0,
          ollama_online: true,
          local_model: "llama3",
          resolved_local_model: "llama3:latest",
          prefer_local_model: false,
          cloud_api_configured: true,
          cloud_provider: "DeepSeek",
          cloud_api_validated: true,
          cloud_api_last_error: null,
          chat_ready: true,
          readiness_issues: [],
          data_dir: "/tmp/openlife-test",
          active_data_dir: "/tmp/openlife-test",
          database_status: "degraded",
          startup_warnings: ["memory.db 初始化失败，正在使用临时数据库"],
          snapshot_count: 1,
          life_model_ready: true,
          app_version: "0.1.0",
          model_empty: false,
          chat_session_count: 1,
          builder_completion: {
            identity: 80,
            goals: 70,
            capabilities: 75,
            state: 65,
            overall: 72.5,
            lowest_dimension: "state",
          },
          data_files: {
            messages_db_exists: true,
            messages_db_size_mb: 0.1,
            vectors_db_exists: true,
            vectors_db_size_mb: 0.2,
            mcp_audit_db_exists: true,
            mcp_audit_db_size_mb: 0.1,
            config_yaml_exists: true,
            life_model_yaml_exists: true,
          },
          ollama_models: [],
          config_source: "default",
        });
      }
      return mockInvoke(cmd, args);
    });

    render(
      <MemoryRouter>
        <App />
      </MemoryRouter>
    );

    expect(await screen.findByText(/Safe Mode：当前数据环境存在风险/)).toBeInTheDocument();
    expect(screen.getAllByText(/memory.db 初始化失败/).length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText("打开恢复控制台")).toBeInTheDocument();
  });

  it("shows usage readiness banner when diagnostics is not usage ready but storage is healthy", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_system_diagnostics") {
        return Promise.resolve({
          router: {
            onnx_available: false,
            onnx_disabled: false,
            active_backend: "regex",
            latency_threshold_us: 50000,
          },
          mcp_server_count: 1,
          mcp_tool_count: 2,
          mcp_recent_audit_count: 1,
          mcp_recent_pii_count: 0,
          memory_chunk_count: 42,
          vector_corrupt_embedding_count: 0,
          unfinished_builder_sessions: 0,
          ollama_online: true,
          local_model: "llama3",
          resolved_local_model: "llama3:latest",
          prefer_local_model: false,
          cloud_api_configured: true,
          cloud_provider: "DeepSeek",
          cloud_api_validated: true,
          cloud_api_last_error: null,
          chat_ready: true,
          readiness_issues: [],
          data_dir: "/tmp/openlife-test",
          active_data_dir: "/tmp/openlife-test",
          database_status: "ok",
          startup_warnings: [],
          snapshot_count: 0,
          life_model_ready: true,
          app_version: "0.1.0",
          model_empty: false,
          chat_session_count: 0,
          usage_ready: false,
          usage_readiness_issues: ["还没有完成首轮真实对话验证。"],
          builder_completion: {
            identity: 80,
            goals: 70,
            capabilities: 75,
            state: 65,
            overall: 72.5,
            lowest_dimension: "state",
          },
          data_files: {
            messages_db_exists: true,
            messages_db_size_mb: 0.1,
            vectors_db_exists: true,
            vectors_db_size_mb: 0.2,
            mcp_audit_db_exists: true,
            mcp_audit_db_size_mb: 0.1,
            config_yaml_exists: true,
            life_model_yaml_exists: true,
          },
          ollama_models: [],
          config_source: "default",
        });
      }
      return mockInvoke(cmd, args);
    });

    render(
      <MemoryRouter>
        <App />
      </MemoryRouter>
    );

    expect(await screen.findByText(/使用准备中/)).toBeInTheDocument();
    expect(screen.getByText("查看准备状态")).toBeInTheDocument();
  });

  it("does not show onboarding over no-backend product route errors", async () => {
    const noBackendError = new Error(
      "当前不在 OpenLife 桌面应用环境中，无法调用原生功能。请在桌面窗口内操作。"
    );
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === "get_system_diagnostics" || cmd === "list_proposals" || cmd === "get_config") {
        return Promise.reject(noBackendError);
      }
      return Promise.resolve({});
    });

    render(
      <MemoryRouter initialEntries={["/mailbox"]}>
        <App />
      </MemoryRouter>
    );

    expect(await screen.findByTestId("mailbox-page", {}, { timeout: 5000 })).toBeInTheDocument();
    expect(await screen.findByText(/当前不在 OpenLife 桌面应用环境中/)).toBeInTheDocument();
    expect(screen.queryByText("欢迎使用 OpenLife")).not.toBeInTheDocument();
  });

  it("declares the canonical product route registry", () => {
    expect(PRIMARY_PRODUCT_ROUTES).toEqual([
      { label: "Today", path: "/today" },
      { label: "Companion", path: "/companion" },
      { label: "Mailbox", path: "/mailbox" },
      { label: "Life Model", path: "/life-model" },
      { label: "Runs", path: "/runs" },
      { label: "Settings", path: "/settings" },
    ]);
    expect(LEGACY_PRODUCT_REDIRECTS).toEqual([
      { from: "/", to: "/today" },
      { from: "/workspace", to: "/today" },
      { from: "/dashboard", to: "/today" },
      { from: "/chat", to: "/companion" },
      { from: "/agent", to: "/companion" },
      { from: "/review", to: "/mailbox" },
      { from: "/builder", to: "/life-model/build" },
      { from: "/life", to: "/life-model" },
      { from: "/map", to: "/life-model" },
    ]);
    expect(RETAINED_LEGACY_ROUTES).toEqual([
      "/",
      "/workspace",
      "/dashboard",
      "/chat",
      "/agent",
      "/review",
      "/builder",
      "/life",
      "/map",
    ]);
    expect(RUN_DETAIL_ROUTE_PATTERN).toBe("/runs/:runId");
    expect(runDetailRoutePattern()).toBe(RUN_DETAIL_ROUTE_PATTERN);
    expect(runDetailRoute("run-product-1")).toBe("/runs/run-product-1");
    expect(mailboxRoute()).toBe("/mailbox");
    expect(mailboxRoute({ proposalId: "proposal-product-1" })).toBe(
      "/mailbox?proposal=proposal-product-1"
    );
    expect(mailboxRoute({ proposalId: "  " })).toBe("/mailbox");
    expect(
      mailboxRouteState({ mainChatTaskSessionId: " task-1 ", returnTo: " /companion " })
    ).toEqual({
      mainChatTaskSessionId: "task-1",
      returnTo: "/companion",
    });
    expect(
      mailboxRouteState({
        mainChatTaskSessionId: " ",
        returnTo: "\n",
      })
    ).toEqual({});
    expect(
      mailboxLinkTarget({
        proposalId: " proposal-product-2 ",
        mainChatTaskSessionId: " task-2 ",
        returnTo: " /companion ",
      })
    ).toEqual({
      to: "/mailbox?proposal=proposal-product-2",
      state: {
        mainChatTaskSessionId: "task-2",
        returnTo: "/companion",
      },
    });
    expect(
      mailboxLinkTarget({
        proposalId: " ",
        mainChatTaskSessionId: " ",
        returnTo: " ",
      })
    ).toEqual({ to: "/mailbox" });
    expect(SECONDARY_PRODUCT_ROUTES).toEqual([
      { label: "Life Model Build", key: "LifeModelBuild", path: "/life-model/build" },
      { label: "Memory", key: "Memory", path: "/memory" },
    ]);
    expect(ADVANCED_PRODUCT_ROUTES).toEqual([
      { label: "MCP / Tools", key: "McpTools", path: "/mcp" },
      { label: "A2A", key: "A2A", path: "/a2a" },
      { label: "Metrics", key: "Metrics", path: "/metrics" },
      { label: "Calibration", key: "Calibration", path: "/calibration" },
      { label: "Versions", key: "Versions", path: "/versions" },
    ]);
    expect(ADVANCED_PRODUCT_ROUTE_GROUPS).toEqual([
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
    ]);
    expect(AGENT_STAGE_ASSET_ROOT).toBe("/assets/agent-stage");
  });

  it("renders product tabs with an active companion tab and a restrained secondary tools menu", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      return mockInvoke(cmd, args);
    });

    render(
      <MemoryRouter initialEntries={["/companion"]}>
        <App />
      </MemoryRouter>
    );

    for (const route of PRIMARY_PRODUCT_ROUTES) {
      expect(await screen.findByRole("link", { name: route.label })).toHaveAttribute(
        "href",
        route.path
      );
    }

    expect(screen.getByRole("link", { name: "Companion" })).toHaveAttribute("aria-current", "page");
    expect(screen.getByRole("link", { name: "Runs" })).toHaveAttribute("href", "/runs");
    expect(screen.getByRole("link", { name: "Settings" })).toHaveAttribute("href", "/settings");
    expect(screen.queryByRole("link", { name: "MCP / Tools" })).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Advanced" }));

    for (const [label, path] of [
      ["MCP / Tools", "/mcp"],
      ["A2A", "/a2a"],
      ["Metrics", "/metrics"],
      ["Calibration", "/calibration"],
      ["Versions", "/versions"],
    ] as const) {
      expect(screen.getByRole("link", { name: label })).toHaveAttribute("href", path);
    }
  });

  it("lets keyboard users open and close the secondary tools menu", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      return mockInvoke(cmd, args);
    });
    const user = userEvent.setup();

    render(
      <MemoryRouter initialEntries={["/companion"]}>
        <App />
      </MemoryRouter>
    );

    await screen.findByRole("link", { name: "Companion" });
    const menuButton = screen.getByRole("button", { name: "Advanced" });
    menuButton.focus();
    expect(menuButton).toHaveFocus();

    await user.keyboard("{Enter}");
    expect(screen.getByRole("link", { name: "MCP / Tools" })).toBeInTheDocument();

    await user.keyboard("{Escape}");
    await waitFor(() => {
      expect(screen.queryByRole("link", { name: "MCP / Tools" })).not.toBeInTheDocument();
    });
    expect(menuButton).toHaveFocus();
  });

  it("closes the secondary tools menu when the route changes", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      return mockInvoke(cmd, args);
    });

    render(
      <MemoryRouter initialEntries={["/companion"]}>
        <App />
      </MemoryRouter>
    );

    await screen.findByRole("link", { name: "Companion" });
    fireEvent.click(screen.getByRole("button", { name: "Advanced" }));
    expect(screen.getByRole("link", { name: "MCP / Tools" })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("link", { name: "Today" }));
    await waitFor(() => {
      expect(screen.queryByRole("link", { name: "MCP / Tools" })).not.toBeInTheDocument();
    });
  });

  it("keeps W166 product surface files free of disabled backend wrappers", () => {
    const productSurfaceFiles = [
      "src/App.tsx",
      "src/components/ProductShell.tsx",
      "src/components/AgentStage.tsx",
      "src/pages/CompanionPage.tsx",
      "src/pages/TodayPage.tsx",
      "src/pages/LifeModelPage.tsx",
      "src/pages/MailboxPage.tsx",
    ];
    const forbiddenWrappers = [
      "saveLifeModel",
      "builderApplySignals",
      "batchAcceptLowRiskProposals",
      "runSkill",
      "getSkillRuntimeStatus",
      "checkRuntimeMigrationGate",
      "runMultiStrategyAgentPreview",
    ];

    for (const filePath of productSurfaceFiles) {
      const source = readFileSync(join(process.cwd(), filePath), "utf8");
      for (const forbiddenWrapper of forbiddenWrappers) {
        expect(source, `${filePath} must not import ${forbiddenWrapper}`).not.toMatch(
          new RegExp(`\\b${forbiddenWrapper}\\b`)
        );
      }
    }
  });

  it("keeps retired product surfaces deleted and legacy routes quarantined to the registry", () => {
    for (const retiredFile of [
      "src/components/OnboardingWizard.tsx",
      "src/components/WorkspaceOverview.tsx",
      "src/pages/DashboardPage.tsx",
      "src/pages/ProposalReviewPage.tsx",
      "src/pages/LifeMapPage.tsx",
    ]) {
      expect(existsSync(join(process.cwd(), retiredFile)), `${retiredFile} must stay deleted`).toBe(
        false
      );
    }

    const appSource = readFileSync(join(process.cwd(), "src/App.tsx"), "utf8");
    expect(appSource).not.toMatch(/OnboardingWizard|DashboardPage|ProposalReviewPage|LifeMapPage/);

    const routeRegistryPath = join(process.cwd(), "src/productShellContract.ts");
    const tauriTypesPath = join(process.cwd(), "src/tauri.ts");
    const appTypesPath = join(process.cwd(), "src/types.ts");
    for (const filePath of productionSourceFiles()) {
      const source = readFileSync(filePath, "utf8");
      if (filePath !== routeRegistryPath) {
        expect(source, `${filePath} must not contain retired default routes`).not.toMatch(
          /["'`]\/(?:chat|review|dashboard|workspace|builder|life|map)["'`]/
        );
        expect(source, `${filePath} must not contain retired hash routes`).not.toMatch(
          /#\/(?:chat|review|dashboard|builder)/
        );
        expect(source, `${filePath} must use runDetailRoute for dynamic run links`).not.toMatch(
          /\/runs\/:runId|\/runs\/\$\{/
        );
        expect(source, `${filePath} must use mailboxRoute for proposal deep links`).not.toMatch(
          /\?proposal=/
        );
      }
      expect(source, `${filePath} must not import retired pages`).not.toMatch(
        /OnboardingWizard|DashboardPage|WorkspaceOverview|ProposalReviewPage|LifeMapPage/
      );
      if (filePath !== tauriTypesPath && filePath !== appTypesPath) {
        expect(source, `${filePath} must not show retired default product copy`).not.toMatch(
          /\b(?:Onboarding|Review Center|Dashboard|Workspace)\b|旧仪表盘|查看 Review|去 Review|打开 Review|待确认 Review|Review 待|Review 处理|Review 修正|Review 确认|Review 整理|Builder Review|仪表盘/
        );
      }
      if (filePath !== routeRegistryPath && filePath !== tauriTypesPath) {
        expect(source, `${filePath} must not read deprecated beta diagnostics aliases`).not.toMatch(
          new RegExp(
            `\\b${["beta", "ready"].join("_")}\\b|\\b${["beta", "readiness", "issues"].join(
              "_"
            )}\\b`
          )
        );
      }
      expect(source, `${filePath} must not expose Builder direct apply API`).not.toMatch(
        /enableLegacyDirectApply|Legacy direct apply|绕过 Mailbox|onApply=\{/
      );
    }
  });

  it("renders /companion as the W162 companion surface with AgentStage", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      return mockInvoke(cmd, args);
    });

    render(
      <MemoryRouter initialEntries={["/companion"]}>
        <App />
      </MemoryRouter>
    );

    expect(await screen.findByTestId("companion-page", {}, { timeout: 5_000 })).toBeInTheDocument();
    expect(screen.getByTestId("agent-stage")).toHaveAttribute("data-state", "idle");
    expect(screen.getByRole("status", { name: /OpenLife Agent 状态/ })).toBeInTheDocument();
  });

  it.each(["/chat", "/agent"])("redirects %s to the Companion product surface", async path => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      return mockInvoke(cmd, args);
    });

    render(
      <MemoryRouter initialEntries={[path]}>
        <App />
      </MemoryRouter>
    );

    expect(await screen.findByTestId("companion-page")).toBeInTheDocument();
    expect(screen.getByTestId("agent-stage")).toHaveAttribute("data-state", "idle");
  });

  it.each([
    ["/companion", "Companion", "companion-page"],
    ["/today", "Today", "today-page"],
    ["/life-model", "Life Model", "life-model-page"],
    ["/mailbox", "Mailbox", "mailbox-page"],
    ["/runs", "Runs", "Runs"],
    ["/settings", "Settings", "Settings"],
  ])("renders the %s product entry for %s", async (path, _label, expectedText) => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      return mockInvoke(cmd, args);
    });

    render(
      <MemoryRouter initialEntries={[path]}>
        <App />
      </MemoryRouter>
    );

    if (expectedText.endsWith("-page")) {
      expect(await screen.findByTestId(expectedText)).toBeInTheDocument();
    } else {
      expect(await screen.findByText(expectedText)).toBeInTheDocument();
    }
  });

  it.each(["/", "/workspace", "/dashboard"])("redirects %s to Today", async path => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      return mockInvoke(cmd, args);
    });

    render(
      <MemoryRouter initialEntries={[path]}>
        <App />
      </MemoryRouter>
    );

    expect(await screen.findByTestId("today-page")).toBeInTheDocument();
    expect(screen.queryByText("仪表盘")).not.toBeInTheDocument();
  });

  it("keeps the completed product entries on their product pages", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      return mockInvoke(cmd, args);
    });

    for (const [path, testId] of [
      ["/companion", "companion-page"],
      ["/life-model", "life-model-page"],
      ["/mailbox", "mailbox-page"],
    ] as const) {
      const { unmount } = render(
        <MemoryRouter initialEntries={[path]}>
          <App />
        </MemoryRouter>
      );
      expect(await screen.findByTestId(testId)).toBeInTheDocument();
      unmount();
    }
  });

  it.each(["/builder"])("redirects %s to the Life Model build subflow", async path => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      return mockInvoke(cmd, args);
    });

    render(
      <MemoryRouter initialEntries={[path]}>
        <App />
      </MemoryRouter>
    );

    expect(await screen.findByText("人生模型构建")).toBeInTheDocument();
    expect(screen.queryByTestId("life-model-page")).not.toBeInTheDocument();
  });

  it.each(["/life", "/map"])("redirects %s to Life Model", async path => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      return mockInvoke(cmd, args);
    });

    render(
      <MemoryRouter initialEntries={[path]}>
        <App />
      </MemoryRouter>
    );

    expect(await screen.findByTestId("life-model-page")).toBeInTheDocument();
  });

  it("keeps /memory reachable as a secondary Life Model route", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      return mockInvoke(cmd, args);
    });

    render(
      <MemoryRouter initialEntries={["/memory"]}>
        <App />
      </MemoryRouter>
    );

    expect(await screen.findByText("语义检索记忆")).toBeInTheDocument();
    expect(screen.queryByTestId("life-model-page")).not.toBeInTheDocument();
  });

  it("redirects /review to Mailbox while preserving route state", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      return mockInvoke(cmd, args);
    });

    render(
      <MemoryRouter
        initialEntries={[
          {
            pathname: "/review",
            state: { mainChatTaskSessionId: "mainchat-task-product-ui-1" },
          },
        ]}
      >
        <App />
      </MemoryRouter>
    );

    expect(await screen.findByTestId("mailbox-page")).toBeInTheDocument();
    expect(screen.queryByText("Review Center")).not.toBeInTheDocument();
  });

  it("keeps Settings reachable as a primary route", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      return mockInvoke(cmd, args);
    });

    render(
      <MemoryRouter initialEntries={["/settings"]}>
        <App />
      </MemoryRouter>
    );

    expect(await screen.findByText("Settings")).toBeInTheDocument();
  });

  it("keeps Runs reachable as a primary route", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      return mockInvoke(cmd, args);
    });

    render(
      <MemoryRouter initialEntries={["/runs"]}>
        <App />
      </MemoryRouter>
    );

    expect(await screen.findByRole("heading", { name: "Runs" })).toBeInTheDocument();
  });
});
