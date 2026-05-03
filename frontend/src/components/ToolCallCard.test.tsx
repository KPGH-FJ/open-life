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
});
