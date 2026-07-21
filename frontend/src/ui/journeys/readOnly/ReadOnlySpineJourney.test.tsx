import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { phase4dFixtureDataSource } from "@/dev/phase4d/phase4d-fixtures";
import type { ReadOnlySpineDataSource } from "./readOnlySpineDataSource";
import { ReadOnlySpineJourney } from "./ReadOnlySpineJourney";

describe("Phase 4D desktop read-only journey", () => {
  it("opens review as unavailable without converting view into an approval", async () => {
    const user = userEvent.setup();
    render(<ReadOnlySpineJourney dataSource={phase4dFixtureDataSource("fixture-ready")} />);

    expect(
      await screen.findByRole("heading", {
        name: "整理下周客户访谈要验证的三个问题",
      })
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^今日\s+当前关注/ })).toHaveAttribute(
      "aria-current",
      "page"
    );

    await user.click(screen.getByRole("button", { name: "查看待决定建议" }));

    expect(screen.getByRole("heading", { name: "审核状态源不可用", level: 1 })).toHaveFocus();
    expect(screen.getByRole("button", { name: /^审核中心\s+建议与权限决定/ })).toHaveAttribute(
      "aria-current",
      "page"
    );
    expect(screen.queryByText("已批准")).not.toBeInTheDocument();
    expect(screen.queryByText("已应用")).not.toBeInTheDocument();
  });

  it("supports real local search/filter and uses task selection only as Inspector context", async () => {
    const user = userEvent.setup();
    const { container } = render(
      <ReadOnlySpineJourney dataSource={phase4dFixtureDataSource("fixture-ready")} />
    );

    await user.click(screen.getByRole("button", { name: /^任务\s+队列与连续性/ }));
    expect(await screen.findByText("共 5 项，当前显示 5 项")).toBeInTheDocument();
    expect(container.querySelectorAll(".ol-readonly-task-row")).toHaveLength(5);
    expect(screen.getByText("待审核，未完成")).toBeInTheDocument();
    expect(screen.getByText("缺少完成证据")).toBeInTheDocument();
    expect(screen.getByText("已完成")).toBeInTheDocument();

    await user.selectOptions(screen.getByLabelText("筛选任务"), "attention");
    expect(screen.getByText("共 5 项，当前显示 3 项")).toBeInTheDocument();
    await user.selectOptions(screen.getByLabelText("筛选任务"), "all");
    await user.type(screen.getByLabelText("搜索任务"), "周报");
    expect(screen.getByText("共 5 项，当前显示 1 项")).toBeInTheDocument();
    expect(screen.getByText("把本周项目进展汇总成一页周报")).toBeInTheDocument();

    await user.clear(screen.getByLabelText("搜索任务"));
    const permissionTask = screen.getByRole("button", {
      name: /整理三次客户访谈，归纳下周要验证的问题/,
    });
    await user.click(permissionTask);

    expect(
      screen.getByRole("heading", {
        name: "整理三次客户访谈，归纳下周要验证的问题",
        level: 2,
      })
    ).toHaveFocus();
    expect(permissionTask).toHaveAttribute("aria-pressed", "true");
    expect(screen.queryByRole("button", { name: /恢复|重试|取消任务/ })).not.toBeInTheDocument();
  });

  it("fails stale state closed and exposes complete product action attributes", async () => {
    const { container } = render(
      <ReadOnlySpineJourney dataSource={phase4dFixtureDataSource("fixture-stale")} />
    );

    expect(await screen.findByText("当前计划已陈旧，只读且不执行")).toBeInTheDocument();
    expect(screen.getByText("传输边界已陈旧")).toBeInTheDocument();
    expect(container.querySelector(".ol-status-label--success")).not.toBeInTheDocument();
    const workspace = screen.getByRole("button", { name: "打开工作区" });
    expect(workspace).toBeDisabled();
    expect(screen.getByText("请先重新读取今日状态。")).toBeInTheDocument();

    const actions = Array.from(container.querySelectorAll<HTMLElement>("[data-action-id]"));
    expect(actions.length).toBeGreaterThan(0);
    for (const action of actions) {
      expect(action).toHaveAttribute("data-action-category", "product");
      expect(action).toHaveAttribute("data-action-id");
      expect(action).toHaveAttribute("data-action-kind");
      expect(action).toHaveAttribute("data-action-enabled");
      expect(action).toHaveAttribute("data-action-disabled-reason");
      expect(action).toHaveAttribute("data-action-target-ref");
    }
  });

  it("does not present a failed Tasks read as a confirmed empty list", async () => {
    const user = userEvent.setup();
    render(<ReadOnlySpineJourney dataSource={phase4dFixtureDataSource("fixture-error")} />);

    await user.click(screen.getByRole("button", { name: /^任务\s+队列与连续性/ }));

    expect(await screen.findByText("任务状态读取失败")).toBeInTheDocument();
    expect(screen.getByText(/当前数量未知/)).toBeInTheDocument();
    expect(screen.queryByText(/共 0 项/)).not.toBeInTheDocument();
    expect(screen.queryByText("当前没有可展示的任务。")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("搜索任务")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("筛选任务")).not.toBeInTheDocument();
  });

  it("treats an error envelope as authoritative even when it carries old payload data", async () => {
    const user = userEvent.setup();
    const fixture = phase4dFixtureDataSource("fixture-ready");
    const dataSource: ReadOnlySpineDataSource = {
      loadToday: async () => {
        const snapshot = await fixture.loadToday();
        return {
          ...snapshot,
          envelope: { ...snapshot.envelope, status: "error" as const },
        };
      },
      loadTasks: async () => {
        const snapshot = await fixture.loadTasks();
        return {
          ...snapshot,
          envelope: { ...snapshot.envelope, status: "error" as const },
        };
      },
    };
    const { container } = render(<ReadOnlySpineJourney dataSource={dataSource} />);

    expect(await screen.findByText("今日状态读取失败")).toBeInTheDocument();
    expect(screen.queryByText("整理下周客户访谈要验证的三个问题")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "打开工作区" })).not.toBeInTheDocument();
    expect(container.querySelector(".ol-shell-context-bar .ol-status-label")).toHaveTextContent(
      "读取失败"
    );

    await user.click(screen.getByRole("button", { name: /^任务\s+队列与连续性/ }));
    expect(await screen.findByText("任务状态读取失败")).toBeInTheDocument();
    expect(screen.queryByText("把本周项目进展汇总成一页周报")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("搜索任务")).not.toBeInTheDocument();
    expect(container.querySelector(".ol-shell-context-bar .ol-status-label")).toHaveTextContent(
      "读取失败"
    );
  });

  it("uses a separate settings context and restores focus when returning", async () => {
    const user = userEvent.setup();
    render(<ReadOnlySpineJourney dataSource={phase4dFixtureDataSource("fixture-ready")} />);
    await screen.findByText("整理下周客户访谈要验证的三个问题");

    await user.click(screen.getByRole("button", { name: "设置" }));
    expect(screen.getByRole("navigation", { name: "设置分类" })).toBeInTheDocument();
    expect(screen.queryByRole("navigation", { name: "产品区域" })).not.toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "模型与供应商暂不可用", level: 1 })).toHaveFocus();

    await user.type(screen.getByRole("searchbox", { name: "搜索设置" }), "API 凭据");
    expect(screen.getByRole("status")).toHaveTextContent("找到 1 个设置分类");
    expect(screen.getByRole("button", { name: /^模型与供应商/ })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /^隐私与网络/ })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "清除设置搜索" }));
    expect(screen.getByRole("status")).toHaveTextContent("共 7 个设置分类");

    await user.click(screen.getByRole("button", { name: "返回工作台" }));
    expect(screen.getByRole("button", { name: "设置" })).toHaveFocus();
    expect(screen.getByRole("navigation", { name: "产品区域" })).toBeInTheDocument();
  });

  it("refreshes through the supplied source and restores Inspector trigger focus", async () => {
    const user = userEvent.setup();
    const fixture = phase4dFixtureDataSource("fixture-ready");
    const dataSource: ReadOnlySpineDataSource = {
      loadToday: vi.fn(fixture.loadToday),
      loadTasks: vi.fn(fixture.loadTasks),
    };
    render(<ReadOnlySpineJourney dataSource={dataSource} />);

    await waitFor(() => expect(dataSource.loadToday).toHaveBeenCalledTimes(1));
    await user.click(screen.getByRole("button", { name: "重新读取" }));
    await waitFor(() => expect(dataSource.loadToday).toHaveBeenCalledTimes(2));

    const inspectorTrigger = screen.getByRole("button", { name: "打开证据检查器" });
    await user.click(inspectorTrigger);
    expect(screen.getByRole("heading", { name: "今日状态依据" })).toHaveFocus();
    const inspector = screen.getByRole("complementary", { name: "今日状态依据" });
    expect(within(inspector).getByRole("heading", { name: "发生了什么" })).toBeInTheDocument();
    expect(within(inspector).getByRole("heading", { name: "风险" })).toBeInTheDocument();
    expect(within(inspector).getByRole("heading", { name: "下一步" })).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "关闭证据检查器" }));
    expect(screen.getByRole("button", { name: "打开证据检查器" })).toHaveFocus();
  });
});
