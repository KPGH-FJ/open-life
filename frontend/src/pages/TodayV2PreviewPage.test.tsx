import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import App from "../App";
import { mockInvoke } from "@/test/mocks/tauri";
import { buildTodayViewModelEnvelope } from "../viewmodels/today/todayViewModelAdapter";
import {
  emptyTodayViewModelInput,
  errorTodayViewModelInput,
  makeDailyGoal,
  makeLifeStateProjection,
  readyTodayViewModelInput,
  safeModeTodayViewModelInput,
  staleTodayViewModelInput,
} from "../viewmodels/today/todayViewModel.fixtures";
import TodayV2PreviewPage, { TodayV2PreviewSurface } from "./TodayV2PreviewPage";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

function renderSurface(envelope = buildTodayViewModelEnvelope(readyTodayViewModelInput)) {
  render(
    <MemoryRouter>
      <TodayV2PreviewSurface envelope={envelope} />
    </MemoryRouter>
  );
}

describe("TodayV2PreviewSurface", () => {
  it("renders ready state from the envelope daily summary, primary goal, review count, and primary actions", () => {
    renderSurface();

    expect(screen.getByTestId("today-v2-preview-page")).toBeInTheDocument();
    expect(screen.getByText("Today state is loaded")).toBeInTheDocument();
    expect(screen.getByText("Draft the weekly planning note")).toBeInTheDocument();
    expect(screen.getByText("待确认 2")).toBeInTheDocument();

    const primaryActions = screen.getByTestId("today-v2-primary-actions");
    expect(within(primaryActions).getByRole("link", { name: /刷新今日状态/ })).toHaveAttribute(
      "href",
      "/today-v2-preview"
    );
    expect(within(primaryActions).getByRole("link", { name: /打开当前工作入口/ })).toHaveAttribute(
      "href",
      "/companion"
    );
    expect(within(primaryActions).getByRole("link", { name: /查看待确认入口/ })).toHaveAttribute(
      "href",
      "/mailbox"
    );
  });

  it("renders empty state without a fake goal or invented next action", () => {
    renderSurface(buildTodayViewModelEnvelope(emptyTodayViewModelInput));

    expect(screen.getByText("No current daily goal or active task")).toBeInTheDocument();
    expect(screen.getByText("没有当前目标")).toBeInTheDocument();
    expect(screen.getByText("下一步暂未生成")).toBeInTheDocument();
    expect(screen.queryByText("Draft the weekly planning note")).not.toBeInTheDocument();
    expect(screen.queryByText(/先做 10 分钟/)).not.toBeInTheDocument();
  });

  it("renders error state with null data and no daily-goal fallback", () => {
    renderSurface(buildTodayViewModelEnvelope(errorTodayViewModelInput));

    expect(screen.getByText("TodayViewModel 不可用")).toBeInTheDocument();
    expect(screen.getAllByText("LifeStateProjection failed to load.").length).toBeGreaterThan(0);
    expect(screen.queryByText("Draft the weekly planning note")).not.toBeInTheDocument();
    expect(screen.queryByText("主要目标")).not.toBeInTheDocument();
  });

  it("shows stale state and disables stale-sensitive actions", () => {
    renderSurface(buildTodayViewModelEnvelope(staleTodayViewModelInput));

    expect(screen.getAllByText("stale").length).toBeGreaterThan(0);
    const primaryActions = screen.getByTestId("today-v2-primary-actions");
    expect(within(primaryActions).getByRole("button", { name: /打开当前工作入口/ })).toBeDisabled();
    expect(within(primaryActions).getByRole("link", { name: /刷新今日状态/ })).toHaveAttribute(
      "href",
      "/today-v2-preview"
    );
  });

  it("shows Safe Mode state without durable-write actions", () => {
    renderSurface(buildTodayViewModelEnvelope(safeModeTodayViewModelInput));

    expect(screen.getByText("Safe Mode（安全模式）")).toBeInTheDocument();
    expect(screen.getByText(/长期写入 已阻断/)).toBeInTheDocument();

    for (const label of ["保存", "应用", "写入长期状态", "同意", "接受全部"]) {
      expect(screen.queryByRole("button", { name: label })).not.toBeInTheDocument();
      expect(screen.queryByRole("link", { name: label })).not.toBeInTheDocument();
    }
  });

  it("renders pending review count from envelope data instead of surface rows", () => {
    const projection = makeLifeStateProjection({
      pending: {
        pendingProposalCount: 3,
        editedProposalCount: 0,
        totalReviewRequiredCount: 3,
        highRiskReviewRequiredCount: 1,
        proposalStoreStatus: "ok",
        requiresUserAction: true,
      },
      surfaces: [
        {
          surface: "today",
          pendingReviewCount: 99,
          editedReviewCount: 99,
          totalReviewRequiredCount: 99,
          readinessStatus: "ready",
          taskStatus: "idle",
          safeModeActive: false,
          waitingPermissionCount: 0,
          activeToolPermissionCount: 0,
        },
      ],
    });

    renderSurface(
      buildTodayViewModelEnvelope({
        projection,
        dailyGoals: [makeDailyGoal()],
      })
    );

    expect(screen.getByText("待确认 3")).toBeInTheDocument();
    expect(screen.queryByText("待确认 99")).not.toBeInTheDocument();
  });

  it("keeps debug-only actions out of the primary action row", () => {
    renderSurface();

    const primaryActions = screen.getByTestId("today-v2-primary-actions");
    expect(
      within(primaryActions).queryByText("Inspect projection source refs")
    ).not.toBeInTheDocument();

    const advancedLane = screen.getByTestId("today-v2-advanced-lane");
    expect(advancedLane).not.toHaveAttribute("open");
    expect(within(advancedLane).getByText("Inspect projection source refs")).toBeInTheDocument();
  });

  it("keeps evidence and debug content only in the collapsed advanced lane", () => {
    renderSurface();

    const advancedLane = screen.getByTestId("today-v2-advanced-lane");
    expect(advancedLane).not.toHaveAttribute("open");
    expect(within(advancedLane).getByText("LifeStateProjection.pending")).toBeInTheDocument();
    expect(within(advancedLane).getByText("Debug only")).toBeInTheDocument();
    expect(screen.getByText("LifeStateProjection.pending").closest("details")).toBe(advancedLane);
  });
});

describe("TodayV2PreviewPage container and route boundary", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_life_state_projection") return Promise.resolve(makeLifeStateProjection());
      if (cmd === "get_daily_goals") return Promise.resolve([makeDailyGoal()]);
      return mockInvoke(cmd, args);
    });
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("loads only the temporary projection and daily-goal inputs before building the envelope", async () => {
    render(
      <MemoryRouter>
        <TodayV2PreviewPage />
      </MemoryRouter>
    );

    expect(await screen.findByTestId("today-v2-preview-page")).toBeInTheDocument();
    await waitFor(() => {
      const calledCommands = vi.mocked(invoke).mock.calls.map(([command]) => command);
      expect(calledCommands).toContain("get_life_state_projection");
      expect(calledCommands).toContain("get_daily_goals");
      expect(calledCommands).not.toContain("get_system_diagnostics");
      expect(calledCommands).not.toContain("list_proposals");
      expect(calledCommands).not.toContain("get_pending_proposals");
      expect(calledCommands).not.toContain("get_state_alerts");
    });
  });

  it("keeps the existing /today route on TodayPage after adding the preview route", async () => {
    render(
      <MemoryRouter initialEntries={["/today"]}>
        <App />
      </MemoryRouter>
    );

    expect(await screen.findByTestId("today-page")).toBeInTheDocument();
    expect(screen.queryByTestId("today-v2-preview-page")).not.toBeInTheDocument();
  });

  it("keeps the preview surface free of forbidden raw-domain helpers and write wrappers", () => {
    const source = readFileSync(join(process.cwd(), "src/pages/TodayV2PreviewPage.tsx"), "utf8");

    expect(source).toMatch(/\bbuildTodayViewModelEnvelope\b/);
    for (const forbidden of [
      "dailyGoalDisplayGuard",
      "reviewRequiredCountFromProjection",
      "tauriDev",
      "getSystemDiagnostics",
      "listProposals",
      "getPendingProposals",
      "acceptProposal",
      "batchAcceptLowRiskProposals",
      "addDailyGoal",
      "updateDailyGoal",
    ]) {
      expect(source, `preview source must not use ${forbidden}`).not.toContain(forbidden);
    }
  });
});
