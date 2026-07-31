import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { workbenchJourneyFixtureDataSource } from "@/test/fixtures/workbench/governedAction";
import { ReadOnlySpineJourney } from "@/ui/journeys/readOnly";
import type { ReviewAction, ReviewItem } from "@/tauri";
import { reviewDecisionFeedback } from "./ReviewGovernedView";

describe("Workbench governed action journey", () => {
  it("keeps view, approval, refresh, resume, and completion as separate states", async () => {
    const user = userEvent.setup();
    const dataSource = workbenchJourneyFixtureDataSource("fixture-ready");
    const dispatchReview = vi.spyOn(dataSource, "dispatchReviewAction");
    const resumeTask = vi.spyOn(dataSource, "resumeTask");

    render(
      <ReadOnlySpineJourney
        dataSource={dataSource}
        governedActionDataSource={dataSource}
        initialSurface="workspace"
      />
    );

    expect(
      await screen.findByRole("heading", {
        name: "整理三次客户访谈，归纳下周要验证的问题",
      })
    ).toBeInTheDocument();
    expect(screen.getByText("任务暂停在一个动作之前")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "查看权限请求" }));
    expect(
      await screen.findByRole("heading", { name: "读取本地客户访谈记录", level: 2 })
    ).toBeInTheDocument();
    expect(dispatchReview).not.toHaveBeenCalled();
    expect(screen.getAllByText("等待决定").length).toBeGreaterThan(0);

    await user.click(screen.getByRole("button", { name: "仅允许本次" }));
    expect(screen.getByRole("dialog", { name: "仅允许这一次？" })).toBeInTheDocument();
    expect(dispatchReview).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "确认仅允许本次" }));
    await waitFor(() => expect(dispatchReview).toHaveBeenCalledTimes(1));
    expect(await screen.findByText("决定已记录，尚未继续任务")).toBeInTheDocument();
    expect(screen.getAllByText("已允许一次").length).toBeGreaterThan(0);
    expect(screen.queryByText("任务已完成")).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "返回工作区" }));
    expect(await screen.findByRole("button", { name: "继续任务" })).toBeEnabled();
    expect(screen.getByText("一次性权限决定已经记录，等待你明确继续任务。")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "继续任务" }));
    expect(screen.getByRole("dialog", { name: "确认继续这项任务？" })).toBeInTheDocument();
    expect(resumeTask).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "确认继续" }));

    await waitFor(() => expect(resumeTask).toHaveBeenCalledTimes(1));
    expect(await screen.findByText("任务已继续，正在处理")).toBeInTheDocument();
    expect(screen.getByText("任务正在处理")).toBeInTheDocument();
    expect(screen.queryByText("任务已完成")).not.toBeInTheDocument();
  });

  it("keeps approval disabled when the backend permission scope is incomplete", async () => {
    const user = userEvent.setup();
    const dataSource = workbenchJourneyFixtureDataSource("fixture-incomplete-permission");
    const dispatchReview = vi.spyOn(dataSource, "dispatchReviewAction");

    render(
      <ReadOnlySpineJourney
        dataSource={dataSource}
        governedActionDataSource={dataSource}
        initialSurface="review"
      />
    );

    expect(await screen.findByText("访问范围不完整")).toBeInTheDocument();
    const approve = screen.getByRole("button", { name: "仅允许本次" });
    expect(approve).toBeDisabled();
    expect(screen.getByText("缺少目标范围和有效期；不能批准。")).toBeInTheDocument();
    await user.click(approve);
    expect(dispatchReview).not.toHaveBeenCalled();
  });

  it("fails stale governed state closed while preserving evidence access", async () => {
    const user = userEvent.setup();
    const dataSource = workbenchJourneyFixtureDataSource("fixture-stale");
    const dispatchReview = vi.spyOn(dataSource, "dispatchReviewAction");

    render(
      <ReadOnlySpineJourney
        dataSource={dataSource}
        governedActionDataSource={dataSource}
        initialSurface="review"
      />
    );

    expect(await screen.findByText("审核状态已陈旧")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "仅允许本次" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "拒绝" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "稍后处理" })).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "查看访问范围" }));
    expect(screen.getByRole("complementary", { name: "读取本地客户访谈记录" })).toBeInTheDocument();
    expect(dispatchReview).not.toHaveBeenCalled();
  });

  it("keeps review pending when the refreshed read model has not confirmed the command", async () => {
    const user = userEvent.setup();
    const fixture = workbenchJourneyFixtureDataSource("fixture-ready");
    const dataSource = {
      ...fixture,
      dispatchReviewAction: vi.fn(async (_action: ReviewAction) => undefined),
    };

    render(
      <ReadOnlySpineJourney
        dataSource={dataSource}
        governedActionDataSource={dataSource}
        initialSurface="review"
      />
    );

    await screen.findByRole("heading", { name: "读取本地客户访谈记录", level: 2 });
    await user.click(screen.getByRole("button", { name: "仅允许本次" }));
    await user.click(screen.getByRole("button", { name: "确认仅允许本次" }));

    expect(await screen.findByText("决定尚未被读模型确认")).toBeInTheDocument();
    expect(screen.getAllByText("等待决定").length).toBeGreaterThan(0);
    expect(screen.queryByText("决定已记录，尚未继续任务")).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "重新读取" }));
    await waitFor(() => expect(screen.queryByText("决定尚未被读模型确认")).not.toBeInTheDocument());
    expect(dataSource.dispatchReviewAction).toHaveBeenCalledTimes(1);
    expect(screen.getAllByText("等待决定").length).toBeGreaterThan(0);
  });

  it("keeps resume pending when the refreshed exact task is still waiting", async () => {
    const user = userEvent.setup();
    const fixture = workbenchJourneyFixtureDataSource("fixture-ready");
    const initial = await fixture.load();
    const approve = initial.reviewEnvelope.data!.items[0].allowedActions.find(
      action => action.kind === "approve"
    )!;
    await fixture.dispatchReviewAction(approve);
    const resumeTask = vi.fn(async () => undefined);
    const dataSource = {
      ...fixture,
      resumeTask,
    };

    render(
      <ReadOnlySpineJourney
        dataSource={dataSource}
        governedActionDataSource={dataSource}
        initialSurface="workspace"
      />
    );

    await screen.findByRole("button", { name: "继续任务" });
    await user.click(screen.getByRole("button", { name: "继续任务" }));
    await user.click(screen.getByRole("button", { name: "确认继续" }));

    expect(await screen.findByText("任务仍未确认继续")).toBeInTheDocument();
    expect(screen.getByText("任务暂停在一个动作之前")).toBeInTheDocument();
    expect(screen.queryByText("任务已继续，正在处理")).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "重新读取" }));
    await waitFor(() => expect(screen.queryByText("任务仍未确认继续")).not.toBeInTheDocument());
    expect(resumeTask).toHaveBeenCalledTimes(1);
    expect(screen.getByText("任务暂停在一个动作之前")).toBeInTheDocument();
  });

  it("dispatches a Tasks surface control and treats the refreshed task as the only result", async () => {
    const user = userEvent.setup();
    const dataSource = workbenchJourneyFixtureDataSource("fixture-ready");
    const initial = await dataSource.load();
    const approve = initial.reviewEnvelope.data!.items[0].allowedActions.find(
      action => action.kind === "approve"
    )!;
    await dataSource.dispatchReviewAction(approve);
    const dispatchTaskControl = vi.spyOn(dataSource, "dispatchTaskControl");

    const { container } = render(
      <ReadOnlySpineJourney
        dataSource={dataSource}
        governedActionDataSource={dataSource}
        initialSurface="tasks"
      />
    );

    await user.click(
      await screen.findByRole("button", {
        name: /整理三次客户访谈，归纳下周要验证的问题/,
      })
    );
    const resume = await screen.findByRole("button", { name: "继续任务" });
    expect(resume).toHaveAttribute("data-action-kind", "resume");
    expect(resume).toHaveAttribute("data-action-effect", "task_resume_request");
    expect(resume).toHaveAttribute("data-action-target-ref", "task-interview-notes");
    expect(resume).toHaveAttribute("data-action-completion-proof-after-dispatch", "false");

    await user.click(resume);
    const dialog = screen.getByRole("dialog", { name: "确认执行这项任务动作？" });
    expect(dispatchTaskControl).not.toHaveBeenCalled();
    await user.click(within(dialog).getByRole("button", { name: "继续任务" }));

    await waitFor(() => expect(dispatchTaskControl).toHaveBeenCalledTimes(1));
    expect(await screen.findByText("任务状态已更新")).toBeInTheDocument();
    expect(
      screen.getByText("刷新后的同一任务当前为 running；这不是完成证明。")
    ).toBeInTheDocument();
    expect(container).not.toHaveTextContent("任务已完成");
  });

  it("ignores old task and review payloads when their envelopes are empty", async () => {
    const user = userEvent.setup();
    const fixture = workbenchJourneyFixtureDataSource("fixture-ready");
    const dataSource = {
      ...fixture,
      load: async () => {
        const snapshot = await fixture.load();
        return {
          ...snapshot,
          workspaceEnvelope: { ...snapshot.workspaceEnvelope, status: "empty" as const },
          reviewEnvelope: { ...snapshot.reviewEnvelope, status: "empty" as const },
        };
      },
    };

    render(
      <ReadOnlySpineJourney
        dataSource={dataSource}
        governedActionDataSource={dataSource}
        initialSurface="workspace"
      />
    );

    expect(await screen.findByRole("heading", { name: "没有活动任务" })).toBeInTheDocument();
    expect(screen.queryByText("任务暂停在一个动作之前")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: /^审核中心\s+建议与权限决定/ }));
    expect(await screen.findByRole("heading", { name: "暂无审核项" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "仅允许本次" })).not.toBeInTheDocument();
  });

  it("allows the first conversation when the backend workspace is truthfully empty", async () => {
    const user = userEvent.setup();
    const fixture = workbenchJourneyFixtureDataSource("fixture-ready");
    const dataSource = {
      ...fixture,
      load: async () => {
        const snapshot = await fixture.load();
        return {
          ...snapshot,
          workspaceEnvelope: {
            ...snapshot.workspaceEnvelope,
            status: "empty" as const,
            data: {
              ...snapshot.workspaceEnvelope.data!,
              activeTask: undefined,
              recentTaskRefs: [],
              pendingReviewItems: [],
              activity: [],
              providerPrivacyBoundarySummary: {
                ...snapshot.workspaceEnvelope.data!.providerPrivacyBoundarySummary,
                routeType: "unknown" as const,
                externalTransmission: "unknown" as const,
                blockedReason:
                  "Network consent is required before provider dispatch (decision_id=fixture).",
              },
            },
          },
        };
      },
    };

    render(
      <ReadOnlySpineJourney
        dataSource={dataSource}
        governedActionDataSource={dataSource}
        workspaceConversationDataSource={dataSource}
        initialSurface="workspace"
      />
    );

    await user.click(await screen.findByRole("button", { name: "新对话" }));
    await user.type(screen.getByRole("textbox", { name: "消息" }), "Start the first task");

    expect(screen.getByRole("button", { name: "开始并发送" })).toBeEnabled();
  });

  it("keeps durable approval separate from refreshed application", async () => {
    const fixture = workbenchJourneyFixtureDataSource("fixture-ready");
    const snapshot = await fixture.load();
    const permissionItem = snapshot.reviewEnvelope.data!.items[0];
    const item: ReviewItem = { ...permissionItem, type: "memory_write" };
    const action = item.allowedActions.find(candidate => candidate.kind === "approve")!;

    expect(
      reviewDecisionFeedback(
        {
          phase: "resolved",
          action,
          refreshed: {
            reviewItemId: item.id,
            status: "approved",
            materializationStatus: "not_started",
          },
        },
        item
      )
    ).toMatchObject({ title: "已批准，尚未应用" });

    expect(
      reviewDecisionFeedback(
        {
          phase: "resolved",
          action,
          refreshed: {
            reviewItemId: item.id,
            status: "approved",
            materializationStatus: "applied",
          },
        },
        item
      )
    ).toMatchObject({ title: "变更已应用" });
  });

  it("does not delete a conversation until the explicit confirmation action", async () => {
    const user = userEvent.setup();
    const dataSource = workbenchJourneyFixtureDataSource("fixture-ready");
    const deleteSession = vi.spyOn(dataSource, "deleteSession");

    render(
      <ReadOnlySpineJourney
        dataSource={dataSource}
        governedActionDataSource={dataSource}
        workspaceConversationDataSource={dataSource}
        initialSurface="workspace"
      />
    );

    expect(await screen.findByRole("button", { name: "删除" })).toBeEnabled();
    await user.click(screen.getByRole("button", { name: "删除" }));
    const dialog = screen.getByRole("dialog", { name: "删除这段对话？" });
    expect(deleteSession).not.toHaveBeenCalled();

    await user.click(within(dialog).getByRole("button", { name: "确认删除" }));

    await waitFor(() => expect(deleteSession).toHaveBeenCalledTimes(1));
    expect(screen.queryByRole("dialog", { name: "删除这段对话？" })).not.toBeInTheDocument();
  });
});
