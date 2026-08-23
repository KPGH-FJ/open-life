import { StrictMode } from "react";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { TaskControl, TaskViewModelItem } from "@/tauri";
import { workbenchFixtureDataSource } from "@/test/fixtures/workbench/workbench";
import {
  createSettingsFixture,
  providerTestReviewItemId,
} from "@/test/fixtures/workbench/settings";
import { ProductWorkbench } from "./ProductWorkbench";

describe("OpenLife product shell", () => {
  it("uses Workbench as the single task surface and removes retired top-level pages", async () => {
    const dataSource = workbenchFixtureDataSource("fixture-ready");
    render(
      <ProductWorkbench workbenchDataSource={dataSource} conversationDataSource={dataSource} />
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

    const resultsHeading = await screen.findByRole("heading", { name: "进度与结果" });
    expect(resultsHeading).toBeInTheDocument();
    expect(screen.getByTestId("conversation-workbench")).toHaveClass(
      "ol-conversation-workbench-layout--with-results"
    );
    const inlineCheckpoint = await screen.findByRole("region", {
      name: "当前 Work 的决定节点",
    });
    expect(screen.queryByLabelText("审核项列表")).not.toBeInTheDocument();
    expect(
      inlineCheckpoint.compareDocumentPosition(resultsHeading) & Node.DOCUMENT_POSITION_FOLLOWING
    ).toBeTruthy();
    expect(await screen.findByTestId("canonical-work-contract")).toHaveTextContent("交付最终回答");
    expect(
      await screen.findByRole("heading", { name: "读取本地客户访谈记录", level: 2 })
    ).toBeInTheDocument();
  });

  it("shows a completed answer Work in Results even when it has no file artifact", async () => {
    const fixture = workbenchFixtureDataSource("fixture-ready");
    const dataSource = {
      ...fixture,
      load: async () => {
        const snapshot = await fixture.load();
        const baseTask = snapshot.tasksEnvelope.data!.items[0];
        const answerTask = {
          ...baseTask,
          lifecycleStatus: "completed" as const,
          terminalDeliveryStatus: "delivered" as const,
          finalDeliveryEvidencePresent: true,
          artifacts: [],
          pendingBlockers: [],
          pendingReviewItemRefs: [],
          allowedControls: [],
          latestResultPreview: {
            status: "delivered" as const,
            label: "最终回答已交付",
            preview: "STAGE6-PROJECT-ALPHA-SCOPED 已完成内部复核。",
            evidenceRefs: baseTask.evidenceRefs,
          },
        };
        return {
          ...snapshot,
          tasksEnvelope: {
            ...snapshot.tasksEnvelope,
            data: {
              ...snapshot.tasksEnvelope.data!,
              items: [answerTask],
            },
          },
        };
      },
    };

    render(
      <ProductWorkbench workbenchDataSource={dataSource} conversationDataSource={dataSource} />
    );

    expect(await screen.findByTestId("canonical-task-answer")).toHaveTextContent(
      "STAGE6-PROJECT-ALPHA-SCOPED 已完成内部复核。"
    );
    expect(screen.queryByText("这项工作还没有可交付的结果。")).not.toBeInTheDocument();
  });

  it("does not present a disclosed evidence limitation as fully verified completion", async () => {
    const fixture = workbenchFixtureDataSource("fixture-ready");
    const dataSource = {
      ...fixture,
      load: async () => {
        const snapshot = await fixture.load();
        const baseTask = snapshot.tasksEnvelope.data!.items[0];
        return {
          ...snapshot,
          tasksEnvelope: {
            ...snapshot.tasksEnvelope,
            data: {
              ...snapshot.tasksEnvelope.data!,
              items: [
                {
                  ...baseTask,
                  lifecycleStatus: "completed" as const,
                  terminalDeliveryStatus: "delivered" as const,
                  finalDeliveryEvidencePresent: true,
                  completionDisposition: "complete_with_disclosed_limitations" as const,
                  artifacts: [],
                  pendingBlockers: [],
                  pendingReviewItemRefs: [],
                  allowedControls: [],
                  latestResultPreview: {
                    status: "delivered" as const,
                    label: "最终回答已交付",
                    preview: "已明确说明仍缺少一项直接来源。",
                    evidenceRefs: baseTask.evidenceRefs,
                  },
                },
              ],
            },
          },
        };
      },
    };

    render(
      <ProductWorkbench workbenchDataSource={dataSource} conversationDataSource={dataSource} />
    );

    expect(await screen.findByText("最终回答已交付，含已说明限制")).toBeInTheDocument();
    expect(screen.getByText("已交付，含限制")).toBeInTheDocument();
    expect(screen.queryByText(/^已完成$/)).not.toBeInTheDocument();
  });

  it("keeps Results pinned to the exact retry target after refreshed task ordering changes", async () => {
    const user = userEvent.setup();
    const fixture = workbenchFixtureDataSource("fixture-ready");
    const initial = await fixture.load();
    const baseTask = initial.tasksEnvelope.data!.items[0];
    const targetTaskId = "task:retry-exact-target";
    const retryControl: TaskControl = {
      id: `${targetTaskId}:retry`,
      label: "重试失败步骤",
      kind: "retry",
      effect: "task_retry_request",
      enabled: true,
      requiresConfirmation: false,
      targetTaskId,
      targetActionId: "failed-web-artifact",
      completionProofAfterDispatch: false,
    };
    let retried = false;
    const dataSource = {
      ...fixture,
      async load() {
        const snapshot = await fixture.load();
        const target: TaskViewModelItem = {
          ...baseTask,
          canonicalTaskId: targetTaskId,
          title: "比较 ChatGPT Work 和 Codex",
          lifecycleStatus: retried ? ("completed" as const) : ("failed" as const),
          terminalDeliveryStatus: retried ? ("delivered" as const) : ("failed" as const),
          finalDeliveryEvidencePresent: retried,
          pendingBlockers: retried ? [] : ["web_artifact_citation_not_in_body"],
          allowedControls: retried ? [] : [retryControl],
          latestResultPreview: {
            status: retried ? ("delivered" as const) : ("failed" as const),
            label: retried ? "最终回答已交付" : "failed",
            preview: retried ? "精确重试目标已经完成。" : "正文来源尚未绑定。",
            evidenceRefs: baseTask.latestResultPreview?.evidenceRefs ?? baseTask.evidenceRefs,
          },
        };
        const older: TaskViewModelItem = {
          ...baseTask,
          canonicalTaskId: "task:older-same-title",
          title: "比较 ChatGPT Work 和 Codex",
          lifecycleStatus: retried ? ("blocked" as const) : ("completed" as const),
          terminalDeliveryStatus: retried ? ("blocked" as const) : ("delivered" as const),
          finalDeliveryEvidencePresent: !retried,
          pendingBlockers: retried ? ["read_tool_blocked"] : [],
          allowedControls: [],
        };
        return {
          ...snapshot,
          tasksEnvelope: {
            ...snapshot.tasksEnvelope,
            data: { ...snapshot.tasksEnvelope.data!, items: [target, older] },
          },
        };
      },
      async dispatchTaskControl(control: TaskControl) {
        expect(control.targetTaskId).toBe(targetTaskId);
        retried = true;
      },
    };

    render(<ProductWorkbench workbenchDataSource={dataSource} />);

    expect(await screen.findByRole("heading", { name: "失败", level: 4 })).toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "failed" })).not.toBeInTheDocument();
    await user.click(await screen.findByRole("button", { name: "重试失败步骤" }));
    await waitFor(() =>
      expect(screen.getByTestId("canonical-task-answer")).toHaveTextContent(
        "精确重试目标已经完成。"
      )
    );
    const targetRow = screen.getAllByRole("button", {
      name: /比较 ChatGPT Work 和 Codex/,
    })[0];
    expect(targetRow).toHaveAttribute("aria-pressed", "true");
  });

  it("opens or exports only a backend-verified Artifact from the result card", async () => {
    const fixture = workbenchFixtureDataSource("fixture-ready");
    const snapshot = await fixture.load();
    const baseTask = snapshot.tasksEnvelope.data!.items[0];
    const artifactTask = {
      ...baseTask,
      lifecycleStatus: "completed" as const,
      terminalDeliveryStatus: "delivered" as const,
      finalDeliveryEvidencePresent: true,
      pendingBlockers: [],
      pendingReviewItemRefs: [],
      artifacts: [
        {
          artifactId: "artifact:travel-checklist",
          version: 1,
          status: "materialized" as const,
          mediaType: "text/markdown; charset=utf-8",
          contentDigest: "sha256:travel-checklist-v1",
          targetReferenceDigest: "sha256:travel-checklist-target",
          materializedReference: "/OpenLife/Results/travel-checklist.md",
          observedContentDigest: "sha256:travel-checklist-v1",
          sourceItemRef: {
            id: "item:travel-checklist-delivery",
            kind: "evidence" as const,
            label: "清单产物草稿",
          },
          evidenceRefs: baseTask.evidenceRefs,
          change: {
            kind: "create" as const,
            status: "materialized" as const,
            targetReference: "/OpenLife/Results/travel-checklist.md",
          },
          preview: { status: "available" as const, content: "# 周末出行清单" },
          verification: {
            status: "verified" as const,
            expectedContentDigest: "sha256:travel-checklist-v1",
            observedContentDigest: "sha256:travel-checklist-v1",
            verificationItemPresent: true,
          },
          undo: { available: false, reasonCode: "not_requested" },
        },
      ],
    };
    const openArtifactResult = vi.fn().mockResolvedValue(undefined);
    const exportArtifactResult = vi.fn().mockResolvedValue("/tmp/travel-checklist.md");
    const dataSource = {
      ...fixture,
      openArtifactResult,
      exportArtifactResult,
      async load() {
        const current = await fixture.load();
        return {
          ...current,
          tasksEnvelope: {
            ...current.tasksEnvelope,
            data: { ...current.tasksEnvelope.data!, items: [artifactTask] },
          },
        };
      },
    };
    const user = userEvent.setup();

    render(
      <ProductWorkbench workbenchDataSource={dataSource} conversationDataSource={dataSource} />
    );

    await user.click(await screen.findByRole("button", { name: "打开文件" }));
    expect(screen.getByText("文件完整性已核验")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "另存为…" }));

    await waitFor(() => {
      expect(openArtifactResult).toHaveBeenCalledWith("artifact:travel-checklist", 1);
      expect(exportArtifactResult).toHaveBeenCalledWith("artifact:travel-checklist", 1);
    });
  });

  it("renders a materialized Artifact while verification is still pending", async () => {
    const fixture = workbenchFixtureDataSource("fixture-ready");
    const snapshot = await fixture.load();
    const baseTask = snapshot.tasksEnvelope.data!.items[0];
    const materializedPendingTask = {
      ...baseTask,
      lifecycleStatus: "running" as const,
      terminalDeliveryStatus: "not_terminal" as const,
      finalDeliveryEvidencePresent: false,
      pendingBlockers: [],
      pendingReviewItemRefs: [],
      artifacts: [
        {
          artifactId: "artifact:pending-verification",
          version: 1,
          status: "materialized" as const,
          mediaType: "text/markdown; charset=utf-8",
          contentDigest: "sha256:pending-verification",
          targetReferenceDigest: "sha256:pending-target",
          materializedReference: "/OpenLife/Results/pending.md",
          sourceItemRef: {
            id: "item:pending-verification",
            kind: "evidence" as const,
            label: "待核验产物",
          },
          evidenceRefs: baseTask.evidenceRefs,
          change: {
            kind: "create" as const,
            status: "materialized" as const,
            targetReference: "/OpenLife/Results/pending.md",
          },
          preview: { status: "available" as const, content: "# 等待核验" },
          verification: {
            status: "pending" as const,
            expectedContentDigest: "sha256:pending-verification",
            verificationItemPresent: false,
          },
          undo: { available: false, reasonCode: "artifact_undo_requires_verified_materialization" },
        },
      ],
    };
    const dataSource = {
      ...fixture,
      async load() {
        const current = await fixture.load();
        return {
          ...current,
          tasksEnvelope: {
            ...current.tasksEnvelope,
            data: { ...current.tasksEnvelope.data!, items: [materializedPendingTask] },
          },
        };
      },
    };

    render(
      <ProductWorkbench workbenchDataSource={dataSource} conversationDataSource={dataSource} />
    );

    expect(await screen.findByTestId("canonical-task-artifacts")).toHaveTextContent("已物化");
    expect(screen.getByText("等待文件完整性核验")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "打开文件" })).not.toBeInTheDocument();
  });

  it("opens a LifeModel checkpoint inline without restoring a separate Review page", async () => {
    const user = userEvent.setup();
    const dataSource = workbenchFixtureDataSource("fixture-ready");
    render(
      <ProductWorkbench
        workbenchDataSource={dataSource}
        personalIntelligenceDataSource={dataSource}
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
    const dataSource = workbenchFixtureDataSource("fixture-ready");
    render(<ProductWorkbench workbenchDataSource={dataSource} />);

    expect(await screen.findByText("本地路由，未外传")).toBeInTheDocument();
  });

  it("shows canonical blocked Work in Needs Attention even when Review is empty", async () => {
    const fixture = workbenchFixtureDataSource("fixture-ready");
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
        };
      },
    };

    render(
      <ProductWorkbench workbenchDataSource={dataSource} conversationDataSource={dataSource} />
    );

    expect(
      await screen.findByRole("button", {
        name: /查询官网标题.*需要处理：所需资料当前不可访问/,
      })
    ).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "进度与结果" })).toBeInTheDocument();
    expect(screen.queryByText("当前没有可展示的任务。")).not.toBeInTheDocument();
  });

  it("shows only implemented settings categories", async () => {
    const user = userEvent.setup();
    const dataSource = workbenchFixtureDataSource("fixture-ready");
    const settings = createSettingsFixture("fixture-ready");
    render(
      <StrictMode>
        <ProductWorkbench
          workbenchDataSource={dataSource}
          settingsDataSource={settings.dataSource}
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
    const fixture = workbenchFixtureDataSource("fixture-ready");
    const load = vi.fn(fixture.load);
    const dataSource = { ...fixture, load };

    render(
      <ProductWorkbench
        workbenchDataSource={dataSource}
        conversationDataSource={dataSource}
        settingsDataSource={createSettingsFixture("fixture-ready").dataSource}
      />
    );

    await waitFor(() => expect(load.mock.calls.length).toBeGreaterThan(0));
    const callsBeforeSettingsReturn = load.mock.calls.length;
    await user.click(await screen.findByRole("button", { name: "设置" }));
    await user.click(await screen.findByRole("button", { name: "返回工作台" }));

    await waitFor(() => expect(load.mock.calls.length).toBeGreaterThan(callsBeforeSettingsReturn));
  });

  it("refreshes Workbench state before opening a review created from settings", async () => {
    const user = userEvent.setup();
    const fixture = workbenchFixtureDataSource("fixture-settings-review-required");
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

    render(<ProductWorkbench workbenchDataSource={dataSource} settingsDataSource={dataSource} />);

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
