import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import App from "./App";
import { invoke } from "@tauri-apps/api/core";
import { mockInvoke } from "@/test/mocks/tauri";
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

    expect(await screen.findByText(/Beta 试用准备中/)).toBeInTheDocument();
    expect(screen.getByText("查看试用完成度")).toBeInTheDocument();
  });

  it("declares the W159 product route and label contract", () => {
    expect(PRIMARY_PRODUCT_ROUTES).toEqual([
      { label: "陪伴", path: "/companion", legacyAlias: "/chat" },
      { label: "今日", path: "/today", legacyAlias: "/" },
      { label: "Life Model", path: "/life-model", legacyAlias: "/builder" },
      { label: "邮箱", path: "/mailbox", legacyAlias: "/review" },
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

  it("renders W160 product tabs with an active companion tab and secondary tools", async () => {
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
    expect(screen.getByRole("link", { name: "Runs" })).toHaveAttribute("href", "/runs");
    expect(screen.getByRole("link", { name: "Settings" })).toHaveAttribute("href", "/settings");

    fireEvent.click(screen.getByRole("button", { name: /二级入口/ }));

    for (const label of ["MCP", "A2A", "Metrics", "Versions", "Calibration"]) {
      expect(screen.getByRole("link", { name: label })).toBeInTheDocument();
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
    ["/companion", "陪伴", "聊天就绪"],
    ["/today", "今日", "今日驾驶舱"],
    ["/life-model", "Life Model", "life-model-page"],
    ["/mailbox", "邮箱", "mailbox-page"],
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

    expect(await screen.findByText("试用控制台")).toBeInTheDocument();
  });

  it("keeps Runs reachable as a secondary route", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "has_completed_onboarding") return Promise.resolve(true);
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
