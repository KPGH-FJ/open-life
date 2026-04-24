import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import OnboardingWizard from "./OnboardingWizard";

const mockNavigate = vi.fn();
vi.mock("react-router-dom", async () => {
  const actual = await vi.importActual<typeof import("react-router-dom")>("react-router-dom");
  return { ...actual, useNavigate: () => mockNavigate };
});

describe("OnboardingWizard", () => {
  it("renders welcome step by default", () => {
    render(
      <MemoryRouter>
        <OnboardingWizard onComplete={() => {}} />
      </MemoryRouter>
    );
    expect(screen.getByText("欢迎使用 OpenLife")).toBeInTheDocument();
    expect(screen.getByText(/终身成长合伙人/)).toBeInTheDocument();
  });

  it("navigates to next step on click", () => {
    render(
      <MemoryRouter>
        <OnboardingWizard onComplete={() => {}} />
      </MemoryRouter>
    );
    fireEvent.click(screen.getByText("下一步"));
    expect(screen.getByText("配置对话后端")).toBeInTheDocument();
  });

  it("shows previous button after first step", () => {
    render(
      <MemoryRouter>
        <OnboardingWizard onComplete={() => {}} />
      </MemoryRouter>
    );
    fireEvent.click(screen.getByText("下一步"));
    expect(screen.getByText("上一步")).toBeInTheDocument();
  });

  it("calls onComplete when finish clicked", async () => {
    const onComplete = vi.fn();
    render(
      <MemoryRouter>
        <OnboardingWizard onComplete={onComplete} />
      </MemoryRouter>
    );
    fireEvent.click(screen.getByText("下一步"));
    fireEvent.click(screen.getByText("下一步"));
    fireEvent.click(screen.getByText("下一步"));
    fireEvent.click(screen.getByText("关闭引导，稍后再探索"));
    await waitFor(() => {
      expect(onComplete).toHaveBeenCalled();
    });
  });

  it("navigates to settings from step 2", () => {
    render(
      <MemoryRouter>
        <OnboardingWizard onComplete={() => {}} />
      </MemoryRouter>
    );
    fireEvent.click(screen.getByText("下一步"));
    fireEvent.click(screen.getByText("前往设置页配置"));
    expect(mockNavigate).toHaveBeenCalledWith("/settings");
  });

  it("renders privacy step content", () => {
    render(
      <MemoryRouter>
        <OnboardingWizard onComplete={() => {}} />
      </MemoryRouter>
    );
    fireEvent.click(screen.getByText("下一步"));
    fireEvent.click(screen.getByText("下一步"));
    fireEvent.click(screen.getByText("下一步"));
    expect(screen.getByText("准备开始试用")).toBeInTheDocument();
    expect(screen.getByText(/PII 检测引擎/)).toBeInTheDocument();
    expect(screen.getByText("推荐试用路线")).toBeInTheDocument();
    expect(screen.getByText("2. 开始第一次对话")).toBeInTheDocument();
  });

  it("navigates to chat from the final recommended route", () => {
    render(
      <MemoryRouter>
        <OnboardingWizard onComplete={() => {}} />
      </MemoryRouter>
    );
    fireEvent.click(screen.getByText("下一步"));
    fireEvent.click(screen.getByText("下一步"));
    fireEvent.click(screen.getByText("下一步"));
    fireEvent.click(screen.getByText("2. 开始第一次对话"));
    expect(mockNavigate).toHaveBeenCalledWith("/chat");
  });
});
