import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

function read(path: string): string {
  return readFileSync(join(process.cwd(), path), "utf8");
}

describe("release Tauri surface", () => {
  it("runs frontend hooks inside the frontend working directory supplied by Tauri", () => {
    const config = JSON.parse(read("../src-tauri/tauri.conf.json")) as {
      build?: { beforeBuildCommand?: string; beforeDevCommand?: string };
    };

    expect(config.build?.beforeBuildCommand).toBe("corepack pnpm build");
    expect(config.build?.beforeDevCommand).toBe("corepack pnpm dev --host 127.0.0.1 --port 5173");
  });

  it("does not expose retired feedback, evolution, proactive, or dev A2A wrappers", () => {
    const releaseClient = read("src/tauri.ts");
    const browserMock = read("src/test/mocks/tauri.ts");

    for (const retiredCommand of [
      "save_feedback",
      "get_feedback_summary",
      "generate_evolution_report",
      "log_analytics_event",
      "get_proactive_suggestions",
      "generate_micro_evolution_changes",
      "calibration_create_proposals",
      "a2a_discover_agent",
      "a2a_send_task",
      "a2a_local_agent_card",
      "a2a_handle_task",
      "a2a_bridge_local",
      "a2a_restart_sidecar",
      "a2a_stop_sidecar",
    ]) {
      expect(releaseClient, retiredCommand).not.toContain(`"${retiredCommand}"`);
      expect(browserMock, retiredCommand).not.toContain(`"${retiredCommand}"`);
    }
  });
});
