import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import ToolCallCard from "./ToolCallCard";

describe("ToolCallCard", () => {
  it("requires explicit confirmation before executing high-risk tools", async () => {
    const onExecute = vi.fn().mockResolvedValue(undefined);

    render(
      <ToolCallCard
        call={{
          name: "write_file",
          arguments: { path: "/tmp/demo.txt" },
          success: false,
          permission_level: "high",
          status: "needs_confirmation",
          requires_confirmation: true,
        }}
        onExecute={onExecute}
      />
    );

    expect(screen.getByText("待授权")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "重新执行" }));

    await waitFor(() => {
      expect(onExecute).toHaveBeenCalledTimes(1);
    });
  });

  it("shows privacy warnings and sanitized arguments", () => {
    render(
      <ToolCallCard
        call={{
          name: "web_search",
          arguments: { query: "查 test@example.com" },
          sanitized_arguments: { query: "查 <EMAIL_0>" },
          success: false,
          permission_level: "medium",
          status: "needs_confirmation",
          requires_confirmation: true,
          pii_found: true,
          privacy_warnings: ["$.query 命中 Email: test@example.com"],
        }}
      />
    );

    expect(screen.getByText("隐私提醒:")).toBeInTheDocument();
    expect(screen.getByText("$.query 命中 Email: test@example.com")).toBeInTheDocument();
    expect(screen.getByText("脱敏后参数预览:")).toBeInTheDocument();
    expect(screen.getByText(/<EMAIL_0>/)).toBeInTheDocument();
  });

  // ── Batch 5: Typed Execution Contract tests ───────────────────────────

  it("renders structured block reason from typed output fields", () => {
    render(
      <ToolCallCard
        call={{
          name: "web.search",
          arguments: { query: "test" },
          success: false,
          permission_level: "medium",
          status: "blocked",
          output: {
            block_reason: "agent_spec_denied",
            agent_spec_id: "main.default",
          },
          permission_decision: "deny",
        }}
      />
    );

    expect(screen.getByText("Typed Reason")).toBeInTheDocument();
    expect(screen.getByText("AgentSpec 拒绝")).toBeInTheDocument();
    expect(screen.getByText("AgentSpec: main.default")).toBeInTheDocument();
  });

  it("renders needs confirmation state from typed proposal_reason", () => {
    render(
      <ToolCallCard
        call={{
          name: "web.search",
          arguments: { query: "test" },
          success: false,
          permission_level: "medium",
          status: "needs_confirmation",
          output: {
            proposal_reason: "network_policy_ask",
            proposal_id: "proposal-net-1",
          },
        }}
      />
    );

    expect(screen.getByText("Typed Reason")).toBeInTheDocument();
    expect(screen.getByText("网络策略询问")).toBeInTheDocument();
    expect(screen.getByText("Proposal: proposal-net-1")).toBeInTheDocument();
  });

  it("renders failure kind from typed output", () => {
    render(
      <ToolCallCard
        call={{
          name: "mcp.call_tool",
          arguments: { server_name: "my-server" },
          success: false,
          permission_level: "high",
          status: "blocked",
          output: {
            block_reason: "missing_mcp_client",
            failure_kind: "mcp_client_error",
          },
        }}
      />
    );

    expect(screen.getByText("Typed Reason")).toBeInTheDocument();
    expect(screen.getByText("缺少 MCP 客户端")).toBeInTheDocument();
    expect(screen.getByText("MCP 客户端错误")).toBeInTheDocument();
  });

  // ── Hardened: invalid typed reasons are never displayed ──

  it("invalid block_reason not displayed as raw text", () => {
    render(
      <ToolCallCard
        call={{
          name: "web.search",
          arguments: { query: "test" },
          success: false,
          permission_level: "medium",
          status: "blocked",
          output: {
            block_reason: "unknown_random_string",
          },
          permission_decision: "deny",
        }}
      />
    );

    // TypedReasonBlock should only appear when there's at least one valid typed field
    // With invalid block_reason, nothing in TypedReasonBlock → component not rendered
    expect(screen.queryByText("unknown_random_string")).toBeNull();
  });

  it("error text does not drive typed reason display", () => {
    render(
      <ToolCallCard
        call={{
          name: "web.search",
          arguments: { query: "test" },
          success: false,
          permission_level: "medium",
          status: "blocked",
          output: {
            block_reason: "invalid_reason",
          },
          error: "network_policy_denied: this is just an error text",
          permission_decision: "deny",
        }}
      />
    );

    // Invalid block_reason + error text should NOT produce typed reason
    expect(screen.queryByText("网络策略拒绝")).toBeNull();
    expect(screen.queryByText("invalid_reason")).toBeNull();
  });
});
