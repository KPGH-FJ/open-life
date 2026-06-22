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
