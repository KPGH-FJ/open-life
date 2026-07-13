import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import { BrowserRouter } from "react-router-dom";
import A2APage from "./A2APage";
import { invoke } from "@tauri-apps/api/core";
import { mockInvoke } from "@/test/mocks/tauri";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("A2APage", () => {
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

  it("renders local A2A service section", async () => {
    render(
      <BrowserRouter>
        <A2APage />
      </BrowserRouter>
    );

    await waitFor(() => {
      expect(screen.getByText("A2A Server - OpenLife 本地服务")).toBeInTheDocument();
    });

    expect(screen.getByText("OpenLife ↔ A2A 桥接调试")).toBeInTheDocument();
  });

  it("runs local bridge preview", async () => {
    render(
      <BrowserRouter>
        <A2APage />
      </BrowserRouter>
    );

    await waitFor(() => {
      expect(screen.getByText("桥接运行")).toBeInTheDocument();
    });

    fireEvent.change(screen.getByPlaceholderText("输入要送入 OpenLife/A2A 桥接的文本"), {
      target: { value: "帮我做一个决策摘要" },
    });
    fireEvent.click(screen.getByText("桥接运行"));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "a2a_bridge_local",
        expect.objectContaining({ text: "帮我做一个决策摘要" })
      );
    });
  });

  it("keeps local service and bridge inputs independent", async () => {
    render(
      <BrowserRouter>
        <A2APage />
      </BrowserRouter>
    );

    await screen.findByText("OpenLife ↔ A2A 桥接调试");
    fireEvent.change(screen.getByPlaceholderText("输入本地固定技能的查询内容"), {
      target: { value: "本地服务输入" },
    });
    fireEvent.change(screen.getByPlaceholderText("输入要送入 OpenLife/A2A 桥接的文本"), {
      target: { value: "桥接输入" },
    });
    fireEvent.click(screen.getByText("桥接运行"));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "a2a_bridge_local",
        expect.objectContaining({ text: "桥接输入" })
      );
    });
  });

  it("freezes one authenticated dev A2A task behind external-target confirmation", async () => {
    render(
      <BrowserRouter>
        <A2APage />
      </BrowserRouter>
    );

    await screen.findByText("A2A Client - 发送 Task");
    fireEvent.change(screen.getByPlaceholderText("Agent Base URL"), {
      target: { value: "https://example.com/a2a" },
    });
    fireEvent.change(screen.getByPlaceholderText("输入要发送给 Agent 的内容"), {
      target: { value: "外部发送测试" },
    });
    const pairingToken = "p".repeat(32);
    fireEvent.change(screen.getByPlaceholderText("远端配对凭证（32 字符以上）"), {
      target: { value: pairingToken },
    });
    fireEvent.click(screen.getByRole("button", { name: "发送" }));

    expect(await screen.findByRole("dialog", { name: "确认发送 A2A Task" })).toBeInTheDocument();
    expect(vi.mocked(invoke).mock.calls.some(([cmd]) => cmd === "a2a_send_task")).toBe(false);

    // Mutating the still-mounted form after the dialog opened must not alter
    // the already-confirmed target or request body.
    fireEvent.change(screen.getByPlaceholderText("Agent Base URL"), {
      target: { value: "https://changed.example/a2a" },
    });
    fireEvent.change(screen.getByPlaceholderText("输入要发送给 Agent 的内容"), {
      target: { value: "未确认的新正文" },
    });
    const confirm = screen.getByRole("button", { name: "发送 Task" });
    fireEvent.click(confirm);
    fireEvent.click(confirm);
    await waitFor(() => {
      const calls = vi.mocked(invoke).mock.calls.filter(([command]) => command === "a2a_send_task");
      expect(calls).toHaveLength(1);
      const args = calls[0][1] as Record<string, unknown>;
      expect(args).toEqual(
        expect.objectContaining({
          url: "https://example.com/a2a",
          pairingToken,
          pairing_token: pairingToken,
        })
      );
      const request = JSON.parse(String(args.requestJson));
      expect(request.message.parts[0].text).toBe("外部发送测试");
      expect(request.message.parts[0].text).not.toBe("未确认的新正文");
      expect(request.id).toMatch(
        /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/
      );
    });
  });

  it("does not expose the dev-only outbound A2A task command in the release page", async () => {
    useRuntimeBuildInfo(releaseRuntimeBuildInfo);

    render(
      <BrowserRouter>
        <A2APage />
      </BrowserRouter>
    );

    await screen.findByText("disabled_by_build");
    expect(screen.queryByText("A2A Client - 发送 Task")).not.toBeInTheDocument();
    expect(screen.queryByPlaceholderText("输入要发送给 Agent 的内容")).not.toBeInTheDocument();
    expect(screen.queryByText("A2A Client - 发现外部 Agent")).not.toBeInTheDocument();

    const invokedCommands = vi.mocked(invoke).mock.calls.map(([command]) => command);
    expect(invokedCommands).toEqual(["get_runtime_build_info"]);
    expect(
      invokedCommands.some(command =>
        [
          "a2a_discover_agent",
          "a2a_local_agent_card",
          "a2a_handle_task",
          "a2a_bridge_local",
          "a2a_restart_sidecar",
          "a2a_stop_sidecar",
          "a2a_send_task",
        ].includes(command)
      )
    ).toBe(false);
  });

  it("fails closed without transient A2A commands when build info is unavailable", async () => {
    vi.mocked(invoke).mockImplementation((command, args) => {
      if (command === "get_runtime_build_info") return Promise.reject(new Error("unavailable"));
      return mockInvoke(command, args);
    });

    render(
      <BrowserRouter>
        <A2APage />
      </BrowserRouter>
    );

    await screen.findByText("unavailable");
    expect(screen.queryByText("A2A Client - 发现外部 Agent")).not.toBeInTheDocument();
    expect(vi.mocked(invoke).mock.calls.map(([command]) => command)).toEqual([
      "get_runtime_build_info",
    ]);
  });
});
