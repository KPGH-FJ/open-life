import { StrictMode } from "react";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { workbenchJourneyFixtureDataSource } from "@/test/fixtures/workbench/governedAction";
import { createSettingsPrivacyFixture } from "@/test/fixtures/workbench/settingsPrivacy";
import { ProductWorkbenchJourney } from "./ProductWorkbenchJourney";

describe("OpenLife product shell", () => {
  it("uses Workbench as the single task surface and removes retired top-level pages", async () => {
    const dataSource = workbenchJourneyFixtureDataSource("fixture-ready");
    render(
      <ProductWorkbenchJourney
        dataSource={dataSource}
        governedActionDataSource={dataSource}
        workspaceConversationDataSource={dataSource}
      />
    );

    expect(await screen.findByRole("heading", { name: "工作区", level: 1 })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^Workbench/ })).toHaveAttribute(
      "aria-current",
      "page"
    );
    expect(screen.queryByRole("button", { name: /^结果/ })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /^需处理/ })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /^今日/ })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /^任务/ })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /^审核中心/ })).not.toBeInTheDocument();

    expect(await screen.findByRole("heading", { name: "Work 进度与结果" })).toBeInTheDocument();
    expect(await screen.findByTestId("canonical-work-contract")).toHaveTextContent("交付最终回答");
    expect(
      await screen.findByRole("heading", { name: "读取本地客户访谈记录", level: 2 })
    ).toBeInTheDocument();
  });

  it("opens a LifeModel checkpoint inline without restoring a separate Review page", async () => {
    const user = userEvent.setup();
    const dataSource = workbenchJourneyFixtureDataSource("fixture-ready");
    render(
      <ProductWorkbenchJourney
        dataSource={dataSource}
        governedActionDataSource={dataSource}
        durableTruthDataSource={dataSource}
      />
    );

    await user.click(await screen.findByRole("button", { name: /^个人智能/ }));
    await user.click(await screen.findByRole("button", { name: "查看并决定" }));

    expect(
      await screen.findByRole("heading", { name: "把上午作为优先深度工作时段", level: 2 })
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^Workbench/ })).toHaveAttribute(
      "aria-current",
      "page"
    );
    expect(screen.queryByRole("button", { name: /^审核中心/ })).not.toBeInTheDocument();
  });

  it("loads transmission boundary independently of Workbench lifecycle health", async () => {
    const dataSource = workbenchJourneyFixtureDataSource("fixture-ready");
    const loadBoundary = vi.fn(dataSource.loadBoundary);
    render(
      <ProductWorkbenchJourney
        dataSource={{ loadBoundary }}
        governedActionDataSource={dataSource}
      />
    );

    await waitFor(() => expect(loadBoundary).toHaveBeenCalledTimes(1));
    expect(await screen.findByText("本地路由，未外传")).toBeInTheDocument();
  });

  it("shows canonical blocked Work in Needs Attention even when Review is empty", async () => {
    const fixture = workbenchJourneyFixtureDataSource("fixture-ready");
    const dataSource = {
      ...fixture,
      load: async () => {
        const snapshot = await fixture.load();
        const blockedTask = {
          ...snapshot.tasksEnvelope.data!.items[0],
          canonicalTaskId: "task:web-blocked",
          title: "查询官网标题",
          lifecycleStatus: "blocked" as const,
          terminalDeliveryStatus: "blocked" as const,
          finalDeliveryEvidencePresent: false,
          needsAttention: true,
          attentionReasonCodes: ["read_tool_blocked"],
          pendingBlockers: ["read_tool_blocked"],
          pendingReviewItemRefs: [],
        };
        return {
          ...snapshot,
          tasksEnvelope: {
            ...snapshot.tasksEnvelope,
            status: "ready" as const,
            data: {
              ...snapshot.tasksEnvelope.data!,
              items: [blockedTask],
              summary: {
                ...snapshot.tasksEnvelope.data!.summary,
                total: 1,
                blockedCount: 1,
                waitingPermissionCount: 0,
                waitingReviewCount: 0,
                pendingReviewCount: 0,
              },
            },
          },
          reviewEnvelope: {
            ...snapshot.reviewEnvelope,
            status: "empty" as const,
            data: { ...snapshot.reviewEnvelope.data!, items: [] },
          },
          workspaceEnvelope: {
            ...snapshot.workspaceEnvelope,
            data: {
              ...snapshot.workspaceEnvelope.data!,
              tasks: [blockedTask],
              activeTask: blockedTask,
              pendingReviewItems: [],
            },
          },
        };
      },
    };

    render(
      <ProductWorkbenchJourney
        dataSource={dataSource}
        governedActionDataSource={dataSource}
        workspaceConversationDataSource={dataSource}
      />
    );

    expect(
      await screen.findByRole("button", {
        name: /查询官网标题.*需要处理：所需资料当前不可访问/,
      })
    ).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Work 进度与结果" })).toBeInTheDocument();
    expect(screen.queryByText("当前没有可展示的任务。")).not.toBeInTheDocument();
  });

  it("shows only implemented settings categories", async () => {
    const user = userEvent.setup();
    const dataSource = workbenchJourneyFixtureDataSource("fixture-ready");
    const settings = createSettingsPrivacyFixture("fixture-ready");
    render(
      <StrictMode>
        <ProductWorkbenchJourney
          dataSource={dataSource}
          governedActionDataSource={dataSource}
          settingsPrivacyDataSource={settings.dataSource}
        />
      </StrictMode>
    );

    await user.click(await screen.findByRole("button", { name: "设置" }));
    expect(screen.getByText("共 3 个设置分类")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^模型与供应商/ })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^隐私与网络/ })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^产品诊断/ })).toBeInTheDocument();
    expect(screen.queryByText("通知")).not.toBeInTheDocument();
    expect(screen.queryByText("账户")).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /^产品诊断/ }));
    expect(screen.getByRole("heading", { name: "产品诊断", level: 1 })).toBeInTheDocument();
  });
});
