import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import { BrowserRouter } from "react-router-dom";
import VersionControl from "./VersionControl";
import { invoke } from "@tauri-apps/api/core";
import { mockInvoke } from "@/test/mocks/tauri";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("VersionControl", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockImplementation(mockInvoke);
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("shows diff summary after comparing versions", async () => {
    render(
      <BrowserRouter>
        <VersionControl />
      </BrowserRouter>
    );

    await waitFor(() => {
      expect(screen.getByText("历史版本")).toBeInTheDocument();
    });

    const checkboxes = await screen.findAllByRole("checkbox");
    fireEvent.click(checkboxes[0]);
    fireEvent.click(checkboxes[1]);
    fireEvent.click(screen.getByText("对比选中版本"));

    await waitFor(() => {
      expect(screen.getByText("差异摘要")).toBeInTheDocument();
    });

    expect(screen.getByText(/身份 · \d+ 处/)).toBeInTheDocument();
    expect(screen.getByText(/目标 · \d+ 处/)).toBeInTheDocument();
    expect(screen.getByText("关键变化")).toBeInTheDocument();
    await waitFor(() => {
      expect(screen.getByRole("heading", { name: /差异对比/ }).closest("section")).toHaveFocus();
    });
  });

  it("shows safe mode banner and blocks create/restore actions when diagnostics are degraded", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_system_diagnostics") {
        return Promise.resolve({
          chat_ready: true,
          readiness_issues: [],
          local_model: "qwen2.5:7b",
          resolved_local_model: "qwen2.5:7b",
          ollama_running: true,
          cloud_api_configured: true,
          life_model_ready: true,
          memory_chunk_count: 10,
          vector_corrupt_embedding_count: 2,
          active_data_dir: "/tmp/openlife",
          database_status: "degraded",
          startup_warnings: ["memory.db 初始化失败，正在使用临时数据库"],
        });
      }
      return mockInvoke(cmd, args);
    });

    render(
      <BrowserRouter>
        <VersionControl />
      </BrowserRouter>
    );

    expect(await screen.findByText(/Safe Mode：版本写入已暂停/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "快照" })).toBeDisabled();
    expect(screen.getAllByRole("button", { name: "回滚" })[0]).toBeDisabled();
    expect(screen.getByText(/去恢复控制台/)).toBeInTheDocument();
  });

  it("sends a governed restore request and reports success", async () => {
    const confirmSpy = vi.spyOn(window, "confirm").mockReturnValue(true);
    try {
      render(
        <BrowserRouter>
          <VersionControl />
        </BrowserRouter>
      );

      await waitFor(() => {
        expect(screen.getByText("历史版本")).toBeInTheDocument();
      });

      fireEvent.click(screen.getAllByRole("button", { name: "回滚" })[0]);

      await waitFor(() => {
        expect(screen.getByText(/回滚成功/)).toBeInTheDocument();
      });
      expect(invoke).toHaveBeenCalledWith("restore_snapshot", {
        version: "0.1.0",
        governedRequest: {
          purpose: "manual_restore",
          explicitUserIntent: true,
          createPreChangeSnapshot: true,
        },
        governed_request: {
          purpose: "manual_restore",
          explicitUserIntent: true,
          createPreChangeSnapshot: true,
        },
      });
    } finally {
      confirmSpy.mockRestore();
    }
  });
});
