import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { ConversationController } from "./useConversationController";
import { ConversationPanel } from "./ConversationPanel";

function workController(): ConversationController {
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

describe("ConversationPanel", () => {
  it("updates the selected Conversation memory mode without changing the global setting", async () => {
    const user = userEvent.setup();
    const controller = workController();
    controller.selectedSessionId = "conversation-memory-mode";

    render(<ConversationPanel controller={controller} onOpenLifeModel={vi.fn()} />);

    await user.selectOptions(screen.getByRole("combobox", { name: /记忆/ }), "use_only");
    expect(controller.setMemoryMode).toHaveBeenCalledWith("use_only");
  });

  it("keeps optional Work context behind one composer disclosure", async () => {
    const user = userEvent.setup();
    const controller = workController();

    render(<ConversationPanel controller={controller} onOpenLifeModel={vi.fn()} />);

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

    render(<ConversationPanel controller={controller} onOpenLifeModel={vi.fn()} />);

    expect(screen.queryByLabelText("Project 名称")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "新建 Project" }));
    expect(screen.getByLabelText("Project 名称")).toBeInTheDocument();
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

    render(<ConversationPanel controller={controller} onOpenLifeModel={vi.fn()} />);

    expect(screen.getByPlaceholderText("告诉 OpenLife 你现在要处理什么")).toBeEnabled();
    expect(screen.getByRole("button", { name: "追加指令" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "停止回复" })).toBeEnabled();
  });
});
