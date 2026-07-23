import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { Phase4dReadOnlyHarness } from "./Phase4dReadOnlyHarness";

describe("Phase 4D read-only harness", () => {
  it("keeps source selection outside the product shell and labels browser fixtures", async () => {
    const previousTauriInternals = window.__TAURI_INTERNALS__;
    window.__TAURI_INTERNALS__ = false;
    const user = userEvent.setup();
    try {
      const { container } = render(<Phase4dReadOnlyHarness />);
      const toolbar = screen.getByRole("banner", { name: "Phase 4D QA 工具栏" });
      const shell = container.querySelector(".ol-workbench-shell");
      const source = within(toolbar).getByLabelText("数据来源");

      expect(shell).toBeInTheDocument();
      expect(shell).not.toContainElement(source);
      expect(source).toHaveValue("fixture-ready");
      expect(screen.getByText("静态 fixture · 非后端状态")).toBeInTheDocument();
      expect(within(source).getByRole("option", { name: /真实 Tauri 后端/ })).toBeDisabled();
      await screen.findByText("整理下周客户访谈要验证的三个问题");

      await user.selectOptions(source, "fixture-stale");
      expect(await screen.findByText("当前计划已陈旧，只读且不执行")).toBeInTheDocument();
      expect(screen.getByRole("status")).toHaveTextContent("静态样例：数据陈旧");
    } finally {
      window.__TAURI_INTERNALS__ = previousTauriInternals;
    }
  });
});
