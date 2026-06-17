import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, fireEvent, act } from "@testing-library/react";
import { BrowserRouter, MemoryRouter, Route, Routes, useLocation } from "react-router-dom";
import ChatPage from "./ChatPage";
import ProposalReviewPage from "./ProposalReviewPage";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { mockInvoke, mockLifeModel } from "@/test/mocks/tauri";
import type { MainChatAgentStateSnapshot, SystemDiagnostics } from "../tauri";
import { FORBIDDEN_ORDINARY_CHAT_COMMANDS } from "@/test/ordinaryChatForbiddenCommands";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

function buildMainChatAgentStateSnapshot(
  overrides: Partial<MainChatAgentStateSnapshot> = {}
): MainChatAgentStateSnapshot {
  const base: MainChatAgentStateSnapshot = {
    task: {
      taskId: "mainchat-task-product-ui-1",
      runId: "run-product-ui-1",
      conversationId: "session-1",
      userMessageId: "message-product-ui-1",
      title: "Workspace planning read",
      strategy: "react_tool_execution",
      status: "completed",
      createdAt: "2026-06-16T00:00:00.000Z",
      updatedAt: "2026-06-16T00:00:02.000Z",
      traceAvailable: true,
      controls: ["retry_failed_action", "cancel_task"],
      actionIds: ["action-product-ui-1"],
      observationIds: ["observation-product-ui-1"],
      blockerIds: [],
      proposalIds: [],
      finalDeliveryId: "delivery-product-ui-1",
    },
    route: {
      strategy: "react_tool_execution",
      reason: "read_workspace_context",
      confidence: 0.91,
    },
    context: [
      {
        contextId: "context-product-ui-1",
        sourceKind: "workspace",
        sourceLabel: "AGENTS.md bounded context",
        evidenceId: "evidence-context-product-ui-1",
      },
    ],
    provider: {
      provider: "scripted_eval_provider",
      model: "scripted-main-chat",
      routeType: "local_eval",
      reason: "eval_trace",
      evidenceId: "evidence-provider-product-ui-1",
    },
    plan: {
      planId: "plan-product-ui-1",
      status: "completed",
      summary: "Read the workspace context and synthesize the next step.",
      editable: false,
      source: "agent_loop",
      evidenceId: "evidence-plan-product-ui-1",
    },
    actions: [
      {
        actionId: "action-product-ui-1",
        actionType: "file.read",
        target: "AGENTS.md",
        label: "Read workspace guidance",
        status: "completed",
        riskLevel: "low",
        policyDecisionId: "policy-product-ui-1",
        startedAt: "2026-06-16T00:00:00.500Z",
        finishedAt: "2026-06-16T00:00:01.000Z",
        observationIds: ["observation-product-ui-1"],
        retryable: false,
      },
    ],
    observations: [
      {
        observationId: "observation-product-ui-1",
        actionId: "action-product-ui-1",
        sourceKind: "workspace_file",
        sourceLabel: "AGENTS.md",
        preview: "Main Chat Agent v1 stays proposal-first and evidence-backed.",
        citationAvailable: true,
        createdAt: "2026-06-16T00:00:01.200Z",
      },
    ],
    blockers: [],
    proposals: [],
    finalDelivery: {
      deliveryId: "delivery-product-ui-1",
      taskId: "mainchat-task-product-ui-1",
      runId: "run-product-ui-1",
      status: "delivered",
      headline: "Workspace guidance summarized",
      answer: "Use the bounded workspace guidance without creating durable writes.",
      completedActions: ["action-product-ui-1"],
      observationsUsed: ["observation-product-ui-1"],
      proposalsCreated: [],
      blockers: [],
      pendingUserActions: [],
      durableChanges: [],
      nextSteps: ["Keep proposal-first boundaries visible."],
      traceAvailable: true,
    },
    diagnostics: [],
    sequence: 11,
    emittedAt: "2026-06-16T00:00:02.000Z",
    events: [
      {
        eventType: "task.created",
        sequence: 1,
        objectId: "mainchat-task-product-ui-1",
        evidenceId: "evidence-task-product-ui-1",
      },
      {
        eventType: "action.updated",
        sequence: 6,
        objectId: "action-product-ui-1",
        evidenceId: "evidence-action-product-ui-1",
      },
      {
        eventType: "final_delivery.created",
        sequence: 10,
        objectId: "delivery-product-ui-1",
        evidenceId: "evidence-delivery-product-ui-1",
      },
    ],
  };

  return { ...base, ...overrides };
}

describe("ChatPage", () => {
  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    vi.mocked(invoke).mockImplementation(mockInvoke);
    vi.mocked(listen).mockResolvedValue(() => {});
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.clearAllMocks();
  });

  it("renders chat page with session list", async () => {
    render(
      <BrowserRouter>
        <ChatPage />
      </BrowserRouter>
    );

    await waitFor(() => {
      expect(screen.getByText("会话 1")).toBeInTheDocument();
    });

    expect(screen.getByText("会话 2")).toBeInTheDocument();
  });

  it("refreshes chat context immediately when arriving from Builder apply", async () => {
    render(
      <MemoryRouter initialEntries={[{ pathname: "/chat", state: { refreshFromBuilder: true } }]}>
        <ChatPage />
      </MemoryRouter>
    );

    await waitFor(() => {
      expect(screen.getByText("会话 1")).toBeInTheDocument();
    });

    const getLifeModelCalls = vi
      .mocked(invoke)
      .mock.calls.filter(([cmd]) => cmd === "get_life_model");
    const getDiagnosticsCalls = vi
      .mocked(invoke)
      .mock.calls.filter(([cmd]) => cmd === "get_system_diagnostics");
    expect(getLifeModelCalls.length).toBeGreaterThan(1);
    expect(getDiagnosticsCalls.length).toBeGreaterThan(1);
  });

  it("shows quick command guide by default", async () => {
    render(
      <BrowserRouter>
        <ChatPage />
      </BrowserRouter>
    );

    await waitFor(() => {
      expect(screen.getByText(/快捷指令/)).toBeInTheDocument();
    });
  });

  it("allows typing a message", async () => {
    render(
      <BrowserRouter>
        <ChatPage />
      </BrowserRouter>
    );

    await waitFor(() => {
      expect(screen.getByPlaceholderText(/输入消息/)).toBeInTheDocument();
    });
    await screen.findByText("会话 1");
    await screen.findByText("你好！我是 OpenLife。");

    const textarea = screen.getByPlaceholderText(/输入消息/);
    fireEvent.change(textarea, { target: { value: "测试消息" } });
    expect(textarea).toHaveValue("测试消息");
  });

  it("renders readiness bar with local model status", async () => {
    render(
      <BrowserRouter>
        <ChatPage />
      </BrowserRouter>
    );

    expect(await screen.findByText("聊天就绪")).toBeInTheDocument();
    expect(screen.getAllByText(/llama3:latest/).length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText(/云端 API：未配置/)).toBeInTheDocument();
  });

  it("shows companion cockpit with life model pulse", async () => {
    render(
      <BrowserRouter>
        <ChatPage />
      </BrowserRouter>
    );

    expect(await screen.findByText("陪跑现场")).toBeInTheDocument();
    expect(screen.getByText("使命")).toBeInTheDocument();
    expect(screen.getByText("成为更好的自己")).toBeInTheDocument();
    expect(screen.getByText("当前重心")).toBeInTheDocument();
    expect(screen.getByText("工作")).toBeInTheDocument();
    expect(screen.getByText("这轮对话会优先参考")).toBeInTheDocument();
    expect(screen.getByText("价值观过滤")).toBeInTheDocument();
  });

  it("refreshes life model pulse when the window regains focus", async () => {
    let currentFocus = "工作";
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_life_model") {
        return Promise.resolve({
          ...mockLifeModel,
          state: {
            ...mockLifeModel.state,
            current_focus: currentFocus,
          },
        } as any);
      }
      return mockInvoke(cmd, args);
    });

    render(
      <BrowserRouter>
        <ChatPage />
      </BrowserRouter>
    );

    expect(await screen.findByText("陪跑现场")).toBeInTheDocument();
    expect(screen.getByText("工作")).toBeInTheDocument();

    currentFocus = "深度工作";
    fireEvent(window, new Event("focus"));

    await waitFor(() => {
      expect(screen.getByText("深度工作")).toBeInTheDocument();
    });
  });

  const createEmptyModel = (): any => ({
    metadata: { version: "0.1.0", created_at: "", updated_at: "", author: "" },
    identity: {
      name: "",
      values: [],
      personality_traits: [],
      life_philosophy: "",
      mission_statement: "",
      role_definition: {
        primary_role: "",
        secondary_roles: [],
        responsibilities: [],
        boundaries: [],
      },
      voice_style: {
        formality: "neutral",
        tone_descriptors: [],
        vocabulary_preference: "",
        emoji_usage: "sparingly",
      },
    },
    goals: {
      short_term: [],
      medium_term: [],
      long_term: [],
      life_goals: [],
      daily: [],
      progress: 0,
      related_memories: [],
    },
    capabilities: { skills: [], resources: [], networks: [], tools: [], knowledge_domains: [] },
    state: {
      current_focus: "",
      health_status: { physical: "", mental: "", energy_level: 5 },
      emotional_state: { current_mood: "", stress_level: 3, fulfillment_score: 5 },
      recent_reflections: [],
      open_questions: [],
      focus_areas: [],
      recent_events: [],
      habit_streaks: [],
      custom_dimensions: [],
      alerts: [],
    },
    relationships: { inner_circle: [], mentors: [], collaborators: [] },
    preferences: {
      work_hours: { preferred_start: "", preferred_end: "", timezone: "" },
      peak_energy_time: "",
      communication_style: "",
      learning_style: "",
      decision_making_style: "",
    },
    evolution_rules: [],
  });

  it("shows first-use guidance when life model is still empty", async () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_life_model") {
        return createEmptyModel();
      }
      if (cmd === "get_system_diagnostics") {
        const base = (await mockInvoke(cmd, args)) as SystemDiagnostics;
        return {
          ...base,
          model_empty: true,
        };
      }
      return mockInvoke(cmd, args);
    });

    render(
      <BrowserRouter>
        <ChatPage />
      </BrowserRouter>
    );

    expect(await screen.findByText("先建立你的人生模型")).toBeInTheDocument();
    expect(screen.getByText("先看仪表盘")).toBeInTheDocument();
    expect(screen.getByText(/也可以直接使用下面的场景卡开始一次通用对话/)).toBeInTheDocument();
  });

  it("guides the user back to Builder when there is an unfinished builder session", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_life_model") {
        return Promise.resolve(createEmptyModel());
      }
      if (cmd === "get_system_diagnostics") {
        return Promise.resolve({
          router: {
            onnx_available: false,
            onnx_disabled: false,
            active_backend: "regex",
            latency_threshold_us: 50000,
          },
          mcp_server_count: 0,
          mcp_tool_count: 0,
          mcp_recent_audit_count: 0,
          mcp_recent_pii_count: 0,
          memory_chunk_count: 0,
          unfinished_builder_sessions: 1,
          ollama_online: true,
          local_model: "llama3",
          resolved_local_model: "llama3:latest",
          prefer_local_model: false,
          cloud_api_configured: true,
          cloud_provider: "DeepSeek",
          cloud_api_validated: true,
          cloud_api_last_error: null,
          chat_ready: true,
          readiness_issues: [],
          data_dir: "/tmp/openlife-test",
          active_data_dir: "/tmp/openlife-test",
          legacy_data_dir: "/tmp/openlife-legacy",
          database_status: "ok",
          startup_warnings: [],
          snapshot_count: 0,
          life_model_ready: true,
          app_version: "0.1.0",
          model_empty: true,
          chat_session_count: 0,
          onboarding_completed: true,
          beta_ready: false,
          beta_readiness_issues: [],
          builder_completion: {
            identity: 0,
            goals: 0,
            capabilities: 0,
            state: 0,
            overall: 0,
            lowest_dimension: "identity",
          },
        } as any);
      }
      return mockInvoke(cmd, args);
    });

    render(
      <BrowserRouter>
        <ChatPage />
      </BrowserRouter>
    );

    expect(await screen.findByText("先建立你的人生模型")).toBeInTheDocument();
    expect(screen.getByText("回 Builder 继续")).toBeInTheDocument();
  });

  it("fills prompt from companion mode card", async () => {
    render(
      <BrowserRouter>
        <ChatPage />
      </BrowserRouter>
    );

    const textarea = await screen.findByPlaceholderText(/输入消息/);
    await screen.findByText("陪跑现场");
    fireEvent.click(screen.getAllByText("目标拆解")[0]);

    expect((textarea as HTMLTextAreaElement).value).toContain("请帮我拆解一个当前目标");
  });

  it("does not call model stream when chat is not ready but keeps slash commands usable", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_system_diagnostics") {
        return Promise.resolve({
          router: {
            onnx_available: false,
            onnx_disabled: false,
            active_backend: "regex",
            latency_threshold_us: 50000,
          },
          mcp_server_count: 1,
          mcp_tool_count: 2,
          mcp_recent_audit_count: 0,
          mcp_recent_pii_count: 0,
          memory_chunk_count: 0,
          unfinished_builder_sessions: 0,
          ollama_online: false,
          local_model: "llama3",
          resolved_local_model: null,
          prefer_local_model: true,
          cloud_api_configured: false,
          chat_ready: false,
          readiness_issues: ["聊天不可用：未检测到可用 Ollama 本地模型，也没有配置云端 API Key。"],
          data_dir: "/tmp/openlife-test",
          snapshot_count: 0,
          life_model_ready: true,
          app_version: "0.1.0",
          model_empty: false,
          chat_session_count: 0,
          onboarding_completed: true,
          beta_ready: false,
          beta_readiness_issues: [],
        } as any);
      }
      return mockInvoke(cmd, args);
    });

    render(
      <BrowserRouter>
        <ChatPage />
      </BrowserRouter>
    );

    const textarea = await screen.findByPlaceholderText(/输入消息/);
    await screen.findByText("需要配置");

    fireEvent.change(textarea, { target: { value: "普通消息" } });
    fireEvent.keyDown(textarea, { key: "Enter", code: "Enter" });

    await waitFor(() => {
      expect(screen.getByText(/普通对话暂不可用/)).toBeInTheDocument();
    });
    expect(invoke).not.toHaveBeenCalledWith("start_stream_message", expect.anything());

    fireEvent.change(textarea, { target: { value: "/goal" } });
    fireEvent.keyDown(textarea, { key: "Enter", code: "Enter" });

    await waitFor(() => {
      const saveCalls = vi.mocked(invoke).mock.calls.filter(([cmd]) => cmd === "save_chat_message");
      expect(saveCalls.length).toBeGreaterThanOrEqual(2);
    });
  });

  it("shows DeepSeek API key guidance when cloud stream fails", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "start_stream_message") {
        return Promise.reject(new Error("DeepSeek error 401: invalid API Key"));
      }
      return mockInvoke(cmd, args);
    });

    render(
      <BrowserRouter>
        <ChatPage />
      </BrowserRouter>
    );

    const textarea = await screen.findByPlaceholderText(/输入消息/);
    await screen.findByText("聊天就绪");
    fireEvent.change(textarea, { target: { value: "帮我规划今天" } });
    fireEvent.keyDown(textarea, { key: "Enter", code: "Enter" });

    expect(await screen.findByText(/DeepSeek 鉴权失败/)).toBeInTheDocument();
    expect(screen.getByText(/去设置页查看“试用就绪检查”/)).toBeInTheDocument();
  });

  it("does not hide DeepSeek runtime errors behind non-blocking readiness warnings", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_system_diagnostics") {
        return Promise.resolve({
          router: {
            onnx_available: false,
            onnx_disabled: false,
            active_backend: "regex",
            latency_threshold_us: 50000,
          },
          mcp_server_count: 0,
          mcp_tool_count: 0,
          mcp_recent_audit_count: 0,
          mcp_recent_pii_count: 0,
          memory_chunk_count: 0,
          unfinished_builder_sessions: 0,
          ollama_online: false,
          local_model: "llama3",
          resolved_local_model: null,
          prefer_local_model: true,
          cloud_api_configured: true,
          cloud_provider: "DeepSeek",
          cloud_api_validated: true,
          cloud_api_last_error: null,
          chat_ready: true,
          readiness_issues: ["当前设置为优先本地模型，但未找到可用模型：llama3。"],
          data_dir: "/tmp/openlife-test",
          active_data_dir: "/tmp/openlife-test",
          legacy_data_dir: "/tmp/openlife-legacy",
          database_status: "ok",
          startup_warnings: [],
          snapshot_count: 0,
          life_model_ready: true,
          app_version: "0.1.0",
          model_empty: false,
          chat_session_count: 1,
          onboarding_completed: true,
          beta_ready: false,
          beta_readiness_issues: [],
          builder_completion: {
            identity: 80,
            goals: 75,
            capabilities: 70,
            state: 65,
            overall: 72.5,
            lowest_dimension: "state",
          },
        } as any);
      }
      if (cmd === "start_stream_message") {
        return Promise.reject(new Error("DeepSeek error 401: invalid API Key"));
      }
      return mockInvoke(cmd, args);
    });

    render(
      <BrowserRouter>
        <ChatPage />
      </BrowserRouter>
    );

    const textarea = await screen.findByPlaceholderText(/输入消息/);
    await screen.findByText("聊天就绪");
    fireEvent.change(textarea, { target: { value: "测试 DeepSeek" } });
    fireEvent.keyDown(textarea, { key: "Enter", code: "Enter" });

    expect(await screen.findByText(/DeepSeek 鉴权失败/)).toBeInTheDocument();
    expect(screen.queryByText(/暂时无法发送普通对话/)).not.toBeInTheDocument();
  });

  it("lets the streaming command persist normal user messages once", async () => {
    render(
      <BrowserRouter>
        <ChatPage />
      </BrowserRouter>
    );

    const textarea = await screen.findByPlaceholderText(/输入消息/);
    await screen.findByText("聊天就绪");
    fireEvent.change(textarea, { target: { value: "今天怎么安排？" } });
    fireEvent.keyDown(textarea, { key: "Enter", code: "Enter" });

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "start_stream_message",
        expect.objectContaining({
          sessionId: "session-1",
          session_id: "session-1",
          args: expect.objectContaining({ sessionId: "session-1", session_id: "session-1" }),
        })
      );
    });
    const saveCalls = vi.mocked(invoke).mock.calls.filter(([cmd]) => cmd === "save_chat_message");
    expect(saveCalls).toHaveLength(0);
  });

  it("ignores duplicate stream completion events for the same run", async () => {
    type StreamListener = (event: { payload: any }) => void | Promise<void>;
    const listeners = new Map<string, StreamListener>();
    vi.mocked(listen).mockImplementation((event, handler) => {
      listeners.set(event, handler as StreamListener);
      return Promise.resolve(() => {});
    });

    render(
      <BrowserRouter>
        <ChatPage />
      </BrowserRouter>
    );

    const textarea = await screen.findByPlaceholderText(/输入消息/);
    await screen.findByText("聊天就绪");
    fireEvent.change(textarea, { target: { value: "今天星期几" } });
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

    const doneHandler = listeners.get("stream-message-done");
    expect(doneHandler).toBeDefined();
    const donePayload = {
      session_id: "session-1",
      run_id: "run-duplicate-done",
      reply: "今天是星期一。",
      reasoning_trace: null,
      tool_calls: [],
    };

    await act(async () => {
      await doneHandler?.({ payload: donePayload });
      await doneHandler?.({ payload: donePayload });
      await Promise.resolve();
    });

    expect(screen.getAllByText("今天是星期一。")).toHaveLength(1);
  });

  it("renders the productized agent control plane from stream agent_state evidence", async () => {
    type StreamListener = (event: { payload: any }) => void | Promise<void>;
    const listeners = new Map<string, StreamListener>();
    vi.mocked(listen).mockImplementation((event, handler) => {
      listeners.set(event, handler as StreamListener);
      return Promise.resolve(() => {});
    });

    render(
      <BrowserRouter>
        <ChatPage />
      </BrowserRouter>
    );

    const textarea = await screen.findByPlaceholderText(/输入消息/);
    await screen.findByText("聊天就绪");
    fireEvent.change(textarea, { target: { value: "Read the workspace guidance" } });
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

    const doneHandler = listeners.get("stream-message-done");
    expect(doneHandler).toBeDefined();
    await act(async () => {
      await doneHandler?.({
        payload: {
          session_id: "session-1",
          run_id: "run-product-ui-1",
          reply: "I read the workspace guidance and kept writes reviewable.",
          reasoning_trace: null,
          tool_calls: [],
          agent_state: buildMainChatAgentStateSnapshot(),
          execution_transcript: [],
          legacy_fallback_used: false,
        },
      });
      await Promise.resolve();
    });

    expect(await screen.findByText("Agent Control Plane")).toBeInTheDocument();
    expect(screen.getByText("Workspace planning read")).toBeInTheDocument();
    expect(screen.getByText("react_tool_execution")).toBeInTheDocument();
    expect(screen.getByText("AGENTS.md bounded context")).toBeInTheDocument();
    expect(screen.getByText("Read workspace guidance")).toBeInTheDocument();
    expect(screen.getByText("file.read")).toBeInTheDocument();
    expect(screen.getByText("AGENTS.md")).toBeInTheDocument();
    expect(
      screen.getByText("Main Chat Agent v1 stays proposal-first and evidence-backed.")
    ).toBeInTheDocument();
    expect(screen.getByText("Workspace guidance summarized")).toBeInTheDocument();
    expect(
      screen.getByText("Use the bounded workspace guidance without creating durable writes.")
    ).toBeInTheDocument();
  });

  it("renders durable event stream status from backend events and ignores duplicates", async () => {
    type StreamListener = (event: { payload: any }) => void | Promise<void>;
    const listeners = new Map<string, StreamListener>();
    vi.mocked(listen).mockImplementation((event, handler) => {
      listeners.set(event, handler as StreamListener);
      return Promise.resolve(() => {});
    });

    render(
      <BrowserRouter>
        <ChatPage />
      </BrowserRouter>
    );

    const textarea = await screen.findByPlaceholderText(/输入消息/);
    await screen.findByText("聊天就绪");
    fireEvent.change(textarea, { target: { value: "Simple direct answer" } });
    fireEvent.keyDown(textarea, { key: "Enter", code: "Enter" });
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "start_stream_message",
        expect.objectContaining({
          sessionId: "session-1",
          session_id: "session-1",
        })
      );
      expect(listeners.get("stream-message-done")).toBeDefined();
      expect(listeners.get("main-chat-agent-event")).toBeDefined();
    });

    const snapshot = buildMainChatAgentStateSnapshot({
      sequence: 1,
      events: [],
      task: {
        ...buildMainChatAgentStateSnapshot().task,
        taskId: "mainchat-task-event-ui-1",
        runId: "run-event-ui-1",
        title: "Simple direct answer",
        strategy: "direct_answer",
        actionIds: [],
        observationIds: [],
      },
      route: {
        strategy: "direct_answer",
        reason: "ordinary_answer",
        confidence: 0.9,
      },
      actions: [],
      observations: [],
    });
    await act(async () => {
      await listeners.get("stream-message-done")?.({
        payload: {
          session_id: "session-1",
          run_id: "run-event-ui-1",
          reply: "A concise direct answer.",
          reasoning_trace: null,
          tool_calls: [],
          agent_state: snapshot,
          execution_transcript: [],
          legacy_fallback_used: false,
        },
      });
      await Promise.resolve();
    });

    const durableEvent = {
      eventId:
        "mainchat_event:mainchat-task-event-ui-1:1:final_delivery.created:delivery-product-ui-1:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      taskSessionId: "mainchat-task-event-ui-1",
      runId: "run-event-ui-1",
      sequence: 1,
      eventType: "final_delivery.created",
      objectType: "final_delivery",
      objectId: "delivery-product-ui-1",
      createdAt: "2026-06-16T00:00:02.000Z",
      source: "finalizer",
      payloadDigest:
        "bytes:2 hash:sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      payload: { evidenceId: "delivery-product-ui-1" },
      backfilled: false,
    };

    await act(async () => {
      await listeners.get("main-chat-agent-event")?.({ payload: durableEvent });
      await listeners.get("main-chat-agent-event")?.({ payload: durableEvent });
      await Promise.resolve();
    });

    expect(await screen.findByText("Event stream")).toBeInTheDocument();
    expect(screen.getByText("receiving_event")).toBeInTheDocument();
    expect(screen.getByText("final_delivery.created")).toBeInTheDocument();
    expect(screen.getByText("1 event")).toBeInTheDocument();
    expect(
      vi.mocked(invoke).mock.calls.some(([cmd]) => cmd === "list_main_chat_agent_events")
    ).toBe(false);
  });

  it("renders command-backed Phase C plan controls only with a valid plan revision", async () => {
    type StreamListener = (event: { payload: any }) => void | Promise<void>;
    const listeners = new Map<string, StreamListener>();
    vi.mocked(listen).mockImplementation((event, handler) => {
      listeners.set(event, handler as StreamListener);
      return Promise.resolve(() => {});
    });
    const promptSpy = vi
      .spyOn(window, "prompt")
      .mockReturnValueOnce("Review priorities with user edit")
      .mockReturnValueOnce("Unsupported in Phase C");
    const phasePlanState = buildMainChatAgentStateSnapshot({
      task: {
        ...buildMainChatAgentStateSnapshot().task,
        taskId: "mainchat-task-plan-phase-c-ui-1",
        runId: "run-plan-phase-c-ui-1",
        title: "Plan interaction",
        strategy: "plan_execute",
        status: "planning",
        controls: ["confirm_plan", "edit_plan", "execute_step", "skip_step", "cancel_task"],
        actionIds: [],
        observationIds: [],
        finalDeliveryId: undefined,
      },
      route: {
        strategy: "plan_execute",
        reason: "phase_c_plan_interaction",
        confidence: 0.92,
      },
      plan: {
        planId: "plan-phase-c-ui-1",
        planSessionId: "plan-session-phase-c-ui-1",
        taskSessionId: "mainchat-task-plan-phase-c-ui-1",
        runId: "run-plan-phase-c-ui-1",
        status: "draft",
        summary: "Review priorities, then run one read-only step.",
        editable: true,
        source: "plan_execute",
        evidenceId: "plan-evidence-phase-c-ui-1",
        revision: 2,
        revisionId: "rev-2",
        controls: ["confirm_plan", "edit_plan", "execute_step", "skip_step", "cancel_task"],
        steps: [
          {
            stepId: "plan-step-phase-c-ui-1",
            planId: "plan-phase-c-ui-1",
            index: 1,
            title: "Review priorities",
            description: "Read-only planning step.",
            kind: "read",
            status: "draft",
            revision: 2,
            basePlanRevision: 2,
            linkedActionIds: [],
            linkedObservationIds: [],
            linkedProposalIds: [],
            blockerIds: [],
            linkedFinalDeliveryIds: [],
            evidenceIds: ["plan-evidence-phase-c-ui-1"],
            controls: ["edit_plan", "execute_step", "skip_step"],
          },
        ],
      },
      actions: [],
      observations: [],
      finalDelivery: undefined,
    });
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (
        [
          "finalize_plan_execute_session",
          "update_plan_execute_session_draft",
          "execute_plan_execute_step",
          "skip_plan_execute_step",
        ].includes(cmd)
      ) {
        return Promise.resolve({ ok: true } as any);
      }
      if (cmd === "get_main_chat_agent_state_snapshot") {
        return Promise.resolve(phasePlanState as any);
      }
      return mockInvoke(cmd, args);
    });

    render(
      <BrowserRouter>
        <ChatPage />
      </BrowserRouter>
    );

    const textarea = await screen.findByPlaceholderText(/输入消息/);
    await screen.findByText("聊天就绪");
    fireEvent.change(textarea, { target: { value: "Plan this work before executing" } });
    fireEvent.keyDown(textarea, { key: "Enter", code: "Enter" });
    await waitFor(() => {
      expect(listeners.get("stream-message-done")).toBeDefined();
    });

    await act(async () => {
      await listeners.get("stream-message-done")?.({
        payload: {
          session_id: "session-1",
          run_id: "run-plan-phase-c-ui-1",
          reply: "Here is a runtime-backed plan draft.",
          reasoning_trace: null,
          tool_calls: [],
          agent_state: phasePlanState,
          execution_transcript: [],
          legacy_fallback_used: false,
        },
      });
      await Promise.resolve();
    });

    fireEvent.click(await screen.findByRole("button", { name: "Confirm plan" }));
    fireEvent.click(await screen.findByRole("button", { name: "Edit step Review priorities" }));
    fireEvent.click(await screen.findByRole("button", { name: "Execute step Review priorities" }));
    fireEvent.click(await screen.findByRole("button", { name: "Skip step Review priorities" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "finalize_plan_execute_session",
        expect.objectContaining({
          input: { sessionId: "plan-session-phase-c-ui-1", baseRevision: 2 },
        })
      );
      expect(invoke).toHaveBeenCalledWith(
        "update_plan_execute_session_draft",
        expect.objectContaining({
          input: expect.objectContaining({
            sessionId: "plan-session-phase-c-ui-1",
            baseRevision: 2,
            steps: [
              expect.objectContaining({
                stepId: "plan-step-phase-c-ui-1",
                title: "Review priorities with user edit",
              }),
            ],
          }),
        })
      );
      expect(invoke).toHaveBeenCalledWith(
        "execute_plan_execute_step",
        expect.objectContaining({
          input: {
            sessionId: "plan-session-phase-c-ui-1",
            stepId: "plan-step-phase-c-ui-1",
            baseRevision: 2,
          },
        })
      );
      expect(invoke).toHaveBeenCalledWith(
        "skip_plan_execute_step",
        expect.objectContaining({
          input: {
            sessionId: "plan-session-phase-c-ui-1",
            stepId: "plan-step-phase-c-ui-1",
            baseRevision: 2,
            reason: "Unsupported in Phase C",
          },
        })
      );
    });
    promptSpy.mockRestore();
  });

  it("hides Phase C plan controls when the runtime plan revision is missing", async () => {
    type StreamListener = (event: { payload: any }) => void | Promise<void>;
    const listeners = new Map<string, StreamListener>();
    vi.mocked(listen).mockImplementation((event, handler) => {
      listeners.set(event, handler as StreamListener);
      return Promise.resolve(() => {});
    });
    const invalidRevisionState = buildMainChatAgentStateSnapshot({
      task: {
        ...buildMainChatAgentStateSnapshot().task,
        taskId: "mainchat-task-plan-no-revision-ui-1",
        runId: "run-plan-no-revision-ui-1",
        title: "Plan interaction",
        strategy: "plan_execute",
        status: "planning",
        controls: ["confirm_plan", "edit_plan", "execute_step", "skip_step"],
      },
      route: {
        strategy: "plan_execute",
        reason: "phase_c_plan_interaction",
        confidence: 0.92,
      },
      plan: {
        planId: "plan-no-revision-ui-1",
        planSessionId: "plan-session-no-revision-ui-1",
        taskSessionId: "mainchat-task-plan-no-revision-ui-1",
        runId: "run-plan-no-revision-ui-1",
        status: "draft",
        summary: "Plan exists but has no auditable revision.",
        editable: true,
        source: "plan_execute",
        evidenceId: "plan-evidence-no-revision-ui-1",
        controls: ["confirm_plan", "edit_plan", "execute_step", "skip_step"],
        steps: [
          {
            stepId: "plan-step-no-revision-ui-1",
            planId: "plan-no-revision-ui-1",
            index: 1,
            title: "Unversioned step",
            description: "This step must not expose commands.",
            kind: "read",
            status: "draft",
            revision: 1,
            basePlanRevision: 1,
            linkedActionIds: [],
            linkedObservationIds: [],
            linkedProposalIds: [],
            blockerIds: [],
            linkedFinalDeliveryIds: [],
            controls: ["edit_plan", "execute_step", "skip_step"],
          },
        ],
      },
      actions: [],
      observations: [],
      finalDelivery: undefined,
    });
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_main_chat_agent_state_snapshot") {
        return Promise.resolve(invalidRevisionState as any);
      }
      return mockInvoke(cmd, args);
    });

    render(
      <BrowserRouter>
        <ChatPage />
      </BrowserRouter>
    );

    const textarea = await screen.findByPlaceholderText(/输入消息/);
    await screen.findByText("聊天就绪");
    fireEvent.change(textarea, { target: { value: "Plan this work before executing" } });
    fireEvent.keyDown(textarea, { key: "Enter", code: "Enter" });
    await waitFor(() => {
      expect(listeners.get("stream-message-done")).toBeDefined();
    });

    await act(async () => {
      await listeners.get("stream-message-done")?.({
        payload: {
          session_id: "session-1",
          run_id: "run-plan-no-revision-ui-1",
          reply: "Here is an incomplete plan payload.",
          reasoning_trace: null,
          tool_calls: [],
          agent_state: invalidRevisionState,
          execution_transcript: [],
          legacy_fallback_used: false,
        },
      });
      await Promise.resolve();
    });

    await screen.findAllByText("Plan interaction");
    expect(screen.queryByRole("button", { name: "Confirm plan" })).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Edit step Unversioned step" })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Execute step Unversioned step" })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Skip step Unversioned step" })
    ).not.toBeInTheDocument();
  });

  it("keeps Phase C step controls available after plan revision advances", async () => {
    type StreamListener = (event: { payload: any }) => void | Promise<void>;
    const listeners = new Map<string, StreamListener>();
    vi.mocked(listen).mockImplementation((event, handler) => {
      listeners.set(event, handler as StreamListener);
      return Promise.resolve(() => {});
    });
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "execute_plan_execute_step") return Promise.resolve({ ok: true } as any);
      if (cmd === "get_main_chat_agent_state_snapshot") {
        return Promise.resolve(advancedRevisionState as any);
      }
      return mockInvoke(cmd, args);
    });
    const advancedRevisionState = buildMainChatAgentStateSnapshot({
      task: {
        ...buildMainChatAgentStateSnapshot().task,
        taskId: "mainchat-task-plan-advanced-revision-ui-1",
        runId: "run-plan-advanced-revision-ui-1",
        title: "Plan interaction",
        strategy: "plan_execute",
        status: "running",
      },
      route: {
        strategy: "plan_execute",
        reason: "phase_c_plan_interaction",
        confidence: 0.92,
      },
      plan: {
        planId: "plan-advanced-revision-ui-1",
        planSessionId: "plan-session-advanced-revision-ui-1",
        taskSessionId: "mainchat-task-plan-advanced-revision-ui-1",
        runId: "run-plan-advanced-revision-ui-1",
        status: "in_progress",
        summary: "Continue remaining plan steps.",
        editable: false,
        source: "plan_execute",
        evidenceId: "plan-evidence-advanced-revision-ui-1",
        revision: 3,
        revisionId: "rev-3",
        controls: ["execute_step", "skip_step", "cancel_task"],
        steps: [
          {
            stepId: "plan-step-advanced-revision-ui-1",
            planId: "plan-advanced-revision-ui-1",
            index: 2,
            title: "Continue next read step",
            description: "Remaining step created before the last execution revision.",
            kind: "read",
            status: "planned",
            revision: 2,
            basePlanRevision: 2,
            linkedActionIds: [],
            linkedObservationIds: [],
            linkedProposalIds: [],
            blockerIds: [],
            linkedFinalDeliveryIds: [],
            controls: ["execute_step", "skip_step"],
          },
        ],
      },
      actions: [],
      observations: [],
      finalDelivery: undefined,
    });

    render(
      <BrowserRouter>
        <ChatPage />
      </BrowserRouter>
    );

    const textarea = await screen.findByPlaceholderText(/输入消息/);
    await screen.findByText("聊天就绪");
    fireEvent.change(textarea, { target: { value: "Continue this plan" } });
    fireEvent.keyDown(textarea, { key: "Enter", code: "Enter" });
    await waitFor(() => {
      expect(listeners.get("stream-message-done")).toBeDefined();
    });

    await act(async () => {
      await listeners.get("stream-message-done")?.({
        payload: {
          session_id: "session-1",
          run_id: "run-plan-advanced-revision-ui-1",
          reply: "Continue with the next plan step.",
          reasoning_trace: null,
          tool_calls: [],
          agent_state: advancedRevisionState,
          execution_transcript: [],
          legacy_fallback_used: false,
        },
      });
      await Promise.resolve();
    });

    fireEvent.click(await screen.findByRole("button", { name: "Execute step Continue next read step" }));
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "execute_plan_execute_step",
        expect.objectContaining({
          input: {
            sessionId: "plan-session-advanced-revision-ui-1",
            stepId: "plan-step-advanced-revision-ui-1",
            baseRevision: 3,
          },
        })
      );
    });
  });

  it("shows cancelled Phase C plan state and review summary without invalid step controls", async () => {
    type StreamListener = (event: { payload: any }) => void | Promise<void>;
    const listeners = new Map<string, StreamListener>();
    vi.mocked(listen).mockImplementation((event, handler) => {
      listeners.set(event, handler as StreamListener);
      return Promise.resolve(() => {});
    });
    const cancelledPlanState = buildMainChatAgentStateSnapshot({
      task: {
        ...buildMainChatAgentStateSnapshot().task,
        taskId: "mainchat-task-plan-cancelled-ui-1",
        runId: "run-plan-cancelled-ui-1",
        title: "Plan interaction",
        strategy: "plan_execute",
        status: "cancelled",
        controls: [],
        actionIds: [],
        observationIds: [],
        finalDeliveryId: undefined,
      },
      route: {
        strategy: "plan_execute",
        reason: "phase_c_plan_cancelled",
        confidence: 0.92,
      },
      plan: {
        planId: "plan-cancelled-ui-1",
        planSessionId: "plan-session-cancelled-ui-1",
        taskSessionId: "mainchat-task-plan-cancelled-ui-1",
        runId: "run-plan-cancelled-ui-1",
        status: "cancelled",
        summary: "Plan was cancelled after one read-only step.",
        editable: false,
        source: "plan_execute",
        evidenceId: "plan-evidence-cancelled-ui-1",
        revision: 4,
        revisionId: "rev-4",
        reviewId: "plan-review-cancelled-ui-1",
        controls: ["open_trace"],
        reviewSummary: {
          reviewId: "plan-review-cancelled-ui-1",
          planId: "plan-cancelled-ui-1",
          planSessionId: "plan-session-cancelled-ui-1",
          planStatus: "cancelled",
          basePlanRevision: 4,
          completedSteps: [
            {
              stepId: "plan-step-cancelled-ui-1",
              title: "Review priorities",
              status: "executed",
              evidenceIds: ["plan-action-cancelled-ui-1", "plan-observation-cancelled-ui-1"],
              linkedActionIds: ["plan-action-cancelled-ui-1"],
              linkedObservationIds: ["plan-observation-cancelled-ui-1"],
              linkedProposalIds: [],
              blockerIds: [],
            },
          ],
          skippedSteps: [],
          blockedSteps: [],
          proposalsCreated: [],
          observationsUsed: [
            {
              stepId: "plan-step-cancelled-ui-1",
              title: "Review priorities",
              status: "executed",
              evidenceIds: ["plan-observation-cancelled-ui-1"],
              linkedActionIds: [],
              linkedObservationIds: ["plan-observation-cancelled-ui-1"],
              linkedProposalIds: [],
              blockerIds: [],
            },
          ],
          unresolved: [
            {
              stepId: "plan-step-cancelled-ui-2",
              title: "Prepare weekly proposal",
              status: "cancelled",
              evidenceIds: ["plan-step-cancel-cancelled-ui-2"],
              linkedActionIds: [],
              linkedObservationIds: [],
              linkedProposalIds: [],
              blockerIds: [],
            },
          ],
          recommendedNextAction: ["Review cancelled steps before starting a new plan."],
          completionClaimed: false,
        },
        steps: [
          {
            stepId: "plan-step-cancelled-ui-1",
            planId: "plan-cancelled-ui-1",
            index: 1,
            title: "Review priorities",
            description: "Read-only planning step.",
            kind: "read",
            status: "executed",
            revision: 3,
            basePlanRevision: 2,
            linkedActionIds: ["plan-action-cancelled-ui-1"],
            linkedObservationIds: ["plan-observation-cancelled-ui-1"],
            linkedProposalIds: [],
            blockerIds: [],
            linkedFinalDeliveryIds: [],
            evidenceIds: ["plan-action-cancelled-ui-1", "plan-observation-cancelled-ui-1"],
            controls: [],
          },
          {
            stepId: "plan-step-cancelled-ui-2",
            planId: "plan-cancelled-ui-1",
            index: 2,
            title: "Prepare weekly proposal",
            description: "Write-like proposal step.",
            kind: "proposal",
            status: "cancelled",
            revision: 4,
            basePlanRevision: 3,
            linkedActionIds: [],
            linkedObservationIds: [],
            linkedProposalIds: [],
            blockerIds: [],
            linkedFinalDeliveryIds: [],
            evidenceIds: ["plan-step-cancel-cancelled-ui-2"],
            controls: [],
          },
        ],
      },
      actions: [],
      observations: [],
      finalDelivery: undefined,
    });
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_main_chat_agent_state_snapshot") {
        return Promise.resolve(cancelledPlanState as any);
      }
      return mockInvoke(cmd, args);
    });

    render(
      <BrowserRouter>
        <ChatPage />
      </BrowserRouter>
    );

    const textarea = await screen.findByPlaceholderText(/输入消息/);
    await screen.findByText("聊天就绪");
    fireEvent.change(textarea, { target: { value: "Review what happened" } });
    fireEvent.keyDown(textarea, { key: "Enter", code: "Enter" });
    await waitFor(() => {
      expect(listeners.get("stream-message-done")).toBeDefined();
    });

    await act(async () => {
      await listeners.get("stream-message-done")?.({
        payload: {
          session_id: "session-1",
          run_id: "run-plan-cancelled-ui-1",
          reply: "Here is the plan review.",
          reasoning_trace: null,
          tool_calls: [],
          agent_state: cancelledPlanState,
          execution_transcript: [],
          legacy_fallback_used: false,
        },
      });
      await Promise.resolve();
    });

    await screen.findAllByText("cancelled");
    expect(screen.getAllByText("cancelled").length).toBeGreaterThan(0);
    expect(screen.getByText("Review summary")).toBeInTheDocument();
    expect(screen.getByText("Completed")).toBeInTheDocument();
    expect(screen.getByText("Observations used")).toBeInTheDocument();
    expect(screen.getByText("Unresolved")).toBeInTheDocument();
    expect(screen.getByText("Review cancelled steps before starting a new plan.")).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Execute step Prepare weekly proposal" })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Skip step Prepare weekly proposal" })
    ).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Cancel plan" })).not.toBeInTheDocument();
  });

  it("recovers event sequence gaps through replay and then snapshot fallback", async () => {
    type StreamListener = (event: { payload: any }) => void | Promise<void>;
    const listeners = new Map<string, StreamListener>();
    vi.mocked(listen).mockImplementation((event, handler) => {
      listeners.set(event, handler as StreamListener);
      return Promise.resolve(() => {});
    });
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "list_main_chat_agent_events") {
        if (args?.afterSequence === 0 || args?.after_sequence === 0) {
          return Promise.resolve([
            {
              eventId:
                "mainchat_event:mainchat-task-gap-ui-1:1:task.created:mainchat-task-gap-ui-1:d1",
              taskSessionId: "mainchat-task-gap-ui-1",
              runId: "run-gap-ui-1",
              sequence: 1,
              eventType: "task.created",
              objectType: "task",
              objectId: "mainchat-task-gap-ui-1",
              createdAt: "2026-06-16T00:00:01.000Z",
              source: "agent_ingress",
              payloadDigest:
                "bytes:2 hash:sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
              payload: { evidenceId: "mainchat-task-gap-ui-1" },
              backfilled: false,
            },
            {
              eventId: "mainchat_event:mainchat-task-gap-ui-1:2:route.selected:direct_answer:d2",
              taskSessionId: "mainchat-task-gap-ui-1",
              runId: "run-gap-ui-1",
              sequence: 2,
              eventType: "route.selected",
              objectType: "route",
              objectId: "direct_answer",
              createdAt: "2026-06-16T00:00:02.000Z",
              source: "strategy_router",
              payloadDigest:
                "bytes:2 hash:sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
              payload: { evidenceId: "mainchat-task-gap-ui-1" },
              backfilled: false,
            },
            {
              eventId:
                "mainchat_event:mainchat-task-gap-ui-1:3:final_delivery.created:delivery-gap:d3",
              taskSessionId: "mainchat-task-gap-ui-1",
              runId: "run-gap-ui-1",
              sequence: 3,
              eventType: "final_delivery.created",
              objectType: "final_delivery",
              objectId: "delivery-gap",
              createdAt: "2026-06-16T00:00:03.000Z",
              source: "finalizer",
              payloadDigest:
                "bytes:2 hash:sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
              payload: { evidenceId: "delivery-gap" },
              backfilled: false,
            },
          ]);
        }
        return Promise.resolve([]);
      }
      if (cmd === "get_main_chat_agent_state_snapshot") {
        return Promise.resolve(
          buildMainChatAgentStateSnapshot({
            sequence: 5,
            task: {
              ...buildMainChatAgentStateSnapshot().task,
              taskId: "mainchat-task-gap-ui-1",
              runId: "run-gap-ui-1",
              title: "Recovered from snapshot",
            },
          })
        );
      }
      return mockInvoke(cmd, args);
    });

    render(
      <BrowserRouter>
        <ChatPage />
      </BrowserRouter>
    );

    const textarea = await screen.findByPlaceholderText(/输入消息/);
    await screen.findByText("聊天就绪");
    fireEvent.change(textarea, { target: { value: "Replay a missed event" } });
    fireEvent.keyDown(textarea, { key: "Enter", code: "Enter" });
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "start_stream_message",
        expect.objectContaining({
          sessionId: "session-1",
          session_id: "session-1",
        })
      );
      expect(listeners.get("stream-message-done")).toBeDefined();
      expect(listeners.get("main-chat-agent-event")).toBeDefined();
    });
    await act(async () => {
      await listeners.get("stream-message-done")?.({
        payload: {
          session_id: "session-1",
          run_id: "run-gap-ui-1",
          reply: "Done.",
          reasoning_trace: null,
          tool_calls: [],
          agent_state: buildMainChatAgentStateSnapshot({
            sequence: 1,
            events: [],
            task: {
              ...buildMainChatAgentStateSnapshot().task,
              taskId: "mainchat-task-gap-ui-1",
              runId: "run-gap-ui-1",
            },
          }),
          execution_transcript: [],
          legacy_fallback_used: false,
        },
      });
      await Promise.resolve();
    });

    await act(async () => {
      await listeners.get("main-chat-agent-event")?.({
        payload: {
          eventId: "mainchat_event:mainchat-task-gap-ui-1:3:final_delivery.created:delivery-gap:d3",
          taskSessionId: "mainchat-task-gap-ui-1",
          runId: "run-gap-ui-1",
          sequence: 3,
          eventType: "final_delivery.created",
          objectType: "final_delivery",
          objectId: "delivery-gap",
          createdAt: "2026-06-16T00:00:03.000Z",
          source: "finalizer",
          payloadDigest:
            "bytes:2 hash:sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
          payload: { evidenceId: "delivery-gap" },
          backfilled: false,
        },
      });
      await Promise.resolve();
    });

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "list_main_chat_agent_events",
        expect.objectContaining({
          taskSessionId: "mainchat-task-gap-ui-1",
          afterSequence: 0,
        })
      );
    });
    expect(await screen.findByText("stream_recovered")).toBeInTheDocument();
    expect(screen.getByText("3 events")).toBeInTheDocument();

    await act(async () => {
      await listeners.get("main-chat-agent-event")?.({
        payload: {
          eventId: "mainchat_event:mainchat-task-gap-ui-1:5:diagnostic.created:gap:d5",
          taskSessionId: "mainchat-task-gap-ui-1",
          runId: "run-gap-ui-1",
          sequence: 5,
          eventType: "diagnostic.created",
          objectType: "diagnostic",
          objectId: "gap",
          createdAt: "2026-06-16T00:00:05.000Z",
          source: "diagnostic",
          payloadDigest:
            "bytes:2 hash:sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
          payload: { evidenceId: "gap" },
          backfilled: false,
        },
      });
      await Promise.resolve();
    });

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "get_main_chat_agent_state_snapshot",
        expect.objectContaining({
          taskSessionId: "mainchat-task-gap-ui-1",
        })
      );
    });
    expect(await screen.findByText("snapshot_refresh_required")).toBeInTheDocument();
    expect(screen.getByText("sequence 5")).toBeInTheDocument();
    expect(screen.getByText("Recovered from snapshot")).toBeInTheDocument();
  });

  it("approves an exact ToolPermission proposal inline and resumes the Main Chat task", async () => {
    type StreamListener = (event: { payload: any }) => void | Promise<void>;
    const listeners = new Map<string, StreamListener>();
    vi.mocked(listen).mockImplementation((event, handler) => {
      listeners.set(event, handler as StreamListener);
      return Promise.resolve(() => {});
    });
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_main_chat_agent_task_state") {
        return Promise.resolve({
          session: {
            id: args?.taskSessionId ?? "mainchat-task-permission-inline-1",
            chatSessionId: "session-1",
            userGoal: "Read a registered MCP note.",
            selectedStrategy: "react_tool_execution",
            status: "waiting_permission",
            currentPlanSummary: "Wait for ToolPermission, then resume the read.",
            actionQueueIds: ["action-mcp-read-1"],
            pendingBlockers: ["tool_permission_required"],
            contextSnapshotRefs: [],
            createdAt: "2026-06-16T00:00:00.000Z",
            updatedAt: "2026-06-16T00:00:01.000Z",
          },
          actions: [
            {
              id: "action-mcp-read-1",
              sessionId: "mainchat-task-permission-inline-1",
              action: {
                actionType: "mcp.read_only",
                description: "Read registered MCP note",
              },
              policy: {
                level: "medium",
                reasonCode: "tool_permission_required",
                executionAllowed: false,
                requiresConfirmation: true,
                requiresProposal: true,
                requiresBlocker: true,
                silentWriteAllowed: false,
              },
              status: "pending_permission",
              attempts: 1,
              observationMetadata: {
                proposalId: "proposal-tool-permission-inline-1",
                directWritesExecuted: false,
              },
              createdAt: "2026-06-16T00:00:00.000Z",
              updatedAt: "2026-06-16T00:00:01.000Z",
            },
          ],
          transcript: [],
          pendingApprovalCount: 1,
          activeToolCount: 0,
          canResume: true,
          canCancel: true,
          canRetry: false,
        });
      }
      if (cmd === "accept_proposal") {
        return Promise.resolve({ success: true });
      }
      if (cmd === "resume_main_chat_agent_task") {
        return Promise.resolve({
          session: {
            id: args?.taskSessionId ?? "mainchat-task-permission-inline-1",
            chatSessionId: "session-1",
            userGoal: "Read a registered MCP note.",
            selectedStrategy: "react_tool_execution",
            status: "running",
            currentPlanSummary: "ToolPermission accepted; replaying the exact read.",
            actionQueueIds: ["action-mcp-read-1"],
            pendingBlockers: [],
            contextSnapshotRefs: [],
            createdAt: "2026-06-16T00:00:00.000Z",
            updatedAt: "2026-06-16T00:00:02.000Z",
          },
          actions: [],
          transcript: [],
          pendingApprovalCount: 0,
          activeToolCount: 0,
          canResume: false,
          canCancel: true,
          canRetry: false,
        });
      }
      return mockInvoke(cmd, args);
    });

    render(
      <BrowserRouter>
        <ChatPage />
      </BrowserRouter>
    );

    const textarea = await screen.findByPlaceholderText(/输入消息/);
    await screen.findByText("聊天就绪");
    fireEvent.change(textarea, { target: { value: "Read the registered MCP note" } });
    fireEvent.keyDown(textarea, { key: "Enter", code: "Enter" });

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "start_stream_message",
        expect.objectContaining({ sessionId: expect.any(String) })
      );
    });
    const streamCall = vi.mocked(invoke).mock.calls.find(([cmd]) => cmd === "start_stream_message");
    const eventSessionId = (streamCall?.[1] as any)?.sessionId ?? "session-1";
    const permissionState = buildMainChatAgentStateSnapshot({
      task: {
        ...buildMainChatAgentStateSnapshot().task,
        taskId: "mainchat-task-permission-inline-1",
        status: "waiting_for_user",
        controls: ["approve_once", "deny", "defer", "cancel", "open_trace"],
        actionIds: ["action-mcp-read-1"],
        blockerIds: ["blocker-permission-inline-1"],
        proposalIds: ["proposal-tool-permission-inline-1"],
        finalDeliveryId: undefined,
      },
      route: {
        strategy: "permission_request",
        reason: "tool_permission_required",
        confidence: 0.88,
      },
      actions: [
        {
          actionId: "action-mcp-read-1",
          actionType: "mcp.read_only",
          target: "registered_mcp://notes.read",
          label: "Read registered MCP note",
          status: "pending_permission",
          riskLevel: "medium",
          policyDecisionId: "policy-mcp-read-1",
          observationIds: [],
          retryable: false,
        },
      ],
      observations: [],
      blockers: [
        {
          blockerId: "blocker-permission-inline-1",
          reasonCode: "tool_permission_required",
          title: "Permission required",
          detail: "Read registered MCP note",
          affectedActionId: "action-mcp-read-1",
          recoverable: true,
          controls: ["approve_once", "deny", "defer", "cancel", "open_trace"],
        },
      ],
      proposals: [
        {
          proposalId: "proposal-tool-permission-inline-1",
          proposalType: "tool_permission",
          status: "pending_review",
          title: "tool_permission proposal",
          summary: "Allow the pending registered MCP read action once.",
          evidenceIds: ["blocker-permission-inline-1"],
          actionIds: ["action-mcp-read-1"],
          controls: ["accept_proposal", "reject_proposal", "defer", "open_review_center"],
        },
      ],
      finalDelivery: undefined,
      diagnostics: [],
    });
    const doneHandler = listeners.get("stream-message-done");
    expect(doneHandler).toBeDefined();
    await act(async () => {
      await doneHandler?.({
        payload: {
          session_id: eventSessionId,
          run_id: "run-permission-inline-1",
          reply: "I need approval before reading that registered MCP note.",
          reasoning_trace: null,
          tool_calls: [],
          agent_ingress: {
            requestId: "req-permission-inline-1",
            sourceSessionId: "session-1",
            taskKind: "conversation",
            selectedStrategy: "react_tool_execution",
            confidence: 0.88,
            reasonSummary: "Permission required before MCP read.",
            fallbackEligible: true,
            privacyRisk: {
              riskLevel: "medium",
              privacyClass: "workspace",
              policyReasonCode: "tool_permission_required",
              localOnlyRequired: false,
              writeLike: false,
              externalWriteLike: false,
            },
            agentTaskSessionId: "mainchat-task-permission-inline-1",
          },
          agent_state: permissionState,
          execution_transcript: [],
          legacy_fallback_used: false,
        },
      });
      await Promise.resolve();
    });

    fireEvent.click(await screen.findByRole("button", { name: "Approve once" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "accept_proposal",
        expect.objectContaining({
          proposalId: "proposal-tool-permission-inline-1",
          proposal_id: "proposal-tool-permission-inline-1",
        })
      );
      expect(invoke).toHaveBeenCalledWith(
        "resume_main_chat_agent_task",
        expect.objectContaining({
          taskSessionId: "mainchat-task-permission-inline-1",
          task_session_id: "mainchat-task-permission-inline-1",
        })
      );
    });
    expect(vi.mocked(invoke).mock.calls.some(([cmd]) => cmd === "grant_tool_permission")).toBe(
      false
    );
  });

  it("does not infer productized actions or observations from assistant text", async () => {
    type StreamListener = (event: { payload: any }) => void | Promise<void>;
    const listeners = new Map<string, StreamListener>();
    vi.mocked(listen).mockImplementation((event, handler) => {
      listeners.set(event, handler as StreamListener);
      return Promise.resolve(() => {});
    });
    const sparseState = buildMainChatAgentStateSnapshot({
      task: {
        taskId: "mainchat-task-product-ui-2",
        runId: "run-product-ui-2",
        conversationId: "session-1",
        userMessageId: "message-product-ui-2",
        title: "Direct response with no tool evidence",
        strategy: "direct_answer",
        status: "completed",
        createdAt: "2026-06-16T00:01:00.000Z",
        updatedAt: "2026-06-16T00:01:01.000Z",
        traceAvailable: true,
        controls: [],
        actionIds: [],
        observationIds: [],
        blockerIds: [],
        proposalIds: [],
        finalDeliveryId: "delivery-product-ui-2",
      },
      route: {
        strategy: "direct_answer",
        reason: "ordinary_answer",
        confidence: 0.75,
      },
      context: [],
      provider: undefined,
      plan: undefined,
      actions: [],
      observations: [],
      blockers: [],
      proposals: [],
      finalDelivery: {
        deliveryId: "delivery-product-ui-2",
        taskId: "mainchat-task-product-ui-2",
        runId: "run-product-ui-2",
        status: "delivered",
        headline: "Direct answer delivered",
        answer: "This answer has no runtime-backed tool observations.",
        completedActions: [],
        observationsUsed: [],
        proposalsCreated: [],
        blockers: [],
        pendingUserActions: [],
        durableChanges: [],
        nextSteps: [],
        traceAvailable: true,
      },
      diagnostics: [],
      events: [
        {
          eventType: "task.created",
          sequence: 1,
          objectId: "mainchat-task-product-ui-2",
          evidenceId: "evidence-task-product-ui-2",
        },
        {
          eventType: "final_delivery.created",
          sequence: 5,
          objectId: "delivery-product-ui-2",
          evidenceId: "evidence-delivery-product-ui-2",
        },
      ],
    });

    render(
      <BrowserRouter>
        <ChatPage />
      </BrowserRouter>
    );

    const textarea = await screen.findByPlaceholderText(/输入消息/);
    await screen.findByText("聊天就绪");
    fireEvent.change(textarea, { target: { value: "Answer without tools" } });
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

    const doneHandler = listeners.get("stream-message-done");
    expect(doneHandler).toBeDefined();
    await act(async () => {
      await doneHandler?.({
        payload: {
          session_id: "session-1",
          run_id: "run-product-ui-2",
          reply: "I used fake.file.read and saw Ghost observation.",
          reasoning_trace: null,
          tool_calls: [],
          agent_state: sparseState,
          execution_transcript: [],
          legacy_fallback_used: false,
        },
      });
      await Promise.resolve();
    });

    expect(await screen.findByText("Agent Control Plane")).toBeInTheDocument();
    expect(screen.getByText("Direct response with no tool evidence")).toBeInTheDocument();
    expect(screen.getByText("Direct answer delivered")).toBeInTheDocument();
    expect(screen.queryByText("fake.file.read")).not.toBeInTheDocument();
    expect(screen.queryByText("Ghost observation")).not.toBeInTheDocument();
    expect(screen.queryByText("Actions")).not.toBeInTheDocument();
    expect(screen.queryByText("Observations")).not.toBeInTheDocument();
  });

  it("renders main chat agent execution state as a task panel", async () => {
    type StreamListener = (event: { payload: any }) => void | Promise<void>;
    const listeners = new Map<string, StreamListener>();
    vi.mocked(listen).mockImplementation((event, handler) => {
      listeners.set(event, handler as StreamListener);
      return Promise.resolve(() => {});
    });
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_main_chat_agent_task_state") {
        return Promise.resolve({
          session: {
            id: args?.taskSessionId ?? "mainchat-task-ui-1",
            chatSessionId: "session-1",
            userGoal: "Prepare a low energy weekly plan",
            selectedStrategy: "react_tool_execution",
            status: "waiting_permission",
            currentPlanSummary: "Search planning memory, then ask before creating tasks.",
            actionQueueIds: ["action-memory-1", "action-write-1"],
            pendingBlockers: ["resumeBlockedByPendingPermission"],
            contextSnapshotRefs: ["ctx_weekly_digest"],
            createdAt: "2026-06-08T00:00:00.000Z",
            updatedAt: "2026-06-08T00:00:01.000Z",
            finalSummary: undefined,
          },
          actions: [
            {
              id: "action-memory-1",
              sessionId: "mainchat-task-ui-1",
              action: {
                actionType: "memory.search",
                description: "Search accepted planning memory",
              },
              policy: {
                level: "low",
                reasonCode: "read_only_memory",
                executionAllowed: true,
                requiresConfirmation: false,
                requiresProposal: false,
                requiresBlocker: false,
                silentWriteAllowed: false,
              },
              status: "observed",
              attempts: 1,
              observationMetadata: { matchedCount: 2, directWritesExecuted: false },
              createdAt: "2026-06-08T00:00:00.000Z",
              updatedAt: "2026-06-08T00:00:01.000Z",
            },
            {
              id: "action-write-1",
              sessionId: "mainchat-task-ui-1",
              action: {
                actionType: "review_center.propose_scheduled_task",
                description: "Create reviewable weekly task proposal",
              },
              policy: {
                level: "medium",
                reasonCode: "proposal_first_write",
                executionAllowed: false,
                requiresConfirmation: true,
                requiresProposal: true,
                requiresBlocker: true,
                silentWriteAllowed: false,
              },
              status: "failed",
              attempts: 1,
              error: "Proposal store unavailable",
              createdAt: "2026-06-08T00:00:00.000Z",
              updatedAt: "2026-06-08T00:00:01.000Z",
            },
          ],
          transcript: [
            {
              id: "tx-plan-1",
              sessionId: "mainchat-task-ui-1",
              kind: "plan",
              summary: "Use read-only memory before proposing any write.",
              createdAt: "2026-06-08T00:00:00.000Z",
            },
            {
              id: "tx-permission-1",
              sessionId: "mainchat-task-ui-1",
              kind: "permission_request",
              summary: "Waiting for explicit approval before write-like task creation.",
              metadata: { pendingPermissionCount: 1 },
              createdAt: "2026-06-08T00:00:01.000Z",
            },
          ],
          pendingApprovalCount: 1,
          activeToolCount: 1,
          canResume: true,
          canCancel: true,
          canRetry: true,
        });
      }
      return mockInvoke(cmd, args);
    });

    render(
      <BrowserRouter>
        <ChatPage />
      </BrowserRouter>
    );

    const textarea = await screen.findByPlaceholderText(/输入消息/);
    await screen.findByText("聊天就绪");
    fireEvent.change(textarea, { target: { value: "Plan my week with low energy constraints" } });
    fireEvent.keyDown(textarea, { key: "Enter", code: "Enter" });

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "start_stream_message",
        expect.objectContaining({
          sessionId: expect.any(String),
        })
      );
    });
    const streamCall = vi.mocked(invoke).mock.calls.find(([cmd]) => cmd === "start_stream_message");
    const eventSessionId = (streamCall?.[1] as any)?.sessionId ?? "session-1";

    const doneHandler = listeners.get("stream-message-done");
    expect(doneHandler).toBeDefined();
    await act(async () => {
      await doneHandler?.({
        payload: {
          session_id: eventSessionId,
          run_id: "run-mainchat-ui-1",
          reply: "I found planning context and need approval before creating tasks.",
          reasoning_trace: null,
          tool_calls: [],
          agent_ingress: {
            requestId: "req-mainchat-ui-1",
            sourceSessionId: "session-1",
            taskKind: "conversation",
            selectedStrategy: "react_tool_execution",
            confidence: 0.91,
            reasonSummary: "Needs read-only memory and a proposal-first task.",
            fallbackEligible: true,
            privacyRisk: {
              riskLevel: "medium",
              privacyClass: "user_state",
              policyReasonCode: "proposal_first",
              localOnlyRequired: true,
              writeLike: true,
              externalWriteLike: false,
            },
            agentTaskSessionId: "mainchat-task-ui-1",
          },
          execution_transcript: [],
          legacy_fallback_used: true,
        },
      });
      await Promise.resolve();
    });

    expect(await screen.findByText("Execution task")).toBeInTheDocument();
    expect(screen.getByText("Goal")).toBeInTheDocument();
    expect(screen.getByText("Prepare a low energy weekly plan")).toBeInTheDocument();
    expect(screen.getByText("Current plan")).toBeInTheDocument();
    expect(
      screen.getByText("Search planning memory, then ask before creating tasks.")
    ).toBeInTheDocument();
    expect(screen.getByText("Execution queue")).toBeInTheDocument();
    expect(screen.getByText("memory.search")).toBeInTheDocument();
    expect(screen.getByText("Search accepted planning memory")).toBeInTheDocument();
    expect(screen.getByText("matchedCount: 2")).toBeInTheDocument();
    expect(screen.getByText("directWritesExecuted: false")).toBeInTheDocument();
    expect(screen.getByText("review_center.propose_scheduled_task")).toBeInTheDocument();
    expect(screen.getByText("Proposal store unavailable")).toBeInTheDocument();
    expect(screen.getByText("Proposal required")).toBeInTheDocument();
    expect(screen.getByText("Permission required")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Open Review Center" })).toHaveAttribute(
      "href",
      "/review"
    );
    expect(screen.getByText("Pending blockers")).toBeInTheDocument();
    expect(screen.getByText("resumeBlockedByPendingPermission")).toBeInTheDocument();
    expect(screen.getByText(/Fallback notice/)).toBeInTheDocument();
    expect(screen.getByText(/visible legacy fallback/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Resume task" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "Retry failed action" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "Cancel task" })).toBeEnabled();
  });

  it("carries proposal review approval back to the blocked main chat task", async () => {
    type StreamListener = (event: { payload: any }) => void | Promise<void>;
    const listeners = new Map<string, StreamListener>();
    let proposalAccepted = false;
    vi.mocked(listen).mockImplementation((event, handler) => {
      listeners.set(event, handler as StreamListener);
      return Promise.resolve(() => {});
    });
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_main_chat_agent_task_state") {
        return Promise.resolve({
          session: {
            id: "mainchat-task-review-1",
            chatSessionId: "session-1",
            userGoal: "Read a workspace file after permission approval",
            selectedStrategy: "react_tool_execution",
            status: proposalAccepted ? "running" : "waiting_permission",
            currentPlanSummary: "Wait for tool permission, then resume file read.",
            actionQueueIds: ["action-file-1"],
            pendingBlockers: proposalAccepted ? [] : ["tool_permission_required"],
            contextSnapshotRefs: ["ctx_file_permission"],
            createdAt: "2026-06-08T00:00:00.000Z",
            updatedAt: "2026-06-08T00:00:01.000Z",
          },
          actions: [
            {
              id: "action-file-1",
              sessionId: "mainchat-task-review-1",
              action: {
                actionType: "file.read",
                description: "Read the governed workspace file",
              },
              policy: {
                level: "medium",
                reasonCode: "tool_permission_required",
                executionAllowed: false,
                requiresConfirmation: true,
                requiresProposal: true,
                requiresBlocker: true,
                silentWriteAllowed: false,
              },
              status: proposalAccepted ? "planned" : "pending_permission",
              attempts: 1,
              observationMetadata: { directWritesExecuted: false },
              createdAt: "2026-06-08T00:00:00.000Z",
              updatedAt: "2026-06-08T00:00:01.000Z",
            },
          ],
          transcript: [],
          pendingApprovalCount: proposalAccepted ? 0 : 1,
          activeToolCount: 0,
          canResume: true,
          canCancel: true,
          canRetry: false,
        });
      }
      if (cmd === "get_pending_proposals" || cmd === "list_proposals") {
        if (proposalAccepted) return Promise.resolve([]);
        return Promise.resolve([
          {
            id: "proposal-tool-permission-1",
            runId: "run-tool-permission-1",
            proposalType: "tool_permission",
            source: "chat_conversation",
            sourceDetail: "mainchat-task-review-1",
            affectedPath: "tools.permissions.file.read",
            before: "",
            after: {
              tool_name: "file.read",
              source: "builtin",
              permission: "allow_once",
              risk_level: "medium",
            },
            reason: "Main Chat requested permission before reading a workspace file.",
            confidence: 0.86,
            riskLevel: "medium",
            status: "pending",
            createdAt: "2026-06-08T00:00:00.000Z",
            expiresAt: "2026-07-08T00:00:00.000Z",
          },
        ]);
      }
      if (cmd === "accept_proposal") {
        proposalAccepted = true;
        return Promise.resolve({ success: true });
      }
      if (cmd === "resume_main_chat_agent_task") {
        return Promise.resolve({
          session: {
            id: args?.taskSessionId ?? "mainchat-task-review-1",
            chatSessionId: "session-1",
            userGoal: "Read a workspace file after permission approval",
            selectedStrategy: "react_tool_execution",
            status: "running",
            currentPlanSummary: "Permission accepted; resume the file read.",
            actionQueueIds: ["action-file-1"],
            pendingBlockers: [],
            contextSnapshotRefs: ["ctx_file_permission"],
            createdAt: "2026-06-08T00:00:00.000Z",
            updatedAt: "2026-06-08T00:00:02.000Z",
          },
          actions: [],
          transcript: [],
          pendingApprovalCount: 0,
          activeToolCount: 0,
          canResume: false,
          canCancel: true,
          canRetry: false,
        });
      }
      return mockInvoke(cmd, args);
    });

    render(
      <MemoryRouter initialEntries={["/chat"]}>
        <Routes>
          <Route path="/chat" element={<ChatPage />} />
          <Route path="/review" element={<ProposalReviewPage />} />
        </Routes>
      </MemoryRouter>
    );

    const textarea = await screen.findByPlaceholderText(/输入消息/);
    await screen.findByText("聊天就绪");
    fireEvent.change(textarea, { target: { value: "Read AGENTS.md after approval" } });
    fireEvent.keyDown(textarea, { key: "Enter", code: "Enter" });

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "start_stream_message",
        expect.objectContaining({ sessionId: expect.any(String) })
      );
    });
    const streamCall = vi.mocked(invoke).mock.calls.find(([cmd]) => cmd === "start_stream_message");
    const eventSessionId = (streamCall?.[1] as any)?.sessionId ?? "session-1";
    const doneHandler = listeners.get("stream-message-done");
    expect(doneHandler).toBeDefined();
    await act(async () => {
      await doneHandler?.({
        payload: {
          session_id: eventSessionId,
          run_id: "run-review-flow-1",
          reply: "I need permission before reading that file.",
          reasoning_trace: null,
          tool_calls: [],
          agent_ingress: {
            requestId: "req-review-flow-1",
            sourceSessionId: "session-1",
            taskKind: "conversation",
            selectedStrategy: "react_tool_execution",
            confidence: 0.88,
            reasonSummary: "Permission required before file read.",
            fallbackEligible: true,
            privacyRisk: {
              riskLevel: "medium",
              privacyClass: "workspace",
              policyReasonCode: "tool_permission_required",
              localOnlyRequired: false,
              writeLike: false,
              externalWriteLike: false,
            },
            agentTaskSessionId: "mainchat-task-review-1",
          },
          execution_transcript: [],
          legacy_fallback_used: false,
        },
      });
      await Promise.resolve();
    });

    fireEvent.click(await screen.findByRole("link", { name: "Open Review Center" }));

    expect(await screen.findByText("Review Center")).toBeInTheDocument();
    expect(await screen.findByText("tools.permissions.file.read")).toBeInTheDocument();
    fireEvent.click(screen.getByText("应用"));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "accept_proposal",
        expect.objectContaining({
          proposalId: "proposal-tool-permission-1",
          proposal_id: "proposal-tool-permission-1",
        })
      );
    });
    fireEvent.click(await screen.findByRole("button", { name: "Resume Main Chat task" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "resume_main_chat_agent_task",
        expect.objectContaining({
          taskSessionId: "mainchat-task-review-1",
          task_session_id: "mainchat-task-review-1",
        })
      );
    });
    expect(await screen.findByText(/Main Chat task resumed/)).toBeInTheDocument();
  });

  it("keeps Send on the existing chat stream path without calling forbidden governed commands", async () => {
    render(
      <BrowserRouter>
        <ChatPage />
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

    for (const forbiddenCommand of FORBIDDEN_ORDINARY_CHAT_COMMANDS) {
      expect(
        vi.mocked(invoke).mock.calls.some(([cmd]) => cmd === forbiddenCommand),
        `${forbiddenCommand} must not be called by ordinary Send`
      ).toBe(false);
    }
  });

  it("passes an explicitly selected skill id through the ordinary chat stream payload", async () => {
    render(
      <BrowserRouter>
        <ChatPage />
      </BrowserRouter>
    );

    const textarea = await screen.findByPlaceholderText(/输入消息/);
    await screen.findByText("聊天就绪");

    fireEvent.change(screen.getByLabelText("Skill context"), {
      target: { value: "weekly-planning" },
    });
    fireEvent.change(textarea, { target: { value: "按这个技能整理本周计划" } });
    fireEvent.keyDown(textarea, { key: "Enter", code: "Enter" });

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "start_stream_message",
        expect.objectContaining({
          selectedSkillId: "weekly-planning",
          selected_skill_id: "weekly-planning",
          args: expect.objectContaining({
            selectedSkillId: "weekly-planning",
            selected_skill_id: "weekly-planning",
          }),
        })
      );
    });

    for (const forbiddenCommand of FORBIDDEN_ORDINARY_CHAT_COMMANDS) {
      expect(
        vi.mocked(invoke).mock.calls.some(([cmd]) => cmd === forbiddenCommand),
        `${forbiddenCommand} must not be called by ordinary Send`
      ).toBe(false);
    }
  });

  it("shows lightweight trust status and run evidence in companion mode only", async () => {
    const companionRun = {
      id: "run-chat-1",
      taskId: "task-chat-1",
      sessionId: "session-1",
      status: "completed",
      kind: "conversation",
      generatedProposals: ["proposal-1"],
      actions: [],
      observations: [],
      contextSummary: {
        lifeModelEmpty: false,
        includedLifeModelSections: ["goals", "preferences"],
        memoryHitCount: 3,
        memorySources: ["vector"],
        usedToolsPrompt: false,
        redactionApplied: false,
        redactionLevel: "none",
      },
      modelRoute: {
        provider: "Ollama",
        model: "llama3:latest",
        routeType: "local",
        preferLocal: true,
        localModel: "llama3",
        reason: "local preferred",
        privacyLevel: "low",
        retryCount: 0,
      },
      startedAt: "2026-06-08T00:00:00.000Z",
    };
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_chat_history") {
        return Promise.resolve([
          { role: "user", content: "今天怎么安排？" },
          { role: "assistant", content: "先做最小的一步。", run_id: "run-chat-1" },
        ]);
      }
      if (cmd === "list_agent_runs_for_session") {
        return Promise.resolve([companionRun]);
      }
      if (cmd === "get_pending_proposals") {
        return Promise.resolve([
          {
            id: "proposal-1",
            proposalType: "life_model_update",
            source: "chat_conversation",
            affectedPath: "goals.daily",
            after: {},
            reason: "需要确认",
            confidence: 0.8,
            riskLevel: "low",
            status: "pending",
            createdAt: "2026-06-08T00:00:00.000Z",
          },
        ]);
      }
      return mockInvoke(cmd, args);
    });

    render(
      <MemoryRouter initialEntries={["/companion"]}>
        <ChatPage companionMode />
      </MemoryRouter>
    );

    expect(await screen.findByText("本地优先")).toBeInTheDocument();
    expect(screen.getByText("Life Model 已加载")).toBeInTheDocument();
    expect(screen.getByText("有信等你回 1")).toBeInTheDocument();
    expect(await screen.findByText("先做最小的一步。")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "查看依据" }));

    expect(screen.getByText("使用 Life Model：是")).toBeInTheDocument();
    expect(screen.getByText("参考记忆：3 条")).toBeInTheDocument();
    expect(screen.getByText("模型路线：本地 / Ollama / llama3:latest")).toBeInTheDocument();
    expect(screen.getByText("产生待确认：是")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "查看完整记录" })).toHaveAttribute(
      "href",
      "/runs/run-chat-1"
    );
    expect(screen.queryByText("工具调用")).not.toBeInTheDocument();
    expect(screen.queryByText(/ReasoningTrace/i)).not.toBeInTheDocument();
  });

  it("runs governed preview explicitly with write-disabled budget and keeps it out of chat messages", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "run_multi_strategy_agent_preview") {
        return Promise.resolve({
          runId: "run-chat-preview-1",
          strategyKind: "react",
          payloadKind: "react",
          userOutput: "PRIVATE PREVIEW USER OUTPUT",
          proposalIds: [],
          warnings: ["preview runtime forces allowWrites=false"],
          metadataSafeSummary: {
            taskKind: "conversation",
            reasonCode: "default_react",
            riskLevel: "low",
            hasHsPacket: false,
            governanceDecisionKind: "allow",
            rawMemoryContext: "must not render",
          },
          governanceDecisionKind: "allow",
        });
      }
      return mockInvoke(cmd, args);
    });

    function ChatWithLocation() {
      const location = useLocation();
      return (
        <>
          <ChatPage />
          <div data-testid="location-path">{location.pathname}</div>
        </>
      );
    }

    render(
      <MemoryRouter initialEntries={["/chat"]}>
        <ChatWithLocation />
      </MemoryRouter>
    );

    const textarea = await screen.findByPlaceholderText(/输入消息/);
    await screen.findByText("聊天就绪");
    fireEvent.change(textarea, { target: { value: "Preview this guarded chat input" } });
    fireEvent.click(screen.getByRole("button", { name: /Governed Preview/ }));
    fireEvent.click(screen.getByRole("button", { name: "Run Governed Preview" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("run_multi_strategy_agent_preview", {
        input: expect.objectContaining({
          sessionId: expect.stringMatching(/^chat-governed-preview-/),
          userText: "Preview this guarded chat input",
          toolsPrompt: "No developer tools catalog supplied for this chat preview.",
          allowPlanning: false,
          localModelAvailable: false,
          executionBudget: expect.objectContaining({ allowWrites: false }),
        }),
      });
    });

    expect(await screen.findByText("run-chat-preview-1")).toBeInTheDocument();
    expect(screen.getByText("Strategy: react")).toBeInTheDocument();
    expect(screen.getByText("preview runtime forces allowWrites=false")).toBeInTheDocument();
    expect(screen.getByText("reasonCode: default_react")).toBeInTheDocument();
    expect(screen.queryByText("PRIVATE PREVIEW USER OUTPUT")).not.toBeInTheDocument();
    expect(screen.queryByText("rawMemoryContext: must not render")).not.toBeInTheDocument();
    expect(invoke).not.toHaveBeenCalledWith("save_chat_message", expect.anything());
    expect(invoke).not.toHaveBeenCalledWith("start_stream_message", expect.anything());

    fireEvent.click(screen.getByRole("link", { name: "View Run Trace" }));
    expect(screen.getByTestId("location-path")).toHaveTextContent("/runs/run-chat-preview-1");
  });

  const mockSuccessfulControlledPilot = (overrides: Record<string, any> = {}) => {
    const metadataSafeSummary = {
      taskKind: "conversation",
      reasonCode: "default_react",
      riskLevel: "low",
      hasHsPacket: false,
      governanceDecisionKind: "allow",
      ...(overrides.metadataSafeSummary ?? {}),
    };
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "check_controlled_chat_pilot_eligibility") {
        return Promise.resolve({
          eligible: true,
          requiredCleanRuns: 3,
          cleanRunCount: 3,
          checkedRunIds: ["run-preview-clean-3", "run-preview-clean-2", "run-preview-clean-1"],
          blockingReasons: [],
          defaultChatUnchanged: true,
        });
      }
      if (cmd === "run_multi_strategy_agent_preview") {
        return Promise.resolve({
          runId: "run-controlled-pilot-1",
          strategyKind: "react",
          payloadKind: "react",
          userOutput: "Pilot-only answer",
          proposalIds: [],
          warnings: [],
          governanceDecisionKind: "allow",
          ...overrides,
          metadataSafeSummary,
        });
      }
      return mockInvoke(cmd, args);
    });
  };

  const runControlledPilotFromChat = async (draft = "Run one controlled pilot turn") => {
    render(
      <BrowserRouter>
        <ChatPage />
      </BrowserRouter>
    );

    const textarea = await screen.findByPlaceholderText(/输入消息/);
    await screen.findByText("聊天就绪");
    fireEvent.change(textarea, { target: { value: draft } });
    fireEvent.click(screen.getByRole("button", { name: /Governed Preview/ }));
    fireEvent.click(screen.getByRole("button", { name: "Run Controlled Pilot" }));
  };

  it("blocks controlled pilot when eligibility fails and does not call preview", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "check_controlled_chat_pilot_eligibility") {
        return Promise.resolve({
          eligible: false,
          requiredCleanRuns: 3,
          cleanRunCount: 1,
          checkedRunIds: ["run-preview-1"],
          blockingReasons: ["only 1 clean preview run found"],
          defaultChatUnchanged: true,
        });
      }
      if (cmd === "run_multi_strategy_agent_preview") {
        return Promise.reject(new Error("preview must not run when pilot is blocked"));
      }
      return mockInvoke(cmd, args);
    });

    render(
      <BrowserRouter>
        <ChatPage />
      </BrowserRouter>
    );

    const textarea = await screen.findByPlaceholderText(/输入消息/);
    await screen.findByText("聊天就绪");
    fireEvent.change(textarea, { target: { value: "Try controlled pilot" } });
    fireEvent.click(screen.getByRole("button", { name: /Governed Preview/ }));
    fireEvent.click(screen.getByRole("button", { name: "Run Controlled Pilot" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("check_controlled_chat_pilot_eligibility", {
        input: {},
      });
    });

    expect(
      vi.mocked(invoke).mock.calls.some(([cmd]) => cmd === "run_multi_strategy_agent_preview")
    ).toBe(false);
    expect(await screen.findByText(/Controlled Pilot blocked/)).toBeInTheDocument();
    expect(screen.getByText("only 1 clean preview run found")).toBeInTheDocument();
    expect(screen.getByText(/Use normal Send for the stable Chat path/)).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Promote Pilot Response" })
    ).not.toBeInTheDocument();
    expect(invoke).not.toHaveBeenCalledWith(
      "record_controlled_pilot_promotion_evidence",
      expect.anything()
    );
  });

  it("runs controlled pilot after eligibility passes and renders pilot response separately without auto-saving", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "check_controlled_chat_pilot_eligibility") {
        return Promise.resolve({
          eligible: true,
          requiredCleanRuns: 3,
          cleanRunCount: 3,
          checkedRunIds: ["run-preview-clean-3", "run-preview-clean-2", "run-preview-clean-1"],
          blockingReasons: [],
          defaultChatUnchanged: true,
        });
      }
      if (cmd === "run_multi_strategy_agent_preview") {
        return Promise.resolve({
          runId: "run-controlled-pilot-1",
          strategyKind: "react",
          payloadKind: "react",
          userOutput: "Pilot-only answer",
          proposalIds: [],
          warnings: [],
          metadataSafeSummary: {
            taskKind: "conversation",
            reasonCode: "default_react",
            riskLevel: "low",
            hasHsPacket: false,
            governanceDecisionKind: "allow",
          },
          governanceDecisionKind: "allow",
        });
      }
      return mockInvoke(cmd, args);
    });

    render(
      <BrowserRouter>
        <ChatPage />
      </BrowserRouter>
    );

    const textarea = await screen.findByPlaceholderText(/输入消息/);
    await screen.findByText("聊天就绪");
    fireEvent.change(textarea, { target: { value: "Run one controlled pilot turn" } });
    fireEvent.click(screen.getByRole("button", { name: /Governed Preview/ }));
    fireEvent.click(screen.getByRole("button", { name: "Run Controlled Pilot" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("run_multi_strategy_agent_preview", {
        input: expect.objectContaining({
          sessionId: expect.stringMatching(/^chat-controlled-pilot-/),
          userText: "Run one controlled pilot turn",
          toolsPrompt: "No developer tools catalog supplied for this chat preview.",
          allowPlanning: false,
          localModelAvailable: false,
          layer: "L2",
          executionBudget: expect.objectContaining({ allowWrites: false }),
        }),
      });
    });

    const calls = vi.mocked(invoke).mock.calls;
    const eligibilityIndex = calls.findIndex(
      ([cmd]) => cmd === "check_controlled_chat_pilot_eligibility"
    );
    const previewIndex = calls.findIndex(([cmd]) => cmd === "run_multi_strategy_agent_preview");
    expect(eligibilityIndex).toBeGreaterThanOrEqual(0);
    expect(previewIndex).toBeGreaterThan(eligibilityIndex);
    expect(await screen.findByText("Pilot response")).toBeInTheDocument();
    expect(screen.getByText("Pilot-only answer")).toBeInTheDocument();
    expect(screen.getByText("run-controlled-pilot-1")).toBeInTheDocument();
    expect(invoke).not.toHaveBeenCalledWith("save_chat_message", expect.anything());
    expect(invoke).not.toHaveBeenCalledWith("start_stream_message", expect.anything());
  });

  it("shows promote operation only for successful controlled pilot responses with userOutput", async () => {
    mockSuccessfulControlledPilot();

    await runControlledPilotFromChat();

    expect(await screen.findByText("Pilot response")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Promote Pilot Response" })).toBeInTheDocument();
    expect(invoke).not.toHaveBeenCalledWith("save_chat_message", expect.anything());
  });

  it("does not show promote operation when successful controlled pilot omits userOutput", async () => {
    mockSuccessfulControlledPilot({ userOutput: undefined });

    await runControlledPilotFromChat("Pilot returns metadata only");

    expect(await screen.findByText("Pilot response")).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Promote Pilot Response" })
    ).not.toBeInTheDocument();
    expect(invoke).not.toHaveBeenCalledWith("save_chat_message", expect.anything());
    expect(invoke).not.toHaveBeenCalledWith(
      "record_controlled_pilot_promotion_evidence",
      expect.anything()
    );
  });

  it("cancels pilot promotion review without writing chat history", async () => {
    mockSuccessfulControlledPilot();

    await runControlledPilotFromChat();
    fireEvent.click(await screen.findByRole("button", { name: "Promote Pilot Response" }));

    expect(screen.getByText("Review pilot promotion")).toBeInTheDocument();
    expect(screen.getAllByText("Pilot-only answer")).toHaveLength(2);
    expect(screen.getAllByText("run-controlled-pilot-1")).toHaveLength(2);
    expect(screen.getByText("Source session")).toBeInTheDocument();
    expect(screen.getByText("Target session")).toBeInTheDocument();
    expect(screen.getByText("Selected strategy")).toBeInTheDocument();
    expect(screen.getByText("Governance summary")).toBeInTheDocument();
    expect(screen.getByText("Payload summary")).toBeInTheDocument();
    expect(screen.getByText(/确认后将写入当前聊天历史/)).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Cancel Promotion" }));

    expect(screen.queryByText("Review pilot promotion")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Promote Pilot Response" })).toBeInTheDocument();
    expect(invoke).not.toHaveBeenCalledWith("save_chat_message", expect.anything());
  });

  it("confirms pilot promotion by saving one assistant chat message", async () => {
    mockSuccessfulControlledPilot();

    await runControlledPilotFromChat();
    fireEvent.click(await screen.findByRole("button", { name: "Promote Pilot Response" }));
    fireEvent.click(screen.getByRole("button", { name: "Confirm Promotion" }));

    await waitFor(() => {
      const saveCalls = vi.mocked(invoke).mock.calls.filter(([cmd]) => cmd === "save_chat_message");
      expect(saveCalls).toHaveLength(1);
    });
    const saveCalls = vi.mocked(invoke).mock.calls.filter(([cmd]) => cmd === "save_chat_message");
    expect(saveCalls[0][1]).toMatchObject({
      sessionId: "session-1",
      session_id: "session-1",
      message: {
        role: "assistant",
        content: "Pilot-only answer",
        run_id: "run-controlled-pilot-1",
      },
    });
    expect(screen.getAllByText("Pilot-only answer")).toHaveLength(2);

    const evidenceCalls = vi
      .mocked(invoke)
      .mock.calls.filter(([cmd]) => cmd === "record_controlled_pilot_promotion_evidence");
    expect(evidenceCalls).toHaveLength(1);
    expect(evidenceCalls[0][1]).toMatchObject({
      input: {
        pilotRunId: "run-controlled-pilot-1",
        sourceSessionId: "session-1",
        targetSessionId: "session-1",
        strategyKind: "react",
        payloadKind: "react",
        governanceDecisionKind: "allow",
        promotedMessageLength: "Pilot-only answer".length,
        promotedMessageHash: expect.any(String),
        promotedAt: expect.any(String),
      },
    });
    expect(JSON.stringify(evidenceCalls[0][1])).not.toContain("Pilot-only answer");
  });

  it("blocks pilot promotion after switching sessions and asks the user to rerun the pilot", async () => {
    mockSuccessfulControlledPilot();

    await runControlledPilotFromChat();
    fireEvent.click(await screen.findByText("会话 2"));
    fireEvent.click(await screen.findByRole("button", { name: "Promote Pilot Response" }));
    fireEvent.click(screen.getByRole("button", { name: "Confirm Promotion" }));

    expect(await screen.findByText(/Promotion blocked/)).toBeInTheDocument();
    expect(screen.getByText(/source session session-1/)).toBeInTheDocument();
    expect(screen.getByText(/target session session-2/)).toBeInTheDocument();
    expect(screen.getAllByText(/Rerun Controlled Pilot in this session/).length).toBeGreaterThan(0);
    const saveCalls = vi.mocked(invoke).mock.calls.filter(([cmd]) => cmd === "save_chat_message");
    expect(saveCalls).toHaveLength(0);
    const evidenceCalls = vi
      .mocked(invoke)
      .mock.calls.filter(([cmd]) => cmd === "record_controlled_pilot_promotion_evidence");
    expect(evidenceCalls).toHaveLength(0);
  });

  it("does not allow repeating promotion for the same pilot response", async () => {
    mockSuccessfulControlledPilot();

    await runControlledPilotFromChat();
    fireEvent.click(await screen.findByRole("button", { name: "Promote Pilot Response" }));
    fireEvent.click(screen.getByRole("button", { name: "Confirm Promotion" }));

    await waitFor(() => {
      const saveCalls = vi.mocked(invoke).mock.calls.filter(([cmd]) => cmd === "save_chat_message");
      expect(saveCalls).toHaveLength(1);
    });
    expect(screen.getByText("Promoted to chat history")).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Promote Pilot Response" })
    ).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Confirm Promotion" })).not.toBeInTheDocument();
    const saveCalls = vi.mocked(invoke).mock.calls.filter(([cmd]) => cmd === "save_chat_message");
    expect(saveCalls).toHaveLength(1);
    const evidenceCalls = vi
      .mocked(invoke)
      .mock.calls.filter(([cmd]) => cmd === "record_controlled_pilot_promotion_evidence");
    expect(evidenceCalls).toHaveLength(1);
  });

  it("does not duplicate the promoted assistant message when evidence recording is retried", async () => {
    let evidenceAttempts = 0;
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "check_controlled_chat_pilot_eligibility") {
        return Promise.resolve({
          eligible: true,
          requiredCleanRuns: 3,
          cleanRunCount: 3,
          checkedRunIds: ["run-preview-clean-3", "run-preview-clean-2", "run-preview-clean-1"],
          blockingReasons: [],
          defaultChatUnchanged: true,
        });
      }
      if (cmd === "run_multi_strategy_agent_preview") {
        return Promise.resolve({
          runId: "run-controlled-pilot-1",
          strategyKind: "react",
          payloadKind: "react",
          userOutput: "Pilot-only answer",
          proposalIds: [],
          warnings: [],
          metadataSafeSummary: {
            taskKind: "conversation",
            reasonCode: "default_react",
            riskLevel: "low",
            hasHsPacket: false,
            governanceDecisionKind: "allow",
          },
          governanceDecisionKind: "allow",
        });
      }
      if (cmd === "record_controlled_pilot_promotion_evidence") {
        evidenceAttempts += 1;
        if (evidenceAttempts === 1) {
          return Promise.reject(new Error("evidence db unavailable"));
        }
        return Promise.resolve({
          evidenceId: "ev_promotion_1",
          created: true,
          pilotRunId: "run-controlled-pilot-1",
          promotedAt: "2026-05-30T00:00:00Z",
        });
      }
      return mockInvoke(cmd, args);
    });

    await runControlledPilotFromChat();
    fireEvent.click(await screen.findByRole("button", { name: "Promote Pilot Response" }));
    fireEvent.click(screen.getByRole("button", { name: "Confirm Promotion" }));

    expect(await screen.findByText(/Promotion evidence recording failed/)).toBeInTheDocument();
    expect(screen.getByText(/Retry will only record evidence/)).toBeInTheDocument();
    let saveCalls = vi.mocked(invoke).mock.calls.filter(([cmd]) => cmd === "save_chat_message");
    expect(saveCalls).toHaveLength(1);

    fireEvent.click(screen.getByRole("button", { name: "Confirm Promotion" }));

    await waitFor(() => {
      const evidenceCalls = vi
        .mocked(invoke)
        .mock.calls.filter(([cmd]) => cmd === "record_controlled_pilot_promotion_evidence");
      expect(evidenceCalls).toHaveLength(2);
    });
    saveCalls = vi.mocked(invoke).mock.calls.filter(([cmd]) => cmd === "save_chat_message");
    expect(saveCalls).toHaveLength(1);
    expect(await screen.findByText("Promoted to chat history")).toBeInTheDocument();
  });

  it("shows controlled pilot fallback when preview fails without writing chat history", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "check_controlled_chat_pilot_eligibility") {
        return Promise.resolve({
          eligible: true,
          requiredCleanRuns: 3,
          cleanRunCount: 3,
          checkedRunIds: ["run-preview-clean-3", "run-preview-clean-2", "run-preview-clean-1"],
          blockingReasons: [],
          defaultChatUnchanged: true,
        });
      }
      if (cmd === "run_multi_strategy_agent_preview") {
        return Promise.reject(new Error("preview runtime unavailable"));
      }
      return mockInvoke(cmd, args);
    });

    render(
      <BrowserRouter>
        <ChatPage />
      </BrowserRouter>
    );

    const textarea = await screen.findByPlaceholderText(/输入消息/);
    await screen.findByText("聊天就绪");
    fireEvent.change(textarea, { target: { value: "Pilot failure path" } });
    fireEvent.click(screen.getByRole("button", { name: /Governed Preview/ }));
    fireEvent.click(screen.getByRole("button", { name: "Run Controlled Pilot" }));

    expect(await screen.findByText(/Controlled Pilot failed/)).toBeInTheDocument();
    expect(screen.getByText(/preview runtime unavailable/)).toBeInTheDocument();
    expect(screen.getByText(/Use normal Send for the stable Chat path/)).toBeInTheDocument();
    expect(invoke).not.toHaveBeenCalledWith("save_chat_message", expect.anything());
    expect(invoke).not.toHaveBeenCalledWith("start_stream_message", expect.anything());
    expect(
      screen.queryByRole("button", { name: "Promote Pilot Response" })
    ).not.toBeInTheDocument();
    expect(invoke).not.toHaveBeenCalledWith(
      "record_controlled_pilot_promotion_evidence",
      expect.anything()
    );
  });

  it("ignores a delayed AgentRun fetch after switching away from the originating session", async () => {
    type StreamListener = (event: { payload: any }) => void | Promise<void>;
    const listeners = new Map<string, StreamListener>();
    vi.mocked(listen).mockImplementation((event, handler) => {
      listeners.set(event, handler as StreamListener);
      return Promise.resolve(() => {});
    });

    let resolveRun: ((value: any) => void) | null = null;
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_agent_run") {
        return new Promise(resolve => {
          resolveRun = resolve;
        }) as Promise<any>;
      }
      return mockInvoke(cmd, args);
    });

    render(
      <BrowserRouter>
        <ChatPage />
      </BrowserRouter>
    );

    await screen.findByText("会话 1");
    const startHandler = listeners.get("stream-message-start");
    expect(startHandler).toBeDefined();
    await act(async () => {
      void startHandler?.({
        payload: {
          session_id: "session-1",
          run_id: "run-old-session",
          reasoning_trace: null,
          tool_calls: [],
        },
      });
      await Promise.resolve();
    });

    await act(async () => {
      fireEvent.click(screen.getByText("会话 2"));
      await Promise.resolve();
    });
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "get_chat_history",
        expect.objectContaining({
          sessionId: "session-2",
        })
      );
    });

    await act(async () => {
      resolveRun?.({
        id: "run-old-session",
        sessionId: "session-1",
        userMessageId: "user-old",
        status: "completed",
        startedAt: new Date().toISOString(),
        completedAt: new Date().toISOString(),
        modelRoute: { provider: "DeepSeek", model: "deepseek-chat", reason: "old session" },
      });
      await Promise.resolve();
    });

    await waitFor(() => {
      expect(
        screen.queryByText(/Run completed · DeepSeek · deepseek-chat/)
      ).not.toBeInTheDocument();
    });
  });

  it("persists slash command messages to chat history", async () => {
    render(
      <BrowserRouter>
        <ChatPage />
      </BrowserRouter>
    );

    const textarea = await screen.findByPlaceholderText(/输入消息/);
    fireEvent.change(textarea, { target: { value: "/goal" } });
    fireEvent.keyDown(textarea, { key: "Enter", code: "Enter" });

    await waitFor(() => {
      const saveCalls = vi.mocked(invoke).mock.calls.filter(([cmd]) => cmd === "save_chat_message");
      expect(saveCalls).toHaveLength(2);
    });

    const saveCalls = vi.mocked(invoke).mock.calls.filter(([cmd]) => cmd === "save_chat_message");
    expect(saveCalls[0][1]).toMatchObject({
      sessionId: "session-1",
      session_id: "session-1",
      message: { role: "user", content: "/goal" },
    });
    expect(saveCalls[1][1]).toMatchObject({
      sessionId: "session-1",
      session_id: "session-1",
      message: { role: "assistant" },
    });
  });

  it("supports adding a daily goal from slash command", async () => {
    render(
      <BrowserRouter>
        <ChatPage />
      </BrowserRouter>
    );

    const textarea = await screen.findByPlaceholderText(/输入消息/);
    fireEvent.change(textarea, { target: { value: "/goal add 阅读30分钟" } });
    fireEvent.keyDown(textarea, { key: "Enter", code: "Enter" });

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("add_daily_goal", { name: "阅读30分钟" });
    });
    expect(await screen.findByText(/已添加今日目标：阅读30分钟/)).toBeInTheDocument();
  });

  it("supports completing a daily goal from slash command", async () => {
    render(
      <BrowserRouter>
        <ChatPage />
      </BrowserRouter>
    );

    const textarea = await screen.findByPlaceholderText(/输入消息/);
    fireEvent.change(textarea, { target: { value: "/goal done 早起" } });
    fireEvent.keyDown(textarea, { key: "Enter", code: "Enter" });

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("toggle_daily_goal", { index: 0 });
    });
    expect(await screen.findByText(/已完成今日目标：早起/)).toBeInTheDocument();
  });

  it("shows safe mode warning and blocks add-to-memory action when diagnostics are degraded", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_system_diagnostics") {
        return Promise.resolve({
          router: {
            onnx_available: false,
            onnx_disabled: false,
            active_backend: "regex",
            latency_threshold_us: 50000,
          },
          mcp_server_count: 1,
          mcp_tool_count: 2,
          mcp_recent_audit_count: 1,
          mcp_recent_pii_count: 0,
          memory_chunk_count: 42,
          vector_corrupt_embedding_count: 2,
          unfinished_builder_sessions: 0,
          ollama_online: true,
          local_model: "llama3",
          resolved_local_model: "llama3:latest",
          prefer_local_model: false,
          cloud_api_configured: true,
          cloud_provider: "DeepSeek",
          cloud_api_validated: true,
          cloud_api_last_error: null,
          chat_ready: true,
          readiness_issues: [],
          data_dir: "/tmp/openlife-test",
          active_data_dir: "/tmp/openlife-test",
          legacy_data_dir: "/tmp/openlife-legacy",
          database_status: "degraded",
          startup_warnings: ["memory.db 初始化失败，正在使用临时数据库"],
          snapshot_count: 1,
          life_model_ready: true,
          app_version: "0.1.0",
          model_empty: false,
          chat_session_count: 1,
          onboarding_completed: true,
          beta_ready: false,
          beta_readiness_issues: [],
          builder_completion: {
            identity: 80,
            goals: 75,
            capabilities: 70,
            state: 65,
            overall: 72.5,
            lowest_dimension: "state",
          },
          data_files: {
            messages_db_exists: true,
            messages_db_size_mb: 0.1,
            vectors_db_exists: true,
            vectors_db_size_mb: 0.1,
            mcp_audit_db_exists: false,
            mcp_audit_db_size_mb: 0,
            config_yaml_exists: true,
            life_model_yaml_exists: true,
          },
          ollama_models: [],
          config_source: "default",
        });
      }
      return mockInvoke(cmd, args);
    });

    render(
      <BrowserRouter>
        <ChatPage />
      </BrowserRouter>
    );

    expect(await screen.findByText(/Safe Mode：/)).toBeInTheDocument();
    fireEvent.click((await screen.findAllByText("加入记忆"))[0]);
    expect(await screen.findByText(/建议先去设置页恢复控制台处理数据风险/)).toBeInTheDocument();
    expect(invoke).not.toHaveBeenCalledWith("index_memory_chunk", expect.anything());
  });
});
