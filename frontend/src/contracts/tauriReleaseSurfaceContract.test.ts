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
      "get_state_alerts",
      "get_state_history",
    ]) {
      expect(releaseClient, retiredCommand).not.toContain(`"${retiredCommand}"`);
    }
  });

  it("exposes the governed legacy LifeModel migration command", () => {
    expect(read("src/ipc/personalIntelligence.ts")).toContain(
      'safeInvoke("draft_legacy_lifemodel_migration", { request })'
    );
  });

  it("creates Project scope only through the native directory picker", () => {
    const conversationIpc = read("src/ipc/conversation.ts");
    expect(conversationIpc).toContain(
      'safeInvoke<ProjectDirectoryCreationResult>("create_project_from_directory"'
    );
    expect(conversationIpc).toContain(
      'safeInvoke<ProjectDirectoryCreationResult>("bind_project_directory"'
    );
    expect(conversationIpc).not.toContain('safeInvoke<ProjectRecord>("create_project"');
  });

  it("exposes revision-bound Project lifecycle controls", () => {
    const conversationIpc = read("src/ipc/conversation.ts");
    for (const command of [
      "update_project_name",
      "archive_project",
      "restore_project",
      "delete_project",
      "select_new_conversation_project",
    ]) {
      expect(conversationIpc).toContain(`"${command}"`);
    }
  });

  it("exposes canonical tool permission inspection and revocation", () => {
    const settingsIpc = read("src/ipc/settings.ts");
    expect(settingsIpc).toContain('"get_tool_permission_view_model"');
    expect(settingsIpc).toContain('"revoke_tool_permission"');
  });

  it("exposes the exact Artifact-bound focused revision command", () => {
    const workIpc = read("src/ipc/work.ts");
    expect(workIpc).toContain('safeInvoke<SendMessageResult>("revise_work_artifact"');
    for (const field of [
      "taskId",
      "artifactId",
      "baseVersion",
      "instruction",
      "newRunId",
      "newTurnId",
    ]) {
      expect(workIpc).toContain(field);
    }
    expect(workIpc).toContain("receipt.run_id !== newRunId");
  });
});
