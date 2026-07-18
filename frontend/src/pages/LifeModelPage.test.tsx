import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, fireEvent, within } from "@testing-library/react";
import { BrowserRouter } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import LifeModelPage from "./LifeModelPage";
import { createMockLifeModelViewModelEnvelope, mockInvoke } from "@/test/mocks/tauri";
import type { LifeModelViewModel, ViewModelEnvelope } from "@/tauri";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

function renderPage() {
  render(
    <BrowserRouter>
      <LifeModelPage />
    </BrowserRouter>
  );
}

function mockLifeModelEnvelope(envelope: ViewModelEnvelope<LifeModelViewModel>) {
  vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
    if (cmd === "get_life_model_view_model") return Promise.resolve(envelope);
    return mockInvoke(cmd, args);
  });
}

describe("LifeModelPage", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) =>
      mockInvoke(cmd, args)
    );
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("renders the Life Model product page and switches build, overview, and evidence sections", async () => {
    renderPage();

    expect(await screen.findByTestId("life-model-page")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Life Model" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "构建" })).toHaveAttribute("aria-selected", "true");
    expect(screen.getAllByText("构建状态").length).toBeGreaterThan(0);
    expect(screen.getByText("快速构建")).toBeInTheDocument();
    expect(screen.getByText("对话构建")).toBeInTheDocument();
    expect(screen.getByText("从已有内容整理")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("tab", { name: "概览" }));
    expect(screen.getByRole("tab", { name: "概览" })).toHaveAttribute("aria-selected", "true");
    expect(await screen.findByText("Identity")).toBeInTheDocument();
    expect(screen.getByText("Goals")).toBeInTheDocument();
    expect(screen.getByText("Capabilities")).toBeInTheDocument();
    expect(screen.getByText("State")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("tab", { name: "依据" }));
    expect(screen.getByRole("tab", { name: "依据" })).toHaveAttribute("aria-selected", "true");
    expect(await screen.findByText("记忆条数")).toBeInTheDocument();
    expect(screen.getByText("待确认更新")).toBeInTheDocument();
  });

  it("calls the backend LifeModelViewModel owner instead of raw reconstruction commands", async () => {
    renderPage();

    await waitFor(() => {
      const calledCommands = vi.mocked(invoke).mock.calls.map(([command]) => command);
      expect(calledCommands).toContain("get_life_model_view_model");
      expect(calledCommands).toContain("get_life_state_projection");
      expect(calledCommands).toContain("builder_list_unfinished");
      for (const forbidden of [
        "get_life_model",
        "get_life_model_current_view",
        "get_system_diagnostics",
        "get_model_4d_completion",
        "count_memory_chunks",
        "get_memory_tier_stats",
        "list_proposals",
      ]) {
        expect(calledCommands).not.toContain(forbidden);
      }
    });
  });

  it("summarizes the Life Model without raw JSON or raw evidence payloads", async () => {
    renderPage();

    fireEvent.click(await screen.findByRole("tab", { name: "概览" }));
    expect(await screen.findByText("测试用户")).toBeInTheDocument();
    expect(screen.queryByText("RAW_LIFEMODEL_JSON_SHOULD_NOT_RENDER")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("tab", { name: "依据" }));
    expect(await screen.findByText("最近依据来源")).toBeInTheDocument();
    const pendingPrimary = await screen.findByTestId(
      "life-model-pending-proposal-primary-proposal:proposal-life-model-1"
    );
    expect(pendingPrimary).toHaveTextContent("OpenLife 发现一条候选更新");
    expect(
      within(pendingPrimary).queryByText("RAW_EVIDENCE_PAYLOAD_SHOULD_NOT_RENDER")
    ).not.toBeInTheDocument();
  });

  it("shows current compatibility and materialization evidence from the backend ViewModel", async () => {
    const base = createMockLifeModelViewModelEnvelope();
    mockLifeModelEnvelope(
      createMockLifeModelViewModelEnvelope({
        data: {
          ...base.data!,
          currentViewSummary: {
            currentViewRef: {
              id: "lifemodel-current:preferences.communication_style",
              kind: "lifemodel",
              label: "沟通偏好",
            },
            compatibilityMode: true,
            label: "沟通偏好",
            summary: "先共情，再给结构化建议",
            divergenceFromCanonical: "unknown",
            evidenceRefs: [
              {
                id: "lifemodel-patch:patch-communication-1",
                label: "Applied LifeModel patch",
                source: "lifemodel",
                sensitivity: "local_private",
              },
            ],
            ownerStatus: "PARTIAL",
          },
          materializedChanges: [
            {
              changeRef: {
                id: "proposal:proposal-communication-1",
                kind: "proposal",
                label: "沟通偏好已物化",
              },
              title: "沟通偏好已物化",
              materializationStatus: "applied",
              materializedAt: null,
              rollbackAvailable: false,
              evidenceRefs: [],
            },
          ],
        },
      })
    );

    renderPage();
    fireEvent.click(await screen.findByRole("tab", { name: "概览" }));

    expect(await screen.findByTestId("communication-style-current-view")).toHaveTextContent(
      "先共情，再给结构化建议"
    );
    expect(screen.getByText("沟通偏好")).toBeInTheDocument();
    expect(
      screen.getByText("lifemodel-current:preferences.communication_style")
    ).toBeInTheDocument();

    fireEvent.click(screen.getByRole("tab", { name: "依据" }));
    expect(await screen.findByText("沟通偏好已物化")).toBeInTheDocument();
    expect(screen.getByText("后端物化证据")).toBeInTheDocument();
  });

  it("renders a light empty state when the backend ViewModel is empty", async () => {
    const base = createMockLifeModelViewModelEnvelope();
    mockLifeModelEnvelope(
      createMockLifeModelViewModelEnvelope({
        status: "empty",
        data: {
          ...base.data!,
          truthMode: "unknown",
          currentViewSummary: null,
          dimensionSummaries: [],
          pendingUpdateCounts: {
            candidate: 0,
            pendingReview: 0,
            approvedNotApplied: 0,
            failedMaterialization: 0,
            ownerStatus: "PARTIAL",
          },
          candidateChanges: [],
        },
      })
    );

    renderPage();
    fireEvent.click(await screen.findByRole("tab", { name: "概览" }));

    expect(await screen.findByText("模型还没有形成稳定摘要")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "去构建" })).toHaveAttribute(
      "href",
      "/life-model/build"
    );
  });

  it("keeps builder, memory, and mailbox reachable", async () => {
    renderPage();

    expect(await screen.findByRole("link", { name: "开始快速构建" })).toHaveAttribute(
      "href",
      "/life-model/build"
    );
    expect(screen.getByRole("link", { name: "开始对话构建" })).toHaveAttribute(
      "href",
      "/life-model/build"
    );
    expect(screen.getByRole("button", { name: "暂不可用" })).toBeDisabled();

    fireEvent.click(screen.getByRole("tab", { name: "依据" }));
    expect(screen.getByRole("link", { name: "查看记忆" })).toHaveAttribute("href", "/memory");
    expect(screen.getByRole("link", { name: "打开 Mailbox" })).toHaveAttribute("href", "/mailbox");
  });

  it("uses product language instead of engineering readiness/proposal labels in the build tab", async () => {
    renderPage();

    expect((await screen.findAllByText("构建状态")).length).toBeGreaterThan(0);
    expect(
      screen.getByText("构建产生候选，Mailbox 确认后才会更新 Life Model。")
    ).toBeInTheDocument();
    expect(screen.queryByText("Builder readiness")).not.toBeInTheDocument();
    expect(screen.queryByText(/Builder review/)).not.toBeInTheDocument();
    expect(screen.queryByText(/proposal/i)).not.toBeInTheDocument();
  });

  it("does not show direct-write actions in Safe Mode", async () => {
    mockLifeModelEnvelope(createMockLifeModelViewModelEnvelope());
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "get_life_model_view_model") {
        return Promise.resolve(createMockLifeModelViewModelEnvelope());
      }
      if (cmd === "get_life_state_projection") {
        return mockInvoke(cmd, args).then((projection: any) => ({
          ...projection,
          safeMode: {
            active: true,
            reason: "memory.db 初始化失败，正在使用临时数据库",
            sourceRefs: ["startup_warnings"],
          },
        }));
      }
      return mockInvoke(cmd, args);
    });

    renderPage();

    expect(await screen.findByTestId("life-model-page")).toBeInTheDocument();
    for (const label of ["保存模型", "应用更改", "直接写入", "批量接受", "接受全部"]) {
      expect(screen.queryByRole("button", { name: label })).not.toBeInTheDocument();
    }
    expect(screen.getByRole("link", { name: "开始快速构建" })).toBeInTheDocument();
  });

  it("does not import raw reconstruction, write, migration, or Skill Runtime wrappers", () => {
    const sourcePath = join(process.cwd(), "src/pages/LifeModelPage.tsx");
    const source = readFileSync(sourcePath, "utf8");
    for (const forbidden of [
      "getLifeModel(",
      "getLifeModelCurrentView(",
      "getSystemDiagnostics(",
      "getModel4DCompletion(",
      "countMemoryChunks(",
      "getMemoryTierStats(",
      "listProposals(",
      "saveLifeModel",
      "builderApplySignals",
      "batchAcceptLowRiskProposals",
      "runMultiStrategyAgentPreview",
      "run_skill",
      "runSkill",
      "get_skill_runtime_status",
      "getSkillRuntimeStatus",
      "check_runtime_migration_gate",
      "checkRuntimeMigrationGate",
    ]) {
      expect(source).not.toContain(forbidden);
    }
  });
});
