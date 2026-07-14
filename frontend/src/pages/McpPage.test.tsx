import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, fireEvent, within, act } from "@testing-library/react";
import { BrowserRouter } from "react-router-dom";
import McpPage from "./McpPage";
import { invoke } from "@tauri-apps/api/core";
import { mockInvoke } from "@/test/mocks/tauri";
import type { ToolManifest } from "../tauri";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

const ARGUMENTS_AUDIT_RECEIPT = JSON.stringify({
  kind: "arguments",
  payloadStored: false,
  valueType: "object",
  bytes: 42,
  digest: "sha256:AT5/r7IBtLhordHw+cHDRUBlUanhoUXBpj8SXOOfjGs",
});

const RESULT_AUDIT_RECEIPT = JSON.stringify({
  kind: "result",
  payloadStored: false,
  valueType: "string",
  bytes: 18,
  digest: "sha256:jCW4voE7OX1fsd3cEkYh4wDHjOVUw90Ga7TB0dnFGwU",
});

function recommendationManifest(name: string, description: string): ToolManifest {
  return {
    id: `mcp:filesystem:${name}`,
    name,
    description,
    parameters: {},
    permission_level: "low",
    risk_level: "low",
    version: "1.0.0",
    source: { type: "Mcp", server_name: "filesystem" },
    capabilities: ["filesystem"],
    requires_confirmation: false,
    enabled: true,
    declarative_only: false,
    action_type: "read",
    idempotency_contract: "idempotent",
    tags: ["filesystem"],
  };
}

function registrationManifest(serverName = "dev-server"): ToolManifest {
  return {
    id: `mcp:${serverName}:probe.read`,
    name: "probe.read",
    description: "Typed read probe",
    parameters: { type: "object", properties: {} },
    permission_level: "low",
    risk_level: "low",
    version: "1.0.0",
    source: { type: "Mcp", server_name: serverName },
    capabilities: ["read"],
    requires_confirmation: false,
    enabled: true,
    declarative_only: false,
    action_type: "read",
    idempotency_contract: "idempotent",
    tags: ["typed_contract"],
  };
}

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

  const expectGovernedAuditRetentionLink = () => {
    expect(screen.getByRole("link", { name: "在隐私设置中管理审计保留" })).toHaveAttribute(
      "href",
      "/settings"
    );
    expect(screen.queryByRole("button", { name: "清理 7 天前日志" })).not.toBeInTheDocument();
    expect(vi.mocked(invoke).mock.calls.some(([cmd]) => cmd === "clear_mcp_audit_logs")).toBe(
      false
    );
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
    const manifests = [registrationManifest()];
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

  it("does not start refresh IPC when a registration completes after unmount", async () => {
    const registration = deferred<void>();
    let componentUnmounted = false;
    let postUnmountIpcCount = 0;
    vi.mocked(invoke).mockImplementation((command, args) => {
      if (componentUnmounted) postUnmountIpcCount += 1;
      if (command === "get_runtime_build_info") return Promise.resolve(devRuntimeBuildInfo);
      if (command === "register_mcp_server") return registration.promise;
      return mockInvoke(command, args);
    });

    const { unmount } = render(
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
      target: { value: JSON.stringify([registrationManifest()]) },
    });
    fireEvent.click(screen.getByRole("button", { name: "注册" }));
    await waitFor(() =>
      expect(
        vi.mocked(invoke).mock.calls.some(([command]) => command === "register_mcp_server")
      ).toBe(true)
    );

    componentUnmounted = true;
    unmount();
    await act(async () => {
      registration.resolve();
      await registration.promise;
      await Promise.resolve();
    });

    expect(postUnmountIpcCount).toBe(0);
  });

  it("does not start any refresh IPC when unregister completes after unmount", async () => {
    const unregistration = deferred<void>();
    const postUnmountCommands: string[] = [];
    let componentUnmounted = false;
    vi.mocked(invoke).mockImplementation((command, args) => {
      if (componentUnmounted) postUnmountCommands.push(command);
      if (command === "get_runtime_build_info") return Promise.resolve(devRuntimeBuildInfo);
      if (command === "unregister_mcp_server") return unregistration.promise;
      return mockInvoke(command, args);
    });
    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(true);

    try {
      const { unmount } = render(
        <BrowserRouter>
          <McpPage />
        </BrowserRouter>
      );
      await screen.findByText("dev_only_enabled");
      fireEvent.click(await screen.findByTitle("删除"));
      await waitFor(() =>
        expect(
          vi.mocked(invoke).mock.calls.some(([command]) => command === "unregister_mcp_server")
        ).toBe(true)
      );

      componentUnmounted = true;
      unmount();
      await act(async () => {
        unregistration.resolve();
        await unregistration.promise;
        await Promise.resolve();
      });

      expect(postUnmountCommands).toEqual([]);
      for (const forbiddenFollowUp of [
        "list_mcp_audit_logs",
        "recommend_mcp_manifests",
        "list_mcp_servers",
        "list_mcp_tools",
        "list_mcp_templates",
        "get_privacy_policy",
      ]) {
        expect(postUnmountCommands).not.toContain(forbiddenFollowUp);
      }
    } finally {
      confirmSpy.mockRestore();
    }
  });

  it("does not start refresh IPC when typed-template registration completes after unmount", async () => {
    const registration = deferred<void>();
    const postUnmountCommands: string[] = [];
    let componentUnmounted = false;
    const template = {
      id: "late-template",
      name: "卸载边界模板",
      description: "验证模板 mutation 的 late completion",
      command: "node",
      args: [],
      required_args: [],
      manifests: [registrationManifest("late-template")],
      tags: ["late_template"],
    };
    vi.mocked(invoke).mockImplementation((command, args) => {
      if (componentUnmounted) postUnmountCommands.push(command);
      if (command === "get_runtime_build_info") return Promise.resolve(devRuntimeBuildInfo);
      if (command === "list_mcp_templates") return Promise.resolve([template]);
      if (command === "register_mcp_server") return registration.promise;
      return mockInvoke(command, args);
    });

    const { unmount } = render(
      <BrowserRouter>
        <McpPage />
      </BrowserRouter>
    );
    await screen.findByText("dev_only_enabled");
    fireEvent.click(await screen.findByRole("button", { name: "安装 typed 模板" }));
    const templateLabel = await screen.findByText("卸载边界模板");
    const templateButton = templateLabel.closest("button");
    expect(templateButton).not.toBeNull();
    fireEvent.click(templateButton as HTMLButtonElement);
    fireEvent.click(await screen.findByRole("button", { name: "确认注册" }));
    await waitFor(() =>
      expect(
        vi.mocked(invoke).mock.calls.some(([command]) => command === "register_mcp_server")
      ).toBe(true)
    );

    componentUnmounted = true;
    unmount();
    await act(async () => {
      registration.resolve();
      await registration.promise;
      await Promise.resolve();
    });

    expect(postUnmountCommands).toEqual([]);
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
    const { container } = render(
      <BrowserRouter>
        <McpPage />
      </BrowserRouter>
    );

    await waitFor(() => {
      expect(screen.getByText("安全审计中心")).toBeInTheDocument();
    });

    // Stats cards
    expect(screen.getByText("本页记录")).toBeInTheDocument();
    expect(screen.getByText("PII 命中")).toBeInTheDocument();

    // Audit log entries
    expect(screen.getAllByText("write_file").length).toBeGreaterThan(0);

    // PII hit badge (in expanded or summary)
    expect(screen.getByText("检测到敏感数据")).toBeInTheDocument();

    const receiptPreview = screen.getByText("收据预览：");
    expect(receiptPreview).toBeInTheDocument();
    const expandableLog = receiptPreview.closest("div.cursor-pointer");
    expect(expandableLog).not.toBeNull();
    fireEvent.click(expandableLog as HTMLElement);
    expect(screen.getByText("参数审计收据")).toBeInTheDocument();
    expect(screen.getByText("结果审计收据")).toBeInTheDocument();
    expect(container.textContent).not.toContain("/tmp/demo.txt");
    expect(container.textContent).not.toContain("工具执行成功");

    // Privacy rules section
    expect(screen.getByText("隐私保护规则")).toBeInTheDocument();
  });

  it("keeps non-audit capabilities when the audit command transport is unknown", async () => {
    vi.mocked(invoke).mockImplementation((command, args) => {
      if (command === "get_runtime_build_info") return Promise.resolve(devRuntimeBuildInfo);
      if (command === "list_mcp_audit_logs") {
        return Promise.reject(new Error("persistence_store_unavailable"));
      }
      return mockInvoke(command, args);
    });

    render(
      <BrowserRouter>
        <McpPage />
      </BrowserRouter>
    );

    expect(await screen.findByText("审计状态：未知")).toBeInTheDocument();
    expect(screen.getByText(/audit_command_transport_failed/)).toBeInTheDocument();
    expect(screen.queryByText("暂无审计记录")).not.toBeInTheDocument();
    for (const label of ["本页记录", "成功", "失败", "PII 命中"]) {
      const card = screen.getByText(label).parentElement;
      expect(card).not.toBeNull();
      expect(within(card as HTMLElement).getByText("未知")).toBeInTheDocument();
      expect(card?.textContent ?? "").not.toMatch(/0\s*$/);
    }

    expect(await screen.findByText(/工具数量:\s*2/)).toBeInTheDocument();
    expect(screen.getByText("read_file")).toBeInTheDocument();
    expect(screen.getByText("Phone")).toBeInTheDocument();
  });

  it.each([
    ["legacy array", []],
    ["unknown status", { status: "future", entries: [] }],
    ["missing available entries", { status: "available" }],
    [
      "malformed available entry",
      {
        status: "available",
        entries: [
          {
            id: 1,
            tool_name: "malformed_audit",
            arguments: ARGUMENTS_AUDIT_RECEIPT,
            result: RESULT_AUDIT_RECEIPT,
            success: "true",
            pii_found: false,
            created_at: "2026-07-14T00:00:00Z",
          },
        ],
      },
    ],
    [
      "status-incompatible reason",
      { status: "degraded", reasonCode: "audit_store_unavailable", entries: [] },
    ],
  ] as const)("maps a malformed %s audit projection to transport failure", async (_name, raw) => {
    vi.mocked(invoke).mockImplementation((command, args) => {
      if (command === "get_runtime_build_info") return Promise.resolve(devRuntimeBuildInfo);
      if (command === "list_mcp_audit_logs") return Promise.resolve(raw);
      return mockInvoke(command, args);
    });

    render(
      <BrowserRouter>
        <McpPage />
      </BrowserRouter>
    );

    expect(await screen.findByText("审计状态：未知")).toBeInTheDocument();
    expect(screen.getByText(/audit_command_transport_failed/)).toBeInTheDocument();
    expect(screen.queryByText("暂无审计记录")).not.toBeInTheDocument();
    expect(await screen.findByText(/工具数量:\s*2/)).toBeInTheDocument();
    expect(screen.getByText("read_file")).toBeInTheDocument();
    expect(screen.getByText("Phone")).toBeInTheDocument();
  });

  it("does not let a never-settling audit read block core MCP facts or refresh", async () => {
    vi.mocked(invoke).mockImplementation((command, args) => {
      if (command === "get_runtime_build_info") return Promise.resolve(devRuntimeBuildInfo);
      if (command === "list_mcp_audit_logs") return new Promise(() => {});
      return mockInvoke(command, args);
    });

    render(
      <BrowserRouter>
        <McpPage />
      </BrowserRouter>
    );

    expect(await screen.findByText(/工具数量:\s*2/)).toBeInTheDocument();
    expect(screen.getByText("read_file")).toBeInTheDocument();
    expect(screen.getByText("Phone")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "刷新" })).toBeEnabled();
    expect(screen.getByText("审计状态：检查中")).toBeInTheDocument();
  });

  it("does not let a never-settling recommendation read block core MCP facts", async () => {
    vi.mocked(invoke).mockImplementation((command, args) => {
      if (command === "get_runtime_build_info") return Promise.resolve(devRuntimeBuildInfo);
      if (command === "recommend_mcp_manifests") return new Promise(() => {});
      return mockInvoke(command, args);
    });

    render(
      <BrowserRouter>
        <McpPage />
      </BrowserRouter>
    );

    expect(await screen.findByText(/工具数量:\s*2/)).toBeInTheDocument();
    expect(screen.getByText("read_file")).toBeInTheDocument();
    expect(screen.getByText("Phone")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "刷新" })).toBeEnabled();
    expect(screen.getByText("正在核验推荐事实…")).toBeInTheDocument();
    expect(screen.queryByText("暂无推荐")).not.toBeInTheDocument();
  });

  it("projects a recommendation transport failure as unknown without blocking core MCP facts", async () => {
    vi.mocked(invoke).mockImplementation((command, args) => {
      if (command === "get_runtime_build_info") return Promise.resolve(devRuntimeBuildInfo);
      if (command === "recommend_mcp_manifests") {
        return Promise.reject(new Error("recommendation unavailable"));
      }
      return mockInvoke(command, args);
    });

    render(
      <BrowserRouter>
        <McpPage />
      </BrowserRouter>
    );

    expect(await screen.findByText(/工具数量:\s*2/)).toBeInTheDocument();
    expect(screen.getByText("read_file")).toBeInTheDocument();
    expect(screen.getByText("Phone")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "刷新" })).toBeEnabled();
    expect(screen.queryByText(/加载失败:.*recommendation unavailable/)).not.toBeInTheDocument();
    expect(screen.getByText(/推荐事实状态未知/)).toBeInTheDocument();
    expect(screen.getByText(/recommendation_command_transport_failed/)).toBeInTheDocument();
    expect(screen.queryByText("暂无推荐")).not.toBeInTheDocument();
    expect(screen.queryByText(/先补充目标和能力数据/)).not.toBeInTheDocument();
  });

  it("ignores a stale recommendation result after a newer trusted empty response", async () => {
    const firstRecommendation = deferred<ToolManifest[]>();
    const secondRecommendation = deferred<ToolManifest[]>();
    let recommendationCalls = 0;
    vi.mocked(invoke).mockImplementation((command, args) => {
      if (command === "get_runtime_build_info") return Promise.resolve(devRuntimeBuildInfo);
      if (command === "recommend_mcp_manifests") {
        recommendationCalls += 1;
        return recommendationCalls === 1
          ? firstRecommendation.promise
          : secondRecommendation.promise;
      }
      return mockInvoke(command, args);
    });

    render(
      <BrowserRouter>
        <McpPage />
      </BrowserRouter>
    );

    const refresh = await screen.findByRole("button", { name: "刷新" });
    await waitFor(() => expect(refresh).toBeEnabled());
    fireEvent.click(refresh);
    await act(async () => {
      secondRecommendation.resolve([]);
      await secondRecommendation.promise;
    });
    expect(await screen.findByText("暂无推荐")).toBeInTheDocument();
    expect(screen.getByText("当前后端返回零条推荐工具。")).toBeInTheDocument();

    await act(async () => {
      firstRecommendation.resolve([
        recommendationManifest("stale_recommendation", "不应覆盖当前推荐事实"),
      ]);
      await firstRecommendation.promise;
    });
    expect(screen.queryByText("不应覆盖当前推荐事实")).not.toBeInTheDocument();
    expect(screen.getByText("暂无推荐")).toBeInTheDocument();
  });

  it("ignores a stale recommendation error after a newer successful response", async () => {
    const firstRecommendation = deferred<ToolManifest[]>();
    const secondRecommendation = deferred<ToolManifest[]>();
    let recommendationCalls = 0;
    vi.mocked(invoke).mockImplementation((command, args) => {
      if (command === "get_runtime_build_info") return Promise.resolve(devRuntimeBuildInfo);
      if (command === "recommend_mcp_manifests") {
        recommendationCalls += 1;
        return recommendationCalls === 1
          ? firstRecommendation.promise
          : secondRecommendation.promise;
      }
      return mockInvoke(command, args);
    });

    render(
      <BrowserRouter>
        <McpPage />
      </BrowserRouter>
    );

    const refresh = await screen.findByRole("button", { name: "刷新" });
    await waitFor(() => expect(refresh).toBeEnabled());
    fireEvent.click(refresh);
    await act(async () => {
      secondRecommendation.resolve([
        recommendationManifest("current_recommendation", "当前可信推荐"),
      ]);
      await secondRecommendation.promise;
    });
    expect(await screen.findByText("当前可信推荐")).toBeInTheDocument();

    await act(async () => {
      firstRecommendation.reject(new Error("stale recommendation failure"));
      await firstRecommendation.promise.catch(() => undefined);
    });
    expect(screen.getByText("当前可信推荐")).toBeInTheDocument();
    expect(screen.queryByText(/推荐事实状态未知/)).not.toBeInTheDocument();
  });

  it.each(["result", "error"] as const)(
    "performs an auxiliary React warning check for a late recommendation %s after unmount",
    async settlement => {
      const recommendation = deferred<ToolManifest[]>();
      const errorSpy = vi.spyOn(console, "error").mockImplementation(() => undefined);
      try {
        vi.mocked(invoke).mockImplementation((command, args) => {
          if (command === "get_runtime_build_info") return Promise.resolve(devRuntimeBuildInfo);
          if (command === "recommend_mcp_manifests") return recommendation.promise;
          return mockInvoke(command, args);
        });

        const { unmount } = render(
          <BrowserRouter>
            <McpPage />
          </BrowserRouter>
        );
        expect(await screen.findByText(/工具数量:\s*2/)).toBeInTheDocument();
        unmount();

        await act(async () => {
          if (settlement === "result") {
            recommendation.resolve([
              recommendationManifest("late_recommendation", "卸载后不得应用"),
            ]);
          } else {
            recommendation.reject(new Error("late recommendation failure"));
          }
          await recommendation.promise.catch(() => undefined);
        });

        expect(
          errorSpy.mock.calls.some(call =>
            call.some(value => String(value).includes("state update on an unmounted component"))
          )
        ).toBe(false);
      } finally {
        errorSpy.mockRestore();
      }
    }
  );

  it("renders a trusted empty recommendation response without inventing a cause", async () => {
    vi.mocked(invoke).mockImplementation((command, args) => {
      if (command === "get_runtime_build_info") return Promise.resolve(devRuntimeBuildInfo);
      if (command === "recommend_mcp_manifests") return Promise.resolve([]);
      return mockInvoke(command, args);
    });

    render(
      <BrowserRouter>
        <McpPage />
      </BrowserRouter>
    );

    expect(await screen.findByText("暂无推荐")).toBeInTheDocument();
    expect(screen.getByText("当前后端返回零条推荐工具。")).toBeInTheDocument();
    expect(screen.queryByText(/先补充目标和能力数据/)).not.toBeInTheDocument();
    expect(screen.queryByText(/推荐事实状态未知/)).not.toBeInTheDocument();
  });

  it("ignores a stale audit response from an older refresh generation", async () => {
    const firstAudit = deferred<any>();
    const secondAudit = deferred<any>();
    let auditCalls = 0;
    vi.mocked(invoke).mockImplementation((command, args) => {
      if (command === "get_runtime_build_info") return Promise.resolve(devRuntimeBuildInfo);
      if (command === "list_mcp_audit_logs") {
        auditCalls += 1;
        return auditCalls === 1 ? firstAudit.promise : secondAudit.promise;
      }
      return mockInvoke(command, args);
    });

    render(
      <BrowserRouter>
        <McpPage />
      </BrowserRouter>
    );

    const refresh = await screen.findByRole("button", { name: "刷新" });
    await waitFor(() => expect(refresh).toBeEnabled());
    fireEvent.click(refresh);
    await act(async () => {
      secondAudit.resolve({
        status: "available",
        entries: [
          {
            id: 2,
            tool_name: "new_generation_audit",
            arguments: ARGUMENTS_AUDIT_RECEIPT,
            result: RESULT_AUDIT_RECEIPT,
            success: true,
            pii_found: false,
            created_at: "2026-07-14T00:00:00Z",
          },
        ],
      });
      await secondAudit.promise;
    });
    expect(await screen.findByText("new_generation_audit")).toBeInTheDocument();

    await act(async () => {
      firstAudit.resolve({
        status: "available",
        entries: [
          {
            id: 1,
            tool_name: "stale_generation_audit",
            arguments: ARGUMENTS_AUDIT_RECEIPT,
            result: RESULT_AUDIT_RECEIPT,
            success: true,
            pii_found: false,
            created_at: "2026-07-13T00:00:00Z",
          },
        ],
      });
      await firstAudit.promise;
    });
    expect(screen.queryByText("stale_generation_audit")).not.toBeInTheDocument();
    expect(screen.getByText("new_generation_audit")).toBeInTheDocument();
  });

  it("renders an unavailable audit projection without treating it as empty entries", async () => {
    vi.mocked(invoke).mockImplementation((command, args) => {
      if (command === "get_runtime_build_info") return Promise.resolve(devRuntimeBuildInfo);
      if (command === "list_mcp_audit_logs") {
        return Promise.resolve({
          status: "unavailable",
          reasonCode: "audit_store_unavailable",
        });
      }
      return mockInvoke(command, args);
    });

    render(
      <BrowserRouter>
        <McpPage />
      </BrowserRouter>
    );

    expect(await screen.findByText("审计状态：不可用")).toBeInTheDocument();
    expect(screen.getByText(/audit_store_unavailable/)).toBeInTheDocument();
    expect(screen.queryByText("暂无审计记录")).not.toBeInTheDocument();
    expect(screen.queryByText("收据预览：")).not.toBeInTheDocument();
    expect(screen.queryByText("参数审计收据")).not.toBeInTheDocument();
    expect(screen.queryByText("结果审计收据")).not.toBeInTheDocument();
  });

  it("renders an unknown audit projection without borrowing successful entries", async () => {
    vi.mocked(invoke).mockImplementation((command, args) => {
      if (command === "get_runtime_build_info") return Promise.resolve(devRuntimeBuildInfo);
      if (command === "list_mcp_audit_logs") {
        return Promise.resolve({ status: "unknown", reasonCode: "audit_read_failed" });
      }
      return mockInvoke(command, args);
    });

    render(
      <BrowserRouter>
        <McpPage />
      </BrowserRouter>
    );

    expect(await screen.findByText("审计状态：未知")).toBeInTheDocument();
    expect(screen.getByText(/audit_read_failed/)).toBeInTheDocument();
    expect(screen.queryByText("暂无审计记录")).not.toBeInTheDocument();
    expect(screen.queryByText("收据预览：")).not.toBeInTheDocument();
    expect(screen.queryByText("参数审计收据")).not.toBeInTheDocument();
    expect(screen.queryByText("结果审计收据")).not.toBeInTheDocument();
  });

  it("renders degraded audit truth with exact receipt entries", async () => {
    vi.mocked(invoke).mockImplementation((command, args) => {
      if (command === "get_runtime_build_info") return Promise.resolve(devRuntimeBuildInfo);
      if (command === "list_mcp_audit_logs") {
        return Promise.resolve({
          status: "degraded",
          reasonCode: "audit_store_read_only",
          entries: [
            {
              id: 65,
              tool_name: "degraded_audit_tool",
              arguments: ARGUMENTS_AUDIT_RECEIPT,
              result: RESULT_AUDIT_RECEIPT,
              success: true,
              pii_found: false,
              created_at: "2026-07-13T00:00:00Z",
            },
          ],
        });
      }
      return mockInvoke(command, args);
    });

    render(
      <BrowserRouter>
        <McpPage />
      </BrowserRouter>
    );

    expect(await screen.findByText("审计状态：只读降级")).toBeInTheDocument();
    expect(screen.getByText(/audit_store_read_only/)).toBeInTheDocument();
    expect(screen.getByText("degraded_audit_tool")).toBeInTheDocument();
    expect(screen.getByText("收据预览：")).toBeInTheDocument();
  });

  it("delegates audit retention to the governed Settings workflow", async () => {
    render(
      <BrowserRouter>
        <McpPage />
      </BrowserRouter>
    );

    await screen.findByText("安全审计中心");
    expectGovernedAuditRetentionLink();
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
    expectGovernedAuditRetentionLink();

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
    expectGovernedAuditRetentionLink();

    const invokedCommands = vi.mocked(invoke).mock.calls.map(([command]) => command);
    expect(invokedCommands).not.toContain("register_mcp_server");
    expect(invokedCommands).not.toContain("unregister_mcp_server");
  });
});
