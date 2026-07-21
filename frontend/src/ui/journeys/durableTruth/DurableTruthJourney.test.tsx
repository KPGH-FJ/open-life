import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { phase4dJourneyFixtureDataSource } from "@/dev/phase4d/phase4d-governed-fixtures";
import { ReadOnlySpineJourney } from "@/ui/journeys/readOnly";

function renderJourney(fixtureId: Parameters<typeof phase4dJourneyFixtureDataSource>[0]) {
  const dataSource = phase4dJourneyFixtureDataSource(fixtureId);
  render(
    <ReadOnlySpineJourney
      dataSource={dataSource}
      governedActionDataSource={dataSource}
      durableTruthDataSource={dataSource}
      initialSurface="life-model"
    />
  );
  return dataSource;
}

describe("Phase 4D durable truth journey", () => {
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
        const dataSource = phase4dJourneyFixtureDataSource("fixture-durable-applied");
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
        const dataSource = phase4dJourneyFixtureDataSource("fixture-stale");
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
});
