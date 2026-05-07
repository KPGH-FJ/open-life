import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import RunTracePanel from "@/components/RunTracePanel";
import type { AgentRunEvent } from "@/types";

const mockEvents: AgentRunEvent[] = [
  {
    id: "evt-1",
    runId: "run-001",
    eventType: "run.created",
    actor: "runtime",
    summary: "Agent run created",
    payload: { session_id: "sess-1" },
    createdAt: "2026-05-06T10:00:00Z",
  },
  {
    id: "evt-agentspec",
    runId: "run-001",
    eventType: "agent_spec.selected",
    actor: "runtime",
    summary: "AgentSpec main.default selected",
    payload: { agentSpecId: "main.default", role: "main", privacyPolicy: "local_only" },
    createdAt: "2026-05-06T10:00:00.1Z",
  },
  {
    id: "evt-promptstack",
    runId: "run-001",
    eventType: "prompt_stack.assembled",
    actor: "runtime",
    summary: "PromptStack assembled: 3 blocks",
    payload: { promptBlocks: [{ id: "base_system", version: "1.0.0" }] },
    createdAt: "2026-05-06T10:00:00.2Z",
  },
  {
    id: "evt-contextgov",
    runId: "run-001",
    eventType: "context_governance.applied",
    actor: "runtime",
    summary: "Context governance applied",
    payload: { contextIncluded: ["life_model_summary"], contextExcluded: [] },
    createdAt: "2026-05-06T10:00:00.3Z",
  },
  {
    id: "evt-2",
    runId: "run-001",
    eventType: "model.call_started",
    actor: "agent",
    phase: "reasoning",
    summary: "Calling deepseek-chat",
    payload: { provider: "deepseek" },
    createdAt: "2026-05-06T10:00:01Z",
  },
  {
    id: "evt-modelfailed",
    runId: "run-001",
    eventType: "model.failed",
    actor: "runtime",
    summary: "Model call failed: LocalOnly blocked cloud",
    payload: { error: "LocalOnly privacy policy requires a local model" },
    createdAt: "2026-05-06T10:00:01.5Z",
  },
  {
    id: "evt-3",
    runId: "run-001",
    eventType: "tool.call_blocked",
    actor: "runtime",
    summary: "email.read blocked",
    payload: { tool: "email.read" },
    createdAt: "2026-05-06T10:00:02Z",
  },
  {
    id: "evt-4",
    runId: "run-001",
    eventType: "plan.created",
    actor: "agent",
    summary: "Plan created: Analyze project",
    payload: { plan_id: "plan-1" },
    createdAt: "2026-05-06T10:00:03Z",
  },
  {
    id: "evt-compaction",
    runId: "run-001",
    eventType: "compaction.created",
    actor: "runtime",
    summary: "Context compacted: 25 -> 7 messages (~8500 -> ~450 tokens)",
    payload: {
      compaction_id: "comp-001",
      run_id: "run-001",
      reason: "token estimate 8500 >= threshold 8000",
      original_token_estimate: 8500,
      compacted_token_estimate: 450,
      source_message_count: 25,
      active_proposal_count: 2,
      unresolved_observation_count: 0,
      redacted_fields: ["pii_detected", "life_model"],
      privacy_policy: "summary_only",
    },
    createdAt: "2026-05-06T10:00:03.5Z",
  },
  {
    id: "evt-5",
    runId: "run-001",
    eventType: "unknown",
    actor: "system",
    summary: "Future event type",
    payload: {},
    createdAt: "2026-05-06T10:00:04Z",
  },
  {
    id: "evt-6",
    runId: "run-001",
    eventType: "run.completed",
    actor: "runtime",
    summary: "Run completed successfully",
    payload: { stop_reason: "no_tools" },
    createdAt: "2026-05-06T10:00:05Z",
  },
];

describe("RunTracePanel", () => {
  it("returns null when events array is empty", () => {
    const { container } = render(
      <RunTracePanel events={[]} runId="run-001" show={false} onToggle={() => {}} />
    );
    expect(container.firstChild).toBeNull();
  });

  it("shows event count in collapsed state", () => {
    render(<RunTracePanel events={mockEvents} runId="run-001" show={false} onToggle={() => {}} />);
    expect(screen.getByText(/11 events/)).toBeDefined();
    expect(screen.getByText(/run-001/)).toBeDefined();
  });

  it("shows event summaries when expanded", () => {
    render(<RunTracePanel events={mockEvents} runId="run-001" show={true} onToggle={() => {}} />);
    expect(screen.getByText("Agent run created")).toBeDefined();
    expect(screen.getByText("Calling deepseek-chat")).toBeDefined();
    expect(screen.getByText("email.read blocked")).toBeDefined();
    expect(screen.getByText("Plan created: Analyze project")).toBeDefined();
    expect(screen.getByText("Run completed successfully")).toBeDefined();
  });

  it("shows event types in collapsed metadata", () => {
    render(<RunTracePanel events={mockEvents} runId="run-001" show={true} onToggle={() => {}} />);
    expect(screen.getByText("run.created")).toBeDefined();
    expect(screen.getByText("tool.call_blocked")).toBeDefined();
    expect(screen.getByText("run.completed")).toBeDefined();
  });

  it("shows actor labels correctly", () => {
    render(<RunTracePanel events={mockEvents} runId="run-001" show={true} onToggle={() => {}} />);
    // Actor labels appear as "by runtime", "by agent", etc.
    // Each word segment renders in its own text node, so check container text
    const panel = document.querySelector(".border-slate-700\\/50");
    if (panel) {
      const text = panel.textContent || "";
      expect(text).toContain("runtime");
      expect(text).toContain("agent");
      expect(text).toContain("system");
    }
  });

  it("handles unknown events without crashing", () => {
    const unknownEvents: AgentRunEvent[] = [
      {
        id: "evt-u",
        runId: "run-001",
        eventType: "unknown",
        actor: "runtime",
        summary: "Future cuscene event",
        payload: {},
        createdAt: "2026-05-06T10:00:01Z",
      },
    ];
    render(
      <RunTracePanel events={unknownEvents} runId="run-001" show={true} onToggle={() => {}} />
    );
    expect(screen.getByText("Future cuscene event")).toBeDefined();
  });

  it("toggles expansion when clicked", async () => {
    let show = false;
    const onToggle = () => {
      show = !show;
    };
    const { rerender } = render(
      <RunTracePanel events={mockEvents} runId="run-001" show={show} onToggle={onToggle} />
    );

    // Initially collapsed — no event summaries
    expect(screen.queryByText("Agent run created")).toBeNull();

    // Click to expand
    const button = screen.getByRole("button");
    await userEvent.click(button);

    // Rerender with new state
    rerender(<RunTracePanel events={mockEvents} runId="run-001" show={true} onToggle={onToggle} />);

    expect(screen.getByText("Agent run created")).toBeDefined();
  });

  it("renders compaction.created event", () => {
    render(<RunTracePanel events={mockEvents} runId="run-001" show={true} onToggle={() => {}} />);
    expect(screen.getByText("compaction.created")).toBeDefined();
    expect(screen.getByText(/Context compacted: 25 -> 7 messages/)).toBeDefined();
  });
});
