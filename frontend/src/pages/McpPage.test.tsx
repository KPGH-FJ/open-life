import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import { BrowserRouter } from "react-router-dom";
import McpPage from "./McpPage";
import { invoke } from "@tauri-apps/api/core";
import { mockInvoke } from "@/test/mocks/tauri";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("McpPage", () => {
  const devRuntimeBuildInfo = {
    profile: "dev",
    gitSha: "test",
    buildTime: "test",
    currentExe: "test",
    binaryKind: "debug_binary",
    frontendMode: "dev_server",
    devUrl: "http://localhost:1420",
    frontendDist: "",
    dataDir: "/tmp/openlife-test",
    a2aPort: 49321,
    a2aStatus: "authenticated_dev_enabled",
    devExtensionsEnabled: true,
    authenticatedDevA2aEnabled: true,
    unauthenticatedDevA2aEnabled: false,
    arbitraryMcpRegistrationEnabled: true,
    bundleIdentifier: "ai.openlife.app",
    productName: "OpenLife",
  };

  const releaseRuntimeBuildInfo = {
    ...devRuntimeBuildInfo,
    profile: "release",
    binaryKind: "release_bundle",
    frontendMode: "bundled_dist",
    devUrl: "",
    a2aStatus: "disabled_by_build",
    devExtensionsEnabled: false,
    authenticatedDevA2aEnabled: false,
    arbitraryMcpRegistrationEnabled: false,
  };

  const useRuntimeBuildInfo = (info: typeof devRuntimeBuildInfo) => {
    vi.mocked(invoke).mockImplementation((command, args) => {
      if (command === "get_runtime_build_info") return Promise.resolve(info);
      return mockInvoke(command, args);
    });
  };

  beforeEach(() => {
    useRuntimeBuildInfo(devRuntimeBuildInfo);
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("renders recommended MCP tools", async () => {
    render(
      <BrowserRouter>
        <McpPage />
      </BrowserRouter>
    );

    await waitFor(() => {
      expect(screen.getByText("推荐工具")).toBeInTheDocument();
    });

    expect(screen.getByText("适合当前阶段进行本地文件读写")).toBeInTheDocument();
    expect(screen.getAllByText("无可执行 typed 契约").length).toBeGreaterThan(0);
  });

  it("does not advertise untyped templates as executable", async () => {
    render(
      <BrowserRouter>
        <McpPage />
      </BrowserRouter>
    );

    const button = await screen.findByRole("button", { name: "暂无 typed 模板" });
    expect(button).toBeDisabled();
    expect(screen.queryByText("预览参数")).not.toBeInTheDocument();
  });

  it("allows arbitrary registration only when the backend enables the dev capability", async () => {
    render(
      <BrowserRouter>
        <McpPage />
      </BrowserRouter>
    );

    await screen.findByText("dev_only_enabled");
    fireEvent.change(screen.getByPlaceholderText("例如: filesystem"), {
      target: { value: "dev-server" },
    });
    fireEvent.change(screen.getByPlaceholderText("例如: npx"), {
      target: { value: "npx" },
    });
    const manifests = [
      {
        id: "mcp:dev-server:probe.read",
        name: "probe.read",
        description: "Typed read probe",
        parameters: { type: "object", properties: {} },
        permission_level: "low",
        risk_level: "low",
        version: "1.0.0",
        source: { type: "Mcp", server_name: "dev-server" },
        capabilities: ["read"],
        requires_confirmation: false,
        enabled: true,
        declarative_only: false,
        action_type: "read",
        idempotency_contract: "idempotent",
        tags: ["typed_contract"],
      },
    ];
    fireEvent.change(screen.getByPlaceholderText('[{"id":"mcp:server:tool","name":"tool",...}]'), {
      target: { value: JSON.stringify(manifests) },
    });
    fireEvent.click(screen.getByRole("button", { name: "注册" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "register_mcp_server",
        expect.objectContaining({ name: "dev-server", command: "npx", args: [], manifests })
      );
    });
  });

  it("rejects incomplete or cross-server manifests before invoking registration", async () => {
    render(
      <BrowserRouter>
        <McpPage />
      </BrowserRouter>
    );

    await screen.findByText("dev_only_enabled");
    fireEvent.change(screen.getByPlaceholderText("例如: filesystem"), {
      target: { value: "dev-server" },
    });
    fireEvent.change(screen.getByPlaceholderText("例如: npx"), {
      target: { value: "npx" },
    });
    fireEvent.change(screen.getByPlaceholderText('[{"id":"mcp:server:tool","name":"tool",...}]'), {
      target: {
        value: JSON.stringify([
          {
            id: "mcp:other:probe.read",
            name: "probe.read",
            parameters: { type: "object" },
            source: { type: "Mcp", server_name: "other" },
            capabilities: ["read"],
            action_type: "read",
            risk_level: "low",
            permission_level: "low",
            idempotency_contract: "idempotent",
          },
        ]),
      },
    });
    fireEvent.click(screen.getByRole("button", { name: "注册" }));

    expect(
      await screen.findByText(/typed manifest 必须完整并绑定当前 MCP Server 名称/)
    ).toBeInTheDocument();
    expect(
      vi.mocked(invoke).mock.calls.some(([command]) => command === "register_mcp_server")
    ).toBe(false);
  });

  it("renders audit logs section with stats", async () => {
    render(
      <BrowserRouter>
        <McpPage />
      </BrowserRouter>
    );

    await waitFor(() => {
      expect(screen.getByText("安全审计中心")).toBeInTheDocument();
    });

    // Stats cards
    expect(screen.getByText("总调用次数")).toBeInTheDocument();
    expect(screen.getByText("PII 拦截")).toBeInTheDocument();

    // Audit log entries
    expect(screen.getAllByText("write_file").length).toBeGreaterThan(0);

    // PII hit badge (in expanded or summary)
    expect(screen.getByText("敏感数据已脱敏")).toBeInTheDocument();

    // Privacy rules section
    expect(screen.getByText("隐私保护规则")).toBeInTheDocument();
  });

  it("delegates audit retention to the governed Settings workflow", async () => {
    render(
      <BrowserRouter>
        <McpPage />
      </BrowserRouter>
    );

    await screen.findByText("安全审计中心");
    const settingsLink = screen.getByRole("link", {
      name: "在隐私设置中管理审计保留",
    });
    expect(settingsLink).toHaveAttribute("href", "/settings");
    expect(screen.queryByRole("button", { name: "清理 7 天前日志" })).not.toBeInTheDocument();
    expect(vi.mocked(invoke).mock.calls.some(([cmd]) => cmd === "clear_mcp_audit_logs")).toBe(
      false
    );
  });

  it("keeps arbitrary MCP registration unavailable in release builds", async () => {
    useRuntimeBuildInfo(releaseRuntimeBuildInfo);

    render(
      <BrowserRouter>
        <McpPage />
      </BrowserRouter>
    );

    await screen.findByText("disabled_by_build");
    expect(screen.queryByText("注册新 MCP Server")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "用模板安装" })).not.toBeInTheDocument();
    expect(screen.queryByTitle("删除")).not.toBeInTheDocument();
    expect(screen.getByText("安全审计中心")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "刷新" })).toBeInTheDocument();

    await waitFor(() => {
      expect(vi.mocked(invoke).mock.calls.map(([command]) => command)).toContain(
        "recommend_mcp_manifests"
      );
    });
    const settledCommands = vi.mocked(invoke).mock.calls.map(([command]) => command);
    const allowedReadOnlyCommands = new Set([
      "get_runtime_build_info",
      "list_mcp_servers",
      "list_mcp_tools",
      "list_mcp_templates",
      "list_mcp_audit_logs",
      "get_privacy_policy",
      "recommend_mcp_manifests",
    ]);
    expect(settledCommands.every(command => allowedReadOnlyCommands.has(command))).toBe(true);
    expect(settledCommands).not.toContain("register_mcp_server");
    expect(settledCommands).not.toContain("unregister_mcp_server");
  });

  it("fails closed without transient arbitrary MCP mutations when build info is unavailable", async () => {
    vi.mocked(invoke).mockImplementation((command, args) => {
      if (command === "get_runtime_build_info") return Promise.reject(new Error("unavailable"));
      return mockInvoke(command, args);
    });

    render(
      <BrowserRouter>
        <McpPage />
      </BrowserRouter>
    );

    await screen.findByText("unavailable");
    expect(screen.queryByText("注册新 MCP Server")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "清理 7 天前日志" })).toBeInTheDocument();

    const invokedCommands = vi.mocked(invoke).mock.calls.map(([command]) => command);
    expect(invokedCommands).not.toContain("register_mcp_server");
    expect(invokedCommands).not.toContain("unregister_mcp_server");
  });
});
