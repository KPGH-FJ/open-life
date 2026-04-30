import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import { BrowserRouter } from "react-router-dom";
import A2APage from "./A2APage";
import { invoke } from "@tauri-apps/api/core";
import { mockInvoke } from "@/test/mocks/tauri";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("A2APage", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockImplementation(mockInvoke);
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("renders local A2A service section", async () => {
    render(
      <BrowserRouter>
        <A2APage />
      </BrowserRouter>
    );

    await waitFor(() => {
      expect(screen.getByText("A2A Server - OpenLife 本地服务")).toBeInTheDocument();
    });

    expect(screen.getByText("OpenLife ↔ A2A 桥接调试")).toBeInTheDocument();
  });

  it("runs local bridge preview", async () => {
    render(
      <BrowserRouter>
        <A2APage />
      </BrowserRouter>
    );

    await waitFor(() => {
      expect(screen.getByText("桥接运行")).toBeInTheDocument();
    });

    const inputs = screen.getAllByRole("textbox");
    fireEvent.change(inputs[3], { target: { value: "帮我做一个决策摘要" } });
    fireEvent.click(screen.getByText("桥接运行"));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "a2a_bridge_local",
        expect.objectContaining({ text: "帮我做一个决策摘要" })
      );
    });
  });
});
