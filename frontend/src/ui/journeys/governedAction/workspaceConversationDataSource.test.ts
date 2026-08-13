import { beforeEach, describe, expect, it, vi } from "vitest";
import type { StreamMessageChunkPayload, StreamMessageStartPayload } from "@/tauri";

const mocks = vi.hoisted(() => ({
  listeners: new Map<string, (event: { payload: unknown }) => void>(),
  unlisten: vi.fn(),
  startStreamMessage: vi.fn(),
  cancelChatTurn: vi.fn(),
  cancelWorkTask: vi.fn(),
  createProject: vi.fn(),
  assignConversationProject: vi.fn(),
  pickAndImportResources: vi.fn(),
  detachResourceFromTurn: vi.fn(),
  listMainChatSkills: vi.fn(),
  selectMainChatSkill: vi.fn(),
  clearMainChatSkill: vi.fn(),
  listMainChatToolCandidates: vi.fn(),
  getMarkdownMemoryViewModel: vi.fn(),
  getConversationViewModel: vi.fn(),
  selectMarkdownMemoryRoot: vi.fn(),
  draftMarkdownMemoryFileProposal: vi.fn(),
  deactivateMarkdownMemoryFileProposal: vi.fn(),
  submitMainChatTaskSteering: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (event: string, handler: (event: { payload: unknown }) => void) => {
    mocks.listeners.set(event, handler);
    return mocks.unlisten;
  }),
}));

vi.mock("@/tauri", () => ({
  cancelMainChatAgentTask: vi.fn(),
  cancelChatTurn: mocks.cancelChatTurn,
  cancelWorkTask: mocks.cancelWorkTask,
  createChatSession: vi.fn(),
  createProject: mocks.createProject,
  assignConversationProject: mocks.assignConversationProject,
  deleteChatSession: vi.fn(),
  getConversationViewModel: mocks.getConversationViewModel,
  renameChatSession: vi.fn(),
  startStreamMessage: mocks.startStreamMessage,
  pickAndImportResources: mocks.pickAndImportResources,
  detachResourceFromTurn: mocks.detachResourceFromTurn,
  listMainChatSkills: mocks.listMainChatSkills,
  selectMainChatSkill: mocks.selectMainChatSkill,
  clearMainChatSkill: mocks.clearMainChatSkill,
  listMainChatToolCandidates: mocks.listMainChatToolCandidates,
  getMarkdownMemoryViewModel: mocks.getMarkdownMemoryViewModel,
  selectMarkdownMemoryRoot: mocks.selectMarkdownMemoryRoot,
  draftMarkdownMemoryFileProposal: mocks.draftMarkdownMemoryFileProposal,
  deactivateMarkdownMemoryFileProposal: mocks.deactivateMarkdownMemoryFileProposal,
  submitMainChatTaskSteering: mocks.submitMainChatTaskSteering,
}));

import { tauriWorkspaceConversationDataSource } from "./workspaceConversationDataSource";

describe("workspace conversation Tauri stream adapter", () => {
  beforeEach(() => {
    mocks.listeners.clear();
    mocks.unlisten.mockClear();
    mocks.startStreamMessage.mockReset();
    mocks.cancelWorkTask.mockReset();
    mocks.createProject.mockReset();
    mocks.assignConversationProject.mockReset();
    mocks.pickAndImportResources.mockReset();
    mocks.detachResourceFromTurn.mockReset();
    mocks.listMainChatSkills.mockReset();
    mocks.selectMainChatSkill.mockReset();
    mocks.clearMainChatSkill.mockReset();
    mocks.listMainChatToolCandidates.mockReset();
    mocks.getMarkdownMemoryViewModel.mockReset();
    mocks.getConversationViewModel.mockReset();
    mocks.selectMarkdownMemoryRoot.mockReset();
    mocks.draftMarkdownMemoryFileProposal.mockReset();
    mocks.deactivateMarkdownMemoryFileProposal.mockReset();
    mocks.submitMainChatTaskSteering.mockReset();
  });

  it("keeps Markdown Memory reads, root selection, writes, and deactivation on backend bridges", async () => {
    const model = {
      roots: [],
      files: [],
      totalCharCount: 0,
      truncated: false,
      sourceRule: "exact",
    };
    mocks.getMarkdownMemoryViewModel.mockResolvedValue(model);
    mocks.selectMarkdownMemoryRoot.mockResolvedValue({
      cancelled: false,
      scope: "project",
      selectedPath: "/project",
    });
    mocks.draftMarkdownMemoryFileProposal.mockResolvedValue({ proposalId: "proposal-1" });
    mocks.deactivateMarkdownMemoryFileProposal.mockResolvedValue({ proposalId: "proposal-2" });

    await tauriWorkspaceConversationDataSource.loadMarkdownMemory?.();
    await tauriWorkspaceConversationDataSource.selectMarkdownMemoryRoot?.("project");
    await tauriWorkspaceConversationDataSource.draftMarkdownMemoryFileProposal?.({
      scope: "project",
      relativePath: "MEMORY.md",
      content: "# Project",
    });
    await tauriWorkspaceConversationDataSource.deactivateMarkdownMemoryFileProposal?.({
      scope: "project",
      relativePath: "MEMORY.md",
      expectedCurrentDigest: "sha256:current",
    });

    expect(mocks.getMarkdownMemoryViewModel).toHaveBeenCalledOnce();
    expect(mocks.selectMarkdownMemoryRoot).toHaveBeenCalledWith("project");
    expect(mocks.draftMarkdownMemoryFileProposal).toHaveBeenCalledWith({
      scope: "project",
      relativePath: "MEMORY.md",
      content: "# Project",
    });
    expect(mocks.deactivateMarkdownMemoryFileProposal).toHaveBeenCalledWith({
      scope: "project",
      relativePath: "MEMORY.md",
      expectedCurrentDigest: "sha256:current",
    });
  });

  it("forwards skill selection and tool discovery through backend-owned bridges", async () => {
    mocks.listMainChatSkills.mockResolvedValue([]);
    mocks.selectMainChatSkill.mockResolvedValue({ sessionId: "conversation-1", skillId: "review" });
    mocks.clearMainChatSkill.mockResolvedValue({ sessionId: "conversation-1", skillId: null });
    mocks.listMainChatToolCandidates.mockResolvedValue({ candidates: [], blockedCount: 0 });

    await tauriWorkspaceConversationDataSource.listSkills?.("conversation-1");
    await tauriWorkspaceConversationDataSource.selectSkill?.("conversation-1", "review");
    await tauriWorkspaceConversationDataSource.clearSkill?.("conversation-1");
    await tauriWorkspaceConversationDataSource.listToolCandidates?.("task-1");

    expect(mocks.listMainChatSkills).toHaveBeenCalledWith("conversation-1");
    expect(mocks.selectMainChatSkill).toHaveBeenCalledWith("conversation-1", "review");
    expect(mocks.clearMainChatSkill).toHaveBeenCalledWith("conversation-1");
    expect(mocks.listMainChatToolCandidates).toHaveBeenCalledWith("task-1");
  });

  it("forwards resource import and detach through the exact Tauri bridge", async () => {
    mocks.pickAndImportResources.mockResolvedValue({ cancelled: true, receipt: null });
    mocks.detachResourceFromTurn.mockResolvedValue({ bindingRemoved: true });

    await tauriWorkspaceConversationDataSource.pickResources("import-1", "turn-1");
    await tauriWorkspaceConversationDataSource.detachResource("detach-1", "turn-1", "resource-1");

    expect(mocks.pickAndImportResources).toHaveBeenCalledWith("import-1", "turn-1");
    expect(mocks.detachResourceFromTurn).toHaveBeenCalledWith("detach-1", "turn-1", "resource-1");
  });

  it("forwards only events bound to the exact conversation and operation", async () => {
    let finish!: (value: unknown) => void;
    mocks.startStreamMessage.mockImplementation(
      () =>
        new Promise(resolve => {
          finish = resolve;
        })
    );
    const onStart = vi.fn();
    const onChunk = vi.fn();
    const pending = tauriWorkspaceConversationDataSource.streamTurn(
      "conversation-1",
      [{ role: "user", content: "继续" }],
      { operationId: "operation-1" },
      { onStart, onChunk }
    );
    await vi.waitFor(() => expect(mocks.listeners.size).toBe(2));

    const start = mocks.listeners.get("stream-message-start")!;
    const chunk = mocks.listeners.get("stream-message-chunk")!;
    start({
      payload: {
        session_id: "conversation-1",
        operation_id: "another-operation",
        task_session_id: "wrong-task",
        run_id: "wrong-run",
        reasoning_trace: {},
        tool_calls: [],
      } satisfies StreamMessageStartPayload,
    });
    chunk({
      payload: {
        session_id: "another-conversation",
        operation_id: "operation-1",
        task_session_id: "wrong-task",
        run_id: "wrong-run",
        chunk: "错误回复",
      } satisfies StreamMessageChunkPayload,
    });
    expect(onStart).not.toHaveBeenCalled();
    expect(onChunk).not.toHaveBeenCalled();

    const exactStart = {
      session_id: "conversation-1",
      operation_id: "operation-1",
      task_session_id: "task-1",
      run_id: "run-1",
      reasoning_trace: {},
      tool_calls: [],
    } satisfies StreamMessageStartPayload;
    const exactChunk = {
      session_id: "conversation-1",
      operation_id: "operation-1",
      task_session_id: "task-1",
      run_id: "run-1",
      chunk: "正确回复",
    } satisfies StreamMessageChunkPayload;
    start({ payload: exactStart });
    chunk({ payload: exactChunk });
    expect(onStart).toHaveBeenCalledWith(exactStart);
    expect(onChunk).toHaveBeenCalledWith(exactChunk);

    finish({
      session_id: "conversation-1",
      operation_id: "operation-1",
      task_session_id: "task-1",
      run_id: "run-1",
      reply: "正确回复",
      status: "completed",
      reasoning_trace: {},
      tool_calls: [],
    });
    await pending;
    expect(mocks.unlisten).toHaveBeenCalledTimes(2);
  });

  it("always removes installed listeners when starting the stream fails", async () => {
    mocks.startStreamMessage.mockRejectedValue(new Error("stream_start_failed"));

    await expect(
      tauriWorkspaceConversationDataSource.streamTurn(
        "conversation-1",
        [{ role: "user", content: "继续" }],
        { operationId: "operation-2" },
        { onStart: vi.fn(), onChunk: vi.fn() }
      )
    ).rejects.toThrow("stream_start_failed");
    expect(mocks.unlisten).toHaveBeenCalledTimes(2);
  });
});
