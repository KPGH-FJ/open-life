import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import ToolCallCard from "./ToolCallCard";
import type { ToolCallResult } from "../tauri";

function toolCall(overrides: Partial<ToolCallResult>): ToolCallResult {
  return {
    toolRef: { id: "unknown_tool", source: "unknown" },
    actionRef: "unknown_action",
    status: "unknown",
    requiresConfirmation: false,
    privacyWarningCount: 0,
    ...overrides,
  };
}

describe("ToolCallCard", () => {
  it("fails closed when the product projection is unavailable", () => {
    render(
      <ToolCallCard
        call={toolCall({
          toolRef: { id: "unknown_tool", source: "unknown" },
          status: "unknown",
          failureCode: "tool_evidence_unverified",
        })}
      />
    );

    expect(screen.getByText("状态未知")).toBeInTheDocument();
    expect(screen.getByText(/缺少可验证的执行投影/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "重新执行" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Mailbox/ })).not.toBeInTheDocument();
  });

  it("does not treat local abort as confirmed remote cancellation", () => {
    render(
      <ToolCallCard
        call={toolCall({
          toolRef: { id: "web_search", source: "network" },
          status: "locally_aborted",
          failureCode: "tool_locally_aborted",
        })}
      />
    );

    expect(screen.getByText("本地已中止")).toBeInTheDocument();
    expect(screen.getByText(/不等于远端执行已确认停止/)).toBeInTheDocument();
    expect(screen.queryByText(/远端已取消/)).not.toBeInTheDocument();
  });

  it("does not collapse an unknown external effect into an ordinary failure", () => {
    render(
      <ToolCallCard
        call={toolCall({
          toolRef: { id: "unknown_tool", source: "network" },
          status: "effect_unknown",
          failureCode: "tool_effect_unknown",
        })}
      />
    );

    expect(screen.getByText("效果未知")).toBeInTheDocument();
    expect(screen.getByText(/副作用是否发生无法确认/)).toBeInTheDocument();
    expect(screen.queryByText("失败")).not.toBeInTheDocument();
  });

  it("renders trace preview without raw arguments or output", () => {
    render(
      <ToolCallCard
        call={toolCall({
          toolRef: { id: "memory.search", source: "local" },
          status: "success",
          outputReceipt: {
            version: 2,
            kind: "tool_output",
            provenance: "observed_tool_adapter_body",
            byteCount: 48,
            digest: `sha256:${"a".repeat(64)}`,
            verified: true,
          },
        })}
      />
    );

    expect(screen.getByText(/48 bytes/)).toBeInTheDocument();
    expect(screen.getAllByText(new RegExp(`sha256:${"a".repeat(64)}`)).length).toBeGreaterThan(0);
    expect(screen.queryByText(/raw-secret-query/)).not.toBeInTheDocument();
    expect(screen.queryByText(/raw memory context/)).not.toBeInTheDocument();
  });

  it("does not create a card-owned retry path for replayable failures", () => {
    render(
      <ToolCallCard
        call={toolCall({
          toolRef: { id: "web.fetch", source: "network" },
          status: "failed",
          failureCode: "tool_failed",
        })}
      />
    );

    expect(screen.queryByRole("button", { name: "重试" })).not.toBeInTheDocument();
  });
});
