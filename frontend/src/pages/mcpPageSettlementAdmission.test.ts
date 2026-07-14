import { describe, expect, it, vi } from "vitest";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import {
  decideMcpPageSettlementAdmission,
  settleMcpPageRead,
  type McpPageSettlementAdmissionState,
} from "./mcpPageSettlementAdmission";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

describe("MCP page settlement admission", () => {
  it("admits only the current generation of an active lifecycle", () => {
    expect(
      decideMcpPageSettlementAdmission({ lifecycle: "active", currentGeneration: 4 }, 4)
    ).toEqual({ admitted: true, reason: "current_generation" });
    expect(
      decideMcpPageSettlementAdmission({ lifecycle: "active", currentGeneration: 4 }, 3)
    ).toEqual({ admitted: false, reason: "stale_generation" });
    expect(
      decideMcpPageSettlementAdmission({ lifecycle: "inactive", currentGeneration: 4 }, 4)
    ).toEqual({ admitted: false, reason: "inactive_lifecycle" });
  });

  it("keeps both independent reads and lifecycle cleanup wired to the admission authority", () => {
    const pageSource = readFileSync(resolve(process.cwd(), "src/pages/McpPage.tsx"), "utf8");

    expect(pageSource.match(/settleMcpPageRead\s*\(/g)).toHaveLength(2);
    expect(pageSource).toContain(
      "decideMcpPageSettlementAdmission(settlementAdmissionRef.current, generation)"
    );
    expect(pageSource).toContain('settlementAdmissionRef.current.lifecycle = "inactive"');
    expect(pageSource).not.toContain("mountedRef");
    expect(pageSource).not.toContain("loadGenerationRef");
  });

  it("returns stale_generation and suppresses a late fulfilled callback", async () => {
    const read = deferred<string>();
    const state: McpPageSettlementAdmissionState = {
      lifecycle: "active",
      currentGeneration: 1,
    };
    const onFulfilled = vi.fn();
    const onRejected = vi.fn();
    const settlement = settleMcpPageRead(read.promise, () => state, 1, onFulfilled, onRejected);

    state.currentGeneration = 2;
    read.resolve("stale");

    await expect(settlement).resolves.toEqual({
      admitted: false,
      reason: "stale_generation",
      settlement: "fulfilled",
    });
    expect(onFulfilled).not.toHaveBeenCalled();
    expect(onRejected).not.toHaveBeenCalled();
  });

  it("returns inactive_lifecycle and suppresses a late rejected callback", async () => {
    const read = deferred<string>();
    const state: McpPageSettlementAdmissionState = {
      lifecycle: "active",
      currentGeneration: 1,
    };
    const onFulfilled = vi.fn();
    const onRejected = vi.fn();
    const settlement = settleMcpPageRead(read.promise, () => state, 1, onFulfilled, onRejected);

    state.lifecycle = "inactive";
    read.reject(new Error("late failure"));

    await expect(settlement).resolves.toEqual({
      admitted: false,
      reason: "inactive_lifecycle",
      settlement: "rejected",
    });
    expect(onFulfilled).not.toHaveBeenCalled();
    expect(onRejected).not.toHaveBeenCalled();
  });

  it("reports admitted fulfillment and rejection only after invoking the matching callback", async () => {
    const state: McpPageSettlementAdmissionState = {
      lifecycle: "active",
      currentGeneration: 7,
    };
    const onFulfilled = vi.fn();
    const onRejected = vi.fn();

    await expect(
      settleMcpPageRead(Promise.resolve("current"), () => state, 7, onFulfilled, onRejected)
    ).resolves.toEqual({
      admitted: true,
      reason: "current_generation",
      settlement: "fulfilled",
    });
    expect(onFulfilled).toHaveBeenCalledWith("current");
    expect(onRejected).not.toHaveBeenCalled();

    const failure = new Error("current failure");
    await expect(
      settleMcpPageRead(Promise.reject(failure), () => state, 7, onFulfilled, onRejected)
    ).resolves.toEqual({
      admitted: true,
      reason: "current_generation",
      settlement: "rejected",
    });
    expect(onRejected).toHaveBeenCalledWith(failure);
  });
});
