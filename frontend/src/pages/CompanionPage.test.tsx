import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import { BrowserRouter } from "react-router-dom";
import CompanionPage from "./CompanionPage";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { mockInvoke } from "@/test/mocks/tauri";
import { FORBIDDEN_ORDINARY_CHAT_COMMANDS } from "@/test/ordinaryChatForbiddenCommands";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

describe("CompanionPage", () => {
  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    vi.mocked(invoke).mockImplementation(mockInvoke);
    vi.mocked(listen).mockResolvedValue(() => {});
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.clearAllMocks();
  });

  it("renders AgentStage with the initial idle state", async () => {
    render(
      <BrowserRouter>
        <CompanionPage />
      </BrowserRouter>
    );

    const stage = await screen.findByTestId("agent-stage");
    expect(screen.getByTestId("companion-page")).toBeInTheDocument();
    expect(stage).toHaveAttribute("data-state", "idle");
    expect(screen.getByRole("status", { name: /OpenLife Agent 状态/ })).toBeInTheDocument();
    expect(await screen.findByPlaceholderText(/输入消息/)).toBeInTheDocument();
  });

  it("moves AgentStage into listening when the composer receives focus", async () => {
    render(
      <BrowserRouter>
        <CompanionPage />
      </BrowserRouter>
    );

    const textarea = await screen.findByPlaceholderText(/输入消息/);
    fireEvent.focus(textarea);

    await waitFor(() => {
      expect(screen.getByTestId("agent-stage")).toHaveAttribute("data-state", "listening");
    });
  });

  it("keeps ordinary Send on startStreamMessage and moves AgentStage into sorting", async () => {
    render(
      <BrowserRouter>
        <CompanionPage />
      </BrowserRouter>
    );

    const textarea = await screen.findByPlaceholderText(/输入消息/);
    await screen.findByText("聊天就绪");
    fireEvent.change(textarea, { target: { value: "默认发送仍然走普通聊天" } });
    fireEvent.keyDown(textarea, { key: "Enter", code: "Enter" });

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "start_stream_message",
        expect.objectContaining({
          sessionId: "session-1",
          session_id: "session-1",
        })
      );
    });
    expect(screen.getByTestId("agent-stage")).toHaveAttribute("data-state", "sorting");

    for (const forbiddenCommand of FORBIDDEN_ORDINARY_CHAT_COMMANDS) {
      expect(
        vi.mocked(invoke).mock.calls.some(([cmd]) => cmd === forbiddenCommand),
        `${forbiddenCommand} must not be called by ordinary Companion Send`
      ).toBe(false);
    }
  });

  it("moves AgentStage into error when the ordinary stream send fails", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "start_stream_message") {
        return Promise.reject(new Error("DeepSeek error 401: invalid API Key"));
      }
      return mockInvoke(cmd, args);
    });

    render(
      <BrowserRouter>
        <CompanionPage />
      </BrowserRouter>
    );

    const textarea = await screen.findByPlaceholderText(/输入消息/);
    await screen.findByText("聊天就绪");
    fireEvent.change(textarea, { target: { value: "测试错误路径" } });
    fireEvent.keyDown(textarea, { key: "Enter", code: "Enter" });

    expect(await screen.findByText(/DeepSeek 鉴权失败/)).toBeInTheDocument();
    expect(screen.getByText(/去设置页查看“试用就绪检查”/)).toBeInTheDocument();
    expect(screen.getByTestId("agent-stage")).toHaveAttribute("data-state", "error");
  });

  it("moves AgentStage into review when pending proposals are visible", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_pending_proposals") {
        return Promise.resolve([
          {
            id: "proposal-1",
            proposalType: "memory_write",
            source: "chat_conversation",
            affectedPath: "memory.pending",
            after: {},
            reason: "Needs confirmation",
            confidence: 0.8,
            riskLevel: "low",
            status: "pending",
            createdAt: new Date().toISOString(),
          },
        ]);
      }
      return mockInvoke(cmd, args);
    });

    render(
      <BrowserRouter>
        <CompanionPage />
      </BrowserRouter>
    );

    expect(await screen.findByText(/1 个待处理提案/)).toBeInTheDocument();
    await waitFor(() => {
      expect(screen.getByTestId("agent-stage")).toHaveAttribute("data-state", "review");
    });
  });

  it("moves AgentStage into privacy when Safe Mode is active", async () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_system_diagnostics") {
        const base = (await mockInvoke(cmd, args)) as Record<string, any>;
        return {
          ...base,
          database_status: "degraded",
          startup_warnings: ["memory.db 初始化失败，正在使用临时数据库"],
          vector_corrupt_embedding_count: 2,
        };
      }
      return mockInvoke(cmd, args);
    });

    render(
      <BrowserRouter>
        <CompanionPage />
      </BrowserRouter>
    );

    expect(await screen.findByText(/Safe Mode：/)).toBeInTheDocument();
    await waitFor(() => {
      expect(screen.getByTestId("agent-stage")).toHaveAttribute("data-state", "privacy");
    });
  });
});
