import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { BrowserRouter } from "react-router-dom";
import MemorySearch from "./MemorySearch";
import { invoke } from "@tauri-apps/api/core";
import { mockInvoke } from "@/test/mocks/tauri";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("MemorySearch", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockImplementation(mockInvoke);
    vi.spyOn(window, "confirm").mockImplementation(() => true);
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("shows memory governance guidance", async () => {
    render(
      <BrowserRouter>
        <MemorySearch />
      </BrowserRouter>
    );

    expect(await screen.findByText("记忆治理说明")).toBeInTheDocument();
    expect(screen.getByText(/这个页面负责回答三个问题/)).toBeInTheDocument();
    expect(screen.getByText("搜索记忆")).toBeInTheDocument();
    expect(screen.getByText("归档与恢复")).toBeInTheDocument();
  });

  it("shows safe mode prompt and blocks indexing when diagnostics are degraded", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_system_diagnostics") {
        return Promise.resolve({
          beta_ready: false,
          beta_readiness_issues: [],
          chat_ready: true,
          readiness_issues: [],
          local_model: "qwen2.5:7b",
          resolved_local_model: "qwen2.5:7b",
          ollama_online: true,
          cloud_api_configured: true,
          life_model_ready: true,
          memory_chunk_count: 10,
          vector_corrupt_embedding_count: 2,
          active_data_dir: "/tmp/openlife",
          legacy_data_dir: "/tmp/openlife-legacy",
          database_status: "degraded",
          startup_warnings: ["memory.db 初始化失败，正在使用临时数据库"],
        });
      }
      return mockInvoke(cmd, args);
    });

    render(
      <BrowserRouter>
        <MemorySearch />
      </BrowserRouter>
    );

    expect(await screen.findByText(/Safe Mode：记忆写入操作已建议暂停/)).toBeInTheDocument();

    const contentInput = screen.getByPlaceholderText("输入要索引的记忆内容...");
    fireEvent.change(contentInput, { target: { value: "需要索引的记忆" } });
    fireEvent.click(screen.getByRole("button", { name: "索引" }));

    expect(await screen.findByText(/当前处于 Safe Mode/)).toBeInTheDocument();
    expect(invoke).not.toHaveBeenCalledWith("index_memory_chunk", expect.anything());
  });

  it("blocks restoring archived memory in safe mode", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_system_diagnostics") {
        return Promise.resolve({
          beta_ready: false,
          beta_readiness_issues: [],
          chat_ready: true,
          readiness_issues: [],
          local_model: "qwen2.5:7b",
          resolved_local_model: "qwen2.5:7b",
          ollama_online: true,
          cloud_api_configured: true,
          life_model_ready: true,
          memory_chunk_count: 10,
          vector_corrupt_embedding_count: 2,
          active_data_dir: "/tmp/openlife",
          legacy_data_dir: "/tmp/openlife-legacy",
          database_status: "degraded",
          startup_warnings: ["memory.db 初始化失败，正在使用临时数据库"],
        });
      }
      if (cmd === "list_archived_chunks") {
        return Promise.resolve([
          {
            id: 1,
            session_id: "sess-1",
            content: "一段已归档内容",
            summary: "归档摘要",
            source: "chat",
            access_count: 2,
            importance_score: 0.42,
            archived_at: new Date().toISOString(),
          },
        ]);
      }
      return mockInvoke(cmd, args);
    });

    render(
      <BrowserRouter>
        <MemorySearch />
      </BrowserRouter>
    );

    expect(await screen.findByText(/归档摘要/)).toBeInTheDocument();
    // 等待 Safe Mode banner 渲染完成（diagnostics 异步加载）
    expect(await screen.findByText(/Safe Mode：记忆写入操作已建议暂停/)).toBeInTheDocument();
    // 确保 handleRestore 已更新（safeMode 为 true）
    await new Promise(resolve => setTimeout(resolve, 50));
    fireEvent.click(screen.getByRole("button", { name: /恢复/i }));
    // 使用 waitFor 轮询等待 archiveMsg 更新
    await waitFor(
      () => {
        const msg = screen.getByTestId("archive-msg");
        expect(msg.textContent).toMatch(/当前处于 Safe Mode/);
      },
      { timeout: 3000 }
    );
    expect(invoke).not.toHaveBeenCalledWith("restore_archived_chunks", expect.anything());
  });

  it("collapses low confidence search noise by default", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "search_memory") {
        return Promise.resolve([
          [
            {
              id: 1,
              session_id: "sess-1",
              content: "重庆开州是用户查询过的地点。",
              source: "manual",
              created_at: new Date().toISOString(),
            },
            0.92,
          ],
          [
            {
              id: 2,
              session_id: "sess-2",
              content: "一条低相关历史记忆，应该默认折叠。",
              source: "chat",
              created_at: new Date().toISOString(),
            },
            0.12,
          ],
        ]);
      }
      return mockInvoke(cmd, args);
    });

    render(
      <BrowserRouter>
        <MemorySearch />
      </BrowserRouter>
    );

    fireEvent.change(await screen.findByPlaceholderText("输入查询语义..."), {
      target: { value: "重庆开州" },
    });
    fireEvent.click(screen.getByRole("button", { name: "搜索" }));

    expect(await screen.findByText("重庆开州是用户查询过的地点。")).toBeInTheDocument();
    expect(screen.getByText("包含精确查询文本")).toBeInTheDocument();
    expect(screen.queryByText("一条低相关历史记忆，应该默认折叠。")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "显示 1 条低相关结果" }));
    expect(await screen.findByText("一条低相关历史记忆，应该默认折叠。")).toBeInTheDocument();
  });
});
