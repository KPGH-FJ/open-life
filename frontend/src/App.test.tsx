import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import App from "./App";
import { invoke } from "@tauri-apps/api/core";
import { mockInvoke } from "@/test/mocks/tauri";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

describe("App onboarding", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockImplementation(mockInvoke);
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("shows onboarding when first-run flag is not completed", async () => {
    render(
      <MemoryRouter>
        <App />
      </MemoryRouter>
    );

    expect(await screen.findByText("欢迎使用 OpenLife")).toBeInTheDocument();
  });

  it("hides onboarding after completion", async () => {
    render(
      <MemoryRouter>
        <App />
      </MemoryRouter>
    );

    expect(await screen.findByText("欢迎使用 OpenLife")).toBeInTheDocument();
    fireEvent.click(screen.getByText("下一步"));
    fireEvent.click(screen.getByText("下一步"));
    fireEvent.click(screen.getByText("下一步"));
    fireEvent.click(screen.getByText("开始使用 OpenLife"));

    await waitFor(() => {
      expect(screen.queryByText("欢迎使用 OpenLife")).not.toBeInTheDocument();
    });
    expect(invoke).toHaveBeenCalledWith("mark_onboarding_completed", undefined);
  });
});
