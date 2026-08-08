import { act, render, renderHook, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { workbenchJourneyFixtureDataSource } from "@/test/fixtures/workbench/governedAction";
import {
  buildDurableFixtureSnapshot,
  durableReviewItem,
} from "@/test/fixtures/workbench/durableTruth";
import { ReadOnlySpineJourney } from "@/ui/journeys/readOnly";
import { useDurableTruthJourney } from "./useDurableTruthJourney";

function renderJourney(fixtureId: Parameters<typeof workbenchJourneyFixtureDataSource>[0]) {
  const dataSource = workbenchJourneyFixtureDataSource(fixtureId);
  render(
    <ReadOnlySpineJourney
      dataSource={dataSource}
      governedActionDataSource={dataSource}
      durableTruthDataSource={dataSource}
      lifeModelBuilderDataSource={dataSource}
      initialSurface="life-model"
    />
  );
  return dataSource;
}

describe("Workbench durable truth journey", () => {
  it("presents LifeModel and Agent Memory as keyboard-accessible peer domains", async () => {
    const user = userEvent.setup();
    renderJourney("fixture-ready");

    const lifeModelTab = await screen.findByRole("tab", { name: /关于我.*LifeModel/ });
    const memoryTab = screen.getByRole("tab", { name: /Agent 记忆.*工作连续性/ });
    expect(lifeModelTab).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("heading", { name: "当前有来源的长期理解" })).toBeVisible();

    lifeModelTab.focus();
    await user.keyboard("{ArrowRight}");
    expect(memoryTab).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("heading", { name: "Agent 记忆" })).toBeVisible();
    expect(screen.getByText("输出建议时先给结论，再补充依据。")).toBeVisible();
    expect(document.getElementById("intelligence-panel-life-model")).not.toBeVisible();
  });

  it("shows a structured canonical version instead of the legacy compatibility summary", async () => {
    const dataSource = workbenchJourneyFixtureDataSource("fixture-ready");
    const snapshot = buildDurableFixtureSnapshot("fixture-ready", "pending");
    if (snapshot.lifeModelEnvelope.data) {
      snapshot.lifeModelEnvelope.data = {
        ...snapshot.lifeModelEnvelope.data,
        truthMode: "canonical",
        canonicalSummary: {
          lifeModelRef: {
            id: "lifemodel-v2:primary:2",
            kind: "lifemodel",
            label: "Canonical LifeModel v2",
          },
          title: "已确认的长期个人模型",
          summary: "2 条经过用户确认的长期信息。",
          versionLabel: "openlife.lifemodel.v2 · version 2",
          lastMaterializedAt: "2026-08-08T10:00:00Z",
          evidenceRefs: [],
          humanProjection: {
            schemaVersion: "openlife.lifemodel.v2.yaml-projection.v1",
            modelId: "primary",
            modelVersion: 2,
            itemCount: 2,
            documentDigest: "sha256:document",
            yamlContentDigest: "sha256:yaml",
            projectionDigest: "sha256:projection",
            yaml: "schemaVersion: openlife.lifemodel.v2\nmodelId: primary\nvalues:\n  - id: value:1\n    statement: Autonomy matters.\n",
          },
        },
        legacyMigrationPreview: {
          schemaVersion: "openlife.lifemodel.legacy-migration-preview.v1",
          sourceDigest: "sha256:legacy",
          items: [],
          reviewRequiredCount: 1,
          externalOwnerCount: 0,
          manualClassificationCount: 0,
          notMigratedCount: 0,
          migrationMetadataCount: 0,
          containsSensitiveItems: false,
          candidates: [],
        },
      };
    }
    vi.spyOn(dataSource, "loadDurableTruth").mockResolvedValue(snapshot);
    render(
      <ReadOnlySpineJourney
        dataSource={dataSource}
        governedActionDataSource={dataSource}
        durableTruthDataSource={dataSource}
        initialSurface="life-model"
      />
    );

    expect(await screen.findByRole("heading", { name: "已确认的长期个人模型" })).toBeVisible();
    expect(screen.getByText("2 条经过用户确认的长期信息。")).toBeVisible();
    expect(
      screen.getByText("openlife.lifemodel.v2 · version 2 · 确认于 2026-08-08T10:00:00Z")
    ).toBeVisible();
    expect(screen.queryByText("当前有来源的长期理解")).not.toBeInTheDocument();
    expect(
      screen.queryByText("负责产品与工程决策，需要保留连续的独立思考时间。")
    ).not.toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "迁移前预览" })).not.toBeInTheDocument();

    await userEvent.click(screen.getByText("查看 YAML 人类视图"));
    expect(screen.getByText(/SQLite 中的版本化 JSON 才是权威/)).toBeVisible();
    expect(screen.getByLabelText("LifeModel YAML 人类视图")).toHaveTextContent("Autonomy matters.");
    expect(screen.queryByRole("button", { name: /保存|导入|应用/ })).not.toBeInTheDocument();
  });

  it("shows a field-complete legacy migration preview without claiming migration", async () => {
    const dataSource = workbenchJourneyFixtureDataSource("fixture-ready");
    const snapshot = buildDurableFixtureSnapshot("fixture-ready", "pending");
    if (snapshot.lifeModelEnvelope.data) {
      snapshot.lifeModelEnvelope.data.legacyMigrationPreview = {
        schemaVersion: "openlife.lifemodel.legacy-migration-preview.v1",
        sourceDigest: "sha256:source",
        reviewRequiredCount: 1,
        externalOwnerCount: 1,
        manualClassificationCount: 1,
        notMigratedCount: 1,
        migrationMetadataCount: 0,
        containsSensitiveItems: true,
        candidates: [],
        items: [
          {
            sourcePath: "identity.values[0].name",
            valuePreview: "独立判断",
            valueDigest: "sha256:value",
            valueTruncated: false,
            disposition: "review_required",
            targetOwner: "life_model_v2",
            targetSection: "values",
            reasonCode: "legacy_value_requires_user_confirmation",
            sensitive: false,
          },
          {
            sourcePath: "state.current_focus",
            valuePreview: "阶段五",
            valueDigest: "sha256:state",
            valueTruncated: false,
            disposition: "external_owner",
            targetOwner: "state_store",
            targetSection: null,
            reasonCode: "current_state_belongs_to_state_store",
            sensitive: false,
          },
          {
            sourcePath: "relationships.inner_circle[0].notes",
            valuePreview: "私人关系说明",
            valueDigest: "sha256:relationship",
            valueTruncated: true,
            disposition: "manual_classification",
            targetOwner: "unassigned",
            targetSection: null,
            reasonCode: "manual_review",
            sensitive: true,
          },
          {
            sourcePath: "identity.personality_traits[0].score",
            valuePreview: "8",
            valueDigest: "sha256:score",
            valueTruncated: false,
            disposition: "not_migrated",
            targetOwner: "unassigned",
            targetSection: null,
            reasonCode: "legacy_personality_score_requires_user_restatement",
            sensitive: true,
          },
        ],
      };
    }
    vi.spyOn(dataSource, "loadDurableTruth").mockResolvedValue(snapshot);
    render(
      <ReadOnlySpineJourney
        dataSource={dataSource}
        governedActionDataSource={dataSource}
        durableTruthDataSource={dataSource}
        initialSurface="life-model"
      />
    );

    expect(await screen.findByRole("heading", { name: "迁移前预览" })).toBeVisible();
    expect(screen.getByText("只读 · 尚未迁移")).toBeVisible();
    expect(screen.getByText("包含敏感个人信息")).toBeVisible();
    expect(screen.getByText("查看全部 4 个来源字段")).toBeVisible();

    await userEvent.click(screen.getByText("查看全部 4 个来源字段"));
    expect(screen.getByText("identity.values[0].name")).toBeVisible();
    expect(screen.getByText("独立判断")).toBeVisible();
    expect(screen.getByText(/需要你审核 · 目标：LifeModel v2/)).toBeVisible();
    expect(screen.getByText(/属于其他区域 · 目标：当前状态/)).toBeVisible();
    expect(screen.getByText(/需要人工判断 · 目标：尚未确定 · 仅显示摘要/)).toBeVisible();
    expect(screen.getByText(/不会迁移 · 目标：尚未确定/)).toBeVisible();
  });

  it("keeps migration candidates unselected and creates only a Review proposal", async () => {
    const user = userEvent.setup();
    const dataSource = workbenchJourneyFixtureDataSource("fixture-ready");
    const snapshot = buildDurableFixtureSnapshot("fixture-ready", "pending");
    if (snapshot.lifeModelEnvelope.data) {
      snapshot.lifeModelEnvelope.data.legacyMigrationPreview = {
        schemaVersion: "openlife.lifemodel.legacy-migration-preview.v1",
        sourceDigest: "sha256:source",
        reviewRequiredCount: 1,
        externalOwnerCount: 0,
        manualClassificationCount: 0,
        notMigratedCount: 0,
        migrationMetadataCount: 0,
        containsSensitiveItems: true,
        items: [
          {
            sourcePath: "identity.name",
            valuePreview: "Alice",
            valueDigest: "sha256:value",
            valueTruncated: false,
            disposition: "review_required",
            targetOwner: "life_model_v2",
            targetSection: "identity",
            reasonCode: "legacy_identity_requires_user_confirmation",
            sensitive: true,
          },
        ],
        candidates: [
          {
            candidateId: "legacy-candidate:one",
            itemId: "legacy:one",
            sourcePaths: ["identity.name"],
            targetSection: "identity",
            proposedValue: { kind: "statement", value: { statement: "Alice" } },
            sensitive: true,
          },
        ],
      };
    }
    vi.spyOn(dataSource, "loadDurableTruth").mockResolvedValue(snapshot);
    const draft = vi.spyOn(dataSource, "draftLegacyLifeModelMigration");
    render(
      <ReadOnlySpineJourney
        dataSource={dataSource}
        governedActionDataSource={dataSource}
        durableTruthDataSource={dataSource}
        initialSurface="life-model"
      />
    );

    const submit = await screen.findByRole("button", { name: "提交到 Review" });
    expect(submit).toBeDisabled();
    expect(screen.getAllByText("敏感").length).toBeGreaterThan(0);
    expect(screen.getByRole("radio", { name: "纳入" })).not.toBeChecked();
    await user.click(screen.getByRole("radio", { name: "纳入" }));
    expect(submit).toBeEnabled();
    await user.click(submit);

    await waitFor(() => expect(draft).toHaveBeenCalledOnce());
    expect(draft).toHaveBeenCalledWith({
      sourceDigest: "sha256:source",
      selections: [
        {
          candidateId: "legacy-candidate:one",
          decision: "include",
          editedValue: { kind: "statement", value: { statement: "Alice" } },
        },
      ],
      nonLifemodelItemsAcknowledged: false,
    });
    expect(await screen.findByText("等待 Review")).toBeVisible();
    expect(screen.getByText(/接受前不会备份、写入 v2 或切换权威源/)).toBeVisible();
  });

  it("keeps the created migration proposal visible when Review refresh fails", async () => {
    const dataSource = workbenchJourneyFixtureDataSource("fixture-ready");
    const snapshot = buildDurableFixtureSnapshot("fixture-ready", "pending");
    const refreshFailed = structuredClone(snapshot);
    refreshFailed.reviewEnvelope = {
      ...refreshFailed.reviewEnvelope,
      data: null,
      status: "error",
      evidenceRefs: [],
    };
    vi.spyOn(dataSource, "loadDurableTruth").mockResolvedValue(refreshFailed);
    vi.spyOn(dataSource, "draftLegacyLifeModelMigration").mockResolvedValue("proposal:migration");
    const announce = vi.fn();
    const { result } = renderHook(() => useDurableTruthJourney(dataSource, announce));

    await act(async () => {
      expect(
        await result.current.draftLegacyMigration({
          sourceDigest: "sha256:source",
          selections: [],
          nonLifemodelItemsAcknowledged: true,
        })
      ).toBe(true);
    });

    expect(result.current.migrationAction).toEqual({
      status: "review_required",
      proposalId: "proposal:migration",
    });
    expect(announce).toHaveBeenLastCalledWith(expect.stringContaining("已经创建"));
    expect(announce).toHaveBeenLastCalledWith(expect.stringContaining("刷新未验证"));
  });

  it("keeps Agent Memory available when only LifeModel fails", async () => {
    const user = userEvent.setup();
    const dataSource = workbenchJourneyFixtureDataSource("fixture-ready");
    const snapshot = buildDurableFixtureSnapshot("fixture-ready", "pending");
    snapshot.lifeModelEnvelope = {
      ...snapshot.lifeModelEnvelope,
      data: null,
      status: "error",
      evidenceRefs: [],
    };
    vi.spyOn(dataSource, "loadDurableTruth").mockResolvedValue(snapshot);
    render(
      <ReadOnlySpineJourney
        dataSource={dataSource}
        governedActionDataSource={dataSource}
        durableTruthDataSource={dataSource}
        initialSurface="life-model"
      />
    );

    expect(await screen.findByText("关于我暂时不可用")).toBeInTheDocument();
    await user.click(screen.getByRole("tab", { name: /Agent 记忆.*工作连续性/ }));
    expect(screen.getByRole("heading", { name: "Agent 记忆" })).toBeVisible();
    expect(screen.getByText("输出建议时先给结论，再补充依据。")).toBeVisible();
    expect(screen.queryByText("个人智能暂时不可用")).not.toBeInTheDocument();
  });

  it("keeps Memory readable but closes reviewed controls when Review Center fails", async () => {
    const user = userEvent.setup();
    const dataSource = workbenchJourneyFixtureDataSource("fixture-ready");
    const snapshot = buildDurableFixtureSnapshot("fixture-ready", "pending");
    snapshot.reviewEnvelope = {
      ...snapshot.reviewEnvelope,
      data: null,
      status: "error",
      evidenceRefs: [],
    };
    vi.spyOn(dataSource, "loadDurableTruth").mockResolvedValue(snapshot);
    render(
      <ReadOnlySpineJourney
        dataSource={dataSource}
        governedActionDataSource={dataSource}
        durableTruthDataSource={dataSource}
        initialSurface="life-model"
      />
    );

    await user.click(await screen.findByRole("tab", { name: /Agent 记忆.*工作连续性/ }));
    expect(screen.getByText("输出建议时先给结论，再补充依据。")).toBeVisible();
    expect(screen.getByRole("button", { name: "纠正" })).toBeDisabled();
    expect(
      screen.getAllByText("Review Center 状态不可用；不能创建无法核对的审核建议。").length
    ).toBeGreaterThan(0);
  });

  it("keeps Memory review items out of the LifeModel change list", async () => {
    const user = userEvent.setup();
    const dataSource = workbenchJourneyFixtureDataSource("fixture-ready");
    const snapshot = buildDurableFixtureSnapshot("fixture-ready", "pending");
    const memoryItem = {
      ...durableReviewItem("pending"),
      id: "review-memory-write",
      type: "memory_write" as const,
      decisionContext: {
        ...durableReviewItem("pending").decisionContext,
        reviewItemId: "review-memory-write",
        title: "Add a memory",
      },
    };
    if (snapshot.reviewEnvelope.data) {
      snapshot.reviewEnvelope.data = {
        ...snapshot.reviewEnvelope.data,
        items: [memoryItem, ...snapshot.reviewEnvelope.data.items],
      };
    }
    vi.spyOn(dataSource, "loadDurableTruth").mockResolvedValue(snapshot);
    render(
      <ReadOnlySpineJourney
        dataSource={dataSource}
        governedActionDataSource={dataSource}
        durableTruthDataSource={dataSource}
        initialSurface="life-model"
      />
    );

    expect(await screen.findByRole("heading", { name: "当前有来源的长期理解" })).toBeVisible();
    expect(screen.queryByText("Add a memory")).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "查看并决定" }));
    expect(
      await screen.findByRole("heading", { name: "把上午作为优先深度工作时段", level: 2 })
    ).toBeInTheDocument();
  });

  it("submits Memory corrections for Review without changing LifeModel", async () => {
    const user = userEvent.setup();
    const dataSource = renderJourney("fixture-ready");
    const correctMemory = vi.spyOn(dataSource, "correctMemory");

    await user.click(await screen.findByRole("tab", { name: /Agent 记忆.*工作连续性/ }));
    await user.click(screen.getByRole("button", { name: "纠正" }));
    const editor = screen.getByLabelText("纠正后的完整内容");
    await user.clear(editor);
    await user.type(editor, "先给结论，再按需补充依据。");
    await user.click(screen.getByRole("button", { name: "提交 Review" }));

    await waitFor(() =>
      expect(correctMemory).toHaveBeenCalledWith(
        "memory:writing-feedback:conclusion-first",
        "先给结论，再按需补充依据。"
      )
    );
    expect(
      screen.getByText("Memory 纠正已进入 Review；旧记忆仍保持当前状态。")
    ).toBeInTheDocument();
  });

  it("opens the exact review without deciding and returns approved-not-applied after refresh", async () => {
    const user = userEvent.setup();
    const dataSource = renderJourney("fixture-ready");
    const dispatchReview = vi.spyOn(dataSource, "dispatchReviewAction");

    expect(
      await screen.findByRole("heading", { name: "当前有来源的长期理解" })
    ).toBeInTheDocument();
    expect(screen.getAllByText("等待决定").length).toBeGreaterThan(0);
    expect(screen.getByRole("button", { name: /^个人智能\s+关于我与记忆/ })).toHaveAttribute(
      "aria-current",
      "page"
    );

    await user.click(screen.getByRole("button", { name: "查看并决定" }));
    expect(
      await screen.findByRole("heading", { name: "把上午作为优先深度工作时段", level: 2 })
    ).toBeInTheDocument();
    expect(dispatchReview).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "批准变更" }));
    expect(screen.getByRole("dialog", { name: "确认批准变更？" })).toBeInTheDocument();
    expect(dispatchReview).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "确认批准" }));

    await waitFor(() => expect(dispatchReview).toHaveBeenCalledOnce());
    expect(
      await screen.findByText("已批准，尚未应用", { selector: ".ol-notice__title" })
    ).toBeInTheDocument();
    expect(
      screen.queryByText("变更已应用", { selector: ".ol-notice__title" })
    ).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "返回个人智能" }));
    await screen.findByRole("heading", { name: "当前有来源的长期理解" });
    expect(screen.getAllByText("已批准，尚未应用").length).toBeGreaterThan(0);
    expect(screen.getByRole("button", { name: "应用变更" })).toBeDisabled();
    expect(
      screen.getByText("后端尚未为该审核项提供可调用的应用命令；批准不等于已应用。")
    ).toBeInTheDocument();
    expect(screen.queryByText("已应用", { selector: ".ol-status-label" })).not.toBeInTheDocument();
  });

  it("renders verified applied only for the exact refreshed proof", async () => {
    const { container } = render(
      (() => {
        const dataSource = workbenchJourneyFixtureDataSource("fixture-durable-applied");
        return (
          <ReadOnlySpineJourney
            dataSource={dataSource}
            governedActionDataSource={dataSource}
            durableTruthDataSource={dataSource}
            initialSurface="life-model"
          />
        );
      })()
    );

    expect(await screen.findByText("读模型已确认")).toBeInTheDocument();
    expect(screen.getAllByText("已应用").length).toBeGreaterThan(0);
    expect(container.querySelector('[data-durable-lifecycle="applied"]')).toBeInTheDocument();
    expect(container.querySelector(".ol-status-label--success")).toBeInTheDocument();
  });

  it("fails stale durable state closed and keeps decision actions out of the page", async () => {
    const { container } = render(
      (() => {
        const dataSource = workbenchJourneyFixtureDataSource("fixture-stale");
        return (
          <ReadOnlySpineJourney
            dataSource={dataSource}
            governedActionDataSource={dataSource}
            durableTruthDataSource={dataSource}
            initialSurface="life-model"
          />
        );
      })()
    );

    expect(await screen.findByText("长期状态已陈旧")).toBeInTheDocument();
    expect(container.querySelector(".ol-status-label--success")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "批准变更" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "查看并决定" })).toBeInTheDocument();
  });

  it("keeps the Review return destination tied to entry origin, not queue selection", async () => {
    const user = userEvent.setup();
    renderJourney("fixture-ready");

    await screen.findByRole("heading", { name: "当前有来源的长期理解" });
    await user.click(screen.getByRole("button", { name: "查看并决定" }));
    await screen.findByRole("heading", { name: "把上午作为优先深度工作时段", level: 2 });
    await user.click(screen.getByRole("button", { name: /读取本地客户访谈记录\s+等待决定/ }));

    expect(screen.getByRole("button", { name: "返回个人智能" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "返回个人智能" }));
    expect(
      await screen.findByRole("heading", { name: "当前有来源的长期理解" })
    ).toBeInTheDocument();
  });

  it("builds first-time candidates into exact review items without claiming durable completion", async () => {
    const user = userEvent.setup();
    const dataSource = renderJourney("fixture-empty");
    const createProposals = vi.spyOn(dataSource, "createProposals");

    expect(await screen.findByRole("heading", { name: "从真实情况开始" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "开始建立 LifeModel" }));
    expect(
      await screen.findByRole("heading", { name: "接下来三个月，你最希望推进什么？" })
    ).toBeInTheDocument();
    await user.type(screen.getByLabelText("你的回答"), "先完成三次访谈分析，再确定下一轮验证重点");
    await user.click(screen.getByRole("button", { name: "继续" }));

    expect(
      await screen.findByRole("heading", { name: "逐项决定哪些内容进入审核" })
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "创建审核建议" })).toBeDisabled();
    const acceptChoices = screen.getAllByRole("radio", { name: "纳入审核" });
    await user.click(acceptChoices[0]);
    await user.click(acceptChoices[1]);
    await user.click(screen.getByRole("button", { name: "创建审核建议" }));

    await waitFor(() => expect(createProposals).toHaveBeenCalledOnce());
    expect(await screen.findByText("审核建议已创建")).toBeInTheDocument();
    expect(screen.getByText(/尚未批准，也尚未应用到 LifeModel/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "前往审核中心" }));

    expect(
      await screen.findByRole("heading", { name: "将客户研究设为近期目标", level: 2 })
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "批准变更" }));
    await user.click(screen.getByRole("button", { name: "确认批准" }));
    expect(
      await screen.findByText("已批准，尚未应用", { selector: ".ol-notice__title" })
    ).toBeInTheDocument();
    expect(screen.queryByText("已应用", { selector: ".ol-status-label" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "返回个人智能" })).toBeInTheDocument();
  });
});
