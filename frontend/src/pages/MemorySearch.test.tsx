import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { BrowserRouter } from "react-router-dom";
import MemorySearch from "./MemorySearch";
import { invoke } from "@tauri-apps/api/core";
import { createMockMemoryViewModelEnvelope, mockInvoke } from "@/test/mocks/tauri";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

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
    expect(screen.getByText(/后台 MemoryViewModel/)).toBeInTheDocument();
    expect(screen.getByText("ReadModel")).toBeInTheDocument();
    expect(screen.getByText("已物化记忆")).toBeInTheDocument();
    expect(screen.getByText("手动收录知识笔记")).toBeInTheDocument();
    expect(screen.getByText(/KnowledgeNote 是独立的可检索资料/)).toBeInTheDocument();
  });

  it("shows unknown instead of an empty archive when canonical lifecycle truth cannot load", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "list_archived_chunks") {
        return Promise.reject(new Error("memory_retrieval_degraded"));
      }
      return mockInvoke(cmd, args);
    });

    render(
      <BrowserRouter>
        <MemorySearch />
      </BrowserRouter>
    );

    expect(await screen.findByText("归档状态未知")).toBeInTheDocument();
    expect(screen.queryByText("暂无归档记忆")).not.toBeInTheDocument();
    expect(screen.getByTestId("archive-msg")).toHaveTextContent("memory_retrieval_degraded");
    expect(screen.getByText("ReadModel").parentElement).toHaveTextContent("ReadModelunknown");
    for (const label of [
      "已物化记忆",
      "待确认/待物化",
      "候选",
      "待审阅",
      "已确认",
      "已回滚",
      "物化失败",
      "向量总数",
      "Tier 1 热记忆",
      "Tier 2 检索记忆",
      "Tier 3 冷记忆",
      "已归档",
    ]) {
      const metric = screen.getByText(label).parentElement;
      expect(metric).toHaveTextContent("—");
      expect(metric).not.toHaveTextContent(/0/);
    }
  });

  it("does not render backend error summaries as known zero counts", async () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string, args?: Record<string, any>) => {
      const result = await mockInvoke(cmd, args);
      if (cmd === "get_memory_view_model") {
        return { ...(result as object), status: "error" };
      }
      return result;
    });

    render(
      <BrowserRouter>
        <MemorySearch />
      </BrowserRouter>
    );

    await screen.findByText("归档状态未知");
    expect(screen.queryByText("暂无归档记忆")).not.toBeInTheDocument();
    expect(screen.getByText("ReadModel").parentElement).toHaveTextContent("ReadModelunknown");
    expect(screen.getByText("已物化记忆").parentElement).toHaveTextContent("—");
    expect(screen.getByText("已归档").parentElement).toHaveTextContent("—");
  });

  it("keeps low-access metrics candidate-only instead of claiming an archive", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "archive_low_access_memories") {
        return Promise.resolve([
          {
            owner: { ownerKind: "knowledge_note", ownerId: "note-candidate-1" },
            tier: 3,
            accessCount: 1,
            importanceScore: 0.2,
            candidateOnly: true,
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

    fireEvent.click(await screen.findByRole("button", { name: "查看低访问候选" }));

    expect(await screen.findByText(/发现 1 条低访问候选.*尚未归档/)).toBeInTheDocument();
    expect(screen.getByText("knowledge_note:note-candidate-1")).toBeInTheDocument();
    expect(screen.getByText(/仅候选，未归档/)).toBeInTheDocument();
    expect(window.confirm).not.toHaveBeenCalled();
    expect(screen.queryByText(/已归档 1 条/)).not.toBeInTheDocument();
  });

  it("reuses one operation id after an unknown transport result", async () => {
    const operationIds: string[] = [];
    let attempt = 0;
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "create_knowledge_note") {
        operationIds.push(String(args?.operationId));
        attempt += 1;
        if (attempt === 1) return Promise.reject(new Error("response lost"));
        return Promise.resolve({
          operationId: args?.operationId,
          replayed: true,
          knowledgeNoteId: 42,
          outboxEventId: "outbox:42",
          canonicalCommitted: true,
          projectionState: "applied",
        });
      }
      return mockInvoke(cmd, args);
    });

    render(
      <BrowserRouter>
        <MemorySearch />
      </BrowserRouter>
    );
    const contentInput = await screen.findByPlaceholderText("输入要收录的知识笔记...");
    fireEvent.change(contentInput, { target: { value: "一次提交，多次安全重试" } });
    fireEvent.click(screen.getByRole("button", { name: "收录" }));
    expect(await screen.findByText(/尚未确认知识笔记是否提交/)).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "收录" }));
    expect(await screen.findByText("知识笔记已提交，检索索引已生效。")).toBeInTheDocument();
    expect(operationIds).toHaveLength(2);
    expect(operationIds[0]).toBe(operationIds[1]);
    expect(operationIds[0]).toMatch(/^[0-9a-f-]{36}$/);
  });

  it("keeps canonical commit truth when the read model refresh fails", async () => {
    let memoryViewModelReads = 0;
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_memory_view_model") {
        memoryViewModelReads += 1;
        if (memoryViewModelReads > 1) return Promise.reject(new Error("refresh unavailable"));
      }
      if (cmd === "create_knowledge_note") {
        return Promise.resolve({
          operationId: args?.operationId,
          replayed: false,
          knowledgeNoteId: 51,
          outboxEventId: "outbox:51",
          canonicalCommitted: true,
          projectionState: "pending",
        });
      }
      return mockInvoke(cmd, args);
    });

    render(
      <BrowserRouter>
        <MemorySearch />
      </BrowserRouter>
    );
    const contentInput = await screen.findByPlaceholderText("输入要收录的知识笔记...");
    fireEvent.change(contentInput, { target: { value: "提交事实不能被刷新错误覆盖" } });
    fireEvent.click(screen.getByRole("button", { name: "收录" }));

    expect(
      await screen.findByText(/知识笔记已提交，检索索引正在后台处理.*当前视图刷新失败/)
    ).toBeInTheDocument();
    expect(screen.queryByText(/尚未确认知识笔记是否提交/)).not.toBeInTheDocument();
    expect(contentInput).toHaveValue("");
  });

  it("keeps the newest archive generation when overlapping reads resolve out of order", async () => {
    const firstMemory = deferred<ReturnType<typeof createMockMemoryViewModelEnvelope>>();
    const secondMemory = deferred<ReturnType<typeof createMockMemoryViewModelEnvelope>>();
    const firstArchive = deferred<any[]>();
    const secondArchive = deferred<any[]>();
    let memoryReads = 0;
    let archiveReads = 0;
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_memory_view_model") {
        memoryReads += 1;
        return memoryReads === 1 ? firstMemory.promise : secondMemory.promise;
      }
      if (cmd === "list_archived_chunks") {
        archiveReads += 1;
        return archiveReads === 1 ? firstArchive.promise : secondArchive.promise;
      }
      if (cmd === "create_knowledge_note") {
        return Promise.resolve({
          operationId: args?.operationId,
          replayed: false,
          knowledgeNoteId: 72,
          outboxEventId: "outbox:72",
          canonicalCommitted: true,
          projectionState: "applied",
        });
      }
      return mockInvoke(cmd, args);
    });

    render(
      <BrowserRouter>
        <MemorySearch />
      </BrowserRouter>
    );
    await waitFor(() => expect(memoryReads).toBe(1));
    fireEvent.change(screen.getByPlaceholderText("输入要收录的知识笔记..."), {
      target: { value: "触发第二代归档读取" },
    });
    fireEvent.click(screen.getByRole("button", { name: "收录" }));
    await waitFor(() => expect(memoryReads).toBe(2));

    secondMemory.resolve(createMockMemoryViewModelEnvelope());
    secondArchive.resolve([
      {
        owner: { ownerKind: "knowledge_note", ownerId: "archive-generation-b" },
        revision: 2,
        lastEventId: "event-b",
        changedAt: new Date().toISOString(),
        canonicalDisposition: "archived",
      },
    ]);
    expect(await screen.findByText("archive-generation-b")).toBeInTheDocument();

    firstMemory.resolve(createMockMemoryViewModelEnvelope());
    firstArchive.resolve([
      {
        owner: { ownerKind: "knowledge_note", ownerId: "archive-generation-a" },
        revision: 1,
        lastEventId: "event-a",
        changedAt: new Date().toISOString(),
        canonicalDisposition: "archived",
      },
    ]);
    await waitFor(() => {
      expect(screen.queryByText("archive-generation-a")).not.toBeInTheDocument();
      expect(screen.getByText("archive-generation-b")).toBeInTheDocument();
    });
  });

  it("shows safe mode prompt and blocks indexing when diagnostics are degraded", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_system_diagnostics") {
        return Promise.resolve({
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

    expect(await screen.findByText(/Safe Mode：记忆写入操作已暂停/)).toBeInTheDocument();

    const contentInput = screen.getByPlaceholderText("输入要收录的知识笔记...");
    fireEvent.change(contentInput, { target: { value: "需要索引的记忆" } });

    expect(screen.getByRole("button", { name: "收录" })).toBeDisabled();
    expect(screen.getByText(/memory.db 初始化失败/)).toBeInTheDocument();
    expect(invoke).not.toHaveBeenCalledWith("create_knowledge_note", expect.anything());
  });

  it("blocks restoring archived memory in safe mode", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_system_diagnostics") {
        return Promise.resolve({
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
          database_status: "degraded",
          startup_warnings: ["memory.db 初始化失败，正在使用临时数据库"],
        });
      }
      if (cmd === "list_archived_chunks") {
        return Promise.resolve([
          {
            owner: { ownerKind: "knowledge_note", ownerId: "note-archived-1" },
            revision: 3,
            lastEventId: "memory-retrieval-event-3",
            changedAt: new Date().toISOString(),
            canonicalDisposition: "archived",
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

    expect(await screen.findByText(/note-archived-1/)).toBeInTheDocument();
    // 等待 Safe Mode banner 渲染完成（diagnostics 异步加载）
    expect(await screen.findByText(/Safe Mode：记忆写入操作已暂停/)).toBeInTheDocument();
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
        return Promise.resolve({
          hits: [
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
          ],
          embeddingProfile: {
            id: "embedding:hash:test:dim:384",
            route: "deterministic_hash",
            provider: "openlife",
            model: "openlife-hash-ngram-v1",
            dimension: 384,
          },
          embeddingReceipt: {
            requestId: "request-test",
            route: "deterministic_hash",
            profileId: "embedding:hash:test:dim:384",
            status: "not_attempted",
            source: "deterministic_hash",
            routeReasonCode: "configured_deterministic_hash",
            cacheHit: false,
          },
          vectorStatus: "ready",
          routeQuality: "deterministic_hash_approximation",
        });
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
    expect(screen.getByText(/本地确定性哈希近似检索/)).toBeInTheDocument();
    expect(screen.queryByText("一条低相关历史记忆，应该默认折叠。")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "显示 1 条低相关结果" }));
    expect(await screen.findByText("一条低相关历史记忆，应该默认折叠。")).toBeInTheDocument();
  });

  it("does not claim zero matches before a search has been submitted", async () => {
    render(
      <BrowserRouter>
        <MemorySearch />
      </BrowserRouter>
    );

    fireEvent.change(await screen.findByPlaceholderText("输入查询语义..."), {
      target: { value: "尚未提交" },
    });

    expect(screen.getByText("等待搜索")).toBeInTheDocument();
    expect(screen.queryByText("未找到相关记忆")).not.toBeInTheDocument();
  });

  it("distinguishes a pending search from a verified empty result", async () => {
    const pendingSearch = deferred<any>();
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "search_memory") return pendingSearch.promise;
      return mockInvoke(cmd, args);
    });
    render(
      <BrowserRouter>
        <MemorySearch />
      </BrowserRouter>
    );

    fireEvent.change(await screen.findByPlaceholderText("输入查询语义..."), {
      target: { value: "等待后端" },
    });
    fireEvent.click(screen.getByRole("button", { name: "搜索" }));
    expect(await screen.findByText("正在检索记忆")).toBeInTheDocument();
    expect(screen.queryByText("未找到相关记忆")).not.toBeInTheDocument();

    pendingSearch.resolve({
      hits: [],
      embeddingProfile: {
        id: "embedding:hash:test:dim:384",
        route: "deterministic_hash",
        provider: "openlife",
        model: "openlife-hash-ngram-v1",
        dimension: 384,
      },
      embeddingReceipt: {
        requestId: "request-empty",
        route: "deterministic_hash",
        profileId: "embedding:hash:test:dim:384",
        status: "not_attempted",
        source: "deterministic_hash",
        routeReasonCode: "configured_deterministic_hash",
        cacheHit: false,
      },
      vectorStatus: "ready",
      routeQuality: "deterministic_hash_approximation",
    });
    expect(await screen.findByText("未找到相关记忆")).toBeInTheDocument();
    expect(screen.queryByText("正在检索记忆")).not.toBeInTheDocument();
  });

  it("shows unknown rather than zero matches when a search fails", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "search_memory") return Promise.reject(new Error("search unavailable"));
      return mockInvoke(cmd, args);
    });
    render(
      <BrowserRouter>
        <MemorySearch />
      </BrowserRouter>
    );

    fireEvent.change(await screen.findByPlaceholderText("输入查询语义..."), {
      target: { value: "失败不等于零" },
    });
    fireEvent.click(screen.getByRole("button", { name: "搜索" }));

    expect(await screen.findByText("检索状态未知")).toBeInTheDocument();
    expect(screen.queryByText("未找到相关记忆")).not.toBeInTheDocument();
  });

  it("keeps backend text hits visible when embedding is degraded", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "search_memory") {
        return Promise.resolve({
          hits: [
            [
              {
                id: 9,
                session_id: "degraded-session",
                content: "关键词回退仍然命中",
                source: "explicit_memory",
                created_at: new Date().toISOString(),
              },
              0.8,
            ],
          ],
          embeddingProfile: {
            id: "unknown",
            route: "unknown",
            provider: "unknown",
            model: "unknown",
            deploymentIdentity: "unknown",
            modelArtifactIdentity: "unknown",
            dimension: 0,
          },
          embeddingReceipt: {
            requestId: "request-degraded",
            route: "ollama",
            profileId: "unknown",
            status: "failed",
            source: "ollama",
            routeReasonCode: "configured_ollama",
            cacheHit: false,
            errorDigest: "sha256:deadbeef",
          },
          vectorStatus: "embedding_failed",
          routeQuality: "unavailable",
          degradedEvidence: {
            reasonCode: "embedding_invocation_failed",
            errorDigest: "sha256:deadbeef",
          },
        });
      }
      return mockInvoke(cmd, args);
    });

    render(
      <BrowserRouter>
        <MemorySearch />
      </BrowserRouter>
    );

    fireEvent.change(await screen.findByPlaceholderText("输入查询语义..."), {
      target: { value: "关键词回退" },
    });
    fireEvent.click(screen.getByRole("button", { name: "搜索" }));

    expect(await screen.findByText("关键词回退仍然命中")).toBeInTheDocument();
    expect(screen.getByText(/Embedding 服务本次调用失败/)).toBeInTheDocument();
  });

  it("explains identity-unknown vectors without suggesting a useless rebuild", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "search_memory") {
        return Promise.resolve({
          hits: [
            [
              {
                id: 10,
                session_id: "identity-unknown-session",
                content: "关键词结果仍然可见",
                source: "manual",
                created_at: new Date().toISOString(),
              },
              0.75,
            ],
          ],
          embeddingProfile: {
            id: "unknown",
            route: "unknown",
            provider: "unknown",
            model: "unknown",
            deploymentIdentity: "unknown",
            modelArtifactIdentity: "unknown",
            dimension: 0,
          },
          embeddingReceipt: {
            requestId: "request-identity-unknown",
            route: "cloud",
            profileId: "unknown",
            status: "completed",
            source: "cloud_provider",
            routeReasonCode: "configured_cloud",
            cacheHit: false,
            providerDispatches: [{ kind: "embedding", startedAt: new Date().toISOString() }],
          },
          vectorStatus: "rebuild_required",
          routeQuality: "identity_unknown",
          degradedEvidence: {
            reasonCode: "embedding_profile_identity_unknown",
          },
        });
      }
      return mockInvoke(cmd, args);
    });

    render(
      <BrowserRouter>
        <MemorySearch />
      </BrowserRouter>
    );

    fireEvent.change(await screen.findByPlaceholderText("输入查询语义..."), {
      target: { value: "关键词结果" },
    });
    fireEvent.click(screen.getByRole("button", { name: "搜索" }));

    expect(await screen.findByText("关键词结果仍然可见")).toBeInTheDocument();
    expect(screen.getByText(/模型版本身份无法验证/)).toBeInTheDocument();
    expect(screen.getByText(/先配置可验证的模型 revision/)).toBeInTheDocument();
    expect(screen.queryByText(/恢复控制台重建索引/)).not.toBeInTheDocument();
  });
});
