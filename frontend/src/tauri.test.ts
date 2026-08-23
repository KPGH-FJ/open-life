import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import {
  acceptProposal,
  archiveMemory,
  startStreamMessage,
  redactInvokeArgs,
  saveConfig,
  pickAndImportResources,
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

  it("redacts stream message content and identifiers from dev invoke logs", async () => {
    vi.mocked(invoke).mockResolvedValue({
      reply: "ok",
      run_id: "run-1",
    });

    await startStreamMessage(
      "session-secret",
      [{ role: "user", content: "我的邮箱 test@example.com 和身份证 11010519491231002X" }],
      { operationId: "c7414f1e-35dc-4aec-b2f0-f704313003a0" }
    );

    const redacted = redactedLogForLastInvoke();
    expect(redacted).not.toContain("session-secret");
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

  it("sends canonical camelCase command arguments", async () => {
    await archiveMemory("memory:note-1");
    expect(invoke).toHaveBeenCalledWith(
      "archive_memory",
      expect.objectContaining({
        memoryId: "memory:note-1",
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
    await detachResourceFromTurn(detachOperationId, turnOperationId, resourceId);

    expect(invoke).toHaveBeenCalledWith("pick_and_import_resources", {
      importOperationId,
      turnOperationId,
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
    });

    await startStreamMessage("session-skill", [{ role: "user", content: "Summarize this" }], {
      operationId: "c7414f1e-35dc-4aec-b2f0-f704313003a3",
      selectedSkillId: "summarize",
    });

    expect(invoke).toHaveBeenCalledWith("start_stream_message", {
      args: expect.objectContaining({
        operationId: "c7414f1e-35dc-4aec-b2f0-f704313003a3",
        selectedSkillId: "summarize",
      }),
    });
  });

  it("sends the canonical proposal decision command", async () => {
    await acceptProposal("proposal-1");

    expect(invoke).toHaveBeenCalledWith("accept_proposal", {
      proposalId: "proposal-1",
    });
  });
});
