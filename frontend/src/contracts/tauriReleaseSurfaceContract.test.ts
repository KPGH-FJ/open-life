import { readFileSync, readdirSync } from "node:fs";
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

  it("does not expose retired lifecycle, feedback, evolution, or proactive wrappers", () => {
    const releaseClient = [
      read("src/tauri.ts"),
      ...readdirSync(join(process.cwd(), "src/ipc"))
        .filter(path => path.endsWith(".ts") && !path.endsWith(".test.ts"))
        .map(path => read(`src/ipc/${path}`)),
    ].join("\n");

    for (const retiredCommand of [
      "create_plan_execute_session",
      "get_plan_execute_session",
      "list_plan_execute_sessions",
      "update_plan_execute_session_draft",
      "finalize_plan_execute_session",
      "cancel_plan_execute_session",
      "review_plan_execute_session",
      "execute_plan_execute_step",
      "skip_plan_execute_step",
      "save_feedback",
      "get_feedback_summary",
      "generate_evolution_report",
      "log_analytics_event",
      "get_proactive_suggestions",
      "generate_micro_evolution_changes",
      "calibration_create_proposals",
      "select_markdown_memory_root",
      "get_markdown_memory_view_model",
      "draft_markdown_memory_file_proposal",
      "deactivate_markdown_memory_file_proposal",
      "draft_memory_stop_recall_proposal",
      "draft_memory_correction_proposal",
      "draft_memory_archive_proposal",
      "restore_archived_chunks",
      "draft_legacy_lifemodel_migration",
      "get_state_alerts",
      "get_state_history",
    ]) {
      expect(releaseClient, retiredCommand).not.toContain(`"${retiredCommand}"`);
    }
  });
});
