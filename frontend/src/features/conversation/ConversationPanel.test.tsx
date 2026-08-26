import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { ConversationController } from "./useConversationController";
import { ConversationPanel } from "./ConversationPanel";

function workController(): ConversationController {
  return {
    sessions: [],
    archivedSessions: [],
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
    executionMode: "scoped_agent",
    provider: {
      status: "ready",
      profiles: [
        {
          profileId: "deepseek-default",
          providerId: "deepseek",
          modelId: "deepseek-v4-flash",
          endpointClass: "cloud",
          selected: true,
          availability: "ready",
          unavailableReason: null,
          sizeBytes: null,
          protocol: "openai_compatible_chat_completions",
          structuredOutputContract: "json_object_requested_locally_validated",
          reasoningControl: "effort_selector",
          supportedReasoningEfforts: ["none", "high", "max"],
          defaultReasoningEffort: "high",
          reasoningMandatory: false,
          reasoningCapabilitySource: "official_builtin",
          inputModalities: ["text"],
          inputCapabilitySource: "adapter_default",
          chatCompatibility: "validated",
          workCompatibility: "unverified",
          workCompatibilityReason: null,
        },
      ],
      selectedProfileId: "deepseek-default",
      selectedReasoningEffort: null,
      errorCode: null,
    },
    workStatus: "available",
    sessionMutation: { phase: "idle" },
    pendingResources: [],
    pendingResourceTurnOperationId: null,
    resourceMutation: { phase: "idle" },
    skills: [],
    selectedSkillId: null,
    selectedSkillDetail: null,
    toolCandidates: null,
    capabilityState: { phase: "idle" },
    busy: false,
    ensureLoaded: vi.fn(),
    reload: vi.fn().mockResolvedValue(true),
    selectSession: vi.fn(),
    startNewConversation: vi.fn(),
    createProject: vi.fn().mockResolvedValue(true),
    bindProjectDirectory: vi.fn().mockResolvedValue(true),
    addProjectReadRoot: vi.fn().mockResolvedValue(true),
    removeProjectReadRoot: vi.fn().mockResolvedValue(true),
    updateProjectName: vi.fn().mockResolvedValue(true),
    archiveProject: vi.fn().mockResolvedValue(true),
    restoreProject: vi.fn().mockResolvedValue(true),
    deleteProject: vi.fn().mockResolvedValue(true),
    assignProject: vi.fn().mockResolvedValue(true),
    setMemoryMode: vi.fn().mockResolvedValue(true),
    selectProviderProfile: vi.fn().mockResolvedValue(true),
    selectReasoningEffort: vi.fn().mockReturnValue(true),
    setDraft: vi.fn(),
    setMode: vi.fn(),
    setExecutionMode: vi.fn().mockReturnValue(true),
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
    archiveSelected: vi.fn().mockResolvedValue(true),
    restoreArchived: vi.fn().mockResolvedValue(true),
    deleteArchived: vi.fn().mockResolvedValue(true),
    deleteSelected: vi.fn().mockResolvedValue(true),
  };
}

describe("ConversationPanel", () => {
  it("restores durable attachment provenance and distinguishes an unavailable owner", () => {
    const controller = workController();
    controller.sessions = [
      {
        session_id: "conversation-attachments",
        title: "Attachment audit",
        created_at: "2026-08-24T00:00:00Z",
        updated_at: "2026-08-24T00:00:00Z",
      },
    ];
    controller.selectedSessionId = "conversation-attachments";
    controller.messages = [
      {
        turnId: "turn-with-file",
        role: "user",
        content: "Use this file",
        attachmentsStatus: "ready",
        attachments: [
          {
            resourceId: "resource-1",
            filename: "requirements.md",
            detectedMime: "text/markdown",
            format: "markdown",
            digest: "a".repeat(64),
            byteCount: 2048,
            chunkCount: 2,
          },
        ],
      },
      {
        turnId: "turn-owner-unavailable",
        role: "user",
        content: "Older turn",
        attachmentsStatus: "unavailable",
        attachments: [],
      },
    ];

    render(<ConversationPanel controller={controller} onOpenLifeModel={vi.fn()} />);

    expect(screen.getByText("requirements.md")).toBeInTheDocument();
    expect(screen.getByText("MARKDOWN · 2 KB")).toBeInTheDocument();
    expect(
      screen.getByText("这一轮的附件记录暂时无法读取，不能确认是否使用过文件。")
    ).toBeInTheDocument();
  });

  it("selects memory mode before creation and updates it for an existing Conversation", async () => {
    const user = userEvent.setup();
    const controller = workController();
    const { rerender } = render(
      <ConversationPanel controller={controller} onOpenLifeModel={vi.fn()} />
    );

    await user.selectOptions(screen.getByRole("combobox", { name: /记忆/ }), "use_only");
    expect(controller.setMemoryMode).toHaveBeenCalledWith("use_only");

    controller.selectedSessionId = "conversation-memory-mode";
    controller.memoryMode = "use_only";
    rerender(<ConversationPanel controller={controller} onOpenLifeModel={vi.fn()} />);
    await user.selectOptions(screen.getByRole("combobox", { name: /记忆/ }), "off");
    expect(controller.setMemoryMode).toHaveBeenLastCalledWith("off");
  });

  it("keeps optional Work context behind one composer disclosure", async () => {
    const user = userEvent.setup();
    const controller = workController();

    render(<ConversationPanel controller={controller} onOpenLifeModel={vi.fn()} />);

    const summary = screen.getByText("文件、技能与工具");
    const details = summary.closest("details");
    expect(details).not.toHaveAttribute("open");
    expect(screen.getByText("按需添加")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "模型" })).toHaveTextContent("deepseek-v4-flash");

    await user.click(summary);
    expect(details).toHaveAttribute("open");
    expect(screen.getByRole("button", { name: "添加文件" })).toBeInTheDocument();
  });

  it("keeps navigation and lifecycle administration out of a new Conversation", () => {
    const controller = workController();

    render(<ConversationPanel controller={controller} onOpenLifeModel={vi.fn()} />);

    expect(screen.queryByText("对话设置")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "新对话" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /打开.*文件夹/ })).not.toBeInTheDocument();
  });

  it("offers only the exact backend-owned recovery control beside a failed Work turn", async () => {
    const user = userEvent.setup();
    const controller = workController();
    controller.selectedSessionId = "conversation-failed-work";
    controller.turnState = {
      phase: "resolved",
      sessionId: "conversation-failed-work",
      status: "failed",
      blockers: ["provider_timeout"],
      taskId: "task-failed-work",
      runId: "run-failed-work",
    };
    const onRequestRecovery = vi.fn();
    const recoveryControl = {
      id: "task-failed-work:retry",
      label: "Retry",
      kind: "retry" as const,
      effect: "task_retry_request" as const,
      enabled: true,
      requiresConfirmation: false,
      targetTaskId: "task-failed-work",
      targetActionId: "run-failed-work",
      completionProofAfterDispatch: false,
    };

    render(
      <ConversationPanel
        controller={controller}
        onOpenLifeModel={vi.fn()}
        recoveryControl={recoveryControl}
        onRequestRecovery={onRequestRecovery}
      />
    );

    await user.click(screen.getByRole("button", { name: "重试并创建新运行" }));
    expect(onRequestRecovery).toHaveBeenCalledWith(recoveryControl, "task-failed-work");
  });

  it("offers a direct model-settings recovery action for a provider preparation failure", async () => {
    const user = userEvent.setup();
    const controller = workController();
    controller.turnState = {
      phase: "failed",
      stage: "send",
      reason: "provider_request_preparation_failed",
    };
    const onOpenProviderSettings = vi.fn();

    render(
      <ConversationPanel
        controller={controller}
        onOpenLifeModel={vi.fn()}
        onOpenProviderSettings={onOpenProviderSettings}
      />
    );

    expect(
      screen.getByText("当前模型连接尚未准备好。请在设置中检查模型后重试。")
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "检查模型连接" }));
    expect(onOpenProviderSettings).toHaveBeenCalledOnce();
  });

  it("keeps the explicit Work execution ceiling in the progressive options disclosure", async () => {
    const user = userEvent.setup();
    const controller = workController();
    render(<ConversationPanel controller={controller} onOpenLifeModel={vi.fn()} />);

    const summary = screen.getByText("本轮选项");
    const details = summary.closest("details");
    expect(details).not.toHaveAttribute("open");
    await user.click(summary);

    const select = screen.getByRole("combobox", { name: /执行方式/ });
    expect(select).toHaveValue("scoped_agent");
    expect(screen.getByText(/扩大范围或产生重要外部影响时再请你决定/)).toBeInTheDocument();
    await user.selectOptions(select, "observe_only");
    expect(controller.setExecutionMode).toHaveBeenCalledWith("observe_only");
  });

  it("shows backend-owned tool admission and blocked reasons instead of name-only badges", async () => {
    const user = userEvent.setup();
    const controller = workController();
    controller.selectedSkillId = "research";
    controller.selectedSkillDetail = {
      skillId: "research",
      manifest: {},
      boundedInstructionsPreview: "Review evidence only.",
      allowedTools: ["web.search"],
      disallowedTools: ["write"],
      policyNotes: ["Bounded context only."],
      requiredPermissions: [],
      evidenceDigest: "sha256:skill-detail",
      redactionSummary: "bounded",
    };
    controller.toolCandidates = {
      taskId: null,
      candidates: [
        {
          candidateId: "notes.lookup",
          toolName: "notes.lookup",
          source: "mcp:local-notes",
          capabilityLabels: ["read", "notes"],
          riskLevel: "low",
          selectionReason: "manifest_default_order",
          policyDecision: "allow",
          requiresPermission: false,
          candidateDigest: "sha256:tool-candidate",
          linkedActionId: null,
        },
      ],
      blockedTools: [
        {
          toolName: "mail.send",
          reasonCode: "write_like_tool_blocked",
          policyDecision: "blocked",
          requiresPermission: false,
          blockerId: "blocker-mail-send",
        },
      ],
      failureRecovery: null,
      evidenceDigest: "sha256:tool-list",
      controls: [],
    };
    render(<ConversationPanel controller={controller} onOpenLifeModel={vi.fn()} />);

    await user.click(screen.getByText("文件、技能与工具"));
    expect(screen.getByText("本轮可用 · 无需逐次授权")).toBeInTheDocument();
    expect(screen.getByText("已注册并满足只读工具契约")).toBeInTheDocument();
    await user.click(screen.getByText("查看技能能力边界"));
    expect(screen.getByText("web.search")).toBeInTheDocument();
    expect(screen.getByText("技能本身不授予权限")).toBeInTheDocument();
    await user.click(screen.getByText("1 个工具未准入本轮"));
    expect(screen.getByText("mail.send")).toBeInTheDocument();
    expect(screen.getByText("包含写入或外部副作用")).toBeInTheDocument();
  });

  it("keeps current Project scope controls in a compact Conversation setting", async () => {
    const user = userEvent.setup();
    const controller = workController();
    controller.projects = [
      {
        id: "project-active",
        name: "Active Project",
        workspaceRoot: "/tmp/active",
        additionalReadRoots: [
          { id: "root-reference", name: "Reference notes", path: "/tmp/reference" },
        ],
        revision: 2,
        status: "active",
        createdAt: "2026-08-24T00:00:00Z",
        updatedAt: "2026-08-24T00:00:00Z",
        activeConversationCount: 0,
        totalConversationCount: 0,
        taskRunReferenceCount: 0,
        selectedForNewConversation: false,
        allowedControls: ["update", "archive"],
        blockerCodes: [],
      },
      {
        id: "project-archived",
        name: "Archived Project",
        workspaceRoot: "/tmp/archived",
        additionalReadRoots: [],
        revision: 5,
        status: "archived",
        createdAt: "2026-08-24T00:00:00Z",
        updatedAt: "2026-08-24T00:00:00Z",
        activeConversationCount: 0,
        totalConversationCount: 0,
        taskRunReferenceCount: 0,
        selectedForNewConversation: false,
        allowedControls: ["restore", "delete"],
        blockerCodes: [],
      },
    ];
    controller.selectedProjectId = "project-active";

    render(<ConversationPanel controller={controller} onOpenLifeModel={vi.fn()} />);

    const settings = screen.getByText("对话设置").closest("details");
    expect(settings).not.toHaveAttribute("open");
    await user.click(screen.getByText("对话设置"));
    await user.click(screen.getByRole("button", { name: "添加读取文件夹" }));
    expect(controller.addProjectReadRoot).toHaveBeenCalledWith("project-active", 2);
    await user.click(screen.getByRole("button", { name: "移除读取范围 Reference notes" }));
    expect(controller.removeProjectReadRoot).toHaveBeenCalledWith(
      "project-active",
      "root-reference",
      2
    );

    await user.click(screen.getByRole("button", { name: "归档" }));
    expect(controller.archiveProject).toHaveBeenCalledWith("project-active", 2);
    expect(screen.queryByText("Archived Project")).not.toBeInTheDocument();
  });

  it("keeps current Conversation lifecycle controls compact and archived history elsewhere", async () => {
    const user = userEvent.setup();
    const controller = workController();
    controller.sessions = [
      {
        session_id: "conversation-active",
        title: "季度研究",
        status: "active",
        allowedControls: ["archive"],
        blockerCodes: [],
        created_at: "2026-08-24T00:00:00Z",
        updated_at: "2026-08-24T00:00:00Z",
      },
    ];
    controller.selectedSessionId = "conversation-active";
    controller.archivedSessions = [
      {
        session_id: "conversation-empty-archived",
        title: "空白草稿",
        status: "archived",
        turnCount: 0,
        itemCount: 0,
        taskReferenceCount: 0,
        activeTaskCount: 0,
        allowedControls: ["restore", "delete"],
        blockerCodes: [],
        created_at: "2026-08-24T00:00:00Z",
        updated_at: "2026-08-24T00:00:00Z",
      },
    ];

    render(<ConversationPanel controller={controller} onOpenLifeModel={vi.fn()} />);

    await user.click(screen.getByText("对话设置"));
    await user.click(screen.getByRole("button", { name: "归档" }));
    expect(controller.archiveSelected).toHaveBeenCalled();
    expect(screen.queryByRole("searchbox")).not.toBeInTheDocument();
    expect(screen.queryByText("空白草稿")).not.toBeInTheDocument();
  });

  it("selects a ready model from the composer instead of only displaying Settings", async () => {
    const user = userEvent.setup();
    const controller = workController();
    controller.provider.profiles.push({
      profileId: "local-llama3",
      providerId: "ollama",
      modelId: "llama3:latest",
      endpointClass: "local",
      selected: false,
      availability: "ready",
      unavailableReason: null,
      sizeBytes: 4_920_753_328,
      protocol: "ollama_chat",
      structuredOutputContract: "json_schema_requested_locally_validated",
      reasoningControl: "provider_default_only",
      supportedReasoningEfforts: [],
      defaultReasoningEffort: null,
      reasoningMandatory: false,
      reasoningCapabilitySource: "unavailable",
      inputModalities: ["text"],
      inputCapabilitySource: "adapter_default",
      chatCompatibility: "reachable_unverified",
      workCompatibility: "observed_contract_failure",
      workCompatibilityReason: "agent_step_artifact_content_type_invalid",
    });

    render(<ConversationPanel controller={controller} onOpenLifeModel={vi.fn()} />);

    await user.click(screen.getByRole("button", { name: "模型" }));
    await user.click(screen.getByRole("option", { name: /llama3:latest/ }));
    expect(controller.selectProviderProfile).toHaveBeenCalledWith("local-llama3");
  });

  it("shows every configured model when the model picker opens", async () => {
    const user = userEvent.setup();
    const controller = workController();
    controller.provider.profiles.push({
      ...controller.provider.profiles[0],
      profileId: "openai-gpt-5-6",
      providerId: "openai",
      modelId: "gpt-5.6",
      selected: false,
    });

    render(<ConversationPanel controller={controller} onOpenLifeModel={vi.fn()} />);

    await user.click(screen.getByRole("button", { name: "模型" }));
    expect(screen.getByRole("option", { name: /gpt-5.6/ })).toBeInTheDocument();
  });

  it("shows and changes reasoning effort only for a supported model profile", async () => {
    const user = userEvent.setup();
    const controller = workController();
    controller.provider.profiles = [
      {
        ...controller.provider.profiles[0],
        profileId: "openai-gpt-5-6-sol",
        providerId: "openai",
        modelId: "gpt-5.6-sol",
        reasoningControl: "effort_selector",
        supportedReasoningEfforts: ["none", "low", "medium", "high", "xhigh", "max"],
        defaultReasoningEffort: "medium",
        reasoningMandatory: false,
        reasoningCapabilitySource: "official_builtin",
      },
    ];
    controller.provider.selectedProfileId = "openai-gpt-5-6-sol";
    controller.provider.selectedReasoningEffort = "medium";

    render(<ConversationPanel controller={controller} onOpenLifeModel={vi.fn()} />);

    const selector = screen.getByRole("combobox", { name: /推理强度/ });
    expect(selector).toHaveValue("medium");
    await user.selectOptions(selector, "high");
    expect(controller.selectReasoningEffort).toHaveBeenCalledWith("high");
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
