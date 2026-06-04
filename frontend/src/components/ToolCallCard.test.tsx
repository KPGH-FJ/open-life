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
    expect(screen.getByText("$.query 命中 Email: [redacted]")).toBeInTheDocument();
    expect(screen.getByText("脱敏后参数预览:")).toBeInTheDocument();
    expect(screen.getByText(/<EMAIL_0>/)).toBeInTheDocument();
    expect(screen.queryByText("test@example.com")).not.toBeInTheDocument();
  });

  it("renders trace preview without raw arguments or output", () => {
    render(
      <ToolCallCard
        call={{
          name: "memory.search",
          arguments: { query: "raw-secret-query@example.com" },
          success: true,
          output: "raw memory context should not render",
          permission_level: "low",
          status: "success",
          requires_confirmation: false,
          react_trace: {
            actionId: "action-1",
            stepIndex: 0,
            toolCallIndex: 0,
            actionType: "mcp_tool",
            toolId: "memory.search",
            toolName: "memory.search",
            toolSource: "builtin",
            actionCategory: "read",
            riskLevel: "low",
            status: "succeeded",
            outputPreview: "48 bytes redacted",
            outputHash: "sha256:abc123",
            outputByteCount: 48,
            metadataSafe: true,
          },
        }}
      />
    );

    expect(screen.getByText(/48 bytes redacted/)).toBeInTheDocument();
    expect(screen.getByText(/sha256:abc123/)).toBeInTheDocument();
    expect(screen.queryByText(/raw-secret-query/)).not.toBeInTheDocument();
    expect(screen.queryByText(/raw memory context/)).not.toBeInTheDocument();
  });

  it("only shows replay affordance when a call is replayable", () => {
    const onReplay = vi.fn().mockResolvedValue(undefined);
    const baseCall = {
      name: "web.fetch",
      arguments: {},
      success: false,
      permission_level: "medium",
      status: "error" as const,
      requires_confirmation: false,
      error: "fetch failed",
    };

    const { rerender } = render(<ToolCallCard call={baseCall} onReplay={onReplay} />);
    expect(screen.queryByRole("button", { name: "重试" })).not.toBeInTheDocument();

    rerender(<ToolCallCard call={{ ...baseCall, replayable: true }} onReplay={onReplay} />);
    expect(screen.getByRole("button", { name: "重试" })).toBeInTheDocument();
  });
});
