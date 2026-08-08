import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { workbenchJourneyFixtureDataSource } from "@/test/fixtures/workbench/governedAction";
import {
  buildDurableFixtureSnapshot,
  durableReviewItem,
} from "@/test/fixtures/workbench/durableTruth";
import { ReadOnlySpineJourney } from "@/ui/journeys/readOnly";

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
