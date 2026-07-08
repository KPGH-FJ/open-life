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

function lifeStateProjection({
  pendingCount = 0,
  safeMode = false,
}: {
  pendingCount?: number;
  safeMode?: boolean;
} = {}) {
  return {
    version: "life_state_projection_v1",
    generatedAt: "2026-07-08T00:00:00.000Z",
    pending: {
      pendingProposalCount: pendingCount,
      editedProposalCount: 0,
      totalReviewRequiredCount: pendingCount,
      highRiskReviewRequiredCount: 0,
      proposalStoreStatus: "ok",
      requiresUserAction: pendingCount > 0,
    },
    readiness: {
      chatReady: true,
      usageReady: true,
      lifeModelReady: true,
      modelEmpty: false,
      pendingBuilderReviewSessions: 0,
      unfinishedBuilderSessions: 0,
      databaseStatus: safeMode ? "degraded" : "ok",
      readinessIssues: [],
      usageReadinessIssues: [],
    },
    taskState: {
      taskStoreStatus: "ok",
      latestTaskId: null,
      latestTaskStatus: null,
      runningCount: 0,
      waitingPermissionCount: 0,
      blockedCount: 0,
      failedCount: 0,
      cancelledCount: 0,
      completedCount: 0,
      activeCount: 0,
    },
    safeMode: {
      active: safeMode,
      reason: safeMode ? "memory.db 初始化失败，正在使用临时数据库" : "系统当前未处于 Safe Mode。",
      sourceRefs: safeMode ? ["diagnostics.startup_warnings"] : [],
    },
    toolPermissions: {
      totalCount: 0,
      activeCount: 0,
      consumedCount: 0,
      allowCount: 0,
      denyCount: 0,
      askEveryTimeCount: 0,
      allowOnceCount: 0,
      allowUntilRevokedCount: 0,
    },
    safePaths: [],
    surfaces: ["today", "mailbox", "chat", "companion", "life_model", "settings"].map(surface => ({
      surface,
      pendingReviewCount: pendingCount,
      editedReviewCount: 0,
      totalReviewRequiredCount: pendingCount,
      readinessStatus: safeMode ? "blocked" : "ready",
      taskStatus: "idle",
      safeModeActive: safeMode,
      waitingPermissionCount: 0,
      activeToolPermissionCount: 0,
    })),
    sourceRefs: ["projection:test"],
  };
}

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
    expect(screen.getByRole("button", { name: "发送消息" })).toBeInTheDocument();
  });

  it("moves AgentStage to listening when the composer receives focus", async () => {
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

  it("keeps ordinary Send on startStreamMessage without forbidden legacy commands", async () => {
    render(
      <BrowserRouter>
        <CompanionPage />
      </BrowserRouter>
    );

    const textarea = await screen.findByPlaceholderText(/输入消息/);
    await screen.findByText(/对话就绪/);
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

    for (const forbiddenCommand of FORBIDDEN_ORDINARY_CHAT_COMMANDS) {
      expect(
        vi.mocked(invoke).mock.calls.some(([cmd]) => cmd === forbiddenCommand),
        `${forbiddenCommand} must not be called by ordinary Companion Send`
      ).toBe(false);
    }
  });

  it("moves AgentStage to error when the ordinary stream send fails", async () => {
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
    await screen.findByText(/对话就绪/);
    fireEvent.change(textarea, { target: { value: "测试错误路径" } });
    fireEvent.keyDown(textarea, { key: "Enter", code: "Enter" });

    await waitFor(() => {
      expect(screen.getByTestId("agent-stage")).toHaveAttribute("data-state", "error");
    });
  });

  it("moves AgentStage to review when pending proposals are visible", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_life_state_projection")
        return Promise.resolve(lifeStateProjection({ pendingCount: 1 }));
      return mockInvoke(cmd, args);
    });

    render(
      <BrowserRouter>
        <CompanionPage />
      </BrowserRouter>
    );

    await waitFor(() => {
      expect(screen.getByTestId("agent-stage")).toHaveAttribute("data-state", "review");
    });
    expect(screen.getByText("有信等你回")).toBeInTheDocument();
  });

  it("moves AgentStage to privacy when Safe Mode is active", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_life_state_projection")
        return Promise.resolve(lifeStateProjection({ safeMode: true }));
      return mockInvoke(cmd, args);
    });

    render(
      <BrowserRouter>
        <CompanionPage />
      </BrowserRouter>
    );

    await waitFor(() => {
      expect(screen.getByTestId("agent-stage")).toHaveAttribute("data-state", "privacy");
    });
    expect(screen.getByText("边界开启")).toBeInTheDocument();
  });

  it("does not render left-stage shortcut actions below the visual", async () => {
    render(
      <BrowserRouter>
        <CompanionPage />
      </BrowserRouter>
    );

    await screen.findByTestId("agent-stage");
    expect(screen.queryByRole("navigation", { name: "陪伴快捷动作" })).not.toBeInTheDocument();
    for (const label of ["整理今天", "看看记忆", "低压力计划", "处理待确认"]) {
      expect(screen.queryByRole("link", { name: label })).not.toBeInTheDocument();
    }
  });

  it("does not expose direct-write assistant quick actions on the product companion surface", async () => {
    render(
      <BrowserRouter>
        <CompanionPage />
      </BrowserRouter>
    );

    expect(await screen.findByText("你好！我是 OpenLife。")).toBeInTheDocument();
    for (const label of ["设为今日目标", "加入记忆"]) {
      expect(screen.queryByRole("button", { name: label })).not.toBeInTheDocument();
    }
  });
});
