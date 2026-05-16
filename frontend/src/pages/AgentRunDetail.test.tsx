import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import AgentRunDetail from "./AgentRunDetail";
import { invoke } from "@tauri-apps/api/core";

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
    expect(screen.getByText("缺少重放规格")).toBeInTheDocument();
    expect(screen.getByText("remote_tool")).toBeInTheDocument();
    expect(screen.getByText("mcp:my-server")).toBeInTheDocument();
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
    expect(screen.getByText("AgentSpec 拒绝")).toBeInTheDocument();
    expect(screen.getByText("main.default")).toBeInTheDocument();
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
    expect(screen.getByText("缺少重放规格")).toBeInTheDocument();
  });
});
