import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { WorkspaceConversationController } from "./useWorkspaceConversation";
import { WorkspaceConversationPanel } from "./WorkspaceConversationPanel";

function workController(): WorkspaceConversationController {
  return {
    sessions: [],
    projects: [],
    selectedProjectId: null,
    selectedSessionId: null,
    globalMemoryEnabled: true,
    memoryMode: "use_and_learn",
    messages: [],
    draft: "",
    loadStatus: "ready",
    loadError: null,
    turnState: { phase: "idle" },
    streamingReply: "",
    activeTaskId: null,
    mode: "work",
    provider: {
      status: "ready",
      profiles: [
        {
          profileId: "deepseek-default",
          providerId: "deepseek",
          modelId: "deepseek-v4-flash",
          endpointClass: "cloud",
          selected: true,
        },
      ],
      selectedProfileId: "deepseek-default",
      errorCode: null,
    },
    workStatus: "available",
    sessionMutation: { phase: "idle" },
    pendingResources: [],
    pendingResourceTurnOperationId: null,
    resourceMutation: { phase: "idle" },
    skills: [],
    selectedSkillId: null,
    toolCandidates: null,
    capabilityState: { phase: "idle" },
    busy: false,
    ensureLoaded: vi.fn(),
    reload: vi.fn().mockResolvedValue(true),
    selectSession: vi.fn(),
    startNewConversation: vi.fn(),
    createProject: vi.fn().mockResolvedValue(true),
    assignProject: vi.fn().mockResolvedValue(true),
    setMemoryMode: vi.fn().mockResolvedValue(true),
    setDraft: vi.fn(),
    setMode: vi.fn(),
    attachResources: vi.fn().mockResolvedValue(true),
    detachResource: vi.fn().mockResolvedValue(true),
    selectSkill: vi.fn().mockResolvedValue(true),
    sendAction: () => ({
      id: "workspace.send",
      label: "发送",
      kind: "start",
      enabled: false,
      disabledReason: "先输入要发送的内容。",
      targetRef: "workspace",
    }),
    send: vi.fn().mockResolvedValue(undefined),
    steer: vi.fn().mockResolvedValue(undefined),
    cancel: vi.fn().mockResolvedValue(undefined),
    renameSelected: vi.fn().mockResolvedValue(true),
    deleteSelected: vi.fn().mockResolvedValue(true),
  };
}

describe("WorkspaceConversationPanel", () => {
  it("updates the selected Conversation memory mode without changing the global setting", async () => {
    const user = userEvent.setup();
    const controller = workController();
    controller.selectedSessionId = "conversation-memory-mode";

    render(<WorkspaceConversationPanel controller={controller} onOpenLifeModel={vi.fn()} />);

    await user.selectOptions(screen.getByRole("combobox", { name: /记忆/ }), "use_only");
    expect(controller.setMemoryMode).toHaveBeenCalledWith("use_only");
  });

  it("keeps optional Work context behind one composer disclosure", async () => {
    const user = userEvent.setup();
    const controller = workController();

    render(<WorkspaceConversationPanel controller={controller} onOpenLifeModel={vi.fn()} />);

    const summary = screen.getByText("文件、技能与工具");
    const details = summary.closest("details");
    expect(details).not.toHaveAttribute("open");
    expect(screen.getByText("按需添加")).toBeInTheDocument();
    expect(screen.getByText(/当前模型：/)).toHaveTextContent(
      "当前模型：deepseek · deepseek-v4-flash"
    );

    await user.click(summary);
    expect(details).toHaveAttribute("open");
    expect(screen.getByRole("button", { name: "添加文件" })).toBeInTheDocument();
  });

  it("keeps Project creation out of the default conversation rail", async () => {
    const user = userEvent.setup();
    const controller = workController();

    render(<WorkspaceConversationPanel controller={controller} onOpenLifeModel={vi.fn()} />);

    expect(screen.queryByLabelText("Project 名称")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "新建 Project" }));
    expect(screen.getByLabelText("Project 名称")).toBeInTheDocument();
  });

  it("shows a human-readable basis for a source-bound answer", () => {
    const controller = workController();
    controller.turnState = {
      phase: "resolved",
      sessionId: "session-source-bound",
      status: "completed",
      blockers: [],
      sourceBoundBasis: {
        factCount: 3,
        sourceTypes: ["current_message"],
        checkStatus: "semantic_support_passed",
      },
    };

    render(<WorkspaceConversationPanel controller={controller} onOpenLifeModel={vi.fn()} />);

    expect(screen.getByText("本轮按限定资料回答")).toBeInTheDocument();
    expect(screen.getByText("查看回答依据")).toBeInTheDocument();
    expect(screen.getByText("采用资料：本轮消息")).toBeInTheDocument();
    expect(screen.getByText("本轮事实块：3 条")).toBeInTheDocument();
  });

  it("does not claim an answer when the source-bound check failed closed", () => {
    const controller = workController();
    controller.turnState = {
      phase: "resolved",
      sessionId: "session-source-bound-blocked",
      status: "blocked",
      blockers: ["source_bound_claim_unsupported"],
      sourceBoundBasis: {
        factCount: 1,
        sourceTypes: ["document_or_resource"],
        checkStatus: "failed_closed",
      },
    };

    render(<WorkspaceConversationPanel controller={controller} onOpenLifeModel={vi.fn()} />);

    expect(screen.getByText("本轮限定资料边界")).toBeInTheDocument();
    expect(
      screen.getByText("OpenLife 没有展示无法在你指定资料范围内核对的回答。")
    ).toBeInTheDocument();
    expect(screen.queryByText("本轮按限定资料回答")).not.toBeInTheDocument();
  });

  it("keeps the composer visible while Work offers steering and stop", () => {
    const controller = workController();
    controller.draft = "把风险结论放在最前面";
    controller.activeTaskId = "task-steer";
    controller.turnState = {
      phase: "streaming",
      sessionId: "conversation-1",
      turnId: "turn-steer",
      taskId: "task-steer",
      runId: "run-steer",
    };

    render(<WorkspaceConversationPanel controller={controller} onOpenLifeModel={vi.fn()} />);

    expect(screen.getByPlaceholderText("告诉 OpenLife 你现在要处理什么")).toBeEnabled();
    expect(screen.getByRole("button", { name: "追加指令" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "停止回复" })).toBeEnabled();
  });
});
