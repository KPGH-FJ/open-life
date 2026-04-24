import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import LifeModelEditor from "./LifeModelEditor";
import { invoke } from "@tauri-apps/api/core";
import { mockInvoke } from "@/test/mocks/tauri";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("LifeModelEditor", () => {
  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    vi.mocked(invoke).mockImplementation(mockInvoke);
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.clearAllMocks();
  });

  it("shows life map read mode by default", async () => {
    render(<LifeModelEditor />);

    expect(await screen.findByText("人生地图")).toBeInTheDocument();
    expect(screen.getByText(/OpenLife 当前对你的理解/)).toBeInTheDocument();
    expect(screen.getByText("Identity 我是谁")).toBeInTheDocument();
    expect(screen.getByText("Goals 我要去哪里")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /编辑模型/ })).toBeInTheDocument();
  });

  it("renders newly added identity and preference sections", async () => {
    render(<LifeModelEditor />);

    fireEvent.click(await screen.findByRole("button", { name: /编辑模型/ }));

    await waitFor(() => {
      expect(screen.getByText("角色与表达风格")).toBeInTheDocument();
    });

    expect(screen.getByText("关系与偏好")).toBeInTheDocument();
    expect(screen.getByLabelText("使命陈述")).toBeInTheDocument();
    expect(screen.getByLabelText("主要角色")).toBeInTheDocument();
    expect(screen.getByLabelText("高能时段")).toBeInTheDocument();
    expect(screen.getByText("工具能力")).toBeInTheDocument();
    expect(screen.getByText("知识领域")).toBeInTheDocument();
  });

  it("saves updated primary role through save_life_model", async () => {
    render(<LifeModelEditor />);

    fireEvent.click(await screen.findByRole("button", { name: /编辑模型/ }));
    const primaryRole = await screen.findByLabelText("主要角色");
    fireEvent.change(primaryRole, { target: { value: "产品负责人" } });
    fireEvent.click(screen.getByRole("button", { name: /保存/ }));

    await waitFor(() => {
      const call = vi.mocked(invoke).mock.calls.find(([cmd]) => cmd === "save_life_model");
      expect(call).toBeTruthy();
      expect(call?.[1]).toMatchObject({
        lifeModel: {
          identity: {
            role_definition: {
              primary_role: "产品负责人",
            },
          },
        },
      });
    });
  });

  it("auto-saves after debounce when model changes", async () => {
    render(<LifeModelEditor />);

    fireEvent.click(await screen.findByRole("button", { name: /编辑模型/ }));
    const primaryRole = await screen.findByLabelText("主要角色");
    fireEvent.change(primaryRole, { target: { value: "自动保存测试" } });

    // 提前推进时间不足以触发保存
    vi.advanceTimersByTime(500);
    const callsBefore = vi.mocked(invoke).mock.calls.filter(([cmd]) => cmd === "save_life_model").length;
    expect(callsBefore).toBe(0);

    // 推进超过 2s debounce
    vi.advanceTimersByTime(2000);

    await waitFor(() => {
      const callsAfter = vi.mocked(invoke).mock.calls.filter(([cmd]) => cmd === "save_life_model").length;
      expect(callsAfter).toBeGreaterThanOrEqual(1);
    });

    // 页面上应出现“已自动保存”提示
    await waitFor(() => {
      expect(screen.getByText("已自动保存")).toBeInTheDocument();
    });
  });

  it("shows load error without auto-saving an empty model", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_life_model") {
        return Promise.reject(new Error("database unavailable"));
      }
      return mockInvoke(cmd, args);
    });

    render(<LifeModelEditor />);

    expect(await screen.findByText("人生模型读取失败")).toBeInTheDocument();
    expect(screen.getByText("database unavailable")).toBeInTheDocument();

    vi.advanceTimersByTime(3000);

    const saveCalls = vi.mocked(invoke).mock.calls.filter(([cmd]) => cmd === "save_life_model");
    expect(saveCalls).toHaveLength(0);
  });

  it("shows safe mode read-only banner and blocks save when diagnostics are degraded", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_system_diagnostics") {
        return Promise.resolve({
          beta_ready: false,
          beta_ready_issues: ["memory degraded"],
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
          legacy_data_dir: "/tmp/openlife-legacy",
          database_status: "degraded",
          startup_warnings: ["memory.db 初始化失败，正在使用临时数据库"],
        });
      }
      return mockInvoke(cmd, args);
    });

    render(<LifeModelEditor />);

    expect(await screen.findByText(/Safe Mode：人生模型当前只读/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /只读查看/ }));
    expect(await screen.findByText(/编辑已切换为只读/)).toBeInTheDocument();

    const saveButton = screen.getByRole("button", { name: /保存/ });
    expect(saveButton).toBeDisabled();

    vi.advanceTimersByTime(3000);
    const saveCalls = vi.mocked(invoke).mock.calls.filter(([cmd]) => cmd === "save_life_model");
    expect(saveCalls).toHaveLength(0);
    expect(screen.getAllByText(/去 Settings 的恢复控制台/).length).toBeGreaterThan(0);
  });

  it("collapses and expands sections via SectionHeader", async () => {
    render(<LifeModelEditor />);

    fireEvent.click(await screen.findByRole("button", { name: /编辑模型/ }));
    await waitFor(() => {
      expect(screen.getByText("基本信息")).toBeInTheDocument();
    });

    // 默认展开时能看到使命陈述输入框
    expect(screen.getByLabelText("使命陈述")).toBeInTheDocument();

    // 点击“基本信息”折叠
    fireEvent.click(screen.getByRole("button", { name: "基本信息" }));

    // 折叠后使命陈述输入框应消失
    await waitFor(() => {
      expect(screen.queryByLabelText("使命陈述")).not.toBeInTheDocument();
    });

    // 再次点击展开
    fireEvent.click(screen.getByRole("button", { name: "基本信息" }));

    await waitFor(() => {
      expect(screen.getByLabelText("使命陈述")).toBeInTheDocument();
    });
  });
});
