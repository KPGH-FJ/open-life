import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { WorkspaceConversationController } from "./useWorkspaceConversation";
import { WorkspaceConversationPanel } from "./WorkspaceConversationPanel";

function controllerWhileMarkdownMemoryIsSubmitting(): WorkspaceConversationController {
  return {
    sessions: [],
    selectedSessionId: null,
    messages: [],
    draft: "",
    loadStatus: "ready",
    loadError: null,
    turnState: { phase: "idle" },
    streamingReply: "",
    activeTaskSessionId: null,
    sessionMutation: { phase: "idle" },
    pendingResources: [],
    pendingResourceTurnOperationId: null,
    resourceMutation: { phase: "idle" },
    skills: [],
    selectedSkillId: null,
    toolCandidates: null,
    capabilityState: { phase: "idle" },
    markdownMemory: {
      phase: "submitting",
      operation: "write",
      model: {
        roots: [
          { scope: "workspace", configured: false, rootPath: null, status: "unconfigured" },
          { scope: "project", configured: true, rootPath: "/project", status: "ready" },
        ],
        files: [
          {
            scope: "project",
            relativePath: "MEMORY.md",
            content: "# Project Memory\nKeep the verified scope exact.",
            contentDigest: "sha256:current",
            charCount: 47,
            active: true,
          },
        ],
        totalCharCount: 47,
        truncated: false,
        sourceRule: "exact roots only",
      },
    },
    busy: false,
    ensureLoaded: vi.fn(),
    reload: vi.fn().mockResolvedValue(true),
    selectSession: vi.fn(),
    startNewConversation: vi.fn(),
    setDraft: vi.fn(),
    attachResources: vi.fn().mockResolvedValue(true),
    detachResource: vi.fn().mockResolvedValue(true),
    selectSkill: vi.fn().mockResolvedValue(true),
    reloadMarkdownMemory: vi.fn().mockResolvedValue(true),
    selectMarkdownMemoryRoot: vi.fn().mockResolvedValue(true),
    proposeMarkdownMemoryWrite: vi.fn().mockResolvedValue(true),
    proposeMarkdownMemoryDeactivation: vi.fn().mockResolvedValue(true),
    sendAction: () => ({
      id: "workspace.send",
      label: "发送",
      kind: "start",
      enabled: false,
      disabledReason: "先输入要发送的内容。",
      targetRef: "workspace",
    }),
    send: vi.fn().mockResolvedValue(undefined),
    cancel: vi.fn().mockResolvedValue(undefined),
    renameSelected: vi.fn().mockResolvedValue(true),
    deleteSelected: vi.fn().mockResolvedValue(true),
  };
}

describe("WorkspaceConversationPanel Markdown Memory", () => {
  it("renders the in-flight proposal state without violating disabled-control truth", () => {
    render(
      <WorkspaceConversationPanel
        controller={controllerWhileMarkdownMemoryIsSubmitting()}
        onOpenLifeModel={vi.fn()}
      />
    );

    expect(screen.getByRole("button", { name: "处理中" })).toBeDisabled();
    expect(screen.getByText("变更正在提交到 Review；文件仍未修改。")).toBeInTheDocument();
  });

  it("shows a human-readable basis for a source-bound answer", () => {
    const controller = controllerWhileMarkdownMemoryIsSubmitting();
    controller.turnState = {
      phase: "resolved",
      sessionId: "session-source-bound",
      status: "completed",
      blockers: [],
      sourceBoundBasis: {
        factCount: 3,
        sourceTypes: ["current_message"],
        checkStatus: "semantic_support_passed",
      },
    };

    render(<WorkspaceConversationPanel controller={controller} onOpenLifeModel={vi.fn()} />);

    expect(screen.getByText("本轮按限定资料回答")).toBeInTheDocument();
    expect(screen.getByText("查看回答依据")).toBeInTheDocument();
    expect(screen.getByText("采用资料：本轮消息")).toBeInTheDocument();
    expect(screen.getByText("本轮事实块：3 条")).toBeInTheDocument();
  });

  it("does not claim an answer when the source-bound check failed closed", () => {
    const controller = controllerWhileMarkdownMemoryIsSubmitting();
    controller.turnState = {
      phase: "resolved",
      sessionId: "session-source-bound-blocked",
      status: "blocked",
      blockers: ["source_bound_claim_unsupported"],
      sourceBoundBasis: {
        factCount: 1,
        sourceTypes: ["document_or_resource"],
        checkStatus: "failed_closed",
      },
    };

    render(<WorkspaceConversationPanel controller={controller} onOpenLifeModel={vi.fn()} />);

    expect(screen.getByText("本轮限定资料边界")).toBeInTheDocument();
    expect(
      screen.getByText("OpenLife 没有展示无法在你指定资料范围内核对的回答。")
    ).toBeInTheDocument();
    expect(screen.queryByText("本轮按限定资料回答")).not.toBeInTheDocument();
  });
});
