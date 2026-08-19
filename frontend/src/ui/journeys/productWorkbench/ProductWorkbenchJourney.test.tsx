import { StrictMode } from "react";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { workbenchJourneyFixtureDataSource } from "@/test/fixtures/workbench/governedAction";
import {
  createSettingsPrivacyFixture,
  providerTestReviewItemId,
} from "@/test/fixtures/workbench/settingsPrivacy";
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

    const resultsHeading = await screen.findByRole("heading", { name: "Work 进度与结果" });
    expect(resultsHeading).toBeInTheDocument();
    const inlineCheckpoint = await screen.findByRole("region", {
      name: "当前 Work 的决定节点",
    });
    expect(
      inlineCheckpoint.compareDocumentPosition(resultsHeading) & Node.DOCUMENT_POSITION_FOLLOWING
    ).toBeTruthy();
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

  it("uses the transmission boundary from the same Workbench snapshot", async () => {
    const dataSource = workbenchJourneyFixtureDataSource("fixture-ready");
    const loadBoundary = vi.fn(dataSource.loadBoundary);
    render(
      <ProductWorkbenchJourney
        dataSource={{ loadBoundary }}
        governedActionDataSource={dataSource}
      />
    );

    await waitFor(() => expect(loadBoundary).not.toHaveBeenCalled());
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

  it("reloads the aggregate Workbench snapshot when returning from settings", async () => {
    const user = userEvent.setup();
    const fixture = workbenchJourneyFixtureDataSource("fixture-ready");
    const load = vi.fn(fixture.load);
    const dataSource = { ...fixture, load };

    render(
      <ProductWorkbenchJourney
        dataSource={dataSource}
        governedActionDataSource={dataSource}
        workspaceConversationDataSource={dataSource}
        settingsPrivacyDataSource={createSettingsPrivacyFixture("fixture-ready").dataSource}
      />
    );

    await waitFor(() => expect(load.mock.calls.length).toBeGreaterThan(0));
    const callsBeforeSettingsReturn = load.mock.calls.length;
    await user.click(await screen.findByRole("button", { name: "设置" }));
    await user.click(await screen.findByRole("button", { name: "返回工作台" }));

    await waitFor(() => expect(load.mock.calls.length).toBeGreaterThan(callsBeforeSettingsReturn));
  });

  it("refreshes governed truth before opening a review created from settings", async () => {
    const user = userEvent.setup();
    const fixture = workbenchJourneyFixtureDataSource("fixture-settings-review-required");
    let providerReviewCreated = false;
    const load = vi.fn(async (...args: Parameters<typeof fixture.load>) => {
      const snapshot = await fixture.load(...args);
      if (providerReviewCreated || !snapshot.reviewEnvelope.data) return snapshot;
      return {
        ...snapshot,
        reviewEnvelope: {
          ...snapshot.reviewEnvelope,
          data: {
            ...snapshot.reviewEnvelope.data,
            batches: snapshot.reviewEnvelope.data.batches.filter(batch =>
              batch.itemIds.every(itemId => itemId !== providerTestReviewItemId)
            ),
            items: snapshot.reviewEnvelope.data.items.filter(
              item => item.id !== providerTestReviewItemId
            ),
          },
        },
      };
    });
    const testProviderConnection = vi.fn(
      async (...args: Parameters<typeof fixture.testProviderConnection>) => {
        const outcome = await fixture.testProviderConnection(...args);
        providerReviewCreated = true;
        return outcome;
      }
    );
    const dataSource = { ...fixture, load, testProviderConnection };

    render(
      <ProductWorkbenchJourney
        dataSource={dataSource}
        governedActionDataSource={dataSource}
        settingsPrivacyDataSource={dataSource}
      />
    );

    await user.click(await screen.findByRole("button", { name: "设置" }));
    await user.click(screen.getByRole("button", { name: "测试连接" }));
    await user.click(screen.getByRole("button", { name: "确认并测试" }));
    await user.click(await screen.findByRole("button", { name: "查看并决定" }));

    expect(
      await screen.findByRole("heading", { name: "允许一次模型连接测试", level: 2 })
    ).toBeInTheDocument();
    expect(load).toHaveBeenCalledTimes(2);
  });
});
