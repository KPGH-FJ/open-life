import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import ErrorBanner from "./ErrorBanner";

describe("ErrorBanner", () => {
  it("renders nothing when message is empty", () => {
    const { container } = render(<ErrorBanner message="" />);
    expect(container.firstChild).toBeNull();
  });

  it("renders error with icon and message", () => {
    render(<ErrorBanner message="Something failed" severity="error" />);
    expect(screen.getByRole("alert")).toHaveTextContent("Something failed");
  });

  it("calls onClose when X clicked", () => {
    const onClose = vi.fn();
    render(<ErrorBanner message="Close me" onClose={onClose} />);
    fireEvent.click(screen.getByLabelText("关闭"));
    expect(onClose).toHaveBeenCalled();
  });

  it("auto-hides after timeout", async () => {
    const onClose = vi.fn();
    render(<ErrorBanner message="Auto" autoHide autoHideMs={100} onClose={onClose} />);
    await waitFor(() => expect(onClose).toHaveBeenCalled(), { timeout: 300 });
  });

  it.each([
    ["error", "bg-rose-50"],
    ["warning", "bg-amber-50"],
    ["info", "bg-blue-50"],
  ] as const)("applies %s severity style", (severity, cls) => {
    render(<ErrorBanner message="x" severity={severity} />);
    expect(screen.getByRole("alert")).toHaveClass(cls);
  });
});
