import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { DesktopShellHarness } from "./DesktopShellHarness";

describe("Phase 4C desktop shell harness", () => {
  it("keeps the fixture selector outside the product shell", () => {
    const { container } = render(<DesktopShellHarness />);
    const toolbar = screen.getByRole("banner", { name: "Phase 4C QA 工具栏" });
    const shell = container.querySelector(".ol-workbench-shell");

    expect(toolbar).toBeInTheDocument();
    expect(shell).toBeInTheDocument();
    expect(shell).not.toContainElement(within(toolbar).getByLabelText("布局状态"));
  });

  it("opens the pending review decision without changing it to approved", async () => {
    const user = userEvent.setup();
    render(<DesktopShellHarness />);

    await user.click(screen.getByRole("button", { name: "查看待审核建议" }));

    expect(screen.getByRole("heading", { name: "审核中心", level: 1 })).toHaveFocus();
    expect(screen.getByRole("heading", { name: "出差前保留准备时间" })).toBeInTheDocument();
    expect(screen.getByText("等待你的决定")).toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "已批准，尚未应用" })).not.toBeInTheDocument();
  });

  it("records approval without presenting it as applied or complete", async () => {
    const user = userEvent.setup();
    render(<DesktopShellHarness />);

    await user.click(screen.getByRole("button", { name: "查看待审核建议" }));
    await user.click(screen.getByRole("button", { name: "批准变更" }));
    expect(screen.getByRole("dialog", { name: "确认批准这条建议" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "确认批准" }));

    expect(screen.getByRole("heading", { name: "已批准，尚未应用" })).toBeInTheDocument();
    expect(screen.getAllByText("尚未应用").length).toBeGreaterThan(0);
    expect(screen.queryByText("已完成")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "应用变更" })).toBeDisabled();
  });

  it("makes unavailable task navigation explicit instead of redirecting", async () => {
    const user = userEvent.setup();
    render(<DesktopShellHarness />);

    await user.click(screen.getByRole("button", { name: /^任务\s+队列与连续性/ }));

    expect(screen.getByRole("button", { name: /^任务\s+队列与连续性/ })).toHaveAttribute(
      "aria-current",
      "page"
    );
    expect(screen.getByRole("heading", { name: "任务页面尚未迁移" })).toBeInTheDocument();
    expect(screen.getAllByText(/没有重定向/).length).toBeGreaterThan(0);
  });

  it("keeps focus on the new page heading when navigation closes the Inspector", async () => {
    const user = userEvent.setup();
    render(<DesktopShellHarness />);

    await user.click(screen.getByRole("button", { name: "打开证据检查器" }));
    await user.click(screen.getByRole("button", { name: /^任务\s+队列与连续性/ }));

    expect(screen.queryByRole("complementary", { name: "任务入口状态" })).not.toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "任务", level: 1 })).toHaveFocus();
  });

  it("opens structured evidence, exposes selection feedback, and restores focus", async () => {
    const user = userEvent.setup();
    render(<DesktopShellHarness />);

    const trigger = screen.getByRole("button", { name: "打开证据检查器" });
    await user.click(trigger);
    expect(screen.getByRole("heading", { name: "今日计划依据" })).toHaveFocus();
    expect(screen.getByRole("heading", { name: "发生了什么" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "风险" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "下一步" })).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /今日关注点样例/ }));
    expect(screen.getByText(/已选择证据 evidence_today_focus_fixture/)).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "关闭证据检查器" }));
    expect(screen.getByRole("button", { name: "打开证据检查器" })).toHaveFocus();
  });

  it("clears evidence selected in the pending state after approval", async () => {
    const user = userEvent.setup();
    render(<DesktopShellHarness />);

    await user.click(screen.getByRole("button", { name: "查看待审核建议" }));
    await user.click(screen.getByRole("button", { name: "打开证据检查器" }));
    await user.click(screen.getByRole("button", { name: /建议来源样例/ }));
    expect(screen.getByText(/已选择证据 evidence_review_proposal_fixture/)).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "批准变更" }));
    await user.click(screen.getByRole("button", { name: "确认批准" }));

    expect(screen.getByRole("heading", { name: "批准与应用状态" })).toBeInTheDocument();
    expect(screen.queryByText(/evidence_review_proposal_fixture/)).not.toBeInTheDocument();
  });

  it("uses a dedicated settings context and restores focus on Back", async () => {
    const user = userEvent.setup();
    render(<DesktopShellHarness />);

    await user.click(screen.getByRole("button", { name: "设置" }));
    expect(screen.getByRole("navigation", { name: "设置分类" })).toBeInTheDocument();
    expect(screen.queryByRole("navigation", { name: "产品区域" })).not.toBeInTheDocument();
    expect(screen.getAllByText("当前传输边界未知").length).toBeGreaterThanOrEqual(2);

    await user.click(screen.getByRole("button", { name: "返回工作台" }));
    expect(screen.getByRole("navigation", { name: "产品区域" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "设置" })).toHaveFocus();
  });

  it("keeps unknown and safe mode amber while external actions stay disabled", async () => {
    const user = userEvent.setup();
    const { container } = render(<DesktopShellHarness />);

    await user.selectOptions(screen.getByLabelText("布局状态"), "safe-mode");

    expect(screen.getAllByText("安全模式").length).toBeGreaterThan(0);
    expect(container.querySelectorAll(".ol-status-label--waiting").length).toBeGreaterThan(0);
    expect(container.querySelector(".ol-status-label--success")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "执行外部动作" })).toBeDisabled();
  });

  it("exposes complete fixture Action Contract attributes", async () => {
    const user = userEvent.setup();
    const { container } = render(<DesktopShellHarness />);
    await user.click(screen.getByRole("button", { name: "查看待审核建议" }));

    const actionNodes = Array.from(container.querySelectorAll<HTMLElement>("[data-action-id]"));
    expect(actionNodes.length).toBeGreaterThanOrEqual(5);
    for (const action of actionNodes) {
      expect(action).toHaveAttribute("data-action-id");
      expect(action).toHaveAttribute("data-action-kind");
      expect(action).toHaveAttribute("data-action-enabled");
      expect(action).toHaveAttribute("data-action-disabled-reason");
      expect(action).toHaveAttribute("data-action-target-ref");
      expect(action).toHaveAttribute("data-action-confirmation");
      expect(action).toHaveAttribute("data-action-materialization");
    }
  });

  it("announces dynamic changes through exactly one polite live region", () => {
    const { container } = render(<DesktopShellHarness />);

    expect(container.querySelectorAll('[aria-live="polite"], [role="status"]')).toHaveLength(1);
    expect(container.querySelector(".phase4c-qa-feedback")).not.toHaveAttribute("role", "status");
  });
});
