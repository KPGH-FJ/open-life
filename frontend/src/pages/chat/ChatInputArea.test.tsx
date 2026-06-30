import { fireEvent, render, screen } from "@testing-library/react";
import type { ComponentProps } from "react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it, vi } from "vitest";
import ChatInputArea from "./ChatInputArea";

function renderComposer(
  overrides: Partial<ComponentProps<typeof ChatInputArea>> = {},
  options: { width?: number } = {}
) {
  const props: ComponentProps<typeof ChatInputArea> = {
    input: "你好",
    sending: false,
    streamInterrupted: false,
    diagnostics: null,
    selectedSkillId: "",
    onInputChange: vi.fn(),
    onSelectedSkillIdChange: vi.fn(),
    onComposerFocus: vi.fn(),
    onSend: vi.fn(),
    onContinueStream: vi.fn(),
    onRetryLastMessage: vi.fn(),
    getFixSuggestion: vi.fn(() => null),
    ...overrides,
  };

  const result = render(
    <MemoryRouter>
      <div style={options.width ? { width: `${options.width}px` } : undefined}>
        <ChatInputArea {...props} />
      </div>
    </MemoryRouter>
  );

  return { ...result, props };
}

describe("ChatInputArea", () => {
  it("exposes stable role/name controls for the composer", () => {
    renderComposer();

    expect(screen.getByRole("textbox", { name: "消息输入" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "发送消息" })).toBeEnabled();
  });

  it("uses distinct accessible names while sending and when stop is available", () => {
    const onCancel = vi.fn();
    renderComposer({ sending: true, canCancel: true, onCancel });

    expect(screen.getByRole("button", { name: "正在发送消息" })).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: "停止生成" }));

    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it("does not submit with Enter during Chinese IME composition", () => {
    const onSend = vi.fn();
    renderComposer({ input: "ni", onSend });

    const textarea = screen.getByRole("textbox", { name: "消息输入" });
    fireEvent.compositionStart(textarea);
    fireEvent.keyDown(textarea, { key: "Enter", code: "Enter" });

    expect(onSend).not.toHaveBeenCalled();

    fireEvent.compositionEnd(textarea);
    fireEvent.keyDown(textarea, { key: "Enter", code: "Enter" });

    expect(onSend).toHaveBeenCalledTimes(1);
  });

  it("keeps Shift+Enter available for textarea newlines", () => {
    const onSend = vi.fn();
    renderComposer({ onSend });

    const textarea = screen.getByRole("textbox", { name: "消息输入" });
    const shiftEnter = new KeyboardEvent("keydown", {
      key: "Enter",
      code: "Enter",
      shiftKey: true,
      bubbles: true,
      cancelable: true,
    });
    fireEvent(textarea, shiftEnter);

    expect(onSend).not.toHaveBeenCalled();
    expect(shiftEnter.defaultPrevented).toBe(false);
  });

  it.each([560, 720])("keeps long input scrollable and primary actions visible at %ipx", width => {
    renderComposer(
      {
        input: "这是一段很长的中文输入。".repeat(80),
        sending: true,
        canCancel: true,
        onCancel: vi.fn(),
      },
      { width }
    );

    expect(screen.getByRole("textbox", { name: "消息输入" })).toHaveClass(
      "min-w-0",
      "overflow-y-auto",
      "max-h-36"
    );
    expect(screen.getByRole("button", { name: "正在发送消息" })).toBeVisible();
    expect(screen.getByRole("button", { name: "停止生成" })).toBeVisible();
  });
});
