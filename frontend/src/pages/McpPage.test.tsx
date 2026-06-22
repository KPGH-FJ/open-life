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
  beforeEach(() => {
    vi.mocked(invoke).mockImplementation(mockInvoke);
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
    expect(screen.getByText("用模板安装")).toBeInTheDocument();
  });

  it("opens template wizard from recommendation install action", async () => {
    render(
      <BrowserRouter>
        <McpPage />
      </BrowserRouter>
    );

    await waitFor(() => {
      expect(screen.getByText("用模板安装")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText("用模板安装"));

    await waitFor(() => {
      expect(screen.getByText("预览参数")).toBeInTheDocument();
    });
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

  it("requires confirmation before clearing old audit logs", async () => {
    render(
      <BrowserRouter>
        <McpPage />
      </BrowserRouter>
    );

    await screen.findByText("安全审计中心");
    fireEvent.click(screen.getByRole("button", { name: "清理 7 天前日志" }));

    expect(
      await screen.findByRole("dialog", { name: "确认清理 MCP 审计日志" })
    ).toBeInTheDocument();
    expect(vi.mocked(invoke).mock.calls.some(([cmd]) => cmd === "clear_mcp_audit_logs")).toBe(
      false
    );

    fireEvent.click(screen.getByRole("button", { name: "清理日志" }));
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("clear_mcp_audit_logs", { days: 7 });
    });
  });
});
