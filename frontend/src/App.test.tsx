import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter } from "react-router-dom";
import App from "./App";
import { invoke } from "@tauri-apps/api/core";
import { mockInvoke } from "@/test/mocks/tauri";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import {
  AGENT_STAGE_ASSET_ROOT,
  PRIMARY_PRODUCT_ROUTES,
  RETAINED_LEGACY_ROUTES,
} from "./productShellContract";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

describe("App onboarding", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockImplementation(mockInvoke);
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("shows onboarding when first-run flag is not completed", async () => {
    render(
      <MemoryRouter>
        <App />
      </MemoryRouter>
    );

    expect(await screen.findByText("欢迎使用 OpenLife")).toBeInTheDocument();
  });

  it("hides onboarding after completion", async () => {
    render(
      <MemoryRouter>
        <App />
      </MemoryRouter>
    );

    expect(await screen.findByText("欢迎使用 OpenLife")).toBeInTheDocument();
    fireEvent.click(screen.getByText("下一步"));
    fireEvent.click(screen.getByText("下一步"));
    fireEvent.click(screen.getByText("下一步"));
    fireEvent.click(screen.getByText("关闭引导，稍后再探索"));

    await waitFor(() => {
      expect(screen.queryByText("欢迎使用 OpenLife")).not.toBeInTheDocument();
    });
    expect(invoke).toHaveBeenCalledWith("mark_onboarding_completed", undefined);
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
          legacy_data_dir: "/tmp/openlife-legacy",
          database_status: "degraded",
          startup_warnings: ["memory.db 初始化失败，正在使用临时数据库"],
          snapshot_count: 1,
          life_model_ready: true,
          app_version: "0.1.0",
          model_empty: false,
          chat_session_count: 1,
          onboarding_completed: false,
          beta_ready: false,
          beta_readiness_issues: [],
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

  it("shows beta progress banner when diagnostics is not beta ready but storage is healthy", async () => {
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
          legacy_data_dir: "/tmp/openlife-legacy",
          database_status: "ok",
          startup_warnings: [],
          snapshot_count: 0,
          life_model_ready: true,
          app_version: "0.1.0",
          model_empty: false,
          chat_session_count: 0,
          onboarding_completed: false,
          beta_ready: false,
          beta_readiness_issues: ["还没有完成首轮真实对话验证。"],
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

    expect(await screen.findByText(/Beta 使用准备中/)).toBeInTheDocument();
    expect(screen.getByText("查看准备状态")).toBeInTheDocument();
  });

  it("does not show onboarding over no-backend product route errors", async () => {
    const noBackendError = new Error(
      "当前不在 OpenLife 桌面应用环境中，无法调用原生功能。请在桌面窗口内操作。"
    );
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (
        cmd === "has_completed_onboarding" ||
        cmd === "get_system_diagnostics" ||
        cmd === "list_proposals" ||
        cmd === "get_config"
      ) {
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

  it("declares the W159 product route and label contract", () => {
    expect(PRIMARY_PRODUCT_ROUTES).toEqual([
      { label: "陪伴", path: "/companion", legacyAlias: "/chat" },
      { label: "今日", path: "/today", legacyAlias: "/" },
      { label: "Review", path: "/mailbox", legacyAlias: "/review" },
      { label: "Life Model", path: "/life-model", legacyAlias: "/builder" },
    ]);
    expect(RETAINED_LEGACY_ROUTES).toEqual([
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
    ]);
    expect(AGENT_STAGE_ASSET_ROOT).toBe("/assets/agent-stage");
  });

  it("renders product tabs with an active companion tab and a restrained secondary tools menu", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "has_completed_onboarding") return Promise.resolve(true);
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

    expect(screen.getByRole("link", { name: "陪伴" })).toHaveAttribute("aria-current", "page");
    expect(screen.queryByRole("link", { name: "Activity" })).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "更多" }));

    for (const [label, path] of [
      ["Settings", "/settings"],
      ["Activity", "/runs"],
      ["MCP 工具", "/mcp"],
      ["A2A 连接", "/a2a"],
      ["版本", "/versions"],
      ["Metrics", "/metrics"],
      ["Calibration", "/calibration"],
    ] as const) {
      expect(screen.getByRole("link", { name: label })).toHaveAttribute("href", path);
    }
  });

  it("lets keyboard users open and close the secondary tools menu", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "has_completed_onboarding") return Promise.resolve(true);
      return mockInvoke(cmd, args);
    });
    const user = userEvent.setup();

    render(
      <MemoryRouter initialEntries={["/companion"]}>
        <App />
      </MemoryRouter>
    );

    await screen.findByRole("link", { name: "陪伴" });
    const menuButton = screen.getByRole("button", { name: "更多" });
    menuButton.focus();
    expect(menuButton).toHaveFocus();

    await user.keyboard("{Enter}");
    expect(screen.getByRole("link", { name: "MCP 工具" })).toBeInTheDocument();

    await user.keyboard("{Escape}");
    await waitFor(() => {
      expect(screen.queryByRole("link", { name: "MCP 工具" })).not.toBeInTheDocument();
    });
    expect(menuButton).toHaveFocus();
  });

  it("closes the secondary tools menu when the route changes", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "has_completed_onboarding") return Promise.resolve(true);
      return mockInvoke(cmd, args);
    });

    render(
      <MemoryRouter initialEntries={["/companion"]}>
        <App />
      </MemoryRouter>
    );

    await screen.findByRole("link", { name: "陪伴" });
    fireEvent.click(screen.getByRole("button", { name: "更多" }));
    expect(screen.getByRole("link", { name: "MCP 工具" })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("link", { name: "今日" }));
    await waitFor(() => {
      expect(screen.queryByRole("link", { name: "MCP 工具" })).not.toBeInTheDocument();
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

  it("renders /companion as the W162 companion surface with AgentStage", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "has_completed_onboarding") return Promise.resolve(true);
      return mockInvoke(cmd, args);
    });

    render(
      <MemoryRouter initialEntries={["/companion"]}>
        <App />
      </MemoryRouter>
    );

    expect(await screen.findByTestId("companion-page")).toBeInTheDocument();
    expect(screen.getByTestId("agent-stage")).toHaveAttribute("data-state", "idle");
    expect(screen.getByRole("status", { name: /OpenLife Agent 状态/ })).toBeInTheDocument();
  });

  it.each(["/chat", "/agent"])("keeps %s on the legacy ChatPage route", async path => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "has_completed_onboarding") return Promise.resolve(true);
      return mockInvoke(cmd, args);
    });

    render(
      <MemoryRouter initialEntries={[path]}>
        <App />
      </MemoryRouter>
    );

    expect(await screen.findByTestId("chat-page")).toBeInTheDocument();
    expect(screen.queryByTestId("companion-page")).not.toBeInTheDocument();
    expect(screen.queryByTestId("agent-stage")).not.toBeInTheDocument();
  });

  it.each([
    ["/companion", "陪伴", "companion-page"],
    ["/today", "今日", "today-page"],
    ["/life-model", "Life Model", "life-model-page"],
    ["/mailbox", "Review", "mailbox-page"],
  ])("renders the %s product entry for %s", async (path, _label, expectedText) => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "has_completed_onboarding") return Promise.resolve(true);
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

  it.each(["/", "/workspace"])("keeps %s on the legacy DashboardPage route", async path => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "has_completed_onboarding") return Promise.resolve(true);
      return mockInvoke(cmd, args);
    });

    render(
      <MemoryRouter initialEntries={[path]}>
        <App />
      </MemoryRouter>
    );

    expect(await screen.findByText("仪表盘")).toBeInTheDocument();
    expect(screen.queryByTestId("today-page")).not.toBeInTheDocument();
  });

  it("keeps the completed product entries on their product pages", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "has_completed_onboarding") return Promise.resolve(true);
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

  it.each(["/builder", "/life"])("keeps %s on the legacy BuilderPage route", async path => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "has_completed_onboarding") return Promise.resolve(true);
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

  it("keeps /memory on the legacy MemorySearch route", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "has_completed_onboarding") return Promise.resolve(true);
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

  it("keeps /review on the legacy ProposalReviewPage route", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "has_completed_onboarding") return Promise.resolve(true);
      return mockInvoke(cmd, args);
    });

    render(
      <MemoryRouter initialEntries={["/review"]}>
        <App />
      </MemoryRouter>
    );

    expect(await screen.findByText("Review Center")).toBeInTheDocument();
    expect(screen.queryByTestId("mailbox-page")).not.toBeInTheDocument();
  });

  it("keeps Settings reachable as a secondary route", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "has_completed_onboarding") return Promise.resolve(true);
      return mockInvoke(cmd, args);
    });

    render(
      <MemoryRouter initialEntries={["/settings"]}>
        <App />
      </MemoryRouter>
    );

    expect(await screen.findByText("Settings")).toBeInTheDocument();
  });

  it("keeps Activity reachable as a secondary route", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "has_completed_onboarding") return Promise.resolve(true);
      return mockInvoke(cmd, args);
    });

    render(
      <MemoryRouter initialEntries={["/runs"]}>
        <App />
      </MemoryRouter>
    );

    expect(await screen.findByRole("heading", { name: "Activity" })).toBeInTheDocument();
  });
});
