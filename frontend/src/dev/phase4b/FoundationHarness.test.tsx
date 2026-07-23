import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { FoundationHarness } from "./FoundationHarness";

describe("Phase 4B foundation harness interactions", () => {
  it("records approval without presenting it as applied", async () => {
    const user = userEvent.setup();
    render(<FoundationHarness />);

    await user.click(screen.getByRole("button", { name: "批准样例" }));
    expect(screen.getByRole("dialog", { name: "确认批准布局样例" })).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "确认批准" }));
    expect(screen.getByText("已批准，尚未应用")).toBeInTheDocument();
    expect(screen.getAllByText(/尚未应用，也未写入长期状态/).length).toBeGreaterThan(0);
    expect(screen.queryByText("已完成")).not.toBeInTheDocument();
  });

  it("gives unavailable navigation and evidence controls visible feedback", async () => {
    const user = userEvent.setup();
    render(<FoundationHarness />);

    await user.click(screen.getByRole("button", { name: /任务/ }));
    expect(screen.getAllByText(/任务页面尚未迁移/).length).toBeGreaterThan(0);

    await user.click(screen.getByRole("button", { name: /样例权限范围/ }));
    expect(screen.getAllByText(/这是布局证据，不是真实授权/).length).toBeGreaterThan(0);
  });

  it("announces dynamic feedback through one polite live region", () => {
    const { container } = render(<FoundationHarness />);

    expect(container.querySelectorAll('[aria-live="polite"], [role="status"]')).toHaveLength(1);
    expect(container.querySelector(".phase4b-feedback")).not.toHaveAttribute("role", "status");
  });
});
