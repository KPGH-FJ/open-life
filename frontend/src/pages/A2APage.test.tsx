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
  beforeEach(() => {
    vi.mocked(invoke).mockImplementation(mockInvoke);
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

  it("requires confirmation before sending to an external A2A URL", async () => {
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
    fireEvent.click(screen.getByRole("button", { name: "发送" }));

    expect(await screen.findByRole("dialog", { name: "确认发送 A2A Task" })).toBeInTheDocument();
    expect(vi.mocked(invoke).mock.calls.some(([cmd]) => cmd === "a2a_send_task")).toBe(false);

    fireEvent.click(screen.getByRole("button", { name: "发送 Task" }));
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "a2a_send_task",
        expect.objectContaining({ url: "https://example.com/a2a" })
      );
    });
  });
});
