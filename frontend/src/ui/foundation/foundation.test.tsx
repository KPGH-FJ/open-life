import { useState } from "react";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { Check, RotateCcw } from "lucide-react";
import { describe, expect, it, vi } from "vitest";
import {
  FoundationActionButton,
  FoundationDialog,
  FoundationEvidenceRow,
  FoundationIconButton,
  FoundationNavRow,
  FoundationStatusLabel,
  FoundationTextField,
  FoundationToggle,
} from "./foundation";

describe("OpenLife UI foundation primitives", () => {
  it("requires and exposes a visible disabled reason", () => {
    render(
      <>
        <FoundationActionButton label="应用变更" disabled disabledReason="缺少后端应用命令。" />
        <FoundationIconButton
          label="关闭"
          icon={<RotateCcw size={18} />}
          disabled
          disabledReason="提交期间不能关闭。"
        />
        <FoundationTextField
          id="disabled-provider"
          label="供应商地址"
          disabled
          disabledReason="当前配置来源未知，暂时不能编辑。"
        />
      </>
    );

    const button = screen.getByRole("button", { name: "应用变更" });
    const reason = screen.getByText("缺少后端应用命令。");
    expect(button).toBeDisabled();
    expect(button).toHaveAttribute("aria-describedby", reason.id);
    expect(screen.getByRole("button", { name: "关闭" })).toHaveAttribute(
      "aria-describedby",
      screen.getByText("提交期间不能关闭。").id
    );
    expect(screen.getByLabelText("供应商地址")).toHaveAttribute(
      "aria-describedby",
      screen.getByText("当前配置来源未知，暂时不能编辑。").id
    );
  });

  it("rejects disabled controls without a reason and unverified green success", () => {
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => undefined);
    try {
      expect(() => render(<FoundationActionButton label="无效动作" disabled />)).toThrow(
        "disabledReason"
      );
      expect(() =>
        render(<FoundationTextField id="invalid-disabled-field" label="无效字段" disabled />)
      ).toThrow("disabledReason");
      expect(() => render(<FoundationStatusLabel label="未经验证" status="success" />)).toThrow(
        "verified=true"
      );
    } finally {
      consoleError.mockRestore();
    }
  });

  it("renders unknown toggle as status instead of a false off switch", () => {
    render(
      <FoundationToggle label="当前传输边界" description="未知不能表现为关闭。" state="unknown" />
    );

    expect(screen.getByText("状态未知")).toBeInTheDocument();
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
    expect(screen.queryByRole("switch")).not.toBeInTheDocument();
  });

  it("supports toggle, icon, nav, evidence, and field keyboard actions", async () => {
    const user = userEvent.setup();
    const onToggle = vi.fn();
    const onReset = vi.fn();
    const onNavigate = vi.fn();
    const onEvidence = vi.fn();
    render(
      <>
        <FoundationToggle label="网络策略" state="off" onChange={onToggle} />
        <FoundationIconButton label="重置" icon={<RotateCcw size={18} />} onClick={onReset} />
        <FoundationNavRow label="今日" icon={<Check size={18} />} current onClick={onNavigate} />
        <FoundationEvidenceRow
          id="evidence_fixture_scope"
          label="权限范围"
          source="fixture.scope"
          sensitivity="local_private"
          onOpen={onEvidence}
        />
        <FoundationTextField
          id="provider"
          label="供应商"
          description="配置样例"
          error="当前值不可用"
        />
      </>
    );

    await user.click(screen.getByRole("switch", { name: "网络策略" }));
    await user.click(screen.getByRole("button", { name: "重置" }));
    await user.click(screen.getByRole("button", { name: "今日" }));
    await user.click(screen.getByRole("button", { name: /权限范围/ }));

    expect(onToggle).toHaveBeenCalledWith("on");
    expect(onReset).toHaveBeenCalledOnce();
    expect(onNavigate).toHaveBeenCalledOnce();
    expect(onEvidence).toHaveBeenCalledOnce();
    expect(screen.getByRole("button", { name: /权限范围/ })).toHaveAttribute(
      "data-evidence-id",
      "evidence_fixture_scope"
    );
    expect(screen.getByRole("button", { name: "今日" })).toHaveAttribute("aria-current", "page");
    expect(screen.getByLabelText("供应商")).toHaveAttribute("aria-invalid", "true");
  });

  it("traps Escape handling and restores focus when a dialog closes", async () => {
    const user = userEvent.setup();

    function DialogFixture() {
      const [open, setOpen] = useState(false);
      return (
        <>
          <FoundationActionButton label="打开确认" onClick={() => setOpen(true)} />
          <FoundationDialog
            open={open}
            title="确认样例"
            description="仅验证对话框行为"
            onClose={() => setOpen(false)}
            footer={<FoundationActionButton label="取消" onClick={() => setOpen(false)} />}
          />
        </>
      );
    }

    render(<DialogFixture />);
    const opener = screen.getByRole("button", { name: "打开确认" });
    await user.click(opener);

    expect(screen.getByRole("dialog", { name: "确认样例" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "确认样例" })).toHaveFocus();

    await user.keyboard("{Shift>}{Tab}{/Shift}");
    expect(screen.getByRole("button", { name: "取消" })).toHaveFocus();
    await user.tab();
    expect(screen.getByRole("button", { name: "关闭对话框" })).toHaveFocus();

    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(opener).toHaveFocus();
  });

  it("keeps dialog focus stable when busy state and inline callbacks rerender", async () => {
    const user = userEvent.setup();

    function BusyDialogFixture() {
      const [open, setOpen] = useState(false);
      const [busy, setBusy] = useState(false);
      return (
        <>
          <FoundationActionButton label="打开忙碌确认" onClick={() => setOpen(true)} />
          <FoundationDialog
            open={open}
            busy={busy}
            title="忙碌确认样例"
            onClose={() => setOpen(false)}
            footer={
              <FoundationActionButton
                label="切换提交状态"
                onClick={() => setBusy(current => !current)}
              />
            }
          />
        </>
      );
    }

    render(<BusyDialogFixture />);
    await user.click(screen.getByRole("button", { name: "打开忙碌确认" }));
    const busyToggle = screen.getByRole("button", { name: "切换提交状态" });

    await user.click(busyToggle);
    expect(busyToggle).toHaveFocus();
    await user.keyboard("{Escape}");
    expect(screen.getByRole("dialog", { name: "忙碌确认样例" })).toBeInTheDocument();
    expect(busyToggle).toHaveFocus();

    await user.click(busyToggle);
    expect(busyToggle).toHaveFocus();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "打开忙碌确认" })).toHaveFocus();
  });
});
