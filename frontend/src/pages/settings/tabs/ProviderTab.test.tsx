import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
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

  it("renders model router status notice", async () => {
    await act(async () => {
      render(
        <MemoryRouter>
          <ProviderTab
            config={mockConfig}
            setConfig={vi.fn()}
            diagnostics={mockDiagnostics}
            routerStatus={null}
            modelRouterStatus={null}
            agentSpec={null}
            agentSpecSaving={false}
            onUpdateAgentSpecPrivacy={vi.fn()}
          />
        </MemoryRouter>
      );
    });
    expect(screen.getAllByText(/ModelRouter/).length).toBeGreaterThan(0);
  });

  it("renders layer 1 router status section", async () => {
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
            agentSpec={null}
            agentSpecSaving={false}
            onUpdateAgentSpecPrivacy={vi.fn()}
          />
        </MemoryRouter>
      );
    });
    expect(screen.getByText(/Layer 1 路由状态/)).toBeInTheDocument();
  });
});
