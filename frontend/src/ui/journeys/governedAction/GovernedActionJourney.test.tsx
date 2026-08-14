import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { workbenchJourneyFixtureDataSource } from "@/test/fixtures/workbench/governedAction";
import { ProductWorkbenchJourney } from "@/ui/journeys/productWorkbench";
import type { ReviewAction, ReviewItem } from "@/tauri";
import {
  reviewDecisionFeedback,
  reviewItemStatus,
  reviewQueueSections,
} from "./ReviewGovernedView";

describe("Workbench governed action journey", () => {
  it("groups LifeModel learning reviews separately and shows at most five at once", async () => {
    const fixture = workbenchJourneyFixtureDataSource("fixture-ready");
    const snapshot = await fixture.load();
    const template = snapshot.reviewEnvelope.data!.items[1];
    const learningItems = Array.from(
      { length: 7 },
      (_, index): ReviewItem => ({
        ...template,
        id: `learning-${index + 1}`,
        status: index < 2 ? "approved" : "pending",
        source: {
          ...template.source,
          proposalId: `learning-${index + 1}`,
        },
        decisionContext: {
          ...template.decisionContext,
          reviewItemId: `learning-${index + 1}`,
          title: `学习建议 ${index + 1}`,
          lifeModelLearning: {
            candidateId: `candidate-${index + 1}`,
            candidateSnapshotDigest: `sha256:${String(index + 1).padStart(64, "0")}`,
            section: "stable_preferences",
            proposedStatement: `偏好 ${index + 1}`,
            explicitness: "explicit_user_request",
            stability: "user_confirmed",
            sensitivity: "internal",
            conflictStatus: "none",
            supportCount: 1,
            independentSupportCount: 1,
            confirmedAt: "2026-08-09T00:00:00Z",
            sourceRefs: [`message:${index + 1}`],
            sourceKinds: ["explicit_user_message"],
          },
        },
      })
    );
    const ordinaryItem = snapshot.reviewEnvelope.data!.items[0];

    const sections = reviewQueueSections([...learningItems, ordinaryItem]);

    expect(sections).toHaveLength(2);
    expect(sections[0]).toMatchObject({
      id: "lifemodel_learning",
      label: "LifeModel 学习建议",
      totalCount: 7,
      hiddenCount: 2,
    });
    expect(sections[0].items.map(item => item.id)).toEqual([
      "learning-3",
      "learning-4",
      "learning-5",
      "learning-6",
      "learning-7",
    ]);
    expect(sections[1]).toMatchObject({
      id: "other",
      label: "其他建议与权限",
      totalCount: 1,
      hiddenCount: 0,
    });
    expect(sections[1].items).toEqual([ordinaryItem]);
  });

  it("keeps an inline checkpoint decision separate from Work completion", async () => {
    const user = userEvent.setup();
    const dataSource = workbenchJourneyFixtureDataSource("fixture-ready");
    const dispatchReview = vi.spyOn(dataSource, "dispatchReviewAction");

    render(
      <ProductWorkbenchJourney
        dataSource={dataSource}
        governedActionDataSource={dataSource}
        workspaceConversationDataSource={dataSource}
        initialSurface="workspace"
      />
    );

    expect(await screen.findByRole("heading", { name: "Work 进度与结果" })).toBeInTheDocument();
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
    expect(
      await screen.findByText("已开始读取本地记录并提取重复问题；尚未形成最终结果。")
    ).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "仅允许本次" })).not.toBeInTheDocument();
    expect(screen.queryByText("任务已完成")).not.toBeInTheDocument();

    expect(screen.queryByRole("button", { name: "继续任务" })).not.toBeInTheDocument();
    expect(screen.queryByText("任务已完成")).not.toBeInTheDocument();
  });

  it("keeps approval disabled when the backend permission scope is incomplete", async () => {
    const user = userEvent.setup();
    const dataSource = workbenchJourneyFixtureDataSource("fixture-incomplete-permission");
    const dispatchReview = vi.spyOn(dataSource, "dispatchReviewAction");

    render(
      <ProductWorkbenchJourney
        dataSource={dataSource}
        governedActionDataSource={dataSource}
        initialSurface="workspace"
      />
    );

    expect(await screen.findByText("访问范围不完整")).toBeInTheDocument();
    const approve = screen.getByRole("button", { name: "仅允许本次" });
    expect(approve).toBeDisabled();
    expect(screen.getByText("缺少目标范围和有效期；不能批准。")).toBeInTheDocument();
    await user.click(approve);
    expect(dispatchReview).not.toHaveBeenCalled();
  });

  it("shows the backend-owned exact LifeModel typed diff before approval", async () => {
    const user = userEvent.setup();
    const fixture = workbenchJourneyFixtureDataSource("fixture-ready");
    const dataSource = {
      ...fixture,
      listSessions: async () => [],
      load: async () => {
        const snapshot = await fixture.load();
        const item = snapshot.reviewEnvelope.data!.items[0];
        const lifeModelItem = {
          ...item,
          type: "life_model_update" as const,
          decisionContext: {
            ...item.decisionContext,
            title: "Review LifeModel changes",
            summary: "Review an exact version-bound LifeModel change.",
            permission: undefined,
            before: {
              kind: "object" as const,
              summary: "LifeModel v2 version 1",
              detail: "sha256:base",
              sensitivity: "local_private" as const,
              truncated: false,
            },
            after: {
              kind: "list" as const,
              summary: "1 LifeModel change(s): 1 add, 0 replace, 0 remove",
              detail: "add values/value:autonomy: Autonomy matters.",
              sensitivity: "local_private" as const,
              truncated: false,
            },
          },
        };
        return {
          ...snapshot,
          reviewEnvelope: {
            ...snapshot.reviewEnvelope,
            data: {
              ...snapshot.reviewEnvelope.data!,
              items: [lifeModelItem],
            },
          },
          workspaceEnvelope: {
            ...snapshot.workspaceEnvelope,
            data: { ...snapshot.workspaceEnvelope.data!, pendingReviewItems: [lifeModelItem] },
          },
        };
      },
    };

    render(
      <ProductWorkbenchJourney
        dataSource={dataSource}
        governedActionDataSource={dataSource}
        initialSurface="workspace"
      />
    );

    expect(
      await screen.findByRole("heading", { name: "Review LifeModel changes", level: 2 })
    ).toBeInTheDocument();
    expect(screen.getByText("LifeModel v2 version 1")).toBeInTheDocument();
    await user.click(screen.getByText("查看精确变更"));
    expect(screen.getByText("add values/value:autonomy: Autonomy matters.")).toBeInTheDocument();
  });

  it("fails stale governed state closed while preserving evidence access", async () => {
    const user = userEvent.setup();
    const dataSource = workbenchJourneyFixtureDataSource("fixture-stale");
    const dispatchReview = vi.spyOn(dataSource, "dispatchReviewAction");

    render(
      <ProductWorkbenchJourney
        dataSource={dataSource}
        governedActionDataSource={dataSource}
        initialSurface="workspace"
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
      <ProductWorkbenchJourney
        dataSource={dataSource}
        governedActionDataSource={dataSource}
        initialSurface="workspace"
      />
    );

    await screen.findByRole("heading", { name: "读取本地客户访谈记录", level: 2 });
    await user.click(screen.getByRole("button", { name: "仅允许本次" }));
    await user.click(screen.getByRole("button", { name: "确认仅允许本次" }));

    expect(await screen.findByText("决定尚未被读模型确认")).toBeInTheDocument();
    expect(screen.getAllByText("等待决定").length).toBeGreaterThan(0);
    expect(screen.queryByText("决定已记录，尚未继续任务")).not.toBeInTheDocument();

    await user.click(
      within(screen.getByRole("region", { name: "当前 Work 的决定节点" })).getByRole("button", {
        name: "重新读取",
      })
    );
    await waitFor(() => expect(screen.queryByText("决定尚未被读模型确认")).not.toBeInTheDocument());
    expect(dataSource.dispatchReviewAction).toHaveBeenCalledTimes(1);
    expect(screen.getAllByText("等待决定").length).toBeGreaterThan(0);
  });

  it("ignores old task and review payloads when their envelopes are empty", async () => {
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
      <ProductWorkbenchJourney
        dataSource={dataSource}
        governedActionDataSource={dataSource}
        workspaceConversationDataSource={dataSource}
        initialSurface="workspace"
      />
    );

    expect(await screen.findByText("继续当前工作")).toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "Work 进度与结果" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "仅允许本次" })).not.toBeInTheDocument();
  });

  it("allows the first conversation when the backend workspace is truthfully empty", async () => {
    const user = userEvent.setup();
    const fixture = workbenchJourneyFixtureDataSource("fixture-ready");
    const dataSource = {
      ...fixture,
      listSessions: async () => [],
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
      <ProductWorkbenchJourney
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

  it("opens Personal Intelligence from a durable Life Model influence receipt", async () => {
    const user = userEvent.setup();
    const fixture = workbenchJourneyFixtureDataSource("fixture-ready");
    const dataSource = {
      ...fixture,
      loadDurableTruth: async () => {
        const snapshot = await fixture.loadDurableTruth();
        return {
          ...snapshot,
          lifeModelEnvelope: {
            ...snapshot.lifeModelEnvelope,
            status: "ready" as const,
            data: {
              ...snapshot.lifeModelEnvelope.data!,
              truthMode: "canonical" as const,
              canonicalSummary: {
                lifeModelRef: {
                  id: "lifemodel:primary:v8",
                  kind: "lifemodel" as const,
                  label: "LifeModel v2 version 8",
                },
                title: "已确认的长期个人模型",
                summary: "1 条已确认信息",
                versionLabel: "v8",
                parentVersion: 7,
                documentDigest: "sha256:document-v8",
                lastMaterializedAt: "2026-08-09T00:00:00Z",
                freshnessStatus: "current",
                conflictStatus: "none",
                evidenceRefs: [],
                document: {
                  schemaVersion: "openlife.lifemodel.v2" as const,
                  modelId: "primary",
                  identity: [],
                  values: [],
                  longTermGoals: [],
                  stablePreferences: [],
                  personalBoundaries: [],
                  importantRelationships: [],
                  capabilities: [],
                  resources: [],
                  decisionPrinciples: [],
                  collaborationPreferences: [
                    {
                      id: "communication-direct",
                      statement: "沟通保持简洁直接",
                      sourceRefs: ["message:user:confirmed-preference"],
                      confirmedAt: "2026-08-09T00:00:00Z",
                    },
                  ],
                },
                humanProjection: {
                  schemaVersion: "openlife.lifemodel.v2.yaml-projection.v1" as const,
                  modelId: "primary",
                  modelVersion: 8,
                  itemCount: 1,
                  documentDigest: "sha256:document-v8",
                  yamlContentDigest: "sha256:yaml-v8",
                  projectionDigest: "sha256:projection-v8",
                  yaml: "collaboration_preferences:\n  - id: communication-direct",
                },
              },
            },
          },
        };
      },
      loadLifeModelInfluence: vi.fn().mockResolvedValue({
        status: "completed" as const,
        lifeModelInfluence: {
          status: "applied_context_building",
          sourceId: "lifemodel.v2.runtime",
          modelVersion: 8,
          selectedItems: [
            {
              itemRef: "collaboration_preferences:communication-direct",
              statement: "沟通保持简洁直接",
              sourceRefs: ["message:user:confirmed-preference"],
              confirmedAt: "2026-08-09T00:00:00Z",
              reasonCode: "task intent matches collaboration_preferences",
            },
          ],
          appliedSurfaces: ["context_building", "communication_style"],
          currentInstructionPriorityPreserved: true,
          policyPriorityPreserved: true,
          permissionGranted: false,
          durableWriteAuthorized: false,
        },
      }),
    };

    render(
      <ProductWorkbenchJourney
        dataSource={dataSource}
        governedActionDataSource={dataSource}
        durableTruthDataSource={dataSource}
        workspaceConversationDataSource={dataSource}
        initialSurface="workspace"
      />
    );

    expect(await screen.findByText("本轮参考了你的 Life Model")).toBeInTheDocument();
    await user.click(screen.getByText("查看使用依据"));
    await user.click(screen.getByRole("button", { name: "在个人智能中查看：沟通保持简洁直接" }));

    expect(await screen.findByRole("tab", { name: /关于我.*LifeModel/ })).toBeInTheDocument();
    expect(screen.getByText("本次影响使用的长期信息")).toBeInTheDocument();
    expect(
      screen
        .getByText("collaboration_preferences:communication-direct")
        .closest("[data-lifemodel-item-ref]")
    ).toHaveAttribute("data-lifemodel-item-ref", "collaboration_preferences:communication-direct");
    expect(screen.getByRole("button", { name: /^个人智能\s+关于我与记忆/ })).toHaveAttribute(
      "aria-current",
      "page"
    );
  });

  it("fails closed when an active task points to a missing conversation", async () => {
    const fixture = workbenchJourneyFixtureDataSource("fixture-ready");
    const initial = await fixture.load();
    const approve = initial.reviewEnvelope.data!.items[0].allowedActions.find(
      action => action.kind === "approve"
    )!;
    await fixture.dispatchReviewAction(approve);
    const dataSource = {
      ...fixture,
      load: async () => {
        const snapshot = await fixture.load();
        return {
          ...snapshot,
          workspaceEnvelope: {
            ...snapshot.workspaceEnvelope,
            data: {
              ...snapshot.workspaceEnvelope.data!,
              selectedConversationId: "another-conversation",
              activeTask: {
                ...snapshot.workspaceEnvelope.data!.activeTask!,
                conversationId: "another-conversation",
              },
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
        initialSurface="workspace"
      />
    );

    expect(
      await screen.findByText("Work 投影尚未匹配当前 Conversation；旧任务不会暂时显示在这里。")
    ).toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "Work 进度与结果" })).not.toBeInTheDocument();
  });

  it("shows only backend-confirmed resources and removes them through the exact binding", async () => {
    const user = userEvent.setup();
    const dataSource = workbenchJourneyFixtureDataSource("fixture-ready");
    const pickResources = vi.spyOn(dataSource, "pickResources");
    const detachResource = vi.spyOn(dataSource, "detachResource");

    render(
      <ProductWorkbenchJourney
        dataSource={dataSource}
        governedActionDataSource={dataSource}
        workspaceConversationDataSource={dataSource}
        initialSurface="workspace"
      />
    );

    await screen.findByText("继续当前工作");
    const workMode = screen.getByRole("radio", { name: "Work" });
    await waitFor(() => expect(workMode).toBeEnabled());
    await user.click(workMode);
    await waitFor(() => expect(screen.getByRole("radio", { name: "Work" })).toBeChecked());
    await user.click(await screen.findByRole("button", { name: "添加文件" }));

    expect(await screen.findByText("访谈记录.md")).toBeInTheDocument();
    expect(screen.getByText("已添加 1/5")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "新对话" })).toBeDisabled();
    expect(pickResources).toHaveBeenCalledWith(expect.any(String), expect.any(String));

    await user.click(screen.getByRole("button", { name: "移除 访谈记录.md" }));

    await waitFor(() => expect(screen.queryByText("访谈记录.md")).not.toBeInTheDocument());
    expect(detachResource).toHaveBeenCalledWith(
      expect.any(String),
      pickResources.mock.calls[0][1],
      "4a006c47-67ee-4421-9f84-736f37926090"
    );
    expect(screen.getByText("未添加")).toBeInTheDocument();
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

  it("credits an artifact write only when refreshed backend digests match", async () => {
    const fixture = workbenchJourneyFixtureDataSource("fixture-ready");
    const snapshot = await fixture.load();
    const base = snapshot.reviewEnvelope.data!.items[0];
    const action = base.allowedActions.find(candidate => candidate.kind === "approve")!;
    const artifact: ReviewItem = {
      ...base,
      type: "external_write_action",
      artifactEvidence: {
        state: "confirmed",
        targetReferenceDigest: "sha256:target",
        contentDigest: "sha256:content",
        observedContentDigest: "sha256:content",
        byteSize: 12,
        mediaType: "text/markdown; charset=utf-8",
      },
    };

    expect(
      reviewDecisionFeedback(
        {
          phase: "resolved",
          action,
          refreshed: {
            reviewItemId: artifact.id,
            status: "approved",
            materializationStatus: "applied",
          },
        },
        artifact
      )
    ).toMatchObject({ title: "文件写入已核验" });

    expect(
      reviewDecisionFeedback(
        {
          phase: "resolved",
          action,
          refreshed: {
            reviewItemId: artifact.id,
            status: "approved",
            materializationStatus: "unknown",
          },
        },
        {
          ...artifact,
          artifactEvidence: {
            ...artifact.artifactEvidence!,
            observedContentDigest: "sha256:other",
          },
        }
      )
    ).toMatchObject({ title: "文件结果尚未确认" });
    expect(
      reviewItemStatus({
        ...artifact,
        artifactEvidence: {
          ...artifact.artifactEvidence!,
          observedContentDigest: "sha256:other",
        },
      })
    ).toEqual({ label: "文件状态未知", status: "unknown" });
  });

  it("labels governed actions by their exact operation and evidence boundary", async () => {
    const fixture = workbenchJourneyFixtureDataSource("fixture-ready");
    const snapshot = await fixture.load();
    const base = snapshot.reviewEnvelope.data!.items[0];
    const action = base.allowedActions.find(candidate => candidate.kind === "approve")!;
    const moved: ReviewItem = {
      ...base,
      type: "external_write_action",
      decisionContext: {
        ...base.decisionContext,
        actionContract: {
          capabilityId: "filesystem.write",
          operation: "move",
          confirmationSummary: "Confirm exact source and destination.",
          terminalEvidenceSummary: "Require matching filesystem receipt.",
          effectBoundary: "local_filesystem",
        },
      },
      artifactEvidence: {
        state: "confirmed",
        targetReferenceDigest: "sha256:target",
        contentDigest: "sha256:content",
        observedContentDigest: "sha256:content",
        byteSize: 12,
        mediaType: "text/markdown; charset=utf-8",
      },
    };
    expect(reviewItemStatus(moved)).toMatchObject({ label: "文件移动已核验", status: "success" });

    const browser: ReviewItem = {
      ...base,
      type: "data_export",
      status: "approved",
      materializationStatus: "applied",
      decisionContext: {
        ...base.decisionContext,
        actionContract: {
          capabilityId: "browser.open",
          operation: "open_browser_url",
          confirmationSummary: "Confirm one exact HTTP(S) address.",
          terminalEvidenceSummary: "System handoff only; page load remains unverified.",
          effectBoundary: "os_browser_handoff_unverified",
        },
      },
    };
    expect(reviewItemStatus(browser)).toMatchObject({
      label: "浏览器交接已记录",
      status: "success",
    });
    expect(
      reviewDecisionFeedback(
        {
          phase: "resolved",
          action,
          refreshed: {
            reviewItemId: browser.id,
            status: "approved",
            materializationStatus: "applied",
          },
        },
        browser
      )
    ).toMatchObject({
      title: "浏览器交接已记录",
      body: expect.stringContaining("page load remains unverified"),
    });
  });

  it("does not delete a conversation until the explicit confirmation action", async () => {
    const user = userEvent.setup();
    const dataSource = workbenchJourneyFixtureDataSource("fixture-ready");
    const deleteSession = vi.spyOn(dataSource, "deleteSession");

    render(
      <ProductWorkbenchJourney
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
