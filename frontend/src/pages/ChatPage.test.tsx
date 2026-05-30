import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, fireEvent, act } from "@testing-library/react";
import { BrowserRouter, MemoryRouter, useLocation } from "react-router-dom";
import ChatPage from "./ChatPage";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { mockInvoke, mockLifeModel } from "@/test/mocks/tauri";
import type { SystemDiagnostics } from "../tauri";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

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

  it("keeps Send on the existing chat stream path without calling governed preview", async () => {
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

    expect(
      vi.mocked(invoke).mock.calls.some(([cmd]) => cmd === "run_multi_strategy_agent_preview")
    ).toBe(false);
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
