import { describe, it, expect, vi, beforeEach } from "vitest";
import { fireEvent, render, screen, act } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import ProviderTab from "./ProviderTab";
import { mockInvoke } from "@/test/mocks/tauri";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("ProviderTab", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockImplementation(mockInvoke);
  });

  const mockConfig = {
    llm: {
      provider: "deepseek",
      openai_base: "https://api.openai.com/v1",
      openai_key: "",
      embedding_model: "text-embedding-3-small",
      chat_model: "deepseek-chat",
    },
    prefer_local_model: false,
    local_model: "llama3",
    system: {
      network_policy: { enabled: true, allow_cloud: true, denylist: [] as string[] },
    },
  } as any;

  const mockDiagnostics = {
    cloud_api_configured: true,
    cloud_provider: "DeepSeek",
    cloud_api_validated: false,
    cloud_api_validation_status: "unvalidated",
    ollama_online: true,
    local_model: "llama3",
    resolved_local_model: "llama3",
  } as any;

  it("renders local, auto, and cloud model route choices", async () => {
    await act(async () => {
      render(
        <MemoryRouter>
          <ProviderTab
            config={mockConfig}
            setConfig={vi.fn()}
            diagnostics={mockDiagnostics}
            routerStatus={null}
            modelRouterStatus={null}
          />
        </MemoryRouter>
      );
    });
    expect(screen.getByRole("button", { name: /Local only/ })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Auto/ })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Cloud/ })).toBeInTheDocument();
    expect(screen.getByText(/LocalOnly 时不会调用云端/)).toBeInTheDocument();
    expect(screen.getByText("unvalidated")).toBeInTheDocument();
    expect(screen.getByText(/尚未验证/)).toBeInTheDocument();
  });

  it("shows validated, failed, and stale provider status distinctly", async () => {
    let view!: ReturnType<typeof render>;
    await act(async () => {
      view = render(
        <MemoryRouter>
          <ProviderTab
            config={mockConfig}
            setConfig={vi.fn()}
            diagnostics={
              {
                ...mockDiagnostics,
                cloud_api_validated: true,
                cloud_api_validation_status: "validated",
                cloud_api_validated_at: "2026-06-27T00:00:00Z",
              } as any
            }
            routerStatus={null}
            modelRouterStatus={null}
          />
        </MemoryRouter>
      );
    });

    expect(screen.getByText("validated")).toBeInTheDocument();
    expect(screen.getByText(/Provider 已验证/)).toBeInTheDocument();

    view.rerender(
      <MemoryRouter>
        <ProviderTab
          config={mockConfig}
          setConfig={vi.fn()}
          diagnostics={
            {
              ...mockDiagnostics,
              cloud_api_validated: false,
              cloud_api_validation_status: "failed",
              cloud_api_last_error: "http_status:401",
            } as any
          }
          routerStatus={null}
          modelRouterStatus={null}
        />
      </MemoryRouter>
    );
    expect(screen.getByText("failed")).toBeInTheDocument();
    expect(screen.getByText(/安全错误标签：http_status:401/)).toBeInTheDocument();

    view.rerender(
      <MemoryRouter>
        <ProviderTab
          config={mockConfig}
          setConfig={vi.fn()}
          diagnostics={
            {
              ...mockDiagnostics,
              cloud_api_validated: false,
              cloud_api_validation_status: "stale",
            } as any
          }
          routerStatus={null}
          modelRouterStatus={null}
        />
      </MemoryRouter>
    );
    expect(screen.getByText("stale")).toBeInTheDocument();
    expect(screen.getByText(/Provider 验证已失效/)).toBeInTheDocument();
  });

  it("lets users opt into capability-first beta without changing the default mode", async () => {
    const setConfig = vi.fn();
    await act(async () => {
      render(
        <MemoryRouter>
          <ProviderTab
            config={{ ...mockConfig, runtime_mode: "local_first_default" }}
            setConfig={setConfig}
            diagnostics={mockDiagnostics}
            routerStatus={null}
            modelRouterStatus={null}
          />
        </MemoryRouter>
      );
    });

    expect(screen.getByRole("button", { name: /Local-first default/ })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /Capability-first beta/ }));
    expect(setConfig).toHaveBeenCalledWith({
      ...mockConfig,
      runtime_mode: "capability_first_beta",
    });
  });

  it("summarizes router state without exposing internals", async () => {
    await act(async () => {
      render(
        <MemoryRouter>
          <ProviderTab
            config={mockConfig}
            setConfig={vi.fn()}
            diagnostics={mockDiagnostics}
            routerStatus={
              {
                onnx_available: true,
                onnx_disabled: false,
                active_backend: "regex",
                latency_threshold_us: 50000,
              } as any
            }
            modelRouterStatus={null}
          />
        </MemoryRouter>
      );
    });
    expect(screen.getByText(/自动路由/)).toBeInTheDocument();
    expect(screen.getByText(/regex/)).toBeInTheDocument();
    expect(screen.queryByText(/路由诊断（高级）/)).not.toBeInTheDocument();
  });

  it("shows llama 3.1 as a local preset when Ollama resolved that installed tag", async () => {
    await act(async () => {
      render(
        <MemoryRouter>
          <ProviderTab
            config={mockConfig}
            setConfig={vi.fn()}
            diagnostics={
              {
                ...mockDiagnostics,
                ollama_service_online: true,
                ollama_online: true,
                resolved_local_model: "llama3.1:8b",
                ollama_models: [{ name: "llama3.1:8b", size_mb: 8192 }],
              } as any
            }
            routerStatus={null}
            modelRouterStatus={null}
          />
        </MemoryRouter>
      );
    });

    expect(screen.getByRole("option", { name: "Llama 3.1" })).toBeInTheDocument();
    expect(screen.getByText(/Ollama 在线/)).toHaveTextContent("llama3.1:8b");
    expect(screen.queryByText(/Ollama 离线/)).not.toBeInTheDocument();
  });

  it("lists arbitrary installed Ollama models as selectable local options", async () => {
    await act(async () => {
      render(
        <MemoryRouter>
          <ProviderTab
            config={mockConfig}
            setConfig={vi.fn()}
            diagnostics={
              {
                ...mockDiagnostics,
                ollama_service_online: true,
                ollama_online: true,
                resolved_local_model: "qwen3:8b",
                ollama_models: [
                  { name: "qwen3:8b", size_mb: 8192 },
                  { name: "llava:13b", size_mb: 13312 },
                ],
              } as any
            }
            routerStatus={null}
            modelRouterStatus={null}
          />
        </MemoryRouter>
      );
    });

    expect(screen.getByRole("option", { name: "qwen3:8b" })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "llava:13b" })).toBeInTheDocument();
  });

  it("allows entering a custom Ollama model tag without waiting for a preset", async () => {
    const setConfig = vi.fn();
    await act(async () => {
      render(
        <MemoryRouter>
          <ProviderTab
            config={mockConfig}
            setConfig={setConfig}
            diagnostics={mockDiagnostics}
            routerStatus={null}
            modelRouterStatus={null}
          />
        </MemoryRouter>
      );
    });

    fireEvent.change(screen.getByPlaceholderText(/qwen3:8b/), {
      target: { value: "future-model:latest" },
    });

    expect(setConfig).toHaveBeenCalledWith({
      ...mockConfig,
      local_model: "future-model:latest",
    });
  });

  it("keeps tools and debug controls out of the model tab", async () => {
    await act(async () => {
      render(
        <MemoryRouter>
          <ProviderTab
            config={mockConfig}
            setConfig={vi.fn()}
            diagnostics={mockDiagnostics}
            routerStatus={null}
            modelRouterStatus={null}
          />
        </MemoryRouter>
      );
    });

    expect(screen.queryByText(/启用 AgentLoop/)).not.toBeInTheDocument();
    expect(screen.queryByText(/ContextAssembler V2/)).not.toBeInTheDocument();
    expect(screen.queryByText(/文件访问（Safe Paths）/)).not.toBeInTheDocument();
    expect(screen.queryByText(/工具权限与确认/)).not.toBeInTheDocument();
  });

  it("does not show internal debug toggles even when Advanced is enabled elsewhere", async () => {
    await act(async () => {
      render(
        <MemoryRouter>
          <ProviderTab
            config={mockConfig}
            setConfig={vi.fn()}
            diagnostics={mockDiagnostics}
            routerStatus={null}
            modelRouterStatus={null}
            showInternalDebug
          />
        </MemoryRouter>
      );
    });

    expect(screen.queryByText(/启用 AgentLoop/)).not.toBeInTheDocument();
    expect(screen.queryByText(/ContextAssembler V2/)).not.toBeInTheDocument();
  });
});
