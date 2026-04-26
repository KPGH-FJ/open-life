import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import { BrowserRouter, MemoryRouter } from "react-router-dom";
import DashboardPage from "./DashboardPage";
import { invoke } from "@tauri-apps/api/core";
import { mockInvoke } from "@/test/mocks/tauri";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("DashboardPage", () => {
  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    vi.mocked(invoke).mockImplementation(mockInvoke);
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.clearAllMocks();
  });

  it("renders dashboard with correct title", async () => {
    render(
      <BrowserRouter>
        <DashboardPage />
      </BrowserRouter>
    );

    await waitFor(() => {
      expect(screen.getByText("仪表盘")).toBeInTheDocument();
    });
  });

  it("refreshes dashboard context immediately when arriving from Builder apply", async () => {
    render(
      <MemoryRouter
        initialEntries={[{ pathname: "/dashboard", state: { refreshFromBuilder: true } }]}
      >
        <DashboardPage />
      </MemoryRouter>
    );

    await waitFor(() => {
      expect(screen.getByText("仪表盘")).toBeInTheDocument();
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

  it("displays daily goals from tauri", async () => {
    render(
      <BrowserRouter>
        <DashboardPage />
      </BrowserRouter>
    );

    await waitFor(() => {
      expect(screen.getByText("今日目标")).toBeInTheDocument();
    });

    expect(screen.getByText("早起")).toBeInTheDocument();
    expect(screen.getByText("运动")).toBeInTheDocument();
  });

  it("displays gap analysis results", async () => {
    render(
      <BrowserRouter>
        <DashboardPage />
      </BrowserRouter>
    );

    await waitFor(() => {
      expect(screen.getByText("完成 AI 项目")).toBeInTheDocument();
    });
    expect(screen.getByText(/关键能力：编程/)).toBeInTheDocument();
    expect(screen.getByText("安排 2 周刻意练习，并补一个可验证里程碑")).toBeInTheDocument();
  });

  it("shows version info when snapshot exists", async () => {
    render(
      <BrowserRouter>
        <DashboardPage />
      </BrowserRouter>
    );

    await waitFor(() => {
      expect(screen.getAllByText(content => content.includes("0.1.0")).length).toBeGreaterThan(0);
    });
  });

  it("displays skill stats from life model", async () => {
    render(
      <BrowserRouter>
        <DashboardPage />
      </BrowserRouter>
    );

    await waitFor(() => {
      expect(screen.getByText("技能")).toBeInTheDocument();
    });
    const skillCard = screen.getByText("技能").parentElement;
    expect(skillCard).toHaveTextContent("2");
  });

  it("shows memory count stats", async () => {
    render(
      <BrowserRouter>
        <DashboardPage />
      </BrowserRouter>
    );

    await waitFor(() => {
      expect(screen.getByText("记忆")).toBeInTheDocument();
    });
    expect(screen.getByText("42")).toBeInTheDocument();
  });

  it("shows state trend explanation for selected dimension", async () => {
    render(
      <BrowserRouter>
        <DashboardPage />
      </BrowserRouter>
    );

    await waitFor(() => {
      expect(screen.getByText("趋势解释")).toBeInTheDocument();
    });

    expect(screen.getByText(/专注度最近有下降趋势/)).toBeInTheDocument();
    expect(screen.getByText(/预警原因：专注度低于阈值/)).toBeInTheDocument();
    expect(screen.getByText("最近备注")).toBeInTheDocument();
  });

  it("shows safe mode recovery prompt when diagnostics are degraded", async () => {
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
        <DashboardPage />
      </BrowserRouter>
    );

    expect(await screen.findByText(/Safe Mode：建议先修复数据环境再深度试用/)).toBeInTheDocument();
    expect(screen.getByText("去恢复控制台")).toBeInTheDocument();
  });

  it("shows recommended trial route to guide the product path", async () => {
    render(
      <BrowserRouter>
        <DashboardPage />
      </BrowserRouter>
    );

    expect(await screen.findByText("推荐试用路线")).toBeInTheDocument();
    expect(screen.getByText("开始一次个性化对话")).toBeInTheDocument();
  });

  it("prioritizes resuming unfinished builder review when model is still empty", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_life_model") {
        return Promise.resolve({
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
          capabilities: {
            skills: [],
            resources: [],
            networks: [],
            tools: [],
            knowledge_domains: [],
          },
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
            work_hours: {
              preferred_start: "09:00",
              preferred_end: "18:00",
              timezone: "Asia/Shanghai",
            },
            peak_energy_time: "",
            communication_style: "",
            learning_style: "",
            decision_making_style: "",
          },
          evolution_rules: [],
        } as any);
      }
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
          vector_corrupt_embedding_count: 0,
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
          snapshot_count: 1,
          life_model_ready: true,
          app_version: "0.1.0",
          model_empty: true,
          chat_session_count: 1,
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
        <DashboardPage />
      </BrowserRouter>
    );

    expect(await screen.findByText("继续 Builder 中待确认的 Review")).toBeInTheDocument();
    expect(screen.getByText(/先回 Builder 应用它们，比重新开始更合适/)).toBeInTheDocument();
  });

  it("shows the rationale behind today action recommendations", async () => {
    render(
      <BrowserRouter>
        <DashboardPage />
      </BrowserRouter>
    );

    expect(await screen.findByText("为什么今天先做这个")).toBeInTheDocument();
    expect(screen.getByText(/这些不是随机建议/)).toBeInTheDocument();
  });

  it("refreshes diagnostics when the window regains focus", async () => {
    let degraded = false;
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
          vector_corrupt_embedding_count: degraded ? 2 : 0,
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
          database_status: degraded ? "degraded" : "ok",
          startup_warnings: degraded ? ["memory.db 初始化失败，正在使用临时数据库"] : [],
          snapshot_count: 1,
          life_model_ready: true,
          app_version: "0.1.0",
          model_empty: false,
          chat_session_count: 1,
          onboarding_completed: true,
          beta_ready: !degraded,
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
        <DashboardPage />
      </BrowserRouter>
    );

    expect(await screen.findByText("推荐试用路线")).toBeInTheDocument();
    expect(screen.queryByText(/Safe Mode：建议先修复数据环境再深度试用/)).not.toBeInTheDocument();

    degraded = true;
    fireEvent(window, new Event("focus"));

    await waitFor(() => {
      expect(screen.getByText(/Safe Mode：建议先修复数据环境再深度试用/)).toBeInTheDocument();
    });
  });
});
