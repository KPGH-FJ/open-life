import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import {
  acceptProposal,
  draftEditMemoryProposal,
  editProposal,
  restoreArchivedMemory,
  startStreamMessage,
  importAllData,
  abandonGovernedDataImportRecovery,
  getGovernedDataImportStatus,
  describeDataImportResult,
  parseOpenLifeExportPayload,
  MAX_OPENLIFE_IMPORT_MESSAGES,
  redactInvokeArgs,
  saveConfig,
  sendMessageV2,
  pickAndImportResources,
  cancelResourceImport,
  getResourceImportStatus,
  detachResourceFromTurn,
} from "./tauri";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("canonical Tauri command arguments", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockResolvedValue(undefined);
  });

  function redactedLogForLastInvoke(): string {
    const calls = vi.mocked(invoke).mock.calls;
    const lastCall = calls[calls.length - 1];
    expect(lastCall).toBeTruthy();
    const [cmd, args] = lastCall as [string, Record<string, any> | undefined];
    return JSON.stringify(redactInvokeArgs(cmd, args));
  }

  it("runtime-validates v1 and v2 backup owner contracts", () => {
    const common = {
      exported_at: "2026-06-03T00:00:00Z",
      life_model: {},
      messages: [],
      vectors: [],
    };
    expect(parseOpenLifeExportPayload(JSON.stringify({ version: "1.0", ...common })).version).toBe(
      "1.0"
    );
    expect(
      parseOpenLifeExportPayload(
        JSON.stringify({
          version: "2.0",
          ...common,
          state_store: {
            schema: "openlife.state-store-daily-tasks-portable.v1",
            exportedAt: "2026-06-03T00:00:00Z",
            canonicalDigest: `sha256:${"1".repeat(64)}`,
            payloadDigest: `sha256:${"2".repeat(64)}`,
            skippedExpiredCount: 0,
            dailyTasks: [],
          },
        })
      ).version
    ).toBe("2.0");
    expect(() => parseOpenLifeExportPayload(JSON.stringify({ version: "2.0", ...common }))).toThrow(
      /StateStore/
    );
    expect(() =>
      parseOpenLifeExportPayload(JSON.stringify({ version: "1.0", ...common, state_store: {} }))
    ).toThrow(/v1/);
  });

  it("rejects oversized import collections before invoking the backend", () => {
    expect(() =>
      parseOpenLifeExportPayload(
        JSON.stringify({
          version: "1.0",
          exported_at: "2026-06-03T00:00:00Z",
          life_model: {},
          messages: Array.from({ length: MAX_OPENLIFE_IMPORT_MESSAGES + 1 }, () => null),
          vectors: [],
        })
      )
    ).toThrow(/消息条目导入上限/);
  });

  it("keeps recovery and degraded import outcomes truthful", () => {
    expect(
      describeDataImportResult({
        success: true,
        status: "recovery_completed_restart_required",
        legacy: false,
        warning: "metadata-safe",
        metadata_safe: true,
        durable_lifemodel_write: true,
        imported_message_count: 0,
        imported_vector_count: 0,
      })
    ).toMatch(/重启 OpenLife/);

    const degraded = describeDataImportResult({
      success: true,
      status: "projection_degraded_recovery_required",
      legacy: false,
      warning: "metadata-safe",
      metadata_safe: true,
      durable_lifemodel_write: true,
      imported_message_count: 0,
      imported_vector_count: 0,
      state_store_projection_status: "degraded",
    });
    expect(degraded).toMatch(/^导入未完成/);
    expect(degraded).not.toMatch(/导入成功|恢复完成/);

    expect(
      describeDataImportResult({
        success: false,
        status: "unexpected_partial_state",
        legacy: false,
        warning: "metadata-safe",
        metadata_safe: true,
        durable_lifemodel_write: true,
        imported_message_count: 0,
        imported_vector_count: 0,
      })
    ).toMatch(/^导入未完成/);
  });

  it("redacts send_message content from dev invoke logs", async () => {
    vi.mocked(invoke).mockResolvedValue({
      reply: "ok",
      reasoning_trace: {},
      tool_calls: [],
      run_id: "run-1",
    });

    await sendMessageV2(
      "session-secret",
      [{ role: "user", content: "我的邮箱 test@example.com 和身份证 11010519491231002X" }],
      { operationId: "c7414f1e-35dc-4aec-b2f0-f704313003a0" }
    );

    const redacted = redactedLogForLastInvoke();
    expect(redacted).toContain("session-secret");
    expect(redacted).not.toContain("test@example.com");
    expect(redacted).not.toContain("11010519491231002X");
    expect(redacted).toContain('"redacted":true');
  });

  it("redacts save_config secrets from dev invoke logs", async () => {
    await saveConfig({
      llm: {
        provider: "openai",
        openai_base: "https://api.openai.com/v1",
        openai_key: "sk-openai-secret",
        embedding_model: "text-embedding-3-small",
        chat_model: "gpt-4o-mini",
      },
      prefer_local_model: false,
      local_model: "llama3",
    });

    const redacted = redactedLogForLastInvoke();
    expect(redacted).not.toContain("sk-openai-secret");
    expect(redacted).toContain("openai_key");
    expect(redacted).toContain('"redacted":true');
  });

  it("redacts import_all_data payloads from dev invoke logs", async () => {
    vi.mocked(invoke).mockResolvedValue({
      success: true,
      legacy: false,
      governed_operation: true,
      metadata_safe: true,
      durable_lifemodel_write: true,
      imported_message_count: 1,
      imported_vector_count: 1,
    });

    await importAllData({
      version: "1.0",
      exported_at: "2026-06-03T00:00:00Z",
      life_model: { identity: { name: "张三" } } as any,
      messages: [
        {
          session_id: "session-import",
          role: "user",
          content: "导入的私密聊天原文",
          created_at: "2026-06-03T00:00:00Z",
        },
      ],
      vectors: [
        {
          session_id: "session-import",
          content: "导入的向量原文",
          embedding: [1, 2, 3],
          source: "chat",
          created_at: "2026-06-03T00:00:00Z",
          tier: 1,
          access_count: 0,
          last_accessed_at: "2026-06-03T00:00:00Z",
        },
      ],
    });

    const redacted = redactedLogForLastInvoke();
    expect(redacted).not.toContain("导入的私密聊天原文");
    expect(redacted).not.toContain("导入的向量原文");
    expect(redacted).not.toContain("张三");
    expect(redacted).toContain("payload");
    expect(redacted).toContain('"redacted":true');
  });

  it("binds governed import abandonment to the exact operation and native evidence", async () => {
    const operationId = "11111111-1111-4111-8111-222222222222";
    const evidence = {
      actionType: "data_import_abandon_recovery" as const,
      preflightId: `danger-preflight:sha256:${"b".repeat(64)}`,
      confirmationPhrase: "PRESERVE CURRENT",
      confirmationScopeDigest: `bytes:4 hash:sha256:${"a".repeat(64)}`,
      safeMode: false,
      targetIds: [operationId],
    };
    vi.mocked(invoke).mockResolvedValue({
      success: true,
      status: "abandoned_preserving_current",
      operation_id: operationId,
      stage: "abandoned_preserving_current",
      recovery_terminalized: true,
      original_import_completed: false,
      rollback_completed: false,
      preserved_current_canonical_data: true,
      abandonment_mutated_canonical_owners: false,
      original_import_effect_state: "preserved_current_observed_per_owner",
      owner_resolution_counts: { before: 1, target: 2, other: 1 },
      resolution_evidence_count: 4,
      restart_required: false,
    });

    await abandonGovernedDataImportRecovery(operationId, evidence);

    expect(invoke).toHaveBeenCalledWith("abandon_governed_data_import_recovery", {
      operationId,
      confirmationEvidence: evidence,
    });
  });

  it("reads bounded governed import status without sending payload or operation data", async () => {
    vi.mocked(invoke).mockResolvedValue({
      status: "abandoned_preserving_current",
      operationId: "11111111-1111-4111-8111-444444444444",
      stage: "abandoned_preserving_current",
      terminal: true,
      terminalAt: "2026-07-17T00:00:00Z",
      recoveryRequired: false,
      runtimeRecoveryIsolationActive: false,
      restartRequired: false,
      originalImportCompleted: false,
      rollbackCompleted: false,
      preservedCurrent: true,
      ownerCount: 4,
      resolutionEvidenceCount: 4,
      ownerResolutionCounts: { before: 1, target: 2, other: 1 },
      observedAt: "2026-07-17T00:00:01Z",
    });

    const result = await getGovernedDataImportStatus();

    expect(result.terminal).toBe(true);
    expect(result.restartRequired).toBe(false);
    expect(invoke).toHaveBeenCalledWith("get_governed_data_import_status", undefined);
  });

  it("sends canonical camelCase command arguments", async () => {
    await restoreArchivedMemory({ ownerKind: "knowledge_note", ownerId: "note-1" });
    expect(invoke).toHaveBeenCalledWith(
      "restore_archived_chunks",
      expect.objectContaining({
        owner: { ownerKind: "knowledge_note", ownerId: "note-1" },
      })
    );
  });

  it("sends one typed argument envelope for the canonical Chat stream command", async () => {
    await startStreamMessage("session-1", [{ role: "user", content: "你好" }], {
      operationId: "c7414f1e-35dc-4aec-b2f0-f704313003a1",
    });

    expect(invoke).toHaveBeenCalledWith("start_stream_message", {
      args: {
        operationId: "c7414f1e-35dc-4aec-b2f0-f704313003a1",
        sessionId: "session-1",
        messages: [{ role: "user", content: "你好" }],
        mode: "chat",
        taskId: undefined,
        runId: undefined,
      },
    });
  });

  it("passes exact durable identities to the governed resource commands", async () => {
    const importOperationId = "c7414f1e-35dc-4aec-b2f0-f704313003b1";
    const turnOperationId = "c7414f1e-35dc-4aec-b2f0-f704313003b2";
    const detachOperationId = "c7414f1e-35dc-4aec-b2f0-f704313003b3";
    const resourceId = "c7414f1e-35dc-4aec-b2f0-f704313003b4";

    await pickAndImportResources(importOperationId, turnOperationId);
    await cancelResourceImport(importOperationId);
    await getResourceImportStatus(importOperationId);
    await detachResourceFromTurn(detachOperationId, turnOperationId, resourceId);

    expect(invoke).toHaveBeenCalledWith("pick_and_import_resources", {
      importOperationId,
      turnOperationId,
    });
    expect(invoke).toHaveBeenCalledWith("cancel_resource_import", {
      operationId: importOperationId,
    });
    expect(invoke).toHaveBeenCalledWith("get_resource_import_status", {
      operationId: importOperationId,
    });
    expect(invoke).toHaveBeenCalledWith("detach_resource_from_turn", {
      operationId: detachOperationId,
      turnOperationId,
      resourceId,
    });
  });

  it("passes the selected skill through canonical chat command wrappers", async () => {
    vi.mocked(invoke).mockResolvedValue({
      reply: "ok",
      reasoning_trace: {},
      tool_calls: [],
    });

    await sendMessageV2("session-skill", [{ role: "user", content: "Summarize this" }], {
      operationId: "c7414f1e-35dc-4aec-b2f0-f704313003a2",
      selectedSkillId: "summarize",
    });
    await startStreamMessage("session-skill", [{ role: "user", content: "Summarize this" }], {
      operationId: "c7414f1e-35dc-4aec-b2f0-f704313003a3",
      selectedSkillId: "summarize",
    });

    expect(invoke).toHaveBeenCalledWith(
      "send_message",
      expect.objectContaining({
        operationId: "c7414f1e-35dc-4aec-b2f0-f704313003a2",
        selectedSkillId: "summarize",
      })
    );
    expect(invoke).toHaveBeenCalledWith("start_stream_message", {
      args: expect.objectContaining({
        operationId: "c7414f1e-35dc-4aec-b2f0-f704313003a3",
        selectedSkillId: "summarize",
      }),
    });
  });

  it("sends the governed import request envelope", async () => {
    vi.mocked(invoke).mockClear();
    vi.mocked(invoke).mockResolvedValue({
      success: true,
      legacy: false,
      governed_operation: true,
      warning: "metadata-safe",
      metadata_safe: true,
      durable_lifemodel_write: true,
      imported_message_count: 0,
      imported_vector_count: 0,
    });
    await importAllData(
      {
        version: "2.0",
        exported_at: "2026-06-03T00:00:00Z",
        life_model: {} as any,
        messages: [],
        vectors: [],
        state_store: {
          schema: "openlife.state-store-daily-tasks-portable.v1",
          exportedAt: "2026-06-03T00:00:00Z",
          canonicalDigest: `sha256:${"1".repeat(64)}`,
          payloadDigest: `sha256:${"2".repeat(64)}`,
          skippedExpiredCount: 0,
          dailyTasks: [],
        },
      },
      undefined,
      "11111111-1111-4111-8111-111111111111"
    );

    expect(invoke).toHaveBeenCalledWith("import_all_data", {
      payload: expect.objectContaining({
        version: "2.0",
        messages: [],
        vectors: [],
      }),
      importRequest: {
        operationId: "11111111-1111-4111-8111-111111111111",
        purpose: "manual_restore",
        explicitUserIntent: true,
        createPreChangeSnapshot: true,
        importTargets: ["life_model", "messages", "vectors", "state_store"],
      },
    });
  });

  it("sends proposal commands without compatibility aliases", async () => {
    await acceptProposal("proposal-1");
    await editProposal("proposal-1", { name: "新值" });
    await draftEditMemoryProposal("proposal-memory-1", { content: "draft" });

    expect(invoke).toHaveBeenCalledWith("accept_proposal", {
      proposalId: "proposal-1",
    });
    expect(invoke).toHaveBeenCalledWith("edit_proposal", {
      proposalId: "proposal-1",
      newAfter: { name: "新值" },
    });
    expect(invoke).toHaveBeenCalledWith("draft_edit_memory_proposal", {
      proposalId: "proposal-memory-1",
      newAfter: { content: "draft" },
    });
  });
});
