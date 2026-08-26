import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { TaskViewModelItem } from "@/tauri";
import { GlobalActivityView } from "./GlobalActivityView";

function task(overrides: Partial<TaskViewModelItem> = {}): TaskViewModelItem {
  return {
    canonicalTaskId: "task-global-1",
    relatedRunIds: ["run-global-1"],
    conversationId: "conversation-global-1",
    title: "跨对话研究任务",
    lifecycleStatus: "waiting_permission",
    terminalDeliveryStatus: "not_terminal",
    finalDeliveryEvidencePresent: false,
    completionLimitations: [],
    items: [],
    artifacts: [],
    pendingBlockers: [],
    pendingReviewItemRefs: [{ id: "review-global-1", kind: "review_item", label: "精确权限决定" }],
    allowedControls: [],
    nextRecommendedControl: "open_review_item",
    evidenceRefs: [],
    updatedAt: "2026-08-24T08:00:00Z",
    ...overrides,
  };
}

describe("GlobalActivityView", () => {
  it("shows durable task states across conversations and opens the exact task", async () => {
    const user = userEvent.setup();
    const onOpenTask = vi.fn();
    const waiting = task();
    const interrupted = task({
      canonicalTaskId: "task-global-2",
      conversationId: "conversation-global-2",
      title: "重启后中断的任务",
      lifecycleStatus: "interrupted",
      pendingReviewItemRefs: [],
      allowedControls: [
        {
          id: "task-global-2:resume",
          label: "继续",
          kind: "resume",
          effect: "task_resume_request",
          enabled: true,
          requiresConfirmation: false,
          targetTaskId: "task-global-2",
          targetActionId: "run-global-2",
          completionProofAfterDispatch: false,
        },
      ],
    });

    render(
      <GlobalActivityView
        items={[waiting, interrupted]}
        selectedTaskId={null}
        onOpenTask={onOpenTask}
      />
    );

    const summary = screen.getByText("全部活动").closest("summary");
    const details = summary?.closest("details");
    expect(summary).toHaveTextContent("2 项需要处理");
    expect(details).not.toHaveAttribute("open");

    await user.click(summary!);
    expect(details).toHaveAttribute("open");
    expect(screen.getByRole("button", { name: /跨对话研究任务/ })).toHaveTextContent(
      "1 个决定节点等待处理"
    );
    expect(screen.getByRole("button", { name: /重启后中断的任务/ })).toHaveTextContent(
      "原运行已终止，可以创建新运行继续"
    );

    await user.click(screen.getByRole("button", { name: /跨对话研究任务/ }));
    expect(onOpenTask).toHaveBeenCalledWith(waiting);
  });
});
