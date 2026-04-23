import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import SettingsPage from "./SettingsPage";
import { invoke } from "@tauri-apps/api/core";
import { mockInvoke } from "@/test/mocks/tauri";
import { save } from "@tauri-apps/plugin-dialog";
import { writeTextFile } from "@tauri-apps/plugin-fs";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  save: vi.fn(),
  open: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-fs", () => ({
  writeTextFile: vi.fn(),
  readTextFile: vi.fn(),
}));

describe("SettingsPage", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockImplementation(mockInvoke);
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  const renderSettings = () =>
    render(
      <MemoryRouter>
        <SettingsPage />
      </MemoryRouter>
    );

  it("renders trial console title and checklist", async () => {
    renderSettings();

    await waitFor(() => {
      expect(screen.getByText("试用控制台")).toBeInTheDocument();
    });

    expect(screen.getByText(/试用路径 Checklist/)).toBeInTheDocument();
    expect(screen.getByText(/核心链路已就绪/)).toBeInTheDocument();
    expect(screen.getByText(/导出全部数据/)).toBeInTheDocument();
  });

  it("shows readiness issues when diagnostics reports chat is not ready", async () => {
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
          mcp_recent_pii_count: 1,
          memory_chunk_count: 42,
          unfinished_builder_sessions: 0,
          ollama_online: false,
          local_model: "llama3",
          resolved_local_model: null,
          prefer_local_model: true,
          cloud_api_configured: false,
          cloud_provider: "DeepSeek",
          cloud_api_validated: false,
          cloud_api_last_error: null,
          chat_ready: false,
          readiness_issues: ["聊天不可用：未检测到可用 Ollama 本地模型，也没有配置云端 API Key。"],
          data_dir: "/tmp/openlife-test",
          active_data_dir: "/tmp/openlife-test",
          legacy_data_dir: "/tmp/openlife-legacy",
          database_status: "ok",
          startup_warnings: [],
          snapshot_count: 0,
          life_model_ready: true,
          app_version: "0.1.0",
          model_empty: false,
          chat_session_count: 0,
          onboarding_completed: true,
          beta_ready: false,
          beta_readiness_issues: ["核心聊天链路未就绪，请先修复试用就绪检查中的问题。"],
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

    renderSettings();

    expect(await screen.findByText(/试用路径 Checklist/)).toBeInTheDocument();
    expect(screen.getByText(/聊天不可用/)).toBeInTheDocument();
  });

  it("exports data with optional app version metadata", async () => {
    vi.mocked(save).mockResolvedValue("/tmp/openlife-export.json");
    vi.mocked(writeTextFile).mockResolvedValue(undefined);

    renderSettings();

    const exportButton = await screen.findByText("导出全部数据");
    fireEvent.click(exportButton);

    await waitFor(() => {
      expect(writeTextFile).toHaveBeenCalled();
    });
    expect(await screen.findByText(/应用版本 0.1.0/)).toBeInTheDocument();
  });

  it("tests the current DeepSeek form config instead of only saved config", async () => {
    renderSettings();

    const keyInput = await screen.findByPlaceholderText("sk-...");
    fireEvent.change(keyInput, { target: { value: "sk-deepseek-form" } });
    fireEvent.click(screen.getByRole("button", { name: "测试连接" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("test_llm_connection", {
        config: expect.objectContaining({
          llm: expect.objectContaining({
            provider: "deepseek",
            openai_base: "https://api.deepseek.com",
            openai_key: "sk-deepseek-form",
            chat_model: "deepseek-chat",
            embedding_enabled: false,
          }),
        }),
      });
    });
    expect(await screen.findByText(/DeepSeek: 连接成功/)).toBeInTheDocument();
  });
});
