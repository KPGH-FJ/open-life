import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import AgentRunDetail from "./AgentRunDetail";
import { invoke } from "@tauri-apps/api/core";
import {
  successfulGovernedRun,
  agentSpecDeniedToolRun,
  needsConfirmationRun,
  replayFailedRun,
} from "@/test/fixtures/agentRunEvents";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const mockRun = {
  id: "run-typed-test",
  taskId: "task-1",
  sessionId: "sess-1",
  status: "completed",
  kind: "conversation",
  userInput: "test input",
  outputPreview: "test output",
  generatedProposals: [],
  actions: [
    {
      id: "action-1",
      actionType: "tool_call",
      status: "blocked",
      input: {},
      timestamp: "2026-05-15T10:00:00Z",
      toolScope: {
        toolId: "web.search",
        toolName: "web.search",
        source: "builtin",
        riskLevel: "medium",
        capabilities: ["network"],
        actionType: "read",
      },
    },
  ],
  observations: [],
  stepCount: 1,
  toolCallCount: 1,
  startedAt: "2026-05-15T10:00:00Z",
  finishedAt: "2026-05-15T10:01:00Z",
};

const mockEvents = [
  {
    id: "evt-replay-fail",
    runId: "run-typed-test",
    eventType: "replay.failed",
    actor: "runtime",
    summary: "Replay failed: ReplaySpecMissing",
    payload: {
      status: "failed",
      run_id: "run-typed-test",
      action_id: "action-replay-2",
      replay_of_action_id: "action-orig-2",
      human_message: "Replay failed: missing action spec",
      block_reason: "replay_spec_missing",
      failure_kind: null,
      tool_name: "remote_tool",
      source: "mcp:my-server",
      agent_spec_id: "main.default",
    },
    createdAt: "2026-05-15T10:01:00Z",
  },
  {
    id: "evt-tool-blocked",
    runId: "run-typed-test",
    eventType: "tool.call_blocked",
    actor: "runtime",
    summary: "web.search blocked by AgentSpec",
    payload: {
      status: "blocked",
      tool_name: "web.search",
      source: "builtin",
      block_reason: "agent_spec_denied",
      proposal_reason: null,
      failure_kind: null,
      agent_spec_id: "main.default",
    },
    createdAt: "2026-05-15T10:00:30Z",
  },
  {
    id: "evt-run-completed",
    runId: "run-typed-test",
    eventType: "run.completed",
    actor: "runtime",
    summary: "Run completed",
    payload: { stop_reason: "no_tools" },
    createdAt: "2026-05-15T10:02:00Z",
  },
];

function renderPage(runId: string) {
  return render(
    <MemoryRouter initialEntries={[`/runs/${runId}`]}>
      <Routes>
        <Route path="/runs/:runId" element={<AgentRunDetail />} />
      </Routes>
    </MemoryRouter>
  );
}

describe("AgentRunDetail", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockImplementation(async (cmd: string, _args?: Record<string, any>) => {
      if (cmd === "get_agent_run") return mockRun;
      if (cmd === "list_agent_run_events") return mockEvents;
      if (cmd === "list_agent_plans_for_run") return [];
      return undefined;
    });
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("shows replay failed trace item from typed event via RunTracePanel", async () => {
    renderPage("run-typed-test");

    expect(await screen.findByText("Chat")).toBeInTheDocument();

    // Expand the RunTracePanel (事件时间线)
    await userEvent.click(screen.getByTestId("run-trace-toggle"));

    // Find the replay.failed event row and click to expand
    const replayBtn = screen.getByTestId("event-row-evt-replay-fail");
    fireEvent.click(replayBtn);

    // Typed contract detail should show the block reason
    expect(await screen.findByText("重放失败 (Typed Contract)")).toBeInTheDocument();
    expect(screen.getAllByText("缺少重放规格").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("remote_tool").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("mcp:my-server").length).toBeGreaterThanOrEqual(1);
  });

  it("shows tool blocked trace item from typed event", async () => {
    renderPage("run-typed-test");

    expect(await screen.findByText("Chat")).toBeInTheDocument();

    // Expand the trace panel
    await userEvent.click(screen.getByTestId("run-trace-toggle"));

    // Verify the blocked event summary appears
    expect(await screen.findByText("web.search blocked by AgentSpec")).toBeInTheDocument();

    // Click the event row to expand it
    const eventBtn = screen.getByTestId("event-row-evt-tool-blocked");
    fireEvent.click(eventBtn);

    // Typed contract fields should now be visible
    expect(await screen.findByText("阻断详情 (Typed Contract)")).toBeInTheDocument();
    expect(screen.getAllByText("AgentSpec 拒绝").length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText("main.default").length).toBeGreaterThanOrEqual(1);
  });

  it("uses typed payload fields not summary text for replay reason via RunTracePanel", async () => {
    renderPage("run-typed-test");

    expect(await screen.findByText("Chat")).toBeInTheDocument();

    // Expand the RunTracePanel
    await userEvent.click(screen.getByTestId("run-trace-toggle"));

    // Expand the replay.failed event
    const replayBtn = screen.getByTestId("event-row-evt-replay-fail");
    fireEvent.click(replayBtn);

    // Must show typed block_reason from payload, not derived from summary
    expect(await screen.findByText("重放失败 (Typed Contract)")).toBeInTheDocument();
    // "缺少重放规格" comes from block_reason in payload, not from summary text
    expect(screen.getAllByText("缺少重放规格").length).toBeGreaterThanOrEqual(1);
  });

  // ── Batch 6: Explainability tests ──────────────────────────────────

  it("renders run-level explanation panel above trace", async () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string, _args?: Record<string, any>) => {
      if (cmd === "get_agent_run") return mockRun;
      if (cmd === "list_agent_run_events") return mockEvents;
      if (cmd === "list_agent_plans_for_run") return [];
      return undefined;
    });

    renderPage("run-typed-test");

    expect(await screen.findByText("Chat")).toBeInTheDocument();

    // Run explanation panel should be present (driven by typed events)
    expect(screen.getByTestId("run-explanation-panel")).toBeInTheDocument();
    // Headline should mention issues since events contain blocked + replay failed
    expect(screen.getByText(/运行遇到问题/)).toBeInTheDocument();
  });

  it("AgentSpec denied run shows adjust AgentSpec next action", async () => {
    const agentSpecDeniedEvents = [
      {
        id: "evt-blocked",
        runId: "run-001",
        eventType: "tool.call_blocked",
        actor: "runtime",
        summary: "blocked",
        payload: {
          status: "blocked",
          tool_name: "web.search",
          source: "builtin",
          block_reason: "agent_spec_denied",
          agent_spec_id: "main.default",
        },
        createdAt: "2026-05-16T10:00:00Z",
      },
    ];
    vi.mocked(invoke).mockImplementation(async (cmd: string, _args?: Record<string, any>) => {
      if (cmd === "get_agent_run") return { ...mockRun, id: "run-agentspec-deny" };
      if (cmd === "list_agent_run_events") return agentSpecDeniedEvents;
      if (cmd === "list_agent_plans_for_run") return [];
      return undefined;
    });

    renderPage("run-agentspec-deny");

    expect(await screen.findByTestId("run-explanation-panel")).toBeInTheDocument();
    expect(screen.getByText(/调整 AgentSpec/)).toBeInTheDocument();
  });

  it("needs confirmation run shows review/grant permission next action", async () => {
    const needsConfEvents = [
      {
        id: "evt-needsconf",
        runId: "run-001",
        eventType: "tool.call_blocked",
        actor: "runtime",
        summary: "needs confirmation",
        payload: {
          status: "needs_confirmation",
          tool_name: "web.search",
          source: "builtin",
          proposal_reason: "network_policy_ask",
          proposal_id: "prop-1",
        },
        createdAt: "2026-05-16T10:00:00Z",
      },
    ];
    vi.mocked(invoke).mockImplementation(async (cmd: string, _args?: Record<string, any>) => {
      if (cmd === "get_agent_run") return { ...mockRun, id: "run-needsconf" };
      if (cmd === "list_agent_run_events") return needsConfEvents;
      if (cmd === "list_agent_plans_for_run") return [];
      return undefined;
    });

    renderPage("run-needsconf");

    expect(await screen.findByTestId("run-explanation-panel")).toBeInTheDocument();
    expect(screen.getByText(/审查待确认提案/)).toBeInTheDocument();
    expect(screen.getByText(/授予所需工具权限/)).toBeInTheDocument();
  });

  it("replay failed run shows retry/inspect next action", async () => {
    const replayFailEvents = [
      {
        id: "evt-replayfail",
        runId: "run-001",
        eventType: "replay.failed",
        actor: "runtime",
        summary: "replay failed",
        payload: {
          status: "failed",
          run_id: "run-001",
          action_id: "a1",
          replay_of_action_id: "orig-1",
          block_reason: "replay_spec_missing",
        },
        createdAt: "2026-05-16T10:00:00Z",
      },
    ];
    vi.mocked(invoke).mockImplementation(async (cmd: string, _args?: Record<string, any>) => {
      if (cmd === "get_agent_run") return { ...mockRun, id: "run-replayfail" };
      if (cmd === "list_agent_run_events") return replayFailEvents;
      if (cmd === "list_agent_plans_for_run") return [];
      return undefined;
    });

    renderPage("run-replayfail");

    expect(await screen.findByTestId("run-explanation-panel")).toBeInTheDocument();
    expect(screen.getAllByText(/重放失败/).length).toBeGreaterThanOrEqual(1);
    expect(screen.getAllByText(/查看详细 trace/).length).toBeGreaterThanOrEqual(1);
  });

  it("developer info is not duplicated — agentSpecId only in developerBullets, no separate row", async () => {
    const eventsWithSpec = [
      {
        id: "evt-spec",
        runId: "run-001",
        eventType: "agent_spec.selected",
        actor: "runtime",
        summary: "Spec selected",
        payload: {
          agent_spec_id: "main.default",
          role: "main",
          privacy_policy: "cloud_allowed",
        },
        createdAt: "2026-05-16T10:00:00Z",
      },
      {
        id: "evt-prompt",
        runId: "run-001",
        eventType: "prompt_stack.assembled",
        actor: "runtime",
        summary: "PromptStack assembled",
        payload: {
          agent_spec_id: "main.default",
          prompt_blocks: [{ id: "base_system", version: "1.0.0" }],
        },
        createdAt: "2026-05-16T10:00:01Z",
      },
      {
        id: "evt-completed",
        runId: "run-001",
        eventType: "run.completed",
        actor: "runtime",
        summary: "Run completed",
        payload: {},
        createdAt: "2026-05-16T10:00:02Z",
      },
    ];
    vi.mocked(invoke).mockImplementation(async (cmd: string, _args?: Record<string, any>) => {
      if (cmd === "get_agent_run") return { ...mockRun, id: "run-no-dup" };
      if (cmd === "list_agent_run_events") return eventsWithSpec;
      if (cmd === "list_agent_plans_for_run") return [];
      return undefined;
    });

    renderPage("run-no-dup");

    expect(await screen.findByTestId("run-explanation-panel")).toBeInTheDocument();

    // Expand developer info
    const devSummary = screen.getByText("开发者信息");
    fireEvent.click(devSummary);

    // The separate "AgentSpec ID:" label must NOT appear (only in developerBullets)
    expect(screen.queryByText("AgentSpec ID:")).toBeNull();
    // But the AgentSpec info IS in developerBullets (checked via text that contains "AgentSpec:")
    expect(screen.getByText(/AgentSpec:/)).toBeInTheDocument();

    // Prompt block info is in developerBullets, NOT as a separate "Prompt blocks:" row
    expect(screen.queryByText("Prompt blocks:")).toBeNull();
  });

  // ── Fixture-based explainability end-to-end tests ──────────────────

  it("fixture: successfulGovernedRun — no misleading nextActions, developer info present", async () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string, _args?: Record<string, any>) => {
      if (cmd === "get_agent_run") return { ...mockRun, id: "run-success" };
      if (cmd === "list_agent_run_events") return successfulGovernedRun;
      if (cmd === "list_agent_plans_for_run") return [];
      return undefined;
    });

    renderPage("run-success");
    expect(await screen.findByTestId("run-explanation-panel")).toBeInTheDocument();

    // Success run should NOT show "建议操作" section at all
    expect(screen.queryByText("建议操作")).toBeNull();
    // No misleading "查看运行 trace" or "查看详细 trace 进行审计"
    expect(screen.queryByText("查看运行 trace")).toBeNull();
    expect(screen.queryByText("查看详细 trace 进行审计")).toBeNull();

    // Developer info collapsible is present
    const devSummary = screen.getByText("开发者信息");
    fireEvent.click(devSummary);
    expect(screen.getByText(/AgentSpec: main.default/)).toBeInTheDocument();
    expect(screen.getByText(/隐私策略: local_only/)).toBeInTheDocument();
  });

  it("fixture: agentSpecDeniedToolRun — error, adjust_agent_spec visible", async () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string, _args?: Record<string, any>) => {
      if (cmd === "get_agent_run") return { ...mockRun, id: "run-denied" };
      if (cmd === "list_agent_run_events") return agentSpecDeniedToolRun;
      if (cmd === "list_agent_plans_for_run") return [];
      return undefined;
    });

    renderPage("run-denied");
    expect(await screen.findByTestId("run-explanation-panel")).toBeInTheDocument();
    expect(screen.getByText(/调整 AgentSpec/)).toBeInTheDocument();
  });

  it("fixture: needsConfirmationRun — warning, review/grant visible", async () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string, _args?: Record<string, any>) => {
      if (cmd === "get_agent_run") return { ...mockRun, id: "run-needsconf" };
      if (cmd === "list_agent_run_events") return needsConfirmationRun;
      if (cmd === "list_agent_plans_for_run") return [];
      return undefined;
    });

    renderPage("run-needsconf");
    expect(await screen.findByTestId("run-explanation-panel")).toBeInTheDocument();
    expect(screen.getByText(/审查待确认提案/)).toBeInTheDocument();
    expect(screen.getByText(/授予所需工具权限/)).toBeInTheDocument();
  });

  it("fixture: replayFailedRun — error, retry/inspect visible", async () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string, _args?: Record<string, any>) => {
      if (cmd === "get_agent_run") return { ...mockRun, id: "run-replayfail" };
      if (cmd === "list_agent_run_events") return replayFailedRun;
      if (cmd === "list_agent_plans_for_run") return [];
      return undefined;
    });

    renderPage("run-replayfail");
    expect(await screen.findByTestId("run-explanation-panel")).toBeInTheDocument();
    expect(screen.getAllByText(/重放失败/).length).toBeGreaterThanOrEqual(2);
    expect(screen.getByText(/查看详细 trace/)).toBeInTheDocument();
  });
});
