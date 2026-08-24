import { beforeEach, describe, expect, it, vi } from "vitest";

const invokeMocks = vi.hoisted(() => ({ safeInvoke: vi.fn() }));

vi.mock("./invoke", () => invokeMocks);

import { reviseWorkArtifact } from "./work";

describe("Work IPC", () => {
  beforeEach(() => vi.clearAllMocks());

  it("binds a focused Artifact revision receipt to the exact generated Run", async () => {
    const runId = "11111111-1111-4111-8111-111111111111";
    const turnId = "22222222-2222-4222-8222-222222222222";
    vi.spyOn(crypto, "randomUUID").mockReturnValueOnce(runId).mockReturnValueOnce(turnId);
    invokeMocks.safeInvoke.mockImplementation(async (_command, args) => ({
      reply: "等待审核",
      status: "completed",
      blockers: [],
      run_id: args.newRunId,
    }));

    await expect(
      reviseWorkArtifact("task-1", "artifact:1", 3, "只缩短结论")
    ).resolves.toMatchObject({ run_id: runId });
    expect(invokeMocks.safeInvoke).toHaveBeenCalledWith("revise_work_artifact", {
      taskId: "task-1",
      artifactId: "artifact:1",
      baseVersion: 3,
      instruction: "只缩短结论",
      newRunId: runId,
      newTurnId: turnId,
    });
  });

  it("fails closed when the backend returns another Run identity", async () => {
    vi.spyOn(crypto, "randomUUID")
      .mockReturnValueOnce("11111111-1111-4111-8111-111111111111")
      .mockReturnValueOnce("22222222-2222-4222-8222-222222222222");
    invokeMocks.safeInvoke.mockResolvedValue({
      reply: "unexpected",
      status: "completed",
      blockers: [],
      run_id: "33333333-3333-4333-8333-333333333333",
    });

    await expect(reviseWorkArtifact("task-1", "artifact:1", 3, "只缩短结论")).rejects.toThrow(
      "artifact_revision_run_identity_mismatch"
    );
  });
});
