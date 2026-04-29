import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import { BrowserRouter } from "react-router-dom";
import BuilderPage from "./BuilderPage";
import { invoke } from "@tauri-apps/api/core";
import { mockInvoke, mockLifeModel } from "@/test/mocks/tauri";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("BuilderPage", () => {
  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    vi.mocked(invoke).mockImplementation(mockInvoke);
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.clearAllMocks();
  });

  it("renders builder page with mode selection", async () => {
    render(
      <BrowserRouter>
        <BuilderPage />
      </BrowserRouter>
    );

    await waitFor(() => {
      expect(screen.getByText("人生模型构建")).toBeInTheDocument();
    });

    expect(screen.getByText("快速构建")).toBeInTheDocument();
    expect(screen.getByText("渐进构建")).toBeInTheDocument();
    expect(screen.getByText("苏格拉底对话")).toBeInTheDocument();
  });

  it("starts quick build mode when clicked", async () => {
    render(
      <BrowserRouter>
        <BuilderPage />
      </BrowserRouter>
    );

    await waitFor(() => {
      expect(screen.getByText("快速构建")).toBeInTheDocument();
    });

    const quickButton = screen.getByText("快速构建").closest("button");
    expect(quickButton).not.toBeNull();
    fireEvent.click(quickButton!);

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "builder_start",
        expect.objectContaining({ mode: "quick" })
      );
    });
  });

  it("shows completion radar chart after loading", async () => {
    render(
      <BrowserRouter>
        <BuilderPage />
      </BrowserRouter>
    );

    await waitFor(() => {
      expect(screen.getByText("人生模型四维完成度")).toBeInTheDocument();
    });
  });

  it("shows next-step suggestions after loading completion", async () => {
    render(
      <BrowserRouter>
        <BuilderPage />
      </BrowserRouter>
    );

    await waitFor(() => {
      expect(screen.getByText("推荐下一步")).toBeInTheDocument();
    });
  });

  it("shows inline error and retry when builder start fails", async () => {
    const alertSpy = vi.spyOn(window, "alert").mockImplementation(() => {});
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "builder_start") {
        return Promise.reject(new Error("builder boom"));
      }
      return mockInvoke(cmd, args);
    });

    render(
      <BrowserRouter>
        <BuilderPage />
      </BrowserRouter>
    );

    const quickButton = await screen.findByText("快速构建");
    fireEvent.click(quickButton.closest("button")!);

    expect(await screen.findByText(/启动构建会话失败/)).toBeInTheDocument();
    expect(screen.getByText("重试启动")).toBeInTheDocument();
    expect(alertSpy).not.toHaveBeenCalled();
    alertSpy.mockRestore();
  });

  it("retries incremental start with the selected target dimension", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "builder_start") {
        return Promise.reject(new Error("builder boom"));
      }
      return mockInvoke(cmd, args);
    });

    render(
      <BrowserRouter>
        <BuilderPage />
      </BrowserRouter>
    );

    fireEvent.click((await screen.findByText("渐进构建")).closest("button")!);
    fireEvent.click(screen.getByTestId("incremental-dim-goals"));

    expect(await screen.findByText(/启动构建会话失败/)).toBeInTheDocument();
    fireEvent.click(screen.getByText("重试启动"));

    await waitFor(() => {
      const startCalls = vi.mocked(invoke).mock.calls.filter(([cmd]) => cmd === "builder_start");
      expect(startCalls.length).toBeGreaterThanOrEqual(2);
      expect(startCalls[startCalls.length - 1][1]).toMatchObject({
        mode: "incremental",
        targetDimension: "goals",
        target_dimension: "goals",
      });
    });
  });

  it("submits phase confirmation without relying on pending input state", async () => {
    const customMock = vi.fn((cmd: string, args?: Record<string, any>): Promise<any> => {
      if (cmd === "builder_start") {
        return Promise.resolve({
          prompt: "这是阶段总结",
          progress: {
            progress: 0.5,
            current_step_label: "阶段确认",
            step_index: 3,
            total_steps: 6,
            waiting_phase_confirmation: true,
            phase_summary: "请确认这段理解",
          },
        });
      }
      if (cmd === "builder_step") {
        return Promise.resolve({
          prompt: "继续",
          finished: false,
          progress: { progress: 0.6, current_step_label: "下一步", step_index: 4, total_steps: 6 },
          mode: "Socratic",
          pending_signals: [],
        });
      }
      return mockInvoke(cmd, args);
    });
    vi.mocked(invoke).mockImplementation(customMock);

    render(
      <BrowserRouter>
        <BuilderPage />
      </BrowserRouter>
    );

    fireEvent.click((await screen.findByText("苏格拉底对话")).closest("button")!);
    expect(await screen.findByText("确认，继续")).toBeInTheDocument();
    fireEvent.click(screen.getByText("确认，继续"));

    await waitFor(() => {
      expect(customMock).toHaveBeenCalledWith(
        "builder_step",
        expect.objectContaining({
          userReply: "确认",
          user_reply: "确认",
        })
      );
    });
  });

  it("submits pairwise choice without relying on pending input state", async () => {
    const customMock = vi.fn((cmd: string, args?: Record<string, any>): Promise<any> => {
      if (cmd === "builder_start") {
        return Promise.resolve({
          prompt: "A 成长 vs B 自由",
          progress: {
            progress: 0.4,
            current_step_label: "价值排序",
            step_index: 2,
            total_steps: 6,
            waiting_pairwise: true,
          },
        });
      }
      if (cmd === "builder_step") {
        return Promise.resolve({
          prompt: "继续",
          finished: false,
          progress: { progress: 0.5, current_step_label: "下一步", step_index: 3, total_steps: 6 },
          mode: "Socratic",
          pending_signals: [],
        });
      }
      return mockInvoke(cmd, args);
    });
    vi.mocked(invoke).mockImplementation(customMock);

    render(
      <BrowserRouter>
        <BuilderPage />
      </BrowserRouter>
    );

    fireEvent.click((await screen.findByText("苏格拉底对话")).closest("button")!);
    expect(await screen.findByText("选 A")).toBeInTheDocument();
    fireEvent.click(screen.getByText("选 A"));

    await waitFor(() => {
      expect(customMock).toHaveBeenCalledWith(
        "builder_step",
        expect.objectContaining({
          userReply: "A",
          user_reply: "A",
        })
      );
    });
  });

  it('passes target_dimension "goals" when starting incremental build for Goals', async () => {
    render(
      <BrowserRouter>
        <BuilderPage />
      </BrowserRouter>
    );

    await waitFor(() => {
      expect(screen.getByText("人生模型构建")).toBeInTheDocument();
    });

    // Enter incremental mode first
    const incrementalBtn = screen.getByText("渐进构建").closest("button");
    expect(incrementalBtn).not.toBeNull();
    fireEvent.click(incrementalBtn!);

    // Then click Goals dimension card
    const goalsButton = screen.getByTestId("incremental-dim-goals");
    expect(goalsButton).not.toBeNull();
    fireEvent.click(goalsButton);

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "builder_start",
        expect.objectContaining({
          mode: "incremental",
          targetDimension: "goals",
          target_dimension: "goals",
        })
      );
    });
  });

  it("restores pending review when resuming a finished quick build session", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "builder_list_unfinished") {
        return Promise.resolve([
          {
            session_id: "review-session",
            mode: "Quick",
            step_index: 6,
            finished: true,
            draft_yaml: "",
            current_prompt: "请审阅以下建议",
            pending_signals: [
              {
                id: "sig-1",
                source_step: 1,
                source_question_id: "q1",
                dimension: "Identity",
                affected_path: "identity.name",
                proposed_value: "OpenLife 用户",
                confidence: 0.8,
                reason: "来自快速构建",
                risk_level: "low",
                user_status: "Pending",
              },
            ],
          },
        ]);
      }
      if (cmd === "builder_start") {
        return Promise.resolve({
          prompt: "请审阅以下建议",
          finished: true,
          progress: {
            progress: 1,
            current_step_label: "审阅模型建议",
            step_index: 6,
            total_steps: 6,
          },
          pending_signals: [
            {
              id: "sig-1",
              source_step: 1,
              source_question_id: "q1",
              dimension: "Identity",
              affected_path: "identity.name",
              proposed_value: "OpenLife 用户",
              confidence: 0.8,
              reason: "来自快速构建",
              risk_level: "low",
              user_status: "Pending",
            },
          ],
          mode: "Quick",
        });
      }
      return mockInvoke(cmd, args);
    });

    render(
      <BrowserRouter>
        <BuilderPage />
      </BrowserRouter>
    );

    expect(await screen.findByText("继续未完成的会话")).toBeInTheDocument();
    expect(screen.getByText("待确认 Review")).toBeInTheDocument();
    expect(screen.getByText("已完成问题收集，等待你确认并应用模型建议")).toBeInTheDocument();
    expect(screen.getByText("待确认内容：请审阅以下建议")).toBeInTheDocument();
    fireEvent.click(screen.getByText("去审阅"));

    expect(await screen.findByText("OpenLife 准备这样理解你")).toBeInTheDocument();
    expect(screen.getByText("名称")).toBeInTheDocument();
    expect(screen.getByText("OpenLife 用户")).toBeInTheDocument();
  });

  it("shows safe mode banner and blocks starting new builder sessions when diagnostics are degraded", async () => {
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

    render(
      <BrowserRouter>
        <BuilderPage />
      </BrowserRouter>
    );

    expect(await screen.findByText(/Safe Mode：构建写入已暂停/)).toBeInTheDocument();
    const quickButton = screen.getByText("快速构建").closest("button");
    expect(quickButton).toBeDisabled();
    expect(screen.getByText(/去恢复控制台/)).toBeInTheDocument();

    const startCalls = vi.mocked(invoke).mock.calls.filter(([cmd]) => cmd === "builder_start");
    expect(startCalls).toHaveLength(0);
  });

  it("passes defined target_dimension when starting incremental build for State", async () => {
    render(
      <BrowserRouter>
        <BuilderPage />
      </BrowserRouter>
    );

    await waitFor(() => {
      expect(screen.getByText("人生模型构建")).toBeInTheDocument();
    });

    // Enter incremental mode first
    const incrementalBtn = screen.getByText("渐进构建").closest("button");
    expect(incrementalBtn).not.toBeNull();
    fireEvent.click(incrementalBtn!);

    // Then click State dimension card
    const stateButton = screen.getByTestId("incremental-dim-state");
    expect(stateButton).not.toBeNull();
    fireEvent.click(stateButton);

    await waitFor(() => {
      const calls = vi.mocked(invoke).mock.calls;
      const startCall = calls.find(c => c[0] === "builder_start");
      expect(startCall).toBeDefined();
      const args = startCall![1] as Record<string, any>;
      expect(args.mode).toBe("incremental");
      expect(args.targetDimension).toBeDefined();
      expect(args.targetDimension).toBe("state");
      expect(args.target_dimension).toBeDefined();
      expect(args.target_dimension).not.toBeUndefined();
      expect(args.target_dimension).toBe("state");
    });
  });

  it("sends edited decisions with proposed_value to builder_create_proposals", async () => {
    const customMock = vi.fn((cmd: string, args?: Record<string, any>): Promise<any> => {
      if (cmd === "builder_start") {
        return Promise.resolve({
          prompt: "请描述你的价值观",
          progress: { progress: 0.2, current_step_label: "价值观", step_index: 1, total_steps: 5 },
        });
      }
      if (cmd === "builder_step") {
        return Promise.resolve({
          prompt: "完成",
          finished: true,
          progress: { progress: 1.0, current_step_label: "完成", step_index: 5, total_steps: 5 },
          mode: "Quick",
          pending_signals: [
            {
              id: "sig_name",
              source_step: 1,
              source_question_id: "name",
              dimension: "Identity",
              affected_path: "identity.name",
              proposed_value: "小林",
              confidence: 0.95,
              reason: "用户直接提供的称呼",
              risk_level: "low",
              user_status: "Pending",
            },
          ],
          model: null,
        });
      }
      if (cmd === "builder_create_proposals") {
        return Promise.resolve({
          success: true,
          created_count: 1,
          rejected_count: 0,
          proposal_ids: ["proposal-edited"],
          run_id: "run-edited",
          warnings: [],
        });
      }
      return mockInvoke(cmd, args);
    });
    vi.mocked(invoke).mockImplementation(customMock);

    render(
      <BrowserRouter>
        <BuilderPage />
      </BrowserRouter>
    );

    // Start quick build
    const quickButton = await screen.findByText("快速构建");
    fireEvent.click(quickButton.closest("button")!);

    // Wait for prompt to appear
    await waitFor(() => {
      expect(screen.getByText("请描述你的价值观")).toBeInTheDocument();
    });

    // Type a reply and send once — mock returns finished=true immediately
    const textarea = screen.getByPlaceholderText("输入你的回答...");
    fireEvent.change(textarea, { target: { value: "test reply" } });
    const nextButton = screen.getByText("下一步");
    fireEvent.click(nextButton);

    // Wait for review mode (BuilderPatchReview save button)
    await waitFor(() => {
      expect(screen.getByText("发送到 Review Center")).toBeInTheDocument();
      expect(screen.queryByText("直接应用（legacy / 绕过 Review Center）")).not.toBeInTheDocument();
    });

    // Ensure the checkbox is checked so save is enabled
    const checkboxes = screen.getAllByRole("checkbox");
    checkboxes.forEach(cb => {
      if (!(cb as HTMLInputElement).checked) {
        fireEvent.click(cb);
      }
    });

    // Edit the signal
    const editButtons = screen.getAllByTitle("编辑");
    fireEvent.click(editButtons[0]);

    const input = screen.getByDisplayValue("小林");
    fireEvent.change(input, { target: { value: "Alex" } });

    const saveEditButton = screen.getByText("保存");
    fireEvent.click(saveEditButton);

    // Click main save
    const saveButton = screen.getByText("发送到 Review Center");
    fireEvent.click(saveButton);

    await waitFor(() => {
      const calls = customMock.mock.calls;
      const proposalCall = calls.find(c => c[0] === "builder_create_proposals");
      expect(proposalCall).toBeDefined();
      expect(calls.find(c => c[0] === "builder_apply_signals")).toBeUndefined();
      const decisions = (proposalCall![1] as Record<string, any>).decisions;
      expect(decisions).toBeDefined();
      const edited = decisions.find((d: any) => d.id === "sig_name");
      expect(edited).toBeDefined();
      expect(edited.status).toBe("edited");
      expect(edited.proposed_value).toBe("Alex");
    });
  });

  it("sends builderCreateProposals payload matching backend contract", async () => {
    const customMock = vi.fn((cmd: string, args?: Record<string, any>): Promise<any> => {
      if (cmd === "builder_start") {
        return Promise.resolve({
          prompt: "请描述你的价值观",
          progress: { progress: 0.2, current_step_label: "价值观", step_index: 1, total_steps: 5 },
        });
      }
      if (cmd === "builder_step") {
        return Promise.resolve({
          prompt: "完成",
          finished: true,
          progress: { progress: 1.0, current_step_label: "完成", step_index: 5, total_steps: 5 },
          mode: "Quick",
          pending_signals: [
            {
              id: "sig_1",
              source_step: 1,
              source_question_id: "q1",
              dimension: "Identity",
              affected_path: "identity.name",
              proposed_value: "Test",
              confidence: 0.9,
              reason: "test",
              risk_level: "low",
              user_status: "Pending",
            },
          ],
          model: null,
        });
      }
      if (cmd === "builder_create_proposals") {
        return Promise.resolve({
          success: true,
          created_count: 1,
          rejected_count: 0,
          proposal_ids: ["proposal-1"],
          run_id: "run-1",
          warnings: [],
        });
      }
      return mockInvoke(cmd, args);
    });
    vi.mocked(invoke).mockImplementation(customMock);

    render(
      <BrowserRouter>
        <BuilderPage />
      </BrowserRouter>
    );

    // Start quick build to reach review
    const quickButton = await screen.findByText("快速构建");
    fireEvent.click(quickButton.closest("button")!);

    // Wait for prompt
    await waitFor(() => {
      expect(screen.getByText("请描述你的价值观")).toBeInTheDocument();
    });

    // Finish the quick build flow in one step
    const textarea = screen.getByPlaceholderText("输入你的回答...");
    const nextButton = screen.getByText("下一步");

    fireEvent.change(textarea, { target: { value: "reply" } });
    fireEvent.click(nextButton);

    // Wait for review mode (BuilderPatchReview save button)
    await waitFor(() => {
      expect(screen.getByText("发送到 Review Center")).toBeInTheDocument();
    });

    // Ensure checkboxes are checked so save is enabled
    const checkboxes = screen.getAllByRole("checkbox");
    checkboxes.forEach(cb => {
      if (!(cb as HTMLInputElement).checked) {
        fireEvent.click(cb);
      }
    });

    // Click save without editing — accepted by default
    const saveButton = screen.getByText("发送到 Review Center");
    fireEvent.click(saveButton);

    await waitFor(() => {
      const calls = customMock.mock.calls;
      const proposalCall = calls.find(c => c[0] === "builder_create_proposals");
      expect(proposalCall).toBeDefined();
      expect(calls.find(c => c[0] === "builder_apply_signals")).toBeUndefined();
      const payload = proposalCall![1] as Record<string, any>;

      // Backend contract: real Tauri expects camelCase, test mocks still accept snake_case.
      expect(payload).toHaveProperty("sessionId");
      expect(typeof payload.sessionId).toBe("string");
      expect(payload).toHaveProperty("session_id");
      expect(typeof payload.session_id).toBe("string");
      expect(payload).toHaveProperty("decisions");
      expect(Array.isArray(payload.decisions)).toBe(true);

      const decisions = payload.decisions;
      expect(decisions.length).toBeGreaterThan(0);
      const decision = decisions[0];
      expect(decision).toHaveProperty("id");
      expect(decision).toHaveProperty("status");
      expect(["accepted", "rejected", "edited"]).toContain(decision.status);

      // Low-risk should default to accepted
      if (decision.id === "sig_1") {
        expect(decision.status).toBe("accepted");
      }
    });
  });

  it("shows Review Center notice after proposal creation succeeds", async () => {
    const customMock = vi.fn((cmd: string, args?: Record<string, any>): Promise<any> => {
      if (cmd === "builder_start") {
        return Promise.resolve({
          prompt: "请描述你的价值观",
          progress: { progress: 0.2, current_step_label: "价值观", step_index: 1, total_steps: 5 },
        });
      }
      if (cmd === "builder_step") {
        return Promise.resolve({
          prompt: "完成",
          finished: true,
          progress: { progress: 1.0, current_step_label: "完成", step_index: 5, total_steps: 5 },
          mode: "Quick",
          pending_signals: [
            {
              id: "sig_1",
              source_step: 1,
              source_question_id: "q1",
              dimension: "Identity",
              affected_path: "identity.name",
              proposed_value: "Test",
              confidence: 0.9,
              reason: "test",
              risk_level: "low",
              user_status: "Pending",
            },
          ],
          model: mockLifeModel,
        });
      }
      if (cmd === "builder_create_proposals") {
        return Promise.resolve({
          success: true,
          created_count: 2,
          rejected_count: 2,
          proposal_ids: ["proposal-1", "proposal-2"],
          run_id: "run-review-center",
          warnings: [],
        });
      }
      return mockInvoke(cmd, args);
    });
    vi.mocked(invoke).mockImplementation(customMock);

    render(
      <BrowserRouter>
        <BuilderPage />
      </BrowserRouter>
    );

    fireEvent.click((await screen.findByText("快速构建")).closest("button")!);
    await waitFor(() => {
      expect(screen.getByText("请描述你的价值观")).toBeInTheDocument();
    });

    fireEvent.change(screen.getByPlaceholderText("输入你的回答..."), {
      target: { value: "reply" },
    });
    fireEvent.click(screen.getByText("下一步"));

    await waitFor(() => {
      expect(screen.getByText("发送到 Review Center")).toBeInTheDocument();
    });

    const checkboxes = screen.getAllByRole("checkbox");
    checkboxes.forEach(cb => {
      if (!(cb as HTMLInputElement).checked) fireEvent.click(cb);
    });
    fireEvent.click(screen.getByText("发送到 Review Center"));

    await waitFor(() => {
      expect(screen.getByText(/已创建/)).toBeInTheDocument();
    });

    expect(screen.getByText(/条待确认 Proposal/)).toBeInTheDocument();
    expect(screen.getByText("去 Review Center 确认 →")).toBeInTheDocument();
    expect(screen.queryByText("本轮写入结果")).not.toBeInTheDocument();
  });

  it("returns review-only builds back to unfinished sessions when choosing not to save", async () => {
    const customMock = vi.fn((cmd: string, args?: Record<string, any>): Promise<any> => {
      if (cmd === "builder_list_unfinished") {
        return Promise.resolve([
          {
            session_id: "draft-review",
            mode: "Quick",
            step_index: 6,
            finished: true,
            draft_yaml: "",
            current_prompt: "请审阅以下建议",
          },
        ]);
      }
      if (cmd === "builder_start") {
        return Promise.resolve({
          prompt: "请描述你的价值观",
          progress: { progress: 0.2, current_step_label: "价值观", step_index: 1, total_steps: 5 },
        });
      }
      if (cmd === "builder_step") {
        return Promise.resolve({
          prompt: "完成",
          finished: true,
          progress: { progress: 1.0, current_step_label: "完成", step_index: 5, total_steps: 5 },
          mode: "Quick",
          pending_signals: [
            {
              id: "sig_1",
              source_step: 1,
              source_question_id: "q1",
              dimension: "Identity",
              affected_path: "identity.name",
              proposed_value: "Test",
              confidence: 0.9,
              reason: "test",
              risk_level: "low",
              user_status: "Pending",
            },
          ],
          model: null,
        });
      }
      return mockInvoke(cmd, args);
    });
    vi.mocked(invoke).mockImplementation(customMock);

    render(
      <BrowserRouter>
        <BuilderPage />
      </BrowserRouter>
    );

    fireEvent.click((await screen.findByText("快速构建")).closest("button")!);
    await waitFor(() => {
      expect(screen.getByText("请描述你的价值观")).toBeInTheDocument();
    });

    fireEvent.change(screen.getByPlaceholderText("输入你的回答..."), {
      target: { value: "reply" },
    });
    fireEvent.click(screen.getByText("下一步"));

    await waitFor(() => {
      expect(screen.getByText("发送到 Review Center")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText("暂不保存"));

    await waitFor(() => {
      expect(
        screen.getByText("这轮理解已暂存到未完成会话，还没有写入人生模型。你可以稍后回来继续审阅。")
      ).toBeInTheDocument();
    });

    expect(screen.queryByText("构建完成！")).not.toBeInTheDocument();
    expect(screen.getByText("继续未完成的会话")).toBeInTheDocument();
  });
});
