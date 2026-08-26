import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ProviderConnectionsViewModel } from "@/tauri";
import { ProviderConnectionsPanel } from "./ProviderConnectionsPanel";

const ipc = vi.hoisted(() => ({
  loadProviderConnections: vi.fn(),
  saveProviderConnection: vi.fn(),
  deleteProviderConnection: vi.fn(),
  testSavedProviderConnection: vi.fn(),
}));

const viewModel: ProviderConnectionsViewModel = {
  connections: [
    {
      id: "connection-1",
      providerId: "openrouter",
      displayName: "OpenRouter",
      endpoint: "https://openrouter.ai/api/v1",
      credentialState: "stored",
      validationState: "ready",
      models: [
        {
          profileId: "profile-1",
          modelId: "stealth/ox-alpha",
          displayName: "Ox Alpha",
          selected: true,
          validationState: "ready",
        },
      ],
    },
  ],
};

describe("ProviderConnectionsPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    ipc.loadProviderConnections.mockResolvedValue(viewModel);
    ipc.saveProviderConnection.mockResolvedValue(viewModel);
  });

  it("shows a compact connection row and retains the stored credential when editing", async () => {
    render(<ProviderConnectionsPanel dataSource={ipc} />);

    expect(await screen.findByText("Ox Alpha")).toBeInTheDocument();
    expect(screen.getByText("可用")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "编辑" }));
    expect(screen.getByLabelText("API 凭据")).toHaveValue("");
    fireEvent.click(screen.getByRole("button", { name: "保存连接" }));

    await waitFor(() =>
      expect(ipc.saveProviderConnection).toHaveBeenCalledWith(
        expect.objectContaining({
          id: "connection-1",
          providerId: "openrouter",
          modelId: "stealth/ox-alpha",
          credential: undefined,
        })
      )
    );
  });
});
