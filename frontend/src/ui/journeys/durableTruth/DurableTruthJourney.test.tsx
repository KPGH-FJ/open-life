import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { workbenchJourneyFixtureDataSource } from "@/test/fixtures/workbench/governedAction";
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
  it("opens the exact review without deciding and returns approved-not-applied after refresh", async () => {
    const user = userEvent.setup();
    const dataSource = renderJourney("fixture-ready");
    const dispatchReview = vi.spyOn(dataSource, "dispatchReviewAction");

    expect(
      await screen.findByRole("heading", { name: "当前有来源的长期理解" })
    ).toBeInTheDocument();
    expect(screen.getAllByText("等待决定").length).toBeGreaterThan(0);
    expect(screen.getByRole("button", { name: /^LifeModel\s+长期状态/ })).toHaveAttribute(
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

    await user.click(screen.getByRole("button", { name: "返回 LifeModel" }));
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

    expect(screen.getByRole("button", { name: "返回 LifeModel" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "返回 LifeModel" }));
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
    expect(screen.getByRole("button", { name: "返回 LifeModel" })).toBeInTheDocument();
  });
});
