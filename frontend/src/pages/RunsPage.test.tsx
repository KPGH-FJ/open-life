import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import RunsPage from "./RunsPage";
import { mockInvoke } from "@/test/mocks/tauri";
import {
  successfulGovernedRun,
  agentSpecDeniedToolRun,
  replayFailedRun,
  malformedAndUnknownRun,
} from "@/test/fixtures/agentRunEvents";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("RunsPage contract", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "list_agent_runs") {
        return Promise.resolve([
          {
            id: "run-1",
            taskId: "task-1",
            sessionId: "session-1",
            status: "completed",
            kind: "conversation",
            userInput: "camel case user input",
            outputPreview: "camel case output preview",
            generatedProposals: ["proposal-1"],
            actions: [],
            observations: [],
            startedAt: new Date().toISOString(),
          },
          {
            id: "run-2",
            taskId: "task-2",
            status: "completed",
            kind: "builder",
            userInput: "deleted run",
            outputPreview: "hidden by default",
            generatedProposals: [],
            actions: [],
            observations: [],
            deletedAt: new Date().toISOString(),
            startedAt: new Date().toISOString(),
          },
        ]);
      }
      return mockInvoke(cmd, args);
    });
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("uses camelCase AgentRun fields for search, preview, proposals, and trash filtering", async () => {
    render(
      <MemoryRouter>
        <RunsPage />
      </MemoryRouter>
    );

    expect(await screen.findByText("camel case output preview")).toBeInTheDocument();
    expect(screen.getByText("1 个提案")).toBeInTheDocument();
    expect(screen.queryByText("hidden by default")).not.toBeInTheDocument();

    fireEvent.change(screen.getByPlaceholderText("搜索输入内容或输出..."), {
      target: { value: "camel case user" },
    });
    expect(screen.getByText(/camel case user input/)).toBeInTheDocument();

    fireEvent.click(screen.getByText("已删除"));
    await waitFor(() => {
      expect(screen.getByText("hidden by default")).toBeInTheDocument();
    });
  });

  // ── Batch 5: Typed hint tests ──────────────────────────────────────

  it("shows typed replay failed hint with block reason", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "list_agent_runs") {
        return Promise.resolve([
          {
            id: "run-blocked",
            taskId: "task-blocked",
            sessionId: "session-1",
            status: "completed",
            kind: "conversation",
            userInput: "test blocked action",
            outputPreview: "blocked run",
            generatedProposals: [],
            actions: [
              {
                id: "action-1",
                actionType: "tool_call",
                status: "blocked",
                input: {},
                timestamp: new Date().toISOString(),
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
            startedAt: new Date().toISOString(),
          },
        ]);
      }
      if (cmd === "list_agent_run_events") {
        return Promise.resolve([
          {
            id: "evt-replay-fail",
            runId: "run-blocked",
            eventType: "replay.failed",
            actor: "runtime",
            summary: "Replay failed",
            payload: {
              status: "failed",
              run_id: "run-blocked",
              action_id: "action-1",
              replay_of_action_id: "action-orig-1",
              block_reason: "replay_spec_missing",
              human_message: "Replay failed: missing spec",
            },
            createdAt: new Date().toISOString(),
          },
          {
            id: "evt-tool-blocked",
            runId: "run-blocked",
            eventType: "tool.call_blocked",
            actor: "runtime",
            summary: "web.search blocked",
            payload: {
              status: "blocked",
              tool_name: "web.search",
              source: "builtin",
              block_reason: "network_policy_denied",
              proposal_reason: null,
              failure_kind: null,
              agent_spec_id: "main.default",
            },
            createdAt: new Date().toISOString(),
          },
        ]);
      }
      return mockInvoke(cmd, args);
    });

    render(
      <MemoryRouter>
        <RunsPage />
      </MemoryRouter>
    );

    // Typed hints from event payloads take priority
    expect(await screen.findByText("重放失败：缺少重放规格")).toBeInTheDocument();
    expect(await screen.findByText("工具被阻断：网络策略拒绝")).toBeInTheDocument();
  });

  it("shows fallback count-based hint when no typed payload exists", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "list_agent_runs") {
        return Promise.resolve([
          {
            id: "run-needsconf",
            taskId: "task-needsconf",
            sessionId: "session-1",
            status: "completed",
            kind: "conversation",
            userInput: "test needs confirmation",
            outputPreview: "needs confirmation run",
            generatedProposals: [],
            actions: [
              {
                id: "action-1",
                actionType: "tool_call",
                status: "needs_confirmation",
                permissionDecision: "ask_every_time",
                input: {},
                timestamp: new Date().toISOString(),
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
            startedAt: new Date().toISOString(),
          },
        ]);
      }
      if (cmd === "list_agent_run_events") {
        return Promise.resolve([]);
      }
      return mockInvoke(cmd, args);
    });

    render(
      <MemoryRouter>
        <RunsPage />
      </MemoryRouter>
    );

    expect(await screen.findByText("1 个工具待确认")).toBeInTheDocument();
  });

  // ── Problem 1: typed events from runs with empty/succeeded actions ──

  it("shows typed replay failed hint even when run.actions is empty", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "list_agent_runs") {
        return Promise.resolve([
          {
            id: "run-empty-actions",
            taskId: "task-empty",
            sessionId: "session-1",
            status: "completed",
            kind: "conversation",
            userInput: "empty actions but replay failed",
            outputPreview: "replay-failed-only",
            generatedProposals: [],
            actions: [],
            observations: [],
            startedAt: new Date().toISOString(),
          },
        ]);
      }
      if (cmd === "list_agent_run_events") {
        return Promise.resolve([
          {
            id: "evt-replay-fail-empty",
            runId: "run-empty-actions",
            eventType: "replay.failed",
            actor: "runtime",
            summary: "Replay failed: NOISE_TEXT replay_spec_missing (ignore)",
            payload: {
              status: "failed",
              run_id: "run-empty-actions",
              action_id: "action-1",
              replay_of_action_id: "action-orig-1",
              block_reason: "replay_spec_missing",
              human_message: "noise: summary contains replay_spec_missing but payload drives it",
            },
            createdAt: new Date().toISOString(),
          },
        ]);
      }
      return mockInvoke(cmd, args);
    });

    render(
      <MemoryRouter>
        <RunsPage />
      </MemoryRouter>
    );

    // Must show typed hint despite empty actions
    expect(await screen.findByText("重放失败：缺少重放规格")).toBeInTheDocument();
    // Must NOT derive reason from summary/human_message text
    // (the hint text "缺少重放规格" comes from typed block_reason, not the noise in summary)
  });

  it("shows typed tool blocked hint even when all actions succeeded", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "list_agent_runs") {
        return Promise.resolve([
          {
            id: "run-succeeded-actions",
            taskId: "task-succeeded",
            sessionId: "session-1",
            status: "completed",
            kind: "conversation",
            userInput: "all succeeded but tool call blocked event exists",
            outputPreview: "succeeded-with-blocked-event",
            generatedProposals: [],
            actions: [
              {
                id: "action-1",
                actionType: "tool_call",
                status: "succeeded",
                input: {},
                timestamp: new Date().toISOString(),
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
            startedAt: new Date().toISOString(),
          },
        ]);
      }
      if (cmd === "list_agent_run_events") {
        return Promise.resolve([
          {
            id: "evt-tool-blocked-succeeded",
            runId: "run-succeeded-actions",
            eventType: "tool.call_blocked",
            actor: "runtime",
            summary: "web.search blocked by network policy (event after action succeeded)",
            payload: {
              status: "blocked",
              tool_name: "web.search",
              source: "builtin",
              block_reason: "network_policy_denied",
              proposal_reason: null,
              failure_kind: null,
              agent_spec_id: "main.default",
            },
            createdAt: new Date().toISOString(),
          },
        ]);
      }
      return mockInvoke(cmd, args);
    });

    render(
      <MemoryRouter>
        <RunsPage />
      </MemoryRouter>
    );

    // Must show typed hint from event stream, not from action status
    // Wait for run card to render first
    await waitFor(() => {
      expect(screen.getByText("succeeded-with-blocked-event")).toBeInTheDocument();
    });
    await waitFor(() => {
      expect(screen.getAllByText(/工具被阻断：网络策略拒绝/).length).toBeGreaterThanOrEqual(1);
    });
  });

  it("does not show typed reason when summary contains reason but payload lacks typed field", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "list_agent_runs") {
        return Promise.resolve([
          {
            id: "run-no-typed",
            taskId: "task-no-typed",
            sessionId: "session-1",
            status: "completed",
            kind: "conversation",
            userInput: "summary contains reason but no typed field",
            outputPreview: "no-typed-reason",
            generatedProposals: [],
            actions: [],
            observations: [],
            startedAt: new Date().toISOString(),
          },
        ]);
      }
      if (cmd === "list_agent_run_events") {
        return Promise.resolve([
          {
            id: "evt-replayfail-no-typed",
            runId: "run-no-typed",
            eventType: "replay.failed",
            actor: "runtime",
            summary: "Replay failed: replay_spec_missing",
            payload: {
              status: "failed",
              run_id: "run-no-typed",
              action_id: "a1",
              replay_of_action_id: "orig-1",
              human_message: "Error: replay_spec_missing occurred",
              // NO block_reason, NO failure_kind — summary alone must not count
            },
            createdAt: new Date().toISOString(),
          },
        ]);
      }
      return mockInvoke(cmd, args);
    });

    render(
      <MemoryRouter>
        <RunsPage />
      </MemoryRouter>
    );

    // Since payload has no valid typed reason, no "缺少重放规格" hint should appear
    await waitFor(() => {
      // The run card should be visible
      expect(screen.getByText("no-typed-reason")).toBeInTheDocument();
    });
    // "缺少重放规格" must NOT appear (it's only in the summary text, not typed payload)
    expect(screen.queryByText("缺少重放规格")).toBeNull();
  });

  // ── Batch 6: Explainability hint tests ───────────────────────────────

  it("run list explanation hint uses typed payload from getTypedRunExplanation", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "list_agent_runs") {
        return Promise.resolve([
          {
            id: "run-explain-1",
            taskId: "task-explain",
            sessionId: "session-1",
            status: "completed",
            kind: "conversation",
            userInput: "explain test",
            outputPreview: "explain test output",
            generatedProposals: [],
            actions: [],
            observations: [],
            startedAt: new Date().toISOString(),
          },
        ]);
      }
      if (cmd === "list_agent_run_events") {
        return Promise.resolve([
          {
            id: "evt-blocked",
            runId: "run-explain-1",
            eventType: "tool.call_blocked",
            actor: "runtime",
            summary: "AgentSpec blocked web.search",
            payload: {
              status: "blocked",
              tool_name: "web.search",
              source: "builtin",
              block_reason: "agent_spec_denied",
              agent_spec_id: "main.default",
            },
            createdAt: new Date().toISOString(),
          },
        ]);
      }
      return mockInvoke(cmd, args);
    });

    render(
      <MemoryRouter>
        <RunsPage />
      </MemoryRouter>
    );

    // Primary reason from run-level explanation should be shown
    expect(await screen.findByText(/AgentSpec 拒绝了工具执行/)).toBeInTheDocument();
  });

  it("misleading summary does not create false hint", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "list_agent_runs") {
        return Promise.resolve([
          {
            id: "run-misleading",
            taskId: "task-misleading",
            sessionId: "session-1",
            status: "completed",
            kind: "conversation",
            userInput: "misleading summary test",
            outputPreview: "misleading run",
            generatedProposals: [],
            actions: [],
            observations: [],
            startedAt: new Date().toISOString(),
          },
        ]);
      }
      if (cmd === "list_agent_run_events") {
        return Promise.resolve([
          {
            id: "evt-misleading",
            runId: "run-misleading",
            eventType: "tool.call_blocked",
            actor: "runtime",
            // Summary says "completed successfully" but typed payload says agent_spec_denied
            summary: "completed successfully — everything is fine (MISLEADING)",
            payload: {
              status: "blocked",
              tool_name: "web.search",
              source: "builtin",
              block_reason: "agent_spec_denied",
              agent_spec_id: "main.default",
            },
            createdAt: new Date().toISOString(),
          },
        ]);
      }
      return mockInvoke(cmd, args);
    });

    render(
      <MemoryRouter>
        <RunsPage />
      </MemoryRouter>
    );

    // Hint must come from typed payload (AgentSpec 拒绝了工具执行), not misleading summary
    expect(await screen.findByText(/AgentSpec 拒绝了工具执行/)).toBeInTheDocument();
    // Must NOT show a success hint from the misleading summary
    expect(screen.queryByText("completed successfully")).toBeNull();
  });

  it("pure typed events still produce preview hint via primary reason", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "list_agent_runs") {
        return Promise.resolve([
          {
            id: "run-pure-typed",
            taskId: "task-pure",
            sessionId: "session-1",
            status: "completed",
            kind: "conversation",
            userInput: "pure typed test",
            outputPreview: "all typed",
            generatedProposals: [],
            actions: [
              {
                id: "action-1",
                actionType: "tool_call",
                status: "succeeded",
                input: {},
                timestamp: new Date().toISOString(),
              },
            ],
            observations: [],
            startedAt: new Date().toISOString(),
          },
        ]);
      }
      if (cmd === "list_agent_run_events") {
        return Promise.resolve([
          {
            id: "evt-1",
            runId: "run-pure-typed",
            eventType: "run.completed",
            actor: "runtime",
            summary: "run completed",
            payload: { stop_reason: "no_tools" },
            createdAt: new Date().toISOString(),
          },
        ]);
      }
      return mockInvoke(cmd, args);
    });

    render(
      <MemoryRouter>
        <RunsPage />
      </MemoryRouter>
    );

    // No primary reason (success case) → fallback to no hint or fallback counts
    // (all actions succeeded, no failure events → no special hints needed)
    await waitFor(() => {
      expect(screen.getByText("all typed")).toBeInTheDocument();
    });
  });

  // ── Fixture-based explainability preview tests ─────────────────────

  it("fixture: successfulGovernedRun — no primaryReason, no misleading hint", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "list_agent_runs") {
        return Promise.resolve([
          {
            id: "run-fixture-success",
            taskId: "task-fixture-success",
            sessionId: "session-1",
            status: "completed",
            kind: "conversation",
            userInput: "fixture success test",
            outputPreview: "fixture success output",
            generatedProposals: [],
            actions: [],
            observations: [],
            startedAt: new Date().toISOString(),
          },
        ]);
      }
      if (cmd === "list_agent_run_events") return Promise.resolve(successfulGovernedRun);
      return mockInvoke(cmd, args);
    });

    render(
      <MemoryRouter>
        <RunsPage />
      </MemoryRouter>
    );

    await waitFor(() => {
      expect(screen.getByText("fixture success output")).toBeInTheDocument();
    });
    // success run has empty nextActions → no primaryReason → no hint badge
    // The primaryReason is null, so getTypedRunExplanation won't produce a hint
  });

  it("fixture: agentSpecDeniedToolRun — primaryReason visible in list", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "list_agent_runs") {
        return Promise.resolve([
          {
            id: "run-fixture-denied",
            taskId: "task-fixture-denied",
            sessionId: "session-1",
            status: "completed",
            kind: "conversation",
            userInput: "fixture denied test",
            outputPreview: "fixture denied output",
            generatedProposals: [],
            actions: [],
            observations: [],
            startedAt: new Date().toISOString(),
          },
        ]);
      }
      if (cmd === "list_agent_run_events") return Promise.resolve(agentSpecDeniedToolRun);
      return mockInvoke(cmd, args);
    });

    render(
      <MemoryRouter>
        <RunsPage />
      </MemoryRouter>
    );

    expect(await screen.findByText(/AgentSpec 拒绝了工具执行/)).toBeInTheDocument();
  });

  it("fixture: replayFailedRun — primaryReason visible in list", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "list_agent_runs") {
        return Promise.resolve([
          {
            id: "run-fixture-replayfail",
            taskId: "task-fixture-replayfail",
            sessionId: "session-1",
            status: "completed",
            kind: "conversation",
            userInput: "fixture replay fail",
            outputPreview: "fixture replay output",
            generatedProposals: [],
            actions: [],
            observations: [],
            startedAt: new Date().toISOString(),
          },
        ]);
      }
      if (cmd === "list_agent_run_events") return Promise.resolve(replayFailedRun);
      return mockInvoke(cmd, args);
    });

    render(
      <MemoryRouter>
        <RunsPage />
      </MemoryRouter>
    );

    expect(await screen.findByText(/重放动作失败/)).toBeInTheDocument();
    expect(await screen.findByText(/重放失败：缺少重放规格/)).toBeInTheDocument();
  });

  it("fixture: malformedAndUnknownRun — no crash, no misleading hint from summary", async () => {
    vi.mocked(invoke).mockImplementation((cmd: string, args?: Record<string, any>) => {
      if (cmd === "list_agent_runs") {
        return Promise.resolve([
          {
            id: "run-fixture-malformed",
            taskId: "task-fixture-malformed",
            sessionId: "session-1",
            status: "completed",
            kind: "conversation",
            userInput: "fixture malformed test",
            outputPreview: "fixture malformed output",
            generatedProposals: [],
            actions: [],
            observations: [],
            startedAt: new Date().toISOString(),
          },
        ]);
      }
      if (cmd === "list_agent_run_events") return Promise.resolve(malformedAndUnknownRun);
      return mockInvoke(cmd, args);
    });

    render(
      <MemoryRouter>
        <RunsPage />
      </MemoryRouter>
    );

    await waitFor(() => {
      expect(screen.getByText("fixture malformed output")).toBeInTheDocument();
    });
    // malformed known typed events → warning primaryReason, NOT silent success
    // Does not crash; does not infer specific block_reason from summary
    expect(await screen.findByText(/无法解析/)).toBeInTheDocument();
  });
});
